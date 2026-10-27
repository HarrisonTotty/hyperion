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
        epoch_incision: LogIncision (R09.T6.c; 0 no epoch, 1 uncut, else 0.5 m × 2^((c − 2) ÷ 16)),
        water_surface_mm: i32, ice: u8, substances: u8 (ice and liquid entries),
        class: SurfaceClass, crater_state: u8, seafloor: u8 (hill relief | ponded sediment,
        decision-r09-t5 item 5) */ }                    // 26 bytes in the payload
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
    // `profile`: in `synth::craters` as built (R09.T7.b), taking the crater's wear (R09.T7.j).
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
    pub mod craters {                    // R09.T7.b's small craters; their wear by R09.T7.j
        pub const DEGRADATION_B: f64 = 0.15;               // Minton et al. 2019, eq. 8
        pub const VISIBILITY_COEFFICIENT: f64 = 0.17;      // K_v: their eq. 7 and Fig. 5
        pub struct CraterWear;           // its diffusion length and share q: FRESH, new,
                                         // of_age(age, &OctaveWear), diffusion, share
        pub struct OctaveWear;           // an octave's window and wear rate: new(surface_age,
            // system_age, production, saturation), window, window_density,
            // mean_square_relief(rim_radius), E[g²]
        pub fn relief_factor(rim_radius: Metres, wear: CraterWear) -> f64;  // max(κt, K_v q)
        pub fn profile(r_over_radius: f64, crater: &CraterShape, band: Metres,
            wear: CraterWear) -> ProfileSample;            // band as its half-width
        impl CraterProfile {
            pub fn new(shape: CraterShape, band: Metres, wear: CraterWear) -> Self;
        }
        impl CraterOctave { pub const fn wear(&self) -> &OctaveWear; }
        impl SmallCrater { pub const fn wear(&self) -> CraterWear; }
    }
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
binary (under nextest, within one test: R09.T16's note); T16 creates that file with its own goldens
so that R10 extends it.

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
  (64 MiB), above this plan's 15 MiB. R03's camera reports are not used to bound the field (Design
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
     not described by a silicate isotherm: a water-ice lid's T_e is (567 W m⁻¹ ÷ F)
     ln(170 K ÷ T_s), from the `H2O` row's conductivity (Risks). T_e never exceeds the body's
     radius (`decision-r09-t12-coarse.md` item 4). _The continental fraction_ (medium-low
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
   lid gets no boundaries: volcanic provinces sized by the volcanism level, with the flexure about
   each load, its moat filled and its bulge planed to the surface's level
   (`decision-r09-t12-coarse.md` addenda 2 and 3).
7. **The variance budget and the landforms** (researched 2026-09-29; Earth2014 split at −2 km; GDH1,
   Stein and Stein 1992; flexure after Turcotte and Schubert 2014 §3.16–3.17, from memory). Crust is
   two populations about sea level, continental and oceanic, as Earth2014 shows (continental mean
   +405 m, s.d. 1.08 km; oceanic −4,281 m, s.d. 0.93 km; the TBI layer at 5′, recomputed
   2026-10-16), and on a stagnant lid one, plan 14's 0.9 km, a Gaussian field whose degree variance
   falls as l^−β₁ from degree 2; ridges follow GDH1's age–depth law, d = 2,600 + 365 √t m below
   20 Myr and 5,651 − 2,473 e^(−0.0278 t) m above, with the age t the floor's steady-state age: the
   kinematic age d_r ÷ u_h, from the distance to the plate's own divergent boundary, a second
   distance transform the pass keeps in its `f64` working state (not the nearest boundary of any
   kind, which would make old floor by a trench young; researched 2026-09-29: Stein and Stein 1992;
   Müller et al. 2008's age grid), ranked by area onto Parsons 1982's triangular area–age
   distribution, whose oldest age t_m is 192.9 Myr (Earth's mean of 64.3 Myr) times the world's
   oceanic share over Earth's 0.595 and Earth's plate speed over the world's (R09.T11's speed law),
   at most the system's age; GDH1 is the basement unloaded of sediment, and the floor stands above
   it by the sediment's isostatic load S (ρ_m − ρ̄_s) ÷ (ρ_m − ρ_w), Athy's compaction with Sclater
   and Christie 1980's shale (`decision-r09-t12-coarse.md` item 1); the floor's roughness follows
   its own ridge too: the half-rate at formation sets the abyssal-hill relief (Goff et al. 2004),
   its age the pelagic drape (GlobSed, Goff 2010), and the turbidites of its plate's passive
   margins, trapped by trenches, its plains (Harris et al. 2014's third of the abyss), carried to
   the synthesis in one byte (`decision-r09-t5.md` item 5); collisional belts 200–500 km wide with
   plateaus near 5 km and peaks near 9 km, trenches 2–4 km below the sea floor and volcanic fronts
   190 km behind a trench whose overriding plate is oceanic and 260 km behind one whose overriding
   plate is continental (Syracuse and Abers 2006's 527 front volcanoes, Table S2a, classified by
   overriding crust by `decision-r09-t5.md` item 6's split of its arc segments, not a split the
   paper prints; 10–90% 140–250 and 180–330 km; Bird 2003's "200–250 km is more typical" brackets
   the median), varied along strike by the interquartile range, arcs 60 km and 175 km wide behind
   them, distances Earth's and unscaled (labelled), all heights scaled by g⊕ ÷ g as
   strength-limited, g⊕ the reference Earth's GM ÷ R², so that the factor is 1 there
   (`decision-r09-t5.md` item 3; the lithosphere factor acts on the constructional share and the
   edifice cap only, Design note 3); flexure w = w₀ e^(−x/α)(cos x/α + sin x/α) with
   α = [4D ÷ ((ρ_m − ρ_fill) g + E T_e ÷ R²)]^¼, the last term a spherical shell's membrane support
   (Brotchie and Silvester 1969), ρ_m the density beneath the lithosphere (3,300 kg m⁻³ under a
   silicate lid, Turcotte and Schubert's; the lid's own ice under a water-ice lid), ρ_fill = ρ_p,
   the province's bulk density, its palette entry's grain density times 0.8 (porosity 0.2, from
   Mars's crust, Goossens et al. 2017), the load's and the moat fill's alike, so that the uniform
   foundation is exact; each load's height is its height above the filled level, and the surface is
   raised by h alone: the moat and the bulge both stand at the filled level, the bulge planed, which
   is the uniform foundation's exact surface (`decision-r09-t12-coarse.md` addendum 3),
   D = E T_e³ ÷ 12(1 − ν²), E 100 GPa, ν 0.25, T_e at most the body's radius; an open moat at
   T_e = 70 km under Mars's gravity gives α = 176 km, and the reference Mars's filled moat at its
   107 km gives 337 km (`decision-r09-t12-coarse.md` items 3 and 4, and addendum 2).
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
   about its area-weighted mean, the coarse craters' share through their kept relief while the scale
   is at most 1 and the non-crater field alone above it, so that the **reconstructed** field's
   variance, the interpolant of Design note 13 integrated by solid-angle quadrature at sub-cell
   points, is σ_h² less the local shares: the realised structural share (above), and the whole share
   the craters and channels add below the cells: the small craters' whole, each coarse crater's
   increase of its mean square from its coarse profile to its band limit's, and the channels'
   (`decision-r09-t5.md` item 4, amended by `decision-r09-t12-coarse.md` addendum 4); and again
   after erosion, when the channels' share is known; the cell values alone would fall several per
   cent short by the spline's smoothing. The local share is not negligible at the plan's own levels:
   above the coarse cell's Nyquist degree it is 0.6% of σ_h² for an Earth or a Mars (about 200 m
   RMS), 4% for Mercury, 5% for Ceres and 7% for the Moon (about 600 m), all contributions together,
   against the brainstorm's 0.1–0.2% and 2%, which hold at a 35 km wavelength. Sea level is the
   level of the ocean, grown by a priority flood from the largest connected region below the
   area-weighted quantile that leaves plan 14's ocean fraction below it until it covers that
   fraction; enclosed depressions below it stay dry until R09.T14 places lakes. The realised σ_h and
   greatest relief go in the header.
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
   confidence; Okuya et al. 2019 for a two-dimensional precedent). The model reads the relief, its
   prognostic temperature staying the sea-level one: each grid point's surface heights, as
   equal-area quantiles, set its albedo and ice heat capacity at the ground's temperature, T − Γz,
   and its outgoing radiation is the sea-level column's less the change at the raised ground,
   transmitted through the column above it (R09.T13.g, decision-r09-t13a-relief). The fields are
   interpolated to the cells bicubically before the lapse term, which re-applies the same Γ at each
   cell's own height. A month is a twelfth of the seasonal orbit in eccentric anomaly, counted from
   periapsis.
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

   Plan 14's figures are imposed on the field as plan 14 defines them
   (decision-r09-t13d-normalisation). The equator–pole contrast is the annual field's T(0°) less
   its poles' mean, by a correction shaped like P₂(sin φ). On a field shaped like P₂ that sets the
   coefficient to −⅔ of the contrast, since T(0°) − T(90°) is −(3/2) T₂ (the sign carries a warm
   pole; T10's science review, 2026-10-09). A locked world's day–night contrast is the μ-weighted
   means of its day and night, by P₁(cos γ), whose coefficient is then ¾ of it. The mean is a
   constant: added last to the surface temperature after the lapse term on the energy-balance and
   isothermal models, and solved with the two contrasts on the radiative model. The radiative
   model's figures are brightness temperatures of its emitted flux, less the night floor's for the
   mean and the equator–pole contrast, as plan 14's are; the other models' are arithmetic. Ice goes
   per condensate, most volatile first, on the cells coldest by their annual radiative mean,
   ⟨T⁴⟩^¼, until its area matches plan 14's fraction. That also places a locked nightside and an
   equatorial belt. Precipitation stays labelled `PrecipitationSource::Heuristic`:
   Siler et al.'s Hadley-cell partition gives E − P with the rain band on the energy-flux equator
   (our extension centres it there each month, capped at the ascending branch's latitude after
   Guendelman and Kaspi 2019, ApJ 881, 67, doi:10.3847/1538-4357/ab2a06 (arXiv:1906.05748), §4.4,
   Eq. 20, decision-r09-t13ge) and the dry belts and storm tracks unplaced by hand;
   its width scales with rotation after Held 2000 (from memory); evaporation over open liquid is
   Priestley–Taylor with α = 1.26 (from memory); onto the land, Hergarten and Robl's linear
   feedback precipitation model carries the sea's air along the model's bands, by the month's
   prevailing wind's component along them, for orography, rain shadows and inland decay, each land
   cell taking the zonal budget's convergence as a vapour source as the sea does, scaled by
   e^(−H/H₀) over high ground, and recycling nothing from frozen ground
   (decision-r09-t13e-revision; ε₀ = 0.75, L_c = L_f = L_d = 25 km, L_l = 500 km, H₀ = 2 km, their set
   for the Himalaya and Tibet, Hergarten and Robl 2022, GMD 15, 2063, §7), solved in each month's
   upwind order of strongly connected components (Tarjan 1972), each component of up to 16 cells
   exactly, by Gauss–Seidel sweeps accelerated by restarted GMRES (Saad and Schultz 1986) until one
   further sweep moves no land cell's rate by more than 10⁻⁴ of it
   (decision-r09-t13e-lfpm-convergence); the sea rains its own evaporation plus the band's
   convergence, a divergent band's export falling on its sea alone; and the whole is rescaled so
   that global precipitation equals global evaporation. Prevailing winds are the
   three-cell pattern scaled by the Hadley width, centred each month on the month's energy-flux
   equator, and converging on the substellar point for slow and locked rotators.
   - Each month's orographic step carries vapour along its recorded wind's component along the
     model's bands; the zonal budget carries the meridional transport, Hadley and eddy, which the
     wind's meridional part would count again over land (decision-r09-t13e-revision). The record
     stores the month's 10 m wind: the direction of its resultant and its mean speed. These
     coincide while the pattern has no transients.
   - The field's rain shadows and its winds' components along the bands therefore agree month by
     month.

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
   (recorded; the paper gives none); the fixed point does not converge exactly, its receivers
   flipping on a floor that grows with the level, and the field takes the last solve on its own
   routing (decision-r09-t14-choices, T14.b follow-up); on a world wet now the uplift is a field
   fixed in space, each cell's own basin's λ in the path integral, refitted each iteration against
   the construction basin's mean elevation by an additive step, and no link of the steady state
   rises more than `DEBRIS_FLOW_SLOPE` times its length (Tzathas et al.'s §6.2 threshold in its hard
   limit, at Stock and Dietrich 2003's debris-flow slope; T14.c follow-ups 1 and 2), its receivers
   sticky from each level's third iteration, a cell keeping its previous receiver while that stays
   strictly lower on the filled surface (T14.c follow-up 2); multigrid runs on the quadtree's own
   parents (solid-angle-weighted means) down to level 4, six iterations a level, the upsample of
   each level's change from its construction through the synthesis's interpolant at child centres
   jittered on the same tag, added to the finer level's construction (multigrid's full
   approximation scheme; T14.b follow-up). The base level is the sea, or on a world dry now the
   terminal outlets left by priority flood (flat directions taken from the pop order, so no
   ε-raising is needed) and Fill–Spill–Merge, which spreads the paleo-inventory through the
   depression hierarchy, a uniform depth of the paleo-inventory by the epoch's liquid's density
   routed by Fill–Spill–Merge, the sphere's root filled to the level its volume reaches (plan 14's
   own ocean a volume fill since P14.T24.b; decision-r09-t14-choices Q2). On a world wet now the
   depressions hold lakes by their water balance, the catchment's runoff against the lake's
   evaporation less its precipitation (Q3). A world wet now runs to steady state, its rock uplift
   fixed in space at u = λ_b max(0, z0 − ℓ_b), z0 the elevation entering the step, b the cell's
   basin on the construction's own routing, ℓ_b its base level (the sea, or a closed basin's lake
   or dry floor), and λ_b fitted in every iteration so that each such basin keeps its
   construction's mean height above ℓ_b (Pinet and Souriau 1988's denudation in proportion to a
   basin's mean elevation, its time constant basin by basin; decision-r09-t14-choices Q1; a
   labelled closure, medium-low confidence), its heights then remapped in order onto its
   construction's absolute hypsometry, each cell no lower than its receiver and no link steeper
   than the threshold (T14.c follow-up 3); one dry now runs for t = `effective_flow`, with the
   drainage area weighted by the epoch's `runoff`, uniform over the cells since no past climate is
   modelled, only where the surface is older than the epoch's end; each cell's surface by its own
   age, a volcanic province's being plan 14's `volcanic_surface_age`, the time since its heat flow
   last stood at or above F_c, renewed to its construction after the solve where it is no older
   than the epoch's end; the coarse cell lowered by its valleys' void, d (w_bf + 5d) ÷ the cell's
   width at most d, the valley's floor its bankfull channel and its walls of mean gradient 0.2
   (width about ten times depth: Luo, Cang and Howard 2017 after Williams and Phillips 2001), the
   channel solved at full K (decision-r09-t14-mars-ceiling); one with no wet epoch skips the step.
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
    age through T7.b's relief factor, worn to equilibrium (R09.T7.j), one law with the small craters
    (`decision-r09-t12-coarse.md` item 5b). They are smoothed into the elevation and sent as a list
    sorted by (`cell_index` of the centre's cell, diameter), each recording in its `reach` every
    cell its reach (2.54 rim radii, Design note 13) touches, so that a block carries every crater
    that reaches its cells. The field keeps a per-cell index of the craters reaching each cell,
    built from the `reach` lists, which is what `FieldView::craters_reaching` walks; the index is
    rebuilt on the client as blocks arrive and is not on the wire.
11. **Classes** (researched 2026-09-29: Peel, Finlayson and McMahon 2007, Table 1; Beck et al.
    2018). Köppen–Geiger for seasonal water-cycle regimes, by Peel et al.'s Table 1, which Beck et
    al. adopt (researched 2026-09-29, high confidence): C and D divide at a coldest month of 0 °C
    (Peel, after Russell 1931); B is tested first and takes precedence, with MAP < 10 P_th, where
    the 70% rule sets P_th = 2 MAT, 2 MAT + 28 or 2 MAT + 14 (mm, MAT in °C) for winter-,
    summer- or neither-dominant precipitation; within B, W is MAP < 5 P_th and S is MAP ≥ 5 P_th;
    h and k divide at MAT 18 °C; when both s and w hold, w if summer precipitation exceeds
    winter's, else s (a tie gives s); E at T_hot ≤ 10 °C (Beck et al. 2018, Table 2: Peel's
    T_hot < 10 leaves 10 °C in no group); C's c without Peel's 1 ≤ T_mon10 bound; the 70% rule
    strict,
    classifying climate and not life: there is no vegetation class, and a lifeless Earth-like world
    keeps its class with bare ground. Months are not Earth months, so the table is applied to
    **rates** (precipitation per 30.4375 d, 365.25 ÷ 12, and per 365.25 d); summer is each cell's
    warmest consecutive half-year (HYPERION's adaptation: Peel and Beck use the warmer of ONDJFM
    and AMJJAS), six months on a circular orbit, and every count of months in the table
    (such as four months above 10 °C) counts their durations in twelfths of the year, since an
    eccentric orbit's months differ in length (Design note 8); the temperature thresholds stay.
    The seasonal water-cycle regimes are the worlds whose model is `EnergyBalance`, with twelve
    months and water as the climate condensable. Worlds with one month, and every
    non-seasonal regime, classify surface state (liquid, ice or frost of a named species, rock,
    regolith, melt, organic sediment) with the regime's named zones, such as a locked world's
    substellar ocean and nightside glacier: the zones are the forms under the header's model, with
    no codes of their own until something reads one. One `SurfaceClass` byte holds either
    (decision-r09-t15-classes, 2026-10-21).
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
    band-limited by a cubic Hermite fillet over the level's band, the central peak fading with it as
    the fillet reaches across the peak's base, and a compensating c (1 − ((r − R) ÷ w)²)² on the
    fillet's span restores the volume it moved (R09.T7.i), so every crater integrates to zero at
    every level; the crater's age enters as its relief factor g (R09.T7.b), worn to equilibrium
    where its octave is saturated (R09.T7.j). Every contribution is attributable to a level; the set
    per level is fixed per body from the level's largest cell. The renderer's morph blends level n
    with the parent band carried in the patch, so the height function itself is discrete per level.

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
    the cell's elevation. That gives about 26 B a cell (21 B, T2's follow-up B's substance byte,
    `decision-r09-t5.md`'s sea-floor byte, follow-up C's two runoff bytes and R09.T6.c's
    epoch-incision byte) plus 50 B per four
    (twelve months of anomaly, precipitation and wind): some 15.1 MB for an Earth at level 8,
    3.8 MB for a Mars at 7 and 0.96 MB for the Moon, at the top of the brainstorm's "about 4 to
    15 MB" and inside R03's 15 MiB check, the binding wire budget (main's ruling for R09.T6.c).
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
multigrid's upsample) needs T4; and T14.c (the Mars volume check) needs T6.b's closed-form incision.
T6.c (valleys of a short wet epoch) runs the other way: it follows T14, whose T14.d keeps the
solver's channel incision, and lands before the serve gate. T17–T19 are the server: T17 needs T3 and
T10, T18 needs T2 (P12.T7 is built), and T19 needs T8, T17 and T18.
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
  Records: [09-surface-generator/records/R09.T0.b.md](09-surface-generator/records/R09.T0.b.md).

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
  Records: [09-surface-generator/records/R09.T1.a.md](09-surface-generator/records/R09.T1.a.md).
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
  Records: [09-surface-generator/records/R09.T1.b.md](09-surface-generator/records/R09.T1.b.md).

### R09.T2 The field's types

Records: [09-surface-generator/records/R09.T2.md](09-surface-generator/records/R09.T2.md).

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
  Records: [09-surface-generator/records/R09.T2.e.md](09-surface-generator/records/R09.T2.e.md).
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

- **R09.T2.f Follow-up F, the palette's FeO_T and the weathering rate**
  (decision-p14-t51d-weathering §1.6 and §2.3, 2026-10-16; lane r10-weathering). It is layout
  of the gate's set, before the first send, so a re-bless at the version in force.
  Records: [09-surface-generator/records/R09.T2.f.md](09-surface-generator/records/R09.T2.f.md).
  - `PaletteEntry.feo_mass_fraction: Option<f64>`, in [+0, 1], after `liquid`, with new
    `BuildPaletteError::{FeoMassFraction, FeoOnLiquid}`.
  - `FieldHeaderParts.weathering_rate: Option<f64>`, finite and ≥ +0, after `organic_cover`,
    present exactly when `maturity` is (`BuildFieldHeaderError::WeatheringParts`).
  - The codec's minimum entry becomes 61 bytes.
  - The DTO mirrors: `PaletteEntryDto.feo_mass_fraction` and `SurfaceHeaderDto.weathering_rate`,
    both `Modelled<f64>`.
  - _Accept:_ as T2.e's, plus `just gen-protocol-check`.

### R09.T3 The payload codec

Records: [09-surface-generator/records/R09.T3.md](09-surface-generator/records/R09.T3.md).

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

Records: [09-surface-generator/records/R09.T4.md](09-surface-generator/records/R09.T4.md).

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

Records: [09-surface-generator/records/R09.T5.md](09-surface-generator/records/R09.T5.md).

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
  Records: [09-surface-generator/records/R09.T6.a.md](09-surface-generator/records/R09.T6.a.md).
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
  Records: [09-surface-generator/records/R09.T6.b.md](09-surface-generator/records/R09.T6.b.md).
- **R09.T6.c Valleys of a short wet epoch** (after R09.T14, whose T14.d keeps the solver's channel
  incision; before the serve gate, and before R09.T16 if it can, so that T16's goldens pin it once,
  otherwise they re-bless at the version in force; decision-r09-t14-choices F2 with its T14.c and
  T14.d follow-ups, and decision-r09-t14-mars-ceiling §2 and §5, which bind). T6.b's valleys are
  the steady state's on every world, so a world dry now is dissected as a steady-state Earth: on
  the reference Mars up to about 2.3 times the 50–350 m of trunk incision (Howard, Moore and Irwin
  2005, as Black et al. 2017 quote it), about four times the networks' 10 km widths, and a void
  100–300 times Luo, Cang and Howard 2017's (F2). On a world dry now (worlds wet now keep T6.b's
  steady state as built):
  Records: [09-surface-generator/records/R09.T6.c.md](09-surface-generator/records/R09.T6.c.md).
  - each segment's valley depth G is the lesser of T6.b's steady-state depth (`valley_depth`) and
    the epoch's incision at the segment; a tributary's incision follows from its cell's at the
    same K t in the short-time regime, ∝ (r_t A_t)^m S_t ÷ (r̄ A)^m S;
  - its cross-section is mars-ceiling §5's: the floor its bankfull channel, half T6.b's
    `bankfull_width` either side, and each wall rising over W_w = 5 G as T6.b's 1 − smoothstep,
    so a top width of w_bf + 10 G, a void of G (w_bf + 5 G) per unit length and a steepest wall
    of 0.3, under `VALLEY_WALL_GRADIENT`'s 0.6 (width about ten times depth: Luo, Cang and Howard
    2017 after Williams and Phillips 2001, whose top width is linear in depth below 125 m and whose
    width-to-depth ratio falls as depth grows, which the floor term gives; medium-low confidence);
    the upland between valleys stays undissected, and no valley link is steeper than
    `DEBRIS_FLOW_SLOPE`;
  - segments whose incision falls below the level cut are not drawn, which lowers a short epoch's
    drainage density as Mars's immature networks show, and a cell carries a valley, and T14's
    closure, only where its drainage exceeds the epoch's channel-initiation area, so that the
    valleys' volume does not grow with the level (on the reference Mars at level 8, T14.c's (B)
    p99 is 1.94 times its bound and (A2) 5.64 m);
  - `has_channels` also needs a catchment runoff above `LogRunoff::NONE`, so that a headwater cell
    inside a late crater's rim, with no water of its own or from upstream, carries no trunk
    (decision-r09-t14-choices, T14.d follow-up C);
  - a surface the epoch did not see carries no epoch valleys: `channel_incision` is zero on the
    reference Mars's renewed provinces (mars-ceiling §2.1).
  - _The input._ T14.d writes `CellState::channel_incision`, an `f64`: the solver's unscaled
    channel incision on the dry-now branch, zero elsewhere, unquantised. T6.c wires it into the
    field with no change to T14's solver. Lean: a per-cell code of it, none on worlds wet now or
    never wet, +1 B a cell (0.39 MB on an Earth at level 8, against the 0.98 MB margin of Risks'
    "Field size", so R03's 15 MiB check is re-run first); the alternative is one header member,
    the epoch's K t (r ÷ 1 m a⁻¹)^m, which needs B by substrate on the client. T6.c's
    re-validation settles the encoding; either is a `FieldHeader` or payload-layout change, so it
    lands before the gate freezes them (Generator version). The owner is the synthesis:
    `hyperion-surface`'s `synth/channels/incision.rs` changes, a shared-terrain change with a
    WebAssembly determinism review.
  - _The closure._ T14.c's width closure, the lowering min(d, d (w_bf + 5d) ÷ w) on each eroded
    dry-now cell (Design note 9; `WALL_RUN_PER_DEPTH`), is edited in T6.c's commit to count
    exactly the valleys T6.c draws in the cell, its trunk and tributaries, none where the incision
    is below the level cut, so that the coarse lowering stays the drawn valleys' void and the
    upland stays at z0 (mars-ceiling §2.2).
  - _Tests._ On the reference Mars: (A2), the coarse volume, at least 1.2 m of global equivalent
    layer (Luo, Cang and Howard 2017), and above 6.1 m (three times their combined 2.04 m) a
    finding to main; the drawn valleys' density over the eroded `Lid` cells recorded against
    Hynek, Beach and Hoke 2010's 0.0115 km⁻¹ for Noachian units (one trunk per 38 km cell is
    already 0.026 km⁻¹); the trunks' incision by drainage class recorded, the share deeper than
    350 m, the p90 and the maximum (T14's 18.2%, 30.8% and 35.7% at 10⁴–10⁵, 10⁵–10⁶ and above
    10⁶ km²), and if the drawn valleys show that tail, a depth relation by drainage belongs here,
    not in K t (Williams and Phillips 2001's walls steepening with depth the candidate); every
    valley link at most `DEBRIS_FLOW_SLOPE`; `has_channels`' runoff condition; and a world wet now
    synthesised as T6.b builds it, to the bit. Acceptance:
    `cargo test -p hyperion-surface synth::channels` and
    `cargo test -p hyperion-sim planetary::surface::steps::erosion`.
  - _Built 2026-10-22_ (lane r09-valleys, from `e1a44211`; Risks, "Deviations in T6.c, as built",
    and "Deferred findings (2026-10-13 rule) (R09.T6.c)"). Main's ruling, option A: a world dry now
    draws its trunks alone, each its epoch's valley, since F2's tributary factor is identically 1
    on T6.b's profiles and the tributaries the epoch's knickpoints reach fail (B) 3.3 times. The
    input is `SynthesisCell::epoch_incision`, a `LogIncision` byte (0 no epoch, 1 uncut, else
    0.5 m × 2^((c − 2) ÷ 16), saturating at 255), the record 26 bytes at
    `SURFACE_PAYLOAD_FORMAT` 1 and `GENERATOR_VERSION` 23 (main's ruling of 2026-10-22, before the
    gate). On the reference Mars: (A2) 1.961 m, asserted; (B) p99 75.8 m; density 0.0247 km⁻¹
    over the eroded Lid; no drawn valley's coarse slope above 0.06; the whole pass's σ_h within
    8.5 × 10⁻⁸. Acceptance as built, beside the two above: `cargo test -p hyperion-surface
field::`, `wire::` and `synth::height_golden` (the codec, the stride and size, and the wet-now
    synthesis to the bit in the Earth-like goldens), `--test panics`, and `cargo test -p
hyperion-sim planetary::surface::quantise`.
- **R09.T6.f Channels kept out of the water** (a serve-gate must-fix for
  `GENERATED_SURFACES_FROM` 24; decision-r09-channel-term §4 and §8, which bind, with main's leans
  of 2026-10-27; T6.d and T6.e are unused). T6.b's first channel level, the trunks, read neither
  water nor base level. Dry cells' trunks cut valleys of 1.1–1.8 km into lake and shelf cells, and
  the level's mean removal lifted the rest of those cells above their water. On the reference
  Earth, 21% of river-mouth points and 16% of lake points were turned to land at every band level
  from 5, and 16% of dry points were cut more than 10 m below the sea (the ruling's probe E).
  Records: [09-surface-generator/records/R09.T6.f.md](09-surface-generator/records/R09.T6.f.md).
  - _(a) Channels on dry ground only._ A coarse cell under water carries no channel term at any
    level, raw or corrected, and its own trunk cuts nothing. A dry cell's raw incision fades by a
    C¹ smoothstep over the level's wall width towards each edge or corner it shares with a wet
    cell. Every correction corner a wet cell shares is zero, and the dry cell's bubble carries the
    rest of its mean.
  - _(b) No valley below its base level._ On a world not dry now, a trunk's valley is held to its
    cell's height above its base level, G ≤ z − ℓ. ℓ is the receiver's water surface where the
    receiver is wet, and the sea level otherwise. The hold is one function that the groove and the
    synthesis's closed forms share, and the tributaries follow it through their inherited top.
  - _Scope._ `synth/channels/incision.rs`, `synth/channels.rs` and their tests. The coarse pass's
    `channel_variance` is kept to the bit, so the coarse field does not move.
  - _Tests._ P1–P4 of the ruling:
    - P1: the term is exactly zero in wet cells;
    - P2: no dry point is cut more than 10 m below the sea, and no trunk axis into water is cut
      below its receiver's surface less its bankfull depth;
    - P3: the means, continuity, cache order and wasm determinism hold, and T19.c's slow
      over-the-wire cases pass;
    - P4: the Mars-like, Moon-like, Ceres-like, flat and one-crater heights are unchanged to the
      bit.
  - _Acceptance._ `cargo test -p hyperion-surface synth::channels`, `synth::levels`,
    `synth::height_golden`, `worker::` and `material::` (the floor's level bound and the
    re-blessed goldens), and the slow `cargo nextest run -p hyperion-sim --features testing --test
channel_water_worlds --cargo-profile slow-test --profile slow --run-ignored only`.
  - _Built 2026-10-27_ (lane r09-channels; Risks, "Deviations in R09.T6.f, as built" and
    "Deferred findings (2026-10-13 rule) (R09.T6.f)"). The cap alone left 1.5% of the reference
    Earth's dry points cut below the sea, so the ruling's fallback is built as well, with main's
    acceptance: a per-point floor at the sea level in `Synthesiser::height_at`. P1 and P2 hold on
    both Earths. The exception is trunk axes into lakes above the sea, which is deferred as a
    figure.

Acceptance: `cargo test -p hyperion-surface synth::channels`.

### R09.T7 Craters

Records: [09-surface-generator/records/R09.T7.md](09-surface-generator/records/R09.T7.md).

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
  Records: [09-surface-generator/records/R09.T7.a.md](09-surface-generator/records/R09.T7.a.md).
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
  Records: [09-surface-generator/records/R09.T7.e.md](09-surface-generator/records/R09.T7.e.md).
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
  Records: [09-surface-generator/records/R09.T7.b.md](09-surface-generator/records/R09.T7.b.md).
- **R09.T7.c Coarse craters' rims.** Sharpening the rims of the coarse list at finer levels. The
  coarse elevation already holds each coarse crater smoothed to the coarse cell, so level n adds
  only the difference between the profile band-limited at level n and at the coarse cell's
  resolution, both by the fillet of T7.b; that difference integrates to zero over each coarse cell,
  so the level-of-detail rule holds. Tests: a coarse crater's profile is continuous across the
  coarse cell edges it spans; the added difference integrates to zero over every coarse cell it
  touches, to 10⁻⁶ of the crater's cavity volume; at the coarse level it adds nothing.
  Records: [09-surface-generator/records/R09.T7.c.md](09-surface-generator/records/R09.T7.c.md).
- **R09.T7.d The cold-trap fraction** (a T7 follow-up, split out of T13.f and built early: the
  galaxy audit's B4; science-r09-drainage.md §4 and §8). It needs nothing of T7.a–c and can be
  built at once. T13.f and galaxy plan 14's P14.T51.f call it, so it lands before the 23 → 24
  batch.
  Records: [09-surface-generator/records/R09.T7.d.md](09-surface-generator/records/R09.T7.d.md).
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
  Records: [09-surface-generator/records/R09.T7.f.md](09-surface-generator/records/R09.T7.f.md).
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
  §8.13; signed off 2026-10-19, `signoff-12.md` item 13). After T7.f, after RM5's close. Until it
  lands the plains' two fits stand, labelled (Risks, "Cold traps"): they match the ray traces at
  σ_s 5.7°, Hayne et al.'s lunar fit and the slope P14.T51.f's tests take, and on rougher plains
  they overstate cold-trap area (water up to 2.9 times at 27°, CO₂ up to 9 times at 11°) and, at
  D 10–20°, permanent shadow (up to 15 times). R09.T13.f places its ice with that overstatement.
  Landing later, T7.g moves P14.T51.f's published areas and R09.T16's goldens, so it rides a
  generator bump.
  - **The table:** a new `hyperion-fit` task, `rough_plains_cold_traps`, ray traces Gaussian
    surfaces built as Hayne et al. 2021's Methods D builds them:
    - random phases on the task's own seeded stream;
    - Fourier amplitudes |k|^−(1+H), H 0.9, between two cut-offs;
    - heights scaled to a directional RMS slope w; periodic (Hayne et al. strip a margin instead);
      horizons by rays at fixed azimuths; a point star;
    - each permanently shadowed pixel's peak irradiance from the sunlit facets it sees (their
      direct flux weighted by view factor), to first order, as §8.13's tracer computes it, with no
      albedo or emissivity in it.

    Over w 0.05–1 (`Terrain`'s 45°), in colatitude ÷ w and D ÷ w (D = δ + α) so that a slope
    below 0.05 reads the 0.05 slice, it tabulates the permanent shadow and the shares of it whose
    peak irradiance lies below fixed fractions of S sin e.

  - **Its use:** `cold_trap`'s plains read it in place of the half-normal bowls,
    `PLAINS_DEPTH_SCALE` and `PLAINS_SURROUNDINGS`. The share is picked by εσT⁴ at the
    threshold, less the heat flow, over (1 − A)(A + ε) S sin e, the body's own A and ε: the
    shadow's absorbed flux to first order (the lit facets' scattered light and their thermal
    emission). None is cold where that is not positive.
  - _Tests:_ its instantaneous shadow is within 15% of Smith 1967's at sun elevations up to 4° (the
    science check's tracer, 2° azimuths: 12%; Hayne et al.'s, 1°: 5%). Its permanent shadow is
    within the ray-traced surfaces' spread at σ_s 5.7° and 16.7°, and within 15% at 11.3° and
    26.6°; its water cold traps are within 15% of §8.13's, and its CO₂ within a factor of two. A
    body of another albedo and emissivity reads the same table.
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
  Records: [09-surface-generator/records/R09.T7.h.md](09-surface-generator/records/R09.T7.h.md).
