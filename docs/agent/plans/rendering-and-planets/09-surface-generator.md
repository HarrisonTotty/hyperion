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
cached while it is wanted and never stored. The field holds plates rasterised as a Voronoi diagram,
elevation scaled so that the reconstructed surface's hypsometric standard deviation σ_h is plan
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
        crust_palette: [Option<u8>; 4], main_liquid: Option<u8> */ }
    pub struct SynthesisCell { /* elevation_mm: i32,
        boundary_distance: i16 (1 km, saturating),
        plate: u8, crust: Crust, boundary: BoundaryKind, boundary_obliquity: u8,
        flow: FlowDirection, drainage: LogArea, steepness: LogSteepness,
        water_surface_mm: i32, ice: u8, substances: u8 (ice and liquid entries),
        class: SurfaceClass, crater_state: u8 */ }      // 22 bytes in the payload
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
    pub struct CraterParams { /* n_1km: PerSquareKilometre, screening: Screening (P ÷ g,
        projectile density, or a crater cutoff), g, k_target, impact_velocity: Option */ }
    pub fn cumulative_density(p: &CraterParams, d: Metres) -> PerSquareKilometre;
    pub fn saturation(d: Metres) -> PerSquareKilometre;   // Trask 1966, 0.079 D⁻² km⁻²
    pub fn transition_diameter(p: &CraterParams) -> Metres;  // 19 km × (1.62 ÷ g) × k_target
    pub struct CraterShape { /* morphology, depth, rim_height, floor_radius, peak_relief,
        peak_radius, exterior_width (a) */ }
    pub fn crater_shape(p: &CraterParams, d: Metres) -> CraterShape;  // Design notes 12, 13
    pub fn diameter_in_octave(p: &CraterParams, octave: Octave, u: UnitUniform) -> Metres;
    pub fn profile(r_over_radius: f64, crater: &CraterShape, band: BandLevel) -> (f64, f64);
}
pub mod synth {
    pub struct BandLevel(u8);
    pub struct BandSpectrum;                               // per-contribution amplitudes by level
    pub struct Synthesiser<'a, F: FieldView> { /* field, detail seed */ }
    impl<'a, F: FieldView> Synthesiser<'a, F> {
        pub fn new(field: &'a F, seed: DetailSeed) -> Self;
        pub fn height_at(&self, cache: &mut SynthCache, dir: [f64; 3], level: BandLevel)
            -> Result<HeightSample, QueryHeightError>;     // NotSurveyed, NonFinite
        pub fn bake_patch(&self, cache: &mut SynthCache, patch: PatchKey)
            -> Result<PatchHeights, QueryHeightError>;     // heights, parent band, gradients
        pub fn height_range_m(&self, patch: PatchKey) -> (f64, f64);  // for R05's culling
    }
    pub struct SynthCache;                                 // caller-owned, u64 keys, with R05's
                                                           // LatticeCache inside
    pub fn level_bound_m(header: &FieldHeader, level: BandLevel) -> f64;   // hard bound
    pub fn local_variance(spectrum: &BandSpectrum, level: CoarseLevel) -> SquareMetres;
    // Per-cell closed forms: they read the cell's crust, boundary, k_s, drainage and crater
    // state through the view, so `None` where the view does not hold the cell.
    pub fn unresolved_variance(field: &impl FieldView, cell: PatchKey, finer_than: Metres)
        -> Option<UnresolvedVariance>;                     // per contribution, R10's weights
    pub fn structure_function(field: &impl FieldView, cell: PatchKey, finer_than: Metres,
        lag: Metres) -> Option<SquareMetres>;              // R10's slope readouts
    pub fn unresolved_rms(field: &impl FieldView, cell: PatchKey, resolution: Metres)
        -> Option<Metres>;
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
    pub fn for_body(record: &BodyRecord, ctx: &SystemContext) -> Result<Self, SurfaceInputsError>;
    pub fn builder() -> CoarseInputsBuilder;
}
pub enum SurfaceInputsError { NoSolidSurface, NotModelled(RecordSection), .. }
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
private `DerivedBody`, so `for_body` answers `NotModelled(RecordSection::Surface)` until plan 14's
surface section is inhabited (P14.T24 and the asks drafted as P14.T48).

### Server (`hyperion_server`)

```rust
pub mod surface { SurfaceService, FieldKey, FieldCache, SurfaceJobError, InputsSource,
                  RecordInputs /* production: CoarseInputs::for_body */ }
pub mod knowledge::surveys { SurveyPass, SurveySource, SurveyLog, Coverage, LoadSurveysError }
pub const MAX_SURVEY_RANGES: usize = 256;   // `limits.rs`: ranges a `survey_pass` may carry
```

`InputsSource` is the seam between the service and plan 14: `fn inputs(&self, record: &BodyRecord,
ctx: &SystemContext) -> Result<CoarseInputs, SurfaceInputsError>`. Production uses `RecordInputs`,
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
  (Criterion, off the browser target). The client's workers, `HeightWorkerPool.postField` and R04's
  `wasm/` loader are R10's to use, not this plan's; the wasm module's exports serve the test planet
  only, and the export that copies a field into the module, which R05's T10.b record assigns to this
  plan, is R10's, with its client (R10 Design note 16).
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

