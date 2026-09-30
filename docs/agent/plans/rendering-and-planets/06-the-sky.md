# Plan R06: The Sky

- **Milestone:** Rendering milestone RM3 (with R07).
- **Depends on:** [R01 Graphics platform and the engine adapter](01-graphics-platform-and-engine.md)
  (the packed cube, the offscreen point pass and the smoke harness, R01.T8.d and T9),
  [R02 Real-scale foundations and the wireframe `VIEW`](02-real-scale-view-and-wireframe.md)
  (the `view/` camera, the photometric pipeline, the PSF and the per-sprite tone curve),
  [R03 The scene subscription and bulk transport](03-scene-subscription-and-transport.md) (outbound
  binary frames and their envelope); galaxy plans
  [06](../galaxy-generation/06-stellar-stage.md) (the stellar brief, `BriefModel`, photometry),
  [09](../galaxy-generation/09-features-and-catalogue-classes.md) (feature members, from P09.T40)
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
impl LuminosityTables { pub fn build(galaxy: &Galaxy) -> Self;
    pub fn get(&self, component: ComponentId, layer: Layer) -> &LuminosityFunction;
    pub fn heap_bytes(&self) -> usize; }

// sky::envelope — the skips (Design note 8)
pub struct BrightnessEnvelope { /* per layer: brightest M_V by mass ceiling and age range */ }
impl BrightnessEnvelope { pub fn build(galaxy: &Galaxy) -> Self;
    pub fn brightest(&self, layer: Layer, component: ComponentId, mass_at_most: SolarMasses,
        ages: (Years, Years)) -> Option<Magnitudes>;
    pub fn mass_floor(&self, layer: Layer, component: ComponentId, faintest: Magnitudes,
        ages: (Years, Years)) -> SolarMasses; }
pub fn max_star_mass(primary_initial: SolarMasses) -> SolarMasses;  // m₁ now; min(2 m₁, 150) once
                                                                    // plan 11 wires binaries

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
  spectrum at R08's `BAKE_WAVELENGTHS_NM` (Design note 6).
- `@hyperion/protocol`: `decodeSkyStars`, `decodeSkyBand`, the payload types, `SKY_STAR_BYTES`,
  `SKY_TEXEL_BYTES`.

### Client (`apps/hyperion/src/renderer/src/view/sky/`)

`SkyModel`, `useSky(requests, cameras)`, `eyeLimitAt(model, direction, fieldFactor)`,
`cameraLimitV(sensor, exposure: ExposureTriple, fovDeg, backgroundCdM2)` with `DEFAULT_VIEW_CAMERA`,
where `ExposureTriple` is R02's `{ aperture, shutterS, iso }` (the argument of `ev100FromTriple`),
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