- **R09.T7.i The fillet through the peak** (a T7 follow-up, after R09.T12.e;
  `decision-r09-t12-coarse.md` addendum 4, D1.5 and D4.2, and addendum 5). Before
  R09.T14, whose drainage reads the coarse field. A fillet whose inner end 1 − w lay inside a
  complex crater's central cone started its Hermite at the peak's top and the cone's slope (−9.5 km
  per rim radius on a 113 km lunar crater, over a span of 2w), which runs below the floor and 4 km
  below the rim, and the compensating c (1 − (x ÷ r₁)²)² over the reach put the volume it took back
  as a dome on the centre. Every fillet held at min(1, a) rim radii reaches the cone, so the coarse
  craters of `D_b` to about 2 `D_b` were inverted in the coarse field (a 113 km lunar crater
  +2,165 m at its centre and −607 m at its rim, fresh −1,080 and +1,553 m), those to about 2.6 `D_b`
  too shallow, and the client drew them so at every level whose band reaches the cone, as it drew
  small complex craters at the first level that keeps them. `synth/craters.rs`'
  `CraterProfile::filleted`:
  Records: [09-surface-generator/records/R09.T7.i.md](09-surface-generator/records/R09.T7.i.md).
  - _The cone faded with the rim._ Where 1 − w lies within the peak's base c_p, the Hermite starts
    from the floor, −(d − h), at zero slope, and the cone keeps the share (1 − w) ÷ c_p of its
    relief, so that it fades continuously to nothing where the fillet is held at one rim radius
    (main's ruling: a cone dropped outright jumps by its whole relief at the switch); elsewhere the
    fillet is as built.
  - _The compensation on the fillet's span_, c (1 − ((x − 1) ÷ w)²)² on [1 − w, 1 + w] with
    c = −15 Δ ÷ (32 w), Δ the volume the fillet and the faded cone took, in place of the
    reach-wide term, which raised a crater's whole floor (786 m on the 113 km crater at w = 0.41 R).
    Outside the span every profile is its fresh shape's, to the bit.
  - Each piece stays a polynomial of degree at most 4, so the moments and `difference_moments`
    stay exact; every profile integrates to zero, reaches no farther, is continuous in w and is C¹
    but for the kept share of the cone's kinks. `rims.rs`' and `bound.rs`' documentation and Design
    note 13 follow.
  - _Tests:_ over shapes of D ÷ `D_t` 0.5–30 and w ÷ R from 0.05 to held: the balance to 10⁻⁶ of
    the cavity volume at levels 0–24; height and slope continuous at 1 − w, 1 + w and 1 − w within
    the cone to 10⁻⁹ relative, and the profile continuous across the switch; the fresh shape
    outside the span, to the bit; a depression, F(0) < 0 and below F at every x ≥ 1; the 113 km
    lunar crater over the reference Moon's coarse band (92.6 km) is −(d − h) at its centre (about
    −3,330 m; +2,165 m as built). In `steps::craters`: one 113 km crater alone in a flat state at
    the reference Moon's level, read through T4's interpolant, lies at its centre at least 1,000 m
    below its mean over 1.2–2 R (main's planar estimate 2,010 m; a Gaussian low-pass of s.d.
    `w_c` ÷ π, 1,450 m; 320 m as built). Reported: the share of the reference Moon's first coarse
    octave whose centre's cell lies below its 1.2–2 R cells, before and after.
  - The synthesis's goldens on the cratered `testing` worlds, `tests/golden/wire/` and R10's that
    read them move: re-blessed at the version in force with no bump (the serve gate), with
    `golden_diff.py` accounting for each. Acceptance: `cargo test -p hyperion-surface craters`,
    `cargo test -p hyperion-surface --lib height_golden`,
    `cargo test -p hyperion-sim --lib planetary::surface::steps::craters` and
    `just test-wasm-fast`.