   - **The volatile history**, `WetEpoch { start, end, effective_flow, paleo_inventory }`, with
     `effective_flow` defined in Earth-equivalent years (the epoch's length scaled by its flood
     frequency relative to an arid-to-semiarid Earth's), which is the solver's t (Design note 9;
     corrected 2026-10-09 by T0.b from "years at effective discharge").
   - **The crater contract**: N(>1 km) with its belt scaling, the screening inputs (surface
     pressure over gravity and a projectile density, or a crater cutoff), g, a target factor
     k_target (1 rock, 0.12 ice-rich), and optionally a mean impact velocity; the surface crate
     derives the rest (Design note 12).
   - **The climate regime classifier** of open question 9, which plan 14 does not yet have: its
     three fields (a thermal regime, a forcing from the locking state, the solar day and the
     obliquity, and a condensable) and the regime's named coarse model, with four corrections:
     Koll's redistribution factor f = 2/3 − (5/12) X ÷ (k + X) at k = 2, with airless-like below X <
     0.087 k and efficient redistribution above 15.7 k, for locked and slow rotators only;
     Wordsworth's CO₂ collapse pressure (his eq. 43) for pure CO₂ between 1 and 10 M⊕ only; a
     slow-rotator onset that rises with flux, a solar day of 16 d at 1.4 S⊕ to 48 d at 1.9 S⊕ (Yang
     et al.'s Table 1), not one number; and an equatorial ice belt at 54–126° obliquity (53.9°
     recomputed) decided with the volatile history, since Kilic et al.'s belt is reached only from a
     colder state.
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
   Each cell takes the seed of largest dot product with its centre, the product perturbed by the
   low-frequency warp on `surface.coarse.warp`, ties to the lower seed index. Boundaries are cell
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
   capped at 200 Myr, beyond which GDH1 is within 40 m of its asymptote; collisional belts 200–500
   km wide with plateaus near 5 km and peaks near 9 km, trenches 2–4 km below the sea floor and arcs
   100–200 km behind them, all heights scaled by (g⊕ ÷ g) × the lithosphere factor as
   strength-limited; flexure w = w₀ e^(−x/α)(cos x/α + sin x/α) with α = [4D ÷ ((ρ_m − ρ_fill)
   g)]^¼, D = E T_e³ ÷ 12(1 − ν²), E 100 GPa, ν 0.25 (Olympus Mons at T_e = 70 km gives α ≈ 180 km,
   as observed). The synthesis's amplitudes are fixed per body by its `BandSpectrum` (per-degree
   variance ∝ l^−1.9 by default, the observed −1.7 to −2.05), so its expected variance below the
   coarse cell is closed-form (`local_variance`). The coarse elevation is scaled affinely about its
   area-weighted mean so that the **reconstructed** field's variance, the interpolant of Design note
   13 integrated by solid-angle quadrature at sub-cell points, is σ_h² less that share, and again
   after erosion; the cell values alone would fall several per cent short by the spline's smoothing.
   The local share is not negligible at the plan's own levels: above the coarse cell's Nyquist
   degree it is 0.6% of σ_h² for an Earth or a Mars (about 200 m RMS), 4% for Mercury, 5% for Ceres
   and 7% for the Moon (about 600 m), against the brainstorm's 0.1–0.2% and 2%, which hold at a 35
   km wavelength. Sea level is the area-weighted quantile that leaves plan 14's ocean fraction below
   it. The realised σ_h and greatest relief go in the header.
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
   each: the equator–pole contrast by replacing the annual field's P₂(sin φ) coefficient (the sign
   carries a warm pole), a locked world's day–night contrast by P₁(cos γ), and the mean by a
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
   wet now runs to steady state; one dry now runs for t = `effective_flow`, only where the surface
   is older than the epoch's end; one with no wet epoch skips the step. m = θ = 0.45 (Kirby and
   Whipple 2012, from memory), with the drainage area replaced by Hergarten 2021's runoff-weighted
   equivalent area from the climate step, normalised by a reference runoff of 1 m a⁻¹, which is the
   only place runoff enters (the first draft also multiplied K by runoff to the m, counting it
   twice). Both the paper's code (Inria research-only licence) and Dendry's (GPL-3.0) are
   implemented from the papers alone. Outputs: final elevation, flow direction, drainage area, a
   steepness index k_s, and water surfaces.

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
   10⁻⁶ on a Titan. _The solver's time_ is in Earth-equivalent years: each K is a long-term
   terrestrial calibration that already holds Earth's flood intermittency (Barnhart et al., ¶70:
   the mean annual flood flows about 2% of the year), so `effective_flow` is the epoch's length
   scaled by its flood frequency relative to an arid-to-semiarid Earth. That is how Hoke, Hynek
   and Tucker 2011 (EPSL 312, 1) state Mars's 10⁵–10⁷ yr, "with runoff rates similar to intense
   storms in arid regions on Earth", against 200–5,000 yr of continuous flow; the first draft's
   "years at effective discharge" would have run Mars's epoch about fifty times too short. As a
   cross-check, Black et al. 2017's supplement (Science 356, 727; Table S2: K = 10⁻⁸ m^(1−2m) a⁻¹
   at m = 0.5 over 60 Myr) matches Mars's 50–350 m of trunk-valley incision with K t ≈ 1.9 m^0.1 at
   m = 0.45, which the Mars K above reaches in about 2 × 10⁵ Earth-equivalent years at 0.1 m a⁻¹
   of runoff. These readings were made from the papers by a research sub-agent of T0.b; Howard 2007
   itself (Geomorphology 91, 332) was read only in abstract, and Barnhart et al. stand in for its
   constants.

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
    km), the parameter, so the input is exact (the published a0 of −3.0876 is a misprint of
    −3.0768), extended by the end slopes outside its range (−2 below 10 m). Other bodies take a
    monotone diameter map by π-group scaling (Holsapple 1993), optional in `CraterParams` and the
    identity at unit ratios. Saturation is Trask's N(>D) = 0.079 D⁻² km⁻²; capped craters draw their
    ages from the last T × N_sat ÷ N_prod of the surface age, inverted through the chronology. That
    window is τ(D), the span a crater of diameter D draws its age from: the whole surface age where
    production is under saturation, and T × N_sat(D) ÷ N_prod(D) where it is capped.
    Screening is a projectile scale, d\* = 1.5 (P ÷ g) ÷ ρ_p (5.2 m for Earth, 0.52 km for Venus,
    8.2 cm for Mars, which are the brainstorm's figures and are projectile sizes), giving a crater
    cutoff D_c = 20 d\* (about 100 m, 10.4 km, 1.6 m) and a taper on the differential production,
    f(D) = 1 ÷ (1 + (D_c ÷ D)^4.5), one rule for every body (researched 2026-10-09, by R09.T0.b;
    the first draft's exponent 2 and its break-up branch above 1 MPa, D_c = 20 km with exponent 1.5
    and a floor at 1.5 km, are withdrawn). The check is against Venus's crater catalogue itself, the
    population Herrick and Phillips 1994 (Icarus 112, 253–281) model, whose text could not be
    reached (publisher paywall): the 881 named craters of the IAU's Gazetteer of Planetary
    Nomenclature (USGS, retrieved 2026-10-09; 2.0–270 km, of about 940 that Magellan mapped, from
    memory), binned in 23 logarithmic bins from 1.5 to 300 km and fitted by Poisson likelihood with
    Neukum's polynomial (Craterstats' coefficients) and a free N(>1 km). The best taper is D_c =
    9.0 km with exponent 5.4; 20 d\* = 10.4 km with exponent 4.5 lies 8 below it in ln L (exponent
    4 lies 28, 5 lies 11), while the first draft's exponent 2 lies 714, its break-up branch 846 and
    the two multiplied 116. The adopted taper passes 0.4% of the production at 3 km, 4% at 5 km,
    23% at 8 km, 46% at 10 km and 95% at 20 km, so it needs no floor; its cumulative counts lie
    within 7% of the catalogue's from 1.5 to 95 km, and its N(>1 km) of 3.1 × 10⁻⁴ km⁻² is a
    lunar-chronology age of 0.37 Ga, inside the few hundred Myr to about 0.75 Gyr published for
    Venus's mean surface (from memory). The exponent is measured on Venus alone, so it is low
    confidence for thinner atmospheres, whose cutoffs keep their projectile scales. The transition
    is D_t = 19 km × (1.62 ÷ g) × k_target (Pike's four bodies give g^−1.01), a zone: bowls below
    0.8 D_t, transitional to 1.5 D_t, central peaks above, peak rings from about 9 D_t, multi-ring
    basins above about 16 D_t. Fresh shapes (`decision-r09-t2.md` item 3; d is rim crest to floor,
    h is rim crest above the surrounding surface):
    - _Bowls_ do not depend on gravity: d = 0.20 D and h = 0.20 D ÷ 5.42 = 0.0369 D.
      - Pike 1980b's 0.196 D^1.010 and 0.036 D^1.014; Stopar et al.'s 0.209 and "∼0.04"; Mercury's
        0.199 D^0.995 (Pike 1988).
      - On an airless body (`Screening::None`), the bowl's d/D falls through Stopar et al.'s A-class
        bins, which are 0.125, 0.152 and 0.166 at 63, 141 and 283 m and 0.20 from 400 m. The fall
        is linear in log D between the bins, held below 63 m, and at D × g ÷ 1.62 off the Moon. The
        plan's own gravity scaling is of low confidence.
    - _Complex craters_ are self-similar in D ÷ D_t:
      - d = 0.150 D_t (D ÷ D_t)^0.303, the mean of Kalynn et al.'s LOLA mare and highland fits;
      - h = 0.0402 D_t (D ÷ D_t)^0.399, Pike 1980b's eq. (4);
      - floor diameter 0.389 (D ÷ D_t)^0.249 of D, held at 0.778 above 16 D_t;
      - a central peak to 9 D_t, of relief 0.0238 D_t (D ÷ D_t)^0.900 (Pike 1980b, Table 6) and
        base diameter 0.3 D (Garvin et al. 2003, Mars).
      - So at a fixed D, d ∝ g^−0.70 and h ∝ g^−0.60.
    - _Transitional craters_ blend the two in log D across 0.8–1.5 D_t.

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
    low amplitude on an abyssal plain, domain warp taking offsets only from octaves no finer than
    the level warped. Threefry2x64-20 stays the lattice hash: with the lattice cached it runs once
    per corner, and the cached path is 2.5–3× faster than any hash. _Channels:_ one Dendry network
    extended through about fourteen levels from the coarse cell (36 km) to the band limit, rather
    than separate instances joined: its first level is the coarse flow network itself, a key point
    per coarse cell joined to the cell its `flow` names; each later level's key points, inherited
    from the coarser one and jittered on `surface.channel` (ε = 0.25), join the nearest segment of
    all coarser levels; neighbourhoods are graph neighbourhoods on the cube and distances 3D chords
    between canonical key points, so a face edge needs no special case. The paper's height
    reconstruction is replaced by the physics: the bed at S = k_s A^−θ with A from Hack's law in SI,
    L = 0.320 A^0.6 (Hack's 1.4 in miles; the brainstorm's 1.5 is Tzathas et al.'s mile-based figure
    and is 4.7× too long in SI), and a valley cross-profile from the distance to the channel; each
    level's mean incision over its parent cell is subtracted. After the first level the distance
    search is 3 × 3 with cached bounding boxes and inverse lengths, and levels whose channels are
    narrower than about four band limits are not evaluated. Those channels, under about 8 m wide,
    are about 0.3–0.6 m deep at bankfull by regional hydraulic geometry (Bieger et al. 2015, from
    memory; Leopold and Maddock 1953; Parker et al. 2007), comparable to the finest band's RMS in
    this spectrum, and the band limit resolves channels from about 4 m, so the cut is a cost lever
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
    the cell's elevation. That gives about 22 B a cell (21 B and T2's follow-up B's substance
    byte) plus 50 B per four (twelve months of anomaly, precipitation and wind): some 13.6 MB for
    an Earth at level 8, 3.4 MB for a Mars at 7 and 0.86 MB for the Moon, inside the brainstorm's
    "roughly 2 to 15 MB" and R03's 15 MiB check.
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
    field build and this plan's header carries (`albedo_scale`, `None` until R10 fills it). R10 also
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
to the points where they read it: T12.d (coarse craters) needs T7.a's density; T12.e (σ_h on the
reconstructed field) needs T4's interpolant and T5's `local_variance`; T14.b (the multigrid's
upsample) needs T4; and T14.c (the Mars volume check) needs T6.b's closed-form incision. T17–T19 are
the server: T17 needs T3 and T10, T18 needs T2 (P12.T7 is built), and T19 needs T8, T17 and T18.
T20 closes. T0.b's research items gate T7.a, T12.c, T13.a and T14.b. Every task ends with `just ci`
green, which the orchestrator runs on integration; a lane runs its targeted checks.

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
    `Province` 3, `Ice` 4, `Liquid` 5, `Cover` 6, `Deposit` 7;
  - `MechanicsFamily` (`u8`): `Silicate` 0, `Metal` 1, `WaterIce` 2, `VolatileIce` 3, `Salt` 4,
    `Organic` 5;
  - `PaletteEntry { substance, role, normal_albedo_bvr: [f64; 3], phase_row: u8,`
    `density_kg_m3: f64, transition: Kelvin, mechanics }`, every value resolved by the server from
    plan 14's registry, so that the field stays the client's only input (Design note 17);
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
  - Mars: `basalt`, `mars_dust`, `H2O` and `CO2` ices;
  - Moon: `anorthosite`, and `basalt` as provinces;
  - Ceres: `phyllosilicate`, `Na2CO3`, `H2O` ice.

  Keys are checked by grammar until P14.T49.b's rows exist, when P14.T49.b's test checks every key
  these worlds use.