`crates/hyperion-sim/tests/common/sky.rs` (built in R06.T8.b): `brute_force_sky(galaxy, query,
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
  `ViewRole` on the view's `CameraState` (Design note 5).
- **R03:** outbound binary frames (`bulk::{encode_header, BinaryFrameHeader, chunk, Answer}`,
  `MAX_BINARY_FRAME_BYTES`, `BULK_QUEUED_BYTES`), `BulkManifestDto`, and on the client
  `parseBinaryFrameHeader`, `BulkAssembler` and `requestBulk`; `TestClient::next_binary()`.
  The scene's host stars' drawn positions (light-time and aberration) for the discs. Chunks are
  assembled and decoded only once complete, off the main thread (R03's Design note 11).
- **R01:** the engine adapter, its device, its WGSL-only guard, `createBuffer`, `createTexture`,
  `createCompute` and `dispatch`, `WGSL_CATALOGUE` (where every shader here is registered), the
  smoke harness (`just test-render`, readback by `copyTextureToBuffer`), `MemoryCategory` (which
  this plan extends), `GpuCapabilities.float32Blendable`, and the packed cube, `createPackedCube`,
  with Babylon's `_hardwareTexture` pinned by R01's typecheck of `internals.ts` and the harness's
  cube round trip (R01.T9.g). **Provided by R01** (its Provides, `RenderEngine`, built with
  R01.T8.d), which this plan asked for and now consumes by these names:
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
- **R07:** `ExposureReading.triple` (`{ aperture, shutterS, iso }`, R07.T13.a), which `cameraLimitV`
  reads once R07 meters; until then the manual triple of R02. R07's `METER_CLASS` (`hostDisc: 0`),
  which the disc pass writes in the HDR target's alpha, with every translucent sky pass (sprites,
  band) blending alpha as source zero, destination one so the class survives (R07's Design note 10),
  which R01's `blend: "additive"` does (R01 Design note 21, R01.T8.i); and R07's
  `GlareSource { direction; angularRadiusRad; excessLuminance: Rgb }` (cd/m² above 65,504), which
  `HostDiscLayer.glareSources` returns (R07's Design note 12). R06 lands before R07 and needs
  nothing of R07 at build time: where R07's `post/` module does not yet exist, R06.T13.e declares
  `METER_CLASS` and `GlareSource` there under R07's names and shapes, and R07 extends that file. R07
  consumes `HostDiscDto`, `glareSources` and `DEFAULT_EYE_OBSERVER`.
- **Galaxy plan 03:** `CellKey`, `generate_cell`, `SystemRecord`, `cells_in_sphere`, `pad_for`,
  `query::pad_speed(Layer)`, `layer_spec`.
- **Galaxy plan 04:** `RequestBody`/`ResponseBody`, `REQUEST_KINDS`, `is_large`,
  `compute::{CpuPool, Priority::Bulk, CancelToken, SingleFlight}`, `cache::ByteLru`, the ±H check,
  the reserved-kinds table.
- **Galaxy plan 06:** `BriefModel`, `SystemStars::{generate, stars, brief_at}`,
  `StarModel::state_at`, `stellar::photometry::{absolute_magnitude_v, bolometric_correction_v}`,
  `premain::protostar_class`, `galaxy::fates::{fates_for, CompanionMasses, StellarFates}`, the
  `system_summary` DTOs. Its asks: A1–A4 below.
- **Galaxy plan 07:** `galaxy::gas::extinction::{sightline, horizon, NoiseMode, Quality}`,
  `NoiseCache`, `ccm::extinction_ratio`, `Band`.
- **Galaxy plan 09:** `FeatureMemberSource` through the server once P09.T40 registers it (R06.T16).
- **Galaxy plan 11:** `stellar::multiplicity::star_positions_at(&SystemHierarchy, t, out)`, the
  hierarchy in `SystemStars`; and, once P11.T6–T11 wire binary evolution into the system stage,
  the rule that every star's mass is at most the pair's total (Design note 8).
- **Galaxy plan 12:** `observe::{Observer, retarded, Drift::of_record, Retardation,
TraceMotionError}`.
- **Galaxy plan 13:** the substellar layers' records and briefs (brown dwarfs through plan 06's
  cooling fits).
- **Galaxy plan 15:** `hyperion-fit`'s dataset and emit machinery (`data.rs`, `emit.rs`,
  `PROVENANCE.toml`), its manifests (`crates/hyperion-fit/manifests/<task>.toml`, with a
  `<task>.smoke.toml` for a slow task), `tables.lock`, `tables::MANIFEST`, `just fit <task>` and
  `just fit-check`.
- **R05:** `QualitySetting` (`"high" | "low"`), `ViewSettings`, which gains a `sky: SkySettings`
  field, and `SETTINGS`, which gains its values (T13.f), as R05 requires of later plans.
- **R08:** `BAKE_WAVELENGTHS_NM`, which the colour table samples each row's spectrum at (R08's ask,
  Design note 6).

**Named asks of galaxy plan 06**, entered in its plan by R06.T1 and consumed with an interim until
each lands:

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
   (F = 1.4, colour-corrected: 8.64), beyond which no texel's limit deepens.
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
   Design note 2; Leinert et al. 1998, A&AS 127, 1, for integrated starlight near μ 23.8 at the
   galactic pole): a coarse band pre-pass, `band_rows` from the luminosity tables alone at 16²
   texels a face with no census and a provisional cut of 7.85, gives each texel's background; the
   eye's cut is then the colour-corrected Crumey limit at the darkest texel (μ clamped at 27) plus
   the largest colour offset, +0.45 mag for a hot star, plus a pad of 0.1 mag, so at most about
   9.2. If that cut is deeper than the provisional one the pre-pass runs once more at it; raising
   the cut removes stars from the band only slightly, so one repeat converges. Glare is left out of
   the pre-pass, which is conservative, since glare only makes limits shallower. Near the Sun the
   rule gives about 7.4 + 0.45 + 0.1 at μ 24.3; the fixed 7.85 alone would be too shallow wherever
   the band, with its bright stars removed, is darker than μ 24.3 (7.72 at μ 25, 8.17 at 26).
6. **The colour table** (researched 2026-09-29). Built by `hyperion-fit` from spectra fetched, not
   vendored (they carry no licence; only the integrated table is committed, with citations): ATLAS9
   (Castelli and Kurucz 2003) for 3,500–50,000 K, PHOENIX (Husser et al. 2013) for 2,300–3,500 K and
   log g to 6, TLUSTY OSTAR2002 above 27,500 K, Koester or Levenhagen 2017 DA spectra for white
   dwarfs to 100,000 K and TMAP beyond; a blackbody above 100,000 K. Each spectrum is integrated
   against the CIE 1931 2° functions and the CIE 1924 V(λ) and 1951 V′(λ) (CIE datasets, CC BY-SA
   4.0, credited): chroma in linear Rec. 709 with a D65 white, desaturated towards white out of
   gamut (Walker's method); `lux_per_v0`, the photopic illuminance of a V = 0 star of that spectrum
   over 2.54 µlx; ρ; the camera band term, the default sensor's electrons per V-band photon as a
   magnitude; and each display channel's A_c ÷ A_V at R_V = 3.1 through plan 07's `extinction_ratio`
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
   +20 at 0.05 mag. It is the quadrature `mean_present_mass` does, over the same Gauss–Legendre
   panels in ln m with the same companions (`CompanionMasses`), with the present mass replaced by
   the V light, integrated over the age distribution against each track's own segments: each phase
   of a track at a mass node is sampled at 32 ages (and at its knots), so short bright phases — the
   post-AGB crossing, the blue loops — are weighted by their duration and not missed. The age
   distribution is taken at the emitted time: tables are built at the query's time less 0, 10³, 10⁴,
   10⁵ and 2.62 × 10⁵ years and interpolated linearly in light age. Class 0/I is dark (A3's
   interim), white dwarfs are dark until A4, and brown dwarfs follow plan 06's cooling fits. The
   tables are per galaxy and per time bucket (Design note 13), cached by the server beside the
   galaxy.
8. **Skips, exact** (researched 2026-09-29 for the binary case; the code at `stellar/binary/mod.rs`,
   `rlof.rs`, `common_envelope.rs`; Hurley, Tout and Pols 2002 §2.7; Sana et al. 2012, Science 337,
   444). Plan 03's candidate draws its position, its acceptance mark and its component before its
   mass. The mass word is independent of all three, so `generate_cell_where` draws the mass first
   and drops a candidate below a floor without its position or density; its result is
   `generate_cell` then a mass filter, bit for bit, which a test pins. The floor comes from
   `BrightnessEnvelope::mass_floor`: the least primary initial mass m₁ whose system's brightest M_V
   at any age the cell's components can hold at the cell's emitted interval (at most about 220 years
   wide for a 128 ly cell) could pass the cut at the cell's least distance with no extinction. The
   envelope is built from the tracks at the luminosity function's mass nodes, taking each track's
   extrema over its phase segments, then made a running maximum over mass, and remains a bound
   between nodes up to a margin of 0.3 mag that a slow test with dense masses validates. It is
   indexed by `max_star_mass(m₁)`, the most massive star the system can hold. Today
   `SystemStars::generate` builds every star as a single star at its initial mass, and no companion
   exceeds the primary (`system.rs`'s `generate_with`), so `max_star_mass(m₁) = m₁`. Once plan 11's
   P11.T6–T11 wire binary evolution into the system stage, accretors are rejuvenated and
   main-sequence mergers take the pair's mass at a young apparent age, so a blue straggler can
   outshine every single star of mass m₁ at the cell's age; then `max_star_mass(m₁) = min(2 m₁, 150
M☉)` (mass comes only from the pair, m₁ + m₂ ≤ 2 m₁) and the age range's lower edge is taken to
   zero. The switch is one function, changed in the task that follows P11's wiring (R06.T16.b), so
   `brute_force_sky` stays the oracle. It lowers the floor by up to half and keeps up to about 2.5
   times the candidates above 0.5 M☉ (Kroupa's α = 2.3; the research's estimate, which T17
   measures), and costs little in old cells, whose giant branches already set the envelope. The
   brainstorm's second skip, by each candidate's age word before its density, is not taken: the age
   enters only through the floor's age range over the cell's components, because a per-candidate age
   test would need the component, which needs the density the skip avoids (a departure for the
   roadmap's corrections). A skip never changes an answer: `brute_force_sky` is the oracle.
9. **Caps, derived.** Each layer's radius is the least beyond which its expected number of stars
   brighter than the cut falls below one, from its luminosity function, the density field and the
   least extinction over `CAP_RAYS` (48) rays of plan 07's `horizon` in `Mean` mode, never beyond
   the rule's bound: the brightest M_V the envelope reaches for the layer, dimmed by that least
   extinction. The rule is the ceiling; the caps are what the census uses, and the response states
   both per layer with the expected count beyond, so that the approximation is stated. The
   brainstorm's figures (C about 3,000 ly, D about 4,300, E about 10,000 near the Sun; some 70 ly
   for A and B with protostars dark) are the benchmark's to confirm at version 15 (open question
   19).
10. **The census, per cell.** Cells are those of `cells_in_sphere` to each cap, padded by
    `pad_for(|t_emit − epoch|, pad_speed(layer))` as the range query pads, in canonical order. For
    each record the skip keeps: `retarded` on `Drift::of_record` (a centre member's
    `TraceMotionError` counts it in the tallies until P09.T28); the brief at the emitted time; a
    bound on the system's flux; if that bound passes the cut, `SystemStars::generate` and every
    star's state, V, position (`star_positions_at` about the system's apparent position), colour and
    one `sightline` (`Realised`, `Budget(64)`, the feature modifiers once P09.T40 supplies them)
    from the apparent position to the observer. The flux bound is gated by the primary's phase
    (researched 2026-09-29; Flower 1996, ApJ 469, 355, and Martins and Plez 2006, A&A 457, 637, for
    hot stars' bolometric corrections; De Marco and Schmutz 1999, A&A 345, 163, for γ² Vel, whose O
    companion outshines its Wolf–Rayet primary in V; Siess et al. 2000 and Baraffe et al. 2015 for V
    rising with mass before and on the main sequence). A living primary can be fainter in V than a
    lighter companion: a post-AGB star, a stripped helium star and a TP-AGB star all can. So the
    bound is n × F₁ only while the primary is a protostar, pre-main-sequence or main-sequence star
    (and, once plan 11's binaries are wired, its system cannot have interacted), since its
    companions are then no further evolved and V rises with mass there; otherwise it is F₁ + (n − 1)
    × F_env, with F_env the envelope's flux at `max_star_mass(m₁)` (Design note 8). A dense-mass
    slow test pins that V never falls with mass along the early isochrones. The census counts stars,
    not systems. A star is kept if its V is brighter than the cut plus its eye colour offset where
    the eye is asked. The observer's own system (`exclude`) is left out; its stars are discs.
11. **Merge, N_max and overflow.** Parts are merged by flux, then system ID, then star index, which
    is total, so the order of cells and jobs cannot change the answer. The brightest N_max are
    listed. The rest (the overflow) the server's band takes (Design note 15); the listed stars a
    client culls below a view's limit that client's band layer takes (Design note 20). Each star's
    light is added once, by one side, so nothing is counted twice and nothing lost. N_max defaults
    to 3 × 10⁵ (7.2 MB of payload, some 28 of R03's 262,144-byte chunks, sent one at a time) and is
    capped there; the client asks less on the low setting (Design note 22).
12. **The per-cell cache is monotone.** `SkyCellCache` keeps, per cell, the records at or above the
    mass floor it was built with, in candidate order. A later query whose floor is at or above the
    cached one filters the cached list; a lower floor rebuilds the cell. Records are epoch state, so
    a jump of up to 1,000 ly reuses most cells, and the cache never changes a reply (tested, as plan
    09's caches are). The server's is a `ByteLru` under `HYPERION_SKY_CACHE_MB` (default 64),
    behind a lock so that the census's parallel bulk jobs share it through `&self`, as P09.T40's
    caches do for `SystemSource`. It is not the server's existing cell cache
    (`HYPERION_CELL_CACHE_MB`): that holds whole cells from `generate_cell`, and the near-Sun caps
    enclose some 2.6 × 10⁷ systems in C to E (the brainstorm's count at version 14), where the sky
    keeps only the bright subset above each floor.
13. **Time.** The sky is asked at a time, like every query, within ±H. The response's
    `valid_until` is the least of one Julian year and the time at which the fastest-moving listed
    star within 1 ly would move a tenth of a pixel at 1080p across 60°. The client re-requests past
    it, on a jump, and when a camera's galactic position moves so far that the nearest baked star
    shifts by a tenth of a pixel (Design note 20). Luminosity tables are cached per galaxy and per
    time bucket of 1,000 years, the clock window's scale.
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
    luminosity function's mean colour per M_V bin, which the table carries. The limit map then adds
    the glare (Design note 4) and gives each texel its eye limit. The band depends on the cut, not
    on the per-direction limit, so there is no loop between them: the cut is uniform, and a star
    between a texel's limit and the cut is the client's to cull and add to the band (Design note
    20).
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
    two `u16` fractions (4), its eye colour offset and its camera band term as `i8` centimagnitudes
    (2). A band texel is 12 bytes: luminance `f32`, chroma two `u16`, eye limit `i16`
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
    Defaults: N = 1.4, t = 1/30 s, η = 1.8, σ_pre = 1.2 e⁻, 5 e⁻ at base ISO, f_pk = 0.35, k = 3,
    1,920 px across the 36 mm sensor (18.75 µm pixels). η is electrons per V-band-equivalent
    photon, not a quantum efficiency: an unfiltered silicon sensor collects about 400–950 nm at a
    quantum efficiency near 0.5, so η ≈ 1.5–2 for a solar-type star; it is the same quantity as
    Design note 6's camera band term, which sets it per spectrum. A Bayer green pixel would be η ≈
    0.5–0.7, about 1.2 mag shallower. At 60° the sky gives only about 3.5 e⁻ a pixel, so read
    noise sets the limit, and since the aperture is f ÷ N with f = 18 mm ÷ tan(fov ÷ 2) the signal
    grows as f². The defaults give V 9.4–9.55 at 60° over μ 22.4–24, 11.1–11.2 at 30° and 12.9–13.1
    at 13° at high gain (8.8, 10.5 and 12.3 at base ISO); a 1/2.3″ sensor at 60° reaches only V
    3.8–5.8 (re-checked 2026-09-29, high confidence on the arithmetic). The model gives V 5.5–6.1
    for Global Meteor Network hardware (IMX291, 4 mm f/0.95, Earth's sky at μ 21, 25 fps), against
    Vida et al. 2021's measured +6.0 ± 0.5, and 7.1–7.7 for CAMS (12 mm f/1.2) against Jenniskens
    et al. 2011's +5.4 (Icarus 216, 40), so it is optimistic for old analogue cameras (medium
    confidence). A bright planet in frame takes the limit to about V 2–3 only when about 12 of 21
    stops come from gain, which raises read noise in electrons (σ_post S_base ÷ S) rather than
    cutting photons, and 9 from shutter and aperture (6.8 mag); 21 stops all from photons would give
    about −6.4. The brainstorm's "about V 10" is the 60° figure to half a magnitude; the plan's
    figure is always stated with the field of view.
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
    `writePackedCubeLevelFromBuffer`, with no readback. Babylon creates the cube (`RawCubeTexture`
    with `TEXTURETYPE_UNSIGNED_INT_5_9_9_9_REV`, null data, mips allocated) but cannot generate its
    mips, so the adapter writes every level itself through `_hardwareTexture`, pinned by R01's
    typecheck of `internals.ts` and the harness's cube round trip (R01.T9.g). A TypeScript packer is
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
23. **Labels and honesty.** The view's label block carries one sky line, drafted for the owner as
    guide nomenclature (R06.T15): the limit and its kind, `STARS V 7.4 EYE` or `STARS V 9.5 CAM`,
    and, while any stand-in holds, what it is: `STARS: RANGE QUERY · VOLUME-LIMITED · NO EXTINCTION`
    (R02's stand-in, R02's Design note 16, until this plan lands), `CLUSTERS NOT MODELLED` (until
    R06.T16), `WD NOT MODELLED` (until A4). The unresolved band is labelled as such in the DOM
    list's view notes. The flash threshold binds the stars: pixel-integrated sprites are the
    mechanism, and a test holds a moving star's summed energy within 1%.

## Tasks

T1 comes first. T2, T3 and T4 (tables) and T5 (quadrature) can then run side by side, and T6.a
with them; T6.b needs T5.a, whose mass nodes it reads. T7 needs T5 and T6; T8 needs T2, T3, T4.b
and T7, and within it T8.e follows T8.b. T9 needs T8; T9.d needs T9.b. T10 needs T9, T4.b and R03's
frames; T11 needs T10, with T11.c after T11.a. The client, T12–T14, needs T10 for its types and
R02's `view/`; T13's subtasks follow T12, T13.g follows T13.b, and T15's draft precedes T13.f,
which builds to it. T16.a waits on P09.T40 and T16.b on plan 11's P11.T6–T11. T17 closes.

Rust files are under `crates/hyperion-sim/src/` unless a path says otherwise.

### R06.T1 Enter the kind and the asks

Add `sky` to galaxy plan 04's reserved-kinds table under a new row for R06, with a note that its
size class is large (`is_large`), as that plan's rule requires before a kind is built. Record asks
A1–A4 in galaxy plan 06's "Risks and open points" under a heading "Asked by rendering plan R06",
each with its interim and the task here that switches from it; A4 includes the check of the "Table
III" citation. Both are drafts for the galaxy plans' owner to accept, as the roadmap's "Awaiting the
owner" says. Files: `docs/agent/plans/galaxy-generation/04-server-and-protocol.md`,
`docs/agent/plans/galaxy-generation/06-stellar-stage.md`. Acceptance: `npx prettier --check` on
both; `grep -n "sky" 04-server-and-protocol.md` finds the row; `grep -c "Asked by rendering plan
R06"` finds one heading.