- **R09.T7.j Craters worn to equilibrium** (a T7 follow-up, after T7.i;
  `decision-r09-t12-coarse.md` item 5b, addendum 4, D1.4 and D4.1, and addendum 5;
  `decision-r09-t7ij-refresh.md`). Before R09.T16's goldens. The relief factor carried only
  Fassett and Thomson 2014's κ, which leaves a 50 km crater 99.95% of its relief after 4 Gyr, so
  a surface at Trask's saturation was a field of fresh Pike profiles, which no equilibrium
  population is (Head et al. 2010: "the most-densely cratered portion of the highlands reached a
  state of saturation equilibrium"). The craters below the cell carried 2–4 times the bodies'
  measured share above l_N (T12.e's test 2: 27.0% of σ_h² on the reference Moon against 4–10%),
  and the Moon's crater term in expectation 1.8 times plan 14's basin share. `synth/craters.rs`
  and `steps/craters.rs`:
  Records: [09-surface-generator/records/R09.T7.j.md](09-surface-generator/records/R09.T7.j.md).
  - _The law._ g = (b ÷ (K ÷ R² + b))², b = 0.15 (Minton et al. 2019, eq. 8, as built), with
    K ÷ R² = max(κt ÷ R², K_v q): κt the as-built diffusion; K_v = 0.17, Minton et al.'s
    visibility function at γ = 2 (eq. 7 and Fig. 5: a crater stops being countable at
    K ÷ r² = 0.17, d/D 0.050; eq. 8 inverted there gives 0.163, and the paper's 0.17 is kept);
    q = N(t) P ÷ (N_S S), the production of the crater's octave since it formed over the octave's
    saturation, N(t) the chronology's density over the crater's age, N_S the surface's, P and S
    the octave's production and Trask's saturation.
  - Over a capped octave's window q runs uniformly from 0 to 1, so its oldest craters stand at the
    visibility limit, where equilibrium loses them (Minton et al. §2: degradation accumulates with
    the production, and a crater is lost when it reaches the visibility function); there
    E[g²] = (b ÷ K_v)(1 − (b ÷ (K_v + b))³) ÷ 3 = 0.2638 where κ is negligible. On an uncapped
    octave q ≤ P ÷ S: the rate per unit production is the cap's, the same at every size (geometric
    similarity, γ = ψ = 2). Below a few kilometres, where κ is the larger, κ's calibration stands.
    No constant is fitted.
  - One law: every small crater's g, the small craters' E[g²] over the window (`CraterOctaves`'
    closed forms, T7.h's among them) and every coarse crater's degradation byte (T12.d's, the
    nearest integer to 255 (1 − g), item 5b). No stream, draw or word changes. T12.e's crater
    scale composes with it (addendum 4, D4.7). Provides: K_v and the law's one public function in
    `synth::craters`, and `profile` and `CraterProfile::new` taking the crater's wear (its
    diffusion and its share q) where they took its diffusion; the names are the record's.
  - Labelled: the visibility limit is Minton et al.'s for simple mare craters, carried to complex
    craters and basins by relative relief, g = (b ÷ (K_v + b))² = 0.220 of the fresh relief at
    every size (medium-low confidence); the uncapped octaves' rate (medium-low); q uniform, the
    wear standing in too for the overprinting the superposed craters lack (Minton et al.'s
    first-order loss, and Riedel et al. 2020's cookie cutting in the highlands, would leave
    survivors fresher); κ held constant (T7.b's finding stands); fluvial and aeolian infill,
    viscous relaxation and mare or melt infill not modelled.
  - _Tests:_ g at q = 1 and κt = 0 is (15 ÷ 32)² to 10⁻¹²; q is the crater's draw share u on a
    capped octave and u P ÷ S on an uncapped one, to 10⁻¹²; E[g²] against dense sums over the
    window to 0.1% on every synthetic world's octaves, and 0.2638 to 10⁻³ on a capped octave of
    50 km craters; on the reference Moon, every coarse octave capped, every byte is at most 199
    and the listed craters' mean g² lies in 0.21–0.32. T12.e's test 2 is asserted where it holds,
    a miss reported and recorded, never loosened (main's estimates: the reference Moon 6.2% in
    4–10%; the Earth 0.44% and Mars 1.0–1.1% in 0.3–1%; Ceres 2.4–2.7% in 3–8%); its tests that
    read T12.d's bytes of 0 keep their invariants. The reference Moon's crater term in expectation,
    rendered at the band limit, lies within 25% of plan 14's 2.05 km basin share (estimated
    1.88–2.02 km; 3.67 km before). Goldens as T7.i's. Acceptance: T7.i's, with
    `cargo test -p hyperion-sim --lib planetary::surface::steps::relief`.

- **R09.T7.k The cold-trap fraction's shared integral and cheaper rules** (a T7.d follow-up,
  built by plan 14's P14.T51.i in lane `p14-t51hij`, the `p14-snapshot` lane's successor;
  decision-p14-t51-batch §1).
  - The multi-threshold form, bit for bit with `fraction`.
  - `instantaneous` at at most 8 points.
  - `INTERVALS` 128.
  - T7.d's tests unchanged.
  - T13.f reads them through plan 14's `ColdTrapGround::fractions_at`, whose signature T51.i keeps
    (decision-r09-t13f-cold-traps §8).
  - _As built (2026-10-19)._ All in `cold_trap.rs`, additive in its API and a value change in its
    rules; no golden reads it, and no stream or tag moves.
    - `fractions(latitude, body, terrain, thresholds, fractions)` writes each threshold's fraction
      in their order. The star's path is found once. Each population's shadow is one trapezoid
      integral over its fixed nodes (`Population::cumulative_each`, `Band::shadowed_each`,
      `Terrain::shadowed_each`), its integrand evaluated once up to the largest cold limit, and
      each threshold's share read from the running sum where its own would stop, by the same
      operations, so each is `fraction`'s to the bit. `fraction` is `fractions` at one threshold.
      Its panics (lengths, a threshold not finite) are in `tests/panics.rs`.
    - The shared integral skips the bowls shallower than the shallowest that keeps any shadow,
      whose share `read`'s first test makes +0, with a margin of a millionth of the ratio, far
      above that test's rounding: neither the bowl nor the density is evaluated there, and the sums
      keep their bits, tested against the unskipped `shadowed` (the ruling's optional "evaluate
      only where it can bind", in its cheapest form). `the_shared_integral_is_the_one_threshold_path_to_the_bit` holds the shared sums to `cumulative` and `shadowed`, which never share.
    - The instantaneous shadow takes the 8-point Gauss–Legendre rule (`GAUSS_LEGENDRE_8`, the sim's
      `tables::gauss_legendre` literals), within 4.4 × 10⁻¹⁶ of the 16-point rule over colatitudes
      0.1–30°, D 0–20° and d/D 0.005–0.5 on 10,800 points (the ruling's port: 4 × 10⁻¹⁶).
    - `INTERVALS` is 128. The cold traps move, relative to 512 intervals and 16 points (the lane's
      figures, the real Sun and Hayne et al.'s lunar terrain): the Moon's water −9.3 × 10⁻⁵ (B) and
      +2.8 × 10⁻⁴ (A); its CO₂ at 58.18 K +1.1 × 10⁻³ (B) and +8.5 × 10⁻⁴ (A), and at 55.53 K
      +2.1 × 10⁻³ and +1.9 × 10⁻³; Mercury's water +2.6 × 10⁻⁴; Ceres's permanent shadow at 4°
      −4.2 × 10⁻⁵ (B) and −3.7 × 10⁻⁶ (A), and at 20° −2.1 × 10⁻⁴ (A).
    - T7.d's tests keep their tolerances but one: the trapezoid's unit-integrand test, whose
      5 × 10⁻⁵ was the 512 intervals' bound. At 128 the rule's h² is 16 times as large, the worst
      measured is 3.2 × 10⁻⁴, and the bound is 4 × 10⁻⁴. The rule's degree test is now 15, the
      8-point rule's.
    - The supervolatiles (decision-p14-t51ij-budget §4.4; plan 14's P14.T51.i record): where a
      species traps at least 10⁻⁵ of the surface, every move is at most 2.0 × 10⁻³ (CH₄ at 10 au);
      above 10⁻² only at a trap's onset, over 2.3 × 10⁻¹⁰ of the surface or less (N₂ up to
      +6.6 × 10⁻², CO +3.3 × 10⁻², CH₄ +3.4 × 10⁻²). On the Moon N₂, CO and CH₄ trap nowhere.
    - _For R09.T13.f_ (decision-p14-t51ij-budget §6.5): T51.i moves the fractions at
      `trap_threshold`'s thresholds by at most 1.1 × 10⁻³ relative (CO₂ at 58.18 K; water less), so
      T13.f's tests there keep at least 2 × 10⁻³ or one cell. At other thresholds the move is larger
      (CO₂ at 55.53 K: 2.1 × 10⁻³; the supervolatiles as above). A T13.f test at another threshold
      compares live plan-14 values or uses at least 5 × 10⁻³.
    - `bowl_permanent_shadow`'s fit (`hyperion-fit`) reads the cap through it, and its 0.003
      acceptance cannot tell 4.4 × 10⁻¹⁶.

Acceptance: `cargo test -p hyperion-surface craters` (T7.a–c, T7.e, T7.i and T7.j, with
`cargo test -p hyperion-sim --lib planetary::surface::steps::craters` for T7.i–j) and
`cargo test -p hyperion-surface cold_trap` (T7.d).

### R09.T8 Assembly, the bound and the patch

Records: [09-surface-generator/records/R09.T8.md](09-surface-generator/records/R09.T8.md).

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

- **R09.T8.d The reach's two loose terms** (a follow-up of T8, high priority: before R10.T14.b's
  measured sizes and T15; decision-r10-t14c-leans F4, ruled in decision-r09-t8d-reach). The
  table's reach, about ±155–160 km on every generated world against a realised relief of 15.8 km
  on the fixture, weakens R05's horizon culling and inflates T14.b's peaks. Two terms make most of
  it. Tighten both and keep every bound sound.
  Records: [09-surface-generator/records/R09.T8.d.md](09-surface-generator/records/R09.T8.d.md).
  - **(a) The coarse rims' count**, in both `contribution_range_m` and `level_bound_m`: count each
    octave's craters over the area where a crater's sharpening is non-zero, not over the reach
    disc. That area is the fillet's span about the rim at the octave's widest w, plus the
    central cone the fillet reaches. The bound stays a 1 − 10⁻⁹ quantile.
  - **(b) The channels' reach and omitted terms, conditioned on the body.** They are zero where no
    coarse cell can carry a network, if the header or the coarse pass can certify that; otherwise
    they are held to the body's largest valley depth rather than the geometric cap.
  - **(c) Tests:** each synthetic world's dense samples of h − base within the reach at every
    level; the reach within a stated factor of the realised relief (set from the worlds); every
    existing bound test passing.
  - **(d) Goldens:** no generated height moves. The worker goldens' `level` lines and any pinned
    bound values are re-blessed at the version in force, with every move stated. The bump is the
    sim-determinism skill's call.
  - **(e)** The client's `lowestHeightM`, selection and T14.c's ceiling read the tightened table
    unchanged. Report the new table's highest and lowest on the fixture and on the reference
    Moon, Mars and Earth.
  - Acceptance: `cargo test -p hyperion-surface --features testing` (the bound, the reach, the
    field's and the wire's refusals among it), `cargo test -p hyperion-sim --features testing --lib
planetary::surface::quantise`, the slow
    `synth::levels::tests::the_level_bound_and_the_reach_hold_at_every_level_at_ten_thousand_points`
    by name, `pnpm --filter hyperion exec vitest run src/renderer/src/view/terrain`, and the
    gate.

### R09.T9 Golden heights across targets

Records: [09-surface-generator/records/R09.T9.md](09-surface-generator/records/R09.T9.md).

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

Records: [09-surface-generator/records/R09.T10.md](09-surface-generator/records/R09.T10.md).

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
  Records: [09-surface-generator/records/R09.T10.c.md](09-surface-generator/records/R09.T10.c.md).
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
  Records: [09-surface-generator/records/R09.T10.d.md](09-surface-generator/records/R09.T10.d.md).
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
  Records: [09-surface-generator/records/R09.T10.e.md](09-surface-generator/records/R09.T10.e.md).
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
  Records: [09-surface-generator/records/R09.T10.f.md](09-surface-generator/records/R09.T10.f.md).
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

Records: [09-surface-generator/records/R09.T11.md](09-surface-generator/records/R09.T11.md).

`steps/plates.rs`, Design note 6. Tests: every cell has one plate and ties go to the lower seed
index; boundaries are exactly the adjacencies between plates; signed distances are zero on
boundaries and grow by at most one edge length a step; boundary kinds follow the relative motion on
hand-placed plates; the plate areas follow the tail law within a Kolmogorov–Smirnov bound over many
seeds; the mean plate count over those seeds is 48 ± 7; the areas sum to 4π; the distance to
the plate's own divergent boundary, which GDH1 reads, is zero on ridges and never taken from a
trench; a stagnant-lid world has none; each oceanic cell's source half-rate is |ω × r| ÷ 2 normal
to the ridge on hand-placed plates, inherited along the own-ridge transform; its age is d_r ÷ u_h,
capped at 200 Myr; a plate with no divergent boundary has 200 Myr and 23 mm a⁻¹
(`decision-r09-t5.md` item 5); that age is the kinematic age, which R09.T12.a ranks into the
steady-state age (`decision-r09-t12-coarse.md` item 1). Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::plates`.

### R09.T12 Coarse elevation

Records: [09-surface-generator/records/R09.T12.md](09-surface-generator/records/R09.T12.md).

- **R09.T12.a Crust and ridges.** `steps/relief.rs`: Design note 7's two crust populations (their
  lithologies from plan 14's `crust`: `Oceanic` the secondary crust's entry, `Continental` the
  tertiary's) about sea level and GDH1's age–depth law on the steady-state age, ranked from the
  distance to divergent boundaries, under the sediment's isostatic load
  (`decision-r09-t12-coarse.md` item 1). Tests: the reference Earth's hypsometry, binned as
  Earth2014 was (area-weighted 250 m bins, the TBI layer at 5′), is bimodal with a land mode within
  300 m of +125 m, an ocean modal bin among the centres −5,125 to −4,125 m (Earth2014's bins within
  10% of its −4,375 m peak) and a mean below −2 km from −4,731 to −3,981 m (Earth2014's
  −4,281 m, +300 m and −450 m, the deep side widened for the intraplate volcanism no task builds;
  `decision-r09-t12-coarse.md` item 1); the unloaded basement is GDH1's at 5, 20 and 100 Myr to 1 m;
  the steady-state ages' area-weighted mean is t_m ÷ 3 to 10⁻⁹ relative, they never decrease with
  the kinematic age, and t_m is 192.9 Myr on the reference Earth and quadruples when the heat flow
  halves. The sea floor's roughness inputs (`decision-r09-t5.md` item 5): H₀ from the source
  half-rate, the pelagic drape from the age and latitude, the ponded wedge from the plate's passive
  margins over its own oceanic cells, quantised into `seafloor`. Tests: H₀ within 30% of the mean of
  Goff et al. 2004's Table 2 entries at full rates of 28, 40, 75 (the axial high alone), 110 and
  160 mm a⁻¹ (`decision-r09-t12-coarse.md` item 5a); S_p is 468 m at 81 Myr on the equator; the
  wedge never crosses a plate boundary; on the reference Earth, oceanic cells whose style amplitude
  is at most 0.2 cover 25–40% of the oceanic crust deeper than 3 km, and the mean of S_p + S_t over
  oceanic crust farther than 200 km from a passive margin is 250–600 m (GlobSed's 404 m). On every
  lid without plates, one structural population about the datum: Gaussian, of plan 14's
  `STAGNANT_LID_RELIEF`, its degree variance ∝ l^−β₁ from degree 2 to the cell (Turcotte 1987)
  (`decision-r09-t12-coarse.md` addendum A1).
  Records: [09-surface-generator/records/R09.T12.a.md](09-surface-generator/records/R09.T12.a.md).
- **R09.T12.b Convergent landforms.** Belts, trenches and arcs by boundary kind, scaled as Design
  note 7 says. Tests: belts lie within their stated distance of convergent boundaries; trenches lie
  2–4 km below the neighbouring sea floor and the arcs' volcanic fronts lie behind them with
  medians within 15 km of 190 km (oceanic overriding plate) and 260 km (continental), 80% of
  fronts within 140–250 and 180–330 km (`decision-r09-t5.md` item 6).
  Records: [09-surface-generator/records/R09.T12.b.md](09-surface-generator/records/R09.T12.b.md).
- **R09.T12.c Stagnant-lid provinces and flexure.** After T0.b's isotherm item. Volcanic provinces
  sized by the volcanism level and the flexure about each load, its moat filled and its bulge planed
  to the surface's level (addenda 2 and 3), with T_e = k (870 K − T_s) ÷ F (T0.b's ruling, Design
  note 3). T_e's isotherm and conductivity come from the crust's substance: the ice lean of Risks
  becomes the `H2O` row's (decision-composition). Tests: a line load's bulge crest lies at πα with
  height 0.043 w₀; α at T_e = 70 km under Mars's gravity (3.71 m s⁻²) and radius (3,389.5 km), with
  ρ_m 3,300 kg m⁻³ and an open moat (ρ_fill = 0), is 180 km ± 10 (its value 175.5 km;
  `decision-r09-t12-coarse.md` item 3); a water-ice lid's T_e is (567 W m⁻¹ ÷ F)
  ln(170 K ÷ T_s), 2.52 km at 200 mW m⁻² and 70 K; T_e never exceeds the body's radius; T_e is
  108 km ± 1 at Mars's 19 mW m⁻² and 210 K, and zero where T_s ≥ 870 K, where each load is
  compensated locally (α = 0); under a load the surface stands at the load's height above the filled
  level, and off the loads at the filled level, in the moat and on the bulge alike; on the reference
  Mars the loads' footprint covers at most 25% of the surface (`decision-r09-t12-coarse.md`
  addendum 2).
  Records: [09-surface-generator/records/R09.T12.c.md](09-surface-generator/records/R09.T12.c.md).
- **R09.T12.d Coarse craters.** `steps/craters.rs`, Design notes 5 and 10, after T7.e. Each
  crater's shape is T7.a's `crater_shape`, so the coarse elevation and the client's T7.c rims are
  one profile. Tests:
  counts over the reference Moon match min(production, saturation) above D_b (Poisson interval),
  about 300 above 100 km with N(1 km) at 4.4 Gyr; ages split before and after the wet epoch on the
  reference Mars; the list is sorted and each crater's `reach` is complete; before degradation,
  the reference Moon's coarse craters' rim heights are Pike 1980b's 0.236 D^0.399 to 1%.
  Records: [09-surface-generator/records/R09.T12.d.md](09-surface-generator/records/R09.T12.d.md).
- **R09.T12.e σ_h and sea level.** Design note 7, on the reconstructed field, after T4 and T5.
  Its σ_h budget books the realised structural share, the area mean of each quadrature point's
  `Relief::point_variance`, which includes the belts' lift (`decision-r09-t5.md` item 8), with the
  whole share the craters and channels add below the cells: the small craters' whole, each coarse
  crater's increase of its mean square from its coarse profile to its band limit's, and the
  channels' (`decision-r09-t5.md` item 4, amended by `decision-r09-t12-coarse.md` addendum 4). The
  header's spectrum is `BandSpectrum::anchored` of the body's crust class (`decision-r09-t5.md`
  item 3), chosen from `surface_material` and `tectonics`.
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
  Records: [09-surface-generator/records/R09.T12.e.md](09-surface-generator/records/R09.T12.e.md).

Acceptance: `cargo test -p hyperion-sim planetary::surface::steps::relief` (T12.a–c and e) and
`cargo test -p hyperion-sim planetary::surface::steps::craters` (T12.d).

### R09.T13 Climate

- **R09.T13.a The zonal model.** After T0.b's transport-cap items. `steps/climate/ebm.rs`: Design
  note 8's implicit seasonal moist
  energy-balance model in latitude, with its heat capacities, gray outgoing radiation, albedo and D
  scaling, on the grid Design note 8 names, the rotation and pressure factors tabulated from Kaspi
  and Showman 2015 (T0.b's transport law). Tests: an Earth-like input converges within the fixed
  orbit count and its annual-mean zonal temperature lies within 5 K of Siler et al.'s Fig. 2b
  (ERA-Interim; _amended since, by decision-r09-t13i's §2.4 follow-up §3 and follow-up 2 §2, as the
  first of R09.T13.i's batch-1 tests states it: on the Earth with its land at the sea level, each
  band's departure from 58.75° S northward, less the solid-angle mean of those departures, lies
  within 5 K, or 7 K in the ice edge's tier from 61.25° N to 71.25° N, and that mean within 3 K;
  each band south of 58.75° S lies within 12 K of ERA-Interim (T13.a's record); every band is
  printed_); with seasons off, the equator–pole contrast, scaled so that the 1 Ω⊕, 1 bar run
  reads their 40 K (Fig. 8a) or 42 K (Fig. 15c), matches Kaspi and Showman's figures to 2 K over
  Ω⊕ ÷ 24 to 12 Ω⊕ and 0.2–50 bar; the scheme is stable at Venus's rotation and
  at a six-hour step for every rotation in the reference set; the months are twelve equal spans of
  eccentric anomaly from periapsis, each record the mean of its span's steps; at e = 0 they are the
  period's twelfths, and at e = 0.6 the periapsis month's record averages 0.4 twelfths of the
  orbit.
  Records: [09-surface-generator/records/R09.T13.a.md](09-surface-generator/records/R09.T13.a.md).
- **R09.T13.b Longitude and locked coordinates.** The periodic longitude solve split after the
  latitude one in a fixed order, and the tidally locked coordinates about P14.T14's substellar
  axis. The locked solver steps seasons as the zonal one does: a world locked 1:1 to its star has
  twelve months wherever its orbit is eccentric or it has obliquity, and a moon locked to its
  planet has its planet's orbit's seasons at its own obliquity to that orbit (Design note 8; a lock
  adds nothing, signed off 2026-10-10, `signoff-2.md` item 4). Tests: with no land–sea contrast the
  zonal model's answer is reproduced to 0.01 K; a locked world is warmest at the substellar point;
  a world locked 1:1 to its star on an eccentric orbit, or with obliquity, has twelve months, and
  its months differ.
  Records: [09-surface-generator/records/R09.T13.b.md](09-surface-generator/records/R09.T13.b.md).
- **R09.T13.c The other regimes.** `steps/climate/{airless,isothermal}.rs`: radiative equilibrium
  with thermal inertia for airless and thin-atmosphere worlds, and the isothermal surface for a
  Venus. _As built since P14.T54.a, the radiative model's Γ is the record's
  (`CoarseInputs::ground_inertia_j_per_m2_k_sqrt_s`): P14.T49.j's law at the crust's lid,
  `substance::thermal::reference_regolith_thermal_inertia` (the lid's family's reference form),
  at the body's equilibrium temperature, with P14.T24.a's 55 and 20 J m⁻² K⁻¹ s^−½ as its
  fallback alone. P14.T49.j's `regolith_thermal_inertia` with `RegolithForm::new` takes a layer
  of a caller's own porosity and grain size, which no R09 task passes yet (galaxy plan 14's
  Risks, "Deviations in P14.T54.a, as built", "One Γ")._ On a locked lava world, cells above the
  transition temperature of their own crust entry
  are melt, from this task's per-cell temperature, not from the global state; the body's
  `melt_area_fraction` is the lid's crust's, the crust P14.T51.c's `lid` names (`CrustInputs::lid`,
  R09.T10.d; science-composition-crusts.md §5, which replaces "the secondary's, or the primary's
  where there is none"). Tests: an airless world's day–night contrast exceeds 300 K; the
  isothermal model's own field, before T13.d imposes plan 14's contrasts, varies by under 1 K; a
  locked lava world's melt is the dayside region above the solidus, and its nightside is solid.
  Records: [09-surface-generator/records/R09.T13.c.md](09-surface-generator/records/R09.T13.c.md).
- **R09.T13.d Normalisation and ice.** Plan 14's mean and signed contrasts imposed on the field as
  plan 14 defines them (decision-r09-t13d-normalisation): the equator–pole contrast on the annual
  field's T(0°) less its poles' mean, a locked world's day–night contrast on its μ-weighted day and
  night means about the substellar axis, and the mean, each read in the model's kind (arithmetic on
  the energy-balance and isothermal models; on the radiative model, which has no lapse term, the
  brightness of the emitted flux, less the night floor's for the mean and the equator–pole
  contrast). The corrections are c + a P₂(sin φ) + b P₁(cos γ), b on a world locked 1:1 only, the
  same at every sample and month of a model grid point. On the energy-balance and isothermal models
  a and b come from an exact linear solve on the sea-level grid, and c is a constant after the lapse
  term. On the radiative model all three are solved together on the converged orbit's moments. No
  record lies below the model's lower bound (the night floor, or the frost point the model holds the
  surface at). Each ice placed per condensate, most volatile first, on the cells coldest against its
  own frost point by their annual radiative mean, ⟨T⁴⟩^¼ over the year (ties by the warmest month,
  then the cell's index), until its area matches plan 14's area for that substance (`surface_ices`),
  less its cold-trapped part (`cold_traps`), which T13.f places (`place_ices` places both parts),
  and each liquid's surface to its share of the ocean fraction (`surface_liquids`); the
  energy-balance albedo function shifted to the climate condensable's freezing point, by
  `SubstanceId` (decision-composition). Tests: the area-weighted surface mean equals the input to
  0.1 K on the energy-balance and isothermal models, and the radiative model's brightness mean of
  its moments equals it to 10⁻⁶ K; the annual field's ends give the input contrast exactly, and on a
  field shaped like P₂ the P₂ coefficient is −⅔ of it; a locked world's μ-weighted day and night
  give its day–night contrast exactly, and on a P₁ field the P₁ coefficient is ¾ of it; a Moon-like
  airless input's correction is under 0.5 K at every grid point; no record lies below the model's
  lower bound on any reference world or on a locked lava world; ice area equals the input fraction
  to one cell; a warm pole gets no cap; a 90°-obliquity world, all land at an Earth-like mean
  temperature, is labelled by FILLET's ice-edge states (an ice belt, not caps, when plan 14's
  history says it started cold), and its warm-start twin places the same cells; the reference Mars
  places CO₂ ice, placed first, inside its water-ice cap's latitude, given by hand, with every iced
  cell poleward of 75°; a Titan-like world's methane liquid fills its basins to its share.
  Records: [09-surface-generator/records/R09.T13.d.md](09-surface-generator/records/R09.T13.d.md).
- **R09.T13.e Precipitation and wind.** Design note 8's heuristic, labelled in the header, and
  each month's 10 m wind. Tests (decision-r09-t13ge §2.13):
  Records: [09-surface-generator/records/R09.T13.e.md](09-surface-generator/records/R09.T13.e.md).
  - global precipitation equals global evaporation, the open liquid's Priestley–Taylor evaporation
    plus the land's ε(z) P, to 1%;
  - the reference Earth's annual zonal precipitation, its land Natural Earth's by 2.5° × 5° box and
    its heights R09.T13.g's band eighths placed by each box's Earth2014 median land height:
    - in each hemisphere the solid-angle mean over 40–60° exceeds that over 15–35° (restored by
      decision-r09-t13e-revision);
    - in each hemisphere the zonal budget's annual E − P has a negative solid-angle mean over 40–60°
      and a positive one over 15–35°;
    - its maximum within 30° of the equator lies within 10° of the annual energy-flux equator;
    - the mean of its all-land climate cells between 50° and 60° N and 40° and 100° E is at least
      0.4 of that between 10° W and 20° E (GPCP v2.3: 0.59);
    - at most a quarter of the solid angle of its all-land climate cells between 40° and 70° N is
      arid by Köppen–Geiger's bound at their own temperatures (Peel et al. 2007, Table 1, as plan 10
      reads it);
    - every month's balance factor lies within [0.85, 1.15];
  - on a Gaussian ridge 3 km high with a 100 km half-width at half maximum, under a steady wind, the
    lee plain three half-widths downwind of the crest receives under half the windward plain's rate
    three half-widths upwind, and the maximum lies on the windward side;
  - over a flat continent under a steady onshore wind, inland precipitation decays with an e-folding
    (ln P fitted from 500 to 2,500 km inland) within 2× of L̃_l = L_c ÷ λ̃₋ (Hergarten and Robl's
    eqs. 24 and 45–47; 2,004 km at their set);
  - the header says heuristic;
  - each month's orographic step reads that month's quantised wind, carrying vapour along its
    component along the model's bands (east, west or calm by its sign);
  - on a synthetic aquaplanet at 60° obliquity and Earth's rotation:
    - no month's rain-band centre lies poleward of the ascending-branch cap, 37.4°;
    - an equatorial cell's meridional wind changes sign twice a year, each month's speed at least
      0.05 m/s;
  - the same aquaplanet at Ω⊕ ÷ 8 has a cap of 90°, and its centre is its energy-flux equator in
    every month;
  - a Titan-like world evaporates only over its methane seas, and its global precipitation equals its
    evaporation to 1%;
  - a dry world under the energy-balance model has no precipitation and winds that are not calm;
  - the reference Mars and the Venus-like body have no precipitation and calm winds;
  - a world at e = 0 with no obliquity to its seasonal orbit has one wind;
  - the annual runoff function is zero over liquid and on a dry world, and over a flat sea-level
    continent is, month by month, 1 − ε₀ of P on ground above the condensable's freezing point and
    all of P on frozen ground;
  - the orographic model's vapour source: on a land ring under a zonal wind with no sea every cell's
    rate is S ÷ (1 − ε₀), and S where the ground is frozen; inland of a coast the rate tends to that
    floor with the coastal excess e-folding within 2× of `L̃_l`; the land's water budget closes with
    the source; a cell 3 km up takes e^(−1.5) of its band's convergence
    (decision-r09-t13e-revision);
  - the orographic solve converges to its test: on the strip, the ridge, a land convergence line and
    a poleward wind against the cell order, every land cell's rate lies within 1% of a deep solve's,
    the land's water budget closes within 10⁻³ of its inflow, and a two-cell cycle equals its closed
    form; on the reference Earth through the coarse pass (slow) every month converges before the
    cap, its budget closes, and every land climate cell's monthly and annual rate lies within 1% of
    a deep solve's (decision-r09-t13e-lfpm-convergence).
- **R09.T13.f Cold traps.** After T13.c, T13.d and T7.d (science-r09-drainage.md §4;
  decision-r09-t13f-cold-traps, whose interface text and tests bind).
  Records: [09-surface-generator/records/R09.T13.f.md](09-surface-generator/records/R09.T13.f.md).
  - **The shared closed form** is T7.d's `hyperion_surface::cold_trap::fraction`, read through
    plan 14's `planetary::hooks::cold_traps::ColdTrapGround`, as P14.T51.f reads it. This task
    changes neither. Its one edit in plan 14's file is that interface, additive and bit for bit, in
    its own commit landed before P14.T51.i (decision-r09-t13f-cold-traps §3).
    - Its sky and ground: the inputs' spin, host flux and seasonal orbit, and
      `cold_trap_inputs()`'s star angular radius and bare-ground Bond albedo.
    - Its heat flow is `heat_flow`, its terrain plan 14's `terrain_of` from `craters()`, and each
      species' threshold `trap_threshold`.
  - **The inputs:** `CoarseInputs::cold_traps()`, the trapped part of each `ices()` share, and
    `cold_trap_inputs()`. Both are empty or `None` from the record until P14.T54.b.
  - **In the airless model:** each field cell's fraction for every trapped condensate, from one
    `fractions_at` call at the cell's latitude, symmetric about the equator.
  - **The placement:** after T13.d's zonal parts, each trapped ice in proportion to each cell's
    fraction, scaled over every cell to plan 14's area and capped at the fraction. A cell names
    one ice: the trapped ice of largest share. Other trapped ices there, and trapped ice on
    another ice's zonal cell, are counted as undrawn, never moved. Its quantised shares are
    rounded coldest first, so each drawn total matches to one cell. It is never placed
    coldest-first by whole cells.
  - _Tests_ (the closed form's own, Ingersoll et al.'s 97 K and Hayne et al.'s Table 1 bands, are
    T7.d's; decision-r09-t13f-cold-traps §7.2–§7.3):
    - the inputs: each refusal of the cold traps and their inputs, by its variant; a valid input
      round-trips through its getters; and the tripwire
      `the_record_gives_no_cold_traps_until_the_24_bump`, which fails at 24 so that P14.T54.b
      wires the record's traps;
    - a Moon-like input (T51.f's lunar case: α 0.2711°, A 0.12, q 0.018 W m⁻², δ 1.54°) given
      1.7 × 10⁻⁵ of water ice, all of it cold-trapped, places it only poleward of 70°, more than
      90% of it poleward of 80°, no cell's share exceeds its fraction, and none of it is short;
    - at 25° obliquity no cell has a fraction, and a trapped share given by hand is short in full
      and placed nowhere;
    - a water ice with no cold-trapped part on a body whose zonal maximum is below its threshold
      is placed by T13.d's own rule;
    - on the Moon-like input the field's Σ Ω f ÷ Σ Ω lies within 5% of plan 14's `f_ct` for the
      same parts, for water and for CO₂;
    - the split: 0.02 of water ice, 10⁻⁵ of it cold-trapped, lays its zonal part on T13.d's
      coldest cells and its traps in proportion to the fraction outside them, the total 0.02 to
      one cell;
    - nesting: with water's and CO₂'s traps, each cell names one ice, water's drawn area matches
      its target to one cell, and CO₂'s drawn plus undrawn matches its own;
    - covered: a cold body's CO₂ shares on its zonal water sheet are undrawn, and none lies on a
      free cell its fraction does not give;
    - a second `place_ices` (R09.T14.d's) leaves every cell's ice to the bit;
    - a cell and its mirror across the equator take equal fractions;
    - the airless frost points of H₂O and CO₂ are `trap_threshold`'s to the bit.
  - _Acceptance:_ `cargo test -p hyperion-sim planetary::surface::steps::climate` and
    `cargo test -p hyperion-sim planetary::surface::inputs` (the §7.2 tests and the tripwire).
- **R09.T13.g Relief in the energy balance** (after T13.a, before T13.d, which takes its lapse rate;
  decision-r09-t13a-relief). `steps/climate/ebm.rs` and one lapse-rate function. Each model grid
  point holds its surface heights above the sea level (the water's surface over water) as eight
  equal-area quantiles of the field cells whose centres it holds, a point holding none taking the
  nearest point's, equatorward first. The prognostic temperature stays the sea-level one, T, and
  the transport is unchanged. The albedo and the ice heat capacity read each quantile's ground
  temperature T − Γz, and the outgoing radiation is σT⁴ ÷ (1 + ¾τ) − e^(−(3/2)τ_z) σ(T⁴ − (T − Γz)⁴)
  with τ_z = τ e^(−z ÷ 2H), plan 14's τ ∝ √p and H the air's scale height at plan 14's mean, each
  averaged over the quantiles. The sea's ice heat capacity reads the water's own surface,
  T − Γ z̄_w, z̄_w the point's flooded cells' mean water-surface height above the sea level. The
  outgoing radiation is never below the ground's emission transmitted through the column above it,
  e^(−(3/2)τ_z) σ(T − Γz)⁴. A point holding no cell takes the cell holding its centre (T13.b's
  rule), and the zonal model's band the nearest band's, equatorward first (T13.a's). τ's zonal
  solve takes each band's quantiles over all its cells; where its final probe's steady state has
  not converged, it takes the nearer, by surface mean, of the last converged probes on each side
  (decision-r09-t13ge). Γ is plan 14's ground gradient, α g ÷ c_p(T_s) (`structure::lapse_factor`),
  the one lapse rate T13.d writes to the header; the reference Earth's air carries its water
  vapour. τ is solved so that the surface mean, of T − Γz, is plan 14's. Tests:
  Records: [09-surface-generator/records/R09.T13.g.md](09-surface-generator/records/R09.T13.g.md).
  - with every height zero, T13.a's output bit for bit;
  - the reference Earth with its land's hypsometry by hand:
    - the set-up: Earth2014's SUR2014 at 5′, eight equal-area quantiles a band; each band's land
      cells at those heights in order of their 2.5° × 5° box's Earth2014 median land height, its sea
      flooded to the sea level; Natural Earth's land by box, its band totals `EARTH_LAND_SHARE`'s
      (decision-r09-t13e-asbuilt);
    - it lies within 5 K of Siler et al.'s Fig. 2b from 58.75° S northward, within 8 K from
      73.75° S to 61.25° S and within 11 K at 76.25° S, every band printed;
    - its 76.25° S and 73.75° S bands are each at least 1 K colder than the same Earth with every
      land cell at the sea level;
    - its τ solve's steady state has converged, with τ, the steady and seasonal surface means and
      the mean constant printed;
  - a band half at sea level and half 3 km up is brighter than one wholly at their mean height
    where that height's ground lies above the albedo's midpoint, and darker below it;
  - the heat capacity of a point half sea and half a 3 km plateau, just above the freezing point at
    sea level, carries the open sea's capacity on its sea share;
  - with no transport and τ = 0, a column's ground temperature at a given sunlight is the same at
    every height;
  - the outgoing radiation is positive and rises with T at every quantile, and is never below the
    ground's emission transmitted through the column above it; a τ = 10 column four scale heights
    up takes that floor.
  - _Acceptance:_ `cargo test -p hyperion-sim planetary::surface::steps::climate`.
- **R09.T13.h The thin air's slab and frost** (after T13.d, in any order with T13.e–g and T13.j;
  decision-r09-t13-thin-air §1.8–§1.10, as decision-r09-t13h-scope amends them, whose specification
  and tests bind). R09.T13.c's thin air revised: the balanced bounded slab with its snowfall.
  Records: [09-surface-generator/records/R09.T13.h.md](09-surface-generator/records/R09.T13.h.md).
  - _The slab's column_ c is the site on the whole-days clock, each phase column on the true-time
    clock and each point on a world locked 1:1. F̄_{c,k} is the previous orbit's mean of εσT_s⁴ over
    column c's samples in month k; in true time column p reads the previous orbit's column at the
    same hour angles, p + N frac(D) (mod N), interpolated linearly between the two either side of a
    fractional turn, a turn within `COMMENSURATE_COLUMNS` of a whole number taken whole. On the first
    orbit F̄_{c,k} = 2(⟨S⟩_{c,k} + E_nf) ÷ (2 − εβ), ⟨S⟩ the site's month mean (T13.c's input) on
    whole days and on a locked world, and the phase column's own month mean in true time.
  - _The slab_ σT_a⁴ = max(F̄ ÷ 2, σT_fa⁴), T_fa the bulk gas's saturation temperature at x_b p_s ÷ e
    from the registry's phase diagram (Kieffer 2013, [46]), absent where `Frost::of` finds no frost.
    The back radiation is βσT_a⁴, of which the surface takes ε. The snow, β(2σT_a⁴ − F̄) ÷ L, zero
    unless the slab is bounded, joins column c's frost at every step of month k over the step's true
    duration, frost or none on the ground, before the frost's balance; a step whose frost (its snow
    included) the step's energy more than sublimes re-solves its surface with the frost's latent
    heat over the step's true duration taken off, so that no remainder is lost
    (decision-r09-t13h-scope §2).
  - _β_ = (2 ÷ ε)(1 − (⟨S⟩ + E_nf) ÷ (σT\*⁴ + E_nf)), held to [0, 1].
  - _The closed form_ (a world locked 1:1 in a one-month year, or one with no seasonal orbit), with
    E_in = S + εσT_CMB⁴ + Q, F = εσT⁴, A = σT_fa⁴ and B = εσT_f⁴, each of A and B absent where
    `Frost::of` finds no frost, is the march's own steady state under constant light, with the snow
    sublimed as the march's is (decision-r09-t13h-scope, "§1.9 item 5 amended", which replaces
    decision-r09-t13-thin-air §1.9 item 5):
    - F = max(B, G), G being the bare column's balance: E_in ÷ (1 − εβ ÷ 2) where E_in ≥ E_hi =
      (2 − εβ)A, the slab unbounded (everywhere, where there is no bound); (E_in − (2 − ε)βA) ÷
      (1 − β) below E_hi, the slab bounded and its snow, β(2A − F), sublimed by the ground; at β = 1
      this branch is empty, and below E_hi the column is held at B;
    - where F = B the column is held at T_f and its frost grows, as an endless reservoir does;
    - the back radiation is R⇓ = β max(F ÷ 2, A), so a frost-held night under the bound takes
      βσT_fa⁴;
    - the form is continuous at E_hi and at E_lo = (1 − β)B + (2 − ε)βA, where G meets B; at β = 1
      it steps from B to 2A at E_in = (2 − ε)A.
  - The spin-up starts each column at its share of that closed form's temperature under its
    annual-mean sunlight. Convergence is the existing 0.1 K stop and `MAX_ORBITS`. The slab's level
    is p ÷ e.
  - No contract moves (`GENERATOR_VERSION` 22): the frost stays internal, and nothing is added to
    the payload or the record (decision-r09-t13h-scope §6.2).
  - _Tests_ (decision-r09-t13-thin-air §1.10; decision-r09-t13h-scope §8 (a)):
    1. **The polar-night slab** (reference Mars): in each pole's coldest month the slab lies at the
       bulk gas's frost point at x p_s ÷ e, the registry's figure, to 0.5 K (141.1 K by James et
       al.'s curve), and in 135–150 K; no column's slab in any month lies below its bound.
    2. **The balance limit**: on a frost-free column under constant sunlight in the closed form
       T_a⁴ = εT_s⁴ ÷ 2 and εσT_s⁴ = 2(S + E_nf) ÷ (2 − εβ) to 10⁻⁹ relative; with Q and T_CMB set
       to 0 the form is T13.c's closed form to 10⁻¹².
    3. **Energy.** Over the converged orbit, ⟨S + εσT_CMB⁴ + Q⟩ = ⟨(1 − β)εσT_s⁴ + (2 − ε)βσT_a⁴⟩ +
       ⟨P_store⟩ − L ΔM ÷ P + β⟨εσT_s⁴ − F̄⟩ to 10⁻⁶ of ⟨S⟩.
       - Each step is weighted by its true share of the orbit, each phase column by 1 ÷ N and each
         grid point by area.
       - P_store is each step's Σᵢ sᵢ (Tᵢ − tᵢ), its own implicit storage. ΔM is the orbit's change
         in the grid's frost, snow included.
       - The stored power and the slab's lag are printed, and each is asserted below 10⁻³ of ⟨S⟩.
       - It runs on the reference Mars, on the 3:2 Mercury with the reference Mars's air, and on a
         seasonal locked world with that air (decision-r09-t13h-scope §3).
    4. **The seasonal caps** (reference Mars): the orbit's swing in Σ frost × area over p_s A ÷ g,
       at the inertia and albedo as built, lies in [0.18, 0.40]; it is printed against [0.20, 0.32]
       and 0.244, the tight window being the calibration's acceptance, not this task's; the south's
       peak exceeds the north's; every column's frost is periodic over the converged orbit to
       1 kg m⁻².
    5. Printed: both poles' CO₂ clearing Ls, against 70–100° for the north.
    6. **Any thin air**: a β = 1 thin-air world (the reference Mars with plan 14's mean raised until
       β saturates) converges within `MAX_ORBITS`, each orbit's move and the band that moves most
       printed, and its polar-night total condensation, surface plus snow, lies within 10% of the
       slab-free column's; if it does not converge, its last move is pinned (below 0.2 K) and
       recorded, and the stop, the cap and the snow rule do not change (decision-r09-t13h-scope §4);
       a Pluto-like N₂ input converges, with its slab at N₂'s bound in its night; an air whose bulk
       gas has no diagram takes no bound and no snow, its polar-night slab (ε⟨T_s⁴⟩ ÷ 2)^¼; the
       lanes test (the reference Moon and Mars, one column at a time and in eights) stays exact to
       the bit with the slab's state.
    - The true-time map: the column it reads is the one `absorbed` lights identically one orbit
      back, for N frac(D) = 12 and 11.5, and each band's ring sum of F̄ equals the previous orbit's
      to 10⁻¹² (decision-r09-t13h-scope §7.2).
    - The closed form (decision-r09-t13h-scope, "§1.9 item 5 amended"): a thin-aired world locked
      1:1 (the reference Mars at e and obliquity 10⁻⁷, against both at 0) marches every point to
      within 0.1 K of its closed form, as the airless one does; G is continuous at E_hi and E_lo to
      10⁻⁹ relative, every bare column's books close, E_in = (1 − β)F + (2 − ε)β max(F ÷ 2, A), to
      10⁻¹² relative, and at β = 1 the band is empty, F stepping from B to 2A at (2 − ε)A.
  - _Acceptance:_ `cargo test -p hyperion-sim planetary::surface::steps::climate`, and the surface
    module's slow tests by name.
- **R09.T13.j The land's water** (after T13.e and T13.f, before R09.T14–T16;
  decision-r09-t13e-revision, whose specification and tests bind). R09.T13.e's revision for its
  finding E-P1, land-heavy midlatitudes too dry. The sea rains its own evaporation plus its band's
  convergence, a divergent band's export falling on its sea alone; each land cell takes the zonal
  budget's convergence as a vapour source, scaled by e^(−H/H₀); the orographic model carries the
  sea's air along the model's bands by the recorded wind's component along them; frozen ground
  recycles nothing, and the annual runoff counts each month's ε; the wholly calm month's rule goes.
  The winds' first Ferrel node stays 35.4° (the ruling's follow-up 2 withdraws its reread). No
  contract moves (`GENERATOR_VERSION` 22). It comes
  before the proposed R09.T13.i, which re-runs its assertions. Its tests are R09.T13.e's above, as
  the ruling's plan text (b) amends them. _Acceptance:_ the ruled Earth test's northern ratio
  (40–60° N over 15–35° N) at least 1.10 (1.05–1.10, or a failed new assertion, reported to main);
  the reference Earth's climate step at level 8 at most 30 s; E-L8's generated sample at most 20
  restarts a month with every deep comparison within 1%;
  `cargo test -p hyperion-sim planetary::surface::steps::climate`; and the surface module's slow
  tests by name, among them `the_coarse_pass_s_reference_earth_s_water_cycle_against_gpcp` and
  `the_reference_earth_s_pass_is_order_independent`.
  Records: [09-surface-generator/records/R09.T13.j.md](09-surface-generator/records/R09.T13.j.md).

- **R09.T13.i Ocean heat transport and the energy balance's next revision** (after T13.j, before
  R09.T16; decision-r09-t13i, with its §2.4 follow-ups 1–3, its §2.4 follow-up 4 (cost), its
  §2.2/§2.3 follow-up 5 and its §2.4 follow-ups 6–10, and decision-r09-t13i-cl-licence, whose
  specification and tests bind). In
  two batches, each with its own gate. No contract moves (`GENERATOR_VERSION` 22); no golden pins
  the climate.
  Records: [09-surface-generator/records/R09.T13.i.md](09-surface-generator/records/R09.T13.i.md).
  - _Batch 1, the energy balance_ (§2):
    - the liquid's heat: an additive, internal saturated-liquid specific heat on `LiquidColumns`,
      each liquid's reference equation of state evaluated in the registry
      (decision-r09-t13i-cl-licence), and the sea's capacity ratio r_l = ρ_l c_l ÷ (ρ_w c_w) at the
      freezing point + 10 K, water's exactly 1;
    - R1 and R2: τ(T, z) = m [τ_d e^(−z ÷ 2H) + τ_w(T − Γz)], τ_w = k_w √(p_w ÷ 1 bar), plan 14's
      water term under the rules in force at the local saturation where liquid water lies at the
      surface and at the lesser of that and the air's own water elsewhere, every partial pressure a
      column weight as plan 14's law takes it (follow-up 2 §4, follow-up 5 §2), m solved per body as
      τ was;
    - O1: the Ekman cells inside the Hadley cell (Held 2001), their gross oceanic stability the
      walls' blend of Held's eq. 7 in a walled basin and the whole contrast to the return water in
      an open ocean (follow-up 5 §1), and the gyres' and eddies' diffusion (Rose and Marshall 2009's
      constant, scaled by rotation, walls, depth and liquid);
    - the mean's correction: a search for u on the zonal model's seasonal runs under a 0.01 K stop,
      its secant runs started from their pair's secant predictor, then two geographic runs: G1 at
      its answer, a measurement of d that ends at a probe stop where that comes before the
      climate's, once its surface mean's own last two ratios agree and its remainder is at most
      0.05 K, and G2, always, at one defect step on the search's own slope, started from G1 plus
      the predicted change; G2 kept, G1 only as follow-up 3's fallback (follow-ups 3, 4 and 6–9);
      each run's slow mode extrapolated while its remainder is at least the bound, with a reject
      rule and at most two rejects (follow-ups 4, 6 and 7); the convergence stop on the move and its
      extrapolated remainder, after a jump at the larger of the fresh ratio and the jump's; and the
      transport's pressure ramp below 0.2 bar.
  - _Batch 2, the water cycle_ (§3): the summer side of the Hadley share, and the sea's evaporation
    in Priestley and Taylor's yearly amount with the Penman split's monthly shape, reading batch 1's
    ocean heat. Built 2026-10-20 (lane r09-water; _Deviations in T13.i batch 2, as built_). With
    it, E-P8's northern half is met and E-P11 is met as decision-r09-t13i §3.4 amends it. Its
    remainder, the land store, is I1, which stands before R09.T15's s, w and f letters.
  - _Tests, batch 1_ (§2.5, as the follow-ups amend it), on T13.g's relief Earth with
    `synthetic_sea`'s floor at the reference Earth's mean ocean depth, 3,689 m:
    1. ERA-Interim (Siler et al.'s Fig. 2b): within 5 K from 58.75° S northward, within 7 K in the
       ice edge's tier from 61.25° N to 71.25° N (until I2's cloud revision, then 5 K), within 3 K
       within 23.75° of the equator, within 6.5 K from 73.75° S to 61.25° S and within 9 K at
       76.25° S, every band printed beside its bound; 76.25° S and 73.75° S each at least 1 K colder
       than the same Earth with every land cell at the sea level run at the relief Earth's kept u
       (its search's first run continued there under the stop), printed with both runs' u and
       means, and reported instead where that run's mean falls more than 5 K below the relief
       Earth's. T13.a's sea-level test holds the profile's shape: each band's departure less the
       solid-angle mean departure within 5 K from 58.75° S (7 K in the ice edge's tier), the mean
       departure within 3 K, the absolute departures and the mean printed, its south printed;
    2. the transports beside Trenberth and Caron's table: each hemisphere's total peak in [4.2, 6.5]
       PW, the ocean's peak in 5–25° N in [0.3, 1.2] and in 5–25° S in [−1.2, −0.3] (printed beside
       Trenberth and Caron's and Klinger and Marotzke's shallow cells), at 47.5° N in [0.15, 1.0]
       and at 47.5° S in [−0.6, 0.1], the atmosphere's share at 35° N in [0.70, 0.95] and at 35° S
       in [0.80, 1.0]; an aquaplanet's ocean peak printed beside Enderton and Marshall's 2.4 PW;
    3. the no-ocean limit: the dry world, the all-land Earth and the relief Earth with no sea are the
       model without the ocean term to the bit; Kaspi and Showman's fit and sweeps are unchanged;
    4. R1: τ(T, 0) = m (τ_d + τ_w(T)) to 10⁻¹² on a frost-free column; a world with no water in its
       gases has one τ; at z = 0 the quantile emission is the sea-level column's to the bit; a 330 K
       aquaplanet converges with every record finite; a world with water in its air and none at its
       surface converges, its τ_w the air's water's above its crossover and the saturation's below,
       to 10⁻¹², and a world with liquid water is unchanged to the bit; both versions' greenhouse
       rules;
    5. the liquid: a methane sea's capacity ratio within 2% of the reference equations' ρ c_p ratio
       at its states (Setzmann and Wagner's, over IAPWS-95 as NIST's WebBook evaluates it), water's
       exactly 1, the Titan-like world's step runs;
    6. the stop: an Earth on an orbit of 0.1 year runs until its remainder is under 0.1 K, or flags,
       its moves and the runs' orbits printed;
    7. the ramp: f_p(0.1 bar) = ½ f_p(0.2 bar), f_p(0) = 0, continuous at 0.2 bar;
    8. the correction: the ruled Earth's geographic surface mean (level 7) before T13.d within 0.3 K
       of plan 14's mean (0.3–0.5 K reported; above, it blocks), each run's u, mean, orbits and
       convergence and T13.d's c printed, and the coarse pass's reference Earth's in the slow
       coarse-pass test; on the relief zonal Earth the fit is labelled a fold, the search's slope
       step lies nearer the mean than its first run, no unconverged run is a secant point and the
       answer lies within 0.3 K (or the test prints and the lane reports); the model keeps the
       converged run nearest the mean (follow-up 3), and the continent aquaplanet's test, as
       written, keeps a converged run and prints its runs and wall time;
    9. the spin-up and the search (follow-up 4 §7): the extrapolated run within 0.2 K of the
       marched one on the relief zonal Earth, with the same outcome; a rejected jump restored to the
       bit; two calls equal to the bit; on the deep all-sea aquaplanet the search's rules (follow-up 8
       §2): every secant point met the search's stop, every predicted start its pair's linear
       prediction, the answer the nearest converged run, the earlier on a tie, two calls equal to
       the bit, its runs printed, no run count or miss asserted; G1's probe stop (follow-ups 8 §1
       and 9 §2): on a geometric sequence of means (ratio 0.9, 1 K from its limit) it ends at the
       first orbit with R̄ ≤ 0.05 K and T̂ is the limit within 10⁻⁹ K, on two modes (changes
       0.3ⁿ + 0.02 × 0.9ⁿ K) it waits for the mean's ratios to agree, ending at orbit 13 with T̂
       within 3 × 10⁻⁵ K, a G1 that never meets it makes G2 run A, and on the coarse Earth its d
       lies within 0.05 K of the converged G1's −0.438 K and the kept G2 within 0.1 K of
       287.969 K;
       the defect step on a hand-built pair; and the walls' blend (follow-up 5 §1): Held's eq. 9 on
       his profiles, χ = 1 and χ = 0 each regime to the bit; the models (follow-up 7 §3): on the
       all-land and the all-sea world with no ocean term, the zonal and the geographic model from
       the same broadcast steady state at the fit's u, three orbits each, every point's annual and
       monthly means within 10⁻⁶ K; the conservation test (follow-up 6 §4): energy and the mean as
       before, `Folded` and g4's choice among the converged candidates;
    10. unchanged and re-run: the 60° aquaplanet and its Ω⊕ ÷ 8 twin, the locked worlds, the
        Titan-like and dry worlds, Mars, Venus and the one-month world, T13.d's tests, both order
        tests, T13.j's ruled Earth assertions and the slow coarse-pass test.
  - _Tests, batch 2_ (§3.4–§3.5): the partition on NCEP R1's observed temperatures (its 40–60° N
    convergence positive every month, its JJA mean at least 0.6 mm/day, its annual E − P at most
    −0.6); on the ruled Earth the 40–60° N sea's DJF ÷ JJA evaporation at most 6 (6–7.5 reported,
    above it blocks) and its all-land JJA at least 0.45 of its DJF (0.35–0.45 reported, below it
    blocks), with §3.4's prints; T13.j's assertions, batch 1's ERA and transport tests, E-L8's
    sample and the slow coarse-pass test re-run; the Hadley share's and the evaporation's unit tests
    (§3.5). §3.5 2's third and sixth bullets, "with no storage and a constant temperature every open
    month is equal" and "`Q_O` = 0 and G = 0 give PT's rate in every month", hold as written only
    where the months' forcing is uniform, since the ruled form spreads the aerodynamic part by
    `q_s`, not by the energy: both are built as one test on uniform months (_Deviations in T13.i
    batch 2, as built_). decision-r09-t13i's 15–35° N GPCP − NCEP +1.43 is the data's +1.42, and its
    bracketed "NCEP's own" annual E − P figures are GPCP less 0.944 × NCEP R1's evaporation; NCEP's
    own 40–60° N figure is −0.52.
  - _Acceptance:_ T13.j's ruled Earth assertions holding (batch 1 lands alone if its northern ratio
    stays at least 1.10); the reference Earth's climate step at level 8 at most 30 s on a quiet
    machine (load ≤ 2), one thread; 30–35 s lands with its figure and each lever's saving recorded,
    over 35 s holds (follow-up 10 §1, restating follow-ups 4 §2 and 6 §3 for R09.T13.i); `cargo test -p hyperion-sim planetary::surface::steps::climate`;
    and the surface module's slow tests by name.

Offline check (recorded, not in CI): the reference Earth's monthly fields against an ExoPlaSim run
of the same inputs, with the differences written into this plan. Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::climate`.

### R09.T14 Erosion

Records: [09-surface-generator/records/R09.T14.md](09-surface-generator/records/R09.T14.md).

- **R09.T14.a Drainage.** After R09.T7.i, so that the coarse craters are depressions. Random
  receivers on `surface.coarse.erosion` with the slope correction; the depth-first order; drainage
  area by solid angle; priority flood with pop-order flats and Fill–Spill–Merge. Tests: drainage
  area at every outlet sums to the draining area; no cell drains uphill after filling; the same seed
  gives the same receivers; a finite inventory fills the lowest basins first and spills.
  _Built 2026-10-20_ (Risks, "Deviations in T14, as built", _T14.a_): `d29a0fc9` on lane
  r09-erosion, with decision-r09-t14-choices Q2's and Q3's measures; the plan's four tests and
  eleven more.
- **R09.T14.b The solver.** After T0.b's erodibility item. Tzathas et al.'s recursion with n = 1,
  the fixed point with its moving
  average, multigrid to level 4 at six iterations a level with the upsample through T4's
  interpolant; K scaled as Design note 9 says with m = 0.45 and the runoff-weighted area (T0.b's
  erodibility: K⊕ converted from Tzathas et al.'s m = 0.4, linear in g and ρ_f, the bed factor B,
  runoff once). t is `effective_flow`, in Earth-equivalent years at the reference intermittency of
  5% (Design note 9), and K carries no intermittency factor of its own. Tests: a ridge-to-sea
  profile matches the closed-form steady state, every link below the threshold slope equal to the
  uncapped sum to the bit and every link at it rising exactly s_c δ, the profile within 1% of the
  closed form wherever x ≤ 0.9 L (decision-r09-t14-choices, T14.c follow-up 2 §5); over a step short
  enough that slopes barely change, a profile's lowering far from base level is linear in t and in
  runoff^0.45 (to 1%), and the saturated-regolith K at a uniform 0.1 m a⁻¹ for 2.2 × 10⁵ yr gives K
  (runoff ÷ 1 m a⁻¹)^0.45 t = 1.9 m^0.1 to 2%; K is 6.3 × 10⁻⁶, 2.4 × 10⁻⁵ and 3.1 × 10⁻⁶ m^0.1 a⁻¹
  (to 2%) for an Earth's rock under water, a saturated Mars's regolith and a Titan's ice under
  methane. K's bed factor B comes from a (substrate, fluid) table over the registry's rows, and ρ_f
  is the liquid's density. The three calibrated cases (rock, saturated regolith, ice under methane)
  stay its tests (decision-composition). The equivalent area, ∫ r dA ÷ (1 m a⁻¹) (Hergarten 2021,
  eq. 16 and its accumulation, eq. 20), is the solver's working state only; T14.d writes the
  geometric area and the two runoffs (science-r09-drainage.md §1).
  The multigrid's test (decision-r09-t14-choices, T14.b follow-up, with the half and the held cells
  of its T14.c follow-ups 2 §3 and 3 §1): on the reference Earth (slow; and the synthetic worlds)
  the fixed point's residual falls within each multigrid level, its last iteration's below half
  its first, and the final elevation is the solver's solution on its last routing, every unbound
  cell with positive uplift at or above its receiver on it and each cell held at its construction
  below its receiver counted apart; each level's first and last residual (the root mean square by
  area over the cells not bound), its largest change and the receivers that changed between its
  last two routings are recorded. The residual is not monotone over the levels: a fixed point of
  random receivers settles on a floor where receivers flip, which grows with the level. Findings
  to main, recorded in the slow tests and not asserted: on the reference Earth at level 8, a final
  RMS residual on the solve above 125 m, a level failing the within-level test, or a gradient
  above 0.2 between land neighbours on the final field (its largest change recorded, no longer a
  trigger); on the reference Mars, a final RMS residual above half the RMS of the solver's
  unscaled channel incision over the cells at full exposure, which then takes the pre-ruled single
  solve on the construction's routing (T14.b follow-up §5; T14.c follow-ups 2 §3 and 3 §T;
  decision-r09-t14-mars-ceiling §4.2).
  _Built 2026-10-20_ (Risks, "Deviations in T14, as built", _T14.b_ and _The fixes after T14.c_):
  `fc121acb`, its level start and receivers revised in `320fe222`, its uplift and threshold in the
  fix `7347836a` and its remap's frame in the fix `a8a25ee0` (lane r09-erosion;
  decision-r09-t14-choices Q1 and its T14.b and T14.c 1–3 follow-ups): the three K within 0.4%, K t
  1.866 m^0.1 on the reference Mars, the residual test restated to the fall within each level.