- **The shape is frozen only from R09.T19's first send.** R10's and R11's re-validations may add
  members (a liquid's index, absorption, viscosity and surface tension for R11.T8) under a
  `SURFACE_PAYLOAD_FORMAT` bump before then.
- Tests: the palette's bounds, order and refusals; every synthetic world valid; `SubstanceKey`'s
  grammar (formula, `e-`, name, and rejections).
- Acceptance: `cargo test -p hyperion-surface field`;
  `cargo test -p hyperion-surface substance_key`.

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

### R09.T6 Channels

- **R09.T6.a The first levels.** `synth/channels.rs`: the network's first level from the coarse
  flow directions and the next levels by nearest-segment joins on `surface.channel`, with graph
  neighbourhoods and 3D chord distances (Design note 13); profiles S = k_s A^−θ with Hack's law in
  SI, C = 0.320 m^−0.2 and h = 0.6 (Hack 1957, eq. 3, converted; flagged as Earth's). Tests: a
  10⁴ km² basin gives a main stream of 280–360 km; every segment descends to its parent within the
  minimum slope; the network leaves each coarse cell through the cell its flow direction names; a
  key point next to a face edge gives the same segments from either face.
- **R09.T6.b The full depth and the mean.** The network to about fourteen levels, with the 3 × 3
  search, cached bounding boxes and the level cut by channel width; each level's incision has its
  mean over the parent cell subtracted; built per patch and cached by integer cell in `SynthCache`;
  the channels' `unresolved_variance` and `structure_function`. Tests: mean incision over every
  parent cell zero to 10⁻⁹ m; the cached build equals the single-point query bit for bit; cache
  order does not change a height. Bench `surface/channels` with a per-level breakdown, against 3.5
  µs a point in wasm (Design note 14), in a new `crates/hyperion-surface/benches/synth.rs`
  (`[[bench]]`, `harness = false`, Criterion off the browser target as `test_planet.rs` is), which
  T8 extends.

Acceptance: `cargo test -p hyperion-surface synth::channels`.

### R09.T7 Craters

- **R09.T7.a The density and the shapes.** After T0.b's Venus item. `craters.rs`:
  `CraterParams` (T2's type, in
  `PerSquareKilometre`); `cumulative_density` from Neukum et al.'s
  a1…a11 with a0 = log₁₀ N(>1 km) (the misprint recorded in the doc comment), the end slopes outside
  10 m–300 km, the optional diameter map, and the screening taper on the differential production
  with exponent 4.5 (Design note 12; T0.b withdrew the break-up rule);
  `saturation`; `transition_diameter`; `diameter_in_octave` by bisection in log D with a fixed
  iteration count. `CraterShape` and `crater_shape`: Design note 12's fresh depth, rim height,
  floor and peak by zone, and Design note 13's exterior width a by the closed-form balance
  (`decision-r09-t2.md` item 3). `testing::FieldBuilder`'s craters switch to it from T2's inline
  0.2 D and 0.84 (D_t ÷ 19 km) D^0.33. Tests: N(>1 km) is the parameter exactly; the function is
  monotone; the
  inversion lands in its octave and its quantiles agree with the density (Kolmogorov–Smirnov over
  10⁵ draws); the projectile scale d\* is 5.2 m, 0.52 km and 8.2 cm for Earth, Venus and Mars, and
  the crater cutoffs 20 d\*; Venus's screened counts over its area at N(>1 km) = 3.07 × 10⁻⁴ km⁻²
  lie within 10% of the Gazetteer's at 3, 5, 10, 20 and 40 km (878, 850, 642, 335 and 104 named
  craters at or above each, Design note 12); the map at Mars's ratios reproduces Ivanov 2001's
  Mars function within ×1.5 in N over 1–100 km; D_t gives Pike's Moon and Earth within 10%;
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
- **R09.T7.b Small craters.** `synth/craters.rs`: octave levels, the 3 × 3 search across face edges,
  counts from the header's density at the canonical point by true area capped at saturation with
  ages from τ(D) (Design note 12), morphology by
  the transition zone, the volume-balanced profile of each crater's `crater_shape` (its own a, at
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

Acceptance: `cargo test -p hyperion-surface craters`.

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
limit. Benches `surface/point` and `surface/patch`, with a per-component breakdown against Design
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
T16 (or T1.b) has not already (whichever lands first bumps, through the orchestrator). R05's
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

### R09.T11 Plates

`steps/plates.rs`, Design note 6. Tests: every cell has one plate and ties go to the lower seed
index; boundaries are exactly the adjacencies between plates; signed distances are zero on
boundaries and grow by at most one edge length a step; boundary kinds follow the relative motion on
hand-placed plates; the plate areas follow the tail law within a Kolmogorov–Smirnov bound over many
seeds; the mean plate count over those seeds is 48 ± 7; the areas sum to 4π; the distance to
the plate's own divergent boundary, which GDH1 reads, is zero on ridges and never taken from a
trench; a stagnant-lid world has none. Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::plates`.

### R09.T12 Coarse elevation

- **R09.T12.a Crust and ridges.** `steps/relief.rs`: Design note 7's two crust populations (their
  lithologies from plan 14's `crust`: `Oceanic` the secondary crust's entry, `Continental` the
  tertiary's) about sea level and GDH1's age–depth law from the distance to divergent boundaries.
  Tests: the reference Earth's hypsometry, binned as Earth2014 was (area-weighted 250 m bins, the
  TBI layer at 5′), is bimodal with a land mode within 300 m of +125 m, an ocean mode within 600 m
  of −4,375 m (the ocean peak is flat from −5,300 to −4,300 m) and an oceanic mean within 300 m of
  −4,281 m (Hirt and Rexer 2015's Earth2014, recomputed 2026-09-29); a ridge's depth follows GDH1 at
  5, 20 and 100 Myr to 1 m.
- **R09.T12.b Convergent landforms.** Belts, trenches and arcs by boundary kind, scaled as Design
  note 7 says. Tests: belts lie within their stated distance of convergent boundaries; trenches lie
  2–4 km below the neighbouring sea floor and arcs 100–200 km behind them.
- **R09.T12.c Stagnant-lid provinces and flexure.** After T0.b's isotherm item. Volcanic provinces
  sized by the volcanism level
  and the flexural moat and bulge about each load, with T_e = k (870 K − T_s) ÷ F (T0.b's ruling,
  Design note 3). T_e's isotherm and conductivity come from the crust's substance: the ice lean of
  Risks becomes the `H2O` row's (decision-composition). Tests: a line load's bulge crest lies at πα
  with height 0.043 w₀; α at T_e = 70
  km under Mars's gravity is 180 km ± 10; T_e is 108 km ± 1 at Mars's 19 mW m⁻² and 210 K, and
  zero where T_s ≥ 870 K, where each load is compensated locally (α = 0).
- **R09.T12.d Coarse craters.** `steps/craters.rs`, Design notes 5 and 10, after T7.a. Each
  crater's shape is T7.a's `crater_shape`, so the coarse elevation and the client's T7.c rims are
  one profile. Tests:
  counts over the reference Moon match min(production, saturation) above D_b (Poisson interval),
  about 300 above 100 km with N(1 km) at 4.4 Gyr; ages split before and after the wet epoch on the
  reference Mars; the list is sorted and each crater's `reach` is complete; before degradation,
  the reference Moon's coarse craters' rim heights are Pike 1980b's 0.236 D^0.399 to 1%.
- **R09.T12.e σ_h and sea level.** Design note 7, on the reconstructed field, after T4 and T5.
  Tests: the reconstructed σ_h equals the input to 10⁻⁶ relative after the step;
  `local_variance ÷ σ_h²` is 5–9% on the reference Moon and 0.4–1% on the reference Earth; the
  realised relief lies at 6–16 σ_h on every reference world (Design note 7's observed 7–15 widened
  by one σ_h each way, since one realisation scatters about it); the ocean fraction below sea level
  equals the input to one cell's solid angle; the header reports realised σ_h and relief.

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
  axis. Tests: with no land–sea contrast the zonal model's answer is reproduced to 0.01 K; a locked
  world is warmest at the substellar point.
- **R09.T13.c The other regimes.** `steps/climate/{airless,isothermal}.rs`: radiative equilibrium
  with thermal inertia for airless and thin-atmosphere worlds, and the isothermal surface for a
  Venus. On a locked lava world, cells above the secondary crust's transition temperature are melt,
  from this task's per-cell temperature, not from the global state. Tests: an airless world's
  day–night contrast exceeds 300 K; the isothermal model's surface varies by under 1 K; a locked
  lava world's melt is the dayside region above the solidus, and its nightside is solid.
- **R09.T13.d Normalisation and ice.** Plan 14's mean and signed contrasts imposed on their
  components (P₂, P₁, a constant after the lapse term); each ice placed per condensate, coldest
  first against its own frost point by annual maximum, until its area matches plan 14's area for
  that substance (`surface_ices`), and each liquid's surface to its share of the ocean fraction
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
  - a locked circular world has one wind.

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
  runoff once). Tests: a ridge-to-sea profile matches the closed-form steady state; the fixed
  point's residual falls monotonically over the multigrid levels on the reference Earth; K is 6.3
  × 10⁻⁶, 2.4 × 10⁻⁵ and 3.1 × 10⁻⁶ m^0.1 a⁻¹ (to 2%) for an Earth's rock under water, a
  saturated Mars's regolith and a Titan's ice under methane. K's bed factor B comes from a
  (substrate, fluid) table over the registry's rows, and ρ_f is the liquid's density. The three
  calibrated cases (rock, saturated regolith, ice under methane) stay its tests
  (decision-composition).
- **R09.T14.c The branches and the Mars check.** The wet-now, dry-now and never-wet branches, after
  T6.b. Tests: the dry-now branch leaves surfaces younger than the epoch's end untouched; a
  never-wet world's elevation is unchanged by the step; on the reference Mars the coarse erosion
  volume **plus**
  the synthesis's expected sub-cell channel incision volume (closed-form per cell from k_s, Hack's
  law and the network's widths) is at least 1.2 m of global equivalent layer (Luo, Cang and Howard
  2017's (1.74 ± 0.8) × 10¹⁴ m³ over 1.444 × 10¹⁴ m²), since valley networks 1–10 km wide live
  mostly below a 38 km cell; the final residual is recorded by a slow test.
- **R09.T14.d Rescale and outputs.** σ_h re-matched on the reconstructed field, lapse and mean
  re-applied, k_s, flow directions and water surfaces. Tests: σ_h and the mean temperature again
  match; lakes are level.

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
`crates/hyperion-sim/benches/surface.rs`. Bump `GENERATOR_VERSION` if T9 (or
T1.b) has not already (whichever lands first bumps, through the orchestrator), or again if the first
bump has been released to a save. Acceptance: `cargo test -p hyperion-sim --test
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
`NotModelled` naming its section. Acceptance: `cargo test -p hyperion-server surface`.

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
  on the scene topic; two pushes merged while a reader is slow carry each body's latest revision.
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
2026-10-09); from then it bumps both. The first bump is made by whichever of T1.b (whose filled
hooks section moves plan 14's golden
systems), T9 and T16 lands first, through the orchestrator, which coordinates bumps one lane at a
time; later tasks that change a committed golden bump again. R05's `FINEST_SPACING_M`, `BAND_LIMIT_M`
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
T2, as built"). R10.T10.a fills `FieldHeader.albedo_scale` and bumps `GENERATOR_VERSION`,
re-blessing this plan's payload and coarse goldens in that commit.

