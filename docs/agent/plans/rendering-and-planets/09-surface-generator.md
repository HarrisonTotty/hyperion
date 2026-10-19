# Plan R09: The Surface Generator: Coarse Pass, Detail Synthesis, Knowledge Coverage

- **Milestone:** Rendering milestone RM5 (with R10).
- **Depends on:**
  [R03 The scene subscription and bulk transport](03-scene-subscription-and-transport.md),
  [R04 Cross-target determinism and the crate split](04-cross-target-determinism.md),
  [R05 Terrain geometry and the descent spike](05-terrain-geometry-and-descent-spike.md); galaxy
  plans [12](../galaxy-generation/12-retarded-observation-alerts.md) (P12.T7's Knowledge store) and
  [14](../galaxy-generation/14-planetary-systems.md) (body records, rotation, hooks and global
  figures, and the asks below).
- **Brainstorm sections covered** (by heading, in
  [the rendering brainstorm](../../brainstorming/rendering-and-planets.md)):
  [What the generator already owes us](../../brainstorming/rendering-and-planets.md#what-the-generator-already-owes-us);
  [Where the height comes from](../../brainstorming/rendering-and-planets.md#where-the-height-comes-from-the-central-tension);
  [The coarse global pass, once per planet](../../brainstorming/rendering-and-planets.md#the-coarse-global-pass-once-per-planet);
  [Who computes the coarse field](../../brainstorming/rendering-and-planets.md#who-computes-the-coarse-field-and-why-it-is-the-server);
  [The per-query evaluation](../../brainstorming/rendering-and-planets.md#the-per-query-evaluation);
  the band limit and height ownership of
  [The line between truth and decoration](../../brainstorming/rendering-and-planets.md#the-line-between-truth-and-decoration);
  the level bound of
  [Level-of-detail consistency](../../brainstorming/rendering-and-planets.md#level-of-detail-consistency-and-why-collision-agrees);
  [Determinism hazards specific to terrain](../../brainstorming/rendering-and-planets.md#determinism-hazards-specific-to-terrain),
  as applied to the code this plan writes;
  [Knowledge, and the surface seed](../../brainstorming/rendering-and-planets.md#knowledge-and-the-surface-seed);
  the coarse-field paragraph of "Bulk payloads travel as binary frames" under
  [Runtime and code shape](../../brainstorming/rendering-and-planets.md#runtime-and-code-shape); the
  height-function determinism item of
  [Testing](../../brainstorming/rendering-and-planets.md#testing); open questions 4, 5, 9 and 20
  (the last two as asks of plan 14) of
  [Open questions](../../brainstorming/rendering-and-planets.md#open-questions); step 7 of
  [Suggested order of attack](../../brainstorming/rendering-and-planets.md#suggested-order-of-attack).
  From the [single-player brainstorm](../../brainstorming/single-player-experience.md), the mapping
  bullet of "Sensors and exploration" and the `knowledge/` directory of "Sessions".

## Goal

When this plan is done, every body with a solid surface has a coarse field: a pure function of its
surface seed and plan 14's global figures, computed on the server at one cube-sphere level per body,
cached while it is wanted and never stored. The field holds plates rasterised as a power diagram,
its weights fitted so that the plates' areas follow their size law, elevation scaled so that the
reconstructed surface's hypsometric standard deviation σ_h is plan
14's, a seasonal moist energy-balance climate with a labelled precipitation heuristic, stream-power
erosion above a depression-filled base level, climate or surface-state classes, and the list of
craters of the boundary diameter D_b and wider. The field is quantised once, and the quantised field
is the only input to the local synthesis on both sides. The server sends it in surveyed regions,
exact and whole, with the margin the synthesis reads, as a bulk payload on R03's binary frames in
answer to a `surface_field` request entered in plan 04's table. What the ship has surveyed is a log
of survey passes in `knowledge/surveys.v1.jsonl` on P12.T7's store, folded on load into a byte per
cell of best resolution. In `hyperion-surface`, the per-query height function replaces R05's
provisional one: base elevation from a C1 interpolant that agrees bit for bit across cube faces,
structural noise conditioned on crust and plate boundary, a Dendry channel network extended from the
coarse flow directions to the band limit and cached per integer cell, small craters by sparse
convolution with volume-balanced profiles, all band-limited per level with a stated bound, at R05's
2 m band limit and finest level, with an analytic gradient, and seeded only by the detail seed on
its own tag, `body.surface.detail`. Golden height files agree bit for bit across native,
`wasm32-wasip1` and `wasm32-unknown-unknown`. Nothing is drawn from any of it yet: that is R10's.

## Scope and non-goals

In scope:

- The asks of galaxy plan 14 before the pass is written: σ_h by the ruling of open question 20
  (Design note 3) with the continental fraction, the volatile (wet-epoch) history, the crater
  contract, open question 9's climate regime classifier with four corrections, and the signed
  contrasts with the surface section that carries them.
- The surface seeds and streams: `SurfaceSeed`, `DetailSeed`, two tag scopes, the cell key and the
  `surface.*` and `body.surface.detail` tags.
- In `hyperion-surface`: the coarse field's types and its quantisation, the codec of its bulk
  payload, the shared cumulative crater density, the per-query synthesis, the per-patch build with
  its caches, the level bound, and the closed forms R10's readouts and material weights need
  (unresolved variance, structure function, RMS), and golden height files.
- In `hyperion-sim`: the coarse pass, `planetary::surface`, with its inputs from plan 14.
- In `hyperion-server`: the field cache and bulk jobs, the survey log and coverage, the
  `surface_field` and `survey_pass` request kinds, coverage gating, and the scene's
  `surface_revisions`, a revision per body.
- Benchmarks of the coarse pass per level and of the height function per point and per patch.

Non-goals, each with its owner:

- Drawing anything: patches from the field, the material class and its weights, readouts with
  their uncertainty, the survey coverage readout, the collision-agrees test on drawn terrain, the
  wireframe's contours, and the albedo scale that ships in the field's header (R10). This plan
  supplies the function, the inputs and the header field.
- The material class function. The brainstorm lists "the authoritative materials" in step 8, not
  step 7, and R10's row names it, so R10 writes it in `hyperion-surface` from this plan's fields
  (Design note 18).
- Scatter, authoritative rocks, `surface.scatter` placement, and 1–2 m craters (open question 18)
  (R11). This plan's research finds 1–2 m craters typically 0.05–0.1 m deep and leaves them
  decoration (Design note 12).
- The crate split, the wasm checks, the flush-to-zero probe, the terrain hazards in the
  sim-determinism skill, and `BodyHooksDto.detail_seed` with `DetailSeedHex` (R04).
- The binary frame header, chunking at 256 KiB and backpressure (R03); this plan defines only the
  payload.
- The cube-sphere geometry, `PatchKey`, the finest level and the band-limit constants, `num::{min,
max}`, the bake's layout and the worker pool (R05).
- Sensors, and the body-level overlay of which detail level the ship holds for each body (the
  sensors plan that follows sessions). Until then the server grants the level asked, and survey
  passes are recorded by an explicit request (Design note 16).
- A ship's descent surveying the ground ahead of it: it needs flight and sensors; the core this
  plan writes is the one that path will call.
- Biomes as habitats, vegetation and life (the planets brainstorm still owed).

## Provides

Rust paths are under the named crate. Signatures are sketches.

### `hyperion-base` (`rng`, added by this plan beside R04's `ObjectKey::system` and `body`)

```rust
pub enum TagScope { /* R04's variants */, SurfaceCoarse, SurfaceDetail }
pub struct SurfaceSeed(u64);   // server only; opens SurfaceCoarse tags and nothing else
pub struct DetailSeed(u64);    // sent to clients; opens SurfaceDetail tags and nothing else
impl SurfaceSeed { pub fn stream(self, tag: DomainTag, key: ObjectKey) -> Stream;
                   pub const fn new(v: u64) -> Self; }  // called by `hooks::surface_seed` only
impl DetailSeed  { pub fn stream(self, tag: DomainTag, key: ObjectKey) -> Stream;
                   pub const fn get(self) -> u64; pub const fn new(v: u64) -> Self; }
impl ObjectKey {
    /// face 3 bits | level 5 bits | i 28 bits | j 28 bits; sub = instance (Design note 2).
    /// The seed streams check the tag's scope, not the key's (see below).
    pub const fn surface_cell(face: u8, level: u8, i: u32, j: u32, instance: u16)
        -> Result<Self, SurfaceCellKeyError>;
    pub const fn surface_item(n: u64) -> Self;          // plate k, crater slot k, …
}
```

A surface key's own scope is not what guards it: `SurfaceSeed::stream` and `DetailSeed::stream`
check that the tag's scope is their seed's, and `Stream::open` refuses both scopes. Each surface tag
is therefore used with one key form and never both, as `ObjectKey::galaxy()` and `galaxy_item(0)`
are today (the Key column below), and T1.a tests the rule over the registry, because a
`surface_item(n)` word and a low `surface_cell` word can coincide.

As built before this plan (re-validated at `bce2aef5`): P14.T23's `SurfaceSeed` is the sim's
(`planetary/hooks/seed.rs`, with `get` and a 16-hex-digit `Display`), so T1.a moves it into base's
`rng` beside `DetailSeed`, adds `new` and `stream`, and leaves the sim re-exporting it at
`planetary::hooks::SurfaceSeed`, where `hooks::surface_seed` now builds it with `SurfaceSeed::new`;
its value does not move. The seeds' `stream` methods must live in base's `rng`, since
`Stream::from_words` is `pub(super)` and `ObjectKey::new` `pub(crate)`. `Stream::open` asserts only
that the tag's scope is the key's or `SelfTest` (`rng/stream.rs`), so T1.a adds the explicit refusal
of both surface scopes there. Every `ObjectKey` carries a scope: the two surface constructors give
their keys one of the two surface scopes (T1.a records which), which the seed streams do not read.
The keying tables in `rng/key.rs` and `rng/mod.rs` gain the two forms.

Domain tags: `body.surface.detail` in the sim's registry (`crates/hyperion-sim/src/rng/tags.rs`,
appended at the end under a "Rendering plan R09" comment, since the macro's order fixes `ALL`; plan
14's `body.surface` sits under its "Plan 14, phase C and E" comment), and the rest in
`hyperion_surface::tags`, the registry R04 gives the surface crate for `surface.*` names (R04's
Design note 4), which holds R05's `selftest.surface.test_planet` and tests the `surface.` prefix
(`surface_tags_carry_the_surface_prefix`), under a "Plan R09" comment after R05's:

| Tag                      | Scope           | Key            | Draws                                                                  |
| ------------------------ | --------------- | -------------- | ---------------------------------------------------------------------- |
| `body.surface.detail`    | `Body`          | `BodyId`       | the detail seed, one block output keyed by `BodyId`                    |
| `surface.coarse.plates`  | `SurfaceCoarse` | `surface_item` | plate count, seeds, Euler poles, crust type per plate                  |
| `surface.coarse.warp`    | `SurfaceCoarse` | `surface_cell` | the low-frequency boundary warp's lattice                              |
| `surface.coarse.relief`  | `SurfaceCoarse` | `surface_cell` | coarse landform noise, volcanic provinces                              |
| `surface.coarse.crater`  | `SurfaceCoarse` | `surface_item` | craters of D_b and wider: count, place, age, morphology                |
| `surface.coarse.erosion` | `SurfaceCoarse` | `surface_cell` | random receivers and the multigrid's jitter                            |
| `surface.relief`         | `SurfaceDetail` | `surface_item` | structural noise: a 3D lattice corner's word, as R05's `noise` keys it |
| `surface.channel`        | `SurfaceDetail` | `surface_cell` | Dendry key points per cell and level                                   |
| `surface.crater`         | `SurfaceDetail` | `surface_cell` | craters below D_b per octave cell                                      |

`surface.relief` is keyed as R05's noise keys its lattice corners (`noise::Octave`'s corner
gradient: object word i << 32 | j of the corner's 32-bit two's complements, draw number
octave << 32 | k), with `surface_item` in place of R05's `galaxy_item`: a cube-sphere cell key cannot
name a corner of R05's 3D lattice (corrected at re-validation; the plan had `surface_cell`). The
coarse tags' lattices are on the cube's own cells, as the coarse pass's grid is.

`body.surface` (plan 14's) is the surface seed's own tag. `surface.scatter` is named by the
brainstorm and reserved here for R11 (Generator version).

### `hyperion-surface`

Cells are R05's `cube::PatchKey` at the field's level; `test_planet::HeightSample`,
`noise::LatticeCache`, `geometry::{FINEST_SPACING_M, BAND_LIMIT_M, finest_level}`,
`spheroid::Spheroid` and `num::{min, max, assert_finite}` are R05's. R05 defines `HeightSample` in
its provisional `test_planet` module, which R10 retires, so T4 moves it to
`hyperion_surface::height` and leaves `test_planet` re-exporting it; `LatticeCache` already lives in
`noise` (R05's review fixes), which `test_planet` re-exports, and stays there.

```rust
pub mod field {
    pub struct CoarseLevel(u8);                            // 5..=8
    pub fn coarse_level(radius: Metres) -> CoarseLevel;    // Design note 4
    pub fn boundary_diameter(level: CoarseLevel, radius: Metres) -> Metres;  // D_b
    pub fn cell_index(cell: PatchKey) -> u32;              // face-major Morton order at the level
    pub struct FieldHeader { /* format, generator_version, body, radius,
        figure: Spheroid (equatorial a, polar c: the height datum, Design note 17), level, D_b,
        sea_level, lapse_rate, spectrum: BandSpectrum, craters: CraterParams,
        climate_model: ClimateModelKind, precipitation: PrecipitationSource (always Heuristic),
        realised_sigma_h, realised_relief, months: u8 (1 or 12),
        season_eccentricity: f64 (the months' orbit, Design note 8),
        anomaly_step: u8 (0.25 K × 2ⁿ, per body, Design note 17), surface_age: Gigayears,
        surface_pressure: Pascals, albedo_scale: Option<f64> (R10's),
        palette: MaterialPalette (≤ 15 PaletteEntry, T2's follow-up B),
        crust_palette: [Option<u8>; 4], main_liquid: Option<u8>,
        maturity: Option<f64>, organic_cover: Option<f64> (P14.T51.d's, in [0, 1],
        absent until P14.T54.b; follow-up E) */ }
    pub struct PaletteEntry { /* substance, role, normal_albedo_bvr, phase_row,
        grain_density_kg_m3, transition, mechanics,
        liquid: Option<LiquidProperties> (follow-up E: required on a Liquid entry, present on a
        crust entry the body melts, absent elsewhere) */ }
    pub const LIQUID_BINS: usize = 15;                     // R08's bake bins, 380–760 nm
    pub enum LiquidBasis { Measured = 0, Derived = 1, StandIn = 2 }   // a coded byte
    pub struct LiquidProperties { /* temperature: Kelvin, density_kg_m3, basis,
        refractive_index: [f64; LIQUID_BINS], absorption_per_m, scattering_per_m:
        Option<[f64; LIQUID_BINS]>, surface_tension_n_per_m, viscosity_pa_s: Option<f64>
        (decision-r11-palette-liquids §1.1) */ }
    pub struct SynthesisCell { /* elevation_mm: i32,
        boundary_distance: i16 (1 km, saturating),
        plate: u8, crust: Crust, boundary: BoundaryKind, boundary_obliquity: u8,
        flow: FlowDirection, drainage: LogArea (geometric), steepness: LogSteepness (k_s on it),
        runoff: LogRunoff, catchment_runoff: LogRunoff (follow-up C),
        water_surface_mm: i32, ice: u8, substances: u8 (ice and liquid entries),
        class: SurfaceClass, crater_state: u8, seafloor: u8 (hill relief | ponded sediment,
        decision-r09-t5 item 5) */ }                    // 25 bytes in the payload
    pub struct ClimateCell { /* at level − 1: sea_level_temperature: i16 (0.01 K),
        month_anomaly: [i8; 12] (header's step), month_precipitation: [u8; 12] (log rate),
        wind: [Wind; 12] (each month's 10 m wind) */ }    // 50 bytes, one per four cells
    pub struct CoarseCrater { /* centre: [f64; 3] unit, diameter: Metres,
        morphology: Morphology, age: f64 (Gyr), degradation: u8,
        reach: Cover (every cell its reach touches, Design note 10) */ }
    pub struct CoarseField { /* header, synthesis: Vec<SynthesisCell> (cell_index order),
        climate: Vec<ClimateCell>, craters: Vec<CoarseCrater> (sorted),
        reaching: per-cell crater indices, built from each `reach` */ }
    pub trait FieldView { fn header(&self) -> &FieldHeader;
        fn cell(&self, cell: PatchKey) -> Option<&SynthesisCell>;
        fn climate(&self, cell: PatchKey) -> Option<&ClimateCell>;
        fn craters_reaching(&self, cell: PatchKey)
            -> impl Iterator<Item = &CoarseCrater> + '_; }   // in list order
    pub struct MonthBlend { /* from: u8, to: u8, weight of `to`: f64 */ }
    pub fn month_blend(header: &FieldHeader, mean_anomaly: Radians) -> MonthBlend;  // Design note 8
    pub struct PartialField;                   // what a client holds; built with the codec (T3)
    impl PartialField { pub fn new(header: FieldHeader) -> Self;
        pub fn insert(&mut self, block: &wire::DecodedBlock) -> Result<(), InsertBlockError>;
        pub fn is_surveyed(&self, cell: PatchKey) -> bool;   // margin cells are held, not surveyed
        pub fn resolution(&self, cell: PatchKey) -> Option<ResolutionCode>; }
    pub struct Cover;                                      // sorted Morton ranges
    pub struct ResolutionCode(u8);                         // Design note 16
    pub const SYNTHESIS_MARGIN_CELLS: u8 = 5;              // Design note 15
}
pub mod wire {
    pub const SURFACE_PAYLOAD_FORMAT: u16 = 1;
    pub const MAX_BLOCK_BYTES: usize = 1 << 20;            // one self-contained block
    /// `cover` is the surveyed cells with their codes; `since`, the cover a client already holds.
    pub fn encode_payload(field: &CoarseField, cover: &Cover, since: Option<&Cover>) -> Vec<u8>;
    pub fn decode_payload(bytes: &[u8]) -> Result<Vec<DecodedBlock>, DecodePayloadError>;
    pub struct DecodedBlock;
}
pub mod craters {
    pub struct CraterParams { /* n_1km: PerSquareKilometre, screening: Screening (P ÷ g, or a
        crater cutoff), g, k_target, impact_velocity: Option, projectile_density,
        target: CraterTarget, system_age: Gigayears */ }
    pub enum TargetClass { NonPorous, Porous }      // codes 0, 1, appended only
    pub struct CraterTarget { /* class: TargetClass, density: KilogramsPerCubicMetre */ }
    pub const LUNAR_IMPACT_VELOCITY: MetresPerSecond;  // 16.7 km/s, the map's reference
    pub const SOLAR_SYSTEM_AGE: Gigayears;             // 4.567 Gyr, the chronology's clock
    pub fn lunar_chronology(surface_age: Gigayears, system_age: Gigayears) -> PerSquareKilometre;
    pub fn chronology_age(n_1km: PerSquareKilometre, system_age: Gigayears) -> Gigayears;
    pub fn cumulative_density(p: &CraterParams, d: Metres) -> PerSquareKilometre;
    pub fn own_density_at_1km(p: &CraterParams) -> PerSquareKilometre;  // T7.e: the body's own
    pub fn saturation(d: Metres) -> PerSquareKilometre;   // Trask 1966, 0.079 D⁻² km⁻²
    pub fn transition_diameter(p: &CraterParams) -> Metres;  // 19 km × (1.62 ÷ g) × k_target
    pub struct CraterShape { /* morphology, depth, rim_height, floor_radius, peak_relief,
        peak_radius, exterior_width (a) */ }
    pub fn crater_shape(p: &CraterParams, d: Metres) -> CraterShape;  // Design notes 12, 13
    pub fn diameter_in_octave(p: &CraterParams, octave: Octave, u: UnitUniform) -> Metres;
    pub fn profile(r_over_radius: f64, crater: &CraterShape, band: BandLevel) -> (f64, f64);
}
pub mod cold_trap {                      // T7.d (science-r09-drainage.md §8); T13.f and P14.T51.f
    pub struct DepthRatios;              // log-normal d/D by mean and variance; HAYNE_A, HAYNE_B
    pub struct Body;                     // BodyParts: flux, greatest_declination, star_radius,
                                         // bond_albedo, emissivity, heat_flow
    pub struct Terrain;                  // TerrainParts: crater_fraction, crater_depths,
                                         // plains_slope, share_under_cutoff
    pub const LUNAR_SHARE_UNDER_CUTOFF: f64 = 0.05;
    pub fn fraction(latitude: Radians, body: &Body, terrain: &Terrain, threshold: Kelvin) -> f64;
    pub fn permanent_shadow(latitude: Radians, body: &Body, terrain: &Terrain) -> f64;
    pub fn shadow_temperature(depth_ratio: f64, elevation: Radians, body: &Body) -> Kelvin;
}
pub mod tables {                         // T7.f: the fitted tables the crate reads (plan 15)
    pub mod bowl_permanent_shadow;       // a spherical bowl's exact permanent shadow
    pub const MANIFEST: &[hyperion_base::version::TableInfo];
}
pub mod synth {
    pub struct BandLevel(u8);
    pub struct BandSpectrum;   // the structural share's law: V₁, β₁, break degree, β₂
                               // (decision-r09-t5 item 2)
    pub struct Synthesiser<'a, F: FieldView> { /* field, detail seed */ }
    impl<'a, F: FieldView> Synthesiser<'a, F> {
        pub fn new(field: &'a F, seed: DetailSeed) -> Self;
        pub fn height_at(&self, cache: &mut SynthCache, dir: [f64; 3], level: BandLevel)
            -> Result<HeightSample, QueryHeightError>;     // NotSurveyed, NonFinite
        pub fn bake_patch(&self, cache: &mut SynthCache, patch: PatchKey)
            -> Result<PatchHeights, QueryHeightError>;     // heights, parent band, gradients
        pub fn height_range_m(&self, patch: PatchKey) -> (f64, f64);  // for R05's culling
        // The level-keyed closed forms (added by R10.T5; decision-r10-t5-closed-forms.md):
        // what band level `level` leaves out of the finest level's ground in `cell`, each
        // contribution from its own cut, which no one wavelength gives; read by R10's T2, T4
        // and T5. `None` where the view does not hold the cell.
        pub fn omitted_variance(&self, cell: PatchKey, level: BandLevel)
            -> Option<UnresolvedVariance>;                 // per contribution
        pub fn omitted_structure_function(&self, cell: PatchKey, level: BandLevel,
            lag: Metres) -> Option<SquareMetres>;
        pub fn omitted_rms(&self, cell: PatchKey, level: BandLevel) -> Option<Metres>;
    }
    pub struct SynthCache;                                 // caller-owned, u64 keys, with R05's
                                                           // LatticeCache inside
    pub fn level_bound_m(header: &FieldHeader, level: BandLevel) -> f64;   // hard for the
        // structural and channel terms; the crater terms a 1 − 10⁻⁹ per-point quantile (T8)
    pub fn local_variance(spectrum: &BandSpectrum, level: CoarseLevel) -> SquareMetres;
    // Per-cell closed forms: they read the cell's crust, boundary, k_s, drainage and crater
    // state through the view, so `None` where the view does not hold the cell.
    pub fn unresolved_variance(field: &impl FieldView, cell: PatchKey, finer_than: Metres)
        -> Option<UnresolvedVariance>;                     // per contribution, in wavelengths
    pub fn structure_function(field: &impl FieldView, cell: PatchKey, finer_than: Metres,
        lag: Metres) -> Option<SquareMetres>;              // shorter than λ; R10 reads the
    pub fn unresolved_rms(field: &impl FieldView, cell: PatchKey, resolution: Metres)
        -> Option<Metres>;                                 // level-keyed forms above
}
pub mod testing {                                          // feature `testing`
    pub enum SyntheticWorld { EarthLike, MarsLike, MoonLike, CeresLike, Flat, OneCrater }
    pub fn synthetic_field(kind: SyntheticWorld) -> CoarseField;
    pub struct FieldBuilder;
}
```

`SurfaceClass` is one `u8`: the Köppen–Geiger classes for seasonal water-cycle regimes and the
surface-state classes of the other regimes (Design note 11).

As built before this plan: base's `units` has `Metres`, `Pascals`, `Kelvin`, `Gigayears` (not
`Gyr`) and `MetresPerSecondSquared`, but no `PerSquareKilometre` or `SquareMetres`; T2 adds the two
with base's `unit!` macro, new types only. `FieldHeader` holds `BandSpectrum` and `CraterParams`, so
T2 defines the header's data types (`synth::{BandLevel, BandSpectrum}`, `craters::CraterParams`,
`ClimateModelKind`, `PrecipitationSource`) and T5 and T7.a give them their behaviour.
`QueryHeightError` implements `std::error::Error + Send + Sync`, the bound R05's
`patch::HeightSource::Error` sets, since R10.T3 implements `HeightSource` for `Synthesiser`. The
surface crate has no `[features]` yet: T2 adds `testing`, which the sim's own `testing` feature
enables.

### `hyperion-sim` (`planetary::surface`, and plan 14's hooks)

```rust
// in `planetary::hooks`, beside P14.T23's seed (hooks/seed.rs, built; re-exported by hooks/mod.rs):
pub fn surface_seed(seed: Seed, body: BodyId) -> SurfaceSeed;  // P14.T23's, on `body.surface`
pub fn detail_seed(seed: Seed, body: BodyId) -> DetailSeed;    // R09's, on `body.surface.detail`
// in `planetary::surface`:
pub struct CoarseInputs { /* Design note 3 */ }
impl CoarseInputs {
    pub fn for_body(record: &BodyRecord, planet: Option<&BodyRecord>, ctx: &SystemContext)
        -> Result<Self, ReadCoarseInputsError>;                   // R09.T10.f
    pub fn planet_of(record: &BodyRecord) -> Option<BodyIndex>;   // `Some` for a moon
    pub fn builder() -> CoarseInputsBuilder;
}
pub const RECORD_TIME: UniverseTime;   // the epoch: the time `for_body`'s records are taken at
pub enum SurfaceInputsError { NoSolidSurface, NotModelled(RecordSection), .. }
pub enum ReadCoarseInputsError { Section(SurfaceInputsError), PlanetMissing(BodyId),
    NotItsPlanet { .. }, NotAtRecordTime, SeasonalOrbit(..), Build(..) }
pub fn coarse_pass(seed: SurfaceSeed, inputs: &CoarseInputs) -> CoarseField;
pub mod steps { plates, relief, craters, climate, erosion, classes }   // each pub for tests
pub mod reference { earth_like, mars_like, moon_like, ceres_like }     // feature `testing`
```

`for_body` reads what the record now carries: `bulk()` (radius, gravity, class, fractions),
`figure()` (P14.T46's `BodyFigure`, whose `spheroid()` gives a, c and f), `rotation()` (P14.T14's
`BodyFixedFrame`: pole, obliquity and `RotationLaw`), `orbit()`, and the context's stars for the host
flux over the orbit (P14.T12's `Illumination` gives only its orbit average). The record's `surface`
section is uninhabited (`record::Surface` is an empty enum), and the atmosphere (`SurfaceState`,
`SurfaceMaterial`, pressure and gases, in `derive::atmosphere`) is reachable only through the
private `DerivedBody`, so `for_body` answers `Section(NotModelled(RecordSection::Surface))` until
plan 14's surface section is inhabited (P14.T24 and the asks drafted as P14.T48). As built by
R09.T10.f (Risks, "Deviations in T10.f, as built"; decision-backlog-2 §3), `for_body` takes a moon's
planet's record beside its own, both taken at `RECORD_TIME`, and checks the surface section's tag,
then the planet, then the crust; a planet not given, or not the moon's, is `PlanetMissing` or
`NotItsPlanet`, which a server answers `internal`.

### Server (`hyperion_server`)

```rust
pub mod surface { SurfaceService, FieldKey, FieldCache, SurfaceJobError, InputsSource,
                  RecordInputs /* production: CoarseInputs::for_body */ }
pub mod knowledge::surveys { SurveyPass, SurveySource, SurveyLog, Coverage, LoadSurveysError }
pub const MAX_SURVEY_RANGES: usize = 256;   // `limits.rs`: ranges a `survey_pass` may carry
```

`InputsSource` is the seam between the service and plan 14: its one method,
`inputs(record, planet, ctx)`, takes `for_body`'s three arguments and answers
`Result<CoarseInputs, ReadCoarseInputsError>`, the service fetching a moon's planet's record at
`RECORD_TIME` beside the body's (R09.T10.f). Production uses `RecordInputs`,
which calls `CoarseInputs::for_body`; the server's tests use one built from
`reference::{earth_like, …}` (feature `testing`), so that the service and the wire are tested
before plan 14's asks land.

Environment: `HYPERION_SURFACE_CACHE_MB` (default 128), with `--surface-cache`, declared in
`config.rs` as the sky's cache is (`ENV_SKY_CACHE_MB`, `DEFAULT_SKY_CACHE_MIB`, the private
`CacheBudget` argument, `ServerConfig::sky_cache_bytes` and its builder). `knowledge::surveys` sits
beside P12.T7's `knowledge::{KnowledgeStore, PersistedKnowledge}` (built), and the service is an
`AppState` field, as the sky's services are.

### Protocol (`hyperion-protocol`, mirrored in `@hyperion/protocol`)

- `surface_field`: `SurfaceFieldRequest { universe, body: BodyIdHex, have_revision: Option<u32> }`,
  answered in bulk (R03's Design note 10): the payload's binary frames, then the enum
  `SurfaceFieldDto`, either `Ready { header: SurfaceHeaderDto, revision: u32, coverage: CoverDto,
bulk: BulkManifestDto }` or, with no bulk, `NotModelled { section }` for a body whose plan 14
  sections the pass needs are not yet modelled (the record's own `not_modelled` convention). An
  unknown body is `unknown_body` and a body with no solid surface `bad_request`, each naming
  `body`; a malformed index (`bad_request`) and a system that does not resolve (`unknown_system`)
  name `body` too, through `convert::planetary::body_refusal`, as `body_detail` refuses them.
- `survey_pass`: `SurveyPassRequest { universe, body, from, to, source: SurveySourceDto,
resolution_m, cells: Vec<CellRangeDto> }` answered by `SurveyPassDto { revision }`, where
  `CellRangeDto` is `{ start: u32, end: u32 }` and `cells` holds at most `MAX_SURVEY_RANGES` (256)
  ranges, so that a request stays under plan 04's 16 KiB `MAX_INBOUND_FRAME_BYTES`; a longer pass is
  sent as several requests with one time span. More ranges is `bad_request` naming `cells`.
- `SurfaceHeaderDto`'s palette entries, each a `PaletteEntryDto` of `substance`, `role`,
  `normal_albedo_bvr`, `phase_row` and `liquid: Option<LiquidDto>`, and its
  `main_liquid: Option<u8>`, `maturity` and `organic_cover` (`Modelled<f64>`, absent until plan
  14 fills them), all additive JSON (T2's follow-up E). `LiquidDto` mirrors `LiquidProperties` (`temperature_k`, `density_kg_m3`, `basis`
  as `LiquidBasisDto`, `refractive_index`, and the four optional members as `Modelled`, absent
  when not modelled), its by-bin members `SURFACE_LIQUID_BINS` (15) long.
- `CoverDto`: `ranges: Vec<CoverRangeDto>`, each `{ start: u32, end: u32, resolution: u8 }`, a
  half-open range of `cell_index` values with its `ResolutionCode`, so that R10's readouts know the
  resolution per cell. It travels server to client only.
- On R03's scene topic, the optional field `surface_revisions: Option<Vec<SurfaceRevisionDto>>` of
  `{ body: BodyIdHex, revision: u32 }` on `SceneNotificationDto`, which R03 leaves room for (its
  Design note 4, where T0.a drafted the field): `#[serde(default, skip_serializing_if =
"Option::is_none")]` and `#[ts(optional)]`, as `tidal_radius_m` is, so `PROTOCOL_VERSION` stays 2;
  a merged pending push (R03's Design note 5, `subscriptions::ScenePush::merge`) merges it by body,
  the latest revision winning.

### Test helpers

`hyperion_surface::testing::{synthetic_field, SyntheticWorld, FieldBuilder}`,
`hyperion_sim::planetary::surface::reference::*`,
`crates/hyperion-server/tests/common/surface.rs::{field_via_wire, decode_all, reference_inputs}`
over R03's `TestClient::next_binary() -> (BinaryHeader, Vec<u8>)` (the server's dev-dependencies
gain `hyperion-sim`'s `testing` feature for `reference_inputs`, T17), and
`crates/hyperion-sim/examples/surface_map.rs` (the sim's first example: T15 creates the directory and
an `[[example]]` entry with `required-features = ["testing"]`), which writes a field's elevation,
class and flow as equirectangular PPM images under `target/` for the check by eye. R10's
reference-world tests live beside this plan's `surface_coarse_golden.rs` in
`crates/hyperion-sim/tests/` (`surface_material_worlds`, `surface_weights_worlds`,
`surface_ground_worlds`, `level_bound_worlds`, `surface_reading_worlds`, `surface_albedo_worlds`),
over a shared `tests/common/surface_worlds.rs` that runs `reference::*` through the pass once per
binary; T16 creates that file with its own goldens so that R10 extends it.

## Consumes

Names are as built, reconciled with the code at `bce2aef5` by R09.T0.a; where a dependency's
Provides sketch differs, the as-built name is given.

- **R04.** `hyperion-base` with `math` (`atan2`, `j0`, `mul_add` on the pinned libm), `rng`,
  `units`, `version` (`GENERATOR_VERSION` 21) and `ObjectKey::{system, body}`; `domain_tags!`
  exported from base with `DomainTag::registered` `#[doc(hidden)] pub`, and the three registries
  with `assert_registries_disjoint`, called once in the sim's `rng/tags.rs`; the `hyperion-surface`
  crate with `hyperion_surface::tags` (holding R05's `selftest.surface.test_planet`), its
  self-contained `clippy.toml`, its relaxed-SIMD `compile_error!` and `generator_version()`;
  `just test-wasm-fast` (both wasm targets, in `just ci`) and `just test-wasm-slow`; the testkit's
  embedded `golden!` arm, which `include_str!`s `tests/golden/<name>.golden` on the browser target,
  so every golden name is a literal, and `golden::check_embedded`; `compute::probe_flush_to_zero`,
  `CpuPool::with_probe` and `JobError::FloatingPointMode`; `BodyHooksDto.detail_seed:
SectionDto<DetailSeedHex>` and the client's `BodyHooks.detailSeed`. As built, the whole hooks
  section is `not_modelled` (or `not_applicable`): the sim's `record::Hooks` is an empty enum and the
  server's `convert::planetary::hooks` an empty match, so T1.b inhabits it. R04's test conventions
  for the surface crate and base (R04 Design note 12): every test module carries the
  `wasm_bindgen_test as test` import (behind the browser target's `cfg`, as the crates' modules have
  it); a test that cannot run on `wasm32-unknown-unknown` sits in a module named `native_only`; and
  every `should_panic` test states `expected` and lives in the crate's own `tests/panics.rs` binary
  (both crates have one).
- **R05.** `hyperion_surface::cube` (`Face` with `TryFrom<u8>`, `FaceUv`, `st_to_uv`, `uv_to_st`,
  `face_uv_to_xyz`, `xyz_to_face_uv` with its canonical-face rule (ties to the lowest face index),
  `unit_dir`, `PatchKey` (private fields; `new`, the getters `face`, `level`, `i` and `j`,
  `to_u64`, `parent`, `children`) with `edge_neighbour`, `edge_neighbour_and_back` and
  `corner_neighbours`, `vertex_dir`, `sample_dir` with `SampleGrid`, `Edge`, `MAX_LEVEL` 24);
  `hyperion_surface::geometry`'s `FINEST_SPACING_M` (0.5 m), `MAX_FINEST_SPACING_M` (0.375 m),
  `BAND_LIMIT_M` (2 m) and `finest_level` (Earth → 19); `test_planet::HeightSample`;
  `noise::LatticeCache` (re-exported by `test_planet`); `hyperion_surface::num`'s `min`, `max` (both
  refusing NaN) and `assert_finite`; and `hyperion_surface::noise`'s
  `gradient_noise(p_m, &Octave, &mut LatticeCache)`, `Octave::new`, `NOISE_BOUND` (B = 1.0681) and
  `NOISE_RMS` (σ_noise = 0.2701), 3D improved Perlin noise with Perlin 2002's 16-entry gradient table
  (R05's Design notes 12 and 15). As built, an `Octave` holds a `Seed` and opens its corners on
  R05's `selftest.surface.test_planet` tag alone, and `LatticeCache` keys its octave table and
  corner boxes by that `Seed`, so T5 generalises both. `spheroid::Spheroid` (`from_volumetric`,
  `flattening`, `normal`, `surface_point`), R07's datum; the `patch` module's `PatchBake` layout
  (`heights` 65 × 65 × 2, own level then morph target), `bake_patch` and `HeightSource` (with
  `prepare_cache` and `Error: std::error::Error + Send + Sync + 'static`), and
  `patch::collision::finest_surface_height`, which R10 moves onto this plan's `Synthesiser`.
  `TEST_PLANET_VERSION` (2, at the crate root) heads every surface golden today, held by
  `cube_golden.rs`'s `native_only::every_golden_file_carries_the_test_planet_version` (a flat
  `read_dir` of `tests/golden/`) and by `golden_diff.py`'s `TEST_PLANET_PREFIX`, which matches the
  whole directory; T9 narrows both. The surface crate's one bench is `benches/test_planet.rs`
  (Criterion, off the browser target). The client's workers, `HeightWorkerPool`'s per-body calls
  (`fieldInit`, `fieldChunk`, `fieldRelease`; R10.T7) and R04's `wasm/` loader are R10's to use, not
  this plan's; the wasm module's exports serve the test planet only, and the export that copies a
  field into the module, which R05's T10.b record assigns to this plan, is R10's, with its client
  (R10 Design note 16).
- **R03.** The server's `pub(crate) mod bulk`: `Answer { body: ResponseBody, bulk:
Option<BulkPayload> }`, `BulkPayload::new(Bytes) -> Result<_, BuildBulkPayloadError>` and
  `manifest()`; `BulkManifestDto { chunks, bytes }`; `MAX_BINARY_FRAME_BYTES` (262,144) and
  `BULK_QUEUED_BYTES` in `limits.rs`; chunks before the terminal response, `cancel` stopping them;
  R06's `requests/sky.rs::answer` as the model of a bulk handler (its jobs through
  `compute::sky::bulk` at `Priority::Bulk`, its cache and `SingleFlight` inside services that
  `AppState` holds); the scene topic's `SceneNotificationDto`, which has no `surface_revisions` yet
  (T19.b adds it); `TestClient::next_binary()`; the Knowledge-bound scene test,
  `tests/scene_knowledge.rs`, whose restrictive `SceneKnowledge` is given through
  `ServerConfigBuilder::scene_knowledge`, and which T19 extends to surveyed cells; the client's
  `requestBulk(body, manifestOf, options)`, whose `manifestOf` returns `null` for a response with no
  bulk, such as `NotModelled`, and its limits `MAX_BULK_CHUNKS` (257) and `MAX_BULK_PAYLOAD_BYTES`
  (64 MiB), above this plan's 15 MB. R03's camera reports are not used to bound the field (Design
  note 19), as R03 already records (its Design note 6 and Risks).
- **Galaxy plan 04.** The reserved-kinds table in
  [its "Extending the convention"](../galaxy-generation/04-server-and-protocol.md#extending-the-convention),
  which new kinds enter first; `RequestBody`, `ResponseBody`, `REQUEST_KINDS` and the
  compile-forced `next_request` and `next_response` walk in `crates/hyperion-protocol/src/envelope.rs`;
  `kind` (`pub(crate)`), `is_large` (`pub(crate)`, an exhaustive match), the `Handlers` match and the
  test helper `every_body` in `crates/hyperion-server/src/requests/mod.rs`; `MAX_INBOUND_FRAME_BYTES`
  (16 KiB) in `crates/hyperion-server/src/limits.rs`, where `every_limit_is_the_plans` pins every
  limit; `compute::{CpuPool, Priority::Bulk, CancelToken, SingleFlight, GalaxyKey}`;
  `cache::{ByteLru, SharedByteLru, HeapBytes}`; the `knowledge/` directory reserved in
  `crates/hyperion-server/src/universe/store.rs`, with `UniverseStore::knowledge_dir(id)`;
  `ErrorCode::{BadRequest, UnknownSystem, UnknownBody, Internal}`; `TestServer`, `TestClient`.
- **Galaxy plan 12.** P12.T7, built (T7.a–b): `knowledge::{KnowledgeStore, PersistedKnowledge,
LoadKnowledgeError, SaveKnowledgeError, KNOWLEDGE_FORMAT}` in `knowledge/{mod,persist,record,
store}.rs`; `PersistedKnowledge::open(&UniverseStore, UniverseId)` over
  `knowledge/contacts.v1.jsonl`, a `{"format":1}` header and then one change a line, appended and
  synced before it is applied, every file operation under `spawn_blocking`, a torn last line
  dropped with a warning, and `LoadKnowledgeError::{Io, UnsupportedFormat, MalformedLine,
Interrupted}`. As built, nothing holds a `PersistedKnowledge` in `AppState` (P12.T8 will), and its
  log helpers (`KnowledgeLog`, `push_line`, `sync_directory`, and `refuse_later_formats`, which
  names `contacts.v`) are private to `persist.rs`.
- **R07.** Design note 19's single height datum, the rotational spheroid, and the flattening f that
  R07.T1 asked of plan 14, now built (P14.T46): the record's `figure()` section holds a
  `BodyFigure` whose `spheroid()` is R05's `Spheroid`, so the datum is the spheroid from the start.
- **Galaxy plan 14.** `BodyRecord` (its sections `bulk`, `figure`, `rotation`, `photometry`,
  `surface` and `hooks` among them) and `Section`, `RecordSection` (twelve sections; thirteen once
  P14.T48.e's gas-envelope split adds `Envelope`),
  `SystemContext`, the `body.surface` tag and P14.T23's `hooks::{surface_seed, SurfaceSeed,
BodyHooks, BulkComposition}` with `PlanetarySystem::{surface_seed, hooks_at}`; P14.T13.c's
  `SurfaceState` (`GasEnvelope`, `MagmaOcean`, `Airless`, `RunawayGreenhouse`, `Temperate`,
  `Snowball`) and `SurfaceMaterial` (`Rock`, `Ice`), surface pressure and gases, in
  `derive::atmosphere`; P14.T14's rotation, `frames::{BodyFixedFrame, body_fixed_at}` returning
  `FrameRotation`, and `PlanetarySystem::rotation_of`, for seasons and a locked world's substellar
  axis; P14.T24.a's `SurfaceConditions` and P14.T24.b's `GlobalFigures` (ocean and ice fractions,
  tectonic regime, volcanism, heat flow, surface age, crater density); and the asks of Design note
  3, drafted in plan 14 as P14.T48. As built: P14.T14.a–c, T23, T35, T46 and T47 are built; T24 is
  not (`hooks/` holds `mod.rs` and `seed.rs` alone, and nothing computes heat flow, a tectonic
  regime, a surface age, a crater density or an ocean fraction, but for regular moons' tidal heat
  and volcanism); the record's `Surface` and the wire's `BodySurfaceDto` are empty enums; and no
  public method gives a body's atmosphere at a time, which only the private `DerivedBody` holds.
- **Plan 01's discipline**, through R04's base crate: `Stream::open`, `domain_tags!`, the tag
  golden `crates/hyperion-sim/tests/golden/rng/tags.golden` and `domain_tags_are_pinned`
  (`crates/hyperion-sim/tests/foundation_golden.rs`, which prints base's, the surface crate's and
  the sim's registries), `hyperion-testkit`'s `golden!` and `GoldenWriter`
  (`crates/hyperion-testkit/src/golden.rs`) with `f64_digest` and `f32_digest`,
  `order::assert_order_independent`, slow tests marked `#[ignore = "slow: …"]`.

## Design notes

Research of 2026-09-29 settled most of what the first draft left to leans; each note so settled
says "researched 2026-09-29" and names its sources. Every timing the research took was measured on
the machine then used for development under other agents' load (load averages 14–20 on eight
threads, the UHD 620 laptop's count; the development machine now has sixteen), so
every timing below is provisional and is re-measured on a quiet machine by the task that owns it.

1. **Where each piece lives.** The field's types, its quantisation, the payload codec, the crater
   density and the whole local synthesis are in `hyperion-surface`, because the client's workers run
   them and the crate must not depend on the sim. The coarse pass, which only the server runs, is
   `hyperion_sim::planetary::surface`, beside plan 14's stage, and hands its field to the surface
   crate as data, as the brainstorm's
   [Runtime and code shape](../../brainstorming/rendering-and-planets.md#runtime-and-code-shape)
   says. The dependency runs sim → surface → base. Nothing in the surface crate reads a clock,
   spawns a thread or holds a cache of its own: `SynthCache` is the caller's. The synthesis builds
   on R05's code (`cube`, `HeightSample`, `LatticeCache`, `num`, the noise basis) rather than
   duplicating it, and leaves R05's `test_planet` in place for R10 to retire.
2. **Seeds, scopes and cell keys.** The surface seed is one block output of (universe seed,
   `body.surface`, `BodyId`) and the detail seed one of (universe seed, `body.surface.detail`,
   `BodyId`), so they share nothing but the universe seed. Each is a newtype, and each opens only
   its own scope: a `SurfaceCoarse` tag opens from a `SurfaceSeed` alone, a `SurfaceDetail` tag
   from a `DetailSeed` alone, and `Stream::open` refuses both scopes, so no path through the seed
   types seeds the synthesis from the universe seed or the coarse pass from the detail seed. The
   public `new` constructors exist for the wire and for `hooks::surface_seed`; they are a discipline
   for an honest client, as the brainstorm says, not a boundary. The seed takes the
   place of the universe seed as key word 0; the body is therefore already in the key, and the
   cell key packs face (3 bits), level (5 bits) and i and j (28 bits each) into counter word 0,
   with `sub` as an instance number (a Dendry level's slot, a crater or rock's index, R11's shape
   draws). 28 bits reach level 28, beyond R05's `MAX_LEVEL` of 24. The brainstorm's "a new
   `ObjectKey` constructor of body, face, level and cell" is met with the body carried by the seed,
   since a `BodyId` alone fills counter word 0 and `sub`. The `surface.*` tags live in the surface
   crate's registry and `body.surface.detail` beside plan 14's `body.surface` in the sim's, both as
   R04's Design note 4 reserves.
3. **The contract with plan 14** (researched 2026-09-29: σ_h computed with pyshtools from Earth2014,
   MOLA, VenusTopo719, LOLA, MESSENGER, Dawn and Cassini shape and gravity models; the classifier's
   thresholds read from Koll 2022, Wordsworth 2015, Yang et al. 2014, Kilic et al. 2018 and Barnes
   et al. 2025). `CoarseInputs` is the whole of what the pass reads: radius and flattening, gravity,
   σ_h, ocean and ice fractions, mean surface temperature with signed equator–pole and day–night
   contrasts, obliquity, rotation and the body-fixed frame, the host flux over the orbit, surface
   pressure and gases, `SurfaceState`, `SurfaceMaterial`, tectonic regime and continental fraction,
   volcanism level, heat flow, surface age, `CraterParams`, an optional `WetEpoch`, and the
   `ClimateRegime`. Five asks go to plan 14 (R09.T0.a drafted them there as P14.T48.a–e, its
   "Phase K", for its owner), because plan 14 as written does not provide them (its P14.T24.a
   contrasts are unsigned, it has no continental fraction, and it has no regime classifier at all):
   - **σ_h, by open question 20's ruling, with the continental fraction f_c it needs.** σ_h is not a
     function of gravity: over seven bodies it scales as g^−0.22 with a scatter of 2.5×, and at
     equal gravity it differs 2.7× (Earth 2.51 km against Venus 0.94) and 2.8× (Mars 2.90 against
     Mercury 1.05). Plan 14 publishes σ_h, relative to the reference equipotential with degree 1
     included, as σ_h² = σ_struct² + σ_crat² + σ_volc²: a structural share with no gravity term
     (mobile lid f_c(1 − f_c) Δ² + f_c σ_c² + (1 − f_c) σ_o² with Δ = 4.69 km, σ_c = 1.08 km, σ_o =
     0.93 km from Earth2014's two populations, which give 2.51 km at f_c = 0.405; stagnant lid 0.9
     km, Venus's); a basin share ∝ 1/g, 2.05 km × (1.62 ÷ g) × k_comp × √min(1, N(>1 km) ÷ 0.018
     km⁻²), k_comp 1 for rock and 0.16 for ice-rich crusts, capped at 0.02 R; and a constructional
     share ∝ 1/g gated by the lithosphere, 2.76 km × (3.71 ÷ g) × V × min(1, T_e ÷ 70 km), with T_e
     ≈ k (870 K − T_s) ÷ F from heat flow and k = 3.1 W m⁻¹ K⁻¹, the mantle value of the plate model
     (Parsons and Sclater 1977; Stein and Stein 1992; researched 2026-09-29, medium confidence). 870
     K (about 600 °C) is kept explicitly as a mechanical-thickness proxy, since the 70 km
     normalisation was calibrated with it; oceanic T_e follows the 450 ± 150 °C isotherm (Watts
     2001; Watts and Burov 2003), and 723 K was weighed against it (ruled below). The
     lithosphere factor acts on that share and the edifice cap only. Every body but one calibrates a
     constant: Earth the mobile-lid terms, Venus the stagnant-lid 0.9 km (0.94 km observed, with
     almost no basin or constructional share, so its −4% is the rounding and not a test), the Moon
     the basin share, Mars the constructional share, Ceres k_comp and Vesta the 0.02 R cap. Mercury
     is the only out-of-sample check, and the fit over-predicts it by 21% (1.27 against 1.05 km), so
     the fit is labelled empirical and tested once (researched 2026-09-29, high confidence). f_c is
     the continental fraction, or failing one the land-plus-shelf area (0.405 on Earth; its law for
     generated worlds is below). Greatest relief, 20 km × (g⊕ ÷ g), is withdrawn as a published
     figure (it is Johnson and McGetchin 1973's 1/g envelope, 53 km for Mars against 29.4
     observed); the field reports the realised relief, which lies at 7–15 σ_h on every body
     measured.

     Three checks of the fit (researched 2026-10-09, by R09.T0.b). _The age law_
     √min(1, N(>1 km) ÷ 0.018 km⁻²) is kept as a global factor, read at plan 14's surface age (the
     time since the last global resurfacing) and never weighted over a surface's geological units.
     The same pyshtools computation, masked, finds no age signal in Mercury's units: Maia 2024's
     stereo shape model to degree 719 (Zenodo, doi:10.5281/zenodo.10809345) over Giuri, van der
     Bogert and Hiesinger 2025's global smooth-plains map (Icarus 441, 116699; data
     doi:10.5281/zenodo.14913540; 19.3% of the surface, 23.9% with the smooth crater floors), plains
     dated at about 3.7 Ga (Wang et al. 2021's 262 dated sites, area-weighted 3.71 Ga; data
     doi:10.5281/zenodo.5155070), gives them band-passed relief at 0.95–0.98 of the older
     intercrater plains' and cratered terrain's at every band from above degree 5 to above degree
     160, where the law predicts 0.79 for the basin share (N(>1 km) = 0.011 km⁻² at 3.71 Ga by the
     lunar chronology, saturated from 3.8 Ga) and 0.90 for the whole. Mars's partly resurfaced
     northern lowlands (north of 50° N) err the other way, at 0.23–0.34 of the southern highlands'
     relief from above degree 20 to above degree 320 (MOLA's shape to degree 719, the same
     script), against the law's 0.6 at 3.6 Ga. Partial resurfacing thus follows no law in N, so the
     factor stays uncalibrated between saturation and the young surfaces where the basin share is
     negligible (no Solar System body lies between), and Mercury's over-prediction is not its
     smooth plains: weighting its two units by the law would lower 1.27 km by 3% only. Mercury's
     1.05 km is confirmed as 1.048 km relative to the geoid (JGMESS160A's degrees 2–100, first
     order), against 1.090 km for the shape alone and 0.98 km on GTMES_150 (degree 150). T10's
     reference worlds may take σ_h from the fit, which gives each of their analogues within 5%.
     _The isotherm of T_e_ is 870 K (ruled): it is the mechanical lithosphere's isotherm, "near
     550–600 °C, depending on strain rate [McNutt, 1984]", the conversion McGovern et al. 2002 use
     for Mars (JGR 107(E12), 5136, §4.2, ¶68), so the 70 km normalisation and T_e = k (870 K −
     T_s) ÷ F are a mechanical-thickness proxy by design. At Venus's ambient heat flow of 10–30
     mW m⁻² (Maia and Wieczorek 2022, JGR Planets 127, e2021JE007004, after Solomatov and Moresi
     1996, Phillips et al. 1997 and O'Rourke and Korenaga 2015) and T_s = 737 K it gives 14–41 km,
     against a global mean of 29 ± 6 km from the gravity of volcanic structures (Barnett, Nimmo and
     McKenzie 2002, JGR 107(E2), 5007) and 15 km under the crustal plateaus (Maia and Wieczorek);
     the elastic 723 K lies below Venus's surface temperature and gives none. On Mars (14–25 mW m⁻²,
     mean 19, Parro et al. 2017, Sci. Rep. 7, 45629; T_s about 210 K) it gives 108 km and 723 K
     84 km, both above 70 km, matching Olympus Mons's > 70 km (McGovern et al. 2004's correction)
     and 93 ± 40 km (Belleguic, Lognonné and Wieczorek 2005, JGR 110, E11005), so 723 K would need
     no re-fit there unless F exceeded 22.7 mW m⁻². Where T_s ≥ 870 K, T_e is zero: no
     constructional share, and Airy compensation (α = 0) about T12.c's loads. An ice-rich crust is
     not described by a silicate isotherm (Risks). _The continental fraction_ (medium-low
     confidence): continental growth on an Earth-sized plate-tectonic planet depends on its history.
     In Höning and Spohn 2023 (Astrobiology 23(4), 372–394, arXiv:2211.09473; after Höning, Tosi,
     Hansen-Goos and Spohn 2019, PEPI 287, 37–50), the positive feedback between continental crust
     and mantle water leaves, after 4.5 Gyr, an ocean planet with about 20% continental crust, an
     Earth-like one with 40% or a land planet with about 70%, from initial mantle temperatures 150
     K apart (their Figs. 4–5 and §5). Plan 14 therefore draws an asymptote f_∞, uniform in
     [0.2, 0.7], once per body on a stream of its own, and grows f_c = f_∞ (1 − e^(−(t − t_p) ÷ τ))
     after t_p = 0.5 Gyr from formation (their onset of continental growth), zero before, with τ =
     1.1 Gyr, fitted by eye to their Fig. 5's Earth-like curve (0.15, 0.25, 0.31 and 0.37 of
     coverage at 1, 1.5, 2 and 3 Gyr; 0.405 at 4.5 Gyr for f_∞ = 0.416). f_c is zero on a stagnant
     lid and on a world that never held surface water, since their continents grow only by the
     melting of hydrated crust at subduction zones. No mass dependence is modelled, since they treat
     Earth-sized planets alone, and the Solar System table gives Earth its measured 0.405. The
     structural share then spans 2.11–2.55 km over the draw's range.

   - **The volatile history**, `WetEpoch { start, end, effective_flow, runoff, paleo_inventory }`,
     with `effective_flow` in Earth-equivalent years (the time channel-forming flow ran at a point
     over the epoch, divided by Earth's reference flood intermittency of 5%), which is the solver's
     t, and `runoff` the epoch's runoff at that intermittency, which weights the drainage area on a
     world dry now (Design note 9; corrected 2026-10-09 by T0.b from "years at effective
     discharge", and 2026-10-10 by science-hoke.md).
   - **The crater contract**: the lunar-equivalent N☾(>1 km) with its belt scaling (Design note
     12), the screening input (surface pressure over gravity, or a crater cutoff), the
     projectiles' density, g, a target factor k_target (1 rock, 0.12 ice-rich), the target's
     scaling class (non-porous or porous) and density, the system's age, and a mean impact
     velocity (optional for builder inputs; plan 14 always gives it, by P14.T48.f); the surface
     crate derives the rest (Design note 12).
   - **The climate regime classifier** of open question 9, which plan 14 does not yet have: its
     three fields (a thermal regime, a forcing from the locking state, the solar day and the
     obliquity, and a condensable) and the regime's named coarse model, with four corrections:
     Koll's redistribution factor f = 2/3 − (5/12) X ÷ (k + X) at k = 2, with airless-like below X <
     0.087 k and efficient redistribution above 15.7 k, for locked and slow rotators only;
     Wordsworth's CO₂ collapse pressure (his eq. 43) for pure CO₂ between 1 and 10 M⊕ only; a
     slow-rotator onset that rises with flux, not one number: Yang et al.'s Table 1 brackets it
     between sidereal rotation periods of 8 and 16 d at 1.40 S⊕ and of 32 and 48 d at 1.92 S⊕, at
     an orbital period of 225 d (sidereal periods, not solar days; T10's science review,
     2026-10-09); and an equatorial ice belt at 54–126° obliquity (53.9° recomputed) decided with
     the volatile history, since Kilic et al.'s belt is reached only from a colder state.
   - **Signed contrasts and the surface section.** P14.T24.a's equator–pole contrast with its sign
     (a warm pole is negative) and, for a locked world, the day–night contrast about P14.T14's
     substellar axis, as open question 9 asks; and the surface section carrying `SurfaceState`,
     `SurfaceMaterial`, P14.T24's conditions and figures and the continental fraction, so that
     `for_body` has one section to read.

   Until an ask lands the pass takes the value from `CoarseInputsBuilder` as a plain argument,
   which the reference worlds and every test use, and `for_body` returns
   `SurfaceInputsError::NotModelled` naming the section, as the galaxy README's slice rule requires.
   Wherever the pass computes a figure plan 14 also states, the result is constrained to plan 14's
   value, never left to drift.

4. **One level per body** (researched 2026-09-29: S2's `s2coords.h` and `s2metrics.cc`, recomputed).
   `coarse_level` takes the shallowest cube-sphere level whose mean cell, √(4πR² ÷ (6 · 4ᴸ)), is no
   wider than 40 km, floored at 5 and capped at 8: level 8 for an Earth (393,216 cells of about
   36 km) and for a 2 R⊕ world (about 72 km, where the cap binds), 7 for a Mars, 6 for the Moon, 5
   for Ceres. D_b is twice the level's largest cell edge, from S2's `kMaxEdge` derivative for the
   quadratic warp, 1.704897 (realised 1.7049 × 2⁻ᴸ at level 8): **84.9 km** on an Earth, 90.3 km on
   a Mars, 92.6 km on the Moon and 50.0 km on Ceres, not the brainstorm's "about 100 km". The
   smallest edge is about 0.947–0.972 × 2⁻ᴸ radians. The warp is C¹ but not C² across each face's
   centre lines (du/ds is continuous there, its derivative is not), so C¹ is the most any field on
   the sphere can promise.
5. **The pass is one job, in a fixed order.** Steps 1–6 run in the brainstorm's order, every loop
   in `cell_index` order, every sum in that order, every sort keyed by (value, cell index), in
   `f64`, with no `HashMap`, and the job ends with R04's probe. The sim spawns no threads, so the
   pass is one bulk job on the server's pool. One exception to the brainstorm's order: on a world
   whose wet epoch ended before its surface age ran out (Mars), each coarse crater is drawn a
   formation age, and only those older than the epoch's end are smoothed in before erosion; the
   younger ones are applied after step 4, so that "the craters of the rest of the surface age
   placed on top of it" holds.
6. **Plates** (researched 2026-09-29: Bird 2003, G³ 4, 1027; Sornette and Pisarenko 2003 from
   memory). Where the regime is mobile-lid, the plates are drawn on `surface.coarse.plates` keyed by
   `surface_item(k)`: seven above 1 sr and a tail N(>A) = 7 (A ÷ 1 sr)^−0.31 down to 0.002 sr,
   giving about 48 plates (41 below 1 sr; Bird's PB2002 has 48 at or above 0.002 sr and 52 in
   all), truncated at the coarse cell's solid angle, with Euler poles and crust types. Bird 2003
   fits 0.002–1 sr by eye with −1/3 (56 plates by this anchor); Sornette and Pisarenko 2003's −0.25
   ± 0.05 is fitted to the 42-plate PB2001 over all sizes with the 4π constraint and no anchor at
   1 sr, so it does not combine with the anchor; a maximum-likelihood fit to PB2002's 41 plates in
   0.002–1 sr gives 0.30 ± 0.09 (researched 2026-09-29 from Bird's Table 1, ¶115–118 and Fig. 19;
   medium confidence in 0.31, high in the counts). The tail is drawn first and the seven large
   plates share whatever of 4π it leaves, since the law's expected tail area, about 3.1 sr, exceeds
   Earth's 2.39.
   Each cell takes the plate of largest score x′ · p + w, x′ its centre moved by the low-frequency
   warp on `surface.coarse.warp`, p the plate's seed and w a weight fitted so that the plate's cells
   cover its drawn area (a power diagram; "Deviations in T11, as built"), ties to the lower seed
   index; a part cut off a plate joins the neighbour it shares most edges with. Boundaries are cell
   adjacencies. The distance transform is a multi-source shortest path over the cell graph with
   great-circle edge lengths, settled in (distance, cell index) order, giving each cell its signed
   distance to the nearest boundary, that boundary's kind (convergent with subduction, convergent
   collision, divergent, transform) from the plates' relative motion, and its obliquity. A stagnant
   lid gets no boundaries: volcanic provinces sized by the volcanism level, with flexural moats and
   bulges about each load.
7. **The variance budget and the landforms** (researched 2026-09-29; Earth2014 split at −2 km; GDH1,
   Stein and Stein 1992; flexure after Turcotte and Schubert 2014 §3.16–3.17, from memory). Crust is
   two populations about sea level, continental and oceanic, as Earth2014 shows (continental mean
   +406 m, s.d. 1.08 km; oceanic −4,279 m, s.d. 0.93 km); ridges follow GDH1's age–depth law, d =
   2,600 + 365 √t m below 20 Myr and 5,651 − 2,473 e^(−0.0278 t) m above, with the age t read from
   the distance to the plate's own divergent boundary, a second distance transform the pass keeps
   in its `f64` working state (not the nearest boundary of any kind, which would make old floor
   by a trench young; researched 2026-09-29: Stein and Stein 1992; Müller et al. 2008's age grid),
   capped at 200 Myr, beyond which GDH1 is within 40 m of its asymptote; the floor's roughness
   follows its own ridge too: the half-rate at formation sets the abyssal-hill relief (Goff et al.
   2004), its age the pelagic drape (GlobSed, Goff 2010), and the turbidites of its plate's passive
   margins, trapped by trenches, its plains (Harris et al. 2014's third of the abyss), carried to
   the synthesis in one byte (`decision-r09-t5.md` item 5); collisional belts 200–500 km wide with
   plateaus near 5 km and peaks near 9 km, trenches 2–4 km below the sea floor and volcanic fronts
   190 km behind a trench whose overriding plate is oceanic and 260 km behind one whose overriding
   plate is continental (Syracuse and Abers 2006's 527 front volcanoes; 10–90% 140–250 and
   180–330 km; Bird 2003's "200–250 km is more typical" brackets the median), varied along strike
   by the interquartile range, arcs 60 km and 175 km wide behind them, distances Earth's and
   unscaled (labelled), all heights scaled by (g⊕ ÷ g) × the lithosphere factor as
   strength-limited; flexure w = w₀ e^(−x/α)(cos x/α + sin x/α) with α = [4D ÷ ((ρ_m − ρ_fill)
   g)]^¼, ρ_fill being the fill's bulk density, its palette entry's grain density times one less
   the porosity T12.c states, D = E T_e³ ÷ 12(1 − ν²), E 100 GPa, ν 0.25 (Olympus Mons at T_e = 70
   km gives α ≈ 180 km, as observed).
   The synthesis's amplitudes are fixed per body by its `BandSpectrum` (the
   structural share's law, `decision-r09-t5.md` items 2–4: per-degree variance ∝ l^−β₁ from the
   coarse cell to a break at λ_b, and ∝ l^−β₂ below it, continuous; on silicate crusts β₁ 2.0, λ_b
   2 km, β₂ 3.0, from SRTM, Earth2014's land, GMRT and Perron et al. 2008; on ice-rich crusts 1.8,
   1 km and 2.6, from Europa's DEMs; no gravity term in the break; the craters' own steepening near
   10 km on the Moon, Mercury and Mars is T7.b's), so its expected variance below the coarse cell
   is closed-form (`local_variance`). `local_variance` is the reference style's. The styles'
   amplitudes are absolute ratios to the body class's reference ground, so the realised structural
   share is the solid-angle mean of Σ_m (a g_m^b)² σ_m² over the reconstructed field, a and b
   interpolated, with the reference-style tail beyond the finest octave: ⟨a²⟩ `local_variance` off
   belts, and more where belts lift their ridge–valley octaves (5.4% on the Earth-like world;
   `decision-r09-t5.md` items 7 and 8). It is never normalised away: a world with more belts is
   rougher (`decision-r09-t5.md` item 1). V₁ is anchored at the coarse cell, never from degree 1:
   the structural RMS in wavelengths shorter than 72 km is 200 m × (g⊕ ÷ g) on a mobile lid
   (Earth2014 with the abyssal hills its altimetric bathymetry lacks), 60 m on a rocky stagnant lid
   (Mars's crater-poor plains and the maria) and 50 m on an ice-rich crust (Europa), all medium to
   medium-low confidence (`decision-r09-t5.md` item 3). The coarse elevation is scaled affinely
   about its area-weighted mean so that the **reconstructed** field's variance, the interpolant of
   Design note 13 integrated by solid-angle quadrature at sub-cell points, is σ_h² less the local
   shares: the realised structural share (above), and the craters' and channels' closed forms finer
   than the coarse cell's Nyquist wavelength, the coarse craters' sharpening included
   (`decision-r09-t5.md` item 4); and again after erosion, when the channels' share is known; the
   cell values alone would fall several per cent short by the spline's smoothing. The local share
   is not negligible at the plan's own levels: above the coarse cell's Nyquist degree it is 0.6% of
   σ_h² for an Earth or a Mars (about 200 m RMS), 4% for Mercury, 5% for Ceres and 7% for the Moon
   (about 600 m), all contributions together, against the brainstorm's 0.1–0.2% and 2%, which hold
   at a 35 km wavelength. Sea level is the area-weighted quantile that leaves plan 14's ocean
   fraction below it. The realised σ_h and greatest relief go in the header.
8. **Climate** (researched 2026-09-29: Ramirez 2024, arXiv:2310.15992; Spiegel, Menou and Scharf
   2008; Siler, Roe and Armour 2018; Hergarten and Robl 2022; Williams and Kasting 1997's exponents
   from memory). Ramirez 2024 cannot be reimplemented from the paper, since its outgoing radiation
   and albedo are unpublished radiative-convective tables, which in any case cover only
   N₂–CO₂–H₂O–H₂ at 150–390 K. The pass therefore runs its own seasonal moist energy-balance model
   from published parts: North and Coakley's diffusive equation in Spiegel et al.'s form, extended
   in longitude, diffusing near-surface moist static energy at 80% relative humidity as Siler et al.
   do (Earth's D = 1.16 × 10⁶ m² s⁻¹); heat capacities of land 5.25 × 10⁶ J m⁻² K⁻¹, ocean 40× and
   ice 9.2× or 2× by temperature (Williams and Kasting 1997, after Spiegel et al.); gray outgoing
   radiation σT⁴ ÷ (1 + ¾τ) with τ solved once so that the mean matches plan 14's; albedo 0.525 −
   0.245 tanh((T − 268 K) ÷ 5 K), both shifted to the condensable's freezing point; D scaling with
   rotation and pressure as an idealised moist GCM gives them (the transport law, below), with heat
   capacity and molar mass as Williams and Kasting 1997 scale them, under an overall cap of 30 D⊕,
   and halved on dry worlds; locked worlds in tidally locked coordinates about P14.T14's
   substellar axis, seasonal unless the world lacks seasonal forcing (the months, below). It steps
   **implicitly**
   (backward Euler, a tridiagonal solve in latitude and a periodic one in longitude, split in a
   fixed order) at the brainstorm's six hours, since the explicit limit is about 11 h on Earth and
   fails for any slower rotator, and runs a fixed maximum of orbits until the annual mean moves
   under 0.1 K. The grid is 2.5° in latitude by 5° in longitude (72 × 72 cells), and 2.5° × 2.5° in
   a locked world's tidally locked coordinates, rather than the brainstorm's 5° × 30°, which is too
   coarse to interpolate to cells of about 72 km: Spiegel et al. 2008 found a 10° latitude grid
   biases the global mean by over 4 K and converged at 1.25° (researched 2026-09-29, medium
   confidence; Okuya et al. 2019 for a two-dimensional precedent). The fields are interpolated to
   the cells bicubically before the lapse term. A month is a twelfth of the seasonal orbit in
   eccentric anomaly, counted from periapsis.
   - Month k spans E from k·30° to (k + 1)·30°. Its share of the period is
     [ΔE − e (sin E_{k+1} − sin E_k)] ÷ 2π: a twelfth on a circular orbit, 29.9–30.9 d on Earth,
     and 0.4–1.6 twelfths at e = 0.6.
   - This is so that twelve samples hold an eccentric orbit's brief periapsis season, which
     twelve equal-time months miss above e ≈ 0.3 (`decision-r09-t2.md` item 1).
   - The seasonal orbit is the one that sets the sun's declination and distance: the body's own
     about its star or stars, or its planet's for a moon. The header carries its eccentricity.
   - A world with no seasonal forcing has one month: e = 0 with no obliquity to its seasonal
     orbit. A lock adds nothing: a world locked 1:1 to its star has seasons wherever its orbit is
     eccentric or it has obliquity, and a moon locked to its planet has its planet's orbit's
     seasons at its own obliquity to that orbit (a Titan, under Saturn's 26.7°). _Signed off
     2026-10-10 by the sign-off agent (owner's delegation); Risks, "Which worlds have one
     month"._
   - Each month's record is the mean over its span of the six-hour steps of the converged orbit.

   Plan 14's figures are imposed on the component that defines
   each: the equator–pole contrast by setting the annual field's P₂(sin φ) coefficient to −⅔ of
   it, since the contrast T(0°) − T(90°) is −(3/2) T₂ (the sign carries a warm pole; T10's science
   review, 2026-10-09), a locked world's day–night contrast by P₁(cos γ), and the mean by a
   constant added last to the surface temperature after the lapse term; ice goes coldest first, by
   the cell's annual maximum, until its area matches plan 14's fraction, which also places a locked
   nightside and an equatorial belt. Precipitation stays labelled `PrecipitationSource::Heuristic`:
   Siler et al.'s Hadley-cell partition gives E − P with the rain band on the energy-flux equator
   (our extension centres it there each month) and the dry belts and storm tracks unplaced by hand;
   its width scales with rotation after Held 2000 (from memory); evaporation over open liquid is
   Priestley–Taylor with α = 1.26 (from memory); onto the cells, Hergarten and Robl's linear
   feedback precipitation model carries vapour along the month's prevailing wind for orography, rain
   shadows and inland decay (L_c = L_f = 25 km, L_l = 500 km, H₀ = 2 km, L_d = 25 km, their South
   American fit), solved by a fixed number of Gauss–Seidel sweeps in `cell_index` order, and
   rescaled so that global precipitation equals global evaporation. Prevailing winds are the
   three-cell pattern scaled by the Hadley width, centred each month on the month's energy-flux
   equator, and converging on the substellar point for slow and locked rotators.
   - Each month's wind is the one its orographic step carried vapour along, stored as the
     month's 10 m wind: the direction of its resultant and its mean speed. These coincide while
     the pattern has no transients.
   - The field's rain shadows and its winds therefore agree month by month.

   The offline check against ExoPlaSim is a recorded task, never run on arrival.

   _The transport law_ (researched 2026-10-09, by R09.T0.b; medium confidence). The FILLET ensemble
   cannot test a cap. Its protocol (Deitrick et al. 2023, arXiv:2302.04980, and its v1.1,
   arXiv:2511.11957) runs Earth-like 1 bar aquaplanets at Earth's rotation with one prescribed D,
   0.5 W m⁻² K⁻¹ in its D = κ ÷ R² (its Table 4), across obliquity, instellation and CO₂, and its
   results paper is not yet out. It bears only on D⊕, which for Siler et al.'s 1.16 × 10⁶ m² s⁻¹ is
   about 0.3 W m⁻² K⁻¹ dry and 0.7 moist at 288 K and 80% relative humidity, bracketing FILLET's
   0.5 and North, Cahalan and Coakley 1981's 0.649, and on T13.d's ice-edge states. The Ω⁻² law
   itself fails. In Kaspi and Showman 2015's idealised moist GCM (ApJ 804, 60, arXiv:1407.6349;
   aquaplanet, no seasons, no ice, gray radiation), the equator–pole surface contrast is about 23 K
   at Ω⊕ ÷ 24, 28 K at Ω⊕ ÷ 8, 36 K at Ω⊕ ÷ 4 and ÷ 2, 40 K at Ω⊕, 48 K at 2 Ω⊕, 58 K at 4 Ω⊕, 70
   K at 8 Ω⊕ and 77 K at 12 Ω⊕ (their Fig. 8a, read to ±1 K), and "the total energy transport is
   rather insensitive to rotation rate when Ω < Ωe" (§3.1): Ω⁻² would raise D 64-fold by Ω⊕ ÷ 8,
   where the contrast has fallen by 30%. Inverting a linear model (ΔT ∝ 1 ÷ (B + 6D′), with B = 2.09
   and D′⊕ = 0.649 W m⁻² K⁻¹) gives D ÷ D⊕ ≈ 1.2 at Ω⊕ ÷ 4, 1.7 at Ω⊕ ÷ 8 and 2.1 at Ω⊕ ÷ 24, and
   0.74, 0.52, 0.34 and 0.26 at 2, 4, 8 and 12 Ω⊕: about (Ω ÷ Ω⊕)^−¼ below Earth's rotation and
   (Ω ÷ Ω⊕)^−½ above. Pressure saturates likewise: the contrast is 47 K at 0.2 bar, 42 K at 1, 37 K
   at 3, 30.5 K at 10, 24 K at 30 and 22 K at 50 bar (their Fig. 15c; the eddy flux levels off
   beyond 10 bar, §3.3), about (p ÷ p⊕)^0.2 (0.84 at 0.2 bar, 1.6 at 10 and 2.4 at 50) where D ∝ p
   gives 50. So the rotation and pressure factors are tabulated from those two figures, T13.a
   fitting the tables with the EBM itself so that its contrast ratios match the GCM's to 2 K over
   Ω⊕ ÷ 24 to 12 Ω⊕ and 0.2 to 50 bar, and holding each at its end value beyond that range (the
   lean of 2026-09-29, (Ω⊕ ÷ Ω)² ≤ 64 under about 100 D⊕, is withdrawn). The cap of 30 D⊕ stays as
   a guard that only the unverified heat-capacity and molar-mass factors can reach, as in an
   H₂-rich air (low confidence).

9. **Erosion** (researched 2026-09-29: Tzathas et al. 2024, CGF 43(2), from the authors' PDF;
   Barnes, Lehman and Mulla 2014; Barnes, Callaghan and Wickert 2021; Hack 1957; Luo, Cang and
   Howard 2017). Where the world has or had surface liquid, the stream-power law ∂z/∂t = u − k A^m
   ‖∇z‖ⁿ is solved by Tzathas et al.'s analytical method, which **requires n = 1** (their
   characteristics are exact only then; a known limitation, labelled). Receivers are one of the
   **four edge neighbours** (every cube-sphere cell has four, corners included), strictly lower,
   chosen at random with probability proportional to the drop per metre of great-circle distance,
   from a per-cell uniform on `surface.coarse.erosion`, with their slope correction on the
   face-local axes; ties by (elevation, cell index). The linear-time recursion marches up each river
   tree depth first; the fixed point iterates routing and solution with a moving-average weight of ½
   (recorded; the paper gives none); multigrid runs on the quadtree's own parents
   (solid-angle-weighted means) down to level 4, six iterations a level, the upsample through the
   synthesis's interpolant at child centres jittered on the same tag. The base level is the sea, or
   on a world dry now the terminal outlets left by priority flood (flat directions taken from the
   pop order, so no ε-raising is needed) and Fill–Spill–Merge, which spreads the paleo-inventory
   through the depression hierarchy after the paleo-sea is placed by plan 14's own logistic. A world
   wet now runs to steady state; one dry now runs for t = `effective_flow`, with the drainage area
   weighted by the epoch's `runoff`, uniform over the cells since no past climate is modelled,
   only where the surface is older than the epoch's end; one with no wet epoch skips the step.
   m = θ = 0.45 (Kirby and Whipple 2012, from memory), with the drainage area replaced by Hergarten
   2021's runoff-weighted equivalent area from the climate step (on a world dry now, the epoch's
   `runoff`), normalised by a reference runoff of 1 m a⁻¹, which is the only place runoff enters
   (the first draft also multiplied K by runoff to the m, counting it twice). Both the paper's code
   (Inria research-only licence) and Dendry's (GPL-3.0) are implemented from the papers alone.
   Outputs: final elevation, flow direction, the geometric drainage area (T14.a's, by solid angle,
   as Hack 1957 measures it), the steepness index k_s = S A^θ on that area (Kirby and Whipple
   2012's k_s, Adams et al. 2020's k_sn), each cell's own erosive runoff and its catchment's
   area-weighted mean runoff, and water surfaces.

   The equivalent area is the solver's alone. It is Hergarten 2021's "catchment-size equivalent of
   the discharge", A_eq = q ÷ p₀ (ESurf 9, 937, eq. 16), with p₀ = 1 m a⁻¹. On the field it is the
   catchment runoff times the drainage area, and the discharge-weighted index k_snQ = k_s (r̄ ÷ 1 m
   a⁻¹)^θ (Adams et al., eq. 3; Leonard, Whipple and Heimsath 2023).

   The geometric area serves what is geometry: Hack's law, which Hergarten keeps in geometric area
   (his eq. 11), the network's lengths and k_sn. The runoffs serve what is water: discharge, width,
   and a tributary's own steady state, which is at its own runoff, not its trunk's (Hergarten and
   Robl 2022, p. 2080). Ruled 2026-10-10 in science-r09-drainage.md §1.

   _Erodibility_ (researched 2026-10-09, by R09.T0.b; low to medium-low confidence, stated in the
   doc comment): K = K⊕ (g ÷ g⊕)(ρ_f ÷ ρ_water) B. K⊕ = 6.3 × 10⁻⁶ m^0.1 a⁻¹: Tzathas et al.'s 2 ×
   10⁻⁵ m^(1−2m) a⁻¹ is for m = 0.4 (their Table 1, with a precipitation of 1 m a⁻¹), not 0.45, and
   converts at n = 1 as K A_ref^(0.4 − 0.45) at A_ref = 10¹⁰ m² (7.1 × 10⁻⁶ at 10⁹ m², 5.6 × 10⁻⁶ at
   10¹¹), which used unconverted would erode about three times too fast; it lies at the
   volcaniclastic end of Stock and Montgomery 1999's field values for m = 0.4 and n = 1 (JGR 104,
   4983: 10⁻⁷–10⁻⁶ for granite and metamorphic rock, 10⁻⁵–10⁻⁴ for volcaniclastics, 10⁻⁴–10⁻²
   for mudstones). K goes as gⁿ in both the shear-stress and the unit-stream-power forms, and as
   ρ_f^(1.5n) and ρ_f^n respectively (Whipple and Tucker 1999, JGR 104, 17661, eqs. 8–10), so at
   n = 1 g enters linearly and ρ_f is taken linearly. B is the bed's factor: 1 for rock; 10 for
   impact-brecciated regolith such as Mars's Noachian highlands, taken where the surface is
   crater-saturated (Barnhart, Howard and Moore 2009, JGR 114, E01003, ¶78, regolith ten times the
   bedrock's erodibility, in a model that is "essentially the DELIM model as reported by Howard
   [1994a, 1997, 2007]", ¶71, whose bedrock erodibility is set from long-term terrestrial rates in
   weak sedimentary rock); and 8 for water-ice bedrock under liquid methane, a combined
   bed-and-fluid factor that puts Titan's K at about half of Earth's at equal runoff. Collins 2005
   (GRL 32, L22202, ¶18) finds Titan's incision rates "likely to be surprisingly similar to
   terrestrial rates, given similar stream conditions" (by saltation abrasion after Sklar and
   Dietrich 2004, cold ice being about 53 times as erodible per unit impact energy as sandstone,
   offset by grains that strike with about 50 times less energy), and Litwin et al. 2012 (JGR 117,
   E08013) find ice strengthening by 7 kPa K⁻¹ from 260 K down to 110 K, about twice as strong at
   Titan's 94 K by extrapolation. That gives K ≈ 2.4 × 10⁻⁵ m^0.1 a⁻¹ on an ancient Mars and 3.1 ×
   10⁻⁶ on a Titan. _The solver's time_ is in Earth-equivalent years (ruled 2026-10-10,
   science-hoke.md; medium-low confidence). Each K is a long-term terrestrial calibration that
   already holds the intermittency of Earth's channel-forming flows, taken as 5% of the time:
   Kleinhans 2005 (JGR 110, E12003, ¶62) writes that "channel-forming discharges on Earth occur
   roughly for about 5% of the time", after Parker, and matches Earth's deltas within a factor of
   three with it (¶82); Hoke, Hynek and Tucker 2011 (Table 3 note b) give "5% of the time, as is
   typical for Earth (Wolman and Miller, 1960)". So `effective_flow` is the time channel-forming
   flow ran at a point over the epoch divided by 0.05, and the runoff that weights the area is the
   storms' runoff while they flow times 0.05. Where the epoch's own intermittency is the
   reference's, they are a point's episode in years and its mean annual runoff. At fixed flow time
   and storm runoff the erosion goes as the reference to the power m − 1, so Barnhart et al.'s 2%
   (¶70, DELIM's own conversion of its flood time, whose erodibility this note does not use) would
   erode 1.7 times more, and an arid river's 1% (Hoke et al.'s Batha) 2.4 times more; both are
   within K's own uncertainty. "Arid to semi-arid" is Howard 2007's and Barnhart et al.'s finding
   for Mars's epoch (Hoke et al. §3), a property of the epoch, not of the reference. Hoke et al.'s
   times assume the reference. "With runoff rates similar to intense storms in arid regions on
   Earth", Mars's valley networks need at least 10⁵–10⁷ yr, and 10⁵–10⁸ yr for the shallower flows
   the authors prefer, at storm runoffs of 0.4–6 cm a day. Those are `effective_flow` as they
   stand, 5 × 10³ to 5 × 10⁶ yr of flow at a `runoff` of 0.07–1.1 m a⁻¹. Only continuous
   hurricane-scale flows, "not the preferred interpretation", would take 200–5,000 yr. The first
   draft's "years at effective discharge" would have run Mars's epoch twenty times too short. As a
   cross-check, Black et al. 2017's supplement (Science 356, 727; Table S2: K = 10⁻⁸ m^(1−2m) a⁻¹
   at m = 0.5 over 60 Myr) matches Mars's 50–350 m of trunk-valley incision with K t ≈ 1.9 m^0.1 at
   m = 0.45. The Mars K above reaches that where `effective_flow` × (`runoff` ÷ 1 m a⁻¹)^0.45 ≈
   7.9 × 10⁴ yr: 2.2 × 10⁵ Earth-equivalent years at 0.1 m a⁻¹, or 1.1 × 10⁴ yr of flow. That is
   the foot of Hoke et al.'s range, whose times are minimums from transport-limited models with an
   order of magnitude in grain size alone, and P14.T48.b sets Mars's epoch there. These readings
   were made from the papers by a research sub-agent of T0.b; Howard 2007 itself (Geomorphology
   91, 332) was read only in abstract, and Barnhart et al. stand in for its constants.

10. **Coarse craters** (researched 2026-09-29). Craters of D_b and wider are drawn on
    `surface.coarse.crater`: the expected count over the body's area is min(production,
    saturation) per octave, which on an old surface is the saturation cap (the reference Moon's
    production gives about 2,500 above 100 km and the cap about 300, the brainstorm's "a few
    hundred"); positions uniform on the sphere, morphology by diameter against D_t, degradation by
    age. They are smoothed into the elevation and sent as a list sorted by (`cell_index` of the
    centre's cell, diameter), each recording in its `reach` every cell its reach (2.54 rim radii,
    Design note 13) touches, so that a block carries every crater that reaches its cells. The field
    keeps a per-cell index of the craters reaching each cell, built from the `reach` lists, which
    is what `FieldView::craters_reaching` walks; the index is rebuilt on the client as blocks
    arrive and is not on the wire.
11. **Classes** (researched 2026-09-29: Peel, Finlayson and McMahon 2007, Table 1; Beck et al.
    2018). Köppen–Geiger for seasonal water-cycle regimes, by Peel et al.'s Table 1, which Beck et
    al. adopt (researched 2026-09-29, high confidence): C and D divide at a coldest month of 0 °C
    (Peel, after Russell 1931); B is tested first and takes precedence, with MAP < 10 P_th, where
    the 70% rule sets P_th = 2 MAT, 2 MAT + 28 or 2 MAT + 14 (mm, MAT in °C) for winter-,
    summer- or neither-dominant precipitation; within B, W is MAP < 5 P_th and S is MAP ≥ 5 P_th;
    h and k divide at MAT 18 °C; when both s and w hold, w if summer precipitation exceeds
    winter's, else s,
    classifying climate and not life: there is no vegetation class, and a lifeless Earth-like world
    keeps its class with bare ground. Months are not Earth months, so the table is applied to
    **rates** (precipitation per 30.44 d and per 365.25 d); summer is each cell's warmest
    consecutive half-year, six months on a circular orbit, and every count of months in the table
    (such as four months above 10 °C) counts their durations in twelfths of the year, since an
    eccentric orbit's months differ in length (Design note 8); the temperature thresholds stay.
    Worlds with one month, and every
    non-seasonal regime, classify surface state (liquid, ice or frost of a named species, rock,
    regolith, melt, organic sediment) with the regime's named zones, such as a locked world's
    substellar ocean and nightside glacier. One `SurfaceClass` byte holds either.
12. **One crater density, in the surface crate** (researched 2026-09-29: Neukum, Ivanov and Hartmann
    2001, Table 1, checked in Michael's Craterstats `functions.txt`; Trask 1966 and Hartmann 1984;
    Pike 1980a (Proc. LPSC 11th, 2159–2189), Tables 1–2, and Pike 1980b (USGS Prof. Paper 1046-C),
    Table 6; Kalynn et al. 2013 (GRL 40, 38–42; LPSC 2013 abstract 1309); Stopar et al. 2017
    (Icarus 298, 34–48), Table 4; Susorney et al. 2016 (Icarus 271, 180–193), Tables 2.1–2.2 of
    Susorney's 2017 dissertation; Robbins and Hynek 2012 (JGR 117, E06001); Tornabene et al. 2018
    (Icarus 299, 68–83); Krüger, Hergarten and Kenkmann 2018; Bland and Artemieva 2006; Ivanov
    2001, Space Science Reviews 96, for the Mars production function T7.a checks against). The
    brainstorm has both passes invert "one shared function in the sim"; the fine pass runs in
    `hyperion-surface`, which cannot depend on the sim, so the function lives in
    `hyperion-surface::craters` and plan 14 calls it from the sim. Its production shape is Neukum's
    polynomial log₁₀ N(>D) = Σ aₙ (log₁₀ D)ⁿ, 10 m to 300 km, holding a1…a11 only: a0 is log₁₀ N(>1
    km), the parameter, so the input is exact and neither published a0 enters (Neukum et al. 2001's
    −3.0876 is the reworked polynomial's own 1-Gyr level, which matches Neukum 1983's below 1 km,
    where the 1983 polynomial's a0 is −3.0768, Ivanov et al. 2002, Table 1; the chronology's linear
    term, 8.38 × 10⁻⁴ = 10^−3.0768, is the 1983 normalisation, kept as published, as Craterstats
    keeps it, rather than Cratermaker's 10^−3.0876 = 8.17 × 10⁻⁴; science-r09-craters.md F3),
    extended by the end slopes outside its range (−2 below 10 m). The contract's N(>1 km) is
    lunar-equivalent: the Moon's density for the body's projectile flux (the chronology × the belt
    scaling). Other bodies take it to their own craters by a monotone diameter map by π-group
    scaling, N(>D) = N☾ × 10^(Σ aₙ (log₁₀ φ(D))ⁿ), so a body's own N(>1 km) is N☾ at φ(1 km): 0.53
    of N☾ on Venus, 0.256 on Mars at Ivanov's velocities, whose 0.49 Mars chronology then implies
    his R_b ≈ 2 (science-r09-craters.md F1). The map is in the gravity regime (Holsapple 1993;
    Ivanov 2001's construction of Mars's function), optional in `CraterParams` and the identity at
    the lunar reference. A lunar crater's transient cavity gives its projectile at the reference
    (non-porous, ρ_t 2,550 kg m⁻³, ρ_p 3,000 kg m⁻³, U 16.7 km/s, g 1.62 m s⁻², D_t 19 km). The same
    projectile's crater on the body follows from its class (non-porous μ 0.55 and K₁ 0.095; porous μ
    0.41 and K₁ 0.132; ν 1/3 for both; Holsapple 1993 as Singer et al. 2019 apply it), its target
    and projectile densities, the mean impact speed (no angle factor), g, its own collapse at its
    D_t, and the class's transient volume-to-diameter constant, a paraboloid as deep as the class's
    fresh bowl: (4/3 · 8/0.20)^⅓ = 3.764 non-porous, (4/3 · 8/0.14)^⅓ = 4.239 porous. Between two
    non-porous bodies the constant, K₁ and the absolute speed cancel; for a porous body they do not
    (science-r09-craters.md F4.2). Ice is non-porous, at ice Ih's density at the body's mean
    surface temperature, 916.7–933.8 kg m⁻³ (IAPWS R10-06(2009); science-r09-craters.md item 2 and
    F5.3). The strength regime is not modelled (Risks). Saturation is
    Trask's N(>D) = 0.079 D⁻² km⁻²; capped craters draw their ages from the last T × N_sat ÷ N_prod
    of the surface age, inverted through the chronology, `craters::chronology_age`, which counts
    from the system's formation (the contract's system age; science-r09-craters.md item 5). That
    window is τ(D), the span a crater of diameter D draws its age from: the whole surface age where
    production is under saturation, and T × N_sat(D) ÷ N_prod(D) where it is capped.
    Screening is a projectile scale, d\* = 1.5 (P ÷ g) ÷ ρ_p (5.2 m for Earth, 0.52 km for Venus
    and 8.5 cm for Mars at NASA's mean surface pressure of 636 Pa, projectile sizes at ρ_p = 3,000
    kg m⁻³ and the brainstorm's 5 m, 0.5 km and 8 cm; this note's earlier 8.2 cm for Mars took
    about 610 Pa, T10's science review, 2026-10-09), giving a crater cutoff D_c = 20 d\* (about
    100 m, 10.4 km, 1.7 m) and a taper on the differential production,
    f(D) = 1 ÷ (1 + (D_c ÷ D)^4.2), one rule for every body (researched 2026-10-09, by R09.T0.b,
    and refitted through the diameter map by science-r09-craters.md F1; the first draft's
    exponent 2 and its break-up branch above 1 MPa, D_c = 20 km with exponent 1.5 and a floor at
    1.5 km, are withdrawn). The check is against Venus's crater catalogue itself, the
    population Herrick and Phillips 1994 (Icarus 112, 253–281) model, whose text could not be
    reached (publisher paywall): the 881 named craters of the IAU's Gazetteer of Planetary
    Nomenclature (USGS, retrieved 2026-10-09; 2.0–270 km, of about 940 that Magellan mapped, from
    memory), binned in 23 logarithmic bins from 1.5 to 300 km and fitted by Poisson likelihood with
    Neukum's polynomial (Craterstats' coefficients) and a free N(>1 km). The best taper is D_c =
    9.0 km with exponent 5.4; 20 d\* = 10.4 km with exponent 4.5 lies 8 below it in ln L (exponent
    4 lies 28, 5 lies 11), while the first draft's exponent 2 lies 714, its break-up branch 846 and
    the two multiplied 116. Unmapped, that taper passes 0.4% of the production at 3 km, 4% at 5 km,
    23% at 8 km, 46% at 10 km and 95% at 20 km, so it needs no floor; its cumulative counts lie
    within 7% of the catalogue's from 1.5 to 95 km, and its N(>1 km) of 3.1 × 10⁻⁴ km⁻² is a
    lunar-chronology age of 0.37 Ga, inside the few hundred Myr to about 0.75 Gyr published for
    Venus's mean surface (from memory). Through the diameter map, at P14.T48.f's 25.7 km/s, the
    same fit gives exponent 4.2 at 20 d\* (the likelihood peaks at 4.25, 0.14 above 4.2; 4.5 lies
    2.1 below, 4.0 3.0 and 5.0 17.5) and a lunar-equivalent N☾(>1 km) of 3.36 × 10⁻⁴ km⁻², 1.79 ×
    10⁻⁴ of Venus's own, an age of 0.40 Gyr ÷ R_b. Its cumulative counts lie within 11% at 3–40 km
    (10.8% at 20 km). A free cutoff, 8.5 km with exponent 5.2, lies 18.8 above it in ln L, but
    π-scaling puts a d\* projectile's crater at 18–21 d\* on Venus, so the rule stands. The adopted taper passes 0.5% of the production at 3 km, 4.5% at
    5 km, 25% at 8 km, 46% at 10 km and 94% at 20 km (science-r09-craters.md F1). The exponent is
    measured on Venus alone, so it is low confidence for thinner atmospheres, whose cutoffs keep
    their projectile scales. The transition
    is D_t = 19 km × (1.62 ÷ g) × k_target (Pike's four bodies give g^−1.01), with k_target 1 for a
    silicate crust and 0.12 for an ice-rich one (science-r09-craters.md item 1: the morphological
    transitions of Ganymede, Rhea, Iapetus, Dione, Tethys, Ceres, Charon, Europa and Pluto, read as
    0.82 D_t, give a geometric mean of 0.125 and a range of 0.06–0.26; Schenk et al. 2004;
    Aponte-Hernández et al. 2021; Ermakov et al. 2017's mixed crust behaves as ice; no resolvable
    temperature term between 40 and 150 K), a zone: bowls below 0.8 D_t, transitional to 1.5 D_t,
    central peaks above, peak rings from about 9 D_t, multi-ring basins above about 16 D_t. Fresh
    shapes (`decision-r09-t2.md` item 3; d is rim crest to floor, h is rim crest above the
    surrounding surface):
    - _Bowls_ do not depend on gravity: d = 0.20 D and h = 0.20 D ÷ 5.42 = 0.0369 D.
      - Pike 1980b's 0.196 D^1.010 and 0.036 D^1.014; Stopar et al.'s 0.209 and "∼0.04"; Mercury's
        0.199 D^0.995 (Pike 1988).
      - On an airless body (`Screening::None`), the bowl's d/D falls through Stopar et al.'s A-class
        bins, which are 0.125, 0.152 and 0.166 at 63, 141 and 283 m and 0.20 from 400 m. The fall
        is linear in log D between the bins, held below 63 m, and at D × g ÷ 1.62 off the Moon. The
        plan's own gravity scaling is of low confidence. Icy airless bodies take the same rule:
        their only fresh simple-crater depths, all at 1 km and above (Ganymede 0.21, Callisto 0.19,
        Europa 0.17, Schenk 2002; Rhea 0.221, Iapetus 0.187, White et al. 2013), lie within what it
        predicts (science-r09-craters.md item 3).
      - On a porous-class target, the bowl is 0.14 D deep at every size, less where the regolith
        rule gives less (the asteroidal value, Aponte-Hernández et al. 2021 §1; medium
        confidence).
    - _Complex craters_ are self-similar in D ÷ D_t:
      - d = 0.150 D_t (D ÷ D_t)^0.303, the mean of Kalynn et al.'s LOLA mare and highland fits;
      - h = 0.0402 D_t (D ÷ D_t)^0.399, Pike 1980b's eq. (4);
      - floor diameter 0.389 (D ÷ D_t)^0.249 of D, held at 0.778 above 16 D_t;
      - a central peak of relief 0.0238 D_t (D ÷ D_t)^0.900 (Pike 1980b, Table 6) × w_on × w_off.
        It grows from 0.8 D_t by the transitional blend's weight, w_on = ln(x ÷ 0.8) ÷ ln(1.875)
        with x = D ÷ D_t: peaks occur in 0% to 100% of fresh lunar craters across 17–35 km (Wood
        and Andersson 1978), Smith and Sanchez 1973's median is 14 km, and Pike's seven peaks under
        28 km sit below his law. It fades across the protobasins, w_off = 1 − ln(x ÷ 9) ÷ ln(13 ÷ 9), Head
        1977's 150–250 km (both via Hale and Head 1979). Its base diameter is Hale and Head 1979's
        0.259 D − 2.5 km, written as 0.259 D − 0.132 D_t and never wider than the floor
        (science-r09-craters.md F2).
      - So at a fixed D, d ∝ g^−0.70 and h ∝ g^−0.60.
    - _Transitional craters_ blend the two in log D across 0.8–1.5 D_t, their central peak growing
      by the same weight.

    Out of sample, the laws hold:
    - Mercury's depths within 10% of MLA (Susorney et al. 2016) and its rims within 14%;
    - Mars's depths within ±27% of MOLA's fresh craters (Robbins and Hynek 2012), but 30% shallow
      at 80 km against the deepest (Tornabene et al. 2018; Risks);
    - Ganymede's within 22% (Schenk 1991).

    The zone's 0.8 and 1.5 D_t match Krüger et al. 2018's lunar 14–17 and 24–28 km. 1–2 m
    craters are typically 0.05–0.1 m deep, and 0.2 m only when fresh, a few per cent of them under
    the saturation rule, so they stay decoration (the input to open question 18, R11's).

13. **The per-query evaluation** (researched 2026-09-29: S2; Perlin's reference code; Gaillard et
    al. 2019; Hack 1957). _Base elevation:_ a uniform cubic B-spline per face over its cells,
    extended by ghost cells three deep whose values are bilinear in the owning face's cells at the
    ghost's direction (the owning face by R05's canonical-face rule), blended in a one-cell band
    about every face edge by the partition of unity w_F = β(s)β(1 − s)β(t)β(1 − t) with β a quintic
    smoothstep, normalised over the faces whose weight is not zero and summed in face order from the
    canonical 3D point, with the analytic gradient of the whole. Both sides of an edge evaluate the
    same faces, weights and order, so values and gradients agree **bit for bit**, and the field is
    C¹ (the warp's limit); the spline is approximating, a low-pass that never overshoots the cells'
    range. Categorical fields come from the nearest cell. _Structural octaves:_ R05's 3D improved
    Perlin noise at the point on the sphere, seamless by construction, with Perlin 2002's 16-entry
    gradient table indexed by four bits so that the mean is exactly zero, R05's certified bound B,
    and R05's `LatticeCache`; ridged multifractal for a mountain belt with its pinned mean removed,
    in the octaves at and above the spectrum's break only, the belt's own shape lifting its
    ridge–valley wavelength (1.5 breaks) 1.8× and steepening it by 0.4 below the break, so that its
    slopes hold near threshold as SRTM's belt cores do (`decision-r09-t5.md` item 7), low amplitude
    on an abyssal plain, domain warp taking offsets only from octaves no finer than
    the level warped. Threefry2x64-20 stays the lattice hash: with the lattice cached it runs once
    per corner, and the cached path is 2.5–3× faster than any hash. _Channels:_ one Dendry network
    extended through about fourteen levels from the coarse cell (36 km) to the band limit, rather
    than separate instances joined: its first level is the coarse flow network itself, a key point
    per coarse cell joined to the cell its `flow` names; each later level's key points, inherited
    from the coarser one and jittered on `surface.channel` (ε = 0.25), join the nearest segment of
    all coarser levels; neighbourhoods are graph neighbourhoods on the cube and distances 3D chords
    between canonical key points, so a face edge needs no special case. The paper's height
    reconstruction is replaced by the physics: the bed at the steady-state slope S = k_s (r̄ ÷ r)^θ
    A^−θ, with A from Hack's law in SI (a geometric area, Hack 1957, p. 47), k_s the coarse cell's
    geometric index, r̄ its catchment runoff and r its own runoff, since a tributary's steady state
    is at its own catchment's runoff (Design note 9); a cell with no runoff of its own carries no
    tributaries; L = 0.320 A^0.6 (Hack's 1.4 in miles; the brainstorm's 1.5 is Tzathas et al.'s
    mile-based figure and is 4.7× too long in SI), and a valley cross-profile from the distance to
    the channel; each
    level's mean incision over its parent cell is subtracted. After the first level the distance
    search is 3 × 3 with cached bounding boxes and inverse lengths, and levels whose channels are
    narrower than about four band limits are not evaluated. Those channels, under about 8 m wide,
    are about 0.3–0.6 m deep at bankfull by regional hydraulic geometry (Bieger et al. 2015, from
    memory; Leopold and Maddock 1953; Parker et al. 2007), a depth and width taken from discharge,
    Q = r A for a tributary and r̄ A for a trunk, and never from area alone, since an area-based
    regional curve carries its region's runoff (Bieger et al. 2015's abstract finds "drainage area
    is a less reliable predictor of bankfull channel dimensions than bankfull discharge"; Leopold
    and Maddock 1953, p. 16, w = aQ^b with b = 0.5), comparable to the finest band's RMS in this
    spectrum, and the band limit resolves channels from about 4 m, so the cut is a cost lever
    that removes resolvable, authoritative channels, not a cut below the band's amplitude
    (researched 2026-09-29, medium confidence): T6.b measures the incision it removes before fixing
    it, and only channels under about two band limits are cut on resolution alone. Three of these
    depart from the brainstorm's [per-query
    evaluation](../../brainstorming/rendering-and-planets.md#the-per-query-evaluation), and are for
    its revision: the first level follows the coarse `flow` rather than joining "its lowest Moore
    neighbour under the control function"; the later search is 3 × 3 rather than "at least a 5 × 5
    neighbourhood", since a level-k segment ends at most two cells from its key point and a
    bounding-box test on the cached segment prunes the rest; and "several instances stacked" are one
    network with more levels, since a stacked instance's first level joins the coarser network's
    segments exactly as a later level does. _Craters_ below D_b: per octave at a quadtree level
    whose cells are at least the octave's reach, a 3 × 3 search across face edges, counts from the
    body's density (`FieldHeader.craters`, one function for the whole body, so the draw reads no
    coarse cell) at the cell's canonical point weighted by its true area and capped at saturation,
    diameters by inversion. The profile is volume-balanced in closed form: interior −d₀ + (H + d₀)
    r², exterior H (1 − s)³(1 + 3s) with s = (r − 1) ÷ a, which balance when d ÷ H = 2 + 1.6a +
    0.4a². The fresh bowl's d ÷ H = 5.42 (Design note 12) gives a = 1.54, ejecta to 2.54 rim radii.
    - Every other shape solves its own a from the same balance in closed form. Its interior is a
      flat floor to r_f, a wall −d₀ + (H + d₀) u² with u = (r − r_f) ÷ (1 − r_f), and a central
      cone; then a = (−0.8 + √(0.64 + 0.8 N)) ÷ 0.4, where N = −(V_int + V_peak) ÷ (π R² H).
    - That gives 1.28–1.46 for transitional and complex craters to 25 D_t (1.05 at 100 D_t), and
      0.73–1.54 for an airless regolith's bowls.
    - None exceeds the bowl's 1.54, so 2.54 rim radii is the reach of every crater.
    - The bowl's and the complex craters' a lie within 15% of the observed continuous-ejecta edge,
      1.35 rim radii beyond the rim (Moore et al. 1974 in Pike 1980b, Table 6).

    The rim is
    band-limited by a cubic Hermite fillet over the level's spacing or the crater's diffusive age,
    √(2κt), whose volume change a compensating (1 − (r/r₁)²)² term cancels, so every crater
    integrates to zero at every level. Every contribution is attributable to a level; the set per
    level is fixed per body from the level's largest cell. The renderer's morph blends level n with
    the parent band carried in the patch, so the height function itself is discrete per level.

14. **The cost** (researched 2026-09-29; kernels measured under shared load, scaled by a guessed
    2–2.3× to a quiet machine; provisional throughout). The whole height function with its gradient
    is estimated at 3.5–6 µs a point native and 4–8 µs in wasm, inside the brainstorm's 10 µs, and a
    65 × 65 patch at 17–34 ms, inside its 40 ms, only with the channel levers of Design note 13: the
    unpruned fourteen-level network alone is about 4 µs native and 5–6 µs in wasm, the brainstorm's
    1–3 µs holding for one four-level network. T8's bench records the breakdown against these wasm
    targets: interpolation 0.2 µs, relief 1.5 µs, channels 3.5 µs, craters 2 µs. The fallbacks for
    the uncached single-point path are one Threefry block per 32 corners' nibbles, then
    Threefry2x64-13.
15. **The margin** (researched 2026-09-29). `SYNTHESIS_MARGIN_CELLS` is **5**: the interpolant reads
    up to four cells past a face edge and about five at a corner (ghosts three deep, each bilinear
    from one more cell, under the warp's 1.44× distortion); the first Dendry level reads flow
    directions up to four cells away; the largest local crater, just under D_b, reaches its ejecta's
    edge at 2.54 rim radii, 1.27 D_b, so ⌈1.27 × 84.9 ÷ 23.5⌉ = 5 cells on an Earth, and the same on
    a Mars, the Moon and Ceres, whose D_b scales with the edge. Small-crater counts read the body's
    density, not a cell (Design note 13), so the octave search's wider cells add nothing to it. It
    holds only because the first Dendry level follows flow directions rather than searching a 7 × 7
    neighbourhood (6 cells otherwise). The instrumented read-set test is T8's, over the whole height
    function once every contribution exists; T4 asserts the interpolant's share. At level 8 it is
    about 180 km of
    margin about a survey. Margin cells arrive whole and exact but are marked held, not surveyed:
    R10 draws only surveyed ground. Every coarse crater whose reach touches a surveyed or margin
    cell comes with the block.
16. **Coverage as survey passes.** A pass is one versioned JSON line in
    `knowledge/surveys.v1.jsonl`, beside P12.T7's `contacts.v1.jsonl`: body, time span, source
    (orbital, close range, landed), resolution in metres, and the cover as `cell_index` ranges at
    the field's level, a run-length form of open question 4's cover at the field's one level. A
    `survey_pass` request carries at most `MAX_SURVEY_RANGES` (256) ranges, which keeps its JSON
    under plan 04's 16 KiB inbound frame (about 30 B a range); a pass whose cover needs more is
    recorded by several requests sharing its time span, each a line of the log. On load the server
    folds the passes into a `ResolutionCode` byte per cell, 0 for none and otherwise max(1, ⌈8
    log₂(r ÷ 1 cm)⌉ + 1), so a pass at 1 cm or finer codes 1, spanning 1 cm to about 36,000 km,
    keeping the finest; 384 KiB at level 8, never stored. Coverage gates which cells are sent (any
    pass); the resolution travels with each range of the cover, and gates what a readout may quote
    (R10, through `unresolved_rms` and `structure_function`). Until sensors exist, a pass is
    recorded by the `survey_pass` request, the stand-in plan 12's design note 9 uses for its
    client-set observer; the core, `SurveyLog::record`, is what the sensors plan will call, and it
    is the only way coverage grows.
17. **Quantisation is the input, and the payload.** The pass works in `f64` and ends by quantising
    into `CoarseField`; the server's own synthesis and collision read that quantised field, never
    the working state, so the client's inputs are identical by construction, as the brainstorm
    requires. Elevation is in millimetres; `boundary_distance` is in kilometres, saturating at
    ±32,767 km, since Earth's plate interiors reach about 4,000 km from a boundary and a 2 R⊕
    world's about 8,000 km (researched 2026-09-29, from Bird 2003's boundaries, medium confidence);
    `month_anomaly` is an `i8` in steps of the header's per-body `anomaly_step`, 0.25 K × 2ⁿ, the
    finest step that holds the body's largest anomaly in 127 steps: 0.25 K on an Earth, whose
    extreme is Verkhoyansk's −30.8 and +30.6 K about its annual mean of −13.9 °C (WMO 1991–2020
    normals, station 24266; Oymyakon, 24688, gives −30.8 and +30.2 K), within the 31.75 K that 127
    steps of 0.25 K hold, 0.5 K on a Mars, whose high latitudes reach ±45–60 K, and 1 K on a
    high-obliquity world with land,
    ±50–70 K (Williams and Pollard 2003; Williams and Kasting 1997; researched 2026-09-29, medium
    confidence for Mars and high obliquity), so no anomaly saturates and no cell grows. The climate
    layer is at level L − 1, because it is interpolated from the energy-balance grid and carries
    nothing finer except the lapse term, which the synthesis re-applies from the header's rate and
    the cell's elevation. That gives about 25 B a cell (21 B, T2's follow-up B's substance byte,
    `decision-r09-t5.md`'s sea-floor byte and follow-up C's two runoff bytes) plus 50 B per four
    (twelve months of anomaly, precipitation and wind): some 14.7 MB for an Earth at level 8,
    3.7 MB for a Mars at 7 and 0.94 MB for the Moon, inside the brainstorm's "roughly 2 to 15 MB"
    and R03's 15 MiB check.
    Heights, elevations and sea level are measured along the normal of the body's
    rotational spheroid, R07's reference body (a = R_vol (1 − f)^(−⅓), c = a (1 − f), with plan 14's
    flattening f, which P14.T46 sends in the record's `figure()`), the one datum R07's Design note
    19 sets for R05's vertices, R07's discs and R10's terrain (researched there, high confidence);
    the header carries a and c. The payload is a sequence of self-contained blocks of at most
    `MAX_BLOCK_BYTES`, in `cell_index` order, so a worker can be posted one block at a time; block 0
    of every payload, delta or whole, carries the whole `FieldHeader`, so that a worker can build
    its `PartialField` from the first block that arrives, and `SurfaceFieldDto`'s header is the same
    header for the main thread; R03 cuts the payload into its 256 KiB frames and the client parses
    it once complete. The format has a version, and the format, its scales and the synthesis all
    belong to `GENERATOR_VERSION` (open question 4).
18. **What R10 takes.** The material class function and its weights, over `FieldView` and the
    gradient, and the albedo scale, which R10's `albedo_scale` computes on the server beside the
    field build and this plan's header carries (`albedo_scale`, `None` until R10 fills it). _As
    built by R10.T10.a (main's rulings, 2026-10-14): the server's `service::compute` sets `Some(c)`
    for a body whose photometry is modelled, whose surface pressure is below 30 kPa, airless
    included, and whose photometric law has a ground term; `None` means no scale, the palette's
    albedos as resolved (c = 1), and is sent where the photometry is not modelled, where the air
    hides the ground (30 kPa and up, whose p is the cloudy disc's), where the law is a cloud deck's
    (a Lommel–Seeliger share of 0, Venus's template among them, at any pressure) and where the
    palette names no substance (R10's Risks, "Deviations in T10.a, as built")._ R10 also
    retires R05's provisional height function when it switches the client. `unresolved_variance`,
    `structure_function` and `unresolved_rms` are this plan's, because only the synthesis knows its
    band amplitudes: per contribution, craters and channels included, for bands finer than a
    wavelength, closed-form per cell, read through the `FieldView` because they depend on the
    cell's own fields. R11's rock abundance needs the surface age and the atmosphere, which both
    sides must hold, so the header carries plan 14's `surface_age` and `surface_pressure`, from
    which R11 derives its atmosphere class.
19. **The service.** `SurfaceService` keys fields by (universe `GalaxyKey`, `BodyId`), holds
    `Arc<CoarseField>` in a `SharedByteLru` bounded by `HYPERION_SURFACE_CACHE_MB` (for context,
    the development machine that hosts the server in testing has 32 GB of RAM), fills it under
    `SingleFlight` at `Priority::Bulk` with a `CancelToken`, and is never persisted (open question
    4). A field is started by the first `surface_field` or `survey_pass` for the body and by
    `SurfaceService::prefetch`, which session code will call on approach. `surface_field` answers
    through R03's `Answer { body, bulk }` once the field exists, the request's in-flight slot held
    until the terminal response. A client that holds revision n asks with `have_revision`, and
    receives only the cells surveyed since. Every surveyed cell is sent, since at most about 12 MB
    they need no bounding by R03's camera reports; R03's Design note 6 expected them to bound the
    field's cells, and R03 records that this plan declines that input (its Design note 6 and its
    Risks, "Cameras bound little"), so the reports bound only the sensors plan's contacts. Tests
    reach the service through its `InputsSource` seam with the reference worlds, because
    `for_body` answers `NotModelled` until plan 14's asks land; the wire then answers
    `SurfaceFieldDto::NotModelled { section }`.

## Tasks

T0 first. T1–T3 are the base: seeds, then types and codec; T2 needs nothing of T1 and may run
beside T1.a. T4–T9 build the synthesis on T2 (T5, T6 and T7.b open their streams from T1.a's
`DetailSeed`); T10–T16, the coarse pass, need T1.a and T2 and may run beside the synthesis only up
to the points where they read it: T12.d (coarse craters) needs T7.a's density and T7.e's target;
T12.e (σ_h on the reconstructed field) needs T4's interpolant and T5's `local_variance`; T14.b (the
multigrid's upsample) needs T4; and T14.c (the Mars volume check) needs T6.b's closed-form
incision. T17–T19 are the server: T17 needs T3 and T10, T18 needs T2 (P12.T7 is built), and T19
needs T8, T17 and T18.
T7.d (the cold-trap fraction, a pure closed form) needs nothing and is built early, since T13.f
and galaxy plan 14's P14.T51.f both call it. T20 closes. T0.b's research items gate T7.a, T12.c,
T13.a and T14.b. Every task ends with `just ci` green, which the orchestrator runs on
integration; a lane runs its targeted checks.

Paths are under `crates/hyperion-surface/src/` or `crates/hyperion-sim/src/` as the task says.
Every figure a task turns into a constant is re-checked against the source its design note names
and cited in the doc comment. Every timing is measured on a quiet machine (no other agents' test
runs; the governor recorded), since the research's figures were taken under load.

### R09.T0 Reconcile and ask

- **R09.T0.a Interfaces and asks.** Check every Consumes item against the code (R04's base crate,
  registries and probe, R05's `cube`, `test_planet` and binding, R03's `Answer` and frames, P12.T7,
  plan 14's records). Write the five asks of Design note 3 into plan 14's text as a named block, the
  `surface_revisions` field and the note that this plan declines the camera reports' bound
  (Design note 19) into R03's, and the `surface_field` and `survey_pass` rows into plan 04's
  reserved-kinds table, each for its owner to accept. Acceptance: `npx prettier --check` on the
  edited plans; `grep -c "R09.T0.a" docs/agent/plans/galaxy-generation/14-planetary-systems.md`
  finds the block; each ask in this plan's Risks carries its state word (drafted, carried or open).
  _Done 2026-10-09: the record is Risks' "Re-validated at `bce2aef5`"; the block is plan 14's
  "Phase K" (P14.T48.a–e). The check ran as `pnpm exec prettier --check`, the same tool, since this
  machine has no `npx`._
- **R09.T0.b Remaining checks.** The research of 2026-09-29 answered the plan's questions; what it
  left open goes to a research agent before the constant it concerns is committed: the σ_h fit's
  age law against Mercury's smooth and intercrater plains (the same pyshtools script, masked), before
  P14.T48.a and before T10's reference worlds take a σ_h from the fit; the energy-balance transport
  cap against the FILLET ensemble, before T13.a; a geomorphology pass on erodibility for Titan and
  Mars (Collins 2005; Howard 2007) before T14.b; Venus's screening parameters against Herrick and
  Phillips 1994's size–frequency distribution before T7.a. The review of 2026-09-29 added the
  transport cap's rule (Risks) before T13.a and the isotherm of T_e (Risks) before T12.c, and the
  re-validation of 2026-10-09 the law of the continental fraction f_c, which P14.T48.a asks plan 14
  to publish and Design note 3 calibrates on Earth alone (0.405), before P14.T48.a. Acceptance:
  `grep -n "researched"` on this plan shows each item in its design note, or the item stays in
  Risks with the agent's lean. _Done 2026-10-09, each item "researched 2026-10-09" in its note:
  the age law, the isotherm of T_e (870 K) and f_c's law in Design note 3; the FILLET check and
  the transport law in Design note 8; erodibility in Design note 9; Venus's screening in Design
  note 12. The residuals and the ruling asked on open question 20 are in Risks ("Remaining checks,
  as researched"). T7.a, T12.c, T13.a, T14.b and P14.T48.a no longer wait on T0.b. Plan 14's Phase
  K draft is amended to match (f_c's law, the isotherm, `effective_flow`'s definition)._

### R09.T1 Seeds, scopes and tags

- **R09.T1.a Scopes, seeds, keys and tags.** `TagScope::{SurfaceCoarse, SurfaceDetail}`,
  `SurfaceSeed`, `DetailSeed`, `ObjectKey::surface_cell` and `surface_item` in the base crate's
  `rng`; the eight `surface.*` tags in `hyperion_surface::tags`, each with its one key form (the
  Key column of Provides); `body.surface.detail` in the sim's registry. Needs nothing of plan 14.
  Tests: `Stream::open` panics on either surface scope (with `expected`, in base's
  `tests/panics.rs`); each seed opens only its own; packing
  round-trips and refuses a level above 28 or a coordinate past 2^level; two cells never share a
  key; each surface tag's documented key form is the only one its call sites use (a test over the
  registry's doc table); `assert_registries_disjoint` covers the new names; `tags.golden`
  regenerated with `domain_tags_are_pinned`. As built before it (the re-validation of
  `bce2aef5`): `SurfaceSeed` moves from the sim's `planetary/hooks/seed.rs` into base's `rng`,
  re-exported by the sim at its old path, with P14.T23's tests unchanged and passing;
  `Stream::open`'s scope assertion (`rng/stream.rs`) gains the refusal of both surface scopes; the
  keying tables of `rng/key.rs` and `rng/mod.rs` gain the two key forms; `body.surface.detail` is
  appended at the end of the sim's registry and the eight tags after R05's entry in the surface
  crate's; the tag golden is the sim's (`crates/hyperion-sim/tests/golden/rng/tags.golden`), which
  only gains lines, so no `GENERATOR_VERSION` bump (`golden_diff.py` reports it "Extended only").
  Acceptance: `cargo test -p hyperion-base rng`, `cargo test -p hyperion-base --test panics`,
  `cargo test -p hyperion-surface tags` and
  `cargo test -p hyperion-sim --test foundation_golden domain_tags`.
- **R09.T1.b The detail seed on the wire.** After T1.a; P14.T23 is built (`hooks/seed.rs`,
  `surface_seed`, `PlanetarySystem::{surface_seed, hooks_at}`). `planetary::hooks::detail_seed` in
  `hooks/seed.rs` beside `surface_seed`, and the record's hooks section inhabited: `record::Hooks`,
  an empty enum today, holds the body's `DetailSeed` (and nothing server-only), the record builder
  sets it `Ok` for a present body at `DetailLevel::Full` (`RecordSection::Hooks`'s level), and the
  server's `convert::planetary::hooks` maps it to `BodyHooksDto { detail_seed: ok }`, turning R04's
  `not_modelled` section `ok`. That moves P14.T32's golden systems (`hooks: NotModelled` on every
  present body), so T1.b is built in galaxy plan 14's 21 → 22 batch, by its lane (`p14-batch`) at
  any point before P14.T48.e (decision-composition §8.2 item 10), and lands under that batch's one
  `GENERATOR_VERSION` bump, by the sim-determinism skill, not alone or with T9 or T16.
  This plan does not build `surface_seed`; the coarse pass takes its `SurfaceSeed` from the builder
  in every test. Tests: the detail seed of 10⁶ bodies has no duplicates and differs from the surface
  seed of each; the hooks section's wire form carries `detail_seed` and no `surface_seed`; a ring,
  a belt or an absent body keeps `not_applicable`; `tests/system_bodies.rs`, which asserts today
  that every hooks section is not modelled, asserts the detail seed instead. Acceptance:
  `cargo test -p hyperion-sim planetary::hooks`, `cargo test -p hyperion-protocol planetary` and
  `cargo test -p hyperion-server --test system_bodies`.

### R09.T2 The field's types

`field.rs`: `CoarseLevel`, `coarse_level`, `boundary_diameter`, `cell_index`, `FieldHeader`,
`SynthesisCell`, `ClimateCell`, `CoarseCrater` with its `reach`, `CoarseField` with its per-cell
crater index, `FieldView` for `CoarseField`, `Cover`, `ResolutionCode`; `testing::{FieldBuilder,
SyntheticWorld, synthetic_field}`, the synthetic worlds T3 to T9 test on (an Earth-, Mars-, Moon-
and Ceres-like field built directly, a flat field and a single crater), each field valid by
construction. With them (the re-validation of `bce2aef5`): the header's data types, which later
tasks give behaviour (`synth::{BandLevel, BandSpectrum}` for T5, `craters::CraterParams` for T7.a,
`ClimateModelKind`, `PrecipitationSource`); `PerSquareKilometre` and `SquareMetres` in base's
`units`, by its `unit!` macro; and the surface crate's first `[features]` entry, `testing = []`,
which the sim's `testing` feature enables (`hyperion-surface/testing`). The climate record holds
twelve winds, and the header holds the months' orbit's eccentricity with `month_blend`
(`decision-r09-t2.md` item 1). `month_blend` is pure arithmetic:

- it finds the month of a mean anomaly by solving Kepler's equation in a fixed number of Newton
  steps;
- it blends linearly in mean anomaly between the two nearest month centres, each centre being its
  span's mean-anomaly midpoint.

Tests: the level table of Design note 4 (Earth 8, 2 R⊕ 8, Mars 7, Moon 6, Ceres 5, with the
brainstorm's cell sizes within 5%); D_b equal to twice the closed-form largest edge, 84.9 km ± 0.5
on an Earth at level 8, and 90.3, 92.6 and 50.0 km on a Mars, the Moon and Ceres ± 0.5;
`cell_index` is a bijection onto 0..6 · 4ᴸ; `craters_reaching` yields exactly the craters whose
`reach` holds the cell, in list order; every synthetic world is built twice with identical values;

- on e = 0 every month is a twelfth of the period;
- on e = 0.6 the months' shares sum to 1 to 10⁻¹², month 0 is (π ÷ 6 − 0.6 sin 30°) ÷ 2π of the
  period, and month 6 is (π ÷ 6 + 0.6 sin 30°) ÷ 2π;
- `month_blend` is continuous across every month edge and gives the month alone at its centre;
- a one-month year's `wind[1..]` are calm, and `CoarseField::new` refuses a wind there.

Acceptance: `cargo test -p hyperion-surface field`.

_Follow-up B, the palette and the substance byte_ (decision-composition, 2026-10-09; after
decision-r09-t2's follow-up, which edits the same files).

- **`hyperion_surface::substance_key::SubstanceKey`**: plan 14's substance key (P14.T49.a). ASCII,
  1–16 bytes, NUL-padded; a formula in chemical case, `e-`, or a lowercase name. It has `new`,
  `as_str` and a `const` constructor that fails to compile on an invalid key. Whichever of this
  follow-up and P14.T49.a is first creates it.
- **`field/palette.rs`:**
  - `PaletteRole` (`u8`, append-only): `PrimaryCrust` 0, `SecondaryCrust` 1, `TertiaryCrust` 2,
    `Province` 3, `Ice` 4, `Liquid` 5, `Cover` 6, `Deposit` 7; `Bulk` 8 (an undifferentiated
    body's own material, not a crust), appended by follow-up D;
  - `MechanicsFamily` (`u8`): `Silicate` 0, `Metal` 1, `WaterIce` 2, `VolatileIce` 3, `Salt` 4,
    `Organic` 5;
  - `PaletteEntry { substance, role, normal_albedo_bvr: [f64; 3], phase_row: u8,`
    `grain_density_kg_m3: f64, transition: Kelvin, mechanics }` (renamed by follow-up C), every
    value resolved by the server from plan 14's registry, so that the field stays the client's only
    input (Design note 17);
  - `MaterialPalette`, at most 15 entries, sorted by (role, key).
- **`FieldHeader`** gains `palette`, `crust_palette: [Option<u8>; 4]` (the entry of each `Crust`
  variant, whose lithology it names) and `main_liquid: Option<u8>`.
- **`SynthesisCell`** gains `substances: u8`: the ice entry in the high nibble and the liquid entry
  in the low, `0xF` none. The record goes from 21 to 22 bytes, 0.39 MB more on an Earth at level 8.
- **`SurfaceClass`'s codes:** 0 unclassified, 1–63 Köppen–Geiger, 64–255 surface-state forms
  whose substance is the cell's palette entry (T15 assigns them).
- **`CoarseField::new` refuses:**
  - a palette over 15 entries, or one not sorted;
  - an index past it;
  - an ice share without an `Ice` entry, or a cell under liquid without a `Liquid` entry;
  - a `crust_palette` entry whose role is not a crust's.
- **The synthetic worlds carry palettes:**
  - Earth: `basalt`, `granite`, `H2O` as ice and as liquid;
  - Mars: `basalt_fe`, `mars_dust` (deposit), `H2O` and `CO2` (ice);
  - Moon: `feldspathic` (primary), `basalt_fe` (province), `H2O` (ice);
  - Ceres: `phyllosilicate` (primary), `Na2CO3` (deposit), `H2O` (ice).

  Keys are checked by grammar until P14.T49.b's rows exist, when P14.T49.b's test checks every key
  these worlds use. Mars's and the Moon's rows are as follow-up D makes them
  (science-composition-crusts.md §1.5 (e)); B built `basalt` for both and `anorthosite` for the
  Moon's primary.

- **The shape is frozen only from R09.T19's first send.** R10's and R11's re-validations may add
  members before then, re-blessing `tests/golden/wire/` at the current version with neither
  `SURFACE_PAYLOAD_FORMAT` nor `GENERATOR_VERSION` bumped; from T19's first send a change bumps
  both (Generator version, the ruling of 2026-10-09, which governs; corrected 2026-10-10 from
  "under a `SURFACE_PAYLOAD_FORMAT` bump"). R11.T8's liquid members are ruled in
  `decision-r11-palette-liquids.md` and built by follow-up E (R09.T2.e), with every other R10 or
  R11 ask on the payload before the gate.
- Tests: the palette's bounds, order and refusals; every synthetic world valid; `SubstanceKey`'s
  grammar (formula, `e-`, name, and rejections).
- Acceptance: `cargo test -p hyperion-surface field`;
  `cargo test -p hyperion-surface substance_key`.

_Follow-up C, runoff, grain densities and the read solidus_ (science-r09-drainage.md §1–§5).

- **Two new bytes:** `SynthesisCell` gains `runoff` and `catchment_runoff`, both `LogRunoff`. The
  record goes from 23 to 25 bytes (the ruling's 22 to 24, before `decision-r09-t5.md`'s sea-floor
  byte), 0.79 MB more on an Earth at level 8.
- **Docs:** `LogArea`'s and `LogSteepness`'s documentation say they are geometric.
- **The codec:** the two bytes follow `steepness`.
- **`PaletteEntry::density_kg_m3` becomes `grain_density_kg_m3`,** a grain density (§3).
- **B's synthetic rows:**
  - basalt's solidus becomes 980 °C (§2);
  - each density becomes a grain density;
  - Ceres's phyllosilicate takes `PrimaryCrust` (§5);
  - the Moon gains `H2O` as `Ice` (§4).
- **Synthetic routing:** `Grid::route` accumulates the equivalent area beside the geometric one
  from a per-world runoff:
  - the Earth-like world: 0.4 of its annual precipitation (about Earth's ratio of global runoff to
    land precipitation, about 0.36, from memory, labelled), so that its rain shadows give r̄ ≠ r;
  - the Mars-like world: a uniform 0.1 m a⁻¹;
  - every other world: none.
- **The sim:** its quantiser writes the bytes from two new `CellState` members, none in
  `Working::initial`.
- _Tests:_
  - each `LogRunoff` code round-trips;
  - catchment runoff × drainage, summed at each confluence, equals the cell's runoff times its own
    area plus its donors' to the steps;
  - on the Mars-like world every routed cell's two runoffs are equal;
  - the synthetic Earth has a cell whose catchment runoff is at least three times its own.
- _Goldens:_ `payloads.golden` is re-blessed at 21 (allowed before T19's first send), and
  `codes.golden` gains the `LogRunoff` rows.
- _Accept:_ `cargo test -p hyperion-surface field`; `cargo test -p hyperion-surface wire`;
  `cargo test -p hyperion-sim planetary::surface`.

Follow-ups A–C landed under R09.T2; D and E take R09.T2.d and R09.T2.e by their letters, and
R09.T2.a–c are not used (main's ruling for R09.T2.e, 2026-10-15).

- **R09.T2.d Follow-up D, an undifferentiated body's role and the crust ruling's synthetic rows**
  (science-composition-crusts.md §1, §3, §4, §7 and §8, 2026-10-11; after follow-up C). One lane,
  in the surface crate: it touches `field/palette.rs` and `testing/worlds.rs`, not R09.T10.c's or
  R09.T10.d's files, and the ruling's §1.6 and §3.3 hold its code text.
  - **`PaletteRole::Bulk = 8`**, appended: an undifferentiated body's own material where no crust
    covers it, its accreted, never-melted rock or ice–dust, as a chondrite or a comet's dust mantle
    (or, on a fragment of a differentiated body, its exposed mantle or core). It is not a crust in
    Taylor's sense (§1).
  - **`is_crust`** takes it: `PrimaryCrust | SecondaryCrust | TertiaryCrust | Province | Bulk`,
    documented as "Whether the role is one a [`Crust`](super::Crust) variant's entry may take: the
    three crusts, a province's and an undifferentiated body's bulk."
  - **`grain_density_kg_m3`'s documentation:** "an ice's crystal density" reads "an ice's crystal
    density at the temperature where it lies on the body" (§3).
  - **B's synthetic rows** (`testing/worlds.rs`), as R09.T10.c and T10.d take them into the
    reference rows:
    - the Moon-like world's primary crust is `feldspathic`, renamed from `anorthosite` and keeping
      its 2,900 kg m⁻³, the highland crust's grain density (§7); its province is `basalt_fe`;
    - the Mars-like world's secondary crust is `basalt_fe` (§2);
    - `basalt_fe` is an iron-rich basalt, 3,310 kg m⁻³ and 1,050 °C (§2);
    - the solidi (§4): `feldspathic`'s 1,260 °C, a plagioclase rock's first melt; the
      phyllosilicate residue's 1,150 °C (1,423.15 K), an upper bound; and the dust's 1,050 °C,
      `basalt_fe`'s (still an assumption);
    - `Na2CO3`'s melting point is 850 °C (NIST-JANAF), its 2,540 kg m⁻³ confirmed (§4);
    - each water-ice entry's density is ice Ih's ρ_s where the ice lies (§3; IAPWS R10-06): the
      Earth- and Mars-like worlds' at their coldest zones, and the Moon- and Ceres-like worlds' at
      the 110 K cold-trap threshold, 932.7 kg m⁻³.
  - _Tests:_ `field_palette_crust_roles_are_the_crusts_and_the_provinces` expects the five roles;
    every synthetic world is valid, with the rows above.
  - _Goldens:_ `payloads.golden` is re-blessed at 21 and `codes.golden` gains `Bulk`'s
    `palette_role` row, an extension (codes only gain), before T19's first send. Neither
    `GENERATOR_VERSION` nor `SURFACE_PAYLOAD_FORMAT` is bumped (Generator version).
  - _Accept:_ `cargo test -p hyperion-surface field`; `cargo test -p hyperion-surface wire`.

- **R09.T2.e Follow-up E, the palette's liquid record and the reserved header parts**
  (`decision-r11-palette-liquids.md` §1, §2 and §4, 2026-10-14; lane r09-palette-liquids; after
  R10.T10.a). The layout of the gate's set, before the first send, so a re-bless at the version in
  force with no bump (Generator version).
  - **`field/palette.rs`:** `LIQUID_BINS` (15, R08's bake bins), `LiquidBasis` (`coded_enum!`,
    `Measured` 0, `Derived` 1, `StandIn` 2) and `LiquidProperties` (§1.1's eight members, each
    with its unit, range and source rule); `PaletteEntry.liquid: Option<LiquidProperties>` with
    §1.3's presence rule, and a `Liquid` entry's `normal_albedo_bvr` documented as the record's
    Fresnel reflectance at normal incidence (§2, item 7); `MaterialPalette::new` refuses a `Liquid`
    entry without a record, a record on an `Ice`, `Cover` or `Deposit` entry, a `Liquid` entry
    whose grain density is not its record's density to the bit, and a member out of range (§1.6).
  - **`field/header.rs`:** `FieldHeaderParts` gains `maturity` and `organic_cover`, each
    `Option<f64>` in [0, 1], after `main_liquid`, with getters and `FieldBuilder`'s setters (§2,
    item 4).
  - **The codec:** the entry's table takes the non-fixed form with the record's presence byte and
    members (§1.5); `LiquidBasis` joins `wire_coded!`; `MaterialPalette::read`'s `need` takes the
    60-byte minimum entry; the header's table gains the two parts.
  - **Values:** the header's `surface_pressure` is exactly 0 for an `Airless` body (§2, item 5);
    the synthetic and reference worlds' liquid entries carry records entered by hand with their
    sources (§4).
  - **The protocol and the server:** `PaletteEntryDto` gains `normal_albedo_bvr`, `phase_row` and
    `liquid: Option<LiquidDto>`; `LiquidDto` and `LiquidBasisDto` are new; `SurfaceHeaderDto` gains
    `main_liquid`, `maturity` and `organic_cover`. `convert/surface.rs` fills them; the client's
    coarse-field store range-checks them.
  - _Goldens:_ `payloads.golden` is re-blessed at 22, with a new `Liquids whole` line;
    `codes.golden` gains `LiquidBasis`'s rows and the phase-row codes, an extension; and
    `worker/moon_like.golden`'s block-0 lines move with the layout (§4: "every other surface-crate
    or sim golden that encodes a palette").
  - _Accept:_ `cargo test -p hyperion-surface field`; `cargo test -p hyperion-surface wire`;
    `cargo test -p hyperion-surface testing`; `cargo test -p hyperion-surface lava_world`;
    `cargo test -p hyperion-sim planetary::surface`; `cargo test -p hyperion-protocol surface`;
    `cargo test -p hyperion-server surface`; `just gen-protocol-check`; `just smart-ci`.

### R09.T3 The payload codec

`wire.rs`: little-endian blocks, each with a header (magic, `SURFACE_PAYLOAD_FORMAT`, generator
version, body, level, block index and count), block 0 also carrying the whole `FieldHeader`, the
cover of the block's surveyed and margin cells with their resolution codes, their records, and the
craters that reach them; the palette (a count, then fixed-size entries) and each cell's substance
byte, as rows of the table-driven layout (decision-composition); `encode_payload` splits at
`MAX_BLOCK_BYTES` in `cell_index` order, taking
the surface crate's `Cover` (the server converts its `Coverage`); `decode_payload` returns errors,
never panics; `DecodedBlock`, and `PartialField` with `insert` and its `FieldView`, which rebuilds
the per-cell crater index as blocks arrive.

Tests: round trip of a synthetic field to identical bytes and identical `FieldView` answers; a
`PartialField` built from block 0 alone carries the field's header, delta payloads included; a
`PartialField` answers `None` outside what it holds; the blocks inserted forwards, backwards and in
three fixed permutations give `PartialField`s whose every `FieldView` answer is equal; a truncated,
oversize,
wrong-version or wrong-body block is refused with its error; no block exceeds 1 MiB; the Earth-sized
synthetic field encodes to 10–15 MB, recomputed with the 22-byte synthesis record and
decision-r09-t2's climate record. Acceptance: `cargo test -p hyperion-surface wire`.

### R09.T4 Interpolation and base elevation

`synth/interp.rs`: Design note 13's per-face B-spline over ghost cells, its partition of unity and
its analytic gradient; categorical reads; `Synthesiser::height_at` returning base elevation at every
level, `QueryHeightError::NotSurveyed` when a read cell is not held (`QueryHeightError` implements
`std::error::Error + Send + Sync`, R05's `HeightSource::Error` bound). `HeightSample` moves from
R05's `test_planet.rs` to `height.rs`, which `test_planet` and `patch` then import or re-export, so
that R10's retirement of the test planet leaves it in place; `LatticeCache` already lives in `noise`
and stays (the re-validation of `bce2aef5`). Every surface golden is unchanged.

Tests: along every face edge and at all eight corners, values and gradients evaluated from either
side's patches are equal bit for bit; the gradient matches a central difference to 10⁻⁶ relative;
the field is C¹ (not C²) across face centre lines, as the warp allows; a constant field interpolates
to itself and no value leaves the cells' range; the interpolant's instrumented read set never
reaches beyond `SYNTHESIS_MARGIN_CELLS` from the query's cell (the whole function's is T8's).
Acceptance: `cargo test -p hyperion-surface synth::interp`.

### R09.T5 Structural octaves

`synth/relief.rs`: the octave stack on `surface.relief` keyed by lattice corner (the Key column of
Provides), over R05's noise basis with its 16-entry gradient table and certified bound, conditioned
on crust and boundary (ridged with its pinned mean removed, low-amplitude, domain-warped from
coarser octaves only); `BandSpectrum`; `local_variance`; the per-contribution
`unresolved_variance`. R05's `noise::Octave` holds a `Seed` and opens its corners and offsets on the
test planet's `selftest.surface.test_planet` tag alone, and `LatticeCache` keys its octave table
and corner boxes by that `Seed`, so T5 first generalises both to a noise key that is either the test
planet's (`Seed` on its tag) or this plan's (`DetailSeed` on `surface.relief`), with the test
planet's output bit for bit unchanged: every R05 golden and `TEST_PLANET_VERSION` (2) stay as they
are, and `just bench -- test_planet` is recorded before and after (provisional under load).

Tests: each octave's mean over 10⁵ points under 1% of its amplitude, ridged included; no sample
exceeds an octave's stated bound; `local_variance` within 5% of the sampled variance on each
synthetic world; a warp offset at level n never reads an octave finer than n (instrumented test).
Acceptance: `cargo test -p hyperion-surface synth::relief`.

_Follow-up, the spectrum's break and the sea floor_ (`decision-r09-t5.md`, 2026-10-10; before T8
and T9).

- `BandSpectrum` gains `break_degree` and `small_scale_exponent`, `SpectrumShape`, the class
  anchors and `anchored`.
- `SynthesisCell` gains the `seafloor` byte. The oceanic style reads it, and the arc bands are set
  by crust.
- The synthetic worlds take the anchored spectra and sea-floor bytes.
- `wire/form.rs`'s two rows change, and `payloads.golden` is re-blessed at 21 with `codes.golden`
  extended (Generator version).
- Tests: the closed forms against direct sums across the break; the anchors; the byte's codes and
  refusals; the styles; the 5% variance test on the new spectra.
- _And the belt's law_ (`decision-r09-t5.md` item 7): the ridged mix confined above the break; the
  belt factor g(λ)^b; the pinned unit-variance structure functions of the plain and ridged terms;
  `octave_bound` with the factor. The plain styles' heights do not move.

### R09.T6 Channels

- **R09.T6.a The first levels.** `synth/channels.rs`: the network's first level from the coarse
  flow directions and the next levels by nearest-segment joins on `surface.channel`, with graph
  neighbourhoods and 3D chord distances (Design note 13); profiles S = k_s A^−θ with Hack's law in
  SI, C = 0.320 m^−0.2 and h = 0.6 (Hack 1957, eq. 3, converted; flagged as Earth's), the field's
  drainage and k_s read as geometric; once follow-up C has landed, the tributary's k_s is the
  cell's k_s (r̄ ÷ r)^θ and a cell with no runoff carries no tributaries (Design note 13;
  science-r09-drainage.md §1), which T6.b applies if T6.a lands first. Tests: a
  10⁴ km² basin gives a main stream of 280–360 km; every segment descends to its parent within the
  minimum slope; the network leaves each coarse cell through the cell its flow direction names; a
  key point next to a face edge gives the same segments from either face.
- **R09.T6.b The full depth and the mean.** The network to about fourteen levels, with the 3 × 3
  search, cached bounding boxes and the level cut by channel width; widths and depths from
  discharge (Design note 13): a tributary's r A, from its cell's `runoff`, and a trunk's r̄ A, from
  its `catchment_runoff`, with the hydraulic-geometry relation and the conversion from mean runoff
  to channel-forming discharge stated with their sources by this task's science check; each
  level's incision has its mean over the parent cell subtracted; built per patch and cached by
  integer cell in `SynthCache`;
  the channels' `unresolved_variance` and `structure_function`. Tests: mean incision over every
  parent cell zero to 10⁻⁹ m; the cached build equals the single-point query bit for bit; cache
  order does not change a height; a tributary in a cell whose own runoff is a tenth of its
  catchment's is 10^0.45 = 2.82 times as steep as the same tributary where the two are equal (to
  10⁻¹²), and narrower by its discharge; a cell with no runoff carries no tributaries. Bench
  `surface/channels` with a per-level breakdown, against 3.5
  µs a point in wasm (Design note 14), in a new `crates/hyperion-surface/benches/synth.rs`
  (`[[bench]]`, `harness = false`, Criterion off the browser target as `test_planet.rs` is), which
  T8 extends.

Acceptance: `cargo test -p hyperion-surface synth::channels`.

### R09.T7 Craters

- **R09.T7.a The density and the shapes.** After T0.b's Venus item. `craters.rs`:
  `CraterParams` (T2's type, in
  `PerSquareKilometre`); `cumulative_density` from Neukum et al.'s
  a1…a11 with a0 = log₁₀ N(>1 km) (both published a0 recorded in the doc comment), the end slopes
  outside 10 m–300 km, the optional diameter map, and the screening taper on the differential
  production with exponent 4.5 (Design note 12; T0.b withdrew the break-up rule);
  `saturation`; `transition_diameter`; `diameter_in_octave` by bisection in log D with a fixed
  iteration count. `lunar_chronology(S, A)` and its inverse `chronology_age` (fixed 64-step bisection),
  counted from the system's formation, with `SOLAR_SYSTEM_AGE` 4.567 Gyr; `reference.rs`'s private
  copy calls it at `SOLAR_SYSTEM_AGE`, bit-identical. Tests: N(S; 4.567) equals the published curve
  to the bit; N is monotone in S; `chronology_age` inverts N to 10⁻¹² Gyr; a 10-Gyr system's 10-Gyr
  surface lies within 1% of the Solar System's 4.567-Gyr one (3.03 km⁻²); `CraterShape` and
  `crater_shape`: Design note 12's fresh depth, rim height,
  floor and peak by zone, and Design note 13's exterior width a by the closed-form balance
  (`decision-r09-t2.md` item 3). `testing::FieldBuilder`'s craters switch to it from T2's inline
  0.2 D and 0.84 (D_t ÷ 19 km) D^0.33. Tests: N(>1 km) is the parameter exactly; the function is
  monotone; the
  inversion lands in its octave and its quantiles agree with the density (Kolmogorov–Smirnov over
  10⁵ draws); the projectile scale d\* is 5.2 m, 0.52 km and 8.5 cm for Earth, Venus and Mars (at
  636 Pa, Design note 12), and the crater cutoffs 20 d\*; Venus's screened counts over its area
  at N(>1 km) = 3.07 × 10⁻⁴ km⁻² lie within 10% of the Gazetteer's at 3, 5, 10, 20 and 40 km
  (878, 850, 642, 335 and 104 named craters at or above each, Design note 12; the mapped check is
  T7.e's); the map at Mars's
  ratios (g 3.71, D_t 8.3 km, U = 16.7 × 9.6 ÷ 16.1 km/s, Ivanov's velocity ratio) reproduces
  Ivanov 2001's Mars function within ×1.5 in N over 1–100 km (0.97–1.03 computed by
  science-r09-craters.md), and P14.T48.f's 10.6 km/s within ×1.5 too (0.92–1.03); D_t gives
  Pike's Moon and Earth within 10%;
  - _The bowl._ It is 0.20 D deep with a rim of 0.0369 D and a = 1.54 to 10⁻³, within 12% of
    Pike 1980b, Stopar et al., Pike 1988 and Robbins and Hynek's deepest at 1–5 km. On an airless
    Moon its d/D is Stopar et al.'s 0.125, 0.152 and 0.166 at 63, 141 and 283 m.
  - _The Moon's complex craters._ Their depth lies between Kalynn et al.'s mare and highland fits
    at 30, 50, 100 and 150 km, and their rim height is within 1% of Pike 1980b's 0.236 D^0.399.
  - _Mercury_ (g 3.70). Depth is within 15% of Susorney et al.'s 1.02 D^0.20, and rim within 15%
    of their 0.25 D^0.28, at 30, 50 and 100 km.
  - _Mars_ (g 3.71). Depth is within 30% of both Robbins and Hynek's 0.250 D^0.527 and Tornabene
    et al.'s 0.323 D^0.538 at 25, 50 and 80 km. Rim is within 15% of Robbins and Hynek's
    0.025 D^0.820 at 50 and 80 km.
  - _Gravity._ In the complex range d ∝ g^−0.70 and h ∝ g^−0.60 at a fixed D, to 10⁻⁹. A bowl's
    d/D and h/D do not depend on g.
  - _Continuity._ d, h and the floor radius are continuous and non-decreasing in D across 0.8 and
    1.5 D_t.
  - _The balance._ Every shape's interior, peak and exterior sum to zero to 10⁻⁹ of its cavity
    volume. a ≤ 1.543 for every D ÷ D_t from 10⁻⁴ to 100, so the reach stays 2.54 rim radii.
- **R09.T7.e The target and the impactors** (science-r09-craters.md items 2 and 5, F1 and F2).
  After T7.a. `craters.rs`, `wire/form.rs`, `testing/worlds.rs`, `testing.rs` and
  `planetary/surface/reference.rs`.
  - `CraterParamsParts` gains `projectile_density`, `target: CraterTarget { class, density }` and
    `system_age`. `Screening::Atmosphere` keeps `column_mass` only, and d\* reads the top-level
    projectile density. `TargetClass` codes are NonPorous 0 and Porous 1.
  - The map takes the class constants of Design note 12, with exponents derived from μ.
    Porous-class bowls are 0.14.
  - The wire layout changes; `tests/golden/wire/` is re-blessed at the current version, as the
    Generator version section allows until T19.
  - The reference and synthetic worlds take the new fields:
    - Earth, Mars and the Moon are non-porous at 2,550 kg m⁻³; Ceres at Ermakov et al. 2017's
      1,287;
    - projectile density 3,000;
    - system age 4.567 Gyr;
    - impact velocities P14.T48.f's: Earth 19.9, Mars 10.6, the Moon 16.7 (the identity) and
      Ceres 5.6 km/s, as named constants citing it.
  - The contract's density is lunar-equivalent (F1). `cumulative_density` maps N☾ without
    renormalising at the body's 1 km, and `own_density_at_1km` gives the body's own.
  - The taper exponent is 4.2.
  - The reference Mars takes R_b 1.92 (0.4928 ÷ own/N☾ at Ivanov's velocities,
    science-r09-craters.md F4.1).
  - The central peak's onset and end ramps and Hale and Head's base (F2).
  - Tests:
    - the identity at the reference holds exactly;
    - the map is monotone for both classes;
    - at Charon (g 0.288 m s⁻², U 2.0 km/s, δ 920, ρ_t 920 and 644), a 10 km simple crater needs a
      1.37 km non-porous and a 1.59 km porous impactor to 1%, both within 15% of Singer et al.
      2019's worked example (1.3 and 1.8 km); the class constants are 3.764 and 4.239
      (science-r09-craters.md F4.2);
    - an icy target of 940 kg m⁻³ widens a crater by (3,000 ÷ 940 against 3,000 ÷ 2,550)^0.261
      to 10⁻⁹;
    - every porous bowl's a ≤ 1.543;
    - at the identity, N(>1 km) is the parameter exactly;
    - under Mars's map at Ivanov's 9.6 ÷ 16.1, `own_density_at_1km` ÷ N☾ is 0.254 within 2%;
    - Venus at N☾ 3.36 × 10⁻⁴, 25.7 km/s and 20 d\* gives the Gazetteer's counts at 3, 5, 10, 20 and
      40 km within 12%; under Mars's map at Ivanov's velocities, R_b × own/N☾ = 0.4928 within 1%;
    - d, h, the floor radius, the peak relief and a are continuous in D at 0.8, 1.5, 9 and
      13 D_t, to 10⁻⁹ relative across each bound;
    - the peak base never exceeds the floor;
    - a ≤ 1.543 over 10⁻⁴–100 D_t;
    - the peak relief is Pike's law to 10⁻⁹ over 1.5–9 D_t;
    - it is zero at and below 0.8 D_t and at and above 13 D_t.
- **R09.T7.b Small craters.** `synth/craters.rs`: octave levels, the 3 × 3 search across face edges,
  counts from the header's density at the canonical point by true area capped at saturation with
  ages from τ(D) through `chronology_age` at the contract's system age (Design note 12), morphology
  by the transition zone, the volume-balanced profile of each crater's `crater_shape` (its own a, at
  most 1.54), its fillet and compensation;
  their `unresolved_variance` and `structure_function`. Tests: the realised size–frequency of a
  sampled region matches min(production, saturation) (Poisson interval per octave); a crater
  straddling a face edge is found from both faces; each crater integrates to zero at every level to
  10⁻⁶ of its cavity volume (a quadrature test pins the fillet's closed form).
- **R09.T7.c Coarse craters' rims.** Sharpening the rims of the coarse list at finer levels. The
  coarse elevation already holds each coarse crater smoothed to the coarse cell, so level n adds
  only the difference between the profile band-limited at level n and at the coarse cell's
  resolution, both by the fillet of T7.b; that difference integrates to zero over each coarse cell,
  so the level-of-detail rule holds. Tests: a coarse crater's profile is continuous across the
  coarse cell edges it spans; the added difference integrates to zero over every coarse cell it
  touches, to 10⁻⁶ of the crater's cavity volume; at the coarse level it adds nothing.
- **R09.T7.d The cold-trap fraction** (a T7 follow-up, split out of T13.f and built early: the
  galaxy audit's B4; science-r09-drainage.md §4 and §8). It needs nothing of T7.a–c and can be
  built at once. T13.f and galaxy plan 14's P14.T51.f call it, so it lands before the 23 → 24
  batch.
  - **Where:** `cold_trap.rs` (declared in `lib.rs`), `hyperion_surface::cold_trap`. It is pure, in
    `f64`, beside the crater density.
  - **Species-general:** only the threshold, in kelvin, carries the species. The threshold rule is
    P14.T51.f's.
  - **Inputs** (grouped as the task chooses):
    - the latitude;
    - the star's greatest declination in the body's sky, in [0°, 90°] (the caller's
      min(ε, 180° − ε) of the obliquity to the seasonal orbit). It is eq. 26's δ, and e_max =
      min(90°, colatitude + δ) is derived from it;
    - the star's angular radius and flux, both at periapsis;
    - the Bond albedo, the emissivity and the internal heat flow;
    - the crater fraction and the craters' d/D, log-normal by mean and variance;
    - the plains' directional RMS slope σ_s;
    - the share of the cold-trapping area under lateral conduction's cut-off (lunar reference
      0.05, labelled);
    - the threshold.
  - **`permanent_shadow`** is the share of ground that no part of the star's disk ever reaches:
    - bowl craters by Hayne et al. 2021's eq. 26 (Nat. Astron. 5, 169), with δ plus the angular
      radius, over the log-normal d/D, with d/D above 0.5 counted as 0.5;
    - the plains as a population of bowls whose d/D is a half-normal of scale 0.6 × tan σ_s ÷ 2, 0.6
      of half the magnitude of a Gaussian surface's directional slope (science-r09-drainage.md §8.10
      and §8.13). The 0.6 is fitted to the permanent shadow of ray-traced Gaussian surfaces built as
      Hayne et al.'s Methods D builds them, since a bowl, a closed hollow, keeps several times the
      permanent shadow of a surface of its own mean-square slope. It is not a log-normal of the same
      moments, whose heavier tail would leave deep bowls in shadow equatorward of 70°. It is a
      labelled stand-in for Hayne et al.'s ray-traced plains, which have no closed form.
  - **`fraction`** is the part of that shadow whose Ingersoll, Svitek and Murray 1992 temperature
    (Icarus 100, 40) stays below the threshold, times one less the conduction share.
    - The star is taken at its flux-weighted elevation at e_max: e_max itself where e_max is at
      least the angular radius, and the visible segment's first moment below that.
    - The internal heat flow adds q (1 − (1 − ε) f) ÷ (ε (1 − f)) to σT⁴, exact in the same
      sphere.
    - The plains' shadows also absorb 0.0035 × S sin e from sunlit terrain beyond their hollows,
      which a bowl's sphere omits. It is fitted to the ray-traced surfaces' CO₂ cold traps at
      σ_s 5.7°, and moves their water cold traps by 1% (science-r09-drainage.md §8.13).
    - Equilibrium at e_max stands in for T_max. Its critical d/D at 110 K matches Hayne et al.'s
      no-conduction curve (their Fig. S8). The fraction's low bias against their Table 1 is mainly
      eq. 26's declination term, which removes shallow bowls' shadow; R09.T7.f replaces eq. 26.
  - It opens no stream, reads no field and moves no output: no golden, tag or version changes
    until a caller lands.
  - Its tests:
    - _Ingersoll's temperature._ At a lunar pole it is 97 K at d/D 0.2 (to 1 K). The heat flow
      alone gives 24.0 K on flat ground at 0.018 W m⁻² and ε 0.95 (to 0.1 K).
    - _The exact bowl._ The transect form matches a direct ray test of the bowl to 0.005 at d/D
      0.1 (sun at 2° and 10°), 0.2 (5°) and 0.3 (20°). The cap, the permanent shadow at most the
      instantaneous shadow at the highest sun, uses it.
    - _The plains._ Plains alone (no craters) under a point star 1.54° from the equator
      (science-r09-drainage.md §8.13):
      - their permanent shadow over 80–90° is within 20% of ray-traced Gaussian surfaces' 2.8%,
        23.6% and 34.8% at σ_s 5.7°, 16.7° and 26.6°; over 70–80° it is within a factor of two of
        0.08%, 7.8% and 18.8%, and over 60–70° at σ_s 5.7° at most 10⁻⁵;
      - their instantaneous shadow is 0.35–1.0 of Smith 1967's at sun elevations of 1–12°, wherever
        Smith's is at least 5%: the recorded cost of matching the permanent one;
      - under the real Sun (1,407.6 W m⁻², angular radius 0.2711°, heat flow 0.018 W m⁻²) at
        σ_s 5.7°, their cold traps over 80–90° are within 15% of the ray traces' 2.16% at 110 K,
        and within a factor of two of their 0.12% at 57 K;
      - their cold-trapping share over 80–90° peaks at a σ_s between 10° and 20°.
    - _Hayne's bands._ These run with a point star (angular radius 0, Hayne et al.'s Methods D),
      Hayne et al.'s best fit (20% craters, plains σ_s 5.7°), 110 K and no conduction share
      (science-r09-drainage.md §8.13).
      - With craters of distribution B (d/D mean 0.076, variance 2.3 × 10⁻⁴; Hayne's "σ" read as
        variances) and of distribution A (mean 0.14, variance 1.6 × 10⁻³), the cold-trap fraction
        is within a factor of two of Table 1 over 80–90° (6.7%) and the whole Moon (0.10%).
      - With both, the cold traps over 70–80° are between 0.016% and 0.14%: from half of Landis et
        al. 2022's Diviner-mapped 553 km² over 60–80° (0.032% of 70–80°) to twice that taken to all
        scales. Table 1's 7 × 10⁻⁴% multiplies its shadow by the cold share of all bowls, which at
        70–80° hold their shadow in the warm ones.
      - With B, the permanent shadow is within a factor of two of Table 1 over 80–90° (8.5%),
        70–80° (0.5%) and the whole Moon (0.15%); it is at most 0.01% over 60–70° and 0.001% over
        50–60°. Table 1's shadow is LROC's times f at d/D 0.1, which B's mean is nearer. A's is
        recorded in the Risks.
    - _Bounds._
      - Both functions lie in [0, 1], and the fraction never exceeds the permanent shadow.
      - Neither rises with the greatest declination or the angular radius, and the fraction does
        not rise with the flux, the heat flow or the conduction share.
      - The fraction does not fall with the threshold.
      - At a greatest declination of 30° the Moon-like fraction at 110 K is zero.
    - _No latitude cut._ A single bowl of d/D 0.25 at 60° (δ 1.54°) keeps a permanent shadow
      between 0.1 and 0.3 of its area, against exact ray geometry's 0.17. A d/D 0.1 bowl there
      keeps none.

- **R09.T7.f The exact bowl's permanent shadow** (a T7 follow-up; science-r09-drainage.md §8.12;
  decision-r09-t7f.md). After T7.d.
  - **The table:** a new `hyperion-fit` task, `bowl_permanent_shadow`, tabulates a spherical
    bowl's permanent shadow by exact ray geometry, as a share of the bowl's plan-view area.
    - A point is lit when its chord toward any position of the star's upper limb leaves through
      the rim. Over a year the star's disc sweeps every declination within D = δ + α of the
      equator, where δ is the greatest declination and α the angular radius. So one axis in D is
      exact.
    - The axes scale with the bowl's rim slope θr = 2 atan(2 d/D), which bounds its shadow:
      ln(d/D) over 0.02–0.5 (17 nodes), the colatitude ÷ θr over 0–1 (41) and D ÷ θr over 0–½
      (26), each uniform. That is 18,122 values, about 150 kB, under the 500 kB hook.
    - Its pole row is Hayne et al.'s eq. 6, x₀″² with x₀″ = sin(θr − 2D) ÷ sin θr (their eq. 3 at
      e = D), held at zero once x₀″ ≤ 0.
    - It is a fast task that reads only `math`, so its fingerprint is `none`. It is emitted with
      plan 15's manifest and staleness check, as a table or as a closed form that fits the table
      within 0.02.
  - **Where:** `crates/hyperion-surface/src/tables/bowl_permanent_shadow.rs`, in the surface
    crate's own `tables::MANIFEST`, since `cold_trap` reads it and the surface crate cannot see
    the sim.
    - `TableInfo` moves to `hyperion_base::version` and is re-exported at
      `hyperion_sim::tables::TableInfo`.
    - `hyperion-fit` gains `TableCrate::Surface` and a direct dependency on `hyperion-surface`.
      Its `no_runtime_crate_depends_on_hyperion_fit` reads `hyperion-base` and
      `hyperion-surface` too.
  - **Its use:** `cold_trap::permanent_shadow` and `fraction` read it in place of Hayne et al.
    2021's eq. 26, for craters and plains, trilinear in its axes. T7.d's cap stays as the
    physical bound.
    - Outside the table the shadow is zero. None survives once (colatitude + D) ÷ θr ≥ 1, where
      the highest sun clears the rim slope, or once D ÷ θr ≥ ½, where eq. 6's polar cap closes.
      Off the pole it closes sooner.
    - Below d/D 0.02 it reads the 0.02 slice at the bowl's own θr, since the shadow is
      self-similar in these axes as the slope falls. Above 0.5 it reads 0.5, as T7.d does.
    - Eq. 26 overstates deep bowls away from the pole, and its declination term removes shallow
      bowls' shadow. A d/D 0.1 bowl at 80° keeps 0.031 by eq. 26, against an exact 0.224.
  - **The plains** (science-r09-drainage.md §8.13): with the exact table, T7.d's half-normal
    plains kept 3.5 times a ray-traced Gaussian surface's permanent shadow over 80–90° at σ_s 5.7°
    and 12 times over 70–80°, and their shadows were too cold for CO₂. So T7.f scales the plains'
    bowls to 0.6 of half the surface's slope, and gives their shadows 0.0035 × S sin e from beyond
    their hollows, both fitted to ray-traced surfaces and labelled.
  - _Tests_ in `hyperion-fit`:
    - the committed table is what `run bowl_permanent_shadow --since <current>` writes;
    - read trilinearly, it matches direct ray tracing to 0.01 at D 1.54°:
      - for d/D 0.06, 0.1, 0.2 and 0.3, every 5° from the pole to 60°;
      - for d/D 0.5 at 45° and 30°, and d/D 0.4 at 50°, where deep bowls keep shadow past 45°
        colatitude;
    - it matches ray tracing to 0.01 at larger D:
      - at 4°: d/D 0.2 at 85° and d/D 0.3 at 75°;
      - at 20°: d/D 0.3 at 80° and d/D 0.5 at 60°;
      - and a hemisphere at the pole at 30° and 40°;
    - ray tracing finds no shadow at D ÷ θr = ½, or at (colatitude + D) ÷ θr = 1. It checks d/D
      0.02, 0.1 and 0.5 at colatitudes 0.1, 0.3 and 0.5 θr;
    - a d/D 0.005 bowl at colatitude 0.2 θr and D 0.05 θr, read from the 0.02 slice, matches ray
      tracing to 0.002;
    - the header's acceptance records the worst error of the read against exact geometry. It
      covers these points and the cells near the pole for d/D above 0.37, where the error peaks.
  - _Tests_ in `hyperion-surface`:
    - the pole row holds eq. 6, held at zero, to 10⁻¹², as eq. 8 does to first order;
    - each of these reads matches its exact value to 0.005:
      - 0.2240 at d/D 0.1, 80° and D 1.54°;
      - 0.2735 at 0.3, 60° and 1.54°;
      - 0.3217 at 0.5, 45° and 1.54°;
      - 0.5375 at 0.3, 75° and 4°;
      - 0.1987 at 0.5, 60° and 20°;
    - the read is zero at both edges;
    - T7.d's band and plains tests re-run against it, as T7.d now states them
      (science-r09-drainage.md §8.13): B serves the permanent-shadow bands, and B and A the cold
      traps;
    - _Ceres._ At Ceres's perihelion flux and angular radius (2.5577 au, 0.1042°), with Hayne et
      al.'s lunar terrain, Ceres's permanent shadow holds Schorghofer et al. 2024's 2,282 km²
      between B's and A's at a 4° greatest declination, and is under 68 km² with either at 20°;
    - _CO₂._ Under the real Sun with the lunar conduction share, with B and with A, half the Moon's
      area below 52.9 K and below 55.6 K lies within 30–408 km²: from half of Schorghofer et al.
      2021's peak-rate 59 km² to twice their time-averaged 204 km² over 80–90°S (GRL 48,
      e2021GL095533, Table 1; 10 kg m⁻² Gyr⁻¹, 55.6 K on Bryson et al. 1974's curve). Half, since the
      model is symmetric and their area is the south's.
  - It moves no output, since no consumer has landed. The client's surface module does not
    change, since no export reaches `cold_trap`. _Acceptance:_
    `cargo test -p hyperion-surface cold_trap`; `cargo test -p hyperion-fit bowl_permanent_shadow`;
    `just fit-check`; `just gen-surface` leaves `hyperion_surface_bg.wasm` unchanged byte for
    byte.
- **R09.T7.g The plains from ray-traced surfaces** (a T7 follow-up; science-r09-drainage.md
  §8.13; drafted). After T7.f. It is not needed before the 23 → 24 batch, but lands before any
  caller relies on supervolatile or rough-plains cold traps.
  - **The table:** a new `hyperion-fit` task, `rough_plains_cold_traps`, ray traces Gaussian
    surfaces built as Hayne et al. 2021's Methods D builds them:
    - random phases on the task's own seeded stream;
    - Fourier amplitudes |k|^−(1+H), H 0.9, between two cut-offs;
    - heights scaled to a directional RMS slope w; periodic; horizons by rays at fixed azimuths; a
      point star;
    - each permanently shadowed pixel's peak absorbed flux from the sunlit terrain it sees.

    Over w 0.05–0.6, the colatitude and D = δ + α, it tabulates the permanent shadow and the
    shares of it whose peak absorbed flux lies below fixed fractions of S sin e.

  - **Its use:** `cold_trap`'s plains read it in place of the half-normal bowls,
    `PLAINS_DEPTH_SCALE` and `PLAINS_SURROUNDINGS`. The threshold's flux, less the heat flow's,
    over S sin e picks the share.
  - _Tests:_ its instantaneous shadow is within 10% of Smith 1967's at sun elevations up to 4°. It
    reproduces science-r09-drainage.md §8.13's ray-traced bands within their surfaces' spread.
    T7.d's band, Ceres and CO₂ tests hold.
- **R09.T7.h The kept craters' sharpening in the closed forms** (a T7 follow-up, after R10.T5;
  decision-r10-t5-closed-forms.md). `synth/craters.rs`: the small craters' closed forms count what
  a level's fillet leaves out of the ground's. For each octave the level keeps, that is its
  density times its craters' mean g² ∫ (F(w₂) − F(w))² dA over its diameters and its window's
  ages, w the level's band and w₂ the band limit's: the coarse rims' form (T7.c), by Campbell's
  theorem. Each octave the level leaves out is counted at the ground's fillet, g² ∫ F(w₂)² dA,
  not the fresh profile's. The structure function takes the kept octaves' sharpening as the rims
  do, 2V (1 − exp(−s Δ² ÷ 2V)), with s its slopes' mean square along a random direction. The
  octaves left out keep T7.b's J₀ form. The wavelength-keyed forms take an octave's kept share,
  1 − `band_share`, at ½ λ; the level-keyed forms (R10.T5's) take the level's kept octaves at its
  band. `CraterOctave::variance` stays the full-resolution moment. Tests: one crater's moments
  against sampling; zero at the band limit; `unresolved_rms` non-increasing as the resolution
  refines; the level-keyed crater part the sum of its octaves'; the realised sharpening and the
  octaves left out recorded on the cratered worlds. `tests/golden/height/closed_forms.golden` is
  re-blessed at 21 with no bump (main's ruling of 2026-10-13, before P14.T48.e), with the
  level-keyed forms added. Then, under R10.T5, the pooled reading test's crater gap is removed.
  Acceptance: `cargo test -p hyperion-surface synth`,
  `cargo test -p hyperion-surface --lib height_golden`, `just test-wasm-fast`, and
  `just test-slow -E 'binary(surface_reading_worlds)'`.

Acceptance: `cargo test -p hyperion-surface craters` (T7.a–c and T7.e) and
`cargo test -p hyperion-surface cold_trap` (T7.d).

### R09.T8 Assembly, the bound and the patch

`synth/mod.rs`: the octave sets per level from R05's `finest_level` and band limit, the full
`height_at`, `bake_patch` (heights, parent-band heights, gradients, in R05's bake layout),
`height_range_m`, `level_bound_m`, `structure_function`, `unresolved_rms`; every emitted height
asserted finite.

Tests: for 10⁴ points on each synthetic world, |h(n) − h(n + k)| ≤ `level_bound_m(n)` for every n
and k (slow, `#[ignore = "slow: …"]`, with a 10³-point fast version); the whole height function's
instrumented read set, interpolant, relief, channels and craters, never reaches beyond
`SYNTHESIS_MARGIN_CELLS` from the query's cell on any synthetic world, face edges and cube corners
included (Design note 15); `bake_patch` equals 65 × 65 point queries bit for bit;
`assert_order_independent` over
patch build orders; `unresolved_rms` falls monotonically with resolution and is zero at the band
limit. Full-belt cells (b = 1) on the Earth-like world have an adirectional RMS slope of 32–40° at
100 m and 22–30° at 1 km, a mean of 26–34° at 100 m, and 0.15–0.35 of their area above 40° at
10 m (`decision-r09-t5.md` item 7). The closed-form structure function matches the sampled one
within 10% on belt and plain cells alike. Benches `surface/point` and `surface/patch`, with a
per-component breakdown against Design
note 14's targets and the brainstorm's 10 µs a point and 40 ms a patch, recorded; a miss is a
finding for open question 5, and the hash levers of Design note 14 are tried only then. Acceptance:
`cargo test -p hyperion-surface synth` and `just bench -- surface` (the recipe passes the filter to
every bench binary, so it selects the `surface/…` IDs; on a quiet machine, under the heavy lock, or
recorded provisional).

### R09.T9 Golden heights across targets

`crates/hyperion-surface/tests/height_golden.rs`, goldens in
`crates/hyperion-surface/tests/golden/`: heights and gradients at 512 pinned points and levels of
each synthetic world, and one baked patch per world, written with `GoldenWriter`, blessed natively
and compared on `wasm32-wasip1` and on `wasm32-unknown-unknown` through R04's embedded arm (each
golden named by a literal, which the browser arm `include_str!`s). Bump `GENERATOR_VERSION` here if
T16 has not already (whichever of the two lands first bumps, through the orchestrator; T1.b rides
galaxy plan 14's 21 → 22 batch and its one bump instead). R05's
goldens stay headed `TEST_PLANET_VERSION` and these carry `GENERATOR_VERSION`, so they live in a
subdirectory, `crates/hyperion-surface/tests/golden/height/`, which `cube_golden.rs`'s flat
`every_golden_file_carries_the_test_planet_version` does not read; `height_golden.rs` holds that
directory to `GENERATOR_VERSION` in a `native_only` test of its own; and `golden_diff.py`'s
`TEST_PLANET_PREFIX` rule (`.claude/skills/sim-determinism/scripts/golden_diff.py`, which today
matches the whole `tests/golden/`, its comment expecting
R09's goldens elsewhere) is narrowed to exclude `height/`, with its comment corrected. The
`surface/point` bench is also run under the Electron `node` shim and recorded. Acceptance:
`just test-wasm-fast` runs the goldens on both wasm targets and `cargo test -p hyperion-surface
--test height_golden` natively, and passes (the orchestrator's `just ci` runs all three); a change
to any synthesis constant fails it; `golden_diff.py` reports the new goldens under the generator's
version and R05's under the test planet's.

### R09.T10 The coarse pass's frame

`planetary/surface/{mod,inputs,grid,quantise}.rs`: `CoarseInputs` and its builder, `for_body`
returning `NotModelled` until plan 14's sections carry values, the cell graph over R05's cube (four
edge neighbours, vertex neighbours, solid angles by the closed form atan2(uv, √(1 + u² + v²))
differenced over the corners, through `math::atan2`), the quantiser, `coarse_pass` running its steps
as no-ops, and `reference::{earth_like, mars_like, moon_like, ceres_like}`. `CoarseInputs` gains
`crust`, `condensates` and the palette's sources (P14.T51.a, T51.c; builder arguments until
P14.T54.a puts them on the record), and `for_body` reads them from `record::Surface`.
`reference::*` sets them (decision-composition). `for_body` reads the
record's `bulk`, `figure`, `rotation` and `orbit` sections and the context's stars (Provides), and
answers `NotModelled(RecordSection::Surface)` while `record::Surface` is uninhabited; a test pins
that on a generated rocky body. `for_body` answers `NoSolidSurface` for a `NotApplicable` surface,
from the tag alone. A test pins it on a generated gas giant, and whichever of this task and
P14.T48.e lands second adds a generated sub-Neptune (decision-p14-t35e-wire).

Tests: a no-op pass yields a valid field twice with identical bytes; solid angles sum to 2π ÷ 3 a
face to 10⁻¹²; edge-neighbour lists are symmetric, every cell has four and a corner cell three
vertex neighbours. Acceptance: `cargo test -p hyperion-sim planetary::surface`.

- **R09.T10.c The crust's options and the reference rows** (science-r09-drainage.md §3–§5; a T10
  follow-up, after T2's follow-up C).
  - **The crust** (§5). In `planetary/surface/inputs.rs`, `CrustInputs::secondary` becomes
    `Option<SubstanceKey>`, "the secondary crust, a solid mantle's partial melt (`basalt` by
    default), or `None` on a body whose mantle never melted (a Ceres)". `CrustInputs::primary` keeps
    its type, documented as "the primary crust, crystallised from the body's first global liquid: a
    magma ocean's flotation crust (`anorthosite` on the Moon), or a frozen ocean's shell
    (`phyllosilicate` on a Ceres, `H2O` on an icy body); or `None`". `BuildCoarseInputsError` gains
    `NoCrust`, refusing a crust with neither.
  - **The crust mapping.** `crust_palette`'s `Oceanic` takes the secondary's entry or none, `Lid`
    the primary's, else the secondary's, and `Province` the provinces' `Province` entry, else the
    entry of whichever of the lid's crusts is the same substance. `melt_area_fraction`'s
    documentation reads "the share of the surface above the solidus of the crust at the surface
    (the secondary's, or the primary's where there is none)".
  - **Grain densities** (§3). The reference `Substance` struct's field becomes
    `grain_density_kg_m3`, and its documentation says that every density is a grain density, the
    porosity being how a material lies, which R10's forms carry. `anorthosite` takes 2,900 kg m⁻³,
    implied by the highlands' GRAIL bulk density of 2,550 kg m⁻³ at 12% porosity (Wieczorek et al.
    2013, Science 339, 671), and `mars_dust` 2,600 kg m⁻³, the grain density that the Viking
    landers' fine material's 58% porosity assumes (Moore et al. 1987, USGS PP 1389, p. 121 and
    Table 36). `basalt`'s 2,900 and `granite`'s 2,650 are relabelled grain densities (low
    confidence), and `phyllosilicate`'s 2,500 is one already.
  - **The reference worlds.** `ceres_like`'s crust is
    `primary: Some(PHYLLOSILICATE_KEY), secondary: None`, its palette's phyllosilicate a
    `PrimaryCrust`, and its table's crust and palette rows read "phyllosilicate (primary crust:
    Ceres's frozen ocean, Castillo-Rogez et al. 2018; Fu et al. 2017)". `moon_like` and `mars_like`
    keep their crusts, with `secondary: Some(BASALT_KEY)`. The
    reference Moon's palette gains `H2O` as `Ice`, which no share names until P14.T51.f (§4). A
    generated Moon carries no such entry until P14.T51.f is on the record (T54.b), an interim the
    reference and synthetic Moons, being test inputs, do not share (science-composition-crusts.md
    §6). Each table states its sources.
  - _Tests:_ a crust with neither primary nor secondary is refused with `NoCrust`; the reference
    Ceres's `Oceanic` has no entry, and its `Lid` and `Province` take the phyllosilicate's; every
    reference world builds a valid field.
  - _Moves:_ nothing, since no reference golden exists before T16.
  - _Accept:_ `cargo test -p hyperion-sim planetary::surface`.

- **R09.T10.d An undifferentiated body's bulk, the lid, and the highland and iron-rich rows**
  (science-composition-crusts.md §1, §2, §5 and §7; after T2's follow-up D, R09.T2.d, and T10.c,
  which left these to it: Risks, "Deviations in T10.c, as built"). `planetary/surface/inputs.rs`
  and `reference.rs`; the ruling's §1.6, §2.3, §5.3 and §7.3 hold its code text. The members follow
  P14.T51.c's `bulk` and `lid`, and stay builder arguments until P14.T54.a reads them from the
  record.
  - **The lid** (§5). `LidCrust { Primary, Secondary, Bulk }` and `CrustInputs::lid: LidCrust`,
    which of the crusts the surface shows (P14.T51.c's `lid`). `BuildCoarseInputsError` gains `LidAbsent(LidCrust)`, refusing a lid that names an
    absent crust ("the lid names a crust the body does not have"). `CrustInputs::primary`'s
    documentation adds "buried or at the surface, as `lid` says".
  - **The Lid rule and the melt test read it** (§5). `crust_palette`'s `Lid` takes the entry of the
    crust `lid` names, in place of T10.c's "the primary's, else the secondary's": `Primary` the
    primary's, `Secondary` the secondary's and `Bulk` the bulk's. `melt_area_fraction`'s
    documentation reads "the share of the surface above the solidus of the lid's crust", which
    amends science-r09-drainage.md §5.2 item 4's "the secondary's, or the primary's where there is
    none". `build` resolves the lid's substance once, so that no step unwraps the options.
  - **The bulk** (§1). `CrustInputs::bulk: Option<SubstanceKey>`, an undifferentiated body's own
    material at its surface (`chondrite_h`, `cometary`), with no crust beside it, or `None`: not a
    crust in Taylor's sense. `BuildCoarseInputsError::BulkBesideCrust` ("the crust names a bulk
    material beside a crust"), and `NoCrust` widened to "neither a primary nor a secondary crust nor
    a bulk material". `check_crust` refuses, in this order: none of `primary`, `secondary` and
    `bulk` (`NoCrust`); `bulk` with any of `primary`, `secondary` and `tertiary`
    (`BulkBesideCrust`); a `lid` whose member is `None` (`LidAbsent`). `crust_palette` maps the bulk
    through a `Bulk` entry, which `Lid` takes where `lid` is `Bulk`, and the provinces may share.
    `CrustInputs`'s type documentation states the two forms and the lid.
  - **The reference rows** (§2 and §7):
    - `basalt_fe` (`BASALT_FE_KEY`, `BASALT_FE`), an iron-rich basalt: 3,310 kg m⁻³, the
      shergottites' mean (Macke, Britt and Consolmagno 2011), inside the lunar basalts'
      3,270–3,460 (Kiefer et al. 2012), with `BASALT_FE_SOLIDUS_K`. `mars_like`'s secondary and
      provinces and `moon_like`'s take it; `earth_like` keeps `BASALT`, whose documentation reads
      "a terrestrial tholeiite's; an iron-rich basalt is [`BASALT_FE`]".
    - `feldspathic` (`FELDSPATHIC_KEY`, `FELDSPATHIC`), the highland crust as impacts mixed it:
      2,900 kg m⁻³, implied by the highlands' GRAIL bulk density at 12% porosity (Wieczorek et al.
      2013), with `PLAGIOCLASE_ROCK_SOLIDUS_K`. It replaces `ANORTHOSITE` in `moon_like`, since a
      reference module keeps no unused row; the `anorthosite` rock, 2,790 kg m⁻³, lives in
      P14.T49.b. The GRAIL test reads `bulk(&FELDSPATHIC, 0.12)`.
    - The tables' rows: the Moon's crust and palette read "feldspathic (primary crust: the
      highlands, its magma ocean's flotation crust as impacts mixed it), iron-rich basalt
      (provinces: the maria, its secondary crust)", and Mars's "iron-rich basalt (secondary crust,
      its provinces' too)". The module documentation's row list follows.
  - **The reference worlds' lids** (§5.2):
    - `moon_like`: `primary: Some(FELDSPATHIC_KEY)`, `secondary: Some(BASALT_FE_KEY)`,
      `lid: LidCrust::Primary`;
    - `mars_like`: `secondary: Some(BASALT_FE_KEY)`, `lid: LidCrust::Secondary`;
    - `earth_like`: `lid: LidCrust::Secondary`;
    - `ceres_like`: `lid: LidCrust::Primary`.
  - _Tests:_
    - the three refusals, a lid naming an absent crust among them (`LidAbsent`);
    - a chondritic body's `Lid` and `Province` take its `Bulk` entry, and `Oceanic` and
      `Continental` none;
    - a Mars given a buried primary with `lid: Secondary` takes its secondary for `Lid`;
    - `BASALT_FE`'s grain density lies within Kiefer et al.'s 3,270–3,460;
    - `FELDSPATHIC`'s grain density lies within Kiefer et al. 2012's four feldspathic highland
      rocks, 2,840–2,910 kg m⁻³ by helium pycnometry (Table 1; science-composition-crusts.md
      §10.1);
    - the reference palettes list `(PrimaryCrust, "feldspathic")`, and `basalt_fe` for the Moon and
      Mars.
  - _Moves:_ nothing, since no reference golden exists before T16.
  - _Accept:_ `cargo test -p hyperion-sim planetary::surface`.

- **R09.T10.e The record's other sections** (new 2026-10-14, decision-bump-plan-after-22 §1.5; no
  bump; a T10 follow-up at version 22). `for_body`'s reading of the record's sections other than
  the surface, in `crates/hyperion-sim/src/planetary/surface/inputs.rs`, so that P14.T54.a then
  only connects the record's members and the palette's resolution. This amends main's ruling of
  2026-10-10 only as to who writes the reading: P14.T54.a still owns the file's final form and
  settles the record's time.
  - **What it reads.** The record's bulk, figure, rotation and orbit; the hosts' light over the
    seasonal orbit; a moon's planet's orbit, through `SystemContext` or the planet's record; and
    T17's `RECORD_TIME`.
  - **How.** It assembles `CoarseInputs` from what it reads and a crust, condensates and palette
    passed as arguments. `for_body` itself still stops at the uninhabited crust, answering
    `NotModelled(RecordSection::Surface)` as T10's test pins.
  - _Tests:_ the assembly on hand-made crusts.
  - _Moves:_ nothing.
  - _Accept:_ `cargo test -p hyperion-sim planetary::surface`.

- **R09.T10.f `for_body`'s planet, and the service's** (new 2026-10-15, decision-backlog-2 §3; S;
  no bump; after T10.e, before P14.T54.a). `for_body` takes a moon's planet's record, and the
  server fetches it, so that the signature P14.T54.a connects, frozen at 23, needs no change. As
  built: Risks, "Deviations in T10.f, as built".
  - **The signature** (§3.1). `CoarseInputs::for_body(record, planet: Option<&BodyRecord>, ctx)`
    gives `Result<Self, ReadCoarseInputsError>`, `planet` the record of the planet a moon orbits,
    taken at `RECORD_TIME` with `record`, `Some` exactly where `planet_of(record)` is; and
    `CoarseInputs::planet_of(record) -> Option<BodyIndex>`, `Some` for a moon (`BodyKind::Moon`
    about `OrbitHost::Body`) and `None` for every other body, a belt's member and a ring among
    them. `ctx` stays, unread. The checks run in this order: the surface section's tag, the
    planet (`PlanetMissing`, `NotItsPlanet`), then the crust, so that a correctly supplied moon
    still answers `Section(NotModelled(Surface))` until P14.T54.a.
  - **The server** (§3.3). `InputsSource::inputs(record, planet, ctx)` takes the same three
    arguments, and `RecordInputs` delegates to `for_body`. The service's `surface_record` takes
    the planet's record with `body_at(ctx, p, RECORD_TIME)` beside the body's, where `planet_of`
    is `Some(p)`, and `compute` and `field_level` pass it on. `SurfaceJobError::Inputs` carries
    `ReadCoarseInputsError`. The handlers answer its `Section(..)` arm as before (`not_modelled`
    naming the section, `bad_request` for no solid surface, `internal` for a withheld section),
    and every other arm `internal`, logged: each is a fault in the server's own reading.
  - **The docs** (§2.5). The header's `surface_pressure` and `CoarseInputs::surface_pressure`
    gain "0 also on a surface holding melt whose volatiles are gone, whose own vapour is not
    modelled (P14.T55.b)".
  - _Tests:_ in the sim, a generated moon with its planet stops at the crust, without it
    `PlanetMissing` and with another body `NotItsPlanet`; a planet and a belt's member take
    `None`; `planet_of` agrees with `from_record`'s rule on every body of the sample systems;
    `from_record`'s doctest calls `for_body(rocky, None, &ctx)`. In the server, a capturing
    source sees `Some` of a generated moon's planet's record (at `planet_of`'s index) and `None`
    for a planet; a source answering `PlanetMissing` is answered `internal`; `RecordInputs`
    answers `NotModelled` naming the surface for a moon too.
  - _Files:_ the sim's `planetary/surface/{inputs,mod}.rs`; the server's
    `surface/{inputs,service,mod}.rs`, `convert/surface.rs`, `requests/surface.rs`, `config.rs`,
    `tests/common/surface.rs` and `tests/surface_service.rs`; the surface crate's
    `field/header.rs` (the doc sentence).
  - _Moves:_ nothing (`golden_diff.py` shows nothing moved at 22).
  - _Accept:_ `cargo test -p hyperion-sim --lib planetary::surface`,
    `cargo test -p hyperion-sim --doc planetary::surface` and
    `cargo test -p hyperion-server surface`.

### R09.T11 Plates

`steps/plates.rs`, Design note 6. Tests: every cell has one plate and ties go to the lower seed
index; boundaries are exactly the adjacencies between plates; signed distances are zero on
boundaries and grow by at most one edge length a step; boundary kinds follow the relative motion on
hand-placed plates; the plate areas follow the tail law within a Kolmogorov–Smirnov bound over many
seeds; the mean plate count over those seeds is 48 ± 7; the areas sum to 4π; the distance to
the plate's own divergent boundary, which GDH1 reads, is zero on ridges and never taken from a
trench; a stagnant-lid world has none; each oceanic cell's source half-rate is |ω × r| ÷ 2 normal
to the ridge on hand-placed plates, inherited along the own-ridge transform; its age is d_r ÷ u_h,
capped at 200 Myr; a plate with no divergent boundary has 200 Myr and 23 mm a⁻¹
(`decision-r09-t5.md` item 5). Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::plates`.

### R09.T12 Coarse elevation

- **R09.T12.a Crust and ridges.** `steps/relief.rs`: Design note 7's two crust populations (their
  lithologies from plan 14's `crust`: `Oceanic` the secondary crust's entry, `Continental` the
  tertiary's) about sea level and GDH1's age–depth law from the distance to divergent boundaries.
  Tests: the reference Earth's hypsometry, binned as Earth2014 was (area-weighted 250 m bins, the
  TBI layer at 5′), is bimodal with a land mode within 300 m of +125 m, an ocean modal bin among
  the centres −5,125 to −4,125 m (Earth2014's bins within 10% of its −4,375 m peak) and a mean
  below −2 km from −4,731 to −3,981 m (Earth2014's −4,281 m, +300 m and −450 m, the deep side
  widened for the intraplate volcanism no task builds; `decision-r09-t12-coarse.md` item 1); the
  unloaded basement is GDH1's at 5, 20 and 100 Myr to 1 m. The sea floor's roughness inputs
  (`decision-r09-t5.md` item 5): H₀ from the source half-rate, the pelagic drape from the age and
  latitude, the ponded wedge from the plate's passive margins over its own oceanic cells, quantised
  into `seafloor`. Tests: H₀ within 30% of the mean of Goff et al. 2004's Table 2 entries at full
  rates of 28, 40, 75 (the axial high alone), 110 and 160 mm a⁻¹ (`decision-r09-t12-coarse.md`
  item 5a); S_p is 468 m at 81 Myr on the equator; the wedge never crosses a plate boundary; on
  the reference Earth, oceanic cells whose style amplitude is at most 0.2 cover 25–40% of the
  oceanic crust deeper than 3 km, and the mean of S_p + S_t over oceanic crust farther than 200 km
  from a passive margin is 250–600 m (GlobSed's 404 m).
- **R09.T12.b Convergent landforms.** Belts, trenches and arcs by boundary kind, scaled as Design
  note 7 says. Tests: belts lie within their stated distance of convergent boundaries; trenches lie
  2–4 km below the neighbouring sea floor and the arcs' volcanic fronts lie behind them with
  medians within 15 km of 190 km (oceanic overriding plate) and 260 km (continental), 80% of
  fronts within 140–250 and 180–330 km (`decision-r09-t5.md` item 6).
- **R09.T12.c Stagnant-lid provinces and flexure.** After T0.b's isotherm item. Volcanic provinces
  sized by the volcanism level
  and the flexural moat and bulge about each load, with T_e = k (870 K − T_s) ÷ F (T0.b's ruling,
  Design note 3). T_e's isotherm and conductivity come from the crust's substance: the ice lean of
  Risks becomes the `H2O` row's (decision-composition). Tests: a line load's bulge crest lies at πα
  with height 0.043 w₀; α at T_e = 70 km under Mars's gravity (3.71 m s⁻²) and radius (3,389.5 km),
  with ρ_m 3,300 kg m⁻³ and an open moat (ρ_fill = 0), is 180 km ± 10 (its value 175.5 km;
  `decision-r09-t12-coarse.md` item 3); T_e is 108 km ± 1 at Mars's 19 mW m⁻² and 210 K, and
  zero where T_s ≥ 870 K, where each load is compensated locally (α = 0).
- **R09.T12.d Coarse craters.** `steps/craters.rs`, Design notes 5 and 10, after T7.e. Each
  crater's shape is T7.a's `crater_shape`, so the coarse elevation and the client's T7.c rims are
  one profile. Tests:
  counts over the reference Moon match min(production, saturation) above D_b (Poisson interval),
  about 300 above 100 km with N(1 km) at 4.4 Gyr; ages split before and after the wet epoch on the
  reference Mars; the list is sorted and each crater's `reach` is complete; before degradation,
  the reference Moon's coarse craters' rim heights are Pike 1980b's 0.236 D^0.399 to 1%.
- **R09.T12.e σ_h and sea level.** Design note 7, on the reconstructed field, after T4 and T5.
  Its σ_h budget books the realised structural share, the area mean of each quadrature point's
  `Relief::point_variance`, which includes the belts' lift (`decision-r09-t5.md` item 8), with
  the craters' and channels' closed forms finer than λ_c. The header's spectrum is
  `BandSpectrum::anchored` of the body's crust class (`decision-r09-t5.md` item 3), chosen from
  `surface_material` and `tectonics`.
  Tests, as main's rulings of 2026-10-19 amend them (`decision-r09-t12-coarse.md` addendum 4): the
  whole surface's reconstructed σ_h, √(Var_recon + S_s + K_w + R_inc + C_c), the craters waiting
  for the erosion step smoothed in, equals the input to 10⁻⁶ relative after the step; the local
  share finer than λ_c, the realised structural share plus the small craters'
  `unresolved_variance(λ_c)`, the coarse craters' sharpening finer than λ_c over the body and the
  channels' at λ_c, divided by σ_h², is 4–10% on the reference Moon, 3–8% on the reference Ceres
  and 0.3–1% on the reference Earth and Mars (Design note 7's 7, 5 and 0.6%), asserted on the Earth
  and on each other world where it holds; craters supply at least 80% of the Moon's; ⟨a²⟩ on the
  reference Earth is 0.6–1.2; the realised structural share equals ⟨a²⟩ `local_variance` to 10⁻¹²
  on a world with no belt, and on the reference Earth exceeds it by 0–15%; the realised relief, the
  cells' range, lies at 6–16 σ_h on every reference world (Design note 3's observed 7–15 widened by
  one σ_h each way, since one realisation scatters about it); the ocean, the cells the flood took,
  covers at least the ocean fraction and exceeds it by no more than the cells that joined at the
  final level, no cell is flooded where the fraction is zero and every cell but the highest where
  it is 1; the header reports the realised σ_h and relief and carries the sea level and the
  spectrum; `structural_spectrum` takes each crust class's anchor, the ice-rich one in every regime,
  and the mobile lid's halves at 2 g⊕.

Acceptance: `cargo test -p hyperion-sim planetary::surface::steps::relief` (T12.a–c and e) and
`cargo test -p hyperion-sim planetary::surface::steps::craters` (T12.d).

### R09.T13 Climate

- **R09.T13.a The zonal model.** After T0.b's transport-cap items. `steps/climate/ebm.rs`: Design
  note 8's implicit seasonal moist
  energy-balance model in latitude, with its heat capacities, gray outgoing radiation, albedo and D
  scaling, on the grid Design note 8 names, the rotation and pressure factors tabulated from Kaspi
  and Showman 2015 (T0.b's transport law). Tests: an Earth-like input converges within the fixed
  orbit count and its annual-mean zonal temperature lies within 5 K of Siler et al.'s Fig. 2b
  (ERA-Interim); with seasons off, the equator–pole contrast, scaled so that the 1 Ω⊕, 1 bar run
  reads their 40 K (Fig. 8a) or 42 K (Fig. 15c), matches Kaspi and Showman's figures to 2 K over
  Ω⊕ ÷ 24 to 12 Ω⊕ and 0.2–50 bar; the scheme is stable at Venus's rotation and
  at a six-hour step for every rotation in the reference set; the months are twelve equal spans of
  eccentric anomaly from periapsis, each record the mean of its span's steps; at e = 0 they are the
  period's twelfths, and at e = 0.6 the periapsis month's record averages 0.4 twelfths of the
  orbit.
- **R09.T13.b Longitude and locked coordinates.** The periodic longitude solve split after the
  latitude one in a fixed order, and the tidally locked coordinates about P14.T14's substellar
  axis. The locked solver steps seasons as the zonal one does: a world locked 1:1 to its star has
  twelve months wherever its orbit is eccentric or it has obliquity, and a moon locked to its
  planet has its planet's orbit's seasons at its own obliquity to that orbit (Design note 8; a lock
  adds nothing, signed off 2026-10-10, `signoff-2.md` item 4). Tests: with no land–sea contrast the
  zonal model's answer is reproduced to 0.01 K; a locked world is warmest at the substellar point;
  a world locked 1:1 to its star on an eccentric orbit, or with obliquity, has twelve months, and
  its months differ.
- **R09.T13.c The other regimes.** `steps/climate/{airless,isothermal}.rs`: radiative equilibrium
  with thermal inertia for airless and thin-atmosphere worlds, and the isothermal surface for a
  Venus. On a locked lava world, cells above the transition temperature of their own crust entry
  are melt, from this task's per-cell temperature, not from the global state; the body's
  `melt_area_fraction` is the lid's crust's, the crust P14.T51.c's `lid` names (`CrustInputs::lid`,
  R09.T10.d; science-composition-crusts.md §5, which replaces "the secondary's, or the primary's
  where there is none"). Tests: an airless world's
  day–night contrast exceeds 300 K; the isothermal model's surface varies by under 1 K; a locked
  lava world's melt is the dayside region above the solidus, and its nightside is solid.
- **R09.T13.d Normalisation and ice.** Plan 14's mean and signed contrasts imposed on their
  components (P₂, P₁, a constant after the lapse term); each ice placed per condensate, coldest
  first against its own frost point by annual maximum, until its area matches plan 14's area for
  that substance (`surface_ices`), but a cold-trapped one, which T13.f places in each cell's
  cold-trap fraction, and each liquid's surface to its share of the ocean fraction
  (`surface_liquids`); the energy-balance albedo function shifted to the climate condensable's
  freezing point, by `SubstanceId` (decision-composition). Tests: the area-weighted surface mean
  equals the input to 0.1 K; the P₂ coefficient gives the input contrast exactly; ice area equals
  the input fraction to one cell; a warm pole gets no cap; a 90°-obliquity world is labelled by
  FILLET's ice-edge states (an ice belt, not caps, when plan 14's history says it started cold);
  the reference Mars places CO₂ ice inside its water-ice cap's latitude, given by hand; a
  Titan-like world's methane liquid fills its basins to its share.
- **R09.T13.e Precipitation and wind.** Design note 8's heuristic, labelled in the header, and
  each month's 10 m wind. Tests:
  global precipitation equals global evaporation to 1%; the reference Earth's zonal precipitation
  has maxima within 10° of the energy-flux equator and in the 40–60° bands and minima at 15–35°; on
  a one-dimensional ridge under a steady wind the lee receives under half the windward rate; inland
  precipitation decays from the sea with an e-folding within 2× of L_l; the header says heuristic;
  - each month's quantised wind is the wind that month's orographic step used;
  - on a synthetic aquaplanet at 60° obliquity, an equatorial cell's meridional wind changes sign
    twice a year;
  - a world at e = 0 with no obliquity to its seasonal orbit has one wind (Design note 8's one
    month; a lock adds nothing, `signoff-2.md` item 4).
- **R09.T13.f Cold traps.** After T13.c, T13.d and T7.d (science-r09-drainage.md §4).
  - **The shared closed form** is T7.d's `hyperion_surface::cold_trap::fraction`, built early in
    the surface crate. This task calls it, as P14.T51.f does, and does not change it.
    Its sky comes from the inputs' spin and seasonal orbit: the greatest declination, the host's
    angular radius and flux at periapsis. Its heat flow is `heat_flow`, and its conduction share the
    lunar reference.
  - **In the airless model:** each cell's fraction for each cold-trapped condensate, at the cell's
    latitude, with its crater fraction from the body's crater state and its plains' slope from the
    synthesis's slope variance at the cold traps' scales. The plains' slope is clamped to 45°
    before a `Terrain` is built, since T7.d's `Terrain::new` refuses a plains' RMS slope above 45°
    (Risks, "Deviations in T7.d, as built"; its stand-in is checked against Smith 1967 only to
    26.6°).
  - **The placement:** T13.d places a cold-trapped ice in proportion to each cell's fraction,
    capped at it and scaled so that its area matches plan 14's. Its quantised shares are rounded
    coldest first, so the total matches to one cell. It is never placed coldest-first by whole
    cells.
  - _Tests_ (the closed form's own, Ingersoll et al.'s 97 K and Hayne et al.'s Table 1 bands, are
    T7.d's):
    - a Moon-like input given 1.7 × 10⁻⁵ of water ice by hand places it only poleward of 70°, more
      than 90% of it poleward of 80°, and no cell's share exceeds its fraction;
    - at 25° obliquity no cell has a fraction;
    - a water ice on a body whose zonal maximum is below its threshold is placed by T13.d's own
      rule.
  - _Acceptance:_ `cargo test -p hyperion-sim planetary::surface::steps::climate`.

Offline check (recorded, not in CI): the reference Earth's monthly fields against an ExoPlaSim run
of the same inputs, with the differences written into this plan. Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::climate`.

### R09.T14 Erosion

- **R09.T14.a Drainage.** Random receivers on `surface.coarse.erosion` with the slope correction;
  the depth-first order; drainage area by solid angle; priority flood with pop-order flats and
  Fill–Spill–Merge. Tests: drainage area at every outlet sums to the draining area; no cell drains
  uphill after filling; the same seed gives the same receivers; a finite inventory fills the lowest
  basins first and spills.
- **R09.T14.b The solver.** After T0.b's erodibility item. Tzathas et al.'s recursion with n = 1,
  the fixed point with its moving
  average, multigrid to level 4 at six iterations a level with the upsample through T4's
  interpolant; K scaled as Design note 9 says with m = 0.45 and the runoff-weighted area (T0.b's
  erodibility: K⊕ converted from Tzathas et al.'s m = 0.4, linear in g and ρ_f, the bed factor B,
  runoff once). t is `effective_flow`, in Earth-equivalent years at the reference intermittency of
  5% (Design note 9), and K carries no intermittency factor of its own. Tests: a ridge-to-sea
  profile matches the closed-form steady state; over a step short enough that slopes barely
  change, a profile's lowering far from base level is linear in t and in runoff^0.45 (to 1%), and
  the saturated-regolith K at a uniform 0.1 m a⁻¹ for 2.2 × 10⁵ yr gives K (runoff ÷ 1 m a⁻¹)^0.45
  t = 1.9 m^0.1 to 2%; the fixed point's residual falls monotonically over the multigrid levels
  on the reference Earth; K is 6.3 × 10⁻⁶, 2.4 × 10⁻⁵ and 3.1 × 10⁻⁶ m^0.1 a⁻¹ (to 2%) for an
  Earth's rock under water, a saturated Mars's regolith and a Titan's ice under methane. K's bed
  factor B comes from a (substrate, fluid) table over the registry's rows, and ρ_f is the
  liquid's density. The three calibrated cases (rock, saturated regolith, ice under methane) stay
  its tests (decision-composition). The equivalent area, ∫ r dA ÷ (1 m a⁻¹) (Hergarten 2021, eq. 16
  and its accumulation, eq. 20), is the solver's working state only; T14.d writes the geometric
  area and the two runoffs (science-r09-drainage.md §1).
- **R09.T14.c The branches and the Mars check.** The wet-now, dry-now and never-wet branches,
  after T6.b. On the dry-now branch the drainage area is weighted by the epoch's `runoff`,
  uniform over the cells.
  - _Tests:_
    - The dry-now branch leaves surfaces younger than the epoch's end untouched.
    - A never-wet world's elevation is unchanged by the step.
    - The reference Mars's wet epoch is `effective_flow` 2.2 × 10⁵ yr at `runoff` 0.1 m a⁻¹ (K t =
      1.9 m^0.1, Black et al. 2017's; the foot of Hoke, Hynek and Tucker 2011's 10⁵–10⁸ yr, Design
      note 9).
    - On it the coarse erosion volume **plus** the synthesis's expected sub-cell channel incision
      volume (closed-form per cell from k_s, the cell's two runoffs, Hack's law and the network's
      widths from discharge, as T6.b computes them) is at least
      1.2 m of global equivalent layer (Luo, Cang and Howard 2017's (1.74 ± 0.8) × 10¹⁴ m³ over
      1.444 × 10¹⁴ m²), since valley networks 1–10 km wide live mostly below a 38 km cell.
    - The coarse lowering is bounded above by the valleys' cross-sections (incision × valley
      width ÷ cell width, with Black et al.'s 50–350 m of trunk incision and the network's widths;
      at most about 92 m on a 38 km cell holding a 10 km trunk). Over Hoke et al.'s three decades
      a floor alone would pass any over-erosion.
    - A slow test records the final residual, the cell-mean lowering against a fine-grid run of
      one basin, and the volume at the window's extremes, 10⁵ yr at 0.073 m a⁻¹ and 10⁸ yr at
      1.10 m a⁻¹.
- **R09.T14.d Rescale and outputs.** σ_h re-matched on the reconstructed field, lapse and mean
  re-applied; flow directions, the geometric drainage area (T14.a's), k_s = S A^θ on it, each
  cell's `runoff` (the source term the solver took: the climate's annual runoff on a world wet
  now, the epoch's `runoff` where the surface is older than the epoch's end on one dry now, none
  elsewhere) and its `catchment_runoff` (the solver's equivalent area over the geometric, times
  1 m a⁻¹), and water surfaces (science-r09-drainage.md §1).
  - _Tests:_
    - σ_h and the mean temperature again match;
    - lakes are level;
    - the quantised catchment runoff times the drainage area equals the solver's equivalent area
      times 1 m a⁻¹ to the two codes' steps;
    - k_s × drainage^−θ is the slope to the receiver to the steepness code's step;
    - on the reference Mars every eroded cell's own runoff is its epoch's 0.1 m a⁻¹, and its
      catchment runoff equals it wherever its whole catchment is older than the epoch's end;
    - every cell younger than the epoch's end has no runoff of its own;
    - on the reference Earth some cell's catchment runoff is at least three times its own.

Bench `coarse/erosion_l8`: the brainstorm's "a few seconds at level 8" (Tzathas et al.'s Table 2,
1.79 s at 512² and 8.18 s at 1,024² in Python with numba), recorded, in a new
`crates/hyperion-sim/benches/surface.rs` (`[[bench]]`, `harness = false`), which T16 extends.
Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::erosion`.

### R09.T15 Classes and crater state

`steps/classes.rs`: Köppen–Geiger by Peel et al.'s Table 1 with Beck et al.'s rules, applied to
rates with each cell's own summer (Design note 11); surface-state classes for the other regimes and
for one-month worlds; each naming its palette entry (codes 64–255), with melt per cell;
Köppen–Geiger 1–63 (decision-composition); the per-cell crater state. Tests: the classifier given
one monthly climatology labelled once as a one-year orbit and once as a four-year orbit (the same
temperatures, and precipitation at the same rates per 30.44 d) returns the same classes, which tests
the rate rule alone, not the climate a longer orbit would have; on an orbit of e = 0.6, four months
above 10 °C centred on periapsis count as 2.0 twelfths of the year, so they do not meet a
four-month threshold; the reference Earth has tropical, arid, temperate, continental and polar
classes in plausible areas; a lifeless world has no vegetation class; the reference Moon is
regolith throughout.
`crates/hyperion-sim/examples/surface_map.rs` (the sim's first example, with an `[[example]]` entry
carrying `required-features = ["testing"]`) writes the reference Earth's elevation, class and flow
as equirectangular PPM images under `target/`, with no new dependency, and the look (belts along
convergent boundaries, rivers to the sea) is recorded in this task's entry. Acceptance: `cargo test
-p hyperion-sim planetary::surface::steps::classes` and `cargo run -p hyperion-sim --features
testing --example surface_map` writing its three images.

### R09.T16 Coarse goldens and benches

`crates/hyperion-sim/tests/surface_coarse_golden.rs`, over a shared
`crates/hyperion-sim/tests/common/surface_worlds.rs` that runs `reference::*` through the pass once
per binary and that R10's world tests reuse: the quantised fields of the reference Ceres
and Moon (fast, so they run on native and wasip1 under R04's recipes) and of the reference Earth
and Mars (slow). The coarse goldens pin each reference world's palette. Bench `coarse/pass` per
level 5–8, beside T14's `coarse/erosion_l8` in
`crates/hyperion-sim/benches/surface.rs`. Bump `GENERATOR_VERSION` if T9 has not already
(whichever of the two lands first bumps, through the orchestrator; T1.b rides galaxy plan 14's
21 → 22 batch and its one bump instead), or again if the first bump has been released to a save.
Acceptance: `cargo test -p hyperion-sim --test
surface_coarse_golden`, the task's own slow tests (`just test-slow` filtered to them), and
`just bench -- coarse`; the orchestrator's `just ci` runs the fast goldens on wasip1.

### R09.T17 The surface service

`crates/hyperion-server/src/surface/{mod,cache,service,inputs}.rs` and `config.rs`: Design note 19;
the `InputsSource` seam with `RecordInputs` for production and, in
`crates/hyperion-server/tests/common/surface.rs`, `reference_inputs` over
`hyperion_sim::planetary::surface::reference::*` for tests; the job runs on R04's probed pool, and
`JobError::FloatingPointMode` refuses the field with `internal` and logs it. As R06's sky does
(`compute/sky_tables.rs`, `compute::sky::bulk`), the cache and `SingleFlight` sit inside the
service, an `AppState` field, and the job goes to the pool at `Priority::Bulk`; the cache size is
`config.rs`'s, on the sky cache's pattern (Provides). The server's `[dev-dependencies]` gain
`hyperion-sim` with its `testing` feature, for `reference_inputs`.

Tests: two concurrent requests compute once (a counter); the cache is bounded and evicts; a
cancelled job leaves nothing cached; the server's field, from `reference_inputs`, equals
`coarse_pass` on the same inputs bit for bit; with `RecordInputs` a generated body answers
`NotModelled` naming its section until the serve gate opens (P14.T54.a's gate constant in
`crates/hyperion-server/src/surface/inputs.rs`; updated 2026-10-15). Acceptance:
`cargo test -p hyperion-server surface`.

### R09.T18 Survey passes and coverage

`crates/hyperion-server/src/knowledge/surveys.rs`, on P12.T7's store (built): `SurveyPass`,
`SurveyLog` (append on record, load on open, unknown version refused, a torn last line dropped with
a warning), `Coverage` folded on load, the field's level and cell count from T2's `coarse_level`
and `cell_index`, and the codes T2's `ResolutionCode`. As built, `persist.rs`'s log helpers are
private and written for contacts (`KnowledgeLog`, `push_line`, `sync_directory`, and
`refuse_later_formats`, which names `contacts.v`), so T18 first moves them into a private shared
module (`knowledge/jsonl.rs`), the file name a parameter, with `persist.rs`'s behaviour and tests
unchanged, and builds `SurveyLog` on them: a `{"format":1}` header, appended and synced before it
is applied, every file operation under `spawn_blocking`, through `UniverseStore::knowledge_dir`.
Nothing yet holds a universe's Knowledge in `AppState` (P12.T8 will), so the `SurfaceService` opens
each universe's `SurveyLog` on its first `survey_pass` or `surface_field` and keeps it (T19.a wires
it).

Tests: round trip; reopening a universe restores the same coverage bytes; the finest resolution
wins; folding is independent of pass order. Acceptance: `cargo test -p hyperion-server knowledge`.

### R09.T19 The wire and the gate

After T8, T17 and T18.

- **R09.T19.a The kinds.** `crates/hyperion-protocol/src/surface.rs` and `just gen-protocol`: the
  two kinds under plan 04's rules (`kind`, `is_large` true for `surface_field`, the `every_body`
  walk), `SurfaceFieldDto` with its `NotModelled` arm, `CellRangeDto`, `CoverDto` and
  `MAX_SURVEY_RANGES` in `limits.rs`; the `survey_pass` handler recording through
  `SurveyLog::record`. An unknown body is `unknown_body` and one with no solid surface
  (`for_body`'s `NoSolidSurface`, from the record's `NotApplicable` surface) `bad_request`, each
  naming `body` (a malformed index and an unresolved system as `body_detail`
  refuses them, through `convert::planetary::body_refusal`, the body ID going through the sim's
  `resolve` as plan 04 requires); more than `MAX_SURVEY_RANGES` ranges is `bad_request` naming
  `cells`. As built, a new kind touches seven places: `RequestBody` and `ResponseBody`,
  `REQUEST_KINDS` and its pinned test list, the protocol's `next_request` and `next_response` walk
  (`envelope.rs`), the server's `every_body` test helper, `kind`, `is_large` and the `Handlers`
  match (`requests/mod.rs`); `MAX_SURVEY_RANGES` joins `every_limit_is_the_plans`. Tests: wire
  forms of every DTO; a `survey_pass` of 256 ranges fits `MAX_INBOUND_FRAME_BYTES` and one of 257
  is refused; a body with no solid surface is refused. Acceptance:
  `cargo test -p hyperion-protocol surface`, `cargo test -p hyperion-server requests` and
  `just gen-protocol-check`.
- **R09.T19.b The payload, the gate and the revisions.** The `surface_field` handler: the payload
  through R03's `Answer`, gated by coverage with the margin and the craters, the revision and the
  delta since `have_revision`, and `NotModelled` for a body whose inputs are not modelled; the
  scene's `surface_revisions` (with R03): an optional field on `SceneNotificationDto`, merged by
  body in `subscriptions::ScenePush::merge`, `PROTOCOL_VERSION` unchanged (Provides; R03's Design
  note 4 as drafted by T0.a), with `just gen-protocol`. Tests: a second pass's delta carries only
  new cells; the same request twice gives identical bytes; a `survey_pass` bumps the body's revision
  on the scene topic; two pushes merged while a reader is slow carry each body's latest revision;
  a merge of two pushes keeps each body's larger revision whatever their order; a resubscribed
  client's `have_revision` request returns the cells surveyed while it was away.
  Acceptance: `cargo test -p hyperion-server surface` and `cargo test -p hyperion-server
subscriptions`.
- **R09.T19.c The Knowledge-bound test.** Over a real socket, extending R03's, on
  `reference_inputs`: after one orbital pass over a region, no block carries a cell outside the
  region and its margin, and margin cells are marked; the cover's resolution codes match the
  passes; the client's decoded `PartialField` gives the server's own heights at every surveyed
  point, the region's edge included; no response or notification carries a key named
  `surface_seed`. Acceptance: `cargo test -p hyperion-server --test surface_knowledge` (the test's
  file, `crates/hyperion-server/tests/surface_knowledge.rs`, beside R03's `scene_knowledge.rs`);
  the orchestrator's `just ci`. R03.T15's 15 MiB (61-chunk) check in the real Electron renderer
  (Risks) needs a body whose field the server binary serves, which needs plan 14's surface section
  (P14.T24, P14.T48), and a page that asks for it, which is R10's client; until both exist it stays
  pending and is recorded so here.

### R09.T20 Verification and hand-over

Run every slow test and bench on a quiet machine, record figures in the doc comments that own them,
mark R05's provisional function deprecated with a pointer to `Synthesiser` for R10, and record the
per-point cost in wasm against the budget and against R05's T3.c figure for the test planet.
Acceptance: `just ci`, `just ci-slow`, `just bench` complete (the orchestrator runs `just ci` and
`just ci-slow`; a lane runs only the slow tests and benches this plan created, the timed ones under
the heavy lock or recorded provisional).

## Verification

- **Agreement:** golden heights equal on native, wasip1 and the browser target (T9); the server's
  heights equal a client's from the wire, edges included (T19.c); the interpolant agrees bit for
  bit across every face edge and corner (T4).
- **Knowledge:** no unsurveyed cell beyond the margin is sent, and the whole height function reads
  nothing beyond it (T8, T19.c); the surface seed never reaches a client (a JSON-level assertion
  over every response and notification, T19.c).
- **Consistency:** the level bound holds for every level pair; patches equal point queries; any
  build order gives the same heights; every crater, and every coarse crater's sharpening, integrates
  to zero at every level (T7.b, T7.c, T8).
- **Constraints to plan 14:** reconstructed σ_h, ocean fraction, ice fraction and mean temperature
  match the inputs (T12, T13, T14).
- **Physics checks:** hypsometric modes against Earth2014, crater counts against the saturation cap,
  zonal climate against ERA-Interim through Siler et al., Hack's law, the Mars valley-network
  volume.
- **Benches:** `surface/point`, `surface/patch`, `surface/channels` (native and wasm),
  `coarse/pass` by level, `coarse/erosion_l8`, each recorded on a quiet machine against the
  brainstorm's figures.
- **By eye:** the reference Earth's field rendered to equirectangular images by T15's
  `surface_map` example, belts along convergent boundaries and rivers to the sea, compared with a
  person looking and recorded in T15's entry.

## Generator version

The coarse pass, its quantisation and payload format, and the synthesis are generated output (open
question 4), so a change to any of them bumps `GENERATOR_VERSION` (21 at the re-validation).
Until R09.T19 first sends a payload, a change to the codec's layout or to the `testing` worlds
re-blesses `tests/golden/wire/` at the current version with neither version bumped (ruling of
2026-10-09); from then it bumps both. _The first send (ruling of "main", 2026-10-13):_ the first
payload the server serves for a generated body, once production inputs inhabit the surface
section (P14.T24 and P14.T48), or a client's first persisting of a payload (a disk cache, say),
whichever comes first. _The serve gate (decision-bump-plan-after-22, 2026-10-14; updated
2026-10-15):_ the first send is the serve gate's bump, and generated bodies answer `not_modelled`
until it opens. From galaxy plan 14's 23 (P14.T54.a) `for_body` is `Ok` in the sim for generated
solid bodies, but the server's `RecordInputs` keeps answering them `not_modelled` until the
gate's version, a constant with a tripwire in `crates/hyperion-server/src/surface/inputs.rs` that
P14.T54.a adds. It opens at the first bump after T16 has landed and R11's liquid members are in
the codec, expected to be 24, or at an earlier bump if "main" opens it for an R10 task. Until it
opens, this plan's and R10's changes re-bless at the version in force with no bump; from that
bump every change to the coarse pass, quantisation, synthesis, material classes or payload
layout rides a bump, and a layout change also bumps `SURFACE_PAYLOAD_FORMAT` (galaxy plan 14's
Generator version). T19.b serves payloads to tests and the `testing` worlds alone, so it is not
the first send, and R11's liquid palette member is still a re-bless with no bump. The freeze
protects consumers that hold payloads across versions, and none exists while every generated body
answers `not_modelled` and no client asks. T1.b, whose filled hooks section moves plan 14's golden
systems, makes no bump of its own: it is built in galaxy plan 14's 21 → 22 batch and lands under
that batch's one bump (plan 14's bump plan, under its Tasks' order). This plan's own first bump is
made by whichever of T9 and T16 lands first, through the orchestrator, which coordinates bumps one
lane at a time; later tasks that change a committed golden bump again. As built, T9 landed at 21
with no bump of its own, and that batch's bump is this plan's first: it re-blesses
`tests/golden/height/` with the rest, since no synthesis output has reached a save or the wire
(main's ruling, 2026-10-13; Risks, "Deviations in T9, as built"). _As built_ (lane `p14-bump`,
2026-10-14): that bump, galaxy plan 14's 21 → 22, is this plan's first. It re-blessed every
golden of this plan's under `crates/hyperion-surface/tests/golden/` and the sim's at 22 (`height/`
with T7.h's `closed_forms`, `ground/`, `material/`, R10.T2's `weights/` and `wire/`), header only,
with T1.b's hooks section on the golden systems; neither `SURFACE_PAYLOAD_FORMAT` nor
`TEST_PLANET_VERSION` moved. R05's `FINEST_SPACING_M`,
`BAND_LIMIT_M`
and `finest_level` join the version when T9 reads them, as R05 notes. Nothing upstream moves but
the hooks section T1.b fills: every draw is on a new tag, plan 14's `body.surface` seed is
unchanged (T1.a moves its type, not its value), and T5's generalised noise leaves the test planet's
output and `TEST_PLANET_VERSION` as they are. Reserved here: the two scopes
and the cell key's packing; the tags of Provides and `surface.scatter` for R11; the `instance` field
for R11's shape draws; `SURFACE_PAYLOAD_FORMAT` 1; `surveys.v1.jsonl`; the level rule's 40 km, 5 and
8; `ClimateCell` at level L − 1; the header's `albedo_scale` for R10 and its `surface_age` and
`surface_pressure` for R11; and T2's header `reference_temperature` and `temperature_step` (0.01 K
× 2ⁿ, n 0–15) and `season_eccentricity`, with the months' eccentric-anomaly rule and twelve winds
per climate record (`decision-r09-t2.md` item 1), and every code's scale (Risks, "Deviations in
T2, as built"), the spectrum's break fields, the sea-floor byte's two scales
(`decision-r09-t5.md`) and `LogRunoff`'s scale for follow-up C's two runoff bytes,
`LogPrecipitation`'s in volume: code 0 none, below 0.1 mm a⁻¹, and code c ≥ 1
0.1 mm a⁻¹ × 2^((c − 1) ÷ 12) (science-r09-drainage.md §1); and T2's follow-up E's palette
liquid record (its eight members in order, each optional member's presence byte, and the
record's own), `LIQUID_BINS`' fifteen bake bins, `LiquidBasis`'s codes 0–2, and the header's
`maturity` and `organic_cover`, dimensionless in [0, 1] and absent until P14.T54.b
(decision-r11-palette-liquids §1 and §2). R10.T10.a fills
`FieldHeader.albedo_scale` and bumps `GENERATOR_VERSION`,
re-blessing this plan's payload and coarse goldens in that commit. _As built (main's ruling for
R10.T10.a, 2026-10-14): no bump. The first send above has not happened, since no generated body is
served before P14.T54.a, so the scale is filled at 22 by the rule of 2026-10-09; and it moves none
of this plan's goldens, the server setting it after `coarse_pass`, whose output and the testing
worlds' payloads keep `None` (R10's Risks, "Deviations in T10.a, as built")._

## Risks and open points

- **Low-confidence constants.** The σ_h fit rests on one body per constant (0.9 km, 0.16, the √N
  age law, the 70 km lithosphere normalisation); stream-power erodibility across fluids and
  gravities; the energy-balance transport cap; Venus's crater screening; the complex-crater peak's
  base width (Hale and Head 1979's lunar law since R09.T7.e, which was Garvin et al.'s 0.3 D for
  Mars) and the regolith bowl's gravity scaling (`decision-r09-t2.md` item 3); Mars's complex
  craters deepening faster than the one law (exponent 0.53–0.58 against 0.30), which leaves its
  largest fresh complex craters about 30% shallow; the diameter map's class constants, the porous
  class's absolute size and its lunar impact velocity (T7.a and T7.e, "Deviations … as built").
  Each is a named constant with its source and confidence in its doc comment.
  T0.b's checks (2026-10-09) covered the first four; what they left open is under "Remaining
  checks, as researched" below.
- **Several tasks are near a day.** T8 (assembly, the bound, the patch and the read set) and T6.b
  are the largest left unsplit; if either runs over, T8's read-set test and T6.b's bench split
  off as their own subtasks.
- **Timings are provisional.** Every cost in Design note 14 was measured under other agents' load
  and scaled by a guess. If the quiet measurement of T6.b and T8 exceeds the budget, the levers are
  the channel search and level cut, then the hash fallbacks; the band limit does not move. The
  coarse pass at level 8 may take tens of seconds with the climate model, which the brainstorm
  accepts if it starts on approach; until sessions exist it starts on first request.
- **Plan 14's asks may not land in time.** The pass runs on builder inputs meanwhile, and `for_body`
  says `NotModelled(RecordSection::Surface)`; no generated world gets a surface until plan 14
  inhabits the record's surface section (P14.T24.a–b, unbuilt) and carries σ_h with f_c, the
  volatile history, the crater parameters, the regime classifier and the signed contrasts (drafted
  as P14.T48.a–e). The ice belt needs plan 14's cold- or warm-start history, which it has no
  classifier to hold yet. The server's tests run on the reference worlds through `InputsSource`
  until then, and no task of this plan waits on them.
- **Reconciled with R08.T1's asks** (2026-10-09). R08.T1 wrote rendering plan R08's asks into plan
  14 beside Phase K (P14.T24.c–f under Phase E, P14.T35.e under Phase H) and reconciled them with
  this plan's five, so that no field is defined twice (plan 14's Risks, "The rendering plans' asks
  of the surface section"; the order is plan 14's Tasks, "Order and parallelism"). For this plan:
  - P14.T48.e stays the one task that defines `record::Surface`, which `for_body` reads. R08's
    vertical structure and aerosol inventory (P14.T24.e, T24.c) join it later as members and
    change nothing `for_body` reads.
  - The mean surface temperature and the surface pressure are P14.T24.a's; T48.c's screening reads
    the pressure as P ÷ g, and its g is the bulk section's. Neither has a second field.
  - T48.d's condensable is also what R08's lapse rate reads (P14.T24.e's α), so T48.d is built
    before T24.e.
  - P14.T24.f (methane on cold worlds, abiotic O₂) is built first, so that T48.d's condensable on a
    Titan is methane and the gases `CoarseInputs` reads carry it.
  - A gas-envelope body, a sub-Neptune included, has its surface section `NotApplicable` from
    P14.T48.e (decision-p14-t35e-wire). `for_body` maps the surface section's tag alone:
    `NotApplicable` is `NoSolidSurface`, which `surface_field` and `survey_pass` refuse as
    `bad_request` naming `body`, and `NotModelled` is `NotModelled(RecordSection::Surface)`. It
    never decides from the class or a `PlanetClass` predicate; `has_surface` is gone, split into
    `is_giant` and `has_solid_surface`. Before P14.T48.e a sub-Neptune's surface is `NotModelled`,
    so it answers `NotModelled` as every body does then.
  - P14.T24.c's dust reads plan 14's own figures, never this plan's coarse wind field, so that no
    cycle runs through the coarse pass.
  - The wire, P14.T35.e, is not this plan's to read: the coarse pass reads the record on the
    server. R10 reads the same section from the wire (its Consumes).
- **Physics leans still to rule** (research of 2026-09-29, low to medium confidence):
  - _The transport cap_ and _the isotherm of T_e_: ruled by T0.b (researched 2026-10-09). The cap's
    lean, (Ω⊕ ÷ Ω)² ≤ 64 under about 100 D⊕, is withdrawn with the Ω⁻² law itself, for rotation and
    pressure factors tabulated from Kaspi and Showman 2015 (Design note 8); the isotherm stays 870
    K (Design note 3).
  - _Small-crater density per cell._ Design note 13 reads the body's density, which keeps the
    margin at five cells but gives a Mars no dichotomy in its small craters. A per-cell density
    read at an octave cell's canonical point would reach 6–8 coarse cells, beyond the margin. Lean:
    keep the body's density; if regional ages are wanted, modulate by the crater's own centre cell
    only for octaves whose search stays inside the margin.
  - _The arc–trench gap off Earth._ Earth's 190 and 260 km are set by slab depth beneath arcs
    (H 101 ± 16 km, England et al. 2004), which may be pressure-limited and so scale as 1 ÷ g
    (England and Katz 2010, from memory). Lean: Earth's distances, labelled; a geodynamics pass
    would settle it.
  - _The belt's shape._ The ridge–valley gain (1.8 at 1.5 breaks, width 0.6 in ln λ) and the
    hillslope steepening (0.4) are fitted to eight SRTM tiles of the Himalaya, Andes and Alps, at
    31 m and coarser. Metre-scale lidar of threshold ranges would test the 1 m figure.
- **Remaining checks, as researched** (R09.T0.b, 2026-10-09). Each item is settled in its design
  note; what stays open, with the lean and what would settle it:
  - _Open question 20, a science ruling for the owner_ (asked by T0.a: does accepting P14.T48.a's
    σ_h model, labelled empirical, rule the rendering brainstorm's open question 20?). Finding: yes,
    in substance. The question asks for three things, and T48.a answers each from this plan's
    research: σ_h published in place of the greatest relief, which the field reports as its
    realised relief instead; σ_h's gravity scaling, none in the structural share and 1/g in the
    basin and constructional shares (g^−0.22 overall with a 2.5× scatter over seven bodies, so not
    a function of gravity alone); and the lithosphere factor's part, min(1, T_e ÷ 70 km) on the
    constructional share and the edifice cap only, with T_e's isotherm now ruled (870 K). Lean:
    accept T48.a, and record open question 20 as ruled by that acceptance with three conditions
    written into the ruling: f_c's law (Design note 3) is part of the model, since the structural
    share is a function of f_c; Mercury's 21% over-prediction stays the out-of-sample measure,
    shown by T0.b not to come from its smooth plains, so T48.a's 25% tolerance on Mercury stays; and
    the age law is stated as global and uncalibrated in its middle (below). What would reopen it is
    a second out-of-sample body, such as Ganymede's σ_h from JUICE's laser altimeter. The brainstorm
    is not edited here; its revision is the owner's.
  - _The age law's middle._ No body tests √min(1, N ÷ 0.018) between saturation (3.8 Ga by the
    lunar chronology) and the young surfaces where the basin share is negligible, and partly
    resurfaced units follow no law in N (Mercury's smooth plains as rough as its older terrain,
    Mars's lowlands far smoother than the law). Lean: keep it, labelled; a globally resurfaced body
    3.0–3.8 Ga old would settle it, and the Solar System has none.
  - _An ice-rich crust's T_e._ The 870 K silicate isotherm, with k = 3.1 W m⁻¹ K⁻¹, does not
    describe an ice lithosphere. Lean: T_e = (651 W m⁻¹ ÷ F) ln(T_iso ÷ T_s), from ice's
    conductivity k = 651 ÷ T W m⁻¹ K⁻¹ (Petrenko and Whitworth 1999, from memory), with T_iso ≈
    170 K, this check's inversion of Giese et al. 2008's Enceladus (GRL 35(24),
    doi:10.1029/2008GL036149: an elastic 0.3 km and a mechanical 2.5 km at 200–270 mW m⁻², which
    give 150–200 K at T_s ≈ 70 K). A pass over icy-satellite flexure (Europa, Ganymede) would
    settle it before T12.c's provinces reach an icy world; until then T12.c applies the lean,
    labelled low confidence.
  - _f_c's distribution and mass._ Höning and Spohn 2023 treat Earth-sized planets only; the
    uniform [0.2, 0.7] draw spans their three outcomes without weighting them, and no mass or
    water-inventory dependence is modelled. Lean: as drafted in P14.T48.a; a continental-growth
    model run over planet mass would settle it. The draw needs a stream of plan 14's own (a body
    tag such as `body.continents`, proposed for its owner).
  - _The transport law beyond its range._ Kaspi and Showman's GCM has no seasons, ice, ocean
    transport or diurnal cycle, and spans Ω⊕ ÷ 24 to 12 Ω⊕ and 0.2–50 bar; beyond that the factors
    are held, and slower rotators lean on P14.T48.d's slow-rotator and locked regimes. FILLET's
    results paper, when published, re-checks D⊕ and T13.d's ice-edge states. The heat-capacity and
    molar-mass factors (Williams and Kasting 1997) are unverified; the 30 D⊕ guard bounds them.
  - _Venus's exponent elsewhere._ The taper's exponent 4.5 is fitted on Venus's named craters alone
    (the Gazetteer omits some small craters and the crater fields). Lean: one exponent for every
    body; Earth's small-crater record (Bland and Artemieva 2006) or Mars's catalogue of new impacts
    would test it under thin atmospheres, and Herrick and Phillips 1994's own fits remain unread.
    Refitted through the map (science-r09-craters.md F1), the exponent is 4.2. It depends on
    Venus's impact velocity (20, 25.7 and 30 km/s give worst cumulative errors of 7, 11 and 19%)
    and on the collapse law (Croft's 1.176 gives 22%), so it is tied to P14.T48.f's model.
  - _Erosion at the coarse cell._ At 36–40 km every cell carries a trunk channel (A ≥ 1.3 × 10⁹
    m²), so the transient solver lowers a whole cell at the trunk's rate, and T14.c's Mars volume
    (at least 1.2 m of global layer) could pass by over-erosion (about 100 m a cell at K t ≈ 1.7,
    the sub-agent's estimate). Ruled 2026-10-10 (science-hoke.md): T14.c bounds the coarse
    lowering from valley cross-sections (incision × valley width ÷ cell width, at most about 92 m
    on a 38 km cell), and records the cell-mean lowering against a fine-grid run of one basin; the
    estimate of about 100 m a cell at the calibrated K t sits at that ceiling, so the check may
    fail and is meant to. Howard 2007, Hergarten 2021 and Burr et al. 2006 were not read in full;
    Barnhart et al. 2009 stand in for Howard's constants.
  - _Plan 14's wet epoch._ P14.T48.b's "10⁵ to 10⁷ years … against 200–5,000 yr of continuous
    flow" and its "arid-to-semiarid Earth's" reference misdescribed Hoke, Hynek and Tucker 2011,
    whose times assume storms flowing 5% of the time, "as is typical for Earth", and whose
    200–5,000 yr is continuous hurricane-scale flow they reject. Ruled 2026-10-10
    (science-hoke.md): the reference is 5%; `effective_flow` is a point's flow time over 0.05,
    not the span; `WetEpoch` gains `runoff`; Mars is 2.2 × 10⁵ yr at 0.1 m a⁻¹ (Design note 9).
    Howard 2007 and Barnhart et al. 2009's full text were not read for it; Barnhart's 2% is
    DELIM's own clock.
  - _The strength regime._ The map is gravity-regime only. On low-g icy bodies, craters below
    Y ÷ (ρ_t g), kilometres at Y ≈ 1 MPa, are strength-dominated, and the map over-enlarges them,
    up to ×2 in D at g = 0.06 m s⁻². Lean: none until a body's small-crater counts test it;
    Holsapple's two-regime π_V form with K₂ and a size-dependent Y is the fix.
  - _The porous threshold._ Porous below a radius of 150 km is the plan's own, between Hyperion
    and Mimas. Above 60% porosity μ falls to about 0.35. Kraus et al. 2011 (ice scaling, CTH) was
    not read. The porous class's absolute crater size is uncertain. For one impactor, Singer et al.
    2019's same-shape assumption and Holsapple 2022's lab shapes bracket porous craters from about
    22% narrower than non-porous ones to the same width; the plan's per-class bowl constant lies
    between (F4.2). Singer et al.'s supplement is internally inconsistent (worked example against
    S2/S4, 13–31%).
  - _Peak rings and basin rings are not drawn._ From 9 D_t the central peak fades over the
    protobasins (science-r09-craters.md F2). The peak ring that replaces it is not drawn: its
    heights grow like central peaks' (Baker et al. 2012). Its ring-to-rim ratio and cross-section
    need Baker et al. 2011 (Icarus 214, 377), which was not read. Lean: a later shape task draws it
    from that source.
  - _Ice transitions at very low gravity._ 1/g × 0.12 overstates D_t on the smallest icy moons:
    Tethys's ~10 km against 25 km (White et al. 2017), with Mimas Schenk's standing exception, and
    Aponte-Hernández et al. 2021 fit g^−0.4 across all icy bodies. Lean: keep 1/g, which fits
    everything from Ganymede to Rhea and Charon; revisit when Schenk et al. 2021's table is read.
  - _Small icy craters._ No fresh crater under 1 km has a measured depth on any icy body. The
    lunar regolith rule is applied to icy airless bodies by physical analogy (a weak porous layer)
    and stretches to 2.9 km on Iapetus through D × g ÷ 1.62. Thin-atmosphere icy bodies (Pluto,
    Triton) keep the plain bowl, because the key is `Screening::None`.
- **The survey stand-in.** `survey_pass` lets a client grant its ship coverage, as the server
  grants detail levels today. It is a discipline for an honest client, and the sensors plan
  replaces the caller, not the core.
- **Field size.** Design note 17's layout gives about 14.7 MB for an Earth, about 1.0 MB inside
  R03's 15 MiB check: 13.96 MB with `decision-r09-t5.md`'s sea-floor byte, and about 14.75 MB
  since follow-up C's two runoff bytes (science-r09-drainage.md §1.3, whose 14.4 MB and 1.4 MB
  predate the sea-floor byte). The transfer budget's margin is now about 0.98 MB, 6.2% of the
  check, so any further per-cell byte (0.39 MB on an Earth at level 8) needs R03's check re-run
  first. If the climate layer at L − 1 proves too coarse for
  R11's clouds, sending it at level L costs about 2.1× (about 28 MB on an Earth), past R03's
  15 MiB check.
- **Dependencies.** Plan 14 calling the surface crate's crater density is a new edge that R04's
  split allows (the sim already depends on the surface crate). P12.T7 is built, and T18 builds on
  it. The channel network's physics profile replaces Dendry's own height reconstruction, which the
  paper did not test.
- **Asks of other plans, which their owners may not yet carry**, each with its state in the
  roadmap's asks table (states as of the re-validation of 2026-10-09):
  - plan 14's five of Design note 3: drafted by T0.a as P14.T48.a–e in plan 14's "Phase K",
    accepted with amendments 2026-10-10 (`signoff-3.md`);
  - plan 14's P14.T23 surface seed: carried by plan 14, built (sim side); the hooks section's wire
    value is this plan's T1.b;
  - plan 14's surface section and P14.T24.a–b's conditions and figures, which `for_body` reads:
    carried by plan 14, unbuilt;
  - plan 04's two table rows: accepted with amendments 2026-10-10 (`signoff-3.md`);
  - plan 12's P12.T7 store: carried by plan 12, built;
  - R03's `surface_revisions` field: accepted with amendments 2026-10-10 (`signoff-3.md`), its
    shape in R03's Design note 4; the note that the camera reports do not bound the field:
    carried (R03's Design note 6 and Risks already say so);
  - R10's use of the header's `albedo_scale`: carried by R10;
  - R11's use of the header's `surface_age` and `surface_pressure` for its `RockSite`: carried by R11
    (`rock_site`);
  - R11's zonal precipitation (below): open. Its monthly wind is met by T2's twelve winds per
    climate record (`decision-r09-t2.md` item 1).

  R10's three asks of this plan (per-contribution variance and structure function, resolution per
  cell, the albedo-scale field) are met in Provides.

- **Brainstorm departures for its revision.** Design note 13's channel network departs from the
  brainstorm's per-query evaluation in three ways (the first level from `flow` not the lowest
  Moore neighbour, a 3 × 3 search not 5 × 5, one extended network not stacked instances), and
  Design note 15's crater reach is 1.27 D, not "about twice its radius"; all four are now in the
  roadmap's brainstorm corrections ("Terrain, the surface and Knowledge"), awaiting the revision.
  R09.T11 adds a fifth, signed off 2026-10-16 (`signoff-11.md`, item 20) and applied to the
  brainstorm with the roadmap's row: the brainstorm's "Each coarse cell takes the nearest plate
  seed, by the largest dot product … in milliseconds" and its Decisions' "rasterises plates as a
  Voronoi diagram" become a power diagram, x′ · p + w with a weight fitted per plate, since an
  unweighted diagram of uniform seeds has cells of comparable size and cannot follow Design note 6's
  size law; the fit costs some 0.2–0.4 s at level 8 (provisional).
- **Asked by R11, not yet designed here** (the roadmap's asks table carries it). The height datum is
  settled as the rotational spheroid (Design note 17, after R07's Design note 19), with the sea
  level a height above it, so R10's datum ask is met. R11 asks for the terrain-independent zonal
  part of the precipitation heuristic (the rain band on the energy-flux equator, the dry belts and
  the storm tracks, by latitude and month) as a function the client can call from plan 14's global
  figures without the coarse field, so that clouds are drawn over unsurveyed ground (R11 Design note
  8). The second ask, whether `ClimateCell.wind` is seasonal, is met: the record holds each month's
  10 m wind, on the months of Design note 8 (ruled 2026-10-09, `decision-r09-t2.md` item 1). That
  costs 16 B more per climate record, about 1.6 MB on an Earth.
- **Found by the ruling on T2's questions** (`decision-r09-t2.md`, "Adjacent findings",
  2026-10-09).
  - _Slow and resonant rotators' climate (T13.b, R11)._ On a world that is not locked but whose
    solar day is not short against a month (a Venus, a 60-day day, a 3:2 resonance, whose
    insolation repeats over two orbits, and the Moon itself, whose solar day is 29.5 d against a
    month of 30.4 d), the substellar point moves through the body-fixed cells
    across the year. A month's body-fixed record is then a climatology only if it is averaged over
    the sun's body-fixed longitude: over the resonance's period when the spin is commensurate, or
    until that longitude is sampled evenly. Otherwise it is one year's weather, which the game
    would show forever. T13.b settles the averaging. Such a world's day–night circulation is then
    not in the climate layer, so R11's terrain-independent zonal function for it would need
    sun-fixed coordinates, for R11's re-validation. The winds' cadence does not change.
  - _Which worlds have one month_ (the T2 fix's science review, for the specification). Design note
    8's "a locked world on a circular orbit" read literally gives a moon locked to its planet, whose
    planet's orbit is circular, one month, though it sees its planet's obliquity (a Titan, under
    Saturn's 26.7°); and a world locked 1:1 to its star lacks seasons only at zero obliquity. The
    physical condition is e = 0 with no obliquity to the seasonal orbit; the lock adds nothing.
    Lean: word it so when the specification is next revised; the header's checks (one month only at
    e = 0) already hold under either reading.
  - _The synthetic winds are not T13.e's._ T2's fix centres the whole three-cell pattern on each
    month's flux equator, an illustration. The Coriolis parameter changes sign at the geographic
    equator, so air crossing it turns eastward: the summer hemisphere's low-level winds near the
    ascending edge are westerly (Guendelman, Waugh and Kaspi, J. Atmos. Sci.,
    doi:10.1175/JAS-D-21-0019.1, about 15° N in the Indian monsoon and 30° S on Mars in southern
    summer; the ruling's Arabian Sea in June blows towards 65°). T13.e centres the meridional
    structure on the flux equator but takes the zonal sense from the geographic hemisphere between
    the two equators.
  - _One transition diameter for Mercury and Mars._ Design note 12's D_t = 19 km × (1.62 ÷ g) ×
    k_target gives both 8.3 km (g 3.70 and 3.71 m s⁻²), but Susorney et al. 2016 measure 11.7 ± 1.2
    km on Mercury and Robbins and Hynek 2012 about 6 km on Mars (5.9–7.0 km by method): the known
    Mercury–Mars anomaly, which Susorney et al. could not trace to the target or the impact
    velocity. The ruling's crater shapes match Mercury's MLA depths best with the formula's 8.3 km
    (with 11.7 km they would come out 16–31% deep, 22% at 50 km), so nothing changes. It is recorded
    for T7.a's D_t test, which checks only the Moon and Earth.
- **The 61-chunk transfer check.** R03.T15's 15 MiB (61-chunk) check is this plan's, after RM3
  (decided 2026-10-07 by the orchestrator). The coarse field, about 15 MiB, is the first bulk kind
  of that size: no sky reaches it (R06's largest is 7.5 MB, 29 chunks; R06.T11.b ran the check at 4
  chunks, recorded in R06's Risks, "Deviations in T11.b, as built", and R03's T15 note). Found at
  the re-validation of 2026-10-09: the server binary serves a field only for a body whose inputs
  are modelled, which waits on P14.T24 and P14.T48, and the page that asks is R10's client, so the
  check stays pending after T19.c until both exist (T19.c says so); a reference-world field served
  by the binary would be test-only code reachable over the network, which R03 declined to build.
  (This bullet opened with R03's task ID in bold, which `plan_task.py` read as a task `R03.T15` of
  this plan; it now opens with a title.)
- **Re-validated at `bce2aef5`** (R09.T0.a, 2026-10-09, against R03–R05, P12.T7 and plan 14 as
  built, merged to `main` in PR #3). No brainstorm drift on this plan's topics since it was written
  (`git diff 9d955b26 -- docs/agent/brainstorming/rendering-and-planets.md`: the finest spacing now
  reads at most 0.375 m, which this plan does not state, and open questions 4, 5, 9 and 20 are
  unchanged; 20 is still **Open**, so the σ_h ask rests on this plan's research and its acceptance
  rules the question). What changed here:
  - _Names and paths._ R05's `geometry::{FINEST_SPACING_M, BAND_LIMIT_M, finest_level}`,
    `noise::LatticeCache` (which T4 no longer moves; only `HeightSample` moves),
    `spheroid::Spheroid` and `patch::collision::finest_surface_height`; base's `Gigayears` for
    `Gyr`, and `PerSquareKilometre` and `SquareMetres`, which do not exist, added by T2; the tag
    golden is the sim's (`crates/hyperion-sim/tests/golden/rng/tags.golden`); the surface registry
    is not empty (R05's `selftest.surface.test_planet`).
  - _Built since the plan was written._ P14.T14.a–c and P14.T23 (`SurfaceSeed` is the sim's, so T1.a
    moves it to base), P14.T46's figure and rotation sections (so the datum is the spheroid from the
    start), P14.T47 and P12.T7 (so T18 no longer waits). T1.b no longer waits on P14.T23, but its
    hooks section moves P14.T32's goldens and needs a `GENERATOR_VERSION` bump.
  - _Corrections so the plan executes._ `surface.relief` is keyed by lattice corner as
    `surface_item`, as R05's noise keys it, not `surface_cell`; T5 first generalises R05's
    `Octave` and `LatticeCache` from the test planet's `Seed` and tag, bit for bit; `Stream::open`
    gains the surface scopes' refusal (it checks only scope equality); T2 defines the header's data
    types and the crate's `testing` feature; T9's goldens go to `tests/golden/height/`, and both
    `TEST_PLANET_VERSION` checks are narrowed; T18 factors `persist.rs`'s private log helpers; T19.a
    lists the seven places a kind touches, and T19.b the field's serde form and merge; the server's
    dev-dependencies gain the sim's `testing` feature; new bench files (`benches/synth.rs`, the sim's
    `benches/surface.rs`) and the sim's first example; acceptance commands that name `just ci` name
    the lane's targeted checks, `just ci` and `just ci-slow` being the orchestrator's.
  - _Not this plan's._ R05's T10.b record says "R09 adds the export that copies [the field] in";
    the client and its wasm entry points are R10's (R10 Design note 16, T6–T7), and this plan
    builds no client code.
  - _Task readiness._ Ready now: T0.b, T1.a and T2 (beside each other). Then, each after what it
    names: T1.b after T1.a, in galaxy plan 14's 21 → 22 batch; T3 and T4 after T2; T5, T6.a and
    T10 after T1.a and T2; T7.a after T2 and T0.b's Venus item; T7.e after T7.a; T7.b after T7.e;
    T7.c after T7.b; T6.b after T6.a; T8 after
    T4–T7; T9 after T8; T11 after T10; T12.a–b after T11; T12.c after T11 and T0.b's isotherm item;
    T12.d after T10 and T7.e; T12.e after T12.a, T4 and T5; T13.a after T10 and T0.b's transport-cap
    items, T13.b–e after it; T14.a after T12; T14.b after T14.a, T4 and T0.b's erodibility item;
    T14.c after T14.b and T6.b; T14.d after T14.c and T13; T15 after T13 and T14; T16 after T15;
    T17 after T3 and T10; T18 after T2; T19.a after T17 and T18; T19.b after T19.a and T8; T19.c
    after T19.b; T20 last. No task waits on an owner's sign-off or on an unbuilt galaxy-plan task;
    what waits on P14.T24 and P14.T48 is a generated world's surface (`for_body`) and the 61-chunk
    check, not a task.
- **Deviations in T1.a, as built** (2026-10-09).
  - _Key scope._ `ObjectKey::surface_cell` and `surface_item` both give their keys
    `TagScope::SurfaceCoarse`. The seeds' `stream` reads only the tag's scope, so the key's scope
    only keeps a surface key from every tag but a self-test one under `Stream::open`, which refuses
    both surface scopes before its own check (`tests/panics.rs` holds both).
  - _Face refusal._ `surface_cell` also refuses a face of 6 or more (`SurfaceCellKeyError::Face`,
    beside `Level` above 28 and `Coordinate` at 2^level or more), since the 3-bit field could hold 6
    and 7. The error keeps the Provides name, which R11 consumes.
  - _Seeds._ Both live in base's `rng/surface.rs`, with `new`, `get`, `stream` and a 16-hex-digit
    `Display` (P14.T23's on `SurfaceSeed`; `DetailSeed`'s matches the wire's `DetailSeedHex`). Their
    `stream` panics, in release builds too, on a tag of any other scope, `SelfTest` included. The
    sim's `hooks/seed.rs` re-exports the type, so `planetary::hooks::SurfaceSeed` and
    `planetary::hooks::seed::SurfaceSeed` both hold; P14.T23's display test changed one token,
    `SurfaceSeed(0xab)` to `SurfaceSeed::new(0xab)`, since the tuple field is private to base.
  - _The key-form rule's test._ The table (Tag, Scope, Key, Draws) is `hyperion_surface::tags`'s
    module documentation. `every_surface_tag_has_one_documented_key_form` holds it to the registry:
    one row per tag but the self-test ones, scope and declaration matched, and `surface.coarse.*`
    exactly the `SurfaceCoarse` tags.
    `native_only::each_surface_tag_s_call_sites_use_its_documented_key_form` reads every `.rs`
    file under `crates/` but the registry's own: each `.stream(` or `::stream(` call naming a
    surface tag's constant (its name upper-cased, dots to underscores) must name that tag's key
    constructor and not the other, and no `Stream::open(` may name one. So T5–T7 and
    T10–T14 name the tag's constant and the key constructor in the one `stream` call. A key built
    before the call fails the test, and a tag passed in by value (the likely shape of T5's
    generalised `Octave`) or held under another name is not seen, so T5 keeps the constant at the
    call or extends the test.
  - _Goldens added_ (new files, header 21, so extensions only): base's `rng/surface_streams`
    (`surface_streams_are_pinned`: the packing of six cell keys, and the first words of each seed's
    stream for cell and item keys under test-minted tags of each scope), and the sim's
    `planetary/surface_seed` (`surface_seeds_are_pinned` in `planetary_golden.rs`: six bodies'
    seeds in four universes, which nothing pinned before, checked against P14.T23's own `seed.rs`
    before the move, so the seed's value did not move). The tag golden gained nine lines, the eight
    `surface.*` after R05's entry and `body.surface.detail` last; `golden_diff.py` reports
    "Extended only", and `GENERATOR_VERSION` stays 21.
  - _The detail seed's word._ `body.surface.detail`'s doc comment fixes T1.b's `detail_seed` as word
    0 of the body's stream, as `surface_seed` is of `body.surface`.
  - _Acceptance._ The four commands build none of the sim's unit tests, so T1.a also ran
    `cargo test -p hyperion-sim --lib -- planetary::hooks rng::` and
    `cargo test -p hyperion-sim --doc hooks` (P14.T23's tests, `registries_are_disjoint` and
    `the_disjointness_check_covers_r09_s_names`).
  - _Found for R11._ R11's `RockCellKey::object_key(&self, instance: u32)` cannot pass a `u32` to
    `surface_cell`, whose `instance` is `sub`, 16 bits wide; R11's re-validation narrows it to
    `u16` or checks it. And since the body is in a 64-bit seed rather than the key, bodies whose
    seeds collide (about n² ÷ 2⁶⁵ pairs among n bodies, as for P14.T23's seed already) share their
    noise, though not their coarse inputs.
- **Deviations in T1.b, as built** (2026-10-14).
  - _Lane and version (main's ruling, 2026-10-14)._ Built in its own lane, `r09-detail-seed`,
    rather than by the batch's lane, and held at `GENERATOR_VERSION` 21: the batch's lane
    (`p14-bump`, then building P14.T48.e) picks this commit before its bump commit, whose bump
    re-blesses every golden at 22. So, unlike plan 14's bump plan ("the goldens move once, at the
    bump"), this commit puts the hooks on the record and re-blesses the system goldens it moves at
    header 21 inside itself; the bump re-blesses them at 22 with the batch's own changes.
  - _The seed._ `planetary::hooks::detail_seed(seed, body)` is word 0 of the body's
    `body.surface.detail` stream, beside `surface_seed` in `hooks/seed.rs`; `hooks` re-exports it
    and `DetailSeed`. Its test, `a_million_detail_seeds_are_distinct_and_none_is_its_surface_seed`,
    takes P14.T23's 10⁶ bodies.
  - _`record::Hooks`_ is a struct with a private `detail_seed: DetailSeed`, `Hooks::new` and the
    getter `detail_seed()`. It derives `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, `PartialOrd` and
    `Ord`, but not `Copy`, since the hooks to come (resources' abundances) need not be; the server's
    `convert::planetary::hooks` takes `&Hooks`.
  - _Which bodies._ The section is `Ok` for a planet, moon or dwarf planet present at the record's
    time (a moon by the record's present arm, which also needs its planet's position), giants
    included, as the task's "a present body" says and since a giant has a surface seed too. A
    private `PlanetarySystem::present_hooks` builds it; `BodyRecord::builder` still starts every
    section `NotModelled`. A ring, a belt and the halo keep
    `NotApplicable`. A planet, moon or dwarf planet **not present** had `NotModelled` hooks, not
    `NotApplicable` as the task's "keeps" says: T1.b makes them `NotApplicable`, as their bulk and
    surface are.
  - _The goldens, re-blessed at header 21_ (the batch's bump re-blesses them at 22). The system
    goldens' writer prints an `Ok` hooks section as `hooks detail_seed = 0x…`, pinning the value,
    and every other state as before. 15 files under `crates/hyperion-sim/tests/golden/planetary/systems/`
    moved, 800 lines, each a `hooks: NotModelled` line replaced in place (no line added or removed):
    722 became `hooks detail_seed = 0x…` and 78 `hooks: NotApplicable` (bodies not present). By
    file (replaced lines, of which `NotApplicable`): `close_binary` 144 (0), `eccentric_giant` 24 (0),
    `fallback_black_hole` 40 (2), `filler_a` 8 (2), `filler_b` 56 (0), `filler_c` 70 (0),
    `halo_star` 4 (0), `hierarchical_triple` 108 (4), `hot_jupiter` 18 (0),
    `m_dwarf_resonant_chain` 12 (0), `red_giant` 42 (4), `solar_like` 42 (0), `subgiant` 90 (0),
    `t_tauri` 62 (62, every body not yet formed) and `wide_binary` 80 (4). No other line of any
    golden moved. Added: `planetary/detail_seed.golden` (`detail_seeds_are_pinned`: the six bodies
    in four universes of `surface_seeds_are_pinned`, through one helper, `body_seeds`, which leaves
    the surface seed golden unchanged).
  - _The wire._ `BodyHooksDto` keeps the shape R04.T3.c gave it; only its doc comments and
    `BodyRecordDto.hooks`'s changed (the bindings regenerated, comments only). The new
    `an_ok_hooks_section_carries_the_detail_seed_alone` pins an `ok` hooks section as whole JSON,
    so it holds no other key; it does not name the surface seed's key, which keeps R04.T3.c's
    acceptance grep empty. `PROTOCOL_VERSION` stays 2: no message, field or string is added,
    removed or changed, no build sent an `ok` hooks section before (the sim's `Hooks` was
    uninhabited), so by plan 04's Design note 15 this first send sets its shape, and every client
    since R04.T3.c parses it (`bodiesWire.ts`'s `toRecord`) and shows its `DETAIL SEED` row.
  - _Tests._ The sim's new `a_present_body_s_hooks_are_its_detail_seed` holds every record of 120
    systems at the epoch to the rule above, with the detail seed `detail_seed` gives and unequal to
    the body's surface seed, and counts the seeded bodies, the bodies not present, the rings and
    the belts it saw; `a_record_tags_what_the_generator_does_not_compute` loses its planets'
    `NotModelled` hooks assertion. `record.rs`'s mixed record carries an `Ok` hooks section, so the degrading tests
    withhold one. `tests/system_bodies.rs` holds each body's `body_detail` hooks to the sim's detail
    seed or `not_applicable`, and reads that answer and the system's list as their text arrived:
    neither has a key named `surface_seed` at any depth, nor a body's surface seed in hexadecimal
    or decimal (the list any body's, a record its own). The checks are `Secret`,
    `check_no_seed_key` and `has_key` in `tests/common/surface.rs`, taken from T19.c's
    `surface_knowledge.rs`, whose private copies T1.b leaves for T19.c's follow-up to drop (moving
    its payload byte check, `check_binary`, into the shared `Secret`).
  - _For T19.c's follow-up._ `tests/surface_knowledge.rs` still synthesises from its stand-in
    `DETAIL_SEED`. Reading the client's seed from `body_detail`'s hooks section (`Session::start`
    already asks for it) means keeping the answer's seed in `Session` and passing it to
    `check_heights`, more than a one-line switch, so T1.b leaves it; the seed it will read is
    `detail_seed(Seed::new(SEED), surface_system().solid)`.
  - _Left as it is._ `packages/protocol/fixtures/planetary.json`, a hand-written fixture, keeps a
    `full` record whose hooks are `not_modelled`, still a valid wire state.
  - _Acceptance._ The sim's filter `planetary::hooks` selects only `hooks/seed.rs`'s tests and the
    hooks doctests, so T1.b also ran
    `cargo test -p hyperion-sim --lib -- planetary::hooks planetary::record a_present_body_s_hooks a_record_tags_what_the_generator_does_not_compute`
    and `cargo test -p hyperion-sim --test planetary_golden`, beside
    `cargo test -p hyperion-sim --doc hooks`, `cargo test -p hyperion-protocol planetary` and
    `cargo test -p hyperion-server --test system_bodies`; the gate is `just smart-ci`.
- **Deviations in T2, as built** (2026-10-09; technical choices, revisable until T9's and T16's
  goldens pin them).
  - _Files._ `field.rs` is a module root over `field/{cells,cover,crater,header}.rs`, re-exported
    at `field::*`; `synth.rs` and `craters.rs` hold only the header's types; `testing.rs` and
    `testing/worlds.rs` sit behind `cfg(any(test, feature = "testing"))`. The sim's `testing`
    feature is `["hyperion-surface/testing"]`. The surface `clippy.toml`'s `doc-valid-idents` gains
    `McMahon`.
  - _Public items added beyond Provides._ `cell_at_index(level, index)` (the inverse of
    `cell_index`, which numbers levels 0 to 14 and panics deeper); the `CoarseLevel` constants
    `MIN` and `MAX` and its `new`, `get`, `cell_count`, `climate_level` and `climate_cell_count`;
    `CoarseField::new` with `BuildFieldError`, its `synthesis`, `climate_layer` and `craters`
    slices, and `with_albedo_scale`, by which R10.T10.a sets the scale without rebuilding the
    field; `FieldHeaderParts` (public fields; `FieldHeader::new` validates them and derives
    `level` and `boundary_diameter`) with `BuildFieldHeaderError`, the getters, `parts`,
    `into_parts`, `with_albedo_scale`, the climate readers `sea_level_temperature` and
    `month_temperature`, the quantisers `quantise_sea_level_temperature` and
    `quantise_month_anomaly`, the step values `temperature_step_k` and `anomaly_step_k` and the
    step choosers `temperature_step_for` and `anomaly_step_for`; `BodyRef` (raw system ID and body
    index, since the crate cannot see the sim's `BodyId`); `CoverRange`, `BuildCoverError`; the
    cell enums and codes `Crust`, `BoundaryKind`, `FlowDirection` (to and from R05's `Edge`),
    `Morphology`, `LogArea`, `LogSteepness`, `LogPrecipitation`, `SurfaceClass` and `Wind`, each
    byte enum with `ALL`, `From<_> for u8` and `TryFrom<u8>` (`DecodeFieldCodeError`), and
    `QuantiseValueError` for every quantiser; the crater types `Screening`, `CraterParamsParts`
    (which `CraterParams::new` takes) and `BuildCraterParamsError`; `NewBandLevelError` and
    `BuildBandSpectrumError` in `synth`; `PlateSpec`, `CellSite`, `ClimateSample`, `Routing` and
    `SyntheticWorld::ALL` in `testing`. R05's `cube` gains `PatchKey::containing(level, dir)` (a
    direction's cell, the same face rule), and `geometry::max_rate` (S2's 1.704 897) is
    `pub(crate)` for `boundary_diameter`.
  - _Names._ The header's parts are read through getters (`sea_level()`, `craters()`,
    `surface_age()`, `surface_pressure()`, `albedo_scale()` and the rest), not fields; Provides'
    `lapse_rate` is `lapse_rate_k_per_m`, its `D_b` is `boundary_diameter()`, its
    `boundary_distance` is `boundary_distance_km`, its `g` is `gravity`, and the steepness index's
    readers are `LogSteepness::from_index_m0_9` and `index_m0_9`. `BoundaryKind` adds `Absent` (a
    stagnant lid's cells) to Design note 6's four kinds. `synth.rs` is the `synth` module's root,
    as `field.rs` is `field`'s: T4–T7.b's `synth/{interp,relief,channels,craters}.rs` are its
    children, and T8's `synth/mod.rs` is `synth.rs`.
  - _The header._ No `format` or `generator_version`: T3's block header carries both and refuses
    others, so a header in memory is this build's. It gains `reference_temperature` (plan 14's
    mean surface temperature, P14.T24.a's) and `temperature_step` (n in 0.01 K × 2ⁿ):
    `sea_level_temperature` is the departure from the reference in that step, since an `i16` of
    0.01 K holds only ±328 K and a Venus is at 737 K, the same no-saturation rule as
    `anomaly_step` (both exponents 0–15). The two join Generator version's reserved list. The
    exponents keep the plan's names, `temperature_step` and `anomaly_step`, beside the kelvin
    values `temperature_step_k` and `anomaly_step_k` (the review's `_exponent` rename was declined
    for the plan's name). `months` is 1 or 12; `season_eccentricity` is the months' orbit's e, in
    [0, 1); `Screening` is `None`, `Atmosphere` (its
    `column_mass` and `projectile_density`) or `Cutoff` (a `diameter`). `ClimateModelKind` is
    `EnergyBalance`, `LockedEnergyBalance`, `RadiativeEquilibrium` or `Isothermal`;
    `PrecipitationSource` has `Heuristic` alone.
  - _`BandSpectrum` is the law, not a table_: the structural share's degree variance, V₁ l^−β₁
    below the break degree l_b and V₁ l_b^(β₂−β₁) l^−β₂ from it (`decision-r09-t5.md` item 2; β₁
    default 2.0; V₁ in `SquareMetres`). T5 derives each level's amplitude from it.
  - _The codes_ (each documented with its SI meaning). `boundary_distance_km` is negative on the
    plate that subducts at a subduction boundary and positive elsewhere (a lean for T11 to keep or
    change), with `SynthesisCell::NO_BOUNDARY_KM` = `i16::MIN` on a stagnant lid, outside the
    ±32,767 a distance saturates at; `boundary_obliquity` in 90° ÷ 255; `drainage` a `u16`,
    2^((c − 1) ÷ 1,024) m², and `steepness` a `u8`, 2^((c − 64) ÷ 8) m^0.9 with θ = 0.45, which
    keeps the 21 bytes; `ice` the share under ice in 255ths; `month_precipitation` is
    `[LogPrecipitation; 12]`, 0.1 mm a year × 2^((c − 1) ÷ 12) in kg m⁻² s⁻¹; `Wind` is an azimuth
    the air moves towards in 256ths of a turn from local north and a speed of 0.01 m/s ×
    2^((c − 1) ÷ 16), and the twelve winds are each month's 10 m wind (the resultant's direction,
    the mean speed), with a one-month year's eleven later winds calm (`decision-r09-t2.md` item 1);
    a crater's `degradation` is the
    share of its fresh rim relief lost, in 255ths. `Crust` adds `Lid` and `Province` (a stagnant
    lid's crust and its volcanic provinces) to the two populations. `SurfaceClass` and
    `crater_state` are bytes whose codes T15 defines: 0 (`UNCLASSIFIED`) until then, in every
    synthetic world too.
  - _Closed sets, for the composition audit_ (the owner's directive of 2026-10-09). `Crust` (four),
    `BoundaryKind` (five), `Morphology` (five), `ClimateModelKind` (four) and
    `PrecipitationSource` (one) are closed sets today. Each is a one-byte code whose unknown codes
    are refused (`DecodeFieldCodeError`), so a new variant takes a new code, gated by T3's
    `SURFACE_PAYLOAD_FORMAT`. The cell's `ice` share and `month_precipitation` name no species:
    the condensable is plan 14's, per body (P14.T48.d), so a world with two ices (water and CO₂ on
    a Mars) has no per-cell species yet; `SurfaceClass`, T15's, is where the liquids, ices and
    frosts of named species go, with 255 codes of room.
  - _Validation._ `CoarseField::new` refuses wrong record counts, water below ground, a boundary
    kind without a distance or the reverse, a month outside the year (or an anomaly in a one-month
    year, or a wind that is not calm past its first),
    a crater off unit length, narrower than D_b, of negative age, whose
    reach misses its centre's cell or leaves the field, or not strictly after its predecessor in
    (centre cell, diameter): the key is unique, so that T3's `PartialField` merges and dedupes the
    craters of several blocks by it and the synthesis sums them in one order (the determinism
    review). T12.d must therefore never emit two craters of one centre cell and diameter. Reaches
    whose index would overflow a `usize` (a `u32` on WebAssembly) are `TooManyReaches`. A crater's
    age is not bounded by the surface age, which is a body's mean.
  - _The synthetic worlds_ (closed forms, no stream). Earth-like level 8 (ten plates, ocean 0.69,
    σ_h 2.4 km, ice 0.12, three craters), Mars-like 7 (dichotomy, Tharsis, dry routing, 23
    craters), Moon-like 6 (maria, 45 craters, SPA reaching most cells), Ceres-like 5 (28 craters,
    k_target 0.12). Only the Earth- and Mars-like worlds are routed and carry ice; the others are
    terminal everywhere with no drainage. Flat and OneCrater are at the Moon's radius, level 6,
    with a one-month year, V₁ = 0 and a crater density of zero (`FieldBuilder::new`'s defaults),
    so that the synthesis adds nothing to Flat and nothing but OneCrater's one 300 km crater, which
    sits on the face 0–2 edge with its reach on both faces; T5's variance test has nothing to
    compare on them. The header's realised σ_h and relief are the cells' area-weighted RMS and
    range, not the reconstructed field's (T4's interpolant does not exist yet), and
    `reference_temperature` is the climate layer's area-weighted mean. `FieldBuilder` routes by
    steepest descent without depression filling (sinks are terminal), takes a reach as every cell
    whose centre is within 2.54 rim radii plus the cell's circumradius (a superset of "touches"),
    samples Design note 13's profile at cell centres rather than smoothing it, and computes D_t
    and the depth inline from Design note 12 (T7.a may switch it to its functions). Earth-like
    builds in 0.6–0.8 s at the dev profile under shared load; every world twice in under 2 s.
  - _Science review._ Every figure turned into code checks against its source. Two corrections
    for the plan's text, both ruled in `decision-r09-t2.md`. Design note 17's Verkhoyansk
    anomalies are now −30.8 and +30.6 K (item 2; the 0.25 K step is unaffected; the header's doc
    quotes the normals). And the crater rims. Design note 13's 0.036 D^1.014 is Pike 1980b's
    (USGS PP 1046-C, Table 6, eq. 3), a different Pike 1980 from Design note 12's. One ratio of
    5.42 did make complex rims about half the observed. Item 3 gives complex craters their own
    self-similar depth and rim laws, which T7.a builds. Ceres's 469.7 km is Ermakov et al. 2017's;
    Design note 12's bounds of 9 and 16 D_t (0.8 and 1.5 now match Krüger et al. 2018) and the −1.7
    to −2.05 spectral range carry no external source and are labelled the plan's own; the 0.12 ice
    factor is sourced by science-r09-craters.md item 1.
  - _For T3, T4, T9 and T18._ `SYNTHESIS_MARGIN_CELLS` is not yet defined (T3 or T4 adds it).
    `PartialField` can reuse `field.rs`'s private `reaching_index` (which returns `None` on
    overflow) for its crater index. The decoders (`LogArea::area`, `index_m0_9`,
    `rate_kg_per_m2_s`, `Wind::speed`, `ResolutionCode::resolution` and the header's temperature
    readers) are pinned at a few points only; the determinism review asks T3's or T9's goldens to
    write every one-byte code's value and a sample of `LogArea`'s. T18 takes `CoarseLevel`,
    `cell_index` and `ResolutionCode` from here.
  - _The months_ (`decision-r09-t2.md` items 1 and 2, applied after T2 landed, 2026-10-09). The
    record is 50 B: `ClimateCell.wind` is `[Wind; 12]`, each month's 10 m wind on the months of
    `month_anomaly` and `month_precipitation`, and `CoarseField::new` refuses a one-month year
    whose `wind[1..]` are not calm (the old rule, four equal winds, is gone).
    - _Files and public items._ `field/months.rs` holds `MonthBlend` (private fields, read by
      `from`, `to` and `weight`; its `Default` is month 0 alone) and `month_blend`, as Provides
      has them, and three items beyond Provides: `month_edges(season_eccentricity)`, the twelve
      months' edges in mean anomaly (`Option<[Radians; 13]>`, `None` outside [0, 1)), for T13.a's
      binning and T15's durations, since the pass bins before it has a header;
      `month_share(header, month)`, a month's share of the period; and
      `month_at(header, mean_anomaly)`, the month that holds a time. The header's
      `season_eccentricity` is refused outside [0, 1) (`BuildFieldHeaderError::SeasonEccentricity`)
      and above 0 in a one-month year (`EccentricOneMonthYear`), since a one-month year's orbit is
      circular.
    - _No Kepler solve._ The task text has `month_blend` find the month by Newton steps on
      Kepler's equation. The edges are mean anomalies already, M_k = k · 30° − e sin(k · 30°),
      from exact sines (√3 ÷ 2 the nearest `f64`), so `month_at` and `month_blend` compare the
      mean anomaly with them: the same months exactly, at every e below 1, with no iteration
      count to choose and nothing but the four operators and `math::fmod`'s exact reduction. A
      mean anomaly is any finite angle; a non-finite one panics (`tests/panics.rs`). The blend's
      weight is linear in mean anomaly between the two nearest centres, and the span across
      periapsis is computed alike from both sides (Sterbenz).
    - _The synthetic worlds._ Earth-like e = 0.016 711 23 and Mars-like 0.093 394 10, Standish
      and Williams 1992's J2000 elements (the ruling's 0.0167 and 0.0934 to more places). The
      ruling's "Moon-like and Ceres-like one month as now" does not match T2, which gave both
      twelve months, nor the ruling's own item 4: neither lacks seasonal forcing (the Moon's
      seasons are its planet's orbit's, e 0.0167, and Ceres has its own e and a few degrees of
      obliquity: 1.5° and 4.0°). So both keep twelve months (adopted by "main", 2026-10-09), the
      Moon-like at Earth's e and the Ceres-like at 0.079 692 295 (JPL's osculating orbit, SBDB
      solution 48); Flat and OneCrater keep one month at e = 0. The Earth- and Mars-like winds
      are the three-cell pattern centred each month on a flux equator (the Earth-like rain band's
      latitude, and on the Mars-like the subsolar latitude, 25.19° into the summer hemisphere at
      the solstices), so their twelve winds differ month by month (an illustration, not T13.e's
      rule: Risks, "Found by the ruling on T2's questions"); the Moon- and Ceres-like are calm.
      Their anomalies carry no global distance term, (e ÷ 2) T̄ cos E, which is a few kelvin; the
      fixtures stay illustrative. `FieldBuilder::season_eccentricity`
      sets e (0 by default). The climates stay closed forms in each month's middle eccentric
      anomaly, 2π (m + ½) ÷ 12.
    - _The codec_ (T3 landed first, so this commit makes follow-up A's codec edits, of T3's
      "For T2's follow-ups"). `wire/form.rs`'s `FieldHeaderParts` table gains
      `season_eccentricity: f64` after `months` (8 B in block 0's header), its `ClimateCell` table
      holds `[Wind; 12]` (a 50 B stride), and `check_climate_cell`, which `CoarseField::new` and
      `PartialField::insert` share, refuses a one-month year's later winds that are not calm.
      The Earth-like world's whole payload is now 13,174,828 B in 13 blocks (11,601,760 in 12),
      the Mars-like 3,315,560 in 4 and the Moon-like 837,756 in 1; each single-block payload grew
      by 8 B plus 16 B a climate record. `tests/golden/wire/payloads.golden`'s 18 block, byte and
      digest lines are re-blessed at 21 with no bump, and its held covers and `codes.golden` do
      not move (the determinism ruling of 2026-10-09; Generator version). A new
      `tests/golden/field/months.golden` (an extension, header 21) pins `month_edges`,
      `month_share`, `month_at` and `month_blend` to the bit on five orbits to e just below 1, at
      signed zeros, a lift that rounds, many turns and a large angle, as the determinism review
      asked; `every_field_golden_carries_the_generator_version` checks its header.
    - _Review._ Declined: the rust-reviewer's rename of `MonthBlend::from` and `to` to `earlier`
      and `later`, since Provides names them. Adopted: a −0 season eccentricity is refused, so a
      circular orbit has one wire form, and a mean anomaly reduces to +0, never −0.
    - _For T13, T15 and T16._ T13.a bins by `month_edges`, T13.e fills each month's wind, T15
      counts months by `month_share`, and T16's goldens pin them on the reference worlds.
  - _The palette and the substance byte_ (follow-up B, decision-composition §1.1 and §1.7,
    2026-10-10; revisable until T19's first send). It makes B's codec rows too, as T3's "For T2's
    follow-ups A and B" asks.
    - _Files._ `substance_key.rs` is new, since P14.T49.a is not built and finds it there.
      `field/palette.rs` is new, re-exported at `field::*`. The surface `clippy.toml`'s
      `doc-valid-idents` gains `McLennan` and `WebBook`.
    - _`SubstanceKey`._ `new` is a `const fn` returning `ParseSubstanceKeyError` (`Empty`,
      `TooLong`, `Malformed { at }`, `Padding`). The constructor that fails to compile is
      `new_const`: it panics, which is error E0080 in a `const` item (three `compile_fail`
      doctests) and a runtime panic elsewhere, so the registry's rows call it in one. Beyond the
      task: `MAX_BYTES` (16), `from_padded` and `as_padded` (the wire form), `form()` returning
      `SubstanceKeyForm` (`Formula`, `Electron`, `Name`), `FromStr`, `Display`, and a `Debug` that
      shows the string. Keys order as their strings do, NUL sorting first. No serde, since the
      crate depends on base alone: the sim writes `as_str`.
    - _The grammar, narrowed._ A formula's count is 2 or more with no leading zero, so `H1`,
      `C1O2` and `H02` are refused and a species has one spelling. No key in §1.1's rows is
      affected. A bare `e` is a name. Element symbols and stoichiometry are P14.T49.a's test.
    - _Where the refusals sit._ The palette's own rules are `MaterialPalette::new`'s
      (`BuildPaletteError`): more than 15 entries; an entry not strictly after its predecessor in
      (role, key); an albedo that is not finite or is below +0 (a −0 is refused, so a black band
      has one wire form); a density or transition that is not finite and positive.
      `crust_palette` and `main_liquid` are `FieldHeader::new`'s
      (`BuildFieldHeaderError::CrustPalette`, `MainLiquid`). So no header that breaks them
      reaches `CoarseField::new`, under which the task lists them. The cell rules are
      `check_substances`' (`BuildFieldError`'s `SubstanceIndex`, `SubstanceRole`,
      `IceWithoutEntry` and `LiquidWithoutEntry`), shared by `CoarseField::new` and
      `PartialField::insert`, and not `decode_block`'s, since only block 0 carries the header (as
      the months rule).
    - _Stricter or looser than the task._ A nibble naming an entry of the wrong role is refused
      even where the share is 0. A cell may name an ice or a liquid it holds none of (a seasonal
      frost's), but never hold a share it does not name. `main_liquid` is a `Liquid` entry. The
      crust roles are `PrimaryCrust`, `SecondaryCrust`, `TertiaryCrust` and `Province`
      (`PaletteRole::is_crust`), any `Crust` variant naming any of them. A cell whose crust names
      no entry is not refused: its lithology is not modelled.
    - _Public items beyond the task._ `field::NO_ENTRY`, `BuildPaletteError` and
      `PaletteRole::is_crust`; `MaterialPalette`'s `MAX_ENTRIES`, `entries`, `get`, `len`,
      `is_empty`, `find` and `has_role`; `SynthesisCell`'s `NO_SUBSTANCES`, `pack_substances`,
      `ice_entry` and `liquid_entry`; `SurfaceClass`'s `FIRST_KOPPEN_GEIGER`,
      `FIRST_SURFACE_STATE` and `range`, with `SurfaceClassRange` (`Unclassified`,
      `KoppenGeiger`, `SurfaceState`); `FieldHeader`'s `palette`, `crust_palette`, `crust_entry`
      and `main_liquid`; `DecodeBlockError`'s `Palette` and `SubstanceKey`.
    - _The codec._ `substances` follows `ice` in `SynthesisCell` and its table (T3's note called
      it `substance`; the task's name is `substances`). `palette`, `crust_palette` and
      `main_liquid` close `FieldHeaderParts` and its table. The palette is a count byte, then
      59-byte entries: the key's 16 padded bytes, the role, three `f64` albedos, the phase row,
      the density, the transition and the mechanics family. `[Option<u8>; 4]` is four
      presence-byte options: the array form now takes any `Wire` element. An empty palette adds
      6 B to block 0's header, each entry 59 B and each present index 1 B. A new `Crust` variant
      now lengthens the header, since `crust_palette` holds one entry a variant, so it takes a new
      format, not only a new code: a `const` assertion holds the four, and `wire.rs` says so.
    - _Sizes._ Whole payloads, inside T3's 10–15 MB bracket and R03's 15 MiB check: Earth-like
      13,567,743 B in 13 blocks (392,915 B more, Design note 17's 0.39 MB), Mars-like 3,414,234 B
      in 4, Moon-like 862,458 B in 1, Ceres-like 218,670 B in 1. Each single-block payload is its
      old size, the header's growth (Earth-like 245 B, Mars-like 244, Moon-like 126, Ceres-like
      184, Flat and OneCrater 6) and a byte a cell carried.
    - _Goldens._ `payloads.golden`'s 18 block, byte and digest lines are re-blessed at 21, and its
      held covers do not move; `codes.golden` gains 18 lines (`palette_role`, `mechanics_family`,
      `substance_nibble 15` and the three `surface_class` ranges). Neither `GENERATOR_VERSION` nor
      `SURFACE_PAYLOAD_FORMAT` is bumped (Generator version, the ruling of 2026-10-09).
    - _The synthetic worlds._ Earth-like: basalt (secondary) for `Oceanic`, granite (tertiary)
      for `Continental`, `H2O` as `Ice` and as `Liquid`, the main liquid `H2O`. Mars-like:
      basalt (secondary) for `Lid` and `Province`, `CO2` and `H2O` ices (its ice cells name `H2O`
      in the north and `CO2` in the south), `mars_dust` as `Deposit`. Moon-like: anorthosite
      (primary) for `Lid`, basalt (province) for `Province`. Ceres-like: phyllosilicate
      (secondary) for `Lid`, `H2O` ice, which no cell holds, `Na2CO3` as `Deposit`. Flat and
      OneCrater have an empty palette. The values are illustrative, each with its source and the
      science review's corrections; the solidi of basalt, granite and anorthosite and the
      densities of basalt and granite stay low confidence, unread. The albedos are grey across B,
      V and R: R10's Design note 8 midpoints, the anorthosite at its range's top, the
      phyllosilicate at Ceres's 0.094, and liquid water's the Fresnel reflectance in each band.
      Every `phase_row` is 0. `FieldBuilder` gains `palette`, `crust_palette`, `main_liquid` and
      `ice_entry` (the default ice is the palette's first `Ice` entry), and panics on ice or a sea
      it has no entry to name; the panics are documented, not tested, since `tests/panics.rs`
      cannot see the `testing` feature.
    - _Supersedes._ T2's "Closed sets": the cell's ice share now names its species through its
      entry; `month_precipitation` still names none, the condensable being P14.T48.d's.
    - _Acceptance._ The two commands miss the `wire::` tests B changed, so B ran the crate's whole
      suite, `cargo test -p hyperion-surface`.
    - _For P14.T49.a, T15, R10 and R11._ P14.T49.a takes `SubstanceKey` as it is, and its test
      checks the synthetic worlds' keys once P14.T49.b's rows exist. T15 assigns the classes in
      `SurfaceClass`'s two ranges. R10 assigns `phase_row`'s codes, and its classifier reads a
      nibble as the species and the share as its presence. R10's and R11's new palette members
      move the layout before T19's first send; whether that bumps `SURFACE_PAYLOAD_FORMAT` (the
      task's text) or re-blesses at the current version (Generator version's ruling, which B
      followed) is asked of "main". Answered 2026-10-10: Generator version's rule governs, and
      the task's text now says so.
  - _The runoffs, grain densities and the rulings' synthetic rows_ (follow-up C,
    science-r09-drainage.md §1–§6, 2026-10-10, with its late materials section; on T5's
    follow-up, `decision-r09-t5.md`'s sea-floor byte).
    - _The record._ `LogRunoff` (`NONE`, `new`, `get`, `from_metres_per_year`,
      `metres_per_year`; `LogPrecipitation`'s scale in m a⁻¹), and `SynthesisCell`'s `runoff` and
      `catchment_runoff`, which follow `steepness` in the struct and its table: 25 bytes a cell,
      the sea floor's byte staying last.
      `LogArea`'s and `LogSteepness`'s documentation say they are geometric, with the ruling's
      text (§1.4), and `PaletteRole`'s two crusts take §5.3's. `PaletteEntry::density_kg_m3` is
      `grain_density_kg_m3`; `BuildPaletteError::Density` keeps its name. `codes.golden` has no
      density row to rename (§3.3 supposed one).
    - _The synthetic routing._ `FieldBuilder::runoff` (a dry cell's own runoff by its centre, m
      a⁻¹; none by default) feeds `Grid::route`, which accumulates the equivalent area beside the
      geometric one in the same order; the catchment runoff is their ratio. A cell under the sea
      sheds none of its own (a choice: runoff is the land's), and a world not routed has none. The
      Earth-like world takes `EARTH_RUNOFF_SHARE`, 0.4, of each cell's annual precipitation, its
      months weighted by their shares of the year, as liquid water at its palette density (the
      ruling's "about 0.36, from memory" is labelled about 0.35, 40 of 113 × 10³ km³ a⁻¹,
      Trenberth et al. 2007, as the science review found it in secondary sources); the Mars-like
      world a uniform 0.1 m a⁻¹ (science-hoke §3), code 121 everywhere, its whole surface taken
      as older than the epoch's end (a simplification the ruling makes).
    - _The ruling's text, adjusted by the science review._ `LogArea`'s "never below its own
      cell's (about 4.5 × 10⁸ m²)" names the mean level-5 cell on a Ceres and the smallest, about
      2.9 × 10⁸; `LogSteepness`'s `k_snQ` says that Adams et al.'s and Leonard et al.'s Q is
      rainfall times area where the field's is runoff times area; the dust's 58 per cent is Viking
      Lander 1's fine material.
    - _B's rows._ Basalt's solidus is 1,253.15 K (§2). Grain densities (§3, with its late
      materials section): basalt 3,010 kg m⁻³ (Peck 1978), anorthosite 2,900, the dust 2,600;
      granite stays 2,650 and the phyllosilicate 2,500, relabelled. The Moon-like palette gains
      `H2O` as `Ice`, which no cell names (§4). Ceres-like's phyllosilicate is `PrimaryCrust`, the
      entry of `Lid` and `Province`, with no `Oceanic` (§5).
    - _The sim._ `CellState` gains `runoff_m_per_year` and `catchment_runoff_m_per_year` (m a⁻¹,
      not SI, as their codes and Hergarten's normalisation are; `CellState`'s documentation says
      so), none in `Working::initial`; the quantiser writes them, naming "runoff" and "catchment
      runoff". `reference.rs` renames the field only: its values, its documentation's convention
      and its Ceres's crust are R09.T10.c's, and so is `coarse_pass`'s Ceres example, whose
      phyllosilicate stays `secondary` until `CrustInputs::secondary` is an `Option`.
    - _Sizes_ (on T5's 23-byte record). Earth-like 14,748,053 B in 15 blocks (786,432 B of
      records and 150 B of framing more), 0.98 MB inside R03's 15 MiB and 0.25 MB inside T3's
      15 MB bracket, whose test now counts 15 blocks; Mars-like 3,706,930 B in 4; Moon-like
      936,261 B (887,050 + 2 × 24,576 + a 59 B palette entry); Ceres-like 237,119 B (224,830 +
      2 × 6,144 + 1 B for `Province`'s present entry); Flat 921,820 and OneCrater 922,186 B
      (+ 2 × 24,576); the Mars-like region 93,478 B (+ 2 × 2,301) and its delta 23,857 B
      (+ 2 × 562).
    - _Goldens._ `payloads.golden`'s 18 block, byte and digest lines are re-blessed at 21, its
      held covers unmoved; `codes.golden` gains the 256 `runoff_m_a` rows. Neither version is
      bumped (Generator version).
    - _For R09.T10.c, T6.b and T14._ T10.c: the reference rows' grain densities and their
      documentation, the reference Moon's `H2O` ice, the reference Ceres's primary crust with
      `CrustInputs::secondary: Option`, and the example's role. T6.b (T6.a landed first): the
      tributary factor (r̄ ÷ r)^θ and the zero-runoff rule from the two bytes, and widths from
      discharge; the synthetic Earth's cells give r̄ ≥ 3 r. T14.b–d: write `CellState`'s two
      runoffs, the geometric drainage and k_s on it, the equivalent area staying the solver's.
  - _An undifferentiated body's role and the crust ruling's synthetic rows_ (follow-up D,
    science-composition-crusts.md §1–§4, §7 and §8, 2026-10-11, with its corrected chondritic
    solidus; after follow-up C and R09.T10.c).
    - _The role._ `PaletteRole::Bulk` 8 and `is_crust` with the ruling's §1.6 text, `Bulk`'s
      split after its first sentence into a summary line and a second paragraph, "It is not a
      crust …" (the Rust rules' summary line; the words are §1.6's); and `grain_density_kg_m3`'s
      "at the temperature where it lies on the body" (§3.3). `PrimaryCrust`'s documentation names
      the lunar highlands' feldspathic crust, not their anorthosite (§7). _Supersedes_ B's crust
      roles: a `Crust` variant may name a `Bulk` entry. `header.rs`'s documentation and
      `CrustPalette`'s message say "a role a `Crust` variant may take"
      (science-composition-crusts.md §10.1).
    - _B's rows_ (`testing/worlds.rs`). The ruling's code text is `reference.rs`'s:
      `BASALT_FE`, `BASALT_FE_SOLIDUS_K` and `FELDSPATHIC` take §2.3's and §7.3's, since T10.c
      left those rows to T10.d, and the other rows take T10.c's as-built text (the plagioclase
      rock's "alteration point … clinoenstatite", Andersen's terms; the ice fit's Table 5 "0.02 per
      cent above 268 K"; `BASALT_SOLIDUS_K`'s "A terrestrial tholeiite's"). The figures:
      `basalt_fe` (3,310 kg m⁻³, 1,323.15 K) for the Mars-like crust and the Moon-like maria;
      `feldspathic` (2,900 kg m⁻³, `PLAGIOCLASE_ROCK_SOLIDUS_K` 1,533.15 K) for the Moon-like
      highlands, replacing `anorthosite`, which no synthetic world names now; the
      phyllosilicate's residue at `CHONDRITIC_SOLIDUS_K` (1,423.15 K) and the dust at
      `basalt_fe`'s; `Na2CO3` at 1,123.15 K, its 2,540 sourced; granite's 2,650 sourced, its
      solidus still unverified. The Earth-like world keeps `basalt` (3,010, 980 °C).
    - _The citation check and its rulings_ (science-composition-crusts.md §10 and §10.1, a
      second commit; no figure or golden moved). `CHONDRITIC_SOLIDUS_K`, `BASALT_FE` and
      `FELDSPATHIC` take §10.1's code text:
      - the charges' volatile loss is LPSC 25's (1994, p. 651), not LPSC 24's, and the lower
        natural solidus is labelled an inference;
      - `basalt_fe`'s `FeO` is sourced (the Lunar Sourcebook's Table A6.1, Meyer 2012's
        shergottites, Peck 1978's Alae tholeiite), and its Kiefer et al. citation adds Table 1
        (NWA 4898 and 70215);
      - `feldspathic` is about 75–80 per cent plagioclase by mass (about 80 by volume), not
        80–90, with its 4–6 wt% bulk `FeO` sourced (Peplowski et al. 2016; McKay et al. 1991;
        Lawrence et al. 2002); Hu, Ehlmann and Seager 2012 give the name only, their type being
        97 per cent plagioclase; and its 2,900 kg m⁻³ lies inside Kiefer et al.'s four
        feldspathic rocks' 2,840–2,910 by helium pycnometry (Table 1) and what 75–78 per cent
        plagioclase with the highlands' mafics gives, 2,880–2,910, the derived 2,750–2,890
        dropped;
      - `header.rs` and `field.rs` say "a role a `Crust` variant may take" for the five roles,
        and `CrustPalette`'s message "not a crust's, a province's or a bulk material's";
      - `McKay` joins the surface crate's `doc-valid-idents` (`clippy.toml`), as the sim's has it.
    - _The ices_ (§3.2 item 3). `ice_ih_density_kg_m3` (the ruling's fit), `COLD_TRAP_H2O_K` and
      `Substance::ice_entry_at` as R09.T10.c built them. Each synthetic climate's zonal mean is
      its equator's value less a constant times sin² φ, so the coldest zone is the pole's annual
      mean at sea level, as the climate is given, read from the world's own closed form at
      `POLE_LAT_RAD` (Mars's now `mars_temperature_k`, the same expression): the Earth-like
      world's 253 K, 919.59 kg m⁻³ (IAPWS R10-06 919.586 at 1 atm), and the Mars-like world's
      180 K, 928.15 (928.151 at 1 atm, 928.141 at its 610 Pa); the Moon-like and Ceres-like
      worlds' cold traps at 110 K, 932.72 (932.728). The zone is the lower of it and the frost
      point: saturated polar air's frost point is its temperature, and 180 K is below the frost
      point wherever water vapour exceeds 5.4 mPa (Murphy and Koop 2005, eq. 7). A land cap 1 km
      up on the Earth-like world would be 0.1 per cent denser. CO₂ ice keeps 1,621 kg m⁻³ at
      150 K.
    - _Files beyond the task's._ `testing.rs`'s palette tests (the keys, the drainage ruling's
      rows on the Earth-like basalt, and the new
      `the_synthetic_fields_palettes_take_the_crust_rulings_rows`), a test module in `worlds.rs`
      for the fit against IAPWS and the worlds' temperatures, `header.rs`'s
      `a_field_header_takes_a_bulk_entry_for_the_lid_and_the_provinces` (the review's:
      `FieldHeader::new` takes a `Bulk` entry for `Lid` and `Province`), and `wire/form.rs`'s
      unknown-code test, whose first unknown `PaletteRole` code is now 9 (read through one
      helper, so that the test stays within Clippy's 100 lines beside R09.T7.e's additions). The
      palette test keeps the task's name,
      `field_palette_crust_roles_are_the_crusts_and_the_provinces`, though it expects `Bulk`
      too. The two acceptance commands select every test this changed.
    - _Goldens._ `payloads.golden`'s 7 digest lines are re-blessed at 21: the four bodies' whole
      payloads, the Mars-like region and its delta, and the Moon-like world in 48 KiB, each block
      0 carrying its palette (the Earth-like world's moves by its ice's density alone). Every
      block count, byte count, length and held cover is unmoved, an entry's size being fixed, and
      the flat field's and the single crater's, whose palettes are empty, are unchanged.
      `codes.golden` gains `palette_role 8 = Bulk`. Neither version is bumped (Generator
      version).
    - _For R09.T10.d._ The reference rows' `feldspathic` and `basalt_fe` (the synthetic ones are
      the same figures, their docs §10.1's word for word), `CHONDRITIC_SOLIDUS_K`'s LPSC 25
      citation, the `FELDSPATHIC` assertion against Kiefer et al.'s 2,840–2,910,
      `CrustInputs::bulk`, and `lid`, whose `LidCrust::Bulk` takes the `Bulk` entry built here.
- **Deviations in T2.e, as built** (2026-10-15; follow-up E, decision-r11-palette-liquids §1, §2
  and §4; lane r09-palette-liquids, on R10.T10.a's `albedo_scale` and R08.T5.g's liquid files).
  - _The ID._ R09.T2.e, after follow-up D's R09.T2.d; the ruling calls it "R09 follow-up E". _Ruled
    2026-10-15: kept._
  - _The water record's temperature is 25 °C, not 15 °C._ The ruling's example is IAPWS-95's
    999.10 kg m⁻³ at 288.15 K; the records take 298.15 K, `water.json`'s and its index's
    temperature, at which IAPWS-95's density (997.047 kg m⁻³, at 0.1 MPa: the science check
    computed 997.047 0 there and 997.047 6 at 1 atm, the same within the formulation), IAPWS
    R1-76's surface tension (0.071 97 N m⁻¹) and IAPWS R12-08's viscosity (8.900 × 10⁻⁴ Pa s, at
    0.1 MPa) are read, so that no member is carried to another temperature; Pope and Fry's
    absorption, at 22 °C, is taken as the file holds it (since R08.T5.h's file, Mason et al.'s at
    23 °C to 495 nm and Pope and Fry's and Kou et al.'s at 22 °C from 500 nm, below). The
    reference Earth's `WATER_RECORD` is the same record; §1.7's temperature rule is P14.T54.a's,
    for generated bodies.
  - _Water's optics are R08.T5.g's file_ (until R08.T5.h's file, below). n is Hale and Querry's,
    unchanged; a, merged from R08.T5.g (`48ff891a`), is Pope and Fry 1997's over 380–727.5 nm and
    Hale and Querry's from 750 nm. Each bin is the uniform mean of the file's 5 nm grid
    interpolated linearly in n and in a = 4πk ÷ λ, to six figures (recomputed after the merge; the
    method reproduces the earlier file's bins exactly): 0.007 29 m⁻¹ in the bin about 443 nm (Hale
    and Querry's k gave 0.031 1) and 0.051 8 in the bin about 545 nm (0.042 3 before).
    `seawater.json` (variant `S35`) is not used: the Earth's sea is `H2O`, pure water, as the
    ruling's §5 leaves it until plan 14 models salinity.
  - _R08.T5.h will move the file_ (decision-r08-t5g.md §1, §2 and §7; main's note, 2026-10-15):
    water's n to IAPWS R9-97, by up to 4.5 × 10⁻³ at 380 nm (superseding "Hale and Querry" in
    decision-r11-palette-liquids §1.8's water row), and its k to Mason, Cone and Fry 2016 over
    380–495 nm and Kou, Labrie and Chýlek 1993 over 730–780 nm, with no bump. The records are
    entered by hand, as §3 of the ruling has them, since the surface crate and the sim read no
    material file; so each water value carries a note that T5.h moves it, and where taking T5.h's
    re-blesses: `wire/payloads.golden`, with `liquid_water`, the reference `WATER_RECORD`, and the
    Earth-like `WATER` rows' four-place normal albedo. No test pins today's `water.json` (until
    the guard, below): the Fresnel test reads a linear index of its own, and water's index bounds
    take R9-97's 1.345 at 380 nm. No record holds ethane, whose T5.h level (1.3887 at 670 nm)
    replaces §1.8's "about 1.40".
  - _R08.T5.h's `water.json`, taken_ (2026-10-15; lane r09-water-refresh, `fix(surface)`). R08.T5.h
    (`9fbc5ea2`) and T2.e landed on the same day, T2.e's bins from R08.T5.g's file, and no test saw
    the copies go stale. The water record's fifteen bins are recomputed from the new file
    by the same method (the method reproduces T2.e's bins from R08.T5.g's file to the last
    figure): n is IAPWS R9-97's at 25 °C, and a Mason, Cone and Fry 2016's at 23 °C over
    380–495 nm, Pope and Fry's at 22 °C over 500–725 nm and Kou et al. 1993's at 22 °C over
    730–780 nm. In the bin about 393 nm (380–405.3 nm) n goes 1.339 609 → 1.343 789 and a
    0.008 164 55 → 0.001 875 00 m⁻¹; in the bin about 443 nm n 1.337 267 → 1.339 564 and a
    0.007 288 68 → 0.006 244 14 m⁻¹; in the bin about 747 nm n 1.330 000 → 1.329 086 and a
    2.380 73 → 2.800 88. a is unchanged over 506.7–709.3 nm (Pope and Fry's rows in both files).
    Every copy follows: `testing::liquid_water` and the sim's reference `WATER_RECORD`; and both
    Earths' `WATER` rows take the new record's Fresnel reflectance, 0.021 092, 0.020 500 and
    0.020 210 at 440, 550 and 640 nm, to four places [0.021 1, 0.020 5, 0.020 2] (was [0.020 8,
    0.020 4, 0.020 2]). The labels stay as T2.e set them, 298.15 K, 100 kPa and `Measured`: the
    file's header is at 298 K, its n at 298.15 K and 1 atm (R9-97 is a reference correlation fitted
    to measurements, inside its range). The methane and basaltic records' files did not move, and
    their bins reproduce as they stand.
    - _One bin is a rounding tie._ The bin about 545 nm has an exact mean of 1.334 395 5, which
      the floating-point mean puts just below the half; the record enters 1.334 396, the exact
      value rounded (the science check). It moves the reflectance at 550 nm by about 10⁻⁷.
    - _The records' precision, restated._ `testing/liquids.rs`'s module documentation said "six
      significant figures, the files' own"; the records hold n to six decimal places and a to six
      significant figures, as T2.e entered them, and the guard holds them to that.
    - _The wire-form fixtures keep their stand-in sea._ `hyperion-protocol`'s
      `palette_liquid_wire_forms` and `coarseFieldStore.test.ts`'s `SEA_HEADER` give an `H2O`
      liquid entry [0.020 8, 0.020 4, 0.020 2] beside a flat n of 1.333. They pin the DTO's form,
      not the record, so they stay.
    - _Re-blessed at 22 with no bump_ (the wire exception of 2026-10-09, and main's ruling of
      2026-10-13 for changes before the serve gate): `wire/payloads.golden`, 2 lines, the Earth-like
      world's whole digest (the sea's record and its normal albedo in block 0) and the liquids'
      whole digest (`liquid_field`'s sea is `liquid_water`); every length is unchanged, since the
      record's form is. No other golden moves: the surface crate's albedo, material, weights,
      ground, height, bake, field and worker goldens pass unchanged, since the classifier never
      names a liquid entry (`material/classify.rs`), so the sea's normal albedo reaches none of
      them (the determinism audit); and the reference worlds are pinned by no golden yet (T16's).
    - _The guard_ (new): `apps/hyperion/src/renderer/src/view/atmosphere/materials/paletteLiquids.test.ts`
      reads the five hand-entered records from their Rust sources (`?raw`, as
      `coarseFieldStore.test.ts` reads `wire.rs`): `liquid_water`, `liquid_methane_titan` and
      `basaltic_melt` in `testing/liquids.rs`, the reference `WATER_RECORD` and the sim's
      `methane_sea` in `inputs.rs`. It holds each by-bin member to its R08 file's bin means at the
      records' precision (n to six decimal places, a to six significant figures, within half a
      unit in the last place): water and the basaltic melt to their files' n and a, methane to
      its file's n carried by Lorentz–Lorenz from 452.4 kg m⁻³ at the file's 90 K (which it
      checks) to the record's density, with its absorption absent; it checks that no record
      carries scattering and that the list covers every record those three sources hold (by their
      `basis` members). Run alone:
      `pnpm --filter hyperion exec vitest run src/renderer/src/view/atmosphere/materials/paletteLiquids.test.ts`.
      It is a vitest test rather than a Rust one so that `just smart-ci` reaches it from either
      side: a change to a material file plans only the
      TypeScript side's checks, and the TypeScript import graph reaches the test from the JSON
      file and from each `?raw` Rust source, where an `include_str!` in a Rust test would be
      reached only from the Rust side. It fails on T2.e's stale water bins (checked). It also
      holds the methane copy to its original, which nothing did.
  - _The methane record is `Derived`, not §1.8's `Measured`._ `liquid_methane_titan` is at
    93.65 K, Huygens's, with Setzmann and Wagner 1991's density there at Huygens's 1,467 hPa,
    447.6 kg m⁻³ (447.614, the science check's evaluation through the NIST WebBook; the WIP's
    447.4, a line between the triple and boiling points, was 0.2 low). Its n is
    `methane-liquid.json`'s at 90 K, each bin's mean carried by Lorentz–Lorenz at constant molar
    refraction by 447.6 ÷ 452.4, where 452.4 kg m⁻³ is the saturated-liquid line extrapolated to
    90 K, 0.69 K below the triple point (90.694 K): an extrapolation beyond the source's range, so
    `Derived` by §1.1's rule. The bin about 545 nm is 1.290 300, 0.003 4 below the file's.
    Absorption is absent, since the file holds k = 0 at 380–400 nm with no bound and its visible k
    is indicative; tension and viscosity are absent. `basaltic_melt` (1,473.15 K, `StandIn`) on the lava
    world's crust entry in `material/classify/tests.rs`: `basaltic-glass.json`'s n and a, a stated
    2,700 kg m⁻³, the rest absent. The density's citation is restated as the science check could
    verify it: about 2,690–2,750 kg m⁻³ for tholeiites at 1,200 °C and 1 bar by Lange 1997's and
    Lange and Carmichael 1987's partial molar volumes as DensityX (Iacovino and Till 2019) combines
    them. The sim's `inputs.rs` tests hold their own `methane_sea`, since the sim's library tests
    cannot reach `hyperion_surface::testing` without its feature; the server's test
    `the_reference_earth_s_sea_is_the_synthetic_water_record` holds the reference Earth's copy of
    the water record to the synthetic one, and nothing holds the methane copy to its original
    (until the guard, under "R08.T5.h's `water.json`, taken").
  - _No record carries scattering._ Einstein–Smoluchowski–Cabannes is P14.T49.h's, and R08.T5.g
    added no scattering column. _Ruled 2026-10-15 (main's ruling for R09.T2.e): absent until
    P14.T49.h, which fills the testing worlds' and the reference Earth's records before the serve
    gate._
  - _The Earth-like world's runoff keeps 999.84 kg m⁻³._ Its sea's grain density becomes the
    record's 997.047 (the rule: a `Liquid` entry's grain density is its record's to the bit), and
    `RUNOFF_WATER_DENSITY_KG_M3` (999.84, the conversion the illustration was built with) keeps
    its runoff, channels and heights where they were.
  - _The airless pressure is the quantiser's._ `header_pressure` writes exactly 0 for
    `SurfaceState::Airless`, the one place the header is built from inputs, so P14.T54.a's
    `for_body` need not write it (test `an_airless_body_s_header_pressure_is_exactly_zero`), and
    makes a −0 pressure +0 under every other state (the determinism audit). _Tenuous air waits for
    P14.T53.c:_ `SurfaceState` has no tenuous state yet, so every body below `AIRLESS_PRESSURE`
    (100 Pa) is `Airless` and its header pressure 0 until T53.c; the match has no wildcard, so
    T53.c's state takes an arm that keeps its own pressure (§2, item 5).
  - _The bins' test runs on every target_, not `native_only` as §4 says:
    `the_liquid_bins_are_r08s_bake_bins` reads `bake_wavelengths_nm.json` through `include_str!`,
    which embeds it at compile time.
  - _The server's length refusal is the compiler's._ The by-bin members are fixed-length arrays
    on both sides, so `convert/surface.rs` asserts `SURFACE_LIQUID_BINS == LIQUID_BINS` at compile
    time, and serde refuses a DTO array of another length (`a_liquid_member_of_another_bin_count_is_rejected`).
  - _`PaletteEntry` is no longer `Copy`_ (the ruling's option; `LiquidProperties` is `Clone`):
    callers that copied entries clone them, and `FieldHeader` borrows its palette as before.
    `PaletteEntryDto` drops `Eq` and `Hash`, since it holds `f64`s now.
  - _Public items beyond the ruling:_ `LIQUID_BIN_CENTRES_NM`; `BVR_EFFECTIVE_WAVELENGTHS_NM`
    (440, 550 and 640 nm, the sim's B, V and Cousins R); `LiquidProperties::refractive_index_at`
    (linear between the bins' centres) and `fresnel_normal_albedo_bvr`, which a `Liquid` entry's
    normal albedo is checked against; `BuildFieldHeaderError::NotAFraction` (a −0 refused);
    `testing`'s `liquid_water`, `liquid_methane_titan`, `basaltic_melt`,
    `LIQUID_WATER_DENSITY_KG_M3` and `LIQUID_METHANE_TITAN_DENSITY_KG_M3`, and
    `FieldBuilder::{maturity, organic_cover}`; the
    protocol's `SURFACE_LIQUID_BINS`; the server's crate-visible `liquid` and `liquid_basis`; and
    the client's `headerFailure` and `LIQUID_BINS` (`view/terrain/coarseFieldStore.ts`, typed as
    the generated tuple's length).
  - _The client's checks_ go beyond §4's list: besides the main liquid, the two fractions and each
    record's members (`null` taken as absent for an optional member), they refuse a `Liquid`
    entry without a record, a record on an ice, a cover or a deposit, an unknown basis, a normal
    albedo below 0 and a phase row that is not a byte, as the codec does. A refused header is
    held as the body's failure, as a level out of range is.
  - _The reference and synthetic Earths' `WATER` keep their normal albedo_, [0.020 8, 0.020 4,
    0.020 2]: the record's Fresnel reflectance to four places, which their tests hold within
    5 × 10⁻⁵. With R08.T5.h's file it is [0.021 1, 0.020 5, 0.020 2] (above).
  - _Tests beyond §4's list._ `liquid_field` in `wire/tests.rs`, a field with every present form
    of the record (a melt on the lid, a sea with every member, scattering a test value) and both
    fractions present, round-trips to the bit (`a_wire_payload_carries_its_liquid_records_and_fractions`)
    and is a new `payloads.golden` line, `Liquids whole`, since the layout's present forms freeze
    at the first send (the determinism audit); the sim's
    `the_liquid_bins_are_the_sky_s_bake_bins_to_the_bit` holds `LIQUID_BIN_CENTRES_NM` to
    `sky::colour::BAKE_WAVELENGTHS_NM`, which the fixture pins to 10⁻⁹ only; and the server's
    `a_field_header_carries_its_liquid_records`.
- **Deferred findings (2026-10-13 rule)** (R09.T2.e). Recorded and deferred: each leaves the
  algorithm and every contract as they are.
  - _The ruling's "about three times"_ (§1.9, finding 3): Hale and Querry's k was 5.06 times Pope
    and Fry's at 440 nm (R08.T5.g's figure); in the bins, the one about 443 nm fell 4.3-fold, and
    the one about 545 nm rose 1.23-fold. With R08.T5.h's file the bin about 443 nm is
    0.006 24 m⁻¹, 4.98 times below Hale and Querry's 0.031 1.
  - _The ruling's Morel 1974 oracle_ for water's b (§1.8, about 0.0045 m⁻¹ at 450 nm and 0.0019 at
    550 nm) is seawater's, not pure water's: Morel's seawater, at S = 38.4, scatters about 30% more
    than pure water (R08.T5.g's note on Zhang, Hu and He 2009). For P14.T49.h's test.
  - _"Johnson R" at 640 nm._ The normal albedo's R is read at 640 nm, the sim's Cousins R; Johnson's
    own R lies near 700 nm. For water the Fresnel reflectance differs by about 5 × 10⁻⁵.
  - _Water's shortest bin._ Its absorption, 0.008 16 m⁻¹ over 380–405 nm, lies above the next
    bin's 0.004 72: Pope and Fry's ultraviolet rise, which R08.T5.g records as possibly
    contaminated below 420 nm (its lean: Mason et al. 2016 there). The record carries the file as
    it stands. _Closed by R08.T5.h's file_ (lane r09-water-refresh): Mason et al.'s absorption
    gives the bin 0.001 875 m⁻¹, below the next bin's 0.003 054, so it now rises with wavelength.
  - _One temperature per record._ Pope and Fry measured at 22 °C, the record is at 25 °C
    (R08.T5.g's deferred "one temperature per liquid"). Since R08.T5.h's file the absorption is
    Mason et al.'s at 23 °C to 495 nm and Pope and Fry's and Kou et al.'s at 22 °C from 500 nm;
    near 740 nm, where water's temperature coefficient is 0.0161 m⁻¹ °C⁻¹ (IOCCG 2018, Table 1.1),
    25 °C would raise a by about 1.7%, the file's reduction states.
  - _The file's grid._ Averaged from Pope and Fry's own 2.5 nm rows, water's bins about 443 and
    545 nm would be 0.007 300 (against R08.T5.g's file's 0.007 29) and 0.051 89 m⁻¹, about 0.2%
    above the record's, since `water.json` keeps every other row (the science check); well inside
    the source's uncertainty, and the ruling takes the file's grid. Since R08.T5.h the bin about
    443 nm is Mason et al.'s, interpolated from their rows every 2 nm, so the 0.2% stands for the
    bin about 545 nm alone, which is still Pope and Fry's.
  - _No scattering in the water records._ §1.8's water row names ES–Cabannes for b, which is
    P14.T49.h's; the hand-entered records leave it absent rather than enter an uncomputed value,
    so R11.T8's tests on the synthetic worlds have no deep-water body term until T49.h (the
    conformance review's question for main).
  - _The liquid member is named by a string._ `BuildPaletteError::LiquidMember`'s `member` is a
    `&'static str`, as the header's `NotFinite { part }` is; a small enum was the Rust review's
    suggestion, not taken.
- **Deviations in T3, as built** (2026-10-09; the layout is generated output from T9's and T16's
  goldens on, revisable until then).
  - _Files._ `wire.rs` (the API, the block layout in its module documentation, the encoder and the
    decoder), `wire/form.rs` (each record's wire form as one table, `wire_table!`, from which its
    writer, its reader and its stride are generated: a field added to `SynthesisCell` or a part to
    `FieldHeaderParts` is one line there, and the strides 21 and 34 are the tables' sums, which a
    test pins), `wire/tests.rs`, and `field/partial.rs` (`PartialField`). `field.rs`'s record and
    crater rules are factored into crate-visible `check_synthesis_cell`, `check_climate_cell`,
    `check_crater`, `crater_key_follows` and `crater_key_order`, shared by `CoarseField::new`, the
    decoder and `PartialField::insert`, with `CoarseField::reaching_indices` for the encoder. One
    rule is new: a crater's reach must carry `ResolutionCode::NONE` on every range
    (`BuildFieldError::CraterReach`), as its documentation already said, so the wire carries no
    reach codes. A second is the review's: the per-cell crater index admits at most
    `MAX_MEAN_REACHES_PER_CELL` (32) entries a cell on average, counted from the reaches' ranges
    before anything is allocated (`TooManyReaches`, in `CoarseField::new` and
    `PartialField::insert` alike), about five times a saturated surface's (0.80 ln(D_max ÷ D_b)
    from Trask's density), so that a block of craters each reaching every cell cannot make a client
    allocate gigabytes; T12.d's coarse craters keep under it. The crater, whose reach has no fixed
    length, is written and read by hand (`put_crater`, `read_crater`, `CRATER_FIXED_BYTES`), not by
    a table: a field added to `CoarseCrater` touches all three, and the encoder's check of every
    block's length against its plan catches a disagreement.
  - _Public items beyond Provides._ `decode_block`, `DecodeBlockError`, `BLOCK_MAGIC` and
    `BLOCK_HEADER_BYTES` in `wire`, since a worker decodes one block at a time (R10's
    `field_chunk`);
    `DecodedBlock`'s getters `index`, `count`, `body`, `level`, `header`, `cover`, `cells`,
    `climate_indices`, `climate` and `craters`; `DecodePayloadError`'s `Empty`, `Block`,
    `Sequence` (a payload's blocks arrive in order and agree on their count), `WrongBody`,
    `WrongLevel` and `MissingBlocks`; `DecodeBlockError`'s refusals beyond the task's list, among
    them `CoverNotCanonical` (covers and reaches are canonical, so a block has one encoding),
    `TrailingBytes` (`decode_block` takes exactly one block's bytes) and `CraterMissesBlock` (a
    block carries only craters that reach its cells); `field::InsertBlockError`
    (`WrongBody`, `WrongLevel`, `WrongHeader`, `Record`, `CellConflict`, `ClimateConflict`,
    `CraterConflict`, `TooManyCraters`, `TooManyReaches`); and `Cover::with_margin(level, steps)`,
    the margin's dilation in king moves across face edges by the cube's neighbour rule, agreed
    with T4, which owns `SYNTHESIS_MARGIN_CELLS` (T3 merged T4's `b914b98e` for it).
  - _The layout._ A block's fixed header is 33 bytes: the magic `HYSF`, the format (`u16`) and the
    block's length (`u32`, which the plan's list lacked) are bytes 0–9, the frame every format
    keeps, so that R10's render thread splits blocks by the length at offset 6 without decoding;
    then the generator version (`u32`), the body (raw system ID `u64`, index `u16`), the level, and
    the block index and count as `u32`, not `u16`, since a block holds at least one cell and a
    level-8 field has more cells than a `u16` counts. Block 0's header section is length-prefixed
    (`u16`) and holds every part of `FieldHeaderParts` in its declared order, the body included,
    which must be the block's (`DecodeBlockError::HeaderBody`). Climate records travel once per
    distinct parent of a block's cells, so a parent whose children straddle two blocks is in both;
    a crater reaching several blocks travels whole in each.
  - _Margin and delta._ `encode_payload` adds the margin itself: every cell within
    `SYNTHESIS_MARGIN_CELLS` king moves of `cover` at `ResolutionCode::NONE`. `since` is the earlier
    survey cover, not its held cover: the delta is every cell of `cover` and its margin whose code
    differs from what `since` and its margin held, absent included, so that a cell surveyed again
    more finely, or a margin cell since surveyed, travels again with its new code. A cell given in
    `cover` at `NONE` is held, not surveyed, and grows a margin like any other. `encode_payload`
    panics on a cover past the field's cells (through `with_margin`), and on a cell whose records
    and craters cannot fit an empty block, which would take thousands of basins over one cell.
  - _Merging._ `PartialField` keeps a slot for every cell of the level, about 13 MB at level 8
    whatever the coverage, so that a read is one index; insertion is all or nothing; a cell held
    twice keeps the finer code (`ResolutionCode::finer`, commutative, so the order of blocks does
    not matter, and five orders are tested); a record that differs from the one held is refused.
    The rules that need the header (a month outside the year, a crater narrower than `D_b`) are
    checked at insertion, the rest at decoding. A header or crater that arrives twice is compared
    by its wire form, bit for bit, since `==` takes a −0.0 for a 0.0 and would let the order of
    arrival choose the bits kept.
  - _Floats._ The crate's `clippy.toml` bans `f64::to_le_bytes`; the one `f64` writer, in
    `wire/form.rs`, carries an `#[expect(clippy::disallowed_methods)]`: it serialises the bits and
    never hashes them.
  - _Goldens_ (new files, header 21, so no bump). `tests/golden/wire/codes.golden` pins every code
    the payload carries, decoded: each one-byte enum's codes, the layout's own tags, every value of
    every one-byte scale (steepness, precipitation, wind speed and azimuth, resolution, obliquity,
    ice, degradation), a sample of `LogArea`'s, the header's steps for every exponent with its
    temperature readers, and the cells' height and distance readers, as T2's determinism review
    asked; a renumbered code fails it and an appended one only extends it.
    `tests/golden/wire/payloads.golden` pins each synthetic world's whole payload (block lengths and
    an FNV-1a 64 digest of the bytes with each block's generator version zeroed, so that a bump
    alone moves only the file's header), the Mars-like region, its delta and its held cover, and
    the Moon-like world split at 48 KiB. They sit in a subdirectory, so `golden_diff.py`'s test-planet rule now
    matches only the files directly under the surface crate's `tests/golden/` (`is_test_planet`),
    and the sim-determinism skill says so: T9's `height/` needs no narrowing of its own, only its
    `native_only` header check, which `wire/` has (`every_wire_golden_carries_the_generator_version`).
  - _Sizes._ The Earth-like world's whole payload is 11,601,760 bytes in 12 blocks (Design note 17's
    "some 12 MB"), the Mars-like 2.92 MB in 3 and the Moon-like 0.74 MB in 1; a decoded block
    holds its records as the field does.
  - _Closed sets, for the composition audit._ The layout's screening tag (0 none, 1 atmosphere, 2
    cutoff) and presence bytes join T2's one-byte enums under `SURFACE_PAYLOAD_FORMAT`'s rule: codes
    grow by appending, never renumbered, a new code a generator-version change but not a new
    format, and an unknown code refused (`DecodeBlockError::Code` or `Tag`). Appending keeps every
    code's meaning, not an older payload readable: a block of another generator version is refused
    (`GeneratorVersion`), so after a bump a client fetches the field again.
  - _Acceptance._ `cargo test -p hyperion-surface wire` selects every test under `wire::` but not
    `Cover::with_margin`'s (`field::cover::tests`) or the two `should_panic` tests in
    `tests/panics.rs`, so T3 ran the crate's whole suite, `cargo test -p hyperion-surface`.
  - _Review, declined._ The rust-reviewer's lean of an enum for `encode_payload`'s
    `since: Option<&Cover>` (no bare `Option` parameter): Provides pins the signature, which T17
    and T19 consume.
  - _For T2's follow-ups A and B_ (the rulings of 2026-10-09: monthly winds and the season's
    eccentricity, then the composition audit's palette; one agent makes both codecs' edits after A
    lands). T3 was built on T2 alone. In `wire/form.rs`: the `ClimateCell` table's
    `wind: [Wind; 4]` becomes `[Wind; 12]`; the `FieldHeaderParts` table gains a
    `season_eccentricity: f64` line at the field's declared place; B adds the palette's line to the
    `FieldHeaderParts` table (with a `Wire` form for the palette type, a count byte and its
    entries, as `CraterParams`' is written) and the substance byte's line to the `SynthesisCell`
    table, `substance: u8` (or its newtype through `wire_scaled!`). Then
    `wire_records_have_design_note_seventeens_strides` moves from 34 to 50 bytes a climate record
    (and from 21 to 22 a synthesis record with B); `wire/tests.rs`'s literal climate cells and
    `[Wind::CALM; 4]` follow the type; the size test's bracket still holds (an Earth's whole field
    is 13.2 MB with A, 13.6 MB with B); and `tests/golden/wire/codes.golden` and
    `payloads.golden` are re-blessed in that commit at the current version, with no bump
    (Generator version), `golden_diff.py` explaining the moved digests and the extended codes.
  - _For T17 and T19._ T19.b converts T18's `Coverage` (a code a cell, `as_bytes`) to a `Cover`,
    its runs of one nonzero code as `CoverRange::new(start, end, code)` through
    `Cover::from_ranges`, and calls `encode_payload(field, &cover, since)` with `since` the survey
    cover at the client's `have_revision` (T18's note: T19.b keeps a cover per revision), never
    with its margin; the bytes go to R03's `BulkPayload::new` as they are. T19.c's helpers decode with `decode_payload`, build a `PartialField::new` from
    block 0's header and `insert` every block, and compute the held set to check against with
    `cover.with_margin(level, SYNTHESIS_MARGIN_CELLS)`.
- **Deviations in T10, as built** (2026-10-09; the inputs' shapes are builder arguments, revisable
  until P14.T54.a puts them on the record).
  - _Files._ `planetary/surface/{mod,inputs,grid,quantise,reference}.rs` and
    `planetary/surface/steps/{mod,plates,relief,craters,climate,erosion,classes}.rs`. The pass's
    working state (`steps::Working`, with `CellState`, `ClimateState` and `WindSample`, every value
    in SI units) and what every step reads (`steps::Pass`: the seed, the inputs, the level and the
    two grids) sit in `steps/mod.rs`; each step is `steps::<name>::run(&Pass, &mut Working)`, a
    no-op that names the task that fills it, run in Design note 5's order by `coarse_pass`. They
    are public for T14's and T16's benches, as Provides' "each pub for tests" says.
    `Working::initial` is a smooth, dry sphere at the datum on `Crust::Lid`, with no plates, ice,
    craters or fine relief (V₁ = 0), under a still climate at the mean surface temperature with no
    lapse rate: realised `σ_h` and relief zero, sea level its lowest elevation (zero), as a flat
    field realises them. The steps that change the elevation keep the realised figures and the sea
    level true (T12.e, T14.d). `steps`' module documentation states two rules for T11–T15: assert a
    value finite before it is sorted or compared, and find an extreme that reaches the header with
    `hyperion_surface::num::{min, max}`. `planetary::surface` re-exports `SurfaceSeed`.
  - _Public items beyond Provides._ `CoarseInputsBuilder` (a setter a value, two of them pairs:
    `surface(state, material)` and `tectonics(regime, continental_fraction)`),
    `BuildCoarseInputsError`; the inputs' vocabulary, each named for the plan-14 task that will
    supply it: `SubstanceRef`, `AreaShare` (T24.b's `surface_liquids` and `surface_ices`),
    `GasShare` (T24.a), `Spin` (obliquity, sidereal period, the solar day or `None` for a 1:1 lock
    to the seasonal host, the equinox's true anomaly in [0, 2π)),
    `SeasonalOrbit` (eccentricity and period, decision-r09-t2's), `TectonicRegime` (`MobileLid`,
    `StagnantLid`, `HeatPipe`, `Episodic`, `IceShell`: decision-composition §1.10's five),
    `WetEpoch` (start and end as Gyr ago), `ThermalRegime`, `Forcing` (`Seasonal`, `IceBelt`,
    `SlowRotator`, `Locked`) and `ClimateRegime` (with its `ClimateModelKind`), `CrustInputs`,
    `Condensate` with `CondensatePhase` and `Persistence` (T51.a's `seasonal`), and
    `MAX_CONDENSATES` (8); the getters, with `radius` (the figure's volumetric radius),
    `ocean_fraction`, `ice_fraction` and `months` (decision-r09-t2 item 1.4's rule: one month with
    no seasonal orbit, or on a circular orbit locked 1:1 to its host or with no tilt; twelve
    otherwise); `CellGraph` (`new`, `for_field`, `for_climate`, its slices, `edge_neighbours`,
    `edge_neighbour`, `corner_neighbours`, `vertex_neighbours` and `arc`), `BuildCellGraphError`
    and `grid::parent_index` (a cell's climate cell, n ÷ 4); `quantise(inputs, working)`, taking
    the state by value, with `QuantiseFieldError`, which `coarse_pass` panics with whole; and
    `reference::SEED`.
  - _No body-fixed frame member._ The field's cells are in P14.T14's body-fixed frame: its pole is
    the cube's +z and its prime meridian +x, which P14.T14 sets facing the primary at pericentre on
    a locked body. So a world locked 1:1 to its seasonal host has its substellar axis on +x, and
    `Spin` needs no pole or meridian; another resonance's substellar point follows from the periods.
  - _The hosts' light._ `host_flux` is P14.T12's orbit average over all hosts; the flux over the
    seasonal orbit is that mean times (a ÷ r)² √(1 − e²), exact for one host. A second star's own
    cycle is lost, which the reading of the record adds.
  - _`for_body`._ `SurfaceInputsError` gains `NotResolved(RecordSection)`, for a record degraded
    below the surface section's level; the service reads full records, so R09.T17 and R09.T19.a
    answer it `internal`. The type keeps Provides' name, which T17 and T19 consume (the
    rust-reviewer's verb-object rename was declined for it). `for_body` reads the surface section's
    tag alone
    (`NotApplicable` is `NoSolidSurface`, `NotModelled` and `NotResolved` name `Surface`) and its
    `Ok` arm is `match *surface {}`, since `record::Surface` is uninhabited: the reading of the
    `bulk`, `figure`, `rotation` and `orbit` sections and the hosts is not written, because no
    record gets past the tag and what it reads depends on the section's members, which P14.T48.e
    and P14.T54.a define (decision-composition §3.4 puts `planetary/surface/inputs.rs` among
    T54.a's files). Two findings for P14.T54.a, R09.T17 and R09.T19.a: the record carries no time,
    while the hosts' light and the spin (the despin to a lock) change with it, so `for_body` and
    `InputsSource::inputs` need the record's time `t`; and a moon's seasonal orbit is its planet's,
    which its own record does not hold, so they need the planet's record or the system. The
    generated sub-Neptune's test is
    P14.T48.e's to add, since this task landed first (decision-p14-t35e-wire).
  - _Composition, partly built._ `crust` (P14.T51.c's members without `redox`, which the pass does
    not read) and `condensates` (P14.T51.a's) are builder arguments, as are the liquids, ices and
    gases by substance, each keyed by `SubstanceRef`: the key's text, standing for P14.T49.a's
    `SubstanceId` (the record's) and follow-up B's `SubstanceKey` (the palette's), neither built.
    It checks only 1–16 printable ASCII bytes, the grammar being `SubstanceKey`'s, and gives way to
    those types when they land. A condensate's `reservoir` and a wet epoch's `paleo_inventory` are
    masses per square metre of the whole surface (`KilogramsPerSquareMetre`), not layers, so that
    no density is assumed and condensates sort by mass as P14.T51.a sorts them; an area share is
    the cover all year, the cells R09.T13.d makes icy by their warmest month, seasonal cover being
    the climate's. `CrustInputs` leaves which entry a `Lid` and a `Province` cell take to T12 and
    follow-up B. _The palette's sources are not built:_ the values each palette entry carries (A_N
    in B, V and R, the phase row, density, transition and mechanics family, resolved by the server)
    are follow-up B's `PaletteEntry` and `MechanicsFamily`, which do not exist yet, and the deposits
    (`mars_dust`, `Na2CO3`) come with them. Whichever of follow-up B and this task lands second adds
    B's header parts (`palette`, `crust_palette`, `main_liquid`) and the cells' `substances` to
    `quantise` (an empty palette and none for the no-op pass); the palette's sources then follow as
    a T10 follow-up (proposed R09.T10.b, after B and before T15; lean: a `MaterialPalette` of
    resolved entries as a builder argument until P14.T49.b–c and T54.a), with the reference worlds'
    palettes, which T15 and T16 need.
  - _The months and winds_ (built on T2's follow-up A, `e37b16d6`, merged before commit). The
    quantiser writes each month's wind from the working state's twelve, a one-month year's later
    eleven calm, and the header's `season_eccentricity` from `inputs.seasonal_orbit()`'s
    eccentricity, zero without one, which `CoarseInputs::months` keeps consistent with the header's
    rule (one month only on a circular orbit or none). The builder refuses a −0 eccentricity, as
    the header does, and the quantiser writes the header's raw `f64` parts (sea level, lapse rate,
    realised figures) with −0 made +0, so that a zero has one form on the wire.
  - _Validation._ `CoarseInputsBuilder::build` refuses a missing value (`Missing` names it), a value
    outside its range, a figure not 0 < c ≤ a, a gas-envelope surface state (`NoSolidSurface`), a
    malformed key, a substance twice in a list (a condensate twice in one phase), shares or mole
    fractions summing above one (by more than 10⁻⁹), over eight condensates, a wet epoch ending
    before it starts, and a crater contract whose gravity is not the body's (10⁻¹² relative). It
    sorts liquids, ices and gases largest first and condensates by reservoir, ties by key (then
    phase), which P14.T24.a's ties by `SubstanceId` replace.
  - _Reference worlds._ Each function's documentation tables its figures and sources, checked by a
    science review (2026-10-09), which corrected Earth's ocean (0.7095, Eakins and Sharman 2010),
    heat flow (0.0916 W m⁻², Davies and Davies 2010) and mean surface pressure (98.55 kPa,
    Trenberth and Smith 2005), Mars's mean temperature (214 K, NASA's fact sheet) and equinox (Ls
    of perihelion 251.0°, Allison and McEwen 2000), and the Moon's obliquity (1.535°, the Cassini
    state) and GM (DE440). Figures with no rule yet are marked placeholders: the contrasts
    (P14.T24.a's), Earth's surface age and V, Mars's and Ceres's ages, Ceres's heat flow. `σ_h` is
    measured for Earth and Mars and from Design note 3's fit at V = 0 for the Moon (2.239 km) and
    Ceres (2.077 km). An airless world's mean temperature is the generator's own: its equilibrium
    temperature at the airless Bond albedo, by `derive::irradiation` and `SurfaceState::albedo`.
    Ceres takes rock for Dawn's dark surface, where `SurfaceMaterial::of` would call a body a
    quarter water ice. Crater densities are the lunar chronology at each surface age, before plan
    14's belt scaling; screening uses a 3,000 kg m⁻³ projectile, the density Design note 12's d\*
    figures imply. Earth's air is the U.S. Standard Atmosphere 1976's (CO₂ 314 × 10⁻⁶, about
    420 × 10⁻⁶ now), since published modern fractions, rounded, sum above one.
  - _Science findings, for "main"_ (the review of 2026-10-09):
    - plan 14 gives an airless body its equilibrium temperature as its mean surface temperature
      (P14.T13.c), a radiative mean: the Moon's 270 K stands against an equator that swings between
      about 95 and 395 K (Diviner, Williams et al. 2017), whose time mean is at most about 260 K, so
      the mean and a contrast imposed arithmetically on the P₂ form are not one field; R09.T13.c–d
      impose it as a radiative mean, ⟨T⁴⟩^¼, or plan 14 states an arithmetic one (lean: the former);
    - Design note 8's "replacing the annual field's P₂(sin φ) coefficient" with the contrast reads
      as T₂ = contrast, where the contrast T(0°) − T(90°) is −(3/2) T₂, so T₂ = −⅔ of it (`inputs`
      documents this);
    - Yang et al. 2014's Table 1 gives sidereal rotation periods at an orbital period of 225 d, not
      solar days, and only brackets the onset (8–16 d at 1.40 S⊕, 32–48 d at 1.92 S⊕), where Design
      note 3 and P14.T48.d say "a solar day of 16 d at 1.4 S⊕ to 48 d at 1.9 S⊕";
    - Design note 12's d\* of 8.2 cm for Mars takes about 610 Pa; NASA's 636 Pa gives 8.5 cm.
  - _Closed sets, for the composition audit._ `TectonicRegime` (five), `ThermalRegime` (three),
    `Forcing` (four), `CondensatePhase` (three) and `Persistence` (two) are physics classes; plan
    14's own types replace the first two when they exist. Heat-pipe, episodic and ice-shell worlds
    have no rule of their own in T11–T12 yet (Composition, below): T11 decides what each gets.
  - _Review, declined._ The rust-reviewer's lean of an enum for `Spin::solar_day`'s `None` (a 1:1
    lock), a struct field its documentation explains; and making `steps`, `Working` and `quantise`
    crate-private, which Provides and the benches need public.
  - _Tests and acceptance._ `cargo test -p hyperion-sim planetary::surface` runs 25 unit tests; the
    two doctests (`coarse_pass`'s Ceres-like builder and `quantise`'s) run with
    `cargo test -p hyperion-sim --doc planetary::surface`, since the filter alone builds the
    integration tests but does not run the doctests here. The tests: the no-op pass twice on every
    reference world, compared as fields and as their whole payloads' bytes, and order-independent
    over two seeds and two worlds; solid angles summing to 2π ÷ 3 a face to 10⁻¹² at levels 0 to 8;
    edge neighbours four, distinct and symmetric; three vertex neighbours at each of the 24
    cube-corner cells and four elsewhere; the quantiser's codes, each month's wind, the header's
    eccentricity and +0 sea level, and its named refusals (`Cell`, `Climate`, `Step`, `Header`,
    `Field`, `Counts`); the builder's refusals and messages; `for_body` on a generated rocky planet
    (`NotModelled(Surface)`), a generated gas giant (`NoSolidSurface`) and a degraded record
    (`NotResolved(Surface)`). The sim's whole suite passed on the merged tree (2,834 tests, before
    the review's fixes). No stream is opened, no tag or golden added, and no output moved.
- **Deviations in T10.b, as built** (R09.T10.b, the palette's sources, 2026-10-10; after T2's
  follow-up B, `b069fd74`; decision-composition §1.7 and §7.4 (c); builder arguments, revisable
  until P14.T54.a reads them from the record).
  - _Keys._ `SubstanceRef` is gone: every substance of the inputs is follow-up B's `SubstanceKey`,
    valid by construction, so `BuildCoarseInputsError::SubstanceKey` is gone too and `build` checks
    no key. The sorted lists still break ties by key, which orders as its string, so no order
    moved. P14.T49.a's `SubstanceId` (`hyperion_sim::substance`, landed as `844888c5` after this
    task was built) is not used. _Dependency for P14.T54.a_, which owns
    `planetary/surface/inputs.rs`: it turns the record's `SubstanceId`s into their rows' keys
    (`id.substance().key()`), or moves these inputs onto `SubstanceId` (ties by id, P14.T24.a's) with
    the palette's keys resolved from the rows.
  - _The palette._ `CoarseInputsBuilder::palette(MaterialPalette)` is required (`Missing("palette")`),
    its entries' values resolved as the server will resolve them from P14.T49.b–c's rows. The
    deposits are its `Deposit` entries (a Mars's `mars_dust`, a Ceres's `Na2CO3`), with no list of
    their own. `build` refuses, as `BuildCoarseInputsError::NotInPalette { part, substance, role }`,
    an ice or a solid condensate with no `Ice` entry, a liquid or a liquid condensate with no `Liquid`
    entry, and a lithology of the crust with no entry; a supercritical condensate, on the ground as
    neither, needs none. An entry nothing names is allowed (Ceres's water ice). Getters:
    `palette`, `crust_palette` and `main_liquid`.
  - _Signed zeros_ (the determinism review). An area share (a liquid's, an ice's, a condensate's)
    and a condensate's reservoir refuse a −0, as the seasonal eccentricity does: `total_cmp` sorts
    −0 before +0, so two zero shares would order by their zeros' signs rather than by key, and the
    first liquid is now the header's main liquid.
  - _The crusts' entries_ (`CoarseInputs::crust_palette`, the header's `crust_palette`, in `Crust`'s
    code order). `Continental` takes the tertiary crust's `TertiaryCrust` entry, or none; `Oceanic`
    the secondary's; `Lid` the primary's `PrimaryCrust` entry where the body has one, the
    secondary's otherwise; `Province` the provinces'. The secondary's entry is its `SecondaryCrust`
    one and the provinces' their `Province` one, but where the two are one substance either serves
    both, the variant's own role first: a Mars's provinces take its lid's basalt (no `Province`
    entry, as `PaletteRole::Province`'s "where it is not the lid's own" says), and a Moon's
    secondary crust its maria's `Province` basalt. This settles T10's "which entry a `Lid` and a
    `Province` cell take", which follow-up B's synthetic worlds set by hand. It differs from them
    only in naming an entry for variants their cells never carry (Mars's and the Moon's `Oceanic`,
    Earth's `Lid` and `Province`, Ceres's `Province`). T12 may revise it.
  - _The main liquid_ (`CoarseInputs::main_liquid`) is the `Liquid` entry of the largest liquid, the
    first of `liquids`, or none.
  - _The quantiser_ writes the header's `palette`, `crust_palette` and `main_liquid` from the inputs.
    Each cell's `substances` packs two new `CellState` members, `ice_entry` and `liquid_entry`
    (`Option<u8>`, palette indices, `None` in `Working::initial`), by
    `SynthesisCell::pack_substances`. An entry of 15 or more is `QuantiseFieldError::Cell`, naming
    "ice entry" or "liquid entry". An entry past the palette or of the wrong role, or a share whose
    entry the cell does not name, is `CoarseField::new`'s refusal, passed on as
    `QuantiseFieldError::Field`. R09.T13.d sets the ice entries per condensate, and the steps that
    fill basins set the liquid entries.
  - _The reference worlds' palettes_ are §1.7's:
    - Earth: basalt (secondary), granite (tertiary), `H2O` as ice and as liquid, the main liquid
      `H2O`;
    - Mars: basalt (secondary), `CO2` and `H2O` ices, `mars_dust` (deposit);
    - Moon: anorthosite (primary), basalt (province), and `H2O` (ice) from R09.T10.c;
    - Ceres: phyllosilicate (primary, by science-r09-drainage.md §5, from R09.T10.c; secondary as
      built), `H2O` ice, `Na2CO3` (deposit). No share names the water
      ice, since plan 14 states no area for its cold traps yet.

    Each substance's figures are a row in `reference.rs` with its sources. They began as follow-up
    B's synthetic palettes' figures and are kept apart from them, not shared, so that a correction
    to one set moves only its own output: the wire goldens pin the synthetic fields', and T16's
    goldens will pin these. The albedos are grey across B, V and R (P14.T49.c resolves each band),
    and every `phase_row` is 0 until R10 assigns its rows. The sim's `clippy.toml` gains `WebBook`.
    The science review (2026-10-10) found no wrong value.
    - It read the basalt solidus as 980 ± 10 °C, a natural tholeiite's (Wright and Okamura 1977,
      USGS PP 1004, Table 15), which these rows take: 70 K below the unread 1,050 °C that the
      synthetic rows kept until follow-up C took 980 °C too (science-r09-drainage.md §2).
    - It labelled what it could not read or what is assumed: the anorthosite's solidus is its
      plagioclase's alone, an upper bound for the rock (its pyroxene melts near 1,270 °C); liquid
      water's Fresnel factor stands in for `A_N`, which a specular surface lacks, until R11's
      ocean; the dust's and the phyllosilicate residue's solidi are basalt's by assumption; and
      granite's density and solidus are from memory (Huang and Wyllie 1975 named, not read).
    - It confirmed the rest against the sources cited, and the Fresnel factors and Mangan et al.'s
      fit at 150 K by recomputation.

    Findings for "main" from it: the synthetic rows' basalt solidus (ruled 980 °C, follow-up C,
    re-blessing the wire goldens at 21); the densities mix grain and porous bulk values (ruled:
    grain densities, porosity apart, follow-up C and R09.T10.c, science-r09-drainage.md §3); and two
    questions on §1.7's lists, a water-ice entry for the Moon's cold traps
    (Li et al. 2018, PNAS 115, 8907) and whether Ceres's altered-ocean crust is better primary
    than secondary (lean for both: keep §1.7's lists until plan 14 states them). Both are ruled,
    against the lean (science-r09-drainage.md §4 and §5): the Moon's palette carries `H2O` as
    `Ice` now (follow-up C and R09.T10.c), its share stated by P14.T51.f and placed by R09.T13.f,
    and Ceres's crust is primary (R09.T10.c).

  - _Other review changes._ The reference worlds' gases are `const` items, so that a
    malformed key fails to compile, and the order-independence test runs over all four worlds.
  - _The months, to the signed-off rule._ `CoarseInputs::months` gives one month only on a
    circular seasonal orbit with no obliquity to it, or with no orbit, as Design note 8 reads since
    its sign-off of 2026-10-10 ("a lock adds nothing"; `signoff-2.md` §4). T10 built
    decision-r09-t2's earlier reading, which gave a world locked 1:1 to its host on a circular orbit
    one month whatever its tilt. No reference world is one, so nothing they give moved; the test
    now gives such a world twelve months, and one at zero obliquity.
  - _Tests and acceptance._ `cargo test -p hyperion-sim planetary::surface` runs 31 unit tests, and
    `cargo test -p hyperion-sim --doc planetary::surface` its two doctests, `coarse_pass`'s Ceres-like
    builder now with a palette. New and changed tests:
    - the builder's `Missing("palette")` and `NotInPalette` refusals, by part and role, a melt share
      outside 0 to 1, and a −0 share or reservoir;
    - the crusts' entries on Earth, the Moon and Mars, and the main liquid of two seas, unequal and
      tied in both orders;
    - the reference palettes;
    - the quantiser's header parts, cell bytes and refusals;
    - the no-op pass's header carrying the inputs' palette, and the pass order-independent over the
      four reference worlds.

    No stream, tag or golden is added, and no output moved.
- **Deviations in T10.c, as built** (R09.T10.c, the crust's options and the reference rows,
  2026-10-11; after T2's follow-up C, `3dc599d5`; science-r09-drainage.md §3–§5, with
  science-composition-crusts.md §3, §4 and §7's corrections to the rows; builder arguments,
  revisable until P14.T54.a reads them from the record).
  - _Basalt's density._ The task text has basalt's 2,900 kg m⁻³ "relabelled". The row takes
    3,010 kg m⁻³, the mean of Alae lava lake's eight tholeiites (Peck 1978, USGS PP 935-B,
    p. B20), as the ruling's §3.2 table and §3.3 code text give it and follow-up C's synthetic row
    has it (accepted by "main", 2026-10-11). `mars_dust` 2,600, granite 2,650 and the
    phyllosilicate 2,500 are the task's, all grain densities. The anorthosite is 2,790, not the
    task's 2,900 (below).
  - _The second ruling's corrections._ science-composition-crusts.md (adopted 2026-10-11, ruling
    this task's gaps) arrived before the commit. "main" had the corrections that sit in
    `reference.rs`'s own rows applied here, and left the new rows and members to R09.T10.d,
    though the ruling's §8 lists them under T10.c. Applied:
    - _the solidi_ (§4): the anorthosite's 1,260 °C, a plagioclase rock's first melt
      (`PLAGIOCLASE_ROCK_SOLIDUS_K`, Andersen 1915), for its plagioclase's 1,500 °C; the
      phyllosilicate residue's 1,150 °C, 1,423.15 K, an upper bound (`CHONDRITIC_SOLIDUS_K`,
      Jurewicz et al. 1993; the ruling's corrected figure, which replaced its first 1,400 K), for
      basalt's by assumption; the dust's 1,050 °C (`BASALT_FE_SOLIDUS_K`, the Lunar Sourcebook's, still an
      assumption), for basalt's; and `Na2CO3`'s 850 °C (NIST-JANAF), for 851;
    - _the densities_: the anorthosite rock's 2,790 kg m⁻³ (§7), for the highland crust's 2,900,
      which is `feldspathic`'s; granite's 2,650 and `Na2CO3`'s 2,540 confirmed and sourced (§4);
      and water ice's ρ_s(T) (§3), IAPWS R10-06 as a fitted polynomial (`ice_ih_density_kg_m3`),
      each world's `Ice` entry at its coldest zone, T₀ − (2/3) × the equator–pole contrast
      (`coldest_zone`: Earth 260 K, 918.6 kg m⁻³; Mars 177.3 K, 928.4), or at the cold-trap
      threshold (`COLD_TRAP_H2O_K`, 110 K: the Moon and Ceres, 932.7), through
      `Substance::ice_entry_at`. The row's own 916.72 is its melting point's.
    - _The ruling's code text, adjusted by the science review of the delta._ That review found the
      ruling's first chondritic solidus misattributed Jurewicz et al.'s "~1150-1160°C" (the H and
      LL chondrites', not Murchison's and Allende's) and took Henke et al.'s 1,400 K, from Agee et
      al.'s runs at 1–27 GPa, as a 1-atm figure; "main" had the ruling corrected (1,423.15 K, an
      upper bound), and the constant takes its corrected text. The plagioclase rock's note uses
      Andersen's "alteration point" and "clinoenstatite"; the anorthosite's
      names albite's density, from which An₉₅'s 2.753 is interpolated; the ice fit's note gives
      Table 5's 0.02 per cent above 268 K; `coldest_zone` states that it assumes a cold pole; and
      the Mars table notes that its contrasts' placeholder pole, 160 K, is colder than the P₂
      fit's.
    - _Left to R09.T10.d_ (with follow-up D for the synthetic rows): the `feldspathic` row for the
      lunar highlands (§7) and `basalt_fe` for the maria and Mars (§2), so until then the
      reference Moon's lid is the 2,790 kg m⁻³ rock and its maria and Mars the 3,010 tholeiite;
      `CrustInputs::lid: LidCrust` with `LidAbsent` (§5); and `bulk` with `BulkBesideCrust` (§1).
  - _The crust mapping's two open cases._ Where both crusts are the provinces' substance and the
    provinces have no `Province` entry, they take the secondary's entry before the primary's. The
    primary crust takes its `PrimaryCrust` entry alone: the task gives the shared-entry fallback
    to the provinces only, and a palette naming the lid's substance as a `Province` entry and
    nothing else would break `PaletteRole::Province`'s "where it is not the lid's own". A
    `Province` entry of the lid's substance beside its `PrimaryCrust` one is accepted, and names
    the provinces (a test's case). The private `entry` helper's one fallback role became a list.
    `build` refuses `NoCrust` straight after the crust's `Missing`, before the melt share and the
    palette, in a new private `check_crust` that keeps `build` within Clippy's line limit.
  - _Documentation beyond the task's._ The reference module says that the rows are the
    substances', serving every body whose composition names them, and labels where a body's
    measurement (the albedos) stands for a substance's. The basalt solidus's note replaces the
    unread "about 1,050 °C (Yoder and Tilley 1962)" with the ruling's read quench runs (Yoder and
    Tilley 1957, Year Book 56, pp. 157–158: two Kilauea tholeiites at about 1,065–1,090 °C, a
    Mauna Kea alkali basalt at 1,040 °C) and Alae's corroboration (§2.1), and calls it a
    terrestrial tholeiite's. The basalt row drops the unread Philpotts and Ague 2009 range, and
    names `basalt_fe` (3,310) as the iron-rich basalt's row. The dust row names Viking Lander 1's
    fine material, as follow-up C's science review found it, where the task has "the Viking
    landers'". The phyllosilicate row gives Ceres's crust's bulk as Ermakov et al.'s 1,287
    (+70/−87) kg m⁻³ in place of "1,200–1,400". `melt_area_fraction`'s documentation adds, after
    the task's sentence, that science-composition-crusts.md §5 amends the crust to the lid's,
    which T10.d's `lid` will name. `CrustInputs` states the classes by mode of origin, and each
    table its crust and palette sources, the ice's temperature among them (§3, §4, §5).
  - _Tests._ `NoCrust` on Earth's crust without its secondary and Ceres's without its primary; the
    reference Ceres's `crust_palette`, `[None, None, shell, shell]` with its `PrimaryCrust` entry;
    the provinces' entries (their own `Province` first, the secondary's before the primary's where
    both crusts are their substance, a deposit never), and the primary refused without its own
    `PrimaryCrust` entry; the reference palettes, the Moon's `H2O` ice and Ceres's
    `PrimaryCrust` among them, with no share naming either ice; the grain densities within their
    sources' ranges (the anorthosite 2,776–2,828, granite G-1's 2,635–2,671, Alae's 2,920–3,040)
    and the dust's giving back Viking Lander 1's fines' 1,100 ± 150 at 58%; ice Ih's density
    against IAPWS R10-06 at 273.15, 100 and 160 K (±0.03), its clamps, and each world's ice entry
    (Earth 918.6, Mars 928.4, the Moon and Ceres 932.7, ±0.1); `NoCrust`'s message; and the
    builder giving back all four reference worlds, not only Earth (its test helper generalised to
    `builder_of`). The existing no-op pass builds each reference world's valid field.
    `coarse_pass`'s Ceres example takes a primary phyllosilicate with no secondary and asserts its
    `Oceanic` has no entry. `cargo test -p hyperion-sim planetary::surface` runs 35 unit tests,
    four of them new, and `cargo test -p hyperion-sim --doc planetary::surface` its two doctests,
    the changed `coarse_pass` example among them (the filter alone does not run the doctests, as
    T10 found).
  - _Moves:_ no pinned output. No reference golden exists before T16, no stream or tag is added,
    and the synthetic worlds and the wire goldens are untouched. The reference worlds' unpinned
    fields do change, at version 21: Ceres's `crust_palette` goes from `[None, 0, 0, 0]` to
    `[None, None, 0, 0]`, its entry 0 from `SecondaryCrust` to `PrimaryCrust`; the Moon's palette
    gains a third entry, `H2O` as `Ice` at index 2, its others keeping theirs; the basalt,
    anorthosite and dust entries take their grain densities, every water-ice entry its ice's
    temperature's, and the anorthosite, phyllosilicate, dust and `Na2CO3` entries their corrected
    transitions. T16's first blessing pins them.
  - _For R09.T12.a._ Ceres's `Oceanic` now has no entry, as `Continental` has none on Mars, the
    Moon and Ceres. Nothing refuses a cell whose crust variant the header names no entry for
    (`check_synthesis_cell`, the header's `crust_palette` check), so the step that first writes
    a cell's crust should refuse one, in the field or the quantiser (the determinism review's
    nit, 2026-10-11).
  - _For R09.T10.d and T13.c._ With both crusts optional, "the crust at the surface" is
    `secondary.or(primary)`, which `NoCrust` guarantees but the type does not carry (the Rust
    review's nit). science-composition-crusts.md §5 replaces it with an explicit
    `lid: LidCrust`, which `Lid` and the melt test both read; T10.d adds it, and `build` should
    resolve the lid's substance once, so that no step unwraps the options.
  - _Lint._ The sim's `clippy.toml` gains `McLennan` in `doc-valid-idents`, as the surface
    crate's has it.
  - _Gaps and questions found while building, ruled by science-composition-crusts.md_ (adopted
    2026-10-11):
    - an undifferentiated body (a chondritic asteroid, a comet nucleus) has neither crust, which
      `NoCrust` refused: it gains `bulk`, its own uncrusted material, in a `Bulk` role (§1;
      follow-up D and T10.d);
    - the maria's and Mars's iron-rich basalt: `basalt_fe`, 3,310 kg m⁻³ and 1,050 °C (§2; T10.d);
    - ice densities: ρ_s(T) for each solid phase (§3; applied here for water ice);
    - the unread figures: verified or corrected (§4; applied here), granite's solidus still
      unverified, and the albedos R10.T1.a's;
    - which crust a stagnant lid shows: neither lean; an explicit `lid`, a `LidCrust` of
      `Primary`, `Secondary` or `Bulk`, which `Lid` and the melt test read (§5; T10.d);
    - a generated Moon's water ice before P14.T51.f: none, an interim labelled NOT YET MODELLED
      under T54.a, which the reference and synthetic Moons, being test inputs, do not share (§6);
    - the anorthosite's density: two rows, the rock's 2,790 (applied here) and the highland
      crust's `feldspathic` 2,900 (§7; T10.d).
  - _For P14.T54.a._ `CrustInputs`, with `primary`, `secondary` and `tertiary` each an
    `Option<SubstanceKey>`, `provinces` a `SubstanceKey` and `melt_area_fraction`, is T51.c's
    `CrustComposition` less its redox class, member for member. So `for_body` passes `secondary`
    through as `None` on a Ceres, and `NoCrust` is the guard that matches T51.c's at-least-one
    rule. The palette's sources must give an icy body's `H2O` primary crust a `PrimaryCrust` entry
    apart from its `Ice` entry, and each ice entry ρ_s at the temperature where the ice lies
    (science-composition-crusts.md §3). T10.d's `lid` and `bulk` follow T51.c's members of the
    same names.
- **Deviations in T10.d, as built** (R09.T10.d, an undifferentiated body's bulk, the lid, and the
  highland and iron-rich rows, 2026-10-13; after T2's follow-up D, `0e931a8b` and `27db4cad`, and
  T10.c, `2f63eb3a`; science-composition-crusts.md §1, §2, §5 and §7, with §10.1's code text for
  the rows; builder arguments, revisable until P14.T54.a reads them from the record).
  - _The lid, resolved once_ (T10.c's "For R09.T10.d and T13.c", discharged). `check_crust`
    returns the lid's `SubstanceKey`, and `crust_palette` looks up its entry by the rule of the
    crust `lid` names: `Primary` its `PrimaryCrust` entry alone, `Secondary` its `SecondaryCrust`
    entry or, where the secondary and the provinces are one substance, the `Province` one, and
    `Bulk` its `Bulk` entry alone. `Lid` is that entry, the same as §1.6's
    `match crust.lid { … }` with no `Option` to unwrap. `CoarseInputs` keeps it as a private
    `lid_entry: u8`, read by `lid_entry()`, a public getter beyond the task's text, so that a step
    reads the lid's entry (and through the palette its substance and solidus) without matching the
    crust's options or unwrapping `crust_palette()[2]`; a test holds `Some(lid_entry())` equal to
    `crust_palette()[2]` on the four reference worlds. The getter returns the index, as
    `crust_palette` and `main_liquid` do, not a `&PaletteEntry` (the Rust review's nit, not taken:
    a step still goes through `palette().get`).
  - _A buried primary crust keeps its entry._ T10.c's rule stands: a primary crust, at the surface
    or buried under a `Secondary` lid, needs its own `PrimaryCrust` entry, though while buried no
    `Crust` variant names it (§5.1: large craters excavate it). The bulk material is looked up only
    as the lid's, since `BulkBesideCrust` and `LidAbsent` leave it under no other lid, and only in
    its `Bulk` role, so a chondrite with only a crust's entry is refused (`NotInPalette`, "bulk
    material"; §1.2, "nothing calls it one").
  - _Members and order._ `CrustInputs` is `primary`, `secondary`, `tertiary`, `bulk`, `lid`,
    `provinces`, `melt_area_fraction`: P14.T51.c's `CrustComposition` in its order, less its redox
    class. `LidCrust` derives as §1.6 has it and is re-exported at `planetary::surface`.
    `check_crust` refuses `NoCrust`, `BulkBesideCrust` and `LidAbsent` in §1.6's order, then
    T10.c's melt share; a tertiary crust alone is `NoCrust`. The three `Display` texts are §1.6's.
  - _§10.1's code text._ `CHONDRITIC_SOLIDUS_K` (the must-fix: LPSC 25's volatile loss),
    `BASALT_FE` and `FELDSPATHIC` take `testing/worlds.rs`'s docs word for word; the two keys keep
    §2.3's and §7.3's one-line docs, and `BASALT_FE_SOLIDUS_K` and `BASALT` take §2.3's text. The
    figures are §2.3's and §7.3's, unchanged.
  - _Documentation beyond the task's._
    - `CrustInputs::primary`'s Moon example reads `feldspathic` for `anorthosite` (§7.2), and the
      type's two forms name §1.2's fragment of a differentiated body, which no rule produces yet.
    - Each table's crust row adds its world's lid, citing §5; the Moon's palette row drops T10.c's
      forward note. The `Substance` struct's albedo exception names the feldspathic crust for the
      anorthosite, and `PLAGIOCLASE_ROCK_SOLIDUS_K`'s doc is reflowed with the same words.
    - `mod.rs`'s and `reference.rs`'s module documentation say what T10.d added.
    - `coarse_pass`'s example gains `bulk: None, lid: LidCrust::Primary`, and its phyllosilicate
      takes the reference row's 1,423.15 K (`CHONDRITIC_SOLIDUS_K` since T10.c) for the basalt
      solidus's 1,253.15 K, which T10.c left in it.
    - _The bulk material is never called a crust_ (§1.2's "nothing calls it one", which §10.1
      item 3d, ruled 2026-10-12, binds on developer documentation and error messages too; "main"
      applied it here, 2026-10-13). What this task wrote says "a crust or bulk material", and so
      do the places where §1.6's own text called it a crust: `LidAbsent`'s documentation and
      `Display`, "the lid names a crust or bulk material the body does not have" (§1.6's "a crust
      the body does not have"); `LidCrust`'s summary, "Which of a body's crusts, or its bulk
      material, is at its surface"; `lid`'s "Which crust or bulk material the surface shows"; and
      `melt_area_fraction`'s "the lid's crust or bulk material". No figure, golden or test moves
      but the message's string, which `errors_say_what_was_refused` pins.
  - _Tests._
    - `a_crust_with_neither_primary_nor_secondary_is_refused` is renamed
      `a_crust_with_neither_crust_nor_bulk_material_is_refused`, a tertiary crust alone among its
      cases.
    - New in `inputs.rs`: `a_bulk_material_beside_a_crust_is_refused` (beside each crust, and
      before a lid naming a crust the body lacks, which pins the order);
      `a_lid_naming_an_absent_crust_is_refused` (Earth's `Primary` and `Bulk`, Ceres's
      `Secondary`, a chondritic body's `Primary`, each before a melt share out of range);
      `a_chondritic_body_s_lid_and_provinces_take_its_bulk_entry` (`[None, None, bulk, bulk]` on
      Ceres's figures with a `chondrite_h` bulk, its no-op field's header taking the same, and a
      chondrite with only a crust's entry refused); and
      `a_lid_takes_the_crust_it_names_over_a_buried_primary` (a Mars with an `ultramafic` primary:
      `Secondary` takes the `basalt_fe`, the primary is refused without its own entry, `Primary`
      takes the ultramafic).
    - New in `reference.rs`: `each_reference_world_shows_the_lid_the_ruling_gives_it` (§5.2's
      lids, beyond the task's list).
    - Added to `the_grain_densities_follow_their_sources`: `FELDSPATHIC` within Kiefer et al.'s
      2,840–2,910 (§10.1, which says T10.d's _Tests_ gain it; the task's list does not yet);
      `bulk(&FELDSPATHIC, 0.12)` within 5 kg m⁻³ of the highlands' 2,550, a tolerance of this
      task's (2,550 ÷ 0.88 is 2,898, which the row rounds; Wieczorek et al.'s 12 per cent is their
      own average, so this checks the arithmetic); `BASALT_FE` within 3,270–3,460; and the Moon's
      lid and maria entries and Mars's lid carrying 2,900 and 3,310.
    - `cargo test -p hyperion-sim --lib planetary::surface` runs 41 unit tests, five of them new;
      `--doc planetary::surface` its two doctests.
  - _Moves:_ no pinned output. No reference golden exists before T16, no stream or tag is added,
    and the synthetic worlds and the wire goldens are untouched (`golden_diff.py`: nothing to
    reconcile; `GENERATOR_VERSION` stays 21). The reference worlds' unpinned fields change at
    version 21: the Moon's entry 0 becomes `feldspathic` at 2,900 kg m⁻³ (the anorthosite's 2,790;
    its transition unchanged), its entry 1 and Mars's entry 0 `basalt_fe` at 3,310 kg m⁻³ and
    1,323.15 K (basalt's 3,010 and 1,253.15 K). Every index and every `crust_palette` stays, each
    changed key being alone in its role. T16's first blessing pins them.
  - _For R09.T12.a._ An undifferentiated body's `Oceanic` and `Continental` name no entry, as
    Ceres's `Oceanic` has not since T10.c, so the step that first writes a cell's crust writes only
    `Lid` and `Province` there.
  - _For R09.T13.c._ The lid's entry is `CoarseInputs::lid_entry()`, its transition the lid's
    solidus.
  - _For P14.T51.c and P14.T54.a._ `planetary::surface::LidCrust` is R09's; T51.c's
    `hooks/crust.rs` declares its own with the same variants, which `for_body` maps one to one (or
    T51.c's replaces this one). The palette's sources must give a buried primary crust (a generated
    Mars's ultramafic) its own `PrimaryCrust` entry, one of the palette's 15, and a bulk material
    its `Bulk` entry.
  - _For the docs lane._ The task's _Tests_ list does not carry §10.1's `FELDSPATHIC` assertion
    against Kiefer et al.'s 2,840–2,910 kg m⁻³, which §10.1 says T10.d's _Tests_ gain; it is
    built, and the line waits for the docs queue. Applied 2026-10-13 (docs-batch-14, `31824f61`).
- **Drainage semantics (ruled 2026-10-10, science-r09-drainage.md §1).** T6.a found that a k_s
  stored on Hergarten's runoff-weighted area and read with Hack's geometric area makes tributaries
  2.8 times too gentle at 0.1 m a⁻¹, and trunks' Hack lengths 4 times too short.
  - The field's `drainage` and `steepness` are geometric.
  - Follow-up C adds each cell's own runoff and its catchment's mean.
  - A tributary's steepness is k_s (r̄ ÷ r)^θ.
  - Widths follow discharge.
  - The solver keeps the equivalent area to itself.
- **Cold traps (ruled 2026-10-10, science-r09-drainage.md §4).** Exposed water ice lies in the
  Moon's cold traps (Li et al. 2018), but no zonal rule makes it: a smooth lunar pole reaches
  156 K. P14.T51.f states a body's cold-trapped area as the cold-trap fraction of one closed form,
  which R09.T7.d builds early in the surface crate (split out of R09.T13.f, the galaxy audit's
  B4), times a drawn exposed fraction.
  - R09.T13.f places it per cell.
  - The reference Moon carries `H2O` as `Ice` from R09.T10.c, with no share until P14.T51.f.
  - _For R10's re-validation:_ the classifier puts a cell's cold-trapped share on its shadowed
    ground (crater floors and pole-facing slopes), not uniformly over the cell, and the share is
    exposed ice, not the cold-trap area.
  - _R09.T7.d's form, with T7.f's exact bowl_ (science-r09-drainage.md §8 and §8.13): the plains
    are labelled half-normal bowls at 0.6 of the surface's half slope, with a fitted share of
    light from beyond their hollows, and equilibrium at the highest sun stands in for T_max.
    - At Hayne et al.'s best fit with B, it gives Table 1's permanent shadow within 15% over
      80–90°, 70–80° and the whole Moon, and its cold traps within 12% over 80–90° and the whole
      Moon.
    - Over 70–80° it keeps 0.12% (B) and 0.025% (A) as cold traps. Table 1 has 7 × 10⁻⁴% and
      Diviner's resolved maps 0.032% (Landis et al. 2022). Table 1 counts the cold share of all
      bowls (its eq. 32), not each cold bowl's own shadow.
    - Distribution A's deep bowls keep 1.5, 8 and 3 times Table 1's permanent shadow over
      80–90°, 70–80° and the whole Moon, and 0.93% of 60–70°, against the LOLA maps' ~0.08%. B
      keeps 0.003% there, since it has no fresh deep bowls. The Moon's craters lie between them.
    - Under the real Sun, A's cold traps over 70–80° (0.016%) fall below Diviner's resolved
      figure.
    - A peak-temperature form understates supervolatile traps (Schorghofer et al. 2021: 3.5
      times for CO₂). Spherical bowls cannot make Amundsen's CO₂ traps at 84.5°, where flat
      floors can, and the model is symmetric where the Moon's north has almost none.
    - The conduction share is lunar, and independent of latitude.
  - _The plains against ray-traced Gaussian surfaces_ (science-r09-drainage.md §8.13).
    - A bowl keeps several times the permanent shadow of a Gaussian surface of its mean-square
      slope, since its shadow stays put as the star circles. So the plains' bowls are scaled to
      0.6, which matches the ray traces within 0.87–1.09 over 80–90° at σ_s 5.7–26.6° (D 1.54°),
      and within 1.00–1.17 at D 4°.
    - So their instantaneous shadow is 0.38–0.92 of Smith 1967's, and their shadow at the pole
      itself 1.3–1.6 times the ray traces'. At D 10° and 20° on rough plains they keep up to 2.4
      and 15 times the ray traces' shadow.
    - Their shadows take the bowl's temperature plus 0.0035 S sin e from beyond the hollow. At
      σ_s 5.7° this matches the ray traces' water and CO₂ cold traps. On rougher plains it still
      overstates them: water by 1.3, 1.9 and 2.8 times at σ_s 11°, 17° and 27°, and CO₂ by 1.4–9
      times, since a rough surface's shadows see more sunlit terrain. R09.T7.g replaces both fits.
    - Hayne et al.'s eq. 5 overstates a spherical bowl's shadow (off-centre transects are not
      similar), so the exact transect form replaces it.
- **Deviations in T7.d, as built** (2026-10-10; the inputs ruled in science-r09-drainage.md §8,
  the plains in §8.10 and the cap in §8.11).
  - _The API._ `cold_trap::fraction(latitude: Radians, &Body, &Terrain, threshold: Kelvin) -> f64`
    and `cold_trap::permanent_shadow(latitude, &Body, &Terrain) -> f64`, the inputs grouped in the
    crate's Parts pattern:
    - `Body::new(BodyParts)`, refused by `BuildBodyError`: `flux`, `greatest_declination`,
      `star_radius`, `bond_albedo`, `emissivity` and `heat_flow`;
    - `Terrain::new(TerrainParts)`, refused by `BuildTerrainError`: `crater_fraction`,
      `crater_depths`, `plains_slope` and `share_under_cutoff`;
    - `DepthRatios::new(mean, variance)`, refused by `BuildDepthRatiosError`, with Hayne et al.'s
      `DepthRatios::HAYNE_A` and `HAYNE_B`;
    - `LUNAR_SHARE_UNDER_CUTOFF` (0.05), and `shadow_temperature(depth_ratio, elevation, &Body)`,
      Ingersoll et al.'s with the heat flow's term, public for the 97 K test and for callers;
    - panics on a latitude outside ±90°, a threshold that is not finite and positive, a depth
      ratio outside (0, 0.5] or an elevation outside [0°, 90°] (the `cold_trap` module of
      `tests/panics.rs`).
  - _Eq. 26 is capped by the instantaneous shadow._ A bowl's permanent shadow is the smaller of
    eq. 26 and its instantaneous shadow at the upper limb's highest elevation, since a permanent
    shadow lies inside every instantaneous one; eq. 26, a small-angle form, otherwise leaves deep
    bowls near the equator in permanent shadow. The instantaneous shadow is §8.11's exact transect
    form (eq. 3 per transect at its own depth ratio, in y = sin θ by a 16-point Gauss–Legendre
    rule). The deep bowls' part of a population, from where the two meet (found by 40 bisection
    steps), takes four panels of the same rule over the population's own ±8σ, in ln γ for the
    log-normal and γ for the half-normal, so that a narrow population is resolved wherever it lies.
    It matches an 801 × 801 ray grid to 10⁻⁴ (the test holds 0.005 on 601 × 601). Against §8.10's
    figures, which had no cap: B's bands do not move under it (the remaining differences from
    §8.10's table are its coarser sums); A's whole Moon falls from 0.371% to 0.367% and its 50–60°
    from 0.103% to 0.097% (6%); Ceres with A falls from 3,720 to 3,640 km² at a 4° declination and
    from 46 to 28 km² at 20° (§8.11's 3,659 and 29 km² were under eq. 5's cap). P14.T51.f's Ceres
    bounds hold (215 ≤ 2,282 ≤ 3,640 km², and 28 < 68 km²). A bowl deeper than a hemisphere counts
    as one, at the hemisphere's cos² e.
  - _Eq. 5's normalisation_ (§8.10's check) is the bowl's: the shadow's horizontal projection over
    the opening's area πD²/4, Hayne et al.'s `A_crater`. Its excess over exact ray geometry, up to
    about 17%, is their eq. 4's: eq. 3 is exact on the transect through the centre (0.559 at d/D
    0.1 and e 10° by both), but the off-centre transects are not similar to it (0.457 at 0.6 of
    the radius, against eq. 4's 0.559). The code uses eq. 5 nowhere.
  - _The plains are half-normal_ (§8.10, replacing §8.1's log-normal of the same moments). With B
    the Moon's permanent shadow is 5.67% of 80–90°, 0.155% of 70–80°, 0.0026% of 60–70°,
    5 × 10⁻⁵% of 50–60° and 0.093% overall; its cold traps are 4.61% of 80–90° and 0.070% overall
    (A: 4.47% and 0.068%). The plains' cold-trap share over 80–90° peaks at a σ_s of 12°. The
    moment of γ⁻¹ takes the exponential integral E₁: its power series (30 terms) to x = 1.5 and its
    even continued fraction (64 levels, from the tail) above, within 6 × 10⁻¹⁵ relative of a
    60-digit series from 10⁻³⁰⁰ to 30. The β term is left out where its coefficient is zero, at a
    pole the limb never leaves, where E[γ⁻¹] diverges.
  - _Smith's shadow._ §8.8's 30% and §8.10's 25% (from §8.1's 26.5% and §8.10's 23.3%) rested on a
    wrong root in the lane's prototype (`(√(β² + 16) − β) ÷ 8` for the shallowest shadowed bowl,
    where 1 ÷ 2γ − 2γ = β gives `(√(β² + 4) − β) ÷ 4`), and are withdrawn (§8.11). With the right root and the exact bowl the
    test asserts §8.11's bounds on a 0.5° grid: within 15% to 4° (measured 11.4%), and at most 1.8
    times from there to 12° where Smith's shadow is at least 5% (measured 1.75).
  - _The inputs' ranges._ `Terrain` refuses a plains' RMS slope above 45°; `DepthRatios` a mean
    above 0.5 or one whose square is not a normal number, and a variance too small beside the
    mean's square for the log-normal to have a spread or so large that its moment of 1 ÷ γ
    overflows; `Body` a star radius of 90° or more, a nonzero star radius or greatest declination
    whose square is not a normal number (at the pole such a declination would leave the plains'
    E₁ infinite), a Bond albedo of 1 and a subnormal emissivity. A plains' slope under 10⁻¹⁰⁰ rad
    counts as flat. The 45° ceiling on the plains' slope is because the stand-in is checked against
    Smith only to 26.6°, and at tan `σ_s` = 1 about a third of its bowls would be deeper than a
    hemisphere. _For T13.f:_ clamp the synthesis's slope at 45° before `Terrain::new`.
  - _The bands' figures._ §8.1's whole-Moon permanent shadow for B, 0.104%, was the prototype's
    poleward of 70° only; with §8.1's log-normal plains it was 0.111% over the whole Moon, and
    with §8.10's half-normal it is 0.093%.
  - _The plan text._ T7.d's tests lead with "Its tests:", not the ruled "Tests:", so that
    `plan_task.py` keeps them with T7.d instead of filing them under all of T7. §8.11's plains test
    is one list, its closing paragraph a third item and the peak sentence moved before it, which
    Prettier would otherwise loosen. In P14.T51.f the
    `permanent_shadow` sentence stands before "Its inputs are:", not after the bullets, which
    Prettier would set apart as a loose paragraph.
  - _The cold bowls' depth ratio_ is found by 64 bisection steps on the view factor, a fixed
    count (the heat flow's term makes the balance a cubic).
  - _Tests beyond the task's:_ §4.1's 83 and 70 K at d/D 0.14 and 0.1; 24.9 K in a d/D 0.2 bowl
    at 0.018 W m⁻², and at a pole the Sun never reaches, 20 K trapping nothing and 40 K every
    shadow; the flux-weighted elevation (2α ÷ 3π on the horizon, continuous where the disk clears
    it, never below the centre); the temperature rising with depth and sun; the 70–80° bound held
    with A too; no water cold trap at a 25° declination as well as 30° (T13.f's); Hayne et al.'s
    "σ" read as a variance; E₁ against a 60-digit series; the half-normal's closed-form moments
    against a quadrature; the instantaneous shadow capping a d/D 0.45 bowl at 0° and 30°, and the
    crossing at 80° strictly inside the population's range; the Gauss–Legendre rule exact to
    degree 31; a smooth crater-free body; the inputs' refusals, a variance that overflows the
    moment of 1 ÷ γ, a star radius and a declination with no normal square and a subnormal
    emissivity among them; the panics.

- **Deviations in T7.f, as built** (2026-10-12; decision-r09-t7f.md, science-r09-drainage.md
  §8.12–8.13).
  - _The axes and the table's home_ are decision-r09-t7f.md's, which replaced the task's text
    before the build: the plan's degree axes (colatitude 0–45°, δ + α 0–30°) are withdrawn.
  - _The method._ A floor point is in permanent shadow exactly when every direction toward the rim
    has a declination above D = δ + α. The cone of the opening is convex and holds the zenith,
    whose declination 90° − e₀ is above D, so the chord test at the band's upper edge decides it.
    The fit finds each lattice point's least rim declination once (a 48-point scan and 24
    golden-section steps), and so serves every D of a row at once. It counts a 256-point lattice
    to the rim's radius on the half disk, within 2 × 10⁻⁴ of a 1,024-point one. Shares off the pole
    are rounded to four places, which keeps the file at 147 kB. One run takes 45 s on one thread,
    so the task is fast, and `check --rerun-fast` reruns it. One and four threads give the same
    bytes.
  - _The acceptance line_ gives Δ for δ + α, since a wrapped "+ α" at a line's start would be a
    Markdown list item to Clippy's `doc_lazy_continuation`. Its figures cover the d/D 0.005 bowl
    too, and its pole-row figure samples the limb's nodes as well as its cells' centres, eight
    points a depth-ratio cell: 0.0045, against decision-r09-t7f.md's 0.0046 by a denser sweep (at
    d/D 0.454 and Δ ÷ θr 0.2). `SHADOW` is a `pub static`, emitted
    whole as `stellar_fates`'s and `binary_reach`'s large arrays are (`large_const_arrays`). The
    surface crate's `tables/mod.rs` keeps `#[rustfmt::skip]` on `MANIFEST`, whose one row rustfmt
    would fold, and `#[allow(clippy::approx_constant)]` on the table's module, as the sim's tables
    have.
  - _Public items beyond the text._
    `cold_trap::bowl_permanent_shadow(depth_ratio, colatitude, limb) -> f64`, one bowl's read with
    the cap, which the fitting crate's acceptance test reads;
    it panics on a depth ratio that is not finite and positive, a colatitude outside [0, π/2] or a
    D outside [0, π) (`tests/panics.rs`). The fitting task's module exposes `Params`, `rim_slope`,
    `shadow_shares`, `polar_shadow`, `TracedPoint` and `traced_points`, `BowlShadowFit` (with
    `interpolate`, the read without the cap), `fit`, `figures`, `Figures`, `Worst` and `render`;
    its ray tracer is the tests'. The surface crate's `tables` imports `TableInfo` privately.
  - _The integral over d/D._ T7.d's closed-form moments (the exponential integral E₁ and the
    partial moments of γ⁰, γ¹ and γ⁻¹) and its 40-step crossing go with eq. 26. Each population
    takes the trapezoid rule on 512 fixed intervals of its own ±8σ window (ln γ for the log-normal,
    γ for the half-normal), and stops at the cold limit under its interval's chord. The nodes
    depend on the population alone, so every monotonicity of the tests holds to the bit, and
    `fraction` ≤ `permanent_shadow` by construction. A Gauss–Legendre composite whose panels
    followed the cold limit broke both by 10⁻⁴ of themselves. The bands moved by under 10⁻⁵ of
    themselves against 16 panels of the 16-point rule.
  - _The flat limit._ A zero colatitude or D reads as zero over any slope, so that γ → 0 at a pole
    the star never leaves reads the first row's 1, not 0 ÷ 0.
  - _Kept._ `Body::new`'s refusal of a declination or star radius with no normal square, and
    `DepthRatios::new`'s of a variance whose moment of 1 ÷ γ overflows, stay as T7.d built them,
    though E₁ no longer needs them: the API is unchanged.
  - _Tests replaced._ E₁'s series, the half-normal's moments, the crossing at 80° and the
    instantaneous cap of a d/D 0.45 bowl at 30° go with the code they tested. The cap's test now
    holds the read's overshoot of the instantaneous shadow to 0.005 from 10° to the pole, and the
    capped read never above it. A new test holds the trapezoid rule to each population's mass to
    10⁻⁵ (its h² f′ ÷ 12 where it stops) and never falling as the cold limit rises. Eq. 8's first
    order is held as a gap of at most 1.5 (β² + 4) δ², since x₀″ = 1 − βδ − 2δ² + O(δ³). The
    exact transect test and the plains' cold-trap peak (now at σ_s 16°) are unchanged and pass;
    §8.13's tests replace the plains' Smith test.
  - _The client's module_ is byte-identical, 108,985 B before and after (`just gen-surface`).
  - _The plan text._ T7.f's acceptance closes its last bullet rather than standing as a bullet of
    its own, so that `plan_task.py` keeps it with T7.f and does not file it under all of T7, as
    T7.d's "Its tests:" does.
  - _The plains_ (science-r09-drainage.md §8.13): the half-normal's scale is 0.6 × tan σ_s ÷ 2
    (`PLAINS_DEPTH_SCALE`), and the plains' shadows absorb 0.0035 × S sin e more
    (`PLAINS_SURROUNDINGS`, through `coldest_limit`'s new `surroundings` argument, so `fraction`
    takes one cold limit for the craters and one for the plains). Both are private; the API is
    unchanged. §8.11's Smith test is replaced by the ray-traced plains' tests, and the plains'
    water peak moves to σ_s 16°.
  - _The bands as built_, which reproduce §8.13's figures to the digits it quotes. Point Sun,
    Hayne et al.'s best fit, 110 K, no conduction share:

    | band                   | Table 1   | B           | A      |
    | ---------------------- | --------- | ----------- | ------ |
    | cold traps 80–90°      | 6.7%      | 6.99%       | 4.73%  |
    | cold traps 70–80°      | 7 × 10⁻⁴% | 0.118%      | 0.025% |
    | cold traps, whole Moon | 0.10%     | 0.112%      | 0.073% |
    | PSR 80–90°             | 8.5%      | 7.42%       | 12.7%  |
    | PSR 70–80°             | 0.5%      | 0.452%      | 4.10%  |
    | PSR 60–70°             | ∼0        | 0.0031%     | 0.93%  |
    | PSR 50–60°             | ∼0        | 5.1 × 10⁻⁶% | 0.137% |
    | PSR, whole Moon        | 0.15%     | 0.133%      | 0.461% |
    - The real Sun (1,407.6 W m⁻², α 0.2711°, q 0.018 W m⁻², conduction share 0.05) gives water
      cold traps over 80–90° and the whole Moon of 5.84% and 0.092% with B, and 3.87% and 0.059%
      with A, inside P14.T51.f's 1.6–13% and 0.024–0.2%.
    - CO₂ under the real Sun, half the Moon's area: B 86.9 km² at 52.9 K and 198.1 km² at 55.6 K,
      A 85.3 and 190.9 km², inside 30–408 km²; at the rule's 57 K, B 280 and A 266 km² (recorded).
    - Ceres at 4°: B 1,050 km² ≤ 2,282 ≤ A 7,558 km²; at 20°, B 0 and A 52.7 km².
    - The plains alone keep 2.79%, 22.9% and 38.1% of 80–90° at σ_s 5.7°, 16.7° and 26.6°, and
      0.049%, 6.32% and 17.3% of 70–80°; their instantaneous shadow is 0.383–0.915 of Smith's;
      under the real Sun at σ_s 5.7° they trap 2.27% of 80–90° below 110 K and 0.121% below 57 K.

- **Rulings of "main" for T10's findings** (2026-10-10).
  - `for_body`'s missing arguments, the record's time and a moon's planet's orbit, are settled by
    P14.T54.a, which owns `planetary/surface/inputs.rs`. R09.T17 and R09.T19.a take `for_body` as
    T54.a leaves it. _Amended 2026-10-15 (decision-backlog-2 §3):_ `for_body`'s signature is
    R09.T10.f's, `for_body(record, planet, ctx)`, which carries a moon's planet's record, both
    records taken at `RECORD_TIME` (the epoch, the sim's constant since R09.T10.e), and it is frozen
    at 23. P14.T54.a only connects the crust, the condensates and the palette's resolution, and
    R09.T17 and R09.T19.a take the signature from T10.f.
  - The airless mean temperature is imposed as the radiative mean ⟨T⁴⟩^¼ in R09.T13.c–d, T10's
    lean, with T13's science check to confirm. `CoarseInputs::mean_surface_temperature`'s
    documentation says so.
- **Deviations in T17, as built** (2026-10-13).
  - _Files._ The task's `surface/{mod,cache,service,inputs}.rs` and `config.rs`, with `lib.rs`
    (the `AppState::surface` field, its start and its shutdown), `stats.rs`
    (`ServerStats::surface`), `tests/common/surface.rs`, `tests/surface_service.rs` and the
    README's table of options; and in the surface crate `CoarseField::heap_bytes` (below).
  - _The API T19 wires._
    - `SurfaceService::new(pool, source, budget_bytes)`, taking an `Arc<CpuPool>`, an
      `Arc<dyn InputsSource>` and a `usize`.
    - `async fn field(&self, key: FieldKey, system: &Arc<GeneratedSystem>)`, which gives a
      `Result<Arc<CoarseField>, SurfaceJobError>`.
    - `fn prefetch(&self, key, system) -> bool` (whether it started or joined a pass),
      `fn stop_prefetching(&self)`, and `fn counters(&self) -> SurfaceCounters` (`cache()`, an
      `LruCounters`, and `computed()`, the passes run).
    - `FieldKey::new(GalaxyKey, BodyId)` with `galaxy()` and `body()`; `FieldCache` (`new`, `get`,
      `insert`, `charge`, `counters`) and `impl HeapBytes for CoarseField`.
    - `SurfaceJobError`, Provides' name kept: `Body(ResolveBodyError)`, `Inputs(SurfaceInputsError)`
      and `Compute(ComputeError)`, with `From<ComputeError>`. (Amended by R09.T10.f:
      `Inputs(ReadCoarseInputsError)`, whose `Section(..)` arm carries the `SurfaceInputsError`
      answers below, every other arm answered `internal`; Risks, "Deviations in T10.f, as built".)
    - `InputsSource: Debug + Send + Sync`, whose one method is Provides' `inputs`, and
      `RecordInputs`.
    - `RECORD_TIME`, the time the record is read at (below).
    - In `config.rs`: `ENV_SURFACE_CACHE_MB`, `DEFAULT_SURFACE_CACHE_MIB` (128), `--surface-cache`,
      `ServerConfig::surface_cache_bytes` and its builder's, on the sky cache's pattern; and,
      beyond Provides, `ServerConfig::surface_inputs` and the builder's `surface_inputs`, which
      takes an `impl InputsSource` (by default `RecordInputs`), on `scene_knowledge`'s pattern.
      T19.c's socket test gives the reference worlds through it.
  - _The system comes from the caller._ The service holds no galaxy, system or body cache:
    `field` and `prefetch` take the body's planetary system, which a handler has from
    `requests::system::bodies_of` (as `body_detail` takes it), and panic if it is not the key's
    body's (its system ID, and for a body with a surface seed that seed against the key's
    universe's), or if the key's generator version is not one this build generates
    (`GeneratorVersion::is_supported`, as the galaxy cache asserts), so that no field is kept
    under another version's key. T19's handler: `openable_universe`, the galaxy, `bodies_of` (its `NoSuchSystem`
    through `body_refusal`), then `state.surface.field(FieldKey::new(universe.key(), body), &system)`.
  - _One bulk job._ It decodes the index, takes the seed from `PlanetarySystem::surface_seed` (a
    ring, a belt or the halo has none: `Inputs(NoSolidSurface)`), reads the record, asks the source
    and runs `coarse_pass`. A refusal is not kept, as no error is: a generated body's `NotModelled`
    costs a bulk job each time it is asked, behind any other bulk work (Deferred findings, below).
  - _The record's time._ The record is read at `RECORD_TIME`, `UniverseTime::EPOCH`, whoever asks
    and whenever: the field is keyed without a time, so a record read at the asker's time would
    make the field kept depend on who asked first. The smallest reversible choice; P14.T54.a, which
    settles `for_body`'s time (the ruling above), settles this one.
  - _The refusals, for T19.a._ `Body(e)` through `body_refusal(&request.body, e)`;
    `Inputs(NoSolidSurface)` `bad_request` naming `body`; `Inputs(NotModelled(section))` the wire's
    `SurfaceFieldDto::NotModelled { section }`; `Inputs(NotResolved(_))` `internal` (T10's
    record); `Compute(e)` through `RequestError::from(e)`, so `FloatingPointMode` and a faulted
    pool are `internal`. The service logs those two at `error`, naming the body; the test checks
    the refusal and its code, not the log line.
  - _The charge._ `HeapBytes` counts a `Vec` by its capacity, which the server cannot see through
    `CoarseField`'s getters, so the surface crate gains `CoarseField::heap_bytes` (a public item
    beyond Provides, with `pub(super)` helpers on `Cover` and `MaterialPalette`): its records, its
    craters and their reaches, its per-cell crater index and its palette, by capacity. Not
    generated output; nothing that generates reads it. Measured on the reference worlds at version
    21, before T12.d's craters: 24.4 MB at level 8, 6.1 MB at 7, 1.5 MB at 6 and 0.38 MB at 5, so
    the default 128 MiB (Provides'; Design note 19 gives the 32 GB context) keeps five Earths'
    fields (an Earth's whole payload is 14.7 MB).
  - _For R10.T10.a._ The albedo scale's call site is `service::compute`, after `coarse_pass`, with
    the record's photometry; the tests that compare the served field with `coarse_pass` bit for
    bit (`tests/surface_service.rs` and two in `surface::service`) then apply the same
    `with_albedo_scale`.
  - _Not here._ The survey logs (T19.a opens them, T18's record); `survey_pass` starting a field
    (T19.a calls `prefetch`); any request handler or wire type.
  - _Test helpers._ `tests/common/surface.rs`: `reference_inputs(ReferenceWorld)` (`Earth`,
    `Mars`, `Moon`, `Ceres`, not `EarthLike`…, which Clippy's `enum_variant_names` refuses), an
    `InputsSource` giving every body whose surface section applies the world's inputs and a giant
    `NoSolidSurface`; `surface_system()`, a generated system of seed `0x4d2` with a solid body and
    a giant; and `SEED`.
  - _Tests._ `surface::service`: two requests at once compute once (the source's calls and
    `computed`, three misses); the cache bounded and evicting the least recently used, computed
    again; a budget of nought keeps nothing; a cancelled pass, queued or running, keeps nothing;
    a prefetch joined by a request, a prefetch with none, prefetches given up at shutdown; a
    flushing worker refused (`internal`), then the faulted pool; a malformed index, the primary's
    and an empty slot refused as the body's; the errors' texts and causes; another system's and
    another generator version's refused (panics). The eviction test's field computed again is
    the first's payload, to the bit. `surface::cache`: the charge and the budget. `config`: the default source and a
    given one. `tests/surface_service.rs`: the field from the Ceres and Moon reference worlds is
    `coarse_pass` of the body's seed on the same inputs, compared as whole payloads; with
    `RecordInputs` a solid body answers `NotModelled(Surface)` and a giant `NoSolidSurface`, and so
    does a giant with the reference inputs. The surface crate: `heap_bytes` by capacity, exact and
    with spare room.
  - _Acceptance._ `cargo test -p hyperion-server surface` runs 16 unit tests (13 under
    `surface::service`, 2 under `surface::cache`, and `config`'s
    `the_surface_inputs_are_the_records_unless_given`) and the 2 of `tests/surface_service.rs`.
    The filter does not select the tests this task edited elsewhere: `config`'s that pin the
    option (`defaults_match_the_builder`, `every_option_is_read`,
    `every_option_falls_back_to_its_variable`, `a_cache_budget_is_written_in_mib`,
    `the_builder_overrides_defaults`), `lib.rs`'s `counters_move_as_requests_are_accepted_run_and_finish`
    and the surface crate's `a_field_counts_its_heap_bytes_by_capacity`, so T17 also ran
    `cargo test -p hyperion-server --lib config::` with `counters_move`, and
    `cargo test -p hyperion-surface --features testing --lib heap_bytes`; the gate, `just smart-ci`,
    ran the whole server and the surface tests the change reaches on every target.
- **Deferred findings (2026-10-13 rule)** (R09.T17). Recorded and deferred: each leaves the
  algorithm and every contract as they are.
  - _The quantiser's spare capacity._ `quantise`'s `collect::<Result<Vec<_>, _>>()` grows the
    records' vectors by doubling, so a level-8 field holds 5.3 MB of spare capacity (24.4 MB
    charged against 19.0 MB of records). An exact capacity in the quantiser, or a `shrink_to_fit`
    in `CoarseField::new`, saves it; no output moves. Lean: with T16's benches.
  - _Concurrent passes._ Passes of different bodies run at once, as many as the pool's workers.
    Once T11–T15 fill the steps, a level-8 pass's working state (`f64`s a cell) may reach some
    100 MB, so a pool of 15 could hold over a gigabyte in passes. Lean: measure the pass's peak
    with T16's benches, and bound the passes in flight (a semaphore in the service) if it is large.
  - _A refusal waits for a bulk slot._ A generated body's `NotModelled` is answered by a bulk job,
    behind a sky's census. Lean: T19.a reads the record's surface tag in an interactive job before
    the bulk pass, or P14.T54.a's inhabited section makes it moot.
  - _The reference worlds name another body._ `reference::*` builds inputs with `BodyRef::new(0, 1)`
    to `BodyRef::new(0, 4)` (system 0, bodies 1–4), so a field served from them names that body in
    its header and blocks, whatever body was asked for. T19.c's checks pass (the blocks agree with
    their header), but a client that compares the header's body with the one it asked for would
    refuse it. Lean: a `CoarseInputs` method that changes the body on built inputs, or
    `reference::*` taking a `BodyRef` (the builder already has `body`), in the sim when T19.c
    needs it.
  - _Repeated prefetches._ A prefetch of a body whose pass is in flight spawns one more waiter on
    the same flight, and finished waiters are reaped only by the next prefetch, as the sky
    tables' are; session code calling it often on approach grows the set by its call rate times
    the pass's length. Lean: a `SingleFlight` query for a key in flight, with session code.
- **Deviations in T19.a, as built** (2026-10-13).
  - _Files._ The protocol's new `src/surface.rs`, with `envelope.rs` and `lib.rs`, the bindings
    `just gen-protocol` writes, and `packages/protocol/src/index.ts`'s re-exports. The server's new
    `requests/surface.rs` and `convert/surface.rs`, with `requests/mod.rs`, `convert.rs`,
    `convert/planetary.rs`, `limits.rs` and `surface/{mod,service}.rs`. The client's
    `apps/hyperion/src/tools/placeShip.test.ts`, whose fake server's switch over request kinds
    oxlint checks for exhaustiveness, gains the two kinds.
  - _The wire, for R10.T7._
    - `surface_field` takes `SurfaceFieldRequest { universe, body, have_revision }`.
      `have_revision` is `number | null` on the wire, a plain `Option` as `SkyRequest`'s are, and
      not an absent key.
    - It is answered by `SurfaceFieldDto`. Its arms are tagged by `state` beside the response's
      `kind`, as `SectionDto` tags its own:
      - `ready` with `header`, `revision`, `coverage` and `bulk`;
      - `not_modelled` with `section`, a new `RecordSectionDto` (the record's twelve sections,
        snake case).
    - `Ready.header` is a `Box<SurfaceHeaderDto>` because of Clippy's `large_enum_variant`, so
      `ResponseBody::SurfaceField` holds the enum unboxed.
    - `CoverDto { ranges: [CoverRangeDto { start, end, resolution }] }`.
    - `survey_pass` takes `SurveyPassRequest`, whose members are `universe`, `body`, `from`, `to`,
      `source`, `resolution_m` and `cells`, a list of `CellRangeDto { start, end }`.
      `SurveySourceDto` is `orbital`, `close_range` or `landed`.
    - It is answered by `SurveyPassDto { revision }`.
  - _`SurfaceHeaderDto` (ruled by "main", 2026-10-13)._ The members are not in the plan.
    - What it carries: the header's scalars with their units in their names: `body`, `level`,
      `radius_m`, `equatorial_radius_m`, `polar_radius_m`, `boundary_diameter_m`, `sea_level_m`,
      `lapse_rate_k_per_m`, `realised_sigma_h_m`, `realised_relief_m`, `months`,
      `season_eccentricity`, `reference_temperature_k`, `surface_age_gyr`, `surface_pressure_pa`
      and `albedo_scale` (`null` until R10.T10.a; as built there, `null` means no scale, the
      palette's albedos unscaled, sent where the photometry is not modelled, where the air hides
      the ground at 30 kPa and up, where the photometric law is a cloud deck's, and where the
      palette names no substance). It also carries the palette as
      `PaletteEntryDto { substance, role }`.
    - The closed classes are closed string unions, so that TypeScript's are exhaustive:
      `climate_model` (`ClimateModelDto`), `precipitation` (`PrecipitationSourceDto`) and each
      palette `role` (`PaletteRoleDto`). They mirror the surface crate's coded enums. A substance
      stays an open registry key, a string.
    - What it leaves to block 0: the band spectrum, the crater parameters, the palette's optical
      and mechanical values, the climate records' steps (`temperature_step`, `anomaly_step`) and
      the palette indices `crust_palette` and `main_liquid`. `main_liquid` would let the main
      thread name the sea; adding it later is additive.
    - Members may be added later. Removing one is a contract change.
    - `body` is the header's own, so a field served from the reference worlds names their body
      (T17's deferred finding).
  - _`surface_field` is not served yet._ The kind is the protocol's, and the handler answers it
    `unsupported`, as `body_events` is answered, until T19.b serves it. `is_large` is true for it
    whatever its arm.
  - _The `survey_pass` handler._
    - Checks, in `body_detail`'s order:
      - the universe;
      - `from` and `to` in the clock window, each named (`convert::clock_time`, which
        `query_time` now calls with `time`);
      - `to` before `from`, refused naming `to`;
      - `resolution_m` with no `ResolutionCode`;
      - more than `MAX_SURVEY_RANGES` ranges, refused naming `cells`;
      - the body ID's two parts.
    - Then the body's system comes from `bodies_of`.
    - The level of its field is read on an interactive job, `SurfaceService::field_level`, which
      returns `FieldLevel::{Modelled(level), NotModelled { level, section }}`. The same job gives
      T19.b a refusal without a bulk slot (T17's deferred finding, "A refusal waits for a bulk
      slot").
    - Only then are the cells checked against the level's cell count (`SurveyPass::new`, its
      errors named `to`, `resolution_m` or `cells`). Then the pass is recorded.
    - A body whose inputs are modelled starts its field (`prefetch`), and the answer does not wait
      for it.
    - A pass that has reached the log stays there if its request is cancelled.
    - Refusals: `SurfaceJobError::Body` goes through `body_refusal`. `NoSolidSurface` is
      `bad_request` naming `body`. `NotResolved` is `internal`, logged. `Compute` is answered as a
      pool job's error is (`convert::surface_refusal`).
  - _A body whose inputs are not modelled (ruled by "main", 2026-10-13)._ Every solid body is in
    this case under `RecordInputs` today.
    - Its pass is recorded all the same, since coverage is Knowledge.
    - The level is `coarse_level` of the record figure's volumetric radius, the radius `for_body`
      will read. Where the source gives inputs, the level is their radius's, so that tests on the
      reference worlds agree with the field's header.
    - No field is started.
    - A body whose figure is not modelled either has no level, and is refused `bad_request` naming
      `body`. A withheld figure is `internal`, as a withheld section of the inputs is, and a figure
      not applicable (a body absent at the epoch) is `NoSolidSurface` (`service::level_from`).
    - `SurveyPassDto` gains no `NotModelled` arm.
  - _The record's time._ A pass's body is resolved, and its surface section read, at T17's
    `RECORD_TIME`, the epoch, not over the pass's `from` to `to`. A body not yet formed at the epoch
    is refused as having no solid surface, and one gone before `from` is surveyed. P14.T54.a
    settles it with T17's.
  - _Public items beyond Provides._ On the wire: `SurfaceHeaderDto`, `PaletteEntryDto`,
    `PaletteRoleDto`, `ClimateModelDto`, `PrecipitationSourceDto` and `RecordSectionDto`. In the
    server, crate-private: `surface::FieldLevel`, `SurfaceService::field_level(key, system, token)`
    and `SurfaceService::survey_log(store, universe)`, and `convert::clock_time`.
  - _The survey logs._
    - `SurfaceService::survey_log(store, universe)` keeps each universe's `SurveyLog` in a map and
      opens it through a `SingleFlight<UniverseId, SurveyLog, Arc<LoadKnowledgeError>>`. Two first
      requests therefore open it once, and a failed open is not kept.
    - The refusals (`convert::surface`) are logged with their causes at `error`, and their messages
      name no path of the server's:
      - a later format is `unsupported_save_format`;
      - a log that cannot be read or is damaged is `internal`;
      - a save that fails is `storage_failed`;
      - `RecordSurveyError::Level` or `Revisions` is `internal`.
  - _Tests_ (`requests::surface`, and `surface::service`'s
    `two_first_asks_for_a_survey_log_open_it_once`):
    - the wire forms of every DTO and enum string;
    - the two kinds in the envelope's walk and pinned list;
    - passes recorded, with revisions 1, 2, and 1 for another body, folded at the field's level and
      kept in the file across a fresh open, and both bodies' fields started;
    - two first passes sharing one log;
    - a body not modelled surveyed at its figure's level, with no field started and its last cell
      bounding the runs;
    - a giant refused naming `body`;
    - reserved and unused indices and unknown systems refused as `body_detail` refuses them;
    - each bad time, resolution and cell list refused naming its field;
    - the widest 256-range frame at most `MAX_INBOUND_FRAME_BYTES`, with 256 ranges recorded over
      the socket and 257 refused naming `cells`;
    - `is_large` for both kinds.
  - _Acceptance._ `cargo test -p hyperion-protocol surface`, then
    `cargo test -p hyperion-server requests`, and `just gen-protocol-check`, which the gate,
    `just smart-ci`, runs. The two filters miss tests this task edited or added, which T19.a ran
    by name and the gate ran with the rest of both crates:
    - the protocol's `envelope::tests` `request_kinds_lists_every_variant`,
      `response_kinds_are_the_request_kinds`, `request_kinds_are_pinned`,
      `every_body_round_trips_inside_its_envelope` and `surface_field_and_survey_pass_wire_forms`;
    - the server's `limits::tests::every_limit_is_the_plans`, and `surface::service`'s
      `two_first_asks_for_a_survey_log_open_it_once` and
      `a_field_s_level_comes_from_the_inputs_or_else_the_figure`;
    - `convert::surface`'s three refusal tests.
- **Deferred findings (2026-10-13 rule)** (R09.T19.a). Recorded and deferred: each leaves the
  algorithm and every contract as they are.
  - _No client constant for the range cap._ `MAX_SURVEY_RANGES` is the server's (`limits.rs`, as
    Provides puts it), so a TypeScript client that splits a long pass has no generated constant.
    Lean: export it from the protocol, as `MAX_SKY_STARS` is, when a client first sends passes (R10
    sends none).
  - _Inputs read twice._ `field_level` reads the record and asks the source, and the pass that a
    prefetch starts reads both again on its bulk job. That costs one record evaluation and one
    inputs call a pass: cheap with the reference worlds, and `for_body`'s cost is unknown until
    P14.T54.a. Lean: nothing unless a measurement shows it.
  - _A prefetch per pass._ Every `survey_pass` of a modelled body calls `prefetch`. During a field's
    pass, each adds one waiter, as T17's "Repeated prefetches" finding records. Lean: the same fix,
    a `SingleFlight` query for a key in flight.
  - _A poisoned log is kept._ After a pass panics part-way, `SurveyLog` refuses every later pass
    as `Poisoned` until it is opened again. The service keeps the log, so every later pass of that
    universe is `storage_failed` until a restart. Lean: drop the service's entry when a record
    answers `Poisoned` and the entry is still that log (the same `Arc`), so that no two writable
    logs of one file live at once; with P12.T8's holder.
  - _The store is the caller's._ `survey_log` takes the `UniverseStore` each call, and the open
    in flight uses the first caller's. The server has one store, so nothing differs. Lean: give
    the service the store at construction when P12.T8's holder takes the logs over.
- **Deviations in T19.b, as built** (2026-10-13).
  - _Files._ The protocol's `surface.rs` (`SurfaceRevisionDto`), `scene.rs` (the field),
    `envelope.rs` and `lib.rs`, the bindings `just gen-protocol` writes and
    `packages/protocol/src/index.ts`'s re-export. The server's `requests/surface.rs` (the handler)
    and `requests/mod.rs`, `convert/surface.rs` and `convert.rs`, `knowledge/surveys.rs`,
    `surface/service.rs`, `scene/topic.rs`, `subscriptions.rs`, and in passing `bulk.rs`,
    `testing.rs`, `scene/core.rs` and `convert/scene_fixture.rs`.
  - _The answer, for R10.T7._
    - `surface_field` answers `Ready { header, revision, coverage, bulk }` after the payload's
      binary frames, or `NotModelled { section }` with no bulk.
    - `revision` is the body's coverage revision the payload reflects: its count of passes, 0
      before the first. A body with no pass is served at revision 0 as block 0 alone, the header.
    - With no `have_revision`, or with 0, the payload is the whole surveyed field: every
      surveyed cell with its code and its margin of `SYNTHESIS_MARGIN_CELLS` at code 0, the
      climate over them and every crater reaching them (`encode_payload`'s gate).
    - With `have_revision` n, the payload is the delta since revision n: the cells of the cover
      and its margin that revision n's cover and its margin did not hold, or held with another
      code. So a cell surveyed again more finely, or a margin cell since surveyed, is sent again
      with its new code. Block 0 always carries the header; asking with the latest revision gives
      block 0 alone.
    - `coverage` is the payload's own cells with their codes, margin cells at 0, the union of
      its blocks' covers. For a delta, these are the delta's cells alone: a client replaces its
      copies of them and keeps the rest. It is not the whole cover at `revision`.
    - A `have_revision` above the body's revision is `bad_request` naming `have_revision`; the
      client asks again without it.
    - The same request at the same revision gives the same bytes, from the field kept or
      computed again.
  - _The order._ The universe; the body ID's two parts; the body's system from `bodies_of`
    (`unknown_system` naming `body`); `SurfaceService::field_level` on an interactive job. Its
    `NotModelled`, and an `Inputs(NotModelled(section))` error, answer `NotModelled` with no bulk
    job, so no bulk slot is taken (T17's deferred finding). The error names `figure` for a body
    whose figure is not modelled either, which `survey_pass` refuses `bad_request` naming `body`
    (T19.a); that case has no test, since no source gives such a record. Other refusals go through `convert::surface_refusal`. Then the survey log
    and the body's coverage, so that a bad `have_revision` is refused before a pass is run; then
    `SurfaceService::field`; then the gate, on one bulk job. A body whose passes index another
    level than its field is `internal`, logged; only a server fault makes it.
  - _The revisions' history._ `Surveys` keeps each body's passes, a code and the cells each, in
    their order beside its fold. `SurveyLog::history(body)` gives a `CoverageHistory` snapshot:
    `latest()`, and `at(revision)`, which folds the first n passes again (a byte a cell, on the
    bulk job). `Coverage::to_cover` gives the `Cover` of its runs, without the margin, for
    `encode_payload`.
  - _The hook is in `SurveyLog::record`, not in the handler._ Every pass recorded raises its body's
    revision in the log's `watch` channel of `RaisedRevisions`, under the file's lock, so a later
    caller of `record` (the sensors plan) notifies too. The surface service makes each universe's
    channel on whichever comes first, a scene subscription (`watch_surveys`) or the log's opening
    (`SurveyLog::open_announcing`). So a scene subscribed before the universe's first pass is told
    of it. The channel holds only the revisions raised since the log was opened, never those
    loaded from the file.
  - _The scene topic._ A subscription takes the revisions raised when it opens as already told,
    since its state carries none (R03's Design note 4). It then pushes each body raised past what
    it told, with its latest revision, as `surface_revisions`, stating the clock then. Every
    surveyed body of the universe is named, not only the scene system's bodies; the client keeps
    those whose fields it holds.
  - _The merge._ `ScenePush::merge` merges `surface_revisions` by body, keeping the larger
    revision, so the order of the pushes does not matter. Bodies new to the push are appended in
    the order first sent. The revisions are kept across an arrival, which replaces only the
    scene's own state. `SurfaceRevisionDto` sits in the protocol's `surface.rs`, beside the other
    surface types; `PROTOCOL_VERSION` stays 2.
  - _Clippy._ The notification's 24 more bytes put `subscriptions::Step` past
    `large_enum_variant`, so `Step::Serialise` holds a `Box<Unsent>`, one allocation a
    notification sent. The tests' `testing::Received` takes an `#[expect]` instead, since every
    socket test matches it by value.
  - _Public items beyond Provides._ In `knowledge::surveys`: `CoverageHistory` (`latest`, `at`),
    `Coverage::to_cover` and `SurveyLog::history`, and crate-private `RaisedRevisions` (`get`,
    `raised_since`) and `SurveyLog::open_announcing`. In the server, crate-private:
    `SurfaceService::watch_surveys`; in `convert`, `FieldRequest`, `surface_header`,
    `climate_model`, `precipitation_source`, `palette_role`, `record_section`, `cover_dto` and
    `surface_revision`; and the tests' `BulkPayload::bytes`.
  - _Tests_ (`requests::surface`, `subscriptions`, `knowledge::surveys`, `convert::surface`, and
    the protocol's `scene` and `surface`):
    - a second pass's delta carries only the new and re-surveyed cells and the margin newly
      reached, cell for cell against `Cover::with_margin`; the whole field holds both passes';
    - the same request twice gives identical bytes, whole and as deltas, from one pass;
    - a body with no pass is block 0 alone; `have_revision` above the body's is refused;
    - a body not modelled answers `not_modelled`, with no bulk job;
    - the refusals naming `body`;
    - a `survey_pass` raises the body's revision on a scene subscribed before the first pass: the
      first rise named alone, then each body's latest revision;
    - two pushes merged while a reader is slow carry each body's latest revision;
    - a merge keeps each body's larger revision whatever the order, and across an arrival;
    - a resubscribed client is told nothing while away, its `have_revision` request over the
      socket returns the cells surveyed meanwhile, and the rise made while it was away is not
      replayed;
    - each revision folded again equals a log of only those passes; a coverage's cover; recorded
      passes raise their revisions and loaded ones none; a raise keeps the larger revision;
    - a field request's revision checked against the body's; the header's members, each against
      its getter; the classes' wire names;
    - `SurfaceRevisionDto`'s wire form, and a notification without the field omitting it.
    - In every answer taken from the handler directly, the tests check that the answer's header
      is block 0's and that `coverage` is the union of the blocks' covers; the answers over the
      socket check their bytes against the manifest.
  - _Acceptance._ `cargo test -p hyperion-server surface`, then
    `cargo test -p hyperion-server subscriptions`. The two filters miss tests this task added or
    edited, which T19.b ran by name (`--lib -- surveys requests:: scene:: convert::`, and the
    protocol's `--lib`), and which the gate, `just smart-ci`, ran with the rest of both crates:
    - `knowledge::surveys`'s four new tests, whose names say `survey`;
    - the protocol's `surface::tests::surface_revision_wire_form`,
      `scene::tests::a_notification_without_surface_revisions_omits_them` and
      `a_notification_carries_what_changed`, and `envelope::tests::notification_wire_form`;
    - the server's `requests::tests::kinds_without_a_handler_are_answered_unsupported` (five kinds
      now, not six), `scene::core::tests::only_an_arrival_is_a_large_notification`,
      `scene::topic::tests::a_body_whose_valid_until_passes_at_100_000x_is_pushed` and
      `convert::scene_fixture`'s sizes;
    - `just gen-protocol-check` for the bindings.
  - _The first send (ruled by "main", 2026-10-13)._ T19.b is the first commit that serves a
    payload, though only to tests: under `RecordInputs` every generated body answers
    `not_modelled`, and no client asks yet (R10.T7). Main's ruling, now in the Generator version
    section: the first send is the first payload served for a generated body (once P14.T24 and
    P14.T48 inhabit the surface section), or a client's first persisting of a payload, whichever
    comes first. So R11's liquid palette member (R11.T8), and any other layout change before then,
    still re-blesses `tests/golden/wire/` with no bump.
- **Deferred findings (2026-10-13 rule)** (R09.T19.b). Recorded and deferred: each leaves the
  algorithm and every contract as they are.
  - _The margin is flooded twice._ `encode_payload` adds the margin to the cover and to `since`,
    and the handler's `sent` repeats both floods to name the payload's cells in `coverage`: four
    floods for a delta where two would do, and two for a whole answer where one would do, each
    a 6 · 4ᴸ-byte map, on the bulk job. `sent` also copies the encoder's selection rule from the
    surface crate, which only the tests' check of the blocks' covers would catch drifting from.
    Lean: a surface-crate function that gives the payload's cover with its bytes, once T16's
    benches measure the cost.
  - _A level job for each request._ Every `surface_field` runs `field_level`'s interactive job,
    even when the field is kept, so each request after a revision rise reads the record and asks
    the source once more. `for_body`'s cost is unknown until P14.T54.a. Lean: skip the job when
    the field is held, since a held field's body is modelled.
  - _The cover's JSON is bounded only by the field._ `coverage` holds a range for each run of the
    payload's cells. Each pass carries at most 256 runs, but passes accumulate, so a fragmented
    whole answer could hold up to one range a cell: 393,216 at level 8, about 40 bytes each in
    JSON, some 15 MB at the worst, near `OUTBOUND_BYTES`' 16 MiB, in a terminal response the pool
    serialises. The blocks already carry the same cover. Lean: measure with R10.T7's client. If it
    grows, the main thread could take the cover from the blocks' own covers, or the answer could
    send a summary.
  - _A body's passes are kept in memory._ The history holds every pass's runs for the process's
    life, about as much as the log file. Lean: checkpoint covers every k revisions with P12.T8's
    holder, if a sensors plan records passes often.
  - _Every surveyed body is named._ A scene subscription is told of every surveyed body of its
    universe, not only those its client keeps. Lean: fine for the stand-in; sessions may scope it
    to the ship's.
  - _The raised revisions are never dropped._ A universe's channel and its map stay for the
    server's life, a few bytes for each body surveyed since it started. Lean: drop them with
    P12.T8's holder when a universe closes.
- **Deviations in T19.c, as built** (2026-10-13).
  - _Files._ The new `crates/hyperion-server/tests/surface_knowledge.rs` and its helpers in
    `tests/common/surface.rs`. No server code: the test found no server bug.
  - _The helpers_ (Provides' test helpers):
    - `field_via_wire(client, request) -> FieldViaWire` reads a `ready` answer as R03's transport
      sends it: the bulk frames through `TestClient::next_binary()`, each checked for its request,
      index and count, then the response through `next_text()`, whose manifest must state the frames
      and the payload's length. `FieldViaWire` holds the header, the revision, the coverage, the
      answer's text as it arrived, the frames' headers and the payload rejoined. It is for a
      connection with no subscription, so a notification among the frames fails it.
    - `decode_all(payload)` is `decode_payload`'s blocks.
    - Beyond Provides: `partial_field(blocks)`, a `PartialField::new` from block 0's header with
      every block inserted, and `insert_all(field, blocks)` for a delta.
  - _Two cases, on the reference worlds through `ServerConfigBuilder::surface_inputs`, of
    `surface_system()`'s solid body:_
    - The Ceres-like world (level 5, the quickest) carries every check. The first pass is orbital,
      at 120 m, over 105 cells: face 0's 8 × 8 corner block, at a cube corner across two face edges,
      whose margin reaches faces 4 and 5; and 41 cells in index order from the sixth of the 8 × 8
      block at face 3's centre, a ragged shape. It also takes runs of 41 cells about the field's
      widest crater and its largest drainage, when the field has them (below). The second pass is
      close range, at 10 m, over face 0's cells 48–127: a quarter of the corner block again, more
      finely, and the block beside it, which the first margin reached. The payloads are 25,451 B
      (600 cells held), a delta of 8,838 B (200 cells) and 31,132 B, each one block in one frame.
    - So the Mars-like world (level 7) adds one orbital pass over faces 1 and 2 whole. Its payload
      is 1,386,466 B (36,558 cells held), 2 blocks in 6 frames, checked block by block as the first
      is, without heights.
  - _The checks, as built._
    - _No block carries a cell outside the region and its margin._ Every cell of every block is one
      of the passes' cells at its code, or a cell of their margin (`Cover::with_margin` of
      `SYNTHESIS_MARGIN_CELLS`) at 0. The blocks' covers together are exactly that whole, or for the
      delta exactly T19.b's cells, and the answer's `coverage` names them. Every record is the
      server's. Each block's climate records are its cells' parents', once each, in order. Each
      crater reaches a cell of its block and is one of the field's. The answer's header names block
      0's body and level. A mutation that expects a margin of 4 fails the first check.
    - _Margin cells are marked._ Over every cell of the level, the client's
      `PartialField::resolution` is the expected code, or `None` for a cell not held, and
      `is_surveyed` is true exactly for the passes' cells. A held cell's record, climate and craters
      reaching are the server's; a cell not held has no record, no climate and no crater.
    - _The cover's codes match the passes._ The expected cover takes each cell's finest code over
      the passes. It is checked on the whole answer after each pass and on the delta since revision
      1, on the client's field merged from the first whole and the delta, and on a field built
      afresh from the second whole.
    - _The client's heights are the server's._ `Synthesiser` runs over the server's field and over
      the client's `PartialField` with one detail seed. Heights and gradients agree to the bit at
      band levels 0, L, L + 4 and `MAX_LEVEL` (the band limit's set). The points are a 3 × 3 lattice
      over each surveyed cell (its corners, its edges' midpoints and its centre) and every fourth
      vertex along each edge a surveyed cell shares with a cell not surveyed, face edges and the
      cube corner included: 2,033 points (1,088 on the edge) after the first pass. After the second,
      only its own cells are sampled, 1,196 points (476 on the edge), since a cell surveyed before
      reads the same records from a field that has only grown. A point in a cell the client does not
      hold is refused `NotSurveyed`, while the server answers it.
    - _No `surface_seed`._ Both connections' text frames are parsed as JSON as they arrive, and none
      may hold a key named `surface_seed` at any depth, or the body's surface seed's value in
      sixteen hexadecimal digits or in decimal. That covers `welcome`, `create_universe`,
      `scene_ship`, the scene's state at `full` with the ship in the body's system, `body_detail`
      and `system_bodies` at `full`, the survey answers, the field answers, and every scene
      notification, the `surface_revisions` among them, which a task of its own reads. No payload
      may hold the seed's eight bytes in either order. `body_detail`'s answer is checked to have its
      `hooks` section, the place a seed would travel. The socket test covers the kinds a session
      sends; no wire type in the protocol crate or its bindings has a member of that name.
  - _Deviations._
    - _The server's heights_ are the synthesis over the test's own `coarse_pass` of the body's
      surface seed on the same inputs, since an integration test cannot reach the running server's
      field. `tests/surface_service.rs` shows that field is the served one, bit for bit, and every
      record the client holds is checked against it.
    - _The detail seed_ is a stand-in, the same on both sides, since R09.T1.b (in plan 14's 21 → 22
      batch) has not put the body's on the wire. The check is that the two sides agree from the
      field and one seed.
    - _"Every surveyed point"_ is the lattice and the region's edge above, since each point is dear
      (deferred, below).
    - _The header's body_ is compared with block 0's, not the body asked for: the reference worlds
      name `BodyRef(0, 1..4)` (T17's deferred finding, still open).
    - _The socket's harness_ is R03's `TestServer` and `TestClient`, with the scene's watcher on a
      second connection read by its own task. The runtime is `multi_thread` with two workers, so
      the server's tasks and the watcher's run while the test's body checks. The coarse pass and the
      heights each run on a thread of their own, waited for within the harness's patience, rather
      than in `spawn_blocking`, whose task a runtime's drop would wait for after the wait had timed
      out.
    - _The records are alike today._ All six of the coarse pass's steps are still empty
      (`steps::*::run`), so every cell of a reference field holds one synthesis record and one
      climate record, the base elevation is the datum everywhere and the structural octaves'
      amplitude is zero. The record checks therefore bind which cells are held and at what codes,
      not which record sits at which cell, and the heights compared vary by the small craters
      alone, though every cell the synthesis reads must still be held (deferred, below).
    - _R03.T15's 15 MiB (61-chunk) check_ in the real Electron renderer stays pending, as the task
      says. It needs a body whose field the server binary serves (P14.T24, P14.T48) and R10's page
      that asks for it ("The 61-chunk transfer check", above). The largest payload here is 1.39 MB
      in 6 frames.
  - _Acceptance._ `cargo test -p hyperion-server --test surface_knowledge`: 2 tests, 13.6 s together
    at a load near 10 (lock-held settings, 4 test threads, a 400% quota). The orchestrator's
    `just ci` is the lane's gate, `just smart-ci`, which ran the server's 70 tests that reach the
    change.
- **Deferred findings (2026-10-13 rule)** (R09.T19.c). Recorded and deferred: each leaves the
  algorithm and every contract as they are.
  - _The reference fields are uniform until the steps land._ With T11–T15 empty, the records are
    one record repeated, no crater and no drainage (the deviation above), so T19.c holds the
    Knowledge bound and the wire's agreement only for which cells travel, their codes and the
    synthesis's reads, and Verification's "Agreement" item (the server's heights equal a client's
    from the wire) is met only for the small craters. The test already takes runs about the widest
    crater and the largest drainage once the field has them; channels need a wet world, the
    Mars-like or the Earth-like. Lean: re-run T19.c when T11 or T12.a–c first vary the records cell
    by cell, again with T12.d and T14, and then add the Mars-like world to the heights check, in the
    slow tier if its cost holds.
  - _A cold point query is dear_, as T9's deferred finding already has it (the small craters'
    uncached draw, 0.1–2.8 ms a point on V8; 1,140 µs on the Moon-like world at level 10). Seen
    natively here in a one-off measurement, not in the test as built: in the test profile
    (`opt-level = 2` with debug assertions), at a load near 13, the first query at a new point at
    band level 16 took about 1 ms on the Ceres-like world and about 4 ms on the Mars-like one, and
    the same point at level 24 then about 70 µs on the Ceres-like world. Only T8's native
    `craters-uncached` bench is still pending. It is why the test's points are few. Lean: T9's.
  - _The detail seed's source._ Once R09.T1.b puts the body's detail seed in `body_detail`'s hooks
    section, the test should read the client's seed from there. Lean: with T1.b, which then also
    checks that the hooks section carries `detail_seed` and no `surface_seed`. _T1.b, as built
    (2026-10-14):_ the wire's checks are in `tests/system_bodies.rs`, and this test's switch is left
    for its follow-up ("Deviations in T1.b, as built").
- **Deviations in T18, as built** (2026-10-09).
  - _The shared log._ `knowledge/jsonl.rs` (private) holds what `persist.rs` held privately:
    `KnowledgeLog` (now carrying its header's format), `push_line`, `sync_directory`, `save_io`
    and `TimeLine`, with the loader `jsonl::load(dir, name, apply)`, whose `apply` takes each
    parsed line and names the bad field when it refuses one, and
    `refuse_later_formats(dir, name)`. The parameter is a `LogName` (stem and format,
    `<stem>.v<format>.jsonl`; format 0 panics), so each log refuses only later files of its own
    stem: a later `contacts` file does not refuse the surveys, nor the reverse. `persist.rs` keeps
    its lines, reasons and torn-line warning; `CONTACTS_FILE` moved into its tests, which gain
    `knowledge_contacts_file_keeps_its_name`, and `KNOWLEDGE_FORMAT` is documented as the contacts
    file's format.
  - _One behaviour change, for crash safety._ A log's first open in a process now syncs
    `knowledge/` and the universe's directory every time, not only when that log made them or its
    file: with two logs in `knowledge/`, one could find the directory made by the other, whose sync
    had not finished, or left unsynced by a failed sync, and report a line saved before the names
    leading to it were durable (P12.T7's single log had the same gap on a retry after a failed
    sync). It costs at most two directory syncs a log a process; on macOS each is an
    `F_FULLFSYNC`. Test: `knowledge_log_first_append_syncs_both_directories_it_did_not_make`.
  - _No `LoadSurveysError`._ `SurveyLog::open` returns `LoadKnowledgeError`, and `record` wraps
    `SaveKnowledgeError` in `RecordSurveyError::Save`: the loader and the appends are shared, so
    the failures are the same, and one type lets a holder open both logs with one `?`. Both stay
    in `persist.rs`; `MalformedLine`'s and `Poisoned`'s docs now speak of either log. No later plan
    names `LoadSurveysError`.
  - _Public items beyond Provides._ `SurveyPassParts` (public fields, the crate's Parts pattern),
    which `SurveyPass::new` checks; `BuildSurveyPassError` (`Span`, `Resolution`, `NoCells`,
    `EmptyRun`, `Unsorted`, `OutsideField`); the `SurveyPass` getters, `code` and `cell_count`
    among them; `RecordSurveyError` (`Level`, `Revisions`, `Save`); `CoverageRevision`
    (`NonZeroU32`, `FIRST`, `new`, `get`, `Display`); `Coverage`'s `level`, `revision`, `code`,
    `as_bytes` and `surveyed`, with no public constructor, so that `record` stays the only way
    coverage grows (its `Debug` shows the surveyed count, not the bytes).
  - _The line._ Format 1 is `body` (the sim's text form), `from` and `to` (`{seconds, nanos}`),
    `source` (`orbital`, `close_range`, `landed`), `resolution_m`, `level` and `cells`
    (`[[start, end], …]`, half-open, sorted, non-overlapping; runs may meet), pinned by
    `survey_lines_of_format_1_are_pinned`. The code is derived from the metres on load. The
    field's level joins Design note 16's fields so that a line's runs are checked against its cell
    count without the body's radius, which the log cannot see, and so that a body's passes share
    one level (`RecordSurveyError::Level` on record, "does not fit the passes before it" on load).
    A bad span, a resolution with no code (not finite, not positive, or coarser than about 36,000
    km) or bad runs make a line `MalformedLine` naming `to`, `resolution_m` or `cells`.
  - _Revisions._ `record` returns the body's `CoverageRevision`, its count of passes, and
    `coverage(body)` an `Arc<Coverage>` snapshot whose codes and revision agree (copied on write
    while a reader holds one); a body with no pass gives `None`, revision 0 for T19.b. The file's
    lock is held over a whole pass, so revision n is always the fold of the body's first n lines,
    and reopening restores it.
  - _Dependency._ The server depends on `hyperion-surface` (for `CoarseLevel` and
    `ResolutionCode`, and `coarse_level`, `cell_index` and `cube` in tests), since the sim does not
    re-export the surface crate; T17 uses the same edge.
  - _For T19.a and T19.b._ A universe's survey file has one writer, as P12.T7's is: the
    `SurfaceService` opens each universe's `SurveyLog` once, even when two first requests race,
    clones it, and hands it to P12.T8's holder once that exists. The log keeps only each body's
    fold, so T19.b's `since` cover for `have_revision` needs the body's first n passes: T19.b adds
    keeping them (or a cover per revision) to `Surveys`, and the `Coverage`-to-`Cover` conversion
    for `encode_payload`.
  - _Tests beyond the four._ A bad span, resolution or cells refused; a pass at another level; a
    body past the last revision; passes after a panic refused as poisoned while the coverage still
    reads; unknown and later formats (a later contacts file refuses nothing); the torn last line
    written over; malformed lines naming their field; a universe with no save, where nothing is
    written; concurrent passes reaching the file in the coverage's order; format 1's lines pinned.
- **Composition (decision-composition, 2026-10-09).** A body's substances are plan 14's
  registry keys, carried in the header's palette with their properties resolved by the server,
  and one byte per cell names its ice and liquid. `Crust` stays structural, its lithology the
  palette's, which closes T2's "closed set, for the composition audit". `BoundaryKind`,
  `Morphology`, `ClimateModelKind` and `PrecipitationSource` are physics classes and stay closed.
  Recorded for later:
  - glacial, aeolian, sublimation and thermal (lava) erosion (T14);
  - ice-shell, heat-pipe and episodic morphologies (T11–T12; their regimes are on plan 14's wire
    already);
  - frost by month (plan 14's P14.T55.e).
- **Deviations in T4, as built** (2026-10-09).
  - _Two commits._ A `refactor` commit (`HeightSample` to `height`, and the margin) went first,
    so that T3's lane could merge `field::SYNTHESIS_MARGIN_CELLS` (5, before `FieldView`); the
    interpolant is the task's `feat` commit. The margin counts king moves (edge and corner
    neighbours at the field's level, across face edges by the cube's neighbour rule), as its doc
    comment says and T3's `Cover::with_margin` counts; the read-set test checks each read against
    the margin `with_margin` sends.
  - _Files and public items beyond Provides._ `height.rs` holds `HeightSample`, which `test_planet`
    re-exports and `patch` imports. `synth/interp.rs` holds `CellValues` (one value a cell at any
    level 1 to 24), `Interpolated` (plain data: the value and its gradient on the unit sphere, per
    radian), `ReadCellError`, `interpolate`, `base_elevation(field, dir)` (a `FieldView`'s
    elevations, no seed) and `cell_at`, the categorical read: the cell that contains the direction,
    by `PatchKey::containing`, taken as the brainstorm's "nearest cell". `Synthesiser` gains
    `field`, `seed` and `cell_at`, its `Clone` and `Copy` written by hand so that it is `Copy` over
    any view; `QueryHeightError` gains `From<ReadCellError>`; `SynthCache::new` is empty until T5.
    The cube gains `pub(crate) uv_on_face`, a direction's gnomonic (u, v) on any face, which
    `xyz_to_face_uv` now calls with the same expressions, so R05's goldens are unchanged.
  - _Tests beyond the list_ (15 under `synth::interp`, and `tests/panics.rs`'s
    `the_interpolant_refuses_level_0`): C² across face edges; the base elevation the same at every
    level; a cell not held answered `NotSurveyed` and the margin held answered as the whole field;
    the categorical read; the low-pass's 4/9, 1/9 at a cell centre; the error's bounds and text;
    the interpolant through `CellValues` at levels 1, 2, 4 and 12 (edges, range, constant); order
    independence over a shared and a fresh `SynthCache`, which T5 and T6.b must keep passing once
    they fill it; `Synthesiser` `Copy` over any view.
  - _`QueryHeightError`_ has `NotSurveyed(PatchKey)` alone. The base elevation is finite by
    construction and asserted so, as R05's heights are, so `NonFinite` waits for T8.
  - _Design note 13 as read._ "A one-cell band": each face's weight is 1 on the face and falls
    to 0 one cell beyond each edge, β(s) = S(2ᴸ s + 1) with S the quintic smoothstep. That is what
    needs the ghosts three deep, and it makes the blend two cells wide, half and half on the edge.
    A ghost's bilinear stencil is clamped to its owning face's cells. Control values are taken
    relative to the query's own cell, in millimetres, so a constant field returns its cell's
    elevation bit for bit with a gradient of +0. The body-fixed gradient is that of
    H(P) = h(M⁻¹P ÷ |M⁻¹P|). Only its tangential part at the spheroid point is the surface
    gradient, which R05's normals read. On an oblate body it also has a part along the normal, up
    to f ÷ (1 − f) of the slope: 0.3% on an Earth, 8% on a Ceres. So T15, R10 and any other reader
    that wants a slope projects the gradient into the tangent plane first, as
    `patch::normals::surface_normal` does.
  - _Measured._ The field is C² across face edges as well, and only C¹ on the three great circles
    x = 0, y = 0 and z = 0 (the faces' twelve centre lines). There the second difference jumps by
    −3φ′, because the warp's s″ goes from 9/8 to −9/8 while s′ is 3/4. The gradient matches a 1 m
    central difference to 10⁻⁶ relative wherever the slope is above 10⁻⁵, which is over 90% of the
    points. Below that, the height's rounding, up to about 2 × 10⁻¹¹ m from 16 to 48 rounded terms
    in millimetres, reaches 10⁻⁶ of the slope over the difference's 2 m.
  - _The margin has no slack at the ends of the edges._ The interpolant's farthest read is
    exactly 5 king moves, near the ends of the face edges at levels 6 to 8. It is 4 in the middle
    of an edge and 4 everywhere at level 5. Near the ends, a ghost three cells past an edge lies up
    to three cells along it from the query's row, so Design note 15's "four past an edge" holds
    only mid-edge. The sweep covered every face edge at 0.001 of its length and 0.1 cell across,
    out to 2.6 cells either side, about 6 × 10⁵ points a level, and never found 6. The sweep was
    run once and is not committed. The committed test runs about 2,750 points a level on flat
    fields at levels 5 to 8, and asserts that the farthest read is exactly 5. So T8's
    whole-function read-set test, and any wider band or deeper ghost, has no slack there.
  - _The low-pass._ The spline does not pass through the cells: the centre of a cell more than a
    cell from a face edge takes 4/9 of its own value, 1/9 of each edge neighbour's and 1/36 of
    each corner neighbour's. Nearer an edge, the neighbouring face's spline is blended in. So the
    reconstructed field's σ_h and relief are below the cells', which T12.e's rescaling on the
    reconstructed field accounts for, and T14.b's upsample through it smooths. The synthetic
    worlds' headers keep the cells' σ_h and relief (T2's).
  - _For T8._ Two `synth::interp` tests hold only while `height_at` is the base elevation alone:
    `the_base_elevation_is_the_same_at_every_level`, which T8's full `height_at` retires, and
    `a_cell_not_held_is_not_surveyed`'s check of the missing cell against the interpolant's read
    set, which moves into T8's whole-function test. The test views `Recorder` (every cell asked)
    and `Held` (a subset), and `king_distances` (the margin's count), are private to that test
    module; T8 may lift them into `testing`, or test against T3's `Cover::with_margin`.
  - _For T12.e and T14.b._ In the middle of the pass there is no `FieldView`, so both read the
    interpolant through `interpolate` over a `CellValues` of their own working values. Its tests
    cover levels 1, 2, 4, 5 to 8 and 12, and T14.b's multigrid reads levels 4 to 8.
  - _For T9._ The determinism review asks T9's `height_golden.rs` to pin `interp::base_elevation`
    apart from the full `height_at`, at band, edge and corner points and once through `interpolate`
    at a low level. Then a change to T5–T8 that leaves the interpolant alone shows as such.
  - _For R05's and R10's re-validations._ R05's Provides comment (its line 138) and its
    review-fixes record (line 3812, "the path R09.T4 moves it from") say that `LatticeCache` moves
    too, and R10.T6.b says R09.T4 moved `HeightSample` "and `LatticeCache`" to `height`. Only
    `HeightSample` moved; `LatticeCache` stays in `noise`.
  - _For the brainstorm's revision._ The per-query evaluation's "Base elevation from the
    interpolated coarse elevation" reads as an interpolant through the cells. Design note 13's
    spline is approximating, and the surface does not pass through the server's cells, so this
    goes with Design note 13's other departures in the roadmap's brainstorm corrections (asked of
    "main"). _Applied 2026-10-09:_ lane docs-consistency amended the brainstorm's per-query step 2
    to "from the coarse elevation through an approximating C² spline", with σ_h matched on the
    surface it reconstructs (`signoff-brainstorm.md`, "Later amendments").
- **Deviations in T5, as built** (2026-10-09).
  - _The noise key._ `noise::NoiseKey` (`TestPlanet(Seed)`, `Relief(DetailSeed)`) replaces the
    `Seed` an `Octave` held: `Octave::new(index, spacing, rotation, offset, key)`, with the getters
    `spacing` (was `spacing_m`) and `key`. The key fixes the unit of every length an octave is
    given (metres for the test planet, the body's mean radius for the relief), which each variant's
    documentation states; the generic lengths (`spacing`, `gradient_noise`'s `point`,
    `LatticeCache::cover`'s `centre` and `radius`) therefore carry no unit in their names, the one
    exception to the rule that a quantity names its unit (the Rust review's finding, kept so).
    `LatticeCache::{take_octaves, restore_octaves}` take the key, its boxes compare keys, and
    `LatticeCache::new` is `const`. The crate-private `NoiseKey::word` reads every corner word and
    the relief's offsets, naming `SURFACE_RELIEF` and `surface_item` in the one `stream` call (T1.a's
    key-form test); the test planet's offsets are still read by `test_planet::octaves::offset` on
    its own stream. R05's rotation formula is `noise::rotation` (crate-private, k 0–31), which
    `test_planet::octaves::rotation` calls. `RIDGE_MEAN` and `RIDGE_RMS` move to `noise`, which
    `test_planet` re-exports; `RIDGE_EPSILON` moves too, a private import there that `bound.rs`
    reads. Every R05 golden is unchanged and `TEST_PLANET_VERSION` stays 2. `test_planet` bench
    (3700X, schedutil, other lanes running, load 9–25, provisional): cached 11.86 and 11.90 ms
    before, 11.89 ms after (2.8 µs a point); uncached 19.02 and 19.28 ms before, 19.67 ms after (4.5
    to 4.7 µs a point, +2–3%, the key's match on every hashed corner).
  - _Files and public items._ `synth/relief.rs` (tests in `synth/relief/tests.rs`): `Relief`
    (`new(header, seed)`, `at(field, dir, through, cache)`, `cover`, `seed`, `first_octave`,
    `finest_octave`, `octave_rms`; `Clone`, not `Copy`, at 280 bytes, so T8's `Synthesiser`, which
    is `Copy`, borrows one or computes it), `ReliefStyle` (`of(cell)`, `amplitude`, `belt`,
    `shear`, `REFERENCE`), `nyquist_degree`, `first_octave`, `finest_octave`, `octave_spacing`,
    `octave_rms`, `octave_bound`, `local_variance`, `WARP_SCALE`, `RIDGE_WEIGHT_RMS` and the
    amplitude constants. `BandSpectrum` gains `variance_from_degree`, `band_variance` and
    `level_variance` (an `impl` in `relief.rs`). `synth.rs` gains `UnresolvedVariance` (`relief`,
    `total`) and `unresolved_variance`, re-exports `Relief` and `local_variance`, and its
    `SynthCache` holds the `LatticeCache` (`lattice_mut`, crate-private). `height_at` is unchanged
    (T8 joins the bands), so T4's order-independence test passes as it was.
  - _The octaves._ One per level m, from L + 1 to the deepest whose lattice spacing is at least
    the 2 m band limit (22 on an Earth, 21 a Mars, 20 the Moon, 18 Ceres). The spacing is 1.5 mean
    cells, √(3π ÷ 2) R ÷ 2^m, at the spheroid point in radii, so the octave table is a function of
    the seed alone. The 1.5 is measured: the noise's planar spectrum (4 planes of 256², Hann
    window; run once, not committed) has its median at 1.92 λ, quartiles 2.9 λ and 1.5 λ, 98% above
    λ, which puts the median at the geometric centre of the octave's band. Octave m carries the
    integer degrees from `nyquist_degree(m − 1)` to `nyquist_degree(m)`, ⌈√(3π ÷ 2) 2^m⌉ (556 at
    level 8, Design note 7's Nyquist degree), so the octaves partition the spectrum. The sets per
    band level stay T8's; under R05's rule (λ at least four of the level's largest vertex
    spacings) band level n reads octaves to n + 4.
  - _The styles._ The plain-to-ridged mix is a rotation, t = cos θ n̂ + sin θ r̃ with θ = (π ÷ 2) b,
    zero mean and unit variance in every mix (n̂ and the even ridged term are uncorrelated: reflecting
    a point through its lattice cell's centre negates the noise with the corners' gradients
    exchanged, which are drawn alike, so the noise is symmetric about zero). r̃ is the ridged multifractal (the brainstorm's,
    built after the plan-conformance review): R05's ridged transform with its pinned mean removed,
    weighted by the octave above, w = (max(r, 0) ÷ (1 − ε))² of its raw ridge (Musgrave's weight,
    from the octave immediately above only, not his cascade), divided by the weight's pinned RMS
    `RIDGE_WEIGHT_RMS` = 0.7199 (10⁷ points, `the_ridge_weight_rms_is_measured`, slow); the first
    octave is unweighted. The lattices being independent, it keeps zero mean, unit variance and no
    correlation with coarser octaves. The warp displaces octave m by the three octaves above it
    along the body axes, times s and `WARP_SCALE` λ_m, from the fourth octave on. The amplitude is
    a factor a. The three are interpolated with T4's spline from per-cell values, so the relief
    reads exactly the base elevation's cells (tested) and adds nothing to the margin. The factors
    carry their sources, the measurements of T5's science check (plane-detrended RMS in 35 km
    windows of Earth2014's 1′ BED, GMRT multibeam at 61 m, MOLA at 32 ppd, LOLA at 16 ppd; its
    scripts are not committed): belts 2.5, continental interiors 0.12, deformation zones 1, young
    oceanic hills 1.5, abyssal plains 0.1, a stagnant lid 1, its provinces 0.25. Widths: collision
    belts full to 100 km and gone by 250, arcs full to 200 km behind the boundary and gone by 350,
    the continental deformation zone 150 to 500, hills to plain 500 to 1,500 (the plan's own),
    shear 50 to 100 km on continental crust and 20 to 50 on oceanic (the plan's own). Ruled
    (`decision-r09-t5.md` item 5): the oceanic amplitude reads the cell's `seafloor` byte (hill
    relief and ponded sediment), which T11 and T12.a derive from the cell's own ridge. The factors
    are a table over T2's `Crust` and `BoundaryKind`, matched exhaustively, which
    decision-composition keeps closed. Ruled (`decision-r09-t5.md` item 7): the ridged term carries
    2.64× the plain term's gradient variance, so a full belt measured 52° at 1 m. The ridged mix is
    confined above the break, and a belt has its own shape, so a full belt gives 37.6°, 35.9° and
    26.0° at 1 m, 100 m and 1 km, inside SRTM's belt cores.
  - _`local_variance` is the reference style's._ The styles move variance about the body, so the
    realised share is the mean a² times `local_variance`: 0.74 on the Earth-like world, 0.93
    Mars-like, 0.95 Moon-like, 1 Ceres-like. The plan's 5% test is therefore against
    `local_variance` times the sampled mean of the interpolated a² at the same points (within 0.9%
    on all four), and Flat and OneCrater have no relief at all. Ruled (`decision-r09-t5.md` item
    1): nothing normalises it. The styles are absolute, V₁ is the reference style's law, and T12.e
    books ⟨a²⟩ `local_variance` in σ_h's budget, with ⟨a²⟩ from its own quadrature of the
    interpolated amplitude. The octaves stop at the band limit, leaving out 2^−(finest − L)(β − 1)
    of `local_variance`: under 10⁻³ at β = 1.9 on the synthetic worlds, 1.8 × 10⁻³ on a Ceres at
    1.7.
  - _Tests as built._ The mean: 2 × 10⁵ area-uniform points on an Earth-sized body (the plan's 10⁵
    raised, so that the 1% is 4.5 standard errors and the 56 checks together stay under α =
    10⁻³), octaves 9 to 22, plain and ridged, warped and not, each mean under 0.01 of unit RMS and
    each RMS within 3%. The variance: 6 × 10⁴ points a world (raised from a first 2 × 10⁴), and the
    test asserts that its 5% is at least 3.29 standard errors of the samples' difference (α =
    10⁻³). The bound: `T_MAX` = √((B ÷ σ_noise)² + (r̂(B) ÷ W)²) = 8.770, reached at n = −B with the
    weight 1; `octave_bound` is `MAX_AMPLITUDE` σ_m `T_MAX`, held on every synthetic world. The
    warp: an instrumented lattice records that the relief through octave n reads octaves L + 1 to n
    once each, in order, and the terms through n are those of a deeper query, bit for bit. Beyond
    the list: `noise::tests`' `each_key_reads_its_own_stream`,
    `a_cache_never_serves_one_key_s_lattice_or_table_for_the_other` and
    `every_octave_s_rotation_is_proper`; the closed forms (Euler–Maclaurin to B₆ from degree 32)
    against direct sums to 10⁻¹¹ for β of 1.1 to 4; the gradient against central differences on
    the Earth- and Ceres-like worlds; order and cache independence, one cache shared by two seeds
    and the test planet; the read set; the styles; the multifractal's weight; the stored octave
    RMS against the direct one, bit for bit; `unresolved_variance` falling to zero at the band
    limit, and zero on a body too small for any octave; twelve `should_panic` tests in
    `tests/panics.rs` for the new public panics.
  - _Acceptance._ `cargo test -p hyperion-surface synth::relief` selects the relief's 20 fast tests
    (the plan's four among them) but nothing of the generalisation, so T5 also ran the crate's whole
    suite, `cargo test -p hyperion-surface --features testing`, merged with `rendering-and-planets`
    at `7523a618`: 178 library tests (11 slow ignored), R05's goldens unchanged, 40 panics, and
    T1.a's call-site test; the noise's two slow tests and the weight's pass too, and `just _browser
prepare` ran the surface crate's 233 tests on the browser target.
  - _The gradient_ is that of H(P) = h(M⁻¹P ÷ |M⁻¹P|), T4's convention (M⁻¹ (I − d dᵀ) M applied
    to the noise's gradient at P), so it adds to the base elevation's.
  - _Science findings, for "main"_ (from the science check; not fixed, since each moves T2's or
    T12's work or the wire):
    - The single law V₁ l^−1.9 to the 2 m band limit is far outside its calibration: on the
      Earth-like world (V₁ = 3.6 × 10⁶ m²) it gives about 1 m RMS in wavelengths of 2–4 m (the
      finest octave, 4.4–8.8 m, carries 1.37 m) and an RMS slope of 62–71° at 1 m and 27° at 17 m (the Moon's median slopes at 17 m are 2.0° in the maria
      and 7.5° in the highlands, Rosenburg et al. 2011). Landscape spectra steepen below about 2 km
      (Hurst 0.7–0.9, Rosenburg et al. 2011 ¶20; Mars's exponent about 3.4 at 0.7–7 km, Aharonson,
      Zuber and Rothman 2001; β 3.5–4.2 below 180–250 m on soil-mantled terrain, Perron, Kirchner
      and Dietrich 2008), and the local slope of Hirt and Rexer 2015's fitted BED2014 degree variance
      steepens from −2.06 at l = 300 to −2.53 at 2,400. Lean: a break in `BandSpectrum`, β₂ ≈ 2.7 below about 2 km on an Earth (5–7 km on a
      Mars), a header field and so a wire change, made before T9's goldens with T2's follow-ups'
      re-bless. The relief reads the law only through `level_variance`, so it takes the break
      unchanged.
    - T2's Earth-like V₁ (σ_h² ÷ ζ(1.9)) normalises the law from degree 1, and gives 116 m RMS
      above l = 556 against Earth2014 BED's 183 m (Hirt and Rexer 2015, Table 3) and Design note
      7's "0.6% … about 200 m". T10's and T12.e's V₁ is fitted at the coarse cell's degrees (about
      9.6 × 10⁶ m² at β = 1.9), but only with the break, without which the 2–4 m wavelengths reach
      1.6 m. Ruled (`decision-r09-t5.md` item 3): 2.253 × 10⁷ m² at the anchor.
    - Design note 7's arcs "100–200 km behind" trenches: Bird 2003 (G³ 4, 1027, ¶105, p. 39) has
      "200–250 km is more typical" for the forearc (medium confidence), for T12.b.
    - The spectrum may count relief twice: lunar highland relief at kilometre scales is mostly
      craters, which T7.b adds, and channels add incision. Lean: the spectrum is the structural
      share alone, and T12.e subtracts each contribution's closed form at the coarse cell
      (`unresolved_variance`).
    - An abyssal plain needs a turbidite supply (a passive margin or a fan), which the coarse field
      does not carry, so every old floor is plain here, where the central Pacific's old floor is
      sediment-draped hills, about 0.65 of young ones on GMRT (the check's estimate): an input for
      T11–T12. The usual "slope under
      1:1,000" (Heezen, Tharp and Ewing 1959) was not reached; the Hatteras plain's multibeam
      median is 1:1,436.
    - _Ruled 2026-10-10 in `decision-r09-t5.md`_: the break (item 2); V₁ at the coarse cell
      (item 3); the structural share alone (item 4); the sea floor's inputs (item 5); the arcs
      (item 6).
  - _For T8._ `Relief::at` with `through` from the level's set; `Relief::cover` for `bake_patch`;
    `octave_bound` for `level_bound_m`. Not benchmarked: a query interpolates three style
    components (about three base elevations' worth) and up to 14 octaves, against Design note 14's
    1.5 µs; one interpolant over the three components is the lever. _For T9_ (the determinism
    review): nothing pins the relief across targets yet, since `height_at` does not include it
    until T8; T9's `height_golden.rs` pins `Relief::at` apart from `height_at` (heights and
    gradients on the four worlds with relief, through L + 3, before the warp, and through the
    finest), with the 32 octaves' offsets, `nyquist_degree` and each `octave_rms`, as T4's note asks
    for the base elevation. _For T6.b and T7.b:_ `UnresolvedVariance` gains their fields and
    `total` their sum. _For T12.e:_ `local_variance` and the mean a² above. _For R05's
    re-validation:_ R05's Provides comment on `gradient_noise` ("`Octave` holds k, offset,
    rotation, seed") and its T3 record ("the index, spacing, rotation, offset and seed") now read a
    `NoiseKey` for the seed; `spacing_m` is `spacing`, and the rotation is `noise::rotation`.
- **Deviations in T6.a, as built** (2026-10-10).
  - _No variance member._ By "main"'s adoption (2026-10-10) of the lean on `decision-r09-t5.md`
    item 4, T6.a adds nothing to `UnresolvedVariance`. T6.b adds the channels' member, a form on
    per-cell inputs for T12.e and a test pinning the two equal. T6.a exposes, per level,
    `head_length(radius, level)` and `tributary_rise(k_s, head, length)`, the bed's rise along a
    tributary in closed form.
  - _Files and public items._ `synth/channels.rs` (tests in `synth/channels/tests.rs`):
    - `Channels` (`new`, `seed`, `first_level`, `network`, which panics on another body's field),
      re-exported as `synth::Channels`;
    - `ChannelNetwork` (`key_point`, `is_inherited`, `segment`), memoised per network by packed
      cell key, the caller's; T6.b moves the memo into `SynthCache`;
    - `KeyPoint` (`face`, `st`, `dir`, `is_in`);
    - `Segment` (`cell`, `level`, `is_trunk`, `from_m`, `to_m`, `crossing_m`, `point_m`, `length`,
      `outlet`, `bed_from`, `bed_to`, `bed_at`, `main_stream_at`, `area_at`, `slope_at`,
      `steepness_m0_9`), `Clone` but not `Copy` at 136 bytes;
    - `Outlet` (`Cell { receiver, exit }`, `Join { parent, at }`);
    - `main_stream_length`, `hack_area`, `head_length`, `tributary_rise`;
    - `KEY_POINT_MARGIN`, `CROSSING_MARGIN`, `HACK_COEFFICIENT`, `HACK_EXPONENT`, `MINIMUM_SLOPE`.

    `segment` returns T4's `ReadCellError`. `surface.channel`'s declaration now names its draws.

  - _Key points._ Face coordinates are integers in units of 2⁻⁵⁰ of the face's side. Each cell
    draws as `surface_cell(face, level, i, j, 0)`, from the top 24 bits of words 0 and 1, uniform
    in [ε, 1 − ε) with ε 0.25. The child that holds its parent's point inherits it, the same bits
    at every level (Dendry's `ReplaceNeighboringPoints`); the other three draw.
  - _Trunks are two chords_ through a crossing on the shared edge: where the straight chord between
    the key points meets the edge's great circle, held to the edge's central half
    (`CROSSING_MARGIN`, T6.a's own). Two cells across a face edge are not together convex, since a
    row of cells bends there, and near a cube corner the straight chord between their key points
    passes through a third cell (the first build's test caught it). The hold binds on 77 of the
    Earth-like world's 121,310 trunks and 140 of the Mars-like world's 98,229, all but one across
    face edges.
  - _Joins follow the brainstorm's rule_ (its per-query evaluation, step 4): the nearest point of
    every coarser level's segments in that level's 3 × 3 graph neighbourhood, across coarse cells
    and faces. Distance is the 3D chord between spheroid points, in metres. Ties go to the coarser
    level, then the lower packed key.
    - A first build confined joins to the key point's own coarse cell, so that every tributary's
      water left through its cell's trunk. The plan-conformance review found that against the
      brainstorm, and it set a divide along every coarse-cell edge, so it was reverted.
    - The coarse drainage areas therefore hold for the trunks, not for every tributary's water,
      and the test "the network leaves each coarse cell through the cell its flow direction names"
      is the first level's: every trunk's first chord lies in its cell and its second in its
      receiver, and every finer segment's chain of parents ends at a trunk.
    - Some tributaries join across face edges (tested).
    - The 3 × 3 is T6.b's search, built here without its bounding boxes. T6.b keeps its results:
      one shared `nearest_on`, pruning only on a box strictly farther than the best, and no cached
      inverse length that changes the bits (the determinism review).
  - _Where tributaries are._ Only in a dry coarse cell whose flow names a receiver and whose
    steepness code is above 0. A tributary reads its own coarse cell's `k_s`, wherever it joins.
  - _Profiles._
    - A trunk's bed runs straight between the two cells' water surfaces (the sea's or a lake's
      level where it reaches one): the coarse field's fall, over the trunk's own length. Its key
      points are 0.5 to 1.6 centre distances apart, so its slope is about 0.6 to 2 times the field's
      `k_s` A^−θ. It is exempt from the minimum slope, and flat across level water.
    - A tributary falls at max(`k_s` C^¾ L^−¾, S_min) with L Hack's main stream, in closed form,
      the stream-power part through square roots (θ ÷ h = ¾). Its bed at the join is its parent's
      there, bit for bit.
  - _T6.a's own constants._
    - The head length is Hack's length of one mean cell of the key point's level: 40.9 km at level
      9 on an Earth, 3.37 km at 12, 4.34 m at 20.
    - The minimum slope is one floor, 10⁻⁵, at every level: two thirds of the lowest water-surface
      gradient measured on the Amazon (Birkett et al. 2002). "Held to a minimum slope by level" is
      read as each level held to its parent.
  - _Hack's C_ is 0.319 741 m^−0.2 (the plan's 0.320). The science check read it from Hack 1957:
    eq. 3, p. 63, with L measured along the channel to the divide (pp. 47–48).
  - _The 10⁴ km² test_ checks Hack's function, 319.7 km, and the Earth-like trunks that drain
    10⁴ km² to 5%: the area-to-length path, since a trunk's main stream is Hack's of its area. The
    network's own path lengths are procedural, not Hack's.
  - _The read set._ The segments below a coarse cell, to L + 3, read cells up to three king moves
    from it (measured on the Earth- and Mars-like worlds; Design note 15 budgets four for the first
    level).
  - _Tests beyond the list._ Eleven more unit tests in `synth::channels`:
    - Hack's conversion and the head lengths;
    - the fixed-point layout and the key points;
    - the first level against the coarse flow;
    - the worlds without a fluvial step;
    - the profile against `k_s` A^−θ and a central difference, and the closed form against
      quadrature across the floor's kink;
    - order independence over fresh networks in four orders, cell by cell alone, and through one
      warm network;
    - the seed;
    - the read set.

    Nine `should_panic` tests are in `tests/panics.rs`, which gains `level_five_field_flowing`.

  - _Acceptance._ `cargo test -p hyperion-surface synth::channels` selects the 15 unit tests; the
    panics need `--test panics`. T6.a also ran the crate's whole suite, `cargo test -p
hyperion-surface --features testing` (193 library tests, 11 slow ignored; 49 panics; every
    golden unchanged), and `just _browser prepare` (the surface crate's 257 tests on the browser
    target).
  - _Science findings, for "main"_ (from the science check):
    - _Runoff-weighted areas._ Design note 9's coarse pass stores Hergarten's runoff-weighted area
      and `k_s` against it, while Hack's law relates length to planimetric area (Hack 1957, p. 47).
      With runoff r̄ in m a⁻¹:
      - a tributary's slope is off by r̄^θ (2.8 times too gentle at 0.1 m a⁻¹);
      - a trunk's main stream is off by r̄^h (about 4 times too short).

      Latent, since the synthetic worlds' areas are geometric and T14.b is unbuilt. Asked of
      "main", lean: the field's `drainage` and `steepness` stay geometric, the weighting inside
      T14's solver, with no wire change.

    - _Ranges._
      - Hack's basins span 0.31 to 971 km² (Hack 1957, p. 45). From level 15 on an Earth a head
        drains less than the smallest (level 14's mean cell, 0.317 km², is just above it), and
        continental trunks far more.
      - Stream power holds only below slopes of about 0.03 to 0.1 (Stock and Dietrich 2003). At
        `k_s` 300 m^0.9 a head exceeds that from level 11.
      - So T6.b's level cut ends the network at the channel heads (`decision-r09-t5.md`, adjacent
        finding C).
    - _Area bookkeeping._ The areas at a level's new joins sum to more than their parent cell (+33%
      at level 12 and +6% at 9): each head drains a whole cell, and Hack's L follows meanders while
      a tributary grows by its chord. That leaves slopes up to about 14% gentle, for T6.b's and
      T14.c's closed forms.
    - _The neighbourhood._ Dendry's paper needs at least 5 × 5 cells for the true nearest segment
      (§4.1.2). The 3 × 3 is the plan's, already among Design note 13's departures.
  - _For T6.b._
    - The incision (cross-profile, depth) and each level's mean over its parent cell, from the
      segments.
    - The level cut at channel heads and widths.
    - The memo into `SynthCache`, keyed by seed and field, since a segment depends on the coarse
      records too.
    - The channels' `UnresolvedVariance` member and `structure_function`.
    - A first-level chord sags below the surface by up to L² ÷ 8R, about 25 m over 36 km, so the
      cross-profile's distance to a trunk measures to the chord's radial projection, or accepts
      that.
  - _For T8._ The channels read three king moves for the network to L + 3, before T6.b's query
    search.
  - _For T9_ (the determinism review). A channel golden in a subdirectory, at `GENERATOR_VERSION`:
    - key points to L + 3 (s, t, inherited);
    - trunks: interior, across a face edge, held at a crossing, and ending at a sea;
    - tributaries: one across a face edge, one clamped at a chord's end, and one at the floor;
    - `main_stream_length`, `hack_area`, `head_length` and `tributary_rise` about the floor's
      crossover.
- **Deviations in T5's follow-up, as built** (2026-10-10, `decision-r09-t5.md`; lane
  r09-synth-fix, after T5, T6.a and T10 landed).
  - _The sim's frame._ T10's quantiser builds `SynthesisCell`s and its initial state a
    `BandSpectrum`, so the follow-up touches the sim as much as compiling needs: `CellState` gains
    `hill_relief: Option<Metres>` and `ponded_sediment: Metres`, `None` and zero until T12.a fills
    them, which `quantise` turns into the byte (`seafloor_byte`); and `Working::initial`'s spectrum
    is a silicate crust's anchored at zero on the body, until T12.e anchors the crust class. No sim
    golden pins the coarse field, so none moves.
  - _The spectrum._ `BandSpectrum::new(BandSpectrumParts)` with `break_degree` and
    `small_scale_exponent`, their getters, `degree_variance(l)` and the crate-private
    `break_factor`; `BuildBandSpectrumError::{BreakDegree, SmallScaleExponent}`, checked after β₁
    and V₁, and one refusal beyond the ruling's, `Total`: parts each in range whose whole variance
    from degree 1 is not finite (a break far past every degree under a much steeper β₂ overflows
    its factor, NaN where V₁ is 0), so that no payload the decoder accepts can make the relief's
    asserts panic (the determinism review's lean). `SpectrumShape` has private fields, getters and
    `new(exponent, break_wavelength, small_scale_exponent) -> Result<_, BuildSpectrumShapeError>`
    (`Exponent`, `BreakWavelength`, `SmallScaleExponent`), so a new crust class is a checked value,
    not a code change. `anchored` builds its result through `new`, panicking with "a spectrum shape
    must give a valid law on this body" when a shape outside every crust's overflows it, and floors
    the break degree at 1, where the ruling's 2πR ÷ λ_b would be refused: on a body under λ_b ÷ 2π
    (318 m for silicate) every degree is below the break wavelength. `band_variance` is the
    difference of the two tails taken segment by segment, so that the terms they share cancel
    exactly; and the second segment's start L\* is held as an integer-valued `f64` (`tail_from`
    beside `degree_tail`), so that a break beyond `u64`'s range is not saturated. Every value of the
    single law is unchanged bit for bit. `BandSpectrum` is a `FixedWire` of 32 B, which the strides
    test pins. The octaves now leave out under 10⁻⁶ of `local_variance`, far below the break.
  - _The field._ `SynthesisCell.seafloor`, `NO_SEAFLOOR` (0),
    `pack_seafloor(hill, ponded) -> Option<u8>` (`None` past a nibble), `hill_relief`,
    `ponded_sediment`, `quantise_hill_relief` and `quantise_ponded_sediment`, with the readers of
    a bare byte crate-private (`field::{hill_relief_of, ponded_sediment_of}`) for
    `ReliefStyle::from_codes`. The hill code saturates at both ends, so every positive relief has a
    code from 1 to 15 and only zero is none; the sediment code is 0 under 21.02 m (half a step
    below 25 m). `BuildFieldError::Seafloor { cell }` is checked last in `check_synthesis_cell`, so
    the decoder refuses it too (`DecodeBlockError::Record`).
  - _The style._ `H_REF` (95 m, `Metres`), `HILL_AMPLITUDE_EXPONENT` (0.6) and the private
    `PONDING` (0.5, 3.0) carry item 5's law; `ARC_OCEANIC_KM` and `ARC_CONTINENTAL_KM` are
    private, as `ARC_KM` was, read through `band` beside `taper`. A stagnant lid's crusts take the
    continental band, never reached since they have no boundary. A byte naming no hills on oceanic
    crust, which no field holds, is taken for a plain.
  - _The testing worlds._ `FieldBuilder` holds its spectrum as an `Option`, and its default, a
    silicate crust anchored at zero on the builder's body, is made in `build`, which checks the
    radius. `FieldBuilder::seafloor` takes a `SeafloorSample` (hill relief and ponded sediment,
    SI) per oceanic cell, the reference hills (`SeafloorSample::REFERENCE`, `H_REF`) by default. The
    Earth-like world's closed forms (`worlds.rs`, labelled as T11's and T12.a's, for them to
    replace) depart from item 5's construction where this world cannot follow it: the ridge is the
    nearest bisector with a plate the builder's own class calls divergent, and its half-rate the
    relative motion's normal component at the cell, halved, not at the own-ridge transform's
    source; distances run along great circles to seed bisectors, which may belong to plates that
    are not neighbours, not along paths over the plate's own cells; and since each of its plates
    has one crust, so that the ruling's same-plate margins would give none, the margin is the
    nearest bisector with a continental plate that does not converge with this one. Its floor is
    old (median about 130 Myr), so GlobSed's drape freezes most of it at H₀ ÷ 2 within 5–20 Myr:
    only some 5% keeps undraped hills. The test therefore classes the floor by its amplitude,
    against the ruling's three floors: hills at a ≥ 1 (the slow ridges', 1.73 undraped at the slow
    MAR's measured H of about 236 m, `decision-r09-t5.md` item 8), draped hills at 0.2 < a < 1 (the
    fast ridges' and the frozen drape, 0.48) and plains at a ≤ 0.2:
    36.6%, 49.6% and 13.8% of its oceanic cells. Its ⟨a²⟩ as ruled, the mean square of the
    interpolated amplitude at the variance test's area-uniform points, is 0.594 (T5's 0.74; its
    cells' unweighted mean is 0.594 too, 0.693 over its oceanic cells); Mars-like 0.934, Moon-like
    0.946 and Ceres-like 1. The 5% variance test holds on all four (ratios 1.0007, 1.0055, 1.0017
    and 0.9902).
  - _The slopes._ `the_reference_style_s_slopes_are_the_ruling_s` samples the reference style's
    adirectional RMS slope over 1, 10 and 100 m from 2 × 10⁴ points a body, atan(√2 ν(b) ÷ b):
    Earth-like 17.67° / 15.45° / 11.64° (the ruling's 18.0 / 15.6 / 12.0), Mars-like 5.42 / 4.75 /
    3.55, Moon-like 5.48 / 4.74 / 3.53, Ceres-like 5.34 / 4.72 / 3.51 (each 5.5 / 4.8 / 3.6), and
    an ice-rich Europa 14.40 / 10.08 / 5.48 (14.7 / 10.2 / 5.6). Each tangent is held within 5% of
    the ruling's (its 0.1° rounding is 1.4% at 3.6°, and its model interpolates the noise's
    structure function linearly between lags), the 5% at least 3.29 standard errors. A collision
    belt's full style measures 52.3° / 47.1° / 37.7°, against the ruling's 39.0 / 35.0 / 28.1,
    which scale the plain noise's slopes by the amplitude: the ridged term is steeper than the
    plain one at the same RMS height. Recorded and asserted only to exceed the reference; for T8's
    closed form and R10's cliff fractions, for "main". Ruled in item 7 and built by the second
    follow-up (below).
  - _Tests otherwise._ T5's single-law tests run on a law whose break is at degree 1; the broken
    law's closed forms against direct sums to 10⁻¹¹ for (2.0, 3.0) and (1.8, 2.6), breaks at 300,
    300.4, 1 and 20,015.1, bands below, across and above; continuity at the break to 10⁻¹²; the
    four worlds' V₁, local RMS and first and finest octaves against the ruling's table (Earth-like
    2.253 × 10⁷ m², 200.0 m, 142.4 m, 9.0 cm), a Europa's V₁ and local RMS, a zero anchor and a
    body under the break; the finest octave's bound is 1.980 m (the ruling's 1.97, from the rounded
    9.0 cm). The gradient test's tolerance is now 10⁻⁵ of the larger of the gradient's magnitude
    and the sum of the octaves': a difference errs in proportion to each octave's own slope (the
    noise is C² at its lattice planes), which the broken law's centimetre finest octaves made
    visible where the octaves' slopes cancel to a hundredth. The sea floor's amplitude is checked
    against the ruled law, bit for bit, at every pair of codes on the ramp. The styles test is
    split in three (styles, arcs, every style against its codes) and the forms' test in two, for
    Clippy's 100 lines. Five new `should_panic` tests (`degree_variance` twice, `anchored` three
    times). As T5 did, the follow-up ran the crate's whole suite,
    `cargo test -p hyperion-surface --features testing`, since the acceptance filter misses the
    codes, the codec, the goldens and the panics, on `rendering-and-planets` at `f3c6159d` with
    T6.a: 203 library tests (11 slow ignored), the goldens and 54 panics; `just _browser prepare`
    lists the surface crate's 272 tests on the browser target, as natively; and the sim's
    `planetary::surface` tests (31) pass.
  - _The goldens_ (Generator version): `payloads.golden` re-blessed at 21, `codes.golden` extended
    by both nibbles' 16 codes, no version bumped. Each single-block payload grew by the header's
    16 B and 1 B a cell: Moon-like 862,458 → 887,050 (24,576 cells), Ceres-like 218,670 → 224,830
    (6,144), Flat 848,076 → 872,668 and OneCrater 848,442 → 873,034 (24,576), the Mars-like
    region 86,559 → 88,876 (2,301) and its delta 22,155 → 22,733 (562, decoded). The Earth-like
    payload is 13,961,471 B in 14 blocks (13,567,743 in 13), Mars-like 3,512,154 in 4, and the
    Moon-like split at 48 KiB 949,134 in 20 (921,718 in 19): their blocks split anew.
  - _For "main"_: the belt's slopes above; the Earth-like world's ⟨a²⟩, 0.594, under T12.e's
    0.6–1.2 for the reference Earth (a property of this world's old floor and few margins); the
    `Total` refusal added beyond the ruling's; and the two shapes, a closed set for the composition
    audit (sulphur, salt or organic crusts have none). From the science check: item 5's H₀ law
    gives 223 m at a half-rate of 14 mm a⁻¹, not the 232 m the ruling quotes (it reaches 232 m at
    12; Goff, Smith and Marks 2004's Table 2 has 235.8 m at a full 28), which T12.a's 30% band
    holds either way, while `H_REF`'s calibration keeps the ruling's measured H of 232 m on the
    slow ridge's flank; and Europa's long-wavelength roughness is described two ways in the
    ruling, limb β about 2 at 10–60 km (item 2) and H about 0.3 to 70 km (item 3, β 1.6), with
    Schenk and Nimmo 2017's Fig. 1 reading H about 0.2 over 12–630 km. Neither moves the ruled
    β₁ 1.8 or the code. The science check could not see Perron et al. 2008's ¶36 or Goff 2020's ν
    in their sources, both the ruling's readings. _Corrected in the ruling_ (2026-10-10, with its
    item 7): item 5 now gives 223 m at a half-rate of 14 mm a⁻¹ (Goff et al.'s 236 m) and the slow
    MAR's H as 223 m (a = 1.67), and item 3 reads Europa's H as about 0.4 from 1 to 70 km, the ice
    shape's β₁ 1.8. Item 8 then takes the slow MAR's measured H, about 236 m (a = 1.73), keeping
    223 m as item 5's fit. Applied by the second follow-up (below).
  - _For T8._ `structure_function` and `unresolved_rms` read the law through `degree_variance`;
    the ruling's two slope tests; the belt's ridged term needs its own structure function in the
    closed form. _For T9:_ the height goldens are first written on the broken law. _For T12.a:_
    `quantise_hill_relief`, `quantise_ponded_sediment` and `pack_seafloor`. _For T12.e:_
    `BandSpectrum::anchored` with `SpectrumShape` and the `STRUCTURAL_RMS_*` anchors (the mobile
    lid's times T12.b's factor), and `ReliefStyle::from_codes` on its quantised codes.
- **Deviations in T5's second follow-up, as built** (2026-10-10, `decision-r09-t5.md` item 7 and
  the ruling's corrections; lane r09-synth, on the first follow-up's branch merged by name before
  it landed).
  - _The belts' law._ `BELT_VALLEY_GAIN` (1.8), `BELT_VALLEY_SCALE` (1.5), `BELT_VALLEY_WIDTH`
    (0.6) and `BELT_HILLSLOPE_STEEPENING` (0.4), public as the other style constants are, each
    citing item 7 and the SRTM belt cores; the private `break_ratio(spectrum, m)` (λ_m ÷ λ_b),
    `ridged_share(ratio)`, `belt_log_factor(ratio)` and `belt_factor(ratio)`. The ridged share is
    the cubic smoothstep in ln λ of the ruling's replica, not the quintic of `taper`, so that its
    fitted figures hold. `Relief` stores ρ_m and ln g_m per octave (792 B, was 280) and gains the
    getters `ridged_share(m)` and `belt_factor(m)` beside `octave_rms`, for T8's closed forms.
    `octave_terms`: θ_m = θ ρ_m, taking θ's sine and cosine at ρ = 1 and (0, 1) at ρ = 0, so that
    only the one octave at most strictly between takes its own; g_m^b = exp(b ln g_m); the height
    a σ g^b t in that order, and its gradient σ g^b (t ∇a + a ∇t + a t ln g ∇b). `octave_bound` is
    `MAX_AMPLITUDE` × σ_m × max(1, g_m) × `T_MAX`, in the height's order so that rounding keeps
    every height inside it; it now panics past octave 31 (the spacing's) rather than past 40.
    `unresolved_relief_variance` sums octave by octave in octave order, (a g_m^b)² times each
    octave's share of the band.
  - _The plain styles._ At b = 0 every term is the earlier arithmetic bit for bit, height and
    gradient (`the_plain_styles_are_unchanged_bit_for_bit`, at the Earth-like world's points off
    its belts and in uniform styles of every crust's amplitude, warped and not). The unresolved
    variance is not: summed octave by octave, it moves in rounding on plain cells too, so nothing
    may rely on its earlier bits. No golden pins the relief or the unresolved variance, so none
    moves; `GENERATOR_VERSION` stays 21 and the wire is unchanged. The variance
    test's expectation now carries each point's lift, a² (L_s + Σ (g_m^(2b) − 1) σ_m²): the belts
    lift the Earth-like world's realised share 5.4% above ⟨a²⟩ L_s, so the test as it stood would
    have failed (ratio 1.052); with the lift it is 0.998, the other worlds unchanged.
  - _The structure functions._ `UnitStructure` (`at(spacings)`, `small_lag_slope()`),
    `STRUCTURE_LAGS` and `PLAIN_STRUCTURE` and `RIDGED_STRUCTURE`. Deviation: the tables are the
    crate's own measure, on R05's noise basis over 4 × 10⁶ samples (an octave and the one above it
    a sample, area-uniform over the Earth-like world, octaves 10–22), not the ruling's replica's.
    The ruling's plain table (`perlin_stats.py`) is 2% above at small lags (0.0173 against 0.01693
    at 0.05 spacings): its noise is a replica, a periodic 64³ table, not R05's basis, normalised
    by its own measured RMS (0.2691, 0.7% of the gap, against the pinned 0.2701), the rest its
    sampling. Its ridged table (`ridged_sf.py`) is 1–3% below at long lags (1.95–1.98 against
    2.00), its ridged variance 0.984 from its rounded μ, σ and W. The ridged term's small-lag
    gradient variance is 2.68× the plain's (the ruling's 2.64). Deviation: the ruling's "D ∝ r²
    below 0.05" is kept with values pinned at 0.01 and 0.02 as well, all twelve of its lags, a
    power law between pinned lags (local exponent 2.0 below 0.05) and k r² below 0.01: the ridged
    term's D ÷ r² falls 1.5% from 0.01 to 0.05 spacings, within reach of the slow test's 2% under a
    pure r² law from 0.05. The slow test (`the_terms_structure_functions_are_pinned`, 10⁶ samples on
    other seeds) agrees with the tables within 0.3% at every lag, and the analytic gradients'
    small-lag slopes (plain 6.807, ridged 18.28) with the tables' (6.795, 18.22) within 0.3%.
  - _The slopes._ `a_full_belt_s_slopes_are_the_ruling_s_and_the_belt_cores` (2 × 10⁴ points):
    37.92° / 37.49° / 36.11° / 25.99° at 1 m / 10 m / 100 m / 1 km against the ruling's 37.6 /
    37.2 / 35.9 / 26.0, each tangent held within 5% (tighter than the ruling's 10% in degrees,
    36.2–39.0° at 37.6°, as the reference style's are). Deviation: the test runs in the fast
    suite, not slow as item 7 lists it; at 2 × 10⁴ points the 5% is at least 3.29 standard
    errors; and 37.05° at 31 m, 36.19° at 93 m and 25.99° at 1 km, each asserted
    inside the SRTM belt cores' RMS (34.8–41.5°, 32.0–38.6° and 23.2–28.6°, six of the ruling's
    eight tiles: it leaves out Annapurna's void-filled RMS and southern Peru's Altiplano margin).
    The 35 km window RMS (`a_full_belt_s_window_relief_is_three_times_the_reference_s`, slow, 200
    paired windows of 40 × 40 points): reference 161.7 m, full belt 508.0 m, ratio 3.14 ± 0.03,
    within 2.8–3.6 (the replica's 159 m, 505 m and 3.2), the range's half-width asserted to be at
    least z(α) standard errors of the ratio. The reference style's slopes are unchanged (17.67° /
    15.45° / 11.64° on the Earth-like world). On the Earth-like world's spectrum ρ is 1 to level
    12 (3.4 km), 0.85 at level 13 (1.7 km) and 0 from level 14 (844 m); g peaks at 1.78 at level
    12 and falls to 0.28 at level 22.
  - _Tests otherwise._ The bound on 1,000 belt points and at every octave's extreme in a full
    belt (above the plain bound exactly where g exceeds 1); ρ and g continuous and at the ruling's
    values; the stored shape the direct one, bit for bit; the gradient against central
    differences at 150 belt points, partial belts among them; the unresolved variance on belt
    cells; the tables' interpolation; five `should_panic` tests (`octave_bound` past 31, the two
    getters past 31, `at` of a negative and of a NaN lag). The crate's whole suite,
    `cargo test -p hyperion-surface --features testing`: 209 library tests (13 slow ignored), the
    goldens and 59 panics; the two new slow tests pass in the slow-test profile.
  - _The ruling's corrections_ (both found by the first follow-up's science check). The slow
    ridge's flank: `H_REF`'s documentation and the sea floor's tests read its measured H, about
    236 m, and a = 1.73 (item 8, on this follow-up's science check), in place of 232 m and 1.71;
    223 m stays item 5's fit at a half-rate of 14 mm a⁻¹, and the Earth-like world's H₀ law no
    longer says the ruling quotes 232 m. `SpectrumShape::ICE_RICH`
    and `STRUCTURAL_RMS_ICE_RICH` take the ruling's one figure for Europa, β₁ 1.8 (H 0.4) from
    70 km down to the break, in place of "a Hurst exponent of about 0.3 out to 70 km".
  - _For T8._ The closed forms are Σ_m (a g_m^b σ_m)² [cos² θ_m D̂_p(Δ ÷ λ_m) + sin² θ_m
    D̂_r(Δ ÷ λ_m)] with `PLAIN_STRUCTURE.at`, `RIDGED_STRUCTURE.at`, `Relief::ridged_share` and
    `Relief::belt_factor`. The first octave's ridged term is unweighted, so the weighted table
    is a few per cent off for it (unmeasured). T8's full-belt cells should land near 36° at
    100 m and 26° at 1 km. _For T9:_ the height goldens should include Earth-like points with
    0 < b < 1, so that the octave of 0 < ρ < 1 (level 13) and the belt factor, whose `sin_cos`
    and `exp` no test yet compares across targets, are pinned (the determinism review's nit).
    _For T12.e:_ with item 7 the realised structural share is ⟨Σ_m (a g_m^b)² σ_m²⟩ plus the tail,
    not item 1's ⟨a²⟩ L_s: 5.4% more on the Earth-like world. Ruled in item 8, the lean adopted:
    T12.e books the area mean of `Relief::point_variance` at the interpolated a and b, which this
    follow-up adds (below); Design note 7's and T12.e's text go to a docs pass. _For R10:_ the
    ruling's cliff fractions, about 30% of a full belt above 40° at 1–10 m, and none on a plain
    style. A belt's figures in break wavelengths are lattice spacings: its ridge–valley octave, 3 km
    apart on a silicate crust, carries wavelengths of about 4–8 km.
  - _Science findings, for "main"_ (the second follow-up's science check; none moves a constant).
    The slow ridge's H: Goff, Smith and Marks 2004's Table 2 gives the northern MAR 235.8 m, and
    Tivey et al. 2003's full rate at TAG (20.8 km Myr⁻¹) gives about 238 m by item 5's law, so
    a ≈ 1.72, not the 1.67 at 223 m; both lie between the 1.35 and 2.3 that calibrate `H_REF`.
    Ruled in item 8: the calibration cites the measured 236 m (a = 1.73), and 223 m stays item
    5's fit. Europa's β₁ 1.8 (H 0.4) is a choice between
    the sources, not a measure: the stereo models give H about 0.3 at 1–10 km and the limb
    profiles about 0.2 at 12–630 km, and the ruling's "limb β about 2 at 10–60 km" is in none of
    the three cited sources (perhaps Nimmo et al. 2007, Icarus 191, 183, uncited); the docs now
    say so. The ruling's `mtn_slopes.py` places southern tiles' centres 1° too far south, which
    raises the low ends of the cores' ranges by about 0.15°, within their rounding.
  - _Item 8, on resume_ (`decision-r09-t5.md` §8, 2026-10-10).
    `Relief::point_variance(amplitude, belt) -> SquareMetres`: Σ_m (a g_m^b)² σ_m² over the body's
    octaves, in octave order, plus a² times the reference style's tail beyond the finest octave,
    which `Relief` now stores (`tail_m2`, 800 B); on a body with no octave the tail runs from the
    coarse cell's Nyquist degree, so that b = 0 gives a² `local_variance` there too. Tests: at b = 0
    it is a² `local_variance` to 10⁻¹² on every synthetic world and a body too small for octaves;
    its area mean on the Earth-like world, at the interpolated a and b over the variance test's 6 ×
    10⁴ points, exceeds ⟨a²⟩ `local_variance` by 5.4% ± 0.5% (5.39%); and (slow,
    `a_full_belt_s_point_variance_is_its_sampled_variance`, 2 × 10⁵ points) it equals a full belt's
    sampled variance within 5% (313,310 m² against 314,002 ± 929 m²). The variance test now takes
    its per-point expectation from it. The plan text for Design note 7 and T12.e is left to a docs
    pass, as "main" directs.
- **Adjacent findings of `decision-r09-t5.md`** (2026-10-10; recorded there, not ruled, each for
  the task it names).
  - _T7.b's equilibrium craters must be degraded._ Fresh shapes at Trask's saturation give absurd
    metre-scale slopes: the ruling's rough estimate is tens of degrees at 10 m, against the lunar
    highlands' 9–10°. The plan's only degradation is the rim fillet's √(2κt), and it gives no
    diffusivity κ to calibrate against; the saturation cap and the diffusive age must agree, since
    equilibrium craters are mostly degraded. T7.b chooses κ, with Fassett and Thomson 2014's lunar
    5.5 m² Myr⁻¹ (from memory, to verify), and calibrates it against Rosenburg et al. 2011's 17 m
    slopes, Cai and Fa 2020's 4–100 m slopes and LOLA's structure function at 1.9 and 7.6 km (171
    and 546 m). T8's whole-function slope test enforces it: at 100 m the structural share gives the
    Moon's highlands 3.6° of their 8.5°, and the rest is the craters'. _Resolved by R09.T7.b with
    the relief factor g_ (main's ruling, 2026-10-13; "Deviations in T7.b, as built"): the fillet
    alone could not degrade a crater far enough, so every profile is scaled by Minton et al.
    2019's g. κ is Fassett and Thomson 2014's 5.5 m² Myr⁻¹, now read in their abstract; its
    calibration against the slopes stays T8's (T7.b's deferred findings).
  - _A province gets the body's crater density._ Design note 13 reads one density per body, so the
    maria would be as cratered as the highlands, and T8's maria band could fail on craters alone.
    The lean of "Physics leans still to rule" (regional ages from the centre cell where the search
    stays inside the margin) is the lever.
  - _T6.b's network should end at channel heads._ The land's spectral rollover at 150–250 m is the
    first-order valley spacing (Perron et al. 2008 ¶36, ¶44–45). Channel levels finer than the
    channel head (a drainage area of about 10⁴–10⁵ m²) over-dissect hillslopes that are smooth on
    real land, so T6.b's level cut should respect it, whatever its width cut.
  - _Abyssal hills are lineated parallel to their ridge_ (aspect 2–5, Goff's anisotropic von
    Kármán). The isotropic noise cannot show it; a fabric azimuth would be another cell field, for
    R10's re-validation.
  - _R10's readouts_ (`structure_function`, `unresolved_rms`, `SLOPE_BASELINES`) read the broken
    law through `BandSpectrum::degree_variance`, for R10's re-validation. The broken law also moves
    R10's cliff fractions: about a third of a belt above 40° at 2 m, none on an interior.
  - _Mars's sedimented northern plains_ (Aharonson et al. 2001's diffusive smoothing, β 1.4 at
    20–200 km) need the `seafloor` byte's reserved low nibble on land. Meanwhile, Phoenix-like
    plains read as lid, at 5.5° from the structural share alone at 1 m against the observed
    1.8–2.8°. T12.c or T14 may fill the nibble, under the layout rule.
  - _The T5 lane's worktree was mid-merge_ when the ruling was read. The follow-up merged T5's
    branch afresh and kept both T4's "_Applied 2026-10-09_" sentence and T5's deviations.
- **Deviations in T7.a, as built** (2026-10-10).
  - _Files and public items._ `craters.rs` keeps `CraterParams` and gains `transition_diameter`
    and `LUNAR_IMPACT_VELOCITY`; its public children `craters::chronology` (`lunar_chronology`,
    `chronology_age`, `SOLAR_SYSTEM_AGE`), `craters::density` and `craters::shape` (tests in
    `density/tests.rs`, `shape/tests.rs` and the golden's `craters/golden.rs`) hold the rest,
    re-exported from `craters`:
    - `cumulative_density`, `saturation` and `diameter_in_octave`, as Provides has them, with
      `production_density` (the same before screening), `projectile_scale` (d\*) and
      `crater_cutoff` (`D_c`) added;
    - Provides' `Octave` is `DiameterOctave` (`new(lower)`, `lower`, `upper`, the octave from its
      lower diameter to twice it), since `noise::Octave` is already public in the crate;
      `UnitUniform` (`new`, `get`, a value in [0, 1)); their errors `BuildDiameterOctaveError`
      and `BuildUnitUniformError` (each with `value()`);
    - `CraterShape` (`diameter`, `morphology`, `depth`, `rim_height`, `floor_depth`,
      `floor_radius`, `peak_relief`, `peak_radius`, `exterior_width`, `reach`, `relief`) and
      `crater_shape`. `relief(r_over_radius)` is the fresh profile, unfiltered; Provides'
      band-limited `profile` stays T7.b's, built on it.
  - _Ranges._ Every diameter must be finite and positive in km too (from about 10⁻³²⁰ m), and
    `CraterParams::new` now also refuses (as `Screening`) a screening whose crater cutoff is under
    that or whose 1,000 `D_c` overflows, so that the quadrature's bounds are always finite.
    `lunar_chronology` refuses a surface older than its system (item 5's 0 ≤ S ≤ A). Each assert
    has its test in `tests/panics.rs`.
  - _The ends of the polynomial._ Above 300 km the cumulative production continues at the
    polynomial's own slope there, −1.826, so it is continuously differentiable; below 10 m at the
    plan's −2 (the polynomial's own slope at 10 m is −3.16, a kink). Neither a0 is a misprint
    (science-r09-craters.md F3): −3.0876 is the 2001 polynomial's own level and −3.0768 Neukum
    1983's; the chronology keeps 8.38 × 10⁻⁴; a0 never enters.
  - _The diameter map._ Design note 12 names π-group scaling and nothing more. As built, after
    Ivanov 2001: the body's crater is uncollapsed to a transient cavity by D ∝ `D_tc`^1.13
    `D_t`^−0.13 (McKinnon and Schenk 1985 as Collins, Melosh and Marcus 2005 state it, made
    continuous at the body's `D_t`; Ivanov himself used Croft 1985's 1.18), scaled by
    (g ÷ g☾)^0.22 (v☾ ÷ v)^0.43 (Ivanov et al. 2002, eq. 4's γ and β for non-porous rock), and
    recollapsed about the Moon's 19 km, with v☾ = `LUNAR_IMPACT_VELOCITY`, 16.7 km/s
    (science-r09-craters.md item 4, replacing the first build's 16.1). The Mars test runs at
    item 4's ratios: at g 3.71 and U = 16.7 × 9.6 ÷ 16.1 km/s (Ivanov's velocity ratio) the map
    reproduces Ivanov's Mars function (Craterstats' coefficients) within 3.3% in N over 1–100 km,
    and at P14.T48.f's 10.6 km/s within 9.2%, against the test's ×1.5; the lunar shape alone is
    2.6× off at 10 km. Per T2's contract the map runs only when `impact_velocity` is given, and
    the strength regime below a few hundred metres is not mapped. R09.T7.e adds the target's
    class and density and the projectiles' density, with exponents derived from each class's μ
    (item 2).
  - _Screening._ "Airless" is no cutoff: `Screening::None`, or an atmosphere of no column mass,
    both for the taper and for the airless bowls. The screened density is integrated in ln D by
    eight-point Gauss–Legendre panels a quarter of a decade wide (the sim's GL8 values), on a
    grid fixed per body and split at the production's kinks (10 m and 300 km lunar-equivalent
    and, under a map, its two collapses), to 1,000 `D_c`, then the closed form; it agrees with an
    independent Simpson integration to 10⁻⁶ and is summed from the top down, so it never rises
    with D to the last bit (it is flat in f64 far below a thick atmosphere's cutoff, so the
    monotonicity test is non-increasing; the production is strictly decreasing). Venus's counts
    lie within 3% of the Gazetteer's at 3–40 km; its N(>1 km) is 1.94 × 10⁻⁶ km⁻² against a
    production of 3.07 × 10⁻⁴. Trask's 0.079 is his 10^−1.1 = 0.0794, rounded as Design note 12
    has it; Hartmann 1984's equilibrium is a different law (slope −1.83), no longer cited for it.
  - _Venus without the map._ The Gazetteer check runs on the unmapped lunar shape, as T0.b's fit
    did. Under P14.T48.f's 25.7 km/s the map would raise Venus's N(>D) ÷ N(>1 km) by ×1.3 at
    3 km to ×2.0 at 40 km, beyond the test's 10%. Adopted lean ("main", 2026-10-10, sent on to
    the crater science agent): R09.T7.e re-runs the check through the map and refits the taper's
    exponent and N(>1 km) if it fails.
  - _The central peak's jump._ By `decision-r09-t2.md` item 3's table the peak enters at full
    relief at 1.5 `D_t` and ends at 9 `D_t`, so the profile jumps in D there (a steps −0.016 and
    +0.040). Adopted lean ("main", 2026-10-10): keep the ruling's jump; a ramp, if the science
    agent rules one, goes to R09.T7.e or the docs pass.
  - _a0's two readings._ Adopted lean ("main", 2026-10-10): the doc records both readings of the
    printed a0 (Design note 12's misprint and Cratermaker's correction of the linear term), with
    the values as ruled; a0 never enters. **Ruled (F3)** (science-r09-craters.md F3): neither a0 is
    a misprint. −3.0876 is the 2001 polynomial's own 1-Gyr level, which matches Neukum 1983's below
    1 km, and −3.0768 is Neukum 1983's; the chronology keeps the published 8.38 × 10⁻⁴, as
    Craterstats pairs it with this polynomial, rather than Cratermaker's 8.17 × 10⁻⁴. R09.T7.e's
    `NEUKUM` doc takes F3's text.
  - _The inversion_ bisects ln D 40 times (an interval of 6.3 × 10⁻¹³) on the craters between the
    octave's lower diameter and the midpoint alone, and is clamped into [lower, upper). Its
    Kolmogorov–Smirnov test runs 10⁵ draws on the unscreened Moon and on mapped, screened Mars
    (2–4 m, by its 1.7 m cutoff) in the fast suite; Venus's 10⁵, each tested against its full
    quadrature, is slow (`#[ignore]`), with a 5,000-draw fast version.
  - _The shapes._ As `decision-r09-t2.md` item 3, with these readings:
    - the floor is held at 0.778 of D from 16.2 `D_t`, where 0.389 x^0.249 reaches it (the
      ruling's script), so it stays continuous;
    - the bowl's exterior width is 1.5426 (d ÷ h = 5.42), which the test pins to 10⁻³; the
      plan's "a = 1.54 to 10⁻³" is that rounded;
    - the exterior widths: transitional craters fall from the bowl's 1.5426 at 0.8 `D_t` to 1.33
      at 1.5 `D_t`, and complex craters lie in 1.28–1.36 to 25 `D_t` (1.05 at 100 `D_t`), with
      steps of −0.016 at 1.5 `D_t`, where the central peak enters at full relief, and +0.040 at
      9 `D_t`, where it ends. Design note 13's "1.28–1.46 for transitional and complex craters"
      holds for the complex ones (1.46 is a at `D_t`), so the test checks 1.28–1.36 and
      1.33–1.543 apart. a ≤ 1.543 everywhere, so the reach stands;
    - the gravity test checks d ∝ g^−0.697 and h ∝ g^−0.601 to 10⁻⁹, the laws' exact exponents
      (1 − 0.303, 1 − 0.399), which the plan rounds to 0.70 and 0.60;
    - a shape whose interior holds no net cavity (D beyond about 10⁵ `D_t`, a basin wider than
      any body) gets no exterior rather than a NaN;
    - 5.42 is Pike 1980a's 0.195 (Table 2) over Pike 1980b's 0.036, not Pike 1980b's own 0.196
      over 0.036 (5.44), as the docs now say.
  - _The transition test_ checks Pike 1980a's Table 3: the Moon against 19 km and Earth against
    3.1 km (the build gives 19.0 and 3.14 km), within 10%.
  - _`FieldBuilder`._ Each crater's morphology and relief are now `crater_shape`'s, and its reach
    is its own (1 + a) rim radii rather than 2.54; a new test holds every listed crater's
    morphology and OneCrater's every cell to its shape. Every cratered synthetic world's elevation
    moves, so `tests/golden/wire/payloads.golden` is re-blessed at 21 under Generator version's
    rule ("main", 2026-10-10); no other golden moves.
  - _A new golden_, `tests/golden/craters/density_and_shapes.golden` (header 21), pins the
    density, the production, the inversion, the chronology and its inverse, and every shape's
    dimensions and relief as `f64` bits on six bodies, so that the WebAssembly runs check the
    targets agree (the determinism review). It extends the goldens; nothing generated moves.
  - _Icy and porous targets, impactors and time_ (raised by the T7.a lane, ruled in
    `science-r09-craters.md`, 2026-10-10). Item 1: `k_target` 0.12 stands, sourced by nine icy
    bodies' transitions (T7.a's docs cite it). Item 2: the target's scaling class (non-porous,
    porous) and density and the projectiles' density are R09.T7.e's; until then the map takes
    non-porous rock's exponents and no density for every target. Item 3: the airless bowls'
    shallowing applies to rocky and icy airless bodies alike (T7.a's docs say so). Item 4: every
    body's mean impact velocity is P14.T48.f's closed-form model, and the map's lunar reference
    is its 16.7 km/s. Item 5: the chronology lives here (below).
  - _The chronology_ (item 5). `lunar_chronology(S, A)` counts Neukum, Ivanov and Hartmann 2001's
    curve from the system's formation, N = 5.44 × 10⁻¹⁴ (e^(6.93 (o + S)) − e^(6.93 o)) +
    8.38 × 10⁻⁴ S with o = `SOLAR_SYSTEM_AGE` (4.567 Gyr, Connelly et al. 2012's 4.5673 rounded)
    − A, in `craters/chronology.rs`; `chronology_age` inverts it by 64 fixed halvings of [0, A],
    zero for a density of zero or less and A at or above the system's whole. `reference.rs`'s
    private copy is gone: its worlds call it at `SOLAR_SYSTEM_AGE`, bit-identical (o is 0.0 and
    e^0.0 is 1.0; the test pins every reference world's age), so nothing moves.
  - _For P14.T48.c._ `cumulative_density(&CraterParams, Metres) -> PerSquareKilometre` returns
    N(>1 km) exactly at 1 km on an unscreened body, and Venus's screened density at 1 km is 0.6%
    of its production. Without `impact_velocity` a body takes the lunar shape, up to 2.6× off on a
    Mars at 10 km: P14.T48.f supplies the velocity. P14.T24.b's N(>1 km) is
    `craters::lunar_chronology(S, A)`.
- **Deviations in T7.e, as built** (2026-10-11; science-r09-craters.md items 2 and 5, F1–F4).
  - _Files and public items._ Beyond the task's five files: `field.rs` exports its `coded_enum!` to
    the crate, so that `craters::TargetClass` is one byte like the field's enums; `field/header.rs`
    refuses a surface older than its system; `wire.rs`, `wire/tests.rs`, `craters/golden.rs`,
    `tests/panics.rs` and the sim's `planetary/surface/mod.rs` doctest take the new fields. Added
    public items: `TargetClass` (`NonPorous` 0, `Porous` 1, `ALL`, `u8` conversions), `CraterTarget`
    (`class`, `density`), `LUNAR_TARGET` (non-porous, 2,550 kg m⁻³) and `LUNAR_PROJECTILE_DENSITY`
    (3,000), the map's reference beside `LUNAR_IMPACT_VELOCITY`; `own_density_at_1km` (before
    screening); `CraterParams::{projectile_density, target, system_age}`;
    `BuildCraterParamsError::{ProjectileDensity, TargetDensity, SystemAge, DiameterMap}`, the last
    for parts each in range whose cavity scale is not finite (a density ratio of 10⁻⁴⁰⁰ against a
    speed's square of 10³¹⁰), so that no NaN reaches the density; and
    `BuildFieldHeaderError::SurfaceOlderThanSystem { surface, system }` (in `Gigayears`), so that
    T7.b's chronology cannot panic on a decoded header, with the sim's
    `BuildCoarseInputsError::SurfaceOlderThanSystem` refusing the same inputs before the coarse pass
    runs (`planetary/surface/inputs.rs`). `TargetClass` is a closed set of two scaling classes,
    appended only (for the composition audit; above 60% porosity μ falls to about 0.35, Risks).
  - _The wire._ The contract appends the projectile density, the target (its class's byte, then its
    density) and the system's age, 25 bytes, after the impact velocity; the atmosphere's tag carries
    its column mass alone, 8 bytes fewer. `wire/codes.golden` extends by the two class codes.
  - _The map._ Each class's exponents come from its μ (2μ ÷ (2 + μ) = 0.4314 and μ ÷ (2 + μ) =
    0.2157 non-porous, 0.340 and 0.170 porous), with each class's own transient constant, a
    paraboloid as deep as its fresh bowl (3.764 and 4.239, F4.2): F4.2's `TRANSIENT_DEPTH` is
    `shape::fresh_bowl_depth(class)`, the bowls' own 0.20 and 0.14, so the volume the map converts
    is the bowl the profile draws, and the regolith's shallowing never enters C. The lunar cavity is
    a power of the body's, (2 + μ) ÷ (2 + μ☾): exactly 1 for a non-porous body, whose form is the
    pure scale P☾ ÷ P (pow(x, 1) is x in the pinned libm; `a_non_porous_map_is_a_pure_scale` pins
    it). At the lunar reference the map is none at all, so the identity holds to the bit, screened
    or not. Against Ivanov's Mars the map lies within 3.9% at his velocity ratio and 9.9% at 10.6
    km/s, at the test's 20 diameters a decade (4.2% and 10.2% sampled continuously; T7.a's rounded
    0.43 and 0.22 gave 3.3% and 9.2%); the test normalises both at 1 km itself, since the production
    no longer does.
  - _Lunar-equivalent densities (F1)._ The production is N☾ at the lunar image of D, not
    renormalised; `n_1km` is the body's own N(>1 km) only at the identity, which T7.a's test, now
    `n_1km_is_the_parameter_exactly_at_the_identity`, checks, and every mapped body's production at
    1 km is `own_density_at_1km`. T7.a's identity-and-monotonicity test splits into
    `the_map_is_the_identity_at_the_lunar_reference_exactly` (bit-exact, screened or not, where it
    held 10⁻¹¹) and `the_map_is_monotone_and_inverted_for_both_classes`.
  - _Venus (F4.1)._ At N☾ 3.36 × 10⁻⁴ km⁻², 25.7 km/s and 20 d\* the built counts lie −1.8, −2.5,
    +5.5, +10.8 and +10.3% from the Gazetteer's at 3, 5, 10, 20 and 40 km (the test: 12%, pinned at
    11%); its own N(>1 km) is 1.79 × 10⁻⁴ (0.532 of N☾) and its screened density at 1 km 1.93 ×
    10⁻⁶, 1.1% of it: its test now holds it under 2% of its own density and in 1.8–2.1 × 10⁻⁶, where
    T7.a's held it under 1% of a production then equal to N(>1 km). T7.a's unmapped check at 3.07 ×
    10⁻⁴ km⁻² stays, as `…_without_the_map`, now at the taper's 4.2: within 2.8%. The taper-share
    test takes F1's 0.5, 4.5, 25, 46 and 94%. Mars's own N(>1 km) at Ivanov's velocities is 0.2560
    of N☾, and R_b 1.92 times it 0.4915, 0.3% under Ivanov's 0.4928.
  - _Charon (F4.2)._ The built impactors for a 10 km simple crater are 1.372 km non-porous and 1.594
    km porous, and the class constants 3.7641 and 4.2394. Charon's 0.288 m s⁻² is GM ÷ R² from
    Brozović et al. 2015 and Nimmo et al. 2017 (from memory), which Singer et al.'s supplement does
    not state.
  - _The shapes._ The porous bowl is 0.14 D, or the airless regolith's where that is shallower, so
    its a lies in 0.73–0.913. F2's peak gives a = 1.326 at 1.5 `D_t`, 1.302 at 9 and 1.323 at 11, so
    T7.a's transitional bracket of 1.33–1.543 becomes 1.32–1.543. The ramps' weights are exactly 0
    at and below their lower bounds and 1 at and above their upper ones, so the relief is Pike's law
    itself over 1.5–9 `D_t` and zero beyond 13. Continuity across 0.8, 1.5, 9 and 13 `D_t` is held
    to 10⁻⁹ of their own values for d, h and a, and of D for the floor radius and the peak relief,
    which start from zero at a bound, on four bodies, a porous one among them. Added tests:
    `porous_bowls_are_the_asteroidal_ones`, `the_shape_is_continuous_at_every_zones_bound`,
    `the_central_peak_ramps_in_and_out_on_pikes_law` (the base against the floor over 0.1–100
    `D_t`), `a_non_porous_map_is_a_pure_scale`, `charons_impactors_are_singers`,
    `an_icy_target_widens_a_crater_by_its_density`, `mars_own_density_implies_ivanovs_impact_rate`,
    `target_classes_have_their_codes`,
    `malformed_wire_crater_contracts_are_refused_with_their_errors`,
    `a_field_header_refuses_a_surface_older_than_its_system` and the sim's
    `the_reference_crater_contracts_carry_their_impactors`.
  - _The reference worlds._ `reference.rs` names `SILICATE_TARGET` (2,550 kg m⁻³), `CERES_TARGET`
    (1,287), the three velocities and `MARS_IMPACT_RATE` (1.92), and its `crater_params` takes an
    `Impactors` (target factor, velocity, target, rate). The reference Moon's gravity, GM ÷ R² =
    1.624 m s⁻², is not the map's 1.62, so its own density lies 0.2% under its N☾. It is not the
    identity to the bit; the synthetic Moon-like world, at 1.62, is.
  - _The synthetic worlds_ take the velocities too, as the task's list has it (F1's "testing worlds
    have no impact velocity" described T7.a). The Moon-like world sits at the reference; the Earth-,
    Mars- and Ceres-like worlds are mapped, with Ceres at 1,287 kg m⁻³. Their N(>1 km) values stay
    as they were but now read as N☾, with no `R_b`, so the mapped worlds' own densities are N☾ times
    own/N☾; no test pins those.
  - _Goldens._ `tests/golden/wire/payloads.golden` and `codes.golden` are re-blessed at 21 with no
    bump under Generator version's rule, and `tests/golden/craters/density_and_shapes.golden` under
    F1's and F2's "Moves" (no bump before this plan's first, T9's or T16's; "main", 2026-10-11).
    `payloads.golden` moves by the layout (block 0 by +25 bytes, the Earth- and Mars-like worlds'
    atmospheres −8 and their new velocities +8, the Moon- and Ceres-like worlds' velocities +8) and
    by F2's shapes through the testing builder's relief and reaches: the Mars-like world gains 100
    bytes of reach ranges and the Moon-like loses 8, and every digest moves with the cells'
    elevations. The crater golden's Venus is now the mapped one at 3.36 × 10⁻⁴; it adds a porous
    Hyperion-like and an icy, thick-aired Titan-like body and each body's own N(>1 km). Its
    saturation and chronology lines, the Moon's densities and the cutoff body's production are
    unchanged. No other golden moves.
  - _F3_ (the docs pass's, T7.e if earlier). `NEUKUM`'s doc and the chronology test's comment take
    F3's text.
- **Deviations in T7.b, as built** (2026-10-13; main's rulings A and A2 of that day).
  - _Files and public items._ `synth/craters.rs`, `synth::craters`, with tests in
    `synth/craters/tests.rs` and its refusals in `tests/panics.rs`'s `small_craters`;
    `synth::SmallCraters` is re-exported.
    - Items beyond Provides: `SmallCraters` (`new`, `seed`, `octaves`, `craters_in`,
      `craters_reaching`, `at`), `SmallCrater`, `CraterCache`, `CraterOctaves` (`new`,
      `for_body`, `octaves`, `unresolved_variance`, `structure_function`), `CraterOctave`,
      `CraterProfile` (`new`, `filleted`, `at`, `relief_factor`, `compensation`,
      `fillet_half_width`), `ProfileSample`, `band_half_width`, `diffusion_length`,
      `relief_factor`, `cell_area`, and the craters' own `unresolved_variance` and
      `structure_function`. Constants: `SMALLEST_DIAMETER` (2 m), `MAX_EXTERIOR_WIDTH` (1.543),
      `DIFFUSIVITY_M2_PER_MYR` (5.5), `DEGRADATION_B` (0.15) and `MAX_PLACEMENT_ATTEMPTS` (64).
    - In `craters`: `CraterShape::cavity_volume_m3`, and the crate-private `relief_and_slope`
      (`relief` is unchanged to the bit, as the crater golden shows), `moment`, `floor_fraction`
      and `peak_fraction`. `GAUSS_LEGENDRE_8`, the chronology's three constants and
      `geometry::min_rate` become `pub(crate)`.
    - Shared `synth` changes: `pub mod craters`, the re-export, and `UnresolvedVariance` gains
      `craters`, with `total` (no longer `const`) summing both. T5's
      `unresolved_variance_falls_with_the_wavelength_to_zero_at_the_band_limit` now asserts
      `total` = `relief` + `craters`. The `surface.crater` tag's doc gives its word layout.
  - _`profile`._ Provides' `craters::profile(r, crater, band: BandLevel) -> (f64, f64)` is
    `synth::craters::profile(r, crater, band: Metres, diffusion: Metres) -> ProfileSample`. A level
    names no width without the figure, and the crater's age enters as its diffusion length.
    `CraterProfile::new(shape, band_half_width(figure, level), diffusion_length(age))` is what
    T7.c builds on.
  - _The octaves._ Octave k runs from `D_b` ÷ 2^(k + 1) to twice that, down to the one that
    straddles 2 m. That last octave draws its whole octave and keeps the craters of 2 m and wider,
    so that nothing between 2 m and the last lower diameter goes undrawn (the plan-conformance
    review's finding). There are 16 octaves on the Earth-, Moon- and Mars-like worlds and 15 on
    the Ceres-like one; the Earth-like world's last octave runs from 1.3 m and keeps 2 to 2.6 m.
    - Each octave is drawn on the deepest level whose narrowest cell holds its reach of 2.543
      lower diameters within 0.98 of the cell's width. That width is S2's `kMinWidth`, 2√2 ÷ 3 ×
      2⁻ⁿ, a bound at every level, taken as a chord on the smallest radius.
    - The levels deepen by one per octave: on the Moon-like world from level 3 to 18, on the
      Earth-like world from 5 to 20. A level names its octave in the cells' keys, and an assertion
      holds the levels strictly increasing.
  - _Counts, places, ages._
    - Saturation caps each octave on the differential: min(N(>D) − N(>2D), 0.079 (D⁻² − (2D)⁻²)).
      τ is the octave's own window.
    - Counts are Poisson on `surface.crater` keyed by `surface_cell(…, 0)`, with the cell's true
      area on the spheroid by Gauss–Legendre to 10⁻⁹.
    - Centres are uniform in the spheroid's area, by rejection with a `Mark` against
      `Threshold::from_ratio` (the determinism review's should-fix). At most 64 candidates are
      drawn; acceptance runs from 0.28 on level-1 cells to over 0.92 from level 12.
    - Then come a diameter word, by `diameter_in_octave`, and an age word.
  - _The band in the fillet, the age in g_ (ruling A2).
    - The fillet's half-width is the level's band, ½ max(2 m, 4 × its largest vertex spacing),
      which is R05's octave rule. It is held at min(1, a) rim radii, and the compensation's r₁ is
      the reach, 1 + a.
    - Diffusion enters only through g = (0.15 ÷ (K ÷ R² + 0.15))², with K = κt. This is Minton et
      al. 2019's eq. 8 (Icarus 326, 63; arXiv:1902.07746, pp. 15–16) over its fresh depth, which
      the science check read in the PDF.
    - g scales the whole profile, so the volume stays zero and the reach unchanged. A fillet over
      the diffusion as well would round the rim twice, since eq. 8 already includes the rounding.
    - Eq. 8 is fitted over K ÷ r² from 0 to 0.3 on CTEM's crater profile. A 300 m crater at 3 Gyr
      (0.73) is therefore extrapolated beyond Minton et al.'s fitted range. It keeps 2.88% of its
      relief, against Fassett and Thomson 2014's ~7% and exact diffusion's 5.2% of this bowl. A
      1 km crater keeps 48%, against their ~52% and 64%.
    - The fillet alone, over √(w² + 3L²), had kept 55% for the 300 m crater (the science check;
      ruling A).
  - _κ_ is 5.5 m² Myr⁻¹ (Fassett and Thomson 2014, read in the abstract), used for every body.
  - _Distances_ are chords between spheroid points M·d. The cone's apex and base are not filleted.
  - _Closed forms._
    - They are body-wide, the same in every cell. They are built by
      `CraterOctaves::for_body(params, figure, D_b, S)`, which T12.e can call before a header
      exists.
    - Campbell's theorem gives each octave's variance at full resolution: the fresh profile's ∫f²
      times E[g²].
      - The diameters come from the octave's own inversion at eight Gauss–Legendre shares of the
        kept range. A power law at the same nodes had lain 12% high on Mars.
      - E[g²] over the window is closed where the production is linear, and the exponential part
        is taken by Gauss–Legendre. g² falls by orders of magnitude across a window, so a few
        young craters carry a saturated octave's variance. Nodes in the age's share had missed
        them by 120 times.
    - Against dense sums, E[g²] agrees to 0.1% and the octave variance to 1.1%. Against 6,000
      samples of an open lunar octave, the ratio is 1.05.
    - Each octave's variance lies on its kept diameters' wavelengths, with share
      clamp(ln(λ ÷ D₀) ÷ ln(D₁ ÷ D₀), 0, 1). The structure function is Σ s 2σ² (1 − J₀(2πb ÷
      √(D₀D₁))).
  - _Tests_ (26 in `synth::craters`, 12 in `tests/panics.rs`):
    - the size–frequency test: each world's octaves against Poisson intervals at α = 10⁻³ ÷ 63;
    - the face-edge test: ≥ 50 points across a face edge, plus a 5 × 5 brute-force check of the
      3 × 3 search near edges and a cube corner;
    - the balance test: 4,875 profiles, five contracts by 13 diameters, three ages and levels 0–24,
      each zero to 10⁻⁶ of the cavity volume by an independent five-point Gauss–Legendre;
    - the fillet's and the shape's moments, each to 10⁻⁹ of its quadrature;
    - the last octave's kept craters, bit for bit the whole draw's of 2 m and wider, so that a
      dropped crater still takes its words;
    - also g, the cavity volume, the placement's uniformity (χ² on the children of a level-1 cell),
      the gradient against differences along the surface, order independence over a shared cache
      and across bodies, and the cache's clear at 16,384 cells.
  - _Generator version._ Untouched (21). The craters reach `height_at` only at T8, and no golden
    moves (`golden_diff.py` clean).
  - _For T7.c._ Use `CraterProfile::new(shape, band, diffusion)` and its `at`, which returns a
    `ProfileSample` in metres and metres per rim radius, or `filleted(shape, w)` for the fillet
    alone. The compensation's closed form is `CraterShape::moment` and the Hermite moments. A
    coarse crater's g is the same at both bands, so its sharpening is g × (filleted at w_n −
    filleted at w_coarse).
  - _For T8._
    - `SmallCraters::at(dir, level, through, &mut CraterCache)`, with T8's choice of octaves per
      level. The `CraterCache` goes into `SynthCache`.
    - `CraterOctaves::new` costs about a millisecond, and up to some ten on screened bodies (eight
      `diameter_in_octave` per octave). The per-cell `unresolved_variance` and
      `structure_function` rebuild it, so R10 and T8 should hold one per body.
    - Bench the uncached cell draw: `diameter_in_octave` under screening is about 80 µs a crater.
  - _For T9._ Pin the small craters apart from `height_at`, as the relief is pinned. Per world:
    - the octave table (level, production, saturation, window, variance);
    - `craters_in` for a few cells, among them a face-edge and a cube-corner cell, and a cell of
      the last octave that draws a crater under 2 m, with that octave's `smallest` and kept share
      (count, then each crater's centre, diameter and age bits);
    - `at` heights and gradients at several levels near edges and corners (the determinism
      review).
- **Deferred findings (2026-10-13 rule)** (R09.T7.b; recorded, not ruled; each for "main"'s list).
  - _κ beyond its calibration._ Fassett and Thomson's 800 m–5 km, 3 Gyr mare value is applied:
    - down to 2 m;
    - held constant over a system's whole age, where a rate that follows the flux (Minton et al.
      2019, §1.4) would give a 4 Gyr crater about 18 times the degradation;
    - to every body.

    A per-body κ would be a contract change. The calibration against Rosenburg et al. 2011, Cai
    and Fa 2020 and LOLA at 171 and 546 m stays T8's.

  - _Eq. 8 extrapolated._ It is fitted to K ÷ r² ≤ 0.3, on CTEM's profile, not ours.
  - _The ring wavelength._ Each octave's variance sits on one J₀ ring at √(D₀D₁). The science
    check finds that this matches a fresh bowl's mean-square slope to 4% but overstates a
    degraded one's up to 2.1×. Fix: a λ_eff from E[∫h²] ÷ E[∫|∇h|²], one more moment of the
    same quadrature.
  - _The central cone's kinks_ (its apex and base) are not band-limited, against the brainstorm's
    "every contribution to height must be band-limited". They lie only on complex craters, those
    above 0.8 `D_t`.
  - _Placement's bound_ takes |M⁻¹d| ≤ 1 ÷ min(a, c). Acceptance is therefore at most c ÷ a,
    0.925 on the Ceres-like figure. A tighter bound would only save words.
  - _One density per body._ Small craters carry no regional ages (maria against highlands), no
    sedimentation under water and no ice cover (the earlier adjacent finding stands).
- **Deviations in T7.c, as built** (2026-10-13; main's ruling on R09.T7.c of that day, options 2,
  B and C).
  - _Files and public items._ `synth/craters/rims.rs` is `synth::craters::rims`, with tests in
    `synth/craters/rims/tests.rs` (10) and its refusals in `tests/panics.rs`'s
    `coarse_craters_rims` (8), so that the acceptance filter `craters` runs them. T7.b's test
    helpers become `pub(super)` for these tests. The items:
    - `coarse_band_half_width(&Spheroid, CoarseLevel) -> Metres` and
      `kept_relief(&CoarseCrater) -> f64`;
    - `RimSharpening`: `new(shape, band, coarse_band, relief)`, `fine`, `coarse`, `relief`,
      `is_zero` and `at -> ProfileSample`;
    - `CoarseRims`: `new(header)`, `for_body(params, figure, level)`, `coarse_band`,
      `coarse_profile(diameter)`, `sharpening(crater, level)` and
      `at(field, dir, level) -> Result<HeightSample, QueryHeightError>`.

    `CoarseRims::at` asserts that the field's header made the rims. `coarse_profile` and
    `sharpening` refuse a diameter that is not finite and positive, as `crater_shape` does. The
    refusals cover a kept relief outside [0, 1], a negative or NaN band, a negative distance
    (with and without a sharpening), no diameter, no direction, and another body's rims.
    `CoarseCrater`'s docs now say the reach is the chord from the centre's spheroid point, and
    that the degradation byte is the share of the whole fresh relief removed (ruling B).

  - _The sharpening_ at a level of band w is g × (F(min(w, w_c)) − F(w_c)), where F(w) is T7.b's
    `CraterProfile::filleted` of the crater's `crater_shape`.
    - It is zero to the bit wherever `band_half_width(level)` ≥ w_c. `CoarseRims::at` reads no
      crater there. It is also zero when both fillets are held at min(1, a) rim radii, or when
      g = 0.
    - It is radial in the chord between spheroid points (T7.b's distance), so it is continuous
      across every coarse cell edge and face edge. The tests check this bit for bit at cell-edge
      vertices: through both cells' indexes, and against the whole list in list order.
  - _Not zero per coarse cell (option 2)._ The task's premise that the difference "integrates to
    zero over each coarse cell" is false.
    - The compensation spreads over the reach, while the fillet's change sits on the rim.
    - The probe took fresh bowls of depth d on planar cells D_b ÷ 2 wide, a fine band of 0.004
      rim radii and the worst of three placements. Its worst cell mean is 0.28 d at D_b, 0.35 d
      at 2 D_b, 0.15 d at 4 D_b and 0.085 d at 8 D_b. The coarse craters of the Moon-, Mars- and
      Earth-like worlds are complex craters, which it did not take.
    - Forcing each mean to zero (option 1) needs a bump in every cell of about 3.5 times that,
      0.3–1.2 d, so it was not built.
    - The per-cell test is therefore the whole-reach test: zero to 10⁻⁶ of the cavity volume at
      levels 0–24. It covers every world's coarse craters and D_b × 1–32 at g 1 and 0.6, by an
      independent five-point rule on both profiles' pieces.
    - Verification's Consistency item asks that "every crater, and every coarse crater's
      sharpening, integrates to zero at every level (T7.b, T7.c, T8)". The brainstorm asks that
      "nothing may be added at a fine level that changes the mean at a coarse one", and that a
      crater's "profile must integrate to the same displacement at every finer level". Main ruled
      the rule met per crater, not per cell. T7.b's small craters keep no per-cell mean either.
    - Coarse-cell means, if they are ever wanted, are for T12.d and T4: they need prefiltered
      coarse values, since the B-spline base keeps no cell mean either.
  - _The coarse band (C)._ w_c = ½ max(2 m, 4 × the coarse level's largest cell edge on the
    equatorial radius).
    - It equals `band_half_width(BandLevel(L − 6))` to the bit at L ≥ 6. At level 5 it is twice
      level 0's band.
    - On a sphere it is D_b to 10⁻¹²: 92.6 km on the Moon.
  - _g (B)_ = 1 − degradation ÷ 255, the list's own byte. Minton's g of the age would keep 99.95%
    of the Ceres-like world's narrowest coarse crater (R 25 km, 4.5 Gyr), and more of every wider
    one. That is eq. 8 extrapolated: it is fitted to one simple crater, with κ calibrated on rim
    radii of 0.4–2.5 km. Exact diffusion of a paraboloid's centre keeps 99.98%.
  - _On the ground._ On a sphere the ground within a chord c is π c², so the profile balance is
    the ground's. The test integrates `CoarseRims::at` over the single-crater sphere in polar
    coordinates and finds zero to 10⁻⁶ at six levels. The isolated integral is zero to 10⁻¹²
    (2 × 10⁻¹⁶ measured). On oblate figures the integral is pinned as it stands (deferred
    findings, below).
  - _The testing builder's reach_ took the angle reach ÷ volumetric radius. That falls short of
    the widest angle a chord within the reach can span on every figure, by 2 asin(x ÷ 2) ÷ x
    with x = reach ÷ radius (2–12% on the Moon-like world's basins), and on an oblate figure
    also by (1 − f)^(−2/3). The continuity test found a crater missing from the Moon-like world's
    index.
    - It now takes 2 asin(reach ÷ (2 min(a, c))), plus the cell's circumradius as before.
    - `tests/golden/wire/payloads.golden` is re-blessed at 21. The new angle is never smaller
      than the old, so covers only grow, though a cover of more cells may take fewer ranges.
      Every cratered world's payload moved: the whole payloads by −64 bytes (Moon-like), −12
      (Mars-like), +8 (Earth-like), +128 (Ceres-like) and +32 (single crater); the Mars-like
      region by −8; and the Moon-like world in 48 KiB blocks by +1,618. The flat world and the
      margin lines did not move. The 2026-10-09 ruling lets a change to the testing worlds
      re-bless with no bump, so `golden_diff.py`'s "not bumped" is that exception.
  - _Generator version._ Untouched (21). No other golden moves, and nothing reaches `height_at`
    before T8.
  - _For T12.d (its contract)._
    - Choose each crater's `degradation` byte first, and smooth it in as its `kept_relief` ×
      `CoarseRims::coarse_profile(D)`, at the chord's rim radii. The coarse elevation and the
      sharpening then use the same g to the bit, and are one profile. The byte carries the
      crater's wear.
    - Write each `reach` to hold every cell that has a point within the chord (1 + a) R of the
      centre's spheroid point, as the builder now does.
  - _For T8._
    - Add `CoarseRims::at(field, dir, level)` at every level. It reads only the query's coarse
      cell and that cell's crater index (`NotSurveyed` without the cell), so the margin does not
      grow.
    - Per crater and query it costs one `crater_shape` and two fillets. Cache per (crater,
      level) if the bench asks for it, with an `assert_order_independent` test over a shared
      cache, as `CraterCache` has.
    - The testing builder's coarse elevation still adds each crater's fresh, unfilleted profile
      at the arc distance, with its degradation recorded but not applied. It must follow T12.d's
      contract before T8 sums `CoarseRims::at` on the synthetic worlds, or the rims sharpen
      twice. That moves the worlds' goldens, which T8 and T9 coordinate with main.
    - `level_bound_m`, `unresolved_rms` and `structure_function` must take the rims in. No closed
      form for them exists yet, and g (F(w_n) − F(w_finest)) can reach the rim height on large
      basins.
  - _For T9._ Pin the coarse rims apart from `height_at`. For each cratered world, pin
    `coarse_band_half_width`, and the bits of `CoarseRims::at` heights and gradients at levels
    L − 4, L + 2 and the finest. Take points at about 0, 0.97, 1.0 and 1.04 rim radii and just
    inside the reach, and the face-edge vertices the continuity test uses (the determinism
    review).
- **Deferred findings (2026-10-13 rule)** (R09.T7.c; recorded, not ruled; each for "main"'s list).
  - _Oblate ground balance._ A profile balanced in the chord integrates over an oblate spheroid's
    ground to a residual of about f² (c ÷ ρ)² + f (c ÷ ρ)⁴ (the science check). The test measures
    it over every listed crater:
    - within 10⁻⁶ of the cavity volume on the Moon-, Mars- and Earth-like figures (f 0.0012,
      0.0059 and 0.0034): at most 2.5 × 10⁻⁷, for the Moon-like world's 2,500 km basin;
    - up to 3.1 × 10⁻⁵ on the Ceres-like world (f 0.075), for its 280 km crater, whose reach
      spans 0.76 of the radius, and pinned at 5 × 10⁻⁵.

    T7.b's small craters share it, at smaller R ÷ ρ. A fix is a distance whose discs have area π r²
    on the spheroid, which would change T7.b and T7.c alike.

  - _The base's spline smooths further than F(w_c)._ The cubic B-spline approximant keeps two
    thirds of a wave four cells long at its knots, (2 + cos(π ÷ 2)) ÷ 3. A sharpened crater is
    g × F(w) only where the base reproduces g × F(w_c). T12.d could prefilter its coarse values.
  - _The cone's kinks return unfiltered_ in the sharpening wherever the coarse fillet is held at
    min(1, a) rim radii (D below about 2 D_b), which extends T7.b's finding to coarse craters.
  - _Per-cell means and the brainstorm._ Main's option 2 rules the task. The brainstorm's
    sentence "nothing may be added at a fine level that changes the mean at a coarse one" is
    read per crater, and is not itself ruled on.
- **Deviations in T6.b, as built** (2026-10-13; lane r09-channels).
  - _Files and public items._ `synth/channels/incision.rs`, a public module (tests in
    `incision/tests.rs`), re-exported through `synth::channels`:
    - `Channels::incision_at(field, dir, through, cache)`, the incision's height and body-fixed
      gradient for levels L to `through`, and `Channels::network_in(field, cache)`, the network
      memoised in the caller's `SynthCache`; `Channels::body()`;
    - `ChannelBody { radius, level, gravity }` (`of(header)`), `valley_depth`, `deepest_level`,
      `channel_head_area`, `channel_variance` and `channel_structure_function`;
    - `CHANNEL_HEAD_AREA`, `DEBRIS_FLOW_SLOPE`, `CUT_WIDTH`, `VALLEY_WALL_SHARE`,
      `VALLEY_WALL_GRADIENT`, `MEAN_SAMPLES`, `TRIBUTARY_SHARE`, `MEAN_TRIBUTARY_LENGTH`,
      `MEAN_TRUNK_LENGTH`, `TRIBUTARY_VARIANCE_SHARE` and `WHOLE_VARIANCE_SHARE`.

    `channels.rs` gains `mean_discharge_m3_s`, `channel_forming_discharge_m3_s`,
    `bankfull_width` and `bankfull_depth` (both taking the body's gravity),
    `tributary_steepness_m0_9`, `BANKFULL_RATIO`, `BANKFULL_RATIO_AREA`,
    `BANKFULL_RATIO_EXPONENT`, the three `BANKFULL_*` Parker constants and `REFERENCE_GRAIN_SIZE`.
    `Segment` gains `mean_discharge_m3_s`, `bankfull_width`, `bankfull_depth` and `below_top`
    (176 bytes, was 136). `Channels` holds the header's body, gravity, sea level, realised `σ_h`
    and relief, and `network` refuses a field whose header differs in any of them.
    `UnresolvedVariance` gains `channels()`, and `total()` sums relief, channels and craters in
    that order. `SynthCache` holds a crate-private `ChannelCache`. `interp::{axes, warp_slope}`
    and `relief::as_function_of_direction` become `pub(crate)`. A new
    `benches/synth.rs` (`required-features = ["testing"]`).

  - _The incision model_ (Design note 13's "valley cross-profile from the distance to the
    channel", the relief from the profiles). Each evaluated segment cuts a groove: a flat floor of
    half its bankfull width, then a cubic-smoothstep wall over W, half the level's mean cell. Its
    depth at the axis is G = min(max(β, d), `G_max`), with d the bankfull depth and `G_max` the
    depth whose steepest wall, 1.5 G ÷ W, is 0.6.
    - β is the bed's depth below its trunk's valley top. A trunk's valley is `valley_depth`: the
      stream-power rise from the channel heads to one mean coarse cell's Hack length, at the
      tributaries' `k_s` (r̄ ÷ r)^θ. Tributaries inherit the top where they join, so their grooves
      shallow upstream as their beds rise.
    - Levels combine by maximum into `E_k`, and level k's incision is −(`E_k` − `E_{k−1}`), so a
      groove inside a deeper one cuts nothing and confluences have no pits.
  - _The mean._ Level k's mean is over its parent cell, and the first level's over its own coarse
    cell, not an L − 1 parent, so that the coarse cell's mean elevation is kept. It is the 8 × 8
    midpoint sum weighted by solid angle. What is subtracted is a correction with that mean,
    continuous across cells: bilinear between corner means (three or four cells, summed in
    packed-key order) plus a bubble zero with its gradient on every edge. A constant per parent
    would step at every edge. The correction is C⁰ across cells.
    - The task's 10⁻⁹ m holds on the defining grid (a test, over 60 parent cells including face
      edges). On an independent 24 × 24 grid the mean is up to 5.6% of the level's RMS over a
      parent cell, and on 48 × 48 up to 2.2% over a coarse cell (tested at 8% and 4%).
  - _The search._ The network's joins keep T6.a's 3 × 3, unpruned (the memo amortises them). The
    groove lists, per cell and level, gather segments headed within two king moves (5 × 5), each
    with a cached box, pruned exactly. A groove reaches half a cell beyond its segment and a trunk
    ends at its receiver's key point, so from a 3 × 3, 22–48% of tributaries and 92–100% of trunks
    (the share longer than 0.75 cells) could be cut off at the neighbourhood's edge. From the 5 × 5
    only segments longer than 1.75 cells are: none at L to L + 2, 0.09–0.52% at L + 3 and L + 4,
    where the height steps (the continuity test found under 1 cm on 13 cells).
  - _The cut_ (per coarse cell). Level k > L is evaluated where a channel draining one mean cell
    of the level at the cell's own runoff is at least 4 m wide (two band limits), and the level's
    mean cell is at least the heads' drainage, max(10⁴ m², (`k_s` ÷ 0.1)^(1/θ)). The trunks are
    always evaluated. Measured before fixing it (closed form, 40 cells a world):
    - the deepest levels: Earth-like 9: 1, 10: 5, 11: 9, 12: 18, 13: 7; Mars-like 7: 1, 8: 1,
      9: 4, 10: 5, 11: 29;
    - an 8 m cut, Design note 13's four band limits, would remove a median 24 m RMS of valleys
      (largest 206 m) on the Earth-like world and 26 m (44 m) on the Mars-like, resolvable relief
      and within budget to keep, so the cost lever is not taken;
    - the 4 m cut leaves out a median 18 m RMS (largest 54 m) below it down to the heads on the
      Earth-like world and 14 m (21 m) on the Mars-like (deferred, below).
  - _Hydraulic geometry._ Parker et al. 2007, eqs. 10a and 12, at D50 0.073 m and the body's own
    gravity, from `header.craters().gravity()`. The channel-forming discharge is the bankfull flood,
    55 (A ÷ 5 km²)^−0.25 times the mean, held outside 5.4 km² to 2.964 × 10⁶ km²: a least-squares
    fit by this task's science check to the 1.5-year floods of seven USGS gauges, not published,
    **drafted for sign-off**. The plan's other candidate, the mean over the 5% reference
    intermittency (20×), was 2.5–8 times too high above 10⁴ km² (must-fix of that check).
  - _Closed forms._ Per level, from measured densities and lengths (0.75, 0.65 and 1.0 cells), a
    finer level's realised share of 0.46 and the whole's 0.52 of the levels' summed variance. The
    levels anticorrelate: a finer level cuts most where the coarser cut least. Each level counts in
    the band from its mean cell's width to twice it. The whole incision's realised variance is
    within ×2 of the closed form on the median of eight cells a world (measured 0.45–2.3, with one
    Mars-like cell at 35 where its neighbours' valleys dominate). The structure function scales
    every level's term by the same 0.52, an approximation at small lags.
  - _The cache_ is identified by body, seed, level, radius, figure, gravity, sea level and
    realised `σ_h` and relief, not by the field's records. It serves views of one field whose held
    cells only grow, and is never pruned; T8 sets the policy. Cells are built lazily as queries
    reach them, not by a separate patch build. Hot-path state: per-level arrays on the stack, the
    groove lists by index with the last per level, and the last correction per level.
  - _The chord's sag_ is accepted, as T6.a allowed: it narrows only a floor narrower than its
    25 m.
  - _Read set._ The incision through L + 4 reads cells up to four king moves from the query's
    coarse cell, one inside the margin's five, at face edges too (tested exactly).
  - _Tests beyond the list._ The mean on a finer grid; a cache started afresh for another body
    or seed; `network_in` equal to `network`; each segment's discharge, width, depth and depth
    below the top; Parker's figures, the gravity scaling and the ratio; the cut; the trunk's
    valley; continuity across cell and face edges; the gradient against a central difference (97%
    of 640, off the C⁰ edges); the read set; the variance member; the structure function's
    limits; the closed form against the whole. One ignored measurement. Seventeen `should_panic`
    tests in `tests/panics.rs` (`mod incision`), which gains `level_five_field_of(body, flow)`.
  - _Acceptance._ `cargo test -p hyperion-surface synth::channels` selects 33 unit tests (T6.a's
    15 and T6.b's 18) and 1 ignored. The crate's whole suite, `--features testing`: 335 library
    tests, 106 panics, every golden unchanged. No golden reaches the channels, and `height_at` does
    not include them until T8, so `GENERATOR_VERSION` stays 21.
  - _The bench_ (`surface/channels/through-<k>/{cached,bake}`, a 65 × 65 patch of the Earth-like
    world's level 12), run under the heavy lock at a load of 2.0, schedutil, 1200% quota:
    cached 0.36, 0.47, 0.57, 0.69 and 0.82 µs a point through L to L + 4, about 0.11 µs a level;
    a cold bake 0.43, 0.88, 1.18, 1.53 and 2.09 µs. At WebAssembly's 1.2–1.5× that is 1.0–3.1 µs,
    inside Design note 14's 3.5 µs. Provisional until a fully quiet run.
  - _For T8._ `incision_at`'s `through` sets each band level's channel levels. The read set is
    four king moves. The closed forms are ×2 estimates, against T8's 10% structure-function check.
    Steep cells' valleys reach about 4 km (Mars-like `k_s` 609), which bears on T8's slope bands.
    `height_at` must never treat a `ReadCellError` as zero incision, or the cache would leak into
    heights.
  - _For T12.e and T14.c._ `channel_variance` takes a `SynthesisCell` and a `ChannelBody`. No
    closed-form incision volume is exposed: T14.c builds it from `valley_depth`, `deepest_level`
    and the hydraulic geometry. A trunk's valley spans its whole coarse cell (2W).
  - _For T9_ (the determinism review). No golden pins the channels yet: T9's points should include
    channelled cells evaluated to L + 1 … L + 4, at face edges among them, and pin the bankfull
    width and depth and `channel_variance`.
- **Deferred findings (2026-10-13 rule)** (R09.T6.b; recorded, not ruled; each for "main"'s list).
  - _The cut by channel width removes resolvable valleys:_ a median 18 m RMS (largest 54 m)
    below the 4 m cut down to the heads on the Earth-like world, 14 m (21 m) on the Mars-like.
    Evaluating to the heads alone would cost about 0.11 µs a level more (three or four levels on
    an Earth). Lean: cut at the heads alone. That changes the task's rule, so it waits for a ruling.
  - _The bankfull ratio is a drafted fit_ to seven humid US gauges, used on every world, a Mars's
    episodic floods included. It needs the sign-off agent, and a published relation if one exists.
  - _Parker's Q̂ range_: ¶12 gives four and a half decades, but the 62 parsed reaches span 3.7
    (242 to 1.3 × 10⁶). Re-derive from all 72.
  - _Bed and liquid_: Parker's fit is for quartz gravel in water (R ≈ 1.65), and no grain size
    is on the field. Ice in methane is an extrapolation.
  - _The mean's quadrature_: a level's mean over a coarser cell is off by up to 2.2% of its RMS
    (metres on a trunk level of 150 m RMS). Whether that meets the brainstorm's "nothing may be
    added at a fine level that changes the mean at a coarse one" is for a ruling; 16 × 16 would
    quarter it at four times the means' cost.
  - _The closed forms are ×2_ and use 0.46 and 0.52, measured on two synthetic worlds; the
    structure function's small-lag slope is the levels' scaled alike. T8's 10% check needs a better
    form, from the realised β and the levels' covariance.
  - _Clipped grooves_: 0.09–0.52% of segments at L + 3 and L + 4 reach past the 5 × 5 and step
    there.
  - _No drawn channel_: the bankfull channel sets only the least valley depth and the floor's
    width; it is not cut into the valley floor. Whether "resolvable, authoritative channels"
    (Design note 13) need it drawn is for the science check.
  - _The wall gradient_ 0.6 is a design cap (Godard et al. 2020's critical gradient), uniform on
    every body and in every climate; low-relief valleys have gentler walls on real land.
  - _The cache's identity_ is the header's marks, not the field's records: two coarse passes of a
    body with the same sea level, `σ_h` and relief would share it.
  - _T14.c's bound_ assumes a 10 km valley in a 38 km cell, but T6.b's trunk valleys span the
    whole coarse cell (2W), and steep cells' valleys reach about 4 km (`k_s` 609 m^0.9 on the
    Mars-like world), which may also push T12.e's local share. For the science check.
- **Deviations in T8, as built** (2026-10-13; lane r09-assembly, after T6.b, T7.b and T7.c; main's
  acceptance of that day of the bound's quantile, the structure function's reading and the bench's
  miss).
  - _Files and public items._ Provides' `synth/mod.rs` is `synth.rs`, with two new public modules
    and a crate-private one:
    - `synth::levels`: `LevelSet` (`level`, `shortest_wavelength`, `relief_through`,
      `channels_through`, `craters_through`), `LevelBounds` (`new`, `set`, `bound_m`,
      `contribution_range_m`), `level_bound_m`, `shortest_wavelength` and `CRATER_BOUND_TAIL`;
    - `synth::bake`: `PatchHeights` (`key`, `samples`, `sample`, `parent_heights_m`,
      `parent_height_m`) and `PARENT_VERTICES`;
    - `synth::craters::bound` (crate-private): each crater octave's and coarse-crater octave's
      shares in the bound, and Chernoff's bound on a sum of Poisson counts.

    `Synthesiser` gains `relief`, `channels`, `craters`, `rims`, `bounds`, `level_set`,
    `level_bound_m`, `bake_patch`, `height_range_m`, and `unresolved_variance`,
    `structure_function` and `unresolved_rms` from the octaves it holds. The free
    `structure_function` and `unresolved_rms` are Provides'; each free closed form builds the body's
    `CraterOctaves` (1–10 ms) per call, which the methods do not. `SynthCache` holds T7.b's
    `CraterCache`. `UnresolvedVariance` gains `rims()`, and `total()` sums relief, channels, small
    craters and rims in that order. Beside them: `relief::relief_structure_function`, the
    incision's `groove_cap_m`, `deepest_possible_level` and `BUBBLE_MEAN_FLOOR` (0.284), the rims'
    `unresolved_rims`, `rims_structure_function` and `kept_relief_of`, all crate-private;
    `CraterOctave` stores its craters' mean reach area from the variance's own nodes, which move no
    variance bit.

  - _`Synthesiser`_ is `Clone`, not `Copy`, and `new` is not `const`: it holds the body's
    `Relief`, `Channels`, `SmallCraters` (one `CraterOctaves` a body, as T7.b's handoff asks),
    `CoarseRims` and `LevelBounds`. Building one costs about 1–10 ms, so a caller builds one per
    body and view. T4's `Copy` test is a `Clone` test.
  - _No `QueryHeightError::NonFinite`_ (Provides' comment): every part asserts its own heights
    finite and `height_at` asserts the sum, a non-finite height being a bug, as the brainstorm's
    "asserted finite before they are … emitted" has it. No later plan names the variant.
  - _The sets._ A band level n resolves wavelengths down to λ_n = max(2 m, 4 × its largest vertex
    spacing on the equatorial radius), R05's rule held at the band limit. Octave m is evaluated
    where its lattice spacing is at least λ_n (through n + 4), channel level k where its key
    points' spacing, a mean cell of level k, is (through n + 3, T6.b's band for that level), and
    a small-crater octave where the smallest diameter it keeps is. The coarse rims sharpen at every
    level. From the shallowest level whose λ_n is the band limit (19 Earth-like, 18 Mars-like,
    17 Moon-like, 15 Ceres-like, each at or above R05's `finest_level`) every level evaluates the
    same set with the same band, so its heights are the finest's to the bit (tested).
  - _`height_at`_ sums the base elevation, the octaves, the incision, the small craters and the
    coarse rims, in that order, height and gradient alike. The base elevation reads first, so
    T4's `a_cell_not_held_is_not_surveyed` holds for the whole function.
  - _The level bound, as built and accepted._ Hard for the structural and channel terms; the
    crater terms, the small craters' and the coarse rims', a 1 − 10⁻⁹ per-point quantile.
    - The octaves: Σ `octave_bound` over those n leaves out.
    - The channels: (1 + 1 ÷ B) Σ `G_j` over the levels n leaves out to the deepest a body's
      cells can reach, `G_j` the groove cap 0.2 of the level's mean cell and B the bubble's mean
      floor, since the raw incision of the levels from k on lies in [−`G_k`, 0] and each level's
      correction is bilinear in [−`G_j`, 0] plus a bubble within ±`G_j` ÷ B (the science check
      confirmed the derivation, and the tighter form without a separate `G_k`).
    - The craters: a cell's count is a Poisson draw, so their overlap has no largest value. Per
      octave, one crater's share (its fresh height plus its fillet's change where the octave is
      new; its fillet's changes at both bands where both levels evaluate it, by the triangle
      inequality; nothing where the bands are equal) with its expected count reaching a point, the
      octave's density times its craters' mean reach area by its own inversion, summed by
      Chernoff's bound to 10⁻⁹. The rims add each coarse octave from `D_b` to π R at Trask's
      saturation over the widest reach of its largest crater, with their sharpening's change
      between the bands held to the coarse band.
    - `height_range_m` inherits the quantile through `contribution_range_m`, and adds the base
      elevation's range over the held cells within five king moves of the coarse cells the patch
      covers (any encodable elevation where none is held).
    - It is zero from the band limit's level, and holds on every synthetic world at 10³ points
      (fast) and 10⁴ points (slow, `#[ignore = "slow: …"]`) for every pair of levels. It is very
      loose: the worst |h(n) − h(m)| ÷ `level_bound_m(n)` over 10⁴ points is 0.016–0.063
      Earth-like, 0.002–0.064 Mars-like, 0.005–0.066 Moon-like, 0.006–0.058 Ceres-like and
      0–0.014 on the one-crater world (R10's expected 99.9th percentile is 0.3–0.5). The bound at
      L + 4 is 542 m Earth-like (relief 125, channels 127, craters 290), 884 m Mars-like, 1.08 km
      Moon-like (898 m of it craters) and 319 m Ceres-like (deferred, below).
  - _The bake._ `PatchHeights` is what R05's bake reads of a source, not `PatchBake`: the 65 × 65
    vertices' height and gradient at the patch's level and the parent level's height at the
    33 × 33 even vertices (none at level 0), each `height_at`'s to the bit, the lattice covered
    once about the patch first. The double-resolution normals' gradients are R10.T6.a's to ask of
    the source. For R10.T3 and T6.a.
  - _The closed forms._ `structure_function` sums the relief's per-octave form (item 7.3: Σ (a
    `g_m`^b)² `V_m` [cos² `θ_m` `D̂_p` + sin² `θ_m` `D̂_r`], an octave partly finer than the band at
    its share of the variance), T6.b's channels', T7.b's small craters' and the rims'. The rims'
    variance is each listed crater's crater-wide mean square, g² ∫ (F(w₂) − F(w))² dA over its
    reach's disc, w = ½ λ held to the coarse band and w₂ the band limit's, and its structure
    function 2V (1 − exp(−s Δ² ÷ 2V)) with s its slopes' mean square for a random direction.
    Sampled over every cell the one-crater world's crater reaches, the variance is the closed
    form's to 0.03% and the structure function within 0.81–0.98 of it at 1, 10 and 50 km (tested
    at 10% and 25%). `unresolved_rms` is the square root of the total, zero at 2 m and below and
    non-increasing as the resolution refines, on every world (tested).
  - _The structure function's 10%, as read and accepted._ Asserted on the relief per octave on
    belt and plain cells of the Earth-like world (sampled ÷ closed 1.011, 1.025, 0.998 on belts and
    0.971, 1.012, 1.046 on plains, at 10 m, 100 m and 1 km) and on the whole function on belt
    cells (1.038, 1.052, 1.039). On channelled plain land the whole is 1.155, 1.293 and 1.370,
    from T6.b's channel form, 3–25 times low per cell (deferred, below).
  - _A full belt's slopes_ (12 cells of the Earth-like world whose 5 × 5 is full belt, 2,000
    points a baseline, the whole function at the band limit's level): RMS 37.5° at 10 m, 36.7° at
    100 m and 26.6° at 1 km, mean 31.0° at 100 m, 0.286 of the area above 40° at 10 m; all inside
    the task's bands.
  - _The read set_ of the whole function, at every sampled level (0, L, L + 4 and the band
    limit's) from a fresh cache per cell, on every world, face edges, a cube corner and coarse
    rims included: at most 4 king moves at the sampled points, within the margin's 5 (T4's 5 lies
    near the ends of face edges, which T4's own test sweeps). T6.b's clipped grooves (0.09–0.52% of
    segments at L + 3 and L + 4 reaching past the 5 × 5) step between neighbouring points, not
    between levels, so neither this test nor the level bound's sees them; they read nothing more.
  - _The testing builder_ holds each coarse crater as T12.d's contract has the coarse pass smooth
    it in: its kept relief, `rims::kept_relief_of` of its degradation (one expression with
    `kept_relief`), times `CoarseRims::coarse_profile(D)` at the chord between spheroid points in
    rim radii. So the synthetic worlds' rims sharpen once. `tests/golden/wire/payloads.golden` is
    re-blessed at 21 under the 2026-10-09 testing-world rule, with no bump: the digests of every
    cratered world (Earth-, Mars-, Moon- and Ceres-like, the one-crater world, the Mars-like region
    and its delta, the Moon-like world in 48 KiB blocks) moved, no byte count or held range did,
    and the flat world's did not.
  - _Tests changed._ T4's `the_base_elevation_is_the_same_at_every_level` becomes
    `with_nothing_finer_than_its_cells_the_height_is_the_base_elevation_at_every_level` (the rough
    field); T4's order test keeps the base elevation, and the whole function's moves to
    `synth::tests`; T6.b's `segments_carry_…`'s bit equality of the groove's top, the top less the
    bed plus the bed, is 4 ulps relative (an ulp apart once the builder moved the worlds); T5's
    and T6.b's total-variance checks add the rims. The synthesis's inline tests move to
    `synth/tests.rs`. The bubble floor's test holds every cell at levels 5 and 6 and sampled cells
    to 24 (a one-off sweep of every cell to level 8 gave 0.284 565 to 0.284 713).
  - _Tests added._ In `synth::tests`: the sum in order; order and cache independence; the read set;
    a cell with its margin held answers as the whole field, bit for bit; the bake against point
    queries and over patch build orders (`assert_order_independent`); the range holds every baked
    height; `unresolved_rms`; the free closed forms and bounds are the synthesiser's, bit for bit;
    the rims' closed forms; the structure functions; the belt's slopes. In `synth::levels::tests`:
    the sets, the bound's zero and monotonicity, the 10³-point bound and its slow 10⁴. In
    `synth::craters::bound::tests`: Chernoff against exact Poisson tails and the octaves' and
    coarse octaves' marks. Four `should_panic` tests in `tests/panics.rs`'s `synth_assembly`. Three
    `#[ignore]` measurements run by hand (the bound's terms, the structure function by
    contribution, the Moon-like world's slopes).
  - _Acceptance._ `cargo test -p hyperion-surface synth` and the crate's whole suite with
    `--features testing`: 366 library tests (20 ignored), 118 panics and the goldens pass; the
    slow bound test passes by name in 177 s (dev profile, with the measurements).
  - _The cache's policy._ Each bake sets the lattice cover afresh and the craters' part is bounded
    at 16,384 cells, but the channels' part is never pruned, so a caller drops the cache when it
    moves on (a worker between descents; the server's collision cache by the same rule).
  - _The bench_ (`benches/synth.rs`; provisional: load 9–12, schedutil, a 400% quota, single
    thread, release): on the Earth-like world, per point at L + 4 (level 12) base elevation
    1.66 µs, relief 8.41, channels 0.90, small craters 2.78, the whole 14.0 µs, and a patch
    (4,225 points and 1,089 parent heights) 76.8 ms warm, 81.8 ms cold; at level 19 base 2.87,
    relief 14.6, channels 0.81, small craters 5.77, the whole 26.4 µs, a patch 128 ms warm and
    169 ms cold. Against Design note 14's WebAssembly targets (0.2, 1.5, 3.5 and 2 µs) and the
    brainstorm's 10 µs and 40 ms, at WebAssembly's 1.2–1.5× native, the point and the patch miss:
    the interpolation is the cost (the base elevation and the relief's three style interpolants),
    not the hash, which the bake's cached lattice already spares, so Design note 14's hash levers
    are not tried. A finding for open question 5; main logs a fused interpolant pass for the
    elevation and the three style values, before R10.T6's bake timing on a quiet machine. The
    rims, the uncached crater draw and the Moon-like world's figures (`surface/point/moon/…`,
    `…/rims-…`, `…/craters-uncached-…`) wait for a run with the heavy lock free.
  - _Generator version._ Untouched (21). No golden pins `height_at` before T9; the payloads golden
    moves under the testing-world rule.
  - _For R10's re-validation_ (Design note 5, T4.a, T4.b) _and R05's `HeightSource` trait doc:_
    `level_bound_m` is hard for the structural and channel terms and a 1 − 10⁻⁹ per-point quantile
    for the crater terms, and it is 15–500 times the realised worst difference. R10.T4's min(hard,
    4σ) is the lever; `LevelBounds::new(header)` builds every level at once. `PatchHeights` is the
    source's view, not R05's `PatchBake`.
  - _For R09.T12.d (its contract):_ draw each coarse octave, [`D_b` 2^k, `D_b` 2^(k+1)], at most at
    Trask's saturation on its own, as Design note 10 has it; the rims' term of the bound counts on
    it (the science check: a cap on the cumulative N(>D) or on another grid could exceed it by up
    to 1.24 in the octave that straddles the crossover).
  - _For T9 (what to pin)._ Heights and gradients through `height_at` at pinned points and levels
    of each world, among them points near coarse rims (at about 0.97, 1.0 and 1.04 rim radii), in
    channelled cells at L + 1 to L + 4, belts with 0 < b < 1, face-edge and cube-corner vertices,
    and the band limit's level beside a finer one (to the bit); one `bake_patch` a world; each
    world's `LevelSet`s, `LevelBounds::bound_m` and `contribution_range_m` at every level (R10
    reads them); `unresolved_variance` per contribution and `structure_function` at a few cells,
    wavelengths and lags; and the parts apart from `height_at` as the earlier records ask (T4's
    base elevation, T5's relief, T6's channels, T7.b's small craters, T7.c's rims).
- **Deferred findings (2026-10-13 rule)** (R09.T8; recorded, not ruled; each for "main"'s list).
  - _The level bound is loose_, 15–500 times the realised worst at 10⁴ points: header-only caps
    (the belt's amplitude 2.5 everywhere, a groove cap of 0.2 cell and a correction factor of 4.5
    on bodies whose cells cut far less, fresh craters where most are worn). Levers: R10.T4's min(hard,
    4σ); a header field of the field's largest amplitude and steepness; the craters' age
    distribution in their marks. Accepted as it stands for now (main, 2026-10-13).
  - _The channel term applies on bodies without channels_ (71 km at level 0 on the Moon-like and
    flat worlds): the header does not say whether any cell drains.
  - _The coarse levels' crater terms are tens to hundreds of kilometres_ (rims of craters up to π R
    at saturation): valid, and irrelevant to selection from afar, but not physical.
  - _The channels' structure function closed form is 3–25 times low per land cell_ at 10 m to
    1 km (median about 5× at 10–100 m), so the whole function is 15–37% above it pooled over
    channelled plains. A T6.b follow-up builds a form from the realised β and the neighbours'
    grooves (main logs it).
  - _The point and the patch miss the budget_ (above): the interpolation, not the hash. A fused
    interpolant pass (main logs it).
  - _An octave partly finer than a band_ takes its whole unit structure function at its share; its
    slice's gradient variance is up to about twice the octave's (the science check), so its
    small-lag share is up to 2× low.
  - _The rims' share is crater-wide, not per cell:_ a cell on a basin's rim has more of it, and one
    on its floor less.
  - _The Moon-like world's slopes_ against T7.b's calibration targets (T7.b's deferred κ): the
    lid's median slope at 17 m is 5.43° (Rosenburg et al. 2011: highlands 7.5°, maria 2.0°), its
    RMS height difference 248 m at 1.9 km and 670 m at 7.6 km (LOLA: 171 and 546 m), the province
    3.59°, 271 m and 828 m. The calibration of κ stays deferred.
  - _The crater closed forms on a sparse field_: on the Earth-like world's young, screened surface
    the sampled craters' structure function is far below the ensemble closed form at a few cells,
    a realisation's sparseness, not a bias.
  - _The heavy cost of a coarse patch's range_: `height_range_m` of a level-0 patch walks the
    65,536 coarse cells under it and the margin about the face (the rings now grow from the
    block's border only).
- **Deviations in T9, as built** (2026-10-13; lane r09-assembly, after T8; main's ruling of that
  day on the bump).
  - _Where the goldens are written._ A module of the crate, `src/synth/height_golden.rs`
    (`cfg(test)`, declared in `synth.rs`), not `tests/height_golden.rs`. The synthetic worlds are
    `cfg(any(test, feature = "testing"))`, and the browser target's test build (`just _browser`)
    passes no features, so an integration test could not see them there. Making it see them takes
    the crate's dev-dependency on itself with `testing`, which changes `Cargo.lock` and makes
    smart-ci plan the full `ci`. R09's other goldens are written from the crate the same way
    (`craters/golden.rs`, `wire/tests.rs`, the months'), and the module also reads crate-private
    items (`synth::test_support`, `Segment::slope_at`, the rims' and the relief's structure
    functions). The acceptance is `cargo test -p hyperion-surface --lib height_golden` natively,
    nextest's `-E 'test(height_golden)'` on `wasm32-wasip1`, and the browser target's `--lib` run,
    all of which `just test-wasm-fast` and smart-ci's suites include.
  - _The files._ 16 goldens in `tests/golden/height/`, 887 KB in all (the largest 111 KB, under the
    500 kB hook). Each world has two:
    - `<world>_points`: 64 sites × 8 levels, 512 `height_at` samples (heights and gradients). The
      levels are 0, L − 1 to L + 4, and the complete level, the shallowest at the band limit. The
      sites are the cube's eight corners; eight face-edge vertices; 16 about four coarse craters
      (0.97, 1.0 and 1.04 rim radii, and 0.98 of the reach); 12 in three cells whose channels reach
      L + 4 (the key point, the trunk's midpoint, two at random), the first on a face edge; eight
      in cells of belt weight 0 < b < 1; and 12 at random. A world lacking a group takes random
      sites in its place. Beside the pinned eight, every site's complete-level height is asserted
      equal, to the bit, to its height one level finer, at `finest_level` and at level 24.
    - `<world>_parts`.

    Four more cover every world: `common`, `levels`, `closed_forms` and `bakes`. The module
    documentation describes each file. R05's `BAND_LIMIT_M`, `FINEST_SPACING_M` and `finest_level`
    are pinned there (`common`, and the levels of the points), so they joined the version at 21,
    as Generator version expects.

  - _The parts, as the earlier records asked._
    - T4: the base elevation at all 64 sites, and `interpolate` at level 2 over a closed form at
      the cube's corners, face edges and random points.
    - T5:
      - each octave's spacing, RMS, ridged share, belt factor and bound;
      - `point_variance` at four styles;
      - `nyquist_degree` at every level;
      - `Relief::at` through L + 3 (before the warp) and through the finest, at 16 sites;
      - the 32 octaves' offsets of the tests' seed, read through a new public
        `noise::Octave::offset` getter and a `cfg(test)` `relief::octave_offsets`.
    - T6.a and T6.b, below six cells with channels (the first on a face edge):
      - the key points (s, t, inherited) and segments of 13 cells, L to L + 3;
      - each cell's `deepest_level` and `channel_variance` at 100 km to 100 m;
      - the first cell of each case T6.a's review named, searched for in cell-index order: a trunk
        into another face, into the sea, and held at its crossing (it leaves the great circle
        through its key points, as T6.a's test tells it, within 4,096 trunks); and a tributary
        joining on another face, clamped at its parent's end, and on the slope floor at its outlet
        (within the cells at L + 1 and L + 2 below the first 64 coarse cells with tributaries);
      - the incision through L to L + 4, at the 16 corner and edge sites and the 12 channel sites;
      - the closed forms: Hack's law both ways, `head_length` from L to 24, `tributary_rise` about
        the slope floor's crossover, `channel_head_area`, `valley_depth`, `bankfull_width` and
        `bankfull_depth`.
    - T7.b:
      - the octave table;
      - for the first, middle and last octaves, `craters_in` at a cube corner's, a face edge's and
        a random site's cells, and at the first cell from the corner's (within 4,096) that holds a
        crater: each cell's count and digest, and its first three craters;
      - `SmallCraters::at` at the 16 corner and edge sites at the first, middle and last levels that
        evaluate craters, and at that crater's centre and rim, at the shallowest level evaluating its
        octave and at the complete level.
    - T7.c:
      - `coarse_band_half_width`;
      - for four craters, the kept relief, and `CoarseRims::at` at L − 4, L + 2 and the finest
        level, at 0, 0.97, 1.0 and 1.04 rim radii and just inside the reach;
      - the same at face-edge vertices, sampled as T7.c's continuity test samples them (every eighth
        vertex of each face-edge cell a reach holds). It takes the first three a crater where the
        rims stand more than a metre high at the finest level, for the first four craters in list
        order that have any.
    - T8:
      - every level's `LevelSet`, `bound_m` and `contribution_range_m`, with `LevelBounds::new` and
        the synthesiser's asserted equal;
      - at up to five cells a world, `unresolved_variance` by contribution at five wavelengths;
        `structure_function` by contribution (relief, channels, small craters, rims) and whole, at
        two wavelengths and at lags of 1, 2, 5, 10, 100 and 1,000 m (2 and 5 m for R10.T5's slope
        baselines); and `unresolved_rms`;
      - one `bake_patch` at L + 3 a world: the digests of its 4,225 samples and 1,089 parent
        heights, six samples and four parent heights.
  - _What the worlds do not give._ The searches write `none` where a world has no case, and these are
    the gaps:
    - No tributary on the slope floor at its outlet on either channelled world: the floor binds
      beyond a 1 km main stream only at the smallest steepness codes. The floor's arithmetic is
      pinned through `tributary_rise` about the crossover.
    - No trunk into the sea on the Mars-like world, which has none.
    - The Earth-like world's last octave holds no crater within 4,096 cells: screened, about
      4 × 10⁻⁸ a cell. Its first and middle octaves do.
    - No Earth-like rim reaches a face edge.
    - T7.b's "cell of the last octave that draws a crater under 2 m" is not singled out, since
      `craters_in` drops such a draw. The last octave's kept craters are pinned, so a target that
      cut at 2 m differently would show.
    - The octaves' rotations (`noise::rotation`, constants) are pinned only through the heights.
  - _Across targets._ The goldens agreed to the bit on all three targets each time they were
    compared: native, `wasm32-wasip1` under wasmtime 49.0.1, and `wasm32-unknown-unknown` on
    Electron 44.4.3's V8 through the shim. No mismatch was found, so nothing was fixed.
  - _A change to any synthesis constant fails it._ Checked by hand by nudging one constant at a
    time, each reverted:
    - `WARP_SCALE` by 2 × 10⁻⁷ fails the Earth-like world's two files, the only world whose cells
      shear;
    - `BANKFULL_WIDTH_COEFFICIENT` fails 10 files;
    - `DIFFUSIVITY_M2_PER_MYR` fails 10 files.
  - _The narrowing._ T3 already narrowed `golden_diff.py`'s test-planet rule to the files directly in
    `tests/golden/` (`is_test_planet`), and `cube_golden.rs`'s check was already a flat `read_dir`.
    So T9 adds only `native_only::every_height_golden_carries_the_generator_version` (16 files).
    `golden_diff.py` reports the 16 as new at the generator's version, finds no inconsistency, and
    still checks R05's goldens against `TEST_PLANET_VERSION`.
  - _Generator version._ No bump: the goldens carry 21. R09's first bump rides plan 14's held 21 → 22
    batch at P14.T48.e, which re-blesses `height/` with the rest (main, 2026-10-13; Generator
    version). T16's "if T9 has not already" now reads: the batch's bump is the first.
  - _The bench under the shim._ `benches/synth.rs` gains `surface/point` for the browser target, as
    `wasm-bindgen-test` 0.3.79's `#[wasm_bindgen_bench]` (20 samples a part). It covers the same
    parts and patches through a `shared` module that both targets' benches read; the native IDs are
    unchanged. It ran on Electron 44.4.3's V8 (15.2) through the shim, under the heavy lock (665 s
    held), at a load of 8 to 12, schedutil, one thread in a 400% quota: provisional. Per point, a
    patch's time ÷ 4,225, from a warm cache:

    | Part             | Earth-like, 12 | Earth-like, 19 | Moon-like, 10 | Moon-like, 17 |
    | ---------------- | -------------: | -------------: | ------------: | ------------: |
    | base             |        2.03 µs |        2.03 µs |       0.27 µs |       0.27 µs |
    | relief           |       12.81 µs |       14.01 µs |       2.92 µs |       4.19 µs |
    | channels         |        1.04 µs |        1.04 µs |       0.30 µs |       0.30 µs |
    | craters          |        3.88 µs |        8.19 µs |       5.08 µs |      10.23 µs |
    | craters-uncached |          98 µs |         204 µs |      1,140 µs |      2,809 µs |
    | rims             |        0.08 µs |        0.08 µs |       0.50 µs |       0.50 µs |
    | height           |       19.98 µs |       25.61 µs |       9.20 µs |      15.76 µs |

    At L + 4 on the Earth-like world that is 1.22 (base), 1.52 (relief), 1.15 (channels), 1.39
    (craters) and 1.43 (the whole) times T8's provisional native figures, within or near T8's
    assumed 1.2–1.5. Against Design note 14's WebAssembly targets the channels meet theirs (3.5 µs).
    The base elevation (0.2 µs), the relief (1.5 µs) and the small craters (2 µs) miss, and the
    whole misses the brainstorm's 10 µs a point. The height sweep of a patch is 84 ms at L + 4, against
    40 ms (deferred, below).

  - _For R10's re-validation._ R10's goldens of the synthesis can follow `height/`'s pattern:
    - a subdirectory each, since the files directly in `tests/golden/` are held to
      `TEST_PLANET_VERSION`;
    - a `cfg(test)` module in the crate, where the synthetic worlds are;
    - a `--lib` acceptance filter, and a `native_only` header check.

    R10.T1.b's text still names `tests/material_golden.rs` and its `--test material_golden`
    acceptance. R10's Consumes' "provisional until R09.T9 lands" can go.
- **Deferred findings (2026-10-13 rule)** (R09.T9; recorded, not ruled; each for "main"'s list).
  - _The point and the patch miss in WebAssembly too_ (the bench above): about 2× the 10 µs a point
    and the 40 ms a patch on the Earth-like world, with the interpolation (base 10×, relief 8.5× of
    Design note 14's targets) the cost, as T8 found natively. That is T8's fused-interpolant
    follow-up, now with WebAssembly figures for R10.T6.a's budget. The small craters' uncached
    draw costs 0.1–2.8 ms a point (each point from an empty crater cache), so a worker must keep
    its crater cache across a patch, as `SynthCache` does.
  - _Goldens moved before the batch._ A synthesis change landing before P14.T48.e (the T6.c
    channel structure-function follow-up, the fused interpolant pass) moves `height/` at 21. The
    Generator version rule says it bumps, and the sim-determinism skill's re-bless exception covers
    `wire/` alone. Whether such a change instead re-blesses at 21, with the batch's one bump
    covering it, is main's to rule when the first one lands; the skill then needs its line.
- **_The level-keyed closed forms_** (R10.T5; `decision-r10-t5-closed-forms.md`). A band level
  evaluates each contribution by its own scale, while the wavelength-keyed closed forms place each
  at wavelengths of their own: a structural octave of spacing s carries the degrees
  N(m − 1) ≤ l < N(m), wavelengths 8/3 s down to 4/3 s; a channel level is counted from its mean
  cell's width to twice it; a small-crater octave from its smallest kept diameter to its upper one;
  the coarse rims' sharpening at half the wavelength. So `unresolved_rms(cell, λ_n)` counts too
  little of what level n leaves out, 2.6 to 3.3 times in the relief's variance on the synthetic
  worlds, and nearly all of the first crater octave it leaves out. As ruled, the wavelength-keyed
  forms stay as they are, their docs corrected, and R10.T5 added the level-keyed forms to
  `Synthesiser` (`omitted_variance`, `omitted_structure_function`, `omitted_rms`), each
  contribution from its own cut (crate-private `levels::LevelCuts`; the relief through a
  degree-keyed entry under its wavelength forms, whose bits are unchanged). R10's T2, T4 and T5
  read them. The small craters' part does not yet count the sharpening of the octaves a level
  keeps: R09.T7.h, which has since added it (below).
- **Deviations in T7.h, as built** (2026-10-14; lane r09-crater-sharpening, after R10.T5;
  `decision-r10-t5-closed-forms.md` §2 and its list for this lane; main's ruling of 2026-10-13 on
  re-blessing at 21).
  - _Files._ `synth/craters.rs` (the moments and both forms); `synth/levels.rs` (`LevelCuts` loses
    its craters' cut, and `LevelSet` gains the crate-private `keeps_crater_octave`); `synth.rs` (`omitted_variance` and `omitted_structure_function` call the
    craters' level-keyed part, and the docs); `synth/craters/rims.rs` (`difference_moments` becomes
    `pub(super)` and sorts its breakpoints on the stack, the rims' bits unchanged); the tests in `synth/craters/tests.rs` and the crater part of `reading/tests.rs`'s
    decomposition test; and `tests/golden/height/closed_forms.golden`. No public signature changes,
    and every new item is crate-private.
  - _The moments_, at each octave's own eight nodes in diameter. Each node keeps its crater filleted
    over the ground's half-width `w₂` = 1 m (`GROUND_FILLET`, ½ `BAND_LIMIT_M`), its ½ ω π R² and
    its E[g²]:
    - `G_k`, the octave's variance at the ground's fillet, is built with the octave
      (`CraterOctave::ground_variance`). `variance()` keeps its bits: the factors are taken in their
      old order, and the parts goldens' octave tables did not move.
    - `Φ_k`(w) and `S_k`(w) are summed per call (`CraterOctaves::sharpening`) from
      `rims::difference_moments` of F(`w₂`) and F(w). Both are zero to the bit where the two fillets
      are one: w ≤ 1 m, or both held at min(1, a) rim radii.
  - _The forms._ The wavelength-keyed ones count the share `s_k` (`band_share`) of an octave left out
    at `G_k`, and the rest at `Φ_k`(max(½ λ, 1 m)). Their structure function puts the share left out
    on T7.b's J₀ ring at `G_k`, and the kept share in the rims' form, 2Φ (1 − exp(−S b² ÷ 2Φ)). The
    level-keyed ones (`CraterOctaves::omitted_variance(set)` and
    `omitted_structure_function(set, lag)`) keep the octaves through the set's `craters_through` at
    its `band_half_width`, and the rest whole. As the ruling foresaw, the craters' part is no
    longer a cut, so `LevelCuts` has no craters' field and `LevelCuts::of` no octaves.
  - _Per call, not tabulated._ Construction gains 0.07 ms a body for the ground moments, on 0.9 ms
    (Moon- and Ceres-like) to 11–13 ms (Mars- and Earth-like, screened). The sharpening costs about
    12 µs an octave kept, so the level-keyed craters' part takes 51–145 µs a call and the
    wavelength-keyed one 72–165 µs, at the levels of 1 km to 10 m surveys (slow-test profile, one
    thread, a shared machine: provisional). A table by band level would put about 3 ms (and some
    30 times that in debug builds) on every construction, which every free closed form and
    `LevelBounds::new` pays. R10.T5's pooled case reads it twice a point.
  - _Tests._ In `synth::craters`:
    - `one_crater_s_sharpening_is_its_two_profiles_sampled`: radially, by a midpoint rule on 2 × 10⁵
      panels blind to the pieces, to 10⁻⁶ in the heights (measured 8 × 10⁻⁹ at worst) and 10⁻⁴ in
      the slopes (4.3 × 10⁻⁶); over the plane, on a 400 × 400 grid with the slope along one axis by
      central differences, to 5 × 10⁻³ (2.2 × 10⁻³, the grid's own error on a fillet 16 cells
      wide). Craters of 2.5 m to 25 km, at fillets of 0.02 to 3 rim radii.
    - `the_closed_form_variance_is_its_dense_quadrature` now also holds `G_k` to 2%, and `Φ_k` and
      `S_k` at bands of a quarter and a half of the smallest diameter to 4%, against 64 diameters
      (the figures are under "Deferred findings", below).
    - `an_octave_s_sharpening_grows_with_its_band_to_no_more_than_its_ground_variance`: zero at the
      ground's fillet and finer; non-decreasing in steps of a quarter, and never above `G_k`, out
      past the widest crater's rim radius. It holds on every octave of every world, so the ruling's
      condition for a monotone V(λ) is met.
    - `both_forms_are_their_octaves_sharpening_and_ground_and_vanish_at_the_band_limit`: the
      level-keyed part is Σ `Φ_k` + Σ `G_k` to the bit at every level, and its structure function the
      rims' form plus the ring, to 10⁻¹²; both are zero from the band limit's level; the
      wavelength-keyed variance is its octaves' sum to the bit, and zero at 2 m and below.
    - `the_unresolved_variance_falls_with_the_wavelength_to_zero_at_the_band_limit` now steps by a
      fifth from 10⁹ m and takes Σ `G_k` as the whole. `unresolved_rms` stays non-increasing as the
      resolution refines (`synth::tests`), and the level-keyed forms as the level deepens
      (`reading::tests`); neither needed a change.
  - _Measured_ (`measure_the_kept_octaves_sharpening_and_the_octaves_left_out`, `#[ignore]`; 400
    cells × 25 points, 10⁴ a world and level, at the deepest level whose `λ_n` is the survey or
    longer; slow-test profile, 277 s). Realised mean square ± its standard error, against closed:

    | World      | Survey | Level | Kept sharpening (m²)      | Octaves left out (m²)     |
    | ---------- | ------ | ----- | ------------------------- | ------------------------- |
    | Moon-like  | 100 m  | 10    | 1.388 ± 0.123 vs 1.299    | 0.053 ± 0.016 vs 0.072    |
    | Moon-like  | 1 km   | 7     | 180.6 ± 13.0 vs 188.2     | 782.0 ± 58.7 vs 829.6     |
    | Mars-like  | 100 m  | 11    | 0.080 ± 0.025 vs 0.084    | 0.208 ± 0.078 vs 0.223    |
    | Mars-like  | 1 km   | 8     | 10.05 ± 2.41 vs 8.10      | 9.84 ± 2.05 vs 32.83      |
    | Ceres-like | 100 m  | 9     | 0.111 ± 0.012 vs 0.125    | 0.146 ± 0.025 vs 0.155    |
    | Ceres-like | 1 km   | 5     | 15.00 ± 3.52 vs 15.45     | 116.2 ± 26.0 vs 107.6     |
    | Earth-like | 100 m  | 12    | 0.0011 ± 0.0006 vs 0.0044 | 0.0072 ± 0.0037 vs 0.0215 |
    | Earth-like | 1 km   | 9     | 0.0023 ± 0.0017 vs 0.117  | 0.039 ± 0.014 vs 1.089    |

    On the Moon-, Mars- and Ceres-like worlds the sharpening agrees within its sampling, and so do
    the octaves left out but for the Mars-like world at 1 km. The Earth-like world's craters are
    screened and young, so a sample of 10⁴ points misses the few that carry E[g²] (deferred,
    below). The realised means are small against the RMS, at most 0.58 m against 28 m (the Moon-like
    world's octaves left out at 1 km), the zero mean the forms assume. The ± takes the points as
    independent, so it is a lower bound: a cell's 25 points are correlated where the kept craters
    are kilometres wide, and a sample that misses the rare young craters understates its own spread.
    On the Earth-like rows it means nothing (the science check).

  - _The golden._ `closed_forms.golden` was extended first, at R10.T5's values, by its level-keyed
    forms at every band level for each pinned cell: `omitted_variance` by contribution and whole,
    and `omitted_structure_function` at lags of 2 m, 5 m and `λ_n` (1,000 labels; `golden_diff.py`:
    extended only). It was then re-blessed at 21 with no bump (main's ruling of 2026-10-13, before
    P14.T48.e), so `golden_diff.py`'s "not bumped" is that ruling's expected problem. 796 lines moved,
    on the four cratered worlds alone, and in them only the craters' column, the total or whole,
    `unresolved rms` and the level-keyed structure functions: 60 wavelength-keyed variances, 180
    structure functions, 30 RMS, and 263 levels' omitted variances with as many of their structure
    functions. The relief,
    channels and rims columns, the flat and one-crater worlds, and every other height golden are
    unchanged. For example, on the Moon-like world's cube-corner cell the craters' part at level 10
    went from 0.0717 to 1.371 m² (the total 1.265 to 2.564 m²), at level 7 from 829.6 to 1,017.8 m²,
    and finer than 100 m from 0.0016 to 0.234 m².
  - _Generator version._ 21, not bumped (main's ruling): the closed forms reach no height, save or
    wire.
  - _R10.T5's pooled case, with the crater sharpening in_, by name in 577 s:
    `just test-slow -E 'binary(surface_reading_worlds)'`. RMS ÷ σ and var z, against 0.984/0.941
    to 1.430/2.473 before:

    | World      | At 100 m        | At 1 km         |
    | ---------- | --------------- | --------------- |
    | Earth-like | 0.9831 / 0.9318 | 0.9869 / 0.9697 |
    | Mars-like  | 0.9941 / 0.9620 | 1.0244 / 1.0230 |
    | Moon-like  | 0.9883 / 0.9748 | 0.9828 / 0.9653 |
    | Ceres-like | 0.9942 / 0.9882 | 1.0858 / 1.1762 |

    Seven of the eight cases are within the plan's 10% and 15%. The Ceres-like world at 1 km is
    within 10% in RMS but its z-scores' variance is 1.176, 0.026 over the plan's 15%. There the
    closed forms leave 37.7 m² of the realised 248 m² unexplained, against craters' closed forms of
    123 m² that the measurement above finds within its sampling (131 ± 26 m² realised at 10⁴
    points). Main ruled (2026-10-14) that this is the ruling's heavy-tailed crater sampling, and
    that the gap's commit under R10.T5 hold every case at the plan's 10% and 15% but this one:
    `CRATER_GAP` is the Ceres-like world at the 1 km survey alone, its RMS ratio at the plan's 10%
    and its var z in [0.85, 1.25] (`GAP_Z`), 1.176 measured with headroom for a change of seed. R10's
    T5 entry records it.

  - _The measurement joins the slow pass._ It is `#[ignore]`, so `just test-slow`
    (`--run-ignored only`) runs it, 277 s on one thread natively, as R10.T5's and R13.T1's
    recorded measurements run.
- **Deferred findings (2026-10-13 rule)** (R09.T7.h; recorded, not ruled; each for "main"'s list).
  - _The eight nodes miss a kink on the widest octaves._ Against 64 diameters, every octave's
    sharpening is within 0.4%, but the widest, complex octaves' at a band of half their smallest
    diameter are not: 3.4% high (Mars-like octave 0, at 22.6 km), 2.5% (Ceres-like octave 0) and 1.1%
    low (Moon-like octaves 0 and 1). There the fillet's inner end crosses the floor's edge as the
    diameter varies across the octave. It touches only the coarsest levels, where the relief
    dominates. A fix is more nodes, or nodes split at that diameter.
  - _Heavy-tailed realisations_ (the ruling's). The Earth-like world's realised sharpening is 0.02
    to 0.25 of the closed form, and its octaves left out 0.04 to 0.33, at 10⁴ points; the Mars-like
    world's octaves left out at 1 km, 0.30. An octave's E[g²] is carried by its youngest craters, which
    are rarest where the atmosphere screens the production. The explanation is plausible, not shown
    (the science check): the Mars-like and Moon-like figures at 100 m converge from the 288 points of
    R10.T5's diagnostic to these 10⁴, but a count of the carrying craters a sample expects to hit,
    or a larger sample on another seed, would settle it. The Mars-like row at 1 km does not test
    T7.h's terms, since a 1 m fillet changes those octaves' ∫h² by some 10⁻⁸. R10.T5's pooled case
    on the Ceres-like world at 1 km (var z 1.176 against the plan's 1 ± 0.15) is the case the ruling
    foresaw, held by main's ruling in [0.85, 1.25] alone: a check weighted by crater count, or a
    larger cratered sample, would retire it, in R10.T15's verification pass.
  - _Per call._ If R10.T2 or T4 read the level-keyed forms per vertex rather than per cell and
    level, a table of `Φ_k` and `S_k` by band level, built once per body, would replace the 50–150
    µs a call.
- **Deviations in T11, as built** (2026-10-14; lane r09-plates, on `a1f1b61a`; the inputs are
  builder arguments until P14.T48.e and P14.T54.a put them on the record).
  - _Files and the frame._ `steps/plates.rs`, its tests in the child `steps/plates/tests.rs` (as
    `synth/relief/tests.rs`). The frame in `steps/mod.rs`, shared by T12–T15, gains
    `CellState::seafloor_origin: Option<SeafloorOrigin>` and the plain-data `SeafloorOrigin`
    (`ridge_distance: Option<Metres>`, `half_rate: MetresPerSecond`, `age: Seconds`),
    `decision-r09-t5.md` item 5's d_r, u_h and τ on oceanic cells, `None` elsewhere; and
    `Working::plates: Vec<plates::Plate>` (seed, angular velocity in rad s⁻¹, drawn and realised
    areas, weight). The quantiser is unchanged: age and rate do not travel. Public: `run`,
    `Plate`, `MEAN_HALF_RATE` and `OLDEST_FLOOR`; the law's constants, the speed scale and the
    warp's are `pub(crate)`.
  - _What it reads_ (all from `CoarseInputs`, builder arguments, no record section being
    inhabited): `tectonics()` (P14.T24.b's `TectonicRegime`), `continental_fraction()`
    (P14.T48.a's f_c), `heat_flow()` (P14.T24.b's `GlobalFigures::heat_flow`), `radius()` and
    `crust_palette()`. Plan 14 gives no plate count: the count is the tail law's draw.
  - _Regimes._ Only `MobileLid` draws plates. `StagnantLid`, `HeatPipe`, `Episodic` and
    `IceShell` take the stagnant lid's: the step leaves the initial state's one lid, for T12.c's
    provinces (T10's "T11 decides what each gets"; the morphologies stay in Composition). So does
    a mobile lid with no secondary crust (no `Oceanic` palette entry), whose plates could form no
    floor. A world with no tertiary crust has no continental crust, whatever its f_c (finding
    below).
  - _The draws._ Plate k on `surface_item(k)`: its seed (two words, z and longitude uniform), its
    angular velocity (two Box–Muller pairs, three components used), its area (one word: a tail
    plate's from `PowerLaw` of density A^−1.31 on [A_min, 1 sr); a large plate's an exponential),
    then its continental nucleus (centre, two words; size u², one). The tail's count is
    Poisson(7 (A_min^−0.31 − 1)), 41.06 from 0.002 sr, drawn from word 2⁴⁰ of item 0 so that it
    and plate 0 share no word; A_min is 0.002 sr or the mean cell's solid angle where larger (a
    level-5 field's 2.05 × 10⁻³ sr). The seven large plates take 1 sr each plus shares of the
    excess, uniformly Dirichlet (the exponentials normalised). Where the tail would overflow the
    4π − 7 sr they leave it (11 of 500 seeds), it ends at the plate that would; at most 256
    plates. The tag table's "crust type per plate" is the continental nucleus.
  - _The raster is a fitted power diagram._ Design note 6's "largest dot product" gains a weight
    per plate, score x′ · p + w, ties to the lower index, since an unweighted diagram of uniform
    seeds has cells of comparable size and cannot follow Design note 6's size law. The weights are
    fitted (the semi-discrete transport problem) by damped Newton steps on the Laplacian of the
    plates' shared boundaries, ℓ_kj ÷ |∇(s_k − s_j)| with ℓ from the edges crossed (π ÷ 4 a cell
    side each), an empty plate coupled to its seed's host and stepped by its deficit there, halving
    until Σ (error ÷ tolerance)² falls, at levels 6, 7, … up to the field's, then single-plate
    sweeps; the tolerance is 1% or two mean cells. `assign` sets aside, per level-4 block, every
    plate whose chordal upper bound falls short of the best lower bound by 10⁻⁹, so its result
    is the full scan's bit for bit (a test compares them on the reference Earth). A part of a
    plate cut off from its largest part joins the neighbouring plate it shares most edges with
    (`join_parts`), and the field's level is fitted with its parts joined, so every plate is one
    region and its area its drawn one. This departs from the brainstorm's Voronoi diagram "in
    milliseconds" as well as from Design note 6; signed off 2026-10-16 (`signoff-11.md`, item 20),
    with the brainstorm, the Goal and Design note 6 corrected.
  - _The warp._ A vector of three uniforms in [−1, 1) at each cell of a level-4 lattice on
    `surface.coarse.warp` (`surface_cell` keys), their Shepard mean under the compact kernel
    (1 − r² ÷ h²)³ within the chord h = 0.21, C² across face edges; a cell's centre moves by 0.12
    times it for the plates and 0.25 for the continental nuclei, both illustrations chosen by eye.
    T4's interpolant was tried first: three interpolations a cell took 1.6 s at level 8, the
    kernel 0.06 s.
  - _The crust._ Continental where |x′ − c| ÷ ρ is least over the plates' nuclei, cells taken in
    (score, index) order while they bring the area nearer f_c × 4π (to half a cell); the rest
    oceanic. The nuclei are not confined to their plates, so continents straddle boundaries
    (collisions, continental rifts and transforms), and plates are mixed, with passive margins
    inside them for T12.a's turbidites (`decision-r09-t5.md` item 5), which the testing world's
    one-crust plates could not give.
  - _The motion._ Each component of ω is a standard normal times Ω = Ω⊕ (F ÷ F⊕)², with
    Ω⊕ = 4√π u_h ÷ (3√3 R⊕) = 1.56 × 10⁻¹⁶ rad s⁻¹ in closed form, so that a ridge's mean
    half-rate is Bird's 23 mm a⁻¹ (measured: 23.4 mm a⁻¹ over 32 reference Earths), and F⊕ =
    0.0916 W m⁻²; the square follows the half-space cooling model's q̄ ∝ √(u ÷ L) with L ∝ R
    (medium-low confidence). A world of no heat flow has plates that do not move, which meet at
    transforms.
  - _The boundaries._ A boundary cell takes, of its edge neighbours' plates, the one whose
    boundary is nearest by the score difference over its gradient; the normal is that gradient's,
    by least squares over the four edge neighbours (within some 10⁻⁵ rad of the plane's on
    hand-placed plates). Classes by the relative motion against it, the testing worlds' equal
    thirds (convergent within 60° of head-on, divergent within 60° of opening); no relative motion
    at all is a transform of obliquity π ÷ 2. A convergent boundary between continents is a
    collision; an oceanic side subducts beneath a continent, and of two oceanic plates the one
    whose floor along their oceanic convergent boundary is older on average, the higher index at
    a tie. T2's sign lean is kept: the distance is negative on the subducting plate, per nearest
    boundary. So on a plate that subducts at one boundary and spreads at another the sign flips
    from −d to +d where the nearest boundary changes, and the test bounds |d| a step, not d (for
    T12.b, which reads the sign). A plate's ridge whose half-rate rounds to zero (a subnormal
    opening, from a heat flow below some 10⁻¹⁵⁰ W m⁻²) is a transform.
  - _The transforms._ Multi-source shortest paths over each plate's own cells, through edge and
    vertex neighbours with great-circle lengths, settled in (distance, cell, source) order. The
    nearest-boundary transform starts from every boundary cell (zero there, +0); the ridge
    transform from the cells whose nearest boundary is divergent, each settled cell inheriting
    its source's half-rate, so d_r is never taken from a trench; τ = min(d_r ÷ u_h, 200 Myr).
    Oceanic cells no ridge of their plate reaches take 200 Myr and 23 mm a⁻¹, with no d_r.
  - _The interim sea floor._ Oceanic cells get `hill_relief = Some(H_REF)`, the reference style's
    95 m, so that the field keeps its rule (hills exactly on oceanic crust) until T12.a writes its
    law; `CellState::hill_relief`'s documentation says so.
  - _Tests and acceptance._ `cargo test -p hyperion-sim planetary::surface::steps::plates`: 18
    fast tests and one slow one. The fast ones: ties to the lower index, and the pruned scan the
    full one's; every plate one region; boundaries exactly the adjacencies; signed distances +0 on
    boundaries, negative only by subduction, stepping by at most a link's length; kinds,
    obliquities and polarity on hand-placed hemispheres (opening, closing and sliding); the older
    oceanic side; half-rates |ω × r| ÷ 2 normal to the ridge to 10⁻⁴, inherited bit for bit; d_r
    zero on ridges and from no trench; τ = d_r ÷ u_h capped; 200 Myr and 23 mm a⁻¹ without a
    ridge; the drawn areas' tail law (KS D = 0.0033 over 20,409 tail plates of 500 seeds,
    p = 0.98), the drawn count (mean 47.82, spread 6.29; the plan's 48 ± 7), the Poisson count
    within 3.29 SE of 41.06, early tails; the areas summing to 4π and each plate within its
    tolerance; f_c to half a cell (0, 0.2, 0.7, 1 and the reference Earth), a collision and a
    passive margin; no plates on the stagnant lid, the other three regimes and a mobile Ceres;
    the same seed twice; levels 5–7 mobile lids at f_c 0.2–1 and heat flows 0–0.3 W m⁻²
    through the whole pass; the reference Earth's mean half-rate (22.45 mm a⁻¹). The slow one
    (`#[ignore = "slow: 32 reference Earths' plates at level 8"]`, 46 s under load): the realised
    tail areas (KS D = 0.024 over 1,379 plates, p = 0.40), the realised count (49.94 ± 7.08) and
    the mean half-rate (23.43 mm a⁻¹ within 20%). So the acceptance filter runs the drawn areas'
    KS test, and the task's "over many seeds" on the realised areas runs by name, with
    `cargo test -p hyperion-sim --features testing --lib planetary::surface::steps::plates -- --ignored`, or in the slow pass; the fast tier holds the
    realised areas to the drawn ones on the reference Earth. "48 ± 7" is read as a bound on the
    mean; the spread (6.29 drawn, 7.08 realised) is measured, not asserted.
  - _Plan text superseded_ (for the docs batch): the tag table's `surface.coarse.plates` draws
    each plate's continental nucleus (centre and size), not a "crust type", and the count from
    word 2⁴⁰ of item 0; `surface.coarse.warp` also warps the nuclei; Design note 6's "Euler poles
    and crust types" are Euler poles and nuclei, the crust per cell, plates mixed; its
    "(distance, cell index)" order is (distance, cell, source), over each plate's own cells. The
    surface crate's registry docs (`tags.rs`) say the same, which T11 left alone to keep its
    files the sim's.
  - _The working state grew_ by `seafloor_origin`, about 40 B a cell (16 MB at level 8), and the
    step holds some 70 MB more while it runs (two warped centres, the contacts, two transforms'
    reaches), beside R09.T17's deferred estimate of about 100 MB a pass ("Concurrent passes").
  - _Cost_ (provisional, at `opt-level = 2` under shared load): the step takes some 0.6–0.9 s on
    the reference Earth at level 8: the warp 0.06 s, the fit 0.2–0.4 s, the crust 0.08 s, the
    layout 0.33 s (two transforms of 0.15 s each). The pass's own tests on the reference Earth
    carry it.
  - _Moves._ No golden pins the coarse field (T16's), none moved (`golden_diff.py`), and
    `GENERATOR_VERSION` stays 21. The reference Earth's unpinned field now has 40–55 plates,
    mixed crust and boundaries cell by cell, so T19.c's lean (re-run it when T11 first varies the
    records) is due.
  - _Review._ The Rust review's findings are applied (the fit passed by value, units in names,
    `#[must_use]`, `pub(crate)`, sourced constants, the warp taken whole, the pair map's
    `expect`); the determinism audit found nothing to fix (its three hardening items: the draw
    golden and the registry's docs are deferred below, and a subnormal opening is no ridge); the
    science check's documentation corrections are applied (Bird's −1/3 against the design note's
    0.31, π ÷ 4's derivation, the 9.5 m margin at 200 Myr, NNR-MORVEL56's speeds, Bird's classes
    and the cooling law's sources), and its figures are deferred below.
- **Deferred findings (2026-10-13 rule)** (R09.T11; recorded, not ruled; each for "main"'s list).
  - _The sea floor is old_ (the science check's one large finding; it would change the ruled
    algorithm, so it is "main"'s to route). On four reference Earths the area-weighted mean age is
    110–125 Myr and 25–37% of the floor sits at the 200 Myr cap, against Earth's 63.8 Myr
    (Müller et al. 2008's grid, as Seton et al. 2020, G³ 21, e2020GC009214, §3.1, quote it; the
    check's own area weighting of that grid gives 64.3 Myr, with 15.2%, 4.7% and 0.22% older than
    120, 150 and 180 Myr). τ = d_r ÷ u_h ages the far side of every plate from its ridge, where
    Earth's trenches consumed it; slow openings near Euler poles and on oblique ridges age whole
    plates; and plates with no ridge take the cap. GDH1 saturates past 100 Myr, so T12.a's ocean
    mean and mode carry it. Lean: T12.a calibrates against the hypsometry, and a ruling chooses
    between bounding τ by the plate's own trench and mapping ages by area rank onto Earth's
    distribution.
  - _Bird's figures, like for like_ (the science check). The 23 mm a⁻¹ is half Bird's mean
    relative speed on oceanic ridges (46.6); their mean opening is 44.7 mm a⁻¹, 22.3 halved, so a
    like-for-like Ω⊕ would be 3% lower. Bird's ridge class spans motion within 70° of opening
    head-on (¶127's ±20° strike-slip rule), the step's 60°; adopting it would raise Ω⊕ by 7.5%.
    Continental rifts open at 17.6 mm a⁻¹ in full against the ridges' 46.6 (Table 3), where the
    step gives them ridge rates. Lean: record; the ruling's 23 and the equal thirds stand.
  - _Slivers._ A small plate between two large ones rasterises as a thin strip along their
    bisector, since a power cell is the intersection of caps about (p_k − p_j) ÷ |p_k − p_j|, not
    about its seed. An additively weighted score, w − θ(x′, p), keeps every cell star-shaped about
    its seed and connected, at an angle per score. Lean: T12.b's re-validation decides, as it reads
    the boundaries. _Seen at the sign-off (signoff-11, item 20): left to T12.b's re-validation,
    which measures the slivers on the reference Earths; the additively weighted score is its first
    candidate, not adopted._
  - _Small plates are not in orogens._ Seeds are uniform; Bird 2003 places most small plates in
    the orogens about the large ones' boundaries. Lean: with the slivers.
  - _No speed–size or slab-pull relation._ Each plate's ω is drawn alone; on Earth small plates
    turn fastest and plates attached to slabs move fastest. Lean: for a geodynamics pass.
  - _Transforms are long._ A boundary is classed cell by cell from smooth geometry, so transforms
    are long arcs, not the short faults offsetting ridge segments (no fracture zones). Lean: T12's
    re-validation.
  - _The classes' thresholds_ (60° and 120°) are the testing worlds' convention (labelled):
    a third of boundaries to each class under random motion, against Earth's 35.2% convergent,
    36.4% divergent and 28.4% transform by length (Bird 2003, Table 3), where Bird's own ±20°
    would give some 22% transforms. Lean: keep them.
  - _Distances run 8-connected._ Shortest paths over edge and vertex links overstate great-circle
    distance by up to about 8% at some bearings, which T12.b's 190 and 260 km fronts read. Lean:
    T12.b measures it against its 15 km medians.
  - _The inputs accept two contradictions,_ for P14.T54.a, which owns `inputs.rs`: a mobile lid
    with no secondary crust, and f_c above zero with no tertiary crust. The step gives the first
    no plates and the second no continents. Lean: `build` refuses both.
  - _The speed law rests on boundary-layer scaling_ (the half-space cooling model's q̄ ∝ u^½;
    q ∝ t^−½ is Davies and Davies 2010's eq. 1, the plate average Turcotte and Schubert's, not
    read), which is contested for hotter mantles (Korenaga 2006 and 2008, from memory, argues
    for slower plates). Lean: a geodynamics pass; the label stays medium-low.
  - _Rifts age floor._ The ridge transform starts from every divergent boundary cell, continental
    rifts included, and its paths cross a plate's continental cells, so some floor takes its age
    and half-rate from a rift that made none. Lean: T12.a's science check.
  - _The draws are unpinned._ Nothing pins the plates' and the warp's word layout until T16's
    field golden; a reordered draw would pass every test. Lean: T16, or a draw golden when "main"
    rules its header (21 or 22).
- **Deviations in the fused interpolant pass, as built** (2026-10-14 and 15; lane r09-fused,
  R09.T8's performance follow-up as "main" logged it (T8's and T9's deferred findings, and
  R10.T6.a's bake's); begun on `373a0158`, brought to `3c921671`, merged with `2955006e` and then
  with `9e3915ab`, on which its gate ran, and landed squashed as `13fb4bf1`).
  - _What it is._ The base elevation (T4's interpolant over the cells' elevations) and the
    relief's three style values (T5's amplitude, belt and shear weights) were four interpolations
    a point, each walking the same stencils over the same cells, and each style read the cell's
    `ReliefStyle::of` again at every cell it read (an oceanic cell's amplitude takes two powers and
    a logarithm). They are one pass now:
    - `interp::interpolate_each` interpolates N values a cell (`CellRecords<N>`), sharing the
      query's cell, each face's weight and basis, the stencil's walk and one read of each cell for
      all N, with each value's own arithmetic in the one-value order. `interpolate` is it at N = 1.
    - `Relief::at` fuses its three styles (`relief::styles`). `Synthesiser::contributions` fuses
      all four where the level evaluates an octave (`relief::base_and_style`, then the crate-private
      `Relief::at_in_style`), and keeps `base_elevation` alone where it does not.
  - _The bake's shared work._ A patch inside one coarse cell reads about 25 control points a face,
    the same at each of its 5,314 queries. `interp::ControlMemo` keeps each control point's raw
    values, a cell's four or a ghost's four cells' with its bilinear fractions, by face and indices,
    direct-mapped in 1,024 slots (about 176 KiB), and each query takes them relative to its own cell
    as before.
    - `SynthCache` holds it (`BakeControls`), for the field its channels' part is kept for (the
      channels' `NetworkId`: body, level, figure and the header's marks).
    - `bake::cover_patch` empties and starts it. That is each bake's cover: R09's `bake_patch`, and
      through T3's `prepare_cache` R05's bake and R10's `bake_terrain`, whose double normals read
      it too. The bake's queries read it, and so do later point queries through the same cache.
    - A query that reads a cell not held reads it afresh and is refused as before, and the memo
      keeps only control points read whole. So under `SynthCache`'s rule (views of one field whose
      held cells only grow) no answer depends on it, refusals included. Its documentation now says
      the base elevation's refusals rest on that rule, as the channels' did.
    - The point query's `Fresh` reads each cell and takes it relative at once, as the one-value
      interpolant did, and a ghost's cells as its stencil names them (`fresh_ghost`). Two earlier
      versions were slower. Reading every control point through the memo's raw form made the base
      elevation alone 16% slower natively (13.37 ms against 11.56 ms a patch's sweep, at loads 3.4
      and 1.0). Reading in place but keying a ghost's four cells first (`ghost_stencil`) made it
      62% slower on V8 (14.08 against 8.67 ms, at loads 10 and 14). This one is the same as before
      within the loads' noise: natively 11.79 against 11.54 ms (loads 11 and 1.2), on V8 8.84
      against 8.61 ms (loads 14 and 9).
  - _Bits._ Unchanged, by construction and as tested: every value takes the same operations on the
    same operands in the same order, and the error is the same first cell not held. Every golden
    passes unchanged, with no re-bless and no bump (22): `tests/golden/height/` (the points
    through the fused pass, the parts through `Relief::at`, the bakes through the memo, the
    closed forms, levels and common figures), `ground/`, `material/`, `weights/`,
    `terrain_bake/`, `wire/` and R05's, natively (33 golden tests of the library) and on
    `wasm32-wasip1` and `wasm32-unknown-unknown` through the gate's suites.
  - _No contract moved._ No public signature, payload or golden changed; `SynthCache` keeps its
    API. The additions are crate-private: `CellRecords`, `interpolate_each`, `Controls`, `Fresh`,
    `ControlMemo`, `elevation_sample` (the base elevation's conversion, shared), `Grid` made
    `pub(crate)`, `relief::{ELEVATION_AND_STYLE, base_and_style, StyleSample}`, `Relief::evaluates`
    and `at_in_style`, and `SynthCache::start_bake`. The style's one-at-a-time interpolation
    (`smooth`, `Component`, `StyleValues`) is kept for the tests as the reference.
  - _Tests added._
    - `relief::tests`: the fused pass's base elevation and styles are each the one-value
      interpolant's, bit for bit, read afresh and through one memo across every point (its slots
      replaced as well as read again), at the seams of every synthetic world (a 5 × 5 grid over each
      face's corner cells and the cells halfway along its edges) and 300 points at random; a second
      query through a memo reads fewer cells; and with each point's reads less one, the fused pass,
      afresh and through a memo its neighbours filled, refuses the cell the base elevation does.
    - `synth::tests`: a cache that baked another world's patch bakes a cube corner's patch as a
      fresh cache does and answers later point queries, and the other world's, as a fresh one; and
      after a bake over a view of a cell and its margin, point queries about it (out to four king
      moves, the outer refused) are order independent and a fresh cache's, answered and refused,
      at the patch's level, its parent's and level 0, on the Earth- and Moon-like worlds.
  - _The bench,_ natively (`benches/synth.rs`'s `surface/point` and `surface/patch`, a part's
    sweep of a patch's 4,225 vertices ÷ 4,225, from the cache a first bake filled; one thread,
    release, schedutil, a 1,200% quota under the heavy lock; before on 2026-10-14 at a load of 1.2,
    after on 2026-10-15 at 11–18 in one hold with the bake and the WebAssembly bench below, 1,648 s;
    provisional):

    | Part, µs a point | Earth-like, 12 | Earth-like, 19 | Moon-like, 10 | Moon-like, 17 |
    | ---------------- | -------------: | -------------: | ------------: | ------------: |
    | base             |    2.73 → 2.79 |    2.73 → 2.74 | 0.290 → 0.291 | 0.290 → 0.289 |
    | relief           |   13.20 → 5.89 |   14.24 → 7.07 |   2.45 → 1.85 |   3.48 → 2.87 |
    | channels         |    0.82 → 0.84 |    0.84 → 0.84 |   0.24 → 0.24 |   0.24 → 0.24 |
    | craters          |    2.59 → 2.61 |    5.49 → 5.66 |   3.31 → 3.39 |   6.85 → 7.15 |
    | rims             |    0.04 → 0.04 |    0.05 → 0.05 |   0.45 → 0.43 |   0.45 → 0.43 |
    | height           |   19.60 → 6.49 |  23.75 → 10.48 |   6.94 → 5.87 | 11.64 → 10.57 |
    | patch, warm (ms) |   103.8 → 34.0 |   125.4 → 55.3 |   36.4 → 30.5 |   61.0 → 55.4 |
    | patch, cold (ms) |   109.4 → 39.7 |   130.1 → 60.1 |   37.7 → 32.0 |   64.9 → 59.1 |
    - `relief` is the point query's (`Relief::at`, its three styles fused, read afresh): the fused
      styles alone take it from 13.2 to 5.9 µs on the Earth-like world.
    - `height` reads the cache a first bake filled, and so the bake's memo: it is a bake's cost a
      vertex, not a cold point query's (about 9.7 µs at L + 4 on the Earth-like world, from the
      parts: the four values fused afresh, the octaves, channels, craters and rims).
    - The Earth-like patch's base elevation costs ten times the Moon-like's, before and after; the
      memo spares a bake nearly all of it.
    - The other parts' code is unchanged; they read up to 4% above before, the load's share. The
      2026-10-14 run of the code before `fresh_ghost` (the point query's ghost path; loads
      1.2–2.2) read them within 3% of before too, and the base elevation 1.5–5% below before, where
      this run reads it 0–2% above: the ghost path's change or the load, which these runs cannot
      tell apart.

    The bake on real terrain (R10's `bake/patch_65`, over the client's `PartialField`; the same
    conditions, before at a load of 1.1, after at 16):

    | Patch, ms            |      Weights |   No weights |        Double | Weights, cold |      Heights | Heights, whole |
    | -------------------- | -----------: | -----------: | ------------: | ------------: | -----------: | -------------: |
    | Earth-like, L + 4    | 160.1 → 60.0 | 137.3 → 36.5 | 476.3 → 145.2 |  163.3 → 65.8 | 134.1 → 35.2 |   104.0 → 35.0 |
    | Earth-like, 19       | 165.0 → 64.1 | 158.8 → 58.3 |             — |             — | 155.2 → 57.0 |   124.9 → 56.7 |
    | Moon-like rim, L + 4 |  60.0 → 51.6 |  40.8 → 32.1 |             — |             — |  39.3 → 32.1 |    36.5 → 32.7 |

    The 2026-10-14 run of the code before `fresh_ghost`, at a load of 1.1, read 59.4, 36.5, 143.6,
    65.3, 35.2 and 34.4 ms on the Earth-like world at L + 4, and 30.8 and 30.7 ms for the Moon-like
    rim's heights: the load's share here is up to 7%, on the rim's heights.

    In WebAssembly (T9's `surface/point` for the browser target, on Electron 44.4.3's V8 15.2
    through the shim, 20 samples a part, in the same hold; before at a load of 8.9, after at 13.7;
    provisional), per point, a patch's time ÷ 4,225, from a warm cache:

    | Part             | Earth-like, 12 | Earth-like, 19 |    Moon-like, 10 |    Moon-like, 17 |
    | ---------------- | -------------: | -------------: | ---------------: | ---------------: |
    | base             | 2.04 → 2.09 µs | 2.04 → 2.07 µs | 0.277 → 0.280 µs | 0.278 → 0.283 µs |
    | relief           |   12.89 → 6.55 |   14.27 → 7.24 |      3.02 → 2.32 |      4.39 → 3.65 |
    | channels         |    0.98 → 1.59 |    0.98 → 1.07 |      0.26 → 0.31 |      0.26 → 0.31 |
    | craters          |    3.70 → 4.82 |    7.83 → 9.19 |      4.68 → 4.70 |      9.79 → 9.76 |
    | craters-uncached |       97 → 100 |      202 → 207 |    1,132 → 1,135 |    2,805 → 2,808 |
    | rims             |    0.08 → 0.11 |    0.08 → 0.08 |      0.61 → 0.50 |      0.61 → 0.50 |
    | height           |   19.89 → 9.08 |  25.42 → 14.57 |      8.94 → 7.87 |    15.51 → 14.25 |
    - The base elevation is back at before's: the 3.32 µs at L + 4 on the Earth-like world (load
      1.1) of the version before `fresh_ghost` was its ghost path, a ghost's four cells keyed
      first, which `fresh_ghost` replaced.
    - The relief halves on the Earth-like world, and `height` (a bake's cost a vertex, as natively)
      falls from 19.9 to 9.1 µs at L + 4: a patch's height sweep is 38 ms, against T9's 84 ms.
    - The other parts' code is unchanged. On the Earth-like world at L + 4, the first parts run,
      the channels, craters and rims read 9–62% above before at the start's load of 13.7; the
      2026-10-14 run at a load of 1.1 read them within 5% (1.03, 3.77 and 0.09 µs). On the
      Moon-like world the channels read 18% slower and the rims 18% faster in both after runs,
      while natively they are unchanged (deferred, below).

  - _Design note 14's hash levers_ are not tried: as T8 found, the cost was the interpolation, and
    the bake's lattice cover already spares the hash.
  - _Review._ The determinism audit found no way a bit can change, on any target, in either path;
    its should-fix (an order-independence test over the memo's state, refusals included) is the
    second `synth::tests` test above, and its keying of the memo on the channels' `NetworkId`
    rather than the body and level is taken. Its other item, ending the memo when the bake's
    queries end, is deferred below. The Rust review's should-fixes are applied (the spline's sums
    a struct, the documentation's summaries and sections, the measurement cited at the memo, the
    tests' assertions and messages, the memo's hit path computing from its slot); `Fresh` stays
    apart from the memo's raw form, for the measured reason above.
- **Deferred findings (2026-10-13 rule)** (the fused pass; recorded, not ruled; each for "main"'s
  list).
  - _Still over the budget at the finest levels and in WebAssembly._ Natively, the Earth-like
    patch at L + 4 bakes in 34 ms, inside the brainstorm's 40 ms, but at 19 in 55 ms and the
    Moon-like rim at 17 in 55 ms. In WebAssembly the Earth-like height sweep is 38 ms at L + 4
    (9.1 µs a point, just inside the 10 µs and the 40 ms, at a load of 14), 62 ms at 19 and 60 ms
    on the Moon-like rim at 17 (14.6 and 14.2 µs a point). Against Design note 14's WebAssembly
    targets the relief (6.5 µs at L + 4, against 1.5) and the small craters (3.7–9.8 µs, against 2)
    still miss, and so does the point query's base elevation (2.1 µs, against 0.2), which the memo
    spares a bake. What is left is the octaves' noise (about 3 µs a point at L + 4, 4 µs at 19 on
    the Earth-like world, natively) and the small craters (2.6 µs at L + 4, 5.7 µs at 19; 3.4 and
    7.1 µs on the Moon-like world, the largest part there). Levers, none tried: the parent level's
    octave sum is a prefix of the patch level's (same terms, same order, from zero), so a bake
    could take its 1,089 parent relief heights from its own queries' sums; the small craters'
    search for a patch's vertices shares its cells, as the interpolant's did.
  - _The weights are now 39% of R10's bake_ (24 ms of 60 at the Earth-like L + 4, against 23 ms,
    14%, of 160 before): R10.T2's per-vertex site, R10.T6.a's own finding, is now the larger lever.
  - _The `PartialField`'s cost was the interpolant's reads._ R10.T6.a's 31% more a height over the
    client's field than over the whole (29% in this lane's before run: 134.1 against 104.0 ms at
    the Earth-like L + 4; 24% at 19, 8% on the Moon-like rim) is now under 1% (35.2 against
    35.0 ms; 0.5% at 19, none on the rim; 2.2% in the 2026-10-14 run at a load of 1.1): the memo
    reads each cell once a patch. The dense array with a held bitmap is not needed for the bake.
  - _The memo outlives its bake._ Point queries after a bake read it until the next bake's cover,
    so the base elevation's refusals there rest on `SynthCache`'s rule, which a caller reusing one
    cache for a view holding fewer cells would break (no caller does;
    `a_cell_not_held_is_not_surveyed` reuses one cache across shrinking views and bakes nothing).
    Ending it with the bake would need an end hook in R05's `HeightSource`, a public trait, since
    R05's and R10's bakes reach R09's cache only through `prepare_cache`.
  - _The memo knows its field as the channels' part does._ Two fields with the same body, level,
    figure and header marks but other cells would share it, as they would the channel networks;
    a header fingerprint (or the whole `FieldHeader`) would tell them apart.
  - _The bench's `height` part reads a bake's memo,_ since its cache is the one a first bake
    filled (T8's choice): it no longer measures a cold point query. A `height-afresh` part, from an
    empty cache each point, would.
  - _Unchanged parts move in WebAssembly._ Between the before and after modules, the Moon-like
    world's channels read 18% slower (0.26 → 0.31 µs) and its rims 18% faster (0.61 → 0.50 µs) in
    both after runs (loads 1.1 and 13.7), with their code unchanged and natively the same: V8's
    code generation or the module's layout, 0.2 ms more and 0.5 ms less a patch's sweep. A run
    that compares two modules cannot tell it from the change. Lean: nothing to do.
- **Deviations in T10.e, as built** (2026-10-15; lane r09-inputs, on `c769dc03`, merged to
  `298f51ca`; decision-bump-plan-after-22.md §1.5, with §1.2's `for_body` and §1.4's contracts;
  decision-r11-palette-liquids.md §2 item 5; no bump). Against the task's text, which
  docs-batch-18 wrote while the lane was building.
  - _Files._ `planetary/surface/inputs.rs` (the reading, its helpers and its tests),
    `planetary/surface/mod.rs` (the re-exports, the module text and its table of inputs by plan),
    and the server's `surface/service.rs`, whose `RECORD_TIME` is now the sim's (below).
  - _A reading beside `for_body`._ `CoarseInputs::from_record` takes `record: &BodyRecord`,
    `seasons: SeasonsFromRecord<'_>`, `crust: CrustInputs`,
    `condensates: impl IntoIterator<Item = Condensate>` and `palette: MaterialPalette`, and gives
    a `Result<CoarseInputs, ReadCoarseInputsError>`. It is re-exported from `surface`, with
    `SeasonsFromRecord<'a> { Own, Planet(&'a BodyRecord) }` (rust-dev's
    two-variant rule, in place of a bare `Option<&BodyRecord>`; the record-bearing form of plan
    14's `SeasonsFrom`). `for_body(record, ctx)` keeps Provides' signature, which freezes at 23,
    and its answer, `NotModelled(Surface)` at the uninhabited crust; it shares the tag's decision
    (`surface_of`) with the reading, and its documentation names `from_record` as what P14.T54.a
    connects. Also public: `RECORD_TIME`, `ReadCoarseInputsError` and
    `From<&hooks::figures::WetEpoch>` for the pass's `WetEpoch`.
  - _A moon's seasons_ come from its planet's record: `SystemContext` holds stars, not bodies, so
    it cannot give the planet's orbit (§1.5's "through `SystemContext` or the planet's record").
    A body is a moon by its kind, `BodyKind::Moon` with a body for its parent; a belt's member,
    listed under its belt, orbits the belt's host and takes its own orbit. The planet's record
    must be that body of that system, by `BodyId`.
  - _The record's time._ The task reads "T17's `RECORD_TIME`". `RECORD_TIME`, the epoch, is now
    the sim's (`planetary::surface::RECORD_TIME`), and T17's server constant is a `pub use` of
    it, so the time the server fetches records at and the time the reading takes the spin and the
    seasonal orbit at are one constant (the determinism review; the handoff had left this to
    T54.a). A record carries no time, so the reading checks only that the seasonal orbit's record
    holds then: `NotAtRecordTime` where its `valid_until` is at or before it, or its drift cell
    starts after it. P14.T54.a still settles the time.
  - _The seasonal orbit_ at `RECORD_TIME`, along its drift (P14.T45.a): eᵣ + ė Δt, nᵣ + ṅ Δt and
    aᵣ + ȧ Δt, `drifting_state_at`'s expressions, built by `KeplerElements::from_period` with
    P = 2π ÷ n and μ = n² a³, so that the period is the model's; the phase keeps the reference's
    mean anomaly, which nothing reads. At the epoch Δt is 0 for every record taken then (the
    cells are aligned on t = 0), so the record's elements pass to the bit, and they are the
    eccentricity, period and orientation that the record's own conditions were derived with
    (`fate.at(t).orbit()`). The drift branch serves a later `RECORD_TIME`; a test pins it bit for
    bit inside a generated cell.
  - _The spin_ is `SeasonalSpin::of(frame, RECORD_TIME, seasonal, whose)`, the obliquity,
    sidereal period and mean solar day that the record's conditions read, with the equinox's true
    anomaly ν = atan2(p̂ · P, −p̂ · Q), Q = n̂ × P (`equinox_true_anomaly`), in [0, 2π), and 0
    where |p̂ × n̂| < 10⁻¹² (`EQUINOX_MIN_TILT`, a numerical guard). Checked on Earth's 77.05281°
    (ϖ 102.94719°, NASA's Earth fact sheet, J2000) and Mars's 109.0° (perihelion at Ls 251.000°,
    Allison and McEwen 2000, eqs. 16–17, through Mars24). North is the frame's pole, the spin's
    angular momentum by the right-hand rule; for a retrograde spin it is the IAU's south, which
    `Spin`'s documentation now says.
  - _The hosts' light_ is the conditions' `stellar_flux`, P14.T12's orbit average over all hosts
    at the record's time: the reading takes no context, and `for_body`'s stays unread. It is exact
    for one host. A companion star's share and a moon's planetshine keep the seasonal orbit's
    cycle, not their own, as `host_flux`'s documentation now says, so T10's "A second star's own
    cycle is lost, which the reading of the record adds" is not met, and Provides' "the context's
    stars for the host flux over the orbit" is not what is read (deferred below: carrying each
    host's share would change `CoarseInputs`).
  - _The rest of the record._ The surface section's state, material, climate regime (by
    `From<&hooks::surface::ClimateRegime>`), conditions (the mean temperature, the two signed
    contrasts, the pressure, and the gases in the record's order by row key) and figures (σ_h,
    the liquids and ices, the tectonic regime and f_c, volcanism, heat flow, the surface age in
    Gyr by the crater contract's ÷ `YEARS_PER_GIGAYEAR`, the craters and the wet epoch); the bulk
    section's surface gravity; the figure section's spheroid. A bulk, figure, rotation or orbit
    section that is `NotModelled` or `NotApplicable` reads as `NotModelled(name)` (a body that
    orbits nothing has no seasons this reading models), and one `NotResolved` as
    `NotResolved(name)`; no generated surface reaches these, since the surface section needs a
    seasonal orbit.
  - _Airless is zero pressure._ An `Airless` state writes exactly +0 (decision-r11-palette-liquids
    §2 item 5; an exhaustive match on `SurfaceState`), and every other state keeps the record's
    pressure. Every airless record of the census already states 0 (562 of 562), so the rule guards
    the header's contract and moves nothing.
  - _A share of no area_ (±0: the record lists water at zero on a dry world, P14.T24.b) is not one
    of the inputs' liquids or ices and needs no palette entry; a negative or NaN share passes, for
    the builder to refuse.
  - _The wet epoch_ carries its start, end, effective flow and paleo-inventory to the bit; its
    runoff (Design note 3's `runoff`, which R09.T14.c's dry-now branch reads) and its liquid
    (water alone, P14.T48.b) are not carried, since the pass's struct has no member for them.
    `WetEpoch::effective_flow`'s documentation now states science-hoke.md's 5% reference, in place
    of the "arid-to-semiarid Earth's" which that ruling withdrew.
  - _Errors._ `ReadCoarseInputsError`'s variants are `Section(SurfaceInputsError)`,
    `PlanetMissing(BodyId)`, `NotItsPlanet { planet: Option<BodyId>, given: BodyId }`,
    `NotAtRecordTime`, `SeasonalOrbit(BuildOrbitError)` and `Build(BuildCoarseInputsError)`, with
    lower-case `Display` texts, `source` giving `SeasonalOrbit`'s and `Build`'s causes, and `From`
    for the first and the last.
  - _Tests_ (`inputs.rs`): the equinox on Earth, on Mars and on any orbit and pole; a generated
    planet read to the bit, its spin against its frame and its light against `luminosity_flux`; a
    moon's seasons from its planet's orbit, with the refusals (`Own` for a moon, the moon given as
    its own planet, a namesake from another system, a contact's withheld orbit, a planet's record
    given for a planet); the drift cell (the model's eccentricity and period bit for bit
    mid-cell, its axis to 10⁻¹⁴, `NotAtRecordTime` before the cell and from its end, and a
    holding orbit's end); the section tags; a share of no area; the refusals and their texts;
    and the census of 40 sample systems, every solid body read with a palette of what lies on the
    ground only, and `for_body` still `NotModelled(Surface)` for each (657 bodies: 88 moons, 562
    airless, 95 with air, 141 on drifting seasonal orbits, and 630 listing a share with no area).
    The doctest searches Sun-like systems 1–63 for a dry rocky planet (candidate 1 holds no rocky
    planet, and a pinned index would move at 23) and builds its palette from the record's ices.
    R09.T2.e's `PaletteEntry::liquid` landed while the lane was building: the doctest's literal
    takes `liquid: None`, its planet is dry because a sea's entry needs the liquid's optical record,
    and the tests' entries take T2.e's `entry` helper, whose liquids carry its methane record.
    Run: `cargo test -p hyperion-sim --lib planetary::surface::inputs` and
    `cargo test -p hyperion-sim --doc planetary::surface`.
  - _No output moves:_ no golden, stream, tag, wire or version (`GENERATOR_VERSION` 22);
    `golden_diff.py` reports nothing to reconcile.
- **Deferred findings (2026-10-13 rule)** (R09.T10.e; recorded, not ruled; each for "main"'s list).
  - _`for_body`'s signature_ (a contract: with a decision agent, through "main").
    `for_body(record, ctx)` cannot carry a moon's planet's record, and `ctx` holds no bodies and
    is not read. Lean: P14.T54.a widens it to
    `for_body(record, seasons: SeasonsFromRecord<'_>, ctx)`, or drops the context;
    `SurfaceInputsError` gains arms for `from_record`'s refusals (`PlanetMissing`,
    `NotItsPlanet`, `NotAtRecordTime`, `SeasonalOrbit`, `Build`), or `for_body` answers
    `ReadCoarseInputsError`, which T17's and T19.a's mapping of refusals to the wire then reads;
    and the server's `InputsSource::inputs(record, ctx)` and `RecordInputs` fetch a moon's
    planet's record at `RECORD_TIME`. The signature freezes at 23. Not built here.
  - _Each host's own cycle_ (T10's "which the reading of the record adds"; it would change
    `CoarseInputs`, so it is "main"'s to route). A companion star's share and a moon's
    planetshine follow the seasonal orbit's (a ÷ r)² cycle: their annual mean is exact, and their
    seasonal shape is off by about ±2e of the share, under 0.5% for the Solar System's moons and
    some ±10% for a moon of a young, eccentric giant (the science check). Lean: R09.T13 adds a
    member for the shares on other cycles when its climate reads them.
  - _The wet epoch's runoff and liquid._ R09.T14.c adds the member it reads; plan 14's T54.a text
    still names that mapping as its own.
  - _A record time off the epoch._ If P14.T54.a moves `RECORD_TIME`, the seasonal orbit is the
    drift cell's linear model there (within `DRIFT_TOLERANCE` in position), while the record's
    conditions took the fate's exact orbit then, so the spin and the conditions differ by the
    model's error. Lean: keep it; or T54.a reads the conditions' own seasonal orbit if the record
    gains it.
  - _Airless craters_ (the plan-conformance review; unchecked). The crater contract screens by
    the derived air's pressure, P ÷ g where it is above 0 (`hooks/figures.rs`), not the
    conditions' pressure, which states 0 on every airless record of the census; whether an
    `Airless` state's air can still screen is plan 14's to check. Lean: keep the contract as the
    record states it.
  - _No golden_ pins the equinox or `from_record`; P14.T54.a's connection adds them with its bump
    (the determinism review).
  - _`hyperion-base`'s `math::atan2`_ documents its range as (−π, π]; libm gives −π for
    atan2(−0, x < 0). Documentation only; lean: the next edit to base.
- **Deviations in T10.f, as built** (2026-10-15; lane r09-inputs, on T10.e's `cd1a09fb`, merged
  to `9fbc5ea2`; decision-backlog-2.md §3, with §2.5's sentence; no bump).
  - _The signature is the ruling's._ `for_body(record, planet: Option<&BodyRecord>, ctx)`, not
    T10.e's `SeasonsFromRecord`, which main asked to weigh: the argument is the planet's whole
    record, which T13's planetshine and eclipses will also read (§3.2), so a type named for the
    seasons would misname a signature frozen at 23; `planet_of`'s `Option<BodyIndex>` maps onto it
    one for one (`Some` exactly where `planet_of` is), so the server's fetch is
    `planet_of(..).map(fetch)`. So rust-dev's two-variant rule, which T10.e applied to
    `from_record` in place of a bare `Option<&BodyRecord>`, yields here to the ruling's form (a
    ruled exception, not one the rule allows); at the one production call the argument is that
    fetched record. `from_record` keeps `SeasonsFromRecord`, whose variants say whose orbit sets
    the seasons. One private check, `checked_seasons`, serves both:
    `for_body` turns its `planet` into the `SeasonsFromRecord` it will pass to `from_record` from
    P14.T54.a, and `from_record` checks its own through it, as T10.e did.
  - _`level_from` and `field_albedo_scale` take no planet._ §3.3 lists them among the functions
    that pass the planet on, but neither reads a record's planet: `level_from` takes the source's
    answer, which has read it, and `field_albedo_scale` the body's own photometry.
    `surface_record` returns the planet's record, and `compute` and `level_of` (the job
    `field_level` runs) pass it to the source.
  - _A moon's planet that does not resolve_ (the ruling is silent) is answered
    `Inputs(PlanetMissing(planet))` by `surface_record`, logged at `error` with its
    `ResolveBodyError`, so `internal`; a moon's own record rules it out. `Body(..)` would have told
    the client its own body does not resolve. The refusal is then logged again at `error` by the
    handler or the prefetch, without the cause: two lines for one fault on a path no generated
    record takes, kept so that the cause is logged where it is known.
  - _Already in place._ §3.3's one time constant needed no change: T10.e had made the server's
    `surface::RECORD_TIME` a `pub use` of the sim's.
  - _A planet's record for a body that is not a moon_ is refused
    `NotItsPlanet { planet: None, .. }`, as T10.e's `from_record` refuses `Planet(..)` for a
    planet: §3.1's "`Some` exactly where `planet_of` is", held in both directions.
  - _The logs._ A prefetched pass refused other than by a section's state is logged at `error`
    (at `debug` before, with every `Inputs`), as §3.3's "logged". `surface_refusal` logs such a
    refusal's text and variant, and answers it with the withheld section's `internal` message,
    "the server failed while reading the body's record" (`reading_failed`).
  - _Tests._ Sim (`inputs.rs`): `for_body_checks_a_moon_s_planet_before_the_crust` (a generated
    moon with its planet, without and with another body; a planet with none and with one; the
    first belt's member with a surface, a dwarf planet whose parent is a belt, with none and with
    its belt's record; the tag first, for a giant given a planet and a degraded moon given none);
    the census also checks `planet_of` against the test's own statement of the rule on every body
    of the 40 sample systems (since `from_record` itself goes through `planet_of`, comparing the
    two would test nothing), and `for_body` with `planet_of`'s planet on every solid body; T10's
    and T10.e's tests and `from_record`'s doctest take the new form, and `for_body` gains a
    doctest, a generated moon with its planet's record from `planet_of` and without it. Server:
    `a_moon_s_source_is_given_its_planet_s_record` (`surface/service.rs`: `field` and
    `field_level` give a capturing source the moon's planet's record, equal to `body_at` at
    `RECORD_TIME`, and the planet `None`);
    `a_planet_s_record_missing_is_answered_internal` (`requests/surface.rs`: `survey_pass` and
    `surface_field`, nothing recorded); `surface_refusal`'s table gains the five other variants;
    `level_from`'s test passes `PlanetMissing` and `NotAtRecordTime` on; and
    `a_generated_surface_is_not_modelled_naming_its_section` (`tests/surface_service.rs`) adds a
    generated moon (`common::surface::moon_system`, found by kind and parent), which
    `RecordInputs` answers `NotModelled(Surface)` through the service and `PlanetMissing` without
    its planet.
  - _Files_ as ruled, with the surface crate's `field/header.rs` for §2.5's sentence on the
    header's `surface_pressure`.
  - _No output moves:_ no golden, stream, tag, wire or version (`GENERATOR_VERSION` 22).
- **Deferred findings (2026-10-13 rule)** (R09.T10.f; both already ruled, recorded for the next
  readers).
  - _T10.e's deferred "`for_body`'s signature"_ is settled here (decision-backlog-2 §3):
    `for_body` answers `ReadCoarseInputsError`, so `SurfaceInputsError` gains no arms.
  - _One fetch for both records_ (§3.3's deferred efficiency, ruled there): a moon's `body_at`
    derives its planet's record and drops it, and `surface_record` derives it again.
  - _The plan's older text_ (§4's docs batch, ruled there) still reads the old forms: Provides'
    `for_body(record, ctx) -> Result<Self, SurfaceInputsError>` and `SurfaceInputsError` as its
    answer, and `InputsSource`'s `inputs(&self, record, ctx)`, answering `SurfaceInputsError`;
    T17's as-built `SurfaceJobError::Inputs(SurfaceInputsError)`. _Corrected 2026-10-16 by
    docs-batch-20: Provides, the `InputsSource` paragraph and T17's record now read R09.T10.f's
    forms._
- **Deviations in T12.a, as built** (2026-10-16; lane `lane/40-r09-t12-coarse-elevation`, on
  `origin/rendering-and-planets` at `f27d0248`; rulings `decision-r09-t12-coarse.md`, items 1 and 5;
  no bump).
  - _Files and items._ `steps/relief.rs` (the step, `CONTINENTAL_MEAN` public, the skew and the
    interim figures), its children `relief/seafloor.rs` (the sea floor's laws), `relief/noise.rs`
    (the coarse landform noise) and `relief/tests.rs`. `CellState` gains
    `seafloor_age: Option<Seconds>`, `τ_ss` on oceanic crust; `SeafloorOrigin::age`'s
    documentation now calls it the kinematic age. R09.T11's `reach` (its multi-source shortest
    path) is generic over a region label and `pub(super)`, as are its `Reach`, `slot` and
    `cell_of`: T11 calls it with the plate labels as before, so its arithmetic and its tests are
    unchanged, and T12.a calls it over each plate's oceanic cells. `crates/hyperion-sim/clippy.toml`
    admits `GlobSed`.
  - _The old floor (ruled, item 1)._ As ruled: κ = `d_r` ÷ `u_h` uncapped as T11 divides them,
    sorted by (κ, cell), blocks of equal κ, the ridgeless cells the last block; each block's mean
    age under the triangular law; `t_m` = 192.9 Myr × (`a_o` ÷ 0.595) × (Ω⊕ ÷ Ω) through T11's
    `angular_speed_scale_rad_per_s` and `EARTH_ANGULAR_SPEED_RAD_PER_S`, at most the contract's
    system age (`seafloor::oldest_floor_age`); 0.595 is one less plan 14's
    `EARTH_CONTINENTAL_FRACTION`. The drape, H and the plains read `τ_ss`; H₀ reads `u_h`. T11's
    `older_sides` keeps the kinematic τ.
  - _The depth._ z = −`d_GDH1`(`τ_ss`) + L(S) + `n_o`, S = `S_p` + `S_t`, as ruled; `ρ_m` is a
    named `MANTLE_DENSITY` (3,300 kg m⁻³) in `relief/seafloor.rs`, the sediment's constants Sclater
    and Christie 1980's shale (`SEDIMENT_*`), seawater 1,030 kg m⁻³. The noise `n_o` is the
    landform noise made zero-mean and unit-RMS over the oceanic cells, times √max(0, (930 m)² −
    s²), 930 m plan 14's `OCEANIC_WIDTH`. The latitude the drape reads is the cell centre's
    geodetic one, from the figure's normal.
  - _The continental population (a lean, technical)._ Plan 14's `CONTINENTAL_WIDTH` (1.08 km)
    about `CONTINENTAL_MEAN` (+405 m), not a Gaussian but a shifted log-normal of shape
    `CONTINENTAL_SKEW` = 0.1784, whose mode at that mean and width is Earth2014's +125 m (125.06 m):
    a Gaussian of mean +405 m puts its modal 250 m bin at 375 or 625 m with nearly even odds, the
    second failing the land-mode test. The realised population is made exactly that mean and width
    over the continental cells (before T12.b's belts).
  - _A stagnant lid's population (a lean, then ruled: `decision-r09-t12-coarse.md`'s addendum of
    2026-10-16)._ Every regime T11 leaves on one lid (`StagnantLid`, `HeatPipe`, `Episodic`,
    `IceShell`, and a mobile lid it cannot lay out) takes one Gaussian population about the datum,
    of plan 14's `STAGNANT_LID_RELIEF` (0.9 km), its area mean zero and its area RMS the constant
    exactly, drawn before T12.c and T12.d: Design note 3's structural share on such a lid, which no
    other step realises (the synthesis's share below the cell is about 60 m), and plan 14's own
    `Hypsometry::One`. As ruled, its field is red, not cell-scale noise: `noise::red_field`, the
    octaves from lattice level 1 (about degrees 2–3) to the field's, each octave's RMS
    2^(−(β₁ − 1) ÷ 2) of the next coarser one's, β₁ 2.0 on silicate crusts and 1.8 on ice-rich ones
    (`relief::crust_shape`: an ice surface or an ice shell is ice-rich, as T12.e's anchored
    spectrum will choose), its degree-0 and degree-1 parts removed by area-weighted least squares
    on 1, x, y, z. Degrees 2–5 carry 68.4% of the reference Moon's lid variance (the ruling's
    about 72% at β 2, Turcotte 1987). It writes only `Lid`, so an undifferentiated body names no
    entry it lacks. Built in the batch's fix commit, after T12.a's.
  - _The noise and its draw layout._ `surface.coarse.relief`, keyed
    `surface_cell(face, ℓ, i, j, 0)` of each lattice cell at levels ℓ = 2 to the field's, one
    standard normal (two words) each;
    a cell's octave is the Shepard mean of the lattice values within the chord 1.8 × 2⁻ℓ of its
    centre, under (1 − r² ÷ h²)³, over the lattice cells within two rings of its own, in ascending
    lattice index; each octave zero-mean and unit-RMS, weighted 2^(−(ℓ − 2) ÷ 2) (a per-degree
    variance ∝ l⁻², the silicate β₁ = 2.0 carried above the coarse cell; an illustration), and the
    sum made zero-mean and unit-RMS. Instance 0 is the landform noise; T12.b and T12.c take others.
  - _The e-folding length, calibrated._ 200 km, not the lean's 350 km: on the reference Earth it
    gives the plains (style amplitude ≤ 0.2) 32.8% of the oceanic crust below 3 km (Harris et al.
    2014's 32.9% of the abyss) and 370 m of `S_p` + `S_t` beyond 200 km of a margin (GlobSed's
    404 m); 225 km gives 36.3% and 399 m, 350 km 50.6% and 548 m, 500 km 63.1% and 718 m
    (`calibrate_the_ponded_sediment_s_e_folding_length`, by hand). 200 km is the low end of
    ordinary margins' 200–500 km.
  - _Goff's Table 2 (ruled, item 5a)._ The test holds H₀ within 30% of the mean of the entries at
    each full rate, every entry listed in its comment: 222.8 m at 28 mm a⁻¹ (235.8 m, −5.5%),
    191.2 at 40 (170.45, +12.2%), 62.4 at 75 (61.7, +1.1%), 55.1 at 110 (56.1, −1.8%) and 55.0 at
    160 (58.7, −6.3%). Untested: the fit's 207.3 m at 35 mm a⁻¹ against the southern MAR's 222 m
    (−6.6%) and 116.0 m at 55 against PAR fast's 90 m (+28.9%).
  - _`09`'s T12.a text edited_ (§2.3.5): the hypsometry clause (lines 2240–2245: the ocean's modal
    bin among the centres −5,125 to −4,125 m and the mean below −2 km from −4,731 to −3,981 m, in
    place of "an ocean mode within 600 m of −4,375 m" and "an oceanic mean within 300 m of −4,281
    m"), the ridge clause ("the unloaded basement is GDH1's at 5, 20 and 100 Myr to 1 m") and the
    Goff clause ("of the mean of … Table 2 entries … the axial high alone"), each citing the ruling.
  - _Measured on the reference Earth_ (level 8, at the step's level, before T12.b and T12.e):
    `a_o` 0.5950 and `t_m` 192.9 Myr; `τ_ss`'s area mean 64.30 Myr, `t_m` ÷ 3 to 10⁻⁹ (Earth's 64.3),
    T11's kinematic mean 110.2 Myr with 27.6% at the cap; `τ_ss` older than 120, 150 and 180 Myr
    14.28%, 4.95% and 0.00% (Müller's 15.2%, 4.7% and 0.22%: the oldest blocks, the ridgeless
    cells among them, take one mean age each, so the tail past about 160 Myr is lost). The
    hypsometry: land mode +125 m; ocean modal bin −4,625 m; below −2 km mean −4,530 m, s.d. 930 m,
    quartiles −5,177, −4,572 and −3,928 m; the oceanic crust's mean −4,524 m and s.d. 942 m. A
    test-only probe on T11's kinematic ages (the same noise): land +125 m, ocean modal bin
    −4,875 m, mean −4,738 m, s.d. 908 m, quartiles −5,350, −4,804 and −4,211 m. Mean S over the
    oceanic crust 593 m and mean load L 362 m; the floor's own s.d. 716 m, so `n_o`'s RMS is
    593 m (about 0.60 km). T11's oceanic ridge production `P_o` = Σ (π ÷ 4) s `u_h` 0.0674 m² s⁻¹
    against the law's 2 `A_o` ÷ `t_m` 0.0997, a ratio of 0.676 (the ruling estimated 0.55–0.60);
    divergent boundaries 47,876 km on oceanic crust and 29,853 km on continental (38% rifts).
  - _Interim figures, until T12.e._ No sea is placed: every cell is dry (its water surface its
    ground), the sea level is the lowest elevation, and the realised `σ_h` and relief are the
    cells' own (on the reference Earth after T12.a 2,618 m, 12,542 m and −7,192 m). T12.e replaces
    them with the reconstructed field's, places the sea at plan 14's ocean fraction, fills it and
    names the liquid, and anchors `Working::spectrum`.
  - _Tests._ In `relief::tests`: GDH1 to a metre at 5, 20 and 100 Myr; L ÷ S at 100 m and 2.4 km to
    10⁻³; H₀ against Goff's means; `S_p` 468 m at 81 Myr; the wedge's fall and end; `t_m` (192.9
    Myr, ×4 at half the heat flow, the system's age without it); `τ_ss`'s mean `t_m` ÷ 3 to 10⁻⁹,
    monotone in κ, equal for equal keys, the ridgeless oldest; the hypsometry; the plains' share
    and the far sediment; the wedge crossing no boundary (on the reference Earth and on hand-set
    plates); the populations' widths and the skew's mode; a stagnant lid's population and the
    interim figures; the same seed twice; the noise's unit field. Two measurements and the
    calibration are `#[ignore]`d, run by hand. `surface/mod.rs`'s
    `a_no_op_pass_yields_a_valid_field_twice_with_identical_bytes` is renamed
    `the_pass_yields_a_valid_field_twice_with_identical_bytes` and loses its all-zero elevation.
- **Deviations in T12.b, as built** (2026-10-16; the same lane, on `5622cb95`; rulings
  `decision-r09-t12-coarse.md` items 2 and 5d and its addendum of 2026-10-16, items 2 and 3; no
  bump).
  - _Files and items._ `relief/convergent.rs`, a `pub(crate)` module: `EARTH_GRAVITY` and
    `height_factor` (for T12.e), the landforms' constants, `belt_lift`, `trench_lift` and the arcs'
    layout. `CellState` gains `subducts: bool` (ruled, the addendum's item 2), appended last, which
    T11's `Layout::write` sets from the nearest boundary's `Boundary`. T11's transform gains
    `reach_from`, sources with start distances; `reach` calls it with zeros, so T11's arithmetic
    and tests are unchanged. `reference::earth_like` takes its gravity from `EARTH_GRAVITY`. Tests:
    `plates::tests::each_cell_carries_whether_its_plate_subducts` (new), and T12.b's in
    `relief::tests`.
  - _The height factor (ruled, item 2)._ h = g⊕ ÷ g, g⊕ the reference Earth's GM ÷ R² from the
    literal expression `earth_like` reads, 9.820 25 m s⁻²: exactly 1 on the reference Earth,
    2.6343 on Mars, 6.0461 on the Moon and 34.5933 on Ceres, and half at 2 g⊕ (tested). It scales
    the belts' plateau, the trenches' depth and the arcs' rises, no distance or width. Design note
    7's "× the lithosphere factor" is a plan error, for a docs batch.
  - _Belts._ On continental crust whose nearest boundary is a collision, a plateau profile of |d|
    over a half-width of 175 km + 50 km × n (100–250 km, a full width of 200–500 km), flat to half
    the half-width and a cosine shoulder beyond, lifting the population by h × (5,000 m − its core
    mean), so that the plateau stands at 5 km (Fielding et al. 1994's 5,023 m, abstract read by
    the science check). Only continental crust is lifted (labelled). On the reference Earth the
    belts cover 4.1% of the continental crust; its cells within 50 km of a collision average
    5,143 m, the highest 8,755 m.
  - _Trenches._ On the subducting side (`subducts`), h × (3 km + 2 ÷ 3 km × n) at the boundary
    cells, falling as (1 − |d| ÷ 100 km)². On the reference Earth, 1,267 axis cells: median depth
    −7,918 m, deepest −10,498 m; below their plate's floor 150–300 km out, median 3,070 m and
    10–90% 1,588–4,221 m.
  - _Arcs and fronts._ Measured from the trench's axis, the edge the two sides share (the
    addendum's item 2): `reach_from` over the overriding plate from each overriding boundary cell
    of a subduction, started at half its link to its nearest subducting boundary cell across, and
    each reached cell reads the great circle to its source's axis point (the two centres'
    normalised midpoint), not the path. G is 190 km or 260 km by the source's crust, × (1 + 0.2 n)
    at the source; the band W 60 km or 175 km by the same crust; the profile a cosine rise over W ÷
    4 before the front, flat over W ÷ 2 and a cosine fall to G + W; the rise 3 km (oceanic) or
    2.5 km (continental) × h over the cell's own crust (illustrations, low confidence). The G
    constants carry the ruling's label and segment lists (item 5d). On the reference Earth: 1,461
    oceanic front cells, median 188 km, 10–90% 141–236 km, 87% within 140–250 km; 177 continental,
    257 km, 197–291 km, 100% within 180–330 km. The 8-connected path's excess at the fronts is
    4.7% at the median, 11.0% at the 90th percentile and 30.9% at most: 9 km and 21 km at 190 km,
    which would threaten the 15 km medians, so the arcs read the great circle (corrected, as the
    issue's lean asked).
  - _The along-strike draw._ `surface.coarse.relief`, instance 1, one octave at lattice level 5,
    made unit-RMS over the sphere and clamped to ±1.5; belts and trenches read it at the cell and
    arcs at their source, so each profile across a trench takes one G.
  - _The continental population, netted (ruled, the addendum's item 3)._ With the landforms on
    T12.a's population (+405 m, 1.08 km, skew 0.1784) the reference Earth's continental crust had
    a mean of +581 m, an s.d. of 1,382 m and a modal bin at +375 m, above the addendum's +505 m and
    1,188 m. So the population is now a core: mean `CONTINENTAL_CORE_MEAN` 222.6 m and width
    `CONTINENTAL_CORE_WIDTH` 628.5 m = √(1,084² − 883.2²), the landforms' lift there averaging
    182.4 m with an s.d. of 883.2 m, and `CONTINENTAL_SKEW` refitted to 0.1047 (mode +125.00 m),
    calibrated once on the reference Earth under `reference::SEED`
    (`calibrate_the_continental_population_net_of_the_landforms`, by hand: the belts' shape averages
    0.035 09 over the continents and the arcs lift them 14.7 m, and the belts lift by h × (5,000 m
    − the core's mean), so the mean solves μ + 0.035 09 (5,000 m − μ) + 14.7 m = 405 m). The crust
    with its landforms then has a mean of 405 m, an s.d. of 1,090 m and a modal bin at +125 m.
    `CONTINENTAL_MEAN` stays Earth2014's +405 m, the crust's whole. T12.a's population test now
    holds the core exactly and the crust with its landforms within 100 m and 10%.
  - _T12.a's hypsometry with T12.b's landforms:_ land mode +125 m; ocean modal bin −4,625 m; below
    −2 km mean −4,559 m (the trenches' tails 29 m of it), s.d. 960 m, quartiles −5,193, −4,581 and
    −3,941 m; the kinematic-age probe's mean −4,767 m. Interim figures: `σ_h` 2,636 m, relief
    19,376 m, sea level −10,498 m.
  - _Slivers and transforms (measured; T11's raster unchanged)._ On four reference Earths
    (`reference::SEED`, `0x0dd5`, `0x5eed_0001` and `0x5eed_0002`), the plates whose widest interior
    point lies within 100 km of their boundary number 9 of 51, 15 of 49, 10 of 41 and 10 of 56, all
    tail plates of 0.002–0.008 sr, 63–197 km across, and two a single row of boundary cells.
    Transforms take 27.2–31.1% of the boundary length (Earth's 28.4%, Bird 2003) in 42–59 runs
    averaging 2,288–2,733 km: long arcs, no fracture zones, as T11 found.
  - _Science-check corrections_ applied with this commit to T12.a's documentation: GDH1 read
    (Stein and Stein 1992, p. 126, sediment-corrected depths, p. 124); the shale's φ₀ and c verified
    through Sheldon and Retallack 2001's Table 1, its grain density not read (a secondary table
    gives 2,700 kg m⁻³); GlobSed's eq. 2a prints 52 m where its Table 4 gives 53.02; its 404 m is
    the deep ocean's, farther than 200 km from the continent–ocean boundary, as the test reads it;
    Goff's Table 2 has 222 m at 35 mm a⁻¹ and 90 m at 55, which the test's comment now lists.
  - _The trench tests' reading_ (the review). `trenches_lie_two_to_four_km_below_their_floor` holds
    each subducting boundary cell's own cut at 2–4 km × h, the floor being the cell's population
    before the cut; `the_trenches_median_depth_below_the_floor_beyond_is_two_to_four_km` holds the
    finished field's median depth below its plate's floor 150–300 km out (3,070 m) within 2–4 km.
    There the 10–90% range is 1.6–4.2 km, so a bound on more than the median is main's to ask.
  - _Belts off Earth._ The plateau stands at μ + h (5,000 m − μ), μ the core's mean, which is 5 km
    × h only where h is 1 (labelled).
  - _The new `subducts` test_ runs under T11's acceptance filter (`steps::plates`), not T12's.
- **Deviations in T12.c, as built** (2026-10-16; the same lane, on `a4dcf814`; rulings
  `decision-r09-t12-coarse.md` items 3 and 4; the province law is the lane's own, not put to main;
  no bump).
  - _Files and items._ `relief/provinces.rs`, a `pub(crate)` module: `elastic_thickness`,
    `conductivity_integral`, `mantle_density`, `foundation_n_per_m3`, `flexural_parameter`,
    `step_response`, `surface_lift_m`, the cone field and its constants; `relief::run` calls it
    after a stagnant lid's population. Tests in `relief::tests`.
  - _The province law (the lane's; sources in the module)._ Plan 14's own constructional model,
    below its saturation: edifices at the edifice cap `h_max` = 21.9 km × (3.71 ÷ g) × min(1,
    `T_e` ÷ 70 km) (plan 14's `TALLEST_EDIFICE_RELIEF`, Plescia 2004), cones of basal radius
    `h_max` ÷ (21.9 ÷ 370) (Olympus Mons's relief over the mean half of its 840 × 640 km base,
    Plescia 2004, Table 1 and ¶32), no narrower than the boundary diameter; covering the share f of
    the surface whose cone field has σ_volc's RMS, f ÷ 6 − f² ÷ 9 = (σ_volc ÷ `h_max`)² (the RMS of
    plan 14's `VOLCANISM_SATURATION` argument, ¾ at its V bound), so f is V's alone (0.10 at V =
    1); gathered into a Poisson count of 4 V provinces (Mars's Tharsis, Elysium, circum-Hellas and
    Syrtis Major: Werner 2009 and Plescia 2004), each centre uniform, the edifices dealt in turn and
    set off by a two-dimensional normal of angular s.d. `r_e` √(n ÷ P) ÷ R; overlapping loads
    summed and held at `h_max`. Where overlaps held at the cap leave the realised s.d. short of
    σ_volc, one correction widens f by the variances' ratio (at most ¾) and lays the field again. Mobile lids
    get no provinces; nor do worlds of no volcanism (the reference Moon's maria and Ceres are not
    modelled, V = 0), nor lids of no rule (below), nor palettes with no `Province` entry.
  - _The flexure (ruled, item 3)._ α = [4D ÷ ((`ρ_m` − `ρ_fill`) g + E `T_e` ÷ R²)]^¼, `ρ_fill`
    being `ρ_p` since the addendum 2 (below), D = E `T_e`³ ÷ 12(1 − ν²), E 100 GPa and ν 0.25 (Belleguic et al. 2005's Table 2, read by the science
    check). DN7's line load is applied to an edifice along its diameter, as if it were an infinite
    ridge of its own cross-section (labelled; the axisymmetric Kelvin-function solution is not
    used): 64 strips of uniform load, each convolved exactly through the line load's step response
    (1 − ½ e^(−t) cos t under the load, ½ e^(−t) cos t beyond, a step at α = 0, the local Airy
    balance), tabulated at 512 distances out to the base plus 2πα (held to πR). `ρ_m` by the lid
    entry's family: `Silicate` the named `MANTLE_DENSITY` (3,300 kg m⁻³, the sediment's too);
    `WaterIce` the lid entry's own grain density; `Metal`,
    `VolatileIce`, `Salt` and `Organic` no rule and no provinces (a closed set for the composition
    audit). A cell takes the `Province` crust where its load or the fill of its deflection is at
    least 10 m (an illustration); a moat below a sea is not modelled, since no sea is placed before
    T12.e.
  - _The filled moat and the planed bulge (ruled: `decision-r09-t12-coarse.md`'s addendum 2,
    2026-10-16, built in the lane's second fix commit, and its addendum 3, 2026-10-19, built in the
    third, each after a push)._ The first rule, Δz = max(h − w, 0) with the load at
    its grain density, counted the fill twice under a load (the science check's must-fix, commented
    on the issue): the uniform foundation already puts fill-density rock beneath the load, and a
    load of 3,310 kg m⁻³ is denser than the 3,300 kg m⁻³ mantle, so a cap-height edifice stood
    nowhere. Now the load and the fill share one density `ρ_p`, the `Province` entry's grain
    density × 0.8 (2,648 kg m⁻³ on the reference Mars), asserted below `ρ_m` (in
    `flexural_parameter`, with `Lid::new`'s guard before it); the load is q = `ρ_p` g h, h its height
    above the filled level, as the cap's calibration on Olympus Mons's present 21.9 km reads it
    (21.8 km is `h_max`'s 21,795 m on the reference Mars). The second fix commit raised the
    surface by Δz = h + max(−w, 0), the bulge standing; main's addendum 3 (2026-10-19, built in the
    lane's third fix commit) planes it: Δz = h (`surface_lift_m`, h the summed load held at
    `h_max`), at h under a load and at the filled level off the loads, in the moat and on the bulge
    alike, the uniform foundation's exact surface, since with the bulge planed the load is `ρ_p` g
    (h + w) everywhere. w (the summed deflection, positive down, as built: the uniform foundation
    and the strips) no longer reaches the elevation; it sets the fill, the `Province` crust and the
    volumes. Labelled in the module: a standing bulge's own foundation is `ρ_m` g + E `T_e` ÷ R²,
    on which the reference Mars's cap-height cone would raise a bulge of about 96 m
    (axisymmetric); not modelled. α is unchanged. A cell's `Province` test reads the fill of its
    deflection, w, where the first rule read w − h; a planed bulge's cells keep their crust.
    The lid an edifice bends (`Lid`, now private) carries `ρ_p` with its foundation, so the load
    and the foundation cannot part, and `Edifice::new` takes no density of its own (the review).
    `Provinces` carries `density`, `edifice`, `load` and `deflection` for the tests and the record.
    Tests: Δz = h (the addendum 3's wording) on every cell of the reference Mars, the lift its
    summed load held at `h_max` bit for bit, and about one hand-placed edifice cells on its bulge
    (w < 0) exist and their lift is exactly 0 (the second commit's "raised by −w" and "h − w on
    another's bulge" are withdrawn); ∫ `k_eff` w
    dx = ∫ `ρ_p` g h dx to 10⁻⁶, for a line load of the reference Mars's cross-section and for that
    cross-section in 64 strips through the step response, which the edifice's own table matches at
    its centre to 10⁻¹² and out to its reach; `ρ_m` − `ρ_p` = 652 kg m⁻³ on the reference Mars,
    and `flexural_parameter` panics at equal densities; on the reference Mars a cap-height load
    stands at 21,795 m over its root `w₀`, pinned at 30,474 m ± 50 (about 38 km at the grain
    density's load), each cell's ground rises by its lift bit for bit, and the loads' footprint is
    at most a quarter of the surface. The bulge crest at πα, 0.043 `w₀` high, was tested already.
  - _Overlapping loads past the cap bend the lid by their whole sum_ (as T12.c built them; the
    addendum 2's q = `ρ_p` g h holds for each load): each edifice's table bends the lid under its
    whole cone, while the surface stops at `h_max`, so the rock clipped there weighs on the lid
    without standing. On the reference Mars it is 8.75 × 10⁵ km³ over 0.15% of the surface, 0.8% of
    the loads' 1.082 × 10⁸ km³; bending the lid under the capped field alone would take a
    convolution of the field. Recorded below.
  - _The coverage correction now runs only at saturation._ Under the addenda 2 and 3 a field
    stands at its cones' height, so its realised s.d. falls short of σ_volc only where overlaps clip
    it near the saturated ¾: on the reference Mars at a volcanism level of 3, past plan 14's bound
    (about 2; at 2.5 the first field still reaches it, 6,957 against 6,867 m; at 3 it falls short,
    7,000 against 8,240 m), where the coverage is ¾ already and the correction lays the same field
    again. The count law is kept as built (the addendum 2); a test runs that world
    through the staged step and holds the second field to the first bit for bit.
  - _This fix's review_, by the same four reviewers by hand over its diff:
    - _Rust._ Should-fix, done: the balance test now holds the edifice's own table to the
      convolution and pins its root; `Lid` carries `ρ_p` and the guard, its fields private; a doc
      summary split. Considers, done: `FILL_POROSITY`'s doc names the load; the asserts print.
    - _Determinism._ No must-fix or should-fix; its consider, done: the saturated case runs the
      correction through the staged step.
    - _Plan conformance._ Should-fix, done: the clipped overlaps measured and recorded, the volume
      held at the cap. Considers, done: each cell's ground against its lift, the table against the
      convolution, the record's wording.
    - _Science._ Must-fix, the uniform foundation on a standing bulge: commented on the issue with
      the lane's lean (keep it until re-validation); main's addendum 3 planed the bulge instead,
      built in the third fix commit (above). Must-fix,
      done: α's 5–7% was about 5%. Should-fix, done: the ridge reading's effect in full, and main's
      unread figures labelled. Considers, done: 17% at the ruled foundation, `ρ_fill` read as `ρ_p`.
    - _The addendum 3's change_, checked by the plan-conformance reviewer by hand (one expression,
      the surface test and the records): no must-fix. Should-fix, done: the records' addendum-2
      wording, and the brainstorm's step 4 recorded (above). Considers, done: the laid field's
      planed cells, the margin at a volcanism level of 3, two doc paragraphs rewrapped.
  - _`T_e` (ruled, item 4)._ `Silicate`: plan 14's `lithosphere_thickness` (3.1 W m⁻¹ K⁻¹ ×
    (870 K − `T_s`) ÷ F, zero at 870 K and above). `WaterIce`: ∫ k dT ÷ F from `T_s` to plan 14's
    `ICE_LITHOSPHERE_ISOTHERM` (170 K, kept), k the lid substance's conductivity row
    (`conductivity_integral`: `Reciprocal` and `Constant` pieces in closed form, the first piece's
    form extended below its range, any other form by 64-point Gauss–Legendre), so the `H2O` row's
    567 ÷ T gives (567 W m⁻¹ ÷ F) ln(170 K ÷ `T_s`). Every `T_e` is min(the law's, R).
  - _`09`'s T12.c text edited_ (§2.3.5): the 180 km clause (lines 2264–2266) now names the radius,
    `ρ_m` and the open moat and gives its value, 175.5 km, citing the ruling.
  - _Measured._ α open at 70 km on Mars: 175.5 km (177.7 km planar); with the fill, 251.7 km at
    70 km (the ruling's 251.9) and 337.4 km at the reference Mars's own 107.0 km, the bulge crest
    near 1,060 km; beside `ρ_m` 3,500 kg m⁻³, 238.2 and 321.0 km. `T_e`: the reference Earth 19.7 km,
    Mars 107.0 km, the Moon 103.3 km, Ceres 469.7 km (its radius; the law's 2,192.9 km). The ice
    rule's test value: 2.516 km at 200 mW m⁻² and 70 K, 1.86 km at 270. The reference Mars, under
    the addendum 3's planed surface: 6 provinces; 35 edifices of a 368 km basal radius and 21,795 m
    at a coverage of 0.102, the first, which realises 2,854 m against σ_volc's 2,747 m (1.04 of it)
    from the loads alone, so no correction runs (2,854 m before and after; 2,899 m, 1.06, with the
    second commit's standing bulges); a lone cap-height edifice stands at 21,795 m over a root `w₀`
    of 30,474 m, its planed bulge 1,242 m high in −w (0.041 `w₀`) at 1,125 km from its centre; the
    loads' footprint is 9.4% of the surface (main's estimate about 32 loads over about 9%), the
    filled moats beyond it 35.5% and the planed bulges (w < 0) 36.0%; the lift reaches the
    footprint's 9.4%, at most 21,795 m; the `Province` crust covers 44.5%, mostly filled moats (a
    reported figure, not a bound); the volume standing above the filled level is 1.07 × 10⁸ km³
    (held at the cap; the loads' sum 1.08 × 10⁸), ∫ (h + max(w, 0)) dA is 7.0 × 10⁸ km³, and the
    planed bulges' ∫ max(−w, 0) dA 7.45 × 10⁷ km³ (over the cells on a sphere of the mean radius;
    main's estimate about 4 × 10⁸, the gap the ridge reading's, below: 35 isolated axisymmetric
    cones would hold 4.6 × 10⁸, the science check's solve and the addendum 3's 4.63). After
    T12.a–c the reference Mars's s.d. is 2,953 m and its relief 25,711 m. Under the first rule the correction
    had laid 163 edifices over 0.480 of the surface (from 1,268 m at 0.10), realising 2,094 m
    (0.76), with 71.4% `Province` crust and an s.d. after T12.a–c of 2,325 m.
  - _Plan 14's figures stay plan 14's_ (the review; Design note 3, never left to drift): σ_volc and
    `h_max` are gated by plan 14's own lithosphere (`published_thickness`: its
    `lithosphere_thickness` of the lid's class, 651 ÷ T for ice), while the flexure reads the ruled
    `T_e` (the `H2O` row's 567 ÷ T); on an ice lid the two differ by 13%. Nothing moves on the
    reference worlds, whose one province lid is silicate.
  - _Province material no lighter than the mantle beneath_ (a rocky province on an ice lid, which
    plan 14's crusts never pair but the builder accepts) gives no provinces (labelled; `Lid::new`
    returns `None`), so the pass cannot panic there; `flexural_parameter` keeps the addendum 2's
    assert of `ρ_m` − `ρ_p` > 0 for its direct callers.
  - _The province test_ holds the realised share at 0.7 σ_volc or more (1.04 as built under the
    addendum 3, uncorrected; 1.06 with the addendum 2's standing bulges; 0.76 under the first
    rule).
  - _The flexure leaves no relief in the elevation_ (the conformance review). Under the addenda 2
    and 3 the moat is filled and the bulge planed, so the surface T12.c hands to the later steps is
    the volcanic construction alone, where the rendering brainstorm's step 4 under "The coarse
    global pass, once per planet" reads "the flexural trough and bulge around each load". Recorded
    for the owner's ruling-driven brainstorm edit, which this lane may not make; DN6's and T12.c's
    "moats and bulges" (lines 752–753 and 2261) could take the same wording in a docs batch.
  - _The hand-placed edifice_ of the surface test is the reference Mars's edifice laid by hand
    through its own table, since `lay` is private to `provinces`; the laid field's planed cells
    (w < 0, no load) are held to a lift of exactly 0 as well.
- **Deviations in T12.d, as built** (2026-10-16; the same lane, on `a4dcf814`; main's rulings on
  R09.T7.c and R09.T8 for T12.d's contract, and `decision-r09-t12-coarse.md` item 5b; no bump).
  - _Files and items._ `steps/craters.rs`: `run` and `run_after_erosion` (public, as the step
    functions are), `CoarseOctave`, `octaves`, `degradation` and `waits` (`pub(crate)`); tests in
    `steps/craters/tests.rs`. `coarse_pass` calls `craters::run_after_erosion` after
    `erosion::run` (`surface/mod.rs`). The reference-world tests' documentation no longer calls
    the steps no-ops.
  - _The count (T8's lean, accepted)._ Each coarse octave [`D_b` 2^k, `D_b` 2^(k+1)] on its own:
    a Poisson count of mean min(production, saturation) × the body's area, production by
    `cumulative_density` between its diameters and saturation Trask's over the same octave; the
    area the spheroid's closed form (`craters::spheroid_area`), which a test holds to the cells'
    true areas summed (`synth::craters::cell_area`) to 10⁻⁹.
    The octaves run while their lower diameter is below the body's radius (labelled; the Moon's
    last, 1,481–2,962 km, holds a South Pole–Aitken).
  - _Each crater and the draw layout._ `surface.coarse.crater`, octave k on `surface_item(k)`:
    the count from word 0; crater j's words from 2³² + 8j, the centre's z and longitude (uniform
    on the sphere of directions), its diameter's uniform (`diameter_in_octave`, the production's
    distribution within the octave, as T7.b's) and its age's (uniform in the cumulative production
    over its octave's window τ(D), through `chronology_age` at the contract's system age, the window
    T7.b's: the surface's density, or its share `N_sat` ÷ `N_prod` where capped). Morphology
    `crater_shape`'s.
  - _Degradation (ruled, item 5b)._ The byte is the nearest integer to 255 × (1 − g), g T7.b's
    `relief_factor` of the rim radius and `diffusion_length` of the age, at most 255. Every coarse
    crater of the four reference worlds takes 0.
  - _The relief (ruling B) and the reach._ Each crater is smoothed in, in draw order, as its
    `rims::kept_relief` times `CoarseRims::for_body(..).coarse_profile(D)` at the chord between
    the cell centre's and the crater centre's spheroid points in rim radii, sampled at each cell's
    centre, as the testing builder does. Its reach is every cell whose centre's angle from the
    crater's, less the cell's circumradius (the widest corner), lies within 2 asin(reach ÷ (2
    min(a, c))), with the centre's own cell, flooded through edge neighbours: the builder's rule,
    a cover of `ResolutionCode::NONE`. A key (centre cell, diameter) drawn twice is dropped (never
    met). Each smoothing keeps the interim sea level and realised figures true.
  - _The wet-epoch split (Design note 5)._ A crater waits where the body's wet epoch ended before
    its surface age ran out (its end above zero and below the surface age) and the crater is no
    older than the end: `run` lists every crater and smooths in the rest, and `run_after_erosion`
    the waiting ones, after the erosion step, which is still a no-op.
  - _Measured._ The reference Moon (`D_b` 92.6 km): its octaves drew 260, 55, 7, 2 and 0 craters
    against 262.3, 65.6, 16.4, 4.1 and 1.0 expected, each within its Poisson interval; above 100 km
    264 against 289.1 expected, the issue's about 300 (Trask's cumulative 299.7 over 3.79 × 10⁷
    km²: within the capped octave that straddles 100 km the draw follows the production's
    distribution, steeper than saturation's D⁻², so fewer fall above 100 km). The reference Mars
    (90.3 km): 325 craters (283 of 285.1, 36 of 44.9, 4 of 12.2, 2 of 3.5, 0 of 1.0 and 0 of 0.3),
    244 older than the epoch's end at 3.6 Gyr and 81 younger. The reference Earth (84.9 km, a
    0.1 Gyr surface): 1 crater against 2.8 in its first octave. Ceres (50.0 km): 75 (65 of 65.6, 10
    of 15.2, 0 of 2.1, 0 of 0.4). The four worlds share one surface seed, so their octaves draw the
    same Poisson quantiles (the third octave lies 2.3σ low on each). Mean reach entries a cell:
    the Moon 1.61, Ceres 1.04, Mars 0.35, Earth 0.00, far under `TooManyReaches`' 32. Every listed
    lunar crater's fresh rim is Pike 1980b's 0.236 D^0.399 to 1% (the Moon's g 1.624 m s⁻², 0.2%
    off T7.a's identity). The largest degradation byte is 0 on every reference world.
  - _Cost of the batch_ (provisional: the test profile, the sim at `opt-level = 2`, one thread, on
    the laptop's Intel i7-8665U under the `powersave` governor at a load of 1.6–3.0, beside another
    lane; each time a test binary's whole run of one probe, `craters::tests::cost`, three runs):
    on the reference Earth at level 8 the plates 1.34–1.42 s (with the grids), the relief
    1.10–1.20 s more (T12.a–b), the coarse craters under 0.05 s more (one crater; 0.46 s before the
    area took its closed form and the circumradii their cells alone); the whole pass on the
    reference Earth 2.50–2.62 s, Mars 0.50–0.52 s, the Moon 0.084 s and Ceres 0.020 s. Again after
    the addendum 3, on Rust 1.99.1 with the machine otherwise idle (a load of 1.1–1.5), the same
    way: the plates 1.29–1.48 s, the relief about 1.05 s more, the coarse craters under 0.05 s more;
    the whole pass on the reference Earth 2.43–2.46 s, Mars 0.36–0.37 s (35 edifices where there
    were 163), the Moon 0.08 s and Ceres 0.02 s.
  - _The batch's review_, run once over its whole diff. The `review-changes` skill's preamble
    (`git -C` into the shared checkout) is refused in this lane's worktree, so the four reviewers
    it routes to ran by hand from the worktree, each fix in this commit:
    - _Rust._ Must-fix: exact float comparisons that Clippy does not flag. The boundary-cell test is
      now one predicate, `CellState::on_boundary` (exactly +0, by `total_cmp`), the lifts are
      written without zero guards, and the tests compare bits. Should-fix: the provinces' assert,
      which builder inputs could reach, is guarded (above); the craters' two calls take a `Phase`,
      not a `bool`; the flexure's API takes `KilogramsPerCubicMetre` and `Metres` (`Cone`, `Lid`,
      `foundation_n_per_m3`, the lifts' and the area's `_m` and `_m2` names); a test pins a
      mid-range and a full degradation byte; three doc summaries are split. Considered and done: the
      `Province` entry is read by its variant's code. Not done: one shared test `World` for the two
      test modules.
    - _Determinism._ No must-fix or should-fix. Done: a compile-time check that the relief step's
      four instances differ, and other-seed tests of the arcs (a small mobile lid) and the provinces
      (the reference Mars). Not done: an interim golden of the coarse field, which the plan keeps
      for T16 (no golden is added).
    - _Plan conformance._ The trench reading, belts off Earth, plan 14's figures and the fill's
      guard are recorded above, and the `tags.rs` docs below. The continental width's bound is
      based on 1.08 km. Done from its considers: the belts' and the noise kernel's docs, and the
      251.7 km comment; the province test is tightened.
    - _Science._ Must-fix: the filled moat counted its fill twice under a load. Commented on the
      issue with the lane's lean, it was ruled by the addendum 2 and built after the push (T12.c's
      "The filled moat" above).
      Should-fix, done: 14.3% (not 14.2%) older than 120 Myr under the triangular law; the core's
      skewness, 0.32, beside Earth2014's 1.20 (not its shape's 0.10); α 251.7 km at 70 km under the
      reference Mars's gravity; `T_e` held at R labelled an extrapolation of the thin-shell model;
      Parsons 1982 and Wieczorek et al. 2013 read in their abstracts. Unverified, as labelled:
      `ρ_m` 3,300, seawater's 1,030 and the shale's 2,720 kg m⁻³ (a secondary table gives 2,700 for
      Sclater and Christie's shale, 13.5 m of load at 2.4 km), Goff 2010's eq. 5, Harris et al.
      2014's and Sloan et al. 2012's figures, and T8's 1.24.
- **Deferred findings (2026-10-13 rule)** (R09.T12.a–d; recorded, not ruled).
  - _Intraplate volcanism on mobile lids_ is not modelled (Harris et al. 2014's GSFM plateaus,
    seamounts, guyots and non-spreading ridges, 10.2% of the ocean, and hotspot swells: some
    250–300 m of Earth2014's ocean mean; Design note 3 books σ_volc about 0.29 km on Earth, which no
    task realises). The deep mean's band returns to ±300 m when it is built.
  - _T11's continents ignore the plate boundaries,_ so 38% of the reference Earth's divergent
    length is continental rift and its oceanic ridges make 0.676 of the law's floor (the ruling's
    estimate 0.55–0.60); the remap compresses near-ridge ages by that ratio, so ridge flanks look
    faster-spreading. For T11's re-validation, with its "Rifts age floor".
  - _T11's subduction polarity_ (`older_sides`) reads the kinematic τ.
  - _The triangular area–age shape is Earth's;_ convection models vary it with the continents'
    configuration (Coltice et al. 2012, from memory). Its oldest blocks take one mean age each, so
    the tail past about 160 Myr is lost on the reference Earth (0.00% older than 180 Myr against
    Müller's 0.22%).
  - _GDH1 is Earth's water-loaded law:_ a dry floor, or one under another liquid, would subside by
    (`ρ_m` − `ρ_l`) ÷ (`ρ_m` − `ρ_w`) of it, not modelled.
  - _Goff's 55 mm a⁻¹ entry_ (PAR fast, 90 m): the fit gives 116.0 m, +28.9%.
  - _The e-folding length sits at the low end_ of ordinary margins' 200–500 km (200 km): T11's
    passive margins are many and its continents ragged, so at 350 km the wedge buries half the deep
    floor. Deep-sea fans are left to T14.d.
  - _Plan 14's stagnant-lid 0.9 km has no radius term_ (the addendum): at the coarse cell it sits
    1.5× (the Moon) and 2.95× (Ceres) above the synthesis's law in amplitude.
  - _An ice shell's lid also takes 0.9 km,_ Venus's figure, for want of an icy calibration.
  - _Dichotomies are not modelled:_ Mars's hemispheric step is in none of the shares' terms.
  - _T12.e's affine scale also acts on the craters,_ whose profile T7.c's one-profile rule and
    Pike's rims fix: each of plan 14's shares needs its own term, so that the scale stays near 1.
  - _The belts' widths, the trenches' depths and the arcs' rises_ have no primary source (Design
    note 7 names none, and the science check found none for 200–500 km, 2–4 km or either rise):
    illustrations, labelled. Fielding et al. 1994's 5,023 m is the one verified figure.
  - _The 8-connected excess stays in T11's signed distances_ (a median 4.7% at the fronts, up to
    31%), which T5's styles and the belts and trenches read; the arcs read the great circle.
  - _Slivers._ 9–15 of 41–56 plates on four reference Earths lie within 100 km of their boundary
    everywhere, two of them a single row of boundary cells. T11's additively weighted score stays
    the first candidate, not adopted.
  - _Transforms are long arcs_ (runs of 2,300–2,700 km), with no fracture zones.
  - _Belts lift continental crust alone:_ a collision's half-width reaching its plate's oceanic
    cells leaves them as they are.
  - _The continental core is a log-normal, Earth2014's is not:_ Earth's continents are a narrow
    core with long tails (an interquartile range of 723 m and a skewness of 1.20, against the
    first population's 1,427 m and 0.54, the addendum's figures), and the ice sheets' surfaces,
    2–4 km over Antarctica and Greenland, are a tail no population or landform holds.
  - _Mars's constructional relief is mostly Tharsis's regional rise_ (the addendum 2), so an
    edifice-only law needs some thirty giant shields to reach σ_volc: 35 on the reference Mars,
    where Mars has about ten shields wider than 300 km (main's figures, not read; Plescia 2004's
    Table 1 would source the sizes). A rise component, held up
    by thickened crust, is for T12.c's re-validation; nor is a size distribution of edifices
    modelled (one size, Mars's style, on every lid).
  - _Loads at `ρ_p` are about 17% lighter_ than Belleguic et al.'s 3,200 ± 100 kg m⁻³ for Mars's
    large volcanoes (17.25%, the science check, which read the abstract), so at the ruled
    foundation they deflect about 17% less (the addendum 2). The piecewise foundation the addendum
    declined makes the solve nonlinear.
  - _A standing bulge's foundation_ (the addendum 3, for T12.c's re-validation with the regional
    rise) is `ρ_m` g + E `T_e` ÷ R², 13,234 N m⁻³ on the reference Mars, 3.94 times the uniform
    3,362. The consistent bulge of a cap-height cone is about 96 m (an axisymmetric cone on the
    foundation switched by the sign of w, against 234 m uniform; on the ridge 508 against 1,242 m;
    `w₀` moves by 0.15–0.20%: the science check's solves, which the addendum's finite-difference
    solve reproduces); planed, there is none. A second axisymmetric deflection on that foundation
    (α_b 239.6 km) reproduces it within 2% (97.6 against 95.9 m), but on the ridge gives 370 m.
  - _Moats fill whatever the volcanic flux_ (the addendum 2), and _underfilled moats_ are not
    modelled (the addendum 3): the visible flexural relief of real volcanoes is mostly their
    unfilled moats. Hawaii's moat is underfilled (Wolfe et al. 1994, JGR, doi:10.1029/94JB00686,
    main's citation, not read); Olympus Mons has a present-day trough (Isherwood et al. 2013).
  - _Water-covered arches_ would take (`ρ_m` − `ρ_w`) g over the arch (Hawaii's, about 0.5 km
    under water beside a line load more than 2,000 km long: DN7's case, main's figures, not read);
    not modelled, since no sea is placed before T12.e.
  - _The planed material is taken at `ρ_p`,_ as the one-density model takes every layer (the
    addendum 3).
  - _Overlapping loads past the cap bend the lid by their whole sum_ (the conformance review): the
    rock the cap clips, 8.75 × 10⁵ km³ over 0.15% of the reference Mars, weighs on the lid without
    standing. Dropping it from the load would take a convolution of the capped field; the
    addendum 2 does not address overlaps.
  - _`ρ_m` per body from plan 14's mantle composition_ (the addendum 2: the ask stands; Mars
    nearer 3,500 kg m⁻³, from memory, which lowers its filled α by about 5%: 321.0 against
    337.4 km at the reference Mars's own `T_e`, and 238.2 against 251.7 km at 70 km) is an ask for
    P14.T54.a: a density at the lithosphere's base
    from `hooks::crust::mantle_iron_oxide_mass_fraction` and `MantleNorm`.
  - _An ocean beneath an ice shell, and sea-filled moats,_ are not modelled.
  - _The shell term is the shallow shell's,_ valid for α ≪ R; low-degree membrane support is not
    modelled.
  - _The ice isotherm of 170 K is of low confidence,_ and the conductivity's spread (Klinger's
    567, Carnahan et al.'s 612 and Slack's about 651 ÷ T) is 15%.
  - _Internal heating is absent from the `T_e` laws,_ which overstates `T_e` on small cold bodies
    (Ceres's law passes its radius).
  - _Lids of other mechanics families_ (`Metal`, `VolatileIce`, `Salt`, `Organic`) have no T12.c
    rule.
  - _The plate model's conductivity is 3.138 W m⁻¹ K⁻¹_ (Parsons and Sclater's, which GDH1 keeps:
    Stein and Stein 1992, p. 125, read by the science check), which Design note 3 and plan 14
    round to 3.1; the ruled 3.1 stands.
  - _Isherwood et al. 2013 does not support "no open moat"_ on Olympus Mons: its abstract speaks
    of "the present-day flexural trough" (the science check). The filled moat stays ruled.
  - _The coarse craters stay near fresh_ (item 5b): T7.b's relief factor keeps at least 99.95% of
    every coarse crater on the Moon, so every byte is 0. What wears old large craters is not
    modelled: later large impacts and their ejecta (which the list's overlaps represent only where
    craters overlap), infill, and on warm or icy crusts viscous relaxation.
  - _The draw within a capped octave follows the production's distribution_ (as T7.b's small
    craters), not saturation's, so the octave that straddles 100 km on the Moon holds 289 craters
    above it in expectation, against Trask's cumulative 300.
  - _No crater is drawn from an octave whose lower diameter reaches the body's radius_ (labelled).
  - _Positions are uniform over directions,_ not over the spheroid's area: on Ceres (f 0.075) the
    density per area is 1 ÷ (1 − f)², 17%, higher at the poles than at the equator; labelled.
  - _The ridge's approximation overstates an edifice's flexure_ at the cap's size against the
    axisymmetric cone's (R09.T12's science check, by a planar FFT solve on a 16,000 km periodic
    domain with the shell term as a foundation term, which reproduces its earlier 18.2 km at
    3,310 kg m⁻³; medium-high confidence): at `ρ_p` the root is 30.5 against 14.6 km (2.1 times),
    the bulge 1,242 against 234 m (5.3 times; 0.041 against 0.016 `w₀`) and the fill per edifice
    2.0 against 1.0 × 10⁷ km³, and the profile turned about the centre breaks the edifice's own
    balance, carrying 1.63 times the load's weight (∫ `k_eff` w dA against ∫ `ρ_p` g h dA; 1.000000
    axisymmetric), which the line-load balance test cannot see: per isolated cap-height cone the
    load, root and fill hold 2.32 against 1.32 × 10⁷ km³ (the addendum 3's solve, which reproduces
    the check's: root 2.09 times, bulge 5.32 times, 13.0 times with the uniform foundation's
    bulge). Under the addendum 3's planed surface it moves only the `Province` crust's share and
    the volume. The axisymmetric profile (Kelvin functions, which the crates lack, or a Hankel
    transform on `hyperion_base::math::j0`) is for T12.c's re-validation; for its tests, the point
    load's −kei(r ÷ l), l = α ÷ √2, has its first zero at 2.768 α and its crest at 3.487 α with
    0.0143 `w₀` (the addendum 3).
  - _`tags.rs`'s registry docs read the old keys_ (for a docs batch, which may edit it):
    `surface.coarse.crater` is keyed by `surface_item(k)` for octave k, not crater slot k, and
    `surface.coarse.relief` by `surface_cell` of lattice cells at levels 1–5 and the field's, and
    level 0's first cell, by instance, not of the coarse cells alone.
  - _Plan 14's ice-rich `T_e` reads 651 ÷ T where the `H2O` row is 567 ÷ T,_ so plan 14's σ_volc
    on an ice lid sits 13% above what the ruled `T_e` would gate: an ask for P14 to read the row.
  - _No golden pins the coarse field before R09.T16,_ so no test compares its new output across
    targets until then (the determinism audit); the pass's twice and order tests run on each
    target alone.
- **Deviations in T12.e, as built** (2026-10-19; lane `lane/43-r09-t12e-sigma-h-sea-level`, on
  `origin/rendering-and-planets` at `8c08d508`; main's rulings of 2026-10-19, R1–R10,
  `decision-r09-t12-coarse.md` addendum 4, amending `decision-r09-t5.md` items 1, 3, 4 and 8; no
  bump, `GENERATOR_VERSION` 22, no golden moved or added).
  - _Files and items._ In the sim, `steps/relief.rs` gains the step's second entry,
    `run_after_craters` (public, as the step functions are), with `structural_spectrum`,
    `structural_anchor`, `CrustClass` and `crust_class` (`pub(crate)`; `crust_shape` now reads
    `crust_class`, as the anchor does, so the two name one class) and `after_late_craters`
    (`pub(super)`), and three child modules:
    - `relief/budget.rs`, the budget, the scale and the figures: `Body`, `LocalShare`,
      `FinerShare`, `StructuralShare`, `Booked`, `Rule`, `Match`, `Figures`, `records`, `book`,
      `match_sigma_h` and `figures`;
    - `relief/quadrature.rs`: `NODES`, `gauss_legendre`, `each_node`, `Values`, `Moments` and
      `cell_mean`;
    - `relief/sea.rs`: `Sea`, `place` and `reflood`, all `pub(crate)`.
    - The tests are `relief/tests/sigma_h.rs`, a child of `relief/tests.rs`, so the acceptance
      filter `steps::relief` selects them.
    - `steps/craters.rs` makes `Phase` `pub(crate)` and gains `smoothed_in_by` and `each_lift`,
      the one expression for a coarse crater's lift, which `apply` and the budget share (`apply`'s
      arithmetic is unchanged).
    - `quantise::synthesis_record` is `pub(crate)`, so the styles and channels T12.e books read
      the field's codes bit for bit.
    - `coarse_pass` calls `relief::run_after_craters` between `craters::run` and `climate::run`,
      and no other line of it changed.
  - _Provides added (the surface crate; ruling R2)._
    - `synth::relief::StructuralVariance`, the octaves' seed-free variance:
      `StructuralVariance::new(spectrum, level, radius)`, `first_octave`, `finest_octave` and
      `point_variance(a, b)`. `Relief` holds one and its `point_variance` delegates to it, bit for
      bit; its fields moved into it, and the relief tests read them there.
    - `CoarseRims::sharpening_moment_m4(crater, finer_than)`: g² ∫ (F(w₂) − F(w))² dA over the
      whole ground, from the moments `share` takes, so that `share`'s bits stay.
    - `CoarseRims::increase_m4(crater)`: g² ∫ (F(w₂)² − F(`w_c`)²) dA, from
      `CraterProfile::square_moment`, which `rims` already reaches as a child module, so no
      visibility changed.
    - Their new panics are in `tests/panics.rs`. Beyond these, the one surface-crate edit is
      `FieldHeaderParts::sea_level`'s documentation, now the connected ocean's rule (R8).
  - _R1, the scale, as ruled._ Each cell's elevation is split into C, every listed crater's lift
    summed in list order by `each_lift` (the craters waiting for the erosion step included, R4),
    and N, the elevation less the part of C already in it. μ is N's cell mean, and `V_N`, K and
    `V_C` come from one quadrature. With T = σ_h² − (`S_s` + `K_w` + `C_c`),
    s* = √(T ÷ (`V_N` + 2K + `V_C` + `R_inc`)).
    - Where s* ≤ 1, each byte becomes the nearest integer to 255 (1 − s* g), at most 255. Where
      every crater's old byte was alike, K′ and `V_C`′ are K and `V_C` times one factor and its
      square, as on every reference world; otherwise a second quadrature reads C′
      (`uneven_bytes_are_read_in_a_second_quadrature`, on the reference Moon with every other
      crater worn to a byte of 40). `s_N` then solves the quadratic at the new bytes.
    - Where s* > 1, the bytes are kept and `s_N` solves the same quadratic.
    - Then z = μ + `s_N` (N − μ), with the craters that do not wait smoothed in again at their
      bytes in list order, as `apply` smooths them.
    - Where T or the whole variance is not positive, or no positive root exists, the elevation and
      the bytes are left as they were (`Rule::Unscaled`, labelled); no reference world comes near
      it (`a_sigma_h_no_scale_can_meet_leaves_the_field_unscaled`, a Moon asked for 10 m). The byte
      cast asserts the kept relief finite, so a NaN cannot become a fresh crater's byte.
    - Lean: of the quadratic's positive roots, `s_N` takes the one nearest s*, the larger at a tie,
      by the root pair that does not cancel. The byte is labelled as R09.T12.d's wear times this
      crater scale, a stand-in for the large craters' unmodelled wear, set by plan 14's σ_h.
  - _R2._ As ruled: the point variance through `StructuralVariance`, built from `working.spectrum`,
    the level and the radius. Tests pin the header-free forms to the `FieldView` forms. On every
    reference world's quantised field: the point variance bit for bit; `CraterOctaves::for_body`
    equal to `CraterOctaves::new` of the header; each cell's channels' share from its record equal
    to the field's, bit for bit; and, on the cells one crater reaches, the crater's sharpening over
    its reach's disc equal to the field's rims' share to 10⁻¹². In the surface crate: the
    seed-free variance against `Relief`'s, bit for bit, on every synthetic world; `share`'s
    variance against the new form over its disc to 10⁻¹²; and the increase against an independent
    piecewise sum to 10⁻¹⁰, as the sharpening's own variance plus twice its product. No golden
    moved: `closed_forms.golden` and the `weights/` goldens are unchanged.
  - _R3 (a deviation of the plan's order)._ T12.e is the relief step's second entry, after the
    coarse craters, whose share the budget counts. DN5's and the module docs' "relief (T12.a–c and
    e)" now means the step's two entries. `relief::run` still ends with the interim figures and
    `craters::run` still keeps them, so T12.d's staged tests stand unchanged.
  - _R4._ The waiting craters join C in the solve at the bytes T12.e sets.
    `craters::run_after_erosion` smooths them in at those bytes, then refloods the ocean against
    the sea level as placed (`sea::reflood`) and recomputes the figures with every crater in, in
    place of the interim figures; it does not rescale. The reference Mars's realised σ_h after the
    whole pass is 2,900.0000 m (tested to 10⁻⁶). R09.T14.d's re-match must count these craters,
    since `run_after_erosion` follows the erosion step.
  - _R5, R6._ The realised σ_h is √(`Var_recon` + `S_s` + `K_w` + `R_inc` + `C_c`), the whole
    surface's, from a quadrature of the elevation as it stands. The realised relief is the cells'
    range, which bounds the reconstructed field; the quadrature's range is reported beside it.
    `budget::figures` computes both, with the craters smoothed in by the end of its phase. In
    `run_after_craters` it takes the match's book (`Booked`: the structural share and the channels'
    cell means), since neither the scale nor the sea changes a code the book reads; a test holds a
    fresh book of the finished state equal to it, bit for bit. `after_late_craters` books afresh.
  - _R7, the whole budget._
    - `S_s`: the quadrature mean of the point variance at the interpolated a and b, with ⟨a²⟩ the
      mean of the interpolated a², each cell's style from `ReliefStyle::of` of its quantised record.
    - `K_w`: `for_body`'s unresolved variance at 2 `D_b`.
    - `R_inc`: Σ `increase_m4` ÷ the spheroid's area. Its product term takes the analytic coarse
      profile for the reconstructed one (labelled).
    - `C_c`: the channels' cell mean at 2 `D_b`, which is at least twice the coarse cell's mean
      width (about 4.7 of them). It is zero, every cell `Terminal`, and asserted +0 on every
      reference world, whole and finer than λ_c.
    - Lean: one wavelength, 2 `D_b`, counts both the small craters and the channels whole. Test 2's
      figures are kept beside the budget, at λ_c = 2πR ÷ `l_N`(L).
  - _R8, the sea, as ruled._ The quantile walks the cells sorted by (elevation, index). The seed is
    the largest edge-connected region at or below `ℓ_q`, ties to the least index. A priority flood
    keyed by (filled level, index) grows it, and ℓ is the first filled level at which the area
    taken reaches f × the grid's sphere, every cell of that level taken, or the last pop's where
    rounding exhausts the cells (f = 1). Every taken cell whose ground's millimetre code is below
    ℓ's takes ℓ and `main_liquid`.
    - Lean: "at or below" the quantile, so that a fraction under one cell still has a seed.
    - Lean: "below ℓ" is read on the codes (flooding by the codes), so that a ground a fraction of
      a millimetre below ℓ stays dry and the quantised field shows it dry. A test lays shore cells
      0.1–0.2 mm under ℓ to exercise this.
    - Taken cells at ℓ's code are the ocean's shore: dry, and counted in its area.
    - f = 0 floods nothing and keeps the lowest elevation. f = 1 floods every cell but the highest.
    - Lean: `reflood` drains every cell of the main liquid and floods again from a fresh seed with
      ℓ kept. On a dry world it keeps ℓ, the lowest elevation as placed; on the reference Mars the
      late craters cut no lower.
    - The areas are the cells' solid angles, a share of the sphere of directions, where plan 14's
      fraction is one of surface area (labelled): about 10⁻⁴ apart on an Earth, up to about 10⁻²
      on a strongly flattened ocean world (the science check).
  - _R9._ `structural_anchor`: the ice-rich anchor (50 m) for an ice surface or an ice shell in
    every regime, the ice mobile lid labelled; 200 m × `height_factor` for a silicate mobile lid;
    and 60 m for every other silicate lid. The anchor and the shape both read `crust_class`.
  - _R10._ The main liquid fills the summed ocean fraction. The other liquids are R09.T13.d's.
  - _The quadrature (a lean, then the brief's rule)._ n × n Gauss–Legendre nodes in each cell's
    (s, t), mapped by `st_to_uv` and `face_uv_to_xyz`. Each node's weight is its two Gauss
    weights times the solid angle's density (du ÷ ds)(dv ÷ dt) ÷ (1 + u² + v²)^(3/2), scaled so
    that each cell's weights sum to its solid angle. Sums run in cell order, then node order.
    - Var_recon of the final field: the Moon 2.574 497, 2.574 933 and 2.574 849 × 10⁶ m² at n = 2,
      3 and 4 (n = 2 is 1.37 × 10⁻⁴ short of n = 4; n = 3 is 3.3 × 10⁻⁵ over); Mars 8.079 293,
      8.079 428 and 8.079 400 × 10⁶ m² (1.3 × 10⁻⁵ and 3.5 × 10⁻⁶). So `NODES` is 3, the smallest
      within 10⁻⁴ on both. The spline's knots lie at the cell centres, so each cell spans two
      pieces.
    - Lean: a cell none of whose cells within `SYNTHESIS_MARGIN_CELLS` king moves differs from it
      takes its own value at its nodes. That is the interpolant's value bit for bit, since it reads
      no farther and gives a constant back exactly. Tests hold it over every node of the Moon and
      Ceres, a sample of Mars's cells and, in the slow test, of the Earth's sparse fields. They hold
      it at levels 5–8 too, beside single-cell spikes at a cube corner and at the ends and middle
      of face edges, where the interpolant reads farthest, at every node 4–7 king moves away. So
      the sparse fields (the crater lift, the belt weight) cost almost nothing.
  - _Constants._ No physical constant is added. `NODES` is set by the convergence check. The
    Gauss–Legendre nodes are in closed form (Abramowitz and Stegun 1964, §25.4.29, from memory),
    tested as the Legendre roots and exact to x^(2n − 1).
  - _`09`'s T12.e text edited_ (§2.3.8). Only the test clause, lines 2281–2297 (formerly
    2281–2290), now names test 1's whole budget, test 2's terms and "where it holds", the relief's
    cells' range, the ocean's area rule with f = 0 and 1, the header's sea level and spectrum, and
    `structural_spectrum`'s classes, citing the ruling. Its "Design note 7's observed 7–15" is
    corrected to Design note 3's, where the figure stands. DN7's text is left to a docs batch.
  - _The tests._ Each check is a function of a reference world.
    - The fast suite runs them on the reference Mars, Moon and Ceres.
    - The reference Earth's pass with T12.e takes about 9 s natively, and nextest runs each test in
      a process of its own, so every test that read the Earth would pay it. One slow test,
      `the_reference_earth_holds_r09_t12e_s_tests`, runs every check on the Earth instead, run by
      name. It also holds the tests only the Earth can: test 2's asserted band, test 4, test 7's
      ocean, its one ocean through quantisation and its hypsometry.
    - The reflood (ruling R4, with a sea) is held on a hand-made basin, where raised cells drain, a
      lowered shore cell floods, ℓ is kept and twice is the same bits. It is also held on a 1,500 km
      Mars-like world with an ocean over 0.3 and the reference Mars's wet epoch, through
      `craters::run_after_erosion`, its late craters smoothed in, where ℓ is kept, the sea is one
      ocean and the whole surface's σ_h is plan 14's to 10⁻⁶.
    - A test deviation: the one-ocean check joins the flooded cells to the flood's seed (which `Sea`
      now carries, with the cells it took) through the cells the flood took. Those include its
      shore at ℓ's code, which the fill by the codes leaves dry, so the join is not through the
      flooded cells alone: a depression joined at ℓ through a saddle at ℓ is flooded, and its saddle
      dry.
  - _Measured_ (test profile; shares of σ_h²; "cells / reconstructed" RMS; before → after the
    scale). Plan 14's shares are 2.51 km structural on the Earth; 0.9, 2.75 and 0.89 km on Mars; 0.9
    and 2.05 km on the Moon; and 0.9 and 1.87 km on Ceres.

    |                                     | Earth                           | Mars                                                  | Moon                                              | Ceres                                             |
    | ----------------------------------- | ------------------------------- | ----------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------- |
    | s*, s_N, rule                       | 0.9602, 0.9602, whole           | 0.9596, 0.9595, whole                                 | 0.7892, 0.7952, whole                             | 1.7302, 2.1311, non-crater                        |
    | largest byte                        | 10                              | 10                                                    | 54                                                | 0                                                 |
    | T ÷ σ_h²                            | 0.9956                          | 0.9804                                                | 0.7735                                            | 0.9272                                            |
    | V_N, K, V_C, R_inc (m²)             | 6.804 × 10⁶, −0.19, 0.20, 7.4   | 8.538 × 10⁶, 2.38 × 10⁴, 2.130 × 10⁵, 1.564 × 10⁵     | 7.815 × 10⁵, 8.26 × 10⁴, 3.182 × 10⁶, 2.096 × 10⁶ | 7.533 × 10⁵, −1.97 × 10³, 9.72 × 10⁴, 4.897 × 10⁵ |
    | low-pass of V_N, V_C                | 2.06%, 65% (one crater)         | 2.08%, 6.0%                                           | 3.52%, 6.79%                                      | 7.0%, 27.4%                                       |
    | whole L before → after              | 0.435% → 0.435%                 | 3.816% → 3.673%                                       | 64.47% → 48.64%                                   | 18.63%                                            |
    | S_s; ⟨a²⟩; S_s ÷ ⟨a²⟩ L_s           | 0.434%; 0.6440; 1.0606          | 0.026%; 0.5764; 1.000 00                              | 0.079%; 1; 1.000 00                               | 0.048%; 1; 1.000 00                               |
    | K_w (whole; finer than λ_c)         | 0.0014%; 0.0014%                | 1.931%; 1.902%                                        | 22.58%; 22.04%                                    | 7.23%; 6.89%                                      |
    | R_inc (sharpening + cross) before   | 0.0001%                         | 1.860% (0.971 + 0.888)                                | 41.82% (27.31 + 14.51)                            | 11.35% (9.15 + 2.20)                              |
    | rims finer than λ_c, before → after | 0.0001%                         | 0.354% → 0.327%                                       | 7.91% → 4.92%                                     | 2.89%                                             |
    | test 2 (band)                       | 0.435% (0.3–1%)                 | 2.255% (0.3–1%)                                       | 27.04% (4–10%)                                    | 9.83% (3–8%)                                      |
    | populations                         | 2,473 / 2,457 → 2,375 / 2,359 m | 900 / 892 → 864 / 856 m                               | 900 / 884 → 716 / 703 m                           | 900 / 868 → 1,918 / 1,850 m                       |
    | landforms or provinces              | 633 / 587 → 607 / 564 m         | 2,854 / 2,825 → 2,739 / 2,711 m                       | none                                              | none                                              |
    | coarse craters; as rendered         | 0.8 / 0.4 m; 2.8 → 2.7 m        | 476 / 462 → 457 / 443 m; 608 → 584 m                  | 1,848 / 1,784 → 1,456 / 1,406 m; 2,298 → 1,811 m  | 366 / 312 m; 766 m                                |
    | realised σ_h (√Var_recon)           | 2,510.000 m (2,504.5)           | 2,890.15 m, 2,900.000 with the late craters (2,842.4) | 2,239.000 m (1,604.7)                             | 2,077.000 m (1,873.6)                             |
    | relief ÷ σ_h (reconstructed range)  | 7.41 (6.82)                     | 9.52 (9.41)                                           | 6.37 (6.10)                                       | 6.41 (5.82)                                       |
    | sea level                           | −170.76 m                       | −5,214.23 m                                           | −7,684.69 m                                       | −6,904.46 m                                       |

    The reference Earth's sea: `ℓ_q` −234.64 m and ℓ −170.76 m; the ocean covers 0.709 501 against
    0.7095, an excess of 1.3 × 10⁻⁶ within the final level's 2.9 × 10⁻⁶. 0.709 498 is under water
    (279,269 cells), and dry ground below ℓ covers 0.0145 of the sphere; the seed holds 273,816
    cells. Its hypsometry relative to ℓ: land mode +125 m; ocean modal bin −4,375 m; below −2 km,
    mean −4,317 m, s.d. 912 m, quartiles −4,918, −4,333 and −3,723 m. All three of T12.a's bands
    hold (tested).

  - _For T14.d._ Its second match books `C_c` through the same `book`, at 2 `D_b`. It must
    count the waiting craters (R4) and keep the bytes' composition: fold, never stretch, a crater
    past its fresh shape. `reflood` drains every main-liquid cell, so T13.d's and T14's lakes of
    the main liquid must be told apart from the sea before they reuse it.
  - _Cost_ (provisional: the test profile, one test thread, on the laptop, an Intel i7-8665U, at a
    load of about 2.2, beside no other lane; each a test binary's whole run of one probe in
    `sigma_h::cost`). With n = 3, T12.e adds 6.33 s to the reference Earth (2.47 s through the
    coarse craters, 8.80 s with T12.e), 1.78 s to Mars (0.35 → 2.13 s), 0.47 s to the Moon
    (0.08 → 0.55 s) and 0.13 s to Ceres (0.02 → 0.15 s).
    - On the Earth the cost is three interpolated fields over 3.5 × 10⁶ nodes: the style
      amplitude, N, and the final field for the figures.
    - At n = 2 it had been 2.8 s, and 8 s before the uniform cells' shortcut and the reuse of
      `S_s`.
    - A throwaway timing of 10⁶ level-8 points gave 0.75 s for two `interpolate` calls against
      0.46 s for one `interpolate_each` of two. A public fused read of the four values would save
      about 1.4 s of the Earth's 6.3 s. Main ruled on 2026-10-19 to keep n = 3 and the plain
      `interpolate` (below).
    - `surface/mod.rs`'s order test makes eight reference-Earth passes, about 70 s.
    - The whole pass, timed later the same way but on battery power, the clocks at 0.4–0.9 GHz
      under `powersave` (`craters::tests::cost::pass_*`): the Earth 13.97 s, Mars 4.85 s, the Moon
      0.88 s and Ceres 0.31 s. In the same conditions the Earth took 4.00 s through the coarse
      craters and 13.99 s with T12.e, so the whole pass is the run through T12.e within 0.1 s, the
      later steps being no-ops but for Mars's late craters, their reflood and their figures.
  - _The figures, checked_ (R09.T12.e's science check, recomputed from the measured models with a
    spherical-harmonic analysis checked to 10⁻⁴ on a known field).
    - The local shares above `l_N`: the Earth 0.53% of σ_h² from Hirt and Rexer 2015's BED fit
      (Table 3 and eq. 6, 183.1–183.3 m above degree 556), about 0.63% with the abyssal hills; Mars
      0.58% (MOLA MEGDR at 16 ppd, 221 m above degree 278); the Moon 6.8% (LOLA LDEM_16, 583 m
      above degree 139). So test 2's bands stand, and the crater model overshoots them (Mars 3.9×,
      the Moon about 4×). Ceres's 5% and Mercury's 4% were not recomputed.
    - The greatest relief: 7.6 σ_h on the Earth, 10.1 on Mars and 8.4 on the Moon, inside 7–15.
      Johnson and McGetchin 1973 exists, but its text was not read.
    - Verified: σ_h of 2,510 m (TBI's 2,508.3 m), 2,900 m (MOLA's 2,901–2,902 m) and the fits'
      2,239 and 2,077 m. The ocean fraction is 361,900,000 of 510,082,000 km², 0.709 494 (NGDC's
      Table 1, read verbatim). Earth2014's hypsometry: land mode in the 0–250 m bin; the modal bin
      −4,375 m, with exactly the five bins −5,125 to −4,125 m within 10% of its peak; the mean
      below −2 km −4,281.0 m.
    - Verified: the Gauss–Legendre rules, to 3 × 10⁻¹⁶ of numpy's; the warp's rate and the
      gnomonic element; the cap within a chord c, π c²; the increase identity; and
      `positive_root`, in 20,000 random cases.
    - The structural anchors are recorded among the deferred findings.
  - _The review_, run once over the whole diff. The `review-changes` skill's preamble (`git -C`
    into the shared checkout) is refused in this lane's worktree, so its four reviewers ran by
    hand over the diff:
    - _Rust_: no must-fix.
      - Should-fix, done: a reflood test with a sea; the shore test's name.
      - Should-fix, done: an enum (`SeaLevel`) and a book (`Booked`) for the bare `Option`
        parameters, and a phase for the tests' `bool`.
      - Should-fix, done: unit-bearing names (`StructuralShare`, the flood's `Flood`, `level_m`,
        `target_sr`, the `_share` fields); the rim forms' `# Panics`; `#[must_use]`; `records` an
        iterator, with capacities.
      - Considers, done: the seed's invariant comment; one `crust_class`; the dead flag.
    - _Determinism_: no must-fix.
      - Should-fix, done: the reflood with a sea, twice, by bits.
      - Considers, done: the spikes at levels 5–8; the byte's finite check; the cached book against
        a fresh one.
    - _Plan conformance_: must-fix, done: the reflood test with a sea and waiting craters.
      - Should-fix, done: `C_c` asserted zero; DN3's citation in the edited line; the one-ocean
        check from the seed through the taken cells, recorded as a test deviation; the Earth's
        tests gathered into one slow test; the whole-pass timings.
      - Considers, done: the second-quadrature and unscaled paths tested; the record's broken code
        spans; the test's name.
    - _Science_: no must-fix. Its should-fix items are in `synth.rs`'s anchors, which predate this
      lane and lie outside its scope, so they are recorded below. Its considers, done: DN3 cited,
      and the area against the solid angle labelled.
- **Deferred findings (2026-10-13 rule) (R09.T12.e)** (recorded; the first two and the cost ruled
  by main on 2026-10-19 as noted).
  - _Test 2 misses on three worlds: the crater model's sub-cell share._ Finer than λ_c, the share
    is 27.0% on the reference Moon (band 4–10%), 2.26% on Mars (0.3–1%) and 9.83% on Ceres
    (3–8%); the Earth's 0.435% holds. Fresh Pike profiles at Trask's saturation carry 2–4× the
    bodies' measured shares above `l_N`, and T7.b's diffusion barely wears them above a few
    kilometres. Ruled by main: keep as built. R09.T7.i–j (the coarse profile through the central
    peak, and craters worn to equilibrium after Minton et al. 2019) is expected to close these
    misses: main estimates the Moon's share finer than λ_c at about 6% after it. It re-measures
    T12.e's figures. For T7.b's and T12.d's re-validation before R09.T16's goldens.
  - _The reference Moon's `s_N` is 0.795, under §2.3.4's 0.8._ As the client renders them, its
    coarse craters (2,298 m RMS with their increase) and small craters (1,064 m) overfill plan 14's
    2.05 km basin share. So s* is 0.789 where main estimated 0.86, and the bytes are 54 where main
    estimated about 35. The coarse craters' cells RMS is 1,848 m, against main's 1,594 m from the
    realised counts. Ruled by main: keep as built, not tuned. The cause is the crater model's
    calibration, which R09.T7.i–j is expected to close, putting the craters' expected relief at
    about 1.9 km and the Moon's s* back near 1.
  - _The coarse profile of a crater near `D_b` is inverted_ (main's figures, §2.2.9, not
    re-measured): a 113 km crater's coarse profile stands +2,165 m at its centre and −607 m at its
    rim, where the fresh crater is −1,080 and +1,553 m. For T7.c's and T12.d's re-validation
    before T14's drainage.
  - _Ceres's non-crater field carries plan 14's basin share._ Its population stretches from
    900 m to 1,918 m RMS on the cells (`s_N` 2.13), since its craters realise about 0.77 km of
    plan 14's 1.87 km (none of its 2.1 expected third-octave craters was drawn). For P14.T48.a,
    with the lid's radius-free 0.9 km.
  - _`R_inc`'s product term takes the analytic coarse profile_ for the reconstructed one (R7). On
    the Moon that term is 14.5% of σ_h² before the scale.
  - _Enclosed depressions below the sea level stay dry_ (R8): 1.45% of the reference Earth's
    sphere. For T14's lakes, and for T12.a's continental population, whose low tail lies inland as
    well as at the margins.
  - _The degradation byte composes two wears:_ T12.d's diffusive wear and T12.e's crater scale.
    T14.d's re-match must keep the composition, folding a crater and never stretching it.
  - _The other liquids' shares are R09.T13.d's_ (R10): the main liquid fills the summed fraction
    until then.
  - _The header's `realised_sigma_h` documentation_ still reads "of the reconstructed field … the
    area-weighted RMS elevation about its mean", where under R5 it is the whole surface's at its
    band limit. This lane may edit only `sea_level`'s documentation there; for a docs batch.
  - _A performance finding for R09.T16: T12.e costs 6.3 s on the reference Earth at n = 3_
    (above). Ruled by main: keep n = 3 and the plain `interpolate`; no fused read is exposed in
    T12.e.
    - A public fused read of N, C, a and b would save about 1.4 s of that.
    - Splitting each cell at its spline knot would make a lower order exact, at the cost of more
      nodes.
    - The suites slow with it: `surface/mod.rs`'s order test goes from about 20 s to about 70 s
      (its eight reference-Earth passes). That test is left as it is: its property needs each pass
      computed in several orders, so one Earth shared across its cases would not hold it.
  - _The ice-rich anchor does not reproduce the measurement it rests on_ (a source mismatch with
    `decision-r09-t5.md` item 3 and the plan, from R09.T12.e's science check; for main's ruling).
    - Nimmo and Schenk 2008's Fig. 5a puts Europa's RMS deviation at 1 km at 35–50 m (log₁₀ about
      1.55–1.7, read off the plot). Under the ice shape, `STRUCTURAL_RMS_ICE_RICH`'s 50 m below
      72 km gives 24.7 m at 1 km, 0.49–0.71 of it; meeting it would take an anchor of about
      71–101 m.
    - It does match the abstract's Table 1 at 100 m: 6.5 m against 5.6–8.5 m on the ridged plains.
      The brief and the ruling cite "Table 1" for the 1 km figure, which is Fig. 5a's;
      `synth.rs`'s documentation cites Fig. 5a correctly.
    - The documentation's slopes at 100 m and 10 m are recomputed at 5.3° and 9.5° (it says 5.7°
      and 10.3°), both still inside Europa's ranges.
    - No reference world is ice-rich, so no test reads it.
  - _The stagnant-lid anchor (60 m) cannot be reproduced from its documentation_ (the science
    check). `synth.rs` names no regions, detrending or statistic for its 43–55 m (Mars's plains)
    and 89 m (the maria), and doesn't say how 60 m follows from them × 1.15, the ruling's
    unpublished ratio: 49–63 and 102 m. On 16 ppd MOLA the 35 km-window RMS runs from 7 to 148 m
    with the windows' bounds and statistic. Medium-low confidence, as labelled; for a docs batch on
    `synth.rs`, which this lane may not edit.
  - _The mobile-lid anchor's abyssal-hill part is uncited_ (the science check). The BED part is
    verified: Hirt and Rexer 2015's Table 3 and eq. 6 give 183.1–183.3 m above degree 556, which
    the documentation rounds to 184 m. "GMRT's multibeam … 50–230 m RMS at 2–10 km over 71%" has no
    primary source or "not re-read" label, and 183 m with 50–230 m over 71% gives 188–266 m, so
    200 m is a choice within that. For a docs batch on `synth.rs`: cite GMRT (Ryan et al. 2009) and
    an abyssal-hill source, or label it.
