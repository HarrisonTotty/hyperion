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

Domain tags, under a "Rendering plan R09" heading: `body.surface.detail` in the sim's registry, and
the rest in `hyperion_surface::tags`, the registry R04 gives the surface crate for `surface.*` names
(R04's Design note 4):

| Tag                      | Scope           | Key            | Draws                                                   |
| ------------------------ | --------------- | -------------- | ------------------------------------------------------- |
| `body.surface.detail`    | `Body`          | `BodyId`       | the detail seed, one block output keyed by `BodyId`     |
| `surface.coarse.plates`  | `SurfaceCoarse` | `surface_item` | plate count, seeds, Euler poles, crust type per plate   |
| `surface.coarse.warp`    | `SurfaceCoarse` | `surface_cell` | the low-frequency boundary warp's lattice               |
| `surface.coarse.relief`  | `SurfaceCoarse` | `surface_cell` | coarse landform noise, volcanic provinces               |
| `surface.coarse.crater`  | `SurfaceCoarse` | `surface_item` | craters of D_b and wider: count, place, age, morphology |
| `surface.coarse.erosion` | `SurfaceCoarse` | `surface_cell` | random receivers and the multigrid's jitter             |
| `surface.relief`         | `SurfaceDetail` | `surface_cell` | structural noise lattices                               |
| `surface.channel`        | `SurfaceDetail` | `surface_cell` | Dendry key points per cell and level                    |
| `surface.crater`         | `SurfaceDetail` | `surface_cell` | craters below D_b per octave cell                       |

`body.surface` (plan 14's) is the surface seed's own tag. `surface.scatter` is named by the
brainstorm and reserved here for R11 (Generator version).

### `hyperion-surface`

Cells are R05's `cube::PatchKey` at the field's level; `HeightSample`, `LatticeCache`,
`FINEST_SPACING_M`, `BAND_LIMIT_M`, `finest_level` and `num::{min, max}` are R05's. R05 defines
`HeightSample` and `LatticeCache` in its provisional `test_planet` module, which R10 retires, so T4
moves them to `hyperion_surface::height` and leaves `test_planet` re-exporting them.

```rust
pub mod field {
    pub struct CoarseLevel(u8);                            // 5..=8
    pub fn coarse_level(radius: Metres) -> CoarseLevel;    // Design note 4
    pub fn boundary_diameter(level: CoarseLevel, radius: Metres) -> Metres;  // D_b
    pub fn cell_index(cell: PatchKey) -> u32;              // face-major Morton order at the level
    pub struct FieldHeader { /* format, generator_version, body, radius,
        figure: (equatorial a, polar c) (the height datum, Design note 17), level, D_b,
        sea_level, lapse_rate, spectrum: BandSpectrum, craters: CraterParams,
        climate_model: ClimateModelKind, precipitation: PrecipitationSource (always Heuristic),
        realised_sigma_h, realised_relief, months: u8,
        anomaly_step: u8 (0.25 K × 2ⁿ, per body, Design note 17), surface_age: Gyr,
        surface_pressure: Pascals, albedo_scale: Option<f64> (R10's) */ }
    pub struct SynthesisCell { /* elevation_mm: i32,
        boundary_distance: i16 (1 km, saturating),
        plate: u8, crust: Crust, boundary: BoundaryKind, boundary_obliquity: u8,
        flow: FlowDirection, drainage: LogArea, steepness: LogSteepness,
        water_surface_mm: i32, ice: u8, class: SurfaceClass, crater_state: u8 */ }
                                                           // 21 bytes in the payload
    pub struct ClimateCell { /* at level − 1: sea_level_temperature: i16 (0.01 K),
        month_anomaly: [i8; 12] (header's step), month_precipitation: [u8; 12] (log rate),
        wind: [Wind; 4] */ }                               // 34 bytes, one per four cells
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

### `hyperion-sim` (`planetary::surface`, and plan 14's hooks)

```rust
// in `planetary::hooks`, beside P14.T23's seed (hooks/mod.rs):
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

Environment: `HYPERION_SURFACE_CACHE_MB` (default 128), with `--surface-cache`.

### Protocol (`hyperion-protocol`, mirrored in `@hyperion/protocol`)

- `surface_field`: `SurfaceFieldRequest { universe, body: BodyIdHex, have_revision: Option<u32> }`,
  answered in bulk (R03's Design note 10): the payload's binary frames, then the enum
  `SurfaceFieldDto`, either `Ready { header: SurfaceHeaderDto, revision: u32, coverage: CoverDto,
bulk: BulkManifestDto }` or, with no bulk, `NotModelled { section }` for a body whose plan 14
  sections the pass needs are not yet modelled (the record's own `not_modelled` convention). An
  unknown body is `unknown_body` and a body with no solid surface `bad_request`, each naming
  `body`.
- `survey_pass`: `SurveyPassRequest { universe, body, from, to, source: SurveySourceDto,
resolution_m, cells: Vec<CellRangeDto> }` answered by `SurveyPassDto { revision }`, where
  `CellRangeDto` is `{ start: u32, end: u32 }` and `cells` holds at most `MAX_SURVEY_RANGES` (256)
  ranges, so that a request stays under plan 04's 16 KiB `MAX_INBOUND_FRAME_BYTES`; a longer pass is
  sent as several requests with one time span. More ranges is `bad_request` naming `cells`.
- `CoverDto`: `ranges: Vec<CoverRangeDto>`, each `{ start: u32, end: u32, resolution: u8 }`, a
  half-open range of `cell_index` values with its `ResolutionCode`, so that R10's readouts know the
  resolution per cell. It travels server to client only.
- On R03's scene topic, the optional field `surface_revisions: Vec<SurfaceRevisionDto>` of
  `{ body: BodyIdHex, revision: u32 }`, which R03 leaves room for.

### Test helpers

`hyperion_surface::testing::{synthetic_field, SyntheticWorld, FieldBuilder}`,
`hyperion_sim::planetary::surface::reference::*`,
`crates/hyperion-server/tests/common/surface.rs::{field_via_wire, decode_all, reference_inputs}`
over R03's `TestClient::next_binary()`, and `crates/hyperion-sim/examples/surface_map.rs`, which
writes a field's elevation, class and flow as equirectangular PPM images under `target/` for the
check by eye. R10's reference-world tests live beside this plan's `surface_coarse_golden.rs` in
`crates/hyperion-sim/tests/` (`surface_material_worlds`, `surface_weights_worlds`,
`surface_ground_worlds`, `level_bound_worlds`, `surface_reading_worlds`, `surface_albedo_worlds`),
over a shared `tests/common/surface_worlds.rs` that runs `reference::*` through the pass once per
binary; T16 creates that file with its own goldens so that R10 extends it.

## Consumes

Names are those of the owning plans' Provides as they stand; R09.T0 reconciles them with the code.

- **R04.** `hyperion-base` with `math`, `rng`, `units`, `version` and `ObjectKey::{system, body}`;
  `domain_tags!` exported from base with `DomainTag::registered` `#[doc(hidden)] pub`, and the three
  registries with `assert_registries_disjoint`; the `hyperion-surface` skeleton with
  `hyperion_surface::tags`, its self-contained `clippy.toml` and relaxed-SIMD `compile_error!`;
  `just test-wasm-fast` (both wasm targets, in `just ci`) and `just test-wasm-slow`; the testkit's
  embedded `golden!` arm and `golden::check_embedded`; `compute::probe_flush_to_zero`,
  `CpuPool::with_probe` and `JobError::FloatingPointMode`; `BodyHooksDto.detail_seed:
SectionDto<DetailSeedHex>`, `not_modelled` until this plan fills it, and the client's
  `BodyHooks.detailSeed`. R04's test conventions for the surface crate and base (R04 Design note
  12): every test module carries the `wasm_bindgen_test as test` import; a test that cannot run on
  `wasm32-unknown-unknown` sits in a module named `native_only`; and every `should_panic` test
  states `expected` and lives in the crate's own `tests/panics.rs` binary.
- **R05.** `hyperion_surface::cube` (`Face`, `FaceUv`, `st_to_uv`, `uv_to_st`, `face_uv_to_xyz`,
  `xyz_to_face_uv` with its canonical-face rule, `PatchKey` with `edge_neighbour` and
  `corner_neighbours`, `vertex_dir`), `FINEST_SPACING_M`, `BAND_LIMIT_M`, `finest_level` (Earth →
  19); `test_planet`'s `HeightSample` and `LatticeCache`, `hyperion_surface::num`'s `min`, `max` and
  `assert_finite`, and `hyperion_surface::noise`'s `gradient_noise`, `NOISE_BOUND` (B) and
  `NOISE_RMS` (σ_noise), 3D improved Perlin noise with Perlin 2002's 16-entry gradient table (R05's
  Design notes 12 and 15); the `patch` module's `PatchBake` layout and `finest_surface_height`,
  which R10 moves onto this plan's `Synthesiser`. The client's workers, `HeightWorkerPool.postField`
  and R04's `wasm/` loader are R10's to use, not this plan's.
- **R03.** `bulk::{Answer, BulkPayload}` with the handler seam's `Answer { body, bulk }`,
  `BulkManifestDto`, `MAX_BINARY_FRAME_BYTES` (262,144) and `BULK_QUEUED_BYTES`, chunks before the
  terminal response, `cancel` stopping them; the scene topic's `SceneNotificationDto` with room for
  `surface_revisions`; `TestClient::next_binary()`; the Knowledge-bound scene test, which T19
  extends to surveyed cells. R03's camera reports are not used to bound the field (Design note 19).
- **Galaxy plan 04.** The reserved-kinds table in
  [its "Extending the convention"](../galaxy-generation/04-server-and-protocol.md#extending-the-convention),
  which new kinds enter first; `RequestBody`, `ResponseBody` and `REQUEST_KINDS` in
  `crates/hyperion-protocol/src/envelope.rs`, and `kind` (`pub(crate)`), `is_large` and the
  `every_body` walk in `crates/hyperion-server/src/requests/mod.rs`; `MAX_INBOUND_FRAME_BYTES`
  (16 KiB) in `crates/hyperion-server/src/limits.rs`;
  `compute::{CpuPool, Priority::Bulk, CancelToken, SingleFlight, GalaxyKey}`;
  `cache::{ByteLru, SharedByteLru, HeapBytes}`; the `knowledge/` directory reserved in
  `crates/hyperion-server/src/universe/store.rs`; `TestServer`, `TestClient`.
- **Galaxy plan 12.** P12.T7's `KnowledgeStore`, its `knowledge/contacts.v1.jsonl` persistence
  through `UniverseStore`, `spawn_blocking` writes, the torn-line rule and
  `LoadKnowledgeError::UnsupportedFormat`. P12.T7 is not built; T18 waits for it.
- **R07.** Design note 19's single height datum, the rotational spheroid, and the flattening f
  that R07.T1 asks of plan 14; until plan 14 sends f the spheroid is a sphere.
- **Galaxy plan 14.** `BodyRecord` and `Section`, `SystemContext`, the `body.surface` tag and
  P14.T23's surface seed; P14.T13.c's `SurfaceState` and `SurfaceMaterial`, surface pressure and
  gases; P14.T14's rotation and `BodyFixedFrame` with `body_fixed_at` (P14.T14.c), for seasons and
  a locked world's substellar axis; P14.T24.a's `SurfaceConditions` and P14.T24.b's
  `GlobalFigures` (ocean and ice fractions, tectonic regime, volcanism, heat flow, surface age,
  crater density); and the asks of Design note 3. None of T14, T23 or T24 is built at the time of
  writing (`crates/hyperion-sim/src/planetary/` has no `hooks` or `frames.rs`, and `record.rs`'s
  `Surface` and `Hooks` have no value).
- **Plan 01's discipline**, through R04's base crate: `Stream::open`, `domain_tags!`, the tag
  golden `tests/golden/rng/tags.golden` and `domain_tags_are_pinned`, `hyperion-testkit`'s
  `golden!` and `GoldenWriter` (`crates/hyperion-testkit/src/golden.rs`),
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
   `ClimateRegime`. Five asks go to plan 14 (R09.T0.a), because plan 14 as written does not provide
   them (its P14.T24.a contrasts are unsigned, it has no continental fraction, and it has no regime
   classifier at all):
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
     2001; Watts and Burov 2003), and moving to 723 K would re-fit that constant (Risks). The
     lithosphere factor acts on that share and the edifice cap only. Every body but one calibrates a
     constant: Earth the mobile-lid terms, Venus the stagnant-lid 0.9 km (0.94 km observed, with
     almost no basin or constructional share, so its −4% is the rounding and not a test), the Moon
     the basin share, Mars the constructional share, Ceres k_comp and Vesta the 0.02 R cap. Mercury
     is the only out-of-sample check, and the fit over-predicts it by 21% (1.27 against 1.05 km), so
     the fit is labelled empirical and tested once (researched 2026-09-29, high confidence). f_c is
     the continental fraction, or failing one the land-plus-shelf area (0.405 on Earth). Greatest
     relief, 20 km × (g⊕ ÷ g), is withdrawn as a published figure (it is Johnson and McGetchin
     1973's 1/g envelope, 53 km for Mars against 29.4 observed); the field reports the realised
     relief, which lies at 7–15 σ_h on every body measured.
   - **The volatile history**, `WetEpoch { start, end, effective_flow, paleo_inventory }`, with
     `effective_flow` defined as years at effective discharge (the epoch's length times its
     intermittency), which is the solver's t (Design note 9).
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
   0.245 tanh((T − 268 K) ÷ 5 K), both shifted to the condensable's freezing point; D scaling as Ω⁻²
   and with pressure, heat capacity and molar mass, capped at 30 D⊕ (provisional; no published cap
   exists, and Risks carries the research's lean) and halved on dry worlds; locked worlds in tidally
   locked coordinates about P14.T14's substellar axis, with no seasons. It steps **implicitly**
   (backward Euler, a tridiagonal solve in latitude and a periodic one in longitude, split in a
   fixed order) at the brainstorm's six hours, since the explicit limit is about 11 h on Earth and
   fails for any slower rotator, and runs a fixed maximum of orbits until the annual mean moves
   under 0.1 K. The grid is 2.5° in latitude by 5° in longitude (72 × 72 cells), and 2.5° × 2.5° in
   a locked world's tidally locked coordinates, rather than the brainstorm's 5° × 30°, which is too
   coarse to interpolate to cells of about 72 km: Spiegel et al. 2008 found a 10° latitude grid
   biases the global mean by over 4 K and converged at 1.25° (researched 2026-09-29, medium
   confidence; Okuya et al. 2019 for a two-dimensional precedent). The fields are interpolated to
   the cells bicubically before the lapse term. A month is a twelfth of the orbital period; a locked
   world on a circular orbit has one. Plan 14's figures are imposed on the component that defines
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
   three-cell pattern scaled by the Hadley width, converging on the substellar point for slow and
   locked rotators. The offline check against ExoPlaSim is a recorded task, never run on arrival.
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
   Whipple 2012, from memory); K scales from Tzathas et al.'s 2 × 10⁻⁵ m^(1−2m) a⁻¹ by fluid density
   and gravity, runoff to the m, and a bedrock factor, with the drainage area replaced by Hergarten
   2021's runoff-weighted equivalent area from the climate step (low confidence, stated in the doc
   comment). Both the paper's code (Inria research-only licence) and Dendry's (GPL-3.0) are
   implemented from the papers alone. Outputs: final elevation, flow direction, drainage area, a
   steepness index k_s, and water surfaces.
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
    **rates** (precipitation per 30.44 d and per 365.25 d); summer is each cell's six warmest
    consecutive months; the temperature thresholds stay. Worlds with one month, and every
    non-seasonal regime, classify surface state (liquid, ice or frost of a named species, rock,
    regolith, melt, organic sediment) with the regime's named zones, such as a locked world's
    substellar ocean and nightside glacier. One `SurfaceClass` byte holds either.
12. **One crater density, in the surface crate** (researched 2026-09-29: Neukum, Ivanov and Hartmann
    2001, Table 1, checked in Michael's Craterstats `functions.txt`; Trask 1966 and Hartmann 1984;
    Pike 1980, Tables 2 and 3; Krüger, Hergarten and Kenkmann 2018; Bland and Artemieva 2006; Ivanov
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
    cutoff D_c = 20 d\* (about 100 m, 10 km, 1.6 m) and a taper f(D) = 1 ÷ (1 + (D_c ÷ D)²); above 1
    MPa break-up adds D_c = 20 km, exponent 1.5 and a floor at 1.5 km (Venus's parameters, low
    confidence). The transition is D_t = 19 km × (1.62 ÷ g) × k_target (Pike's four bodies give
    g^−1.01), a zone: bowls below 0.8 D_t, transitional to 1.5 D_t, central peaks above, peak rings
    from about 9 D_t, multi-ring basins above about 16 D_t. Depth is 0.2 D for simple craters and
    0.84 (D_t ÷ 19 km) D^0.33 km for complex ones, which reproduces the Moon and Earth. 1–2 m
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
    0.4a²; Pike's lunar ratio 5.42 gives a = 1.54, ejecta to 2.54 rim radii, used for every body;
    complex craters add a flat floor and a central peak inside the same balance. The rim is
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
    extreme is Verkhoyansk's −31.0 and +30.2 K about its annual mean (WMO 1991–2020 normals), 0.5 K
    on a Mars, whose high latitudes reach ±45–60 K, and 1 K on a high-obliquity world with land,
    ±50–70 K (Williams and Pollard 2003; Williams and Kasting 1997; researched 2026-09-29, medium
    confidence for Mars and high obliquity), so no anomaly saturates and no cell grows. The climate
    layer is at level L − 1, because it is interpolated from the energy-balance grid and carries
    nothing finer except the lapse term, which the synthesis re-applies from the header's rate and
    the cell's elevation. That gives about 21 B a cell plus 34 B per four: some 12 MB for an Earth
    at level 8, 3 MB for a Mars at 7 and under 1 MB for the Moon, inside the brainstorm's "roughly 2
    to 15 MB". Heights, elevations and sea level are measured along the normal of the body's
    rotational spheroid, R07's reference body (a = R_vol (1 − f)^(−⅓), c = a (1 − f), with plan 14's
    flattening f; a sphere of R_vol until plan 14 sends f), the one datum R07's Design note 19 sets
    for R05's vertices, R07's discs and R10's terrain (researched there, high confidence); the
    header carries a and c. The payload is a sequence of self-contained blocks of at most
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
    field's cells, and T0.a notes in R03 that this plan declines that input, so the reports bound
    only the sensors plan's contacts. Tests reach the service through its `InputsSource` seam with
    the reference worlds, because `for_body` answers `NotModelled` until plan 14's asks land; the
    wire then answers `SurfaceFieldDto::NotModelled { section }`.

## Tasks

T0 first. T1–T3 are the base: seeds, then types and codec. T4–T9 build the synthesis on T2;
T10–T16, the coarse pass, need T1–T2 and may run beside the synthesis only up to the points where
they read it: T12.d (coarse craters) needs T7.a's density; T12.e (σ_h on the reconstructed field)
needs T4's interpolant and T5's `local_variance`; T14.b (the multigrid's upsample) needs T4; and
T14.c (the Mars volume check) needs T6.b's closed-form incision. T17–T19 are the server: T17 needs
T3 and T10, T18 needs P12.T7, and T19 needs T8, T17 and T18. T20 closes. Every task ends with
`just ci` green.

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
- **R09.T0.b Remaining checks.** The research of 2026-09-29 answered the plan's questions; what it
  left open goes to a research agent before the constant it concerns is committed: the σ_h fit's
  age law against Mercury's smooth and intercrater plains (the same pyshtools script, masked); the
  energy-balance transport cap against the FILLET ensemble; a geomorphology pass on erodibility for
  Titan and Mars (Collins 2005; Howard 2007) before T14.b; Venus's screening parameters against
  Herrick and Phillips 1994's size–frequency distribution before T7.a. The review of 2026-09-29
  added the transport cap's rule (Risks) before T13.a and the isotherm of T_e (Risks) before
  T12.c. Acceptance: `grep -n "researched" ` on this plan shows each item in
  its design note, or the item stays in Risks with the agent's lean.

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
  regenerated with `domain_tags_are_pinned`. Acceptance: `cargo test -p hyperion-base rng` and
  `cargo test -p hyperion-surface tags`.
- **R09.T1.b The detail seed on the wire.** Waits for P14.T23, which builds `hooks/mod.rs`,
  `surface_seed` and the hooks section: `planetary::hooks::detail_seed` beside it, and
  `BodyHooksDto.detail_seed` filled, turning R04's `not_modelled` field `ok`. This plan does not
  build `surface_seed`; until P14.T23 lands the coarse pass takes its `SurfaceSeed` from the
  builder, as every test does. Tests: the detail seed of 10⁶ bodies has no duplicates and differs
  from the surface seed of each; the hooks section's wire form carries `detail_seed` and no
  `surface_seed`. Acceptance: `cargo test -p hyperion-sim planetary::hooks` and
  `cargo test -p hyperion-protocol planetary`.

### R09.T2 The field's types

`field.rs`: `CoarseLevel`, `coarse_level`, `boundary_diameter`, `cell_index`, `FieldHeader`,
`SynthesisCell`, `ClimateCell`, `CoarseCrater` with its `reach`, `CoarseField` with its per-cell
crater index, `FieldView` for `CoarseField`, `Cover`, `ResolutionCode`; `testing::{FieldBuilder,
SyntheticWorld, synthetic_field}`, the synthetic worlds T3 to T9 test on (an Earth-, Mars-, Moon-
and Ceres-like field built directly, a flat field and a single crater), each field valid by
construction.

Tests: the level table of Design note 4 (Earth 8, 2 R⊕ 8, Mars 7, Moon 6, Ceres 5, with the
brainstorm's cell sizes within 5%); D_b equal to twice the closed-form largest edge, 84.9 km ± 0.5
on an Earth at level 8, and 90.3, 92.6 and 50.0 km on a Mars, the Moon and Ceres ± 0.5;
`cell_index` is a bijection onto 0..6 · 4ᴸ; `craters_reaching` yields exactly the craters whose
`reach` holds the cell, in list order; every synthetic world is built twice with identical values.
Acceptance: `cargo test -p hyperion-surface field`.

### R09.T3 The payload codec

`wire.rs`: little-endian blocks, each with a header (magic, `SURFACE_PAYLOAD_FORMAT`, generator
version, body, level, block index and count), block 0 also carrying the whole `FieldHeader`, the
cover of the block's surveyed and margin cells with their resolution codes, their records, and the
craters that reach them; `encode_payload` splits at `MAX_BLOCK_BYTES` in `cell_index` order, taking
the surface crate's `Cover` (the server converts its `Coverage`); `decode_payload` returns errors,
never panics; `DecodedBlock`, and `PartialField` with `insert` and its `FieldView`, which rebuilds
the per-cell crater index as blocks arrive.

Tests: round trip of a synthetic field to identical bytes and identical `FieldView` answers; a
`PartialField` built from block 0 alone carries the field's header, delta payloads included; a
`PartialField` answers `None` outside what it holds; the blocks inserted forwards, backwards and in
three fixed permutations give `PartialField`s whose every `FieldView` answer is equal; a truncated,
oversize,
wrong-version or wrong-body block is refused with its error; no block exceeds 1 MiB; the Earth-sized
synthetic field encodes to 10–15 MB. Acceptance: `cargo test -p hyperion-surface wire`.

### R09.T4 Interpolation and base elevation

`synth/interp.rs`: Design note 13's per-face B-spline over ghost cells, its partition of unity and
its analytic gradient; categorical reads; `Synthesiser::height_at` returning base elevation at every
level, `QueryHeightError::NotSurveyed` when a read cell is not held. `HeightSample` and
`LatticeCache` move from R05's `test_planet` to `height.rs`, which `test_planet` re-exports, so that
R10's retirement of the test planet leaves them in place.

Tests: along every face edge and at all eight corners, values and gradients evaluated from either
side's patches are equal bit for bit; the gradient matches a central difference to 10⁻⁶ relative;
the field is C¹ (not C²) across face centre lines, as the warp allows; a constant field interpolates
to itself and no value leaves the cells' range; the interpolant's instrumented read set never
reaches beyond `SYNTHESIS_MARGIN_CELLS` from the query's cell (the whole function's is T8's).
Acceptance: `cargo test -p hyperion-surface synth::interp`.

### R09.T5 Structural octaves

`synth/relief.rs`: the octave stack on `surface.relief` keyed by cell, over R05's noise basis with
its 16-entry gradient table and certified bound, conditioned on crust and boundary (ridged with its
pinned mean removed, low-amplitude, domain-warped from coarser octaves only); `BandSpectrum`;
`local_variance`; the per-contribution `unresolved_variance`.

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
  µs a point in wasm (Design note 14).

Acceptance: `cargo test -p hyperion-surface synth::channels`.

### R09.T7 Craters

- **R09.T7.a The density.** `craters.rs`: `CraterParams`; `cumulative_density` from Neukum et al.'s
  a1…a11 with a0 = log₁₀ N(>1 km) (the misprint recorded in the doc comment), the end slopes outside
  10 m–300 km, the optional diameter map, the screening taper and Venus's break-up rule;
  `saturation`; `transition_diameter`; `diameter_in_octave` by bisection in log D with a fixed
  iteration count. Tests: N(>1 km) is the parameter exactly; the function is monotone; the
  inversion lands in its octave and its quantiles agree with the density (Kolmogorov–Smirnov over
  10⁵ draws); the projectile scale d\* is 5.2 m, 0.52 km and 8.2 cm for Earth, Venus and Mars, and
  the crater cutoffs 20 d\*; the map at Mars's ratios reproduces Ivanov 2001's Mars function within
  ×1.5 in N over 1–100 km; D_t gives Pike's Moon and Earth within 10%.
- **R09.T7.b Small craters.** `synth/craters.rs`: octave levels, the 3 × 3 search across face edges,
  counts from the header's density at the canonical point by true area capped at saturation with
  ages from τ(D) (Design note 12), morphology by
  the transition zone, the volume-balanced profile with a = 1.54, its fillet and compensation;
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
`cargo test -p hyperion-surface synth` and `just bench -- surface`.

### R09.T9 Golden heights across targets

`crates/hyperion-surface/tests/height_golden.rs`, goldens in
`crates/hyperion-surface/tests/golden/`: heights and gradients at 512 pinned points and levels of
each synthetic world, and one baked patch per world, written with `GoldenWriter`, blessed natively
and compared on `wasm32-wasip1` and on `wasm32-unknown-unknown` through R04's embedded arm. Bump
`GENERATOR_VERSION` here if T16 has not already (whichever of T9 and T16 lands first bumps). The
`surface/point` bench is also run under the Electron `node` shim and recorded. Acceptance: `just ci`
(through `just test-wasm-fast`) runs the goldens on all three targets and passes; a change to any
synthesis constant fails it.

### R09.T10 The coarse pass's frame

`planetary/surface/{mod,inputs,grid,quantise}.rs`: `CoarseInputs` and its builder, `for_body`
returning `NotModelled` until plan 14's sections carry values, the cell graph over R05's cube (four
edge neighbours, vertex neighbours, solid angles by the closed form atan2(uv, √(1 + u² + v²))
differenced over the corners, through `math::atan2`), the quantiser, `coarse_pass` running its steps
as no-ops, and `reference::{earth_like, mars_like, moon_like, ceres_like}`.

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

- **R09.T12.a Crust and ridges.** `steps/relief.rs`: Design note 7's two crust populations about
  sea level and GDH1's age–depth law from the distance to divergent boundaries. Tests: the
  reference Earth's hypsometry, binned as Earth2014 was (area-weighted 250 m bins, the TBI layer at
  5′), is bimodal with a land mode within 300 m of +125 m, an ocean mode within 600 m of −4,375 m
  (the ocean peak is flat from −5,300 to −4,300 m) and an oceanic mean within 300 m of −4,281 m
  (Hirt and Rexer 2015's Earth2014, recomputed 2026-09-29); a ridge's depth
  follows GDH1 at 5, 20 and 100 Myr to 1 m.
- **R09.T12.b Convergent landforms.** Belts, trenches and arcs by boundary kind, scaled as Design
  note 7 says. Tests: belts lie within their stated distance of convergent boundaries; trenches lie
  2–4 km below the neighbouring sea floor and arcs 100–200 km behind them.
- **R09.T12.c Stagnant-lid provinces and flexure.** Volcanic provinces sized by the volcanism level
  and the flexural moat and bulge about each load. Tests: a line load's bulge crest lies at πα with
  height 0.043 w₀; α at T_e = 70 km under Mars's gravity is 180 km ± 10.
- **R09.T12.d Coarse craters.** `steps/craters.rs`, Design notes 5 and 10, after T7.a. Tests:
  counts over the reference Moon match min(production, saturation) above D_b (Poisson interval),
  about 300 above 100 km with N(1 km) at 4.4 Gyr; ages split before and after the wet epoch on the
  reference Mars; the list is sorted and each crater's `reach` is complete.
- **R09.T12.e σ_h and sea level.** Design note 7, on the reconstructed field, after T4 and T5.
  Tests: the reconstructed σ_h equals the input to 10⁻⁶ relative after the step;
  `local_variance ÷ σ_h²` is 5–9% on the reference Moon and 0.4–1% on the reference Earth; the
  realised relief lies at 6–16 σ_h on every reference world (Design note 7's observed 7–15 widened
  by one σ_h each way, since one realisation scatters about it); the ocean fraction below sea level
  equals the input to one cell's solid angle; the header reports realised σ_h and relief.

Acceptance: `cargo test -p hyperion-sim planetary::surface::steps::relief` (T12.a–c and e) and
`cargo test -p hyperion-sim planetary::surface::steps::craters` (T12.d).

### R09.T13 Climate

- **R09.T13.a The zonal model.** `steps/climate/ebm.rs`: Design note 8's implicit seasonal moist
  energy-balance model in latitude, with its heat capacities, gray outgoing radiation, albedo and D
  scaling, on the grid Design note 8 names. Tests: an Earth-like input converges within the fixed
  orbit count and its annual-mean zonal temperature lies within 5 K of Siler et al.'s Fig. 2b
  (ERA-Interim); the scheme is stable at Venus's rotation and at a six-hour step for every rotation
  in the reference set.
- **R09.T13.b Longitude and locked coordinates.** The periodic longitude solve split after the
  latitude one in a fixed order, and the tidally locked coordinates about P14.T14's substellar
  axis. Tests: with no land–sea contrast the zonal model's answer is reproduced to 0.01 K; a locked
  world is warmest at the substellar point.
- **R09.T13.c The other regimes.** `steps/climate/{airless,isothermal}.rs`: radiative equilibrium
  with thermal inertia for airless and thin-atmosphere worlds, and the isothermal surface for a
  Venus. Tests: an airless world's day–night contrast exceeds 300 K; the isothermal model's surface
  varies by under 1 K.
- **R09.T13.d Normalisation and ice.** Plan 14's mean and signed contrasts imposed on their
  components (P₂, P₁, a constant after the lapse term); ice coldest first by annual maximum. Tests:
  the area-weighted surface mean equals the input to 0.1 K; the P₂ coefficient gives the input
  contrast exactly; ice area equals the input fraction to one cell; a warm pole gets no cap; a
  90°-obliquity world is labelled by FILLET's ice-edge states (an ice belt, not caps, when plan 14's
  history says it started cold).
- **R09.T13.e Precipitation and wind.** Design note 8's heuristic, labelled in the header. Tests:
  global precipitation equals global evaporation to 1%; the reference Earth's zonal precipitation
  has maxima within 10° of the energy-flux equator and in the 40–60° bands and minima at 15–35°; on
  a one-dimensional ridge under a steady wind the lee receives under half the windward rate; inland
  precipitation decays from the sea with an e-folding within 2× of L_l; the header says heuristic.

Offline check (recorded, not in CI): the reference Earth's monthly fields against an ExoPlaSim run
of the same inputs, with the differences written into this plan. Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::climate`.

### R09.T14 Erosion

- **R09.T14.a Drainage.** Random receivers on `surface.coarse.erosion` with the slope correction;
  the depth-first order; drainage area by solid angle; priority flood with pop-order flats and
  Fill–Spill–Merge. Tests: drainage area at every outlet sums to the draining area; no cell drains
  uphill after filling; the same seed gives the same receivers; a finite inventory fills the lowest
  basins first and spills.
- **R09.T14.b The solver.** Tzathas et al.'s recursion with n = 1, the fixed point with its moving
  average, multigrid to level 4 at six iterations a level with the upsample through T4's
  interpolant; K scaled as Design note 9 says with m = 0.45 and the runoff-weighted area. Tests: a
  ridge-to-sea profile matches the closed-form steady state; the fixed point's residual falls
  monotonically over the multigrid levels on the reference Earth.
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
1.79 s at 512² and 8.18 s at 1,024² in Python with numba), recorded. Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::erosion`.

### R09.T15 Classes and crater state

`steps/classes.rs`: Köppen–Geiger by Peel et al.'s Table 1 with Beck et al.'s rules, applied to
rates with each cell's own summer (Design note 11); surface-state classes for the other regimes and
for one-month worlds; the per-cell crater state. Tests: the classifier given one monthly climatology
labelled once as a one-year orbit and once as a four-year orbit (the same temperatures, and
precipitation at the same rates per 30.44 d) returns the same classes, which tests the rate rule
alone, not the climate a longer orbit would have; the reference Earth has tropical, arid, temperate,
continental and polar classes in plausible areas; a lifeless world has no vegetation class; the
reference Moon is regolith throughout. `crates/hyperion-sim/examples/surface_map.rs` writes the
reference Earth's elevation, class and flow as equirectangular PPM images under `target/`, with no
new dependency, and the look (belts along convergent boundaries, rivers to the sea) is recorded in
this task's entry. Acceptance: `cargo test -p hyperion-sim planetary::surface::steps::classes` and
`cargo run -p hyperion-sim --features testing --example surface_map` writing its three images.

### R09.T16 Coarse goldens and benches

`crates/hyperion-sim/tests/surface_coarse_golden.rs`, over a shared
`crates/hyperion-sim/tests/common/surface_worlds.rs` that runs `reference::*` through the pass once
per binary and that R10's world tests reuse: the quantised fields of the reference Ceres
and Moon (fast, so they run on native and wasip1 under R04's recipes) and of the reference Earth
and Mars (slow). Bench `coarse/pass` per level 5–8. Bump `GENERATOR_VERSION` if T9 has not
already (whichever of T9 and T16 lands first bumps), or again if the first bump has been released
to a save. Acceptance:
`just ci`, `just test-slow`, `just bench -- coarse`.

### R09.T17 The surface service

`crates/hyperion-server/src/surface/{mod,cache,service,inputs}.rs` and `config.rs`: Design note 19;
the `InputsSource` seam with `RecordInputs` for production and, in
`crates/hyperion-server/tests/common/surface.rs`, `reference_inputs` over
`hyperion_sim::planetary::surface::reference::*` for tests; the job runs on R04's probed pool, and
`JobError::FloatingPointMode` refuses the field with `internal` and logs it.

Tests: two concurrent requests compute once (a counter); the cache is bounded and evicts; a
cancelled job leaves nothing cached; the server's field, from `reference_inputs`, equals
`coarse_pass` on the same inputs bit for bit; with `RecordInputs` a generated body answers
`NotModelled` naming its section. Acceptance: `cargo test -p hyperion-server surface`.

### R09.T18 Survey passes and coverage

`crates/hyperion-server/src/knowledge/surveys.rs`, on P12.T7's store: `SurveyPass`, `SurveyLog`
(append on record, load on open, unknown version refused, a torn last line dropped with a warning),
`Coverage` folded on load.

Tests: round trip; reopening a universe restores the same coverage bytes; the finest resolution
wins; folding is independent of pass order. Acceptance: `cargo test -p hyperion-server knowledge`.

### R09.T19 The wire and the gate

After T8, T17 and T18.

- **R09.T19.a The kinds.** `crates/hyperion-protocol/src/surface.rs` and `just gen-protocol`: the
  two kinds under plan 04's rules (`kind`, `is_large` true for `surface_field`, the `every_body`
  walk), `SurfaceFieldDto` with its `NotModelled` arm, `CellRangeDto`, `CoverDto` and
  `MAX_SURVEY_RANGES` in `limits.rs`; the `survey_pass` handler recording through
  `SurveyLog::record`. An unknown body is `unknown_body` and one with no solid surface
  `bad_request`, each naming `body`; more than `MAX_SURVEY_RANGES` ranges is `bad_request` naming
  `cells`. Tests: wire forms of every DTO; a `survey_pass` of 256 ranges fits
  `MAX_INBOUND_FRAME_BYTES` and one of 257 is refused; a body with no solid surface is refused.
  Acceptance: `cargo test -p hyperion-protocol surface` and `just gen-protocol-check`.
- **R09.T19.b The payload, the gate and the revisions.** The `surface_field` handler: the payload
  through R03's `Answer`, gated by coverage with the margin and the craters, the revision and the
  delta since `have_revision`, and `NotModelled` for a body whose inputs are not modelled; the
  scene's `surface_revisions` (with R03). Tests: a second pass's delta carries only new cells; the
  same request twice gives identical bytes; a `survey_pass` bumps the body's revision on the scene
  topic. Acceptance: `cargo test -p hyperion-server surface`.
- **R09.T19.c The Knowledge-bound test.** Over a real socket, extending R03's, on
  `reference_inputs`: after one orbital pass over a region, no block carries a cell outside the
  region and its margin, and margin cells are marked; the cover's resolution codes match the
  passes; the client's decoded `PartialField` gives the server's own heights at every surveyed
  point, the region's edge included; no response or notification carries a key named
  `surface_seed`. Acceptance: `just ci`.

### R09.T20 Verification and hand-over

Run every slow test and bench on a quiet machine, record figures in the doc comments that own them,
mark R05's provisional function deprecated with a pointer to `Synthesiser` for R10, and record the
per-point cost in wasm against the budget and against R05's T3.c figure for the test planet.
Acceptance: `just ci`, `just ci-slow`, `just bench` complete.

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
question 4), so a change to any of them bumps `GENERATOR_VERSION`. The first bump is made by
whichever of T9 and T16 lands first; later tasks that change a committed surface golden bump
again. R05's `FINEST_SPACING_M`, `BAND_LIMIT_M`
and `finest_level` join the version when T9 reads them, as R05 notes. Nothing upstream moves: every
draw is on a new tag, and plan 14's `body.surface` seed is unchanged. Reserved here: the two scopes
and the cell key's packing; the tags of Provides and `surface.scatter` for R11; the `instance` field
for R11's shape draws; `SURFACE_PAYLOAD_FORMAT` 1; `surveys.v1.jsonl`; the level rule's 40 km, 5 and
8; `ClimateCell` at level L − 1; the header's `albedo_scale` for R10 and its `surface_age` and
`surface_pressure` for R11. R10.T10.a fills `FieldHeader.albedo_scale` and bumps
`GENERATOR_VERSION`, re-blessing this plan's payload and coarse goldens in that commit.

## Risks and open points

- **Low-confidence constants.** The σ_h fit rests on one body per constant (0.9 km, 0.16, the √N
  age law, the 70 km lithosphere normalisation); stream-power erodibility across fluids and
  gravities; the energy-balance transport cap; Venus's crater screening; the complex-crater floor
  and peak dimensions. Each is a named constant with its source and confidence in its doc comment,
  and T0.b's remaining checks cover the first four.
- **Several tasks are near a day.** T8 (assembly, the bound, the patch and the read set) and T6.b
  are the largest left unsplit; if either runs over, T8's read-set test and T6.b's bench split
  off as their own subtasks.
- **Timings are provisional.** Every cost in Design note 14 was measured under other agents' load
  and scaled by a guess. If the quiet measurement of T6.b and T8 exceeds the budget, the levers are
  the channel search and level cut, then the hash fallbacks; the band limit does not move. The
  coarse pass at level 8 may take tens of seconds with the climate model, which the brainstorm
  accepts if it starts on approach; until sessions exist it starts on first request.
- **Plan 14's asks may not land in time.** The pass runs on builder inputs meanwhile, and `for_body`
  says `NotModelled`; no generated world gets a surface until plan 14 carries σ_h with f_c, the
  volatile history, the crater parameters, the regime classifier and the signed contrasts. The ice
  belt needs plan 14's cold- or warm-start history, which it has no classifier to hold yet. The
  server's tests run on the reference worlds through `InputsSource` until then.
- **Physics leans still to rule** (research of 2026-09-29, low to medium confidence):
  - _The transport cap._ No published cap on the energy-balance model's D exists; the lean is to
    cap the rotation factor at (Ω⊕ ÷ Ω)² ≤ 64, since Hadley cells are near-global by Ω⊕ ÷ 8
    (Kaspi and Showman 2015), then apply the pressure, heat-capacity and molar-mass factors under
    an overall cap of about 100 D⊕ for stiffness. The provisional 30 D⊕ saturates at Ω⊕ ÷ 5.5.
  - _The isotherm of T_e._ 870 K is a mechanical-thickness proxy; the elastic 723 K (450 °C) would
    need the 70 km normalisation re-fitted against Mars.
  - _Small-crater density per cell._ Design note 13 reads the body's density, which keeps the
    margin at five cells but gives a Mars no dichotomy in its small craters. A per-cell density
    read at an octave cell's canonical point would reach 6–8 coarse cells, beyond the margin. Lean:
    keep the body's density; if regional ages are wanted, modulate by the crater's own centre cell
    only for octaves whose search stays inside the margin.
- **The survey stand-in.** `survey_pass` lets a client grant its ship coverage, as the server
  grants detail levels today. It is a discipline for an honest client, and the sensors plan
  replaces the caller, not the core.
- **Field size.** Design note 17's layout gives about 12 MB for an Earth; if the climate layer at
  L − 1 proves too coarse for R11's clouds, sending it at level L costs about 40% more.
- **Dependencies.** Plan 14 calling the surface crate's crater density is a new edge that R04's
  split allows. P12.T7 is unbuilt, and T18 waits on it. The channel network's physics profile
  replaces Dendry's own height reconstruction, which the paper did not test.
- **Asks of other plans, which their owners may not yet carry**, each with its state in the
  roadmap's asks table:
  - plan 14's five of Design note 3: drafted by T0.a;
  - plan 14's P14.T23 surface seed and hooks section, on which T1.b waits: carried by plan 14,
    unbuilt;
  - plan 04's two table rows: drafted by T0.a;
  - R03's `surface_revisions` field: carried (R03 leaves room); the note that the camera reports
    do not bound the field: drafted by T0.a;
  - R10's use of the header's `albedo_scale`: carried by R10;
  - R11's use of the header's `surface_age` and `surface_pressure` for its `RockSite`: carried by R11
    (`rock_site`).

  R10's three asks of this plan (per-contribution variance and structure function, resolution per
  cell, the albedo-scale field) are met in Provides.

- **Brainstorm departures for its revision.** Design note 13's channel network departs from the
  brainstorm's per-query evaluation in three ways (the first level from `flow` not the lowest
  Moore neighbour, a 3 × 3 search not 5 × 5, one extended network not stacked instances), and
  Design note 15's crater reach is 1.27 D, not "about twice its radius"; none is yet in the
  roadmap's brainstorm corrections.
- **Asked by R11, not yet designed here** (the roadmap's asks table carries it). The height datum is
  settled as the rotational spheroid (Design note 17, after R07's Design note 19), with the sea
  level a height above it, so R10's datum ask is met. R11 asks for the terrain-independent zonal
  part of the precipitation heuristic (the rain band on the energy-flux equator, the dry belts and
  the storm tracks, by latitude and month) as a function the client can call from plan 14's global
  figures without the coarse field, so that clouds are drawn over unsurveyed ground (R11 Design note
  8), and whether `ClimateCell.wind`'s four entries are seasonal, since its cloud advection and sea
  state want the month's.
- **R03.T15's 15 MiB (61-chunk) transfer check is this plan's, after RM3** (decided 2026-10-07 by
  the orchestrator). The coarse field, about 15 MiB, is the first bulk kind of that size: no sky
  reaches it (R06's largest is 7.5 MB, 29 chunks; R06.T11.b ran the check at 4 chunks, recorded in
  R06's Risks, "Deviations in T11.b, as built", and R03's T15 note).