## Risks and open points

- **Low-confidence constants.** The σ_h fit rests on one body per constant (0.9 km, 0.16, the √N
  age law, the 70 km lithosphere normalisation); stream-power erodibility across fluids and
  gravities; the energy-balance transport cap; Venus's crater screening; the complex-crater peak's
  base width (0.3 D, Mars's) and the regolith bowl's gravity scaling (`decision-r09-t2.md` item
  3); Mars's complex craters deepening faster than the one law (exponent 0.53–0.58 against 0.30),
  which leaves its largest fresh complex craters about 30% shallow. Each is a named constant with
  its source and confidence in its doc comment.
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
  - _Erosion at the coarse cell._ At 36–40 km every cell carries a trunk channel (A ≥ 1.3 × 10⁹
    m²), so the transient solver lowers a whole cell at the trunk's rate, and T14.c's Mars volume
    (at least 1.2 m of global layer) could pass by over-erosion (about 100 m a cell at K t ≈ 1.7,
    the sub-agent's estimate). Lean: T14.c also bounds the coarse volume from valley cross-sections
    (incision × valley width ÷ cell width), and records the cell-mean lowering against a fine-grid
    run of one basin. Howard 2007, Hergarten 2021 and Burr et al. 2006 were not read in full;
    Barnhart et al. 2009 stand in for Howard's constants.
  - _Plan 14's wet epoch._ P14.T48.b's "10⁵ to 10⁷ years of active flow (Hoke and Hynek 2009; …)"
    was Hoke, Hynek and Tucker 2011's elapsed time at Earth-like arid runoff; T0.b corrected the
    Phase K draft with `effective_flow`'s definition (Design note 9).
- **The survey stand-in.** `survey_pass` lets a client grant its ship coverage, as the server
  grants detail levels today. It is a discipline for an honest client, and the sensors plan
  replaces the caller, not the core.
- **Field size.** Design note 17's layout gives about 13.2 MB for an Earth. If the climate layer
  at L − 1 proves too coarse for R11's clouds, sending it at level L costs about 2.1× (about
  28 MB on an Earth), past R03's 15 MiB check.
- **Dependencies.** Plan 14 calling the surface crate's crater density is a new edge that R04's
  split allows (the sim already depends on the surface crate). P12.T7 is built, and T18 builds on
  it. The channel network's physics profile replaces Dendry's own height reconstruction, which the
  paper did not test.
- **Asks of other plans, which their owners may not yet carry**, each with its state in the
  roadmap's asks table (states as of the re-validation of 2026-10-09):
  - plan 14's five of Design note 3: drafted by T0.a, as P14.T48.a–e in plan 14's "Phase K", for
    plan 14's owner to accept;
  - plan 14's P14.T23 surface seed: carried by plan 14, built (sim side); the hooks section's wire
    value is this plan's T1.b;
  - plan 14's surface section and P14.T24.a–b's conditions and figures, which `for_body` reads:
    carried by plan 14, unbuilt;
  - plan 04's two table rows: drafted by T0.a, in plan 04's reserved-kinds table, for plan 04's
    owner to accept;
  - plan 12's P12.T7 store: carried by plan 12, built;
  - R03's `surface_revisions` field: drafted by T0.a, its shape in R03's Design note 4, for R03's
    owner to accept (R03 left room for it); the note that the camera reports do not bound the field:
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
    names: T1.b after T1.a (with the bump); T3 and T4 after T2; T5, T6.a and T10 after T1.a and T2;
    T7.a after T2 and T0.b's Venus item; T7.b after T7.a; T7.c after T7.b; T6.b after T6.a; T8 after
    T4–T7; T9 after T8; T11 after T10; T12.a–b after T11; T12.c after T11 and T0.b's isotherm item;
    T12.d after T10 and T7.a; T12.e after T12.a, T4 and T5; T13.a after T10 and T0.b's transport-cap
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
  - _`BandSpectrum` is the law, not a table_: degree variance V(l) = V₁ l^−β (β default 1.9,
    above 1; V₁ in `SquareMetres`). T5 derives each level's per-contribution amplitudes from it,
    and may add fields before T9.
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
    Design note 12's bounds of 9 and 16 D_t (0.8 and 1.5 now match Krüger et al. 2018), the 0.12
    ice factor and the −1.7 to −2.05 spectral range carry no external
    source and are labelled the plan's own.
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
      followed) is asked of "main".
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
    - Moon: anorthosite (primary), basalt (province);
    - Ceres: phyllosilicate (secondary), `H2O` ice, `Na2CO3` (deposit). No share names the water
      ice, since plan 14 states no area for its cold traps yet.

    Each substance's figures are a row in `reference.rs` with its sources. They began as follow-up
    B's synthetic palettes' figures and are kept apart from them, not shared, so that a correction
    to one set moves only its own output: the wire goldens pin the synthetic fields', and T16's
    goldens will pin these. The albedos are grey across B, V and R (P14.T49.c resolves each band),
    and every `phase_row` is 0 until R10 assigns its rows. The sim's `clippy.toml` gains `WebBook`.
    The science review (2026-10-10) found no wrong value.
    - It read the basalt solidus as 980 ± 10 °C, a natural tholeiite's (Wright and Okamura 1977,
      USGS PP 1004, Table 15), which these rows take: 70 K below the unread 1,050 °C that the
      synthetic rows keep.
    - It labelled what it could not read or what is assumed: the anorthosite's solidus is its
      plagioclase's alone, an upper bound for the rock (its pyroxene melts near 1,270 °C); liquid
      water's Fresnel factor stands in for `A_N`, which a specular surface lacks, until R11's
      ocean; the dust's and the phyllosilicate residue's solidi are basalt's by assumption; and
      granite's density and solidus are from memory (Huang and Wyllie 1975 named, not read).
    - It confirmed the rest against the sources cited, and the Fresnel factors and Mangan et al.'s
      fit at 150 K by recomputation.

    Findings for "main" from it: the synthetic rows' basalt solidus, whose correction re-blesses the
    wire goldens; the densities mix grain and porous bulk values (a convention for P14.T49.b, with
    porosity apart); and two questions on §1.7's lists, a water-ice entry for the Moon's cold traps
    (Li et al. 2018, PNAS 115, 8907) and whether Ceres's altered-ocean crust is better primary
    than secondary (lean for both: keep §1.7's lists until plan 14 states them).

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
- **Rulings of "main" for T10's findings** (2026-10-10).
  - `for_body`'s missing arguments, the record's time and a moon's planet's orbit, are settled by
    P14.T54.a, which owns `planetary/surface/inputs.rs`. R09.T17 and R09.T19.a take `for_body` as
    T54.a leaves it.
  - The airless mean temperature is imposed as the radiative mean ⟨T⁴⟩^¼ in R09.T13.c–d, T10's
    lean, with T13's science check to confirm. `CoarseInputs::mean_surface_temperature`'s
    documentation says so.
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