### R06.T2 The naked-eye threshold and glare

Build `sky::eye` (Design notes 2–4), `MAX_CUT_V`, and the units `Lux`, `CandelasPerSquareMetre`,
`MagnitudesPerArcsec2` and `SolarLuminositiesV` where they do not exist, in
`crates/hyperion-base/src/units.rs` (R04 moves `units` there; the sim re-exports it). Every
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
  the task's registration in `tasks/mod.rs`; integration on a common wavelength grid of chroma,
  `lux_per_v0` and ρ (Design note 6). Files: `crates/hyperion-fit/src/tasks/{star_colour,mod}.rs`,
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
  `camera_band_mag` (Design notes 6 and 18: η per spectrum), each channel's `extinction_ratio`, and
  `bake_spectrum` at `BAKE_WAVELENGTHS_NM` normalised to unit photopic illuminance (Design note 6);
  the task's revision is bumped and the table re-fitted. Tests: the 15-bin illuminance of every row
  is 1 lx to 10⁻⁶ and within 1% of the exact integral; η for a 5,772 K spectrum lies in 1.5–2;
  `sky::colour::BAKE_WAVELENGTHS_NM` equals R08's constant (a fixture both sides read). Acceptance:
  `cargo test -p hyperion-fit star_colour`, `cargo test -p hyperion-sim sky::colour`,
  `just fit-check`.

