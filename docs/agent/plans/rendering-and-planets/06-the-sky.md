# Plan R06: The Sky

- **Milestone:** Rendering milestone RM3 (with R07).
- **Depends on:** [R01 Graphics platform and the engine adapter](01-graphics-platform-and-engine.md)
  (the packed cube, the offscreen point pass and the smoke harness, R01.T8.d and T9),
  [R02 Real-scale foundations and the wireframe `VIEW`](02-real-scale-view-and-wireframe.md)
  (the `view/` camera, the photometric pipeline, the PSF and the per-sprite tone curve),
  [R03 The scene subscription and bulk transport](03-scene-subscription-and-transport.md) (outbound
  binary frames and their envelope); galaxy plans
  [06](../galaxy-generation/06-stellar-stage.md) (the stellar brief, `BriefModel`, photometry),
  [09](../galaxy-generation/09-features-and-catalogue-classes.md) (feature members, for R06.T16.a,
  which is out of RM3's scope: from P09.T2.c and P09.T40)
  and [12](../galaxy-generation/12-retarded-observation-alerts.md) (`hyperion_sim::observe`).
  Through them it reads galaxy plans 03, 04, 07, 11, 13 and 15 (`hyperion-fit`). R05's
  `QualitySetting`, `ViewSettings` and `SETTINGS` carry the sky's settings (T13.f).
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/rendering-and-planets.md)): "The sky", with its subsections
  "The star field is the galaxy, not a photograph" and "The local star as a disc"; the star-field
  rows of "Performance budget" (frame time, the star cubemap's memory); the sky's parts of "Runtime
  and code shape" (the sky's list as binary frames, the reserved-kinds rule, the size class);
  "Several views in one client" for the sky cubemap's sharing and re-bake; "Two deployments, one
  scene" for the sprites' true pixel solid angle; the stars of "The view before the planets"; the
  "Exposure and photometry" item of "Testing" as it touches stars; open questions 13, 16, 17 and 19;
  the "The sky" entry of "Decisions"; step 4 of "Suggested order of attack".

## Goal

When this plan is done, the view's stars are the galaxy's own. A `sky` request, answered on the
server at bulk priority, takes an observer (the camera's galactic position), a time, the eye's field
factor and the deepest camera limit of the views open, and returns every star brighter than those
limits, up to a count budget N_max, each placed and described at its own retarded time with its
companions, its extinction from plan 07's sightline integral, and a colour and a photopic flux from
a spectral-library table. It also returns a map of the galactic band, the light of every star it did
not list, integrated along rays from the observer with extinction from a new per-population
cumulative luminosity function in the sim, and from that band and the glare of the brightest listed
stars a naked-eye limit per direction by Crumey's (2014) threshold. The client bakes the faint
majority into an `rgb9e5ufloat` cubemap, draws the bright and the near stars as pixel-integrated
sprites every frame, draws the band beneath them, thresholds each view by its own limit (the eye's
per direction, a camera's by an explicit noise-floor model), and draws each host star of the
camera's system as a limb-darkened disc of its true angular size. Nothing is authored by eye, and
the range query's stand-in that R02 draws is retired.

## Scope and non-goals

In scope:

- `hyperion_sim::sky`: the naked-eye threshold and glare, the star colour and disc tables, the
  cumulative luminosity function, the brightness envelope and the candidate skips, the layer caps,
  the census, the band map and the limit map, and the host discs.
- One function each in two built modules: a mass-first candidate walk in `galaxy::placement` and a
  cumulative extinction profile beside `horizon` in `galaxy::gas::extinction`, both bit-identical
  to what exists.
- `hyperion-fit` tasks for the colour and limb-darkening tables, with each colour row's spectrum
  sampled at R08's bake wavelengths (R08's ask).
- The `sky` request kind, its DTOs and its binary payloads on R03's frames; the server's handler,
  bulk jobs and per-cell cache.
- The client: decoding in `@hyperion/protocol`, the sky model, the per-view limits (eye and camera),
  the cubemap bake, the sprites, the band, the host discs, the low setting, and the label block's
  sky line.
- The asks of galaxy plan 06, entered in its plan, and the `sky` kind entered in plan 04's table.

Non-goals:

- The exposure model, metering, the full-screen tone-mapping pass and bloom. R02 builds the exposure
  triple and the per-sprite tone curve, R07 the full-screen AgX pass, the histogram and the glare
  pass. This plan hands R07 each disc's glare energy and draws nothing into the bloom chain itself.
- Shading of bodies by the stars, eclipses and penumbrae. R07 lights bodies; it reads this plan's
  `HostDisc` for the disc's size and limb darkening.
- Zodiacal light. The brainstorm counts it in the background inside a system with a zodiacal cloud,
  but galaxy plan 14 has no zodiacal cloud (a search of plan 14 and the sim finds none), so it waits
  for one; Risks.
- Transients. A nova or a supernova reaches the sky through plan 12's alerts, not through a re-bake.
- Atmospheric scintillation and extinction on a planet's surface (R08).
- Sensor consoles, telescopes and any magnitude limit other than the eye's and the view camera's.
- Stars in the view's DOM list. The list holds contacts, bodies and the selected target, as the
  brainstorm's accessibility section says.
- Feature members in RM3: star clusters, OB associations, globulars and the galactic centre's
  cluster. R06.T16.a is out of RM3's scope and lands with galaxy plan 09's P09.T2.c. Until
  then the field keeps their stars, spread through it, and every view says
  `CLUSTERS: NOT YET MODELLED` (decided 2026-10-05, `decision-r06-t16a-scope.md`).

## Provides

Rust paths are under `hyperion_sim` unless a crate is named. Signatures are sketches.

### `sky`

```rust
// sky::eye — Crumey 2014 and CIE 146:2002 (Design notes 2–4)
pub struct EyeObserver { /* field_factor: f64 (default 1.4), age_years: f64 (25),
    pigmentation: f64 (0.5) */ }
pub fn threshold_illuminance(eye: &EyeObserver, background: CandelasPerSquareMetre,
    background_sp_ratio: f64) -> Lux;                              // eq. 34, colour-corrected
pub fn naked_eye_limit(eye: &EyeObserver, background: CandelasPerSquareMetre,
    background_sp_ratio: f64) -> Magnitudes;                       // −2.5 log ΔI − 13.99
pub fn star_colour_offset(star_sp_ratio: f64, background: CandelasPerSquareMetre) -> Magnitudes;
pub fn veiling_luminance(eye: &EyeObserver, illuminance: Lux, angle: Degrees)
    -> CandelasPerSquareMetre;                                     // CIE general disability glare
pub const REFERENCE_SP_RATIO: f64;                                  // 2.297, B − V = 0.7
pub fn surface_brightness(luminance: CandelasPerSquareMetre) -> MagnitudesPerArcsec2;
pub const MAX_CUT_V: f64;              // 11.0; hyperion_protocol::sky re-states it (Design note 5)

// units (in hyperion-base after R04, re-exported by the sim): Lux, CandelasPerSquareMetre,
// MagnitudesPerArcsec2, SolarLuminositiesV (V-band light in L☉,V: M_V☉ = 4.81, Willmer 2018)

// sky::photometry — the interims of A3 and A4 in one place (Design note 7)
pub fn absolute_v_of_state(state: &StarState) -> Option<Magnitudes>;
pub fn is_dark_in_v(state: &StarState) -> bool;

// sky::colour — the spectral table (Design note 6)
pub struct StarColour { /* chroma: [f32; 2] (linear Rec. 709 r and g of unit luminance),
    lux_per_v0: f64, sp_ratio: f64, camera_band_mag: f64, extinction_ratio: [f64; 3],
    bake_spectrum: [f64; BAKE_WAVELENGTH_COUNT] (R08's ask; Design note 6) */ }
pub const BAKE_WAVELENGTHS_NM: [f64; BAKE_WAVELENGTH_COUNT];   // R08's 15, mirrored (Design note 6)
pub const CAMERA_ETA_SUN: f64;   // η☉, the default sensor's e⁻ per V-band photon for the Sun's row (Design note 18), ≈ 3.0
pub enum AtmosphereGrid { MainSequence, Giant, WhiteDwarf }
pub fn star_colour(teff: Kelvin, log_g: f64, grid: AtmosphereGrid) -> StarColour;
pub fn surface_gravity(mass: SolarMasses, radius: SolarRadii) -> f64;   // log₁₀ g, cgs

// sky::luminosity — the cumulative luminosity function (Design note 7)
pub struct LuminosityFunction { /* per component and layer, tabulated in M_V */ }
impl LuminosityFunction {
    pub fn light_fainter_than(&self, m_v: Magnitudes, emitted_ago: Span) -> SolarLuminositiesV;
    pub fn count_brighter_than(&self, m_v: Magnitudes, emitted_ago: Span) -> f64; // per system
    pub fn total_light(&self, emitted_ago: Span) -> SolarLuminositiesV;
}
pub struct LuminosityTables { /* every component × layer of a galaxy */ }
impl LuminosityTables { pub fn build(galaxy: &Galaxy) -> Self;   // once, at REFERENCE_TIME = +H
    pub fn age_for(&self, t: UniverseTime, emitted_ago: Span) -> Span;   // a + (t_ref − t)
    pub fn plan(galaxy: &Galaxy) -> TablesPlan;   // the job split the server runs (T5, T11.c)
    pub fn get(&self, component: ComponentId, layer: Layer) -> &LuminosityFunction;
    pub fn heap_bytes(&self) -> usize; }

// sky::envelope — the skips (Design note 8)
pub struct BrightnessEnvelope { /* per layer: brightest M_V by mass ceiling and age range */ }
impl BrightnessEnvelope { pub fn build(galaxy: &Galaxy) -> Self;   // reads the fitted sky_envelope
    pub fn brightest(&self, layer: Layer, component: ComponentId, mass_at_most: SolarMasses,
        ages: (Years, Years)) -> Option<Magnitudes>;
    pub fn mass_floor(&self, layer: Layer, component: ComponentId, faintest: Magnitudes,
        ages: (Years, Years)) -> SolarMasses; }
pub fn max_star_mass(primary_initial: SolarMasses) -> SolarMasses;  // min(2 m₁, 150) since
                                                                    // R06.T16.b (m₁ before)

// sky::caps (Design note 9)
pub struct LayerCap { /* layer, radius: LightYears, rule_bound: LightYears,
    expected_beyond: f64 */ }
pub fn layer_caps(galaxy: &Galaxy, tables: &LuminosityTables, envelope: &BrightnessEnvelope,
    observer: &Observer, cut: Magnitudes, cache: &mut NoiseCache) -> Vec<LayerCap>;
pub const CAP_RAYS: usize;                                          // 48

// sky::census (Design notes 10–13)
pub struct SkyQuery { /* observer: Observer, cut: Magnitudes, eye: Option<EyeObserver>,
    n_max: NonZeroU32, cone: Option<Cone>, exclude: Option<SystemId> */ }
pub struct SkyQueryBuilder;                                        // SkyQuery::builder(..)
impl SkyQuery { #[cfg(any(test, feature = "testing"))]
    pub fn with_caps_forced(self, radius: LightYears) -> Self; }   // brute_force_sky's census
pub struct Cone { /* axis: UnitVector, half_angle: Degrees */ }
pub struct SkyStar { /* system: SystemId, star: StarIndex, apparent: GalacticPosition,
    distance: LightYears, emitted: UniverseTime, v: Magnitudes, a_v: Magnitudes,
    colour: StarColour */ }
pub trait SkyCellCache: Sync { fn bright_subset(&self, galaxy: &Galaxy, key: CellKey,
    floor: SolarMasses, out: &mut Vec<SystemRecord>); }   // &self: interior mutability, dyn-safe
pub struct NoSkyCellCache;
pub struct SkyContext<'a> { /* tables: &'a LuminosityTables, envelope: &'a BrightnessEnvelope,
    noise: NoiseCache (the job's own), cells: &'a dyn SkyCellCache,
    sources: &'a [&'a dyn SystemSource], modifiers: &'a dyn GasModifierSource */ }
pub struct CensusPlan { /* caps: Vec<LayerCap>, cells: Vec<CellKey> (canonical order) */ }
pub fn census_plan(galaxy: &Galaxy, tables: &LuminosityTables, envelope: &BrightnessEnvelope,
    query: &SkyQuery, cache: &mut NoiseCache) -> CensusPlan;
pub fn census_cell(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, key: CellKey, query: &SkyQuery,
    out: &mut Vec<SkyStar>);
pub struct SkyCensus { /* listed: Vec<SkyStar> (by flux, then system, then star),
    overflow: Vec<SkyStar>, tallies: CensusTallies */ }
impl SkyCensus { pub fn empty() -> Self; }                    // the eye-cut pre-pass's band
pub fn merge_census(parts: Vec<Vec<SkyStar>>, n_max: NonZeroU32) -> SkyCensus;
pub struct CensusTallies { /* per layer: cells, candidates opened, accepted, listed,
    without_photometry, feature_members_absent: bool */ }

// sky::band (Design notes 14–15)
pub enum CubeFace { PosX, NegX, PosY, NegY, PosZ, NegZ }   // galactic axes, WebGPU face order
pub struct BandSpec { /* face_texels: u16 (64), steps per ray */ }
pub struct BandTexel { /* luminance: CandelasPerSquareMetre, chroma: [f32; 2],
    sp_ratio: f64, eye_limit: Option<Magnitudes> */ }
pub fn band_rows(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, query: &SkyQuery,
    census: &SkyCensus, spec: &BandSpec, face: CubeFace, rows: Range<u16>,
    out: &mut Vec<BandTexel>);

// sky::limits (Design notes 4 and 5)
pub fn eye_cut(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, observer: &Observer,
    eye: &EyeObserver) -> Magnitudes;              // coarse pre-pass, darkest texel, +0.45 +0.1
pub fn limit_map(eye: &EyeObserver, spec: &BandSpec, band: &mut [BandTexel],
    listed: &[SkyStar]);                                          // glare, then V_lim per texel

// sky::disc (Design note 16)
pub struct PowerTwo { /* c: f64, alpha: f64 */ }
impl PowerTwo { pub fn intensity(&self, mu: f64) -> f64; pub fn disc_average(&self) -> f64; }
pub fn limb_coefficients(teff: Kelvin, log_g: f64, grid: AtmosphereGrid) -> [PowerTwo; 3];
pub struct HostDisc { /* star: StarIndex, radius: Metres, teff: Kelvin, log_g: f64,
    mean_luminance: [CandelasPerSquareMetre; 3] (disc mean per channel, at the surface),
    central_luminance: [CandelasPerSquareMetre; 3] (I(1) = mean ÷ disc average),
    limb: [PowerTwo; 3] (B, V, R for the display's b, g, r), colour: StarColour */ }
pub fn host_discs(galaxy: &Galaxy, stars: &SystemStars, t: UniverseTime) -> Vec<HostDisc>;
pub fn angular_radius(radius: Metres, distance: Metres) -> Radians;     // asin(R ÷ d)
```

### In built modules

```rust
// galaxy::placement (Design note 8)
pub fn generate_cell_where(galaxy: &Galaxy, key: CellKey, keep: impl Fn(SolarMasses) -> bool,
    out: &mut Vec<SystemRecord>);   // = generate_cell then retain by mass, bit for bit
// galaxy::gas::extinction (Design note 14)
pub fn profile(field: &GasField, origin: &GalacticPosition, direction: UnitVector,
    nodes: &[LightYears], mode: NoiseMode, quality: Quality, modifiers: &[GasModifier],
    cache: &mut NoiseCache, out: &mut Vec<Magnitudes>);          // cumulative A_V at each node
```

### Protocol (`hyperion_protocol::sky`, mirrored in `@hyperion/protocol`)

- Kind `sky`: `SkyRequest { universe, observer: GalacticPosition, time, eye: Option<EyeDto>,
camera_limit_v: Option<f64>, n_max: Option<u32>, cone: Option<ConeDto>, exclude_system:
Option<SystemIdHex> }`, answered by `SkyResponse { universe, time, observer, valid_until,
cut_v, census: Vec<SkyLayerCensusDto>, listed: u32, overflow: u32, band: BandSpecDto, hosts:
Vec<HostDiscDto>, not_modelled: Vec<SkyGapDto>, bulk: BulkManifestDto }` (R03's). Large size class.
  The response also carries `stars_bytes: u64` and `band_bytes: u64`, which split the one bulk
  payload (their sum equals `bulk.bytes`), and `MAX_CUT_V` is re-stated as a protocol constant.
- One bulk payload on R03's binary frames: the stars (24 bytes each, Design note 17), then the
  band (12 bytes a texel). It is decoded only once complete, as R03 requires.
- `HostDiscDto`: the star index, radius in metres, `mean_luminance_cd_m2` and
  `central_luminance_cd_m2` per channel, the power-2 coefficients per channel, `teff_k`, `log_g`,
  the host's `StarColour` fields (`chroma`, `lux_per_v0`), and `bake_spectrum`, the colour row's
  spectrum at R08's `BAKE_WAVELENGTHS_NM` (Design note 6). The names, pinned at re-validation for
  R07 (which reads them): `HostDiscDto { star: u8, radius_m: f64, teff_k: f64, log_g: f64,
mean_luminance_cd_m2: [f64; 3], central_luminance_cd_m2: [f64; 3], limb: [PowerTwoDto; 3],
chroma: [f32; 2], lux_per_v0: f64, bake_spectrum: [f64; 15] }` with `PowerTwoDto { c: f64,
alpha: f64 }`, each array in the order B, V, R for the display's b, g, r.
- `@hyperion/protocol`: `decodeSkyStars`, `decodeSkyBand`, the payload types, `SKY_STAR_BYTES`,
  `SKY_TEXEL_BYTES`.

### Client (`apps/hyperion/src/renderer/src/view/sky/`)

`SkyModel`, `useSky(requests, cameras)`, `eyeLimitAt(model, direction, fieldFactor)`,
`cameraLimitV(sensor, exposure: ExposureTriple, fovDeg, backgroundCdM2)` with `DEFAULT_VIEW_CAMERA`,
where `ExposureTriple` is R02's `{ aperture, shutterS, iso, ndEv? }` (the argument of
`ev100FromTriple`; `ndEv` from R07.T13.c),
`decodeSky` in `view/sky/decode.worker.ts` (off the main thread, as R03 asks), `bakeSkyCube(engine,
stars, setting)`, `packRgb9e5` (TypeScript reference), `SkySprites`, `BandLayer`, `HostDiscLayer`
with `glareSources(camera, viewport): GlareSource[]` (R07's type), `DEFAULT_EYE_OBSERVER` (`{
fieldFactor: 1.4, ageYears: 25, pigmentation: 0.5 }`, the client's mirror of `EyeObserver`'s
defaults, which `EyeDto` sends and R07's glare spread reads), `skyLabel(model, view)`, `SkySettings`
(face size, sprite budget, N_max, re-bake cadence), added as R05's `ViewSettings.sky` with its
values in R05's `SETTINGS`, not a second list, and the `MemoryCategory` members `"sky-cube"` and
`"sky-scratch"`. The engine-adapter capabilities the bake needs are R01's `RenderEngine` members,
which this plan asked for: `createPackedCube`, `writePackedCubeLevelFromBuffer` (a GPU buffer copied
with `copyBufferToTexture`, not a CPU array), and `createPointSplat`, an offscreen `point-list` pass
with additive blending into an `rgba32float` target (Consumes).

### Test helpers

`crates/hyperion-sim/tests/common/sky.rs` (built in R06.T8.e): `brute_force_sky(galaxy, query,
radius)` (every system of every cell of every layer within `radius` of the observer, padded as the
range query pads, with no skip; the census it is compared with runs with every cap forced to
`radius` through `SkyQuery::with_caps_forced`), `observer_near_sun(galaxy)`,
`observer_in_nuclear_disc(galaxy)`.

## Consumes

Names are the owning plans' as they stand; where one has changed by the time this plan runs, only
the call sites here change.

- **R02:** `view/`'s camera (pose as frame plus offset, field of view, viewport, the camera's
  galactic position), the photometric module (magnitude to illuminance at V = 0 ↔ 2.54 µlx, the
  exposure triple and EV100, pre-exposure, `rgba16float`), `psfPixelWeights` and `pixelLuminance`
  (the pixel-integrated Gaussian PSF of R02's Design note 10), `starSprite.wgsl` and the per-sprite
  tone curve, which the sky's sprites reuse and do not rebuild, the interim star field (R02.T16)
  that this plan retires, the label block, `prefers-reduced-motion` handling, and the UX guide items
  R02 drafts (the view class, exposure as an instrument). Each view's role, eye or camera, is R02's
  `ViewRole` on the view's `CameraState` (Design note 5). As built (re-validated 2026-10-02):
  `CameraState` (`view/camera/state.ts`) carries `pose`, `fovDeg` and `role` but no viewport and no
  galactic position. The viewport is `Viewport { widthPx, heightPx }` with
  `pixelSolidAngle(dir, camera, viewport)` in `view/camera/projection.ts`, and the camera's galactic
  position is composed from `ViewScene.barycentre` (`GalacticPosition | null`) and the pose through
  `view/coords/position.ts` (`galacticTranslated`, `expressIn`). The photometry is
  `view/photometry/{magnitude,exposure,toneCurve}.ts`: `illuminanceLx`, `V0_ILLUMINANCE_LX`,
  `psfPixelWeights(subpixel, sigmaPx?)` (49 row-major weights), `pixelLuminance`, `ExposureTriple`,
  `ev100FromTriple`, `exposureScale`, `spriteToneCurve` (WGSL `agxSprite`) and
  `preExpose(cdPerM2, previousExposureScale)`. The sprites are the `wireframe:starSprite` material
  (`STAR SPRITES`), drawn as `instanceCount` quads that read two `vec4f` per sprite from a storage
  buffer at `@group(2) @binding(0)` (`view/wireframe/{drawList,submit}.ts`), tone-mapped per sprite
  straight to the canvas with alpha 1: **the wireframe has no HDR target** (R02.T13's deviation),
  and `RenderStyle` is `"wireframe"` alone until R07. The interim field is `view/stars/interim.ts`
  with `displays/view/useInterimStars.ts` (`useInterimStars(input: InterimStarsInput)`); its labels
  are `STAR_SOURCE` and `STARS_WITHOUT_POSITION` in `displays/view/viewRun.ts`, whose `labelLines`
  composes the label block (`ViewLabelBlock`, with its `countLine`). Reduced motion is
  `usePrefersReducedMotion` (`lib/`). R02's `starColour.ts` (white without a T_eff) is what this
  plan's colours replace.
- **R03:** outbound binary frames (`bulk::{encode_header, BinaryFrameHeader, chunk, Answer}`,
  `MAX_BINARY_FRAME_BYTES`, `BULK_QUEUED_BYTES`), `BulkManifestDto`, and on the client
  `parseBinaryFrameHeader`, `BulkAssembler` and `requestBulk`; `TestClient::next_binary()`.
  The scene's host stars' drawn positions (light-time and aberration) for the discs. Chunks are
  assembled and decoded only once complete, off the main thread (R03's Design note 11). The sky
  request's `observer` is the scene system's barycentre, `barycentreAt(place, t)` over
  `SceneSystem.place` (R03.T16), plus the ship's offset in the system's frame. As built: the
  server's seam is `BulkPayload::new(bytes)` (`Result<_, BuildBulkPayloadError>`), carried as
  `Answer { body, bulk: Some(payload) }`, whose `frames` use `chunk(payload: Bytes, request)`;
  `BulkPayload::new` still carries `expect(dead_code)` "until R06 and R09", which T11.b removes.
  `BulkManifestDto.bytes` is a JSON number. The client's request is the method
  `RequestClient.requestBulk(body, manifestOf)`, and `BulkAssembler` refuses more than
  `MAX_BULK_CHUNKS` (257) or `MAX_BULK_PAYLOAD_BYTES` (64 MiB), which the sky's 7.5 MB at N_max
  fits. `FakeWebSocket` is `test/FakeWebSocket.ts`, with `test/binaryFrames.ts` building frames.
  `SceneSystem.place` is `SystemPlace | null`, a union (`stated`, `charted`, `unknown`), and
  `barycentreAt` returns `null` for `unknown`: then no sky is asked, and `STARS` keeps
  `STARS_WITHOUT_POSITION`. A host's drawn position is `sceneAt(..).stars[i].apparentM`
  (`lib/scene/apparent.ts`, light time and aberration, system-frame metres). R03.T15's smoke check
  of a 15 MiB transfer in the real renderer waits on this plan's request, the first real bulk kind
  (R03's Risks), and is recorded by T11.b.
- **R01:** the engine adapter (HYPERION's own WebGPU renderer, R01 Design note 24), its device,
  standard WGSL with compile errors reported by material name (Design note 23), `createBuffer`,
  `createTexture`,
  `createCompute` and `dispatch`, `WGSL_CATALOGUE` (where every shader here is registered), the
  smoke harness (`just test-render`, readback by `copyTextureToBuffer`), `MemoryCategory` (which
  this plan extends), `GpuCapabilities.float32Blendable`, and the packed cube, `createPackedCube`,
  made on the device directly (a six-layer `rgb9e5ufloat` texture with the named mips, R01.T8.d as
  built) and checked by the harness's cube round trip (R01.T9.g). **Provided by R01** (its Provides,
  `RenderEngine`, built with R01.T8.d), which this plan asked for and now consumes by these names:
  - `writePackedCubeLevelFromBuffer(cube: TextureHandle, level: number, packed: BufferHandle):
void`, a `copyBufferToTexture` from a GPU buffer the pack kernel wrote, in place of (or beside)
    today's `writePackedCubeLevel(cube, level, packed: Uint32Array)`, which would force a readback
    of up to 300 MB;
  - `createPointSplat(spec: PointSplatSpec): PointSplatHandle` with `PointSplatSpec { name;
vertexWgsl; fragmentWgsl; format: "rgba32float"; blend: "additive" }` and
    `PointSplatHandle.draw(target: TextureHandle, points: BufferHandle, count: number)`: an
    offscreen `point-list` pass with additive blending into a 2D target, needing
    `float32-blendable`. Where the adapter lacks it, `createPointSplat` throws and the bake falls
    back to the CPU splat of Design note 21 (R01's comment says "a compute splat"; the choice is the
    caller's, and R06 takes the CPU one, which needs no atomics on floats).

  As built (re-validated 2026-10-02; R01's T8.a, T8.d, T8.h deviations and RM1's M1):
  - `createPackedCube(sizePx: number, mips: number, category: MemoryCategory): TextureHandle`,
    positional, under the fixed name `"packed star cube"`, usage `TEXTURE_BINDING | COPY_DST |
COPY_SRC`. `writePackedCubeLevelFromBuffer` reads six faces one after another, each row padded
    to 256 bytes (`paddedBytesPerRow(size, 4)`), `rowsPerImage` the face size; the pack kernel
    writes that layout, and a buffer without `COPY_SRC` or too small throws.
  - Compute kernels (`KernelPair { name, reference, subgroup, readback }`) and the point splat are
    plain WGSL with `main` entry points and `layout: "auto"`, not the materials' Frame/Draw
    convention (R01 Design note 23 covers materials and post-processes). A kernel must use every
    binding it declares and takes no sampler; a cube binds to it as a `2d-array` storage view at a
    level. The splat reads its points from `@group(0) @binding(0) var<storage, read>` by
    `vertex_index`, draws into a one-layer 2D `rgba32float` target with `RENDER_ATTACHMENT`, loads
    the target (so the bake clears each face by writing zeros first) and blends one, one on every
    channel, alpha included (`SPLAT_BLEND`), which a bake scratch with no meter class allows.
  - `WGSL_CATALOGUE` has material, post-process and compute entries; compute entries carry no
    `displayName`, and **no entry kind takes a point splat**. T13.h adds one.
  - `RenderEngine` has **no destroy for a buffer or a texture**: `destroyed` events are raised only
    at the engine's disposal. The bake's transient scratch (Design note 21) and T14's release of a
    view's cube need one; T13.h adds it.
  - `MemoryCategory` is `"render-targets" | "other"` in `view/engine/memory.ts`, which says later
    plans add theirs.
  - Readback of a multi-level packed cube written by a `presentation-only` kernel is refused (RM1
    M1); the harness's bake checks read back a one-level cube, or declare the pack kernel
    `bit-exact`.
  - Materials' `additive` blend is colour (src-alpha, one) and alpha (zero, one), so a sky
    material writes alpha 1 and keeps the destination alpha, as R02's sprites do.

- **R07:** R07's view camera (`VIEW_CAMERA`, `programTriple`, in R02's `exposure.ts`, R07.T13.c),
  whose deepest triple the request and the cull take and whose triple at the shown exposure the
  label's limit takes (R07.T13.e); `ExposureTriple` gains `ndEv`, applied as a transmission 2^−ndEv.
  R07's `METER_CLASS` (`hostDisc: 0`), which the disc pass writes in the HDR target's alpha, with
  every translucent sky pass (sprites, band) blending alpha as source zero, destination one so the
  class survives (R07's Design note 10), which R01's `blend: "additive"` does (R01 Design note 21,
  R01.T8.i); and R07's
  `GlareSource { direction; angularRadiusRad; excessLuminance: Rgb }` (cd/m² above 65,504), which
  `HostDiscLayer.glareSources` returns (R07's Design note 12). R06 lands before R07 and needs
  nothing of R07 at build time: where R07's `post/` module does not yet exist, R06.T13.e declares
  `METER_CLASS` and `GlareSource` there under R07's names and shapes, and R07 extends that file. R07
  consumes `HostDiscDto`, `glareSources` and `DEFAULT_EYE_OBSERVER`.
- **Galaxy plan 03:** `CellKey`, `generate_cell(galaxy, key, out)` (candidate order),
  `SystemRecord`, `cells_in_sphere(layer, &QuerySphere)` (the caller pads the sphere,
  `QuerySphere::new(centre, radius, time, pad)`), `pad_for(t, speed)`, `query::pad_speed(Layer)`
  (a `const fn`), `layer_spec`, `placement::resolve`. As built, a candidate draws its position
  (three words), its acceptance mark (one, which also picks the component) and then, if accepted,
  its mass and its age, each on its own stream keyed by the ID, so the mass word is independent of
  the rest (Design note 8; pinned by `outcome_follows_the_plans_streams_word_for_word`).
- **Galaxy plan 04:** `RequestBody`/`ResponseBody`, `REQUEST_KINDS` (with a test that every kind is
  in it), the server's `kind()` and `is_large` (exhaustive, no `_` arm),
  `compute::{CpuPool, Priority::Bulk, CancelToken, SingleFlight}`, `cache::ByteLru` (and
  `SharedByteLru`), the ±H check (`convert::query_time`, private to `convert.rs`, which T11.a
  reuses rather than copies), the reserved-kinds table (its R03 row the precedent for T1's).
- **Galaxy plan 06:** `BriefModel` (`new`, `of_member`, `of_record(galaxy, interiors, record)`,
  which panics for a centre member; `brief_at`), `SystemStars::{generate, stars, state_at,
brief_at}`, `StarModel::state_at`, `StarState` (its temperature getter is
  `effective_temperature()`), `stellar::photometry::{absolute_magnitude_v,
bolometric_correction_v}`, `premain::protostar_class`, `galaxy::fates::{fates_for,
CompanionMasses, StellarFates}` (`fates_for` returns `&'static MultiplicityFates`), the
  `system_summary` DTOs. Its asks: A1–A4 below. The server's `brief_absolute_v`
  (`convert/stellar.rs`, R02.T5) is R02's interim field's reading and leaves the sim's photometry
  to `sky::photometry`.
- **Galaxy plan 07:** `galaxy::gas::extinction::{sightline, horizon, NoiseMode, Quality}`
  (`Quality::Budget(NonZeroU32)`), `NoiseCache`, `ccm::extinction_ratio(wavelength:
Micrometres)`, `ccm::Band`, `GasModifier`, `GasModifierSource`. P07.T10.a–c (the
  `extinction_map` and `extinction` kinds, the server's `compute::sightlines::{SIGHTLINE_QUALITY,
SightlineMarcher, SharedSightlineCache}`) is on `origin/galaxy-generation` only, not on `main`
  (re-validated 2026-10-02); this plan needs none of it, and T11 reuses `SIGHTLINE_QUALITY` once it
  merges.
- **Galaxy plan 09:** `FeatureMemberSource` through the server once P09.T40 registers it (R06.T16).
  As built, neither `SystemSource` nor `FeatureMemberSource` is `Sync`, so each census job builds
  its own `SkyContext` and sources over shared caches, as `range_query`'s callers do; P09.T40 is not
  built on `main` or on `origin/galaxy-generation`, and the server's range query passes no sources.
  R06.T16.a, which reads it, is out of RM3's scope and lands with P09.T2.c (decided 2026-10-05,
  `decision-r06-t16a-scope.md`). Until P09.T2.c nothing reads φ (`FeatureShare::None`), so the field
  keeps the members' share, and `FeatureGas` is passed nowhere.
- **Galaxy plan 11:** `stellar::multiplicity::star_positions_at(&SystemHierarchy, t, out)` (its
  `out` holds `(BodyId, SystemPosition)`), `StarIndex`, the hierarchy in `SystemStars`
  (`hierarchy()`); and the rule that every star's mass is at most the pair's total (Design note
  8). **P11.T11 is built** (version 16): `SystemStars::state_at(t) -> Option<SystemState>` gives
  every star's state from its pair's timeline where the engine ran one (a merged-away star is
  `NoRemnant`, so the list keeps its length), while `stars()` and `brief_at` stay single-star. The
  census therefore reads `SystemState::stars()` for each star's state, and R06.T16.b is due now
  (its ordering note). P11.T6 and T8–T10 are not built and move no star's mass.
- **Galaxy plan 12:** `observe::{Observer, retarded, Drift::of_record, Retardation,
TraceMotionError}`; `Drift::of_record` still returns `TraceMotionError::CentreOrbitNotBuilt` for a
  centre member (P09.T28.a's propagator is unwired). P12.T6's observed mode on the wire is on
  `origin/galaxy-generation` only; this plan does not use it.
- **Galaxy plan 13:** the substellar layers' records and briefs (brown dwarfs through plan 06's
  cooling fits).
- **Galaxy plan 15:** `hyperion-fit`'s dataset and emit machinery (`data.rs`, `emit.rs`,
  `PROVENANCE.toml`), its manifests (`crates/hyperion-fit/manifests/<task>.toml`, with a
  `<task>.smoke.toml` for a slow task), `tables.lock`, `tables::MANIFEST`, `just fit <task>` and
  `just fit-check`. As built, a task is registered in `crates/hyperion-fit/src/task.rs`'s
  `REGISTRY` (a fixed-size array in name order, a test enforcing the order) as well as by its
  `pub mod` line in `tasks/mod.rs`.
- **R05:** `QualitySetting` (`"high" | "low"`), `ViewSettings`, which gains a `sky: SkySettings`
  field, and `SETTINGS`, which gains its values (T13.f), as R05 requires of later plans; and
  `AllocationTally` (R05.T11.a), which counts the sky's memory categories. **Pending R05**: R05 is
  not built and is being re-validated in parallel (2026-10-02), so these names, their file
  (`view/quality/qualitySetting.ts` in R05 as written) and T13.f are re-checked when R05 lands.
- **R08:** `BAKE_WAVELENGTHS_NM`, which the colour table samples each row's spectrum at (R08's ask,
  Design note 6).

**Named asks of galaxy plan 06**, entered in its plan by R06.T1 and consumed with an interim until
each lands. At re-validation (2026-10-02) none is built on `main` or on `origin/galaxy-generation`,
and plan 06 has no "Asked by rendering plan R06" heading yet: `wind.rs` keeps P06.T10's constant
`MODERN_LBV` beyond the limit, `absolute_magnitude_v` treats a protostar as living, and
`photometry.rs` still gives white dwarfs no V and keeps its "Table III" note. Every interim below
holds.

- **A1, `BriefModel` over the retarded interval** (open question 17): a constructor such as
  `BriefModel::new_for(galaxy, record, earliest_emitted)` whose routes are tested from the earliest
  emitted time to +H. Interim: for an emitted time outside the clock window the census reads
  `SystemStars::generate(..).brief_at`, as plan 12's observed mode does (P12.T0's finding), which is
  exact and dearer.
- **A2, the Humphreys–Davidson ruling** (open questions 13 and 17): whether the tracks keep their
  cool supergiants above log L 5.8. The census follows the tracks as they stand; the cap rule reads
  the envelope, so a ruling moves the caps with no code change here. The research lean is in Design
  note 19.
- **A3, protostars dark in V** (open question 13): Class 0/I (`protostar_class`) has no V
  magnitude. Interim: the census and the luminosity function apply the rule themselves, in one
  function, `sky::photometry::is_dark_in_v`, which A3 replaces with plan 06's.
- **A4, V magnitudes of white dwarfs and of M giants**: an absolute V for white dwarfs from the
  Montreal grids (Bédard et al. 2020) and giant bolometric corrections for late M giants (up to 1.7
  mag too bright today, `photometry.rs`'s own note); and a check of `photometry.rs`'s "Table III"
  citation of Straižys and Kuriliene, as the roadmap's asks table records. Interim: a white dwarf
  is left out of the census and counted in `without_photometry`, which the response reports; M
  giants are as bright as plan 06 says.

## Design notes

1. **Where each part runs.** Everything that decides which stars exist and how bright they are is
   sim code on the server, because it reads the galaxy: the census, the extinction, the luminosity
   function, the band and the limit map. The client decides only what each view draws of what it
   was sent: its limit, the bake, the sprites. The server therefore needs the eye's threshold, the
   colour table and the disc parameters, and the client needs none of the tables: every star and
   texel arrives with its chroma and its photopic flux already computed, and carries its eye colour
   offset and its camera band term as fields that each view applies (Design note 17). One table in
   Rust, no TypeScript copy: what a client plan needs of a host star's colour (R07, R08) travels on
   `HostDiscDto`.
2. **Crumey's threshold, eq. 34 everywhere** (researched 2026-09-29; Crumey 2014, arXiv:1405.4209,
   eqs. 5–7, 18, 26–28, 32–34, 53–55 and §1.3; checked by computation). ΔI = F (√(a₁B^½ + a₂B^¾ +
   a₃B) + a₄B^¼ + a₅B^½)² lux, a₁ = 5.949 × 10⁻⁸, a₂ = −2.389 × 10⁻⁷, a₃ = 2.459 × 10⁻⁷, a₄ = 4.120
   × 10⁻⁴, a₅ = −4.225 × 10⁻⁴, with B in cd/m²; the limit is −2.5 log₁₀ ΔI − 13.99, and μ_V =
   −2.5 log₁₀ B + 12.58. One formula from μ 15 to 25 rather than eq. 53 joined to eq. 34: eq. 34
   is within 0.02 mag of eq. 53 above μ 20, and eq. 53 alone is wrong below μ ≈ 16.7, where its
   bracket peaks and brighter skies give fainter limits; the brainstorm's 17.5 → 5.3 holds only
   under eq. 34, to about 5.25. Eq. 34 has a 0.029 mag dip over B_equiv 0.047–0.022 cd/m² (μ
   15.9–16.75 before the colour correction, about 16.3–17.2 after), Crumey's transition region
   around 7.08 × 10⁻² cd/m² where his two fits part (§2.2), an artefact of the fit rather than of
   vision. It is removed by the running minimum from the dark side, L*(μ) = min over μ′ ≥ μ of
   L(μ′), which is conservative and, since B_equiv rises with B, closed-form: for B_equiv in
   0.0216–0.0649 cd/m² the limit is 5.2446 − 2.5 log₁₀(F ÷ 1.4) (researched 2026-09-29, high
   confidence on the numbers, medium-high that the dip is an artefact). So the limit is 5.24 at μ
   16.5 and 5.25 at 17.5, where the brainstorm has 5.3 for both.
   Eq. 34 is written without F in the paper and takes it as eq. 53 does, so F moves every limit by
   exactly −2.5 log₁₀ F; the client applies a field factor other than the request's as that offset.
   Eq. 34 has no absolute threshold: as B → 0, ΔI tends to F(√a₁ + a₄)² B^½ and the limit
   diverges (V 15.3 at μ 40), and Blackwell's data constrain nothing below about 10⁻⁵ cd/m²
   (researched 2026-09-29; low confidence on that bound). So the background is clamped at μ 27
   (F = 1.4, colour-corrected: 8.64), beyond which no texel's limit deepens. _Decided 2026-10-02
   (Risks, "The eye's darkest background"): Crumey's own clamp, a colour-corrected 10⁻⁵ cd m⁻²,
   limit 7.99 at F = 1.4, reached at μ 25.6 in starlight._
3. **The colour corrections.** The background is taken to Blackwell's 2,850 K light by B_equiv =
   (ρ₀ ÷ 1.408) B, with ρ₀ the band texel's scotopic-to-photopic ratio from the colour table (2.26
   for starlight gives the brainstorm's 0.4–0.5 mag; 0.51 computed). Each star's threshold moves by
   2.5 log₁₀(ρ★ ÷ 2.297), the reference being B − V = 0.7, Cinzano's typical naked-eye star, which
   keeps F's calibration. Both corrections are scotopic and fade with the CIE 191:2010 MES2 weight
   in mesopic backgrounds (the research agent's construction, not Crumey's: under 0.03 mag of
   difference between μ 16 and 19). ρ comes from real spectra, not from B − V through eq. 18,
   which gives an M dwarf about 0.2–0.3 mag where the blackbody gives the brainstorm's 0.3–0.4.
4. **Glare from resolved stars** (researched 2026-09-29; CIE 146:2002 general disability glare via
   Vos 2003; Adrian 1989 as Crumey's "standard way"). L_veil = E [10 ÷ θ³ + (5 ÷ θ² + 0.1 p ÷ θ)(1 +
   (A ÷ 62.5)⁴) + 0.0025 p], θ in degrees clamped at 0.1°, summed over every listed star within
   100°, with E rod-weighted by ρ★ ÷ 1.408 as the background is. It is added to the band's
   luminance before the threshold. Defaults A = 25, p = 0.5 are `EyeObserver` fields. F stays 1.4:
   the glare is then modelled rather than folded into F, a small double count Risks records. The
   glare of the camera's own star and sunlit bodies is not in the map; the brainstorm names only the
   resolved stars.
5. **Two kinds of limit, one request.** A view is either the eye (the single-player cockpit window)
   or a camera (the main screen and every other view). The eye's limit per direction is the limit
   map's; a camera's is `cameraLimitV` (Design note 18). The response's `cut_v` is the deepest of
   the two over the views open. The camera's part is the request's `camera_limit_v`, clamped at
   `MAX_CUT_V`, 11.0, above which a narrow zoom would ask for 10⁶ stars (research finding); a
   deeper individual exposure asks for a cone. The eye's part cannot be the limit map's deepest
   texel, since the map is computed from the band and the listed stars, which are computed to the
   cut. It is set by the server before the census (researched 2026-09-29; Crumey 2014 eq. 34 as in
   Design note 2; Gaia DR3 flux sums for the integrated starlight: μ 24.3 at the galactic poles
   for the stars fainter than V 6.5, and 24.6 for those fainter than V 8.1
   (`decision-r06-t9b-band.md`)): a coarse band pre-pass, `band_rows` from the luminosity tables
   alone at 16² texels a face with no census and a provisional cut of 7.85, gives each texel's
   background; the eye's cut is then the colour-corrected Crumey limit at the darkest texel
   (clamped as Design note 2 says; 7.99 at most since 2026-10-02) plus the largest colour offset,
   +0.45 mag for a hot star (the colour table's largest is 0.43, at μ 30), plus a pad of 0.1 mag,
   so at most about 9.2 (8.54 under the 2026-10-02 clamp). If that cut is deeper than the
   provisional one the pre-pass runs once more at it; raising the cut removes stars from the band
   only slightly, so one repeat converges. Glare is left out of the pre-pass, which is
   conservative, since glare only makes limits shallower. Near the Sun the rule gives about 7.6 +
   0.45 + 0.1 ≈ 8.15 for the real sky. The pre-pass's darkest texel holds the light fainter than
   the cut, μ 24.73–24.77 by Gaia DR3 at b ≈ +79°, darker than the 24.3 of the light fainter than
   V 6.5. The fixture, whose poles are about 0.3 mag faint, gives about 8.27 (decided 2026-10-06,
   `decision-r06-t9b-band.md`). The fixed 7.85 alone would be too shallow wherever the band is
   darker than μ 24.3 (7.72 at μ 25, 8.17 at 26).
6. **The colour table** (researched 2026-09-29). Built by `hyperion-fit` from spectra fetched, not
   vendored (they carry no licence; only the integrated table is committed, with citations): ATLAS9
   (Castelli and Kurucz 2003) for 3,500–50,000 K, PHOENIX (Husser et al. 2013) for 2,300–3,500 K and
   log g to 6, TLUSTY OSTAR2002 above 27,500 K, Koester or Levenhagen 2017 DA spectra for white
   dwarfs to 100,000 K and TMAP beyond; a blackbody above 100,000 K. Each spectrum is integrated
   against the CIE 1931 2° functions and the CIE 1924 V(λ) and 1951 V′(λ) (CIE datasets, CC BY-SA
   4.0, credited): chroma in linear Rec. 709 with a D65 white, desaturated towards white out of
   gamut (Walker's method); `lux_per_v0`, the photopic illuminance of a V = 0 star of that spectrum
   over 2.54 µlx; ρ; the camera band term, −2.5 log₁₀(η ÷ η☉), where η = ∫S·QE·λ dλ ÷ ∫S·R_V·λ dλ
   is the default sensor's electrons per V-band photon (R_V Bessell and Murphy 2012's photonic V,
   peak 1, which gives Φ₀ of Design note 18) and η☉ = `CAMERA_ETA_SUN`, the same quantity for the
   table's 5,772 K, log g 4.438 row (so the Sun's term is 0; +0.11 at O5V, −0.70 at M2V, −2.14 at
   M6V, about −3 at 2,300 K; decision-camera-eta.md); and each display channel's A_c ÷ A_V at R_V = 3.1 through plan 07's `extinction_ratio`
   at the channel's effective wavelength for that spectrum. For R08's spectral bakes (its Design
   note 5) each row also carries `bake_spectrum`, the spectrum's average over each of
   `BAKE_WAVELENGTH_COUNT` (15) bins of R08's `BAKE_WAVELENGTHS_NM`, normalised to unit photopic
   illuminance by the same 15-bin sum (Σ 683 ȳᵢ Sᵢ Δλ = 1 lx, with bin-averaged CIE functions), so
   that a bake scales exactly by a star's illuminance in lux and `lux_per_v0` bridges back to V
   (researched 2026-09-29, medium-high confidence). Bin averages rather than point samples, because
   Balmer and TiO features would alias at points and averages conserve flux. The bins are proposed
   as fifteen of 25.33 nm over 380–760 nm, centred at 392.67 + 25.33 k nm (after Bruneton's use of
   bin midpoints; medium confidence); R08 pins its wavelengths and this table mirrors them as
   `sky::colour::BAKE_WAVELENGTHS_NM` with a test that the two agree. The values are `f64`, as R08
   asks (`StarColour.bake_spectrum: [f64; 15]`, delivered to the client on `HostDiscDto`; 120 B a
   host). No other star than a host needs one. Pickles
   1998 is the empirical check. Gravity comes from the star's own mass and radius, which plan 06
   computes, not from a luminosity-class guess, and the grid is chosen by the star's kind. That
   departs from the brainstorm's "the gravity from the brief's luminosity class": the brief has no
   gravity, and plan 06's mass and radius give it exactly, so the roadmap's corrections should carry
   it.
7. **The cumulative luminosity function.** For each density component (which fixes the age
   distribution; a population's is the sum) and each layer: per system, the V light of stars fainter
   than M_V, and the number brighter, primaries and companions both, as a table in M_V from −12 to
   +20 at 0.05 mag. It is the quadrature `mean_present_mass` does, over Gauss–Legendre panels in ln m
   whose edges are the mass function's, the fates' and the layers' breaks and, at each
   metallicity node, the masses where that node's tracks end each living phase at each
   component's age edges (R06.T5.e), at 4, 8 or 16 nodes by the panel's width, with the same
   companions (`CompanionMasses`), with the present mass replaced by
   the V light, integrated over the age distribution against each track's own segments: each phase
   of a track at a mass node is sampled at 32 ages (and at its knots), so short bright phases — the
   post-AGB crossing, the blue loops — are weighted by their duration and not missed. Binary
   evolution enters through the pair-evolved difference of R06.T5.d's fitted table
   (`sky_binary_light`): the light and colour take it in full, the counts only where it raises
   them (Design note 9). The age distribution is taken at the emitted time: one table per galaxy
   is built at the reference time t_ref = +H (+1,000 years, `CLOCK_WINDOW_H`), with snapshots at the
   light ages 0, 10³, 2 × 10³, 10⁴, 10⁵ and 2.62 × 10⁵ years, interpolated linearly in light age; a
   query at time t with light age a reads the age a + (t_ref − t), through
   `LuminosityTables::age_for` (decided 2026-10-03, `decision-r06-tables.md`). Class 0/I is dark
   (A3's interim), white dwarfs are dark until A4, and brown dwarfs follow plan 06's cooling fits.
   The tables are one per galaxy (Design note 13), cached by the server beside the galaxy.
8. **Skips, exact** (researched 2026-09-29 for the binary case; the code at `stellar/binary/mod.rs`,
   `rlof.rs`, `common_envelope.rs`; Hurley, Tout and Pols 2002 §2.7; Sana et al. 2012, Science 337,
   444). Plan 03's candidate draws its position, its acceptance mark and its component before its
   mass. The mass word is independent of all three, so `generate_cell_where` draws the mass first
   and drops a candidate below a floor without its position or density; its result is
   `generate_cell` then a mass filter, bit for bit, which a test pins. The floor comes from
   `BrightnessEnvelope::mass_floor`: the least primary initial mass m₁ whose system's brightest M_V
   at any age the cell's components can hold at the cell's emitted interval (at most about 220 years
   wide for a 128 ly cell) could pass the cut at the cell's least distance with no extinction. The
   envelope depends on no galaxy, so it is a fitted, checked-in table, the `hyperion-fit` task
   `sky_envelope` with fit-check and sim-fingerprint, stored in integer millimagnitudes rounded
   brighter so that it stays a bound (decided 2026-10-03, `decision-r06-tables.md`). It is built from the tracks at the luminosity function's mass nodes, taking each track's
   extrema over its phase segments, then made a running maximum over mass, and remains a bound
   between nodes up to a margin of 0.3 mag that a slow test with dense masses validates. It is
   indexed by `max_star_mass(m₁)`, the most massive star the system can hold. When this plan was
   written `SystemStars::generate` built every star as a single star at its initial mass, and no
   companion exceeds the primary's initial mass (`system.rs`'s `generate_with`), so
   `max_star_mass(m₁) = m₁`. Since P11.T11 (version 16, built; re-validated 2026-10-02)
   `SystemStars::state_at` evolves every pair that can interact by +H, so accretors are rejuvenated and
   main-sequence mergers take the pair's mass at a young apparent age, so a blue straggler can
   outshine every single star of mass m₁ at the cell's age; then `max_star_mass(m₁) = min(2 m₁, 150
M☉)` (mass comes only from the pair, m₁ + m₂ ≤ 2 m₁) and the age range's lower edge is taken to
   zero. The switch is one function, changed in the task that follows P11's wiring (R06.T16.b, now
   due: its ordering note puts it straight after T8.e), so `brute_force_sky` stays the oracle. The
   census reads each star's state from `SystemStars::state_at(t).stars()`, never from
   `stars()[i].state_at`, which is the single-star model. It lowers the floor by up to half and keeps up to about 2.5
   times the candidates above 0.5 M☉ (Kroupa's α = 2.3; the research's estimate, which T17
   measures), and costs little in old cells, whose giant branches already set the envelope. The
   brainstorm's second skip, by each candidate's age word before its density, is not taken: the age
   enters only through the floor's age range over the cell's components, because a per-candidate age
   test would need the component, which needs the density the skip avoids (a departure for the
   roadmap's corrections). A skip never changes an answer: `brute_force_sky` is the oracle.
   Measured 2026-10-05 (`decision-r06-census-cost.md`): the brainstorm's age skip, under every
   component over the cell's light-time interval, would pass every C–E record near the Sun, before
   and after T16.b. Any system at or above its turnoff mass can hold a companion at the turnoff,
   whose giant branches the envelope's running maximum holds: M_V −5.7 to −6.8 at 1–10 Gyr in the
   fitted table. So it is not taken, and the departure above needs no ruling. From R06.T8.g the
   per-record bound is star by star (Design note 10). `max_star_mass` stays for the cell floor.
9. **Caps, derived.** Each layer's radius is the least beyond which its expected number of stars
   brighter than the cut falls below one, from its luminosity function (whose counts take
   R06.T5.d's pair-evolved excess only, never its deficit, so the caps stay a conservative
   estimate), the density field and
   each of `CAP_RAYS` (768) rays dimming the stars of its own solid angle by its own extinction
   profile (`extinction::profile`, `Realised`, `Quality::Full`; the census lists the realised
   field's stars, and a mean field undercounts where dust is patchy), and the rule's bound by the
   least extinction over those rays (decision 2026-10-03, `decision-r06-t7-caps.md`), never beyond
   the rule's bound: the brightest M_V the envelope reaches for the layer, dimmed by that least
   extinction. The rule is the ceiling; the caps are what the census uses, and the response states
   both per layer with the expected count beyond, so that the approximation is stated. The
   brainstorm's figures (C about 3,000 ly, D about 4,300, E about 10,000 near the Sun; some 70 ly
   for A and B with protostars dark) are the benchmark's to confirm at the current generator version (19 at re-validation) (open question
   19). From R06.T7.b each layer's radius is one per ray, with the criterion summed over the rays
   (decided 2026-10-05, `decision-r06-census-cost.md`; adopted under the owner's delegation,
   `decision-r06-census-cost-signoff.md`).
10. **The census, per cell.** Cells are those of `cells_in_sphere` to each cap, padded by
    `pad_for(|t_emit − epoch|, pad_speed(layer))` as the range query pads, in canonical order. For
    each record the skip keeps: `retarded` on `Drift::of_record` (a centre member's
    `TraceMotionError` counts it in the tallies until P09.T28); the brief at the emitted time; a
    bound on the system's flux; if that bound passes the cut, `SystemStars::generate` and every
    star's state, V, position (`star_positions_at` about the system's apparent position), colour and
    one `sightline` (`Realised`, `Budget(64)`, and plan 09's feature modifiers from R06.T16.a,
    after RM3) from the apparent position to the observer. The flux bound is gated by the primary's phase
    (researched 2026-09-29; Flower 1996, ApJ 469, 355, and Martins and Plez 2006, A&A 457, 637, for
    hot stars' bolometric corrections; De Marco and Schmutz 1999, A&A 345, 163, for γ² Vel, whose O
    companion outshines its Wolf–Rayet primary in V; Siess et al. 2000 for V rising with mass on
    the main sequence, Baraffe et al. 2015 to 1.4 M☉ only). A living primary can be fainter in V than a
    lighter companion: a post-AGB star, a stripped helium star and a TP-AGB star all can. So the
    bound is per star, since each star is kept alone. Until R06.T8.g it is the envelope at
    `max_star_mass(m₁)` (n × F_env before T8.f), and a forced single's at m₁ and its own age.
    From T8.g each star takes its own mass, \[Fe/H\] and age relative to its lifetime in
    `sky_phase_envelope`, from plan 11's `hierarchy_bound`, and each pair that may have interacted
    takes plan 11's `pair_light_bound` (decided 2026-10-05, `decision-r06-census-cost.md`, under
    decision item 2's trigger). From R06.T8.f the bound is first tested before the drift, at the
    epoch position's distance less the cell's pad and offset and over the ages the light's travel
    then allows, and a star already past the cut with no extinction takes no sightline. _Corrected
    2026-10-03:_ the plan's premise that V rises with mass along the early phases at a fixed age,
    which an n × F₁ bound for young multiples needed, is false in V (R06.T6.b measured falls of up
    to at least 0.38 mag on the pre-main sequence, e.g. 5–6 M☉ at 0.52 Myr and \[Fe/H\] −2, the
    hotter star's larger bolometric correction); nothing depends on it since item 2. The census counts stars,
    not systems. A star is kept if its V is brighter than the cut, with or without the eye, and,
    for a query with a cone, if it lies inside the cone. The eye's colour offset is the views' to
    apply (Design note 20, T13.a). The eye's cut carries the largest offset already (Design note
    5), and the band subtracts at the cut, so the listing and the band share one boundary (decided
    2026-10-06, `decision-r06-t9b-band.md`; R06.T8.k). The observer's own system (`exclude`) is
    left out; its stars are discs.
11. **Merge, N_max and overflow.** Parts are merged by flux, then system ID, then star index, which
    is total, so the order of cells and jobs cannot change the answer. The brightest N_max are
    listed. The rest (the overflow) the server's band takes (Design note 15); the listed stars a
    client culls below a view's limit that client's band layer takes (Design note 20). Each star's
    light is added once, by one side, so nothing is counted twice and nothing lost. N_max defaults
    to 3 × 10⁵ (7.2 MB of payload, some 28 of R03's 262,144-byte chunks, sent one at a time) and is
    capped there; the client asks less on the low setting (Design note 22). From R06.T8.i and
    T11.d a reply may be partial (decided 2026-10-05, `decision-r06-census-cost.md`; adopted under
    the owner's delegation, `decision-r06-census-cost-signoff.md`). It states each layer's
    complete-to radius, and the band carries the rest. A partial reply lists only the stars within
    that radius in their direction, so that listing and band share one boundary and each star's
    light is still added once.
12. **The per-cell cache is monotone.** `SkyCellCache` keeps, per cell, the records at or above the
    mass floor it was built with, in candidate order. A later query whose floor is at or above the
    cached one filters the cached list; a lower floor rebuilds the cell. Records are epoch state, so
    a jump of up to 1,000 ly reuses most cells, and the cache never changes a reply (tested, as plan
    09's caches are). The server's is a `ByteLru` under `HYPERION_SKY_CACHE_MB` (default 64),
    behind a lock so that the census's parallel bulk jobs share it through `&self`, as P09.T40's
    caches do for `SystemSource`. It is not the server's existing cell cache
    (`HYPERION_CELL_CACHE_MB`): that holds whole cells from `generate_cell`, and the near-Sun caps
    enclose some 2.6 × 10⁷ systems in C to E (the brainstorm's count at version 14), where the sky
    keeps only the bright subset above each floor. From R06.T8.h the cache is keyed by the
    faintest listable magnitude at a cell's least distance, and not by a mass floor, which near the
    Sun is every cell's band edge (decided 2026-10-05, `decision-r06-census-cost.md`). T8.h also
    sets `HYPERION_SKY_CACHE_MB`'s default from one near-Sun sky's entry bytes, so that the warm
    budget holds at the default (`decision-r06-census-cost-signoff.md`).
13. **Time.** The sky is asked at a time, like every query, within ±H. The response's
    `valid_until` is the least of one Julian year and the time at which the fastest-moving listed
    star within 1 ly would move a tenth of a pixel at 1080p across 60°. The client re-requests past
    it, on a jump, and when a camera's galactic position moves so far that the nearest baked star
    shifts by a tenth of a pixel (Design note 20). Luminosity tables are one table per galaxy, at
    t_ref = +H, which serves the whole ±H window (Design note 7; decided 2026-10-03). From R06.T8.i
    and T11.d a sky may arrive as several replies, nearest first. Each states each layer's
    complete-to radius and whether it is final, the band carries the rest, and the client replaces
    its sky with each (decided 2026-10-05; Design note 11).
14. **The band's rays and the extinction profile.** The band map is a cube map on the galactic
    axes of `face_texels` (64) a face, one ray per texel centre, 24,576 rays, marched outward to the
    root cube's edge on distance nodes spaced geometrically from 0.01 ly (twelve a decade). Along
    each ray the cumulative A_V at every node comes from `extinction::profile`, a new function in
    plan 07's module beside `horizon`, which marches once from the origin with `sightline`'s pieces
    and steps (`Realised`, `Budget(256)`, as P07.T10.c's request fixes its quality) and records the
    running sum; one `sightline` per node would cost the square. It is not symmetric, as `horizon`
    is not, and is documented as an instrument's integral. Its end value equals `sightline` over
    the whole ray to rounding, which a test pins.
15. **What the band holds.** At each node, for each component and layer, the density times the
    luminosity function's light fainter than M_V = cut − DM(d) − A_V(d) at the emitted time, all of
    a layer's light beyond its cap, and then the overflow (the kept stars past N_max) splatted into
    their texels as points. The listed stars are never in the server's band: a client that culls one
    adds it to its own band layer (Design note 20), so each star's light is counted once. The census
    skips faint stars by mass without summing them, so the subtraction is exact in expectation, as
    the brainstorm says. The texel's luminance, chroma and ρ are the flux-weighted sums, using the
    luminosity function's mean colour per M_V bin, which the table carries, reddened by the dust in
    front of each node: each display channel, the photopic light and the scotopic light by its own
    A ÷ A_V, the colour table's solar row's for the band and each star's own for the overflow,
    through `StarColour::reddened` (decided 2026-10-06, `decision-r06-t9b-band.md`; R06.T9.e).
    The limit map then adds the glare (Design note 4) and gives each texel its eye limit. The band
    depends on the cut, not on the per-direction limit, so there is no loop between them: the cut
    is uniform, and a star between a texel's limit and the cut is the client's to cull and add to
    the band (Design note 20).
16. **The discs** (researched 2026-09-29; Maxted 2018, A&A 616, A39; Claret and Southworth 2022,
    VizieR J/A+A/664/A128, table3, and 2023, J/A+A/674/A63; Claret et al. 2020, J/A+A/634/A93, for
    white dwarfs). The power-2 law I(μ)/I(1) = 1 − c(1 − μ^α): the tables give g = c and h = α in
    Johnson B, V and R directly, so no conversion from Maxted's h₁, h₂ is needed. ATLAS coefficients
    above 4,000 K, PHOENIX below; clamped at 50,000 K for O stars, at the least tabulated log g for
    hot giants, at 2,300 K below, at 100,000 K for white dwarfs. The solar row (5,772 K, log g 4.5)
    gives c = 0.7837, α = 0.6893 in V and a disc average of 0.799, against the brainstorm's 80%; the
    limb reads 0.22 at μ = 0 and 0.38 at μ = 0.1, so the brainstorm's 30% holds at μ ≈ 0.05–0.1,
    where the polynomial it cites is known to be poor at the edge. The star's photopic surface
    luminance from its V flux and radius is the disc mean L̄ per channel, since flux conservation
    gives E = π L̄ (R ÷ d)² = π L̄ sin²ρ exactly for a sphere, with L̄ = ∫ I(μ) 2μ dμ = I(1)(1 − cα
    ÷ (α + 2)) (researched 2026-09-29, high confidence). `HostDisc` carries both: `mean_luminance`,
    L̄ per channel, which R07's illuminance E = π L̄ sin²ρ reads, and `central_luminance`, I(1) = L̄
    ÷ (1 − cα ÷ (α + 2)) with each channel's own c and α, which the shader multiplies by the law,
    I(μ) = I(1)(1 − c(1 − μ^α)). It also carries the host's T_eff, log g and `StarColour`. Angular
    radius is asin(R ÷ d), from plan 06's radius. The disc is drawn analytically; texels above
    `rgba16float`'s 65,504 after pre-exposure are clamped and the energy above the clamp is handed
    to R07's glare pass per channel, as one R07 `GlareSource` per disc from
    `HostDiscLayer.glareSources`.
17. **The wire.** A star is 24 bytes, little-endian: its unit direction from the observer as three
    `f32` (12; 0.012″ of rounding), its distance in light-years as `f32` (4; parallax sprites need
    it), its apparent V after extinction as `i16` millimagnitudes (2), its chroma after reddening as
    two `u16` fractions (4), its eye colour offset as `i8` centimagnitudes and its camera band term
    as `i8` in units of 1/32 mag, rounded half away from zero and saturating at −4.0 and +3.97 (2).
    A band texel is 12 bytes: luminance `f32`, chroma two `u16`, eye limit `i16`
    millimagnitudes at the request's F (`i16::MIN` where the eye was not asked), and its ρ as `u16`
    × 10⁻⁴. Stars then texels form the response's one bulk payload, announced by R03's
    `BulkManifestDto` (with the response's `stars_bytes` and `band_bytes` splitting it) and carried
    in R03's binary frames of `MAX_BINARY_FRAME_BYTES` (262,144 bytes with the header) with one
    chunk queued at a time. The client assembles the whole payload before decoding it, as R03
    requires, so nothing is drawn before the terminal response; the decoders are in
    `@hyperion/protocol`, since no wire decoding happens elsewhere, and run in a module worker
    (`view/sky/decode.worker.ts`) whose typed arrays are transferred back, as R03's Design note 11
    asks. JSON
    would be 220 B a star; binary is what lets 3 × 10⁵ fit.
18. **The camera's noise floor** (researched 2026-09-29; the CCD equation after Merline and Howell
    1995; Bessell, Castelli and Plez 1998's zero point; checked against Vida et al. 2021's measured
    limits). A camera view detects a star when its peak pixel's signal S reaches k √(S + N_b), with
    S = f_pk Φ₀ A η t 10^(−0.4 V), Φ₀ = 8.8 × 10⁹ photons s⁻¹ m⁻² in V, A the aperture from the
    view's field of view on a 36 mm sensor, N_b the sky's electrons per pixel (from the band texel)
    plus dark current and read noise, σ_r² = σ_pre² + (σ_post S_base ÷ S)², and the exposure
    triple's aperture, shutter and sensitivity from R02's exposure model, sensitivity as gain.
    Defaults: N = 1.4, t = 1/30 s, η☉ = 3.0 (`CAMERA_ETA_SUN`, 3.02 for the solar spectrum), σ_pre = 1.2 e⁻, 5 e⁻ at base ISO, f_pk = 0.35, k = 3,
    1,920 px across the 36 mm sensor (18.75 µm pixels). η is electrons per V-band-equivalent
    photon, not a quantum efficiency; a star's is η☉ × 10^(−0.4 c) with c its camera band term
    (Design note 6), and the sky's electrons use η☉. The default sensor is unfiltered
    back-illuminated silicon, QE(λ) = 0.60 (1 − e^(−α(λ) 16 µm)) over 400–1,100 nm with α from Green
    2008 (Sol. Energ. Mat. Sol. Cells 92, 1305; values via the CC0 refractiveindex.info database):
    peak 0.60, 0.45 at 800 nm, 0.13 at 950 nm. Its Sun-relative terms match Gaia's measured
    G−V(V−I) relation (Riello et al. 2021, A&A 649, A3, Table 5.7 of the EDR3 documentation) to 0.03
    mag from O5V to M6V on Pickles 1998's spectra (re-computed 2026-10-02; decision-camera-eta.md).
    A Bayer green pixel behind an IR cut would be η ≈ 0.5–0.7, about 1.6–1.8 mag shallower. At 60°
    the sky gives only about 6 e⁻ a pixel, so read
    noise sets the limit, and since the aperture is f ÷ N with f = 18 mm ÷ tan(fov ÷ 2) the signal
    grows as f². The defaults give V 9.85–10.1 at 60° over μ 22.4–24, 11.5–11.75 at 30° and
    13.4–13.6 at 13° at high gain (9.4, 11.05 and 12.9 at base ISO; re-computed 2026-10-02 with η☉
    3.02, the model reproducing the earlier figures exactly at 1.8); a 1/2.3″ sensor at 60° reaches
    only V 3.8–5.8 (at η 1.8; re-checked 2026-09-29, high confidence on the arithmetic). The model
    gives about V 6.1–6.7 for Global Meteor Network hardware (IMX291, 4 mm f/0.95, Earth's sky at μ 21, 25 fps), against
    Vida et al. 2021's measured +6.0 ± 0.5, and 7.7–8.3 for CAMS (12 mm f/1.2) against Jenniskens
    et al. 2011's +5.4 (Icarus 216, 40), so it is optimistic for old analogue cameras by about 2.5
    mag (medium confidence). A bright planet in frame takes the limit to about V 2.5–3.5 only when about 12 of 21
    stops come from gain, which raises read noise in electrons (σ_post S_base ÷ S) rather than
    cutting photons, and 9 from shutter and aperture (6.8 mag); 21 stops all from photons would give
    about −5.9. R07's view camera fixes the split (decision-r07-exposure-camera): f/1.4 throughout,
    12 stops of gain at 1/30 s from ISO 409,600 to 100, then the shutter to 1/8,000 s and an ND
    filter, which gives V 2.56 at 60° at EV100 15; darker than EV100 −6.12 the picture is pushed
    digitally and the limit stays at the high-gain figure. The sensitivity never falls below base,
    where the gain term would not hold. The brainstorm's "about V 10" is the 60° figure to 0.1 mag;
    the plan's figure is always stated with the field of view.
19. **Humphreys–Davidson, protostars and giants: the leans handed to plan 06** (researched
    2026-09-29, a physics ruling left to plan 06). The cause of the excess is in the code: under the
    default `WindRecipe::Modern` (`stellar/sse/wind.rs`) a star beyond the limit loses a constant
    1.5 × 10⁻⁴ M☉/yr, which on the cool side is below the Nieuwenhuijzen–de Jager rate it replaces
    (8.1 × 10⁻⁴ at log L 6.3 and 4,000 K), so crossing the limit slows the wind, and stripping an
    envelope takes the 2 × 10⁵ years the brainstorm found; `Hurley2000`'s own LBV term walls the
    region off. Lean for A2 (medium-high): enforce the limit through mass loss, not a photometry
    clamp, by flooring the Modern rate at the cool-side rate and adding Hurley, Pols and Tout's
    LBV term at their log L 5.78 line (Humphreys and Davidson 1979 and 1994; Davies, Crowther and
    Beasor 2018 put red supergiants at 5.5, which is left); E's brightest then reaches M_V ≈ −9.7.
    Lean for A3 (high): Class 0/I has no V magnitude (envelopes of A_V ≳ 100 for Class 0 and tens
    to about 100 for Class I; André, Ward-Thompson and Barsony 1993; Whitney et al. 2003); Class II
    takes no circumstellar term, the birth cloud's extinction being plan 07's (Baraffe et al.
    2015's M_V ≈ 4.8 for 0.75 M☉ at 0.5 Myr re-derived from the BHAC15 tracks). Lean for A4's
    giants (medium-high): a gravity-dependent BC_V, with Fluks et al. 1994 or Worthey and Lee 2011
    for M giants and Levesque et al. 2005's column for supergiants. Each is a generator-version
    change in plan 06; this plan reads whatever the envelope and photometry then give.
20. **Culling, bake and sprites.** Each view culls the stars fainter than its limit in their
    direction (eye) or overall (camera) and adds their flux into the band layer's texel, so its
    total light is kept. Of those it keeps, a star is a sprite if it is among the view's sprite
    budget brightest or its parallax across the system (206,265 × 30 au ÷ d arcseconds, the
    brainstorm's 30 au journey as the baseline, which takes every star within about 9 ly at a tenth
    of a 110″ pixel) exceeds a tenth of a pixel; the rest are baked. A sprite is drawn by R02's
    `starSprite.wgsl` with R02's `psfPixelWeights`: the pixel-integrated Gaussian of σ = 0.64 px,
    whose erf-differenced weights sum to one, so its flux is the same at every sub-pixel position
    and it cannot alias or flash, and whose luminance takes the pixel's true solid angle through
    `pixelLuminance`, so point sources brighten with resolution (R02's Design note 10). This plan
    adds only the selection, the per-frame parallax positions and instancing; a change to the PSF
    would be an ask of R02. The hand-over between bake and sprite is at the same total flux. Where
    parallax sprites outrun the budget (the nuclear disc and cluster), the bake is redone whenever
    the nearest baked star's accumulated shift reaches a tenth of a pixel.
21. **The cubemap** (researched 2026-09-29; WebGPU §26.1.3 packed formats;
    EXT_texture_shared_exponent). `rgb9e5ufloat` is filterable and copyable but neither renderable
    nor a storage format, so the bake draws the baked stars as a `point-list` with additive blending
    into an `rgba32float` scratch face through R01's `createPointSplat` (it needs
    `float32-blendable`, which R01 requests when present and never requires, and which SwiftShader
    and the UHD 620 report by probe; the development machine's RTX 3080 is read from R01's feature
    report). Where `GpuCapabilities.float32Blendable` is false the bake
    worker splats on the CPU into a `Float32Array` per face and uploads it with `createTexture`, the
    same sums in another order (to 10⁻⁶ relative). Then a compute pass divides by each texel's solid
    angle, scales by a power of two chosen so the brightest texel lands near 2¹⁵ (exact, and kept
    with the texture), builds the mips from the `f32` scratch weighted by solid angle, packs each
    level to `u32` in a GPU buffer and copies it in with `copyBufferToTexture` through R01's
    `writePackedCubeLevelFromBuffer`, with no readback. The adapter creates the cube on the device
    (a six-layer `rgb9e5ufloat` texture with its mips allocated, R01.T8.d as built) and writes every
    level itself, checked by the harness's cube round trip (R01.T9.g). A TypeScript packer is
    the reference the WGSL one is tested against. Faces are 3,072² on the high setting (about 300 MB
    with mips) and 1,024² on the low (34 MB); 3,072 is not a power of two, so its last mip step
    filters 3 × 3. The bake also holds, one face at a time and only while it runs, an `rgba32float`
    scratch with its mips (3,072² × 16 B × 4/3 ≈ 201 MB on the high setting, 22 MB on the low) and
    the packed level's staging buffer (up to 38 MB); these transients count under `MemoryCategory`
    `"sky-scratch"`, the cube under `"sky-cube"`, so R05's tally and R12 see them. For context only:
    the high setting's peak during a bake, about 540 MB, is about a twentieth of the development
    machine's 10 GiB of VRAM, while the low setting's comes from the UHD 620's shared system
    memory. Every shader of
    the sky (splat, pack, band, disc) is registered in R01's `WGSL_CATALOGUE`, so `just test-render`
    renders it.
22. **The low setting.** 1,024² faces, whose centre texel (2 ÷ 1,024 rad, 403″ = 6.7′) is larger
    than a pixel of the low setting's 720p view 60° across (168.8″ on average, 186″ at the centre, 2
    tan 30° ÷ 1,280), so faint stars are magnified into about 2.2 px, as the brainstorm's memory
    table says (researched 2026-09-29; at 1080p the same texel spans 3.25 of the centre's 124″
    pixels, and a 3,072² face's 134″ texel 1.08); the low setting pushes the sprite budget's
    magnitude fainter to compensate, within a sprite budget of 2,048 against 4,096 on the high, and
    asks an N_max of 10⁵. Those three figures are provisional starting values, not sourced: open
    question 16 leaves them open, and R06.T17 decides them from measurements. The budget is under
    0.5 ms at 720p, measured by hand and recorded.
23. **Labels and honesty.** The view's label block carries one sky line, signed off as guide
    nomenclature in R06.T15: the limit in `mag` and its kind, `STARS V 7.4 mag EYE` or
    `STARS V 9.5 mag CAM`, and, while any stand-in holds, what it is:
    `STARS: RANGE QUERY · VOLUME-LIMITED · NO EXTINCTION` (R02's stand-in, R02's Design note 16,
    until this plan lands), and what the sky leaves out as one composed note after a middle dot,
    `CLUSTERS: NOT YET MODELLED` (until R06.T16.a, after RM3, and the centre's members),
    `WHITE DWARFS: NOT YET MODELLED` (until A4), or both, `CLUSTERS AND WHITE DWARFS: NOT YET MODELLED`. The band is noted as
    `INTEGRATED STARLIGHT` in the DOM list's view notes, in the photorealistic style only. The
    flash threshold binds the stars: pixel-integrated sprites are the mechanism, and a test holds a
    moving star's summed energy within 1%.

## Tasks

T1 comes first. T2, T3 and T4 (tables) and T5 (quadrature) can then run side by side, and T6.a
with them; T6.b needs T5.a, whose mass nodes it reads. T7 needs T5 and T6; T8 needs T2, T3, T4.b
and T7, and within it T8.e follows T8.b. T9 needs T8; T9.c needs T9.e, T9.d needs T9.b, T9.f needs
T9.e, and T11.c needs T9.f. T10 needs T9, T4.b and R03's frames; T11 needs T10, with T11.c after
T11.a. The client, T12–T14, needs T10 for its types and R02's `view/`; T13's subtasks follow T12,
T13.g follows T13.b and T13.h, and T15's draft precedes T13.f, which builds to it. T16.a is out of
RM3's scope. It waits on P08.T12, P09.T2.c, P09.T23.b and P09.T40.a's feature part, and lands in
one integration with P09.T2.c (decided 2026-10-05, `decision-r06-t16a-scope.md`). T17 closes RM3's
part of this plan without it.

Re-validated 2026-10-02: P11.T11 has wired binary evolution into `SystemStars::state_at`, so
T16.b no longer waits: it follows T8.e directly and precedes T9, so that the band, the server and
the client are never built on a census that misses blue stragglers. T13.a, T13.b and T13.h read no
census and need only R01 and R02, so they can run before T12; T13.f waits on R05 as well as T15
(pending on R05); T12 and T14 take the setting's figures (N_max, re-bake cadence) as arguments
until T13.f wires them to `SETTINGS`.

Decided 2026-10-05 (`decision-r06-census-cost.md`), the census's cost work runs in this order:
T16.b; T8.f; T9.b; T9.e; T9.c–d; T8.k with T8.j; T9.f; T8.g, once plan 11's asks A and B are on
`rendering-and-planets`; T8.h; T7.b; T8.i with T11.d, after T11.a–c (T11.c on T9.f); T5.f and
T9.g before T17's goldens; then T17 (the order amended 2026-10-06, `decision-r06-t9b-band.md`).
T7.b, T8.i and T11.d waited on the owner's sign-off. A decision agent advised on it, and its
advice was adopted on 2026-10-05 under the owner's standing delegation
(`decision-r06-census-cost-signoff.md`). T8.j, the census in motion (decided 2026-10-05,
`decision-r06-pad-speed.md`), runs after T9.b–d and before T8.g.

Rust files are under `crates/hyperion-sim/src/` unless a path says otherwise.

### R06.T1 Enter the kind and the asks

Add `sky` to galaxy plan 04's reserved-kinds table under a new row for R06, with a note that its
size class is large (`is_large`), as that plan's rule requires before a kind is built. Record asks
A1–A4 in galaxy plan 06's "Risks and open points" under a heading "Asked by rendering plan R06",
each with its interim and the task here that switches from it; A4 includes the check of the "Table
III" citation. Both are drafts for the galaxy plans' owner to accept, as the roadmap's "Awaiting the
owner" says. The row follows the R03 row's form (`04-server-and-protocol.md`, the table and its
acceptance note after it). Files: `docs/agent/plans/galaxy-generation/04-server-and-protocol.md`,
`docs/agent/plans/galaxy-generation/06-stellar-stage.md`. Acceptance: `npx prettier --check` on
both; `grep -n "sky" 04-server-and-protocol.md` finds the row; `grep -c "Asked by rendering plan
R06"` finds one heading.

### R06.T2 The naked-eye threshold and glare

Build `sky::eye` (Design notes 2–4), `MAX_CUT_V`, and the units `Lux`, `CandelasPerSquareMetre`,
`MagnitudesPerArcsec2` and `SolarLuminositiesV` where they do not exist, in
`crates/hyperion-base/src/units.rs` (R04 moved `units` there; the sim re-exports it). As built,
none of the four exists; they are added with base's `unit!` macro, and `Magnitudes`' doc, "an
extinction or a colour excess", is widened to cover the absolute and apparent magnitudes this plan
gives it. `Span` and `UniverseTime` stay in `hyperion_sim::time`. Every
constant carries its equation number and source (Crumey 2014; CIE 191:2010; CIE 146:2002 via Vos
2003; Willmer 2018 for M_V☉) in its doc comment, re-checked against the paper as the Figures rule
requires. Files: `sky/{mod,eye}.rs`, `lib.rs`, `crates/hyperion-base/src/units.rs`.

Tests: eq. 53 at μ 21.83 with F = 1 gives 6.93 (Crumey's own figure) and eq. 34 agrees within 0.02
mag for μ ≥ 20; with F = 1.4 and ρ₀ = 2.26 the limits are 6.60 ± 0.03 at μ 22.4, 7.41 at 24.3, 7.72
at 25, 8.17 at 26, 8.64 at 27 and at every μ beyond it (the clamp), 6.07 at 21, 5.65 at 19.7, 5.42
at 18.8, 5.25 ± 0.01 at 17.5 and 5.24 at 16.5 (the clamp of Design note 2); F = 2 costs
0.387 mag; the limit is monotone from μ 15 to 27; the colour offset is 0 at ρ = 2.297 and
positive for a hotter star; the veiling luminance of a V = 0 star at 1° is below its value at
0.1° and follows Design note 4's formula to 10⁻⁹ relative at 0.1°, 1°, 10° and 100°, with θ below
0.1° taken as 0.1°; NaN and negative inputs are refused by type or by `Option`. Acceptance:
`cargo test -p hyperion-sim sky::eye` and `cargo test -p hyperion-base units`.

### R06.T3 The colour table

- **R06.T3.a The offline task.** `hyperion-fit` task `star_colour`: datasets `atlas9_ck04`,
  `phoenix_husser2013`, `tlusty_ostar2002`, `wd_koester_da` (or `levenhagen2017`) and `tmap`,
  fetched, with `PROVENANCE.toml` (citation, URL, date, terms as found: none stated), and `cie_cmf`
  committed (CC BY-SA 4.0, attributed); its manifest
  `crates/hyperion-fit/manifests/star_colour.toml` and, since it is slow, `star_colour.smoke.toml`;
  the task's registration in `tasks/mod.rs` and in `task.rs`'s `REGISTRY` (its length bumped, name order kept); integration on a common wavelength grid of chroma,
  `lux_per_v0` and ρ (Design note 6). Files: `crates/hyperion-fit/src/{task.rs,tasks/{star_colour,mod}.rs}`,
  `crates/hyperion-fit/data/<dataset>/PROVENANCE.toml`, `crates/hyperion-fit/manifests/`. Tests: a
  2,856 K blackbody gives ρ = 1.41 (the published figure for Illuminant A) and 5,772 K gives 2.32;
  the D65 white maps to r = g = b; an out-of-gamut chroma is desaturated towards white, never
  clipped per channel; the smoke manifest runs. Acceptance: `cargo test -p hyperion-fit
star_colour`.
- **R06.T3.b The committed table and its reader.** `just fit star_colour` writes
  `tables/star_colour.rs` with the header naming the tool, inputs and version, its entry in
  `crates/hyperion-fit/tables.lock` and in `tables::MANIFEST`; `sky::colour::{star_colour,
surface_gravity, StarColour, AtmosphereGrid}`, bilinear in log T_eff and log g within a grid,
  clamped at its edges (Design note 6's grid ranges). Tests: Pickles 1998's spectra of O5V, A0V,
  G2V, K5V, M2V, K0III and M3III, integrated by the same code, lie within Δ(u′, v′) < 0.005 of the
  table at their types' T_eff and log g; an M dwarf's chroma is less red than its blackbody's by
  0.01–0.02 in uv (the brainstorm's figure, re-checked); lux per V0 is 1 within 0.08 mag from O5 to
  M6 (Pickles against CIE 1924, as the brainstorm states). Files: `tables/{star_colour,mod}.rs`,
  `sky/colour.rs`, `crates/hyperion-fit/tables.lock`. Acceptance: `cargo test -p hyperion-sim
sky::colour` and `just fit-check`.
- **R06.T3.c The camera, reddening and bake columns.** The default sensor's response for
  `camera_band_mag` (Design notes 6 and 18: η per spectrum), relative to `CAMERA_ETA_SUN`, which the
  fit emits, from the default sensor of Design note 18 (Green 2008's k for silicon, CC0, committed
  as an input with its citation and a `NOTICE` "Data" line), each channel's `extinction_ratio`, and
  `bake_spectrum` at `BAKE_WAVELENGTHS_NM` normalised to unit photopic illuminance (Design note 6);
  the task's revision is bumped and the table re-fitted. Tests: the 15-bin illuminance of every row
  is 1 lx to 10⁻⁶ and within 1% of the exact integral; `CAMERA_ETA_SUN` lies in 2.9–3.15 and the
  5,772 K, log g 4.438 row's `camera_band_mag` is 0 to 10⁻⁹; Pickles 1998's O5V, A0V, K5V, M2V, M5V
  and M6V integrated by the same code give +0.11, +0.14, −0.30, −0.70, −1.58 and −2.14 ± 0.05; for
  Pickles dwarfs O5V–M6V the Sun-relative term lies within 0.1 mag of Riello et al. 2021's
  G−V(V−I_C) polynomial (coefficients −0.01597, −0.02809, −0.2483, 0.03656, −0.002939), V−I
  synthetic through Bessell and Murphy 2012's I; every row's term lies in [−4.0, +3.97];
  `sky::colour::BAKE_WAVELENGTHS_NM` equals R08's constant (a fixture both sides read). Files:
  `crates/hyperion-fit/data/green2008_si/{Green-2008.yml,PROVENANCE.toml}`. Acceptance:
  `cargo test -p hyperion-fit star_colour`, `cargo test -p hyperion-sim sky::colour`,
  `just fit-check`.

### R06.T4 The limb-darkening table and the host discs

- **R06.T4.a The table.** `hyperion-fit` task `limb_darkening` from J/A+A/664/A128 (`table3.dat`),
  J/A+A/674/A63 and J/A+A/634/A93, committed with their provenance (VizieR data, cited), with its
  manifest and registration; `just fit limb_darkening` writes `tables/limb_darkening.rs`, its
  `tables.lock` and `tables::MANIFEST` entries; `sky::disc::{PowerTwo, limb_coefficients}` with the
  clamps of Design note 16. Tests: the solar row gives c = 0.784 ± 0.005, α = 0.689 ± 0.005 in V
  and a disc average of 0.799 ± 0.005; I(0.1) within 0.015 of Cox 2000's polynomial; every clamp
  returns a finite row. Files: `crates/hyperion-fit/src/{task.rs,tasks/{limb_darkening,mod}.rs}` (`REGISTRY` as in T3.a),
  `crates/hyperion-fit/manifests/limb_darkening.toml`, `tables/limb_darkening.rs`, `sky/disc.rs`.
  Acceptance: `cargo test -p hyperion-fit limb_darkening`, `cargo test -p hyperion-sim sky::disc`,
  `just fit-check`.
- **R06.T4.b The host discs.** `sky::disc::{HostDisc, host_discs, angular_radius}` (Design note
  16): each star of a system at `t`, its radius, T_eff and log g from plan 06's state, its
  `StarColour`, the mean luminance per channel from its V flux and radius, the central luminance
  from the mean and each channel's disc average. Tests: the Sun from 1 au subtends 0.533° ± 0.001°
  (`angular_radius`); π × mean luminance × sin²ρ equals the illuminance from the star's V through
  2.54 µlx within 1% in V; the central luminance times the disc average returns the mean to
  10⁻¹²; a white-dwarf host takes the white-dwarf rows. Files: `sky/disc.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::disc`.

### R06.T5 The cumulative luminosity function

- **R06.T5.a Primaries.** `sky::luminosity` over one component and layer: the mass × age
  quadrature of Design note 7 on `fates.rs`'s panels, with `sky::photometry::{absolute_v_of_state,
is_dark_in_v}` (A3's and A4's interims in one place). Tests: the total light per system of the
  old thin disc's primaries in layer A agrees with a direct quadrature of the main sequence alone
  within 1%; doubling the samples per phase moves no bin above 1%; a layer-C table has a post-AGB
  tail brighter than M_V −3; a component too young for any star to have died has no remnants'
  light. Acceptance: `cargo test -p hyperion-sim sky::` passes (one filter covers both modules).
- **R06.T5.b Companions, time and tables.** Companions through `CompanionMasses` and
  `StellarFates::companion_mass_ratio_cdf`; the emitted-time buckets and their interpolation;
  per-bin mean colour and ρ; `LuminosityTables::build` and `heap_bytes`. Tests: interpolation at
  half-bucket against a table built there, within 1%; the brainstorm's star-count slope near the
  Sun, 0.49 dex a magnitude between V 5 and 6.5, recovered within 0.1 dex from the tables and the
  density field alone with no extinction (a consistency check, not a fit); build time recorded by
  the bench `sky/luminosity_tables`. Acceptance: `cargo test -p hyperion-sim sky::luminosity` and
  `cargo bench -p hyperion-sim --no-run`.
- **R06.T5.c Against realised cells (slow).** For 200 cells of each layer at the solar circle and
  in the bulge, the summed V light of every realised system, each star's state from
  `SystemStars::state_at(t).stars()` (pair-evolved since P11.T11), against the density times the
  table, within a Poisson and track-sampling interval; and, recorded but not gated, the ratio of
  realised to tabulated counts brighter than M_V = −3 and −5 per layer; a ratio above 1.3 in a
  layer whose cap is set by its bright end is a finding for T7, which then scales that layer's
  expected count beyond by the measured ratio (decision record item 3). Acceptance: `just test-slow
luminosity_matches_realised_cells` passes.
- **R06.T5.d Pair-evolved light** (decided 2026-10-03, `decision-r06-tables.md`). A hyperion-fit
  task `sky_binary_light` writes `tables::sky_binary_light` (sim-fingerprint, since-generator-version,
  fit-check), independent of the galaxy: for each cell (layer C, D or E; \[Fe/H\] −2, −1, −0.5, 0
  and +0.18; log-age bins of 0.2 dex across the layer's luminous ages up to 1.5 × 10¹⁰ yr) the mean
  per born system of the layer of (pair-evolved − single-evolved) V light, colour sums and star
  count, in 1-mag M_V bins (only those the layer populates), from ≥ 2 × 10⁴ systems a cell (2 × 10⁵
  in layer C, decided 2026-10-04) drawn
  by the generator's own laws (the primary from the default mass function within the band,
  companions and orbits from plan 11's laws, ages log-uniform within the bin), each read from
  `SystemStars::state_at(t).stars()` and, single, from each `StarModel` alone, on the fit's own
  statistical seed. `LuminosityTables::build` adds to each component bin and snapshot the
  born-weighted sum over the age bins (the same born-system weights) of the differences, linear in
  \[Fe/H\] between nodes and over the component's three Gauss–Hermite nodes, in the bin's finish
  step in `TablesPlan::assemble`, once its single-star sums hold every node. Light and colour: each
  1-mag bin's difference is spread over its 20 sub-bins in proportion to the single-star light there
  (evenly where that is zero), the differential light clamped at 0, the cumulative sums recomputed.
  Counts take only the increase: each edge holds the larger of the single and corrected counts, then
  made non-decreasing (superseding item 3's ratio scaling). The envelope and the rule's bound are
  untouched (T16.b). A non-default mass-function kind takes no correction (a deviation). Gates
  (`cargo test -p hyperion-sim sky::luminosity`, and `luminosity_matches_realised_cells`): the fit's
  born-weighted correction has a 1σ under 2% of each layer's light, and the clamped light is under
  0.5% of it, at T5.c's two sites, per layer, summed over the components there by their systems
  (decided 2026-10-04, "T5.d gate reading"); and, as a component guard, for every component bin of
  the fixture's build, its 1σ and its clamped light summed over C, D and E are under 2% and 0.5% of
  the bin's light over all its layers, each layer weighted by its share of the component's systems
  (a bin that fails takes more systems in the cells that carry its error, never a smaller
  correction); in T5.c the corrected
  tables' pair-against-single deficit per layer matches the cells' paired deficit within its
  interval, the realised light stays within the existing interval, and each layer's residual is
  recorded before and after. It does not block T8; it lands before T9.b's band gates and T17's
  goldens (no `GENERATOR_VERSION` bump while no band or census output is served or has goldens).
  The job split: `LuminosityTables::plan(galaxy) → TablesPlan`, in stages, one metallicity at a
  time (as built, 2026-10-04): each stage, an \[Fe/H\] node from the lowest, makes its track
  samples per chunk of mass nodes (`sample_jobs`, `run_samples`, `track_samples`), then adds them
  to each component bin that reads the node, one accumulation job per bin (`accumulate_jobs`,
  `run_accumulate`), into the bin's running sums (`BinSums`, from `bin_sums`), which the caller
  holds and which refuse a bin's nodes out of order; the stage's samples are then dropped.
  `assemble` finishes each bin and puts the bins in index order; the sim spawns no threads. Test
  `parallel_build_equals_serial`: any partition and order of the jobs, stages sampled ahead of
  their turn and any threads (`order::assert_order_independent`) give `build`'s bits; and
  tables at +H read at t = 0 through `age_for` equal a build at t = 0 within 10⁻³ of every bin's
  light and count. Acceptance: `cargo test -p hyperion-sim sky::luminosity`, `just fit-check`,
  `just test-slow luminosity_matches_realised_cells`. As built (2026-10-04): three fit tasks, one a
  layer (`sky_binary_light_c`, `_d` and `_e`, each its own table under the 500 kB limit), and the
  two fit gates held per site and layer; the record is in Risks ("T5.d's pair-evolved light").
- **R06.T5.e Panels and fewer nodes** (decided 2026-10-03; gates decided 2026-10-05,
  `decision-r06-t5e-gate.md`; panels and cuts re-decided the same day,
  `decision-r06-t5e-gate-2.md`). Each \[Fe/H\] node's stage takes its own mass panels: the
  global breaks (the mass function's, the fates', the layers' band edges and their companion
  kinks) and the masses at which that node's tracks (median draws) end each living phase, or
  die, at each age edge of every component reading the node, found by a fixed scan and
  bisection and independent of the components a build asks for. These replace the edges at the
  fates' fitted lifetimes, which left an old population's giant branch inside panels: 16 nodes
  a panel were off by about 1% in an old halo component's light and colour against 32, and 4
  nodes by 6–8%. `BuildOptions::FULL` (test-only) is these panels at 16 nodes a panel, 32 parts
  and 0.05 dex; the slow test `the_full_build_matches_thirty_two_nodes_a_panel_where_the_tables_are_read`
  holds it within 0.25% of 32 nodes a panel on every component bin's light and colour sums and
  on every band ray below. `BuildOptions::STANDARD` takes a Gauss–Legendre order scaled with
  each panel's width, 4 nodes on a panel at most a quarter of `MAX_PANEL_LN_MASS` wide, 8 on one
  at most half and 16 on a wider one and 32 parts; neither 16 parts (layer A's cap at cut 11
  moved one node in, leaving 1.6–2.5 expected stars beyond it under FULL) nor \[Fe/H\] at 0.1
  dex (a bias of −0.6% in layer B's band light near the Sun) was kept. Slow test
  `standard_nodes_match_the_full_build_where_the_tables_are_read` (lib), STANDARD against FULL
  on the Milky Way fixture at `REFERENCE_TIME`, every snapshot: every component bin's light and
  four colour sums, summed over its layers by their shares of its systems, within 1%; along
  each of 768 Fibonacci rays at T17's four points, at cuts 7.95 and `MAX_CUT_V` (11.0), the
  light fainter than the cut less the distance modulus out to 120,000 ly and its colour sums,
  without extinction and through each ray's realised profile, within 1%, and at the Sun behind
  a wall at 300, 1,000 and 3,000 ly within the larger of 1% and 1/(3√N) for a 64²-face texel's
  N expected systems; every layer's cap at the six points of `caps_converge_in_rays`, at both
  cuts, within one radial node, with FULL's expected count beyond STANDARD's caps under 1.5;
  and the stars brighter than V 5 and 6.5 near the Sun within 1%. A second seed's component
  bins were checked once. T5.a–T5.d's tests, `luminosity_matches_realised_cells` and
  `caps_converge_in_rays` pass on STANDARD; `doubling_the_samples_moves_no_bin_above_one_percent`
  measures FULL's phase sampling. No gate is loosened; the cost is recorded, and above 30 CPU-s
  T17 proposes the disk cache. `galaxy::fates` and `mean_present_mass` are untouched.
  Acceptance: `cargo test -p hyperion-sim -- sky::luminosity sky::caps`, `just test-slow
the_full_build_matches_thirty_two_nodes_a_panel_where_the_tables_are_read
standard_nodes_match_the_full_build_where_the_tables_are_read luminosity_matches_realised_cells
caps_converge_in_rays`, and the measurements recorded in Risks ("T5.e's panels and node cuts").
- **R06.T5.f The tables against the realised sky (new, slow; after T8.k, before T17's goldens;
  decided 2026-10-06, `decision-r06-t9b-band.md`).** Records; gates nothing at first.
  1. T5.c's paired comparison at the solar circle, on enough C and D cells that the paired
     deficit's interval is under 3 percentage points: the fit's correction (C 2.1%, D 10.3%)
     against the realised (T5.c: 9.4%, 21.0%).
  2. Eight observers on the solar circle at z☉, 45° apart in azimuth, each a census to V 8
     within 300 ly with caps forced and no eye. Per layer, record:
     - the listed light and count, against the tables' expectation;
     - the counts once as tabulated, and once with the full pair correction, deficit included,
       through a test-only read;
     - the light with and without each layer's nearest 50 ly, on both sides;
     - the eight observers' median light ratio, against the skew's expected median from a
       Monte Carlo of the tables' own types at these radii (about 0.90 at 200 ly).

  A layer is a finding for T5.d's fit (its young age bins at solar metallicity), handed to the
  tables lane with these data, if:
  - its ensemble count ratio under the full correction lies outside 1 ± 3σ;
  - its paired deficit differs from the fit's at 3σ; or
  - its median light ratio lies below the skew's expected median at 3σ.

  The band changes nothing on it. Files: `crates/hyperion-sim/tests/sky_realised.rs`,
  `sky/luminosity.rs` (the test-only read). Acceptance:
  `just test-slow tables_against_the_realised_sky`, recorded in Risks.

Files: `sky/{luminosity,photometry,binary_light}.rs`, `tables/{sky_binary_light,sky_envelope}.rs`,
`crates/hyperion-fit/src/tasks/{sky_binary_light,sky_envelope}.rs`,
`crates/hyperion-sim/benches/sky.rs`.

### R06.T6 Candidate skips

- **R06.T6.a Mass first.** `galaxy::placement::generate_cell_where` (Design note 8). Tests: for 500
  cells of every layer and ten floors, the result equals `generate_cell` filtered by mass, record
  for record; with a floor of zero it equals `generate_cell`. Files:
  `galaxy/placement/{generate,candidate}.rs`. Acceptance: `cargo test -p hyperion-sim
placement::generate` and `just ci` (every golden unchanged).
- **R06.T6.b The envelope.** `sky::envelope::{BrightnessEnvelope, max_star_mass}` with its running
  maximum and `mass_floor`, indexed by `max_star_mass(m₁)`, which returns m₁ until R06.T16.b
  (Design note 8). Tests (slow): for 10⁴ masses drawn densely in each layer and ages across each
  component, no track is brighter than the envelope; the margin of 0.3 mag is never used by more
  than 0.1 mag. (The plan's third test, that V never falls with mass along the early phases, was
  dropped, decided 2026-10-03: Design note 10's correction.) The envelope is a fitted table
  (decided 2026-10-03, `decision-r06-tables.md`): the hyperion-fit task `sky_envelope` (fit-check,
  sim-fingerprint) writes `tables::sky_envelope` from `build_with`, in integer millimagnitudes rounded
  brighter (toward −∞) with a sentinel for "dark", so it stays a bound; `BrightnessEnvelope::build`
  reads it at no cost, and the slow tests `envelope_bounds_dense_tracks` and
  `envelope_bounds_pair_states` (T16.b) test the checked-in table. Acceptance: `cargo test -p
hyperion-sim sky::envelope`, `just fit-check`, and `just test-slow envelope_bounds_dense_tracks`
  passes.

### R06.T7 Layer caps

`sky::caps::layer_caps` (Design note 9). Needs R06.T9.a (pulled forward). Tests at Milky Way
parameters, the current generator version (19), cut 7.95 (the eye's near the Sun, 7.4 + 0.45 +
0.1), as decided 2026-10-03 (`decision-r06-t7-caps.md`): at the Sun (0, 26,000, 68) and in the
nuclear disc (0, 150, 0), every cap at or inside its rule bound with 0 ≤ `expected_beyond` < 1;
near the Sun A and B under 100 ly, and D and E each within a factor of **three** of 4,300 and
10,000 ly (a sanity bracket, not a confirmation of the brainstorm's version-14 figures), while C,
whose cap rests on rare bright phases of pair channels since T5.d's count excess, keeps only its
floor, at least 1,000 ly, with its rule bound as its ceiling (decided 2026-10-04, "T5.d caps after
the pair correction"; no cap value is pinned); in
the nuclear disc E under 1,500 ly, and each of C, D and E smaller than near the Sun; a ray's last
profile node equals `sightline` (`Realised`, `Full`) to 10⁻¹². Slow test `caps_converge_in_rays`:
at six points ((0, 26,000, 68), (0, 150, 0), (26,000, 0, 68), (−18,385, −18,385, 68), (0, 8,000, 0)
and (0, 26,000, 2,000)), every layer's expected count beyond its cap, recounted with 3,072 rays and
twice the radial steps, is under 1.5; if it fails, `CAP_RAYS` rises to 1,536 or 3,072, never the
gate. The measured caps are recorded in the doc comment and in the notes of R06.T17 for open
question 19. Decided 2026-10-03 (`decision-r06-tables.md`): the rays are split into chunks the
server runs as pool jobs, each with its own `NoiseCache`, the count serial in ray order (the bits
unchanged); the caps are re-recorded after R06.T5.d and R06.T5.e; item 3's ratio scaling is
replaced by T5.d's count excess. Files: `sky/caps.rs`, `galaxy/gas/extinction.rs` (`profile`). Acceptance: `cargo test
-p hyperion-sim sky::caps`, `cargo test -p hyperion-sim gas::extinction` and `just test-slow
caps_converge_in_rays`.

- **R06.T7.b Caps by direction (new; after T8.g; signed off).** Decided 2026-10-05
  (`decision-r06-census-cost.md`). The sign-off was advised by a decision agent and adopted on
  2026-10-05 under the owner's standing delegation (`decision-r06-census-cost-signoff.md`,
  question 3). Design note 9's criterion is unchanged: under one expected star brighter than the
  cut beyond the caps, per layer, summed over the sky. Its radius becomes one per ray:
  - For each of the 768 rays, the radius is the outermost radial interval whose expected listable
    stars per system are at least λ. λ is the largest value that keeps the layer's expected count
    beyond under 1.
  - Each ray's radius is then raised to the largest of its neighbours within twice the ray spacing.
  - `plan_cells` opens a cell when its padded box meets the cone of a ray within the ray spacing,
    at less than that ray's radius.
  - The reply carries the radii per ray. T9's band and the client read them. The band and the
    listing share that boundary: T9.b reads the same widened per-ray radii that `plan_cells` used,
    interpolated identically.

  Measured with the lane's tables (`decision-r06-census-cost.md`):

  | Near the Sun                                 | C           | D           | E           |
  | -------------------------------------------- | ----------- | ----------- | ----------- |
  | Systems opened, % of the sphere's            | 86%         | 72%         | 25%         |
  | Expected count beyond, sphere → by ray       | 0.61 → 0.20 | 0.84 → 0.38 | 0.46 → 0.27 |
  | Expected stars gained beyond the sphere      | 0.51        | 0.78        | 0.44        |
  | Expected stars dropped that the sphere lists | 0.11        | 0.32        | 0.25        |

  E's ray radii have a median of 6,146 ly, a 90th percentile of 14,563 ly and a maximum of
  61,341 ly.

  The harness also measures visibility-based caps, at the eye's cut and at the camera's. They
  count the stars brighter than the pre-pass's per-texel limit plus the colour offset and the pad,
  rather than the uniform cut; each ray takes the deepest pre-pass limit within its cone. Their
  safety test: no texel of the final limit map, with glare, is deeper than the limit its ray's cap
  was counted at. They are adopted, with no further sign-off, if they pass that test and
  `caps_converge_in_rays` and open at least 20% fewer systems near the Sun at either cut.

  Tests:
  - `caps_converge_in_rays` at all six points with the per-ray caps: the expected count beyond
    them, recounted with 3,072 rays and twice the radial steps, is under 1.5. If it fails, the
    neighbour widening grows, never the gate.
  - T7's tests per layer, on each ray's radius.
  - The identity tests, with forced caps, are unchanged.
  - No listed star lies beyond its own ray's widened radius, beyond the cell overshoot the final
    reply already allows.
  - Near the Sun the systems opened are at most 75% of the sphere's, and E's at most 35%.

  The bench records the realised stars that the spherical caps list and the rays drop, and those
  the rays gain, with no gate. A realised drop far above the expected 0.1–0.3 a layer is a finding
  on the tables.

  Files: `sky/caps.rs`, `sky/census/query.rs`. Acceptance: `cargo test -p hyperion-sim sky::caps
sky::census::query`, `just test-slow caps_converge_in_rays`.

### R06.T8 The census

- **R06.T8.a Query and plan.** `SkyQuery`, its builder (observer within the cube and ±H, cut finite
  and at most `MAX_CUT_V`, `n_max` at most 3 × 10⁵, a cone of half-angle in (0°, 90°]),
  `SkyQuery::with_caps_forced` (test builds only), `SkyContext`, `census_plan`. Tests: every
  refusal names its field; the plan's cells are a superset of those whose padded box meets each
  cap's sphere, in canonical order; a cone keeps only cells whose box meets the cone; forced caps
  replace every layer's. Acceptance: `cargo test -p hyperion-sim sky::census::query`.
- **R06.T8.b One cell.** `census_cell` (Design note 10), with the retardation, brief (A1's
  interim), the phase-gated flux bound, companions, positions, colour, extinction and the kept
  test. Each star's state is `SystemStars::state_at(t_emit).stars()[i]` (the pair-evolved state,
  P11.T11), its position from `star_positions_at`'s `(BodyId, SystemPosition)` rows; the brief is
  `BriefModel::new` (`of_member` once T16.a brings members); a centre member is caught by
  `Drift::of_record`'s `TraceMotionError` before any brief is built, since `BriefModel::of_record`
  panics for one. The tables' light ages are read through `LuminosityTables::age_for(t, a)`
  (one table per galaxy at t_ref = +H, decided 2026-10-03). Tests: the observer's own system is absent; a system whose primary is a white
  dwarf beside a bright companion lists the companion; a system whose primary is post-AGB beside a giant
  companion takes the envelope bound and lists the companion; a centre member is tallied, not
  listed. Acceptance: `cargo test -p hyperion-sim sky::census::cell`.
- **R06.T8.c Merge.** `merge_census`, `SkyCensus` (with `empty`), `CensusTallies`. Tests: any split
  of the cells into parts, in any order, gives the same bits (`order::assert_order_independent`);
  N_max keeps the brightest; overflow plus listed equals the unbounded census. Acceptance: `cargo
test -p hyperion-sim sky::census::merge`.
- **R06.T8.d The cell cache.** `SkyCellCache`, `NoSkyCellCache`, the monotone rule (Design note
  12). Tests: a query after a looser one and after a tighter one, with and without the cache, give
  the same bits; the cache is never read for a lower floor than it holds. Acceptance: `cargo test -p
hyperion-sim sky::census::cache`.
- **R06.T8.e The oracle.** `crates/hyperion-sim/tests/common/sky.rs` with `brute_force_sky(galaxy,
query, radius)`, `observer_near_sun` and `observer_in_nuclear_disc` (Test helpers), and the
  identity tests: with caps forced to the radius, the census and the brute force agree star for
  star and bit for bit on a 1,000 ly sphere near the Sun and a 200 ly sphere in the nuclear disc.
  Files: `crates/hyperion-sim/tests/{common/sky,sky_census}.rs`. Acceptance: `cargo test -p
hyperion-sim --test sky_census`.
- **R06.T8.f Census cost: the cheap exact steps (new; after T16.b, before T9.b).** Decided
  2026-10-05 (`decision-r06-census-cost.md`). Each step leaves every listed star's bits unchanged.
  `brute_force_sky` is the oracle.
  1. **Each star is cut before its sightline.** A star whose M_V + DM(d) exceeds the cut (plus its
     own colour offset until R06.T8.k) takes no `sightline`, since A_V ≥ 0. Today's sightlines
     cost 14,100 CPU-s near the Sun.
  2. **An O(1) cell floor.**
     - `BrightnessEnvelope` keeps, per mass node, its brightest magnitude over every age bin. The
       floor's question, ages 0 to `MAX_AGE_YEARS`, then reads one value a node, bit for bit the
       same as the scan.
     - `cell_offset_bound` reads a per-galaxy, per-layer table of `tidal_radius_bound_within` on
       galactocentric distance nodes. The table gives the bound at the first node at or beyond the
       cell's farthest corner, so it is never smaller than the exact one.
     - Today `cell_floor` costs 40–51 µs a cell, 5,100 CPU-s near the Sun. Gate: at most 2 µs a
       cell.
  3. **The record's offset is its cell's** (`cell_offset_bound`, computed once per cell) and no
     longer its own tidal radius. The floor and the bound may fall slightly; that moves tallies,
     not stars.
  4. **No n factor.** The bound is the brightest single star's, since each star is kept alone:
     `flux_bound` drops its 2.5 log₁₀ n, and so does `cell_floor`. This is exact while the census
     keeps stars one by one, as Design note 10 says it does. If it ever lists an unresolved
     system's blended light, the bound returns to a flux sum over the system's stars.
  5. **A bound before the drift.** Before `Drift::of_record`, the flux bound is tested at:
     - the epoch position's distance, less the cell's pad and offset;
     - the ages across the light-time interval that distance allows.

     A record that fails skips the drift, `retarded` and generation.

  6. **The plan streams its cells.** `CensusPlan` keeps the caps and each layer's sphere. Its cells
     come from an iterator in canonical order, with a count, and jobs take them in chunks of the
     walk's x-slabs. No `Vec<CellKey>` is held: near the Sun one would be 2.2 GB.
  7. **The bench samples.** `HYPERION_SKY_BENCH_SAMPLE=k` censuses the cells whose key hash is 0
     modulo k, using a fixed mixer of the layer and coordinates. It prints the tallies and CPU
     scaled by k, labelled as an estimate. Every census bench also prints each layer's records,
     generated and generated share.

  Tests:
  - `a_cells_census_equals_its_unskipped_records` and T8.e's identity tests, unchanged;
  - over the stars of 10⁴ generated systems, the sightline cut never drops a star that the cut
    would keep;
  - the O(1) floor equals `mass_floor`'s scan bit for bit at 10⁴ random queries;
  - the offset table is at least the exact bound at 10⁴ random cells;
  - the pre-drift bound never rejects a record whose post-drift bound passes;
  - streamed cells equal `plan_cells`' for the queries of T8.a's tests.

  Files: `sky/census/{cell,query}.rs`, `sky/envelope.rs`, `benches/sky.rs`. Acceptance: `cargo
test -p hyperion-sim sky::census sky::envelope`, `cargo test -p hyperion-sim --test sky_census`,
  `just test-slow the_census_is_its_oracle_1000_ly_from_the_sun`, `just ci`. Record the sampled
  near-Sun cold figure. About 1.64 × 10⁶ CPU-s is expected: T8.f saves some 20,000 now and matters
  after T8.g. Added by the lane (decided 2026-10-05 with the task, from T16.b's open items): the
  floor of a layer whose systems are all single, the brown dwarfs', reads each primary's own mass,
  not 2 m₁, as its flux bound does. As built: Risks, "Deviations in T8.f, as built".

- **R06.T8.j The census in motion (new; after T9.b, before T8.g; no output moves).** Decided
  2026-10-05 (`decision-r06-pad-speed.md`). The census pads its cells (`layer_walk`) and its
  bound before the drift (`CellReach`) by `pad_speed(layer)`, as the range query does. But no
  record has ever moved in its tests:
  - its identity tests and benches run in the fixture galaxy, which is built without kinematic
    tables;
  - its oracle opens the census's own cells, so it could not see a record that outruns the pad.

  This task tests the census in a galaxy whose systems move.
  1. `the_census_plan_holds_every_record_its_caps_see` (unit, `sky::census::cell`).
     - The galaxy is the Milky Way fixture's seed, built `with_full_potential` once for the
       module's tests in a `OnceLock`. T8.f's moving-galaxy test then shares it.
     - The observers stand at the Sun at the epoch, at +H and at −H, and 250 ly from the Sun at
       +900 years.
     - Caps are forced to A 24, B 48, brown dwarfs 48, C 96, D 192 and E 384 ly.
     - Each layer's cells are walked independently of `pad_speed`: to the cap plus `pad_for` at
       5,000 km/s over the earliest emitted time. They are generated whole.
     - For every record:
       - its speed is below its layer's `pad_speed`;
       - its displacement from its epoch position, at the emitted time and at the observer's time
         (the retardation's first guess), is at most its cell's `CellReach` pad;
       - if its apparent position lies within its layer's cap, its cell is among `plan_cells`'.
     - Some 10⁴ records are expected, in a few seconds after the galaxy's build.
  2. (slow) `the_census_is_its_oracle_150_ly_from_the_sun_in_motion` (`tests/sky_census.rs`):
     T8.e's 150 ly identity test at cut 11.0, in the moving galaxy, with the observer at +H.
     Every assertion is unchanged.
  3. `CellReach::of`'s doc comment names P08.T17's assertion.

  Files: `sky/census/cell.rs`, `tests/sky_census.rs`, `tests/common/sky.rs` (a moving galaxy, and
  an observer near the Sun at a given time) and `06-the-sky.md`. Acceptance:
  `cargo test -p hyperion-sim sky::census`, `cargo test -p hyperion-sim --test sky_census`,
  `just test-slow the_census_is_its_oracle_150_ly_from_the_sun_in_motion`, `just ci`. No
  GENERATOR_VERSION bump.

- **R06.T8.k One boundary (new; after T9.b, before T8.g, with or after T8.j; decided
  2026-10-06, `decision-r06-t9b-band.md`).** `kept_to` is the cut, `faintest_listable` drops
  `EYE_OFFSET_BOUND_MAG` (removed), and the per-star cut before the sightline is the cut alone.
  A cone's census keeps only the stars inside the cone. `brute_force_sky` follows. The eye
  still sets the cut (T9.d) and the views' culls.

  Tests:
  - a census with the eye equals one without it at the same cut, bit for bit;
  - every kept star is brighter than the cut, and inside the cone when there is one;
  - T8.e's identity tests and T16.b's pinned systems pass, unchanged in form;
  - T9.b's conservation tests run with the eye asked, and give the bits they give without it;
  - for a cone, the listed and band light together equal the full sky's inside the cone, within
    1%.

  Record each layer's listed and generated counts near the Sun, before and after. Files:
  `sky/census/{cell,query}.rs`, `tests/common/sky.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::census sky::band` and
  `cargo test -p hyperion-sim --test sky_census`.

- **R06.T8.g Census cost: a bound star by star (new; after T8.f and plan 11's asks A and B).**
  Decided 2026-10-05 (`decision-r06-census-cost.md`), under decision item 2's trigger. Near the
  Sun, the multiple-system bound left 98% of the census in generation. The per-record bound becomes
  one per star, computed before generation. The order is:
  1. the record's composition (`draw_metallicity`);
  2. plan 11's `hierarchy_bound`: every star's initial mass and each star–star pair's least
     periastron, over every attempt the generator can keep;
  3. each pair's `pair_light_bound` over the record's light-time ages: `Detached`, `Remnants` or
     `Bright(M)`;
  4. for each single star and each star of a `Detached` pair, the new fitted table
     `sky_phase_envelope` at the star's own mass interval and \[Fe/H\] interval, at its age
     relative to its lifetime, and at its own η draw (read from its `StarDraws`). The relative age
     is age ÷ `FittedFates::lifetime_bracket` (P06.T38.d) at that η, and the table is read over
     the bracket's whole span.

  A `Bright` pair gives M for any of its stars. A `Remnants` pair gives nothing while white dwarfs
  are dark (ask A4), and afterwards the table's white-dwarf rows. The system is generated only if
  some star's bound is listable at the system's nearest distance. The test runs first at the epoch
  distance less the pad and offset (T8.f's step 5), then after `retarded`. `max_star_mass`'s
  widened envelope stays for the cell floor and for any pair plan 11 cannot bound.

  `sky_phase_envelope` is a hyperion-fit task, with fit-check and sim-fingerprint, depending on no
  galaxy:
  - Each cell is a mass interval of the envelope's nodes × a \[Fe/H\] interval of `FE_H_NODES` ×
    an η interval × a relative-age bin. Bins are fine (10⁻³) over 0.8–1.05 and coarse elsewhere.
    Beyond the bracket only remnants remain, which are dark until A4.
  - The η axis is needed: today's envelope reaches M_V −5.7 at 10 Gyr only through the η = 0 tail
    draw, and the proxy's pass rates used each star's own η.
  - Each cell holds the brightest V of single stars sampled densely within it: 8 masses, 3
    \[Fe/H\] and 3 η, plus `MARGIN_MAG`.
  - It is stored in integer millimagnitudes, rounded brighter, as `sky_envelope` is.

  Tests:
  - (slow) `phase_envelope_bounds_dense_tracks`: over ≥ 10⁵ single stars of random mass
    (0.0124–150 M☉), \[Fe/H\], η and age, no V is brighter than the table, within the margin.
    Record the margin used.
  - (slow) `the_star_bound_holds_for_realised_systems`: over ≥ 10⁵ realised systems of each layer
    near the Sun and in the bulge, at two emitted times, every star of `state_at(t).stars()` is no
    brighter than its bound.
  - T8.e's identity tests pass unchanged, with T16.b's B skip check restored. In the slow near-Sun
    tests, each of B, C, D and E generates under 25% of its records.
  - The pinned mergers are listed.

  Gate, from the sampled bench near the Sun at the spherical caps:
  - at most 1% of the records past the floor are generated (98.3–99.99% in T8.f's sampled bench);
  - the cold estimate is at most 10,000 CPU-s.

  Record also the sampled cold estimate at cut 10.06, the camera's, beside the eye's 7.95
  (`decision-r06-census-cost-signoff.md`). Record the share of pairs taking `Detached`, `Remnants`
  and `Bright` in each layer. Record the cost of each bound step: the target is about 6 µs a record
  in all.

  Files: `sky/census/cell.rs`, `sky/phase.rs` (new: the table's reader),
  `crates/hyperion-fit/…/sky_phase_envelope`, `tables/sky_phase_envelope.rs`, and the fit-check
  list. Acceptance: `cargo test -p hyperion-sim sky::census sky::phase`, `just fit-check`,
  `just test-slow phase_envelope_bounds_dense_tracks the_star_bound_holds_for_realised_systems
the_census_is_its_oracle_for_the_dwarfs_near_the_sun the_census_is_its_oracle_1000_ly_from_the_sun
the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc`, `just ci`. No generator bump: the
  census only reads, and plan 11's functions read existing words.

- **R06.T8.h The cell cache keyed by magnitude (new; after T8.g).** Decided 2026-10-05
  (`decision-r06-census-cost.md`). Near the Sun every C–E floor is its band's lower edge, so Design
  note 12's mass key makes an entry hold every record of its cell. At 64 MiB the cache then holds
  only a few cells. After T8.g:
  - An entry holds the records whose star-by-star bound passes at the cell's least distance, with
    that faintest listable magnitude as its key and each record's own bound beside it.
  - An entry serves a query whose key is no fainter than its own, by filtering each record's bound.
    A fainter key rebuilds the cell.
  - An entry is built with a margin, which the task chooses from the warm bench. It is fainter than
    the cell needs, so that an approach of up to 1,000 ly is served. For a cell 8 kly out that is
    about 0.3 mag; for one 4 kly out, about 0.6 mag.

  Record one near-Sun sky's entry bytes, margin included, and set `HYPERION_SKY_CACHE_MB`'s default
  so that the warm budget is met at the default (`decision-r06-census-cost-signoff.md`). At today's
  64 MiB a warm census could quietly be a cold one.

  Tests: T8.d's (a query after a looser one, a tighter one and a move gives the bits no cache
  gives; no entry is read below its key). Acceptance: `cargo test -p hyperion-sim
sky::census::cache` and the warm bench recorded against T17's warm budget (≤ 25% of cold).

- **R06.T8.i Nearest first: the shell plan (new; after T8.g; with T11.d; signed off).** Decided
  2026-10-05 (`decision-r06-census-cost.md`). The sign-off was advised by a decision agent and
  adopted on 2026-10-05 under the owner's standing delegation
  (`decision-r06-census-cost-signoff.md`, question 2), with its amendments here. `census_plan`
  orders its cells by distance shell. The shell edges, per layer, are 500 ly, then 1,000 × 2^k ly
  up to the cap; the edges are constants, which never adapt to the machine, so every machine and
  worker count gives the same sequence of replies. A, B and the brown dwarfs are one shell each.
  - A cell belongs to the first shell its padded box meets.
  - `SkyCensus` carries, per layer, the radius to which it is complete.
  - A partial shell's census lists only the stars within its stated radius in their direction. A
    straddling cell's stars beyond it wait for the next shell. So listing and band share one
    boundary, and no star's light is counted twice (Design note 11).
  - Merging shells 1 to k gives the census to shell k, and the last gives the one-shot census,
    which keeps today's rule of listing every star of the cells it opens.

  Tests: shells 1–k merged equal the census with caps forced to shell k's edge, bit for bit, less
  that census's stars beyond the edge; the last equals `census_plan`'s; every cell is in exactly
  one shell; no listed star of a partial census lies beyond its complete-to radius. Acceptance:
  `cargo test -p hyperion-sim sky::census::query`.

Files: `sky/census/{mod,query,cell,merge,cache}.rs`. Bench: `sky/census_near_sun` (eye cut, cold
and warm cache) and `sky/census_nuclear_disc` (eye cut, 150 ly from Sgr A*). The brainstorm's
figures, some 6 × 10⁷ candidates and 5–10 CPU-seconds near the Sun on first arrival, were the
targets to contradict. The 5–10 CPU-seconds are retired (decided 2026-10-05,
`decision-r06-census-cost.md`): an exact census to Design note 9's caps must touch some 3–4 × 10⁸
systems. The budget is T17's.

### R06.T9 The band and the limit map

- **R06.T9.a The extinction profile.** Built in T7 (decided 2026-10-03); T9 calls it at
  `Budget(256)`. `galaxy::gas::extinction::profile` (Design note 14), under
  plan 07's rules, with `horizon`'s `#[expect(clippy::too_many_arguments, reason = …)]` (nine
  arguments) and the quality `Quality::Budget(NonZeroU32::new(256))`, the value of P07.T10.c's
  `SIGHTLINE_QUALITY` (on `origin/galaxy-generation` only at re-validation). Tests: its last node equals `sightline` over the same segment to 10⁻¹²
  relative; it is monotone in distance; at `Mean` it reads no cache. Files:
  `galaxy/gas/extinction.rs`. Acceptance: `cargo test -p hyperion-sim gas::extinction`.
- **R06.T9.b The band.** `sky::band::{CubeFace, BandSpec, BandTexel, band_rows}` (Design note 15),
  reading the tables' light ages through `LuminosityTables::age_for(t, a)` (decided 2026-10-03).
  Tests: the sum over rows equals one call over the face; an observer above the disc sees a band
  brighter towards the plane than towards the pole by the model's own integral; near the Sun the
  band's surface brightness lies within 0.5 mag of 22.05 in the plane (|b| under 5°) and 24.3 at
  the poles (|b| over 80°), each region's mean luminance of the stars fainter than V 6.5 (Gaia DR3
  flux sums, 22.06 and 24.28, V from G by Riello et al. 2021, Table C.2;
  `decision-r06-t9b-band.md`); lowering the cut (a brighter limit) moves light from the listed
  stars and overflow into the band, and raising it moves light back, conserving the total within
  1% either way. Decided 2026-10-05
  (`decision-r06-census-cost.md`): `band_rows` takes the census's complete-to radius per layer (per
  ray after T7.b). Beyond it, the band holds all of the layer's light, not only the light fainter
  than the cut. So a sky that is still filling in (T8.i, T11.d) is as bright as the final one, and
  the expected light of the stars beyond the caps is carried rather than dropped. The band's
  boundary is the census's: per layer, per ray after T7.b, as the census lists
  (`decision-r06-census-cost-signoff.md`). Test, added to the conservation test: the listed,
  overflow and band light together are independent of the complete-to radius, within 1%. The
  radii are those a reply can state, from the first shell's edge to the final caps, not zero,
  which no reply has (decided 2026-10-06, `decision-r06-t9b-band.md`). T9.f re-sums one march at
  several radii; T17 measures the first shell (500 ly) against the final caps. Acceptance:
  `cargo test -p hyperion-sim sky::band`.
- **R06.T9.e Reddening (new; after T9.b, before T9.c; decided 2026-10-06,
  `decision-r06-t9b-band.md`).** The colour table gains four columns per row from T3's fit:
  `A_P ÷ A_V` and `A_S ÷ A_V`, by the display channels' method (plan 07's law at the effective
  wavelength of S·V(λ) and S·V′(λ), over V's), and `A_cam ÷ A_V` at A_V → 0 and at A_V 2, from
  the integral over S·QE·λ. Every existing column stays bit for bit (`just fit-check`). If a
  fetched source no longer matches its checksum, the four are computed from each row's bake
  spectrum and the vendored CIE functions, and that is recorded.

  `StarColour::reddened(a_v) → Reddened` gives the r, g and b transmissions, the photopic
  transmission, the reddened ρ and the reddened camera band term, bit for bit the colour's own at
  A_V 0. `band_rows` reddens each node's light with the solar row's ratios, in five sums (the
  photopic light, R, G, B and the scotopic light), and each overflow star by its own `reddened`.
  T9.c's glare and T11's wire (chroma, eye offset, camera term) read `reddened`. The same commit
  moves T9.b's plane reference from 22.4 to 22.05 in `sky::band`'s test and doc (item 2).

  Tests:
  - `reddened(0)` is the colour;
  - the transmissions order blue < green < red, and scotopic < photopic;
  - the solar row's `A_S ÷ A_V − A_P ÷ A_V` lies in 0.11–0.15, its `A_P ÷ A_V` in 0.97–1.00, and
    its `A_cam ÷ A_V` at A_V → 0 in 0.78–0.88;
  - a Plummer cloud of A_V 1 on +X lowers the texels behind it in ρ and in blue, against an
    unreddened march, by its ratios to 1%, and leaves −X's bits;
  - near the Sun the plane's ρ falls 5–20% and the poles' under 1.5%, and the plane's μ moves
    under 0.05;
  - an overflow star's sums are its reddened colour's.

  Files: `sky/{colour,band}.rs`, `tables/star_colour.rs`,
  `crates/hyperion-fit/src/tasks/star_colour/{columns,mod}.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::colour sky::band` and `just fit-check`.

- **R06.T9.c The limit map.** `sky::limits::limit_map` with the glare of Design note 4. Tests:
  each texel's limit is `naked_eye_limit` at its band luminance plus its glare and at its ρ, to
  10⁻⁹ mag; near the Sun, with the band at cut 8.15 (the eye's cut there, T9.d, which follows),
  the median texel limit is 6.5 ± 0.20 in the band (|b| under 5°) and 7.55 ± 0.22 at the poles
  (|b| over 80°): Crumey's limit at Gaia DR3's light fainter than the cut there (μ 22.18 and
  24.62), and T9.b's 0.5 mag of μ through Crumey's slope (0.40 and 0.45 per mag) (decided
  2026-10-06, `decision-r06-t9b-band.md`; the fixture, whose poles are about 0.3 mag faint, Risks,
  "The galaxy's local light is low", gives about 6.53 and 7.71); a texel within 1° of a V = −1.5
  star is at least 0.3 mag shallower than its neighbours' mean; the map is a function of the
  listed stars and the band alone. Acceptance: `cargo test -p hyperion-sim sky::limits`.
- **R06.T9.d The eye's cut.** `sky::limits::eye_cut` (Design note 5): the coarse pre-pass at 16²
  texels a face through `band_rows` with `SkyCensus::empty()`, the darkest texel's limit, clamped by
  `naked_eye_limit` itself (Crumey's 10⁻⁵ cd m⁻², decided 2026-10-02; no second clamp), +0.45 and +0.1, and one repeat when the cut deepens. Tests: the cut is the darkest pre-pass texel's
  `naked_eye_limit` + 0.45 + 0.1 after the repeat, to 10⁻⁹ mag; the colour table's largest eye
  colour offset at μ 30 is at most 0.45 (it is 0.43); near the Sun the cut is 8.15 ± 0.22:
  Crumey's limit at the darkest 16² texel of Gaia DR3's light fainter than the cut (μ
  24.73–24.77), plus 0.55, with T9.b's tolerance carried through Crumey's slope (decided
  2026-10-06, `decision-r06-t9b-band.md`; the fixture gives about 8.27); no texel of the full
  limit map, with glare, is deeper than the cut less the 0.45 colour offset; the repeat changes
  the cut by under 0.05 mag. Acceptance: `cargo test -p hyperion-sim sky::limits`.
- **R06.T9.f The band's march, kept (new; after T9.e, before T11.c; decided 2026-10-06,
  `decision-r06-t9b-band.md`).** `sky::band::{march_rows, BandMarch, sum_rows}`.
  - `march_rows` marches each ray once, at the rows' texels. Every edge a reply can state is a
    node: each layer's shell edges up to its cap (Design note 13; T8.i's), and its cap (per ray
    after T7.b).
  - It keeps, per layer and edge, the running dimmed sums of the light fainter than the cut and
    of all of the light.
  - `sum_rows(march, census, complete_to, …)` gives a reply's texels with its overflow points,
    reading no profile and no table.
  - `band_rows` becomes `march_rows` at `complete_to`'s own radii, then `sum_rows`, with T9.e's
    bits.

  Tests:
  - `sum_rows` at every radius among the edges equals `band_rows` with the same edges as nodes,
    bit for bit;
  - the listed, overflow and band light at radii 100, 200 and 400 ly (caps forced) agree within
    1%;
  - any split of the rows gives the same bits;
  - the march's heap is recorded at 64² near the Sun.

  Bench: `sky/band_near_sun` split into the march and one sum. Files: `sky/band.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::band`.

- **R06.T9.g Diffuse galactic light (new; research first; after T9.f, before T17's goldens;
  decided 2026-10-06, `decision-r06-t9b-band.md`).** A research agent proposes a model of the
  starlight that the band's dust scatters into each ray, from the plan's own dust field and the
  band's own light. For example: single scattering with Draine's 2003 V-band albedo of 0.677 and
  ⟨cos θ⟩ of 0.538 (R_V 3.1; ApJ 598, 1017), g tried over 0.54–0.8 (Mattila et al. 2018), and
  each node's radiation field approximated from the band.

  Targets: DGL ÷ all-star ISL of about 0.10–0.35 by direction (Toller 1981, via Mattila et al.
  2018, A&A 617, A42, §3.1.1), and 0.13 in the year-mean zenith at 40° N (Masana et al. 2021,
  Table 4), with measured DGL–100 µm slopes considered (Brandt and Draine 2012; Ienaka et al.
  2013). The proposal states its cost against the band's.

  If no model is within 10% of the band's cost and within a factor 1.5 of the ratios, the
  omission is stated in Risks and reported to the owner. Otherwise:
  - it is built into the march's sums as a sum of its own, reddened as the band's light is, so
    that T9.b's starlight test still compares starlight with Gaia;
  - it has tests against the targets;
  - it re-derives T9.c's and T9.d's references with Gaia's ISL plus the DGL.

Files: `sky/band.rs`, `sky/limits.rs`. Bench: `sky/band_near_sun` (all six faces).

### R06.T10 The protocol

`hyperion_protocol::sky` with the DTOs under Provides (among them `stars_bytes`, `band_bytes`,
`HostDiscDto` with its colour, `teff_k`, `log_g` and `bake_spectrum`, and `MAX_CUT_V`),
`RequestBody::Sky`/`ResponseBody::Sky`, the kind string in `REQUEST_KINDS`, the server's `kind()`
and `is_large()` arms (`is_large` is exhaustive; `Sky` is large), and the payload's byte layouts
(Design note 17) documented beside the types; `stars_bytes` and `band_bytes` are `u64` with
`#[ts(type = "number")]`, as `BulkManifestDto.bytes` is; `just gen-protocol` (bindings in
`packages/protocol/src/generated/`, re-exported by hand from `index.ts`); the payload's encoder,
`encode_sky_payload`, in `crates/hyperion-server/src/bulk/sky.rs` beside R03's `bulk.rs` (a
`mod sky;` in it; the protocol crate holds wire types only), over plain star and texel values so
that it does not wait on the sim's types; `packages/protocol/src/sky.ts` with `decodeSkyStars`, `decodeSkyBand`,
`SKY_STAR_BYTES` and `SKY_TEXEL_BYTES`, and their re-export from `index.ts`. `PROTOCOL_VERSION`
stays at 2: a new kind is additive, and R03's Design note 12 rules that the first binary frames do
not bump it. Files: `crates/hyperion-protocol/src/{sky,lib}.rs`,
`crates/hyperion-server/src/bulk/sky.rs`, `packages/protocol/src/{sky,index}.ts`, generated
bindings.

Tests: the wire forms of request and response; a hand-built star and texel encoded in Rust to
pinned bytes, byte for byte, and the same pinned bytes decoded in TypeScript to the same values
bit for bit, as R03.T10.a and T11 pin the header; `stars_bytes + band_bytes = bulk.bytes`; a
truncated payload is an error naming its length; a camera band term of −3.1 encodes to −99 and
decodes to −3.09375 exactly, and −4.5 saturates to −128. Acceptance: `cargo test -p hyperion-protocol
sky`, `cargo test -p hyperion-server bulk::sky`, `pnpm --filter @hyperion/protocol test`,
`just ci`.

### R06.T11 The server

- **R06.T11.a Handler, validation and the census.** `requests/sky.rs`: validation (the observer in
  the cube, time within ±H, `n_max` and `camera_limit_v` in range, a known `exclude_system` through
  `resolve`), the eye's cut by `eye_cut` and the request's cut as the deeper of it and
  `camera_limit_v`, the census as `Priority::Bulk` jobs of a few hundred cells each under the
  request's `CancelToken`, merged once all finish; the census never enters the interactive queue,
  so a chart's query is never held behind it. The time check reuses `convert::query_time`
  (made `pub(crate)`), and the system check `placement::resolve` in a pool job, as
  `requests/scene.rs` does. Each job builds its own `SkyContext`, the sources not being `Sync`.
  The reply's `not_modelled` lists `feature_members` whenever the merged census's
  `feature_members_absent` holds, which is every reply until R06.T16.a. It lists `centre_members`
  and `white_dwarfs` by T8.c's mapping. `SkyGapDto`'s doc comments in `hyperion-protocol`'s
  `sky.rs` take the signed-off wording (`CLUSTERS: NOT YET MODELLED`,
  `WHITE DWARFS: NOT YET MODELLED`) and "until R06.T16.a" (decided 2026-10-05,
  `decision-r06-t16a-scope.md`). The doc comments were corrected with this plan text
  (2026-10-06), ahead of T11.a. Tests (integration, over the WebSocket, at a small census): a
  cancelled request stops its queued jobs and sends nothing further; a range query sent while a
  sky's jobs run is answered first; `n_max` above the cap is `BadRequest` naming `n_max`; a sky
  near the Sun lists `feature_members` in `not_modelled`. Acceptance:
  `cargo test -p hyperion-server --test sky`. Each star's wire chroma, eye colour offset and camera
  band term are its `StarColour::reddened(a_v)`'s (R06.T9.e).
- **R06.T11.b Transfer.** The response and its payload through R03's `BulkPayload::new` and
  `Answer { body, bulk }` (whose `frames` call `bulk::chunk`), the stars then the band, split by
  `stars_bytes` and `band_bytes`, with `BulkPayload`'s `expect(dead_code)` removed; the per-cell
  `ByteLru` of Design note 12 under `HYPERION_SKY_CACHE_MB` (`config.rs`), behind a lock (a
  `SharedByteLru`), with its counters on `ServerStats`. Tests: the manifest matches what was sent
  (`TestClient::next_binary`); the cache's config reads its environment variable. R03.T15's
  pending 15 MiB transfer check in the real renderer is run with this kind, hidden, as R03's Risks
  ask, and recorded. Acceptance: `cargo test -p hyperion-server sky` and `just ci`.
- **R06.T11.c The band, the limits, the discs and the tables.** The band as bulk jobs by face and
  row through T9.f's `march_rows`, then `sum_rows`, then the limit map, then `host_discs` of
  `exclude_system` at the request's time; the luminosity tables built once per galaxy, keyed by
  `GalaxyKey` alone, under `SingleFlight` in a `ByteLru` of their own budget,
  `HYPERION_SKY_TABLES_MB` (default 160, two galaxies; separate from `HYPERION_SKY_CACHE_MB`), as
  `Priority::Bulk` pool jobs from `LuminosityTables::plan` (the envelope is the fitted table, with
  nothing to build), and the caps as ray-chunk pool jobs (decided 2026-10-03,
  `decision-r06-tables.md`). Tests: a sky near the Sun returns the stars, texels and host discs the
  sim returns for the same query; a second identical request shares the tables' build; a second
  sky in another time bucket shares the build. Acceptance:
  `cargo test -p hyperion-server --test sky`. Each star's wire chroma, eye colour offset and camera
  band term are its `StarColour::reddened(a_v)`'s (R06.T9.e).
- **R06.T11.d Delivery nearest first (new; after T8.i, T10 and T11.a–c; signed off).** Decided
  2026-10-05 (`decision-r06-census-cost.md`). The sign-off was advised by a decision agent and
  adopted on 2026-10-05 under the owner's standing delegation
  (`decision-r06-census-cost-signoff.md`, question 2), with its amendments here. The server runs a
  sky request's census shell by shell as bulk jobs. After each shell it sends a sky reply with that
  census and the band for that completeness, and it marks the last reply final. The client
  replaces its sky with each reply (T12, T13).
  - **The order.** Within a layer, nearest first. Across layers, D's and E's shells to 4,000 ly run
    before C's beyond 2,000 ly, or any order that passes T17's V 3.0 gate. The order changes no
    reply's contents: each reply is still the exact census to its radii.
  - **The jobs.** Census jobs are sized to about 50 ms of expected work, since the pool never
    preempts a running job. A cancelled or superseded census keeps its finished cells, so that a
    moving ship's outer shells still converge through the cache.
  - **The band.** Each reply's band is re-summed from one march per request (R06.T9.f's
    `BandMarch`): the rays keep their integrals over the fixed shell intervals, and each reply sums
    the light fainter than the cut inside its radii and all the light beyond them.
  - **The client** swaps each reply's cube in whole. It bakes the new cube, then swaps it in, with
    no blank or half-baked frame. It may coalesce partial replies, but it always applies the final.
  - **The label.** The view's stars-arriving note and its guide rows are drafted here for the UX
    sign-off: `STARS V <m> mag EYE|CAM · BEYOND <edge> ly: STREAMING` in the `STARS` line while a
    reply is not final, and `STARS: PENDING` before the first. The note goes in the line's
    composed-note slot, before any `NOT YET MODELLED` note. It is steady, in `--text` with no
    status colour, an annunciation in the form of `TERRAIN: STREAMING` rather than an alert or a
    data state, and it clears by itself on the final reply. Its figure is the fixed shell edge
    reached, the least over the layers not yet final, in the guide's digit grouping. On loss of the
    link the radius takes its `S` and the note holds. There is no spinner, skeleton or progress
    bar, and stars appear by a cut, with no fade-in. Whether the composed line fits the label block
    at 1280 × 720 and in the instrument slots needs the running client.

  The protocol (T10) gains each layer's `complete_to_ly`, a per-ray table once T7.b lands, and
  `final`. A request for a new sky supersedes the old one. Any later consumer of the sky's list
  reads `final` and `complete_to_ly`.

  Tests:
  - the final reply equals a one-shot census;
  - every reply's stars are the census to its stated radii;
  - T11.a's test that a range query sent while a sky's jobs run is answered first also bounds its
    wait;
  - near the Sun the first reply arrives within T17's first-sky budget on the dev machine.

  Acceptance: the server's sky tests and `just ci`.

Files: `crates/hyperion-server/src/requests/{mod,sky}.rs`,
`crates/hyperion-server/src/compute/sky.rs`, `crates/hyperion-server/src/config.rs`, `stats.rs`,
`crates/hyperion-server/tests/sky.rs`. Bench (server, Criterion): `sky_near_sun_cold` with the
default workers, in `crates/hyperion-server/benches/sky.rs`.

### R06.T12 The client's sky model

`view/sky/model.ts` (`SkyModel` from a response and its payload, the stars' directions kept as
`Float32Array`s), `view/sky/decode.worker.ts` (the payload decoded off the main thread once R03's
`requestBulk` resolves, the typed arrays transferred back), `useSky` (requests on arrival, past
`valid_until`, on a jump and on the parallax rule of Design note 13, with `camera_limit_v` of
Design note 5 over the open views; cancels a superseded request; keeps the last sky marked stale on
link loss), and `eyeLimitAt`. R02's interim star field (R02.T16, `view/stars/interim.ts`) is
removed from the view where the sky has arrived and kept where it has not, with its label. Files:
`apps/hyperion/src/renderer/src/view/sky/{model,useSky,limits,decode.worker}.ts`, tests with the
client's `FakeWebSocket` (`test/FakeWebSocket.ts`, with R03's `serverSendsBinary` and
`test/binaryFrames.ts`) and R03's `BulkAssembler`. As built (re-validated 2026-10-02): the request
is `RequestClient.requestBulk(body, manifestOf)` with `manifestOf = (r) => r.bulk`; the observer is
`barycentreAt(place, t)` plus the camera's offset, and a `null` place asks nothing and keeps
`STARS_WITHOUT_POSITION`; the worker is spawned as `surface.worker.ts` is
(`new Worker(new URL("./decode.worker.ts", import.meta.url), { type: "module" })`, no file
importing a `*.worker` module), any non-worker helper it imports joins `tsconfig.worker.json`'s
`include`, and tests use a fake worker after `test/FakeSurfaceWorker.ts`. N_max is an argument
until T13.f. Tests: the re-request rules; a field factor of 2 lowers every eye limit by 0.387
mag; stale on link loss; the worker's decode equals the main-thread decoder's. Acceptance: `pnpm
--filter hyperion exec vitest run src/renderer/src/view/sky`, `just ci`.

### R06.T13 Drawing the sky

- **R06.T13.a Per-view limits and culling.** `cameraLimitV` over R02's `ExposureTriple` and
  `DEFAULT_VIEW_CAMERA` (Design note 18; its `etaSun` equals `CAMERA_ETA_SUN` through a fixture
  both sides read), the cull (a star is in a camera's view when V + its camera band term is
  brighter than the limit) and the band hand-off (Design note 20), each star's display luminance
  from its V through R02's `illuminanceLx` and `pixelLuminance` at 2.54 µlx and the pixel's true
  solid angle. Tests: 60°, 30° and 13° give 9.95, 11.65 and 13.5 ± 0.3 in a dark sky at high gain
  (9.4, 11.05, 12.9 ± 0.3 at base ISO); 21 stops of exposure, 12 of them gain, lower the limit by at least 5 mag;
  the limit falls with a brighter band texel; culled flux arrives in the band layer to 10⁻⁶
  relative. Files: `view/sky/{cameraLimit,cull,photometry}.ts`. Acceptance: `pnpm --filter
hyperion exec vitest run src/renderer/src/view/sky`.
- **R06.T13.b The pack and the mips, on the CPU.** `packRgb9e5` in TypeScript, the solid-angle
  division, the power-of-two scale and the mip chain weighted by solid angle, as the reference the
  WGSL is tested against, and the CPU splat of Design note 21's fallback. Tests: `packRgb9e5`
  against the extension's worked values (maximum 65,408, smallest 2⁻²⁴, the mantissa round-up
  case); mip weights by solid angle conserve flux to 10⁻⁶; 3,072's last step filters 3 × 3. Files:
  `view/sky/{pack,mips,splatCpu}.ts`. Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/sky`.
- **R06.T13.c Sprites.** `SkySprites`: the selection (budget and parallax at the 30 au baseline),
  per-frame positions for parallax sprites differenced in `f64` against the camera's galactic
  position as R02 prescribes, drawn instanced through R02's `starSprite.wgsl` and
  `psfPixelWeights` (Design note 20). As built, R02's sprites are `view/wireframe/drawList.ts`'s
  `starSprites` (flux-sorted, capped at 2,000 on the low setting), drawn through the
  `wireframe:starSprite` material from a storage buffer of two `vec4f` per sprite; `SkySprites`
  feeds that layout and supersedes the interim's selection. For the photorealistic style it adds an
  HDR twin of R02's `starSprite.wgsl` (same file, tone step compiled out, `rgba16float` pipeline);
  the wireframe keeps R02's material (decision record item 1). Tests: a star 0.1 ly away moves nine pixels across 30 au at
  1080p and 60°, as the brainstorm computes; a star crossing the bake/sprite threshold keeps its
  flux; a moving star's summed energy stays within 1% (Design note 23). Files:
  `view/sky/{sprites,select}.ts`. Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/sky`.
- **R06.T13.d The band layer.** `BandLayer`: the band map uploaded as a small cube
  (`rgba16float`, 64² faces, through `createTexture` with `dimension: "cube"` and a
  `viewDimension: "cube"` binding), bilinearly filtered, drawn first, with the culled stars added.
  Decided 2026-10-02 (decision record item 1): the band, disc, sprite and cube passes have an HDR
  variant (pre-exposed linear, meter-class alpha, `rgba16float` pipelines), checked by the harness
  on a target the test creates; R07.T7 creates the views' HDR target. The wireframe draws the baked
  cube and the sprites tone-mapped per pixel by R02's `agxSprite` straight to the canvas (the cube's
  display variant, registered beside the HDR one), and the band and the discs only in the
  photorealistic style. The cube-sampling draw is `BandLayer`'s full-screen draw: in the HDR
  variant it samples the band and the cube together, in the display variant the cube alone; both
  variants are registered in `WGSL_CATALOGUE` with their `displayName`s. Files:
  `view/sky/band.ts`, `view/sky/shaders/band.wgsl`. Tests: upload layout; the culled-flux sum.
  Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/sky`,
  `just test-render`.
- **R06.T13.e Host discs.** `HostDiscLayer`: each host star of the camera's system from the
  response's `hosts` and the scene's drawn position, angular radius asin(R ÷ d), the power-2 law per
  channel in the fragment shader on `central_luminance`, the clamp and the glare hand-off to R07
  (Design note 16), a point sprite below three pixels; the disc pass writes `METER_CLASS.hostDisc`
  (0) in the HDR target's alpha (R07.T7's; the harness's own target until then) (R02 leaves that channel to R07's meter class), and the sprite and
  band passes blend alpha as source zero, destination one so the class survives;
  `HostDiscLayer.glareSources(camera, viewport): GlareSource[]` returns one R07 `GlareSource` per
  disc, `{ direction; angularRadiusRad; excessLuminance: Rgb }` per channel in cd/m² above 65,504,
  for eye views also for a disc up to 45° outside the frame (R07's asks). Where R07's `post/` module
  does not yet exist, this task declares `METER_CLASS` and `GlareSource` there under R07's names and
  shapes; `DEFAULT_EYE_OBSERVER` is added in `view/sky/eye.ts`. Tests: the Sun from 1 au subtends
  0.533° ± 0.001°; the drawn disc's integrated flux equals π × `mean_luminance` × sin²ρ within 1%; a
  disc below three pixels is a sprite of the same flux; a disc's pixels carry `METER_CLASS.hostDisc`
  and a sprite drawn over them leaves it; an eye view's disc 30° outside the frame yields a
  `GlareSource` and one 50° outside does not; `DEFAULT_EYE_OBSERVER` equals the Rust defaults (a
  fixture both read). Files: `view/sky/{disc,discFlux,eye}.ts`, `view/post/{meter,glare}.ts` (if
  R07's are absent), `view/sky/shaders/disc.wgsl`. Acceptance: `pnpm --filter hyperion exec vitest
run src/renderer/src/view/sky`, `just test-render`. _Since 2026-10-06 the disc lies on its limb's
  plane rather than at infinity, and since R07.T19.e it is a quad over its screen rectangle, with
  none drawn wholly off the view (Risks, "The host discs at their limb's depth" and the entry
  after it)._
- **R06.T13.f The low setting and the label.** `SkySettings` as R05's `ViewSettings.sky`, with its
  high and low values in R05's `SETTINGS`: face size, sprite budget, N_max and re-bake cadence
  (Design note 22); the label block's sky line (Design note 23), to T15's draft; the two styles: the
  wireframe draws the stars as exposed sprites and the band only in the photorealistic style. Tests:
  the label for each stand-in, R02's string included; the setting table. Files:
  `view/sky/{setting,label}.ts`, `view/quality/qualitySetting.ts` (R05's). Acceptance: `pnpm
--filter hyperion exec vitest run src/renderer/src/view/sky`.
- **R06.T13.g The bake on the GPU.** `bakeSkyCube`: the point-list splat into `rgba32float` scratch
  through R01's `createPointSplat`, the pack and mip compute pass, and each level through
  `writePackedCubeLevelFromBuffer` into `createPackedCube`'s cube (Design note 21); the CPU splat
  where `float32Blendable` is false; the `"sky-cube"` and `"sky-scratch"` memory categories; the
  shaders registered in `WGSL_CATALOGUE`. Tests with the headless harness (SwiftShader): the WGSL
  packer's output equals the TypeScript packer's for 10⁴ texels; the GPU splat equals the CPU splat
  to 10⁻⁶ relative; every texel read back is finite; the allocation events name both categories.
  Files: `view/sky/bake.ts`, `view/sky/shaders/{splat,pack}.wgsl`, `view/engine/memory.ts`.
  As built in R01: the splat and the pack kernel are plain WGSL with `main` entry points (not the
  materials' convention), the splat target is cleared by writing zeros before each face, the cube
  takes `writePackedCubeLevelFromBuffer`'s 256-byte padded rows, and the scratch and staging
  buffers are released after each face through T13.h. Acceptance: `just test-render`,
  `pnpm --filter hyperion exec vitest run src/renderer/src/view/sky`.
- **R06.T13.h The engine's releases and the splat's catalogue entry (added at re-validation).**
  R01 as built has no destroy for a buffer or a texture (`destroyed` events come only at disposal),
  no catalogue entry kind for a point splat, and a fixed name for `createPackedCube`. This subtask
  extends R01's adapter as R01's Provides allows later plans to: `RenderEngine.releaseBuffer(handle)`
  and `releaseTexture(handle)` (the WebGPU `destroy`, a `destroyed` allocation event, a released
  handle refused afterwards, resilient-engine forwarding and replay), an optional `name` for
  `createPackedCube`, and a `{ kind: "point-splat", spec: PointSplatSpec }` entry in
  `WGSL_CATALOGUE` that `just test-render` compiles. Tests: release emits one `destroyed` event with
  the bytes created; a released handle throws on use; two named cubes report their own names; the
  catalogue test covers the new kind. Files: `view/engine/{types,memory,catalogue,resilientEngine}.ts`,
  `view/engine/webgpu/{engine,resources,pointSplat}.ts`, `smoke/catalogue.ts`. Acceptance: `pnpm
--filter hyperion exec vitest run src/renderer/src/view/engine`, `just test-render`.

Acceptance for T13 as a whole: `pnpm test`, `just ci`, `just test-render`, and by hand, recorded
in the plan: near the Sun the brightest stars the census lists match the brainstorm's statistics
(some 15,000 visible to the eye, some 740 in a 60° view), no star flickers as the camera turns
slowly, the band shows its dust lanes, and the Sun's disc is limb-darkened; the star field's GPU
time on the development machine's RTX 3080, the discrete target, at 1080p (under 0.2 ms) and, by
the owner, on the UHD 620 at 720p (target under 0.5 ms), each recorded with its setting.

### R06.T14 Several views and the re-bake

The sky cubemap is shared between views on the one device and baked once per arrival; where a
view's camera is far enough from another's that the parallax rule re-bakes, it holds its own cube,
as the brainstorm's "Several views in one client" says. A camera view and the eye view share the
census and differ in their cull. Files: `view/sky/cache.ts`. Tests: two views near one another
share one texture; two far apart in the nuclear disc hold two; releasing a view releases its cube.
A released cube is freed through T13.h's `releaseTexture`. By hand, recorded: the cockpit and two
instrument canvases (R01's proof) draw one sky. Acceptance:
`pnpm test`, `just ci`.

### R06.T15 Guide nomenclature for the owner

Draft, in one commit for the owner to read, the sky's additions to `docs/frontend/ux-guidelines.md`
on top of R02's nine items: `STARS`, `EYE`, `CAM` and the stand-in phrases of Design note 23 in the
nomenclature list; the rule that the limit shown with the sky is a magnitude with its kind; and a
sentence under the view class that the unresolved band is labelled. As built, the guide already
has `STARS` as the chart's filter and heading, and R02's drafts `STARS: RANGE QUERY ·
VOLUME-LIMITED · NO EXTINCTION` and the interim count line `STARS <n> DRAWN · …`: the sky's
`STARS V <m> EYE|CAM` is drafted as a further use of the existing `STARS` row (a view's label),
not a second row, and the interim's two rows are marked as withdrawn where the sky has arrived. It
is drafted before T13.f. It
ends when **the owner signs off**; until then the client is built to the draft, as the galaxy
plans' guide drafts are. Acceptance: `pnpm format:check`, `grep` finds `STARS V` in the guide
draft, and the owner's sign-off recorded in this plan.

### R06.T16 Feature members and binaries

- **R06.T16.a Feature members (out of RM3's scope; lands with P09.T2.c).** Decided 2026-10-05
  (`decision-r06-t16a-scope.md`). RM3 closes without this subtask. Every sky reply lists
  `feature_members` in `not_modelled`, so each view's label reads `CLUSTERS: NOT YET MODELLED`
  (Design note 23).
  - **Why it can wait, and why it cannot land alone.** Until galaxy plan 09's P09.T2.c nothing
    reads φ. The field keeps every population's whole budget (`FeatureShare::None`), so:
    - the clusters' and associations' stars are already in the sky, spread through the field
      instead of gathered into clusters;
    - the counts, the caps and the band are right in expectation.

    Members taken in before P09.T2.c would be counted twice. P09.T2.c without this subtask would
    take φ out of the sky, its caps and its band. As built, φ_young stays near 0.9 until
    associations begin to dissolve at 30 Myr. So that would be about nine in ten of the O and
    early-B stars, and at least a quarter of all B stars. The two therefore land in one
    integration, at P09.T2.c's generator version, as T16.b did with plan 11's wiring.

  - **Prerequisites:**
    - P08.T12, whose `stay_share` P09.T2.c reads;
    - P09.T2.c;
    - P09.T23.b, which pads each feature by its own members' speed bound;
    - the part of P09.T40.a this subtask reads: a byte-bounded `FeatureCellCache` and interior
      cache (`ClusterModelCache`) that the census's jobs share, `FeatureMemberSource` registered
      in `SystemsInRange`, and `FeatureGas` passed where `NoModifiers` is.

    P09.T40's centre and catalogue-class sources wait on P09.T29 and T37, and are not needed. The
    centre's members join with P09.T28.b–T29, which clears the `centre_members` gap.

  - **The census.** The sky handler builds the same sources over P09.T40's caches. The census
    reads `FeatureMemberSource` through the `SystemSource` hook, with the same skips (a member's
    mass word is its own, so the floor applies), and `FeatureGas` as the sightline's modifiers.
    Feature members, and from P09.T28 the centre's members, are a source's records and move by
    their own laws. The census takes them within their source's own reach: each feature padded
    by its own speed bound (P09.T23.b), and the centre by its orbits (P09.T29). It never pads
    them by the cell rule, and it always generates them (Design note 10).
  - **To design at re-validation, before it starts:**
    1. **The band and the caps.** They read the density field (Design notes 9 and 15), which from
       P09.T2.c holds 1 − φ of each population. The members' light fainter than the cut and
       beyond each cap, a globular's included, must reach the band. Their counts brighter than
       the cut must reach each cap's `expected_beyond`. Otherwise the band loses φ of the young
       disc's light, and a cap can stop short of a bright association.
    2. **The cost.** Members are always generated. A living star costs about 10 µs and a remnant
       1.35–1.6 ms, because `draw_member` builds a `StarModel` per attempt (plan 09's T23
       findings). A query in the disc visits every feature whose reach touches it, a nursery's
       1,750 ly included. Before this subtask lands, the census near the Sun is benched with
       members against T17's budget.
  - **The label.** `CLUSTERS` leaves the label's composed `NOT YET MODELLED` note once neither
    `feature_members` nor `centre_members` is listed. `CLUSTERS: NOT YET MODELLED` is then
    withdrawn, and `CLUSTERS AND WHITE DWARFS: NOT YET MODELLED` becomes
    `WHITE DWARFS: NOT YET MODELLED`.
  - **Tests:**
    - the Pleiades-like cluster of a pinned seed appears as a clump of bright stars from 400 ly;
    - the globular core's sky from its centre lists stars to V 6.5 within a factor of two of the
      brainstorm's about 4 × 10⁵ (its 47 Tuc row);
    - near the Sun, the census's count to V 6.5 is within Poisson error of the count expected
      from the field and the members together (item 1's design).

    Acceptance: `cargo test -p hyperion-sim sky::census::features`, the member-cost bench of
    item 2, and `just ci`.
- **R06.T16.b Binaries (after P11.T6–T11; due now, after T8.e).** When plan 11 wires binary
  evolution into `SystemStars`, `max_star_mass` returns min(2 m₁, 150 M☉), the envelope's age
  range starts at zero, and the n × F₁ bound requires a system that cannot have interacted (Design
  notes 8 and 10). Re-validated 2026-10-02: P11.T11 has done that wiring (version 16; the pair
  timelines reach `SystemStars::state_at`), and P11.T6 and T8–T10 move no star's mass, so this
  subtask runs straight after T8.e. The brief (`brief_at`) is the primary's single-star model and
  cannot tell whether a multiple system has interacted, and P11's `can_interact` is crate-private
  and reads the models' tracks, which need `SystemStars::generate`. Decided 2026-10-02
  (decision record item 2): for n ≥ 2 the bound is n × F_env at `max_star_mass(m₁)` over ages
  from zero; n × F₁ only for single stars. No ask of plan 11 unless T17 finds the multiple-system
  bound above 25% of census time in a benched field, in which case a public period-and-mass
  interaction test is asked of plan 11 then. Tests: a pinned blue straggler of an old cell (a
  merger or an accretor) is listed and equals `brute_force_sky`; the identity tests of T8.e pass
  unchanged; (slow) `envelope_bounds_pair_states`: over ≥ 10⁴ realised multiple systems in old and
  young cells of each layer, every star of `SystemStars::state_at(t).stars()` is no brighter in V
  than the envelope at `max_star_mass(m₁)` with ages from zero, within the 0.3 mag margin.
  **Found by T8.e's oracle (2026-10-04):** two of T8.e's slow identity tests fail until this
  subtask, each on a merger product the flux bound at m₁ skips (Risks, "Merger products outshine
  the flux bound"). Those two systems, evolved merger products (an evolved blue straggler each, now
  red giants), are the pinned mergers to use; an unevolved straggler is still to be pinned for the
  main-sequence case. This subtask removes their exclusion from `.config/nextest.toml`'s
  `[profile.slow]` `default-filter`, with its comment. It drops "fails until R06.T16.b" from the
  two tests' ignore reasons, and the paragraph saying so from `tests/sky_census.rs`'s module doc.
  It changes no assertion, and their identity checks then pass unchanged. Two of their checks are
  not identities: that the census skips some systems, and, in the A/B test, that A and B each do.
  With the bound widened to 2 m₁ over ages from zero, they could fail. If one does, that is a
  finding on the widened bound's cost, to be reported, not an edit to make. Acceptance:
  `cargo test -p hyperion-sim --test sky_census`, `just test-slow envelope_bounds_pair_states
the_census_is_its_oracle_for_the_dwarfs_near_the_sun the_census_is_its_oracle_1000_ly_from_the_sun
the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc` (with the exclusion removed) and `just
ci`. Decided 2026-10-05 (`decision-r06-census-cost.md`): the widened bound is the baseline the
  census's cost levers must hold under. It passes C 98.5%, D 99.995% and E 100% of records near the
  Sun, and B skips none of 110,151 within 500 ly. In the A/B slow test, the check that B skips some
  systems is removed. In its place is a printed tally and a comment naming R06.T8.g, which restores
  the check as part of its acceptance. Every other assertion is unchanged, including every
  identity, and A's skip check stays. The two pinned mergers and the unevolved main-sequence merger
  `0x41feeca200000000` (C, m₁ 0.90 → 1.80 M☉ on the main sequence, M_V 1.98 against the old
  bound's 2.35, 267 Myr) are T16.b's pinned systems. As built: Risks, "Deviations in T16.b, as
  built".

### R06.T17 Verification pass

Run the slow tests and benches this plan creates (by name, not the whole slow suite or every bench,
as the RM2/RM3 lanes' rules require: `just test-slow luminosity_matches_realised_cells
envelope_bounds_dense_tracks envelope_bounds_pair_states caps_converge_in_rays
the_full_build_matches_thirty_two_nodes_a_panel_where_the_tables_are_read
standard_nodes_match_the_full_build_where_the_tables_are_read
the_census_is_its_oracle_for_the_dwarfs_near_the_sun the_census_is_its_oracle_1000_ly_from_the_sun
the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc`, `just bench -- sky`) and record the
figures in the doc comments that own them and in this plan: the caps (at the six points of
`caps_converge_in_rays`, against the brainstorm's C 3,000, D 4,300, E 10,000 ly near the Sun and
"a few hundred to about 1,000" in the nuclear disc, with what sets each: C the M_V −2 to −4 AGB tips
and post-AGB crossings, D post-AGB and bright giants through clear windows, E supergiants; handed
to the brainstorm's sky section and open question 19, with C's post-AGB re-derivation and D's
post-AGB count; and `layer_caps`'s CPU time per call near the Sun and in the inner bulge, 768
`Full` realised profiles: above 10% of the census's CPU time in either bench, propose fewer rays
with a finer convergence proof or a coarser quality the slow test still passes), candidates opened, CPU-seconds and listed stars near the Sun and in the inner
bulge, re-deriving open question 19's counts at the current version and explaining why candidates exceed the
systems layers C to E hold; the candidates the binary rule of T16.b costs; check the per-layer
counts against `range_500ly_floor_d` (37,675 systems at version 15, re-measured at the current version). Add goldens:
`crates/hyperion-sim/tests/golden/sky/census_near_sun.golden`, the census of a pinned observer near
the Sun to V 7 (IDs, star indices, V to 10⁻⁶ mag), and `sky/band_face_row.golden`, one band face
row, read by the testkit's golden harness. Record the per-record bound's pass rate (records
generated ÷ records skipped) for single and multiple systems in each census bench, against T16.b's
25% trigger (decision record item 2). Decide N_max and the sprite budget per setting from the
measurements (open question 16) and record them. Record the tables' build in CPU and wall time, its
heap and the cold first sky against the budget decided 2026-10-03 (`decision-r06-tables.md`): the
per-galaxy tables at most 30 CPU-s on a quiet machine, and the cold first sky near the Sun at most
10 s wall on the dev machine with the default workers; if either fails, propose the disk cache
(keyed by `GalaxyKey` and a sim fingerprint) or deeper node cuts. The build measured is
`BuildOptions::STANDARD`'s, T5.e's panels and nodes (decided 2026-10-05,
`decision-r06-t5e-gate-2.md`), whose own timing is provisional; deeper node cuts must pass
T5.e's two slow tests. If the cold first sky fails, the stage chain's schedule (T11.c's
lookahead and chunking; T5.e records each stage's nodes) is measured before any further cut.
Without R06.T16.a, which is out of RM3's scope (decided 2026-10-05,
`decision-r06-t16a-scope.md`), every figure here is the grid's, and the census takes no feature
members. The brainstorm's globular-core row (47 Tuc) is recorded as pending T16.a, not re-derived.
When T16.a lands, it re-benches the census with members against this budget.

The census budget, decided 2026-10-05 (`decision-r06-census-cost.md`), applies near the Sun at the
eye's cut as T9.d computes it (about 8.27 on the fixture), benched also at 7.95, its estimate when
the budget was set (decided 2026-10-06, `decision-r06-t9b-band.md`; the eye's cut; the camera's
cut, 10.06 at 60°, is benched beside it and its budget ruled from that figure,
`decision-r06-census-cost-signoff.md`), at the current caps, on a quiet machine:

- the first sky in at most 150 CPU-s and 10 s wall on the dev machine's default workers;
- the full cold census in at most 4,000 CPU-s;
- after a jump of up to 1,000 ly, at most 25% of cold.

The budget was accepted as the eye's only. A decision agent advised that, and the advice was
adopted on 2026-10-05 under the owner's standing delegation (`decision-r06-census-cost-signoff.md`,
question 1). A decision agent rules the camera's budget from the cut-10.06 bench, and reports it to
the owner, before T17 closes and before any bridge play-test.

Run the census benches sampled (`HYPERION_SKY_BENCH_SAMPLE`), at the eye's cut as T9.d computes it
and at 7.95 (`decision-r06-t9b-band.md`), and beside them at 10.06, and once whole if the sampled
estimate is under an hour on the machine. The census benches, and the 1,000 ly identity test once,
run on a galaxy built `with_full_potential`, as the server's galaxies are. The fixture has no
kinematic tables, so its records stand still and `Drift::of_record` costs 0.05–0.08 µs. A drawn
velocity costs about 0.72 µs (plan 08's T2–T6 timings), which is some 290 CPU-s over the 4 × 10⁸
records near the Sun until T8.g's bound rejects most of them before their drift (decided
2026-10-05, `decision-r06-pad-speed.md`). For each, record:

- each layer's cells, candidates, records, generated and listed;
- the share of pairs in each of T8.g's cases;
- the time to each shell's reply;
- the first reply, the full census and a warm jump at `HYPERION_WORKERS=4`, the laptop's proxy;
- the photorealistic view's frame time, p95 and p99, while a census runs: on the dev machine here,
  and by the owner on the laptop. A census that breaks the frame budget is a finding. Its remedy is
  for the server to lower its bulk work's priority, or the workers it gives bulk work, beside a
  client, not to shrink the census;
- the per-reply cost of the band and the limit map. The first reply's band, limit map and shell
  together must fit the first-sky budget;
- the sky's total light (listed, overflow and band) in the first reply against the final's, at
  the eye's cut near the Sun, within 1% (T9.b's amendment, decided 2026-10-06,
  `decision-r06-t9b-band.md`); a miss is a finding for R06.T5.f, not a looser gate;
- the delivery time of the final reply's stars brighter than V 3.0, against 25% of the full
  census's wall time.

Every figure states its cut and its machine: "10 s" alone is the dev machine at the eye's cut. The
nuclear-disc bench is recorded, with no budget. If a budget is missed, in this order:

1. ask plans 06 and 11 for faster generation (post-main-sequence tracks 0.5–0.66 ms a star, pair
   evolution 2–7 ms a pair);
2. build R06.T11.e, a disk cache of each cell's survivors keyed by `GalaxyKey` and a sim
   fingerprint. Build it also, even when the dev machine passes, if the owner's laptop run finds
   the fill at session start objectionable, since every single-player session start is a cold
   census. Its key adds a `SKY_CACHE_VERSION` that census code changes bump, the fit-check
   fingerprints of the committed tables the census reads, and a probe at server start: a census of
   a few pinned small cells near the Sun and in the bulge, compared with digests stored beside the
   cache, any difference discarding the whole cache. The probe catches a code change that bumped
   nothing, as the envelope's move to a fitted table did. The cache lives outside the universe's
   directory (for example `<data_dir>/cache/sky/`), bounded by size, so that deleting it changes no
   reply and only a universe's identity is persisted. A smaller variant persists each universe's
   last final reply under the same key, served at session start within its `valid_until` and then
   recomputed (`decision-r06-census-cost-signoff.md`);
3. put Design note 9's criterion back to the owner, with measured figures, as a visibility-based
   rule or a rule labelled on a low setting. The ruling's optional looser rule (under 1% of each
   layer's listed stars expected beyond) was declined on 2026-10-05, on the same adopted advice,
   and nothing is authorised in advance (`decision-r06-census-cost-signoff.md`, question 4).

Record T5.d's residuals and the
band's pair correction with its 1σ (from `LuminosityFunction::pair_light_sigma`) at the decision's
three harness points ((0, 26,000, 68), (0, 8,000, 0) and (0, 3,000, 0)) and at a halo point, (0,
26,000, 15,000) ly (decided 2026-10-04, "T5.d gate reading"); a 1σ over 2% of the band's light at
any of the four is a finding. The fits `sky_binary_light_c`, `_d` and `_e`, `sky_envelope` and
T8.g's `sky_phase_envelope` join the check list (`just fit-check`). Each timing is taken on a
quiet machine, as the
roadmap's conventions require, or marked provisional. Acceptance: `just ci`, and the named `just test-slow` and `just
bench -- sky` runs above complete.

## Verification

- **Exactness of the shortcuts:** the skips, the flux bound and the caches never change an answer
  (`brute_force_sky` with forced caps, T6, T8.e, T16.b); the extinction profile ends on
  `sightline`. The caps are an approximation, bounded by each layer's `expected_beyond`, which the
  response states (T7).
- **Photometry against published figures:** Crumey's limits (T2), the colour table against Pickles
  (T3), the solar limb and the discs' flux (T4), and the camera model at 60°, 30° and 13° (T13.a).
  The camera model's agreement with measured cameras (Vida et al. 2021; Jenniskens et al. 2011) is
  Design note 18's research, re-checked by hand in T17, not a test.
- **The galaxy's statistics:** the star-count slope and the band's surface brightness near the Sun
  (T5, T9); the brainstorm's sky table rows re-derived at the current version (T17), all but the
  globular core's, which waits on R06.T16.a.
- **Conservation:** light moves between points, overflow and band without loss (T9, T13).
- **Order independence** of the census over jobs and cells (T8.c) and of the band over rows (T9.b).
- **Benches:** `sky/luminosity_tables`, `sky/census_near_sun`, `sky/census_nuclear_disc`,
  `sky/band_near_sun`, `sky_near_sun_cold` (server). The census benches run sampled
  (`HYPERION_SKY_BENCH_SAMPLE`), against the budget of `decision-r06-census-cost.md`, at the eye's
  cut and beside it at the camera's (`decision-r06-census-cost-signoff.md`).
- **By hand, recorded:** the star field's GPU time on both machines, no flicker, the band's lanes,
  the discs, several views sharing one cube.

## Generator version

No change to generated output and no bump. `generate_cell_where` is `generate_cell` filtered, and
the goldens prove it; the luminosity tables, the envelope, the caps and the census only read. The
sky's own output is a function of the generator version and of the committed colour,
limb-darkening, envelope (`sky_envelope`, T6.b) and pair-evolved light (`sky_binary_light_c`, `_d`
and `_e`, T5.d) tables, so its goldens (T17) are regenerated whenever any of them moves. The
envelope's move to a fitted table (2026-10-04) made it up to 1 mmag brighter, which moves the caps'
rule bound and so the caps and the census's planned cells, with no bump: no golden pins them and
nothing serves them yet (`decision-r06-tables.md`, A.5). T5.d's correction (2026-10-04) moves the
luminosity tables' light, colour and counts in layers C, D and E, with no bump for the same reason.
T5.e's panel edges and node rule (2026-10-05) move the tables' light, colour and counts, by
about 1% in an old halo component's total light and colour toward the converged quadrature and
by under 0.25% wherever else they are read, with no bump for the same reason; `galaxy::fates`
and `mean_present_mass` are untouched, so no generated output moves. Not adopted:
drawing a cell's mass words in sorted order, which the brainstorm offers as a generator-version
change to skip light candidates without opening their streams; the mass-first walk already skips
their position and density, and the benchmark decides whether the rest is worth a bump (Risks). The
plan reserves no tag, prefix or stream. The census's cost subtasks (decided 2026-10-05,
`decision-r06-census-cost.md`) need no bump either. The census only reads, and
`sky_phase_envelope` (T8.g) joins fit-check. Plan 11's `hierarchy_bound` and `pair_light_bound`
must read the generator's existing words and add no stream or word, which their tests pin. T7.b
changes which cells the census opens; with no golden pinning them before T17 and nothing served,
it needs no bump.

## Risks and open points

- **Re-validated at ce7aeb3** (2026-10-02, `main` and `rendering-and-planets` at RM1's close, R05
  not built; `origin/galaxy-generation` compared). Consumes now record the as-built names of R01
  (positional `createPackedCube`, the padded-row buffer layout, `main`-entry kernels and splat, no
  splat catalogue kind, no per-resource destroy, the readback guard), R02 (no viewport or galactic
  position on `CameraState`, no HDR target in the wireframe, the interim's real files and labels,
  the sprites' storage layout), R03 (`BulkPayload`, `requestBulk(body, manifestOf)`, the client's
  64 MiB / 257-chunk limits, `FakeWebSocket`'s path, the `SystemPlace` union, R03.T15's pending
  transfer check), R04 (units in base, four of this plan's missing; `Span` and `UniverseTime` in
  the sim) and the galaxy plans (the candidate's word order confirmed; `query_time` private;
  `Quality::Budget(NonZeroU32)`; fit tasks registered in `task.rs`'s `REGISTRY`; `SystemSource`
  and `FeatureMemberSource` not `Sync`; generator version 19, not 15). Task edits: T16.b is due
  now, after T8.e, because P11.T11 (version 16) wired the pair timelines into
  `SystemStars::state_at`, which the census reads (Design note 8); new subtask **T13.h** adds the
  engine's buffer and texture release, a named packed cube and a point-splat catalogue kind, which
  T13.g's transient scratch and T14's release need; T10 classifies `Sky` in `kind()` and
  `is_large()` and builds its encoder over plain values; T11 reuses `query_time` and removes
  `BulkPayload`'s `expect(dead_code)`; T15 extends the guide's existing `STARS` row; T17 runs its
  named slow tests and benches only; `HostDiscDto`'s field names are pinned for R07. Brainstorm drift since the plan's creation (899db5e): only the
  CSP ruling (R04.T10.a), which the sky's decode worker, a same-origin module worker compiling no
  WebAssembly, meets as it is. No generator-version bump and no protocol-version change follow.
  **Pending re-validation:** T13.f waits on R05 (`QualitySetting`, `ViewSettings`, `SETTINGS`,
  `AllocationTally`, being re-validated in parallel); T16.a waits on P09.T40 (not built on `main`
  or `origin/galaxy-generation`). **Missing galaxy work** at re-validation: P09.T40 (T16.a);
  P09.T28.b (centre members, tallied until then, T8.b); A1–A4 and the plan 06 heading (T1 drafts
  them; interims hold); P07.T10.a–c is on `origin/galaxy-generation` only and is not needed.
- **No HDR target before R07 (closed 2026-10-02 by the decision record's item 1).** R07.T7 creates
  each photorealistic view's HDR scene target; R06's sky passes get HDR variants tested on targets
  the tests create; the wireframe draws the cube and the sprites tone-mapped per pixel, the band
  and the discs only in the photorealistic style; T13.c–e carry the edits. The question as it
  stood: R06 Design notes 16 and 21 and T13.d–e write into "the HDR target" with pre-exposure and `METER_CLASS` in its alpha,
  and R07 calls the HDR target R02's; but R02 as built draws the wireframe straight to the canvas,
  tone-mapping each sprite (R02.T13's deviation), and nothing draws into an `rgba16float` scene
  target until R07's photorealistic style. Lean (the smallest reversible choice): R06 builds the
  band and disc passes for the HDR target and checks them in the harness on a target it creates;
  until R07, the wireframe view draws the sprites and the baked cube tone-mapped per pixel by R02's
  `agxSprite`, and the band and discs only in the photorealistic style, as T13.f already says of
  the band. R07, being re-validated in parallel, owns where the HDR target is created.
- **Deviations in T15, as built (2026-10-02).** Drafted in `docs/frontend/ux-guidelines.md`: the
  existing `STARS` row gains the label-block use `STARS V <m> EYE|CAM` (in `mag`, one decimal);
  new rows `EYE`, `CAM` (one row), `CLUSTERS NOT MODELLED`, `WD NOT MODELLED` and
  `UNRESOLVED STARS`, the last being the band's name in the DOM list's view notes, which Design
  note 23 left unnamed (the lane's choice); R02's two interim rows are marked withdrawn on a view
  once the sky has arrived; the "Views" class gains the rule that the star limit is always a V magnitude with
  its kind, with any stand-in after a middle dot, and a paragraph that the unresolved band is
  labelled and drawn only in the photorealistic style. **Signed off with amendments** (2026-10-03,
  delegated; orchestration `decision-r06-t15-guide.md`): `mag` on the limit; the stand-ins in the
  composed `<WHAT>: NOT YET MODELLED` form, `WD` spelled out; `UNRESOLVED STARS` renamed
  `INTEGRATED STARLIGHT`; the table's stray `|` fixed.
- **Deviations in T13.b, as built (2026-10-02).** A fourth file, `view/sky/cube.ts`, holds what
  the CPU splat, the mips and T13.g's WGSL share: `cubeTexelOf` (WebGPU's face order and (u, v)
  orientation, ties to x then y then z) and `texelSolidAnglesSr` (the exact atan2 texel area). The
  splat's point layout is pinned here for T13.g's GPU splat: `SPLAT_POINT_FLOATS` (8), direction
  (x, y, z, 0) then illuminance (r, g, b, 1) in lx, so alpha sums the count. `pack.ts` exports
  `packRgb9e5`, `unpackRgb9e5`, `packRgb9e5Texels`, `RGB9E5_MAX`, `RGB9E5_MIN_POSITIVE`; `mips.ts`
  exports `divideBySolidAngle`, `peakScaleExponent` (brightest channel to (2¹⁴, 2¹⁵]),
  `scaleByPowerOfTwo`, `faceMipChain` (children weighted by their summed solid angle), `mipStep`,
  `mipSizes` and `cubeLevels` (faces joined for `writePackedCubeLevel`). The extension gives no
  worked numbers; the tests pin its limits (65,408, 2⁻²⁴, 1.0 = exponent 16 mantissa 256) and the
  round-up case (0.99999 packs as 1.0). Review fixes: both the texel a direction falls in and the packer's rounding
  are computed so that `f32` and `f64` agree bit for bit, which T13.g's WGSL must copy: a texel is
  the largest c with f32(c × 2m) ≤ f32(f32(a + m) × size), found from an estimate by those
  comparisons alone (WGSL's division is not correctly rounded, its sums and products are), and a
  mantissa is ⌊q⌋ plus one where q − ⌊q⌋ ≥ ½ (⌊q + ½⌋ rounds the sum in `f32`). `CubeFace` (0–5)
  types the face; the mip functions refuse faces of the wrong size.
- **Deviations in T10, as built (2026-10-02).** The wire's chroma (stars and texels) is the
  linear Rec. 709 chromaticity r ÷ (r + g + b), g ÷ (r + g + b), each in [0, 1] as Design note
  17's `u16` fractions require, not `StarColour::chroma`'s "r and g of unit luminance", which
  exceeds 1 for red and blue stars; the client recovers unit luminance by dividing by
  0.2126 r + 0.7152 g + 0.0722 b, and T11 converts. `HostDiscDto.chroma` keeps the pinned
  `StarColour` meaning. Shapes the plan left open: `EyeDto` (`field_factor`, `age_years`,
  `pigmentation`); `ConeDto` (`axis: [f64; 3]`, `half_angle_deg`); `BandSpecDto`
  (`face_texels`); `SkyGapDto` (`feature_members`, `centre_members`, `white_dwarfs`);
  `SkyLayerCensusDto` (`layer`, `cap_ly`, `rule_bound_ly`, `expected_beyond`, `cells`,
  `candidates_opened`, `accepted`, `listed`, `without_photometry`, `feature_members_absent`; the
  two `u64` counts as JSON numbers); `ResponseBody::Sky` is boxed. Constants beside `MAX_CUT_V`:
  `MAX_SKY_STARS` (3 × 10⁵, the default and cap of `n_max`), `SKY_STAR_BYTES`,
  `SKY_TEXEL_BYTES`, `SKY_BAKE_BINS` (15), restated in `packages/protocol/src/sky.ts` as R03's
  frame constants are. The server's encoder takes `SkyStarWire` and `SkyTexelWire` and returns
  `EncodedSky` (`bytes`, `stars_bytes`, `band_bytes`); quantised fields round to nearest and clamp
  to their integer's range (NaN as 0), and an eye limit never takes the `i16::MIN` sentinel. The
  decoders return a result union (`SkyDecoded<T>`) of struct-of-arrays (`SkyStars`, `SkyBand`,
  eye limit NaN where absent), and `splitSkyPayload(payload, response)` splits by `stars_bytes`
  and `band_bytes`. Until T11 the server answers `sky` with `unsupported` under its own ID, as it
  does `body_events`. The camera band term is Sun-relative and travels in 1/32 mag
  (decision-camera-eta.md, applied here): `thirty_seconds` in the encoder, ÷ 32 in the decoder.
- **Deviations in T13.h, as built (2026-10-02).** `RenderEngine` gains `releaseBuffer` and
  `releaseTexture` (the registry's existing `destroyBuffer`/`destroyTexture`, which raise one
  `destroyed` event with the bytes created; a released handle is refused by every later call with
  "… was released"), an optional fourth argument `name` on `createPackedCube` (default
  `PACKED_CUBE_NAME`, "packed star cube"), and `createPointSplatAsync(spec)`, added beyond the
  task's list so that the harness's catalogue check sees a splat's WGSL error as a rejection, as
  `createMaterialAsync` and `createComputeAsync` do; without `float32-blendable` the check records
  the splat as not compiled rather than failing (so on such a device, possibly the UHD 620, the
  splat's WGSL is unchecked by the harness). The engine refuses to release a render target's colour
  or depth ("belongs to a render target and is released with it"); its other internal textures and
  buffers are never handed to callers. `ResilientEngine` keeps the engine that made each handle (a
  `WeakMap`): a release reaches it if it is the current engine, is dropped if a lost engine made it
  (it died with its device, as a write to it is), and throws for a handle it never made. Both test
  fakes implement the new members (`FakeRenderEngine` now fakes `createBuffer` and records
  releases; R05's `CountingRenderEngine` raises `destroyed` events). `WGSL_CATALOGUE` takes
  `{ kind: "point-splat", spec }` and holds `ENGINE_CHECK_SPLAT`, R01's harness splat lifted from
  `smoke/blending.ts`, until T13.g registers the sky's bake splat.
- **Deviations in T13.a, as built (2026-10-02).** `cameraLimit.ts` exports `cameraLimitV`,
  `cameraLimitParts` (the limit with its sky electrons, read noise and V = 0 peak electrons),
  `DEFAULT_VIEW_CAMERA` (a `ViewCameraSensor` with `etaSun` = `CAMERA_ETA_SUN`, 2.9557 from
  T3.c's fit, pinned by `packages/protocol/fixtures/camera_eta_sun.json`), `surfaceBrightnessV`
  and `V0_PHOTON_FLUX_PER_S_M2`. Design note 18 names no dark current, and it is 0 (0.1 e⁻ s⁻¹
  would add 0.003 e⁻ at 1/30 s). High gain in the tests is ISO 409,600, where the read noise is
  σ_pre's alone; base is ISO 100. `cull.ts` exports `cullSky(stars, limit, bandFaceTexels)` with
  `ViewStarLimit` (`eye` with `limitAt(x, y, z)`, NaN keeping every star, or `camera` with
  `limitV`) and `starIsSeen`; an eye keeps a star with V < limit + its eye colour offset, a
  camera with V + its camera band term < limit; the dropped stars' illuminance goes to the band's
  texels in `f64` (`bandIlluminanceLx`, three channels a texel). `photometry.ts` exports
  `unitLuminanceRgb` (the wire's chromaticity to unit luminance by Rec. 709's weights),
  `starIlluminanceRgbLx` and `starPixelLuminanceRgb`. A black background (0 cd/m²) is allowed and reads as read noise alone.
- **Deviations in T13.f, as built (2026-10-03).** `view/sky/setting.ts` exports `SkySettings`
  (`faceSizePx`, `spriteBudget`, `nMax`, `rebakeShiftPx`), `HIGH_SKY` (3,072, 4,096, 3 × 10⁵,
  0.1 px) and `LOW_SKY` (1,024, 2,048, 10⁵, 0.1 px), wired as R05's `ViewSettings.sky` (appended
  after `atmosphere`) with their values in `SETTINGS`, and `SKY_LAYERS` per `SkyStyle`
  (`wireframe`: sprites and cube; `photorealistic`: all four), the decision record's item 1. The
  file is `view/quality/qualitySetting.ts` as R05 built it. `view/sky/label.ts` exports
  `skyLabelValue(limitV, limitKind, gaps)`, the `STARS` line's reading after its label (`V 7.4 EYE`,
  then `CLUSTERS NOT MODELLED` for the feature and centre gaps, once, and `WD NOT MODELLED`, each
  after a middle dot); it does not import `displays/`, and the label block chooses between it and
  R02's `STAR_SOURCE`/`STARS_WITHOUT_POSITION` where the view's sky is wired (T13.c, with the
  sprites). The low setting's fainter sprite magnitude is T13.c's selection; the values stay
  provisional until T17.
- **Deviations in T12, as built (2026-10-03).** Built: `view/sky/model.ts` (`SkyModel` with
  `request`, `response`, `stars`, `band`, `stale`; `skyRequestReason(held, { request, cameras })`
  naming `arrival`, `expired`, `jump` or `parallax`; `SkyCamera`, `bakedBeyondM`,
  `nearestStarBeyondM`, `PARALLAX_BASELINE_M`, `PARALLAX_THRESHOLD_PX`), `view/sky/limits.ts`
  (`eyeLimitAt(source, direction, fieldFactor)` over `EyeLimitSource { band, faceTexels,
requestFieldFactor }`, `fieldFactorOffsetMag`, `DEFAULT_FIELD_FACTOR`), `decodePayload.ts`
  (`decodeSkyPayload`, `transferablesOf`, the pure work of `decode.worker.ts`, added to
  `tsconfig.worker.json`'s `include`) and `useSky(request, cameras, { createDecoder })` with
  `createWorkerSkyDecoder`, returning `SkyView { model, failure, pending }`. The caller builds the
  request (observer, time, limits, N_max); `null` asks nothing. A request in flight for another
  arrival is cancelled; a failure is held and not retried until the next arrival (no timer: a
  bulk census may take minutes), while a request the link cut off (`link_lost`, `aborted`,
  `superseded`) is asked again once the link returns. A `SkyCamera` is the camera's galactic
  `position` with its field of view and width, and the rule measures its offset from the held
  sky's observer. The rule's fifth reason, `limits`, asks again when a view asks for more than the
  held request did (a camera limit deeper by over 0.05 mag, a larger N_max, the eye or other eye
  parameters, another cone); a shallower limit is the cull's. `useSky` returns no model for a held
  sky of another arrival. `createWorkerSkyDecoder(start)` takes the worker's starter (tests pass a
  fake) and settles waiting decodes on a load error, an unreadable reply or a failed post. The
  test fixtures are `test/skyFixtures.ts`. **Moved to T13.c**
  (approved by the orchestrator 2026-10-03): wiring `useSky` into `ViewDisplay`, retiring R02's
  interim field and its label where the sky has arrived, and composing the request's observer
  from `barycentreAt` and the camera, so that the label never claims the sky while the view still
  draws the interim field.
- **Deviations in T13.c, as built (2026-10-03), with T12's view wiring.** `select.ts`
  (`selectSkySprites(stars, kept, spriteBudget, camera)` → `SkySelection { sprites, baked }`:
  the budget's brightest of the kept stars, and any star nearer than `bakedBeyondM`, about 9 ly at
  1080p and 60°), `sprites.ts` (`skySpriteStars(stars, indices, cameraFromObserverM)`, each
  star's position less the camera's offset in `f64`), `camera.ts` (`cameraGalacticPosition`,
  `cameraFromObserverM`), `eye.ts` (`DEFAULT_EYE_OBSERVER`, `SkyEyeObserver`, `eyeDto`; T13.e
  adds the fixture test against the Rust defaults), `viewSky.ts` (`viewSkyRequest`,
  `viewSkyLimit`, `limitTriple`, `DARK_SKY_CD_M2`) and `spriteHdr.ts`
  (`SKY_SPRITE_HDR_MATERIAL`, `STAR SPRITES HDR`, renamed `POINT SPRITES HDR` in T13.e since R07 draws point bodies through it: R02's `starSprite.wgsl` composed with an
  identity `agxSprite` in place of `toneCurve.wgsl`'s, registered in `WGSL_CATALOGUE`). R02's
  `DrawOptions` gains `skyStars` (`SpriteStar { id, direction, illuminanceRgbLx }`), which the
  sprite path draws in place of the scene's interim stars; the interim stars pass through the
  same `SpriteStar` form, so their sprites are unchanged. `displays/view/useViewSky.ts` asks the
  sky on the published run (4 Hz) for the server's scene where its system's position is known,
  culls it to the view's limit and selects its sprites; the stage draws them each frame and the
  label block's `STARS` line reads `skyLabelValue` (R02's count line hidden) once it has arrived,
  while R02's interim field and labels stand until then; `ViewSky.pending` (added in T13.e for
  R07's lighting label) says a sky is asked and not answered. The `VIEW` display's role is `eye`, so
  it asks the eye's limits and states their deepest; a camera view asks its noise-floor limit at
  a dark sky of μ 24 (`DARK_SKY_CD_M2`) until the band layer (T13.d) gives a texel's background,
  and at a manual exposure's triple or else R02's default `MAN` triple until R07 states the
  metered triple (R07.T13.e replaced `limitTriple`: the request and the cull take the view
  camera's deepest triple at every exposure, `deepestTriple`, and the label the shown exposure's,
  `viewSkyLabelV`). Until T13.g bakes the cube, the stars beyond the sprite budget are not drawn,
  and the high setting's N_max and sprite budget are used, the view not yet taking a quality
  setting. `useSky` makes its decoder only once a payload is in hand (and the effect still live),
  so that a request never answered starts no worker. A sky is drawn and labelled only for the
  system it was asked about while that system's position is known; otherwise the interim field and
  its labels stand. `just test-render` compiles `STAR SPRITES HDR` (2026-10-03, exit 0).
- **Deviations in T13.d, as built (2026-10-03).** `view/sky/band.ts` exports `BAND_MATERIAL`
  (`STAR BAND`: a full-screen triangle at infinity whose fragment turns its view ray back to the
  galactic axes by the transpose of `frame.viewRotation` and samples the band cube, bilinear,
  additive with alpha 1, so R07's meter class is kept), `bandTexels(band, faceTexels,
culledIlluminanceLx)` (each texel's luminance in its chromaticity's colour of unit luminance,
  plus the culled stars' illuminance over the texel's exact solid angle, clamped at 65,504) and
  `BandLayer` (`update`, which makes the `rgba16float` cube once per face size through
  `createTexture` and uploads half floats, `draw(exposureScale)`, `dispose`, releasing the cube
  through T13.h's `releaseTexture`). `half.ts` is the half-float encoder (`toHalfBits`,
  `toHalfArray`, `fromHalfBits`, `HALF_MAX`), since `Float16Array` is not in every runtime the
  tests run under. The band's HDR draw is the only variant (the wireframe draws no band); it is
  registered in `WGSL_CATALOGUE` and checked by `smoke/sky.ts`'s `checkSkyBand` on an
  `rgba16float` target the check makes. The band layer is not yet wired into a view: no view
  draws into an HDR scene target until R07.T7.
- **Deviations in T13.e, as built (2026-10-03).** `view/post/` already held R07's `METER_CLASS`
  (`meter.ts`) and `GlareSource` (`glare.ts`), so this task declares neither and imports them.
  `discFlux.ts` exports `angularRadiusRad`, `rgbOfBvr` (the wire's B, V, R to red, green, blue),
  `discLuminanceRgb(host, mu)`, `discIlluminanceRgbLx(host, rho)` (π L̄ sin²ρ) and
  `discExcessLuminanceRgb(host, exposureScale)` (the disc-averaged luminance above 65,504 ÷ the
  scale, by 256 rings, R07's `excessLuminance`). `disc.ts` exports `DISC_MATERIAL` (`STAR DISCS`:
  a full-screen triangle at infinity that discards outside the disc, sin θ from a cross product for
  the Sun's 0.27° in `f32`, the law per channel, clamped at 65,504, alpha `METER_CLASS.hostDisc`
  with no blend), `DISC_MIN_DIAMETER_PX` (3), `EYE_GLARE_REACH_RAD` (45°) and `HostDiscLayer`
  (`frame(placements, camera, viewport, exposureScale)` → draws and the sprites of discs under
  three pixels, of the same illuminance; `glareSources(camera, viewport, role)`, the plan's
  signature with the view's role added, keeping a disc within the frame's half-diagonal, plus 45°
  for the eye). A `HostPlacement` is `{ host, direction, distanceM }` from the camera, built by
  `hostPlacements(scene, hosts, pose)` through R07's join (decision-r07-t8a.md, R06 coordination):
  `HostDiscDto.star` is the star's body index, so its body is `formatBodyId({ system, bodyIndex:
star })` and the disc sits at that body's drawn centre; a host the scene lacks is left out.
  `DiscFrame.draws` are `DiscDraw { star, item }`, keyed for R07's painter order. The reach rule
  measures the angle past the frame's nearer edge plane (left/right or top/bottom) less ρ: a camera
  keeps a disc touching the frame, the eye one up to 45° beyond it. A host drawn as a sprite casts
  no glare source, its light being in the sprite. The disc stays hard-edged (the same record,
  item 3). The harness draws the band, not a sprite, over the disc to check the meter class is kept:
  both are R01's `additive` mode.
  `eye.ts` (from T13.c) is pinned to the sim's `EyeObserver::default()` by
  `packages/protocol/fixtures/eye_observer.json`, which a Rust test in `sky/eye.rs` and the client's
  disc test both read. The harness's `checkSkyDisc` draws a disc and the band over it on a target
  of its own and checks the centre's luminance, the meter class kept under the band, the clamp,
  and nothing lit outside. _Since 2026-10-06 the draw lies on the disc's limb's plane, not at
  infinity, and since R07.T19.e it is a quad over the disc's screen rectangle, with none drawn
  wholly off the view (the next two entries)._
- **The host discs at their limb's depth (2026-10-06; R07's shading lane, the follow-up to R07.T9
  queued before R10).** Recorded in full in R07's Risks, "The star's disc at its limb's depth".
  - **Why.** At depth 0 (infinity under reversed-Z), a mesh body beyond a star showed over the
    star's disc: R07.T9's figure writes its depth, which the disc then failed. R07 ruled it a
    stated limit for RM3 (2026-10-05), with this follow-up due before R10's depth writers.
  - **The depth.** `disc.wgsl`'s vertex stage puts the full-screen triangle on the camera's polar
    plane of the star's sphere, d cos²ρ along the axis, as R07.T9 puts a mesh body's limb. Its
    reversed depth n (u · axis) ÷ (d cos²ρ) is affine on the view. The plane lies inside the star
    along every ray that meets the disc, so whatever is nearer than the star hides the disc and a
    mesh body beyond it is hidden. The disc still writes no depth.
    - `DISC_MATERIAL` gains the uniform `inverseLimbDistance`, 1 ÷ (d cos²ρ), m⁻¹. Its other
      uniforms and its fragment stage are unchanged.
    - `HostDiscRecord` and `hostDiscRecord` hold what a draw carries, `rasteriseHostDisc` is the
      shader's `f64` twin, and `DiscDraw` gains `record`.
    - Where nothing writes depth, every view draws as before: the buffer holds 0 under the disc.
  - **Tested.** The depth against the star's two sides and R07.T9's limb plane, and the twin's
    light against the law, the flux and the clamp. A mesh giant behind the star in R07's eclipse
    scene, promoted by a synthetic depth writer, leaves no texel of its own, on the CPU twin and on
    the GPU (`smoke/eclipse.ts`).
- **The host discs' draw bounded, and none off the view (R07.T19.e, 2026-10-06; recorded here for
  R06's lanes, who are told of the change to their files through the orchestrator, as
  decision-r07-small-disc-cost asks).** Recorded in full in R07's Risks, "Deviations in T19.e, as
  built".
  - `HostDiscLayer.frame` gives no draw to a disc wholly beyond a side plane of the view widened
    by `OUTSIDE_VIEW_MARGIN_PX`, 8 px (R02's `sphereOutsideView`). It keeps the disc among those
    whose glare sources it returns, so the eye's 45° reach is unchanged.
  - Each other draw is a quad over `sphereScreenRect` of the star (the whole view where the
    silhouette reaches behind the near plane), not a full-screen triangle. `DISC_MATERIAL` gains
    the uniform `rect` (px), `HostDiscRecord` gains `rect`, `hostDiscRecord` takes it, and the
    twin scans only it. `disc.wgsl`'s fragment stage is unchanged.
  - The full-view triangle lit no pixel outside that rectangle, nor any pixel at all for a disc
    off the view. Tested against the twin over the whole view, on a sweep of disc sizes and
    places.
- **Deviations in T13.g, as built (2026-10-03).** `view/sky/bake.ts` exports `bakeSkyCube(engine,
input)` over `BakeInput { directions, illuminanceLx, faceSizePx, name }`, returning `BakedCube {
cube, peak, faceSizePx, path }`, and `bakeSkyCubeOnCpu`, `releaseBakedCube`, `paddedRowTexels`,
  `BAKE_SPLAT` and `BAKE_KERNELS` (clear, peak, mip, pack). On the GPU each face is splatted twice
  into level 0 of the face's `rgba32float` chain, which is the splat's target (flux and a count):
  a first pass over the six faces finds the brightest texel's luminance by an atomic maximum of
  its `f32` bits, from which the kernels and the cube's draw take the power of two
  (`bakeCommon.wgsl`'s `scaleExponent`, `mips.ts`'s `peakScaleExponent`); the second sums the mips
  as flux and solid angle (the first step and level 0's pack reading the solid angles from a
  buffer, computed in `f64` once per face size and narrowed, since the four-corner formula cancels
  badly in `f32` at 3,072²) and packs each level into a staging buffer of one face of one level,
  copied with `writePackedCubeLevelFromBuffer`'s new optional `face` (R01 extended, approved
  append-only). The splat's points buffer starts with a header (face, size), rewritten before each
  face's draw, so that one splat serves the six faces and a star off the face is clipped; level 0
  is cleared by a kernel first, the splat loading its target. The transients at 3,072² are the
  chain (201 MB), the solid angles (38 MB) and the staging (38 MB), so a bake's peak with the
  300 MB cube is about 580 MB against Design note 21's 540; they are released in a `finally`,
  with the cube and peak too on a failure. The peak is kept with the cube (`sky-cube`). The CPU
  fallback holds the six faces at once (some 100 MB at the low setting's 1,024²; the high
  setting's devices have `float32-blendable`). `cubeLayer.ts`'s `SkyCubeLayer` draws the cube in
  its two variants, `CUBE_DISPLAY_MATERIAL` (`BAKED STARS`, toned by `agxSprite`, the
  wireframe's) and `CUBE_HDR_MATERIAL` (`BAKED STARS HDR`, linear), a full-screen draw at infinity,
  and makes its handles again on a device restore; the cube is a separate draw rather than part of
  `BandLayer`'s, which T13.d's ruling text leaned to, keeping the band and the cube to their own
  styles. R02's `WireframeRenderer.render` and `frame` gain an optional `background` list, encoded
  after the occluders and before everything else. The view's use of the bake (baked once per sky,
  shared between views) is T14's `SkyCubeCache`. The harness's `checkSkyBake` checks the WGSL
  packer bit for bit over 10⁴ texels, the GPU splat against the CPU splat to 10⁻⁶, the whole bake
  against the CPU bake to one mantissa step at levels 0 and 5 (each cube's own scale undone),
  finiteness, and the two memory categories.
- **Deviations in T14, as built (2026-10-03).** `view/sky/cache.ts` exports `SkyCubeCache`
  (`acquire(view, request)`, `release(view)`, `size`) over `CubeRequest { stars, baked,
bakeInput }`, and `skyCubeCacheOf(engine)`, one cache per engine's device. A cube is shared by
  every view that bakes the same sky's stars (the same `SkyStars`, the same baked indices) and
  released with its last view (T13.h's releases). A cube's stars are placed from its sky's
  observer, so baking it again for a camera moved past the parallax rule would give the same
  pixels: the re-bake comes with the new sky that `useSky`'s request rule asks from the new place,
  and two views far apart (two cameras across the nuclear disc) ask skies of their own and hold a
  cube each. A restore after a device loss forgets the cubes, which died with the device, and the
  views bake again. The `VIEW` display acquires its cube each frame (the drawn sky memoised on the
  model, the view's limit and its size, so that it keeps its identity across the 4 Hz published
  runs and is baked once), draws it through `SkyCubeLayer`'s display variant in the wireframe's
  background slot, does not bake a sky again after its bake failed until another sky or a
  restore, and releases it on cleanup; a device without `float32-blendable` bakes at the low
  setting's face size, read at each bake. Each view asks its own sky (`useSky` per view), so two
  views share a cube only when they share a sky model: a client-wide sky for its views, the
  brainstorm's one census per arrival, waits on the views' hosting (R07's main screen, and the
  three-canvas proof by hand). The view's cube wiring has no test of its own (the fake view engine
  makes no packed cube), only the cache's.
- **The luminosity function ignores binary evolution.** T5's quadrature, like `mean_present_mass`,
  treats primaries and companions as single stars, while the census since P11.T11 reads
  pair-evolved states. The band's faint light is unaffected to first order; blue stragglers and
  mergers brighter than the cut are listed by the census itself. The caps (T7) read the envelope,
  which T16.b widens. T5.c's comparison with realised cells measures the difference. Accepted
  2026-10-02 (decision record item 3); T5.c measures both the integrated light and the bright end.
- **Merges with the galaxy branch.** `origin/galaxy-generation` adds the `extinction_map`,
  `extinction` and observed-mode kinds to `envelope.rs`, `REQUEST_KINDS`, `requests/mod.rs` and the
  protocol package's `index.ts`, the same lists T10 and T11 extend; whichever lands second merges
  by hand and reruns `just gen-protocol`. The two histories share no merge base.
- **The census's cost** rests on the brainstorm's estimates (open question 19): 5–10 CPU-seconds
  near the Sun and 400–800 in the inner bulge. At bulk priority it cannot starve the charts, but in
  single-player it shares the machine with a descent. If the inner bulge is too slow, the fallbacks
  are, in order: the sorted mass words above (a bump); a coarser cap rule for layers whose
  candidates dominate; caching the census by observer cell across sessions in memory.
  _Superseded 2026-10-05 by the next item: the 5–10 CPU-s are retired, the sorted mass words skip
  nothing near the Sun, and the fallbacks are T17's. A running bulk job is never preempted, so
  T11.d sizes census jobs to about 50 ms._
- **The census's cost (measured 2026-10-05, `decision-r06-census-cost.md`).**
  - Near the Sun at today's caps the cold census costs about 1.6 × 10⁶ CPU-s, 30 hours on 15
    workers. 98.4% of it is `SystemStars::generate`: 0.9, 4.2 and 8.4 ms a system in C, D and E.
  - The flux bound passes 83%, 99.6% and 99.99% of the 4 × 10⁸ records past the floor, and after
    T16.b nearly all of them. The envelope's running maximum over companion masses holds a giant
    companion at the turnoff for almost any old system, so with n = 5 the bound reaches M_V −7.5
    or brighter.
  - The brainstorm's age skip would pass every one.
  - Some 46,000 stars are listed. E's records beyond 8 kly are 92% of its work for about 290 of its
    5,586 stars.
  - No exact census to these caps can cost under about 10³ CPU-s, since it must place each of some
    3 × 10⁸ systems.
  - The ruling keeps Design note 9's completeness and the oracle, retires the 5–10 CPU-s, and
    orders the levers T8.f, T8.g (plan 11's two asks), T8.h, T7.b, T8.i and T11.d. The new budget:
    the first sky in 10 s on the dev machine, nearest first; the full sky in 4,000 CPU-s; a warm
    sky in 25% of cold. The estimate after all of them is 2,500–4,500 CPU-s.
  - T7.b, T8.i and T11.d waited on the owner's sign-off. A decision agent advised on its four
    questions, and its advice was adopted on 2026-10-05 under the owner's standing delegation
    (`decision-r06-census-cost-signoff.md`). The budget is accepted as the eye's only, and the
    camera's cut is benched and ruled separately. Nearest first is accepted, with a 500 ly first
    shell and partial replies that list only the stars within their radius (T8.i, T11.d). Caps by
    direction are accepted (T7.b). The optional looser criterion is declined, and nothing is
    authorised in advance (T17's fallbacks).
  - The nuclear disc's census is bounded by generating its listed systems and is measured in T17.
- **The census budget is the eye's (cut 7.95; `decision-r06-census-cost-signoff.md`).** A camera
  view asks 10.06: some 4 × 10⁵ stars (Tycho-2 has 3.8 × 10⁵), beyond N_max's 3 × 10⁵, and
  plausibly 5–10× the work (an estimate), more if the caps reach the bulge. The bridge's main
  screen is always a camera, so until then the accepted budget covers the single-player cockpit
  and nothing a bridge crew watches. It is ruled once benched (T8.g, T17).
- **The laptop (`decision-r06-census-cost-signoff.md`).** On a 4-core laptop the figures are about
  4× longer, and a census contends with the integrated GPU for power. Every single-player session
  start is a cold census until T11.e. T17 measures a 4-worker proxy here and the frame time during
  a census, and the owner the laptop itself.
- **Prefetch, an open point with the jump-drive plan (`decision-r06-census-cost-signoff.md`).**
  Prefetch on universe open and on a plotted jump is a small T11 follow-up, not a condition. When
  the server opens a universe, it knows the ship's saved position and time, and can start the sky
  census while the client is still starting. When a jump target is plotted or the drive armed, it
  can run the destination's census at bulk priority. No universe time passes in transit, so that
  census is valid at arrival within Design note 13's `valid_until`; unused, it has still warmed
  the cache. Each census is a pure function of its query, so determinism is untouched. The
  priority would be the current sky's first shell, then the destination's first shell, then the
  current sky's outer shells. It needs the jump-drive plan's console to send the plotted target.
- **Merger products outshine the flux bound until T16.b (found by T8.e's oracle, 2026-10-04).**
  The per-record flux bound reads the envelope at `max_star_mass(m₁)` = m₁ and the record's age.
  But plan 11's pair evolution (P11.T11) can merge a pair into one star of up to 2 m₁, which then
  evolves as the heavier star it has become. Two of T8.e's slow identity tests, near the Sun at
  cut 7.95 with the eye, each miss one such star. The cell floors were at their bands' lower
  edges, so the per-record bound skipped both:
  - **B within 500 ly:** system `0x21fe56487ff00001`, cell B (−14, 1608, −2), m₁ 0.638 M☉. Its
    pair merged into one 1.187 M☉ first-giant-branch star (the other `NoRemnant`) of M_V 2.17,
    listed at V 7.53 from 354 ly. The flux bound was M_V 3.78.
  - **C within 1,000 ly:** system `0x42046c99ff00000a`, cell C (17, 806, −4), m₁ 0.758 M☉. It
    merged into a 1.229 M☉ first-giant-branch star of M_V 0.85, listed at V 7.44 from 620 ly. The
    flux bound was M_V 2.70.

  So the plan's expectation that T8.e's identity tests pass before T16.b did not hold. T8.b's
  sample found no straggler above the bound, but a sphere of 10⁵–10⁶ systems holds some.
  Decided by the orchestrator (2026-10-04): T8.e commits the two tests unchanged, ignored with a
  reason naming T16.b, and excluded by name from the slow profile (`.config/nextest.toml`,
  `[profile.slow]`'s `default-filter`). T16.b widens the bound, removes the exclusion, and both
  pass unchanged. No tolerance moved. The fast identity tests, and the nuclear disc's slow one,
  hold no such star and pass. Until T16.b, a census can miss a merged giant of an M, K or G dwarf
  system: here one of 36 listed B stars within 500 ly, and one of 21,210 listed C stars within
  1,000 ly.

  **Closed by T16.b (2026-10-05).** The bound now reads the envelope at twice m₁ over ages from
  zero. The two systems' bounds move from M<sub>V</sub> 3.78 and 2.70 to −7.78 and −7.95. Both
  tests are back in the slow profile and pass unchanged, except the A/B test's check that B skips
  some systems, which is now a printed figure (see "Deviations in T16.b, as built").

- **The mass skip barely bites near the Sun (found in T8.d, 2026-10-04; ruled: no change now).**
  `cell_floor` reads the envelope over every age, 0 to `MAX_AGE_YEARS`, which allows a giant of a
  band's least mass. So near the Sun every C–E cell's floor is its band's lower edge out to
  hundreds of light-years (D and E to at least 800 ly, C to 200 ly, at cuts 6 to 9 with the eye),
  and the floor rises only for A beyond about 100 ly, B beyond about 200 ly and C beyond about
  400 ly. Inside that, a cell's bright subset is the whole cell, and each record is skipped only one
  at a time, by its flux bound at its own age, after its drift and retarded time. T8.e's identity
  tests show it: within 150 ly at cut 11, layers B to E generated every system. **Candidate
  remedy:** the brainstorm's age skip (open question 13). A candidate's age word is independent of
  the component picked, so it can be tested under every population component the cell can hold,
  over the cell's light-time interval, before the density is evaluated: a candidate too faint at
  each of its possible ages is skipped in the walk, as a light one is by the mass skip, rather than
  after its record, drift and retarded time are built. Design note 8 declined this skip as
  needing the component, a departure marked for the roadmap's corrections. The brainstorm's
  reading needs no component, since it tests them all. Design note 8 also meant the floor's age
  range to be the cell's components', but as built `cell_floor` takes every age. Whether the
  departure stands is the owner's to rule. T17's warm-cache bench
  (`sky/census_near_sun/warm`) measures the present floor against `HYPERION_SKY_CACHE_MB` = 64 and
  decides whether the remedy is due. _Ruled 2026-10-05 (`decision-r06-census-cost.md`): the age
  skip would pass every C–E record near the Sun, so it is not built, and the departure needs no
  ruling (Design note 8). T8.f made the floor O(1), T8.g bounds each star, and T8.h keys the cache
  by magnitude._
- **Extinction per candidate.** Each star that passes the skip takes one `sightline`. Plan 07's
  noise is log-normal with no floor, so no cheaper exact lower bound exists; in the nuclear disc,
  where the zero-extinction test passes almost everything, this may dominate. A per-direction
  profile from the band's rays could pre-screen candidates, but only as a stated approximation; the
  benchmark decides.
- **Humphreys–Davidson (A2).** Until plan 06 rules, E's caps follow tracks that exceed every
  observed star by about 1.3 mag at the top, and E's cap is larger than it will be. The research
  lean is Design note 19's.
- **Interims A1, A3, A4.** Reading `SystemStars` for old emitted times is exact but dear; white
  dwarfs are absent (Sirius B, V 8.4, is missing from a camera's sky); protostars' darkness is ours
  until plan 06 owns it. Each is labelled or tallied.
- **Glare double count.** F = 1.4 was fitted on real fields that include some glare, and the map
  adds glare explicitly; the error is small against the model's own 0.1–0.2 mag, and a field factor
  setting absorbs it.
- **The camera model's defaults** are a full-frame video camera of today at high gain; open
  question 16 leaves its parameters open, and the performance runs and the owner's sense of the
  main screen may move them. They are one table in `cameraLimit.ts`. The model is optimistic for
  old analogue cameras by about 2.5 mag (CAMS, Design note 18), and the split of a large exposure
  change between gain and photons is R07's view camera (decision-r07-exposure-camera): 12 stops of
  gain, then shutter and ND, which takes the limit to V 2.56 at 60° for a sunlit planet at
  EV100 15. A camera view's request asks the camera's deepest limit, V 10.06 at 60°, so it holds
  fewer stars than under R02's old f/1, 2 s default (V 13.7, cut at `MAX_CUT_V`'s 11).
- **The camera cut is in V.** The census and `camera_limit_v` cut in V, so a star redder than the
  Sun that only its camera band term lifts over a camera's limit is not listed (a late M dwarf up
  to 2–3 mag below the cut); its light is in the band. Padding the flux bound by the most negative
  term would cost more census than those stars are worth (decision-camera-eta.md).
- **Binaries.** The flux bound and the envelope are exact for single-star systems today; once plan
  11 wires binary evolution in, R06.T16.b must land with it or the census can miss blue
  stragglers and mergers (P11.T11 has wired it, so T16.b follows T8.e; re-validated 2026-10-02), and its cost (up to about 2.5 times the
  candidates above 0.5 M☉, the research's estimate) is T17's to measure. _T16.b landed
  2026-10-05. Its cost is far above that estimate where B and C stars dominate, since 2 m₁ over
  every age reaches the envelope's late giants ("Deviations in T16.b, as built")._
- **The eye's cut** rests on Crumey's eq. 34 at the darkest pre-pass texel, clamped at a
  colour-corrected 10⁻⁵ cd m⁻² where Blackwell's data give no constraint (decided 2026-10-02; μ 25.6
  in starlight); a view darker than that is drawn to the clamp's limit, 7.99 at F = 1.4.
- **`float32-blendable`.** The GPU splat needs it; without it the CPU splat is exact but slower,
  and a bake on the high setting's 3,072² faces on the CPU is unmeasured (T17 records it).
- **No zodiacal light**, because plan 14 has no zodiacal cloud; inside a dusty system the background
  and the limits are too dark by up to 0.3 mag near the ecliptic (23.3 against 24 at the Sun's
  ecliptic pole). A zodiacal cloud belongs to plan 14.
- **Band resolution.** 64² faces are 1.4° texels; nearby dust lanes are resolved, distant thin
  lanes are not. The face size is a server constant, measured by `sky/band_near_sun`.
- **Licences of the spectral grids** are silent rather than permissive. Only integrated tables are
  committed, with citations; if the project is ever sold, the authors should be asked (research
  finding).
- _Closed by R01 Design notes 23 and 24 (2026-09-30): Babylon was dropped, no engine internals
  remain, and the adapter creates the cube on the raw device._ **Babylon internals.** This plan
  would have added `_hardwareTexture` to R01's pinned internals.
- **Asked by later plans, now designed here.** R07's two asks are met by R06.T13.e: the disc pass
  writes `METER_CLASS.hostDisc` in the HDR target's alpha, and `glareSources` returns R07's
  `GlareSource` per disc, for eye views also when it is up to 45° outside the frame (R07 Design
  notes 10 and 12). R08's ask, `StarColour.bake_spectrum: [f64; 15]`, is met by R06.T3.c and
  delivered on `HostDiscDto.bake_spectrum`: each colour-table row's spectrum as bin averages at
  `BAKE_WAVELENGTHS_NM`, normalised to unit photopic illuminance (Design note 6). R08 pins the
  fifteen bins (centres 392.67 + 25.33 k nm, R08's `BAKE_WAVELENGTHS_NM`, which this plan's
  `sky::colour::BAKE_WAVELENGTHS_NM` mirrors) and reads the unit-lux normalisation (Σ 683 ȳᵢ Sᵢ Δλ
  = 1 lx); `lux_per_v0` converts to V-band terms where a caller needs them.
- **M-star spectra at 1 nm, open (asked by R08's Risks).** R08's per-absorber curves of growth
  integrate r̄_c S e^(−σu) dλ over narrow molecular bands; for M dwarfs, whose TiO bands at 590–630
  and 705–760 nm overlap Chappuis and methane's 727 nm band, fifteen bins are not enough (R08's
  Risks). R08's lean is a model spectrum at 1 nm for M stars (PHOENIX, Husser et al. 2013, A&A 553,
  A6), some 380 values a row over the PHOENIX rows below about 3,900 K only. It is not in this
  plan's scope: it adds a second committed table from the same unlicensed grid at full resolution,
  which waits on the owner's data-licence ruling. The interim for R08's curves of growth is R08's to
  choose; the ask stays open in the roadmap's between-plans table (R06 owner, R08 asking).
- **Deviations in T1, as built.** None: the R06 row of galaxy plan 04's reserved kinds and plan 06's
  "Asked by rendering plan R06" heading with A1–A4 are drafted for the galaxy plans' owner
  (`fb5b47c`).
- **Deviations in T6.a, as built.** `generate_cell_where` draws each candidate's mass through
  `placement::record::candidate_mass` (factored out of `SystemRecord::of_candidate` with the same
  calls) and evaluates a kept candidate whole, drawing its mass word a second time. The 500-cell
  test passes over cells within 2,000 ly of the centre, whose 10⁵ systems would take a debug build
  minutes, and takes one bulge cell at 3,000 ly instead; it covers the five stellar and two
  substellar layers at ten floors.
- **Deviations in T3, as built** (rulings of 2026-10-02 by delegated decision, R06.T3 lane).
  - _Sources._ ATLAS9, TLUSTY OSTAR2002, Koester's DA and TMAP spectra are the Spanish Virtual
    Observatory's ASCII copies (collections `Kurucz2003` at [M/H] 0, `tlusty_ostarbin` at Z/Z0 1,
    `koester2` at log g 6.5–9.5 by 0.5, `tmap` at He mass fraction 0 and 0.3); PHOENIX is the
    Göttingen HiRes FITS. Levenhagen 2017 is not used (the plan's "Koester or Levenhagen"). The
    grids: not white dwarfs, PHOENIX 2,300–3,400 K, ATLAS9 3,500–27,000 K, TLUSTY 27,500–55,000 K,
    TMAP H+He (Y 0.3) 60,000–100,000 K, a blackbody at 120,000–500,000 K, log g 0–6 by 0.5; white
    dwarfs, Koester 5,000–80,000 K, TMAP pure H at 90,000 and 100,000 K, the blackbody beyond, log
    g 6.5–9.5 by 0.5. A node a model set does not hold takes the nearest gravity it holds at that
    temperature (448 nodes in the first grid, 2 in the second). `AtmosphereGrid::MainSequence` and
    `Giant` read the same grid, which spans log g 0–6.
  - _Licences_ (decisions-r05.md item 4). Every spectral grid, Bessell and Murphy's V (also needed
    as a fetched dataset, `bessell_murphy_2012`), Pickles' library (`pickles1998`, sixteen files)
    and the three limb-darkening catalogues are fetched with checksums (`PROVENANCE.toml` and
    `urls.txt` committed); the CIE tables (`cie_cmf`, CC BY-SA 4.0, CRLF kept, excluded from the
    line-ending hook) and Green 2008's silicon (`green2008_si`, CC0, excluded from Prettier) are
    committed raw; `NOTICE` gains a Data entry for the CIE. The smoke manifest needs only committed
    data: a blackbody at every node and the photopic V(λ) standing in for the V band.
  - _The V zero point_ is BCP98's −21.100 (offset 0.000) applied to Bessell and Murphy's photonic
    V; their own zero point would put V = 0 0.016 mag brighter (§7.2 Tables 3 and 5). Kept, so
    every `lux_per_v0` is 0.016 mag lower than under BM12's own.
  - _Corrected figures._ `lux_per_v0` runs 0 to +0.10 mag above 2.54 µlx from O5 to M6 (Pickles'
    own spectra: O5V 0.000, K5V 0.100, M2V 0.093, M5V 0.074), not "within 0.08": the tests take
    |m| < 0.11. The Pickles colour check holds Δ(u′, v′) < 0.005 for A0V, F5V, G2V, K0V (Mamajek's
    2022.04.16 T_eff, log g from its masses and radii) and G8III (Pickles' own adopted 5,012 K), with
    four measured exceptions in the table's header: O5V 0.0072 < 0.008 (Martins et al. 2005 Table 1,
    41,540 K; it lies off the models' locus at every temperature, likely residual reddening), M3III
    0.0113 < 0.012 (its colour is a 4,240 K model's), M2V 0.0057 < 0.007 at 3,560 K (best fit
    3,260 K), K0III 0.0050 < 0.006 at Pickles' 4,853 K (best fit 5,050 K); and, restored to the
    plan's list after review, K5V 0.0075 < 0.008 at Mamajek's 4,440 K (best fit 4,200 K, Pickles'
    own 4,188 K), a fifth exception. Evaluating every spectrum at Pickles' own adopted T_eff was
    tried (the orchestrator's re-ruling): it leaves four exceptions (M2V 0.0057, K0III 0.0050, O5V
    0.0075, M3III 0.0113) and puts G2V at 0.0046, so the five-exception set on Mamajek's dwarf scale
    was kept, as that ruling provided. An M dwarf is less red
    than its blackbody by 0.019, 0.015 and 0.011 in CIE 1960 uv at 2,900, 3,000 and 3,100 K (0.008
    at 3,200 K), so the test takes 2,900–3,100 K. Pickles' lux check runs to M6V (0.067 mag, in
    bracket).
  - _Review fixes and records._ T3.a–c were fitted and committed as one change, so `star_colour`
    stays at revision 0 (there was no earlier table to bump from). `StarColour`'s fields are private
    with getters, adding `blue()`, `red_green()` (the unrounded chroma; `chroma()` narrows to `f32`)
    and `tables::star_colour::LUMINANCE_RGB`; `extinction_ratio()` is red, green, blue, the table's
    column order, while `HostDisc`'s arrays are B, V, R, so a consumer building `HostDiscDto`
    reverses it. log g stays a bare `f64` (`star_colour`, `limb_coefficients`, `HostDisc::log_g`,
    `surface_gravity`) as Provides sketches it, not base's `Dex`. The fit's integration tests are
    named `star_colour_*`, so `cargo test -p hyperion-fit star_colour` selects them; T3.b's Pickles
    checks run there, not under `cargo test -p hyperion-sim sky::colour`. The fetched V band falls
    back to the photopic stand-in only when it is not fetched (a file failing its hash is an
    error). `NOTICE` carries Green 2008 as well as the CIE. Golden pins of `sky::colour` and
    `sky::disc`'s interpolation are left to T17's goldens (determinism audit).
  - _T3.c as built_ (decision-camera-eta.md). `CAMERA_ETA_SUN` is 2.9557 (the ATLAS9 grid's,
    geometrically interpolated at 5,772 K and log g 4.438 so the Sun's interpolated term is 0 to
    10⁻¹⁵; Pickles G2V gives 3.02), and the shared fixture
    `packages/protocol/fixtures/camera_eta_sun.json` pins it for the client's `etaSun`. The term is
    stored unrounded and saturated at the wire's −4.0 and +3.97: only PHOENIX's 2,300 K rows at log
    g 0–0.5 (−4.02) reach it. A channel's effective wavelength weights the spectrum by the positive
    part of its Rec. 709 colour-matching function; the ratio is plan 07's law at it over the law at
    the V band's photon-weighted effective wavelength. `BAKE_WAVELENGTHS_NM` is pinned by
    `packages/protocol/fixtures/bake_wavelengths_nm.json` (R08 unbuilt; its constant reads the same
    file). The bake spectra are a second `static` per grid (`NORMAL_BAKE`, `WHITE_DWARF_BAKE`).
  - _Shape._ The task is the module `tasks/star_colour/` (`columns`, `photometry`, `pickles`,
    `spectrum`), slow class. The table's rows are `static` arrays of eight-column `[f64; 8]` (`r`,
    `g`, `lux_per_v0`, `sp_ratio`, `camera_band_mag`, red, green and blue `A_c ÷ A_V`), the bake
    spectra `[f64; 15]`, written as plain source text, comma-separated with no spaces and with
    `unreadable_literal` allowed, so that the file (497 KB) stays under the repository's 500 KB
    hook; `sky::colour` reads the columns by index.
    The Pickles comparison, the M-dwarf check and the bake integrals are in
    `crates/hyperion-fit/tests/star_colour.rs` (they need the fetched data and say so when it is
    absent); the sim's `sky::colour` tests pin the committed table. `sky/mod.rs` was created here
    (T2 had not landed); expect a trivial merge with T2's.
- **Deviations in T4.a, as built.** `limb_darkening` is a fast task over three fetched VizieR
  catalogues (`claret_southworth_2022` Table 3 at [M/H] 0 and 2 km/s, `claret_southworth_2023`
  Table 9, the first truncation method M1, and `claret_2020_white_dwarfs` table gh, both stored
  decompressed). Grids: PHOENIX-COND 2,300–3,900 K and ATLAS 4,000–50,000 K at log g 0–6 by 0.5
  (ATLAS clamped above 5 and at each temperature's least gravity); white dwarfs DA in LTE
  3,750–35,000 K and DA in non-LTE 40,000–100,000 K at log g 6.5–9.5 (the non-LTE grid's gaps,
  such as log g 8.0, interpolated linearly in log g). Rows are `sky::disc::LimbRow` (c and α in
  B, V, R) as `static` arrays. The solar row comes out c 0.7837, α 0.6884, disc average 0.7993.
  The test "I(0.1) within 0.015 of Cox 2000's polynomial" is taken against Pierce and Slaughter
  1977's quadratic at 5,522 Å (Table III, 0.390), the source of Cox's table, since Cox 2000 could
  not be read; the power-2 law gives 0.377, inside the tolerance by 0.002, and the quadratic
  itself overestimates the limb by about 0.01 against fifth-degree fits (Neckel and Labs 1994,
  0.382 at 550 nm).
- **Deviations in T4.b, as built.** `HostDisc`'s fields are private with getters. Its
  `mean_luminance` per channel (B, V, R) is the photopic mean L̄ times the star's linear Rec. 709
  blue, green and red at unit luminance, so the channels' Rec. 709 luminance
  (`sky::disc::channel_luminance`, added) is L̄; the test "π × mean luminance × sin²ρ equals the
  illuminance from V within 1% in V" is taken on that luminance. L̄ = 2.54 µlx × `lux_per_v0` ×
  10^(−0.4 M_V) × (10 pc)² ÷ (π R²), with `sky::disc::V0_ILLUMINANCE_LX` (added; Allen's value).
  M_V is plan 06's `absolute_magnitude_v`; a white dwarf, which plan 06 leaves without one until
  A4, takes M_bol − BC_V(T_eff) from the dwarfs' corrections (up to about 0.6 mag off at 4,000 K,
  `photometry`'s own caution). A star with no V (neutron star, black hole, merged-away, substellar)
  has no disc. The grid follows the phase: white dwarfs theirs, protostar to main sequence the
  dwarfs', every other living phase the giants'. Each star's state is `SystemStars::state_at(t)`
  (pair-evolved); `host_discs` does not read its `galaxy` argument yet. `StarIndex::from_body`
  (added to plan 11's `multiplicity::hierarchy`) gives the index.
- **The eye's darkest background (decided 2026-10-02, T2).** Design note 2's clamp at μ 27 (8.64 at
  F = 1.4) is replaced by Crumey's own: the threshold is constant for a background, colour-corrected
  to Blackwell's light, at or below 10⁻⁵ cd m⁻² (Crumey 2014, §2.3, eqs. 47–52; §3.2, eq. 71, ζ =
  1.150 × 10⁻⁹ lx), the bound Design note 2 itself cites. The zero-background limit at F = 1.4 is
  7.99, reached at μ 25.6 in starlight (ρ₀ 2.26); T2's figures 8.17 at μ 26 and 8.64 at μ 27 are
  7.99. Design note 5's eye cut is at most about 7.99 + 0.45 + 0.1 = 8.54 (not 9.2); near the Sun,
  whose darkest texel is about μ 24.3, T9.d's 7.96 ± 0.15 is unaffected. Recorded as a brainstorm
  correction in the roadmap.
- **Deviations in T2, as built.** `sky::eye` takes its background as a validated
  `SkyBackground { luminance, sp_ratio: SpRatio }` (`SkyBackground::new`, `SpRatio::new`, both
  `Result<_, BuildEyeError>`), so `threshold_illuminance(eye, &SkyBackground)`,
  `naked_eye_limit(eye, &SkyBackground)` and `star_colour_offset(star: SpRatio, &SkyBackground)`
  replace the sketches' `(background, background_sp_ratio)` pairs: NaN and negative inputs are
  refused by type, and the star's offset needs the background's ratio for the MES2 weight.
  `EyeObserver::new` returns `Result<_, BuildEyeError>` (field factor > 0, age ≥ 0, pigmentation
  0–1.2); `veiling_luminance` and `surface_brightness` return `Option` (`None` for NaN or negative
  input; a source beyond 100° gives zero). Added: `luminance(μ)`, `mesopic_weight`,
  `blackwell_equivalent_factor` (which the limit map's glare weighting reads),
  `magnitude_of_illuminance`, `illuminance_of_magnitude`, `DARKEST_BACKGROUND` (10⁻⁵ cd m⁻², a
  `CandelasPerSquareMetre`), `BLACKWELL_SP_RATIO` (1.408) and, in base,
  `SolarLuminositiesV::from_absolute_v` and `consts::SOLAR_ABSOLUTE_MAGNITUDE_V` (4.81, Willmer
  2018). Two technical corrections (approved 2026-10-02): the mesopic fade weighs each light by its
  MES2 mesopic luminance, (m + (1 − m) ρ C) ÷ (m + (1 − m) 1.408 C) with C = 683 ÷ 1699, the
  equal-mesopic-luminance analogue of Crumey's eq. 6, in place of m + (1 − m) ρ ÷ 1.408, which
  over-weighted the rods (the background moves by ≤ 0.005 mag, a red star's offset at μ 16 by about
  0.16); and eq. 34 and eq. 53 differ by 0.026 mag at μ 20 and by under 0.02 only from μ 20.6, so
  T2's test holds 0.03 on μ 20–20.5 and 0.02 beyond (Design note 2's "within 0.02 mag above μ 20"
  is that much loose). MES2's weight takes CIE 191's end tests on the inputs (L<sub>s</sub> ≤ 0.005,
  L<sub>p</sub> ≥ 5 cd m⁻²); a starlit background is scotopic below μ 19.2. The running minimum is
  the threshold held at no less than eq. 34's value at the bump's dark edge, B_equiv = 0.021 567
  cd m⁻² (local minimum of the limit, 5.2446 at F = 1.4; the bump peaks at 0.0471 and closes at
  0.0650). Taking eq. 34's thresholds as those of the B − V = 0.7 star is documented as the plan's
  convention (Crumey offers it "if this is considered the standard", §3.1; read literally his eqs.
  6 and 16 put them at 2,850 K). The pigmentation bound 1.2 (CIE 146's very light eyes) was not
  confirmed from a primary text by the science check (medium confidence). With the MES2 fade the
  starlit limits in mesopic backgrounds move off the plan's scotopic figures: 5.427 at μ 18.8 (plan
  5.42) and 5.256 at μ 17.5 (plan 5.25 ± 0.01); T2's test holds every figure without a stated
  bracket to ±0.01. After review: the field factor is accepted within 0.1–100, an S/P ratio within
  0.01–100 and a background within 0–10¹² cd m⁻², so every limit is finite and `naked_eye_limit`
  cannot panic; `veiling_luminance` refuses a negative angle; MES2's weight is a `PhotopicWeight`
  newtype (0–1), which `mesopic_weight` returns and `blackwell_equivalent_factor` takes.
- **Binary light and the tables' cost (decided 2026-10-03, `decision-r06-tables.md`).**
  R06.T5.c's sample found pair-evolved light 9% and 21% below single-star light in the solar
  circle's layers C and D, and 4%, 11% and 10% below it in the bulge's C, D and E. That is
  first-order, against item 3's second-order estimate. T5.d's fit (2.6 × 10⁶ systems a layer in D and E, 2.6 × 10⁷ in
  C) puts it at 2.1%, 10.2% and 21.2% in the solar circle's C, D and E, and 3.2%, 9.7% and 13.3%
  in the bulge's, so the uncorrected band near the Sun would be about 3–5% (0.03–0.06 mag) too
  bright and redder (the gate reading's 2–4%, with C's raised sample), which T17 measures at its
  points. T5.d corrects the light with a fitted, galaxy-independent
  table of pair-minus-single differences, by layer, age, \[Fe/H\] and 1-mag M_V bin. The counts the
  caps read take only the excess, and the envelope and the rule's bound are untouched (T16.b).
  Those tables are bounds, and a deficit cannot be allowed to loosen them. The fitted correction
  is only as good as its sampling (gate: 1σ under 2% of a layer's light) and as plan 11's pair
  laws. A galaxy with a non-default mass function takes no correction unless it is fitted.
  On cost: a full table build took 64–142 s on one thread and holds about 55 MiB. The envelope,
  which depends on no seed, is now a checked-in fit. The tables depend on the seed, so they are
  built once per galaxy (one reference time, +H, serves the whole ±1,000-year window) as parallel
  pool jobs, with T5.e's panels at the tracks' phase ends and its node counts under gates on
  what the tables' readers integrate (1%; `decision-r06-t5e-gate.md`, `-2.md`). The first sky of
  a session waits on that build: about 5–10 s on the dev machine, and longer on a 4-core laptop.
  T17 checks it against 30 CPU-s and a 10 s cold first sky, with a sim-fingerprinted disk cache
  as the fallback.
- **Deviations in T5, as built.** `sky::luminosity` and `sky::photometry` as Design note 7 sets
  them out, with these differences. Each living phase of a node's track is cut at its segment
  ends, its knots and 32 equal parts, and each part is read at three-point Gauss–Legendre's nodes
  (three sub-parts weighted 5:8:5) rather than at its middle; doubling the samples moves no bin by
  1% of the function's light or stars. **Metallicity:** a table is not read at the population's
  reference \[Fe/H\] but over each component's distribution at three Gauss–Hermite nodes (mean,
  mean ± √3 σ; 2/3, 1/6, 1/6), rounded to 0.05 dex and held within the tracks' Z clamp (\[Fe/H\]
  −2.30 to +0.18), since V light is convex in \[Fe/H\]. A component with a radial gradient (the
  thin discs) holds one table per quarter dex of mean \[Fe/H\] over the range its probes at 0, 1
  and 3 solar radii find, read through `LuminosityTables::get_at(component, layer, &PointLy)` by the
  mean at the point's cylindrical radius (`get` reads the solar circle, where the gradient gives a
  solar mean, `SOLAR_RADIUS_LENGTHS` thin-disc scale lengths out). R06.T5.c found the realised
  inner galaxy 7–11% fainter than solar-circle tables, the metal-rich inner thin disc being
  fainter; with the bins it agrees. Added: `LuminosityFunction::{colour_fainter_than,
stars_per_system, dark_per_system, remnants_per_system}`, `LightColour` (flux-weighted
  `lux_per_v0`, chroma and ρ of the light fainter than a magnitude, from the colour table's
  `colour_of_state`, the per-bin mean colour Design note 15 reads), `sky::photometry::colour_of_state`,
  the table constants (M_V −12 to +20 at 0.05 mag, snapshots at the light ages 0, 10³, 10⁴, 10⁵ and
  2¹⁸ yr); `build(galaxy, time)` takes the time the plan's sketch left implicit. The brown dwarfs'
  layer is plan 13's objects on the substellar branch, single; the rogue planets' layer is dark.
  **Cost (provisional, under load):** a full build took 64–140 s on one thread in release, some 30
  metallicities × 2,240 mass nodes of tracks (about 2.5 s each) and 50 component bins of
  accumulation (about 0.5 s each); the server builds it once per galaxy and time bucket (T11.c),
  and T17's `sky/luminosity_tables` bench records it. The crate's tests share one build
  (`sky::testing`). **Measured (T5.c, slow, 216 cells a layer at the solar circle and 3,000 ly from
  the centre, version 19):** the realised V light agrees with the tables within the test's interval
  in every layer; realised against tabulated light, solar circle A +3%, B −2%, C −6%, D −29%
  (within the interval: 3,310 systems), E +16%; bulge A −4%, B +1%, C −7%, D −10%, E −18% (the
  interval is wide: 4.9 × 10⁵); the pair-evolved light of the same stars is below their single-star light (each `StarModel`
  alone) by 0–1% in A, B and the brown dwarfs, and by 9% and 21% in the solar circle's C and D, 4%
  and 11% in the bulge's C and D and 10% in its E: binary evolution, which the tables leave out
  (decided 2026-10-02, item 3), is a first-order term in C and D's integrated light, though inside
  the test's interval at these sample sizes. The bright end,
  recorded not gated (decided 2026-10-02, item 3), realised against tabulated counts with the
  table's 95% Poisson interval: solar circle E, brighter than M_V −3, 6 against 3.8 (1–8), ratio
  1.59, p 0.36; brighter than −5, 2 against 0.69 (0–3), ratio 2.90, p 0.30; bulge D −3: 9 against
  9.0 (4–15), ratio 1.00; bulge E −3: 133 against 141 (119–165), ratio 0.94, p 0.51; −5: 26 against
  23.8 (15–34), ratio 1.09, p 0.70. No layer's ratio is distinguishable from 1 at these counts, so
  T7 scales nothing; the solar circle's E figures rest on 6 and 2 stars and decide nothing.
  **Found by T5.c:** record 0x81fd865fd000000f overflowed the stack in plan 11's binary engine
  (common envelope and merger recursing on a held bare core), fixed as a P11.T11 fix (plan 11's
  Risks); the held state's own inconsistency is with a decision agent.
- **The tables' build memory and the tests' cost (fix, 2026-10-04).** With T5's metallicity nodes
  and gradient bins, `build_with` held every metallicity's samples to the build's end, about
  150 MB each (2,240 nodes at 32 samples a phase): 17 for the young thin disc's seven bins, 22
  for a full build. `doubling_the_samples_moves_no_bin_above_one_percent` peaked at 4.1 GB
  natively and ran out of wasm32's 4 GiB on wasip1 (CI-13), and most table tests ran 5–20 times
  longer than at CI-11. `build_with` now makes one metallicity's samples at a time, the lowest
  \[Fe/H\] that any bin adds next, adds them to every bin whose next node it is, and drops them.
  Each bin keeps its own snapshots' bins and adds its three nodes in their order, so the tables
  are unchanged bit for bit: bit fingerprints of a full build at +H, a build at twice the samples
  and a primaries-only build at −55,000 yr match the old code's. A build's peak is now one
  metallicity's samples: 304 MiB across those three builds, against 4.2 GB. Each part's share
  reads `born_cdf` once per end, not twice per part. That took a young thin-disc bin's
  accumulation from 3.2–4.6 s to 1.8–1.9 s, and a full build from 136 s to 104 s (test profile,
  under load). The unit tests that read only `get` build a gradient component's solar-circle bin
  alone (`GradientBins::SolarCircle`, test-only), which is the full build's bin bit for bit: three
  metallicities rather than 17. The slow test
  `a_solar_circle_build_is_the_full_builds_bin_bit_for_bit` pins that equality, and its build of
  all seven bins at twice the samples guards the memory. The doubling test now takes 52 s and
  289 MiB natively (217 s and 4.1 GB before), and 61 s and 365 MiB on wasip1. At CI-11 it took
  28 s and 33 s, with one metallicity per component; the rest of the difference is the three
  Gauss–Hermite nodes. T5.d's job split keeps this peak (next item).
- **The job split in stages (T5.d merged with the memory fix, 2026-10-04).**
  - **Before.** T5.d's split (062e32c) made every \[Fe/H\] node's samples, then built each
    component bin from all of them, so it held every node's samples at once. A full build peaked
    at 2.7 GB serially and 2.6 GB on 16 threads.
  - **How it works now.** `TablesPlan` runs in stages, one metallicity at a time from the lowest
    \[Fe/H\]. The Milky Way fixture has 22 stages and 53 bins.
    - A stage's sample jobs are unchanged.
    - Its accumulation jobs, one per bin that reads the node, add into that bin's running
      `BinSums`. The caller holds those sums from `bin_sums` to `assemble`, and they panic if a
      bin's nodes come out of order. After that the stage's samples can be dropped.
    - `assemble` finishes each bin. T5.d's pair correction goes in that finish step, applied to
      the bin's complete single-star sums.
  - **Deviation from the T5.d sketch** (approved by the orchestrator the same day).
    - `track_samples` takes one stage's chunks.
    - `run_accumulate` adds to the caller's sums instead of returning a bin's tables.
    - `BinTables` is gone.
  - **Bits.** Three builds give the same fingerprint and the same `heap_bytes` at 7772faf, at
    062e32c's integration head (d400869) and after the merge:

    | Build                                                  | Fingerprint        | `heap_bytes` |
    | ------------------------------------------------------ | ------------------ | ------------ |
    | Full, at +H                                            | `36d76d03f1cc651a` | 68,775,072   |
    | Young thin disc's seven bins, twice the samples, at +H | `f794503ac9529263` | 29,828,064   |
    | Primaries only, every component, at −55,000 yr         | `0bdedce256b01507` | 68,775,072   |

    Both versions' pool builds on 16 threads give the full build's fingerprint, and so does the
    merge's pool build with lookahead.

  - **Peak RSS, serial** (MiB, test profile, under shared load):

    | Build             | 062e32c | 7772faf | Merge |
    | ----------------- | ------- | ------- | ----- |
    | Full              | 2,735   | 273     | 279   |
    | Twice the samples | 3,346   | 304     | 295   |
    | Primaries         | 2,708   | 249     | 238   |

    The peaks are `getrusage`'s maximum RSS of the test process and its children, the figure
    GNU `time -v` prints (it is not installed here). Each test below ran alone:
    - natively, the doubling test peaked at 4,072 MiB and took 228 s at 062e32c's integration
      head, against 304 MiB and 53 s after the merge;
    - on wasip1, after the merge, the doubling test passed in 65 s at 363 MiB;
    - `a_solar_circle_build_is_the_full_builds_bin_bit_for_bit` passed in 120 s at 307 MiB.

  - **The pool.** It holds:
    - one stage's samples, shared by its threads: 151 MiB at 32 samples a phase;
    - every bin's sums, about 65 MiB;
    - per thread, a sample chunk of at most 64 nodes, about 4 MiB.

    On 16 threads the staged pool took 23.2 s wall and 237 MiB. 062e32c's took 18.8 s and
    2.6 GB. Within a stage only the bins that read its node accumulate (1–22 jobs), so
    accumulation took 13.2 s of the staged pool's time, against 9.7 s for 062e32c's.
    Sampling stage k + 1 while stage k accumulates holds two stages' samples and gives the same
    bits: 18.8 s and 439 MiB.

  - **T11.c follow-up.** The server's pool should schedule that one-stage lookahead. The sim
    spawns no threads, so the schedule belongs to the caller.
  - **The test.** `parallel_build_equals_serial` still pins the halo's serial build to the
    pre-split fingerprint, `0x6ca2_bafb_4c81_c7b4` (fae1c11). Its parallel part now builds the
    bulge and the long bar, which share two of their four stages. It samples every stage ahead of
    its turn, runs each stage's jobs reversed on four threads, and checks the result against
    their serial build.
  - **Guards.** `run_accumulate` panics on a bin's nodes out of order and on another stage's
    samples, and `track_samples` panics on another stage's chunk; each has a `should_panic` test.
- **T5.d's pair-evolved light, as built (2026-10-04/05, `decision-r06-tables.md` A, "T5.d gate
  reading" and "T5.d caps after the pair correction").**
  - **One task a layer (a deviation).** The ruling's one `sky_binary_light` task is three,
    `sky_binary_light_c`, `_d` and `_e` (class Slow, revision 0, since generator version 20),
    each writing `tables::sky_binary_light_<layer>` (193, 185 and 216 kB), as `stellar_fates_{low,
mid,high}` are split: their some 68,000 values do not fit the 500 kB
    `check-added-large-files` limit in one file. One struct (`SkyBinaryLightTask { layer:
FitLayer }`, in `crates/hyperion-fit/src/tasks/sky_binary_light.rs`), three statics in
    `REGISTRY`, three manifests and three `.smoke.toml`s (two systems a cell). The manifests
    repeat the code's constants (the cell layout, the luminosity tables' bin edges and the task's
    chunk of 250 systems), enforced by `expect_params`.
  - **The table.** Per layer: `FIRST_BIN` (the first 1-mag bin held, counted from M_V −12);
    `DELTA: [[[f64; 6]; W]; 130]`, per cell (\[Fe/H\] node, then age bin) and held bin, the mean
    per born system of pair-evolved less single-evolved V light, the four colour sums and the star
    count; and `LIGHT_SE: [f64; 130]`, each cell's standard error of its mean total-light
    difference. Four significant digits. Held: C from M_V −7, D from −8 and E from −12, each to
    +20. The cells' ages are 25 bins of 0.2 dex from 10⁵ to 10¹⁰ years and one to 1.5 × 10¹⁰,
    the same for every layer: "the layer's luminous ages" are taken as every age from 10⁵ years
    (younger stars are Class 0 protostars, dark in V; the protostar phase runs to 5 × 10⁵ years,
    `PROTOSTAR_DURATION`), since a layer-E system's companions shine for gigayears after its
    primary dies. Their modules carry `#[allow(clippy::approx_constant)]` in `tables/mod.rs`,
    since the files are written whole.
  - **The sampling** is the sim's (`sky::binary_light`, new): `fit_galaxy` (the Milky Way-like
    parameters under the fit's own seed, `GALAXY_SEED` 0x5b1a_0005_0000_5eed), `fit_record`,
    `system_difference`, `CellSums` and `cell_sums`, with the cell layout's constants and
    `age_edge`. A cell's n-th system takes the candidate ID cell × 2²⁴ + n along +x from the
    solar circle's cell row in the primary's band layer, `SystemOrigin::Grid` of component 0, the
    point (0, 26,000, 0) ly, the primary from `quantile_in` within the band and the age
    log-uniform in its bin on the R2 low-discrepancy sequence (`sample_point`: Roberts, M. 2018,
    "The Unreasonable Effectiveness of Quasirandom Sequences", Extreme Learning, from ½, in exact
    integer arithmetic at the middles of 2⁻⁵² steps, so no stream and no domain tag), the node's
    composition with ΔY = 0 and `SystemStars::generate_with(.., MultiplicityContext::Free)`.
    Pair-evolved stars are read from `state_at(EPOCH).stars()` and single ones from each
    `stars()[i].state_at(EPOCH)`, binned by the tables' own `Seen::of` (now `pub(super)`). Systems
    a cell: 2 × 10⁴ in D and E and 2 × 10⁵ in C, whose cap rests on rare bright pair phases
    ("T5.d caps"). Pair evolution changed 1.10% (C), 6.98% (D) and 12.78% (E) of them. The fit
    takes `raised_cells = [[cell, systems], …]` for a cell that carries a component bin's error
    past the guard (`Sampling`); none is needed.
  - **Cost, memory and determinism.** Chunks of 250 systems summed in index order on
    `map_reduce_chunks`, so any thread count gives the same bytes; reruns of C and D reproduced
    their bodies byte for byte, and the smoke run is the same on 1 and 3 threads. At version 20,
    under loads of 25–45 (provisional): C 1,166 s on 8 threads (2.6 × 10⁷ systems), D 637 s on 8,
    E 1,797 s on 14. The fit's RSS is about 17 MB; it builds no luminosity tables, so its cost does
    not depend on the build's memory fix. With the correction the fixture's full serial build
    peaks at 216 MiB (getrusage maxrss of the gate test alone, after the memory fix a934837), the
    `sky::` suite on 2 threads at 559 MiB, and T5.c's slow test at 221 MiB.
  - **Fingerprint.** Per layer, two cells where its primaries are giants, at solar and a tenth
    solar \[Fe/H\] (C: age bins 21 and 24, 640 systems each; D: 16 and 17, 96; E: 11 and 12, 48),
    and one young cell where the protostars are, age bin 2 at solar \[Fe/H\], 2,000 systems (the
    caps ruling, so that fit-check sees plan 11's protostar fix): each of a bin's six summed
    differences, the light difference times its bin's index (where the light lands), its
    absolute value and the systems changed. About 1.5 s a layer in fit-check.
  - **The build.** `TablesPlan::correct_for_pairs` is the first part of `assemble`'s finish step
    for each bin, once its every metallicity stage is in: it adds the correction to the bin's
    single-star `Bins` at each snapshot, before the cumulative sums. Each cell's coefficient is
    the bin's born share in the cell's age bin then (the same `born_cdf`) times each
    Gauss–Hermite node's weight, split linearly between the bracketing \[Fe/H\] nodes (held at −2
    and +0.18), so two nodes on one cell count its error once. Light and colour are spread over
    the 20 sub-bins by the single-star light (evenly where there is none); a sub-bin's light is
    held at zero, with its colour sums then zero, and each colour sum is held at zero. Counts
    spread the count difference by the single-star counts; each edge takes the larger of the
    single and corrected cumulative counts, then a running maximum. Only under
    `MassFunctionKind::default()`, which the fit drew from: a Kroupa galaxy takes none (the
    ruling's deviation). Each snapshot keeps the applied light, its standard error and the clamped
    light: `LuminosityFunction::{pair_light, pair_light_sigma}` are new, which T5.c reads and T17
    records the band's correction and 1σ from. `parallel_build_equals_serial`'s pin moved from
    `0x6ca2_bafb_4c81_c7b4` to `0x8c44_443c_67ac_13bf` (the correction, and `bits` hashing the
    snapshots' pair fields).
  - **Gates** (`the_pair_correction_is_known_to_two_percent_and_clamps_under_half_a_percent`).
    Per site and layer ("T5.d gate reading"): the 1σ under 2% and the clamped light under 0.5% of
    each layer's light at T5.c's two sites, summed over the components by their systems there (the
    fit's error is shared, so it adds linearly). Measured (correction, 1σ, clamped): solar circle
    C −2.07%, 0.29%, 0.000%; D −10.16%, 0.72%, 0.019%; E −21.21%, 1.11%, 0.008%; bulge C −3.24%,
    0.29%, 0.000%; D −9.70%, 0.82%, 0.034%; E −13.32%, 0.55%, 0.019%. The component guard, every
    one of the fixture's 53 component bins with its layers weighted by its systems and C, D and E
    folded together: the largest 1σ is 1.17% (the old thin disc at \[Fe/H\] −1) and the largest
    clamp 0.32% (the old thin disc at −0.5; the thick disc 0.29%, the halo at −0.6 0.17%), all
    within 2% and 0.5%. At 2 × 10⁴ systems in C one bin had failed (the old thin disc at −0.5,
    2.38%), 64% of its error from one C cell, which raising C cleared. A failing bin's message
    names the cells that carry its error (`binary_light::cell_errors`, test-only). Read per
    component and layer, old components' D and E reach a 1σ of 3–35% and clamps up to 22%, on a
    few per cent of the sites' D and E light.
  - **T5.c after the correction** (`luminosity_matches_realised_cells` passes, 772 s at load
    25–30). Realised against tabulated light, single-star tables then corrected: solar circle C
    −6.4% → −4.4%, D −28.6% → −20.4%, E +15.5% → +47.7%; bulge C −6.7% → −3.6%, D −9.6% →
    +0.5%, E −17.6% → −4.5%; A, B and the brown dwarfs are not corrected (+2.9%, −2.4%, +5.6%;
    −4.2%, +0.7%, −6.3%). The new gate holds the cells' paired deficit against the tables' within
    Z √(Σ d² + σ²), with d each system's pair-evolved less single-star light and σ the fit's: solar
    circle C 9.4% against 2.1%, D 21.0% against 10.3%, E −1.0% against 21.8%; bulge C 3.8%
    against 3.3%, D 10.5% against 10.1%, E 10.2% against 13.8%, all within. The solar circle's E
    rests on 7,089 systems whose light is a handful of bright stars (6 brighter than −3), and is
    evidence of nothing either way.
  - **Caps after the correction** (cut 7.95; `caps_near_the_sun_and_in_the_nuclear_disc`, whose C
    bracket is now its 1,000 ly floor and its rule bound, as "T5.d caps" rules; D and E keep ×3).
    Near the Sun C 6,764 → 8,193 ly, D 8,193 → 9,925, E 21,369 (unchanged); nuclear disc C 116, D
    204 → 205, E 362. At 2 × 10⁴ systems C's cap had been 13,232 ly on one fit system (a giant
    branch and helium-star core merger whose helium giant reaches M_V −6 to −7 for some 10⁵ yr).
    **For T17:** C's cap is set by rare pair-channel phases whose fitted rate rests on few
    systems: the table's net excess of stars brighter than the cut less the cap's modulus (M_V
    −4.05) is some 3 stars of C's 2.6 × 10⁷ fit systems (D: 31 below −4.47; E: 286 below −6.13),
    and C's cap moved by two radial nodes on them. T17 re-derives open question 19's C figure with
    this; the six points of `caps_converge_in_rays` are recorded in `layer_caps`' doc comment.
  - **Finding: the correction is smaller than the ruling's first estimate near the Sun.** The
    ruling scaled T5.c's one sample, 9% and 21% in the solar circle's C and D; the fit gives 2.1%
    and 10.2%. With the ruling's harness shares (all sky C 0.775, D 0.161, E 0.038) the
    uncorrected band near the Sun is about 4% (0.04 mag) too bright over the whole sky, 5% in the
    plane and 3% at high latitude, and D carries as much of it as C. T17 measures it at its four
    points. These figures measure the generator against itself: the literature brackets them
    (Li and Han 2008, ApJ 685, 225, about 11% less flux in 1–15 Gyr populations of 100% binaries;
    Eldridge and Stanway 2009, MNRAS 400, 1019, fewer red supergiants), without a V deficit per
    mass band to test them against.
  - **Found: protostar mergers (a plan 11 bug, for its 20 → 21 batch).** A pair whose
    pre-main-sequence radii overlap is merged at age 0 and shown, until the product reaches the
    main sequence, as a 0.010 M☉ Substellar object (M_V 17.4) beside a NoRemnant: for example
    the fit's D cell 80, index 144 (ID 0x6214596800000090), whose 0.755 and 0.425 M☉ stars read
    that way at 10⁶ and 3 × 10⁷ yr and as a 1.18 M☉ merger product at 10⁹ yr. It puts differences
    of about 10⁻³ L☉,V and +0.003 to +0.025 stars a system at M_V 17–19 in the young cells:
    negligible light, nothing for the caps. The young fingerprint cell marks the tables stale when
    plan 11 fixes it, and they are refitted then.
  - **No `GENERATOR_VERSION` bump.** Nothing served or golden reads the tables yet (A.5).
- **T5.e's panels and node cuts (decided 2026-10-05, `decision-r06-t5e-gate.md` and `-2.md`).**
  The plan's first gate (every 0.05-mag bin within 1% of its function's light against the full
  build) failed every cut and the full build itself, but it measured the comb of light that
  moving mass nodes shifts between neighbouring bins, which every reader's integral over
  magnitude cancels: no reader takes a bin or one component's layer alone (the band, the limit
  map and `eye_cut` sum density × light fainter than a limit over components and layers; the
  caps sum one layer's counts over components and 768 rays; the census reads the tables only
  through the caps; T5.b and T5.c sum over components; T5.d's guard reads a component bin over
  its layers). The gates are what they integrate, at the eye's cut and at `MAX_CUT_V`. Those
  gates then found a real error: T5's panels were cut at the fates' fitted lifetimes, not where
  the tracks at each \[Fe/H\] node end their phases, so an old, sharp-edged population's giant
  branch fell inside panels. Four nodes a panel put an old halo component's light 6–8% high and
  the halo point's outward band 2.1% off; T5's own 16 were 1% off against 32 and converged
  slowly (3.7, 1.2, 0.8% at 4, 8, 16 nodes). With edges at the tracks' phase ends at every age
  edge, per \[Fe/H\] stage, the full build is held within 0.25% of 32 nodes a panel by a slow
  test, and the cut is measured against it. The change moved T5's bits and an old halo
  component's light by about 1%, the band by under 0.15% on any ray and no cap. 16 parts a
  phase were not kept: equal parts in age undersample a young star's bright early contraction,
  and layer A's cap at cut 11 moved one node in with 1.6–2.5 expected stars beyond it. \[Fe/H\]
  at 0.1 dex was not kept: a bias (two of the seven thin-disc bins move their central node the
  full 0.05 dex in every galaxy) of −0.64% in layer B's band light near the Sun. Both builds
  round \[Fe/H\] nodes to 0.05 dex, which no gate sees: probably ≲ 0.3% of the band near the Sun.
  A single function or bin of the cut tables is not good to 1% (up to about 11% and 6% under
  T5's panels at 4 nodes; the as-built record gives the corrected figures); a new reader of one
  alone re-gates against `BuildOptions::FULL`. Both slow tests re-run whenever where the light
  lives moves: the tracks, the fates, the components' ages, the pair-light fits (P11's refits) or
  the white dwarfs' light (A4).
- **T5.e's panels and node cuts, as built (2026-10-05).** Measured on the Milky Way fixture at
  `REFERENCE_TIME`, every snapshot; the logs are in the tables lane's scratch
  (`.git/rm23-scratch/r06-tables2/t5e/t2-*.log`, `gate-*.log`).
  - **Rung adopted: the first, the scaled order** (`MassNodes::Scaled`, 32 parts, 0.05 dex):
    every gate passed on the corrected panels, so no later rung was needed. The 8-node floor was
    measured as well and passed too, at twice the nodes; it is not kept. `BuildOptions::FULL` is
    `{ MassNodes::Sixteen, SAMPLES_PER_PHASE }` (`FULL_SAMPLES_PER_PHASE` is gone), and the
    test-only `MassNodes::ThirtyTwo` (`GL32_NODES`, `GL32_WEIGHTS`) is the reference check's.
  - **The decided record's two figures, read against the runs.** "No cap" holds for the shipped
    build: every STANDARD cap at the six points and both cuts is T5's. The full build's
    nuclear-disc C cap at cut 7.95 moved one node out (116 → 127 ly), where the count beyond
    116 ly sits at the threshold, 0.999 under T5's FULL and 1.000 under the corrected one, a move
    of 0.1%. "About 1%" in an old halo component is its colour (0.92%, `lux·r`) and one layer's
    function (0.97%); its light moved 0.24–0.63%. The decided text and the Generator version
    sentence are kept as ruled.
  - **Panels and the search.** Each stage's grid is the panels of the global breaks' 125 edges,
    cut further at its phase-end masses: 138 to 188 panels a stage, 3,690 in all, every one
    within a quarter of `MAX_PANEL_LN_MASS`, so 552 to 752 nodes a stage at the scaled order
    (14,760 in all; T5's grid was 140 panels and 560 nodes at 4 a panel, shared by every stage)
    and 2,208 to 3,008 at 16 (59,040). A stage offers 10 to 13 sample jobs (250 in all) at the
    shipped nodes. The search (`phase_end_masses`) reads each end's age on tracks at 122 masses,
    0.1 M☉ and the global panels' ends above it, built to twice the oldest age sampled, and
    bisects every crossing ten times in ln m on tracks built 1% past the edge, then interpolates
    the last bracket linearly in ln age. A stage has 2 to 9 age edges (the components reading
    it, every bin of the full plan) and 16 to 73 solved ends: the end of the pre-main-sequence
    (young edges), the main sequence, the Hertzsprung gap, the giant branch, core helium
    burning, the early and pulsing AGB and the post-AGB crossing, and the death, the last two
    at the same mass. Every solved end lies within 3.9 × 10⁻⁹ of its edge's age (a unit test
    holds 10⁻⁶ at the halo's \[Fe/H\] −1.5). Its cost is 3.15 CPU-s for the 22 stages
    (0.07–0.19 a stage), inside the plan's 4.7 CPU-s with the companions' cells. The search builds
    tracks throughout; the SSE's analytic timescales the ruling allowed were not used. Labels are
    the phase of each segment's midpoint state; the flash bridge reads as core helium burning, so
    its end, 10⁴ years after the giant branch's tip (`FLASH_YEARS`), some 3 × 10⁻⁷ in ln m at the
    halo's turnoff, is not cut separately. The plan is made serially, so a server calls
    `LuminosityTables::plan` off its runtime; a pool could take the search as one job a stage if the
    cold first sky needs it.
  - **The companions' H(c) (a deviation in form).** It is integrated over the panels of the
    global breaks alone, not over a stage's own, and held in one memo for the plan
    (`CompanionsMemo`); for every c beyond a band's top it is read at the top, where H is flat,
    bit for bit (`companions_below_is_flat_beyond_each_bands_top`). H reads only the mass
    function and the companions, whose kinks are the global breaks and the mass ratios' cuts, so
    the phase-end masses are no breaks of it. T5 integrated it over its grid's edges, the fates'
    fitted lifetimes among them; a scratch path rebuilding T5's panels with the top-of-band read
    reproduced T5's bits, so the read moves none.
  - **Reference check** (corrected 32 nodes a panel against corrected FULL, gate 0.25%): G1
    worst 0.0128% (the young thin disc's +0.25 bin, its `lux·r`); G2 worst ray 0.0067% (the
    halo point, cut 7.95, no extinction), all-sky at most 0.0013%, at both cuts and in both
    modes. Seed 1's G1 worst 0.0128%. The slow test passes (503 s at load, 543 MiB peak).
  - **Corrected FULL against T5's FULL** (G1–G3): G1 moves the halo's bins by 0.24–0.63% in
    light and up to 0.92% in colour (Halo −0.60, `lux·r`), every other bin by at most 0.12% in
    light and 0.16% in colour (old thin disc −0.50); G2's worst ray moves 0.125% at the halo
    point, at most 0.016% at the other three, all-sky at most 0.0074%; G3: one cap moves, the
    nuclear disc's C at cut 7.95 (0, 150, 0), 116 → 127 ly (one node out), where the count
    beyond 116 ly is 0.999 under T5's FULL and 1.000 under the corrected one; V 5 and 6.5 move
    −0.004% and −0.005%. Recorded:
    a bin's light up to 0.70% (old thin disc +0.50, B), a function's total 0.97% (Halo −0.60,
    D). FULL's whole-build FNV moved from `e1ef2a3bbc1167ba` to `651d0083b629b70b`; a scratch
    path rebuilding T5's panels reproduced `e1ef2a3bbc1167ba` bit for bit, so the refactor moved
    nothing else.
  - **STANDARD against FULL.** G1 worst 0.073% (old thin disc +0.50, `lux·r`); seed 1's
    0.073% (55 bins, 35 stages). G2, all-sky and worst ray, by point at cut 7.95 then 11, each
    without extinction and through the realised profile: near the Sun 0.0020/0.0024% and
    0.0019/0.0020% all-sky, worst rays 0.0077/0.0108% and 0.0110/0.0112%; the inner disc
    0.0026/0.0030% and 0.0051/0.0019%, worst 0.0079/0.0078% and 0.0174/0.0182%; the bulge point
    0.0043/0.0039% and 0.0097/0.0076%, worst 0.0116/0.0118% and 0.0180/0.0181%; the halo point
    0.0019/0.0027% and 0.0038/0.0032%, worst 0.0286/0.0334% and 0.0171/0.0215%. By layer near the
    Sun, at most +0.011% (E, cut 7.95). Walls near the Sun, the ray nearest its tolerance, cut
    7.95 then 11: 300 ly (N 7.1–8.5, tolerance 11.4–12.5%) 0.014% and 0.014%; 1,000 ly (N 164–359,
    1.8–2.6%) 0.010% and 0.006%; 3,000 ly (N 1,163–14,681, 1%) 0.008% and 0.007%; recorded only,
    30 ly (N under 0.01) 0.030% and 0.020%, 100 ly (N 0.3) 0.027% and 0.009%. G3: every cap at
    the six points and both cuts equals FULL's but the nuclear disc's C at cut 7.95, one node in
    (116 against 127 ly), where FULL counts 1.000 beyond it, the largest count beyond any
    STANDARD cap; V 5 −0.018%, V 6.5 −0.004%. The slow test passes (340 s at load, 275 MiB).
  - **Recorded, not gated** (STANDARD against FULL): the worst bin's light by layer A 3.1%, B
    4.9% (old thin disc +0.50, M_V 7.65), C 2.2%, D 1.2%, E 1.1%; each edge's count A 0.81%,
    B 3.2%, C 1.2%, D 0.25%, E 0.14%; the light fainter than an edge A 2.6%, B 2.9%, C 1.1%,
    D 0.68%, E 0.64%; a function's total at most 0.20% (old thin disc +0.50, E), against 10.81%
    (Halo −1.50, D) under T5's panels at 4 nodes, and the light fainter than an edge at most
    2.9%, against 10.95%.
  - **Timed serial builds** (quiet machine, load 0.6–1.8, under the heavy-test lock, release test
    binary, 2026-10-05): STANDARD 31.3 and 31.7 CPU-s (plan 4.7 with the search's 3.15,
    sampling 17.6–17.8, accumulation 8.9–9.0), wall 32.0–32.4 s, peak RSS 155 MiB, largest stage
    set 57 MiB; FULL 116.3 CPU-s (9.5, 70.7, 35.8), wall 117.3 s, peak 335 MiB, stage set
    228 MiB; T5's FULL earlier the same day, outside the lock, 79.9–80.7 CPU-s (at load 1.4 and
    10.5), peak 255–274 MiB. STANDARD
    is over the 30 CPU-s target by 1.3–1.7 CPU-s: recorded, not gated, and T17 proposes the disk
    cache. The plan's search runs serially when the plan is made; a pool could take it as one
    job a stage if the cold first sky needs it.
  - **The other gates on STANDARD.** The release `sky::` lib suite passes (101 passed, 5
    ignored, 135 s on two threads at load, 522 MiB peak). T5.d's site gates read as at ae1a494 to the
    digit printed (the solar circle's C −2.07% ± 0.29%, D −10.16% ± 0.72%, E −21.21% ± 1.11%;
    the bulge's −3.24% ± 0.29%, −9.70% ± 0.82%, −13.32% ± 0.55%; clamps at most 0.034%), and its
    guard's worst 1σ is 1.168% (old thin disc −1.00) and worst clamp 0.31% (old thin disc −0.50),
    against 1.17% and 0.32%. T5.c (`luminosity_matches_realised_cells`, 586 s) reads every layer's
    residual as at ae1a494 to the 0.1% printed but the bulge's E against the corrected tables,
    −4.4% against −4.5%. `caps_converge_in_rays` passes with every cap of `layer_caps`' doc table
    unchanged; `a_solar_circle_build_is_the_full_builds_bin_bit_for_bit` passes;
    `cargo bench -p hyperion-sim --no-run` builds; `just fit-check` is 16 fresh, 0 failures.
  - **The doubling test** keeps its per-bin form on FULL: its worst bin moves 0.51% of its
    function's light and its worst edge 0.08% of its stars from 32 to 64 parts.
  - **Pins.** `SERIAL_FINGERPRINT` (STANDARD's halo at 2 parts) is `0xe56b_4376_5fe8_1768`, and
    `FULL_SERIAL_FINGERPRINT` (FULL's) `0xa022_ae9e_49f0_c996`, both asserted in
    `parallel_build_equals_serial`; T5's build gave `0x8c44_443c_67ac_13bf`. A third pin beyond
    the ruling's two, `PAIR_SERIAL_FINGERPRINT` (`0x9bc9_1279_f62c_9962`), is the bulge's and
    the long bar's coarse build under STANDARD, asserted beside them: their stages read the thin
    discs' young edges, so it pins the search's pre-main-sequence ends and the deaths of
    intermediate-mass stars, which the halo's old edges never reach. `parallel_build_equals_serial`
    and the subset test also pass on `wasm32-wasip1` under wasmtime 49.0.1.
    STANDARD's whole-build FNV is `4a25321f0d9ff62d`, FULL's `651d0083b629b70b`.
  - **Tests added.** `a_stages_grid_is_the_same_whatever_components_are_built` (the bulge alone
    and with the long bar, bit for bit, and \[Fe/H\] 0's ages including the thin discs'; the
    full plan is `a_solar_circle_build_is_the_full_builds_bin_bit_for_bit`'s),
    `a_stages_panels_end_where_its_tracks_end_their_phases_at_the_age_edges` (every solved mass
    of the halo's \[Fe/H\] −1.5 a panel edge of its grid, its end at the edge to 10⁻⁶, and the
    main sequence's and the giant branch's ends where the track's own states change phase),
    `companions_below_is_flat_beyond_each_bands_top`, and the two slow lib tests.
  - **No `GENERATOR_VERSION` bump**, and `galaxy::fates` and `mean_present_mass` untouched: no
    golden or served output reads the tables.
- **The A3 interim's Class I sources (T5).** `is_dark_in_v` treats every Class I protostar as dark;
  a few per cent of them, seen pole-on down an outflow cavity (A<sub>V</sub> about 1.5; Whitney et
  al. 2003a, ApJ 591, 1049, §2 and Fig. 3), would show in V. Plan 06's A3 decides.
- **Deviations in T6.b, as built.** `sky::envelope::{BrightnessEnvelope, max_star_mass}` with
  `brightest(layer, component, mass_at_most, ages)` and `mass_floor(layer, component, faintest,
ages)` as sketched; the envelope is one for every component (layer and component are read for the
  caller's question only). It is built from 192 mass nodes even in ln m over 0.0124–150 M☉ (with the
  band edges), at 12 \[Fe/H\] nodes (−2.5, which the tracks read as their clamp, to +0.18 every
  quarter dex) and at the Reimers η draws z = −0.5 ÷ 0.07 (η = 0, no Reimers wind, the physical
  extreme, where a giant's tip is brightest), −3.5, 0, +3.5 and +7 (`ETA_DRAWS`), in 192 log-age
  bins with one 0–10⁴ yr bin; each part of each phase enters its brightest of five points in every
  bin it overlaps; each node's brightest is spread over ages within a factor of 1.6
  (`AGE_SPREAD_FACTOR`) and then made a running minimum over mass, and brightened by the 0.3 mag
  margin. **The bound is empirical, not proved:** the dense slow test (6 × 10⁴ masses over every
  layer, \[Fe/H\] −2.3 to +0.4, η from 0 to +7σ, 24 ages each) finds no star brighter than the
  envelope and none of the 0.3 mag margin used; before the age spread and the finer \[Fe/H\]
  nodes, single nodes missed metal-poor giants by up to 5.6 mag, their bright phases falling
  between nodes in age. A star of z above +7 (one in 10¹²) lies outside what it checks. The test
  `early_v_rises_with_mass` was dropped (decided 2026-10-03; Design note 10's correction).
  `mass_floor` bisects on the primary's mass, monotone because the envelope is a running minimum
  and `max_star_mass` rises. Build: 11–22 s under load (provisional).
- **T6.b's fitted envelope, as built (2026-10-04, `decision-r06-tables.md` B.1).** The
  `hyperion-fit` task `sky_envelope` (class Fast, revision 0, since generator version 19) writes
  `tables::sky_envelope`, 266 kB: `DARK` (`i32::MAX`, for +∞), `MASSES` (the 198 nodes of
  `mass_nodes()`: 192 even in ln m plus the band edges and 0.1 M☉, after duplicates) and
  `BRIGHTEST_MMAG: [[i32; 193]; 198]`, one node a line.
  - **The sim side.** `BrightnessEnvelope::build(_galaxy)` keeps its signature and reads the
    table. `build_with(fe_h, samples)` stays the generator, now `pub` and `#[doc(hidden)]`, as the
    pieces the fit uses: `raw_node(m, fe_h, samples) -> [f64; 193]` (one node's own brightest per
    bin), `BrightnessEnvelope::assemble(masses, raw)` (the spread, the running minimum and the
    margin, in mass order), `mass_nodes()`, `rows()`, and `to_millimag` and `from_millimag`. The
    fit builds the nodes on its pool, one a chunk, and assembles them in mass order. The output
    is byte-identical on 1 and 16 threads: 13 s on one thread, about 1 s on 16.
  - **Rounding.** `to_millimag` floors v × 1000, then lowers k while k ÷ 1000 > v, so every
    stored value is 0–1 mmag brighter than the build and the table stays a bound.
  - **Acceptance.** 9,240 tracks; 1,856 dark bins; brightest −12.850 mag.
  - **Science check (2026-10-04).** The extremes bound this generator, not nature. The −12.85 at
    150 M☉ and about −5.7 for old stars of at most 1 M☉ likely come from the tracks'
    super-Eddington excursions after the main sequence and the η = 0 late giants. Both are 2–3 mag
    brighter than observed steady stars. A refit is due when P06.T39's wind lands, and when
    P15.T7 fits `tables::helium` (the envelope is built at ΔY = 0).
  - **Loose but still a bound.** The envelope is about 2 mag loose for 0.1–0.19 M☉ at
    3.5–28 Myr. Each long phase is cut into parts equal in linear age, so its first part carries
    its young end's magnitude for some 15 Myr. That costs only skip efficiency. Cutting in log age
    would tighten it.
  - **Fingerprint.** The node count and ends, plus `raw_node` at 0.05, 0.1, 0.4, 1, 3, 25 and
    100 M☉ at \[Fe/H\] −2.5 and 0 with 4 parts a phase: each node's brightest bin, its sum of
    shining bins and their count. That is 60 tracks, which fit-check computes in every `just ci`.
  - **Tests.** Lib:
    - `the_fitted_table_is_on_the_mass_nodes_and_falls_with_mass`;
    - `millimagnitudes_round_brighter`;
    - `raw_nodes_are_order_independent` (`assert_order_independent`).

    `sky::testing::milky_way_envelope` now copies the table and builds no galaxy. The slow
    `envelope_bounds_dense_tracks` reads the table through `build`, and the new slow
    `the_fitted_envelope_is_the_build_rounded_brighter` checks every bin against
    `to_millimag(build_with(..))`. T16.b's `envelope_bounds_pair_states` is not built yet. It
    will read the table through `build` too.

  - **No `GENERATOR_VERSION` bump.** The envelope moves brighter by at most 1 mmag. No golden
    and no served output reads it yet.
- **The caps' extinction (decided 2026-10-03, `decision-r06-t7-caps.md`).** Design note 9 first
  dimmed every direction by the least extinction of 48 rays. Near the Sun that is the polar rays'
  extinction, and it gave C/D/E caps of 9,018 / 16,029 / 61,341 ly. Per-ray extinction in the mean
  field fixed the angular error, but through the realised field its caps missed 2–5 expected
  naked-eye stars near the Sun and up to 110 of layer D's at inner-disc points (clear windows
  6–10 kly out). In the nuclear disc it overstated the caps tenfold (E 5,584 ly against the
  realised 534). The caps now count through the realised field on 768 rays, one `profile` march
  each. They remain an approximation: a window narrower than the rays' spacing, about 7°, can still
  hide stars beyond the rule's bound. Each response states its expected count beyond, and
  `caps_converge_in_rays` bounds it at 1.5 against 3,072 rays. The near-Sun caps (C 6,684, D 8,301,
  E 19,740 ly at the harness's reading) are 1.9–2.2 times the brainstorm's version-14 estimates.
  The tests do not gate on those estimates; T17 re-derives them for open question 19.
- **Deviations in T7, as built.** `sky::caps::{layer_caps, LayerCap, CAP_RAYS (768),
CAPPED_LAYERS}` as decided, with `RayExtinctions` (each ray's realised `Full` profile at 24 nodes
  a decade from 1 ly to 120,000 ly, interpolated linearly), `CapResolution`, `layer_caps_at` and
  `expected_beyond_caps` (the convergence test's recount), and `LayerCap::forced` (T8.a's forced
  caps). Each star's density table is read where it lies (`LuminosityTables::get_at`,
  T5's metallicity bins). Measured (version 19, cut 7.95; `caps_converge_in_rays` passes, every
  layer at every point under 1.5 at 3,072 rays, the largest 1.03 for B in the nuclear disc and
  1.02 for D at (−18,385, −18,385, 68)): near the Sun A 11, B 68, C 6,764, D 8,193, E 21,369 ly
  (2.3, 1.9 and 2.1 times the brainstorm); nuclear disc C 116, D 204, E 362 ly; the full table at
  six points is in `layer_caps`'s doc comment. `layer_caps` took about 2.5 s a call on one thread
  under load (provisional; 768 realised profiles to 120,000 ly, then the count), for T17. T9.a's
  `galaxy::gas::extinction::profile` was built here (its test, the last node equal to `sightline`
  to 10⁻¹², monotone, no cache read at `Mean`, is in `gas::extinction`); it takes the nodes as
  `&[LightYears]` and fills `&mut Vec<Magnitudes>`, as sketched.
- **Deviations in T8.a, as built.** `sky::census::{SkyQuery, SkyQueryBuilder, Cone,
BuildSkyQueryError, SkyContext, CensusPlan, census_plan}` as sketched, with `MAX_N_MAX` (3 ×
  10⁵) and a public `plan_cells(query, caps)`, the cells a census opens for given caps, which the
  brute force shares so that both open the same cells. `with_caps_forced(radius)` is public, not
  test-only (integration tests do not see `cfg(test)`), and returns `Result<_, BuildSkyQueryError>`
  (`ForcedCap` unless the radius is finite, non-negative and within the root cube's diagonal,
  `MAX_FORCED_CAP_LY`); `with_caps_forced_per_layer(&[(Layer, LightYears)])` forces each listed
  layer and opens no cell for the rest (accepted 2026-10-03: the identity tests at the plan's
  1,000 ly and 200 ly radii, about 10⁷ systems generated whole over every layer, are infeasible;
  T8.e compares every layer at 150 ly near the Sun and 10 ly in the nuclear disc, and slowly the
  plan's radii for C–E near the Sun and D–E in the nuclear disc, with a slow A/B check at a radius
  a slow test can afford, since identity there tests that the envelope bounds every M dwarf;
  narrowed in the nuclear disc by T8.e's record). A cell is kept for a cone when its bounding
  ball, padded, meets the cone. `SkyContext` is a plain bundle of borrows with public fields, the
  census's one accessor to the tables (so the tables lane's change of their source touches nothing
  here). The cell cache's trait, `NoSkyCellCache` and
  the monotone rule, `serve_from_entry` returning `Served::{Served, Rebuild}`, are built here
  because `SkyContext` holds a cache (T8.d adds the rest). Planned for T8.b, accepted 2026-10-03:
  the per-record flux bound is n × the envelope's flux at `max_star_mass(m₁)` and the record's age
  at the emitted time, not the primary's brief (ask A1's interim would cost a full generation
  outside ±H), with n the generator's `MAX_COMPANIONS + 1` for a grid system (carve redraws can
  change its count; a test holds no generated grid system above it) and 1 for a forced single.
- **Deviations in T8.b, as built.** `sky::census::cell` holds `SkyStar`, `LayerTally`,
  `CensusTallies`, `Bound`, `flux_bound`, `cell_floor`, `census_record` and `census_cell`, with
  `EYE_OFFSET_BOUND_MAG`, `GRID_STAR_BOUND`, `star_offset_bound` and `cell_offset_bound`.
  - `EYE_OFFSET_BOUND_MAG` is 0.6 mag, above every row of the colour table, whose hottest give about
    0.43; the table is bilinear in ρ, so no colour exceeds its rows.
  - `census_cell` returns its `CensusTallies` rather than only filling `out`. `census_record`, with
    its `Bound`, is public so that the oracle measures each record as the census does, with
    `Bound::Ignored`.
  - The flux bound is n × the envelope's flux at `max_star_mass(m₁)` and the record's age at the
    emitted time, as accepted for T8.a, but **n is `GRID_STAR_BOUND` = `MAX_COMPANIONS` + 2 = 5 for
    a grid system, not `MAX_COMPANIONS` + 1** (accepted 2026-10-04). Plan 11's draw caps a
    hierarchy at four stars, but P11.T2.d's brown-dwarf companion comes on top as a fifth body,
    which `state_at(t).stars()` lists. A test holds every generated system to five bodies and to
    four stars of 0.08 M☉ or more. A forced single takes n = 1. Since the census lists stars, not
    systems, n = 1 would already bound each star, and n only adds margin.
  - A record with no density component (a feature member's, from T16.a) takes no bound and is
    always generated.
  - **Added: the stars' offsets from their barycentre.** Plan 11 keeps every apocentre inside half
    the system's tidal radius, which is some light-years near the Sun, so a companion can be nearer
    the observer than its system. The flux bound takes the distance to the system's apparent
    position, from which each star's is measured, less `star_offset_bound`. That is
    (`GRID_STAR_BOUND` − 1) orbits × `TIDAL_CUT_SHARE` × the tidal radius at 5 m₁ at the epoch
    position, and 0 for a forced single.
  - The cell floor takes the cell's least distance less the motion pad and `cell_offset_bound`. The
    pad is solved with the light's age, pad = β (|t − epoch| + far + offset) ÷ (1 − β), since a
    record can lie a pad outside its box. `cell_offset_bound` is the same bound at the band's top
    mass. It reads the new `PotentialTables::tidal_radius_bound_within`: the floored tidal radius at
    the least Ω² at the cell's farthest radius from the centre and at every grid point inside it,
    less 1% for the interpolation. A test checks it against the tidal radius at dense points over
    the fixture and 24 drawn galaxies.
  - The eye's colour offset is taken at full scotopic adaptation (μ 30), as the eye's cut is at the
    darkest texel.
  - The census reads no luminosity table, so it has no call to `age_for`. The tests' tables are
    built for no component at `REFERENCE_TIME`.
  - Each star's state is indexed by its body index from `star_positions_at`'s rows.
  - The envelope still takes the age at the emitted time and `max_star_mass(m₁) = m₁`. Pair-evolved
    stragglers are T16.b's; none broke the bound in the sample.
  - Tests (`cargo test -p hyperion-sim sky::census::cell`):
    - a white-dwarf primary lists its bright companion and tallies the dwarf;
    - a post-AGB primary of 2 M☉, at the end of its crossing beside a near-twin giant that outshines
      it, takes the envelope bound, and at a cut between the two only the giant is listed;
    - a centre member is tallied, not listed;
    - the observer's own system is absent;
    - a forced single takes the envelope's own bound, and a grid system one 2.5 log₁₀ 5 brighter;
    - every star's light lies under its flux bound, and every star lies within its offset bound,
      over 400 records at each of seven to nine places and layers (dense slow variants at
      3,000–4,000);
    - the census of 15 cells near the Sun (A–E, at 0, 1 and 3 cells out, eye asked, cut 7) equals,
      bit for bit, every record of those cells measured with no skip;
    - cells censused in reverse order with a one-entry noise cache equal a warm forward run;
    - the tallies add.
- **Deviations in T8.c, as built.** `sky::census::merge` holds `SkyCensus` (`empty`, which is its
  `Default`, and the getters `listed`, `overflow` and `tallies`), `merge_census` and `sky_order`.
  - `merge_census(parts, n_max)` takes each part as one job's `(Vec<SkyStar>, CensusTallies)`,
    from any `IntoIterator`, not the sketch's `Vec<Vec<SkyStar>>`. `census_cell` returns its
    tallies (T8.b), so the merge adds them, and the split and order tests cover the tallies too.
    The sum starts from the first part's tallies, so `feature_members_absent` is true if any part
    lacks the members (the default's true, with no part); T16.a's parts can clear it.
  - `sky_order` is public, for the band (T9) and the encoder (T11) to share. It orders by V
    through `total_cmp`, brightest first, then by `SystemId`, then by `StarIndex`. Ordering by V
    instead of by a computed flux keeps the order exact. No two stars share a system and an index,
    so the order is strict and the sort is unstable, in place. The merge asserts, in release
    builds too, that every V is finite (a NaN's sign, which places it in the order, differs
    between targets) and that no star is in two parts at the same V. The listed are shrunk to fit
    after the cut.
  - **`CensusTallies` now carries `accepted` and `listed`.** T8.b's `LayerTally::listed`, the stars
    kept, is renamed `accepted`. The new `listed` counts, per layer, the stars `merge_census` lists
    within `n_max`; it is zero in a cell's or a job's tallies, and a merge recounts it.
    `feature_members_absent` is one flag for the census, not one per layer. `candidates` (records
    past the mass skip) and `centre_members` are kept beyond the sketch. For T11's DTO: `cells` →
    `cells`, `generated` → `candidates_opened`, `accepted` → `accepted`, `listed` → `listed`,
    `without_photometry` → `without_photometry` (each `u32` count narrowed); the census's flag goes
    into every layer's `feature_members_absent`; `centre_members` > 0 → `SkyGapDto::CentreMembers`;
    `without_photometry` > 0 → `SkyGapDto::WhiteDwarfs`.
  - **`SkyStar` gains `layer()`**, its record's layer, so that the per-layer listed count also
    holds for T16.a's feature members, whose IDs name no layer.
  - Tests (`cargo test -p hyperion-sim sky::census::merge`):
    - ten cells at the Sun (A–E, the Sun's own cell and the next along x, cut 9): each cell's part,
      censused through a shared 64-entry noise cache, is order independent
      (`assert_order_independent`) and equals the part a job's own cache gives;
    - five splits into jobs (whole, one cell each forwards and backwards, interleaved, and uneven
      parts reversed with an empty one) give the same census at `n_max` 1, a third of the stars and
      unbounded, and at each of those cuts the census lists the brightest and counts the listed per
      layer;
    - 45 synthetic stars with 39 ties in V: six round-robin deals (1, 2, 3, 7, 45 and 60 jobs, each
      part reversed) and the reversed list give the same bits; ties go by system, then star; a cut
      inside a tie keeps the lower;
    - `n_max` keeps the brightest and counts the listed per layer, at nine cuts;
    - listed plus overflow is the unbounded census at every `n_max` from 1 to 47;
    - a census merged again recounts its listed; the feature-member flag; a star in two parts and
      a NaN V are refused; the empty census.
- **Deviations in T8.d, as built.** T8.a built `SkyCellCache`, `NoSkyCellCache`, `Served` and
  `serve_from_entry`. T8.d adds no public item.
  - `serve_from_entry` serves only when `floor >= held`, so a NaN on either side rebuilds. T8.a's
    `floor < held` refusal let a NaN through, and an entry built at a NaN floor holds no record.
  - The trait's doc lists what an implementation that keeps entries owes, with an example (a
    one-entry cache keyed by seed and cell):
    - it serves only through `serve_from_entry`, and keeps a rebuilt cell at the floor it was
      rebuilt for, never at a NaN floor;
    - it keys entries by galaxy as well as by cell, as `CellCache` does;
    - it is bounded in bytes, each entry weighing at least `cell_heap_bytes` of its records
      (T11.b's `ByteLru` under `HYPERION_SKY_CACHE_MB`). Eviction is always safe.
  - The sim holds no keeping cache. The tests' `KeepBright` (test builds only) is built on the rule,
    as the server's will be: one galaxy's entries in a `BTreeMap` behind a `Mutex`, shared through
    `&self`, least recently used first out. It builds a missing cell outside the lock and replaces
    an entry only with one of a lower floor.
    - **Its memory bound:** at most `max_entries` entries, together weighing at most `max_bytes`.
      An entry weighs its records' capacity × `size_of::<SystemRecord>()`, plus its key and its
      own struct; the map's nodes are not counted. An entry heavier than the whole bound is served
      and not kept. The orchestrator required a stated, tested bound of any cache.
    - T11.b's server cache should pass the same scenarios: looser then tighter, a planted entry,
      and `assert_order_independent` through a shared cache that evicts.
  - **Measured on the fixture (2026-10-04),** along x through the Sun at cuts 6 to 9 with the
    eye: every D and E cell's floor is its band's lower edge out to 800 ly (the farthest probed),
    and every C cell's out to 200 ly. `cell_floor` takes the envelope over ages 0 to
    `MAX_AGE_YEARS`, which allows a giant of the band's least mass. So two queries' floors part
    only in A from about 100 ly, in B from about 200 ly and in C from about 400 ly (by 3% of the
    band's log width at cut 6), and there a C–E cell's bright subset is the whole cell. T11.b's
    default of 64 MB and the warm-cache bench should be read with that in mind. The brainstorm's
    skip by age, under every component the cell can hold, is not in the plan.
  - Tests (`cargo test -p hyperion-sim sky::census::cache`; 40 s on 2 threads under load) run over
    25 cells along x through the Sun (A–C at 0, ±100, ±200 and ±800 ly; D and E at 0 and 200 ly).
    They use three queries: cut 9 with the eye, cut 6, and cut 6 from 200 ly along x. Each census
    is compared with the `NoSkyCellCache` census, its stars by `PartialEq` and every float's bits:
    - the rule serves at or above the floor, at a record's own mass too, and refuses below it and
      on a NaN, leaving `out` untouched;
    - a tighter query after a looser one is served from every entry and filters some; a looser
      one after a tighter one rebuilds some, which then serve the tighter again; the move after
      the tighter both rebuilds and filters; a query 900 years before the epoch reads the entries;
    - a planted entry of another cell's records is never read below its floor (the cell is rebuilt
      and replaces it), and is read at or above it; a NaN on either side rebuilds;
    - the bound holds after every lookup: least recently used first out; five entries alone and a
      third of the bytes alone each evict without changing a reply; an entry heavier than the
      bound is not kept;
    - each (query, cell) part is order independent (`assert_order_independent`) through one
      shared 8-entry cache that evicts, rebuilds and filters;
    - two threads censusing different queries through one cache get the uncached bits. This test
      is left out on wasm32-wasip1, which has no threads.
  - A by-hand check, not committed: with the rule made to serve every floor, four of the then
    five tests failed.
  - Two of T8.b's doc links in `cell.rs` are mended: the module doc's link to
    `SkyCellCache::bright_subset`, and `star_offset_bound`'s link to the private
    `offset_bound_at`.
  - Not built: the benches `sky/census_near_sun` (cold and warm cache) and
    `sky/census_nuclear_disc` of T8's shared paragraph, which no subtask names. Lean: T8.e builds
    them, after its observers, with a bench-local cache on `serve_from_entry`, since `KeepBright`
    is test-only.
- **Deviations in T8.e, as built.** `crates/hyperion-sim/tests/common/sky.rs` and
  `tests/sky_census.rs`, with one public addition to the sim and the benches T8's shared paragraph
  names.
  - **Added: `LuminosityTables::dark(galaxy)`** (orchestrator, 2026-10-04). The integration tests
    cannot call the `pub(crate)` `build_with`, and a census with forced caps reads no table. It
    returns tables for no component, built at `REFERENCE_TIME`, every function zero. It has a doc
    example, and `the_dark_tables_hold_no_star_at_the_reference_time` checks it: every component
    and layer, read by `get` and by `get_at` at both test places, at light ages 0 to 400,000 years
    through `age_for`, holds no light, count, star, dark star, remnant or colour.
    `sky/testing.rs`'s `milky_way_dark_tables()` now calls it.
  - **The helpers:**
    - `brute_force_sky(galaxy, query, radius)`, as the Test helpers name it. Beside it,
      `brute_force_parts` and `census_parts` take `(galaxy, query, radii)` and return one
      `(stars, tallies)` part per cell, so that a test merges them at two `n_max`.
    - `observer_near_sun(galaxy)` stands at (0, 26,000, 68) ly, and
      `observer_in_nuclear_disc(galaxy)` at (0, 150, 0) ly, both at the epoch. Each asserts that
      its place fits the galaxy: the bar ends short of 26,000 ly, and the nuclear disc's scale
      length is beyond 150 ly.
    - Both sides open the cells of the census's own plan for the forced caps (`census_plan` after
      `with_caps_forced_per_layer`, whose cells are `plan_cells`', which T8.a made public for the
      brute force to share), so they differ only in the skips. The oracle measures every record of
      `generate_cell` with `census_record(…, Bound::Ignored, …)`.
    - Both read `LuminosityTables::dark` and the fitted envelope. They run the cells on up to
      eight threads, or one on wasm32, and put the parts back in the plan's order.
    - The queries are taken by value.
  - **The identity tests' scope** (amending the accepted deviation of 2026-10-03). The systems in
    the opened cells were counted on the fixture first:

    | Test | Place        | Layers within                          | Cut (eye) | Systems          |
    | ---- | ------------ | -------------------------------------- | --------- | ---------------- |
    | fast | Sun          | every layer, 150 ly                    | 11        | 39,600           |
    | fast | Sun          | every layer, 40 ly (`brute_force_sky`) | 11        | within the above |
    | fast | nuclear disc | A and B, 1.5 ly                        | 11        | 49,534           |
    | slow | Sun          | A 300 ly, B 500 ly                     | 7.95      | 246,600          |
    | slow | Sun          | C, D and E, 1,000 ly                   | 7.95      | 1.40 M           |
    | slow | nuclear disc | D and E, 20 ly                         | 7.95      | 0.88 M           |
    - **The nuclear disc is narrower than accepted.** Every layer within 10 ly opens 1.9 M systems
      there, since a D cell holds about 96,000 and an E cell about 124,000. D and E within the
      plan's 200 ly would open 17.3 M, about 15,000 CPU-seconds for the oracle alone at the 0.9 ms
      a system measured under load. So the fast test takes A and B within 1.5 ly, and the slow one
      D and E within 20 ly. Each is the four cells of its layer that meet at the place, which lies
      on the cell boundaries x = 0 and z = 0. Reported to the orchestrator with this record.
    - Each test compares the listed and the overflow star for star, by `PartialEq` and by every
      float's bits (the apparent position's offset in metres as held). It compares them at the
      query's `n_max` and at a third of the listed, where the overflow is not empty. It compares
      each layer's accepted and listed, and asserts that the census lists stars and skips systems.
      The A/B check asserts both A and B skip some.
    - The nuclear disc's slow test passes (2026-10-05, under load average 15–21): D generated
      383,434 of 383,439 systems and E 494,726 of 494,729, listing 5,323 and 1,010 stars. It took
      53 minutes on eight threads, about 14 ms a system per thread for each side. Its eight cells
      are the unit of parallel work, so no more threads would shorten it. That is heavy for the
      slow suite, and T17 should weigh it.
    - Measured in the fast tests (2026-10-04, under load). Within 150 ly of the Sun, B to E
      generated every system (T8.d's finding on the floors). A generated 10,767 of 18,469, and the
      brown dwarfs none of 8,268 (1,073 candidates). In the nuclear disc, A generated 20,328 of
      21,450 and B all 28,084. The three fast identity tests took 10 s on two test threads,
      eight workers each.
    - **Not compared in the nuclear disc: layers C and the brown dwarfs.** Near the Sun both are
      compared within 150 ly. At the nuclear-disc place, their four cells alone hold some 280,000 C
      systems (C under 8 ly) and 63,000 brown dwarfs (under 6 ly). That is left for a later rerun
      of the oracle (T16.b or T17) rather than added to a lock-bound slow suite now.
    - **The slow identity tests run by name.** `cargo test -p hyperion-sim --test sky_census` runs
      the four fast tests. `just test-slow the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc`
      runs the nuclear disc's slow test. The two near the Sun fail until T16.b (Risks, "Merger
      products outshine the flux bound") and are excluded from the slow profile by name until then.
      T17's list names all three. _T16.b removed the exclusion; all three run by name._
    - `tests/common/mod.rs` declares `pub mod sky`, so every integration binary that includes
      `common` compiles the oracle. Each already carries `#[expect(dead_code)]`. Beside the named
      helpers, `sky.rs` exports `every_layer`, `float_bits`, `assert_same_stars`, `Part`, `THREADS`,
      `SUN_LY` and `NUCLEAR_DISC_LY`.
    - nextest reserves eight slots for each `sky_census` test (`threads-required`), since each runs
      eight threads of its own.

  - **On wasm32-wasip1,** which has no threads, the oracle runs on one thread. There the fast
    identity tests at 150 ly and in the nuclear disc are ignored as slow, so `just test-wasm-slow`
    runs them as a 32-bit check. The 40 ly test of `brute_force_sky` stays in the fast suite, and
    the slow ones are compiled out, since they would take hours.
  - **Built here: the benches** `sky/census_near_sun/cold`, `sky/census_near_sun/warm` and
    `sky/census_nuclear_disc` (`benches/sky.rs`), which no subtask named (orchestrator, 2026-10-04).
    - Each census runs much as the server will run it. The plan, with its caps, runs on one thread.
      The server will split the caps' rays into pool jobs (T7's decision), so the printed wall time
      overstates its own. Then chunks of 256 cells run on the machine's threads less one, each job
      with its own noise cache, and `merge_census` joins them.
    - The figure is CPU time: the plan's plus every job's, summed over the threads. The cut is
      V 7.95 with the eye, the eye's cut near the Sun (7.4 + 0.45 + 0.1), until T9's `eye_cut`. The
      nuclear disc takes it too. Its own eye cut will be shallower, so its figure is an upper bound.
      The brainstorm's 400–800 CPU-s are the inner bulge's under the near-Sun caps held fixed, so
      they are not a target like for like.
    - The bench-local cell cache is built on `serve_from_entry`. It evicts the least recently used
      first, and is bounded by `HYPERION_SKY_CACHE_MB` (default 64 MiB; a malformed value panics).
      Its entries share their records, so a lookup filters them outside the lock.
    - The cold census starts each iteration with an empty cache. The warm one reads the cache that
      one census of the same query left.
    - Each prints its first census's caps, cells, candidates, generated, accepted, listed and the
      cache's counts.
    - **Run once, provisionally (2026-10-05):** `sky/census_near_sun/cold`, one census in
      criterion's `--test` mode under the heavy-test lock, was stopped unfinished after 20 minutes
      on the orchestrator's word. By then it had taken **≥ 14,000 CPU-s** (14,750 CPU-s of the
      process, of which the tables' build is some 300): 15 workers, cut 7.95 with the eye, RSS
      1.6 GB, on a machine shared with other lanes. The brainstorm's target is 5–10 CPU-s and
      some 6 × 10⁷ candidates on first arrival (open question 13), so it misses by over a thousand
      times. No per-stage split was printed, since the run did not finish. The likely causes:
      - the caps are about twice the brainstorm's (T7's record: C 6,764, D 8,193, E 21,369 ly
        against 3,000, 4,300 and 10,000), about ten times the volume and the candidates;
      - T8.d's floor finding: C–E floors are their bands' lower edges out to hundreds of
        light-years, so every C–E record there is drifted and retarded before its flux bound
        skips it;
      - each generated system costs about 1 ms near the Sun, and up to 14 ms in the nuclear disc
        (the identity tests' timings).

      The warm and nuclear-disc benches are not run, and are left for T17 or the owner on a quiet
      machine. The orchestrator puts the census's cost to its own investigation (see "The
      census's cost" and "The mass skip barely bites near the Sun").
- **Deviations in T16.b, as built (2026-10-05).** The bound for binaries, as Design notes 8 and
  10 and decision item 2 set it, with one choice of the lane's and three test checks changed by
  ruling.
  - **The rule.** `max_star_mass(m₁)` is min(2 m₁, 150 M☉), the cap being `sse::MAX_INITIAL_MASS`.
    Plan 11's engine runs only pairs of two stars (`run_pairs`), and no companion outweighs its
    primary, so m₁ + m₂ ≤ 2 m₁. The engine builds no track above 150 M☉ (`track_mass`).
    `flux_bound` reads the envelope at `max_star_mass(m₁)` over ages from zero to the system's
    for a grid system (n = `GRID_STAR_BOUND`). It reads the primary's own mass and age for a
    forced single (n = 1), which is never in a pair. `cell_floor`'s code is unchanged: it reads
    the widened mass through `BrightnessEnvelope::mass_floor`, which already took every age.
  - **The lane's choice: the floor takes 2 m₁ in every layer.** That includes the brown dwarfs,
    whose systems are forced singles and need only m₁. Their floor is lower than needed: within
    150 ly at cut 11, 5,102 candidates against 1,073 before. None is generated either way, since
    their flux bound keeps m₁. A layer-aware mass in `mass_floor` would win that back, and is
    left to the cost ruling.
  - **The caps.** The rule bound reads `max_star_mass` of the band's top, so it moves, but no cap
    moves at the fast test's two points: caps are count-limited.

    | Layer | Rule bound near the Sun, before → after (ly) | Nuclear disc (ly) |
    | ----- | -------------------------------------------- | ----------------- |
    | A     | 105 → 17,595                                 | 20 → 573          |
    | B     | 199 → 21,616                                 | 25 → 704          |
    | C     | 28,404 → 50,603                              | 925 → 1,649       |
    | D     | 58,854 → 67,698                              | 1,917 → 2,206     |
    | E     | 120,000 → 120,000                            | 14,707 → 14,707   |
    | BD    | 7 → 20                                       | 5 → 9             |

    The caps are the same before and after: A 11, B 68, C 6,764, D 8,193 and E 21,369 ly near
    the Sun, and in the nuclear disc A 13, B 17, C 116, D 205 and E 362 ly. `layer_caps`'s doc
    table gives the nuclear disc's D as 204 ly. HEAD reads 205 too, so the table predates T16.b
    (it was measured before the fitted envelope); T17 re-records it.

  - **How loose the bound is.** At 2 m₁ over every age, a B or C primary's bound reaches the
    envelope's late giants at η = 0 (about −5.7 for stars of at most 1 M☉, T6.b's science check),
    less 2.5 log₁₀ 5. The two pinned systems' bounds go from M<sub>V</sub> 3.78 and 2.70 to −7.78
    and −7.95, so a B or C record is almost never skipped inside any cap. Records past the mass
    skip and then the flux bound, at HEAD and after, from T8.e's place near the Sun (cut 7.95,
    eye), counted by a probe that mirrors `census_record`'s skip without generating (it gives
    T8.e's own nuclear-disc counts exactly):

    | Sphere      | Records   | Past the floor        | Past the bound      |
    | ----------- | --------- | --------------------- | ------------------- |
    | A, 300 ly   | 138,945   | 37,978 → 92,021       | 987 → 79,038        |
    | B, 500 ly   | 110,151   | 74,887 → 110,151      | 20,562 → 110,151    |
    | C, 1,000 ly | 1,097,274 | 1,092,043 → 1,097,274 | 951,125 → 1,097,259 |
    | D, 1,000 ly | 238,967   | all → all             | all → all           |
    | E, 1,000 ly | 74,777    | all → all             | 74,776 → 74,776     |

  - **The cost, measured.** The sample is 648 fixed cells near the Sun. At each distance along x,
    a 3 × 3 × 3 block of cells: A to 300 ly, B to 500, the brown dwarfs to 60, C to 6,000, D to
    8,000 and E to 20,000. The query is cut 7.95 with the eye. `census_cell` runs on one thread,
    and the figure is the least wall time of three runs. The builds ran alternately, twice each,
    in the debug profile with the sim at opt-level 2, at load 4–12, so the times are provisional:

    | Layer | Cells | Records | Past the floor | Generated     | ms a cell     |
    | ----- | ----- | ------- | -------------- | ------------- | ------------- |
    | A     | 108   | 62      | 35 → 48        | 17 → 45       | 0.055 → 0.065 |
    | B     | 108   | 112     | 97 → 112       | 65 → 112      | 0.077 → 0.103 |
    | BD    | 81    | 160     | 113 → 136      | 0 → 0         | 0.051 → 0.052 |
    | C     | 135   | 1,247   | 1,217 → 1,247  | 1,065 → 1,235 | 6.60 → 6.68   |
    | D     | 108   | 1,589   | 1,589 → 1,589  | 1,588 → 1,589 | 63.4 → 62.7   |
    | E     | 108   | 2,977   | 2,977 → 2,977  | 2,977 → 2,977 | 251.7 → 251.4 |
    | All   | 648   | 6,147   | 6,028 → 6,109  | 5,712 → 5,958 | 53.9 → 53.8   |

    Near the Sun, T16.b barely moves the census's cost. D and E were generated whole already. The
    C systems it newly generates are cheap ones, so C's cost a cell beyond 1,000 ly rises 2–3%.
    The cost rises steeply only in A and B, in relative terms, and their caps are 11 and 68 ly. But
    the floor and the bound now skip almost no B or C record, so the census's cost (T8.e's cold
    bench, `decision-r06-census-cost.md`) cannot be cut by tightening its skips without a rule
    that knows which pairs can interact.

  - **Three checks changed by ruling** (orchestrator, 2026-10-05; `decision-r06-census-cost.md`
    takes the figures). They are cost checks, not identities, and each site's comment names the
    decision. No identity check and no tolerance moved.
    - The fast `the_census_is_its_oracle_in_the_nuclear_disc` passes `Skips::Recorded` to
      `agree`. It now prints that the census generates 49,534 of 49,534 systems: A and B within
      1.5 ly skip none, where A skipped 1,122 of 21,450 before.
    - In the slow A/B test, B's per-layer check is printed, not asserted. B now generates
      110,151 of 110,151 systems, against 20,562 before. A's check holds: A generates 79,038 of
      138,945, against 987 before.
    - T8.b's `a_cells_census_equals_its_unskipped_records` keeps its `skipped > 0`. Its 15 cells
      (A–E at 0, 1 and 3 cells from the Sun, cut 7) no longer skip anything ("30 listed,
      0 skipped"), so it gains cells where skips remain. These are the brown dwarfs' at 0, 1
      and 3 cells out (seven records, each skipped by a forced single's bound), and a 5 × 5 block
      of A cells 288–320 ly along x. The block's records are skipped by the floor, and one
      candidate by a multiple system's bound.
  - **The tests.**
    - The `[profile.slow]` `default-filter` and its comment are gone from `.config/nextest.toml`,
      and "fails until R06.T16.b" is gone from both ignore reasons and from the module doc.
      The three slow tests ran by name, unlocked and capped, in the slow-test profile with
      `--test-threads 2` (2026-10-05, load 12–18, provisional), and each passed:
      - `the_census_is_its_oracle_1000_ly_from_the_sun`, in 858 s. The census generates
        1,411,002 of 1,411,018 systems: C 1,097,259 of 1,097,274, D all 238,967, E 74,776 of
        74,777. It accepts C 21,210, D 7,760 and E 1,247 stars, the oracle's counts, with the
        C merger.
      - `the_census_is_its_oracle_for_the_dwarfs_near_the_sun`, in 9.5 s. The census generates
        189,189 of 249,096 systems and accepts A 3 and B 36 stars, the B merger among them.
      - `envelope_bounds_pair_states`, in 49 s.
    - The pinned mergers are fast tests in `tests/sky_census.rs`. Each censuses its system's cell
      beside its oracle (the new `census_parts_of` and `brute_force_parts_of` in
      `tests/common/sky.rs`, for given cells) and checks four things:
      - the two agree bit for bit;
      - the merger product is listed, heavier than m₁ and at most `max_star_mass(m₁)`, its
        companion `NoRemnant`;
      - it is brighter than the bound at m₁ and its age, which would have skipped it for the
        query;
      - it lies within the widened bound.
    - `the_merged_giants_near_the_sun_are_listed_as_their_oracle_lists_them` takes T8.e's two
      giants at the eye's cut. `a_main_sequence_merger_is_listed_as_its_oracle_lists_it` takes the
      unevolved case the plan left to pin: `0x41feeca200000000`, C near the Sun, a 0.90 M☉
      primary and its near twin merged into a 1.80 M☉ main-sequence star of M<sub>V</sub> 1.98
      (bound at m₁ 2.35), V 5.87 from 191 ly. The query is a camera cut of 5.95 with no eye,
      inside the window (5.87, 6.02) where the bound at m₁ skips it.
    - That system is 267 Myr old, not "of an old cell" as the plan's test reads. Nor is it a blue
      straggler in Sandage's sense, brighter than the turnoff, which lies near 3.4 M☉ at 267 Myr
      (science check). It is a merger product above every single star of its primary's mass and
      age. A search of
      8,000 records in each of A, B and C, near the Sun, at (0, 8,000, 0) and in the bulge, found
      it the only main-sequence star above its primary's mass brighter than the bound at m₁.
      Mass gainers are rare (0–8 per 8,000 records), and in old cells the envelope at m₁ and the
      system's age already holds the turnoff's giants, as Design note 8 expected.
    - Unit tests: `max_star_mass_is_twice_the_primary_up_to_the_tracks_top`;
      `the_mass_floor_admits_every_primary_whose_merger_could_pass` (the floor inverts the
      envelope at `max_star_mass`, and some cut puts A's floor below where m₁ alone would);
      `a_forced_single_takes_its_own_envelope_bound`, whose grid half now reads 2 m₁ from zero.
    - The slow `envelope_bounds_pair_states` (`tests/sky_envelope.rs`): 1,000 multiple systems
      of each stellar layer near the Sun and 1,000 in the bulge (10⁴), at the epoch and 900 years
      before. Every pair-evolved star must be no brighter than the envelope at `max_star_mass`
      over ages from zero, margin included and without the n factor.
      - It finds no violation, and no star uses any of the 0.3 mag margin. The sample's closest
        star is 2.12 mag fainter than the envelope before its margin (D in the bulge).
      - Mass gainers are few: 12 star states above their primary's mass in the 10⁴ multiples,
        and none beyond the bound at m₁. The engine ran a pair in 0–5 of each 1,000 multiples in
        A and B, 323–408 in C, and 950–997 in D and E.
      - The three pinned mergers are therefore checked too (`PINNED_MERGERS`). Each is beyond
        the bound at m₁ at both times, and their closest is 0.31 mag fainter than the envelope
        before its margin.
  - **Science check (2026-10-05).** It found no must-fix. Its fixes are applied:
    - "Shines as a younger star of its new mass" is true only of main-sequence mergers and
      accretors (BSE eq. 80 and §2.6.6). A donor keeps its fractional age at a lower mass, so its
      own track's age can pass the system's by up to some 40 times. An evolved accretor keeps its
      track's luminosity. `flux_bound`'s doc now says so. Each is fainter than a single star of at
      most 2 m₁ at an age within the system's, which `envelope_bounds_pair_states` checks
      empirically.
    - A main-sequence accretor above 150 M☉ is not held to the 150 M☉ track. Its closed forms are
      extrapolated, and the top node bounds it through that track's later phases and the margin.
    - If plan 11 ever evolves a merged pair with its third star, the ceiling becomes 3 m₁.
  - **Not done here (ruled):** plan 11's public interaction test, which would let pairs that
    cannot interact keep n × F₁ at m₁ and their own age. It is the lever decision item 2 names
    for T17, handed to the cost ruling.
- **Deviations in T8.f, as built (2026-10-05).** The cheap exact steps of
  `decision-r06-census-cost.md`, and T16.b's open item on the brown dwarfs' floor. Every listed
  and overflow star is unchanged, bit for bit. The identity tests, the oracle and their
  assertions are unchanged.
  - **The steps as built.**
    - The sightline cut is taken under `Bound::Applied` only. The oracle (`Bound::Ignored`) still
      takes every star's sightline. The star's V is still summed as (M_V + DM) + A_V. A_V is a sum
      of non-negative dust columns, so the sum is never below M_V + DM.
    - One addition to make that unconditional: the census holds a negative A_V at zero
      (`star_extinction`), compared rather than clamped by `max`, so that a NaN still shows. The
      science check found that a modifier cloud's column, a difference of two values of its
      antiderivative, can round below zero far outside the cloud. That happens beyond some 100
      cores, while clouds reach 10, but a nursery's reach can be many cores. Census and oracle
      share the path. With no modifiers, as today, nothing changes. The rounding itself is plan
      09's `plummer_column`, a note for its owner.
    - `BrightnessEnvelope` keeps each node's brightest over every age bin (`brightest_ever`).
      `mass_floor` reads it whenever its ages span every bin. The bisection is unchanged, and its
      test at each step compares the first passing node's lower neighbour with the star mass:
      the scan's answer, since the brightest never rises with mass. The constructor asserts that.
      Other age ranges still take the scan.
    - The offset table is a new type, `sky::census::CellOffsets`, built per galaxy (43 ms in the
      debug build, 16 KB). `SkyContext` gains `offsets: &CellOffsets` beside the tables and the
      envelope, and `cell_floor`'s signature becomes `(galaxy, ctx, key, query)`. Its nodes are
      16 to the octave from 2⁻⁴ to 2¹⁷ ly, 337 of them. Each holds the running maximum of the
      offset bound at `tidal_radius_bound_within` of the layer's 5 × band top. A cell reads the
      first node at or beyond its farthest corner from the centre. `tidal_radius_bound_within`
      bounds the tidal radius anywhere within its radius, so the value bounds every record's own
      `star_offset_bound`. At 10⁴ random cells it is at least `cell_offset_bound` and at most
      1.034 times it. The ruling's "so it is never smaller than the exact one" holds wherever the
      circular frequency does not rise between the cell's corner and its node, since the exact
      bound reads Ω² at the corner itself (science check). The census needs only the bound on each
      record's offset, which holds without that condition. The plan's sentence stands as ruled,
      and the code's docs state the condition.
    - The table keeps the parameters of the galaxy it was built for, which fix its potential.
      `CellOffsets::is_for` checks them, and the census asserts it in debug builds, since another
      galaxy's table could be too small (determinism audit).
    - The record's offset is the cell's, read once per cell with the pad and the least distance
      (a private `CellReach`). `census_record`, called per record, finds its record's cell.
    - No n factor in `flux_bound` or the floor. A forced single's bound was n = 1 already, so its
      bits are unchanged.
    - The bound before the drift (`passes_before_drift`) uses the cell's pad. That pad bounds the
      record's displacement at the retardation's first guess too, from which the light's age is
      taken. So the light's age lies within the pad of the epoch distance, and the apparent
      distance is at least the epoch distance less the pad. The ages are widened by 1 year and the
      distance shortened by 10⁻⁹ of itself, beyond the pad, for rounding. A multiple's ages run
      from zero, as after the drift; a forced single's span the light-time interval. The pad
      rests, as the range query's padding and the floors do, on every grid record moving slower
      than its layer's `pad_speed`. Plan 08's draw holds speeds below the least of the escape
      speed and 1,000 km/s, and layer E pads at 3,000 km/s. Plan 08 places nothing faster than
      1,000 km/s outside layer E. Its kicks are at most 990 km/s since ruling 96.2, and its exempt
      classes are capped below their layer's pad. P09.T34.b raises layer D's pad together with the
      survivors it places there. So nothing breaks the premise (decided 2026-10-05,
      `decision-r06-pad-speed.md`). P08.T17 asserts it at every grid velocity, and R06.T8.j tests
      the census in motion.
    - The plan holds each layer's padded sphere, and `CensusPlan::cells()` streams the cells.
      `cell_count()` counts them column by column (by walking, for a cone). The jobs are
      `CensusPlan::slabs()`: a `CellSlab` for each x slab of a layer's walk, which streams its
      own cells. `plan_cells` keeps the held list. The slabs need two functions in plan 03's
      `galaxy/query/walk.rs`, outside T8.f's file list: `sphere_slabs` and
      `cells_in_sphere_slab`. `cells_in_sphere` now runs on the same per-slab walk, its output
      unchanged. The walk's tests check, for every sphere they walk, that the slabs in turn are
      `cells_in_sphere`, and that the cells either side of the slabs hold nothing.
    - The bench's jobs are the slabs. With `HYPERION_SKY_BENCH_SAMPLE=k`, each job censuses the
      cells whose `sample_hash`, a SplitMix64 mix of the layer and the origin's coordinates, is 0
      modulo k. The plan's time and the walk of every slab's cells are counted once. The sampled
      cells' census is scaled by k, and that estimate is what each iteration returns.
  - **Added: the brown dwarfs' floor** (the task's assignment, T16.b's open item). A layer whose
    systems are all single, `envelope::always_single` (the brown dwarfs and rogue planets, after
    `SystemKind::of_layer`), takes each primary's own mass in `mass_floor`
    (`envelope::max_star_mass_in`). Its offset is zero: `cell_offset_bound` returns zero, and the
    table holds none. A test holds every record to it: a record is a forced single exactly when
    its layer is `always_single`. Within 150 ly at cut 11, the brown dwarfs' candidates fall from
    5,102 to 76, against 1,073 before T16.b. None is generated either way.
  - **Other changes.**
    - `tests/common/sky.rs` collects the forced plan's cells into a vector, as before, so that the
      oracle's threads take one cell at a time. It builds a `CellOffsets` for each context.
    - `a_forced_single_takes_its_own_envelope_bound`'s grid half now asserts that the bound
      equals the envelope at 2 m₁ from zero bit for bit, with no 2.5 log₁₀ 5.
    - Design note 10 takes the ruling's replacement. It keeps a forced single's bound at m₁ and
      its age, and adds a sentence on the bound before the drift and the sightline cut.
    - Of the ruling's plan text only T8.f's is applied here, with Design note 10's. T16.b's, T9.b's
      and T17's amendments, T8.g–i, T7.b, T11.d, Design notes 8 and 12, and the Risks and
      Verification lines are left for the orchestrator. _Applied 2026-10-05 by the census lane's
      docs commit, with the sign-off's amendments (`decision-r06-census-cost-signoff.md`)._
  - **The tests** (fast unless named slow):
    - `the_floor_over_every_age_is_the_scan_bit_for_bit`: 10⁴ random queries, a seventh of them at
      a node's own value. 1,165 of them bisect inside their band.
    - `the_offset_table_is_at_least_the_exact_bound`: 10⁴ random cells from 1 ly to beyond the
      cube's faces.
    - `the_bound_before_the_drift_never_rejects_what_the_bound_after_it_passes`: some 13,000
      record and observer pairs, in a galaxy built `with_full_potential` so that the records move.
      The fixture's galaxy has no kinematic tables, so its drifts stand still. Observers stand at
      the Sun at the epoch, 250 ly from it at +900 years, and 126 ly from it at −700 years.
    - `the_sightline_cut_never_drops_a_star_the_cut_keeps`: the stars of 10⁴ generated systems of
      every layer, near the Sun and in the bulge, with each star's A_V checked non-negative.
    - `streamed_cells_are_plan_cells`: T8.a's three queries, with the count and each slab's
      layer and x.
    - `a_systems_heaviest_star_is_its_primary_where_every_system_is_single` and
      `the_brown_dwarfs_floor_reads_each_primarys_own_mass`. The latter checks that a single
      layer's floor inverts the envelope at m₁, and that some cut raises it above the floor at
      2 m₁.
    - Reviews: the determinism audit found nothing to fix (`golden_diff` reconciles nothing;
      GENERATOR_VERSION 20 → 20). It suggested the table's galaxy check and the A_V guard, both
      taken. The science check found no must-fix. Its three doc fixes are applied: the table's
      condition, "under a microsecond a cell", and the SplitMix64 citation (Vigna 2015's
      `splitmix64.c`, Stafford's Mix13). So are its notes on the first guess's light time and on
      the speed premise.
    - Gates (2026-10-05, every run capped): fmt; clippy native and wasm32-wasip1; `--lib`
      `sky::` and `galaxy::query::walk`; `--test sky_census`, 6 of 6. Slow, by name, unlocked, in
      the slow-test profile with `--test-threads 2`, at load 12–19, so provisional:
      - `the_census_is_its_oracle_1000_ly_from_the_sun` passes in 797 s, and in 787 s on the
        final code, after the reviews' changes. The census generates
        1,406,413 of 1,411,018 systems, against T16.b's 1,411,002: C 1,092,670, D 238,967 and
        E 74,776. It accepts C 21,210, D 7,760 and E 1,247 stars, as before.
      - `the_census_is_its_oracle_for_the_dwarfs_near_the_sun` passes in 5.9 s (5.8 s). A takes
        42,099 records past the floor, against 92,021, and generates 37,820 of 138,945, against
        79,038.
        B generates 108,000 of 110,151, against all of them, so B skips again. Its check stays
        printed until T8.g restores it. A accepts 3 stars and B 36, as before.
      - The fast 150 ly test generates 31,673 of 39,943 systems, against 31,675.
  - **The cost, measured.** The sample is T16.b's: 648 fixed cells near the Sun, at cut 7.95
    with the eye, censused by `census_cell` on one thread. Each figure is the least wall time of
    three runs, in the debug build with the sim at opt-level 2. The stages were timed by
    temporary instrumentation, never committed. The builds ran alternately, twice each, at load
    about 15–20, so the times are provisional. Every layer's stars hash the same before and
    after, bit for bit.

    | Layer | Past the floor | Generated     | Sightlines  | ms a cell     | `cell_floor`, µs |
    | ----- | -------------- | ------------- | ----------- | ------------- | ---------------- |
    | A     | 48 → 37        | 45 → 37       | 54 → 3      | 0.059 → 0.012 | 42.2 → 0.51      |
    | B     | 112 → 112      | 112 → 110     | 176 → 17    | 0.096 → 0.043 | 37.6 → 0.15      |
    | BD    | 136 → 28       | 0 → 0         | 0 → 0       | 0.047 → 0.003 | 41.6 → 0.15      |
    | C     | 1,247 → 1,247  | 1,235 → 1,229 | 1,558 → 203 | 6.67 → 6.26   | 37.6 → 0.16      |
    | D     | 1,589 → 1,589  | 1,589 → 1,589 | 651 → 84    | 62.7 → 62.4   | 37.6 → 0.16      |
    | E     | 2,977 → 2,977  | 2,977 → 2,977 | 437 → 85    | 251.2 → 250.5 | 37.8 → 0.16      |
    | All   | 6,109 → 5,990  | 5,958 → 5,942 | 2,876 → 392 | 53.7 → 53.5   | 39.0 → 0.36      |

    The stages, summed over the 648 cells, from the quietest run of each build (ms, with µs a
    call):

    | Stage                      | Before         | After          |
    | -------------------------- | -------------- | -------------- |
    | `cell_floor`               | 25.3 (39.0)    | 0.23 (0.36)    |
    | of which the offset        | 24.2 (37.3)    | 0.06 (0.09)    |
    | of which `mass_floor`      | 0.97 (1.50)    | 0.12 (0.18)    |
    | `bright_subset`            | 10.0           | 9.75           |
    | bound before the drift     | none           | 3.83 (0.64)    |
    | `Drift::of_record`         | 0.54           | 0.61           |
    | `retarded`                 | 2.99 (0.49)    | 2.90 (0.49)    |
    | flux bound with its offset | 9.36 (1.56)    | 1.93 (0.33)    |
    | `SystemStars::generate`    | 34,581 (5,804) | 34,643 (5,830) |
    | `state_at`                 | 107            | 107            |
    | positions and photometry   | 7.4            | 7.4            |
    | `sightline`                | 91.6 (31.9)    | 3.37 (8.6)     |
    - `cell_floor` meets its gate of 2 µs a cell, at 0.15–0.51 µs. A's 0.51 µs is its floor's
      bisection, inside the band.
    - The bound before the drift costs 0.64 µs a record, twice the bound after it, since its age
      range reads more bins. On this sample it rejects all 28 of the brown dwarfs' records past
      the floor, 18 of C's and one of B's. With the bound after the drift, a record now costs
      0.97 µs of bounds, against 1.56: the offset is no longer a tidal radius a record.
    - The sightlines fall by 86% and their time by 96%: only stars that the cut could keep take
      one.
    - The stages outside generation fall from 254 to 137 ms, by 46%. But generation is 99.3% of
      the sample's time before and 99.6% after, so the whole moves by under 1%, inside the load's
      noise. The ruling expected some 20,000 of 1.6 × 10⁶ CPU-s, 1.2%.

  - **The sampled bench.** `HYPERION_SKY_BENCH_SAMPLE=1000`, `sky/census_near_sun/cold` once
    (criterion's `--test`), under the heavy-test lock, which it held for 143 s. It ran on 15
    workers at load about 15, so the figures are provisional. The caps are today's: C 8,193,
    D 9,925 and E 21,369 ly. The plan holds 1.08 × 10⁸ cells and took 2.4 s, with
    `layer_caps`, on one thread. Walking every slab's cells took 8.1 CPU-s. The sample's census
    took 1,829 CPU-s, 125 s wall.

    | Near the Sun, estimated | C          | D          | E          |
    | ----------------------- | ---------- | ---------- | ---------- |
    | Cells                   | 7.13 × 10⁷ | 1.61 × 10⁷ | 2.05 × 10⁷ |
    | Records past the floor  | 1.99 × 10⁸ | 6.51 × 10⁷ | 1.34 × 10⁸ |
    | Generated               | 98.26%     | 99.92%     | 99.99%     |

    The cold estimate is 1.83 × 10⁶ CPU-s. The ruling's own runs gave 1.6–2.0 × 10⁶ before
    T8.f at load 15–17, and it expected about 1.64 × 10⁶ after. This run, at a similar load, lies
    within that spread: nearly all of it is generation, which T8.f does not touch. T8.g's bound
    star by star is the step that moves it (its gate: at most 1% of the records past the floor
    generated, and a cold census of at most 10,000 CPU-s). The plan no longer holds 2.2 GB of
    cell keys.
- **Plan 09's `plummer_column` can round below zero (found in T8.f; a pointer for plan 09's
  owner).** A modifier cloud's column, the difference of two values of its antiderivative
  (`galaxy/gas/extinction.rs`), can round just below zero far from the cloud, beyond some 100
  cores. The census holds a negative A_V at zero (`star_extinction`), so it is guarded. The
  rounding is plan 09's to fix.
- **The pad speed premise (decided 2026-10-05, `decision-r06-pad-speed.md`).** Found in T8.f; this
  replaces that task's pointer for plan 08's owner, whose kick figure the ruling retires.
  - The census's walk and its bound before the drift rest on every record of a layer moving
    below `pad_speed(layer)`. So do plan 03's range query and plan 12's lensing walk.
  - It holds today, when every grid record moves below min(v_esc, 1,000 km/s).
  - It holds after plan 08. Its kicks are at most 990 km/s since ruling 96.2, and its fast classes
    are layer E's, which pads at 3,000 km/s.
  - It holds after P09.T34.b, which raises layer D's pad together with its survivors.
  - Nothing the census lists moves faster than 1,000 km/s, since neutron stars, black holes and
    white dwarfs have no V. That is right for neutron stars and black holes at these cuts; white
    dwarfs wait for A4, and the real ones brighter than 11.0 (Sirius B, 40 Eri B, Procyon B) are
    slow companions. The stars expected to be missed near the Sun are 0 at cut 7.95 and 0 at
    11.0.
  - P08.T17 owns the premise and asserts it, P09.T23.b pads feature members by their own bound,
    and R06.T8.j tests the census in motion.
- **Deviations in T9.b, as built (2026-10-06).** `sky::band::{CubeFace, BandSpec, BandTexel,
band_rows}` as Design notes 14 and 15 set them out, with these differences.
  - **The amendment's argument.** `band_rows(galaxy, ctx, query, census, complete_to, spec, face,
rows, out)` takes `complete_to: &CompleteTo` after the census. `CompleteTo` is new and public:
    one radius for each layer of `CAPPED_LAYERS`. `of_caps(&[LayerCap])` gives each layer its cap
    (nowhere for a layer the caps lack), `everywhere()` serves a band with no census (T9.d's
    pre-pass), and `nowhere()` and `radius(layer)` complete it. Until T8.i a census is complete to
    its plan's caps; T7.b makes the radii per ray and T8.i may move them onto `SkyCensus`. Each
    radius is a node of its ray, so the trapezoid never straddles the step from the fainter light
    to all of it. `out` is appended to, row by row from the top, each row from its left.
  - **`BandSpec`** holds `face_texels` and `nodes_per_decade` (the sketch's "steps per ray"; a
    ray's node count depends on its distance to the root cube's edge): `STANDARD` (64, 12),
    `Default`, `new` returning `None` for a zero or a face above `MAX_FACE_TEXELS` (1,024). Added
    for T9.c, T11 and the tests: `texel_direction`, `texel_of` (WebGPU's face orientation and the
    client's x-then-y-then-z ties, in `f64`, not T13.b's `f32`-exact rule, so an overflow star on
    a texel edge may land one texel over from the client's) and `texel_solid_angle_sr`, the exact
    atan2 area of `texelSolidAnglesSr`; both texel functions panic off the face. `CubeFace` gains
    `ALL` and `layer()`.
  - **The rays.** `FIRST_NODE_LY` 0.01 ly; `BAND_PROFILE_QUALITY` `Budget(256)` (P07.T10.c's
    `SIGHTLINE_QUALITY` value), with the modifiers near each ray. The light is the trapezoid
    rule's in distance; twelve nodes a decade agree with 48 to 0.02 mag. The light nearer than
    0.01 ly, at most some 10⁻⁵ of a ray's, is left out. A ray outside a query's cone is complete
    nowhere.
  - **The colours** of the texels and of the overflow's points are the stars' own, before
    reddening; extinction dims the luminance in V only (reddened from R06.T9.e), which for
    starlight over-dims the photopic light by 1–2% of A_V (science check, CCM 1989 against CIE
    V(λ)). A texel with no light is white, (1, 1), at the reference ρ.
  - **Added in `sky::luminosity`:** `LuminosityFunction::colour_sums_fainter_than` (crate-only),
    the unnormalised sums, which `colour_fainter_than` now divides (same bits). A test holds each
    sum, over every layer and light age, to the snapshot read bit for bit.
  - **The tests** (`cargo test -p hyperion-sim sky::band`, 15 with the luminosity read's; about
    140 s on two threads, most of it the shared table build and one census):
    - the row split: four splits, rows run last first, warm and cold noise caches, a census's
      overflow past `n_max` 20 among the faces, bit for bit; the overflow's light reaches the
      faces to 10⁻⁹;
    - above the disc, 2,000 ly above the Sun, the texels 1.3° from each pole at `STANDARD`: μ 26.69
      towards the pole and 23.35 towards the plane, each within 3% of the model's own integral (96
      nodes a decade, full-quality sightlines, the tables' public reads: 26.70 and 23.34), and so
      their ratio;
    - near the Sun, the stars fainter than V 6.5 (no census, complete everywhere), 16² faces, each
      region's mean luminance: the plane (|b| under 5°) μ 22.09 against 22.4 and the poles (|b| over
      80°) 24.57 against **24.3, the brainstorm table's figure, not the task text's 24**, which 24.57
      misses by 0.07 mag (pending a ruling, below). The science check summed Gaia DR3 itself by the
      test's definitions (V from G by Riello et al. 2021, Table C.2): 22.0 in the plane and
      24.25–24.29 at the poles;
    - the cut: a census near the Sun to V 8 within 200 ly, every cap forced, no eye, restricted to
      V 7 and V 6 (a census of a brighter cut keeps a subset of a deeper one's stars at the same V):
      the listed and band light together hold within 0.02%, the list losing 4.34 × 10⁻⁶ lx from V 8
      to 7 as the band gains 4.46 × 10⁻⁶, and 6.02 against 6.10 × 10⁻⁶ from 7 to 6; at `n_max` 100
      the light leaving the list reaches the band to 10⁻⁹;
    - the complete-to radius, a test of its own: complete to 100 ly against 200 ly, with the census
      listing only the stars within the radius (as T8.i's partial replies will), 0.68% apart (the
      next item for the band of no census);
    - the cone, the modifiers (a Plummer cloud on +X dims that face and leaves −X's bits), a
      texel with no light, rows or a texel off the face, the face mapping (the client's own
      `cubeTexelOf` cases, and every texel's centre back to itself), the solid angles (4π).
  - **The bench** `sky/band_near_sun`: the six faces of `STANDARD` near the Sun at cut 7.95 with
    the eye, complete to `layer_caps`, an empty census, one face row a job. One run (criterion's
    `--test`, under the heavy-test lock, load 3–5, provisional): 20.2 CPU-s on 15 workers, 1.37 s
    wall. On one thread at 16² the reads of the luminosity functions take four fifths of a ray
    (some 135 ns each, 6 layers × 17 components a node) and its profile one fifth. Sharing each
    node's age bracket and magnitude bin across its functions is the first lever, if T17 needs one.
  - **Cross-target bits** rest on review (determinism audit: nothing to fix) until T17's
    `sky/band_face_row.golden`, which should take a `CompleteTo::of_caps` radius inside its rays
    and a census with an overflow, so that the radius nodes and the points are pinned too.
  - Not built: `eye_limit` stays `None` (T9.c sets it, and will need a crate-visible setter);
    `sky/limits.rs` (T9.c, T9.d).
- **The band's conservation (found in T9.b; for the orchestrator).** Against the band of no census
  (complete nowhere, all of the light), the listed and band light near the Sun to V 8 fall short by
  1.12% complete to 100 ly and 1.79% to 200 ly: the realised census lists 79% and 82% of the light
  the tables expect of its stars brighter than the cut. At 300 ly, per layer, the listed light is C
  86%, D 87% and E 63% of the expectation, over 5,679, 642 and 143 stars. The counts are C 0.95,
  D 0.75 and E 0.85 of `count_brighter_than`, which keeps only the pair-evolved excess by design
  (T5.d), so they read high by an amount not yet measured; C's and D's are 4σ and 6σ below it,
  which skew cannot make. Two causes (science check): the skew of a light sum that the rare
  bright and the nearest stars carry (about r₁ ÷ R of a type's light within R lies inside the
  radius r₁ that holds one expected star of it: some 15–20 pc for giants, a third of the local V
  light, and 4 pc for A and F stars), so most realisations hold less than the mean and the deficit
  shrinks slowly with the radius (57% listed at 50 ly); and T5.c's realised-against-table
  residuals at the solar circle (C −4.4%, D −20.4% with the correction), a systematic of the same
  sign that likely carries most of D's. Averaging over some eight observers on the solar circle,
  or leaving each layer's nearest r₁ out of both sides, would part them (the check's estimate of
  what remains: 5–10%). Between two radii the census lists
  (100 and 200 ly) the light holds to 0.68%, and between cuts to 0.02%. Not asserted: radius zero,
  which no reply has (the view shows `STARS: PENDING` until the first, sign-off condition 4); and
  T8.i's first shell (500 ly) against the final caps, which needs a census past a unit test's
  cost and is T8.i's or T17's to measure.

  Ruled 2026-10-06 (`decision-r06-t9b-band.md`): not the band's. The light's 79–82% is a low but
  possible realisation under the sum's skew (median about 90%); it and the counts' C and D
  deficits are measured by R06.T5.f.

- **The band's boundaries that are the census's (found in T9.b; for the orchestrator).**
  - **The eye's colour offset (decided 2026-10-06, `decision-r06-t9b-band.md`: R06.T8.k keeps
    the census to the cut alone).** With the eye asked, the census keeps each star to the cut plus
    its colour offset at μ 30 (T8.b), while the band subtracts at the cut alone, as Design note
    15 states it. A blue star between the cut and the cut plus its offset is listed and in the
    band; a red one between the cut plus its (negative) offset and the cut is in neither. The
    tests run without the eye. The science check estimates some 2% of the band's light misallocated
    at the poles (half counted twice, half missed; net 0.5% or less, under 0.01 mag in μ and 0.005
    mag in the eye's cut) and some 1.5% gross in the plane. Options: accept it as stated; have the
    band subtract each M_V bin at the cut plus the bin's mean offset (the tables carry each bin's
    ρ); or have the census keep to the cut alone and leave the offset to the views (a T8.b
    change).
  - **A cone (decided 2026-10-06, `decision-r06-t9b-band.md`: R06.T8.k lists only the stars
    inside the cone).** A ray outside the cone is complete nowhere, but the census lists every
    star of each cell whose padded ball meets the cone, some outside it, whose light the band then
    holds too, as the caps' overshoot is held. Options: accept it; or have the census list only
    the stars inside the cone (a T8 change).
  - **The final reply's cells.** A final census lists every star of the cells it opens, some just
    beyond its caps, whose light the band also holds, within the caps' stated expected count
    beyond, as the sign-off's condition 2 accepts.
  - **One march per call.** `band_rows` marches its rays on every call, and the radius nodes move
    every layer's nodes. T11.d's one march a request, re-summed per reply (sign-off condition 5),
    needs the march split from the sums: each layer's running fainter and all-light sums at fixed
    nodes with the shell edges among them, then a cheap sum per reply. R06.T9.f builds it (decided
    2026-10-06, `decision-r06-t9b-band.md`); T11.c and T11.d consume it.
- **Pending rulings from T9.b.** Ruled 2026-10-06 (`decision-r06-t9b-band.md`):
  - the poles 24.3 and the plane 22.05;
  - T9.c 6.5 ± 0.20 and 7.55 ± 0.22, and T9.d 8.15 ± 0.22;
  - reddening (R06.T9.e);
  - the census keeps to the cut (R06.T8.k);
  - the independence covers the radii a reply states (T17);
  - the march split (R06.T9.f);
  - the shortfall measured (R06.T5.f);
  - the diffuse galactic light (R06.T9.g).
- **The galaxy's local light is low (found in T9.b; a pointer for the galaxy plans' owner).** The
  fixture's V luminosity density at the Sun is 0.042 L☉ pc⁻³, against Flynn et al. 2006 (MNRAS 372,
  1149): 0.056 for all stars, 0.045–0.047 for M_V ≥ −1, about 10% uncertain. Half of their column,
  24.4 L☉ pc⁻², gives an all-star polar μ_V of 23.67, where the band reads 24.0 (NGP) and 23.9
  (SGP), and the poles of the stars fainter than V 6.5 are 0.28–0.32 mag fainter than the science
  check's Gaia sums: the same size and sign. The 0.5 mag tolerance of T9.b's test holds it.

  Measured 2026-10-06 (`decision-r06-t9b-band.md`):
  - The local V light is 0.0417 L☉ pc⁻³, 20% of it brighter than M_V −1 as in Flynn et al.'s, so
    the deficit is the turnoff and clump light per unit mass (volume (M/L)_V 1.00 against 0.75 ±
    15%).
  - The column is 19.2 against 24.4 L☉ pc⁻². Σ★ is 30.5 against 33.2 without brown dwarfs, and
    the column's M/L is 1.59 against 1.36. That is 0.26 mag, which accounts for the poles' 0.29.
  - It does not touch the census's 79–82%, which compares the model with itself.

  A calibration finding for galaxy plans 02 and 06 (and 11 for the pair light). Their owner
  compares the fixture's Φ(M_V) with Hipparcos/CNS5 and adds a light row to plan 02's brackets.
  R06 changes nothing; its near-Sun tests hold the offset.

- **No diffuse galactic light (found 2026-10-06, `decision-r06-t9b-band.md`).** The band holds
  direct starlight only. The light its dust scatters, about 10–35% of the integrated starlight
  and 23–47% of the band's own background (the light fainter than the cut), makes the eye's
  limits about 0.10–0.18 mag too deep. R06.T9.g decides it.
- **Feature members are out of RM3's scope (decided 2026-10-05, `decision-r06-t16a-scope.md`).**
  - **Why.** R06.T16.a needs:
    - P08.T12 and P09.T2.c, two generator-version bumps of the galaxy plans;
    - P09.T23.b;
    - a part of P09.T40.a. P09.T40 is built on no branch and, as written, follows plan 09's
      phase 7.

    None of that is RM3's, and it would put two bumps and an unmeasured member cost on the
    census's critical path.

  - **What the sky is meanwhile.** Until P09.T2.c the field keeps the members' share, so the sky
    has the right stars and light in expectation, spread through the field.
    - What it lacks is clumps: no Pleiades- or Hyades-like clusters, no OB associations
      gathering the bright B stars, and no globulars as naked-eye points.
    - In the real sky, such members are about 5% of the 8,874 stars to V 6.5, but 16% of the B
      stars. Scorpius–Centaurus alone has 157 B-type members, 107 of them brighter than V 6.5
      (de Zeeuw et al. 1999).
    - Sightlines carry no clouds or superbubble holes from plan 09 (`FeatureGas`), as no other
      consumer does yet.
    - Every view states the gap: `CLUSTERS: NOT YET MODELLED`.
  - **The rule that keeps it honest.** P09.T2.c lands only with R06.T16.a. Members before it
    are counted twice. P09.T2.c alone would take φ of each population out of the sky and its
    band. That is about nine in ten of the O and early-B stars, since φ_young stays near 0.9
    until associations dissolve from 30 Myr.