- **R09.T14.c The branches and the Mars check.** The wet-now, dry-now and never-wet branches,
  after T6.b. On the dry-now branch the drainage area is weighted by the epoch's `runoff`,
  uniform over the cells.
  - _Tests:_
    - Q4's test of the surfaces younger than the epoch's end, in four parts
      (decision-r09-t14-choices Q4 and its T14.d follow-up A; decision-r09-t14-mars-ceiling §4.1):
      1. a surface no older than the epoch's end throughout leaves the elevation unchanged to the
         bit, with no runoff;
      2. the waiting craters are placed untouched and are not the solver's bounds: (2a) each
         waiting crater's byte after T14.d is the erosion-off run's under the second match's rule,
         `byte_of(s*₂ × kept_relief(byte₁))` under `Rule::Whole`, otherwise unchanged; (2b) the
         final elevation equals, bit for bit, the state before `craters::run_after_erosion` plus
         each waiting crater's `each_lift` at its final byte, added in draw order; (2c) on the
         solver's own output, before T14.d's re-match, at least one cell inside a waiting crater's
         rim lies below the elevation that entered the step, the rim cells' mean lowering recorded
         beside every eroded cell's;
      3. no cell inside a waiting crater's rim has runoff of its own;
      4. every `Province` cell of the reference Mars equals its elevation entering the step, bit
         for bit, with no runoff of its own; and a builder world with `province_surface_age` at or
         above its epoch's start erodes its provinces (some `Province` cell lowered), their bed
         from their own crater density, and they meet (B) as every eroded cell does (printed, not
         asserted, as built: Risks, "Deferred findings (2026-10-13 rule) (R09.T14)").
    - A never-wet world's elevation is unchanged by the step.
    - The reference Mars's wet epoch is `effective_flow` 2.2 × 10⁵ yr at `runoff` 0.1 m a⁻¹ (K t =
      1.9 m^0.1, Black et al. 2017's; the foot of Hoke, Hynek and Tucker 2011's 10⁵–10⁸ yr, Design
      note 9).
    - On it the floor is three items (decision-r09-t14-choices F1; decision-r09-t14-mars-ceiling
      §4.1 and §4.2):
      - (A0), recorded as the plan's figure, not the solver's test: the coarse erosion volume plus
        the synthesis's expected sub-cell channel incision volume (closed-form per cell from k_s,
        the cell's two runoffs, Hack's law and the network's widths from discharge, as T6.b
        computes them);
      - (A1), asserted: the median channel incision over the eroded cells (not bound, their
        equivalent area and exposure above zero) draining at least 10⁴ km², the trunk reaches,
        lies within Black et al. 2017's 50–350 m, its distribution recorded (the trunks' p90 and
        the share deeper than 350 m); a miss is a finding with its figure, not a loosening;
      - (A2), recorded now and asserted after R09.T6.c: the coarse volume alone against Luo, Cang
        and Howard 2017's 1.2 m of global equivalent layer ((1.74 ± 0.8) × 10¹⁴ m³ over 1.444 ×
        10¹⁴ m²), at least 1.2 m once T6.c's valleys are the epoch's and the closure lowers each
        cell by their share; above 6.1 m (three times their combined 2.04 m) a finding to main.
    - (B): the coarse lowering is bounded above by the valleys' cross-sections, the 99th
      percentile by area of the eroded cells' coarse lowering at most 91.3 m, the maximum and every
      cell above recorded (incision × valley width ÷ cell width at Black et al.'s 350 m of trunk
      incision and the 10 km upper end of the networks' 1–10 km widths: 91.3 m on the reference
      Mars's 38.32 km cells; decision-r09-t14-choices F3, decision-r09-t14-mars-ceiling §4.1).
      Over Hoke et al.'s three decades a floor alone would pass any over-erosion.
    - A slow test records the final residual, the cell-mean lowering against a fine-grid run of
      one basin, and the volume at the window's extremes, 10⁵ yr at 0.073 m a⁻¹ and 10⁸ yr at
      1.10 m a⁻¹.
    - On the final field: (i) every reference world's realised relief, the cells' range after the
      whole pass, lies within T12.e's 6–16 σ_h; (ii) on every world wet now the remapped set's
      highest cell above the sea is the construction's highest over the same set, to the pass's
      lowering, no land cell lies above the construction's land maximum, and T12.a's land bands
      hold; (iii) every unbound cell with positive uplift lies at or above its receiver on the
      solve's last routing, and each cell held at its construction below its receiver is counted
      and recorded apart as a closed depression (decision-r09-t14-choices, T14.c follow-ups 1–3).
  - _Built 2026-10-20_ (Risks, "Deviations in T14, as built", _T14.c_): `a6c551b4` (the wet
    epoch's runoff and liquid, landed) and `320fe222`, with the fixes `7347836a` and `a8a25ee0`
    (lane r09-erosion; decision-r09-t14-choices Q2, Q4–Q6a, F1–F3 and the T14.c follow-ups 1–3;
    decision-r09-t14-mars-ceiling): the floor restated as (A0)–(A2), (B) asserted as the area p99
    at 91.3 m, Q4's test in four parts; on the reference Mars (A1) 187.8 m, (B) 88.28 m, (A2)
    2.226 m.
- **R09.T14.d Rescale and outputs.** In decision-r09-t14-choices Q7's order: σ_h re-matched on the
  reconstructed field, counting the waiting craters and folding their bytes, never stretching them;
  the sea placed again at the same ocean fraction; on a world wet now the lakes by their water
  balance outside the ocean (Q3); the lapse and mean re-applied to the final water surfaces and ℓ,
  with the ices and liquids placed again; then the outputs: flow directions, the geometric drainage
  area (T14.a's), k_s = S A^θ on it, each cell's `runoff` (the source term the solver took: the
  climate's annual runoff on a world wet now, the epoch's `runoff` where the surface is older than
  the epoch's end on one dry now, none elsewhere) and its `catchment_runoff` (the equivalent area
  accumulated over the field's final routing, over the geometric, times 1 m a⁻¹), and water surfaces
  (science-r09-drainage.md §1).
  - _Tests:_
    - σ_h and the mean temperature again match;
    - lakes are level;
    - the quantised catchment runoff times the drainage area equals the equivalent area over the
      field's final routing times 1 m a⁻¹ to the two codes' steps;
    - k_s × drainage^−θ is the slope to the receiver to the steepness code's step;
    - on the reference Mars every eroded cell's own runoff is its epoch's 0.1 m a⁻¹, and its
      catchment runoff equals it wherever its whole catchment is older than the epoch's end;
    - every cell younger than the epoch's end (every cell whose centre lies inside a waiting
      crater's rim, or every cell where the surface age is at most the epoch's end, or every
      `Province` cell where the provinces' surface age is at most the epoch's end) has no runoff of
      its own;
    - on the reference Earth some cell's catchment runoff is at least three times its own.
  - _R09.T8.d's certificate:_ on a body of surface pressure exactly 0 Pa every cell keeps its
    runoff and catchment runoff none, ice cells included, which `CoarseField::new` enforces
    (`BuildFieldError::RunoffWithoutAir`) and the level bound's zero channel terms count on. A
    change that lets such a body carry runoff (an atmosphere lost since a wet epoch, lava
    channels) replaces that certificate with a header figure in the same task.
  - _Built 2026-10-20_ (Risks, "Deviations in T14, as built", _T14.d_, _The merges_ and _The
    review_): the batch's last commit (lane r09-erosion, after the merges `9ddb2f55` and `be163ce5`;
    decision-r09-t14-choices Q3, Q6b, Q7, its T14.d follow-up and "T14.d lakes"; the review of
    2026-10-20, no blocker, its fixes applied): the re-match, the sea, the lakes, the climate and
    the outputs in Q7's order, and the final routing again after the late craters; σ_h by one
    re-match, the reference Mars's whole pass 1.754 × 10⁻⁴ above plan 14's and asserted at 2 × 10⁻⁴
    under the owner's FEATURES FIRST of 2026-10-20, the ruled predicted-record iteration skipped and
    recorded, and a world wet now asserted at 10⁻⁴; a full lake no bound in the wet-now solve, and
    each cell's net loss in the lake balance (E − P) + r, r its own runoff (the review's M1); the
    catchment runoff over the field's final routing; the bench `coarse/erosion_l8` 10.504 s per
    `erosion::run` on the reference Earth at level 8, the solve about 2 s of it. The lakes stay a
    finding, which main records in deferred-corrections.md: the reference Earth's lakes covered
    14.1% of its land before "T14.d lakes", 6.16% after it and 7.04% after the first merge and M1
    (the merged climate alone 6.97%, M1 0.07 points more), and 6.68% after the second merge
    (R09.T13.i's sea evaporation, which `NetLoss` follows), against Verpoorter et al. 2014's about
    3.7%.

Bench `coarse/erosion_l8`: the brainstorm's "a few seconds at level 8" (Tzathas et al.'s Table 2,
1.79 s at 512² and 8.18 s at 1,024² in Python with numba), recorded, in a new
`crates/hyperion-sim/benches/surface.rs` (`[[bench]]`, `harness = false`), which T16 extends.
Acceptance:
`cargo test -p hyperion-sim planetary::surface::steps::erosion`.

### R09.T15 Classes and crater state

Records: [09-surface-generator/records/R09.T15.md](09-surface-generator/records/R09.T15.md).

`steps/classes.rs`: Köppen–Geiger by Peel et al.'s Table 1 with Beck et al.'s rules, applied to
rates with each cell's own summer (Design note 11); surface-state classes for the other regimes and
for one-month worlds; each naming its palette entry (codes 64–255), with melt per cell;
Köppen–Geiger 1–63 (decision-composition); the per-cell crater state. The codes, the order of
decision and the crater state are decision-r09-t15-classes's (2026-10-21): Köppen–Geiger 1–30 in
Beck et al. 2018's legend order, 31–63 open; the forms 64 Sea, 65 Lake, 66 FrozenSea, 67 Ice,
68 Frost, 69 Melt, 70 OrganicSediment, 71 Regolith and 72 Rock, 73–255 open, each of the entry
the form names (§3); decided Melt (above a non-ice crust entry's transition, or a `MagmaOcean`),
then under liquid, then Köppen–Geiger on a seasonal water cycle's land, then ice by its quantised
byte, the organic cover under an atmosphere, regolith under 30 kPa and rock; and the crater state
a log code of the lunar-equivalent N(>1 km) the cell's own surface has seen, eight codes an octave
with 1 km⁻² at code 216, code 1 none retained (§6). Tests: the classifier given
one monthly climatology labelled once as a one-year orbit and once as a four-year orbit (the same
temperatures, and precipitation at the same rates per 30.4375 d) returns the same classes, which
tests the rate rule alone, not the climate a longer orbit would have; on an orbit of e = 0.6, four
months above 10 °C centred on periapsis count as 2.0 twelfths of the year, so they do not meet a
four-month threshold; the reference Earth has tropical, arid, temperate, continental and polar
classes in plausible areas (each group's share of the land within [0.5×, 2×] of Peel et al.'s, and
the tropical land nearer the equator than the polar, on a slow test run by name; the s, w and f
letters and B's seasonal offset printed until I1); a lifeless world has no vegetation class; the
reference Moon is regolith throughout, its substances its crust's entries.
`crates/hyperion-sim/examples/surface_map.rs` (the sim's first example, with an `[[example]]` entry
carrying `required-features = ["testing"]`) writes the reference Earth's elevation, class and flow
as equirectangular PPM images under `target/`, with no new dependency, and the look (belts along
convergent boundaries, rivers to the sea) is recorded in this task's entry. Acceptance: `cargo test
-p hyperion-sim planetary::surface::steps::classes` and `cargo run -p hyperion-sim --features
testing --example surface_map` writing its three images.

### R09.T16 Coarse goldens and benches

Records: [09-surface-generator/records/R09.T16.md](09-surface-generator/records/R09.T16.md).

After R09.T7.j, whose wear moves the fields it pins. After R09.T6.c too if it can, so that its
goldens pin T6.c's valleys once; otherwise they re-bless at the version in force, as the rule before
the serve gate allows (decision-r09-t14-choices F2).
`crates/hyperion-sim/tests/surface_coarse_golden.rs`, over a shared
`crates/hyperion-sim/tests/common/surface_worlds.rs` that runs `reference::*` through the pass once
per binary and that R10's world tests reuse: the quantised fields of the reference Ceres
and Moon (fast, so they run on native and wasip1 under R04's recipes) and of the reference Earth
and Mars (slow). Under nextest each test runs in a process of its own (`.config/nextest.toml`), so
"once per binary" holds within one test. The checks that share a world's pass are one test, or the
field comes from a file that one process of the run writes, as `sky::testing::shared` does. The
surface module's order tests (`planetary::surface::tests`) stay where they are: their property
needs each pass computed in several orders (decision-r09-t13-test-cost). The coarse goldens pin
each reference world's palette. On a world wet now the
coarse land hypsometry is its construction's by rule (decision-r09-t14-choices, T14.c follow-up 2
§2), so a golden or check of land hypsometry after erosion reads T12.a's bands. Bench `coarse/pass`
per level 5–8, beside T14's `coarse/erosion_l8` in
`crates/hyperion-sim/benches/surface.rs`. Bump `GENERATOR_VERSION` if T9 has not already
(whichever of the two lands first bumps, through the orchestrator; T1.b rides galaxy plan 14's
21 → 22 batch and its one bump instead), or again if the first bump has been released to a save.
Acceptance: `cargo test -p hyperion-sim --test
surface_coarse_golden`, the task's own slow tests (`just test-slow` filtered to them), and
`just bench -- coarse`; the orchestrator's `just ci` runs the fast goldens on wasip1.

_Built 2026-10-23_ (lane r09-t16, on `898a3c94`; Risks, "Deviations in T16, as built", and
"Deferred findings (2026-10-13 rule) (R09.T16)"). `GENERATOR_VERSION` stays 23 (main's ruling):
six new goldens at header 23 pin the existing output of the reference Earth, Mars, Moon and Ceres
and of two new sample worlds, `reference::europa_like` and `reference::locked_earth_like`, from the
sim's library (`planetary::surface::coarse_golden`, after T9's precedent), R09.T6.c's epoch codes
and R09.T7.j's degradation bytes among them; `tests/common/surface_worlds.rs` gives R10 its
`sample_worlds()`, which `tests/surface_coarse_golden.rs` ties to the goldens; and
`benches/surface.rs` gains `coarse/pass_l5` to `coarse/pass_l8`. The reference Earth's pass takes
94.9 s at level 8, a finding.

- **R09.T16.a The level-8 pass's cost since R09.T14.d** (a T16 follow-up; after R09.T16, in
  either order with R09.T17.a, before R09.T20; decision-r09-pass-cost §B). No contract or golden
  moves: every lever built here is bit for bit, and `golden_diff` shows nothing.
  Records: [09-surface-generator/records/R09.T16.a.md](09-surface-generator/records/R09.T16.a.md).
  - _Attribution_, under the heavy lock on a quiet machine, one thread a pass. First
    `coarse/erosion_l8` at `032a8c5f` and at its parent. If the phase-data switch does not account
    for the erosion step's rise from 10.504 to 17.881 s within the runs' spread, the same bench
    halves `01674b8c..898a3c94`, P14.T54.a's commits that touch the climate or the reference
    worlds first (`840e319a`, `13875532`, `9f3f5e1a`, `013f2f90`, `d1271637`, `88a06096` and
    `b9a17c55`), then the range's other registry commits. Then the climate step, by a probe as
    R09.T13.i's (inserted, built and deleted), at the commit that names its rise and at that
    commit's parent. Each run splits the model, the water cycle and the rest, and records G1's,
    G2's and the zonal search's orbits. The figures go into this plan.
  - _The levers_ that the attribution names, each bit for bit and tested against the path it
    replaces, for one the saturation curve's per-call constants hoisted out of its sum. A lever
    that moves bits, such as a tabulated curve or powers by multiplication, is a generator-version
    change. It is proposed to main with its measured saving and not built here. The route back
    under 30 s (RRE(2), or Newton on the G runs; I13) stays under its own ruling.
  - _Reference figures_ (recorded, not gates): the reference Earth's climate step back within
    R09.T13.i's landed band, at most 35 s quiet, and `coarse/erosion_l8` within 10% of R09.T14.d's
    figure (10.504 s). After the levers, `coarse/pass_l8` with `HYPERION_COARSE_STEPS` is run, and
    the fast `the_pass_is_order_independent` and
    `the_pass_yields_a_valid_field_twice_with_identical_bytes` are timed quiet. If the order test
    is still over 120 s, the lane reports it with each world's share and cuts nothing.
  - _Acceptance:_ `cargo test -p hyperion-sim --lib planetary::surface::`,
    `cargo test -p hyperion-sim --features testing --test surface_coarse_golden`, the surface
    module's slow tests by name and the bench under the heavy lock.
  - _Built 2026-10-23_ (lane r09-t17a, on `71532a17`, gated merged with `762073e7`; Risks,
    "Deviations in T16.a, as built", and "Deferred findings (2026-10-13 rule) (R09.T16.a)"). The
    erosion's rise is the phase-data switch (`032a8c5f`), the curve's cost a call; the climate's is
    `840e319a`, the snapshot lane's hold deleted, under whose greenhouse τ's fit takes about 1,300
    relax sweeps a steady state where it took about 375. Both are ruled physics. Three levers, each
    bit for bit and held to the path it replaces: B4, each month's evaporation terms shared by a
    climate cell's cells at one surface temperature; `hyperion_base::math::PowBase`, `libm`'s `pow`
    with its logarithm stage shared by the powers of one base; and B3 with B9, the energy balance's
    direct path reading each saturation pressure once. The reference Earth's pass takes 57.1 s at
    level 8 (94.9 s at T16), its climate step 37.3 s and `coarse/erosion_l8` 12.08 s. τ's fit on a
    table, the one bit-moving lever ruled in, is R09.T16.b (decision-r09-climate-levers), behind
    version 24, for the lane's next agent.
- **R09.T16.b τ's fit on a table** (a T16 follow-up, after R09.T16.a and before R09.T20; a rider
  of version 24 if its commit lands before P14.T54.b.i's, else of 25; decision-r09-climate-levers).
  R09.T16.a found the climate's rise in τ's fit, whose steady states evaluate h, the albedo and
  the water term directly. R09.T13.a's record kept that path ("The steady states that set τ
  evaluate directly") so that T13.a's τ moved no bit. Each such state now takes about 1,300
  sweeps at 23's air, against 375 before. With this task the fit's 51 probes, and the zonal
  search's two slope probes on the fit's set-up, read one table built once a climate, as the
  seasonal runs read theirs.
  Records: [09-surface-generator/records/R09.T16.b.md](09-surface-generator/records/R09.T16.b.md).
  - _The table._ `Tabulated`'s lattice, spacing, Hermite reads and check, spanning the fit's
    start T\* by the relief's reach and `TABLE_MARGIN_K`, as `orbits` spans its own. It holds
    h's latent part, the albedo and `τ_w`, each with its slope, none of which depends on a
    probe's multiplier m. Where the table holds no transmission at a probe's m, the probe takes
    e^(−(3/2) m `τ_w`) and its slope from the read `τ_w` by `wet_of`. An interval is read only
    where every value read from it meets `TABLE_TOLERANCES` and `WATER_TABLE_TOLERANCES` at its
    quarter points. Outside the span, or in an interval that misses, the probe evaluates
    directly, as now. The table is a value of the fit's call, built in one order, not a cache.
  - _Unchanged:_ the isothermal start at T\*, `STEADY_TOLERANCE_K` (10⁻⁹ K) and
    `STEADY_ITERATIONS` (2,000); the 48 bisections, the fold guard, `FOLD_MISS_K` and
    `Radiation::at`. Also unchanged are the seasonal runs' tables and reads, which hold their own
    multiplier's transmission. The test helpers `steady_state` and `steady_state_over` keep the
    direct path, so Kaspi and Showman's sweeps and the transport tables' fit stay to the bit.
  - _The hold:_ `FIT_TABLE_VERSION` = 24 in `ebm.rs`, with its tripwire and the tests' switch,
    in the 24 holds' pattern (`UNLIT_MEAN_VERSION`'s). At 23 the fit evaluates directly, and
    `golden_diff` shows nothing. P14.T54.b.l deletes the hold with the others, and P14.T54.b.k
    attributes its moved values apart from P14.T53.a's water term's. If its commit misses
    P14.T54.b.i's, the hold is renumbered 25 and it rides 25. It holds nothing and is not a
    serve-gate item.
  - _Checks before the commit._ The table's arm against the direct one, both with every
    landed 24 hold in force (the 24 holds' helper). They are recorded in this plan and not
    kept, since the bump deletes the direct arm.
    - The fit, on the reference Earth, `small_earth`, `locked_earth_like`, E-L8's four seeds, and
      R09.T13.i's relief Earth, sea-level Earth and continent aquaplanet (`Folded` there):
      - each probe's side of T\* and its convergence are equal, except a `Solved` fit's probes
        within 10⁻⁵ K of T\*;
      - the label and the kept candidate's side are equal;
      - u₀ is within 10⁻⁷, and to the bit where the decisions agree, the probes being the
        bracket's midpoints;
      - the steady state's surface mean is within 10⁻⁵ K, and every band within 10⁻⁴ K;
      - each probe's sweeps are printed.
    - The table's worst departures of the transmission and its slope from the direct ones over
      the reference Earth's probes, printed, within `WATER_TABLE_TOLERANCES`.
    - The pass, on the reference Earth at level 8 and `small_earth`:
      - every discrete decision of the climate is equal: the zonal search's runs with their
        orbits, jumps, rejects and convergence; G1's and G2's orbits; the kept run; and each
        month's orographic restarts;
      - every climate record before quantisation lies within 10⁻⁵ K, and each of its rates
        within 10⁻⁶ of itself;
      - where the payload's climate records differ (the temperature, anomalies, precipitation
        and winds), each lies within one code, and those cells are counted;
      - the other layers' differing cells are counted by layer.
  - _Outcomes, ruled in advance._ A discrete decision that flips is listed with the largest move
    it causes, which must lie within its own bound: the search's stop of 0.01 K, the climate's
    stop of 0.1 K, and the orographic solve's 1%. A differing convergence or kept candidate, or
    a move beyond its bound, takes the fallback. In it, a probe whose relax on the table ends at
    or after its sweep 1,800, converged or not, is solved again directly and that answer is
    kept, and the checks are re-run. The lever lands if the reference Earth's fit at the holds
    of 24 takes at most half its direct time, timed in both arms by a probe as R09.T16.a's
    (inserted, built and deleted; one thread, under the heavy lock). Otherwise, or if the
    fallback still misses, it is dropped and its figures are recorded here, with no further
    ruling.
  - _Tests kept:_
    - two fits on the table's arm, equal to the bit;
    - a fast test that pins that arm's u₀ and steady surface mean to the bit on a small world of
      the climate's unit tests with the water term, relief and a sea, in both the native and the
      wasip1 suites (the pass runs on the server only, so wasip1 is R04's cross-target guard);
    - the hold's tripwire.
  - _Reference figures_ (recorded, not gates; the ruling's estimates): the fit about 13–14 s after
    R09.T16.a's levers and about 3–4 s on the table; the climate step about 30–35 s, back in
    R09.T13.i's landed band; and `coarse/pass_l8` about 50–55 s. The probe measures the fit, the
    climate step and the pass in both arms.
  - _Acceptance:_ `cargo test -p hyperion-sim --lib planetary::surface::`,
    `cargo test -p hyperion-sim --features testing --test surface_coarse_golden`, the surface
    module's slow tests by name, and the probe under the heavy lock.
  - _Dropped 2026-10-26_ (lane r09-t16b, on `1c27ebac`; Risks, "R09.T16.b's checks, as run", and
    "Deferred findings (2026-10-13 rule) (R09.T16.b)"). The lever was built behind its hold and
    checked in both arms, with every landed 24 hold in force. It met E2's fit checks but missed
    its pass checks. On the reference Earth at level 8, the zonal search's Z2 took 16 orbits
    directly and 17 on the table. The kept climate's records then moved by up to 2.4 × 10⁻³ K,
    and 174 of the payload's climate codes by more than one code. On `small_earth`, Z3 took 7
    orbits against 11, and its mean moved 1.075 × 10⁻² K, past the search's stop. E2's fallback
    cleared the fit's checks, but it left both passes' misses as they were, and it took the
    saving with it. So, by the outcomes ruled in advance (main's word, 2026-10-26: §3 as ruled,
    no re-ruling), the lever is dropped. No code lands and `FIT_TABLE_VERSION` is not added. At
    24, R09's fields move by P14.T53.a's water term alone. The timing was not run. The lever as
    built, its checks and their logs are kept outside the tree, in the lane's scratch
    (`r09-t16b/`).

### R09.T17 The surface service

Records: [09-surface-generator/records/R09.T17.md](09-surface-generator/records/R09.T17.md).

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

- **R09.T17.a The passes in flight** (a T17 follow-up, after R09.T16; decision-r09-pass-cost §C).
  R09.T17's deferred _Concurrent passes_, which R09.T16 measured at 408 MB a level-8 pass.
  `SurfaceService` bounds the passes it runs at once by a semaphore of `surface_passes` permits
  (`--surface-passes`, `HYPERION_SURFACE_PASSES`, in `config.rs`'s table). The default is the
  lesser of 4 and the pool's workers less one, and at least 1; 0 is refused, as `--num-workers`
  refuses it. A pass takes its permit inside its flight, before its bulk job is submitted, and
  holds it until the job ends, however it ends. A pass waiting for a permit is skipped once every
  asker has given up, as a queued pass is, and a prefetched pass waits as a requested one does. So
  at most four level-8 passes, about 1.6 GB, run at once, and on a pool of two workers or more one
  worker stays free for the interactive jobs that the displays' requests wait on. No field,
  payload or protocol moves.
  - _Tests:_ with one permit on a two-worker pool, a second body's pass starts only when the
    first ends, while an interactive job runs at once; a waiting pass whose askers give up never
    runs; a pass that panics frees its permit; the default, and the refusal of 0.
  - _Acceptance:_ `cargo test -p hyperion-server surface` and
    `cargo test -p hyperion-server config`.
  - _Built 2026-10-23_ (lane r09-t17a, on `71532a17`). `SurfaceService::new` takes the bound as a
    fourth argument, `passes: NonZeroUsize`, which the server gives from
    `ServerConfig::surface_passes()`; with neither `--surface-passes` nor its variable that
    resolves the default from the configuration's own workers, so a builder's `workers` alone
    moves it. The permit is
    awaited after the flight's second look at the cache and moved into the bulk job's closure, so
    the job holds it, not the flight: a flight given up while its job runs leaves the pass counted
    until the job ends. A bound past `Semaphore::MAX_PERMITS` is taken as that many. The service's
    `Debug` shows the permits free, and the startup log `surface_passes`. As built, the variable's
    fallback and its refusal of 0 are tested in `tests/cli.rs`, through the binary, since setting
    a variable in a unit test needs `unsafe`, which the workspace forbids; the unit tests take a
    `Turnstile` source that holds each body's pass until the test lets it through. Beyond the
    entry's list, tests also hold that a prefetched pass waits for its permit as a requested one
    does, that a bound past `Semaphore::MAX_PERMITS` is taken as its most, and that the option
    wins over its variable. The README's and `config.rs`'s option tables were re-padded for the
    new default's text.

### R09.T18 Survey passes and coverage

Records: [09-surface-generator/records/R09.T18.md](09-surface-generator/records/R09.T18.md).

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
  Records: [09-surface-generator/records/R09.T19.a.md](09-surface-generator/records/R09.T19.a.md).
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
  Records: [09-surface-generator/records/R09.T19.b.md](09-surface-generator/records/R09.T19.b.md).
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
  Records: [09-surface-generator/records/R09.T19.c.md](09-surface-generator/records/R09.T19.c.md).

### R09.T20 Verification and hand-over

Records: [09-surface-generator/records/R09.T20.md](09-surface-generator/records/R09.T20.md).

Run every slow test and bench on a quiet machine, record figures in the doc comments that own them,
mark R05's provisional function deprecated with a pointer to `Synthesiser` for R10, and record the
per-point cost in wasm against the budget and against R05's T3.c figure for the test planet.
Acceptance: `just ci`, `just ci-slow`, `just bench` complete (the orchestrator runs `just ci` and
`just ci-slow`; a lane runs only the slow tests and benches this plan created, the timed ones under
the heavy lock or recorded provisional).

_Built 2026-10-26_ (lane r09-t20, on `e36c29fd`, gated on `8fe83fa3`; Risks, "Verification and
hand-over, as run (R09.T20)" and "Deferred findings (2026-10-13 rule) (R09.T20)"). R05's
`TestPlanet::height` is deprecated, its note pointing at `Synthesiser::height_at`, which R10's
client path reads. The spike's `HeightSource` path is kept, and no output moves. Every slow test
the plan created passes by name: the sim's 51 and the surface crate's 13. The benches ran under the
heavy lock at the day's load, so they are provisional, and their quiet re-run is pending on main's
schedule. In WebAssembly the whole height takes 8.80 µs a point on the Earth-like world at the
coarse level plus four, inside the brainstorm's 10 µs, and 14.08 µs at the band limit's level,
1.41 times it. Those are 2.1 and 3.4 times R05.T3.c's test planet (4.1 µs, natively). The reference
Earth's pass takes 59.9 s at level 8, T16.a's 57.1 s within the load's spread. The record maps
each Verification item to its tests, and names what R09 hands to R10, galaxy plan 14, R11, R12,
the owner and main.

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

_As verified (R09.T20, 2026-10-26):_ item by item in Risks, "Verification and hand-over, as run
(R09.T20)". The by-eye look stays pending for the owner. The channels have not been compared
across the wire (a deferred finding there), and the benches are provisional.

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
0.1 mm a⁻¹ × 2^((c − 1) ÷ 12) (science-r09-drainage.md §1); R09.T6.c's `LogIncision`, after
`catchment_runoff`: 0 none, 1 uncut, c ≥ 2 0.5 m × 2^((c − 2) ÷ 16), saturating at 255; and T2's follow-up E's palette
liquid record (its eight members in order, each optional member's presence byte, and the
record's own), `LIQUID_BINS`' fifteen bake bins, `LiquidBasis`'s codes 0–2, and the header's
`maturity` and `organic_cover`, dimensionless in [0, 1] and absent until P14.T54.b
(decision-r11-palette-liquids §1 and §2); and T2's follow-up F's palette `feo_mass_fraction`, in
[+0, 1], and the header's `weathering_rate`, finite and ≥ +0 and present exactly when `maturity`
is (decision-p14-t51d-weathering §1.6 and §2.3). The gate does not open before R09.T2.f lands. Nor
does it open before R09.T6.c lands, since T6.c's input is a field or header member and the layout
freezes at the gate (decision-r09-t14-choices F2). R10.T10.a fills
`FieldHeader.albedo_scale` and bumps `GENERATOR_VERSION`,
re-blessing this plan's payload and coarse goldens in that commit. _As built (main's ruling for
R10.T10.a, 2026-10-14): no bump. The first send above has not happened, since no generated body is
served before P14.T54.a, so the scale is filled at 22 by the rule of 2026-10-09; and it moves none
of this plan's goldens, the server setting it after `coarse_pass`, whose output and the testing
worlds' payloads keep `None` (R10's Risks, "Deviations in T10.a, as built")._

## Risks and open points

Task records as built: [09-surface-generator/records/](09-surface-generator/records/), one file per task, each linked from its entry under Tasks.

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
  R09.T16 measured it: 94.9 s for the reference Earth's pass at level 8, its climate step 68.9 s
  (decision-r09-pass-cost; R09.T16.a). R09.T16.a's bit-identical levers brought it to 57.1 s, the
  climate step 37.3 s. R09.T20 re-ran every bench of the plan under the heavy lock, at the day's
  load: the pass took 59.9 s, and the whole height 8.80 µs a point in WebAssembly at the coarse
  level plus four and 14.08 µs at the band limit's level. All are provisional, with the quiet re-run
  pending (Risks, "Verification and hand-over, as run (R09.T20)").
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
  - _Intraplate volcanism on a mobile lid._ No task builds oceanic plateaus, seamounts or hotspot
    swells (GSFM: 10.2% of the ocean), which hold some 250–300 m of Earth2014's ocean mean; T12.a's
    mean band is widened on its deep side by 150 m until they are built
    (`decision-r09-t12-coarse.md` item 1).
  - _A regional volcanic rise._ T12.c's provinces are edifices on filled moats. Mars's
    constructional relief is mostly Tharsis's rise, which an edifice law reaches only with some 30
    cap-height shields; a rise component is for T12.c's re-validation (`decision-r09-t12-coarse.md`
    addendum 2); with it, a standing bulge on its own foundation ρ_m g + E T_e ÷ R² and the
    axisymmetric (Kelvin-function) profile about each edifice, which the line-load reading
    overstates (root 2.1×, bulge 5.3×) (`decision-r09-t12-coarse.md` addendum 3).
- **The survey stand-in.** `survey_pass` lets a client grant its ship coverage, as the server
  grants detail levels today. It is a discipline for an honest client, and the sensors plan
  replaces the caller, not the core.
- **Field size.** Design note 17's layout gives about 14.7 MB for an Earth, about 1.0 MB inside
  R03's 15 MiB check: 13.96 MB with `decision-r09-t5.md`'s sea-floor byte, and about 14.75 MB
  since follow-up C's two runoff bytes (science-r09-drainage.md §1.3, whose 14.4 MB and 1.4 MB
  predate the sea-floor byte). The transfer budget's margin was then about 0.98 MB, 6.2% of the
  check. _R09.T6.c (2026-10-22):_ its epoch-incision byte makes the record 26 bytes and the Earth
  at level 8 15,141,470 bytes (`wire/payloads.golden`), 587,170 bytes, about 0.59 MB and 3.7%,
  inside R03's 15 MiB (15,728,640 B), 58 of its 61 frames and still 15 blocks; the wire test is
  bound at R03's 15 MiB, the binding wire budget, where it read the brainstorm's decimal 15 MB
  (main's ruling). **The next per-cell byte needs a field-size ruling before it is built** (main's
  ruling): at 26 bytes the Earth at level 8 has about 1.5 bytes a cell of headroom (0.39 MB a
  byte), so one more byte fits with 0.19 MB (1.2%) to spare at 60 of the 61 frames, and a second
  does not. If the climate layer at L − 1 proves
  too coarse for
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
    items, T13.b–e and T13.g after it, T13.d after T13.g; T14.a after T12; T14.b after T14.a, T4 and T0.b's erodibility item;
    T14.c after T14.b and T6.b; T14.d after T14.c and T13; T15 after T13 and T14; T16 after T15;
    T17 after T3 and T10; T18 after T2; T19.a after T17 and T18; T19.b after T19.a and T8; T19.c
    after T19.b; T20 last. No task waits on an owner's sign-off or on an unbuilt galaxy-plan task;
    what waits on P14.T24 and P14.T48 is a generated world's surface (`for_body`) and the 61-chunk
    check, not a task.
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
      overstates them: water by 1.3, 1.9 and 2.9 times at σ_s 11°, 17° and 27°, and CO₂ by 1.4–9
      times, since a rough surface's shadows see more sunlit terrain. R09.T7.g replaces both fits.
    - Hayne et al.'s eq. 5 overstates a spherical bowl's shadow (off-centre transects are not
      similar), so the exact transect form replaces it.
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