### R06.T4 The limb-darkening table and the host discs

- **R06.T4.a The table.** `hyperion-fit` task `limb_darkening` from J/A+A/664/A128 (`table3.dat`),
  J/A+A/674/A63 and J/A+A/634/A93, committed with their provenance (VizieR data, cited), with its
  manifest and registration; `just fit limb_darkening` writes `tables/limb_darkening.rs`, its
  `tables.lock` and `tables::MANIFEST` entries; `sky::disc::{PowerTwo, limb_coefficients}` with the
  clamps of Design note 16. Tests: the solar row gives c = 0.784 ± 0.005, α = 0.689 ± 0.005 in V
  and a disc average of 0.799 ± 0.005; I(0.1) within 0.015 of Cox 2000's polynomial; every clamp
  returns a finite row. Files: `crates/hyperion-fit/src/tasks/{limb_darkening,mod}.rs`,
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
  in the bulge, the summed V light of every realised system (`SystemStars`) against the density
  times the table, within a Poisson and track-sampling interval. Acceptance: `just test-slow
luminosity_matches_realised_cells` passes.

Files: `sky/{luminosity,photometry}.rs`, `crates/hyperion-sim/benches/sky.rs`.

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
  than 0.1 mag; along the protostar, pre-main-sequence and main-sequence phases V never falls with
  mass at a fixed age (the premise of Design note 10's n × F₁ bound). Acceptance: `cargo test -p
hyperion-sim sky::envelope`, and `just test-slow envelope_bounds_dense_tracks` and `just test-slow
early_v_rises_with_mass` pass.

### R06.T7 Layer caps

`sky::caps::layer_caps` (Design note 9). Tests at Milky Way parameters, generator version 15:
near the Sun with the eye's cut (7.4 + 0.45 + 0.1), C, D and E within a factor of two of 3,000,
4,300 and 10,000 ly and A and B under 100 ly, and the caps shrink in the nuclear disc to under 1,500
ly for E; every cap is at most its rule bound; `expected_beyond` is under 1 by construction. The
measured caps are recorded in the doc comment and in the notes of R06.T17 for open question 19.
Files: `sky/caps.rs`. Acceptance: `cargo test -p hyperion-sim sky::caps`.

### R06.T8 The census

- **R06.T8.a Query and plan.** `SkyQuery`, its builder (observer within the cube and ±H, cut finite
  and at most `MAX_CUT_V`, `n_max` at most 3 × 10⁵, a cone of half-angle in (0°, 90°]),
  `SkyQuery::with_caps_forced` (test builds only), `SkyContext`, `census_plan`. Tests: every
  refusal names its field; the plan's cells are a superset of those whose padded box meets each
  cap's sphere, in canonical order; a cone keeps only cells whose box meets the cone; forced caps
  replace every layer's. Acceptance: `cargo test -p hyperion-sim sky::census::query`.
- **R06.T8.b One cell.** `census_cell` (Design note 10), with the retardation, brief (A1's
  interim), the phase-gated flux bound, companions, positions, colour, extinction and the kept
  test. Tests: the observer's own system is absent; a system whose primary is a white dwarf beside
  a bright companion lists the companion; a system whose primary is post-AGB beside a giant
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

Files: `sky/census/{mod,query,cell,merge,cache}.rs`. Bench: `sky/census_near_sun` (eye cut, cold
and warm cache) and `sky/census_nuclear_disc` (eye cut, 150 ly from Sgr A*). The brainstorm's
figures are the targets to contradict: some 6 × 10⁷ candidates and 5–10 CPU-seconds near the Sun on
first arrival.

### R06.T9 The band and the limit map

- **R06.T9.a The extinction profile.** `galaxy::gas::extinction::profile` (Design note 14), under
  plan 07's rules. Tests: its last node equals `sightline` over the same segment to 10⁻¹²
  relative; it is monotone in distance; at `Mean` it reads no cache. Files:
  `galaxy/gas/extinction.rs`. Acceptance: `cargo test -p hyperion-sim gas::extinction`.
- **R06.T9.b The band.** `sky::band::{CubeFace, BandSpec, BandTexel, band_rows}` (Design note 15).
  Tests: the sum over rows equals one call over the face; an observer above the disc sees a band
  brighter towards the plane than towards the pole by the model's own integral; near the Sun the
  band's surface brightness lies within 0.5 mag of the brainstorm's 22.4 in the plane and 24 at the
  poles (Gaia DR3 flux sums of stars fainter than V 6.5, as the brainstorm cites); lowering the cut
  (a brighter limit) moves light from the listed stars and overflow into the band, and raising it
  moves light back, conserving the total within 1% either way. Acceptance: `cargo test -p
hyperion-sim sky::band`.
- **R06.T9.c The limit map.** `sky::limits::limit_map` with the glare of Design note 4. Tests: near
  the Sun the eye limits run 6.6 ± 0.2 in the band and 7.4 ± 0.2 at the poles; a texel within 1° of
  a V = −1.5 star is at least 0.3 mag shallower than its neighbours' mean; the map is a function of
  the listed stars and the band alone. Acceptance: `cargo test -p hyperion-sim sky::limits`.
- **R06.T9.d The eye's cut.** `sky::limits::eye_cut` (Design note 5): the coarse pre-pass at 16²
  texels a face through `band_rows` with `SkyCensus::empty()`, the darkest texel's limit clamped at
  μ 27, +0.45 and +0.1, and one repeat when the cut deepens. Tests: near the Sun the cut is 7.96 ±
  0.15; no texel of the full limit map, with glare, is deeper than the cut less the 0.45 colour
  offset; the repeat changes the cut by under 0.05 mag. Acceptance: `cargo test -p hyperion-sim
sky::limits`.

Files: `sky/band.rs`, `sky/limits.rs`. Bench: `sky/band_near_sun` (all six faces).

### R06.T10 The protocol

`hyperion_protocol::sky` with the DTOs under Provides (among them `stars_bytes`, `band_bytes`,
`HostDiscDto` with its colour, `teff_k`, `log_g` and `bake_spectrum`, and `MAX_CUT_V`),
`RequestBody::Sky`/`ResponseBody::Sky`, the kind string in `REQUEST_KINDS`, and the payload's byte
layouts (Design note 17) documented beside the types; `just gen-protocol`; the payload's encoder,
`encode_sky_payload`, in `crates/hyperion-server/src/bulk/sky.rs` beside R03's `bulk` (the protocol
crate holds wire types only); `packages/protocol/src/sky.ts` with `decodeSkyStars`, `decodeSkyBand`,
`SKY_STAR_BYTES` and `SKY_TEXEL_BYTES`, and their re-export from `index.ts`. `PROTOCOL_VERSION`
stays at 2: a new kind is additive, and R03's Design note 12 rules that the first binary frames do
not bump it. Files: `crates/hyperion-protocol/src/{sky,lib}.rs`,
`crates/hyperion-server/src/bulk/sky.rs`, `packages/protocol/src/{sky,index}.ts`, generated
bindings.

Tests: the wire forms of request and response; a hand-built star and texel encoded in Rust to
pinned bytes, byte for byte, and the same pinned bytes decoded in TypeScript to the same values
bit for bit, as R03.T10.a and T11 pin the header; `stars_bytes + band_bytes = bulk.bytes`; a
truncated payload is an error naming its length. Acceptance: `cargo test -p hyperion-protocol
sky`, `cargo test -p hyperion-server bulk::sky`, `pnpm --filter @hyperion/protocol test`,
`just ci`.

### R06.T11 The server

- **R06.T11.a Handler, validation and the census.** `requests/sky.rs`: validation (the observer in
  the cube, time within ±H, `n_max` and `camera_limit_v` in range, a known `exclude_system` through
  `resolve`), the eye's cut by `eye_cut` and the request's cut as the deeper of it and
  `camera_limit_v`, the census as `Priority::Bulk` jobs of a few hundred cells each under the
  request's `CancelToken`, merged once all finish; the census never enters the interactive queue,
  so a chart's query is never held behind it. `is_large` gains `Sky`. Tests (integration, over the
  WebSocket, at a small census): a cancelled request stops its queued jobs and sends nothing
  further; a range query sent while a sky's jobs run is answered first; `n_max` above the cap is
  `BadRequest` naming `n_max`. Acceptance: `cargo test -p hyperion-server --test sky`.
- **R06.T11.b Transfer.** The response and its payload through R03's `bulk::chunk` and `Answer`,
  the stars then the band, split by `stars_bytes` and `band_bytes`; the per-cell `ByteLru` of
  Design note 12 under `HYPERION_SKY_CACHE_MB`, behind a lock. Tests: the manifest matches what was
  sent; the cache's config reads its environment variable. Acceptance: `cargo test -p
hyperion-server sky` and `just ci`.
- **R06.T11.c The band, the limits, the discs and the tables.** The band as bulk jobs by face and
  row, then the limit map, then `host_discs` of `exclude_system` at the request's time; the
  luminosity tables and envelope built once per galaxy and time bucket under `SingleFlight` in a
  `ByteLru`. Tests: a sky near the Sun returns the stars, texels and host discs the sim returns for
  the same query; a second identical request shares the tables' build. Acceptance: `cargo test -p
hyperion-server --test sky`.

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
client's `FakeWebSocket` (`lib/connection.test.ts`'s, with R03's `serverSendsBinary`) and R03's
`BulkAssembler`. Tests: the re-request rules; a field factor of 2 lowers every eye limit by 0.387
mag; stale on link loss; the worker's decode equals the main-thread decoder's. Acceptance: `pnpm
--filter hyperion exec vitest run src/renderer/src/view/sky`, `just ci`.

### R06.T13 Drawing the sky

- **R06.T13.a Per-view limits and culling.** `cameraLimitV` over R02's `ExposureTriple` and
  `DEFAULT_VIEW_CAMERA` (Design note 18), the cull and the band hand-off (Design note 20), each
  star's display luminance from its V through R02's `illuminanceLx` and `pixelLuminance` at 2.54
  µlx and the pixel's true solid angle. Tests: 60°, 30° and 13° give 9.5, 11.1 and 13.0 ± 0.3 in a
  dark sky at high gain; 21 stops of exposure, 12 of them gain, lower the limit by at least 5 mag;
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
  `psfPixelWeights` (Design note 20). Tests: a star 0.1 ly away moves nine pixels across 30 au at
  1080p and 60°, as the brainstorm computes; a star crossing the bake/sprite threshold keeps its
  flux; a moving star's summed energy stays within 1% (Design note 23). Files:
  `view/sky/{sprites,select}.ts`. Acceptance: `pnpm --filter hyperion exec vitest run
src/renderer/src/view/sky`.
- **R06.T13.d The band layer.** `BandLayer`: the band map uploaded as a small cube
  (`rgba16float`, 64² faces), bilinearly filtered, drawn first, with the culled stars added. Files:
  `view/sky/band.ts`, `view/sky/shaders/band.wgsl`. Tests: upload layout; the culled-flux sum.
  Acceptance: `pnpm --filter hyperion exec vitest run src/renderer/src/view/sky`,
  `just test-render`.
- **R06.T13.e Host discs.** `HostDiscLayer`: each host star of the camera's system from the
  response's `hosts` and the scene's drawn position, angular radius asin(R ÷ d), the power-2 law per
  channel in the fragment shader on `central_luminance`, the clamp and the glare hand-off to R07
  (Design note 16), a point sprite below three pixels; the disc pass writes `METER_CLASS.hostDisc`
  (0) in the HDR target's alpha (R02 leaves that channel to R07's meter class), and the sprite and
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
run src/renderer/src/view/sky`, `just test-render`.
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
  Acceptance: `just test-render`, `pnpm --filter hyperion exec vitest run
src/renderer/src/view/sky`.

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
By hand, recorded: the cockpit and two instrument canvases (R01's proof) draw one sky. Acceptance:
`pnpm test`, `just ci`.

### R06.T15 Guide nomenclature for the owner

Draft, in one commit for the owner to read, the sky's additions to `docs/frontend/ux-guidelines.md`
on top of R02's nine items: `STARS`, `EYE`, `CAM` and the stand-in phrases of Design note 23 in the
nomenclature list; the rule that the limit shown with the sky is a magnitude with its kind; and a
sentence under the view class that the unresolved band is labelled. It is drafted before T13.f. It
ends when **the owner signs off**; until then the client is built to the draft, as the galaxy
plans' guide drafts are. Acceptance: `pnpm format:check`, `grep` finds `STARS V` in the guide
draft, and the owner's sign-off recorded in this plan.

### R06.T16 Feature members and binaries

- **R06.T16.a Feature members (after P09.T40).** P09.T40 registers plan 09's sources in the
  `SystemsInRange` handler; the sky handler builds the same sources over P09.T40's caches, and the
  census reads them: `FeatureMemberSource` and the centre's members through the `SystemSource`
  hook, with the same skips (a member's mass word is its own, so the floor applies), and
  `FeatureGas` as the sightline's modifiers. The label's `CLUSTERS NOT MODELLED` is withdrawn.
  Tests: the Pleiades-like cluster of a pinned seed appears as a clump of bright stars from 400 ly;
  the globular core's sky from its centre lists stars to V 6.5 within a factor of two of the
  brainstorm's about 4 × 10⁵ (its 47 Tuc row). Acceptance: `cargo test -p hyperion-sim
sky::census::features` and `just ci`.
- **R06.T16.b Binaries (after P11.T6–T11).** When plan 11 wires binary evolution into
  `SystemStars`, `max_star_mass` returns min(2 m₁, 150 M☉), the envelope's age range starts at
  zero, and the n × F₁ bound requires a system that cannot have interacted (Design notes 8 and 10).
  Tests: a pinned blue straggler of an old cell (a merger or an accretor) is listed and equals
  `brute_force_sky`; the identity tests of T8.e pass unchanged. Acceptance: `cargo test -p
hyperion-sim --test sky_census` and `just ci`.

### R06.T17 Verification pass

Run every slow test and bench above and record the figures in the doc comments that own them and in
this plan: the caps, candidates opened, CPU-seconds and listed stars near the Sun and in the inner
bulge, re-deriving open question 19's counts at version 15 and explaining why candidates exceed the
systems layers C to E hold; the candidates the binary rule of T16.b costs; check the per-layer
counts against `range_500ly_floor_d` (37,675 systems at version 15). Add goldens:
`crates/hyperion-sim/tests/golden/sky/census_near_sun.golden`, the census of a pinned observer near
the Sun to V 7 (IDs, star indices, V to 10⁻⁶ mag), and `sky/band_face_row.golden`, one band face
row, read by the testkit's golden harness. Decide N_max and the sprite budget per setting from the
measurements (open question 16) and record them. Each timing is taken on a quiet machine, as the
roadmap's conventions require, or marked provisional. Acceptance: `just ci`, `just test-slow`, `just
bench` complete.

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
  (T5, T9); the brainstorm's sky table rows re-derived at version 15 (T17).
- **Conservation:** light moves between points, overflow and band without loss (T9, T13).
- **Order independence** of the census over jobs and cells (T8.c) and of the band over rows (T9.b).
- **Benches:** `sky/luminosity_tables`, `sky/census_near_sun`, `sky/census_nuclear_disc`,
  `sky/band_near_sun`, `sky_near_sun_cold` (server).
- **By hand, recorded:** the star field's GPU time on both machines, no flicker, the band's lanes,
  the discs, several views sharing one cube.

## Generator version

No change to generated output and no bump. `generate_cell_where` is `generate_cell` filtered, and
the goldens prove it; the luminosity tables, the envelope, the caps and the census only read. The
sky's own output is a function of the generator version and of the committed colour and
limb-darkening tables, so its goldens (T17) are regenerated whenever either moves. Not adopted:
drawing a cell's mass words in sorted order, which the brainstorm offers as a generator-version
change to skip light candidates without opening their streams; the mass-first walk already skips
their position and density, and the benchmark decides whether the rest is worth a bump (Risks). The
plan reserves no tag, prefix or stream.

## Risks and open points

- **The census's cost** rests on the brainstorm's estimates (open question 19): 5–10 CPU-seconds
  near the Sun and 400–800 in the inner bulge. At bulk priority it cannot starve the charts, but in
  single-player it shares the machine with a descent. If the inner bulge is too slow, the fallbacks
  are, in order: the sorted mass words above (a bump); a coarser cap rule for layers whose
  candidates dominate; caching the census by observer cell across sessions in memory.
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
  old analogue cameras by about 2 mag (CAMS, Design note 18), and the split of a large exposure
  change between gain and photons is R07's metering, which sets how far a bright planet takes the
  limit (V 2–3 under Design note 18's split).
- **Binaries.** The flux bound and the envelope are exact for single-star systems today; once plan
  11 wires binary evolution in, R06.T16.b must land with it or the census can miss blue
  stragglers and mergers, and its cost (up to about 2.5 times the candidates above 0.5 M☉, the
  research's estimate) is T17's to measure.
- **The eye's cut** rests on Crumey's eq. 34 at the darkest pre-pass texel, clamped at μ 27 where
  Blackwell's data give no constraint; a view darker than μ 27 is drawn to the clamp's limit.
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
- **Babylon internals.** `_hardwareTexture` joins `_device` and `_disableEngineYFlip` among the
  pinned names; a Babylon change fails R01's typecheck of `internals.ts` or the cube round trip
  (R01.T9.g), and the fallback is the adapter creating the cube with the raw device.
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
