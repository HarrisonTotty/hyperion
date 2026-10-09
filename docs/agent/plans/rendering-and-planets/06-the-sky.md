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
  the census, the band map (with the diffuse galactic light) and the limit map, and the host discs.
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
    bake_spectrum: [f64; BAKE_WAVELENGTH_COUNT] (R08's ask; Design note 6),
    the colour's place in its grid, where the five reddening tables are read (R06.T9.e) */ }
impl StarColour { pub fn reddened(&self, a_v: Magnitudes) -> Reddened;     // R06.T9.e
    pub fn reddening(&self) -> Reddening; }
pub struct Reddening { /* the six parts at unit luminance, nine bands' secants at A_V → 0 and
    REDDENING_A_V_NODES */ }   impl Reddening { pub fn through(&self, a_v: Magnitudes) -> Reddened; }
pub struct Reddened { /* transmission: [f64; 3], photopic_transmission: f64,
    scotopic_transmission: f64, red_green: [f64; 2], sp_ratio: f64, camera_band_mag: f64,
    v_extinction: Magnitudes */ }
pub const REDDENING_A_V_NODES: [f64; 7];   // 2, 5, 7.5, 10, 15, 20, 30 (R06.T9.e, the band ruling's addendum)
pub fn lift_into_gamut(rgb: [f64; 3]) -> [f64; 3];   // T3's rule, the fit's `unit_rgb` bit for bit
pub fn solar_colour() -> StarColour;   // SUN_TEFF_K, SUN_LOG_G: the band's reddening (R06.T9.e)
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
pub struct LayerCap { /* layer, radius: LightYears (the farthest ray's since R06.T7.b),
    rule_bound: LightYears, expected_beyond: f64, rays: Option<RayRadii> */ }  // Clone, not Copy
impl LayerCap { pub fn radius_toward(&self, direction: UnitVector) -> LightYears;
    pub fn rays(&self) -> Option<&RayRadii>; }                      // R06.T7.b, as built
pub struct CapLattice;      // the rays: a Fibonacci lattice, its spacing (covering radius), index
pub struct RayRadii;        // one layer's radius a ray; `toward(direction)`: the largest radius
                            // of the rays whose cones, of half-angle the spacing, hold it
pub struct CapCount;        // the per-ray count; `caps()`, `spheres()`, `stars_beyond(cap)`,
                            // `systems_within(cap)`, `stars_within_and_beyond(a, b)`
pub fn layer_caps(galaxy: &Galaxy, tables: &LuminosityTables, envelope: &BrightnessEnvelope,
    observer: &Observer, cut: Magnitudes, cache: &mut NoiseCache) -> Vec<LayerCap>;
pub fn layer_caps_by_visibility(galaxy: &Galaxy, tables: &LuminosityTables,
    envelope: &BrightnessEnvelope, observer: &Observer, visibility: &EyeVisibility,
    cache: &mut NoiseCache) -> Vec<LayerCap>;                       // R06.T7.b, as built
pub const CAP_RAYS: usize;                       // 48; 768 in R06.T7; 1,536 since R06.T7.b
pub const SUB_RAYS: usize;                       // 3 since R06.T7.b: each ray the clearest of them
impl RayExtinctions { pub fn measure_rays(galaxy: &Galaxy, origin: &GalacticPosition,
    rays: usize, which: Range<usize>, cache: &mut NoiseCache) -> Self;  // one share of the rays
    pub fn measure_clearest_rays(galaxy: &Galaxy, origin: &GalacticPosition,
        lattice: Arc<CapLattice>, sub_rays: usize, which: Range<usize>,
        cache: &mut NoiseCache) -> Self;              // R06.T7.b: the caps' rays, one share
    pub fn join(shares: impl IntoIterator<Item = Self>) -> Self; }     // R06.T11.c, as built
pub fn layer_caps_over(galaxy: &Galaxy, tables: &LuminosityTables, envelope: &BrightnessEnvelope,
    observer: &Observer, cut: Magnitudes, rays: &RayExtinctions) -> Vec<LayerCap>; // the server's
pub fn layer_caps_by_visibility_over(galaxy: &Galaxy, tables: &LuminosityTables,
    envelope: &BrightnessEnvelope, observer: &Observer, visibility: &EyeVisibility,
    rays: &RayExtinctions) -> Vec<LayerCap>;                        // R06.T7.b, as built

// sky::census (Design notes 10–13)
pub struct SkyQuery { /* observer: Observer, cut: Magnitudes, eye: Option<EyeObserver>,
    eye_cut: Option<Magnitudes> (the eye's own, at most the cut; R06.T9.j, as built),
    n_max: NonZeroU32, cone: Option<Cone>, exclude: Option<SystemId> */ }
pub struct SkyQueryBuilder;                                        // SkyQuery::builder(..)
impl SkyQuery { pub fn eye_cut(&self) -> Option<Magnitudes>; }    // R06.T9.j: the cut when unstated
impl SkyQueryBuilder { pub fn eye_cut(self, eye_cut: Magnitudes) -> Self; } // a camera's deeper cut beside it
impl SkyQuery { #[cfg(any(test, feature = "testing"))]
    pub fn with_caps_forced(self, radius: LightYears) -> Self; }   // brute_force_sky's census
pub struct Cone { /* axis: UnitVector, half_angle: Degrees */ }
pub struct SkyStar { /* system: SystemId, star: StarIndex, apparent: GalacticPosition,
    distance: LightYears, emitted: UniverseTime, v: Magnitudes, a_v: Magnitudes,
    colour: StarColour */ }
pub trait SkyCellCache: Sync {                  // &self: interior mutability, dyn-safe
    fn keeps_entries(&self) -> bool;
    fn block(&self, galaxy: &Galaxy, key: BlockKey) -> Option<Arc<SkyBlock>>;
    fn keep(&self, galaxy: &Galaxy, cell: CellKey, params: &BlockParams,
        records: &[HeldRecord]);
    fn note(&self, cell: CellKey, outcome: CellOutcome) {} }
                                          // R06.T8.h, as built (it was `bright_subset`)
pub struct NoSkyCellCache;
pub struct SkyBlock;   // R06.T8.h: 4³ cells of one layer, built for one BlockParams
pub struct BlockKey;  pub struct BlockParams;  pub struct HeldRecord; // record + light, 96 B
pub enum CellOutcome { Served, Missed, Rebuilt(Rebuild) }
pub enum Rebuild { Key, Window, Parameters }
pub struct SkyContext<'a> { /* tables: &'a LuminosityTables, envelope: &'a BrightnessEnvelope,
    noise: NoiseCache (the job's own), cells: &'a dyn SkyCellCache,
    sources: &'a [&'a dyn SystemSource], modifiers: &'a dyn GasModifierSource */ }
pub struct CensusPlan { /* caps: Vec<LayerCap>, cells: Vec<CellKey> (canonical order) */ }
pub fn census_plan(galaxy: &Galaxy, tables: &LuminosityTables, envelope: &BrightnessEnvelope,
    query: &SkyQuery, cache: &mut NoiseCache) -> CensusPlan;
pub fn census_plan_of(query: &SkyQuery, caps: Vec<LayerCap>) -> CensusPlan; // R06.T11.c, as built
impl SkyQuery { pub fn forced_caps(&self) -> Option<&[LayerCap]>; }        // R06.T11.c, as built
impl SkyQueryBuilder { pub fn eye_visibility(self, visibility: EyeVisibility) -> Self; }
                            // R06.T7.b: an eye-only request's caps by the eye's visibility
pub fn census_cell(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, key: CellKey, query: &SkyQuery,
    out: &mut Vec<SkyStar>);
pub fn census_cell_with_cost(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, key: CellKey,
    query: &SkyQuery, out: &mut Vec<SkyStar>) -> (CensusTallies, CensusCost); // R06.T8.h, as built
pub struct CensusCost; pub struct LayerCost;  // R06.T8.h: the path's counts, no part of a reply
pub struct SkyCensus { /* listed: Vec<SkyStar> (by flux, then system, then star),
    overflow: Vec<SkyStar>, tallies: CensusTallies */ }
impl SkyCensus { pub fn empty() -> Self; }                    // the eye-cut pre-pass's band
pub fn merge_census(parts: Vec<Vec<SkyStar>>, n_max: NonZeroU32) -> SkyCensus;
pub const SHELL_EDGES_LY: [u32; 9]; pub const SHELLED_LAYERS: [Layer; 3]; pub struct Shell;
pub struct Completeness; impl CensusPlan { pub fn shells(&self); pub fn shell_slabs(&self,
    shell: Shell); pub fn completeness(&self, done: impl IntoIterator<Item = Shell>)
    -> Completeness; pub fn complete(&self) -> Completeness; pub fn replies(&self)
    -> Vec<CompleteTo>; } impl CellSlab { pub fn shell(&self) -> Shell; }
pub fn merge_shells(parts, n_max: NonZeroU32, completeness: Completeness) -> SkyCensus;
impl SkyCensus { pub fn completeness(&self) -> Option<&Completeness>; }  // R06.T8.i, as built
#[doc(hidden)] pub fn census_plan_with_edges(query: &SkyQuery, caps: Vec<LayerCap>,
    edges_ly: &[u32]) -> CensusPlan;      // R06.T11.d, as built: a test's nearer shell edges
pub struct CensusTallies { /* per layer: cells, candidates opened, accepted, listed,
    without_photometry, feature_members_absent: bool */ }

// sky::band (Design notes 14–15)
pub enum CubeFace { PosX, NegX, PosY, NegY, PosZ, NegZ }   // galactic axes, WebGPU face order
pub struct BandSpec { /* face_texels: u16 (64), steps per ray */ }
pub struct BandTexel { /* luminance: CandelasPerSquareMetre, chroma: [f32; 2],
    sp_ratio: f64, eye_limit: Option<Magnitudes> */ }
pub struct CompleteTo { /* each layer's radius, ly: 0 nowhere, +∞ everywhere; per ray since
    R06.T7.b, `radius_toward(layer, direction)` */ }  // R06.T9.b; Clone, not Copy, since R06.T7.b
impl CompleteTo { pub fn rays_ly(&self, layer: Layer) -> Option<&[f64]>; } // R06.T11.d: the wire's
pub fn band_rows(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, query: &SkyQuery,
    census: &SkyCensus, complete_to: &CompleteTo, spec: &BandSpec, face: CubeFace,
    rows: Range<u16>, out: &mut Vec<BandTexel>);   // march_rows of its one reply, then sum_rows
pub struct BandMarch { /* spec, face, rows, the observer's position, each layer's kept radii,
    each ray's five sums per layer and radius, and under a camera's deeper cut the same of the
    light fainter than the eye's cut (R06.T9.j) */ }               // R06.T9.f, as built
impl BandMarch { pub fn holds(&self, complete_to: &CompleteTo) -> bool;
    pub fn eye_cut(&self) -> Option<Magnitudes>;   // R06.T9.j: Some where it keeps the eye's light
    pub fn heap_bytes(&self) -> usize; }
pub fn march_rows(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, query: &SkyQuery,
    replies: impl IntoIterator<Item = CompleteTo>, spec: &BandSpec, face: CubeFace,
    rows: Range<u16>) -> BandMarch;              // every reply's radii a node of each ray
pub fn sum_rows(march: &BandMarch, census: &SkyCensus, complete_to: &CompleteTo,
    out: &mut Vec<BandTexel>);                   // one reply's texels: no profile, no table

// sky::limits (Design notes 4 and 5)
pub fn eye_cut(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, observer: &Observer,
    eye: &EyeObserver, illumination: Option<&Illumination>)
    -> Magnitudes;                                 // coarse pre-pass, darkest texel, +0.453 +0.1
pub struct EyeVisibility;   // R06.T7.b: the pre-pass's 16² limits +0.453 +0.1, of one observer and eye
pub fn eye_visibility(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, observer: &Observer,
    eye: &EyeObserver, cut: Magnitudes, illumination: Option<&Illumination>)
    -> EyeVisibility;                              // R06.T7.b, as built
pub struct Glare { /* each listed star's direction, photopic and scotopic illuminance at the eye
    after its own reddening, and their pyramid over the band's texels */ } // R06.T9.c and T9.i, as built
impl Glare { pub fn of_listed(observer: &Observer, listed: &[SkyStar],
    spec: &BandSpec, eye_cut: Magnitudes) -> Self;  // R06.T9.i and T9.j, as built: for its band's
                                                    // texels; only the stars at or brighter than the eye's cut glare
    pub fn of_points(points: impl IntoIterator<Item = (UnitVector, Lux, SpRatio)>,
        spec: &BandSpec) -> Self; }                 // R06.T9.i: point sources, a synthetic sky's
pub fn limit_rows(eye: &EyeObserver, spec: &BandSpec, glare: &Glare, face: CubeFace,
    rows: Range<u16>, texels: &mut [BandTexel]);                  // glare, then V_lim per texel
pub fn limit_map(eye: &EyeObserver, spec: &BandSpec, glare: &Glare,
    band: &mut [BandTexel]);                                      // limit_rows over six faces
pub fn eye_offsets(eye: &EyeObserver, spec: &BandSpec, glare: &Glare,
    band: &[BandTexel]) -> Vec<Magnitudes>;         // R06.T9.h: each listed star's own limit less its texel's

// sky::dgl — the diffuse galactic light (R06.T9.g; decision-r06-t9g-dgl.md)
pub const ILLUMINATION_SPEC: BandSpec;      // 16² on the standard nodes: the eye cut's pre-pass's
pub struct Illumination { /* the observer's own sky of all starlight at ILLUMINATION_SPEC: each
    texel's five sums and its ray's A_V to the edge, and the scattered field's fixed point */ }
impl Illumination { pub fn march(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, observer: &Observer)
        -> Self;                            // or by rows for the server's jobs, then assembled
    pub fn observer(&self) -> &Observer; pub fn heap_bytes(&self) -> usize; }
impl Illumination { pub fn march_rows(galaxy: &Galaxy, ctx: &mut SkyContext<'_>,
        observer: &Observer, face: CubeFace, rows: Range<u16>) -> IlluminationRows;
    pub fn assemble(observer: &Observer, parts: impl IntoIterator<Item = IlluminationRows>)
        -> Self;                            // any order and split; R06.T9.g, as built
    pub fn iterations(&self) -> u32; }      // the scattered field's steps
pub struct IlluminationRows;                // one job's rows: face, rows, sums and A_∞
// BuildSkyQueryError::Illumination: one marched for another observer or time (as built)
impl SkyQueryBuilder { pub fn illumination(self, illumination: Arc<Illumination>) -> Self; }
impl SkyQuery { pub fn illumination(&self) -> Option<&Illumination>; }
impl BandTexel { pub fn diffuse_luminance(&self) -> CandelasPerSquareMetre; }
pub fn eye_cut(galaxy: &Galaxy, ctx: &mut SkyContext<'_>, observer: &Observer,
    eye: &EyeObserver, illumination: Option<&Illumination>) -> Magnitudes;

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
- Since R06.T11.d, as built: `ServerMessage::PartialResponse { id, body }` (`partial_response`),
  each answer of a `sky` before its last, its own chunks before it numbered from 0, and the
  terminal `response` the last; `SkyResponse.final` (`is_final` in Rust); `SkyLayerCensusDto`'s
  `complete_to_ly`, `complete_to_rays_ly` (1,536 a layer where it is complete to one radius a ray,
  empty otherwise) and `final`. In `@hyperion/protocol`, `RequestClient.requestBulk` takes a third
  argument, `{ onPartial }`, which is handed each partial answer whole (`PartialBulkAnswer`,
  `BulkRequestOptions`).

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
   texel arrives with its chroma and its photopic flux already computed, and carries its eye offset
   (its own eye limit less its texel's) and its camera band term as fields that each view applies
   (Design note 17). One table in Rust, no TypeScript copy: what a client plan needs of a host
   star's colour (R07, R08) travels on `HostDiscDto`.
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
   90° of the texel's centre, with E rod-weighted by ρ★ ÷ 1.408 as the background is. E is the
   illuminance in the plane of the eye: E cos θ for a star of illuminance E at θ from the line of
   sight, and none from behind the eye's plane (CIE 146:2002; IJspeert et al. 1990, Vision Res. 30,
   699; Stiles and Crawford 1937, Proc. R. Soc. B 122, 255). A star's own veil is not its own
   background: each listed star's eye offset is its own limit less its texel's (Design note 17;
   R06.T9.h). The far field is summed over a pyramid of the band's texels, within 0.001 mag of the
   exact sum (R06.T9.i). From R06.T9.j the eye's map is the eye-only request's at the same census
   radii, whatever the request's cut: its background is the expected light fainter than the eye's
   cut, and only the listed stars brighter than it glare (as built, at or brighter than it: the
   census's boundary). Where a camera's caps reach farther, its census lists the real stars brighter
   than the eye's cut beyond the eye-only caps, under one expected a layer, which glare and may be
   seen where an eye-only request holds their expected light (decided 2026-10-07,
   `decision-r06-t9c-glare.md`, addendum 2). Decided 2026-10-06, `decision-r06-t9c-glare.md`. It is
   added to the band's luminance before the threshold. Defaults A = 25, p = 0.5 are `EyeObserver`
   fields. F stays 1.4: the glare is then modelled rather than folded into F, a small double count
   Risks records. The glare of the camera's own star and sunlit bodies is not in the map; the
   brainstorm names only the resolved stars.
5. **Two kinds of limit, one request.** A view is either the eye (the single-player cockpit window)
   or a camera (the main screen and every other view). The eye's limit per direction is the limit
   map's; a camera's is `cameraLimitV` (Design note 18). The response's `cut_v` is the deepest of
   the two over the views open. The camera's part is the request's `camera_limit_v`, clamped at
   `MAX_CUT_V`, 11.0, above which a narrow zoom would ask for 10⁶ stars (research finding); a
   deeper individual exposure asks for a cone. A cone is an instrument's field stop: its census
   and band share the band's texels that meet it, and no star outside them is listed or glares,
   so the naked eye, which has none, cannot ask one (decided 2026-10-07,
   `decision-r06-t8k-cone.md`; R06.T8.l). The eye's part cannot be the limit map's deepest
   texel, since the map is computed from the band and the listed stars, which are computed to the
   cut. It is set by the server before the census (researched 2026-09-29; Crumey 2014 eq. 34 as in
   Design note 2; Gaia DR3 flux sums for the integrated starlight: μ 24.3 at the galactic poles
   for the stars fainter than V 6.5, and 24.6 for those fainter than V 8.1
   (`decision-r06-t9b-band.md`)): a coarse band pre-pass, `band_rows` from the luminosity tables
   alone at 16² texels a face with no census and a provisional cut of 7.85, gives each texel's
   background; the eye's cut is then the colour-corrected Crumey limit at the darkest texel
   (clamped as Design note 2 says; 7.99 at most since 2026-10-02) plus the largest colour offset,
   the colour table's own largest, at most 0.46 (it is 0.453, at μ 30, for its 500,000 K
   blackbody rows; first given here as 0.43, and the cut as +0.45; the orchestrator's ruling of
   2026-10-06 on R06.T9.h's open question 1), plus a pad of 0.1 mag, so at most about 9.2 (8.54
   under the 2026-10-02 clamp). If that cut is deeper than the provisional one the pre-pass runs
   once more at it; raising the cut removes stars from the band only slightly, so one repeat
   converges; the diffuse galactic light, which no cut changes, makes the step smaller still (about
   0.01 near the Sun; R06.T9.g). Glare is left out of the pre-pass, which is conservative, since
   glare only makes limits shallower. Near the Sun the rule gives about 7.55 + 0.453 + 0.1 ≈ 8.10
   for the real sky. The pre-pass's darkest texel holds the light fainter than the cut and the
   diffuse galactic light, μ about 24.60–24.65: Gaia DR3's 24.73–24.77 at b ≈ +79°, and the light
   its dust scatters (R06.T9.g). That is darker than the 24.3 of the light fainter than V 6.5. The
   fixture gives about 8.0. Its poles are about 0.3 mag faint in starlight, and its dust gives them
   about four times the real diffuse light (decided 2026-10-06, `decision-r06-t9b-band.md`;
   re-derived 2026-10-07, `decision-r06-t9g-dgl.md`). The fixed 7.85 alone would be too shallow
   wherever the band is
   darker than μ 24.3 (7.72 at μ 25, and 7.99, Crumey's clamp, from μ 25.6; the 8.17 at 26 first
   given here is eq. 34 before the 2026-10-02 clamp).
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
   at the channel's effective wavelength for that spectrum. Five companion tables,
   `star_colour_reddening` and its node files `_av02_05`, `_av07p5`, `_av10_15` and `_av20_30`, row
   for row the same, add 78 columns: for the six positive and negative parts of the Rec. 709
   colour-matching functions, V, the scotopic and the camera, each band's moment at A_V → 0 and
   its secant at `REDDENING_A_V_NODES`, with each part's value per unit luminance, which
   `StarColour::reddened` reads (R06.T9.e; Risks, "Deviations in T9.e, as built"). Every A_V in the colour tables,
   `StarColour::reddened` and the census is plan 07's sightline A_V, the law's normalisation at
   0.549 µm times the dust column, and each reddening column is a band's broadband extinction over
   it for the row's own spectrum; a star's V, which the census cuts and the wire carries, is M_V +
   DM plus its own V band's extinction (about 1.003 A_V for the Sun's row, 0.976 A_V at A_V 10),
   and the band subtracts at the solar row's (decided 2026-10-06, `decision-r06-t9b-band.md`,
   addendum item 4). For R08's spectral bakes (its Design
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
   each of `CAP_RAYS` (768; 1,536 since R06.T7.b, each through the clearest of three sub-rays)
   rays dimming the stars of its own solid angle by its own extinction
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
   `decision-r06-census-cost-signoff.md`); each ray counts the stars brighter than
   cut − DM − v☉(A_V) A_V, the band's boundary (decided 2026-10-07, `decision-r06-t8k-cone.md`);
   and an eye-only request may count each ray to the eye's own limit about it, the eye-cut
   pre-pass's, rather than the uniform cut (R06.T7.b's visibility-based caps). From R13.T2, RM3's
   interim, layers C to E take their caps by ray at the synthetic ceiling V_P rather than at the
   cut, never beyond the cut's own: V 5.0 in RM3, 4.5 from R13.T7. Beyond them the band holds all
   of the light until R13's synthetic stars fill [V_P, cut) (decided by the owner 2026-10-08;
   R13 Design notes 2–3, `feasibility-hybrid-sky.md`).
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
    decision item 2's trigger). A pair's verdict bounds only those of its stars that depart from
    their own single-star models, and its products. Every other star takes its own bound (decided
    2026-10-07, `decision-p11-t16-hierarchy-bound.md`). From R06.T8.f the bound is first tested
    before the drift, at the epoch position's distance less the cell's pad and offset and over the
    ages the light's travel then allows, and a star already past the cut with no extinction takes no
    sightline. _Corrected
    2026-10-03:_ the plan's premise that V rises with mass along the early phases at a fixed age,
    which an n × F₁ bound for young multiples needed, is false in V (R06.T6.b measured falls of up
    to at least 0.38 mag on the pre-main sequence, e.g. 5–6 M☉ at 0.52 Myr and \[Fe/H\] −2, the
    hotter star's larger bolometric correction); nothing depends on it since item 2. The census counts stars,
    not systems. A star is kept if its V is brighter than the cut, with or without the eye, and,
    for a query with a cone, if its band texel meets the cone: its texel's centre lies within the
    half-angle plus the band's largest texel radius, the region in which the band is complete
    (decided 2026-10-07, `decision-r06-t8k-cone.md`; R06.T8.l). The eye's colour offset is the
    views' to apply (Design note 20, T13.a). The eye's cut carries the largest offset already
    (Design note 5), and the band subtracts at the cut, so the listing and the band share one
    boundary (decided 2026-10-06, `decision-r06-t9b-band.md`; R06.T8.k). The observer's own
    system (`exclude`) is left out; its stars are discs.
11. **Merge, N_max and overflow.** Parts are merged by flux, then system ID, then star index, which
    is total, so the order of cells and jobs cannot change the answer. The brightest N_max are
    listed. The rest (the overflow) the server's band takes (Design note 15); the listed stars a
    client culls below a view's limit that client's band layer takes (Design note 20). Each star's
    light is added once, by one side, so nothing is counted twice and nothing lost. N_max defaults
    to 3 × 10⁵ (7.2 MB of payload, some 28 of R03's 262,144-byte chunks, sent one at a time) and is
    capped there; the client asks less on the low setting (Design note 22). From R06.T8.i and
    T11.d a reply may be partial (decided 2026-10-05, `decision-r06-census-cost.md`; adopted under
    the owner's delegation, `decision-r06-census-cost-signoff.md`). It states each layer's
    complete-to radius, and the band carries the rest. Until a layer's last shell is merged, a
    reply lists only its stars within that radius towards their band texel, read at the centre of
    the texel `BandSpec::texel_of` places each star in at the query's `band_spec`, the radius the
    band's ray through that texel reads; so listing and band share one boundary and each star's
    light is still added once. Once a layer's last shell is merged, every reply lists every star of
    its opened cells, as the one-shot census does (decided 2026-10-08,
    `decision-r06-t8i-listing.md`).
12. **The per-cell cache is monotone.** `SkyCellCache` keeps, per cell, the records at or above the
    mass floor it was built with, in candidate order. A later query whose floor is at or above the
    cached one filters the cached list; a lower floor rebuilds the cell. Records are epoch state, so
    a jump of up to 1,000 ly reuses most cells, and the cache never changes a reply (tested, as plan
    09's caches are). The server's is a `ByteLru` under `HYPERION_SKY_CACHE_MB` (default 64),
    behind a lock so that the census's parallel bulk jobs share it through `&self`, as P09.T40's
    caches do for `SystemSource`. It is not the server's existing cell cache
    (`HYPERION_CELL_CACHE_MB`): that holds whole cells from `generate_cell`, and the near-Sun caps
    enclose some 2.6 × 10⁷ systems in C to E (the brainstorm's count at version 14), where the sky
    keeps only the bright subset above each floor. From R06.T8.h the cache holds blocks of 4³
    cells, each built for one observer and cut (decided 2026-10-05, `decision-r06-census-cost.md`;
    amended 2026-10-08, `decision-r06-t8h-warm.md`).

    - A cell's key is the faintest absolute V listable at its least distance from any observer
      within 1,000 ly of the builder's, and its window is every emitted time that such an observer,
      at any time in ±H, can receive. A mass floor would not do: near the Sun the floor is every
      cell's band edge.
    - An entry holds the records whose star-by-star bound over its window passes at its key, and
      serves a query whose key is no fainter and whose window it holds.
    - The stored bound only skips records. Every record it passes takes the census's own steps, so
      a warm census generates the same systems as a cold one, and its reply is the same bit for bit.
    - R06.T8.n sets `HYPERION_SKY_CACHE_MB`'s default so that one near-Sun sky and a jump's new
      cells fit (`decision-r06-census-cost-signoff.md`), at most 2,048 MiB.

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
    luminosity function's light fainter than M_V = cut − DM(d) − v☉(A_V) A_V(d) at the emitted
    time (the solar row's V secant, since the census cuts each star's own V; R06.T8.k, decided
    2026-10-06, `decision-r06-t9b-band.md`, addendum item 4; first written − A_V(d)), all of
    a layer's light beyond its cap, and then the overflow (the kept stars past N_max) splatted into
    their texels as points. The listed stars are never in the server's band: a client that culls one
    adds it to its own band layer (Design note 20), so each star's light is counted once. The census
    skips faint stars by mass without summing them, so the subtraction is exact in expectation, as
    the brainstorm says. The texel's luminance, chroma and ρ are the flux-weighted sums, using the
    luminosity function's mean colour per M_V bin, which the table carries, reddened by the dust in
    front of each node: each display channel, the photopic light and the scotopic light by its own
    A ÷ A_V, the colour table's solar row's for the band and each star's own for the overflow,
    through `StarColour::reddened` (decided 2026-10-06, `decision-r06-t9b-band.md`; R06.T9.e).

    To each texel's light the band adds the diffuse galactic light (R06.T9.g;
    `decision-r06-t9g-dgl.md`):
    - the observer's own sky of all starlight, scattered once by the ray's dust (Henyey and
      Greenstein's phase function, Draine's 2003 albedo and g per sum), with its higher orders where
      the dust is thick;
    - reddened by the solar row's curves through the ray's A_V.

    It is the same for every cut, reply, census and cone, and is no star's light, so the listing and
    the band still share one boundary.

    The limit map then adds the glare (Design note 4) and gives each texel its eye limit and each
    listed star its eye offset, its own limit less its texel's (R06.T9.h). Its background is the
    light fainter than the eye's cut, even when a camera's deeper cut sets the band's (R06.T9.j).
    The band depends on the cut, not on the per-direction limit, so there is no loop between
    them: the cut is uniform, and a star between a texel's limit and the cut is the client's to cull
    and add to the band (Design note 20).

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
    two `u16` fractions (4), its eye offset as `i8` centimagnitudes (its own eye limit less its
    texel's: its colour offset against its texel's background and its self-exclusion, R06.T9.h;
    decided 2026-10-06, `decision-r06-t9c-glare.md`) and its camera band term as `i8` in units of
    1/32 mag, rounded half away from zero and saturating at −4.0 and +3.97 (2).
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
and T7, and within it T8.e follows T8.b. T9 needs T8; T9.c needs T9.e, T9.h needs T9.c, T9.d needs
T9.b, T9.i needs T9.h, T9.f needs T9.e, T9.j needs T9.d, T9.f and T9.i, T8.l needs T8.k and T9.f,
T8.g needs T8.f, T8.m, P11.T16 and P11.T17.c, and T11.c needs T9.f and T9.j. T10 needs T9, T4.b and
R03's frames; T11 needs T10, with T11.c after T11.a.
The client, T12–T14, needs T10 for its types and R02's `view/`; T13's subtasks follow T12,
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
T16.b; T8.f; T9.b; T9.e; T9.c, T9.h, T9.d and T9.i; T8.k with T8.j; T9.f; T9.j; T8.l; T8.m, which
needs no plan-11 task; T7.b and T8.i, which no longer wait for T8.g; T8.g, once T8.m, P11.T16 and
P11.T17.c are on `rendering-and-planets`; T8.h, built on T8.g as committed (decided 2026-10-08,
`decision-r06-t8h-warm.md`); T8.g's final gates once P11.T17.c lands, then T8.n; T11.d, after
T8.g, T8.i and T11.a–c (T11.c on T9.f); T11.f, the stars-arriving note split from T11.d
(2026-10-08), after it; T5.f and T9.g before T17's goldens; then T17 (the order amended
2026-10-06, `decision-r06-t9b-band.md`, `decision-r06-t9c-glare.md` and `decision-r06-t8k-cone.md`,
and 2026-10-07, `decision-p11-t16-hierarchy-bound.md`).
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
- **R06.T5.f The tables against the realised sky (new, slow; after T8.k, before T17's goldens).**
  Decided 2026-10-06 (`decision-r06-t9b-band.md`). Records; gates nothing at first.
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

  _As built (2026-10-07, generator version 21; Risks, "R06.T5.f's measurements, as built"): the
  test-only reads are `TablesPlan::with_full_pair_counts` and `without_pair_correction`; item 1
  takes 64 C blocks and 256 D blocks (intervals 1.64 and 1.94 points), and records 64 E blocks and
  the blocks' light and counts besides. No paired-deficit or median finding. Two count findings,
  handed to the tables lane: C's ensemble count under the full correction is 0.966 ± 0.004 and
  E's 1.164 ± 0.034. The paired counts place them in the single-star tables, which the realised
  single stars brighter than M<sub>V</sub> 4 fall 3% short of in C and exceed by 11% in E, and,
  for E, in the fit's count change (−32.3% against the realised −29.0%)._

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

- **R06.T7.b Caps by direction (new; signed off; it may precede T8.g, decided 2026-10-07,
  `decision-p11-t16-hierarchy-bound.md`, and its census figures are then taken sampled and re-taken
  in T17).** Decided 2026-10-05 (`decision-r06-census-cost.md`). The sign-off was advised by a
  decision agent and adopted on 2026-10-05 under the owner's standing delegation
  (`decision-r06-census-cost-signoff.md`, question 3). Design note 9's criterion is unchanged: under
  one expected star brighter than the cut beyond the caps, per layer, summed over the sky. Its
  radius becomes one per ray:
  - For each of the 768 rays, the radius is the outermost radial interval whose expected listable
    stars per system are at least λ. λ is the largest value that keeps the layer's expected count
    beyond under 1.
  - Each ray's radius is then raised to the largest of its neighbours within twice the ray spacing.
  - `plan_cells` opens a cell when its padded box meets the cone of a ray within the ray spacing,
    at less than that ray's radius.
  - The reply carries the radii per ray. T9's band and the client read them. The band and the
    listing share that boundary: T9.b reads the same widened per-ray radii that `plan_cells` used,
    interpolated identically.
  - Each ray counts the stars brighter than cut − DM − v☉(A_V) A_V, the band's own boundary
    (R06.T8.k), in place of the bare A_V, and the rule bound takes the same v☉. Decided
    2026-10-07 (`decision-r06-t8k-cone.md`, item 3). If T7.b slips past T17's goldens, this
    switch lands alone before them. Record the caps' move near the Sun and in the inner bulge.

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
  safety test: no texel of the final limit map with no glare, which bounds every listed star's own
  limit after its self-exclusion (R06.T9.h), is deeper than the limit its ray's cap was counted at.
  They are adopted, with no further sign-off, if they pass that test and
  `caps_converge_in_rays` and open at least 20% fewer systems near the Sun at either cut.

  Tests:
  - `caps_converge_in_rays` at all six points with the per-ray caps: the expected count beyond
    them, recounted with 3,072 rays and twice the radial steps, is under 1.5. If it fails, the
    neighbour widening grows, never the gate.
  - T7's tests on the per-ray caps (decided 2026-10-08, `decision-r06-t7b-brackets.md`): on every
    ray, the rule bound, A and B under 100 ly near the Sun, C at least 1,000 ly near the Sun and E
    under 1,500 ly in the nuclear disc; on each layer's median ray, D and E near the Sun within a
    factor of three of 4,300 and 10,000 ly, and C, D and E nearer in the nuclear disc than near the
    Sun. The far rays are held by the criterion's own gates (the rule bound, the stated count
    beyond, `caps_converge_in_rays`) and the systems-opened gates below, not by a radius bracket.
  - The identity tests, with forced caps, are unchanged.
  - No listed star lies beyond its own ray's widened radius, beyond the cell overshoot the final
    reply already allows.
  - Near the Sun the systems opened are at most 75% of the sphere's, and E's at most 35%.

  The bench records the realised stars that the spherical caps list and the rays drop, and those
  the rays gain, with no gate. A realised drop far above the expected 0.1–0.3 a layer is a finding
  on the tables.

  Files: `sky/caps.rs`, `sky/census/query.rs`. Acceptance: `cargo test -p hyperion-sim sky::caps
sky::census::query`, `just test-slow caps_converge_in_rays`.

  As built (Risks, "Deviations in T7.b, as built"): the ray spacing is the lattice's covering
  radius, so that every direction lies in a ray's cone; `CAP_RAYS` is 1,536 and each ray counts
  through the clearest of three sub-rays, since 768 rays failed `caps_converge_in_rays` above the
  Sun and a wider widening broke the 75% gate; the test also holds uniform caps at 8.54 and
  10.06; T7's brackets hold on every ray, and D's and E's near the Sun on the median ray, as ruled
  (`decision-r06-t7b-brackets.md`); the eye's visibility-based caps pass and are adopted, opt-in on
  an eye-only request (`SkyQueryBuilder::eye_visibility`), each ray taking the deepest limit of its
  cone's 16² texels and their neighbours.

- **R06.T7.c The visibility caps' safety with the illumination (new; test only; after T11.d;
  before the default switch serves eye-only skies by visibility; decided 2026-10-08,
  `decision-r06-t11d-first-sky.md` §4).** T7.b's safety test (`sky::caps`'
  `the_eyes_visibility_caps_count_what_it_sees_and_open_a_fifth_fewer_systems` and
  `least_margin`) built the eye's cut, its visibility and the final 64² limit map without the
  illumination, which the server now states on all three (R06.T9.g, R06.T11.d). Re-run it with
  `Illumination::march` at each observer, stated on the cut, the visibility and the final map's
  query, at the six points of `caps_converge_in_rays` (a slow test if it outgrows the fast
  suite), and record each point's least margin beside T7.b's +0.094 mag near the Sun. A negative
  margin anywhere widens `visible_cuts_v`'s reach, never the test. Until it passes, the server
  leaves `eye_visibility` unset: T7.b's uniform caps by ray at the eye's cut, complete every way,
  which moves the full census's cost and barely the first sky's. Acceptance: the test, by name,
  and `cargo test -p hyperion-sim sky::caps`.

  As built (Risks, "Deviations in T7.c, as built"): it passes at all six points, the least margin
  +0.073 mag, 2,000 ly above the Sun. The six points are the slow
  `the_eyes_visibility_caps_are_safe_with_the_illumination_at_six_points`. The fast near-Sun test
  keeps T7.b's gates, with the illumination.

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
  GENERATOR_VERSION bump. As built: Risks, "Deviations in T8.j, as built".

- **R06.T8.k One boundary (new; after T9.b, before T8.g, with or after T8.j).** Decided
  2026-10-06 (`decision-r06-t9b-band.md`). `kept_to` is the cut, `faintest_listable` drops
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

  The kept test is the star's own V, M_V + DM + its `reddened(A_V)` V extinction, which the wire
  carries; the band subtracts the luminosity function's light at M_V = cut − DM − v☉(A_V) A_V,
  the solar row's V secant. The two boundaries differ by (v★ − v☉) A_V, under 0.05 mag at A_V 2
  and up to about 0.2 at A_V 10, which the conservation tests hold within 1%. T8.f's per-star cut
  before the sightline and the flux bound stay exact, since v★ A_V ≥ 0. `brute_force_sky` takes
  the same V (addendum item 4). The band's overflow (`point_lux`) then takes each star's
  unextinguished light as V less its own V extinction (R06.T9.e's `Reddened::v_extinction`).

  Record each layer's listed and generated counts near the Sun, before and after. Files:
  `sky/census/{cell,query}.rs`, `tests/common/sky.rs`. Acceptance:
  `cargo test -p hyperion-sim sky::census sky::band` and
  `cargo test -p hyperion-sim --test sky_census`. The first is two commands as built,
  `cargo test -p hyperion-sim --lib -- sky::census` and `… -- sky::band`: `cargo test` takes one
  filter before `--`. As built: Risks, "Deviations in T8.k, as built".

- **R06.T8.l The cone by the band's texels, and no eye with a cone (new; after T8.k and T9.f;
  before T11.a).** Decided 2026-10-07 (`decision-r06-t8k-cone.md`, items 1 and 2).
  - A texel is in a cone's region when its centre lies within α + ρ of the axis. Here α is the
    half-angle, and ρ is the request's `BandSpec`'s largest texel radius (1.266° at 64²).
  - The band's region texels are complete, and the others complete nowhere.
  - The census keeps a star when its texel (`BandSpec::texel_of`) is in the region, and the plan
    opens cells by the widened cone. The query carries the `BandSpec` the band uses, and the
    census and the band read one cosine.
  - `SkyQueryBuilder::build` refuses an eye with a cone (`BuildSkyQueryError::ConeWithEye`), and
    `Cone`'s docs call it an instrument's field stop.
  - Tests, near the Sun to V 8 within 200 ly (at 16², cones of 1°, 10° and 30°; at 64², 0.3°, on
    the rows it meets):
    - the listed stars are the full census's stars whose texels are in the region, bit for bit;
    - each region texel's band is the full sky's, and each other texel the complete-nowhere
      band's, bit for bit;
    - so the listed and band light equal the full sky's in the region, texel by texel, to 10⁻¹²
      relative;
    - every direction of a fixed lattice of 10⁴ inside the cone lies in a region texel;
    - the narrow cones' regions list the full census's stars inside the cone;
    - `an_eye_with_a_cone_is_refused`.

    T8.k's 30° test takes this form.

  - Record the region's area against the cone's, at each test cone.
  - Files: `sky/census/{query,cell}.rs`, `sky/band.rs`. Acceptance:
    `cargo test -p hyperion-sim --lib -- sky::census` and `… -- sky::band`, as two commands.
  - As built (Risks, "Deviations in T8.l, as built"): the plan opens cells by α + 2ρ, which a
    region texel's stars need, not α + ρ; the ruling's (1 + ρ ÷ α)² is the region's mean area
    over axes, not its ceiling.

- **R06.T8.g Census cost: a bound star by star (new; after T8.f, T8.m, P11.T16 and
  P11.T17.c).** Its landing leaves the server's sky off by default. R06.T11.d turns T11.c's switch
  on (decided 2026-10-08, `decision-p11-t17c-bright.md`): at P11.T17.c's verdicts the cold census
  stays near 10⁶ CPU-s, and only T11.d delivers it nearest first. Decided 2026-10-05
  (`decision-r06-census-cost.md`), under decision item 2's trigger. Its inputs and gates were
  amended 2026-10-07 (`decision-p11-t16-hierarchy-bound.md`).
  Near the Sun, the multiple-system bound left 98% of the census in generation. The per-record
  bound becomes one per star, computed before generation. The order is:
  1. the record's composition (`draw_metallicity`);
  2. plan 11's `hierarchy_bound` (P11.T16). It gives the hierarchy of every attempt the generator
     can keep, by the generator's own draw. So each star's initial mass is exact, with its body
     and attempt, and so is each star–star pair's drawn periastron;
  3. each pair's `pair_light_bound` (P11.T17) over the record's light-time ages: `Detached`,
     `Unchanged`, `Remnants`, `Bright(M)`, or none;
  4. for each star, T8.m's `sky_phase_envelope`, read at:
     - the star's own mass interval and \[Fe/H\] interval;
     - its age relative to its lifetime;
     - its own η draw, read from its `StarDraws` at its attempt (the primary's at attempt 0).

  A star of a `Detached` or `Unchanged` pair, or of no pair, takes its own bound. A `Remnants` pair
  gives nothing while white dwarfs are dark (ask A4), and afterwards the table's white-dwarf rows.
  A `Bright(M)` pair bounds each of its living stars by the brighter of its own bound and M, and
  its products by M. A pair that plan 11 cannot bound sends its record to generation. The primary
  is the same star at every attempt and is bounded once. The system is generated only if some
  star's bound, over every attempt listed, is listable at the system's nearest distance. The test
  runs first at the epoch distance less the pad and offset (T8.f's step 5), then after
  `retarded`. `max_star_mass`'s widened envelope stays for the cell floor, and for any record with
  a pair that plan 11 cannot bound.

  Tests:
  - (slow) `the_star_bound_holds_for_realised_systems`: over ≥ 10⁵ realised systems of each layer
    near the Sun and in the bulge, at two emitted times, every star of `state_at(t).stars()` is no
    brighter than its bound.
  - T8.e's identity tests pass unchanged, with T16.b's B skip check restored. In the slow near-Sun
    tests, B and C each generate under 25% of their records. D's and E's shares are recorded:
    about 45–50% and 80–85% are expected at P11.T17.c's verdicts (decided 2026-10-08,
    `decision-p11-t17c-bright.md`).
  - The pinned mergers are listed.

  Cold cost, from the sampled bench near the Sun (re-stated 2026-10-08,
  `decision-p11-t17c-bright.md`): recorded, not gated, at the caps the bench uses (named) and at
  the spherical caps, at the eye's cut and at 7.95.
  - The gates first set, 6 × 10⁴ CPU-s at the spherical caps and 2.5 × 10⁴ at T7.b's caps, assumed
    P11.T17.c's `Bright` and `Unchanged` about as tight as each pair's own evolution.
  - P11.T17.c's tables bound a cell and its neighbours. So `Bright`'s M is the brightest product
    of a neighbourhood: median M<sub>V</sub> −6.4, −7.3 and −5.0 in C, D and E. And a record is
    generated if any pair of any attempt listed is unbounded or listably bright: E lists 5.6
    attempts.
  - About 0.8–1.3 × 10⁶ CPU-s is expected at the spherical caps and 0.4–1.0 × 10⁶ at T7.b's,
    against 1.44 × 10⁶ at P11.T17.a's answers. E is about 80% of it. Pairs bound as tight as their
    own truth would give about 5.5 × 10⁴ and 2.6–4.2 × 10⁴.
  - The gates are structural:
    - the slow realised-systems test and T8.e's identity tests, as above;
    - each layer's generated share recorded beside T8.g's 20.8%, 55.9% and 86.3% at P11.T17.a's
      answers. P11.T17.c's verdicts are never looser than T17.a's, so a rise beyond the sampling
      is a finding for P11.T17.c.
  - The miss is reported with R06.T17's full-cold gate (`decision-p11-t17c-bright.md` §4).

  Target, recorded: at most 1% of the records past the floor are generated, C to E together
  (98.3–99.99% in T8.f's sampled bench). It is missed at version 21: C's own bounds alone give about
  1.7% of C–E. Its levers are deferred (`deferred-corrections.md`, "Census cost").

  Record also the sampled cold estimate at cut 10.06, the camera's, beside the eye's 7.95
  (`decision-r06-census-cost-signoff.md`). Record:
  - each layer's generated share;
  - the share of pairs taking each of plan 11's verdicts, and none;
  - the cost of each bound step: about 6 µs a record for every step but `hierarchy_bound`, whose
    own cost is P11.T16's.

  Files: `sky/census/cell.rs`. Acceptance:
  - `cargo test -p hyperion-sim --lib -- sky::census`;
  - `just test-slow the_star_bound_holds_for_realised_systems
the_census_is_its_oracle_for_the_dwarfs_near_the_sun the_census_is_its_oracle_1000_ly_from_the_sun
the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc`;
  - `just ci`.

  No generator bump: the census only reads, and plan 11's functions read existing words.

  As built so far (Risks, "Deviations in T8.g, as built"): written against P11.T17.a's interface
  and committed before its gates, which a fresh agent takes once P11.T17.c lands (the
  `--serve-sky` default moved to T11.d, 2026-10-08); the phase envelope read process-wide; the
  widened envelope kept as a first tier for every record; the pairs' verdicts taken once, over
  the ages before the drift. Its final gates were taken on 2026-10-08 at P11.T17.c's answers. The
  identity tests pass, and the cold census is about 7–15 × 10⁵ CPU-s, recorded as a miss. The
  slow realised-systems test fails on one system in D in the bulge (a `Remnants` miss), and T8.g
  lands after plan 11's fix.

- **R06.T8.m The phase envelope (new; split from T8.g on 2026-10-07; it needs no plan-11 task,
  so it may start at once).** Decided 2026-10-05 (`decision-r06-census-cost.md`) as part of T8.g,
  and split on 2026-10-07 (`decision-p11-t16-hierarchy-bound.md`). `sky_phase_envelope` is a
  hyperion-fit task, with fit-check and sim-fingerprint, depending on no galaxy:
  - Each cell is a mass interval of the envelope's nodes × a \[Fe/H\] interval of `FE_H_NODES` ×
    an η interval × a relative-age bin. Bins are fine (10⁻³) over 0.8–1.05 and coarse elsewhere.
    Beyond the bracket only remnants remain, which are dark until A4.
  - The η axis is needed. Today's envelope reaches M_V −5.7 at 10 Gyr only through the η = 0 tail
    draw, and the proxy's pass rates used each star's own η.
  - Each cell holds the brightest V of single stars sampled densely within it, plus `MARGIN_MAG`:
    8 masses, 3 \[Fe/H\] and 3 η.
  - It is stored in integer millimagnitudes, rounded brighter, as `sky_envelope` is.
  - The relative age is age ÷ `FittedFates::lifetime_bracket` (P06.T38.d) at the star's η, and
    the table is read over the bracket's whole span. `lifetime_bracket` starts at 0.741 M☉ and
    allocates (P11.T17's lane, 2026-10-07). So T8.m folds the bracket into the table's index, or
    adds an allocation-free reader, so that T8.g reads no allocation per star. It states how stars
    below the bracket's first mass are bounded.

  Tests: (slow) `phase_envelope_bounds_dense_tracks`. Over ≥ 10⁵ single stars of random mass
  (0.0124–150 M☉), \[Fe/H\], η and age, no V is brighter than the table, within the margin.
  Record the margin used.

  Files: `sky/phase.rs` (new: the table's reader), `crates/hyperion-fit/…/sky_phase_envelope`,
  `tables/sky_phase_envelope.rs`, and the fit-check list. Acceptance:
  - `cargo test -p hyperion-sim --lib -- sky::phase`;
  - `just fit-check`;
  - `just test-slow phase_envelope_bounds_dense_tracks`;
  - `just ci`.

  As built (Risks, "Deviations in T8.m, as built"): the relative age is read against the table's
  own track lifetimes, not `lifetime_bracket`; three η intervals; each cell widened in relative age
  by its own spread; runs merged within 0.2 mag; the rows packed (format 1, 375 kB).

- **R06.T8.h The cell cache keyed by magnitude (new; after T8.g as committed; amended
  2026-10-08).** Decided 2026-10-05 (`decision-r06-census-cost.md`), and amended 2026-10-08
  (`decision-r06-t8h-warm.md`). Near the Sun every C–E floor is its band's lower edge, so Design
  note 12's mass key makes an entry hold every record of its cell. And about 1.08 × 10⁸ cells
  at about 128 B an entry are 10–14 GB, whatever the records.
  - **Blocks.** The cache keeps blocks of 4³ cells of one layer, each built for one observer
    position and cut. A cell's key and window are recomputed from those, so a cell stores
    neither.
    - Its key is the faintest absolute V listable at its least distance from any observer within
      `CACHE_APPROACH_LY` (1,000 ly, the jump drive's range), at the cut plus
      `CACHE_CUT_SLACK_MAG` (0.1, provisional).
    - Its window is every emitted time such an observer, at any time in ±H, can receive.
  - **Held records.** An entry holds the records whose star-by-star bound over that window can
    be listed at the key, in candidate order, each with that bound: at most 96 B a record, and
    at most 8 B an opened cell on average.
  - **The rule.** An entry serves a query whose key is no fainter than its own and whose window
    it holds. Any other cell is rebuilt, at the block's parameters if they serve the query,
    otherwise at the query's.
  - **The stored bound is a pre-filter.** Each record it passes takes the census's own steps for
    the query, `hierarchy_bound` included, so the warm census generates the same systems as the
    cold one. The reply is the same bit for bit, with no rebuild after a jump of up to 1,000 ly
    within ±H.
    - This rests on each bound only loosening as its window widens, which a test holds.
    - The counts that depend on the path (candidates, records bounded, pairs, held,
      pre-filtered) move out of `CensusTallies`, which keeps what the wire reads.
  - **The default.** `HYPERION_SKY_CACHE_MB` stays 64 MiB until R06.T8.n sets it.

  Tests:
  - T8.d's, through the new rule: a query after a looser one, a tighter one, a move of 1,000 ly
    and a time of ±H gives the stars and tallies no cache gives, and no entry is read for a
    fainter key or a window it does not hold;
  - each bound over an entry's window is no tighter than over any window within it, over 10⁴
    records of each of C, D and E near the Sun and in the bulge;
  - the pre-filter keeps every record the census without a cache generates;
  - a census in the moving galaxy through a cache built elsewhere and earlier equals one without
    a cache;
  - the server's jobs census, cold and warm, equals the one-pass census.

  Bench: `sky/census_near_sun/warm` and a new `…/warm_jump` (1,000 ly towards the centre). A
  sampled run's cache budget is the setting ÷ k. Each prints held bytes, served and rebuilt
  cells by cause, held, pre-filtered, records bounded, generated and generated-listable, and
  warm ÷ cold, recorded provisionally at P11.T17.a.

  Acceptance:
  - `cargo test -p hyperion-sim --lib -- sky::census`;
  - `cargo test -p hyperion-sim --test sky_census`;
  - `cargo test -p hyperion-server`;
  - `just ci`.

  No `GENERATOR_VERSION` bump.

  As built (Risks, "Deviations in T8.h, as built"):
  - the rule crate-private and the trait reshaped;
  - the windows' and keys' slack doubled;
  - merges built outside the lock;
  - the bench sampled by block, its `cold` on T8.g's path;
  - the server's second sky 20 ly off.

  Provisional at P11.T17.a: 13.6 GB of entries for a near-Sun sky; the repeat at 102% of cold,
  the jump at 121%; no cell rebuilt, and the same reply.

- **R06.T8.n The sky cache's default, from measured entries (new; after P11.T17.c and T8.g's
  final gates).** Decided 2026-10-08 (`decision-r06-t8h-warm.md`).
  1. Run T8.h's window test on P11.T17.c's verdicts. A violation is P11.T17.c's to fix.
  2. Run the sampled `cold`, `warm` and `warm_jump` benches at the eye's cut on T7.b's caps.
  3. Set `CACHE_CUT_SLACK_MAG` in [0, 0.25].
  4. Set `HYPERION_SKY_CACHE_MB`'s default to the larger of 64 and 1.25 times the held bytes
     after the Sun's census and the jump's, rounded up to 256 MiB, at most 2,048. If over, first
     store held records as cell-local indices with their bound (about 8 B), placed again when
     served, then re-measure. If still over, 2,048 MiB, and report.
  5. Record T17's warm figures.

  Acceptance:
  - the benches recorded;
  - `cargo test -p hyperion-sim --lib -- sky::census`;
  - `cargo test -p hyperion-server`;
  - `just ci`.

- **R06.T8.i Nearest first: the shell plan (new; signed off; it may precede T8.g, and T11.d builds
  on it; decided 2026-10-07).** Decided 2026-10-05 (`decision-r06-census-cost.md`). The sign-off
  was advised by a decision agent and adopted on 2026-10-05 under the owner's standing delegation
  (`decision-r06-census-cost-signoff.md`, question 2), with its amendments here. `census_plan`
  orders its cells by distance shell. The shell edges, per layer, are 500 ly, then 1,000 × 2^k ly
  up to the cap; the edges are constants, which never adapt to the machine, so every machine and
  worker count gives the same sequence of replies. A, B and the brown dwarfs are one shell each.
  - A cell belongs to the first shell its padded box meets.
  - `SkyCensus` carries, per layer, the radius to which it is complete.
  - After shell k a layer is complete, towards each direction u, to the lesser of shell k's edge
    and `cap.radius_toward(u)`: its cap's rays each held within the edge, or the edge in every
    direction where every ray reaches it.
  - Until its last shell is merged, a layer lists only the stars within that radius towards
    their band texel. That radius is read towards the centre of the texel that
    `BandSpec::texel_of` places the star in, at the query's `band_spec`. The star is placed from
    the same displacement the band places an overflow star by (R06.T8.l's lookup). It is the
    radius the band's ray through that texel reads, bit for bit. A straddling cell's stars beyond
    it wait for a later shell, and they are neither listed nor overflow. So listing and band share
    one boundary in every texel, and no star's light is counted twice (Design note 11). For
    uniform radii it is the radius in the star's own direction. Decided 2026-10-08
    (`decision-r06-t8i-listing.md`).
  - A layer whose last shell is merged lists every star of the cells it opens, as the one-shot
    census does, in that reply and every later one.
  - Where a ray cone's edge crosses a texel, the radius towards the texel's centre can exceed the
    radius towards one of its stars. If that star's cell is unopened, its light is in neither the
    census nor the band, in partial and final replies alike. This is T7.b's gap, deferred by the
    owner on 2026-10-08. The expected count beyond the caps bounds it, under one star a layer, and
    near the Sun it is estimated at about 10⁻² stars over the sky (R06's Risks, "Deviations in
    T8.i, as built").
  - Merging shells 1 to k gives the census to shell k, and the last gives the one-shot census,
    which keeps today's rule of listing every star of the cells it opens.

  Tests:
  - Shells 1–k merged hold, bit for bit, every star of the census with caps forced to shell k's
    radii that lies within its layer's radius towards its band texel.
    - Shell k's radii are each ray's radius held within edge k. For a cap of one radius, or where
      every ray reaches the edge, they are edge k in every direction.
    - Any other star that shells 1–k list lies within that radius towards its texel, and at or
      beyond it towards its own direction. Such a star is one of the gap's. The one-shot plan's
      padded ball opens its cell, and the forced census's smaller ball does not.
    - For uniform caps the two sets are equal: the census with caps forced to edge k, less its
      stars at or beyond the edge.
  - The last shell merged gives `census_plan`'s one-shot census, bit for bit.
  - The shells partition `census_plan`'s cells, each cell in exactly one.
  - In a layer not yet final, no listed star lies at or beyond its layer's radius towards its band
    texel. The band summed for that census reads, towards each texel's centre, the same radius bit
    for bit.

  Acceptance: `cargo test -p hyperion-sim sky::census::query`. As built (Risks, "Deviations in
  T8.i, as built"): the shells rank by rank, `SkyCensus` carrying `Option<Completeness>`, the
  census tests at a test's nearer edges, and the acceptance as built.

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
- **R06.T9.e Reddening (new; after T9.b, before T9.c).** Decided 2026-10-06
  (`decision-r06-t9b-band.md`). The colour table gains a companion's 78 columns per row from T3's
  fit and spectra, each over plan 07's sightline A_V: for the six positive and negative parts of
  the Rec. 709 colour-matching functions, V, the scotopic and the camera, each band's moment at
  A_V → 0 and its secant at `REDDENING_A_V_NODES` (2, 5, 7.5, 10, 15, 20 and 30; 7.5 ruled the
  same day, the addendum's amendment), with each part's
  value per unit luminance. The photopic is the parts' luminance. They are split by node across
  files under the 500 KB limit; every existing column stays bit for bit (`just fit-check`).
  Decided 2026-10-06, `decision-r06-t9b-band.md`, addendum.

  `StarColour::reddened(a_v) → Reddened` gives the r, g and b transmissions, the photopic
  transmission, the reddened colour of unit luminance (each channel its two parts, each dimmed by
  its own secant, over their luminance and lifted into gamut by T3's rule), the reddened ρ, the
  star's own V extinction and the reddened camera band term, bit for bit the colour's own at A_V 0. Each secant is piecewise linear in A_V through its moment and the nodes, and held beyond 30.
  `band_rows` reddens each node's light with the solar row's curves, in five sums (the photopic
  light, R, G, B and the scotopic light), and each overflow star by its own `reddened`. T9.c's
  glare and T11's wire (chroma, eye offset, V, camera term) read `reddened`. The same commit moves
  T9.b's plane reference from 22.4 to 22.05 in `sky::band`'s test and doc (item 2).

  Tests (the addendum's, added to or replacing T9.e's first ones):
  1. at the nodes, `reddened(A_j)` reproduces each node's columns to their digits, on every row of
     both grids;
  2. between the nodes (a fit test on the fetched spectra, run once as the reproduction tests
     are), for the solar row, 30,000 K and 10,000 K dwarfs, a 4,000 K, log g 1.5 giant, 3,500 K
     and 3,000 K dwarfs, the coolest lifted PHOENIX row and a 10,000 K white dwarf, at A_V 0.5, 1,
     1.5, 3, 4, 7, 12, 17 and 25, against the direct integrals, the interpolated values lie
     within:

     | Quantity                                       | To A_V 5  | To A_V 20             | To A_V 30 |
     | ---------------------------------------------- | --------- | --------------------- | --------- |
     | the lifted colour at unit luminance, a channel | 0.005     | 0.02                  | 0.07      |
     | the photopic                                   | 0.003 mag | 0.003 mag             | 0.02 mag  |
     | the camera term                                |           | 0.035 mag (to A_V 10) | 0.2 mag   |
     | the eye offset (ρ)                             | 0.01 mag  |                       | 0.05 mag  |
     | V's extinction                                 | 0.005 mag |                       | 0.05 mag  |

  3. the Sun: at A_V 1, b is 0.689 ± 0.003 at unit luminance (T3.c's ratios gave 0.725); at A_V
     2, 0.500 ± 0.005 (amended, below);
  4. continuity and one lift: the sim's lift of each row's c⁺(0) − c⁻(0) equals `star_colour`'s
     chroma within 10⁻⁶, and equals the fit's `unit_rgb` bit for bit;
  5. the photopic identity: with `LUMINANCE_RGB`, Σ Y_c c(A) from the exact parts equals the
     direct V(λ) integral within 10⁻⁴ at every node (fit test);
  6. monotone: every band's transmission falls with A_V on every row;
  7. held beyond 30: `reddened(40)` uses the 30 node's secants;
  8. the solar row's ranges: `A_S ÷ A_V − A_P ÷ A_V` in 0.11–0.15, `A_P ÷ A_V` in 0.97–1.01,
     `A_cam ÷ A_V` at A_V → 0 in 0.84–0.92, and V's own in 0.99–1.02;
  9. the band's tests unchanged in form: a Plummer cloud of A_V 1 on +X lowers the texels behind
     it in ρ and in blue over green (the new transmissions), against an unreddened march, by its
     curves to 1%, and leaves −X's bits; near the Sun the plane's ρ falls 5–20% and the poles'
     under 1.5%, and the plane's μ moves under 0.05; an overflow star's sums are its reddened
     colour's. The plane's blue at unit luminance (expected near −23%) and its ρ are re-measured
     and recorded.

  Amended the same day (the ruling's "Amendment, 2026-10-06: test 3's reference and the node at
  7.5"): test 3's reference is the interpolated solar row's own spectrum, at A_V 1 b 0.689 ± 0.003
  and at A_V 2 0.500 ± 0.005 (the first figures, 0.685 and 0.496, were the 5,750 K, log g 4.5
  node's); and the node at 7.5 keeps test 2's camera term within 0.035 mag to A_V 10, where test 2
  also samples 6.25 and 8.75. As built, test 2 also takes a 45,000 K dwarf (Risks, "Deviations in
  T9.e, as built").

  Files: `sky/{colour,band}.rs`,
  `tables/star_colour_reddening{,_av02_05,_av07p5,_av10_15,_av20_30}.rs`,
  `crates/hyperion-fit/src/tasks/star_colour/{columns,mod,photometry}.rs`, the five tasks'
  manifests. Acceptance (as built, Risks, "Deviations in T9.e, as built"): `cargo test -p
hyperion-sim sky::colour`, `cargo test -p hyperion-sim sky::band`, `cargo test -p hyperion-fit
star_colour` and `just fit-check`, and once on the fetched spectra the four slow tests
  (`cargo test --profile slow-test -p hyperion-fit --test star_colour -- --ignored`): the colour
  table's and the five reddening tables' reproduction on eight threads and on one, every row's lift and photopic identity, and test 2.

- **R06.T9.c The limit map.** `sky::limits::limit_map` with the glare of Design note 4. Tests: each
  texel's limit is `naked_eye_limit` at its band luminance plus its glare and at its ρ, to 10⁻⁹ mag;
  near the Sun, with the band at cut 8.15 and the diffuse galactic light (R06.T9.g), the median
  texel limit is 6.41 ± 0.20 in the band (|b| under 5°) and 7.51 ± 0.22 at the poles (|b| over 80°).
  These are Crumey's limit at Gaia DR3's light fainter than the cut there plus the diffuse light (μ
  21.95 and 24.53): in the band, Toller's 0.21 of the light fainter than V 6.5 (Leinert et al. 1998,
  Table 39); at the poles, 250 nW m⁻² sr⁻¹ per magnitude of Schlafly and Finkbeiner's A_V of about
  0.04. The tolerance is T9.b's 0.5 mag of μ through Crumey's slope (0.40 and 0.45 per mag) (decided
  2026-10-06, `decision-r06-t9b-band.md`; re-derived 2026-10-07, `decision-r06-t9g-dgl.md`). The
  fixture gives about 6.29 and 7.41 (6.53 and 7.71 without the diffuse light): its poles are about
  0.3 mag faint in starlight, and its dust is thicker than the sky's (Risks). A texel within 1° of a
  V = −1.5 star is at least 0.3 mag shallower than its neighbours' mean; the map is a function of
  the listed stars and the band alone. Acceptance: `cargo test -p hyperion-sim sky::limits`. As
  built (Risks, "Deviations in T9.c, as built"): `Glare::of_listed` resolves each listed star's
  reddened light once a census, and `limit_rows` sets a job's rows, as `band_rows` takes them. The
  self-veil, the map's cost and E's plane, which T9.c left open, were decided 2026-10-06
  (`decision-r06-t9c-glare.md`): R06.T9.h, R06.T9.i and R06.T9.j.
- **R06.T9.h The eye offsets and the eye's plane (new; T9.c's follow-up, before T9.d).**
  Decided 2026-10-06 (`decision-r06-t9c-glare.md`, items 1 and 3).
  - A star's own veil is not its own background: Blackwell's thresholds hold a target's own
    scattered light, and CIE 146's veil is one source's over another target. `limit_rows` keeps
    each texel's veil, photopic and scotopic, beside its limit.
  - `eye_offsets(eye, spec, glare, band)` gives each listed star, in the census's order, its own
    eye limit less its texel's:
    - its self-exclusion: `naked_eye_limit` at its texel's background less the veil the map
      added for it, at its angle from its texel's centre (`BandSpec::texel_of` of its direction),
      less the texel's limit;
    - plus `star_colour_offset` of its reddened ρ against that background.

    A star that adds no veil takes its colour offset alone. The texels' limits do not change.

  - E is the illuminance in the plane of the eye (CIE 146:2002; IJspeert et al. 1990, Vision
    Res. 30, 699; Stiles and Crawford 1937, Proc. R. Soc. B 122, 255). A star of illuminance E at
    θ from a texel's centre gives E cos θ, and none at or beyond 90°. `veiling_luminance` keeps
    its arithmetic, and its doc names the plane.
  - Tests:
    - the identity. Every texel's limit, and every listed star's own limit (its texel's plus its
      eye offset), is `naked_eye_limit` (plus `star_colour_offset`) at the background written
      again from the definition, to 10⁻⁹ mag. The definition uses atan2 angles and E cos θ, and
      leaves the star's own term out of its own limit. It runs on T9.c's fixture and its two
      placed sources;
    - a star of the reference colour, 0.05° from a 64² texel's centre at the poles and 0.05 mag
      brighter than its own limit (its texel's less its own veil), is culled at its texel's limit
      and kept at its own;
    - a field factor of 2 leaves every eye offset as at 1.4, to 10⁻⁹;
    - every self-exclusion is non-negative in the fixture's scotopic texels;
    - the reach: a source at 89.995° veils by exactly E cos θ times its `veiling_luminance` per
      lux, and one at 90.005° adds exactly nothing;
    - T9.c's other tests, as built.
  - Record:
    - the medians against T9.c's 6.544 and 7.668;
    - the largest self-exclusion that decides a near-Sun star (its V between its texel's limit
      and its own);
    - the largest eye offset any listed star then has, against the wire's +1.27 (at Crumey's
      clamp the ruling gives about 1.0 + 0.43);
    - the largest self-exclusion of a star within 0.5 mag of its limit and 0.6° or more from its
      texel's centre (the ruling's model: under 0.01).
  - Files: `sky/{limits,band,eye}.rs`. Acceptance: `cargo test -p hyperion-sim sky::limits` and
    `cargo test -p hyperion-sim sky::eye`.
- **R06.T9.d The eye's cut.** `sky::limits::eye_cut` (Design note 5): the coarse pre-pass at 16²
  texels a face through `band_rows` with `SkyCensus::empty()`, the darkest texel's limit, clamped by
  `naked_eye_limit` itself (Crumey's 10⁻⁵ cd m⁻², decided 2026-10-02; no second clamp), + the colour
  table's largest colour offset (0.453) and +0.1, and one repeat when the cut deepens. Tests: the
  cut is the darkest pre-pass texel's `naked_eye_limit` + 0.453 + 0.1 after the repeat, to 10⁻⁹ mag;
  the colour table's largest eye colour offset at μ 30 is at most 0.46 (it is 0.453); near the Sun,
  with the diffuse galactic light (R06.T9.g), the cut is 8.10 ± 0.22. That is Crumey's limit at the
  darkest 16² texel of Gaia DR3's light fainter than the cut plus the diffuse light (μ about
  24.60–24.65; the diffuse light at 250 nW m⁻² sr⁻¹ per magnitude of Schlafly and Finkbeiner's A_V,
  0.03–0.07 there), plus 0.553, with T9.b's tolerance carried through Crumey's slope (decided
  2026-10-06, `decision-r06-t9b-band.md`; re-derived 2026-10-07, `decision-r06-t9g-dgl.md`). The
  fixture gives about 8.0, and 8.28 without the diffuse light; no texel of the full limit map with
  no glare, which bounds every listed star's own limit after its self-exclusion (R06.T9.h), is
  deeper than the cut less the largest colour offset (0.453), and a miss is a finding for the pad,
  not a looser test; the repeat changes the cut by under 0.05 mag (about 0.01 near the Sun with the
  diffuse light, which no cut changes). The colour offset is the table's real maximum, 0.453, not
  the first 0.43 and +0.45 (the orchestrator's ruling of 2026-10-06 on R06.T9.h's open question 1).
  Acceptance: `cargo test -p hyperion-sim sky::limits`.
- **R06.T9.i The far field (new; after T9.h, and after T9.d in the order; before T9.j, T11.c and T17's goldens).**
  Decided 2026-10-06 (`decision-r06-t9c-glare.md`, item 2).
  - Per face, a pyramid of the band's texels, from its leaves (the band's texels) to one node a
    face. Each node keeps its glaring stars' photopic and scotopic illuminance, their
    photopic-weighted mean direction, and an angular radius r that bounds them about it.
  - For each texel, a node at angle d, measured to its mean direction:
    - is skipped when d − r ≥ 90°;
    - is taken whole when r ≤ 0.25 d and d − r ≥ 4°: its illuminance times E cos θ's veil per lux
      at its mean direction;
    - is otherwise opened; an opened leaf is summed star by star.
  - A texel always opens its own leaf, so T9.h's offsets stay exact. The traversal and the sums
    are in a fixed order, so any split of rows gives the same bits. The radius and the angle are
    the ruling's model's, and may be tuned to the tolerance.
  - A test-only exact sum (every node opened) keeps T9.c's and T9.h's identity tests at 10⁻⁹.
  - Tests:
    - every texel's limit within 0.001 mag of the exact sum, and their mean within 0.0003 mag, at
      64². This holds on the near-Sun fixture (the census within 200 ly to V 8.15, with T9.c's two
      placed sources) and on a synthetic sky of 300,000 stars to V 10.06, concentrated towards
      the plane. Every eye offset is within 0.001 mag of the exact sum's;
    - on the synthetic sky the pyramid evaluates at most a hundredth of the exact sum's
      star–texel pairs. They are counted, so the test does not depend on the machine (the
      ruling's model: about a three-hundredth);
    - any split of rows gives the same bits.
  - Bench: `sky/limit_map`, at 64², on the near-Sun fixture and at 300,000 synthetic stars. T17
    records it per reply. The gate, provisional: at most 3 CPU-s at 300,000 stars on the dev
    machine (the ruling's model: about 2).
  - Files: `sky/limits.rs`, `benches/sky.rs`. Acceptance:
    `cargo test -p hyperion-sim sky::limits`.
  - As built (Risks, "Deviations in T9.i, as built"): `Glare::of_listed` takes the band's
    `BandSpec`, whose texels its pyramid holds, and `Glare::of_points` gives a synthetic sky's glare
    for the bench. The synthetic sky's test of every texel is slow, and runs as:
    `cargo test --profile slow-test -p hyperion-sim --lib -- --ignored sky::limits::tests::on_300_000_synthetic_stars`.
- **R06.T9.f The band's march, kept (new; after T9.e, before T11.c).** Decided 2026-10-06
  (`decision-r06-t9b-band.md`). `sky::band::{march_rows, BandMarch, sum_rows}`.
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

  As built (Risks, "Deviations in T9.f, as built"): `march_rows` takes the replies a request
  states, each a `CompleteTo`, and keeps per layer and radius one slot, the sum of a reply
  complete to it; `band_rows`'s bits move at rounding only. The benches are
  `sky/band_near_sun/march` and `sky/band_near_sun/sum`.

- **R06.T9.j The eye's own sky under a camera's cut (new; after T9.d, T9.f and T9.i; before T11.c and T17's goldens).**
  Decided 2026-10-06 (`decision-r06-t9c-glare.md`, the finding in item 2).
  - The eye's map is the eye-only request's at the same census radii, whatever the request's cut.
  - When the request's cut is deeper than the eye's (a camera's), `march_rows` also keeps, per
    layer and edge, the five sums of the light fainter than the eye's cut. `sum_rows` gives the
    eye's background from them, beside the band's texels.
  - `Glare::of_listed(observer, listed, eye_cut)`: only the listed stars brighter than the eye's
    cut glare. A listed star at or fainter than it adds neither glare nor background, since its
    light is in the expected light already. Its eye offset is its colour offset alone.
  - The band's texels as sent do not change.
  - Tests:
    - near the Sun, a request at a camera's cut of 10.06 with the eye's cut at 8.15 gives the
      eye limits and eye offsets of the request at 8.15, bit for bit, for the stars both list. Both
      use the same census radius;
    - a listed star between the cuts changes no texel's eye limit;
    - a request whose cut is the eye's gives T9.i's bits;
    - every `sum_rows` at the eye's cut from a deeper march equals the march at the eye's cut,
      bit for bit.
  - Record the march's heap and time with the second sums, at 64² near the Sun.
  - Follow-up (decided 2026-10-07, `decision-r06-t9c-glare.md`, addendum 2): beyond the eye-only
    caps, a camera's census lists real stars brighter than the eye's cut, accepted as the truer
    sky. Test `the_eyes_light_is_independent_of_the_census_radius`: near the Sun at 16², the
    camera's request at 10.06 with the eye's cut at 8.15, forced to 100 ly and to 200 ly, each band
    complete to its radius, gives the eye's light (the listed stars at or brighter than the eye's
    cut, and the eye's background) within 1% at both.
  - Files: `sky/{band,limits}.rs`. Acceptance: `cargo test -p hyperion-sim sky::band` and
    `cargo test -p hyperion-sim sky::limits`, as two commands.
  - As built (Risks, "Deviations in T9.j, as built"): the query states the eye's cut
    (`SkyQuery::eye_cut`, `SkyQueryBuilder::eye_cut`), which `march_rows` and `band_rows` read,
    so their signatures stay; each texel holds the eye's light beside its own, which the limit map
    reads; the eye's light holds the overflow's stars at or brighter than the eye's cut, so the
    identity holds at any N_max; and a star at the eye's cut itself glares, as the census keeps it.
    Bench: `sky/band_near_sun/march_camera` and `/march_camera_no_eye`.

- **R06.T9.g Diffuse galactic light (new; research first; after T9.f, before T17's goldens).**
  Decided 2026-10-06 (`decision-r06-t9b-band.md`); its model decided 2026-10-07
  (`decision-r06-t9g-dgl.md`). `sky::dgl`: the starlight the band's dust scatters into each ray,
  from plan 07's dust and the band's own light.
  - **The illumination.** The observer's own sky of all starlight: `march_rows` at 16² texels a
    face (the eye cut's pre-pass's directions), complete nowhere, with no census, reddened as the
    band is. It holds each texel's five sums, in cd m⁻², and its ray's A_V to the root cube's
    edge, A_∞. It is every scattering point's field (the local-field approximation), exact in a
    uniform medium. It depends on the observer and time alone, not on the cut, the eye, the cone,
    the census or the replies. It is built once a request, before the eye's cut, and stated on
    the query (`SkyQueryBuilder::illumination`, refused for another observer); `eye_cut` takes
    it.
  - **Single scattering.** For a ray along d and each sum X (photopic, red, green, blue,
    scotopic):
    - J_X(d) = Σ_k Φ_X(d · d_k) F_X,k Ω_k ÷ Σ_k Φ_X(d · d_k) Ω_k over the illumination's texels,
      with Φ_X Henyey and Greenstein's (Draine 2003, eq. 4) at the sum's g;
    - D_X = 1 − t_X(A_∞), with t_X the solar row's transmission of the sum through the ray's own
      A_∞ (`Reddening::through`): κ_X A_∞ ÷ 1.0857 for thin dust and 1 for thick;
    - the single-scattered light is ω_X D_X J_X[F].
  - **The higher orders, where the dust is thick.** S is the fixed point, on the illumination's
    texels, of S_X = ω_X D_X (J_X[F] + J_X[S]), iterated to 10⁻⁴ of the largest photopic S (at
    most 64 times). A ray's diffuse light is DGL_X = ω_X D_X (J_X[F] + D_X J_X[S]): ω ÷ (1 − ω)
    of a uniform medium's light, and within −8% to +11% of an exact plane-parallel solution at
    every latitude near the Sun, for the fixture's dust and for a realistic layer.
  - **The dust.** ω and g are Draine's (2003; the WD01 R_V 3.1 model,
    `kext_albedo_WD_MW_3.1_60_D03.all`), linear in ln λ. Each is taken at the wavelength where
    Cardelli et al.'s A_λ ÷ A_V is the solar row's moment of the sum at A_V → 0 (for a channel,
    its two parts' (c⁺k⁺ − c⁻k⁻) ÷ (c⁺ − c⁻)). That gives about ω 0.677 and g 0.536 photopic,
    0.676 and 0.551 scotopic, and 0.667–0.677 and 0.51–0.565 in the channels. The camera reads
    the band's luminance at η☉, as before (Design note 18).
  - **In the band.** `march_rows` keeps each ray's diffuse sums beside its slots, and `sum_rows`
    adds them to each texel and to the eye's light. So the light is the same in every reply and
    under any cut, census or cone, and T9.j's eye light keeps its bits.
    `BandTexel::diffuse_luminance` gives the part. A query with no illumination gives the band
    as before, bit for bit: T9.b's starlight test and the conservation tests take it so.
  - Tests:
    - the kernel within 10⁻⁷ of its closed form, its weights normalised to 10⁻¹², and Draine's V
      row;
    - a uniform sky: J = F, single scattering ω D F, grey D = 1 − 10^(−0.4 A_∞), to 10⁻¹²;
    - behind thick dust (A_∞ 100), ω ÷ (1 − ω) of the light within 10⁻⁴;
    - thin dust, linear in A_∞ and equal to ω κ A_∞ ÷ 1.0857 of the light, within 10⁻³;
    - a lone source scatters by the kernel's ratio, forward over backward above 1;
    - D_X = 1 − t_X to 10⁻¹⁵, monotone, and κ_X ÷ 1.0857 per magnitude for thin dust;
    - the diffuse sums bit for bit whatever the cut (6.5, 8.15, 10.06 with the eye at 8.15), the
      replies, the census, a 30° cone, the split of the rows and the order of the illumination's
      jobs;
    - a texel is its starlight and its diffuse light, and the eye's light under a camera's cut
      takes the same diffuse sums;
    - on a plane-parallel sky (Flynn et al. 2006's local light as 41% at h 100 pc and 59% at 300
      pc; grey dust of h 125 pc at 0.7 mag kpc⁻¹; the observer 20.8 pc up), the diffuse light
      over the starlight in Toller's bins (Leinert et al. 1998, Table 39: 0.21, 0.34, 0.31, 0.19,
      0.25, 0.17, 0.17, 0.12 at |b| 0–5, 5–10, 10–15, 15–20, 20–30, 30–40, 40–60, 60–90°)
      within a factor 1.5, and within 5% of the ruling's 0.286, 0.213, 0.192, 0.180, 0.165, 0.147,
      0.129 and 0.113;
    - near the Sun at 16², over texels at |b| over 40°, the median of the diffuse light's μ_V
      plus 2.5 log₁₀ A_∞ within 23.82 ± 0.44. That is 250 nW m⁻² sr⁻¹ per magnitude of A_V,
      ×/÷ 1.5: Kawara et al. 2017, Ienaka et al. 2013, Matsuoka et al. 2011, Brandt and Draine
      2012 and Postman et al. 2024, through Schlafly and Finkbeiner 2011's 0.0505 mag per MJy
      sr⁻¹. The ruling's model of the fixture gives about 24.0. The slope's median at |b| 30–40°
      over that above 70° lies in 1.1–1.8 (about 1.3);
    - T9.c's and T9.d's references, re-derived with Gaia's ISL plus the diffuse light (T9.c,
      T9.d).
  - Record:
    - the fixture's diffuse light over its starlight in Toller's bins, over the whole sky
      (Leinert et al. 1998: typically 20–30%), and in the year-mean zenith at 40° N (Masana et
      al. 2021, Table 4: 0.13, modelled);
    - over the eye's background at the poles and in the band;
    - the iteration count, the 16² illumination against 8², and T9.d's pad with the light.
    - The ruling's model of the fixture gives 0.59 to 0.30 by bin and 0.47 over the sky, about
      twice the real sky's, from its dust (Risks, "The fixture's dust is thick for the diffuse
      light").
  - Bench: `sky/illumination`, and `sky/band_near_sun/march` with and without it. Gate,
    provisional: the illumination and the march's increase together at most 10% of the march
    without them, in one run (the ruling's estimate: 6–9%).
  - Files: `sky/{dgl,band,limits,mod}.rs`, `sky/census/query.rs`, `benches/sky.rs`. Acceptance:
    `cargo test -p hyperion-sim sky::dgl`, `cargo test -p hyperion-sim sky::band` and
    `cargo test -p hyperion-sim sky::limits`, as three commands.
  - As built (Risks, "Deviations in T9.g, as built"): the illumination is marched by rows as a
    request's jobs and assembled (`Illumination::march_rows`, `Illumination::assemble`); the
    scattered field stops on the bounded rule, its change × L ÷ (1 − L) under 10⁻⁴; Toller's bins
    are tested at his 440 nm within ×/÷ 1.5 of his 1σ range, and in V within 5% of the ruling's
    figures; and the fixture gives 6.35, 7.47 and a cut of 8.18, since its darkest polar texels
    hold little dust. Also `sky/colour.rs`, `sky/testing.rs` and `.config/nextest.toml`.

Files: `sky/band.rs`, `sky/limits.rs`. Bench: `sky/band_near_sun` (all six faces; since R06.T9.f
`sky/band_near_sun/march` and `/sum`) and `sky/limit_map` (R06.T9.i).

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
  `resolve`, and no `eye` with a `cone`), the request's illumination (R06.T9.g: its 1,536 rays as
  bulk jobs, then its fixed point), then the eye's cut by `eye_cut` with it and the request's cut as
  the deeper of it and `camera_limit_v`, the census as `Priority::Bulk` jobs of a few hundred
  cells each under the request's `CancelToken`, merged once all finish; the census never enters
  the interactive queue,
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
  sky's jobs run is answered first; `n_max` above the cap is `BadRequest` naming `n_max`; a
  request with both `eye` and `cone` is `BadRequest` naming `cone`; a sky
  near the Sun lists `feature_members` in `not_modelled`. Acceptance:
  `cargo test -p hyperion-server --test sky`. Each star's wire chroma, eye offset and camera band
  term are its `StarColour::reddened(a_v)`'s (R06.T9.e). Until T11.c builds the limit map, its eye
  offset is its colour offset alone, against a scotopic background. From T11.c it is
  `sky::limits::eye_offsets`', its own eye limit less its texel's (R06.T9.h; decided 2026-10-06,
  `decision-r06-t9c-glare.md`), quantised once. The wire field's doc comments say so:
  `hyperion-protocol`'s `sky.rs` table, `@hyperion/protocol`'s `eyeOffsetMag`, the server's
  `SkyStarWire` and `view/sky/cull.ts`. No bytes change. The wire's V is the star's own, M_V + DM +
  v★(A_V) A_V, which the census cuts, and its camera term is relative to it (addendum item 4).
  The protocol's doc comments on `SkyRequest.cone` and `ConeDto` call a cone an instrument's
  field stop, refused with the eye (`decision-r06-t8k-cone.md`). As built: Risks, "Deviations in
  T11.a, as built".
- **R06.T11.b Transfer.** The response and its payload through R03's `BulkPayload::new` and
  `Answer { body, bulk }` (whose `frames` call `bulk::chunk`), the stars then the band, split by
  `stars_bytes` and `band_bytes`, with `BulkPayload`'s `expect(dead_code)` removed; the per-cell
  `ByteLru` of Design note 12 under `HYPERION_SKY_CACHE_MB` (`config.rs`), behind a lock (a
  `SharedByteLru`), with its counters on `ServerStats`. Tests: the manifest matches what was sent
  (`TestClient::next_binary`); the cache's config reads its environment variable. R03.T15's
  pending 15 MiB transfer check in the real renderer is run with this kind, hidden, as R03's Risks
  ask, and recorded. Acceptance: `cargo test -p hyperion-server sky` and `just ci`.
- **R06.T11.c The band, the limits, the discs and the tables.** The band as bulk jobs by face and
  row through T9.f's `march_rows` on the query that states the request's illumination (R06.T9.g),
  then `sum_rows`, then the limit map, against the eye's background and with T9.j's glare at the
  request's eye cut, and each listed star's eye offset (R06.T9.h), then `host_discs` of
  `exclude_system` at the request's time; the luminosity tables built once per galaxy, keyed by
  `GalaxyKey` alone, under `SingleFlight` in a `ByteLru` of their own budget,
  `HYPERION_SKY_TABLES_MB` (default 160, two galaxies; separate from `HYPERION_SKY_CACHE_MB`), as
  `Priority::Bulk` pool jobs from `LuminosityTables::plan` (the envelope is the fitted table, with
  nothing to build), and the caps as ray-chunk pool jobs (decided 2026-10-03,
  `decision-r06-tables.md`). Tests: a sky near the Sun returns the stars, texels and host discs the
  sim returns for the same query; a second identical request shares the tables' build; a second
  sky in another time bucket shares the build. **The landing switch** (decided 2026-10-07 by the
  orchestrator): T11.a, T11.b and T11.c land together after T11.c, behind a server switch that
  T11.c adds. It is off by default, and the server then answers `sky` as it did before T11.a
  (`unsupported`, T10's interim), until R06.T11.d turns it on (decided 2026-10-08,
  `decision-p11-t17c-bright.md`; first ruled to turn on with T8.g). Tests turn it on. Test: with the
  switch off, `sky` is answered `unsupported` and no job reaches the pool. Acceptance:
  `cargo test -p hyperion-server --test sky`. Each star's wire chroma, eye offset and camera band
  term are its `StarColour::reddened(a_v)`'s (R06.T9.e). Its eye offset is
  `sky::limits::eye_offsets`', its own eye limit less its texel's (R06.T9.h; decided 2026-10-06,
  `decision-r06-t9c-glare.md`). The wire's V is the star's own, M_V + DM + v★(A_V) A_V, which the
  census cuts, and its camera term is relative to it (addendum item 4). As built: Risks,
  "Deviations in T11.c, as built" (the switch is `--serve-sky`, `HYPERION_SERVE_SKY`, off).
- **R06.T11.d Delivery nearest first (new; after T8.g, T8.i, T10 and T11.a–c; signed off).** Decided
  2026-10-05 (`decision-r06-census-cost.md`). The sign-off was advised by a decision agent and
  adopted on 2026-10-05 under the owner's standing delegation
  (`decision-r06-census-cost-signoff.md`, question 2), with its amendments here. The server runs a
  sky request's census shell by shell as bulk jobs. After each shell it sends a sky reply with that
  census and the band for that completeness, and it marks the last reply final. The client
  replaces its sky with each reply (T12, T13).
  - **The default** (decided 2026-10-08, `decision-p11-t17c-bright.md`). T11.d's landing turns
    T11.c's switch on by default if two things hold:
    - near the Sun, its first reply meets T17's first-sky budget on the dev machine (the test
      below);
    - the owner has accepted the full sky's time at version 21 (`decision-p11-t17c-bright.md` §4).

    Turning it on means three things. `SkyService`'s default becomes `Served`. `--serve-sky` takes
    a value (`--serve-sky=false`), so that it can still be turned off. And the README and
    `config`'s tests follow (Risks, "Deviations in T11.c, as built"). Otherwise the switch stays
    off, and the shortfall goes to the orchestrator. If the first reply misses its budget, a
    decision agent rules from the measured figure. Its levers are the order of the first reply's
    layers and the first shell's edge.

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
    at 1280 × 720 and in the instrument slots needs the running client. _Split off on 2026-10-08
    as R06.T11.f, below, with the guide rows' draft; T11.d builds the server, the protocol and the
    client's whole-reply swap and coalescing._
  - **The default switch and the interim** (decided by the owner 2026-10-08, answering
    `decision-p11-t17c-bright.md` §4: the hybrid sky, `feasibility-hybrid-sky.md` §10–11). The
    owner's condition for turning T11.c's switch on is met by RM3's interim, R13.T2: layers C to E
    capped at V_P 5.0, the band for the rest, labelled. Once R13.T2 and T11.d have both landed,
    the server's sky is on by default: `SkyService`'s default becomes `Served`, and `--serve-sky`
    takes a value (`--serve-sky=false`) so that it can still be turned off. The README and
    `config`'s tests follow. The first reply's time is still measured against T17's first-sky
    budget. A miss goes to a decision agent, whose levers are the first reply's layer order and
    the first shell's edge.

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

  T11.d's first items are T11.c's hookups (Risks, "Deviations in T11.c, as built", "Not done
  here"): R06.T9.g's illumination, R06.T7.b's per-ray radii and its eye visibility map, the near-Sun
  test's replies, and the bench `sky_near_sun_cold`.

  The server passes `camera_limit_v` as asked, neither deepening nor padding it. It sets
  `eye_visibility` only on a request with no camera part, never by comparing cuts: a camera at
  120° is shallower than the eye's cut, and the eye's visibility caps would not cover its cull
  (`decision-r06-t7b-brackets.md`).

- **R06.T11.f The stars-arriving note (new; split from T11.d on 2026-10-08; after T11.d).** T11.d's
  label bullet, built on the client to the sign-off's condition 4
  (`decision-r06-census-cost-signoff.md`, question 2). Named T11.f because R06.T17 already names a
  possible R06.T11.e, the disk cache. T11.d delivers each reply with `final` and each layer's
  `complete_to_ly` and `final`, and `useSky`'s model holds them; nothing reads them yet.
  - **The line.** `view/sky/label.ts`'s `skyLabelValue` takes the streaming edge: while the held
    reply is not final, `STARS V <m> mag EYE|CAM · BEYOND <edge> ly: STREAMING`, the note in the
    composed-note slot before any `· CLUSTERS AND WHITE DWARFS: NOT YET MODELLED`. Its figure is
    the least `complete_to_ly` over the layers not yet final (the sim's `least_edge()`, a fixed
    shell edge), in the guide's digit grouping (`2000 ly`, `16,000 ly`) in a field sized for the
    largest edge. It is steady, in `--text`, with no status colour, and clears by itself when the
    final reply is held. Each instrument's block carries its own (`InstrumentView`).
  - **Before the first reply** the line reads `STARS: PENDING`, as `LIGHTING: PENDING` does, in
    place of R02's interim reading, including after a jump. `useSky`'s `pending` (true until the
    request's first reply is held, since T11.d) says when.
  - **On loss of the link** the radius takes its `S` (the model's `stale`) and the note holds until
    a final reply; T11.d's request rule asks again for a sky held at a reply that is not final
    (`SkyRequestReason` `"partial"`).
  - **Never** a spinner, skeleton or progress bar; stars appear by a cut, with no fade-in (the
    bake's whole-cube swap, T11.d).
  - **The guide rows** (`docs/frontend/ux-guidelines.md`, the `STARS` row and a `STREAMING` and
    `PENDING` entry beside `TERRAIN: STREAMING`) are drafted here and go to the UX decision agent
    for the owner's sign-off, as T15's rows did. Build to the draft.
  - Tests: the line's readings (pending, streaming at each fixed edge, final, with and without the
    not-modelled note, stale) and the label block's line in `ViewDisplay` and an instrument's.
    Pending by hand for the owner: whether the composed line fits the label block at 1280 × 720 and
    in the instrument slots, in the running client. R07.T19.f was editing `ViewLabelBlock.tsx` and
    `CameraControls.tsx` when this was split: merge it first.
  - Acceptance: the client's tests of `view/sky` and `displays/view`
    (`pnpm --filter hyperion exec vitest run src/renderer/src/view/sky src/renderer/src/displays/view`),
    the UX review, and `just ci`.

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
the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc
the_census_is_its_oracle_150_ly_from_the_sun_in_motion phase_envelope_bounds_dense_tracks
the_star_bound_holds_for_realised_systems`,
`just bench -- sky`) and record the
figures in the doc comments that own them and in this plan: the caps (at the six points of
`caps_converge_in_rays`, against the brainstorm's C 3,000, D 4,300, E 10,000 ly near the Sun and
"a few hundred to about 1,000" in the nuclear disc, with what sets each: C the M_V −2 to −4 AGB tips
and post-AGB crossings, D post-AGB and bright giants through clear windows, E supergiants; since
R06.T7.b, for each layer, the sphere at the same count (`CapCount::spheres`, the radius the
brainstorm's figures describe), the rays' median, 10th and 90th percentiles and largest, the median
of the rays within 10° of the plane, and the largest ray's galactic longitude and latitude
(`decision-r06-t7b-brackets.md`; a D or E ray beyond 10,000 ly at |b| above 30° is a finding for the
discs' vertical profiles or the tables, deferred, not for the caps); handed
to the brainstorm's sky section and open question 19, with C's post-AGB re-derivation and D's
post-AGB count; and `layer_caps`'s CPU time per call near the Sun and in the inner bulge, 4,608
`Full` realised profiles since R06.T7.b (1,536 rays, three sub-rays each): above 10% of the
census's CPU time in either bench, propose fewer rays
with a finer convergence proof or a coarser quality the slow test still passes), candidates opened, CPU-seconds and listed stars near the Sun and in the inner
bulge, re-deriving open question 19's counts at the current version and explaining why candidates exceed the
systems layers C to E hold; the candidates the binary rule of T16.b costs; check the per-layer
counts against `range_500ly_floor_d` (37,675 systems at version 15, re-measured at the current version). Add goldens:
`crates/hyperion-sim/tests/golden/sky/census_near_sun.golden`, the census of a pinned observer near
the Sun to V 7 (IDs, star indices, V to 10⁻⁶ mag), `sky/band_face_row.golden`, one band face
row with its eye limits after `limit_rows` and the eye offsets of its listed stars, generated after
R06.T9.i and T9.j (whose bits it pins; `decision-r06-t9c-glare.md`), against the glare of a census
with stars within 1° and near 90° of its texels (the limit map's bits across targets, which no
other golden pins; determinism audit of R06.T9.c), and `sky/colour_reddened.golden`, every field of `reddened` at A_V 0.5, 2 and 5, off the
nodes at 1, 7, 17 and 25, and held at 40, for a few points on both grids (the per-star reddening
T11's wire carries, which no other golden pins; determinism audit of R06.T9.e and the band
ruling's addendum), and `sky/eye_cut.golden`, each pre-pass's darkest limit and cut and the eye's
cut near the Sun (a repeat) and in the nuclear disc (none), whose bits set every eye reply's
census and no other golden pins (determinism audit of R06.T9.d), read by the testkit's golden
harness. The goldens `sky/band_face_row.golden` and `sky/eye_cut.golden` are written with the
illumination. Record the A_V distribution of the
listed stars (the shares above A_V 2, 5, 10 and 20) in each census bench, near the Sun at both
cuts and in the inner bulge (`decision-r06-t9b-band.md`, addendum item 2). Record the per-record bound's pass rate (records
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
eye's cut as T9.d computes it (about 8.0 on the fixture with the diffuse light, R06.T9.g; 8.28
before it; 8.18 as built, Risks, "Deviations in T9.g, as built"), benched also at 7.95, its
estimate when
the budget was set (decided 2026-10-06, `decision-r06-t9b-band.md`; the eye's cut; the camera's
cut, 10.06 at 60°, is benched beside it and its budget ruled from that figure,
`decision-r06-census-cost-signoff.md`), at the current caps, on a quiet machine:

- the first sky in at most 150 CPU-s and 10 s wall on the dev machine's default workers;
- the full cold census at the hybrid's real boundary (R13.T2, RM3's interim: layers C to E at
  T7.b's caps by ray computed at V_P 5.0, the band for the rest; decided by the owner 2026-10-08,
  `feasibility-hybrid-sky.md` §10–11): recorded, not gated.
  - About 1.0–1.25 × 10⁴ CPU-s is estimated near the Sun at the eye's cut with spherical caps,
    and less by ray.
  - The ruled 4,000 CPU-s stays the target, for R13's V_P of 4.5 (about 4.5–5.8 × 10³
    estimated).
  - The exact census to the cut's caps, about 0.4–1.0 × 10⁶ CPU-s at P11.T17.c's verdicts
    (`decision-p11-t17c-bright.md` §2), is recorded beside it, sampled, as R13.T1's reference. Its
    first gate, 2.5 × 10⁴ CPU-s at version 21 (decided 2026-10-07,
    `decision-p11-t16-hierarchy-bound.md`), is withdrawn: P11.T17.c's tables put it out of reach;

- after a jump of up to 1,000 ly, at the default `HYPERION_SKY_CACHE_MB` (R06.T8.n):
  - every cell both plans open is served from the cache, none rebuilt;
  - the warm census's stars and tallies are the destination's cold census's, bit for bit;
  - one near-Sun sky's entries and the jump's fit the default.

  This is the gate (decided 2026-10-08, `decision-r06-t8h-warm.md`). Warm ÷ cold, sampled with
  the cache budget scaled, is recorded against the target of at most 25%, with the repeat's.
  About 30–45% is expected at version 21 after P11.T17.c, since a warm census still generates
  every system its bound passes, and a jump's new cells are cold. A miss of the target is
  deferred (`deferred-corrections.md`, "Census cost": lever 13, then the realised window bound)
  and does not hold T17. Above 50%, it is reported before T17 closes.

The budget was accepted as the eye's only. A decision agent advised that, and the advice was
adopted on 2026-10-05 under the owner's standing delegation (`decision-r06-census-cost-signoff.md`,
question 1). A decision agent rules the camera's budget from the cut-10.06 bench, and reports it to
the owner, before T17 closes and before any bridge play-test.

Run the census benches sampled (`HYPERION_SKY_BENCH_SAMPLE`), at the eye's cut as T9.d computes it
and at 7.95 (`decision-r06-t9b-band.md`), and beside them at 10.06, with, near the Sun, the caps by
ray at the camera's black-sky limit, 10.15 at 60°, and the systems they open, from `layer_caps`
alone, and Design note 18's camera limit over the final 64² band at the camera's cut with the
illumination, for the camera-budget ruling (`decision-r06-t7b-brackets.md`); and once whole if the
sampled
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
- the per-reply cost of the band and the limit map (T9.i's pyramid; `sky/limit_map` at 300,000
  stars). The first reply's band, limit map and shell
  together must fit the first-sky budget;
- the illumination's cost per request, inside the first sky's budget;
- the sky's total light (listed, overflow and band) in the first reply against the final's, at
  the eye's cut near the Sun, within 1% (T9.b's amendment, decided 2026-10-06,
  `decision-r06-t9b-band.md`); a miss is a finding for R06.T5.f, not a looser gate;
- near the Sun, the expected count of stars brighter than the eye's cut between the eye-only caps
  and the camera's, per layer: the bound on what a camera's census adds to the eye's sky
  (`decision-r06-t9c-glare.md`, addendum 2). Where a whole camera census runs, the realised count,
  and how many of those stars the eye sees;
- the delivery time of the final reply's stars brighter than V 3.0, against 25% of the full
  census's wall time;
- R03.T15's transfer check at the sky's largest payload: a real sky near the Sun at N_max
  (3 × 10⁵ stars with the band; 7,494,912 bytes, 29 chunks), with T11.c's switch on, asked in the
  real renderer, hidden. Its chunks arrive in order before the response, and its bytes are the
  server's. T11.b ran the check at 4 chunks, which closes T11.b's part (decided 2026-10-07 by the
  orchestrator; R06's Risks, "Deviations in T11.b, as built").

Every figure states its cut and its machine: "10 s" alone is the dev machine at the eye's cut. The
nuclear-disc bench is recorded, with no budget. At version 21 the full cold census's 4,000 CPU-s
target is not a gate. Its step 1 below is taken by deferral: the asks are in
`deferred-corrections.md` as lever 13. They are plan 11's cheaper hierarchy draw at a version bump,
and faster post-main-sequence tracks and pair evolution. Steps 2 and 3 wait for that re-measure,
except that T11.e's laptop trigger stands. If a budget is missed, in this order:

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
T8.m's `sky_phase_envelope` join the check list (`just fit-check`). Each timing is taken on a
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
  `sky/band_near_sun/march` and `/sum` (R06.T9.f), `sky_near_sun_cold` (server). The census
  benches run sampled (`HYPERION_SKY_BENCH_SAMPLE`), against the budget of
  `decision-r06-census-cost.md`, at the eye's cut and beside it at the camera's
  (`decision-r06-census-cost-signoff.md`).
- **By hand, recorded:** the star field's GPU time on both machines, no flicker, the band's lanes,
  the discs, several views sharing one cube.

## Generator version

No change to generated output and no bump. `generate_cell_where` is `generate_cell` filtered, and
the goldens prove it; the luminosity tables, the envelope, the caps and the census only read. The
sky's own output is a function of the generator version and of the committed colour (with its
reddening, `star_colour_reddening` and its node files `_av02_05`, `_av07p5`, `_av10_15` and
`_av20_30`, T9.e),
limb-darkening, envelope (`sky_envelope`, T6.b) and
pair-evolved light (`sky_binary_light_c`, `_d` and `_e`, T5.d) tables, so its goldens (T17) are
regenerated whenever any of them moves. The reddening tables took effect at generator version 20
with no bump (at revision 1, the band ruling's addendum): nothing generated reads them, and no
golden pins the sky before T17. The
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
`sky_phase_envelope` (T8.m) joins fit-check. Plan 11's `hierarchy_bound` and `pair_light_bound`
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
  a dark sky of μ 24 (`DARK_SKY_CD_M2`) until the band layer (T13.d) gives a texel's background (not
  wired as of R06.T7.b; when the cull reads it, the request must ask the black-sky limit in the same
  change, `decision-r06-t7b-brackets.md`), and at a manual exposure's triple or else R02's default
  `MAN` triple until R07 states the metered triple (R07.T13.e replaced `limitTriple`: the request
  and the cull take the view camera's deepest triple at every exposure, `deepestTriple`, and the
  label the shown exposure's, `viewSkyLabelV`). Until T13.g bakes the cube, the stars beyond the
  sprite budget are not drawn, and the high setting's N_max and sprite budget are used (_since
  R07.T17 the setting's own, and its cube's face size where the device blends `float32`_), the view
  not yet taking a quality setting. `useSky` makes its decoder only once a payload is in hand (and
  the effect still live), so that a request never answered starts no worker. A sky is drawn and
  labelled only for the system it was asked about while that system's position is known; otherwise
  the interim field and its labels stand. `just test-render` compiles `STAR SPRITES HDR`
  (2026-10-03, exit 0).
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
- **The hierarchy bound's cost, and the full census at version 21 (decided 2026-10-07,
  `decision-p11-t16-hierarchy-bound.md`).**
  - Plan 11's ask A cannot be exact at 3 µs, and it must be exact:
    - Without the period, a companion's q is known only to a factor of about 2.
    - Without the stability test, the kept try is unknown: every companion keeps its first try in
      75% of D's multiples and 25% of E's.
    - So an interval bound collapses to the running maximum over companion masses, and leaves no
      periastron for P11.T17.
  - P11.T16 is the generator's own draw at each attempt it can keep: about C 8.5–13, D 45 and
    E 180 µs a record with the carve cover.
  - **The full census.** Near the Sun at T7.b's caps, after T8.g, it is about 1.1–1.9 × 10⁴ CPU-s.
    That is about 13–21 minutes on 15 workers, and about an hour on a 4-core laptop. The first
    sky stays within 10 s, nearest first.
  - The 4,000 CPU-s needs the deferred lever 13 (P11's cheaper draw at a version bump, and faster
    generation), or a pass rate under about 0.3%.
  - T8.g cannot leave RM3's path. Without it the census is 1.83 × 10⁶ CPU-s (about 7 × 10⁵ with
    T7.b), and the 500 ly first shell is about 375 CPU-s.
  - Without P11.T17.b's tables, T8.g barely bounds E: 90% of E's records hold a pair with a
    remnant, and E is 73% of the census's CPU.
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
  setting absorbs it. Its likeliest part is the far field (science check of R06.T9.c): all the stars brighter than V
  8.15 sum to about V −5.4 for the real sky, which spread evenly veil the fixture's poles by some
  12%, −0.04 to −0.06 mag on their limits. The pupil's plane (R06.T9.h) makes the far field about
  0.71 times the as-built one, so this double count is about a third smaller, some −0.03 to −0.04
  mag at the poles. CIE's 90°–100° range, light past the cornea and through the eye wall, takes no
  illuminance in the eye's plane, and is left out. The two readings of E differ in all by
  0.010–0.015 mag at the poles (decided 2026-10-06, `decision-r06-t9c-glare.md`).
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
  term would cost more census than those stars are worth (decision-camera-eta.md). Reddening
  widens it (decided 2026-10-06, `decision-r06-t9b-band.md`, addendum): the camera term reaches
  about −3 mag at A_V 10 and −8 at A_V 20, so a camera towards the galactic centre misses many
  near-infrared-bright stars, and the band carries their light at its photopic colour. For the
  camera's budget decision before any bridge play-test (T17).
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
  correction in the roadmap. _Superseded 2026-10-06: T9.d's reference is 8.15 ± 0.22
  (`decision-r06-t9b-band.md`, item 1), and the largest colour offset is 0.453, so the cut is at
  most 7.99 + 0.453 + 0.1 = 8.54 (R06.T9.d)._
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
  as the fallback. _R06.T5.f (2026-10-07, version 21) measured T5.c's solar-circle sample on
  more cells: C 2.56% ± 0.49 on 165,055 systems and D 8.91% ± 0.53 on 1,062,507, against the
  fit's 2.01% and 8.67% on the same cells; T5.c's 9% and 21% were its one block's scatter (Risks,
  "R06.T5.f's measurements, as built")._
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
    `sky_binary_light_c`, `_d` and `_e` (class Slow, revision 0, since generator version 20;
    refitted at 21 to 176, 172 and 211 kB, Risks, "Generator version 21"),
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
    caps ruling, so that fit-check sees plan 11's protostar fix; moved to cells that pair
    evolution still changes at 21, Risks, "Generator version 21"): each of a bin's six summed
    differences, the light difference times its bin's index (where the light lands), its
    absolute value and the systems changed. About 1.5 s a layer in fit-check (2.5, 4.0 and 2.2 s
    with the probe cells of 21).
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
    0.43; the table is bilinear in ρ, so no colour exceeds its rows. _It is 0.4526, ρ 3.4850, the
    500,000 K blackbody rows (R06.T9.d)._
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
    _Tested by R06.T8.j (2026-10-07): in the fixture with its kinematic tables, 63,715 records
    seen by four observers moved at most 295–463 km/s by layer (E the fastest), each within its
    cell's pad, and every one seen within its cap was in a planned cell._
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
    rule's in distance; twelve nodes a decade agree with 48 to 0.02 mag (_per texel, R06.T9.f
    measured up to 0.012–0.026 mag near the Sun at 8², and 0.005–0.007 over the band; Risks,
    "Deviations in T9.f, as built"_). The light nearer than 0.01 ly, at most some 10⁻⁵ of a ray's,
    is left out. A ray outside a query's cone is complete nowhere.
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
      24.25–24.29 at the poles. Ruled 22.05 and 24.3 (`decision-r06-t9b-band.md`); R06.T9.e moved
      the test's plane reference to 22.05, and reddened the band reads 22.07 and 24.56;
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
  - Not built: `eye_limit` stays `None` (T9.c sets it, and will need a crate-visible setter;
    built in T9.c as `set_eye_limit`); `sky/limits.rs` (T9.c, T9.d).
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

  _Measured in R06.T5.f (2026-10-07; Risks, "R06.T5.f's measurements, as built"): the Sun's
  place holds 82% of its light within 200 ly against its own skew's median of 91% (interquartile
  83–102%), and eight observers' median holds 97% against the skew's 88%. D's 0.75 is the counts'
  pair deficit, which the shipped counts leave out by design (1.013 ± 0.013 with it). C's 0.95 is
  0.966 ± 0.004 with it, and lies in the single-star tables._

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
    change). _Built in R06.T8.k (2026-10-07): near the Sun to V 8.15 within 200 ly the eye's
    census listed 101 stars fainter than the cut and missed 43 brighter; it now lists the no-eye
    census's 4,823, bit for bit (Risks, "Deviations in T8.k, as built")._
  - **A cone (decided 2026-10-06, `decision-r06-t9b-band.md`: R06.T8.k lists only the stars
    inside the cone).** A ray outside the cone is complete nowhere, but the census lists every
    star of each cell whose padded ball meets the cone, some outside it, whose light the band then
    holds too, as the caps' overshoot is held. Options: accept it; or have the census list only
    the stars inside the cone (a T8 change). _Built in R06.T8.k (2026-10-07): a 30° cone's cells
    within 200 ly of the Sun hold 424 stars brighter than V 8 outside it, 20% of the light of the
    290 inside, which its census no longer lists._
  - **The final reply's cells.** A final census lists every star of the cells it opens, some just
    beyond its caps, whose light the band also holds, within the caps' stated expected count
    beyond, as the sign-off's condition 2 accepts.
  - **One march per call.** `band_rows` marches its rays on every call, and the radius nodes move
    every layer's nodes. T11.d's one march a request, re-summed per reply (sign-off condition 5),
    needs the march split from the sums: each layer's running fainter and all-light sums at fixed
    nodes with the shell edges among them, then a cheap sum per reply. R06.T9.f builds it (decided
    2026-10-06, `decision-r06-t9b-band.md`); T11.c and T11.d consume it. _Built in R06.T9.f
    (2026-10-07): `march_rows` keeps every reply's radii, `sum_rows` sums one reply (Risks,
    "Deviations in T9.f, as built")._
- **Pending rulings from T9.b.** Ruled 2026-10-06 (`decision-r06-t9b-band.md`):
  - the poles 24.3 and the plane 22.05;
  - T9.c 6.5 ± 0.20 and 7.55 ± 0.22, and T9.d 8.15 ± 0.22;
  - reddening (R06.T9.e);
  - the census keeps to the cut (R06.T8.k);
  - the independence covers the radii a reply states (T17);
  - the march split (R06.T9.f);
  - the shortfall measured (R06.T5.f);
  - the diffuse galactic light (R06.T9.g; modelled 2026-10-07, `decision-r06-t9g-dgl.md`).
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

- **The diffuse galactic light (found 2026-10-06, `decision-r06-t9b-band.md`; modelled
  2026-10-07, `decision-r06-t9g-dgl.md`; R06.T9.g).**
  - **The model.** The band holds the starlight its dust scatters: the observer's own sky as each
    scattering point's field, with the higher orders weighted by the ray's depth.
  - **Its accuracy.** Against an exact plane-parallel solution near the Sun it is within −8% to
    +11% at every latitude, for the fixture's dust and for a realistic layer. The plain local
    field's series is 19–49% high at high latitude, and single scattering alone 18–31% low in the
    band.
  - **Against the measurements,** on a realistic layer: Toller's ratios within a factor 1.5 in
    every latitude bin (0.62–1.36; _as built, 0.62 is a factor of 1.61: in V the model is 1.5–1.6
    times low of his central values at 5–15° and 20–30°, and at his 440 nm within ×/÷ 1.5 of
    his 1σ range in every bin, 0.78–1.17 of his central values but 1.75 in the plane's; Risks,
    "Deviations in T9.g, as built"_); the year-mean zenith at 40° N within 1.6 (0.20, against
    Masana et al.'s modelled 0.13); and the measured slopes per magnitude of dust at the top of
    their spread. Those slopes are 1.3–1.8 times their geometric mean at |b| 30–90°, within
    1.3 of Kawara et al.'s and Pioneer's, and about twice New Horizons'.
  - **The phase function's g.** Draine's g of about 0.54 is below most fitted optical values
    (0.6–0.8; Gordon 2004, Mattila et al. 2018):
    - g 0.7 with ω 0.65 would lower the high-latitude light per magnitude of dust by about a
      quarter, and miss Toller's band bin;
    - g 0.8 with ω 0.58 would lower it by about 45%, and miss his bins at 20–60°.
  - **Left out:**
    - extended red emission, some 20–30% of the red diffuse light (Witt et al. 2008; Chellew et
      al. 2022);
    - reflection nebulae about listed stars;
    - the field's departure from the observer's own off the disc and inside clouds;
    - the realised stars' light: the field is the tables' expected light, some 20% above the
      fixture's realised sky (Risks, "The galaxy's local light is low").
  - **Its effect.** Near the Sun it moves the real sky's eye limits by about −0.04 at the poles
    and −0.09 in the band, and T9.d's cut by about −0.05.
  - **The upgrade,** if the galaxy plans keep a thick dust layer: the field at a few heights
    above and below the observer, at 2–5 CPU-s.
- **The fixture's dust is thick for the diffuse light (found 2026-10-07,
  `decision-r06-t9g-dgl.md`; a pointer for galaxy plan 07's owner).**
  - **The fixture:** its neutral layer is 700 ly high (`GasDiscParams::HEIGHT`), and its dust
    column to the poles is 0.25 mag (P07.T8).
  - **The real sky:**
    - the dust's scale height near the Sun is 125–135 pc (Marshall et al. 2006; Drimmel and
      Spergel 2001);
    - Schlafly and Finkbeiner's (2011) A_V is 0.032 at the north galactic pole, 0.042 at the
      south and about 0.066 at (l 230°, b +79°), the darkest Gaia texel;
    - the Sun lies in the low-density Local Bubble, which the fixture lacks.
  - **The diffuse light:** the fixture's is about 2–3 times Toller's ratios by latitude (0.59 to
    0.30, against 0.21–0.34 to 0.12). At the poles it is about 0.7 of the eye's background,
    against about 0.09 for the real sky. _As built: 0.43 to 0.24 by bin, 0.41 over the sky, and
    0.54 of the eye's background at the poles (Risks, "Deviations in T9.g, as built")._
  - **The limits:** the fixture's polar eye limits move by about −0.27 mag, against −0.04, and
    its eye's cut falls to about 8.0. _As built: −0.23 at the poles (7.693 to 7.468, the band alone)
    and −0.20 in the band (6.545 to 6.347), and the cut to 8.18, since the darkest polar texels
    hold little dust._
  - R06's references are the real sky's; the fixture's offset is recorded and routed. The same
    column reddens the band's poles and every nearby star's sightline.
- **Deviations in T9.g, as built (2026-10-07).** `sky::dgl`, the diffuse galactic light, as
  `decision-r06-t9g-dgl.md` and its plan text set it out, with these details.
  - **The signatures.**
    - `Illumination::march(galaxy, ctx, observer)`, and by rows for the server's jobs:
      `Illumination::march_rows(galaxy, ctx, observer, face, rows) -> IlluminationRows`, then
      `Illumination::assemble(observer, parts)`. The parts may come in any order and any split. It
      panics unless each texel comes once, for the observer.
    - `observer()`, `iterations()`, `heap_bytes()`; `ILLUMINATION_SPEC`, which the eye cut's
      pre-pass now takes as its own.
    - `SkyQueryBuilder::illumination(Arc<Illumination>)` and `SkyQuery::illumination()`. The new
      `BuildSkyQueryError::Illumination` refuses one marched for another place or time.
    - `eye_cut(galaxy, ctx, observer, eye, illumination: Option<&Illumination>)`. Its pre-pass
      holds the illumination by reference, so it calls a crate `band_rows_lit`, not `band_rows`.
    - `BandTexel::diffuse_luminance()`. `march_rows` asserts, in release builds too, that an
      illumination's observer is the query's.
  - **The scattered field's stop (main's ruling, 2026-10-07).**
    - The ruling's literal stop, the change under 10⁻⁴ of the largest photopic S, leaves about
      2 × 10⁻⁴ behind thick dust, where the ratio is ω. That fails its own test 3.
    - Each sum stops instead when its largest change times L ÷ (1 − L) is under 10⁻⁴ of the
      largest photopic S, with L = ω × the largest |D|. So the error left is bounded by 10⁻⁴.
    - Each sum is iterated on its own kernel matrix, the photopic first: 1,536² weights, 19 MB
      while that sum runs, each cosine taken once a pair. Iterating all five together, as the
      ruling words it, recomputed the kernel every step and took 0.69 CPU-s, not 0.2.
    - Steps: 11 near the Sun, 7 on the plane-parallel sky, and 24 behind A 100, which leaves
      8.9 × 10⁻⁵.
  - **Toller's bins (main's rulings, 2026-10-07).**
    - The ruling's "within a factor 1.5 (0.62–1.36)" is an arithmetic slip: 0.62 is a factor of
      1.61.
    - Toller's ratios are at λ ≈ 440 nm (Leinert et al. 1998, §11 and Table 39, Pioneer 10's blue),
      but the ruling's model and its figures are V's (science check).
    - Test 9 so compares the plane-parallel sky at 440 nm with Toller, within ×/÷ 1.5 of his 1σ
      range (Table 39's ± values). The dust is × 1.3245 by the sim's law there, and Draine's ω
      0.666 and g 0.566 at 0.44 µm; the stars keep their V layers. In V it holds the ruling's own
      figures within 5%, a regression on the implementation.
    - In V the model is 1.5–1.6 times low against Toller's central values at 5–15° and 20–30°.
      At 440 nm it is 0.78–1.17 of them in seven bins, and 1.75 in the plane's (0–5°), still within
      ×/÷ 1.5 of his 1σ range there (0.368 against (0.21 + 0.05) × 1.5 = 0.39). So most of the
      gap was the band. What remains is the plane's bin, 1.75 high at 440 nm, which the stars'
      bluer light, lower in the disc than the V layers taken here, may move. Deferred
      (`deferred-corrections.md`), not tuned.
    - Scaling Toller's 0.21 to V by the model's own V-to-440 ratio in that bin (0.775) would move
      T9.c's band reference from 6.408 to 6.427, well inside its ±0.20. T9.c is unchanged.
    - The plane-parallel sky, as built (the ruling's figures beside the V row):

      | \|b\|           | 0–5°        | 5–10°       | 10–15°      | 15–20°      | 20–30°      | 30–40°      | 40–60°      | 60–90°      |
      | --------------- | ----------- | ----------- | ----------- | ----------- | ----------- | ----------- | ----------- | ----------- |
      | At 440 nm       | 0.368       | 0.272       | 0.241       | 0.223       | 0.202       | 0.178       | 0.155       | 0.135       |
      | Toller ± 1σ     | 0.21 ± 0.05 | 0.34 ± 0.07 | 0.31 ± 0.03 | 0.19 ± 0.04 | 0.25 ± 0.04 | 0.17 ± 0.04 | 0.17 ± 0.02 | 0.12 ± 0.02 |
      | In V            | 0.285       | 0.214       | 0.194       | 0.181       | 0.166       | 0.149       | 0.130       | 0.114       |
      | The ruling's, V | 0.286       | 0.213       | 0.192       | 0.180       | 0.165       | 0.147       | 0.129       | 0.113       |

      The year-mean zenith at 40° N in V is 0.205 (the ruling 0.20; Masana et al. 0.13, modelled).

    - **The sky's dust layer** (main's ruling, 2026-10-07): an exponential of scale height 125 pc,
      as the ruling took it, documented as the ruling's assumption. Marshall et al. 2006 (A&A 453,
      635, §5.5.1) give 125 (+17, −7) pc for a sech² profile and 134 (+44, −11) pc for an
      exponential, which moves the V bins by +4% to +7.5%, past the 5% regression; sech² at 125
      pc moves them by +6% in the plane's bin and −1% to −2% elsewhere (science check). Toller's
      check passes in every variant. The sky's real dust is the galaxy's, not this test sky's.
    - The light's split, 41% at 100 pc and 59% at 300 pc, makes the column Flynn et al.'s
      Σ<sub>L</sub> of 24.4 L☉ pc⁻²; the two heights and the observer's 20.8 pc are the ruling's.
  - **The scatterers.** Each sum's (λ; ω; g):
    - photopic 0.5536 µm, 0.6774, 0.5360;
    - red 0.6432, 0.6735, 0.5019;
    - green 0.5444, 0.6773, 0.5391;
    - blue 0.4430, 0.6668, 0.5651;
    - scotopic 0.4981, 0.6750, 0.5529.

    The red channel's moment, 0.838, falls at 0.643 µm, not the ruling's about 0.62, so its g is
    0.502, not about 0.51. The scotopic ω is 0.675, against the ruling's about 0.676. Test 1 holds
    the ruling's "about" figures to 0.002 in ω and 0.003 in g, and the channels within 0.666–0.678
    in ω and 0.495–0.570 in g: the blue's ω 0.6668 and the green's 0.6773 lie just past the plan's
    0.667–0.677, which are "about".
    - Draine's rows take the data file's own wavelengths (0.398107, 0.446684, 0.501187, 0.602560
      and 0.707946 µm, which the ruling rounded); over the band's wavelengths the twelve rows give
      the full table's ω and g within 1.5 × 10⁻⁴ (science check).
    - The kernel's half width at half maximum is about 28° at g 0.54, not the ruling's 23°; the
      16² illumination's own measure is the 8² comparison below.

  - **The tests' details.**
    - **Test 4** holds the linear law at A<sub>∞</sub> 10⁻³ (6.0 × 10⁻⁴). At 2 × 10⁻³ the blue's
      second-order term is 1.2 × 10⁻³, which the doubling's ratio (within 6.0 × 10⁻⁴) holds.
    - **Test 5** takes the kernel's ratio on the single-scattered light, since the higher orders
      add about 10⁻⁶ of it. Forward over 90° is 14.6; forward over backward is 35.9.
    - **Test 6.** A signed channel whose two parts the dust dims apart passes zero behind thick
      dust and comes back towards it, so its depth rises a hair past 1 and falls back. The blue's
      reaches 1 + 8.5 × 10⁻⁶; the green's reaches 1 + 1.06 × 10⁻¹⁰ past A 20, falling 8 × 10⁻¹²
      a step. The test holds the photopic, red, green and scotopic depths monotone within 10⁻⁹,
      and the blue's within 10⁻³ of 1.
    - **The midpoint sum** of the unrenormalised kernel is within 5.2 × 10⁻⁴ of 1 at 16² and 64²
      targets. The renormalised weights sum to 1 within 7.8 × 10⁻¹⁵. Test 2's J = F, exact, also
      reads the production means toward the 8² and 64² bands' texels.
    - **Test 10** reads the illumination's own 16² texels, their directions and A<sub>∞</sub>,
      which are the pre-pass's, rather than a lit 16² band, whose rays have the same bits.
    - **Added** (reviews): the scattered field's kernel against the direct phase-weighted means,
      bit for bit, on a 4² grid; `assemble`'s refusals of a texel marched twice and of one not
      marched; an illumination's bits whatever was marched before it on a warm context
      (`assert_order_independent`, the Sun and 1,000 ly above it), and the Sun's against a cold
      context's; and `eye_cut` without an illumination, the starlight's alone, 8.2819 with its
      repeat of +0.0425, against 8.1787 and +0.0274 with it.
  - **Near the Sun.**
    - The slope (test 10): μ<sub>V</sub> + 2.5 log₁₀ A<sub>∞</sub> is 23.633 over |b| > 40°,
      against 23.82 ± 0.44; the ruling's model gave about 24.0. Its fall from 30–40° to above 70°
      is 1.240 (1.1–1.8; the ruling 1.29). The light per magnitude of dust is about 1.4 times the
      ruling's model's because the field is: the fixture's all-sky mean starlight is μ<sub>V</sub>
      22.89, 591 nW m⁻² sr⁻¹ by the ruling's conversion, against its plane-parallel fixture's 433
      (the realistic sky's 762).
    - T9.c's medians with the light: 6.347 in the band and 7.468 at the poles from the band alone,
      and 6.347 and 7.378 with the glare of the 1,514 stars within 100 ly. The references are
      6.41 ± 0.20 and 7.51 ± 0.22; the ruling's fixture estimate was 6.29 and 7.41.
    - T9.d: the first pass gives 8.1513 and the repeat 8.1787 (+0.0274, under 0.05), against
      8.10 ± 0.22. The ruling expected about 8.0 and +0.01; T17's "about 8.0" and the T9.d
      record's "about +0.01" are its estimates, and T17 benches the census at the 8.18 the fixture
      computes.
    - The darkest 16² texel moved from |b| 78.8° (μ 24.916) to 75.1° (μ 24.740 with the light).
      There the diffuse light is about 18% of the background, not the ruling's 57–70%.
    - The reason: the darkest texel is the least dusty. The 16² rays at |b| over 60° hold
      A<sub>∞</sub> 0.054 to 0.472 (median 0.191). The ruling's plane-parallel fixture took 0.25
      everywhere. So the cut and the repeat stay nearer their unlit values.
    - The pad has 0.0956 to spare (0.0935 before).
    - Records (the ignored `record_the_fixtures_diffuse_light_near_the_sun`, 64², complete
      nowhere for the starlight). The diffuse light over the fixture's starlight by Toller's bins
      is 0.429, 0.481, 0.495, 0.468, 0.403, 0.319, 0.291 and 0.236; the ruling's plane-parallel
      fixture gave 0.587 to 0.301. It is 0.410 over the whole sky (the ruling 0.47) and 0.337 in
      the zenith mean at 40° N (the ruling 0.48; l = 90° along +X).
    - Over the eye's background (the starlight fainter than V 8.15) it is 0.540 at the poles and
      0.588 in the band; in a single 64² texel it reaches 2.17 (|b| 51.4°, behind a cloud).
    - The 8² illumination against the 16²: within 4.0% in a texel, 2.0% of the light over the
      sky, and −2.3% over the band (the ruling: within 3%).
  - **The cost** (criterion's medians of ten samples each, in one run, release: three workers
    at `CPUQuota=400%`, without the heavy-test lock, which the orchestrator's `just ci` held,
    load about 4–8, so provisional).
    - `sky/illumination` 0.947 CPU-s: its 1,536 rays and its scattered field, about 0.15 of it.
    - `sky/band_near_sun/march` 16.11 CPU-s, against 15.73 for `/march_no_dgl`.
    - So the illumination and the march's increase are 1.33 CPU-s, **8.4% of the march**, under
      the 10% gate (the ruling 6–9%).
    - Earlier single runs at load 7–17 gave 9.7% and noise either way: the march alone moves by
      up to 2 CPU-s between runs.
    - The phase-weighted means run four texels a step, their partial sums in four lanes joined
      ((0 + 1) + (2 + 3)), a fixed order. The march's heap at 64² grows by 0.98 MB, from 21.97 MB
      to 22.95 MB.
    - The pre-pass's own diffuse light (two 16² bands) adds about 2 × 1,536² kernel pairs to
      `eye_cut`; T17 times it with the first sky.
  - **Bits.**
    - A query with no illumination keeps today's bits: a probe's digests of the band, a march's
      two replies with a census's overflow, a camera's eye limits and the eye's cut are identical
      before and after (`.git/rm23-scratch/r06-sky/t9g/probe/`).
    - The march's heap grows by each ray's A<sub>∞</sub>, 8 bytes, lit or not.
    - The sim's own API keeps the units: A<sub>∞</sub> as `Magnitudes`, the texels' solid angles
      in sr, and each ray's direction as a `UnitVector` (Rust review).
    - GENERATOR_VERSION stays 21, and no golden moves.
  - **Files** beyond the plan's:
    - `sky/colour.rs`: `Reddening::sum_moments`, crate;
    - `sky/testing.rs`: the Sun's illumination, shared by the tests;
    - `.config/nextest.toml`: `sky::dgl` in the sky-tables group.
- **Deviations in T9.e, as built (2026-10-06; amended the same day to the band ruling's
  addendum).** `StarColour::reddened` and the band's five reddened sums, as
  `decision-r06-t9b-band.md`'s item 3 and its addendum rule them, with these differences. T9.e's
  first build (62e2074: four columns at revision 0, A_P ÷ A_V and A_S ÷ A_V at effective
  wavelengths, the camera's at A_V → 0 and 2) never reached the integration branch; the amendment
  replaced it in one regeneration, as the addendum asked, and took the node at A_V 7.5 the same
  day (the ruling's "Amendment, 2026-10-06: test 3's reference and the node at 7.5").
  - **Five companion tables, split by node.** `tables::star_colour_reddening` holds the curves at
    A_V → 0 (`NORMAL_REDDENING`, `WHITE_DWARF_REDDENING`, `[[f64; 15]; N]`: the six parts' values
    at unit luminance, r⁺, r⁻, g⁺, g⁻, b⁺ and b⁻, then the nine bands' moments).
    `star_colour_reddening_av02_05`, `_av10_15` and `_av20_30` hold every band's secant at two
    nodes each (`A_V_NODES`, `NORMAL_SECANTS`, `WHITE_DWARF_SECANTS`, `[[f64; 18]; N]`), and
    `_av07p5` at the one node 7.5 (`[[f64; 9]; N]`): 78 columns a row.
    - Five slow tasks in the `star_colour` module, the same `fit`, one manifest and one smoke
      manifest each, each table its own `@generated` header and `tables.lock` entry, revision 1
      since generator version 20 (REGISTRY 27). A const assertion holds all five to the colour
      table's row counts.
    - Sizes 282, 309, 164, 314 and 317 KB, 1.39 MB together (the addendum's estimate for 69
      columns: 1.45 MB). `star_colour.rs` (497 KB), its lock entry and its revision are untouched,
      byte for byte.
    - `REDDENING_A_V_NODES` replaces `CAMERA_REDDENING_A_V`. It is the sim's (`sky::colour`),
      which the fit reads; each manifest records `a_v_nodes`, checked against it, so the inputs hash
      covers it; each node file restates its own, which a sim test holds to the sim's.
    - The fingerprint (all five tables): plan 07's law at 12 wavelengths from 0.38 to 1.10 µm;
      the dust's transmission at 0.8 µm through each node, by the sim's `exp10`, and its `log10`,
      through which every secant is taken; the seven nodes; and the sim's `lift_into_gamut` of three
      fixed colours, two of them out of gamut (the channel lifted to zero is not probed).
    - **The parts take nine significant digits**, every other column seven. At seven, the lift of
      a row's c⁺ − c⁻ missed `star_colour`'s chroma by 1.0 × 10⁻⁶ (test 4 asks within 10⁻⁶); at
      nine, by 5.0 × 10⁻⁷ at most, the chroma's own seven-digit rounding.
  - **No row is out of gamut unreddened.** The sim's lift equals the fit's `unit_rgb` bit for bit
    on all 1,754 rows, and none of them is lifted (the addendum and T9.e's first science check
    took the coolest PHOENIX rows to be), so the acceptance and the slow every-row test also check
    the lift bit for bit behind A_V 30, where every row's light is out of gamut (determinism audit).
    Test 2's "coolest lifted PHOENIX row" is the coolest, 2,300 K at log g 5, which reddening lifts
    from about A_V 5. From about A_V 17 every row's light is the far red, which T3's lift takes to
    green 0 and a little blue, as it takes monochromatic 700 nm light to (4.56, 0, 0.42).
  - **The photopic light beyond 30 is held as one band.** Each part's own held secant lets the
    slowest part outlast the rest, and their luminance falls below zero from A_V 81.4 (the first of
    225 rows to do so by A_V 200). So beyond the last node the photopic light keeps its own secant
    at 30, the parts' there, and the colour of unit luminance is the 30 node's; the V, scotopic and
    camera secants are held as ruled. To 30, the parts' luminance is positive on every row (a test
    from A_V 0.5 to 100).
  - **`reddened` at A_V ≤ 0 is the colour itself**: a profile's rounding below zero no longer
    brightens it. At the first dust the colour is the lift of the parts, continuous to 5 × 10⁻⁷.
  - **The texel lifts too.** `BandTexel::of_sums` lifts its colour into gamut by T3's rule where
    the dust has taken a channel below zero (the solar row's blue passes a negative fraction past
    A_V 7–8); a colour in gamut is bit for bit as before.
  - **The overflow's channels add before any lift.** Each overflow star adds its unextinguished
    light times its own channel transmissions times its unreddened colour, as the first build did,
    and the texel lifts the total: light adds linearly in the channels, before any gamut mapping
    (science check of the amendment). It is exact because no row is out of gamut unreddened, and a
    star reddened out of gamut adds no white of its own lift. `point_lux` still takes the census's
    V less the bare A_V, until T8.k makes the census's V the star's own.
  - **`StarColour` holds its place in its grid** (the grid, the cell, the fractions: 24 bytes), not
    the 78 columns, and `reddening()` reads the five tables there as `star_colour` reads the colour
    table; `SkyStar` is 8 bytes smaller than T9.e's first build left it. `Reddening` (656 bytes,
    `StarColour::reddening`, `Reddening::through`) is the curves resolved, which the band reads
    once a call. `photopic_extinction_ratio`, `scotopic_extinction_ratio` and
    `camera_extinction_ratio` give way to `reddened`'s outputs; `Reddened` gains `v_extinction`,
    and `lift_into_gamut` is the sim's copy of T3's rule.
  - **Measured** (the tables' headers):
    - the solar row as A_V → 0: A_P ÷ A_V 0.9913, A_S ÷ A_V 1.1278 (their difference 0.1365),
      A_cam ÷ A_V 0.8774 and V's own 1.0043 (the addendum: 0.876 for the camera, 1.003 for V);
    - its secants at A_V 2, 5, 7.5, 10, 15, 20 and 30: V 0.9990, 0.9909, 0.9840, 0.9770, 0.9628,
      0.9482 and 0.9184 (the addendum: 0.998, 0.990, —, 0.976, —, 0.947); the camera 0.8187,
      0.7468, 0.7007, 0.6643, 0.6112, 0.5746 and 0.5264; the scotopic 1.1178, 1.1038, 1.0931,
      1.0832, 1.0656, 1.0502 and 1.0238; the photopic 0.9844, 0.9748, 0.9674, 0.9604, 0.9474,
      0.9353 and 0.9126;
    - at A_V 2 the Sun's camera term moves −0.361 mag and its eye offset −0.267;
    - over the dwarfs (log g 4.5, 3,000–30,000 K) as A_V → 0: A_P ÷ A_V 0.9716–1.0135, A_S ÷ A_V
      1.0975–1.1575, A_cam ÷ A_V 0.6344–1.0532, V's 0.9856–1.0196;
    - the parts' photopic transmission against the direct V(λ) integral within 3.6 × 10⁻¹⁵ at
      every node on every row (test 5's 10⁻⁴).
  - **Against the direct integrals** (test 2, the rows' own spectra, the solar row its four ATLAS9
    nodes mixed at unit luminance by the sim's weights; A_V 0.5, 1, 1.5, 3, 4, 6.25, 7, 8.75, 12,
    17 and 25), the worst over the eight rows, and a 45,000 K dwarf added beyond the addendum's
    list (science check of the amendment):

    | Quantity                | To A_V 5 | To A_V 20 | To A_V 30 | Its row (worst)             |
    | ----------------------- | -------- | --------- | --------- | --------------------------- |
    | the colour, a channel   | 0.0042   | 0.0169    | 0.0624    | 45,000 K dwarf              |
    | the photopic, mag       | 0.0022   | 0.0030    | 0.0200    | 45,000 K; 10,000 K; 2,300 K |
    | the camera term, mag    | 0.0089   | 0.0510    | 0.1158    | the Sun; 45,000 K; 45,000 K |
    | the eye offset (ρ), mag | 0.0016   | 0.0078    | 0.0333    | 45,000 K dwarf              |
    | V's extinction, mag     | 0.0002   | 0.0012    | 0.0153    | 3,000 K; 3,000 K; 2,300 K   |

    The photopic's 0.0030 to A_V 20 and 0.0200 to 30 sit at their tolerances, so the secants keep
    seven digits (the addendum allowed six). The camera term's 0.0510 to A_V 20 is at 12 or 17,
    under that range's 0.2. The cool rows are tighter (2,300–4,000 K: the colour within 0.007 at
    every dust, the camera term within 0.057).

  - **The node at A_V 7.5 holds the camera term to 0.035 mag to A_V 10** (ruled 2026-10-06, the
    ruling's amendment). With nodes at 5 and 10 only, the camera's secant, linear between them,
    missed the direct integrals by 0.048 mag at A_V 7 (30,000 K dwarf; the 10,000 K dwarf 0.046,
    the 10,000 K white dwarf 0.042, the solar row 0.034): a secant linear between nodes errs by
    about ½ k″ (A − A₁)(A₂ − A), and the camera term carries that times A, and the camera secants
    bend by k″ ≈ 0.0015 (solar) to 0.0022 (hot rows) over A_V 5–10 (the addendum's 0.035 had been
    checked on blackbodies). With the node at 7.5, test 2 samples A_V 6.25, 7 and 8.75 between 5
    and 10 as well: the camera term is within 0.0084 at A_V 7 on every row, and within 0.0143 at
    worst between 5 and 10 (the 30,000 K dwarf at 8.75), against the kept 0.035. A loosened 0.05
    and a monotone cubic were the alternatives declined.
  - **Test 3's references are the solar row's own spectrum's** (ruled 2026-10-06, the ruling's
    amendment): the Sun's blue at unit luminance is 0.6894 at A_V 1 and 0.5003 at 2, against the
    direct integrals of the table's solar spectrum, 0.6893 at 1, which the sim reproduces to 1.1 ×
    10⁻⁴. The addendum's first 0.685 ± 0.003 and 0.496 ± 0.005 are the 5,750 K, log g 4.5 ATLAS9
    node's alone (0.6847 and 0.4967 from the tables' columns, as T9.e's first science check took
    the nearest node for the Sun); a 5,772 K blackbody gives 0.669 and 0.482. The test asserts
    0.689 ± 0.003 and 0.500 ± 0.005, and that T3.c's ratios gave 0.725.
  - **Figures the ruled text rounds** (kept as ruled, 2026-10-06; the ruled sentences are left as
    written): Design note 6's V secant, "about 1.003 A_V … 0.976 A_V at A_V 10", is 1.0043 and
    0.9770 in the tables; the Risk "The camera cut is in V"'s "about −3 mag at A_V 10 and −8 at
    A_V 20" is −3.13 and −7.47 for the solar row, (k_cam − k_V) A_V. T3.c's `extinction_ratio`, which
    `reddened` no longer reads, is the one column not over the sightline's A_V (it is over the law
    at the V band's effective wavelength: −1% for hot stars, +0.3% for the Sun, +2% for M stars), so
    Design note 6's "every A_V in the colour tables" holds for the reddening tables.
  - **The band, re-measured** (near the Sun, cut 6.5, no census, complete everywhere, 16² faces;
    reddened against grey dust):
    - μ: the plane 22.074 against 22.088, the poles 24.564;
    - ρ: the plane 2.2068 against 2.4689 (−10.6%), the poles 2.2271 against 2.2543 (−1.2%);
    - the plane's colour at unit luminance: r 1.2207, g 0.9505, b 0.8404, against 1.0646, 0.9719
      and 1.0882 (−22.8% blue; the addendum expected near −23%, T9.e's first build −18.9%).
    - The cloud test (A_V 0.787 behind its central texels): ρ 0.9095 against its curves' 0.9066,
      blue over green 0.8046 against 0.8019.
    - Conservation (cuts 8, 7 and 6 within 200 ly) and the complete-to radius are T9.b's to the
      third figure.
  - **Acceptance as run.** The task's single command took two test filters, which `cargo test`
    does not take; the acceptance line reads `cargo test -p hyperion-sim sky::colour`,
    `cargo test -p hyperion-sim sky::band`, `cargo test -p hyperion-fit star_colour` and
    `just fit-check`, with the four slow tests once on the fetched spectra (the colour table's
    and the five reddening tables' reproduction among them). T8.k's line has the
    same two-filter form, left for T8.k. T9.e also moved five task titles (T5.f, T8.k, T9.e, T9.f,
    T9.g) onto one bold line each, so that `plan_task.py` finds them.
  - **T17 gains `sky/colour_reddened.golden`** (determinism audit): every field of `reddened` at a
    few points on both grids at A_V 0.5, 2 and 5, off the nodes at 1, 7, 17 and 25, and held at 40.
    Until it lands, no gate compares the reddened light's bits across targets (native against
    wasm32-wasip1): the sim's bit checks compare one target with itself, and the rest take
    tolerances (determinism audit of the amendment; nothing generated or served reads it yet;
    the deferral to T17 accepted 2026-10-06).
  - **The fetched spectra** are read through read-only per-dataset symlinks into
    `.claude/worktrees/agent-ad163063e17117877/crates/hyperion-fit/data/cache/`, whose checksums
    all match.
  - **Test 8 without the fetched V band** (fixed forward after integration CI-40). Without
    Bessell and Murphy's Table 1, as in CI, on GitHub and on a fresh checkout, the fit's observer
    takes the photopic stand-in, V(λ) λ, whose moment for the 5,772 K blackbody is 0.9839, outside
    V's 0.99–1.02. The fit's unit test `the_reddening_columns_of_sunlight_are_the_rulings` holds V
    to that range only on the fetched V band; otherwise it holds V's column to the stand-in's own
    integral (to 10⁻¹²) and below the photopic's. The committed table's solar row, fitted on the
    real V band, stays held to all four ranges by the sim's `the_solar_rows_ratios_are_the_rulings`,
    fetched or not.
  - **Not changed.**
    - Until R06.T8.k, the census's per-star eye offset (`kept_to`) reads the unreddened ρ, and its
      V is M_V + DM + the bare A_V, so no listing moves. T8.k cuts on each star's own V (the
      addendum's item 4).
    - The protocol and the client are unchanged; T11.a and T11.c read `reddened` for the wire.
    - The cost is not benched: each node takes seven `exp10` and a lift in place of five `exp10`,
      some half a CPU-s against the band's 20; T9.f splits the bench.
  - **The first build's four pending rulings are decided** (`decision-r06-t9b-band.md`, addendum):
    the display channels are exact (item 1); every band has nodes to A_V 30 (item 2); the ruling's
    camera figures were effective-wavelength values and are corrected to the integrals (0.874 →
    0.820 for the Sun, 0.84–0.88 for 5,000–5,800 K, 0.81, 0.77, 0.70 and 0.64 from 4,500 to
    3,000 K, at A_V 2 −0.36 and −0.67 to −0.77; item 3, and so the solar camera range is now
    0.84–0.92); and every column divides by plan 07's sightline A_V (item 4, Design note 6).
- **Deviations in T9.c, as built (2026-10-06).** `sky::limits::{Glare, limit_rows, limit_map}`
  as Design note 4 sets the glare out, with these differences.
  - **The signatures.** The sketch's `limit_map(eye, spec, band, listed)` cannot place a star
    without the observer, and a server runs the band's rows as jobs (T11.c). So
    `Glare::of_listed(observer, listed)` resolves each listed star once a census: its direction,
    its photopic illuminance after its own reddening (`band::unextinguished_lux`, shared with the
    overflow's points, times the photopic transmission) and its scotopic light, that times
    `colour().reddened(a_v()).sp_ratio()`. It debug-asserts the census's order (`sky_order`), in
    which the veil is summed. `limit_rows(eye, spec, glare, face, rows, texels)` sets the eye
    limits of a job's rows, in `band_rows`' order, and `limit_map(eye, spec, glare, band)` runs it
    over the six faces. The default `Glare` holds no star (T9.d's pre-pass). The overflow does not
    glare: its light is in its texel already, as E ÷ Ω, about 2,000 sr⁻¹ times E at 64², against
    a texel mean of its own glare of about 560 sr⁻¹ times E (science check). A star at the observer's position adds none.
  - **The background.** A texel of band luminance B and ratio ρ is seen against B′ = B + Σ E★
    K(θ★) at ρ′ = (ρ B + Σ ρ★ E★ K(θ★)) ÷ B′, K CIE 146's veil per lux. In a scotopic background
    that is exactly Design note 4's rod weighting, ρ★ ÷ 1.408; in a mesopic one it is MES2 over the
    whole background at one adaptation (MES2 is linear in the light at a fixed m), where `eye.rs`'s
    example had weighed the veil at the band's own m (under about 0.03 mag apart). The example and
    `blackwell_equivalent_factor`'s doc now take the mixture. With no veil a texel's background is
    its own luminance and ρ, bit for bit. ρ′ is held within 0.01–100 and B′ at 10¹² cd m⁻²
    (`SpRatio::MIN`, `SpRatio::MAX` and `SkyBackground::MAX_LUMINANCE`, new), which no starlight
    reaches. The band's luminance, chroma and ρ are unchanged.
  - **θ** is from the texel's centre, the direction its ray takes, clamped at 0.1° by
    `veiling_luminance`. A cosine test skips the sources past 100.01° and leaves the 100° cut to
    `veiling_luminance`, so the two never part (a test pins both sides).
  - **Also:** `BandTexel::set_eye_limit` (crate) and a test-only `BandTexel::of_light`;
    `eye::GLARE_MAX_ANGLE_DEG` crate-visible.
  - **The tests** (`cargo test -p hyperion-sim sky::limits`, 9; about 55 s on two threads, most of
    it the tables, a 16² band and a census within 100 ly):
    - the identity: every texel of the near-Sun band at 16², cut 8.15, against 1,522 sources (the
      1,520 stars the census lists to V 8.15 within 100 ly, and two placed: V −1.5 0.05° from a
      texel's centre, read at 0.1°, and V −9 0.3° from another's, whose two texels are mesopic),
      within 3.1 × 10⁻¹¹ mag of the definition written again with `atan2` angles and each
      channel's own `veiling_luminance`;
    - the medians at cut 8.15, 16² faces, the reddened band complete everywhere with no census:
      against the band alone 6.545 over 128 texels in the band (|b| under 5°) and 7.693 over 8 at
      the poles (|b| over 80°); with the glare of the 1,520 stars, 6.544 and 7.668. The ruled
      6.5 ± 0.20 and 7.55 ± 0.22 (the ruling's estimate for the fixture, 6.53 and 7.71). The census
      within 100 ly stands in for the final reply's, too dear for a unit test: its brightest stars
      are the nearest, but the far field of the whole list is larger (see "Glare double count");
    - a V −1.5 star of Sirius's colour 0.5° from a 64² texel's centre: 5.65 against its
      neighbours' mean 7.06 towards the north galactic pole, 5.63 against 6.51 in the plane (the
      science check reproduced 5.69 / 7.09 and 5.66 / 6.45);
    - a field factor of 2 moves every limit by −2.5 log₁₀(2 ÷ 1.4) to 10⁻⁹, as the client's
      `fieldFactorOffsetMag` assumes;
    - the map reads only the band's light and the listed stars: row splits give the same bits,
      the chroma does not enter, with no glare each limit is the band's own bit for bit, and a
      texel with no light and no glare is seen to Crumey's clamp;
    - the glare reads each star's reddened light (all 1,520 near-Sun stars lie behind some dust);
    - the reach: 99.995° veils by exactly its `veiling_luminance`, 100.005° adds exactly nothing,
      and 100.02° and 120° are not read; two refusals.
  - **The cost** (a probe, not committed; dev build, the sim at opt-level 2, load 5–8,
    provisional): 19 ns a texel–star pair, so a 64² band against 4,824 stars (V 8.15 within
    200 ly) takes 2.25 CPU-s, and `Glare::of_listed` 0.6 µs a star. The map is texels × listed:
    some 14 CPU-s at the eye's cut with the final caps if the fixture lists 3 × 10⁴ stars there
    (an estimate), and about 140 CPU-s at `MAX_N_MAX` (300,000, a camera's cut with the eye
    open), the whole first-sky budget. For T17 to measure. Every listed star veils its texel
    within 0.1° at some 10⁴ sr⁻¹ times its E, so dropping faint stars changes the map unless the self-veil
    below is ruled first; a far field from a coarse map is the other lever. _Decided 2026-10-06
    (`decision-r06-t9c-glare.md`, item 2): R06.T9.i. Built 2026-10-07 (below, "Deviations in
    T9.i, as built")._
  - **Cross-target bits** rest on review (determinism audit: nothing to fix, GENERATOR_VERSION 20,
    no golden moves) until T17's `sky/band_face_row.golden`, which now also pins the row's eye
    limits against a census's glare.
  - **Open, for a ruling: a star veils its own texel.** Design note 4 sums every listed star and
    T9.c samples at the texel's centre, so a listed star near its texel's centre veils that texel,
    and the client gives every star of a texel its limit (`view/sky/limits.ts`). Crumey's eq. 34
    fits Blackwell's point-source thresholds, which already hold the target's own scattered light;
    CIE 146's equation is for a source 0.1°–100° from the target. The science check, on the
    fixture's median band at 64²: a star 0.01, 0.1, 0.3 and 0.5 mag above the limit culls itself
    within 0.49°, 0.22°, 0.15° and 0.12° of the centre at the poles (0.28° and 0.13° for the
    first two in the plane, none beyond), up to 0.77 mag above it within 0.1° (0.20 in the
    plane). Averaged, that is a limit about 0.055 mag shallower at the poles and 0.015 in the
    plane, some 4–5% and 1.5% of the visible stars lost, on a grid of texel centres. Options: (a)
    keep it as stated (built); (b) per-star self-exclusion: the server gives each listed star the
    difference ΔV★ ≥ 0 between its texel's limit without its own veil and with it, in which F
    cancels, carried on the star's wire eye offset (Design note 17) with no wire change (the
    science check's lean); (c) a texel mean of the veil over Crumey's summation area, about one
    64² texel (eq. 63: 37.6′ radius at μ 21.83), smoother but still about 0.05 mag biased, best
    with (b). _Decided 2026-10-06 (`decision-r06-t9c-glare.md`, item 1): option (b), R06.T9.h.
    The ruling's model, scored against each star's own limit, puts (a)'s loss at 2–3% at the poles
    (−0.02 to −0.03 mag), with 17–20% fewer visible stars within 0.25° of each texel's centre.
    (c)'s bias is 5–7%; (b) is unbiased. The other stars' veils stay sampled at the texel's
    centre, which is unbiased but places a bright star's halo as a square; T13's by-hand check
    looks for it, and the ruling has its remedy._
  - **Not settled by the sources** (science check): CIE 146 itself was not reached, and the
    equation was checked against a secondary quotation; whether its E is the illuminance normal
    to the star, as built, or on the pupil's plane (E cos θ, nothing past 90°), which would make
    the far-field veil 0.71 times as large (at the poles −0.041 mag rather than −0.056 for the
    whole list; nothing within about 20° of a star moves). _Decided 2026-10-06
    (`decision-r06-t9c-glare.md`, item 3): the plane of the eye, E cos θ, and none from behind it;
    R06.T9.h._
- **The eye's background when a camera is open (found and decided 2026-10-06,
  `decision-r06-t9c-glare.md`).** As T9.c is built, the band holds the light fainter than the
  request's cut. A camera's 10.06 then darkens the eye's background, against the band ruling's
  "light fainter than the eye's cut", and lists stars between the cuts that veil, so opening a
  camera view would change the cockpit eye's stars. Each camera-only star's own light, added to
  its texel, would instead leave single texels up to 0.2 mag deep, past T9.d's pad. From
  R06.T9.j the eye's map is the eye-only request's: the expected light fainter than the eye's
  cut, and the glare of the stars brighter than it. _Built in R06.T9.j (Risks, "Deviations in
  T9.j, as built"): near the Sun the camera's band had moved the eye's median limits by +0.12 in
  the band and +0.24 at the poles; they are now the eye-only request's, bit for bit._ _At equal
  census radii. A camera's own caps reach farther, and there its census's real stars brighter than
  the eye's cut, under one expected a layer, are accepted as the truer sky (decided 2026-10-07,
  `decision-r06-t9c-glare.md`, addendum 2)._
- **Deviations in T9.h, as built (2026-10-06).** `sky::limits::eye_offsets`, as the task sets
  it, with these details.
  - **One term, used twice.** `GlareSource::veil_per_lux` (private) is the veil per lux in the
    plane of the eye, `veiling_luminance` per lux at θ times cos θ, and `None` at cos θ ≤ 0. The
    map sums it, and `eye_offsets` recomputes it for each star at its texel's centre
    (`BandSpec::texel_of`, then `texel_direction`), so the term subtracted is the term the map
    added, bit for bit. A rounded sum of non-negative terms is never less than any of them, so the
    difference is never negative; a debug assertion holds it.
  - **The veil kept.** `BandTexel` keeps the veil beside its limit (a private `EyeLimit`);
    `set_eye_limit(limit, veil)` and `eye_veil()` are crate-visible, and `eye_limit()` is
    unchanged.
  - **One glare entry a listed star.** `GlareSource` gains `sp_ratio` (the reddened ρ, for the
    colour offset) and its direction becomes an `Option`, so `Glare` keeps every listed star in
    the census's order and `eye_offsets` returns one offset a star. A star at the observer's own
    position has no direction, glares nothing and takes its colour offset against a scotopic sky.
    `Glare::len()` now counts the listed stars, not only those that glare; in a census they are
    the same.
  - **E cos θ.** `REACH_MARGIN_DEG` is gone, and `GLARE_MAX_ANGLE_DEG` is private to `eye.rs`
    again. `veiling_luminance` keeps its arithmetic and its 100° bound. Its doc names the plane
    with the three sources, and its example takes E cos 1°. `crates/hyperion-sim/clippy.toml`
    gains `IJspeert` in `doc-valid-idents`.
  - **"Decides".** In the tests and records, the self-exclusion decides a star when the star's V
    lies between its limit before the self-exclusion (its texel's limit plus its colour offset)
    and its own limit. The task's "between its texel's limit and its own" would also count, for a
    hot star, stars that the colour offset decides alone.
  - **The saturation, measured (for the owner).** Two of the ruling's figures are corrected
    here, and both are reported for the owner to rule on.
    - The colour table's largest colour offset is +0.4526: the 500,000 K blackbody row of both
      grids, ρ 3.4850. The blackbody rows run from ρ 3.4501 (+0.4417) at 120,000 K, and the
      Rayleigh–Jeans limit is +0.4560 (science check). The ruling and Design note 5 have +0.43.
      Design note 5's +0.45 is short by 0.0026, which its 0.1 pad absorbs; it is a finding for
      T9.d's test "at most 0.45 (it is 0.43)". _Ruled 2026-10-06 by the orchestrator (T9.h's open
      question 1): the table's real maximum, +0.453, everywhere. Design note 5 and T9.d read "at
      most 0.46 (it is 0.453)", and the 0.1 pad keeps its role. The ruled 0.16 mag of window lost
      stays as it is. Built in R06.T9.d._
    - At Crumey's clamp, over a texel whose colour-corrected background is exactly 10⁻⁵ cd m⁻²,
      the hottest star sits 0.05° from a 64² texel's centre (read at 0.1°), at the default eye
      (F 1.4, 25 years). Its own limit is 8.440. The brightest star its self-exclusion decides is
      V 7.443, under a texel limit of 6.990. That star has the largest eye offset, +1.450 (0.997
      self-exclusion and +0.453 colour), 0.180 past the wire's +1.27, where the ruling estimated
      about 1.0 + 0.43.
    - A star's V less its texel's limit rises with its V (science check: the texel limit's slope
      in V is at most 0.905 times the star's share of the background). So the +1.27 culls a star
      the offset would keep only when a star at its own limit lies more than 1.27 fainter than
      its texel's limit. At the default eye it lies 1.100 fainter, so the saturation culls none:
      the window lost is 0.000 mag. The ruling's "at most about 0.16" took the offset's excess,
      at the window's bright end, for the window lost.
    - It passes 1.27 from F ≈ 2.2 at 25 years (2.07 at 70, 1.97 at 80 with p 1.2). The window
      lost is then 0.046 mag at Crumey's F 2.4, and 0.120 at F 2.4, 80 years and p 1.2. That is
      within the ruled 0.16, which the test holds over those eyes. The client sends F 1.4 today.
    - Near the Sun no listed star's verdict changes with the saturation, in the fixture or at 64²
      (below).
    - Older than T9.h, at the wire's other end (science check): a cool star behind several
      magnitudes of dust can have an offset below −1.28. Its colour offset falls by the scotopic
      less the photopic extinction, about 0.137 mag per magnitude of A_V (the Sun's). That
      happens for a 2,300 K dwarf (−0.825 unreddened) from about A_V 3.3, and for a red giant
      (about −0.35) from A_V 7. The wire then keeps such a star where its own limit would cull
      it. It never happens near the Sun, but a dusty sightline elsewhere can reach it. This is for
      T11.a, where the wire's doc comments land.
  - **Tests** (`cargo test -p hyperion-sim sky::limits`, 15; `sky::eye`, 15, unchanged):
    - the identity: texels and stars' own limits, within 3.07 × 10⁻¹¹ mag;
    - the pole star: V 7.662, 0.05° from a 64² texel's centre towards the north galactic pole. Its
      texel sees to 7.162 and the star to its own 7.712 (offset +0.550), which is its texel's
      limit with no glare, bit for bit;
    - F = 2 against 1.4: the offsets agree within 1.1 × 10⁻¹⁴;
    - no self-exclusion is negative in a scotopic texel (all 1,520 stars' texels);
    - the reach at 89.995° and 90.005° (and 99.995° and 120°, which add nothing), and the texel's
      kept veil;
    - also new: the saturation at the clamp, at F 1.4, F 2.4, and F 2.4 at 80 years (above); row
      splits give the same veils and eye offsets; a star that adds no veil, of no illuminance
      in a mesopic texel or with no direction, takes its colour offset alone; and two refusals
      (a band of another size, a band without its limits).
  - **Records.** The fixture: 16², the census within 100 ly (1,520 stars). A probe, not
    committed: 64², the census within 200 ly (4,824 stars). Both near the Sun at cut 8.15, on a
    dev build. The probe's source and output are in `.git/rm23-scratch/r06-census/t9h/`
    (`probe64.rs`, `probe64.txt`).
    - The medians, plane and poles: at 16², 6.544 and 7.671, against T9.c's 6.544 and 7.668 (no
      glare: 6.545 and 7.693). At 64²: no glare 6.548 and 7.684; E normal to 100°, as T9.c built
      it, 6.542 and 7.659; E cos θ 6.542 and 7.664. The median texel difference at the poles is
      +0.0045 (largest +0.0050). The ruling's +0.010 is for a 30,000-star list's larger far field.
    - The largest self-exclusion that decides a star: 0.483 at 16² (one star decided), and 0.610
      at 64² (49 decided, median 0.252). The ruling's model gives 0.73 at the poles, median 0.23.
    - The largest eye offset: of a decided star, +0.480 at 16² and +0.697 at 64²; at Crumey's
      clamp, +1.450 (above). Any listed star's can be larger (+1.75 and +2.26), for a bright star
      near its texel's centre: such a star is much brighter than its texel's limit, so the
      saturation changes nothing for it.
    - The largest self-exclusion of a star within 0.5 mag of its own limit and 0.6° or more from
      its texel's centre: 5.2 × 10⁻³ at 16², and 7.2 × 10⁻³ at 64². The ruling's model: under
      0.01.
    - The self-exclusion's effect at 64²: 2,719 of the 4,824 listed stars are seen at their own
      limits, against 2,670 at their texel's limit plus their colour offset, +1.8%. That is +3.0%
      at |b| over 60° (449 against 436), +1.9% at the poles (54 against 53), and none in the
      plane (176 against 176). Within 0.25° of a texel's centre 316 are seen against 280, +13%:
      the grid's loss is removed. The ruling's model puts (a)'s loss at 2–3% at the poles.
  - **Not changed.** No wire bytes, no client code and no wire doc comments: T11.a carries those
    (the ruling's §5). GENERATOR_VERSION stays 20, and no golden moves.
- **Deviations in T9.d, as built (2026-10-06).** `sky::limits::eye_cut`, with the sketch's
  signature, as Design note 5 sets it, with these details.
  - **The colour offset is the table's.** A private `largest_colour_offset()` is
    `star_colour_offset` of `colour::largest_sp_ratio()` in a scotopic sky: +0.4526.
    `largest_sp_ratio` (crate, new) is the largest row's ρ over both grids, 3.4850, the 500,000 K
    blackbody rows. This follows the orchestrator's ruling of 2026-10-06 on T9.h's open question 1.
    Design note 5, the Provides comment and T9.d now read 0.453, "at most 0.46" and 0.553.
    - Against the first +0.45 the cut moves by +0.003 (the 0.0026 and the repeat's share). That is
      under the ruling's 0.01, and the pad keeps all of its 0.1.
    - A test in `sky::colour` holds every node colour and the sampled ones at or under that ρ,
      unreddened and behind A_V 0.1–40 (15,040 cases). Dust lowers ρ for a CCM law because V′
      weighs bluer light than V, which is not a theorem, so the test guards it (science check).
    - `EYE_OFFSET_BOUND_MAG`'s doc takes the figure. Its 0.6 stays until R06.T8.k removes it.
  - **The pieces.** All private:
    - `PRE_PASS_SPEC`, 16² on the standard 12 nodes a decade;
    - `PROVISIONAL_CUT_V`, 7.85, and `CUT_PAD_MAG`, 0.1;
    - `PrePass` (the darkest limit and the cut) and `EyeCutPasses` (the first pass and the repeat).

    The pre-pass is `band_rows` with `SkyCensus::empty()` and `CompleteTo::everywhere()`, on a
    query with no eye or cone (the band reads neither), then `limit_map` with `Glare::default()`.
    The repeat runs when the first cut is deeper than 7.85. A debug assertion holds the repeat's
    darkest limit no shallower than the first's, to 10⁻⁹. That is the premise that what the repeat
    leaves lies on the side the pad covers.

  - **Held at `MAX_CUT_V`.** `SkyQuery` refuses a cut deeper than V 11, and the repeat queries at
    the cut, so the cut is held there. Only an eye of F under about 0.15 reaches it (`EyeObserver`
    takes F from 0.1). A test holds an F 0.1 eye there.
  - **The full map in the bound test.** The task's "full limit map with no glare" is the 64² band
    at the cut, with no census and complete everywhere. That is the darkest band any reply at that
    cut has: a census's caps add the light beyond them, and its overflow adds its stars' own light.
    So it bounds every eye-only reply's map, and, from R06.T9.j, every reply's eye map. The test
    also takes the glare of the stars a census lists within 100 ly at the cut, and holds each
    one's own limit within the cut.
  - **Tests** (`cargo test -p hyperion-sim sky::limits`, 23, of which 8 are new; and the new test
    in `sky::colour`):
    - the identity: both passes' darkest limits and cuts, against the 16² band's
      `naked_eye_limit` recomputed for each texel plus the table's largest offset at μ 30, to
      10⁻⁹. They also match the darkest limit plus `largest_colour_offset` plus the pad bit for
      bit, and `eye_cut` gives the repeat's bits;
    - the offset: +0.4526, at most 0.46, and above the 120,000 K row's +0.442;
    - near the Sun: 8.2811, against 8.15 ± 0.22;
    - the repeat: 8.2386 → 8.2811, +0.0425, under 0.05;
    - the bound: the deepest 64² texel without glare (|b| 81.9°) sees to 7.7350. That is 0.0935
      within the cut less the offset, 7.8285. With the glare of the 1,587 stars listed within
      100 ly at the cut, the deepest own limit is 8.0099 and the largest eye offset +2.267;
    - also new:
      - in the nuclear disc, at (0, 150, 0), the darkest 16² texel sees to 6.8172, so the cut is
        7.3698, with no repeat;
      - an F 0.1 eye is held at V 11;
      - on one warm context, the Sun's and the nuclear disc's cuts have the same bits in any order
        (`assert_order_independent`), and the Sun's are a cold context's (determinism audit).
  - **Records** (near the Sun, the default eye, dev build):
    - The first pass: the darkest 16² texel, at |b| 78.8° and μ 24.916, sees to 7.6860, so the
      cut is 8.2386. The repeat: the same texel, at μ 25.011, sees to 7.7285, so the cut is 8.2811.
      The band ruling's probe had 24.92, 7.68 and about 8.27. With the first +0.45 the cut would be
      about 8.278.
    - The pad: 0.0065 of it is used. About 0.0046 is the final band at 8.2811 being darker than
      the repeat's at 8.2386 (0.11 mag of limit per magnitude of cut). About 0.002 is the 64²
      texels against the 16².
    - The stars listed at the cut: 1,587 within 100 ly (1,520 at 8.15), and 5,168 within 200 ly
      (4,824 at 8.15).
    - The cost: `eye_cut` took 5.4 s on one thread in the test profile, with the tables built, at
      load about 15. That is two 16² bands, 3,072 rays. A release timing is T17's, with the
      per-reply costs; T11.a may run the faces as jobs.
    - T9.c's and T9.h's tests, and T9.i's and T9.j's, keep the ruled 8.15, the real sky's figure.
      T17 benches the census at the computed 8.28, beside 7.95. _With the diffuse light (R06.T9.g,
      `decision-r06-t9g-dgl.md`) the fixture's cut is about 8.0 and the repeat about +0.01: a light
      no cut changes dilutes the step. As built, 8.18 and +0.027 (Risks, "Deviations in T9.g, as
      built")._
    - The probe's source and the logs are in `.git/rm23-scratch/r06-census/t9d/`.
  - **The pad rests on a smooth sky (science check; for the orchestrator).** The clamp alone keeps
    the 64² map within the pad only where the darkest 16² texel sees to 7.89 or deeper (μ about
    25.4). Near the Sun its 7.73 leaves that to the polar sky being smooth on 5° scales, which the
    test measures at one observer.
    - A dark cloud smaller than a 16² texel (7°), lying between the pre-pass's rays, could put a
      64² texel behind it more than 0.1 deeper than the pre-pass saw. The excess is at most the
      clamp's 7.99 less the darkest limit.
    - It was not seen here: the fixture has no modifiers, and plan 09's clouds reach no
      sightline yet.
    - A second observer near a cloud in the test, or a pre-pass that also samples the clouds'
      directions, would settle it.
  - **Also amended.**
    - Design note 5's "8.17 at 26" was eq. 34 before the 2026-10-02 clamp; it now reads 7.99 from
      μ 25.6 (science check).
    - T17 gains `sky/eye_cut.golden`. No golden pins the cut before then, and one added now would
      need a re-bless in the 20 → 21 batch (determinism audit).
  - **Files and acceptance.** Besides `sky/limits.rs`, two files:
    - `sky/colour.rs`: `largest_sp_ratio` and its test;
    - `sky/census/cell.rs`: a doc comment and an assertion message.

    Acceptance as built: `cargo test -p hyperion-sim sky::limits` and
    `cargo test -p hyperion-sim sky::colour`.

  - **Not changed.** GENERATOR_VERSION stays 20. Nothing served or golden reads the cut (T11.a
    will call it), and no golden moves. The older records citing 7.95 (T7's, T8.e's and T8.f's)
    stand as the estimate they were.
- **Deviations in T9.i, as built (2026-10-07).** The glare's pyramid in `sky::limits`, as the task
  sets it out, with the ruling's near field and opening angle (4° and 0.25, not tuned), and with
  these details.
  - **The signatures.** The pyramid holds the band's own texels, so a glare is built for its band.
    - `Glare::of_listed(observer, listed, spec)` takes the band's `BandSpec`.
    - `limit_rows`, `limit_map` and `eye_offsets` keep their signatures. Each refuses a glare built
      for faces of another size.
    - `Glare::default()` has no pyramid and serves a band of any size, as T9.d's pre-pass needs.
    - New and public: `Glare::of_points(points, spec)`, the glare of point sources, each a direction,
      a `Lux` and an `SpRatio`. The bench needs it for its synthetic sky, since no `SkyStar` can be
      made outside the crate.
    - T9.j still adds the eye's cut to `of_listed`.
  - **The pyramid.** All of it is private: `Pyramid`, `GlareNode`, `Square`, `NodeHolds` and
    `GlareLight`.
    - Per face, the leaves are the band's texels. Each square of texels is halved, rounding up for
      a side that is not a power of two, up to one root a face. A square with no glaring star has no
      node.
    - A star glares where it has a direction and light. Its leaf is `BandSpec::texel_of` of its
      direction, as `eye_offsets` finds it. A leaf keeps its stars in the census's order (a stable
      sort).
    - Each node keeps its stars' photopic and scotopic light and their photopic-weighted mean
      direction. That direction is the normalised Σ E u of its stars, carried up unnormalised, not a
      mean of its children's directions.
    - Its radius is a leaf's largest angle to one of its stars, and a parent's largest angle to a
      child's direction plus that child's radius, which bounds its stars.
    - A node of one star keeps that star's direction, bit for bit, and radius 0. A node of one child
      keeps the child's direction and radius. So a lone star taken whole gives its own exact term.
  - **The rule, in cosines.** Each node keeps two cosines, against which a texel's centre is tested
    by one dot product:
    - −sin r, at or below which the node is skipped (d − r ≥ 90°);
    - cos max(4 r, r + 4°), at or below which it is taken whole (r ≤ 0.25 d and d − r ≥ 4°).

    A node taken whole adds its light times `veil_per_lux_at` at its mean direction, and nothing at
    or beyond 90°. Every term, a star's or a node's, is that one function, K(θ) cos θ, which T9.h's
    subtraction reads.

  - **A texel's own leaf.** Before the cosines, every node whose square holds the texel is opened, by
    an integer test of its face, level, row and column. So the rule holds at any band size. At 16² a
    face-centre texel's corner lies 5° from its centre, past the near field: the test places two
    stars there in a leaf that the cosines alone would take whole.
  - **The order.** The roots go in face order and each node's children in their squares' order,
    depth first. So each texel's veil depends on its direction, its own texel and the pyramid alone,
    and any split of rows gives the same bits (tested at 16² and 64²). The sim spawns no threads,
    and a server's jobs split rows.
  - **The exact sum** is the same traversal with every node opened (`Opening::Every`, test-only),
    each leaf's stars summed one by one. T9.c's and T9.h's identity test and T9.h's reach test take
    it, still within 3.07 × 10⁻¹¹ mag. The pyramid gives a lone source's bits too, which the reach
    test also checks.
  - **Counted.** A private `Tally` trait counts the nodes tested, the nodes taken whole and the pairs
    summed. The map's own runs count into `()`, which costs nothing.
  - **Tests** (`cargo test -p hyperion-sim sky::limits`: 28, of which 5 are new and 1 is slow; 143 s
    on four threads; logs in `.git/rm23-scratch/r06-census/t9i/`, the final runs in `final/`):
    - Near the Sun at 64², cut 8.15, against the glare of the census within 200 ly (4,824 stars):
      texel limits within 4.35 × 10⁻⁴ mag of the exact sum, 2.32 × 10⁻⁵ on average, and eye offsets
      within 5.9 × 10⁻⁵. The pyramid sums 151,559 pairs and takes 6,989,356 nodes whole, 16.6 times
      fewer evaluations than the exact sum's 118,554,624 pairs. With T9.c's two placed sources too
      (4,826 sources): within 4.35 × 10⁻⁴ mag, 1.70 × 10⁻⁵ on average, and eye offsets within 5.9 ×
      10⁻⁵, from 151,660 pairs and 6,994,610 nodes. Rows split as jobs give the same bits on +X and
      +Z.
    - The synthetic sky of 300,000 stars to V 10.06, the ruling's model's `sphere`: density 1 + 3
      exp(−|b| ÷ 10°), N(< V) ∝ 10^(0.45 V) from V −1.5, ρ 1.5–3, and a band of μ 24.6 at the poles
      to 22.2 in the plane. The pyramid makes 995 evaluations a texel: 12,790,116 pairs and
      11,657,759 nodes taken whole, 301.6 times fewer than the exact sum's 7,372,800,000 pairs (the
      ruling's model: 989 and 303 times). It tests 17,695,511 nodes. One row of each face lies
      within 2.73 × 10⁻⁴ mag of the exact sum, and rows split as jobs give the same bits on −Y.
    - Slow (`#[ignore]`, 140 s in the slow-test profile): every texel of the synthetic sky within
      3.82 × 10⁻⁴ mag of the exact sum, 1.51 × 10⁻⁴ on average, and every eye offset within 2.53 ×
      10⁻⁴ (the ruling's model: 4.2 × 10⁻⁴ and 1.5 × 10⁻⁴).
    - A texel always opens its own leaf (above): there its veil and the stars' eye offsets are the
      exact sum's, bit for bit, and the texel across the corner takes the leaf whole.
    - A glare built for 16² faces is refused by an 8² band.
    - The synthetic sky is held to a digest, `SYNTHETIC_SKY_DIGEST`, which the bench's copy of it
      asserts too.
  - **The cost** (provisional):
    - By the process's CPU clock (`utime` and `stime`), on one thread in the slow-test profile at
      load 6.6–8.3: at 300,000 stars the pyramid's map takes 1.38 CPU-s and the glare's build 0.07,
      1.45 CPU-s in all, against 161.3 CPU-s for the exact sum (21.9 ns a pair), 111 times as much.
      That is within the gate of 3 CPU-s. An earlier run by wall time, at load 11–23, gave 1.42 s
      and 0.08 s against 190.9 s (25.9 ns a pair).
    - The bench, `sky/limit_map`, release, without the heavy-test lock: another lane's refit held
      it for hours, so the orchestrator ruled a provisional run at `CPUQuota=400%` (3 workers, load
      7.7–9.5). `near_sun`: 0.32 CPU-s, 0.11 s wall (criterion 289 ms an iteration, the glare's
      build 0.005 s). `synthetic_300k`: 1.03 CPU-s, 0.38 s wall (criterion 1.03 s, the build 0.055
      s). The jobs' CPU time is each job's wall time, as the file's other benches take it. A
      locked re-timing on a quiet machine is pending, for the orchestrator to schedule.
    - The pair count is the gate that does not depend on the machine: 301.6 times fewer
      evaluations, against the 100 the task asks for.
  - **The medians** at 64² near the Sun, with the census's glare: 6.542 in the band and 7.664 at the
    poles, as T9.h's probe had with E cos θ (6.542 and 7.664). With the placed V −9 star too, 6.489
    and 7.334. That star lights a whole hemisphere: at 60° its veil, about 1.7 × 10⁻⁵ cd m⁻², is about
    1.5 times the poles' own band (about 1.2 × 10⁻⁵).
  - **Not changed.** No wire bytes, client code or wire docs change, and T9.d's tests read no glare.
    GENERATOR_VERSION stays 20, and no golden moves: nothing generated or golden reads the map.
    Cross-target bits rest on review until T17's `band_face_row.golden` pins the pyramid's bits,
    after T9.j. The skip and whole decisions turn on libm's `sin`, `cos` and `acos`, as the
    sightlines' do (determinism audit).
  - **Reviews.** Determinism audit: nothing to fix (`golden_diff` 0). Applied from its
    suggestions: the test and the bench hold their two copies of the synthetic sky to one digest,
    and a test holds a leaf to the census's order. Science check: the physics and the cosine forms
    are confirmed. Applied: the synthetic band's profile and the exact sum's cost in the docs, the
    tolerance's quantum (a texel limit's millimagnitude, a tenth of an eye offset's centimagnitude),
    the 0.13 mag's opening angle (0.5), a note that a node's scotopic sum errs at first order in its
    stars' spread of ρ, and two phrases here.
- **Deviations in T8.k, as built (2026-10-07).** One boundary, as the band ruling
  (`decision-r06-t9b-band.md`, item 4 and the fourth item of its addendum) sets it out, built with
  T8.j, with these details.
  - **The kept test** (`sky/census/cell.rs`). A star is kept if its own V, M_V + DM plus
    `colour.reddened(a_v).v_extinction()` behind its sightline's A_V, is at or brighter than the
    cut, with the eye or without it.
    - `faintest_listable` is the cut less the distance modulus. `kept_to`, the scotopic sky it read
      and `EYE_OFFSET_BOUND_MAG` (0.6) are gone, the constant from `sky::census`'s public API too,
      with its test `the_eye_offset_bound_holds_over_the_colour_table`. T9.d's tests hold the cut's
      largest offset, 0.453.
    - Private `own_v_extinction(colour, a_v)` debug-asserts a non-negative extinction. The
      determinism audit found V's secant at least 0.865 on every row of the five reddening tables,
      so v★ A_V ≥ 0 and the cut before the sightline and the flux bound stay exact.
    - The colour is read after the cut before the sightline, not before it: a star cut there no
      longer reads the colour table. No output moves for it.
    - `SkyStar::v` is now that V, and `SkyStar::a_v` stays the sightline's A_V. The wire (T11.a)
      carries the V.
  - **The cone** (`sky/census/query.rs`, new and public): `Cone::cos_half_angle()` and
    `Cone::holds(&GalacticDisplacement)`, the cosine of a displacement with the axis at least the
    half-angle's, its edge included, tested in metres. The band's rays read the same cosine. Each
    star is tested on its direction once its position is known, before its distance and
    sightline, in both modes, so `brute_force_sky` follows with no change of its own. The cells a
    cone's plan opens and their floors are unchanged.
  - **The band** (`sky/band.rs`). `light_along` subtracts the light at cut − DM − v☉(A_V) A_V,
    the `v_extinction` of the node's own solar curves (for the tests' grey dust, A_V as before).
    `unextinguished_lux(colour, v, &Reddened)` takes the star's reddening and subtracts its own V
    extinction, so the overflow's points (`point_lux`) and the glare (`Glare::of_listed`) take a
    star's unextinguished light as M_V + DM exactly. Design note 15's formula is amended to match.
  - **Tests** (the acceptance's two-filter line is two commands, `--lib -- sky::census` and
    `--lib -- sky::band`, since `cargo test` takes one filter before `--`):
    - `a_census_with_the_eye_equals_one_without_it`: the 18 cells by the Sun at V 9, with the eye
      and without it, list the same 47 stars and tallies, bit for bit; 4 of them lie within 0.46
      mag of the cut.
    - `every_kept_star_is_brighter_than_the_cut_and_inside_the_cone`: each cell of a 30° cone's plan
      about +X within 60 ly at V 11, the eye asked, keeps 60 stars, each brighter than the cut and
      inside the cone. They are the no-cone census's stars inside it, star for star and bit for bit,
      and its oracle's. The 243 outside are left out.
    - T8.e's identity tests and T16.b's pinned systems pass unchanged in form
      (`--test sky_census`, 6 of 6). `pinned_merger`'s check of T8.b's skip adds no eye offset to
      the cut, which the constant's removal makes.
    - T9.b's conservation tests run with the eye asked and assert their totals' bits equal without
      it: the cuts 8, 7 and 6 within 0.02%, as before; against the band complete nowhere −1.11% to
      100 ly and −1.78% to 200 ly (T9.b's −1.12% and −1.79%).
    - `a_cones_listed_and_band_light_are_the_full_skys_inside_it`: a 30° cone near the Sun to V 8
      within 200 ly, at 16², lists the full census's 290 stars inside the cone, bit for bit, and
      its band's texels inside it are the full band's. Together 9.4586 × 10⁻⁵ lx, as the full
      sky's inside the cone. Its cells hold 424 more stars outside the cone, 20.0% of that light,
      which the census listed before T8.k and the band, complete nowhere outside, held too.
    - Updated, not new: the sightline-cut test reads the star's own V extinction; the post-AGB
      test's M_V, the overflow test's V and `sky::limits`' `sources_of` subtract it.
    - The cache's order tests take a 60° cone in their looser query (determinism audit), so the
      cached-equals-uncached checks cover a cone's census.
  - **Counts near the Sun, before and after** (the census within 200 ly, every cap forced; a probe,
    not committed, `.git/rm23-scratch/r06-census/t8k/zz_t8k_probe.rs`):

    | Query                  | Listed: A, B, C, D, E  | Listed                   | Generated: A, B, C, D, E            |
    | ---------------------- | ---------------------- | ------------------------ | ----------------------------------- |
    | V 8.15, eye, before    | 3, 42, 4,218, 465, 154 | 4,882 (101 past the cut) | 23,979, 8,196, 13,606, 3,628, 1,800 |
    | V 8.15, no eye, before | 4, 55, 4,157, 456, 152 | 4,824                    | 18,145, 8,196, 13,606, 3,628, 1,800 |
    | V 8.15, either, after  | 4, 55, 4,156, 456, 152 | 4,823                    | 18,145, 8,196, 13,606, 3,628, 1,800 |
    | V 7.95, eye, before    | 3, 35, 3,746, 432, 147 | 4,363 (137 past the cut) | 22,114, 8,196, 13,606, 3,628, 1,800 |
    | V 7.95, no eye, before | 4, 47, 3,636, 425, 142 | 4,254                    | 16,216, 8,196, 13,606, 3,628, 1,800 |
    | V 7.95, either, after  | 4, 47, 3,634, 425, 142 | 4,252                    | 16,216, 8,196, 13,606, 3,628, 1,800 |

    The brown dwarfs list none. At 8.15 the eye's census listed 101 blue stars past the cut, which
    the band held too, and missed 43 red ones within it (13 of them in B). The star's own V moves
    one C star past 8.15 and two past 7.95 (v★ above 1). Dropping the 0.6 mag from the floor and
    the flux bound generates 24% fewer of A's systems there; C to E generate every record either
    way.

    The sampled near-Sun bench at the bench's eye cut, 7.95 (`HYPERION_SKY_BENCH_SAMPLE=1000`,
    criterion's `--test`, release, 3 workers at `CPUQuota=400%`, unlocked, so its time is
    provisional), per layer, the records past the floor (the same in both) and the share
    generated, before → after: C 1.988 × 10⁸, 98.26% → 98.22%; D 6.514 × 10⁷, 99.92% → 99.91%; E
    1.343 × 10⁸, 99.99% → 99.99%. Accepted
    (scaled by 1,000): C 27,000 → 25,000, D 5,000 → 6,000, E 14,000 → 9,000; the sample listed 46
    → 40. A, B and the brown dwarfs open no sampled cell. The estimate fell from 1.75 × 10⁶ to 1.40
    × 10⁶ CPU-s, but the load fell too (about 15–20 before, 10–14 after), so the change is not
    measured. Near the Sun the census's cost stays generation's, which T8.g's bound addresses.

  - **Not changed.** GENERATOR_VERSION stays 20 and no golden moves (`golden_diff` 0): nothing
    generated or golden reads the census or the band. The caps (T7) still count stars brighter
    than cut − DM − A_V on each ray's bare A_V; they are an estimate, and the rule's bound takes
    the least extinction over the rays, where v★ is within about 0.01 of 1, so the gap is a few
    thousandths of a magnitude. T7.b, which counts each ray's own cap, can take each ray's v☉.
    _Decided 2026-10-07 (`decision-r06-t8k-cone.md`, item 3): T7.b does; safe meanwhile, since
    the census is exact within its caps._
  - **Open, for the orchestrator** (science check; not built, since each departs from the
    ruling's text):
    - The cone's edge is shared to a band texel, not exactly: the census tests each star's
      direction, the band each texel's centre. For a 30° cone the misplaced light largely cancels.
      A cone narrower than a texel (about 1.4° at 64²) can hold no texel centre, and then the band
      holds all of the light there beside the listed stars, near the Sun some 0.9 mag brighter than
      the light fainter than the cut. Keeping a star by its texel's centre, at the request's
      `BandSpec`, would share the split exactly. _Decided 2026-10-07 (item 1): the band's texels
      that meet the cone, R06.T8.l._
    - With a cone, no star outside it is listed, so none glares over the texels inside it, though
      the veil reaches 90°. That is right for an instrument's field stop and not for a naked eye;
      the cockpit eye asks no cone. _Decided (item 2): an eye with a cone is refused, R06.T8.l
      and T11.a._
    - In release builds a sightline's NaN A_V gives a NaN V, which `merge_census`'s release
      assertion refuses, as before (determinism audit; older than T8.k).
  - **Gates** (2026-10-07, on the code of T8.k and T8.j together, capped at `CPUQuota=400%`, load
    8–13, the heavy lock held elsewhere, so timings are provisional; logs
    `.git/rm23-scratch/r06-census/t8k/final/`):
    - fmt; clippy, workspace native and sim wasm32-wasip1 (`-D warnings`);
    - `--lib -- sky::census` 36 passed, 2 ignored (83 s, four threads); `--lib -- sky::band` 18
      passed (120 s); the rest of `sky::` 117 passed, 4 ignored (165 s);
    - `--test sky_census` 6 of 6 (20 s); sky doctests 22 of 22, `Cone::holds`' example among them;
    - Prettier on the plan; every pre-commit hook.
  - **Reviews.** Determinism audit: nothing must-fix or should-fix. Applied: `Cone::holds` takes a
    `GalacticDisplacement`, so its unit is in its type; the cache's queries take a cone. Science
    check: nothing must-fix; the kept test, the band's v☉ subtraction, the unextinguished light and
    the (v★ − v☉) A_V figures (−0.038 to +0.034 at A_V 2, −0.21 to +0.18 at 10, on every row but
    the 2,300 K dwarfs, which never sit at the boundary) are confirmed. Applied: `SkyStar::a_v`'s V secant at
    A_V → 0, 2 and 10 with its source, and the band test's reference integral cut and dimmed by
    the solar point's curves.
- **A narrow cone's region (decided 2026-10-07, `decision-r06-t8k-cone.md`).** A cone's census
  and band share the band's texels that meet it, so a cone of half-angle α lists up to
  (1 + ρ ÷ α)² its own area: 2.3 times at 2.5°, 27 at 0.3°. A sub-degree instrument field needs a
  band finer than 64² for its own background in any case. That band is the instrument plan's to
  design. No RM3 view asks a cone. _As built (R06.T8.l, science check): (1 + ρ ÷ α)² is the area
  of the cap the region's texel centres lie in, so it is the region's mean over axes, not its
  ceiling. The texels reach to α + 2ρ, and the ceiling is cap(α + 2ρ) ÷ cap(α): 1.17 at 30°, 1.64
  at 9°, 4.05 at 2.5°, 89 at 0.3° and 692 at 0.1° at 64². Measured 29 at 0.3° and 2.32 at 10° (16²),
  each a little above the mean (Risks, "Deviations in T8.l, as built")._
- **Deviations in T8.l, as built (2026-10-07).** The cone by the band's texels and no eye with a
  cone, as the cone ruling (`decision-r06-t8k-cone.md`, items 1 and 2) sets it out, with these
  details.
  - **The region** (`sky/census/query.rs`, new and public). `ConeRegion::new(cone, spec)` holds the
    cone, the band's `BandSpec` and cos(α + ρ). It gives `cone()`, `spec()`, `reach()` (α + ρ),
    `cos_reach()`, `holds_texel(face, row, column)` (the texel's centre against `cos_reach`) and
    `holds(&GalacticDisplacement)` (the texel `BandSpec::texel_of` places the direction in; a zero
    displacement held, as `Cone::holds` holds it, and a non-finite one not, which no star has).
    - ρ is `BandSpec::largest_texel_radius()` (`sky/band.rs`), atan(√2 ÷ n): 1.266° at 64² and
      5.051° at 16². A test scans every texel's corners and edge points at eight face sizes, 1² to
      64²; the science check scanned the corners to 1,024². The texels are largest at a face's
      centre, and their edges are great circles, so a corner is a texel's farthest point.
    - Every texel that meets the cone is in the region, and some that do not: at 30° (16²) 96
      texels against about 88 that meet it. The docs say "the band's texels about it" where the
      ruling says "that meet it".
  - **The query carries its band.** `SkyQuery::band_spec()` and `SkyQueryBuilder::band_spec(spec)`
    give it, `BandSpec::STANDARD` unless set. The region is computed once, when the cone or the band
    is set (`SkyQuery::cone_region()`), and the census and the band read its one cosine.
    `march_rows` refuses, in release builds too, a cone's band marched at another `BandSpec`. A
    query with no cone reads its band nowhere, so the eye cut's 16² pre-pass is unchanged.
  - **The census** keeps a star when `region.holds(observer.displacement_to(apparent))`, the
    displacement `sum_rows` places an overflow star by, so a star and its texel agree bit for bit.
    `Cone::holds` stays as the cone's own edge, which the tests read.
  - **The band.** `Reach::of(region, face, row, column)` reads `holds_texel`, the one place a ray's
    region is decided. Without a cone every ray is `Inside`, as before.
  - **The plan opens cells by α + 2ρ, not the ruling's α + ρ** (science check: should-fix, for the
    ruling's bullet). A region texel's centre lies within α + ρ of the axis and its points within ρ
    of its centre, so its stars lie up to α + 2ρ out. Cells opened by α + ρ could miss the stars on
    a region texel's far side, whose light its complete band leaves out. The plan text says only
    "the widened cone". `ConeRegion`'s private `meets_ball` widens it, plus 10⁻⁹ rad against
    rounding (`PLAN_MARGIN_RAD`; determinism audit). `Cone::meets_ball` is gone; a private
    `ball_meets_cone` serves the plan and its test. Opening more cells moves no listed star.
  - **The region's area.** The ruling's "at most (1 + ρ ÷ α)²" is the area of the cap the region's
    texel centres lie in. That is the mean over axes, not a ceiling, which is cap(α + 2ρ) ÷ cap(α)
    (the "A narrow cone's region" entry above). `ConeRegion`'s docs give both.
  - **No eye with a cone.** `BuildSkyQueryError::ConeWithEye` reads "the naked eye cannot ask a
    cone: it has no field stop". `SkyQueryBuilder::build` refuses it after the eye cut's checks,
    whichever of the two was set first. `Cone`'s docs and `SkyQueryBuilder::cone`'s call a cone an
    instrument's field stop. They state that stray light scattered into the field is not modelled,
    and that the eye has no field stop narrower than its own field (science check). The protocol's
    doc comments (`SkyRequest.cone`, `ConeDto`) and the server's `BadRequest` are T11.a's, as its
    plan text says.
  - **Tests** (`--lib -- sky::census` and `-- sky::band`). Near the Sun to V 8, every cap forced to
    200 ly, each cone's census and band against the full sky's:

    | Cone, band | Axis                                                   | Region texels, sr | ÷ the cone | (1 + ρ ÷ α)² | Ceiling | Listed | Inside the cone | Cells opened   |
    | ---------- | ------------------------------------------------------ | ----------------- | ---------- | ------------ | ------- | ------ | --------------- | -------------- |
    | 1°, 16²    | the brightest star within 200 ly 2° from every centre  | 2, 0.02940        | 30.7       | 36.6         | 122.9   | 9      | 1               | 1,653 (1.8%)   |
    | 10°, 16²   | (1, 1, 1), a corner of the cube, over three faces      | 45, 0.2219        | 2.32       | 2.27         | 4.01    | 63     | 24              | 4,124 (4.4%)   |
    | 30°, 16²   | +X                                                     | 96, 1.111         | 1.32       | 1.37         | 1.75    | 396    | 290             | 13,127 (14.0%) |
    | 0.3°, 64²  | the brightest star within 200 ly, on the rows it meets | 6, 0.002503       | 29.1       | 27.2         | 89.1    | 3      | 3               | 406 (0.43%)    |

    Cells opened are counted against the full sky's 93,440.
    - Each of the four tests asserts the ruling's five items. The listed stars are exactly the full
      census's stars of the region's texels, star for star and bit for bit (V, A_V, distance and
      position). Each region texel is the full sky's and each other the band complete nowhere, bit
      for bit. Texel by texel, the light is the full sky's to 10⁻¹² relative. A lattice of 10⁴
      directions inside the cone lies in region texels. The full census's stars inside the cone
      itself are listed. The 1° test also asserts that no texel centre lies within 1°.
    - The 30° cone's 290 stars inside the cone are T8.k's 290. Its region lists 396. T8.k's
      `a_cones_listed_and_band_light_are_the_full_skys_inside_it` (within 1%) is replaced by
      `a_30_degree_cone_shares_the_bands_texels_exactly`.
    - `a_cones_overflow_falls_in_its_region` (determinism audit, should-fix). The 10° cone's 49 stars
      within 200 ly are merged at an `n_max` of 10. Every overflowing star's texel, as the band
      places it, is in the region. Every other texel is the band complete nowhere, bit for bit, and
      each region texel's light is the full sky's to 10⁻¹² relative (26 texels hold points).
    - Also new: `the_largest_texel_radius_is_the_farthest_point_of_any_texel_from_its_centre`,
      `a_cones_band_at_another_resolution_is_refused`, `an_eye_with_a_cone_is_refused` and
      `a_cones_region_is_of_the_querys_band`. `ConeRegion`'s doctest: a 1° cone at a 16² face's
      centre holds no texel centre, but its region holds the four texels about it.
    - Changed: `a_cone_is_complete_only_in_its_region` (8²; it was `…_inside_it`) and
      `every_kept_star_is_brighter_than_the_cut_and_in_the_cones_region` (cell; 65 kept, 272 left
      out). Both take the region and no eye. `a_cone_keeps_only_cells_whose_box_meets_it` takes the
      widened cone and asserts it opens cells the cone alone would not. `every_refusal_names_its_field`
      takes `ConeWithEye`. The cache's looser query keeps its 60° cone without the eye.

  - **wasm32's 32-bit `usize`** (the orchestrator's sweep after CI-53's T9.i overflow, 218f0cb9).
    `march_rows`' slot count, rows × face side × slots a ray, is a checked product now (T9.f's
    code). Every other `usize` product or sum in T8.l's code and tests, and in the T9.f and T9.j
    code it touches, is bounded by a face's 1,024² texels, a test's few thousand stars, or a
    `Vec`'s own bytes (`heap_bytes`). T9.i's `assert_within_tolerance` multiplied its pairs in
    `usize` too, 300,000 × 24,576 in its slow test; that is fixed in a commit of its own.
  - **Owed elsewhere.** T7.b's caps at v☉ (ruling item 3) are T7.b's, as its plan text says. The
    ruling's §1 bullet "cells are opened by the widened cone, α + ρ" and its cost table's "at most"
    are for the orchestrator to correct in the decision record.
  - **Not changed.** GENERATOR_VERSION stays 20 and no golden moves (`golden_diff` 0). No query
    without a cone changes: every ray is `Inside` and no star is tested.
- **Deviations in T8.j, as built (2026-10-07).** The census in motion, as the pad-speed ruling
  (`decision-r06-pad-speed.md`, §4 item 6) sets it out, built with T8.k, with these details.
  - The moving galaxy is the fixture cloned and built `with_full_potential`, in a `OnceLock` that
    T8.f's moving-galaxy test now shares. Its parameters are the fixture's, so its offsets are.
  - `the_census_plan_holds_every_record_its_caps_see`: the walk is a cube of cells about each
    observer, keeping each cell whose box comes within the cap plus the pad at 5,000 km/s
    (`cells_meeting_ball`, test-only), not plan 03's `cells_in_sphere`. It checks 63,715 records
    (the task's "some 10⁴"; asserted at least 30,000), 27,681 of them within their caps, each
    in a planned cell. The fastest records move at A 295, B 279, the brown dwarfs 293, C 334, D
    307 and E 463 km/s. Each lies within its cell's pad at the observer's time, at the
    retardation's first guess and at its emitted time. The task's parenthetical calls the
    observer's time the first guess; the first guess is the observer's time less the present
    distance's light time, from which the light's age is taken (science check), and the test
    checks all three.
  - The slow `the_census_is_its_oracle_150_ly_from_the_sun_in_motion` (`tests/sky_census.rs`,
    `agree_in` with the galaxy, helpers `moving` and `observer_near_sun_at` in
    `tests/common/sky.rs`) passes: the census generates 32,917 of 42,076 systems (39,943 at the
    epoch at rest) and agrees with its oracle bit for bit. 22 s in the slow-test profile, unlocked,
    at load about 10. Native only, as the other slow identity tests are.
  - `CellReach::of`'s doc names P08.T17's assertion in `epoch_velocity`.
  - **Gates**: T8.k's, which ran on both; the motion test is among `sky::census`' 36, and the slow
    test ran by name in the slow-test profile, 23 s.
  - **Not changed.** No output moves, GENERATOR_VERSION stays 20 and no golden moves: the task
    adds tests and helpers only.
  - **Reviews** (with T8.k's). Determinism audit: nothing to fix; the moving galaxy's `OnceLock`
    and the walk's `BTreeSet` are order-free. Science check: the pad's fixed point,
    β(|t| + far + offset) ÷ (1 − β), and the walk's pad over the earliest emitted time are
    confirmed. Applied: the first-guess check above.
- **Deviations in T8.m, as built (2026-10-07).** The phase envelope, as the hierarchy-bound
  ruling's T8.m block sets it out, with these details. The coordinator approved the storage and
  the four deviations below on 2026-10-07, after the size measurement, on five conditions, each
  met:
  - a versioned, documented format (format 1, below);
  - its lengths checked against its header in `PhaseEnvelope::fitted`, decoded once, with no
    allocation per reading;
  - fit-check and the sim fingerprint cover the packed bytes, and a refit reproduces them;
  - a test that decodes the table and spot-checks cells against tracks sampled now;
  - the file under about 400 kB with margin. The fit crate's
    `the_committed_table_stays_under_its_ceiling` holds it there, so a refit past 400 kB fails
    before the 500 kB hook and needs a new ruling.
  - **The size.** At the ruled resolution a cell's row needs some twenty segments even merged
    within 0.2 mag, since the giant branches climb several magnitudes within the fine bins (a probe
    of every cell, 2026-10-07). As fixed bins the table would be some 17 MB of text. As merged
    runs of readable decimal integers it would be 0.9–1.5 MB, over the 500 kB hook, which is never
    skipped. The one readable option under about 400 kB halves both the mass and the \[Fe/H\]
    intervals: its mean looseness over bright bins (V < 0) is 0.23 mag against the cells' own rows,
    and the ruled cells packed are 0.07 mag.
  - **Format 1** (`sky::phase::FORMAT`, `tables::sky_phase_envelope`). In order:
    - `FORMAT`;
    - `DIMENSIONS`: mass nodes 198, \[Fe/H\] nodes 12, η nodes 4, bins 322, distinct rows 3,349,
      segments 66,112;
    - `DARK` (`i16::MAX`);
    - `LIFETIMES`: per node of mass, \[Fe/H\] and η, in that nesting, a `u16` in 10⁻⁴ dex of
      log₁₀ years below the cap of 2 × 10¹⁰ years, 0 at the cap;
    - `CELL_ROWS`: per cell (mass, then \[Fe/H\], then η interval), its row among `ROWS`, from 0;
    - `ROWS`: one distinct row a line, its segments in order of relative age. Each segment is four
      base64 characters (RFC 4648, no padding) of three bytes: the run of bins it covers (1–255),
      then its value as a little-endian `i16`, the brightest M<sub>V</sub> of its bins, margin
      included, in integer millimagnitudes rounded brighter, or `DARK`. A row ends where its runs
      reach the last bin.

    `PhaseEnvelope::fitted` checks the format, the axes and the decoded counts against
    `DIMENSIONS`, panics on a mismatch, and decodes once. A reading allocates nothing. The file is
    375,389 bytes. Fit-check's body hash covers the packed rows, and a refit reproduces them byte
    for byte. `a_handful_of_cells_are_the_fit_of_todays_tracks` decodes the table and holds three
    mass intervals' cells, every \[Fe/H\] and η interval, segment for segment to tracks sampled
    now.

  - **The relative age** is the star's age over the table's own lifetime. That lifetime is the
    death age of the generator's track at each node (median draws but η), capped at 2 × 10¹⁰
    years, interpolated linearly in ln L within the star's cell. The fit and the reader take it
    alike, so no bracket is read.
    - The ruled `FittedFates::lifetime_bracket` answers only within its η nodes, −3σ to +3σ
      (±2.4σ above 8 M☉), so not at η = 0 or ±3.5σ, both inside the table's span. It starts at
      0.741 M☉, refuses changes of route, allocates, and reaches ±1.4% at its 99th percentile
      below 2.5 M☉ and ±2% in a usable cell (14–20 fine bins).
    - The lifetime keeps η, which moves it by up to 0.0051 dex at a node.
    - A star below 0.741 M☉ is bounded as any other. Below 0.1 M☉, and where a track outlives
      the cap, it is read against the cap, so its relative age never reaches the fine bins.
  - **The η nodes** are −0.5 ÷ 0.07 (η = 0), −3.5σ, the median and +7σ: three intervals. The
    brightness envelope's +3.5σ node is merged into [0, 7], which is conservative. It would split
    only the 2 × 10⁻⁴ of stars above it, whose winds are stronger.
  - **Runs merged** within 0.2 mag of their brightest (`MERGE_TOLERANCE_MAG`), growing by 0.2 mag a
    mag fainter than M<sub>V</sub> +10. Only layer A (cap about 11 ly) and the brown dwarfs (2 ly)
    could list stars that faint. A bright value is at most 0.2009 mag brighter than a bin it holds,
    beyond the margin, and on average about 0.07 mag.
  - **A spread in relative age per cell**, set by the slow test's findings (the table widens,
    never the test). Its first run found 53 stars brighter than the table in 1.2 × 10⁶ checks:
    - a 6.5 M☉ TP-AGB star at relative age 1.006, where the table was dark. Plan 06's
      companion-stripped mark moves a death in 5.5–11 M☉ by up to 1.05%;
    - a 1 M☉ star at η +4.7, which becomes a helium star and lives 1.3% past the table's lifetime.
      The lifetime kinks between its η nodes at that change of route;
    - massive stars (58–104 M☉) in the Hertzsprung gap and core helium burning, by up to 1.4 mag.
      Their brief excursions move by up to 3% of the lifetime over 0.18 dex of \[Fe/H\], and by a
      bin or two between mass samples;
    - one 2 M☉ giant 0.026 mag past the margin.

    So within 5.5–11 M☉ each sample is the brighter of the star with and without the stripped
    mark. Each cell is widened by the least spread of `SPREADS` (0.05% to 6.4% of the relative
    age) under which each of its samples with neighbours on both sides along an axis is bounded
    by those two alone, margin included: a test at twice the samples' spacing. The cells' spreads
    are:

    | Spread | 0.05% | 0.1% | 0.2% | 0.4% | 0.8% | 1.6% | 3.2% | 6.4% |
    | ------ | ----- | ---- | ---- | ---- | ---- | ---- | ---- | ---- |
    | Cells  | 4,662 | 5    | 826  | 447  | 391  | 87   | 31   | 52   |

    So 72% of the cells keep the fine bins' resolution, and the rest take their spread's. All 52
    cells at 6.4% passed no spread: their samples differ by more than the margin at twice their
    spacing whatever the spread, so they take the largest, and only the slow test holds them. 64 cells shine beyond 1.05 after widening, which the last bin holds.

  - **The slow test**, `phase_envelope_bounds_dense_tracks` (`tests/sky_phase_envelope.rs`, 44 s).
    It checks 10⁵ single stars, log-uniform in mass over 0.0124–150 M☉, \[Fe/H\] −2.6 to +0.4, an
    η draw from −7.5 to +7σ and a realised star's other draws, each at 12 ages: a third
    log-uniform over 10⁴ years to 1.5 × 10¹⁰ years and two thirds in the fine bins.
    - On the final table no star is brighter than it: no violation in 1.2 × 10⁶ checks, which
      bounds the rate of violations under the test's sampling below 2.5 × 10⁻⁶ at 95%.
    - **The margin used** is at most 0.068 mag of the 0.3 mag margin (`MARGIN_MAG`; a 90 M☉
      star in core helium burning), and 8 of the 1.2 × 10⁶ checks use any of it. That seed is in-sample:
      its first run set the spreads and the stripped mark's sampling. An independent seed
      (`0x7a3_1f00_5eed_0002`, the stars' own seeds offset by 9 × 10⁶), run once, used at most
      0.006 mag.
  - **The fit.** `hyperion-fit`'s task `sky_phase_envelope` (slow class, with a smoke manifest of
    the two intervals either side of 0.1 M☉) takes 72 s wall and 266 CPU-s on 4 threads at load
    4–7 (provisional), for 253,736 samples (each interval sampling its own ends), 197,225 of them
    tracks. Its fingerprint probes the mass nodes and eleven samples' lifetimes and parts, two of
    them in the electron-capture window.
  - **The reader.** `PhaseEnvelope::fitted` copies and decodes the table once. Then:
    - `brightest(mass, composition, η, ages)` reads the star's cell, its lifetime there, the bins
      its relative ages span, and a binary search of the cell's segments;
    - `lifetime(…)` gives the lifetime read;
    - `sky::testing::phase_envelope()` serves the crate's tests.

    A positive mass outside 0.0124–150 M☉ reads the nearest cell. A NaN or non-positive mass, or a
    helium excess, reads the table's brightest value, which skips nothing. An empty range of ages,
    or a NaN end, reads nothing, as the brightness envelope's does. A draw
    above +7σ (one in 10¹²) is read at +7σ, outside the slow test, as the brightness envelope
    reads it.

  - `sky::envelope` shares its phase-cutting with this table (`track_parts`, a bit-exact
    refactor of `enter_track`): `sky_envelope`'s fast refit reproduces its body unchanged.
  - **Helium excess.** The table is built at ΔY = 0, as `sky_envelope` is, and reads a
    composition by its clamped Z alone. `tables::helium` (P06.T17) is the identity today, so it
    bounds every star; the ruled bracket would refuse any ΔY ≠ 0. A star with a helium excess reads
    the table's brightest value, which skips nothing. When P15.T7 fits `tables::helium`, the table
    must be refitted with helium-excess nodes. That axis would grow the table past the approved
    ceiling, so its storage needs a ruling before P15.T7 lands.
  - **The mass axis.** Format 1 stores no masses. It reads `tables::sky_envelope::MASSES`, checked
    by count in `fitted()`, by value in `every_cell_of_the_fitted_table_covers_every_bin`, and by
    the fingerprint's `mass_nodes` probes (count, ends and sum). A refit of `sky_envelope` on new
    nodes needs a refit of this table.
  - **Reviews (2026-10-07).** The determinism audit found nothing that moves output, and asked for
    fingerprint probes inside the electron-capture window and among the 50–110 M☉ stars (6.5, 8
    and 80 M☉ added) and a window interval in the spot-check (6.4–6.8 M☉ added). The science check
    found no constant wrong; its doc corrections are applied, and its one finding of looseness is
    below. The Rust review's must-fixes (a documented panic in `quantize_lifetime`, and discarded
    `fmt` results) and should-fixes are applied: typed sample and lifetime functions, private
    cell fields, `pub(crate)` internals, a range-read test, a test of each malformed table, and a
    NaN lower age or a non-positive mass made safe.
  - **Deferred corrections, looseness only (reported to main).**
    - Stars of 1.9–3.3 M☉ at \[Fe/H\] −0.5 to +0.18 leave their main sequence at relative age
      0.77–0.80, inside the last coarse bin, 0.64–0.80. So a third of their main sequence is bounded
      1.5–2 mag too bright (a 2.2 M☉ star at M<sub>V</sub> +0.5, listable to about 1,000 ly at cut
      8, takes a bound listable to about 2,300 ly). Fine bins from 0.75 would cure it, against the
      ruled 0.8.
    - Any spread widens a coarse bin by a whole neighbour on each side, about ±25% in relative age,
      where the spread asks for 0.05–6.4%. Widening each part's ages before binning, rather than
      the bins, would keep the coarse bins' resolution.
    - A pre-main sequence is cut into 32 equal parts with no knots, so its first part carries the
      birthline's brightness: 0.15 M☉ is bounded at M<sub>V</sub> 9.19 to about 11 Myr, some 2 mag
      looser than the star at 10 Myr. Cutting it at the coarse bins' edges, as the cooling fits are
      cut, would cure it.
  - **Not changed.** No generated output moves: `GENERATOR_VERSION` stays 21 and no golden moves.
    Nothing generated reads the table yet; R06.T8.g will.
  - **Gates (2026-10-07).** fmt; clippy, native and wasm32-wasip1; `--lib -- sky::phase` and
    `sky::envelope`; `--lib -- sky::`; the doctests of `sky::phase`; the fit crate's tests;
    `just fit-check`, fresh; `sky_envelope`'s refit, body unchanged; and the slow test by name.
    `just ci` is the orchestrator's (Day 2 protocol).
  - **Open, for T8.g.** The share of records the table lets T8.g reject is unmeasured until T8.g.
    The widened cells (28%), the 0.2 mag merging, the three η intervals and the deferred
    looseness above all loosen it against the ruled design, which could not be stored.
- **Deviations in T8.g, as built (2026-10-07; written against P11.T17.a, its gates taken after
  P11.T17.c).** The bound star by star, as the hierarchy-bound ruling sets it out (§§5–7). The
  ruling's §9.2 let it be written against P11.T17.a's interface before P11.T17.b and T17.c land.
  So it is committed with every `PairLight` verdict handled, and its final gates wait for T17.c:
  the cold estimate, the 1% target, D's and E's quarter, the camera's figure and the
  `--serve-sky` default. A fresh agent takes them (the census handoff's T8.g section). They were
  taken on 2026-10-08, as re-stated that day (`decision-p11-t17c-bright.md`): the cold figure is
  recorded, not gated, D's and E's shares are recorded, and the default moved to R06.T11.d (below,
  "The final gates, at P11.T17.c's answers").
  - **What is built** (`sky/census/cell.rs`). `StarBounds` is a record's light bounded star by
    star, in the plan's order:
    - `draw_metallicity`;
    - `hierarchy_bound`;
    - `pair_light_bound` per pair of two stars over the record's light-time ages;
    - then `PhaseEnvelope::brightest` per star, at its mass, \[Fe/H\], η and age.

    Each star's light is a `StarLight`:
    - `Own`, for no pair, or a `Detached` or `Unchanged` one;
    - `OwnOr(M)`, for a `Bright(M)` pair: the brighter of its own bound and M, and M where its own
      model is dark;
    - `Dark`, for a `Remnants` pair.

    A pair plan 11 cannot bound makes the whole record `RecordLight::Unbounded`. Each η is the
    star's own: `StarDraws::eta_for_attempt` at its attempt, and the primary's `primary_eta`.
    The primary is bounded once, by the least light that bounds it at every attempt, and by its
    own bound where the fallback is listed. For a grid record the primary is never `Dark`: the
    cover ends at an attempt with no pair the engine may run, where the primary is its own model,
    or lists the fallback.

  - **Two tiers, before and after the drift.** The widened envelope's bound at `max_star_mass`
    over ages from zero (T16.b) stays first for every record, not only those with a pair plan 11
    cannot bound, since it costs one read. The star bound is taken only for records it passes.
    Both are bounds, so a record that either rejects is skipped. A record with an unbounded pair
    is generated whenever the widened bound passes, before the drift and after `retarded`.
  - **The verdicts are taken once.** Each pair's verdict is taken over the light-time ages before
    the drift (T8.f's step 5: the pad, plus a year of slack), and reused after `retarded`. There
    each star's own bound is read at the exact emitted age. `StarBounds` keeps that window.
    `brightest` debug-asserts its ages lie within it. The census generates the record if the
    emitted age ever falls outside it, which costs one comparison. Over 10⁴ records in a moving
    galaxy, the window always holds the emitted age, and the star bound before the drift never
    rejects what it passes after (`the_bound_before_the_drift_never_rejects_what_the_bound_after_it_passes`).
  - **The phase envelope is read process-wide**, through `PhaseEnvelope::shared()`, a `OnceLock`
    of `fitted()`. It is not a `SkyContext` field: `query.rs` is T7.b's in parallel, and the table
    depends on no galaxy. A process decodes it once, about 0.5 MB. The server's `SkyTables`
    budget does not count it. `the_shared_envelope_is_the_fitted_table` pins it. It takes the
    sim-determinism skill's lazy-value exception, as `LifetimeBracket::shared` does. The
    determinism audit asks that the skill name a static of a constant table explicitly.
  - **The tallies.** `LayerTally` gains `star_bounded`, `unbounded_records` and `pairs`, a
    `PairTally` of the verdicts. The sampled bench prints each layer's shares.
  - **The wasm32 golden** asked by T8.m's determinism audit: `sky/phase_envelope`, blessed at 21.
    It holds:
    - the reader's lifetimes as bits, at 22 masses on and between nodes × 4 \[Fe/H\] × 5 η;
    - its readings at 13 relative ages and over 4 ranges, each held bit for bit to its integer
      millimagnitude ÷ 1,000, with the bins of each end;
    - 256 random queries.

    It passes on wasm32-wasip1 under wasmtime.

  - **Files beyond `cell.rs`:**
    - `sky/census/mod.rs`, the re-exports;
    - `sky/phase.rs`, for `shared()` and the golden;
    - `sky/testing.rs`;
    - `sky/photometry.rs`, whose A4 note names both bounds that rest on dark white dwarfs;
    - `tests/sky_census.rs`;
    - `benches/sky.rs`: `sky/star_bound/*`, `census_near_sun/cold_camera` and the printed
      shares;
    - `.config/nextest.toml`, four slots for the realised-systems tests.
  - **Tests.**
    - The synthetic verdicts: `each_verdict_leaves_the_stars_the_light_the_ruling_gives` feeds
      every `PairLight`, and none, through a test-only seam (`with_verdicts`), against
      `ruled_light`, a separate reading of the ruling, on a triple of D and a binary of C.
      `the_primary_is_bounded_once_by_the_least_light_that_bounds_it_at_every_attempt` takes a
      fallback record of E. `each_bound_star_is_the_generators_star` holds masses and η bit for
      bit.
    - The fast `the_star_bound_holds_for_some_realised_systems` takes 300 records of each layer
      at each place. The census's tallies are counted, and its fast oracle tests pass unchanged.
      Within 150 ly the census now generates 13,430 of 39,943 systems, and 47,314 of 49,534 for
      A and B within 1.5 ly in the nuclear disc.
    - The slow `the_star_bound_holds_for_realised_systems` (in `cell.rs`) takes 10⁵ records of
      each of A–E and the brown dwarfs, near the Sun and in the bulge, at the epoch and 900 years
      before. Each pair's verdict is taken at the system's exact age, the tightest window the
      census asks. Every star of the kept attempt is held to its own bound, or to the widened
      envelope for a record with an unbounded pair, and at least 10⁵ systems of each layer are
      realised.
      - It passed in 825 s on four threads (slow-test profile, under the heavy-test lock, load
        about 6–13). It realised 1.2 × 10⁶ systems and found no violation.
      - It checked 1.60 × 10⁶ shining stars against their own bounds and 2.6 × 10⁵, of records
        with a pair plan 11 cannot bound, against the widened envelope.
      - No star used any of the phase envelope's 0.3 mag margin. The least slack, M<sub>V</sub>
        less its bound, is 0.3045 mag, in B in the bulge.
      - Of the pairs, plan 11 bounds 99.7% in A, 98% in B, 75% (Sun) and 69% (bulge) in C, 24%
        and 23% in D, and 0.7% and 0.5% in E. So the stars E checked by their own bounds are only
        336 near the Sun and 334 in the bulge.
    - `tests/sky_census.rs`:
      - B's skip check is restored.
      - B's and C's quarter shares are asserted against the oracle's generated count. D's and E's
        are printed until T17.c: under P11.T17.a, D's and E's pairs are mostly `None`.
      - The pinned mergers are also held to the star bound; all three are `Unbounded` under
        T17.a.
      - The slow identity tests pass by name (2026-10-08, under the heavy-test lock, eight
        workers each, load 3–15):
        - `the_census_is_its_oracle_1000_ly_from_the_sun`, in 781 s. The census generates
          455,965 of 1,411,018 systems: C 254,273 of 1,097,274 (23.2%), D 137,122 of 238,967
          (57.4%) and E 64,570 of 74,777 (86.4%). It accepts C 20,103, D 7,112 and E 1,158
          stars, the oracle's, at version 21. T16.b's run generated 1,411,002.
        - `the_census_is_its_oracle_for_the_dwarfs_near_the_sun`, in 4.6 s. It generates 2,720 of
          249,096 systems (189,189 under T16.b): A 788 of 138,945 and B 1,932 of 110,151 (1.75%).
          It accepts A 4 and B 48 stars.
        - `the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc`, in 2,584 s. It generates
          655,481 of 878,168 systems: D 227,812 of 383,439 (59.4%) and E 427,669 of 494,729
          (86.4%).
  - **The provisional figures, at P11.T17.a's answers.** Unlocked, inside a 400% CPU quota, at load
    7–10, so provisional:
    - **A record's cost** (`sky/star_bound`, the 400 records of each layer nearest the Sun), µs,
      in A–E:

      | Step                      | A    | B    | C    | D    | E     |
      | ------------------------- | ---- | ---- | ---- | ---- | ----- |
      | `draw_metallicity`        | 0.06 | 0.06 | 0.06 | 0.06 | 0.06  |
      | `hierarchy_bound`         | 4.8  | 6.9  | 17.8 | 51.9 | 143   |
      | the pairs' verdicts and η | 0.33 | 0.44 | 0.81 | 1.39 | 2.04  |
      | the phase reads           | 0.33 | 0.34 | 0.30 | 0.09 | 0.015 |
      | the whole                 | 5.1  | 7.7  | 16.9 | 53.2 | 159   |

      Every step but `hierarchy_bound` is well inside its 6 µs. The phase reads in D and E are
      few, because a record with a pair plan 11 cannot bound reads none. `hierarchy_bound` is
      P11.T16's own: 4.7, 7.1, 14.8, 50.2 and 142.9 µs in its bench.

    - **The sampled cold census** (`HYPERION_SKY_BENCH_SAMPLE` 1,000, criterion's `--test`, three
      workers, 486 s wall for the sample): **1.44 × 10⁶ CPU-s estimated**, against T8.f's
      1.83 × 10⁶. The ruling expected about 1.2 × 10⁶ with P11.T17.a alone. The caps are the
      spherical ones: C 8,193, D 9,925 and E 21,369 ly. Near the Sun:

      | Near the Sun, estimated                    | C          | D          | E          |
      | ------------------------------------------ | ---------- | ---------- | ---------- |
      | Records past the floor                     | 1.99 × 10⁸ | 6.51 × 10⁷ | 1.34 × 10⁸ |
      | Bounded star by star                       | 98.2%      | 99.9%      | 99.99%     |
      | Of those, with a pair plan 11 cannot bound | 17.8%      | 55.7%      | 86.0%      |
      | Pairs `Detached`                           | 72.4%      | 22.9%      | 0.59%      |
      | Pairs none                                 | 27.6%      | 77.1%      | 99.4%      |
      | Generated                                  | 20.8%      | 55.9%      | 86.3%      |
      | (T8.f's sampled bench)                     | (98.3%)    | (99.9%)    | (99.99%)   |

      C to E together generate 48.6% of their records past the floor. `Unchanged`, `Remnants`
      and `Bright` are 0%, since T17.a answers none of them.

    - **A finding for the 1% target (reported to main; deferred, it does not hold T8.g).** The
      records whose every pair is bounded are generated at about 3.3% in C, at least 6.6 × 10⁶
      of its 1.99 × 10⁸ (the generated less every unbounded record), and at least 0.3% in D and
      in E. The ruling's proxy expected 0.24%, 0.15% and 0.014%. The sample's C census accepts
      some 25,000 stars from those 4.1 × 10⁷ systems.
      - So C's own bounds alone generate about 1.7% of the records of C to E, beyond the 1%
        target, whatever P11.T17.c's tables give.
      - The likely cause is T8.m's recorded looseness, all deferred corrections:
        - stars of 1.9–3.3 M☉ bounded 1.5–2 mag too bright over a third of their main sequence;
        - a spread widening a whole coarse bin;
        - the merging and the three η intervals.
      - It is unmeasured by cause. The cold estimate is not at risk: with every pair as tight as
        its stars' own bounds, these figures give about 3.7 × 10⁴ CPU-s at the spherical caps.
        That is about 2.6 × 10⁴ of hierarchy bounds, 1.0 × 10⁴ of generation and the rest,
        inside the 6 × 10⁴ gate and R06's re-quoted 3.6–4.4 × 10⁴.
  - **The final gates, at P11.T17.c's answers (2026-10-08; `decision-p11-t17c-bright.md`).**
    `rendering-and-planets` was merged by name at b6cb51b1, with P11.T17.c (4b7c6920), T7.b, T8.h
    and T8.i. `GENERATOR_VERSION` is still 21. No census code changed: `StarLight::of` and
    `PairTally::count` already took every verdict. The bench gained
    `census_near_sun/cold_eye_visibility` and `/cold_spheres`. With T7.b in, `cold` plans on T7.b's
    caps by ray, so the spheres the gate was first set at, and the caps the server asks of an
    eye-only request, each need a bench of their own.
    - **The slow realised-systems test fails, on one system (reported to main; a finding for
      P11.T17.c or plan 11's engine, not fixed here).**
      - In D in the bulge, `SystemId(0x61fec2d802000037)` (Grid, Bulge, primary 6.286 M☉,
        8.70 Gyr, \[Fe/H\] +0.556, attempt 0) holds two stars on the helium main sequence. They
        were born at 6.286 and 5.588 M☉ and shine at M<sub>V</sub> 2.52 and 1.30. Star 1's pair
        reads `Remnants`, so its bound is `Dark`, at the epoch and 900 years before.
      - A helium star that bright lives about 1–2 Myr (Hurley, Pols and Tout 2000, eq. 79). The
        longest helium main sequence is about 1 Gyr, near 0.32 M☉. So two such stars at 8.7 Gyr
        look like an engine state that T17.b's tables never sampled, not only a table miss.
      - A probe (uncommitted, kept in the worktree for the P11 lane) counts instead of panicking.
        It finds this system alone, 2 star checks of 64,094 in D's 10⁵ bulge systems, and none
        in E's or the brown dwarfs' there. The test checked A–E and the brown dwarfs near the Sun
        and A–C in the bulge before it stopped. So it is 1 system of 1.2 × 10⁶, with no margin
        used elsewhere: the least slack is 0.3045 mag, in B in the bulge.
      - The census could therefore drop such a star from a sky within reach of it. T17.c's tables
        widen; the test does not (the census handoff). A P11 lane diagnoses it, widens the bound
        at version 21 and re-runs the test. T8.g lands after that fix.
      - _Resolved 2026-10-08 by P11.T17.c's fix (66c59b29, plan 11's T17.c as built)._ The pair
        is F15's: it reaches the engine's 64-segment cap at 78 Myr, and its last segment holds
        both helium stars alive at every later age. The fix is a read-side floor, the brightest
        hydrogen or helium main-sequence star of at most the pair's mass, for any pair that may
        have interacted by the window's end. It keeps inclusion monotonicity and moves no
        generated system. With it, `the_star_bound_holds_for_realised_systems` passes over
        1.2 × 10⁶ systems, the bulge D case included, under the heavy-test lock. The floor costs
        generation near the Sun: D 48.98% → 52.55% and E 69.37% → 84.06% of records, C
        unchanged, about 18% more cold CPU-s, almost all in E. The cold figures below were taken
        before the fix.
    - **The identity tests pass**, under the heavy-test lock (3,705 s for the four):
      - `the_census_is_its_oracle_1000_ly_from_the_sun`, in 625 s. It generates 436,474 of
        1,411,018 systems: C 247,662 of 1,097,274 (22.6%, asserted under a quarter), D 126,463 of
        238,967 (52.9%) and E 62,349 of 74,777 (83.4%). It accepts C 20,103, D 7,112 and E 1,158,
        as before. The ruling expected about 45–50% and 80–85%, so D lies a little above, and
        both lie below T17.a's 57.4% and 86.4%.
      - `the_census_is_its_oracle_for_d_and_e_in_the_nuclear_disc`, in 2,536 s: D 212,950 of
        383,439 (55.5%) and E 410,896 of 494,729 (83.1%), against 59.4% and 86.4% before.
      - `the_census_is_its_oracle_for_the_dwarfs_near_the_sun`, in 3.4 s: as before, A 788 of
        138,945 and B 1,932 of 110,151 (1.75%).
    - **The cold census, recorded as a miss** (`HYPERION_SKY_BENCH_SAMPLE` 1,000 by block,
      criterion's `--test`, at 7.95, under the heavy-test lock, load 1–15).
      - A census's CPU-s is its jobs' wall time summed. So 15 workers on the dev machine's 8
        cores with SMT read about 1.28 times what 3 workers at `CPUQuota=400%` read, with the same
        tallies. The ruling's spherical estimate is calibrated on T8.g's 3-worker run. Its T7.b
        estimate is scaled from T8.h's 15-worker run.

      | Near the Sun, at 7.95           | Cells      | CPU-s, 15 workers | CPU-s, 3 workers | Ruling's estimate               | Gate first set | Miss, 15 (3) workers |
      | ------------------------------- | ---------- | ----------------- | ---------------- | ------------------------------- | -------------- | -------------------- |
      | T7.b's caps by ray (`cold`)     | 1.04 × 10⁸ | 9.46 × 10⁵        | 7.43 × 10⁵       | 0.4–1.0 × 10⁶ (uniform 0.83)    | 2.5 × 10⁴      | 38× (30×)            |
      | T7.b's caps by visibility       | 8.70 × 10⁷ | 7.16 × 10⁵        | —                | (0.52 × 10⁶ at the eye's 8.282) | 2.5 × 10⁴      | 29×                  |
      | R06.T7's spheres                | 1.08 × 10⁸ | 1.45 × 10⁶        | 1.13 × 10⁶       | 0.8–1.3 × 10⁶ (central 1.08)    | 6 × 10⁴        | 24× (19×)            |
      | The camera's 10.06, caps by ray | 1.46 × 10⁹ | 2.80 × 10⁶        | —                | 5–10 × the eye's                | none           | —                    |
      - Like for like, each figure lies inside the ruling's range: by ray at 15 workers near
        its top, and the spheres at 3 workers near its centre (against 1.44 × 10⁶ at P11.T17.a).
        The camera's census is 3.0 times the eye's, under
        the ruling's 5–10. Its walk alone is 994 s on one thread.
      - A generated system costs 6.4–7.1 ms at 3 workers and 8.1–9.2 ms at 15. E is
        most of it: on the spheres E generates 9.2 × 10⁷ of the 1.6 × 10⁸ systems generated.
      - T8.g's figures were taken before the P11 lane's bulge fix, which should move them
        negligibly. That lane reports its effect.

    - **Each layer's generated share**, beside T17.a's (the spheres): C 19.9% (20.8%), D 48.9%
      (55.9%) and E 64.1% (86.3%). None rises, so there is no finding for P11.T17.c. The caps by
      ray give C 19.7%, D 49.0% and E 69.4%; by visibility 19.9%, 48.9% and 71.3%; the camera
      20.7%, 49.7% and 77.5%. Against the ruling's central 18.9%, 42.8% and 63.5%, C and D lie
      just above its unfavourable 19.5% and 44.4%, and E is near its centre.
    - **The pairs' verdicts** (the spheres), `Detached` / `Unchanged` / `Remnants` / `Bright` /
      none: C 72.0 / 0.01 / 0.45 / 25.2 / 2.3%, D 20.9 / 0.59 / 16.2 / 58.7 / 3.6% and E 0.56 /
      1.07 / 33.0 / 63.2 / 2.1%. Records holding a pair plan 11 cannot bound fall from 17.8%,
      55.7% and 86.0% to 2.1%, 6.0% and 9.4%.
    - **The 1% target is missed**, and recorded so: C to E generate 40.0% of their records past
      the floor on the spheres, 35.3% by ray, and 35.7% by visibility. That is 35–40 times the
      target, against the floor of about 1.7% from C's own bounds. Its levers are deferred
      (`deferred-corrections.md`, "Census cost").
    - **A record's cost** (`sky/star_bound`, under the lock, load about 1), µs:

      | Step                      | A    | B    | C    | D    | E    |
      | ------------------------- | ---- | ---- | ---- | ---- | ---- |
      | `draw_metallicity`        | 0.06 | 0.06 | 0.06 | 0.06 | 0.06 |
      | `hierarchy_bound`         | 4.4  | 6.9  | 15.9 | 51.9 | 138  |
      | the pairs' verdicts and η | 0.39 | 0.45 | 0.73 | 1.25 | 2.72 |
      | the phase reads           | 0.27 | 0.32 | 0.38 | 0.43 | 1.16 |
      | the whole                 | 5.1  | 7.7  | 17.1 | 55.3 | 142  |

      Every step but `hierarchy_bound` is at most 2.7 µs, inside its 6 µs. T17.c's table reads
      add some 0.7 µs in E, and the phase reads grow because far fewer records are unbounded.
      Of the 400 records nearest the Sun, 7, 33 and 54 of C, D and E are unbounded (50, 210 and
      362 at T17.a).

    - **The other gates:** fmt; clippy `-D warnings` over the sim's targets, natively and on
      wasm32-wasip1; `--lib -- sky::census sky::phase`, 82 passed; `--test sky_census`, 6 passed.
      A Rust review of the bench's change found one must-fix, a borrowed query cloned inside its
      helper, and two should-fixes, `#[must_use]` and a citation; all three are applied.
  - **Reviews (2026-10-07).**
    - The Rust review's must-fix is applied: magnitudes are typed, `Magnitudes` in `may_list`.
      So are its should-fixes:
      - the window kept in `StarBounds`;
      - the hierarchy's primary debug-checked against the record;
      - no heap buffer per attempt;
      - `shared()`'s `# Panics`;
      - `pub(crate)` internals;
      - `unbounded_records`.
    - The determinism audit found nothing that moves output. Its should-fix is applied: the
      golden's readings are pinned bit for bit, with their bins.
    - The plan-conformance review found no must-fix. Its should-fixes, a per-layer count in the
      slow test and this record, are applied, and so are its considers: the release guard on the
      window, and the A4 note.
    - The science check found no must-fix in the composition.
  - **A finding for P11.T17 (the science check; reported to main, not fixed).** `Detached` says
    each star is its own single-star model. But a member below 0.1 M☉ is a cooling member that
    carries mass. `run_pairs` runs a pair with a remnant by +H, or that can interact by +H,
    which may lie after the window's end, so in such a run pair that member accretes its
    companion's wind (BSE equation 6). That is about 10⁻³ M☉, perhaps 0.05–0.1 mag in V. It is
    probably inside the phase envelope's 0.3 mag margin, and such a star (M<sub>V</sub> 15.5–18)
    is listable only within about a light-year, but nothing proves it. The lean for T17.c:
    - `Detached` for such a pair only if it cannot be run by +H either;
    - `light.rs`'s "accrete no wind" stated for stars of at least 0.1 M☉;
    - such pairs added to T17's slow test.
  - **`Remnants` gives nothing** only while white dwarfs are dark in V (ask A4), as T8.m's bins
    past the lifetime are. A4 must bring white-dwarf rows to both: `sky::photometry`'s module doc
    says so.
  - **Not changed.** No generated output moves. `GENERATOR_VERSION` stays 21, and no existing
    golden moves (`golden_diff.py`: one new golden).
- **Deviations in T8.h, as built (2026-10-08; written against P11.T17.a).** The cell cache in
  blocks keyed by magnitude, as the warm ruling sets it out (`decision-r06-t8h-warm.md` §2.1),
  with these details. `HYPERION_SKY_CACHE_MB` stays 64 MiB until R06.T8.n.
  - **What is built** (`sky/census/cache.rs`):
    - `SkyBlock` (`with_cell`, `cell`, `built_cells`, `held`, `heap_bytes`): 4³ cells of one layer,
      with its `BlockKey`, its `BlockParams` (the builder's position and cut), a 64-bit set of the
      cells built, a `u16` count a cell, and the held records cell by cell;
    - `HeldRecord`: the record and its `RecordLight`, 96 B (`a_held_record_is_96_bytes`);
    - `CellOutcome` (`Served`, `Missed`, `Rebuilt(Rebuild)`) and `Rebuild` (`Key`, `Window`,
      `Parameters`);
    - the constants `CACHE_APPROACH_LY` (1,000), `CACHE_CUT_SLACK_MAG` (0.1) and
      `BLOCK_SIDE_CELLS` (4), all `pub(crate)`.
  - **The trait is reshaped.** `SkyCellCache::bright_subset` gives way to `keeps_entries`, `block`,
    `keep` and `note`. The census applies the rule and builds every entry, and a cache only holds
    blocks.
    - `serve_from_entry` and `Served` leave the public API. The rule is `serve_from_block`, which
      answers `Lookup::{Served, NotBuilt, Key, Window}`. It, `CellNeed`, `entry_need` and
      `query_need` are `pub(crate)`, since no cache applies the rule now.
    - `NoSkyCellCache` keeps no entry. Its census places each cell at the query's floor, T8.g's
      path bit for bit, and draws each hierarchy once.
    - No later plan reads the old names. The Provides sketch is updated.
  - **The slack is doubled.** Windows widen by 2 × `BEFORE_DRIFT_SLACK_YEARS` (2 yr) and keys
    bring the box nearer by 2 × `BEFORE_DRIFT_SLACK_SHARE` (2 × 10⁻⁹), entry and query alike, not
    by one each.
    - Each record's ages before the drift, and the magnitude its bounds there read, already carry
      one slack. The second keeps them strictly inside the query's need against the rounding of
      ages and distances, and so inside every entry that holds the need.
    - So the query's key is a hair fainter than `floor_at`'s magnitude, which still sets its
      floor. Test 4 asserts the windows' containment.
  - **The costs.**
    - `census_cell` keeps its signature. `census_cell_with_cost` returns `(CensusTallies,
CensusCost)`.
    - `LayerCost` holds the ruled counts: candidates, held, pre-filtered, `star_bounded`,
      `unbounded_records`, pairs and `generated_listable`. It also holds each layer's cells
      served, missed and rebuilt by cause.
    - `LayerTally` keeps cells, generated, accepted, listed, `without_photometry` and
      `centre_members`.
    - `generated_listable` is counted before the cone's texel test, whatever the cone. The star's
      distance and unextinguished V move before that test, unchanged, so no output moves.
  - **Held records.**
    - A rogue planet is never held, since the census lists no star of one.
    - A record with no density component is held `Unbounded`.
    - An empty cell is held, so its records are not bounded again.
    - A cell whose held records exceed its `u16` count (65,535) is not kept, and the block held
      stays.
  - **A cell not served.**
    - If its block's parameters hold the query's need, it is built at them and merged in. That
      counts as `Missed`, as a cell no block holds does.
    - Otherwise it is built at the query's parameters, which replace the block, and counts as
      `Rebuilt(Key | Window | Parameters)`.
    - Its records are placed at the lower of the entry's floor and the query's.
    - One draw of the composition and `hierarchy_bound` (`Drawn`) gives both `StarBounds`, each
      bit for bit `StarBounds::of`'s (`a_drawn_hierarchy_gives_the_star_bounds_bit_for_bit`).
      Debug builds assert there that every record the query's census generates is held and
      passes the pre-filter, so every debug census through a cache checks the inclusion
      property.
    - A query its own parameters cannot hold (none within ±H, which the builder enforces) is
      censused with no entry, and debug builds assert that it never happens.
  - **The server.**
    - `SharedSkyCellCache` is a `SharedByteLru` of `SkyBlock`s over `(GalaxyKey, BlockKey)`. A
      block is charged its held records, `size_of::<SkyBlock>()` and the 96 B overhead.
    - A merge is built outside the lock from the new `ByteLru::peek`. The new
      `SharedByteLru::insert_if_unchanged` stores it only over the block it was built from, and
      the merge is built again otherwise. So jobs filling one block all land
      (`jobs_filling_one_block_side_by_side_all_land`).
    - `SkyCellCounters` adds `served`, `missed`, `rebuilt_for(Rebuild)` and `rebuilt()`. Its
      cache's hits and misses are block lookups, one a cell.
    - `requests/sky.rs` is unchanged: the wire reads `generated` and `without_photometry`, which
      stay in `LayerTally`.
  - **Tests.**
    - Tests 4 and 5 are in `cell.rs`, since they read `BeforeDrift`, `CellReach` and `held_light`.
      - Test 4 builds its entries for an observer at each place. Its three queries are that
        place at the epoch, 900 ly along +x at +H, and 600 ly along −x at −H at the cut plus 0.1.
        Each is read over its ages before the drift and at single ages at their ends and middle:
        720,000 checks.
      - Test 5 takes test 2's cells, with A and B at three places. Its entries are built at the
        Sun at V 6 and V 9. Its queries are test 2's but the cone and the 1,500 ly move. It found
        2,664 generated records held and passing in 251 entries and queries, and 1,340 others
        the pre-filter skips.
    - Beyond the ruling's list:
      - `a_block_merges_cells_at_its_parameters_and_others_replace_it`;
      - `a_drawn_hierarchy_gives_the_star_bounds_bit_for_bit`;
      - in the server, `a_cells_census_does_not_depend_on_what_was_asked_before`. It runs the
        census through the server's own cache, warm, fresh and evicting, under
        `assert_order_independent` (determinism audit).
    - The jobs test compares the stars' floats by their bits.
    - `moving_galaxy` moves to `sky/testing.rs`.
    - **The server's second sky is 20 ly along x, not 1,000 ly** (`tests/sky.rs`). The test's
      forced cap is 30 ly, so a 1,000 ly jump would share no cell, and "none rebuilt" would hold
      of nothing. The sim's test 2 and the bench take the 1,000 ly move.
    - Not done (the determinism audit's "consider"): test 4 on records whose window straddles age
      zero, under 10⁴ yr old at the epoch. They are about 10⁻⁶ of a sample, and each reader clamps
      the ages at zero, which reads as monotone.
  - **The bench.**
    - It samples whole blocks (`sample_hash` of the `BlockKey`), so that the sampled bytes scale.
      Its estimates are therefore not like for like with T8.f's and T8.g's samples by cell.
    - `census_near_sun/cold` reads no cache, T8.g's path, so its gate reads that path. The warm
      benches' fills are the cold census through an empty cache, the server's path, which also
      builds each entry's second `StarBounds`.
    - The double take, ruled at about 300–500 CPU-s, was not separated from the noise. The step
      bench's costs (the pairs, η and phase reads, 1.1–2.1 µs a record in C–E, on some 3.3 × 10⁸
      records bounded) give about 450 CPU-s.
  - **The provisional figures, at P11.T17.a's answers** (2026-10-08). The run took
    `census_near_sun/warm` and `/warm_jump`, sampled 1 block in 1,000, 15 workers, under the
    heavy-test lock while other lanes built (load about 15), so provisional. The caps are T7.b's by
    ray at the uniform cut: C 14,563, D 13,232 and E 46,010 ly at most, 1.04 × 10⁸ cells.
    `HYPERION_SKY_CACHE_MB` was 65,536, so that the sampled cache held the sample. Estimated,
    CPU-s:

    | Census                                          | CPU-s            | Of cold |
    | ----------------------------------------------- | ---------------- | ------- |
    | The Sun, cold through an empty cache (two runs) | 1.08, 1.14 × 10⁶ |         |
    | The Sun again, warm                             | 1.10 × 10⁶       | 102%    |
    | 1,000 ly toward the centre, cold                | 1.06 × 10⁶       |         |
    | The same, through the Sun's entries             | 1.28 × 10⁶       | 121%    |
    - **S1 is met.** No cell was rebuilt. After the jump, 84,581 of the sample's 93,514 cells
      (90.4%) were served, and 8,933 new ones merged into their blocks.
    - **S2 is met.** Both warm replies equal their cold ones, stars and tallies, with the same
      systems generated: 133,946 and 133,687 in the sample.
    - **S3 fails at the shipped 64 MiB, as ruled for T17.a.** The entries hold 1.36 × 10⁸ records:
      13.6 GB for the Sun's sky and 14.4 GB with the jump's new cells. That is 5.0 B a built
      cell beyond its records, inside the 8 B budget, so the blocks stay 4³.
    - **The ratios are noise about 100%, as the ruling expected (about 98%).** The jump bounds C's
      records star by star 5.9 × 10⁷ times against 2.0 × 10⁸ cold, D's 3.1 against 5.0 × 10⁷, and
      E's 6.3 against 7.3 × 10⁷. That saves some 5,000 CPU-s, under 1% of a census that is 98%
      generation. The two cold fills of one query differed by 6%. No finding.
    - Of the 1.34 × 10⁸ systems generated, 2.25 × 10⁵ have a star past the cut unextinguished
      (`generated_listable`): what the deferred realised bound could at best leave.

  - **To re-take after P11.T17.c, in R06.T8.n:**
    - every figure above;
    - the held bytes B, and so the default;
    - `CACHE_CUT_SLACK_MAG`;
    - test 4 on T17.c's verdicts;
    - the warm ratios, at the eye's cut on T7.b's caps (`decision-r06-t8h-warm.md` §2.2–2.3).

    At T17.a, B is about 14 GB. The ordinal form (about 8 B a record) would give some 1.1 GB of
    records plus the cells, but T8.n's step 5 takes that only after measuring B on T17.c's
    verdicts.

  - **T11.b's question** ("A question for R06.T8.h") is answered: no, the cache holds records and
    their bounds, not generated stars (ruling §2.4).
  - **Not changed.**
    - No generated output moves: `GENERATOR_VERSION` stays 21, and no golden or protocol type
      moves (determinism audit, `golden_diff` 0).
    - The ruling's `deferred-corrections.md` text (§4) is the orchestrator's to add.
- **Deviations in T9.f, as built (2026-10-07).** The band's march kept, as the band ruling
  (`decision-r06-t9b-band.md`, item 7) sets it out, with these details.
  - **The API** (`sky/band.rs`):
    - `march_rows(galaxy, ctx, query, replies, spec, face, rows) -> BandMarch` takes the replies a
      request states, each a `CompleteTo` (`impl IntoIterator<Item = CompleteTo>`), and not a list
      of edges. Every radius of every reply, in any layer, is a node of each ray inside the cone,
      and each layer keeps its own radii (private `Edges`).
    - `sum_rows(march, census, complete_to, out)` appends one reply's texels and its overflow's
      points. It panics on a radius the march does not keep.
    - `BandMarch` has `spec`, `face`, `rows`, `holds(&CompleteTo)` and `heap_bytes`.
    - `band_rows` keeps its signature. It is `march_rows` of its one reply, then `sum_rows`.
    - `CompleteTo::of_caps` now writes a radius at or below zero as +0, not through `max`, which
      may keep −0. A march and its sums then find one radius by its bits (determinism audit).
  - **One slot per layer and radius.** The ruling keeps two running sums per layer and edge: the
    light fainter than the cut and all of the light. A reply reads only their combination, so each
    slot holds that, as five sums: the fainter light out to the radius, run from the first node,
    plus all of the light beyond it, run from the radius out. Each is added interval by interval in
    distance order, and the two are added once.
    - The light beyond a radius is run from the radius, not taken as the total less the run to it.
      So a slot holds the bits a march keeping that radius alone takes over the same nodes,
      whatever other radii the march keeps, and nothing cancels.
    - A slot is 40 bytes, a layer's radius on a ray. That is half of the ruling's two sums. Its 1.7
      kB a ray and 42 MB at 64² counted four sums. With five, its layout is 2.16 kB and 53 MB
      (science check).
    - The fainter light and all of it are both read at the nodes between a layer's nearest and
      farthest radius. Before, both were read only at the radius's own node.
  - **A cone.** `Reach::of` (private) is the one place a ray's region is decided, at the cone's
    cosine.
    - A ray outside the cone is marched for all of its light, on the grid's nodes alone, and that
      sum fills each slot of its layer. So `sum_rows` reads every ray alike.
    - R06.T8.l (`decision-r06-t8k-cone.md`) widens the region to α + ρ there. The overflow's points
      are placed by `BandSpec::texel_of`, by which T8.l's census keeps its stars.
  - **T9.e's bits.** The task's "with T9.e's bits" cannot hold beside its bit-identity test. T9.e's
    `band_rows` ran one sum over the layers, node by node. The march keeps each layer apart and adds
    the layers per reply, so `band_rows`'s bits move at rounding (determinism audit):
    - by at most 2.9 × 10⁻¹⁵ relative in a texel's luminance and ρ;
    - the chroma (`f32`) is unchanged;
    - measured over 3,072 texels of five bands: near the Sun at 16² with no census; at 8² with an
      overflow, complete within 50 ly; complete nowhere; a 30° cone; and 2,000 ly above the Sun
      (probe `.git/rm23-scratch/r06-census/t9f/probe_bits.rs`, logs `probe-before.txt` and
      `probe-after.txt`).

    Every test's printed figure is unchanged at its digits. GENERATOR_VERSION stays 20 and no golden
    moves (`golden_diff` 0): nothing served or golden reads the band.

  - **A reply's band depends on its request's other replies,** within the quadrature. Their radii
    are nodes of every ray.
    - Summed from the request's march, a reply differs from its own `band_rows` by up to 2.0% in a
      texel at twelve nodes a decade, and by 0.14% over the band. That is on the 8² test band near
      the Sun, against the six replies of `a_march_sums_each_reply_as_band_rows_with_the_same_nodes`.
    - The worst texels are two rays 5–7° below the plane towards the inner Galaxy (the −Y face, 7°
      and 41° from the centre).
    - Nearly all of the difference is the node at 4,300 ly, which splits the 3,831–4,642 ly
      interval: −1.5% and +2.0% on those two texels. Most of the rest is the node at 3,000 ly
      (−0.4%), and every other node gives under 10⁻⁵ (probe `probe_nodes.rs`). The science check's
      reading: the realised dust at 3,000–4,300 ly, where those rays cross the inner disc. The bulge
      lies between unmoved nodes.
    - At 48 nodes a decade the difference is 0.046% and 0.003%. Twelve a decade lie within 1.1–2.4%
      of 48 in a texel and 0.5–0.7% over the band, and 96 agrees with 48. So T9.b's "0.02 mag" is
      no per-texel bound here (science check).
    - The test bounds the difference at 3% a texel and 0.2% over the band. These are regression
      bounds on the fixed fixture, not error bounds.
    - About 0.03 mag of luminance moves the eye's limit by about 0.01 mag. All of a request's
      replies share one set of nodes, so a sky filling in is consistent with itself.
    - So T11.d's march must take every reply the shell plan can state, not only the replies sent:
      a warm cache that sends fewer would otherwise give other bits. T11.c's comparison of the
      server's texels with the sim's should march the same replies, not call `band_rows`.
    - For T17: record the largest per-texel difference at 64² between the final reply's sum from
      the march and its own `band_rows`. Write `sky/band_face_row.golden` through `march_rows` of a
      stated set of replies, listed in the golden, then `sum_rows` of one reply (determinism
      audit).
  - **Tests** (`--lib -- sky::band`, 22, four of them new):
    - `a_march_sums_each_reply_as_band_rows_with_the_same_nodes` takes six replies:
      - complete nowhere;
      - 50, 100 and 400 ly;
      - caps of A 70, B 150, C 3,000, D 4,300, E 50,000 and the brown dwarfs 30 ly (E beyond the
        root cube's edge on the rays towards +Y, which end 39,536 ly out);
      - everywhere.

      It uses a census within 50 ly with an overflow past 20, at 8² faces, so 36 slots a ray.
      Each reply's texels from one march equal those of a march that keeps that reply alone over
      the same nodes, bit for bit: `band_rows` with the same edges as nodes. The band's light falls
      from nowhere to 400 ly and is least everywhere. The quadrature figures above are measured
      here.

    - `a_march_in_any_split_of_the_rows_sums_to_the_same_bits` checks four splits, the rows run
      last first, through warm and cold noise caches, for every reply. A march of the replies in
      reverse, with one of them twice, gives the same bits (determinism audit).
    - `one_marchs_light_at_100_200_and_400_ly_agrees_within_1_percent` measures near the Sun to V
      8, with every cap forced to 400 ly and no eye. The census's cells run on four threads (one on
      WebAssembly). Each reply lists the stars within its radius: 878, 3,514 and 9,520. The 500
      brightest are listed and the rest overflow. The totals are 9.1927, 9.1303 and 9.1375 × 10⁻⁴
      lx:
      - 100 → 200 ly: −0.68%, T9.b's figure;
      - 200 → 400 ly: +0.08%;
      - 100 → 400 ly: −0.60%.

      This is `sky::band`'s dearest test, about 100 s alone on four threads, for a census over
      eight times the volume of the 200 ly one.

    - `a_reply_the_march_does_not_keep_is_refused`, and `complete_to_reads_each_layers_cap` now
      also holds a −0 cap to +0.
    - T9.b's, T9.e's and T8.k's band tests pass unchanged in form through `band_rows`.
  - **The bench** `sky/band_near_sun` is split into `/march` and `/sum`. It takes a stand-in for
    R06.T8.i's shell plan, `shell_replies`: C, D and E complete to 500 ly, then 1,000 × 2^k ly
    below their caps, then their caps; A, B and the brown dwarfs complete to their caps in every
    reply. One run of each, provisional: criterion's `--test`, release, three workers at
    `CPUQuota=400%`, unlocked at load 13–17 (logs `bench-band.txt` and
    `bench-band-caps-alone.txt`).
    - Near the Sun at V 7.95 with the eye, the caps are A 11, B 68, C 8,193, D 9,925, E 21,369 and
      the brown dwarfs 1 ly. They give seven replies (the shells of 500, 1,000, 2,000, 4,000, 8,000
      and 16,000 ly, then the caps) and 22 slots a ray.
    - **The march's heap at 64² near the Sun is 21.8 MB** (21,774,336 bytes, 880 bytes a ray),
      against 5.97 MB for the caps alone.
    - The march took 22.7 CPU-s (7.6 s wall). The caps alone, the band before the split, took 18.5
      CPU-s (6.2 s, a temporary build of the bench). So the shells' radii cost some 23% more: the
      reads of both lights between each layer's nearest and farthest radius.
    - One sum of the final reply took 0.002 CPU-s for its 24,576 texels.
    - T9.b's 20.2 CPU-s ran on 15 workers under the lock.
  - **For R06.T8.i and T11.d.** The census's shells should reach the march as the `CompleteTo` of
    each reply, from the first shell to the final caps, in one list per request.
  - **Gates** (2026-10-07, on the final code, capped at `CPUQuota=400%`, four jobs, load 8–21, so
    timings are provisional; logs `.git/rm23-scratch/r06-census/t9f/final/`):
    - fmt; clippy, workspace native and sim wasm32-wasip1 (`-D warnings`);
    - `--lib -- sky::` 175 passed, 6 ignored (333 s, four threads), `sky::band`'s 22 among them;
    - `--test sky_census` 6 of 6; sky doctests 23 of 23, `march_rows`' example among them;
    - Prettier on the plan; every pre-commit hook.
  - **Reviews.**
    - Determinism audit: nothing must-fix. Its should-fix was this entry: the bits that moved, why
      T9.e's cannot be kept, and a reply's dependence on its request's radii. Applied its
      considers: the −0 radius, the test of the replies' order and repeats, and T17's golden
      through `march_rows`.
    - Science check: nothing must-fix. The v☉ subtraction, the slots' units and the conservation
      figures are confirmed. Applied its should-fixes: the 2% is the 3,000–4,300 ly intervals'
      quadrature, not the bulge's, measured node by node; and T9.b's 0.02 mag is annotated with the
      per-texel figures. Its considers are recorded above: T11.d marches every reply the shell plan
      can state, T17 records the 64² difference, and the ruling's memory counted four sums.
- **Deviations in T9.j, as built (2026-10-07).** The eye's own sky under a camera's cut, as the
  glare ruling (`decision-r06-t9c-glare.md`, the finding in item 2) sets it out, with these details.
  - **The eye's cut is the query's** (`sky/census/query.rs`, outside the task's files).
    - `SkyQuery::eye_cut()` gives it, and `SkyQueryBuilder::eye_cut(m)` sets it beside `.eye(…)`.
      Unstated, it is the query's cut, the eye-only request's. It is refused without the eye, not
      finite, or deeper than the cut (`BuildSkyQueryError::EyeCut`). A forced cap keeps it. The
      census does not read it.
    - The task names no place for it. The march reads it from its query, so `march_rows`,
      `band_rows` and `sum_rows` keep their signatures, and every band path gives the eye's
      background. A parameter of `march_rows` alone would have left `band_rows` silently at the
      camera's background.
    - T11.c builds the query to the deeper cut, with `.eye(eye).eye_cut(eye_cut)`, and passes
      `query.eye_cut()` to `Glare::of_listed`.
  - **The second sums** (`sky/band.rs`). Only where the eye's cut is shallower than the query's,
    each ray holds a second set of slots after the band's (`BandMarch::eye_cut()`, public, and the
    private `ray_len`).
    - Each slot is the light fainter than the eye's cut within its radius, run from the first
      node, plus the band's own sums beyond it, which both sets share.
    - `layer_light` reads `colour_sums_fainter_than` at the eye's limit beside the cut's, over the
      components in one order. The limit is the same expression at the eye's cut, with the distance
      modulus taken once a node.
    - `fill_slots`, `Rays::trace` and `NodeLight::dimmed` are split out of the march, the arithmetic
      unchanged. A ray outside a cone holds all of its light in both sets.
  - **The eye's light beside each texel.** `sum_rows` gives each texel a private `eye_light`: the
    luminance and ρ of the light fainter than the eye's cut, with that cut. It has no chroma and is
    not on the wire.
    - `BandTexel::eye_background()` (crate) gives it, or the texel's own light where there is none.
      The limit map's `background` reads it, so `limit_rows`, `limit_map` and `eye_offsets` keep
      their signatures.
    - `luminance_and_ratio` is shared by `of_sums` and the eye's light, so the two take one
      arithmetic. The band's texels as sent are unchanged, bit for bit.
  - **The overflow in the eye's light** (beyond the ruling's text). The overflow's stars at or
    brighter than the eye's cut are points in the eye's background, as they are in the eye-only
    request's band. Those fainter add nothing, since their light is expected light.
    - The census lists by V (`sky_order`). So an eye-only census of the same radius is the camera's
      census's first stars, and its overflow is the camera's overflow's stars at or brighter than
      the eye's cut, in the same order.
    - So the eye's map is the eye-only request's bit for bit at any N_max. The ruling claims it
      "wherever the eye's own listing fits N_max". The test holds it with every star listed, with
      the camera's census overflowing by stars between the cuts, and with both overflowing.
    - The science check agrees: these are the eye-only request's own points, not camera-only
      stars, so the review's must-fix against adding E ÷ Ω does not apply to them.
  - **The boundary.** The census keeps V ≤ cut (R06.T8.k), so a star at exactly the eye's cut is
    listed by an eye-only census and glares there. `Glare::of_listed` glares V ≤ `eye_cut`, and
    `sum_rows` takes overflow points at V ≤ the eye's cut. The ruling's "at or fainter than it adds
    neither" is read as the census's "fainter". The case is of measure zero, but the bits need it.
    Design note 4 says so.
  - **`Glare::of_listed(observer, listed, spec, eye_cut)`.** A star fainter than the eye's cut
    keeps its entry, one per listed star as `len()` counts, with no light. The pyramid skips it.
    Its eye offset is exactly its colour offset against its texel's background, its self-exclusion
    exactly +0. The glare keeps its eye cut. In debug builds `limit_rows` refuses a texel whose eye
    light is of another cut (determinism audit). Without that check, a glare to the camera's cut
    would have the stars between the cuts glare over a background that holds their light already.
  - **At the same census radius, not at the server's caps** (determinism audit, should-fix; for
    the orchestrator).
    - The identity holds between requests of the same replies, as the task's test has them. A
      server's caps come from the cut, so the camera's request at 10.06 reaches farther than the
      eye-only request at 8.15.
    - In that further shell, the camera's census lists the stars brighter than the eye's cut. By
      the caps' rule they are under one expected star a layer. They glare, and may be seen, where
      the eye-only request holds them as expected light, and the eye's background there holds only
      the light fainter than the eye's cut.
    - The eye's map is then not the eye-only request's bit for bit, though it is the more complete
      sky: real stars in place of their expected light (science check). A cone has the same effect.
      T8.l refuses an eye with a cone.
    - Holding the bits there too would take the eye-only request's own caps as another reply of the
      march, the eye's light summed at them, and the glare and the eye's overflow kept to the stars
      within the eye's cap for their layer. The eye's stars would then be a subsequence of the
      camera's, not a prefix. Not built: it is a ruling's to make. The docs say "at the same census
      radius". _Decided 2026-10-07 (`decision-r06-t9c-glare.md`, addendum 2): option (a), the
      truer sky, held by `the_eyes_light_is_independent_of_the_census_radius` and recorded in
      T17._ _Measured (2026-10-07, at 16², every star listed): within 100 ly the eye's light
      lists 922 stars beside its background, 9.372 × 10⁻⁴ lx in all; within 200 ly 3,815 stars,
      9.310 × 10⁻⁴ lx (−0.66%). The 2,893 stars between the radii give 3.79 × 10⁻⁵ lx in place
      of 4.42 × 10⁻⁵ lx of expected light, 86% of it: the census's realisation against the
      tables' expectation, not the band's (`decision-r06-t9b-band.md`, items 5 and 8), and the
      step T9.b's `the_light_does_not_depend_on_the_complete_to_radius` measures between the same
      radii at V 8 (−0.68%; −1.11% and −1.78% against the band complete nowhere, as T8.k re-ran
      it). A double count would read +4% and a loss −5%. The test took 58 s under nextest, its
      tables built in its own process (test profile, its census on four threads,
      `CPUQuota=400%`, load about 11, provisional); nextest gives it four slots. On
      wasm32-wasip1 its census runs on one thread (not timed)._
  - **Tests** (`--lib -- sky::band`, `sky::limits` and `sky::census::query`; seven new or changed):
    - `a_camera_cut_leaves_the_eye_the_limits_and_offsets_of_the_eye_only_request`, the task's
      first test. It runs near the Sun at 16², both censuses within 100 ly with every cap forced:
      922 stars at V 8.15 and 1,537 at 10.06. Both bands are complete to 100 ly or everywhere. At
      N_max 1,537, 1,229 and 461 the eye limits, the veils and the eye-only stars' offsets are equal
      bit for bit. The eye-only census is the camera's to 8.15, star for star.
    - `a_listed_star_between_the_cuts_changes_no_texels_eye_limit`. The 615 stars between the cuts
      move no texel's limit, and each one's eye offset is its colour offset alone, bit for bit.
      Glaring at the camera's cut over the band's own light, as T9.i took them, they move the
      limits. The brightest, V 8.155, would take 2.4 × 10⁻⁵ mag from its own texel's limit.
    - `a_request_whose_cut_is_the_eyes_gives_t9is_bits`. The eye asked at its own cut, stated or by
      default, and no eye at all give `band_rows`' texels with one set of slots. The glare is T9.i's
      sources and pyramid, so the map and the offsets are T9.i's.
    - `every_sum_at_the_eyes_cut_from_a_deeper_march_is_the_march_at_the_eyes_cut`, at 8² over T9.f's
      six replies. With 10.06 beside 8.15 and no census, the eye's light is brighter than the band's
      in all 384 texels complete everywhere, by 1.171 times at least. With 8.0 beside 6.5, the census
      within 50 ly at N_max 20 overflows in both requests.
    - `under_a_camera_cut_t9is_map_was_not_the_eyes`: the record below. Its poles' step is held to
      Gaia DR3's 0.52 ± 0.15 mag (science check).
    - `a_glare_to_another_eye_cut_is_refused_by_the_eyes_light` (debug builds).
    - Changed: `a_march_in_any_split_of_the_rows_sums_to_the_same_bits` also runs a request with the
      eye's cut at 6.5 and compares each texel's eye light (determinism audit).
    - `every_refusal_names_its_field` takes `EyeCut`. The doctest of `SkyQueryBuilder::eye_cut` is
      new, and `eye_offsets`' asks the eye and passes its cut.
  - **T9.i's bits.** A probe (not committed; `.git/rm23-scratch/r06-census/t9j/probe_full.rs`, logs
    `probe-before.txt` and `probe-after-same.txt`) hashed four maps with their eye offsets before
    and after the change. The four are T9.c's 16² fixture, the eye-only and as-built camera
    requests at 16² complete to 100 ly, and T9.i's 64² fixture. All four are identical:
    0x29ad9027412cd0ab, 0x3ddb49800fbcdb91, 0x71d60f26027c5a8e and 0x4400b67988bfd2e9. The camera
    request with the eye's cut gives the eye-only request's digest for its eye-only stars. Nothing in
    the tree pins those bits before T17's `sky/band_face_row.golden` (determinism audit).
  - **The record.** Probe logs `probe-after-record.txt`. Near the Sun, both bands complete
    everywhere, each census's glare within the radius. These are the eye's median limits in the
    band (|b| < 5°) and at the poles (|b| > 80°):

    | Faces, glare within               | Eye only (V 8.15) | Camera to 10.06, as T9.i built | Camera, T9.j    |
    | --------------------------------- | ----------------- | ------------------------------ | --------------- |
    | 16², 100 ly (922 / 1,537 stars)   | 6.5445 / 7.6738   | 6.6836 / 7.9210                | 6.5445 / 7.6738 |
    | 64², 200 ly (3,815 / 7,883 stars) | 6.5428 / 7.6672   | 6.6664 / 7.9043                | 6.5428 / 7.6672 |
    - Without glare at 64²: 6.5480 / 7.6834 for the eye alone, and 6.6722 / 7.9345 as built.
    - So the camera had moved the eye's limits by +0.12 in the band and +0.24 at the poles, deeper
      than T9.d's bound (the cut less 0.453). The camera's extra glare took back only 0.006 and
      0.030 of it within 200 ly.
    - The light between V 8.15 and 10.06 is 39.1% of the eye's background at the poles (μ 24.922
      against 25.461, 64²) and 22.7% in the plane (22.192 against 22.472).
    - Against Gaia DR3, from the band ruling's own pull within 10° of each pole, it is 38.0%: 38.5%
      at the north pole and 37.5% at the south. The plane is about 18%, at low confidence: G-binned,
      with V − G shifting 0.2–0.5 (science check). So the fixture's share at the poles is right,
      though its pole light is 0.3 mag faint (Risks, "The galaxy's local light is low"). The ruling's
      "a fifth to a third" is low: Gaia's step from V 6.5 to 8.1 is 26%, but from 8.15 to 10.06 it is
      38%. Correcting the ruling's text is the orchestrator's call.
    - Complete to 100 ly instead, the camera moved the 16² medians by under 0.001, since beyond
      100 ly both bands hold all of the light.

  - **The march's heap and time** (bench, one run each, criterion's `--test`, release, three
    workers at `CPUQuota=400%`, without the heavy-test lock, which another lane held, at load about
    10, so provisional):
    - `sky/band_near_sun/march_camera` is near the Sun at V 10.06 with the eye's light to 7.95. Its
      caps are A 46, B 260, C 19,416, D 19,416, E 41,804 and the brown dwarfs 1 ly, with eight
      replies. It holds **49.3 MB** (49,299,456 bytes) and took **29.5 CPU-s** (9.9 s wall).
    - `/march_camera_no_eye`, the same request with no eye, holds 24.7 MB and took 22.5 CPU-s (7.5
      s wall).
    - So the second sums double the heap and add 31% to the time. The eye-only `/march` at 7.95 took
      20.0 CPU-s in the same run, against T9.f's 22.7. Logs: `bench-march.txt`.
  - **Not changed.** No wire bytes, wire docs or client code. T9.d's tests and their bound are
    unchanged: the eye's map under a camera is now the eye-only map they bound. GENERATOR_VERSION
    stays 20, and no golden moves (`golden_diff` 0): nothing generated or golden reads the band or
    the map.
  - **Gates** (2026-10-07, capped at `CPUQuota=400%`, four jobs, load 6–14, so provisional; logs in
    `.git/rm23-scratch/r06-census/t9j/final/`): fmt; clippy, workspace native and sim
    wasm32-wasip1 (`-D warnings`); `--lib -- sky::`; `--test sky_census`; sky doctests; Prettier on
    the plan; every pre-commit hook.
  - **Reviews.**
    - Determinism audit: nothing must-fix. All four bit-identity claims hold by construction, for
      the same replies. Applied:
      - the "same census radius" qualifier and the record above (should-fix);
      - the row-split test with the eye's light (should-fix);
      - the glare's eye cut checked against the texels' (consider);
      - the T9.i probe digests (consider).
    - Science check: nothing must-fix. The physics and the boundary reading are confirmed. Applied:
      - the poles' share against Gaia DR3 (should-fix);
      - the module doc's "all of the light beyond it" (should-fix);
      - the "same census radius" qualifier, Design note 4's boundary wording, and an assertion on
        the poles' step (considers).
- **Deviations in T7.b, as built (2026-10-07).** Caps by direction, as the census-cost ruling
  (`decision-r06-census-cost.md`), its sign-off (question 3) and the cone ruling's item 3 set them.
  T7.b was built before T8.g (`decision-p11-t16-hierarchy-bound.md`), so its census figures are
  sampled, and T17 re-takes them.
  - **Built.**
    - `sky::caps`: `CapLattice`, `RayRadii`, `CapCount`, `WIDENING_SPACINGS`, `SUB_RAYS`,
      `layer_caps_by_visibility`, `layer_caps_by_visibility_over` and
      `expected_beyond_caps_by_visibility`.
    - Also `LayerCap::{radius_toward, rays}`,
      `CapResolution::{rays, steps_per_decade, sub_rays}` and
      `RayExtinctions::{measure_clearest, measure_clearest_rays}`, beside R06.T11.c's
      `measure_rays` and `join`.
    - `sky::limits::{EyeVisibility, eye_visibility}`, `SkyQueryBuilder::eye_visibility`,
      `SkyQuery::eye_visibility`, `BuildSkyQueryError::EyeVisibility` and
      `CompleteTo::radius_toward`.
    - `LayerCap`, `CompleteTo` and `CellSlab` are `Clone`, no longer `Copy`: they share a cap's radii
      by `Arc`.
    - λ is found exactly among the yields (stars per system of each ray's intervals), the largest
      that keeps the layer's count beyond under 1. Each radius is then widened to the largest
      within twice the spacing.
    - The census's radius towards a direction is the largest widened radius of the rays whose
      cones, of half-angle the spacing, hold it. `plan_cells` opens a cell whose padded bounding
      ball meets such a cone nearer than its ray's radius. A 32² cube map indexes the rays by
      direction, and most far cells are rejected by one lookup.
    - The band takes the same radius towards each texel's centre. A texel of a reply by ray is that
      of a uniform reply at its radius, bit for bit (test), and so is `BandMarch::ray_light`, the
      illumination's (R06.T9.g).
    - Each ray counts the stars brighter than cut − DM − v☉(A_V) A_V. The rule bound takes v☉ at
      the least extinction.
    - For caps by ray, `CensusPlan::cell_count` walks the cells, with no column count. The walk's
      sphere is the farthest ray's.
    - The server (R06.T11.c): its caps' ray jobs now measure each ray through its sub-rays, on a
      lattice one job builds, 8 rays a job (192 jobs). They take the eye's visibility where a query
      asks it, and stay `layer_caps`' bit for bit (`the_caps_rays_measured_in_jobs_are_the_sims_caps`).
  - **The ray spacing (deviation; the coordinator's instruction).** The plan does not define it.
    - The census-cost probe took a ray's own cap of equal area, 4.14° at 768 rays. A cone of that
      leaves directions up to 5.64° from any ray (5.49° the farthest of 10⁵ sampled), so it would
      leave gaps.
    - The spacing is the lattice's covering radius, bounded from above by measurement: a 256² cube
      map's texel centres, plus their largest radius. It is 5.81° at 768 rays and 4.20° at 1,536.
    - The cones and the widening (twice the spacing) both use it.
  - **`CAP_RAYS` 1,536 and three sub-rays a ray (deviation; approved by the coordinator).**
    - **768 rays at widening 2 failed** `caps_converge_in_rays` above the Sun, at (0, 26,000, 2,000):
      E 1.53 at 7.95, and 3.85 for the eye's visibility caps at 8.54. Fine rays 4–7° below the
      horizon see E supergiants of the inner disc through edge-on windows.
    - **Widening alone, at 768 rays:** 3 spacings left E at 1.13 there. 4 passed (0.80), but opened
      85.3% of the spheres' systems near the Sun (E 38.9%), against the ruling's 75% and 35%.
    - **1,536 rays at widening 2** passed at 7.95 (1.46). Uniform caps at deeper cuts still failed
      above the Sun: E 2.5 at 8.2, 3.5 at 8.54, 4.2 at 8.8 and 4.1 at 9.1. They passed again from
      9.3 (1.11 at 10.06). E alone would need 5–12 spacings; no other layer passed 1.5 at any cut.
    - **The fix: each ray counts through the clearest of three sub-rays at each distance.** These
      are the ray and two more a quarter of the spacing (1.05°) to either side, across the ray from
      galactic north. It passes everywhere: worst 0.998 over the six points at 7.95, 8.2, 8.54, 8.8,
      9.1 and 10.06, with no table by cut.
    - **Variants measured** also converged, but cost more near the Sun at 7.95:
      - 5–9 sub-rays 0.7 spacing out opened 102–109% of the spheres' systems;
      - 3 at 0.35 opened 75.8%;
      - 3 offset in latitude opened 79–84% at 0.25–0.35.
    - **Cost:** `layer_caps` takes about 12 s a call in the test profile, against 2.5 s in T7 and
      5–7 s for 1,536 rays of one profile each. In the release bench it is about 12 s a call: two
      plans' caps took 23.7 s, against 2.2 s for R06.T7's 768 rays of one profile each
      (2026-10-08, after another lane's locked run, so provisional). That is some 0.1% of the
      census near the Sun, far under R06.T17's 10% trigger.
  - **T7's tests per ray (deviation; ruled 2026-10-08, `decision-r06-t7b-brackets.md`).**
    - On every ray: each within its rule bound, A and B under 100 ly, C at least 1,000 ly, and in
      the nuclear disc E under 1,500 ly (its largest ray is 555 ly).
    - On the median ray: D and E near the Sun within a factor of three of 4,300 and 10,000 ly, and
      each of C–E nearer in the nuclear disc than near the Sun. Their largest rays reach far past
      the brackets through the clear windows (E's largest to 61,341 ly; its direction is not
      measured, and T17 records it), so a bracket on every ray would fail by design.
    - The ruling: the brackets were a sanity check against the least-ray and mean-field regressions,
      set on one radius a layer, as the brainstorm's figures are. Caps that follow the dust spread
      wider than the bracket by design, so D and E read the median ray. The systems gates (75%, E
      35%) fail a least-ray regression and the nuclear disc's per-ray E a mean-field one. No far-ray
      or pole-ward bracket is added: a ray's radius follows its yield per system, so the scale
      height enters through the yield, and a ray over empty volume costs cells and hides no star.
  - **The stated count beyond** is counted at the caps' own rays, beyond each ray's radius towards
    its centre. Near the Sun at 7.95: A 0.99, B 0.97, C 0.42, D 0.33 and E 0.29 (the spheres: 0.75,
    0.80, 0.75, 0.63 and 0.42). λ fills the budget of 1 before the widening, so A and B, whose rays
    the widening hardly moves, sit just under 1.
  - **Measured near the Sun at 7.95**, on a recount of 3,072 rays and 48 steps a decade, against
    R06.T7's spheres (768 rays, one profile a ray, v☉):
    - Systems opened: A 76.4%, B 93.3%, C 99.9%, D 80.9%, E 32.9%, and 74.2% in all. The gates are
      75%, and 35% for E, so both pass, narrowly. The ruling's table (the lane's tables, the
      probe's 8.3° widening, one profile a ray) had 86%, 72% and 25%.
    - Expected stars the spheres list that the rays drop, and that the rays gain: A 0.248 and 0,
      B 0.160 and 0, C 0.094 and 0.429, D 0.119 and 0.281, E 0.149 and 0.508.
    - Expected count beyond, by ray and by sphere: C 0.290 and 0.625, D 0.196 and 0.359, E 0.185
      and 0.544.
    - The rays' radii, median (10th–90th percentile) and largest: C 8,193 (6,764–9,018) and 14,563;
      D 6,764 (6,146–9,925) and 13,232; E 4,610 (4,188–14,563) and 61,341. In the nuclear disc
      E is 235 (145–417) and 555. `layer_caps`' doc comment has the table.
  - **The eye's visibility-based caps are adopted** (the sign-off's alternative 1).
    - At T9.d's eye cut near the Sun, 8.282: the rays' cuts run 7.206–8.282 (median 8.010).
    - They open 62.7% of the systems of the uniform caps at that cut, 37% fewer.
    - Safety test (the final 64² map with no glare, plus 0.453, against the cut of every ray whose
      cone meets a texel): least margin +0.094 mag over 85,772 pairs.
    - Recount at 3,072 rays: at most 0.935 (A). At every point's own eye cut it is at most 0.94.
    - Each ray takes the deepest limit of the 16² texels whose centres lie within the spacing plus
      twice the 16² texel radius (approved): every texel its cone meets and more about it. Within
      the spacing plus once, at 1,536 rays, the margin was −0.042 mag: a 64² texel darker than its
      parent by more than the pad.
    - Opt-in: a request asks it by `SkyQueryBuilder::eye_visibility`, only for an eye-only request
      at its own cut. **T11 should set it on every eye-only request.** Without it the census takes
      the uniform caps, which are also complete, at about 1.6 times the systems.
    - **Not adopted at the camera's 10.06.** The harness's camera limit (`cameraLimit.ts`' at f/1.4,
      1/30 s, ISO 409,600 and 60°) runs 9.68–10.13 over the 16² pre-pass. So the per-ray cut is the
      request's 10.06 nearly everywhere, and the saving is 0.2%.
    - **A finding, not fixed (ruled 2026-10-08, `decision-r06-t7b-brackets.md`; deferred).** A
      camera's request and cull both read Design note 18's limit over a uniform μ 24 sky
      (`viewSky.ts`' `DARK_SKY_CD_M2`), so they agree and the census is complete to the cull. The
      modelled sky at the camera's cut is darker than μ 24 over about two-thirds of the sky near the
      Sun. There the per-texel limit reaches 10.13 at 60° (darkest texel about μ 25.5), and in the
      band's brightest texels it is 9.68. The bound is the black-sky limit, read noise alone: 10.15
      at 60°, 0.095 mag deeper than μ 24's at every field of view, reached only by fields wider
      than about 41°. The correction is the client's, both together:
      - the cull reads each texel's background;
      - the request asks the black-sky limit, so that no cull is deeper than its census.

      Near the Sun its extra stars are N_max overflow, and it costs about 9–14% more of a camera
      census. The camera-budget ruling adopts or defers it with T17's figures. Until then the
      request and the cull stay at μ 24 together. Nothing changes in T7.b, the census or the
      server.
  - **The caps' move from v☉** (R06.T7's spheres): none near the Sun (A 11, B 68, C 8,193, D 9,925,
    E 21,369 ly). In the nuclear disc B–E stay on the same radial nodes. Every radius there moves
    about 3% (17 → 18, 127 → 131, 225 → 232, 362 → 374 ly), because the farthest rule bound, which
    sets the count's radial grid, moves out under v☉. A move smaller than one radial step, about
    10%, is not resolved; near the Sun the grid ends at 120,000 ly and cannot move.
  - **At 10.06 near the Sun** (for the camera-budget ruling, with no gate): the uniform caps by ray
    open 36.8% of R06.T7's spheres' systems (E 12.2%). The spheres hold 2.8 × 10⁹ systems there,
    against 3.5 × 10⁸ at 7.95.
  - **Realised drops and gains** (the bench, `sky/caps_by_ray_near_sun`, at 7.95; 2026-10-08,
    sampled 1 in 1,000, under the heavy lock, `CPUQuota=400%`, three workers; provisional):
    - The union of the three plans' sampled cells, 134,535 of them, was censused in 1,475 CPU-s.
      It listed 40 stars, scaled to C 25,000, D 6,000 and E 9,000, the same for every plan.
    - None was dropped and none gained in the sample. The expected figures are under one a layer,
      by ray C 0.09 and 0.43, D 0.12 and 0.28, E 0.15 and 0.51. By the eye's visibility at 7.95
      they are A 0.47 and 0, B 0.59 and 0, C 0.48 and 0.25, D 0.80 and 0.10, E 0.46 and 0.36. A
      sample this sparse cannot see them, and R06.T17 re-takes them on the final census.
    - The plans open 1.08 × 10⁸ cells (the spheres), 1.04 × 10⁸ (by ray) and 8.7 × 10⁷ (by the eye's
      visibility at 7.95, which opens 60–79% of the spheres' systems layer by layer, 25% in E).
      Far cells of few systems make up the by-ray plan's. Walking all three took 184 s on one
      thread; a server walks them slab by slab on its pool.
  - **Gates** (2026-10-08, capped, build-slot, 4 jobs, `CPUQuota=400%`):
    - fmt, and clippy `-D warnings` over the workspace natively and on the sim for wasm32-wasip1;
    - `--lib -- sky::`: 248 passed;
    - `--test sky_census` 6/6, and the sky doctests 31/31;
    - the server's `--test sky` 11/11 and `compute::sky` 17/17;
    - `just test-slow caps_converge_in_rays` passed on 9c84ca04 (640 s, under the heavy lock;
      2026-10-08). Every one of its 144 recounts at 3,072 rays is under 1.0, the worst 0.996 (A,
      above the Sun at 8.54).
  - **`caps_converge_in_rays`'s figures** (recounted at 3,072 rays and 48 steps a decade): the
    worst layer at each point, for the uniform caps at 7.95, 8.54 and 10.06, and for the caps by
    the eye's visibility at the point's own eye cut.
    - Near the Sun: 0.990, 0.956 and 0.911; by visibility at 8.282, 0.935.
    - In the nuclear disc: 0.866, 0.822 and 0.970 (the brown dwarfs); by visibility at 7.372, 0.878.
    - At (26,000, 0, 68): 0.989, 0.962 and 0.929; by visibility at 8.403, 0.934.
    - At (−18,385, −18,385, 68): 0.972, 0.988 and 0.952; by visibility at 8.280, 0.944.
    - In the inner disc (0, 8,000, 0): 0.990, 0.966 and 0.854; by visibility at 7.243, 0.926.
    - 2,000 ly above the Sun: 0.969, 0.996 and 0.959; by visibility at 8.540, 0.945. Its E, which
      failed before the sub-rays, is 0.524, 0.239 and 0.188, and 0.571 by visibility.
    - The worst layer is A or B nearly everywhere: λ fills their budget of one and the widening
      hardly moves their rays.
  - **Reviews.**
    - Rust review: its must-fix (the lattice taken by value) is applied, and so are its
      should-fixes (units in the names, the visibility's observer and eye, the per-ray clone, a
      server test of the visibility branch, the summaries and an example).
    - Determinism audit: no must-fix. Its should-fix (by-ray replies in the row-split test) and
      considers (the server's visibility test, the spacing's bits pinned) are applied.
    - Science check: no must-fix. Its doc corrections are applied.
    - Plan conformance: no must-fix. Its should-fixes are applied.
  - **Files (deviation).** Beyond `sky/caps.rs` and `sky/census/query.rs`, T7.b touches:
    - `sky/band.rs`: `CompleteTo` by ray, and the band's and `ray_light`'s reading of it;
    - `sky/limits.rs`: `EyeVisibility` and `eye_visibility`;
    - `sky/dgl.rs`: two clones;
    - `benches/sky.rs` (`sky/caps_by_ray_near_sun`) and `tests/sky_caps.rs`;
    - the server's `compute/sky.rs`, `requests/sky.rs` and `tests/sky.rs`: its ray jobs take the
      sub-rays and the visibility, and `CompleteTo` is cloned into each job.
  - **The cell test (deviation).** `plan_cells` tests each cell's padded bounding ball against the
    cones, not its padded box, as R06.T8.l's cone does. It is a superset: it opens a few more cells,
    never fewer.
  - **Measured outside the harness.** The camera's visibility caps at 10.06, the 3- and 4-spacing
    widenings, the widening by cut and the sub-ray variants came from scratch probes that are not
    kept. Their logs are in `.git/rm23-scratch/r06-census/t7b/logs/` (`vis1`, `conv1`–`3`, `widen1`–`3`,
    `sub1`–`4`).
  - **Acceptance as built:**
    - `cargo test -p hyperion-sim --lib -- sky::caps sky::census::query sky::band sky::limits`
      (libtest ORs its filters; the plan's two filters before `--` are refused);
    - `cargo test -p hyperion-sim --test sky_census` and the sky doctests;
    - `just test-slow caps_converge_in_rays`;
    - the server's `cargo nextest run -p hyperion-server --test sky` and its `compute::sky` tests.
  - **A finding for plan 07 and R06.T9.a (not fixed).** `extinction::profile`'s last node does not
    always equal `sightline` to 10⁻¹²: two rays of a 12-ray lattice from the Sun differ by
    1.8 × 10⁻⁴ and 2.6 × 10⁻⁴ relative at 120,000 ly, while their neighbours agree to 10⁻¹⁵. R06.T9.a's
    test checks the 8 rays of an 8-ray lattice. T7.b's sub-ray test checks each ray against its
    sub-rays' own profiles, bit for bit.
  - **Open.**
    - T11 carries each layer's radii per ray (1,536 × 6) on the wire, with the lattice's ray count,
      and the client reads them (DN9; R06.T11.d). The wire's `cap_ly` is now each layer's farthest
      ray.
    - T11 also computes `eye_visibility` with the request's illumination for every eye-only
      request. The safety test ran without an illumination; the diffuse light's effect on the gap
      between the 16² and 64² maps is not measured.
    - T5.e's slow `band_gate` reads `CAP_RAYS`, so it now takes 1,536 rays of one profile each.
    - The visibility caps were safety-tested near the Sun alone, and recounted at all six points.
    - The cost estimates "at T7.b's caps" elsewhere in this plan assumed the ruling's 25% for E. As
      built E opens 32.9% and all layers 74.2%, within the gates by 2.1 and 0.8 points; T8.g and T17
      re-derive them.
- **Deviations in T7.c, as built (2026-10-08).** The visibility caps' safety with the illumination,
  as `decision-r06-t11d-first-sky.md` §4 rules it. Test only, in `sky/caps.rs`' test module. It
  passes at all six points, so it no longer holds back serving the eye's visibility caps by default.
  - **Built.**
    - `least_margin` takes the request's illumination and states it on the final 64² map's query.
    - `the_eyes_visibility_caps_count_what_it_sees_and_open_a_fifth_fewer_systems` (fast, near the
      Sun) states the fixture's `Illumination::march` at the Sun (`sun_illumination`) on the eye's
      cut, its visibility and the final map. T7.b's gates are unchanged.
    - The slow `the_eyes_visibility_caps_are_safe_with_the_illumination_at_six_points` marches its
      own illumination at each of `caps_converge_in_rays`' six points and checks the safety alone:
      some texel–ray pairs compared, each ray's cut at most the eye's, and no negative least
      margin. A NaN margin now fails both tests; `f64::min` used to drop it.
    - Its points, `CONVERGENCE_POINTS`, copy `tests/sky_caps.rs`' `POINTS`, since `least_margin`
      and `visible_cuts_v` are private to the module. `POINTS`' doc comment says so.
  - **The six points are a slow test (as the task allows).** Their illuminations, eye cuts and final
    maps take 231 s in the slow profile. The rule bounds, the count beyond, the 3,072-ray recount
    and the fifth-fewer gate stay in the fast test, near the Sun.
  - **Near the Sun**, from the fast test, with T7.b's figures without the illumination in brackets:
    - the eye's cut is 8.179 (8.282);
    - the rays' cuts run 6.916–8.179, median 7.736 (7.206–8.282, median 8.010);
    - the least margin is +0.088 mag over 85,772 pairs (+0.094);
    - the visibility caps open 53.1% of the uniform caps' systems at that cut (62.7%);
    - the recount at 3,072 rays is at most 0.961, in A (0.935).
  - **The least margin at the six points**, with the illumination, run under the heavy lock in the
    slow profile on 2026-10-08. The least is +0.073 mag, 2,000 ly above the Sun: 0.021 mag under
    T7.b's near-Sun figure, where the ruling expected a move of hundredths. Every point compares
    85,772 texel–ray pairs, a count set by the lattice and the map alone, and the near-Sun row
    equals the fast test's. Each eye's cut without the illumination is in brackets, from T7.b's
    `caps_converge_in_rays` record.

    | Point                  | The eye's cut | The rays' cuts (median) | Least margin |
    | ---------------------- | ------------- | ----------------------- | ------------ |
    | Near the Sun           | 8.179 (8.282) | 6.916–8.179 (7.736)     | +0.088 mag   |
    | The nuclear disc       | 5.807 (7.372) | 5.800–5.806 (5.804)     | +0.100 mag   |
    | (26,000, 0, 68)        | 8.109 (8.403) | 6.909–8.109 (7.721)     | +0.090 mag   |
    | (−18,385, −18,385, 68) | 7.730 (8.280) | 6.784–7.721 (7.428)     | +0.086 mag   |
    | The inner disc         | 6.722 (7.243) | 6.051–6.684 (6.491)     | +0.076 mag   |
    | 2,000 ly above the Sun | 8.540 (8.540) | 6.900–8.540 (8.085)     | +0.073 mag   |

  - **Measured, with no gate:** the diffuse light makes the eye's cut shallower where it is bright,
    by 0.10 mag near the Sun, 0.52 in the inner disc and 1.57 in the nuclear disc. There the rays'
    cuts are nearly uniform (5.800–5.806). 2,000 ly above the Sun the cut is unchanged to the
    thousandth.
  - **The server** already sets `eye_visibility`, with the illumination, on every eye-only request
    (R06.T11.c, R06.T11.d). Nothing changes there, and the ruling's fallback, leaving it unset, is
    not needed.
  - **T7.b's Open items** "the safety test ran without an illumination" and "safety-tested near the
    Sun alone" are closed.
  - **Open.** `caps_converge_in_rays`' visibility pass still builds its eye's cut and visibility
    without the illumination. The visibility caps as served are therefore recounted at 3,072 rays
    near the Sun alone (0.961), and not at the nuclear disc's 5.807 or the inner disc's 6.722, which
    are shallower than any cut that test counts. The ruling words T7.c as the safety test alone.
    Adding the illumination there is a test-only change for the orchestrator to assign.
  - **Gates** (capped, build-slot, 4 jobs, `CPUQuota=400%`):
    - fmt, and clippy `-D warnings` on the sim's targets, natively and for wasm32-wasip1;
    - `cargo test -p hyperion-sim sky::caps`: 16 passed and 1 ignored (the slow test), 180 s;
    - the slow test, under `just _locked`, in nextest's slow profile: passed, 231 s.
  - **Reviews.**
    - Rust review: its must-fix (the helper borrowed the `Arc`, then cloned it) is applied, and so
      are its should-fix (a needless clone) and one consider (the NaN margin).
    - Plan conformance: no must-fix beyond this record. Its considers are applied: the pairs check
      and the points' pointer.
    - Determinism and science reviews were skipped: the change is test only, with no generated
      output and no new figure.
  - **Acceptance as built:**
    - `cargo test -p hyperion-sim sky::caps`, where the slow test is ignored;
    - `just test-slow the_eyes_visibility_caps_are_safe_with_the_illumination_at_six_points`, run
      here as its nextest command on the sim's lib under `just _locked`.
- **Deviations in T8.i, as built (2026-10-08).** Nearest first, as the census-cost ruling
  (`decision-r06-census-cost.md`), its sign-off (question 2, conditions 1 and 2) and the listing
  ruling (`decision-r06-t8i-listing.md`, adopted 2026-10-08) set it out. No golden moves and no
  bump: the one-shot census and its band are unchanged (determinism audit, `golden_diff` 0,
  GENERATOR_VERSION 21).
  - **Built** (`sky::census`):
    - `SHELL_EDGES_LY` (`[u32; 9]`: 500 ly, then 1,000 × 2^k ly to 128,000, the last below
      `MAX_FORCED_CAP_LY`), `SHELLED_LAYERS` (C, D, E), `Shell` (`layer`, `index`, `edge`,
      `is_last`; it orders as `shells()` gives them) and `Completeness` (`complete_to`,
      `observer`, `band_spec`, `is_final`, `edge`, `least_edge`, `lists`, `radius_for`);
    - `CensusPlan::{shells, shell_slabs, completeness, complete, replies}`, `CellSlab::shell`,
      `merge_shells` and `SkyCensus::completeness`;
    - crate-private: `galaxy::query::cells_in_shell_slab` (plan 03's module),
      `RayRadii::{within, all_reach}`, `CompleteTo::{of_caps_within, same}`,
      `GalacticPosition::is_same_point` and `plan_with_edges`.
  - **The shells.** A layer of `SHELLED_LAYERS` has one shell for each edge below its cap's
    farthest radius, then one to its cap; A, B and the brown dwarfs one each. Shell k walks the
    sphere of its edge, padded as a walk to that edge pads (the pad at the edge's light time), and
    each column skips the run of cells the sphere of the edge before keeps, by the walk's own test,
    so the shells partition the cells without visiting the inner ones. The cells are still opened
    with the one-shot plan's pad and cones, so the shells change no cell the plan opens, and for
    uniform caps shells 1–k are exactly the cells of the census forced to edge k.
  - **Rank by rank (deviation).** `shells()`, `slabs()`, `cells()` and `plan_cells` give every
    layer's first shell in `CAPPED_LAYERS`' order, then every layer's second, and so on; before,
    layer by layer. No census moves: the merge's order is total. It is not T11.d's order (D's and E's
    shells to 4,000 ly before C's beyond 2,000 ly), which T11.d sets from `shells()`.
    `completeness` reads a set of shells: a shell after a gap in its layer's run adds nothing until
    the gap is filled.
  - **`SkyCensus` carries `Option<Completeness>` (deviation).** `merge_shells` gives `Some`;
    `merge_census` (signature unchanged) and `empty` give `None`, a census that states no radius of
    its own and is complete to its plan's caps (`CompleteTo::of_caps`). The server's one-shot path
    stands unchanged until T11.d.
  - **`plan_cells(query, caps: Vec<LayerCap>)` (deviation).** It took `&[LayerCap]`; it is now
    `census_plan_of(query, caps).cells()`, so it takes the caps by value (Rust review: no borrow then
    clone), and yields the cells shell by shell. Its callers are tests.
  - **The listing: the ruling's (b).** Until its last shell is merged a layer lists a star whose
    distance is less than the radius at the centre of its band texel (`BandSpec::texel_of` at the
    query's `band_spec`, of `observer.displacement_to(star.apparent())`, the overflow's lookup),
    in `Completeness::radius_for` alone. Stars at or beyond it are dropped before the `n_max` cut,
    neither listed nor overflow; the tallies still count them as accepted. A final layer lists every
    star of its opened cells. `CensusPlan` keeps the observer and `band_spec` for every query.
    - Per ray, a layer done to edge e is complete to `RayRadii::within(e)` (each ray's radius held
      within e), or to `Uniform(e)` where every ray reaches it.
    - `sum_rows` asserts, for a census of shells, that `complete_to` is the census's own (by bits,
      `CompleteTo::same`) and, unless it is final, that the march is at its `band_spec` and from its
      observer (the ruling's note; the observer is the determinism audit's).
  - **`replies()`** gives one `CompleteTo` per rank, from the first shell to the caps (7 near the
    Sun at the eye's caps), holding every layer's every radius once that layer's first shell is
    done. A layer with no shell done is complete nowhere, which `replies()` lacks: `nowhere` adds no
    node (radii at or below `FIRST_NODE_LY` are not nodes) but a slot a layer and ray.
  - **Test-only edges (deviation).** A census past 500 ly is a slow test's cost, so the census tests
    take nearer edges through `plan_with_edges`: 40 and 80 ly with every cap forced to 160 ly, and
    50 and 100 ly with caps by ray jagged between 40 and 150 ly. The fixed edges are tested on plans
    and the partition.
  - **Tests** (`sky::census::query`, 6 new; `galaxy::query::walk`, 2; `sky::band`, 3; a split and
    order case in `sky::census::merge`; doctests of `completeness`, `merge_shells` and `replies`):
    - uniform: to 40 ly C, D and E list 70 stars, as the census forced to 40 ly does, bit for bit,
      and 223 of their shells' stars wait; to 80 ly 452 and 561; the last is `census_plan`'s
      one-shot census, star for star and tally for tally, 3,159 stars over 49,152 cells in 12
      shells;
    - by ray: 124 and 656 listed, every star of the census forced to the clipped radii within its
      texel's radius among them, and 0 gap stars (the gap clause is not exercised);
    - the partition: 347,648 cells in 13 shells (`SHELL_EDGES_LY`, uniform caps C 1,100, D 2,100,
      E 1,100 ly), 264,897 in 18 (by ray) and 11,474 in 12 (a cone), each cell in the first shell
      whose sphere its box meets;
    - the two census tests take 10.9 and 9.2 s on four threads under nextest (an override gives
      them four slots).
  - **R06.T7.b's gap and its mirror** (`decision-r06-t8i-listing.md` §2; the gap deferred by the
    owner on 2026-10-08, in `deferred-corrections.md`, "R06 caps by direction"; not fixed):
    - **The gap.** The band reads a texel's radius at its centre u_T; the cells are opened by the
      rays' cones about each star's own direction u_s. A star at r with R(u_s) ≤ r < R(u_T) whose
      cell is unopened has its light in neither the census nor the band, in partial and final
      replies alike. The caps' count beyond bounds it, under one star a layer: ≤ 0.75 expected over
      the sky near the Sun at 7.95, in C and D only (A's, B's and E's cells and pads are wider than
      the texel); estimated at about 10⁻² stars, about 10⁻⁸ of the band, the same order at 10.06;
      ≤ about 1.9 for the eye's visibility caps, estimated at a few × 10⁻².
    - **The mirror.** A final layer lists every star of its opened cells, so the upper sliver's
      stars, at [R(u_T), R(u_s)), are listed and in the band's light too: perhaps 10–300 stars,
      ≲ 10⁻³ of the band, within T9.b's 1%. No layer has it before it is final.
    - **Fixes** (i)–(v) are the decision's. (i), the plan's cones widened by ρ (5.47° at 64²), closes
      the gap with this listing unchanged, and may need T7.b's 75% and 35% gates re-ruled by the
      owner with measured figures.
  - **Docs corrected in passing** (the ruling's note): `CompleteTo`'s doc and the `sky::band` and
    `sky::caps` module docs, which said the band reads the radius the cells were opened by; caps.rs
    now says "padded ball", as T7.b built it.
  - **Files (deviation).** Beyond `sky/census/{mod,query,merge}.rs`: `galaxy/query/{walk,mod}.rs`,
    `sky/band.rs` (the guard, its tests, the docs), `sky/caps.rs`, `coords/galactic.rs` (one crate
    method), `sky/census/cell.rs` (one test's `plan_cells` call) and `.config/nextest.toml`.
    `sky/census/cache.rs` is untouched.
  - **Acceptance as built:**
    `cargo test -p hyperion-sim --lib -- sky::census galaxy::query::walk sky::band::tests::a_census_of_shells`,
    the census doctests, `--test sky_census`, and the server's `compute::sky` and `requests::sky`,
    since it streams `plan.cells()` in the new order.
  - **Not measured** (T17's): the first shell (500 ly) against the final caps (T9.b), each shell's
    cost and the time to the first sky. The bench `sky/band_near_sun/march` still takes T9.f's
    stand-in, `shell_replies` (one uniform radius a layer, the least of its farthest ray and the
    edge), not `CensusPlan::replies`, which clips each ray: T17 re-takes the march's heap and cost
    at the real replies.
  - **For T11.d:**
    - run the shells nearest first within each layer, each as its `shell_slabs`' jobs; keep every
      part, the stars that wait among them, and make each reply with
      `merge_shells(parts, n_max, plan.completeness(done))` over the parts so far;
    - march once a request at `query.band_spec()` over `plan.replies()` (plus `nowhere()` if a reply
      may come before some layer's first shell) and sum each reply at
      `census.completeness().complete_to()`;
    - the wire: `final` is `is_final()`; each layer's per-ray table is the cap's rays held within
      `edge(layer)` (none once final); the label's figure is `least_edge()`.
  - **For T17:** a golden of a partial reply near the Sun, each layer's complete-to radius bits and
    the IDs and star indices it lists, beside `census_near_sun.golden`'s one-shot census
    (determinism audit).
  - **Gates** (2026-10-08, capped, build-slot, four jobs, `CPUQuota=400%`, load 3–12, so timings
    are provisional; logs `.git/rm23-scratch/r06-census/t8i/final/`): fmt; clippy `-D warnings` on
    the sim, the server and the fit crate natively (all targets) and on the sim for wasm32-wasip1;
    `sky::census`, `sky::band`, `sky::caps` and `galaxy::query::walk` under nextest, 117 passed
    (826 s); `--test sky_census` 6 of 6; the census doctests, 12; the server's `compute::sky` and
    `requests::sky`, 28; Prettier on the plan.
  - **Reviews.**
    - Rust review: its must-fixes are applied (`Reached::Edge` holds `LightYears`; `plan_cells`
      takes its caps by value), and so are its should-fixes (the guards compare by bits, the
      `SHELL_EDGES_LY` doc, `RayRadii::within` checks its edge, one-sentence summaries, an example
      of `replies`) and two considers (`streamed_cells_are_plan_cells` builds its cells
      independently; each edge keeps its whole light-years, so no cast). Not taken: `sum_rows`
      reading the census's own completeness instead of its argument, T11.d's to decide.
    - Determinism audit: nothing must-fix or should-fix. Its considers are applied: `merge_shells`
      in any split and order, the observer in `sum_rows`' guard, and T17's partial golden above.
    - Plan conformance: Design note 11 amended to the ruling, the Provides line, this entry and the
      acceptance as built; the bench's stand-in is recorded above for T17.
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
- **Generator version 21 (the galaxy plans' Phase J lane, 2026-10-06 and 07; R06's code is
  unchanged; its three pair-light tables are refitted at 21, twice, their young probe cells move,
  and two of its test pins move).** The 20 → 21 bump lands plan 11's P11.T4.h–k, 9a0950e's
  protostar and build-age fixes, and plan 14's P14.T47.e and T13.c (galaxy plan 11's Risks, "The
  20 → 21 bump, as built"). Two of this plan's pins move with them. Each was bisected on
  `rendering-and-planets` (a50d8a73), with each held commit's code applied in turn:
  - `sky::luminosity`'s `SERIAL_FINGERPRINT`, `FULL_SERIAL_FINGERPRINT` and
    `PAIR_SERIAL_FINGERPRINT` moved in the bump with P11.T4.h alone: the early AGB's core and
    small-envelope remnant at SSE's τ, which the single-star tracks read. 2ecdeb4a pinned them at
    `0x5c11_7d93_64a9_5339`, `0xb581_e39e_74e3_414c` and `0xffe0_8298_a09d_eb2e`. The refit below
    moved them again, to `0x9d13_13d6_2725_9e79`, `0xd063_b6fb_cde6_ee63` and
    `0xc6c9_59ed_9001_fd37`.
  - `sky_census`'s `the_merged_giants_near_the_sun_are_listed_as_their_oracle_lists_them` moves
    with P11.T4.i alone, which starts a pair at its first arrival:
    - 0x4204_6c99_ff00_000a's pair merges at 58.4 Myr, not 144.2 Myr. At the census time
      (5.36 Gyr) the product is a 1.12 M☉ core-helium-burning giant at V 7.08, not a 1.23 M☉
      first-giant-branch star at V 7.44. The test now takes each system's phase.
    - 0x21fe_5648_7ff0_0001 merges at 107.3 Myr, not 136.0 Myr, and stays a 1.19 M☉ giant, now at
      V 7.27, not 7.53.
    - Census and oracle still agree star for star. Both stars stay brighter than T8.b's bound at
      their primaries' mass and within T16.b's.
  - **Stale at 21 after the bump:** `sky_binary_light_c`, `_d` and `_e`. `just fit-check`
    reported each one's probes moved: `cell([Fe/H] 0, age bin 21).light`, `(…, 16).light` and
    `(…, 11).light`. So `hyperion-fit`'s `the_committed_tables_are_fresh` failed, and `just ci`
    could not pass without the refit. The determinism auditor's must-fix (the refit moves the
    sky's output) was ruled on 2026-10-07: refit at `--since 21` on the bump's branch, and land
    the refit with the batch, with no bump of its own.
  - **Before the refit** (2026-10-07, under the heavy lock), the nine slow sky tests the bump had
    left unrun all passed at 21. They are `sky_envelope`'s three, `caps_converge_in_rays`,
    `luminosity_matches_realised_cells`, T5.e's
    `the_full_build_matches_thirty_two_nodes_a_panel_where_the_tables_are_read`, and the census's
    oracle tests 1,000 ly from the Sun, for the dwarfs near it and for D and E in the nuclear disc.
    - `the_fitted_envelope_is_the_build_rounded_brighter` passes, so `sky_envelope` is fresh in
      fact and not only by its 15 sparse probes. It is not refitted.
    - `hyperion-fit check --rerun-fast` reproduces every fast table it can rerun, except plan 06's
      `stellar_fates_low` and `_mid` (below). `limb_darkening` and `wd_cooling` are not rerun
      without their fetched datasets; they read only `math` and constants from the sim.
  - **The first refit, as built.** The three layers ran one after another under the heavy lock, each
    with the `just fit` recipe's command from its release build:
    `hyperion-fit run sky_binary_light_<layer> --since 21 --threads 16`.
    - _Cost_: C 472 s, D 416 s and E 1,063 s, so 1,951 s of wall time and 2.61 × 10⁴ CPU-s
      (7.25 CPU-hours), against about 4.0 × 10⁴ CPU-s at version 20 under loads of 25–45. The
      load stayed at 15–18 and the fit's RSS at no more than 37 MiB. No data were fetched (the
      tables read none).
    - _What moved_: the bodies, their sim fingerprints and `since-generator-version` 21. Manifests,
      inputs and revisions are unchanged.
    - _Size_: the files are 176, 172 and 211 kB (193, 185 and 216 at 20), all under the 500 kB
      limit.
    - _Systems changed by pair evolution_: C 1.10% → 0.99%, D 6.98% → 6.99% and E 12.78% →
      13.21%.
    - _Young cells_: cells younger than about 10⁷ yr (age bins 0–10) lose many of their pair
      differences: the tables hold 464, 372 and 129 more empty bins in C, D and E. 9a0950e keeps
      protostars apart, and P11.T4.i starts a pair at its first arrival. The fingerprint's young
      probe cell (age bin 2 at solar \[Fe/H\]) changed 16, 47 and 12 systems at 20 and none at 21.
    - _The pair-correction gate_
      (`the_pair_correction_is_known_to_two_percent_and_clamps_under_half_a_percent`; correction,
      1σ, clamped; version 20's are in T5.d's entry):
      - solar circle: C −2.05%, 0.29%, 0.000%; D −10.40%, 0.73%, 0.019%; E −21.45%, 1.13%,
        0.010%;
      - bulge: C −3.24%, 0.29%, 0.000%; D −9.64%, 0.90%, 0.034%; E −13.65%, 0.56%, 0.037%.

      In the component guard, the largest 1σ is 1.00% (the old thin disc at \[Fe/H\] −1) and the
      largest clamp 0.31% (the old thin disc at −0.5). All are within 2% and 0.5%.

    - _After it_: `just fit-check` gives 21 fresh, the 6 provisional tables' warnings and none
      stale. `check --rerun-fast` finds only the two fates tables, as before. The three
      fingerprints are re-pinned. R06's whole slow sky suite passes on the refitted tables: 13
      tests, T5.e's two among them, in 4,161 s under the lock. The nuclear disc's census oracle
      test took 2,430 s of that.
    - _T5.c's gate_ (`luminosity_matches_realised_cells`), realised against the corrected tables:
      - solar circle C −4.4%, D −20.2%, E +49.2%;
      - bulge C −3.3%, D +0.3%, E −4.0%.

      At 20 they were solar circle −4.4%, −20.4% and +47.7%, and bulge −3.6%, +0.5% and −4.5%.
      The cells' paired deficit against the tables', each within Z √(Σ d² + σ²):
      - solar circle C 9.4% against 2.1%, D 21.1% against 10.6%, E −0.9% against 22.1%;
      - bulge C 3.6% against 3.3%, D 10.8% against 10.0%, E 10.5% against 14.2%.

    - _The caps_ (cut 7.95, `layer_caps`' doc table, re-measured at 21): as at 20 everywhere but
      the nuclear disc, where C goes from 116 to 127 ly and D from 205 to 225 ly. Before the refit
      they were version 20's to the light year.
  - **Then the fates, and a second refit** (the orchestrator's ruling, `status.md`, 2026-10-07).
    `check --rerun-fast` had found plan 06's `stellar_fates_low` and `_mid` stale by rerun at 21.
    P11.T4.h alone moves a few white-dwarf nodes, by about a unit in the eighth significant digit.
    The ruling first kept them for the version-22 batch, then had them refitted at 21 (plan 06's
    Risks has both and the detail). The binary-light tables, which read the fates through
    `fit_galaxy`, were then refitted again on them, with the young probe cells below.
    - _Cost_: C 464 s, D 362 s and E 1,056 s, so 1,882 s of wall time and 2.61 × 10⁴ CPU-s. With
      the first refit, that makes 3,833 s and 5.22 × 10⁴ CPU-s (14.5 CPU-hours) on 16 threads.
    - _What moved_: the three bodies are byte for byte the first refit's. Only the probes and their
      sim fingerprints changed, so every figure above stands.
    - _Downstream_: the re-bless at 21 passed 3,808 of 3,808 and moved no golden. The three
      luminosity fingerprints held at the values above.
    - _Checks_: `just fit-check` gives 21 fresh and 6 provisional. `check --rerun-fast` gives 21
      fresh and 0 failures, with only the provisional tables and the two fits whose datasets are
      not fetched as warnings. R06's whole slow sky suite passes again on the final tables: 13
      tests in 4,130 s, with the caps and T5.c's figures as above to the digit.
  - **The young probe cells, amended** (the orchestrator's sign-off, `status.md`, 2026-10-07, on
    the determinism auditor's point). T5.d's caps ruling took one young probe cell a layer: age bin
    2 at solar \[Fe/H\] on 2,000 systems, where the protostars were, so that `fit-check` sees a
    change to them. At 21 it read 0 in all nine values in every layer, since 9a0950e and P11.T4.i
    leave no pair interacting there, so it could see nothing.
    - The sign-off named C's bin 8, D's bin 9 and E's bin 3, the tables' first filled young bins.
      At solar \[Fe/H\], though, C's bin 8 and D's bin 9 change none of their first 20,000
      systems; C's fills only at its 2 × 10⁵.
    - So each layer takes the youngest solar cell whose first systems include a handful that pair
      evolution changes, at a cost of seconds. C takes age bin 12 (25–40 Myr) on 20,000 systems
      (14 changed at 21), D bin 12 on 10,000 (8) and E bin 3 (0.4–0.6 Myr) on 2,000 (9).
      `FitLayer::probe_cells` says so.
    - `fit-check` now takes 2.5, 4.0 and 2.2 s for the three layers' fingerprints, against about
      1.5 s each before. No body depends on the probes.
    - With working probes, a later batch's `fit-check` sees these Slow tasks again, which
      `--rerun-fast` cannot. Its fallback stays to rerun each layer into scratch
      (`hyperion-fit run sky_binary_light_<layer> --out <path>`) and compare bodies, or to refit
      at the new version.
- **Deviations in T11.a, as built (2026-10-07).** The handler, its checks and the census as bulk
  jobs, as the task sets them out, with these details.
  - **Where.** `requests/sky.rs` holds the handler (`answer`), the checks (`SkyAsk`, a
    `TryFrom<&SkyRequest>`), the reply and the wire's star (`wire_star`), as the task names; the
    other kinds check their fields in `convert/`. The jobs are in `compute/sky.rs` (new):
    `SkyCaps`, `SkyTables` and the bulk jobs of the tables, the eye's cut, the plan and the census.
    `convert.rs` makes `query_time` and `mass_layer` `pub(crate)` and splits
    `root_cube_position(field, …)` from `query_centre`; `bulk::sky` becomes `pub(crate)`.
  - **The checks**, in this order: the universe, `time`, `observer` (canonical and in the root
    cube), `eye.field_factor`, `eye.age_years` and `eye.pigmentation` (at `EyeObserver::new`'s
    ranges), `camera_limit_v` (finite and at most V 11, refused rather than clamped), `n_max`,
    `cone` and `exclude_system`. Beyond the task's list:
    - a request that asks neither the eye nor a camera has no cut, and is refused naming
      `camera_limit_v`;
    - `n_max` 0 is refused, as above the cap;
    - an eye with a cone is refused first, with the sim's `ConeWithEye` text. Then a cone's axis
      must be of unit length within 10⁻⁶, and its half-angle within (0°, 90°];
    - `exclude_system` is decoded, then looked up by `resolve` in an interactive `try_submit` job
      after the galaxy and before any bulk job, as `scene_ship` does. Either refusal is
      `unknown_system` naming `exclude_system`.
  - **Protocol doc comments, no wire change.** `SkyRequest.cone` and `ConeDto` call a cone an
    instrument's field stop, refused with the eye. `ConeDto.half_angle_deg` is at most 90, the
    sim's (it read 180). `EyeDto` states the ranges the server takes, F 0.1–100, age ≥ 0 and p
    0–1.2 (it read F ≥ 1, age > 0 and p 0–1, which nothing enforced). `n_max` is from 1, a request
    of neither eye nor camera is refused, and `MAX_CUT_V` is tested. The glare ruling's wire doc
    comments land here (`decision-r06-t9c-glare.md`, §4): the payload table's byte 22,
    `eyeOffsetMag`, `SkyStarWire` and `view/sky/cull.ts`, with T9.h's −1.28 saturation. T11.c's
    two sentences of that ruling are applied with T11.a's text.
  - **The cut.** The eye's cut is `sky::limits::eye_cut` in one bulk job, the request's cut the
    deeper of it and the camera's limit, and the query takes `.eye(eye).eye_cut(eye_cut)`
    (R06.T9.j). T9.d's "T11.a may run the faces as jobs" is not taken. Plan text not yet applied:
    `decision-r06-t9g-dgl.md`'s T11.a sentence (the request's illumination before the eye's cut)
    waits for R06.T9.g.
  - **The census's jobs.**
    - 256 cells a job (`CENSUS_JOB_CELLS`), in the plan's canonical order. The cells are handed
      out on the request's task, and at most `BULK_QUEUE_CAPACITY` (256) jobs are outstanding.
      Sizing by cost, about 50 ms a job, is T11.d's.
    - Each job builds its own `SkyContext`: 2¹⁴ noise slots, `NoSkyCellCache` (T11.b brings the
      server's), no sources and `NoModifiers`. _Since T11.b: the server's `SharedSkyCellCache`._
    - Each job checks, before each cell, the request's token and a census-scoped token that a
      `CancelOnDrop` raises when the census ends early (a failed job, a dropped future).
    - The parts are joined as they arrive: the stars concatenated and the tallies summed from the
      first, as `merge_census` sums them. The join is merged in one more bulk job, which is the
      same census, since the merge's order is total. `the_jobs_census_is_the_one_pass_census_star_for_star`
      holds them equal.
    - A job's cost is far from its cells' count. Near the Sun at forced caps the few D and E cells
      about the observer dominate, since the census generates their every system until T8.g. A
      probe on one thread of a test build at load 16 gave: 20 ly, 256 cells in 5.7 s; 50 ly, 2,008
      cells in 7.3 s; 120 ly, 21,760 cells in 85 jobs in 36 s.
    - Considered, not done: fewer jobs outstanding, so that a density map's bands, also bulk, need
      not queue behind up to 256 census jobs.
  - **The tables are built for each request,** in one bulk job (`SkyTables`): the luminosity
    tables (`LuminosityTables::build`, 64–140 s of one worker in release), the envelope and the
    cells' offset bounds. The plan's caps are one bulk job. So a chart's query can wait behind
    those single jobs on a busy pool, though never in the interactive queue. T11.c builds the
    tables once per galaxy as staged jobs under a single flight, caches them, and runs the caps as
    ray chunks. _Since T11.c: so built, in `compute/sky_tables.rs`'s `SkyTablesService`._
  - **A small census for tests: `SkyCaps` (new, public in `hyperion_server::compute`).** The plan
    names no seam, and a census to the derived caps near the Sun costs 10³–10⁶ CPU-s until T8.g.
    `ServerConfigBuilder::sky_caps(SkyCaps::forced(radius))` forces every layer's cap; no option
    or variable sets it. A forced census reads no luminosity table, so it is given
    `LuminosityTables::dark`. The eye's cut is then a dark sky's, V 8.54 at the default eye
    (Crumey's 7.99 plus 0.553), and each layer states its cap as the radius with nothing expected
    beyond it. _Since T11.c: dark by default; `with_galaxy_tables()` builds the galaxy's own for a
    test of the band's light._
  - **The reply is a stub until T11.b and T11.c.** It is the census's JSON:
    - `cut_v`, each capped layer's census (A, B, C, D, E, brown dwarfs) by T8.c's mapping, `listed`
      and `overflow`, `valid_until` and `not_modelled`;
    - an empty manifest (0 chunks, 0 bytes), with `stars_bytes` and `band_bytes` 0;
    - no `hosts`, and the query's band shape, 64² a face.

    `listed` counts the census's stars though the payload carries none yet. `sky` is no longer
    answered `unsupported` (T10's interim). _Since T11.b: the stars in bulk, with the manifest and
    the split; the band and the discs from T11.c._

  - **`valid_until`.** Design note 13's rule, with each listed star within 1 ly taken at its
    layer's `pad_speed` (1,000 km/s; 3,000 for E), since a `SkyStar` carries no velocity. The sky
    is then asked again early, never late: a star 0.5 ly off holds it 5.3 days, or 1.8 in E.
  - **The wire's star** (`wire_star`, `expect(dead_code)` until T11.b sends it; _since T11.b,
    sent_):
    - its V is `SkyStar::v`;
    - its chroma is the chromaticity of `reddened(a_v).red_green()` (T10's conversion);
    - its camera term is the reddened one, relative to that V;
    - its eye offset is its colour offset alone, scotopic, at the reddened ρ (the glare ruling's
      interim), until T11.c's `eye_offsets`.
  - **Tests.**
    - `--test sky`, 6 tests, 20 s at 4 threads under load 16: the task's five, and
      `the_cut_is_the_eyes_under_a_shallower_camera`, bit for bit the sim's dark eye cut. The
      near-Sun test also matches each layer's tallies and the listed and overflow counts with the
      sim's one pass.
    - The cancel test's bound is exact. Once `cancelled` is read the token is cancelled, so at most
      the one job in hand completes afterwards, and the queued ones are skipped.
    - The range query's test reads the query's answer before the sky's, with the census's jobs
      in the pool and none in the interactive queue.
    - Every wait is on the pool's counters or the frames, bounded by the harness's patience.
    - Unit tests: `requests::sky`, 8 (every refusal names its field, the cut's rule, the wire's
      star over a 25 ly census, `valid_until`, the chromaticity, the protocol's limits, the
      feature members' gap, an unknown `exclude_system`); `compute::sky`, 2.
  - **Open, for the orchestrator.**
    1. **The stub reaches the live client.** The `VIEW` display asks for its sky on every
       published run (`useViewSky`). With T11.a merged it holds a sky with no stars and no
       texels. `view/sky/band.ts`'s `bandTexels` then throws "a band of 0 texels is not six faces
       of 64²", and R02's interim field is gone. Land T11.a with T11.b and T11.c, or keep the
       client from asking until then. _Decided 2026-10-07 by the orchestrator: T11.a–c land
       together after T11.c, behind T11.c's server switch, off until T8.g (T11.c)._
    2. **The cost of a real sky.** Until T8.g, a sky to the derived caps near the Sun is some
       10⁵–10⁶ CPU-s of bulk work (T8.f's sampled 1.64 × 10⁶), after a table build of minutes, and
       the client's request starts one whenever a view asks for a sky. Its jobs never block a
       chart, and closing the view cancels it, but they fill every worker meanwhile, against the
       2026-10-06 load limits. _Decided 2026-10-07 with item 1: the switch stays off until T8.g._
- **Deviations in T11.b, as built (2026-10-07).** The payload through R03's frames and the per-cell
  cache, as the task sets them out, with these details.
  - **Landing (decided 2026-10-07 by the orchestrator; closes T11.a's open items 1 and 2).** T11.a,
    T11.b and T11.c land together after T11.c, behind a server switch that T11.c adds. It is off
    by default, so the server answers `sky` as before T11.a (`unsupported`), until T8.g lands and
    turns it on (moved to R06.T11.d on 2026-10-08, `decision-p11-t17c-bright.md`). Tests turn it
    on. The plan text is in T11.c's task. Until then T11.a and T11.b are held on the server lane.
  - **The payload.**
    - `requests/sky.rs`'s `answer` returns R03's `Answer { body, bulk: Some(payload) }`, so
      `requests/mod.rs`'s `Sky` arm no longer goes through `answered`.
    - After the census, one more bulk job (`payload`) takes each listed star, in the census's
      order, through `wire_star`, and then `encode_sky_payload`. It runs on the pool, since it
      walks up to 3 × 10⁵ stars. The census is shared with it through an `Arc`.
    - `BulkPayload::new` takes the bytes. Its refusal, a chunk count past a `u32`, cannot happen
      to a payload of at most 7.5 MB, so it is an `expect` that states the bound.
    - The response's `bulk`, `stars_bytes` and `band_bytes` are the payload's, set once in
      `response`.
    - The `expect(dead_code)`s on `BulkPayload::new`, `BuildBulkPayloadError`,
      `encode_sky_payload`, `wire_star`, `chromaticity` and `scotopic_colour_offset` are gone.
      `SkyTexelWire` needs none: the encoder's signature uses it.
  - **The band's part is empty until T11.c.** The payload is the stars alone: `band_bytes` is 0,
    and `band.face_texels` still states 64. The client's `bandTexels` would throw on this reply,
    which is one reason for the switch above. _Since T11.c: the band's 24,576 texels follow the
    stars._
  - **The cache** (`compute/sky_cells.rs`, new; Design note 12).
    - `SharedSkyCellCache` is a `SharedByteLru` over `(GalaxyKey, CellKey)`. Its entry,
      `SkyCellEntry`, holds the floor it was built at and the cell's records at or above it, in
      candidate order. An entry is charged `cell_heap_bytes` of its records (kept at their length)
      plus its size and `ENTRY_OVERHEAD_BYTES`.
    - `SkyCellCacheHandle` is the sim's `SkyCellCache`. It serves only through
      `serve_from_entry`, so it never changes a reply. A floor below the entry's rebuilds the cell
      outside the lock and replaces the entry with the one that serves more. A NaN floor's cell is
      served and not kept. The handle asserts the galaxy's seed, as `CellCacheHandle` does.
    - Two requests may build one cell at once, and the later insert wins. Either entry serves by
      the rule, so this costs only a rebuild that a single flight would save, as with
      `SharedByteLru`'s other caches. Two census jobs never share a cell.
    - The budget is `HYPERION_SKY_CACHE_MB`, or `--sky-cache` (default 64 MiB, a whole MiB, 0
      caches nothing), in `config.rs`, with `ServerConfigBuilder::sky_cache_bytes`, the server's
      start log (`sky_cache_mib`) and the README's options table.
    - It is held on `AppState::sky_cells` as an `Arc`. The census jobs take it through
      `compute::sky::CensusInputs` (new: the galaxy, its key, the tables, the cache and the query),
      which keeps `census` at four arguments. Each job builds its `SkyContext` over its own handle.
      The eye's pre-pass and the plan read no cell and keep `NoSkyCellCache`.
    - **Counters, beyond the plan:** `ServerStats::sky_cells()` gives `SkyCellCounters`, the
      cache's `LruCounters` and `rebuilt`. A hit is a lookup that found its cell's entry, and
      `rebuilt` counts the hits whose entry was built above the floor asked, which rebuilt the cell.
      So the lookups served are the hits less `rebuilt`.
  - **Tests.**
    - `--test sky`:
      - `a_skys_manifest_matches_what_was_sent`, named so that the task's acceptance filter
        (`sky`) runs it. It reads every frame with `TestClient::next_frame` (new in
        `tests/common/mod.rs`) rather than the task's `next_binary`. The chunk count arrives only
        in the terminal response, after the chunks, and `next_binary` panics on a text frame, so
        the reader must take either kind. The chunks come in order, for the request, before the
        response, each a frame of at most 262,144 bytes. Their count and bytes are the manifest's,
        and `stars_bytes + band_bytes` is the manifest's bytes, with `stars_bytes` = 24 × `listed`
        and `band_bytes` 0. Each star's wire V and distance are the sim's one-pass census's, star
        for star in its order, through no cache. The same sky asked again gives the same bytes,
        and every cell the first census looked up is served from the cache: no new miss, no
        rebuild, no eviction.
      - `served` now reads the frames before the response. The near-Sun test asserts the payload's
        split in place of the stub's empty manifest.
    - `tests/cli.rs`'s `the_sky_caches_budget_falls_back_to_its_variable`. It starts the binary with
      `HYPERION_SKY_CACHE_MB=3`, reads its log up to `listening` and kills it. Its log's
      `server started` line states `sky_cache_mib=3`. The process harness `tests/stop.rs` had
      (`Running`, `Stopped`, the binary with every option's variable cleared, read from the parser)
      moved to `tests/common/process.rs`, shared by both files. `stop.rs` keeps its own requests
      to the server (`/healthz`, stdin, signals) as an `impl` of its own.
    - `config.rs`'s tests take the option and variable throughout. `lib.rs`'s fresh-stats test
      takes the new counters.
    - Unit tests, `compute::sky_cells`, 7, which pass T8.d's scenarios as its record asks:
      - looser, then tighter, then lower, with each hit, miss and rebuild counted;
      - a planted entry served, filtered, at or above its floor, and rebuilt below it;
      - `assert_order_independent` over a cell of each of A–E about the Sun at three floors each,
        through a warm cache, a fresh cache per call and a cache whose budget evicts (and holds);
      - entries kept per galaxy, a budget of 0, a NaN floor, and another galaxy's cells refused.
    - `compute::sky`'s jobs test now runs the census twice through one cache: cold and warm both
      equal the one-pass census with no cache, and the warm one is served wholly from the cache.
  - **R03.T15's transfer check, run (2026-10-07; R03's Risks, T15's pending item).** The 4-chunk
    run below closes T11.b's part (decided 2026-10-07 by the orchestrator). The sky's largest
    payload runs in R06.T17, on a real sky with the switch on (T17's list).
    - **Out of reach at 15 MiB.** No sky's payload reaches R03's 15 MiB (61 chunks). The largest
      is N_max's 3 × 10⁵ stars × 24 bytes plus 24,576 texels × 12 bytes: 7,494,912 bytes, 29
      chunks. The 61-chunk check belongs to R09's coarse field, about 15 MiB, after RM3 (decided
      2026-10-07 by the orchestrator; a pointer in R09's Risks).
    - **The setup.** The check ran with this kind, in the real renderer, hidden. It is a scratch
      harness, not committed, in `.git/rm23-scratch/r06-server/t11b/transfer/`:
      - The server was started in a test binary with T11.a's seam, `SkyCaps::forced(200 ly)`, 4
        workers, the default 64 MiB sky cache and the seed `0x4d2`. The server binary has no option
        for forced caps, and a derived-caps sky near the Sun is not to be run.
      - Electron 44.4.3 (Chromium 152) ran headless: Ozone headless, SwiftShader, an offscreen
        window that was never shown, a fresh profile and no D-Bus session, under the GPU lock in a
        capped scope. Nothing on the path (the socket, the assembly, the decode) uses the GPU, so
        R03's "on this machine (RTX 3080)" asks nothing of the graphics adapter here.
      - Its page asked `sky` near the Sun at V 11 (a camera's limit, no eye) through
        `@hyperion/protocol`'s `RequestClient.requestBulk` over a real WebSocket. It read each
        frame's header as it arrived and decoded the payload with the app's own
        `decodeSkyPayload`, on the page itself, not in the app's decode worker
        (`decode.worker.ts`, which only hands each request to that function).
    - **What was sent:** 36,177 stars listed, none overflowing, and no band. 868,248 bytes in 4
      chunks: three of 262,120 bytes and one of 81,888 (frames of at most 262,144 bytes).
    - **The result: passed.**
      - The 4 chunks arrived for the one request, in order, and the terminal response after the
        last.
      - The assembled 868,248 bytes are the manifest's and `stars_bytes`'. Their FNV-1a,
        `f56690fe`, equals the server's own over the same sky, asked by its test client. Two runs
        with the server's test client alone gave the same.
      - The decode gave 36,177 stars and 0 band texels.
      - Provisional, under load 10–40: the census took about 87 s, cold, on 4 workers of a test
        build, and the transfer, first chunk to response, about 1 ms on loopback.
      - The harness's own startup: the page's socket was welcomed 19.6 s after the page loaded,
        before any request. It is probably Chromium's network service without a D-Bus session,
        and was not looked into; it is no part of the transfer.
    - **Not run:** the sky at N_max (about 420 ly at V 11, some 2,500 CPU-s of census before T8.g)
      and the integrated app's `VIEW`, whose band would throw (above). The first is T17's (above);
      the second waits for T11.c and the switch.
  - **Not built: the server bench `sky_near_sun_cold`** (T11's Files, `benches/sky.rs`). T11.a and
    T11.b leave it, since a cold sky to the derived caps near the Sun is 10⁵–10⁶ CPU-s before
    T8.g. T11.d, the first server task after T8.g, builds it.
  - **Found: a warm repeat costs as much as cold (a measurement, not fixed; FEATURES FIRST).** On
    that server, the same 200 ly sky asked again found all 93,440 cells built (25.5 MB of entries;
    no miss, no rebuild), yet took 90 s against the cold 87 s, under load 27–43 against the cold
    census's 10–40. Provisional, under load.
    - The census's cost is generating each candidate system's stars (`SystemStars::generate`, 98%
      of it, `decision-r06-census-cost.md`), and an entry holds the records, not the stars.
    - **A question for R06.T8.h** (recorded 2026-10-07 at the orchestrator's direction; the cache
      is not changed here): must Design note 12's cache hold the generated stars, not only the
      records, for the warm budget of at most 25% of cold? The hierarchy-bound ruling's Risks
      already flag it (`decision-p11-t16-hierarchy-bound.md`, "The warm budget": with records
      alone a warm census still generates the survivors, about 20–35% of cold after T8.g). T8.h
      measures it.
      _Answered by R06.T8.h (2026-10-08, `decision-r06-t8h-warm.md` §2.4): no. The cache holds
      records and their bounds, and the warm budget is re-stated as structural, with 25% a
      recorded target (T17)._
- **Deviations in T11.c, as built (2026-10-07).** The band, the limits, the discs, the tables and
  the landing switch, as the task sets them out, with these details.
  - **The landing switch.** `--serve-sky`, or `HYPERION_SERVE_SKY` (clap's boolish values), a
    switch like `--stop-on-stdin-close`, off by default, in `config.rs` and the README's options
    table. `config::SkyService` (`Unsupported`, the default, and `Served`;
    `hyperion_server::SkyService`) is set by `ServerConfigBuilder::sky_service`, held on
    `AppState` and logged at start (`sky_service`).
    - Off, `Handlers` answers `sky` with `not_served_yet("sky")` before reading anything, as before
      T11.a. On, it calls `sky::answer`.
    - Tests: `config`'s `the_sky_is_served_only_when_its_switch_is_on` and its lists of options and
      variables; `requests`' `kinds_without_a_handler_are_answered_unsupported`, which takes `sky`
      again (the unit harness leaves the switch off); and `tests/sky.rs`'s
      `with_the_switch_off_a_sky_is_unsupported_and_no_job_reaches_the_pool`, in which the pool's
      counters, the tables' and the cells' are unchanged by the request. Every other server in
      `tests/sky.rs` turns the switch on.
    - **For R06.T11.d's landing** (first ruled for T8.g's; moved 2026-10-08,
      `decision-p11-t17c-bright.md`, a pointer in T11.d): `SkyService`'s default becomes `Served`. A
      clap `SetTrue` switch cannot then be turned off on the command line, so the option takes a
      value too (for example `--serve-sky=false`); the variable already takes `0` or `false`.
  - **The tables** (`compute/sky_tables.rs`, new).
    - `SkyTablesService`: a `SharedByteLru<GalaxyKey, SkyTables>` behind a `SingleFlight`, keyed by
      `GalaxyKey` alone and made with the one source its server's caps name. Its budget is
      `HYPERION_SKY_TABLES_MB`, or `--sky-tables` (default 160 MiB, a whole MiB, 0 caches
      nothing), with `ServerConfigBuilder::sky_tables_bytes` and the start log's `sky_tables_mib`.
    - `SkyTablesCounters` (the cache's `LruCounters` and `builds`) are on
      `ServerStats::sky_tables()`, beyond the plan. A request that builds counts two misses, one
      before its flight and one inside it, as the density maps do, and one that joins a flight one.
    - An entry is `SkyTables`: the luminosity tables, the envelope (the fitted table) and the cells'
      offset bounds, charged their `heap_bytes`. `SkyTables::new(tables, galaxy)` makes one;
      `compute::sky::tables` and `SkyTables::build` are gone.
    - The build is bulk jobs under the flight's own token, cancelled once every waiter has gone:
      one job makes `LuminosityTables::plan`; then each stage's sample jobs, and its accumulation
      jobs, each bin's `BinSums` moved into its job and handed back; then one job assembles the
      tables.
    - The next stage's samples are queued just after a stage's accumulation, so the workers take
      the accumulation first: the one-stage lookahead that the memory fix's record leaves to T11.c
      (Risks, "The job split in stages"). It holds two stages' samples at once, about 440 MiB on 16
      threads against 240 MiB. Not measured here: T17 times the build.
    - Equal on the wire: the near-Sun test below holds every star's 24 bytes and every texel's 12
      over the pool-built tables equal to those over `LuminosityTables::build`. The tables are not
      compared directly; the sim's `parallel_build_equals_serial` holds the split.
  - **Forced caps and the tables (a test seam beyond the plan).** `SkyCaps` also says which tables
    a sky reads: `SkyTablesSource::Galaxy` for the derived caps, and `Dark` (T11.a's
    `LuminosityTables::dark`, built at once) for `SkyCaps::forced`. `with_galaxy_tables()` builds
    the galaxy's own on the pool under forced caps, for the near-Sun test. Under dark tables the
    band holds only the overflow's stars.
  - **The caps as ray-chunk jobs (a sim change, additive).** T7's text leaves the split to the
    server, but the sim had no seam for it. Added:
    - `RayExtinctions::measure_rays(galaxy, origin, rays, which, cache)`, one share of a lattice,
      and `RayExtinctions::join(shares)`, which refuses shares out of order or of other lattices
      (it now records its origin, lattice and first ray);
    - `sky::caps::layer_caps_over(galaxy, tables, envelope, observer, cut, &rays)`, the caps over
      a measured lattice of `CAP_RAYS`; `sky::census::census_plan_of(query, caps)`, the plan to
      given caps; and `SkyQuery::forced_caps()`, which the server's plan branches on;
    - `count` split into the measure and `count_over`, with the same arithmetic, so no bit moves.

    The server measures 24 rays a job (`CAP_JOB_RAYS`, 32 jobs, each with its own noise cache),
    then one job counts them in ray order and makes the plan. A query's forced caps are one job, as
    before. Tests: the sim's `caps_over_rays_measured_in_shares_are_layer_caps` (the rays and the
    caps near the Sun at 7.95, bit for bit), `shares_out_of_order_are_refused`,
    `a_plan_to_given_caps_is_the_plan_that_forces_them` and `measure_rays`' doctest; the server's
    `the_caps_rays_measured_in_jobs_are_the_sims_caps`, over dark tables, so every count is nought
    but each rule bound reads every ray.

  - **The band** (`compute::sky`).
    - The march reads no census, so it runs while the census does (`try_join`): two rows of a face
      a job (`BAND_JOB_ROWS`, 192 jobs of 128 rays), each `march_rows` of the one reply the plan's
      `CompleteTo::of_caps` states, at the query's band (`SkyQuery::band_spec`).
    - After the merge, where the eye is asked, one job builds the glare (`Glare::of_listed` at the
      eye's cut). Then one job a march sums it (`sum_rows`) and sets its texels' limits
      (`limit_rows`), and one more job gives every listed star its eye offset (`eye_offsets`) over
      the whole band.
    - Without the eye there is no glare, no limit and no offset: each texel's eye limit is the
      wire's `i16::MIN`, and each star's eye offset its colour offset alone against a scotopic
      background, T11.a's interim, unchanged.
    - `BulkJobs` (new) queues jobs and gives their results in order. A failed job, or a dropped
      future, abandons the jobs not yet begun, as the census's do.
    - Each sum walks the census's whole overflow to find its texels' stars. At 10⁶ overflowing
      stars (a camera's V 11 near the Sun) that is some 15 ms a job, 3 CPU-s a reply, beside a
      census of 10⁵ CPU-s. At the eye's cut and the default N_max nothing overflows near the Sun
      (3–6 × 10⁴ listed). Not measured.
    - `eye_offsets` is one job over every listed star, the sim's signature taking them all. Its
      cost at N_max is not measured, and `sky/limit_map` does not time it: T17's per-reply cost of
      the limit map should include it.
  - **Single jobs of seconds.** The tables' plan (about 5 CPU-s for the Milky Way), its assembly,
    the eye's cut (two pre-passes), the glare and the eye offsets each run as one job. On a busy
    pool a chart's query can wait for one of them, though never in the interactive queue. T11.d
    sizes the jobs.
  - **The discs.** `exclude_system`'s stars come from the server's system cache
    (`SharedSystemCache::get_or_generate`) in one bulk job, then `host_discs` at the request's
    time, each a `HostDiscDto`. A system whose stars are not generated (a centre member, a rogue
    planet: `KindNotGenerated`, `LayerNotGenerated`) has no disc, and the reply's `hosts` is empty;
    a system that does not resolve, which `check_exclude` has ruled out, panics the job
    (`internal`).
  - **The wire.** `wire_star` takes the eye offset, and `wire_texel` (new) gives a texel's
    luminance as `f32`, its chromaticity, its eye limit (`i16::MIN` where the eye was not asked)
    and ρ. The doc comments on the eye offset say where it now comes from: `hyperion-protocol`'s
    `sky.rs`, `@hyperion/protocol`'s `eyeOffsetMag`, the server's `SkyStarWire` and
    `view/sky/cull.ts`. No bytes change.
  - **Tests.**
    - `cargo nextest run -p hyperion-server`: 472 passed, 5 skipped, 299 s at 4 threads under load
      7–12. Of it, `--test sky` is 11 tests. A served sky now also marches its band, some 27 CPU-s
      in a test build, so each served sky takes 45–60 s; the refusal tests march nothing.
    - `a_sky_near_the_sun_returns_the_stars_texels_and_host_discs_the_sim_returns` (81 s): forced
      to 30 ly over the galaxy's tables, built on the server's pool and serially in the test. It
      asks the eye and a camera's V 9, so the march keeps the eye's light (R06.T9.j), `n_max` 16, so
      that the overflow is splatted into the band, and a system of the Sun's own cell in layer E
      left out. Each star's 24 bytes and each texel's 12 equal those Design note 17's table makes
      of the sim's census, its `march_rows` of the same one reply and `sum_rows`, `limit_map` and
      `eye_offsets`, as T9.f's record asks of this comparison; the hosts equal `host_discs`. A
      nextest override puts it in the `sky-tables` group at four slots.
    - `a_second_identical_sky_shares_the_tables_build`: two skies at once on two connections, one
      build, the same bytes. `a_second_sky_in_another_time_bucket_shares_the_build`: at the epoch,
      then 900 years before it, one build and a hit. Both over dark tables, whose build is
      moments, so a cache hit would also give one build: the flight itself is pinned by
      `compute::sky_tables`' unit tests.
    - The two T11.b tests' `band_bytes` of 0 become six faces of 64² texels.
    - Unit tests: `compute::sky_tables`' (two skies asked at once, the worker held, share one build
      with three misses; a build no sky waits for is given up, its job skipped; a budget of nought
      keeps nothing; another galaxy's key is refused); `compute::sky`'s band jobs' order,
      `BulkJobs`' order and a dropped `BulkJobs`' jobs not run; `requests::sky`'s
      `a_wire_texel_is_its_band_texel`, `a_host_disc_on_the_wire_is_the_sims` and
      `an_excluded_systems_stars_are_its_discs_and_a_rogue_planet_has_none`.
    - Acceptance as built: `cargo test -p hyperion-server --test sky`; also the server's lib tests
      (`compute::sky`, `compute::sky_tables`, `requests::sky`, `config`),
      `cargo test -p hyperion-sim sky::caps`, `cargo test -p hyperion-sim sky::census::query` and
      `measure_rays`' doctest.
  - **Open, for the owner (low priority).** When the observer's own system is a centre member,
    `hosts` is empty and `not_modelled` says nothing of it, so the view would draw no disc with no
    label saying why (Design note 23's honesty). No RM3 view is expected to start in one.
  - **Not done here (T11.d's first items).**
    1. R06.T9.g's illumination (not landed). The request's illumination is built first, 1,536
       rays as bulk jobs, then its fixed point, and stated on the query
       (`SkyQueryBuilder::illumination`), which `march_rows` and `band_rows` read with their
       signatures unchanged; `eye_cut` gains an `Option<&Illumination>` (`decision-r06-t9g-dgl.md`,
       §3.4 and §5). The server calls `march_rows` once (`compute::sky::march`) and `eye_cut` once
       (`compute::sky::eye_cut`).
    2. R06.T7.b's per-ray radii, which the reply carries. Since R06.T7.b `CompleteTo` is `Clone`,
       no longer `Copy`, and `compute::sky::march` and `band` clone it into each job; the wire's
       `cap_ly` is each layer's farthest ray.
    3. R06.T7.b's eye visibility map (`sky::limits::eye_visibility`, the eye-cut pre-pass's 16²
       limits, set on `SkyQueryBuilder`), which the server sets on eye-only requests, with the
       request's illumination. `compute::sky::plan` already takes it where a query asks it; near
       the Sun it opens 37% fewer systems than the uniform caps at the eye's cut.
    4. The near-Sun test marches one reply; T11.d's several replies change the replies it gives.
    5. The server bench `sky_near_sun_cold`, T11.d's since T11.b.
- **Deviations in T11.d, as built (2026-10-08).** Delivery nearest first, as the census-cost
  ruling, its sign-off (question 2), the listing ruling (`decision-r06-t8i-listing.md`) and the
  brackets ruling (`decision-r06-t7b-brackets.md`) set it out, with T11.c's "Not done here" items
  1–5. The label and its guide rows are split off as R06.T11.f. No golden moves and no bump
  (determinism audit, `golden_diff` 0; GENERATOR_VERSION 21). `PROTOCOL_VERSION` stays 2.
  - **The hookups.**
    - The illumination (R06.T9.g): its 16² rows two to a job (`ILLUMINATION_JOB_ROWS`, 48 jobs),
      then `Illumination::assemble` in one, which gives `Illumination::march`'s bits (tested). It
      is stated on the query, and `eye_cut` and `eye_visibility` take it.
    - The eye's visibility (R06.T7.b): one job, only for a request with the eye and no camera part
      (`SkyAsk::counts_by_visibility`, tested), never by comparing cuts. `camera_limit_v` is taken
      as asked.
    - The per-ray radii on the wire, below. The near-Sun test marches the plan's replies and sums
      at the census's radii; to 30 ly it is one reply, final.
    - The bench `sky_near_sun_cold`, below.
  - **The order** (`compute::sky::delivery_steps`). Step k holds every layer's k-th shell, as
    `CensusPlan::shells` ranks them, except C's from its fourth (2,000–4,000 ly) on, a step later.
    So D's and E's shells to 4,000 ly run before C's beyond 2,000 ly. Near the Sun (caps C 14,563,
    D 13,232, E 61,341 ly) that is 8 steps, and 8 replies. The first step holds every layer's
    first shell, so A, B and the brown dwarfs are final from the first reply, and the march needs
    no `nowhere()`. A plan of no shell is one step of none. The order is the plan's alone. Later
    steps' jobs may run while an earlier step's last ones do; the replies go out in order. T17's
    V 3.0 gate is not measured (T17's).
  - **The census in steps** (`census_in_steps`).
    - **Jobs by time, not by expected work (deviation).** A cell's cost is unknown until its
      records' bounds are read: most are skipped in microseconds, a few generated for seconds. So
      a census job runs its cells until 50 ms (`CENSUS_JOB_SLICE`) are spent, and at least one
      cell, then hands back the rest of the slab it was in, walked to its end and split in two
      halves (`CellWork::split`), and the slabs it had not begun. They go back in the queue ahead
      of every later step's work, so a slab of dear cells spreads over the workers. Which job
      holds which cells changes no census (the merge is total, the tallies are integer sums and
      flags; determinism audit). A job takes one slab's cells, or a part of them: jobs of up to 32
      slabs, built first, held the slabs a job had not begun until its slice was spent, so the
      first step ran on about one worker at a time (below).
    - At most 2 census jobs a worker are outstanding (`CENSUS_JOBS_PER_WORKER`; T11.a had 256), so
      a later step's jobs, each reply's own jobs and other requests' bulk jobs wait behind few.
      `CpuPool::workers()` is new (crate).
    - The cell cache is the server's shared one, so a cancelled or superseded census keeps every
      cell it built (not tested apart: T11.b's warm test holds the sharing).
    - **On R06.T8.h's block cache** (merged 2026-10-08, after T8.h landed): each census job reads
      and keeps its cells through T8.h's blocks keyed by magnitude, by the same `census_cell`
      through its context's handle, the stored bound a pre-filter only. Jobs of one step filling
      one block side by side all land (`insert_if_unchanged`). The steps' unit test takes T8.h's
      counters (every cell missed cold and served warm) and compares each step's stars with the
      sim's serial census by their bits, as T8.h's jobs test did. After the merge: the server's
      suite 480 passed, 5 skipped; workspace clippy and the sim's wasm32-wasip1 clippy clean; the
      bindings check clean.
    - **Single jobs of seconds remain (deviation; T11.c's list).** The tables' plan and assembly,
      the eye's cut and visibility, and each reply's merge, glare, eye offsets and payload. Not
      split here.
  - **Each reply** (`requests::sky::Reply`). The march is one per request over `plan.replies()`
    (R06.T9.f), as `Vec<Arc<BandMarch>>`, awaited before the first reply. After each step,
    `merge_step` (one job) merges every step's parts with `merge_shells` at
    `plan.completeness(done)`; `band` sums each march at the census's own `complete_to()`
    (`sum_rows` keeps its argument; T8.i's "not taken"), then the glare, limits and offsets; one
    job encodes the payload. The discs are made once, before the first reply, and every reply
    carries them. Every reply but the last goes through `Replies::send`; the last is the
    terminal response. The final reply's stars, overflow and tallies are the one-shot census's.
    Its band is not the T11.c band's bit for bit: the march keeps every shell edge as a node,
    within T9.f's quadrature (up to 2.0% a texel at 12 nodes a decade, measured there;
    determinism audit), which no golden reads.
  - **The wire.**
    - `ServerMessage::PartialResponse { id, body }` (`partial_response`, new): each answer but the
      last, after its own chunks, numbered from 0 again. Sent only for `sky`, so a version 2
      client that asks none never receives one, as the `notification` ruling has it; R03's Risks
      has a pointer. Plan 04's reserved `response_part` (parts of one payload) is not reused for
      it: an answer in parts is a whole answer that a later one replaces.
    - `SkyResponse.final` (`is_final` in Rust); `SkyLayerCensusDto` gains `complete_to_ly` (the
      fixed edge reached while not final, `cap_ly` once final), `complete_to_rays_ly` and `final`,
      and is no longer `Copy`.
    - **The per-ray table (deviation from T8.i's "none once final").** `complete_to_rays_ly` is
      the census's `CompleteTo::rays_ly` (new, sim): the cap's rays held within the edge, and the
      cap's rays themselves once final, as T7.b's "Open" asks; empty where the layer is complete to
      one radius every way (forced caps, or all rays reaching the edge). Its length is the ray
      count; there is no field of its own. No client reads it yet. A final layer's table is
      1,536 numbers of JSON, so a reply near the Sun carries A's, B's and the brown dwarfs' from
      the first (some 100 KB).
    - The server: `Handler::handle` takes `Replies` (each request's ID and serial, and the
      connection's channel of `PARTIALS_QUEUED`, 2). The connection takes partials only while
      fewer than 2 stream, drains the channel before a request's end so that no terminal frame
      overtakes its last partial, drops a partial whose request has ended (`Requests::is_current`,
      by serial), and queues a partial frame after its chunks under the frame-count limit, not
      held for byte room as a terminal frame is (consider, Rust review). A large partial body is
      serialised on the pool as a response's is (`serialise`, shared).
    - The client: `RequestClient.requestBulk(body, manifestOf, { onPartial })` checks each partial
      answer against its own manifest and restarts the assembler; a partial of another kind is a
      protocol violation.
  - **The client** (`view/sky/useSky.ts`). Replies are decoded one at a time; each is held whole in
    place of the one before, in one render, so the model is never empty between replies and
    `SkyCubeCache.acquire` bakes the new cube before the frame draws it (unchanged). A partial
    reply still waiting when a later one arrives is passed over; the final is always held.
    `pending` is now true until the request's first reply is held, so R07's `LIGHTING: PENDING`
    clears at the first reply, whose discs are the final ones. `skyRequestReason` gains
    `"partial"`: a held reply that is not final is asked again once no request is in flight (a
    lost link mid-sky). `connection.ts` lists `partial_response`.
  - **Test seams (beyond the plan).** `SkyCaps::forced_per_layer` (crate), `forced_radii` (test),
    `on`, and `with_shell_edges` / `shell_edges_ly`: a test's nearer shell edges. The sim's
    `census_plan_with_edges` is `#[doc(hidden)] pub` (it asserts the edges ascend from above
    nought) and is the server's plan path, with `SHELL_EDGES_LY` for every sky a client asks.
  - **Tests.**
    - `--test sky`, 12: `a_sky_arrives_nearest_first_each_reply_the_census_to_its_stated_radii`
      (new): caps forced to 30 ly, test edges 10, 15, 20 and 25 ly, 6 replies. Each is a whole
      sky's frames; each layer states its radius and finality; its stars, byte for byte, are the
      sim's census of the shells done and the census forced to the reply's radii less its stars
      beyond them; its band is the sim's one march's sum at those radii, byte for byte; the least
      edge runs 10, 15, 20, 20, 25 ly and then none; the last is the one-shot census. Uniform caps
      only: caps by ray are the sim's tests' (T8.i).
    - The range query's wait is bounded in jobs, not timed: between its sending and its answer the
      one worker completes at most twice its own jobs plus one. Each census job is bounded by its
      slice and one cell; a march job (0.1 s) is the longest bulk job a query waits behind.
    - The first-sky budget is the bench's, below, not a test: no test build affords a census to
      500 ly.
    - Unit tests: `compute::sky` 8 (each step's census against the sim's serial census at slices of
      0 and 50 ms over three workers, cold and warm; the delivery's order; one march holding every
      step's radii; the illumination, cut and visibility in jobs; the forced caps); `ws` (answers
      in parts in order, a cancel between them, a failure after them); `requests` (an answer sent
      before a cancel is not the next request's under its ID); `requests::sky` (the visibility
      rule); `sky::band::a_layers_rays_are_its_caps_held_within_its_edge`; the protocol's wire
      form of `partial_response`; `bulk.test.ts` (5), `useSky.test.tsx` (4), `model.test.ts` (1).
  - **The bench** `sky_near_sun_cold/first_reply` (`benches/sky.rs`): a fresh server with the
    default workers (15), the server's own galaxy for seed 0x4d2, the default eye near the Sun at
    the epoch, eye-only, timed from sending to the first reply. Criterion's 10 samples are hours,
    so it runs with `--test`, one iteration.
    - **Measured (2026-10-08, one iteration under the heavy lock, release; other lanes' unlocked
      work kept the load at 17–21; provisional):** the first reply in **922 s wall and 12,594
      CPU-s** of the server's process, against T17's 10 s and 150 CPU-s. Cut V 7.766 (with the
      diffuse light); 28,549 listed and none overflowing; A (to 11.0 ly), B (56.0 ly) and the
      brown dwarfs final, C, D and E to 500 ly. That was with jobs of up to 32 slabs.
    - **Attributed** (the handler's phase log, the bench's `RUST_LOG`; unlocked at
      `CPUQuota=400%`, so four cores' worth of 15 workers; provisional). With jobs of 32 slabs: the
      first reply in 875 s and 2,592 CPU-s, the first step's census jobs 852 s in all, so it ran on
      about one worker at a time, and the locked run's other cores ran later steps' jobs. With one
      slab a job (as built): the first reply in **286 s wall and 837 CPU-s**. Its phases, from the
      request: the tables 18.2 s (cold), the illumination 0.4 s, the eye's cut and visibility 2.2
      s, the caps and plan 4.7 s, the march done at 31.0 s; then the first step's census, **764
      job-seconds** (C, D and E to 500 ly, with A, B and the brown dwarfs), and its reply 0.6 s.
      So some 100 CPU-s before the census and some 760 in it: on 15 workers about 60 s wall.
    - **As built, under the heavy lock** (2026-10-08, one iteration, release, 15 workers, load
      about 5 at the start; provisional): the first reply in **100.5 s wall and 1,084 CPU-s**,
      about 10 and 7 times T17's 10 s and 150 CPU-s. The tables 12.3 s (cold), the illumination
      0.3 s, the eye's cut and visibility 2.4 s, the caps and plan 2.3 s, the march done at
      19.6 s; the first step's census 1,183 job-seconds on 15 workers (about 80 s wall), and its
      reply 1.4 s. An earlier locked run of the same code, its close then mishandled, read the
      reply at 86.4 s.
    - **A finding, not fixed (FEATURES FIRST; for the orchestrator and T17).** The sign-off
      estimated the 500 ly first shell at some 30 CPU-s; it is some 760 CPU-s here (seed 0x4d2's
      galaxy at the eye's cut V 7.77, before T8.g's final landing). A sky near the Sun is not
      served within T17's first-sky budget until T8.g and T17 re-take it. The cold tables are in
      the figure (some 12 s wall); whether they count inside the budget is T17's reading.
  - **Not measured** (T17's): the time to each later reply, the per-reply cost of the band and
    limit map, the V 3.0 gate, a cancelled census's cells kept, and the visibility caps' safety
    margin with the illumination (T7.b's "Open").
  - **Files beyond T11's list.** The protocol's `envelope.rs`, `lib.rs` and `sky.rs`; the server's
    `ws.rs`, `bulk.rs`, `compute/pool.rs`, `scene/topic.rs`, `testing.rs`, `Cargo.toml` and
    `benches/sky.rs`; the sim's `sky/band.rs` and `sky/census/{mod,query}.rs`;
    `@hyperion/protocol`'s `requests.ts`, `index.ts`, `bulk.test.ts` and generated types; the
    client's `connection.ts`,
    `view/sky/{model,useSky}.ts`, their tests and `test/{FakeWebSocket,skyFixtures}.ts`. Not
    touched: `config.rs` and `stats.rs`.
  - **Gates** (capped, build-slot, 4 jobs, `CPUQuota=400%`): fmt; clippy `-D warnings` on the
    server, the sim and the protocol (all targets) and on the sim for wasm32-wasip1;
    `cargo nextest run -p hyperion-server` 480 passed, 5 skipped (397 s); `--test sky` 12/12; the
    sim's `sky::band` tests; `just gen-protocol` and the bindings check; `pnpm typecheck`, `lint`
    (oxlint `--deny-warnings`) and Prettier; vitest over the protocol package and the client's
    `view`, `lib` and `displays/view`.
  - **Reviews.** Rust: two must-fixes (a step and the request's universe taken by value; the
    edges' unit in their names) and its should-fixes applied (the partials bounded and drained
    before a request's end; tests of a failure after partials and of a reused ID; the sim's edges
    checked once; crate visibility; the docs' section order; the query taking the illumination);
    its considers on `Replies::detached` and a shared serialiser applied, the partial frame's byte
    room recorded above. TypeScript: its must-fix (`void` on the discarded promises) and
    should-fixes (no second copy of the held request; a test that the reply held stays while the
    next decodes) applied, and its considers. Determinism audit: nothing must-fix or should-fix;
    its considers applied (the final band's bits recorded above; each reply's stars compared byte
    for byte). Plan conformance: its must-fix, the first reply's miss unattributed, applied (the
    phase log; the attribution found the jobs of 32 slabs, now one); its should-fixes applied (this
    entry, R03's pointer, the jobs of seconds and the job-counted bound recorded) and its considers
    (the visibility rule's test; the bench run with `--test`), but a test that a cancelled census
    keeps its cells, recorded above.
- **R06.T5.f's measurements, as built (2026-10-07, generator version 21; `decision-r06-t9b-band.md`,
  item 8).** The tables against the realised sky, as the task sets it out: a record that gates only
  its own sample. Seed 0x0926_0000 (the fixture), at the epoch, on the shipped tables (T5.d's
  correction as refitted at 21).
  - **The build.**
    - `crates/hyperion-sim/tests/sky_realised.rs`, one slow test, `tables_against_the_realised_sky`.
      It keeps the plan's name, which its acceptance filters by. It runs on 8 threads, and its
      nextest slow-profile override takes 8 slots. It is native only, since wasm32 has no threads.
    - The tables are built once from `LuminosityTables::plan` on the test's threads and assembled
      three ways from the same `BinSums`: as shipped, with the full pair counts and as single stars.
    - The test-only reads (`sky/luminosity.rs`) are two `#[doc(hidden)] pub` plan modifiers, as
      `sky::envelope` exposes its fit's helpers, since the integration test reaches only the public
      API:
      - `TablesPlan::with_full_pair_counts` takes each 1-mag bin's whole fitted count difference,
        spread over its sub-bins as the shipped counts spread it, neither held at the single-star
        count nor made non-decreasing. Its light, colours and `pair_light` are the shipped tables'
        bit for bit;
      - `TablesPlan::without_pair_correction` gives the single-star tables. It is a deviation: the
        task names one read, and the paired counts need the single-star counts (the science check).
    - `sky/binary_light.rs`: `apply` takes `PairCounts` (`Excess`, the shipped rule, unchanged;
      `Full`), and the two count rules are `count_excess` and `count_whole`. The shipped path's bits
      are unchanged: `parallel_build_equals_serial`'s three pins hold. Unit tests:
      - `the_full_count_rule_keeps_the_deficit_and_changes_no_light`;
      - `the_full_pair_counts_keep_the_light_and_the_deficit`: the shipped counts are, edge by edge,
        the running maximum of the single-star and full counts; the halo loses stars at some edge;
        a plan without the correction is unchanged.
  - **1. The paired deficit** at the solar circle: T5.c's 6³ block at (0, 26,000, 0) ly and its
    turns about the centre, in the plane. f = Σ(S − P) ÷ ΣS over every realised system, against the
    fit's correction over the same blocks (−`pair_light` ÷ the single-star light, its 1σ added
    linearly over components and blocks, an upper bound):

    | Layer        | Blocks (cells) | Systems   | Realised f    | The fit's     | Difference          | Interval |
    | ------------ | -------------- | --------- | ------------- | ------------- | ------------------- | -------- |
    | C            | 64 (13,824)    | 165,055   | 2.56% ± 0.49  | 2.01% ± 0.28  | +0.55 points, +1.0σ | 1.64     |
    | D            | 256 (55,296)   | 1,062,507 | 8.91% ± 0.53  | 8.67% ± 0.60  | +0.23 points, +0.3σ | 1.94     |
    | E (recorded) | 64 (13,824)    | 557,330   | 14.94% ± 3.41 | 17.09% ± 0.55 | −2.15 points, −0.6σ | 13.06    |
    - The 1σ is the ratio of sums' (the delta method). T5.c's form, √Σδ² ÷ ΣS, gives 0.50, 0.54 and
      3.77, and a jackknife over the blocks 0.43, 0.59 and 3.97. The interval is a half-width:
      3.29 times the widest of the three, asserted under 3 points in C and D (the full widths are
      3.28 and 3.88). It leaves out the fit's 1σ, which no number of cells can shrink.
    - D's 256 blocks are the most that fit about the circle without sharing a cell (638 ly apart,
      a block 384 ly wide). A pilot of 8 blocks a layer gave a 1σ of 1.3 points in C and 3.2–4.9 in
      D, whose few bright systems carry its scatter. E is not asked for; it places E's count
      finding below.
    - T5.c's block alone gives C 9.44% and D 21.12% against the fit's 2.06% and 10.56% there:
      T5.c's 9.4% and 21.0% were that block's scatter (about 1.9σ and 1.0σ). The fit's D share
      varies with azimuth (10.56% at T5.c's block, 8.67% over the circle), as the young disc does.
    - The same systems' light against the tables (1σ compound Poisson, then the blocks' jackknife):

      | Layer | Pair-evolved against the tables | Single against the single-star tables |
      | ----- | ------------------------------- | ------------------------------------- |
      | C     | 0.955 ± 0.010 ± 0.010           | 0.961 ± 0.010 ± 0.010                 |
      | D     | 1.000 ± 0.010 ± 0.010           | 1.003 ± 0.009 ± 0.009                 |
      | E     | 1.103 ± 0.072 ± 0.061           | 1.075 ± 0.068 ± 0.063                 |

      C's single-star light by component falls with age: the young thin disc 1.021 ± 0.040, the
      old thin disc's sub-discs at 0.56, 1.5, 3.1, 5.6 and 8.6 Gyr 0.962, 0.996, 0.970, 0.946 and
      0.925 (± 0.017–0.027), the thick disc 0.832 ± 0.069. D's components lie within 1–2σ of 1
      but the oldest sub-disc, 1.052 ± 0.021. E's, at 0.05, 0.56 and 1.5 Gyr, are 1.08 ± 0.08,
      1.026 ± 0.016 and 1.138 ± 0.024.

    - The same systems' stars brighter than M<sub>V</sub> 4, 2 and 0 (1σ compound Poisson by
      system), single against the single-star tables and pair-evolved against the full counts,
      and the pairs' change of the count against the fit's:

      | Layer, M<sub>V</sub> | Single        | Pair-evolved, full counts | The pairs' change | The fit's |
      | -------------------- | ------------- | ------------------------- | ----------------- | --------- |
      | C, 4                 | 0.969 ± 0.006 | 0.970 ± 0.006             | −1.64% ± 0.09     | −1.75%    |
      | C, 2                 | 0.965 ± 0.014 | 0.965 ± 0.014             | −2.50% ± 0.30     | −2.51%    |
      | C, 0                 | 0.922 ± 0.051 | 0.894 ± 0.050             | −2.41% ± 2.76     | +0.57%    |
      | D, 4                 | 1.013 ± 0.003 | 1.021 ± 0.003             | −20.79% ± 0.10    | −21.41%   |
      | D, 2                 | 1.002 ± 0.004 | 1.003 ± 0.004             | −13.71% ± 0.13    | −13.86%   |
      | D, 0                 | 1.008 ± 0.007 | 1.015 ± 0.007             | −6.23% ± 0.20     | −6.87%    |
      | E, 4                 | 1.111 ± 0.005 | 1.165 ± 0.006             | −28.95% ± 0.17    | −32.27%   |
      | E, 2                 | 1.077 ± 0.008 | 1.113 ± 0.009             | −28.35% ± 0.25    | −30.63%   |
      | E, 0                 | 1.037 ± 0.013 | 1.048 ± 0.015             | −23.16% ± 0.46    | −23.97%   |

      The shipped counts of D and E equal the single-star ones there, as the excess-only rule
      gives where pairs remove stars. The fit's count change carries no tabulated error, so the
      pairs' columns differ by more than their realised σ alone would allow without being a test.

  - **2. Eight observers** at (26,000 sin φ, 26,000 cos φ, 68) ly, φ = 0°, 45°, …, 315° (observer 0
    the sim's Sun-like place), each a census to V 8 within 300 ly with every cap forced and no eye,
    its stars within 300 ly. The expectation integrates the density field times the tables along
    768 Fibonacci rays at 1-ly nodes, each ray through its own realised dust at the census's
    sightline quality (`Budget(64)`), selected at the cut less the distance modulus and the Sun's V
    extinction behind the ray's A<sub>V</sub> (T8.k's subtraction; the census's own stars take
    their own colours', some 0.01 mag apart within 300 ly). Light is V flux, Σ 10<sup>−0.4 V</sup>
    of the dimmed stars, not the band's photopic lux. Per layer, listed against the expectation:

    | Observer | C listed, tabulated, full | Light, from 50 ly | D listed, tabulated, full | Light, from 50 ly | E listed, tabulated, full | Light, from 50 ly |
    | -------- | ------------------------- | ----------------- | ------------------------- | ----------------- | ------------------------- | ----------------- |
    | 0        | 5,682, 5,998, 5,894       | 0.859, 0.957      | 633, 852, 653             | 0.876, 0.841      | 139, 168, 111             | 0.651, 0.795      |
    | 1        | 9,318, 9,747, 9,594       | 0.988, 1.028      | 1,603, 1,879, 1,579       | 1.423, 1.361      | 336, 402, 290             | 0.607, 0.699      |
    | 2        | 5,765, 6,020, 5,916       | 1.004, 1.004      | 651, 853, 654             | 3.594, 1.266      | 151, 168, 111             | 0.471, 0.504      |
    | 3        | 7,526, 8,142, 8,013       | 0.914, 0.926      | 1,528, 1,740, 1,478       | 1.270, 1.006      | 311, 377, 273             | 0.708, 0.817      |
    | 4        | 5,518, 5,693, 5,594       | 1.123, 0.974      | 658, 827, 635             | 0.883, 1.081      | 122, 163, 108             | 0.551, 0.677      |
    | 5        | 9,174, 9,718, 9,566       | 1.123, 0.930      | 1,634, 1,876, 1,577       | 0.992, 1.048      | 285, 402, 290             | 0.346, 0.420      |
    | 6        | 5,816, 6,008, 5,904       | 1.049, 0.974      | 676, 853, 653             | 1.328, 0.962      | 136, 168, 111             | 32.4, 0.767       |
    | 7        | 8,783, 9,293, 9,147       | 0.878, 0.948      | 1,508, 1,841, 1,551       | 0.821, 0.893      | 358, 396, 286             | 0.766, 0.942      |

    The observers between the axes (odd) hold about 1.6 times the others' C and 2.2 times their D
    and E stars, and the tables expect it: the density there (the fixture's arms). Observer 0
    reproduces T9.b's single observer (C 5,679, D 642 and E 143 listed, 86%, 87% and 63% of the
    light) to the census's changes since (T8.k, T9.e). A and B list 2–7 and 32–56 stars each.
    - **The ensemble's counts**, σ the listed stars' compound-Poisson scatter by system (√Σk²)
      alone, since the fit tabulates no error of its count difference (so each σ is an upper bound
      on the significance):

      | Layer | Listed | Tabulated (ratio) | Full pair counts (ratio)        |
      | ----- | ------ | ----------------- | ------------------------------- |
      | A     | 33     | 32.8 (1.006)      | 32.8 (1.006 ± 0.190)            |
      | B     | 378    | 342.1 (1.105)     | 342.1 (1.105 ± 0.061, +1.7σ)    |
      | C     | 57,582 | 60,616.8 (0.950)  | 59,628.1 (0.966 ± 0.004, −7.8σ) |
      | D     | 8,891  | 10,720.3 (0.829)  | 8,780.6 (1.013 ± 0.013, +1.0σ)  |
      | E     | 1,838  | 2,242.2 (0.820)   | 1,578.9 (1.164 ± 0.034, +4.9σ)  |

      The brown dwarfs, some 10⁻⁸ stars a census, are left out.

    - **The skew's Monte Carlo**, 20,000 trials of all eight observers, each trial against its
      model's exact mean, which lies within 0.2% of the rays' expectation for every observer but
      observer 3 (−1.3% to −1.6% in C, D and E, its patchier dust). The eight observers' median
      light ratio against the distribution of the trials' eight-observer medians, z from the share of
      trials at or below it (the half-width's z beside it):

      | Ratio                         | Eight observers' median | One observer's (interquartile) | Eight's median (16–84%) | z             |
      | ----------------------------- | ----------------------- | ------------------------------ | ----------------------- | ------------- |
      | A within 300 ly               | 0.805                   | 0.685 (0.400–1.111)            | 0.691 (0.507–0.929)     | +0.51         |
      | B within 300 ly               | 0.968                   | 0.867 (0.719–1.073)            | 0.869 (0.775–0.986)     | +0.86         |
      | C within 300 ly               | 0.996                   | 0.964 (0.922–1.021)            | 0.965 (0.938–0.997)     | +0.98         |
      | D within 300 ly               | 1.131                   | 0.910 (0.816–1.041)            | 0.911 (0.850–0.984)     | +2.40 (+3.02) |
      | E within 300 ly               | 0.629                   | 0.656 (0.456–0.988)            | 0.661 (0.534–0.840)     | −0.23         |
      | C from 50 to 300 ly           | 0.965                   | 0.996 (0.971–1.024)            | 0.996 (0.981–1.013)     | −2.03         |
      | D from 50 to 300 ly           | 1.027                   | 0.972 (0.894–1.070)            | 0.973 (0.924–1.030)     | +0.95         |
      | E from 50 to 300 ly           | 0.733                   | 0.737 (0.516–1.082)            | 0.742 (0.604–0.930)     | −0.06         |
      | Every layer within 300 ly     | 0.964                   | 0.915 (0.840–1.023)            | 0.917 (0.870–0.976)     | +0.82         |
      | Every layer from 50 to 300 ly | 0.938                   | 0.956 (0.893–1.040)            | 0.957 (0.918–1.004)     | −0.47         |
      | Every layer within 200 ly     | 0.972                   | 0.882 (0.785–1.021)            | 0.883 (0.825–0.959)     | +1.14         |

      Each observer's own Monte Carlo is printed too. The Sun's place holds 0.821 of its light
      within 200 ly against its own median of 0.907 (interquartile 0.832–1.021), and the ruling's
      "about 0.90" was its estimate there. The observers between the axes, whose light more rare
      bright stars carry, have medians of 0.83–0.85.

    - The Monte Carlo's approximations, none of which matters at the 3σ thresholds (science check):
      - every 0.05-mag bin of the tables at the observer is a type at the bin's middle magnitude, as
        many stars as carry its light (±2.3% in a star's light across a bin);
      - each shell takes its sky-averaged density per component (exact, the light depending only on
        r once A is fixed) and its sky-averaged extinction (second order);
      - one light age, at 150 ly (about 10⁻⁵ in the light);
      - each star is drawn alone, though a system's stars stand together (Σk² ÷ Σk is 1.46 for D's
        listed stars and 1.54 for E's), so D's and E's distributions are somewhat narrow.

  - **Findings for the tables lane** (the task's rules; T5.d's fit and the tables, with these data
    and the logs below). Recorded, not fixed (FEATURES FIRST, 2026-10-07):
    - **C's counts** under the full correction, 0.966 ± 0.004 (−7.8σ). The pair correction is not
      its cause: the realised pairs change C's counts as the fit does (−1.64% against −1.75%
      brighter than M<sub>V</sub> 4) and its light as the fit does (2.56% against 2.01%). The
      realised single stars fall short of the single-star tables, by 3.1% in stars brighter than
      M<sub>V</sub> 4 and 3.9% in light, the more so the older the component (by 7.5% at 8.6 Gyr
      and 17% in the thick disc), and C's light beyond 50 ly is short by as much (0.965, −2.0σ
      against the skew). T5.c's gate allows 5% for
      the tables' median draws, track sampling and reference metallicity, which may hold it.
    - **E's counts** under the full correction, 1.164 ± 0.034 (+4.9σ). The realised single stars
      exceed the single-star tables by 11% brighter than M<sub>V</sub> 4 (by 4% brighter than 0),
      and the fit's count change removes more than the realised pairs do (−32.3% against −29.0% ±
      0.2): together 1.165, the ensemble's. The shipped counts, which the caps read, are the
      single-star ones there, so the second does not reach the caps; the first does, by E's bright
      stars short in the counts the caps take beyond their radii.
  - **What is settled.**
    - No paired-deficit finding: the fit's correction matches the realised in C and D at the solar
      circle, and in E within E's wide interval. The band ruling's suspicion that the fit
      under-corrects young D populations there is not borne out.
    - D's count deficit (T9.b's 0.75, 0.829 here) is the pair deficit the shipped counts leave out
      by design (1.013 ± 0.013 with it); its light is the tables' (1.000 ± 0.010).
    - The 79–82% near the Sun is that place's low realisation (its own skew's 25th percentile or
      so), as ruled.
    - No median-light finding. D's median lies above the skew's (+2.4σ by the tail, +3.0σ by the
      half-width), which the one-sided rule does not count; its nearest 50 ly carry it.
  - **Deviations.**
    - Item 1 records E's blocks and, for C, D and E, the light (with its σ, the jackknife and the
      components) and the counts against the single-star tables, beside the paired light the
      task asks for: the science check's should-fix, to place the count findings. The interval is
      asserted in C and D only.
    - The second test-only read, `without_pair_correction`, and `sky/binary_light.rs` and
      `.config/nextest.toml` beyond the task's Files.
    - The record ran its slow-test binary by name (`--ignored --exact`), the record-making run, in
      a capped scope at a 400% CPU quota and not under `just _locked`: the lock's queue held the
      orchestrator's `just ci` and a multi-hour hold, and the lane's rules keep a single CPU-only
      slow test off the lock. `just test-slow` would also rerun `hyperion-fit check --rerun-fast`,
      which nothing here touches.
    - The median test's statistic is the trials' tail share at 20,000 trials (the science check),
      not 3 times the half-width.
  - **Cost** (provisional: unlocked, at a 400% quota, load 4–35):
    - The record: 3,759 s wall and 14,886 CPU-s (user 14,873, system 13), peak 737 MiB (the
      scope's `memory.peak`).
    - Its parts: the tables and C's blocks 50 s; D's blocks 24.8 min (some 5.6 ms a system); E's
      29.7 min (some 12.8 ms a system); the eight observers 6.5 min; the Monte Carlo 53 s.
    - A first record with 4,000 trials and without E and the light, component and count lines
      gave the same figures elsewhere: 1,975 s and 7,849 CPU-s, 681 MiB. A pilot of 8 blocks a
      layer, 2 observers and 400 trials: 151 s and 578 CPU-s, 484 MiB.
  - **Gates**: `cargo fmt --check`; Clippy `-D warnings`, the workspace natively and the sim on
    wasm32-wasip1; `cargo test -p hyperion-sim --lib -- sky::` (192 pass); the slow test by name
    as above. Determinism audit: nothing to fix (no output, golden or fingerprint moves; no bump).
    Science check: no must-fix; its three should-fixes applied (the light's σ, the count σ's
    statement, the paired counts and E to place the findings). Rust review: no must-fix; applied
    (the full read's untested branch, exhaustive matches, an `expect`, a cast's reason); declined:
    the behaviour name, since the plan's acceptance names the test.
  - **Logs**: `.git/rm23-scratch/r06-census/t5f/record2/` (the record's binary, its tree's diff,
    `record.log` and memory samples), `record.log` (the first record) and `pilot.log`.
