# Plan 07: Gas and dust field, extinction

## Header

- **Milestone:** M2.
- **Depends on:** 02 (galaxy model). It also extends what plans 01, 03, 04 and 05 built, which plan
  02 already depends on or which are complete by M2 (the server resolves system IDs through plan 03;
  the protocol and display tasks build on 04 and 05).
- **Brainstorm sections covered:**
  - "Between the stars": the bullets "Dust and gas" and "What dust and gas do".
  - "Fields": the bullet "Dust and gas".
  - "Galaxy parameters": the bullet "Gas".
  - "Supernova remnants: one route, not two": only what the shell window needs from the gas (density
    and pressure at a site, smoothed to the shell's scale; the pressure floor), and the sentence
    that a superbubble "is a hole in the gas field".
  - "Large features": the sentence that a nebula "adds to the dust field along a line of sight" (the
    hook only).
  - "Visualiser": "Dust lanes join the map when the dust field exists."
  - "Decisions": "Dust and gas do what physics says" and "The gas field has three phases and a
    pressure." Also the entry "2026-09-21: local density rulings", whose ruling 1 (each disc cored
    in height) and ruling 6 (the age–metallicity relation) reach this plan; see Risks.
  - "Open questions": "The fixture's gas", which asks whether the gas disc's column at the Sun's
    radius should be raised to the measured value. See Risks.
  - "Suggested order of attack": step 5.

## Goal

When this plan is done `hyperion-sim` has a gas and dust field: a pure function of seed and position
giving hydrogen density, thermal pressure, phase and dust content anywhere in the root cube, made of
three smooth phases, arm-following lanes, a central molecular disc and a mean-preserving log-normal
lattice noise. On top of it sit a numerical line integral that returns visual extinction, hydrogen
columns and per-band extinction between any two points (the same in both directions), a site
function for the supernova-shell window of plan 09, hooks through which plan 09's superbubbles and
clouds will modify the field, and the local density a sublight hazard model will read. In the app,
the `GALAXY` display gains an extinction layer on the galaxy map (dust lanes, alone or as an overlay
on the density map) and extinction and reddening figures in the local chart's readout.

## Scope and non-goals

In scope:

- The gas parameters of a galaxy, derived from plan 02's parameters plus draws on this plan's own
  streams.
- The smooth field: neutral disc with its hole, central molecular disc, warm ionised layer, hot
  corona, lanes.
- Lattice noise with exact interpolation, the log-normal, evaluation limited to a smoothing scale.
- Closed-form pressure with the corona's floor; phases and their filling factors.
- Dust-to-gas following metallicity; the hydrogen column per magnitude.
- The extinction line integral: symmetric, adaptive, budgeted, with a caller-owned noise cache.
- Cardelli–Clayton–Mathis wavelength dependence and a small band set.
- Hooks: gas modifiers (holes and clouds), the site function, local density for hazards, neutral
  hydrogen column for 21 cm emission.
- Map functions, protocol messages, server handling and client display changes for the two products
  above.

Non-goals:

- Superbubbles, molecular clouds, nebulae and their emission classes. They are plan 09's. This plan
  defines the `GasModifier` type they will produce and integrates it; until plan 09 the source of
  modifiers is empty.
- The shell window itself (Cioffi–McKee–Bertschinger and Tang–Wang branches, β, the turbulent
  floor). Plan 09 owns it and calls `GasField::state`.
- Any sublight hazard model. Only the density API exists here.
- Apparent magnitudes of stars, sensor horizons in play and retarded-time observation (plans 06 and
  12). They consume `Sightline`.
- What a faster-than-light drive does in gas. The brainstorm leaves it to the drive's brainstorm.
- Feeding the hole in the gas disc back into plan 02's potential (see Risks).
- X-ray photoelectric absorption. The hydrogen column it needs is provided; the cross-section is a
  sensor matter.

## Provides

All Rust paths are under `hyperion_sim::galaxy::gas` unless stated.

```rust
// params.rs
pub struct GasParams { /* private; getters below */ }
impl GasParams {
    pub fn from_galaxy(seed: Seed, params: &GalaxyParams) -> GasParams;
    pub fn milky_way_like() -> GasParams;       // pairs with GalaxyParams::milky_way_like()
    pub fn gas_mass(&self) -> SolarMasses;      // plan 02's GasDiscParams::mass()
    pub fn radial_scale(&self) -> LightYears;   // R_g, plan 02's GasDiscParams::length()
    pub fn hole_scale(&self) -> LightYears;     // R_m
    pub fn neutral_height(&self) -> LightYears; // h_n, plan 02's GasDiscParams::HEIGHT
    pub fn warm_fraction(&self) -> f64;         // f_w
    pub fn warm_height(&self) -> LightYears;    // h_w
    pub fn molecular_disc(&self) -> MolecularDisc; // mass, scale length, height
    pub fn corona_density(&self) -> HydrogenPerCm3;
    pub fn pressure_floor(&self) -> KelvinPerCm3;
    pub fn pressure_height(&self) -> LightYears;
    pub fn sigma_ln(&self) -> f64;
    pub fn lane(&self) -> LaneParams;           // offset, width, fraction
}

// field.rs
pub struct GasField { /* params + normalisations + borrowed arm geometry and metallicity */ }
impl GasField {
    pub fn new(seed: Seed, galaxy_params: &GalaxyParams, fields: &Fields) -> GasField;
    pub fn mean_density(&self, p: &GalacticPosition) -> HydrogenPerCm3;        // no noise
    pub fn density(&self, p: &GalacticPosition, scale: SmoothingScale,
                   cache: &mut NoiseCache) -> HydrogenPerCm3;                   // noise applied
    pub fn density_with(&self, p: &GalacticPosition, mods: &[GasModifier],
                        cache: &mut NoiseCache) -> HydrogenPerCm3;              // hazard API
    pub fn pressure(&self, p: &GalacticPosition) -> KelvinPerCm3;              // P ÷ k
    pub fn phase(&self, n: HydrogenPerCm3, p_over_k: KelvinPerCm3) -> GasPhase;
    pub fn dust_per_hydrogen(&self, p: &GalacticPosition) -> f64;              // ζ, 1 at [M/H] = 0
    pub fn state(&self, p: &GalacticPosition, scale: SmoothingScale,
                 cache: &mut NoiseCache) -> GasState;       // plan 09's `SiteGas`, at AtLeast(250 ly)
    pub fn neutral_bound(&self, cell: &CellBox) -> HydrogenPerCm3; // mean neutral gas, for thinning
}
pub enum SmoothingScale { Full, AtLeast(LightYears) }
pub struct GasState { /* density(), pressure(), temperature(), thermal_sound_speed(), phase() */ }
pub enum GasPhase { Hot, Warm, Cold, Molecular }

// noise.rs
pub struct NoiseCache { /* caller-owned, fixed capacity, direct-mapped */ }
impl NoiseCache { pub fn with_capacity(entries: usize) -> NoiseCache; pub fn clear(&mut self); }
pub fn log_normal_factor(seed: Seed, p: &GalacticPosition, sigma_ln: f64,
                         scale: SmoothingScale, cache: &mut NoiseCache) -> f64;
pub const OCTAVE_WAVELENGTHS_LY: [u32; 5];     // 1_024, 512, 256, 128, 64

// modifiers.rs
pub enum GasModifier {
    Hole  { centre: GalacticPosition, radius: LightYears, interior: HydrogenPerCm3 },
    Cloud { centre: GalacticPosition, core_radius: LightYears,          // Plummer profile
            central_density: HydrogenPerCm3, dust_per_hydrogen: f64 },
}
pub trait GasModifierSource {
    fn modifiers_near_segment(&self, a: &GalacticPosition, b: &GalacticPosition,
                              out: &mut Vec<GasModifier>);
}
pub struct NoModifiers;                        // the only source until plan 09

// ccm.rs
pub fn extinction_ratio(wavelength: Micrometres)
    -> Result<f64, EvaluateExtinctionError>;   // A_λ ÷ A_V at R_V = 3.1; Err below 0.1 µm
pub enum Band { U, B, V, R, I, J, H, K, MidInfrared, Radio }
impl Band { pub fn wavelength(self) -> Option<Micrometres>; pub fn ratio(self) -> f64; }
pub const R_V: f64;                            // 3.1
pub const HYDROGEN_COLUMN_PER_MAG: f64;        // cm⁻² mag⁻¹, Bohlin, Savage and Drake 1978

// extinction.rs
pub enum NoiseMode { Mean, Realised }
pub enum Quality { Full, Budget(NonZeroU32) }  // maximum number of steps
pub struct Sightline { /* a_v, hydrogen_column, neutral_column, steps */ }
impl Sightline {
    pub fn a_v(&self) -> Magnitudes;
    pub fn in_band(&self, band: Band) -> Magnitudes;
    pub fn reddening(&self) -> Magnitudes;                  // E(B − V) = A_B − A_V
    pub fn hydrogen_column(&self) -> PerCm2;                // all gas, corona included
    pub fn neutral_hydrogen_column(&self) -> PerCm2;        // the 21 cm hook
}
pub fn sightline(field: &GasField, a: &GalacticPosition, b: &GalacticPosition,
                 mode: NoiseMode, quality: Quality, mods: &[GasModifier],
                 cache: &mut NoiseCache) -> Sightline;
pub fn horizon(field: &GasField, origin: &GalacticPosition, direction: UnitVector, band: Band,
               limit: Magnitudes, max_range: LightYears, quality: Quality,
               cache: &mut NoiseCache) -> LightYears;

// map.rs, shaped like plan 02's `galaxy::map` so that plan 04's banded map service can drive it
pub fn extinction_face_on(field: &GasField, x: f64, y: f64, pixel_ly: f64) -> Magnitudes;
pub fn extinction_edge_on(field: &GasField, x: f64, z_lo: f64, z_hi: f64) -> Magnitudes;
pub fn render_extinction_rows(field: &GasField, spec: &MapSpec, rows: Range<u32>,
                              out: &mut Vec<f64>);   // plan 02's `MapSpec`; its selection is unused
```

Every argument called `p` above is plan 01's `coords::GalacticPosition`; the `x`, `y`, `z_lo` and
`z_hi` of `map.rs` are light-years, as plan 02's `galaxy::map` has them.

Other provisions:

- `Galaxy::gas(&self) -> &GasField`, added to plan 02's handle (`galaxy/mod.rs`), and `pub mod gas;`
  in the same file's module list.
- `hyperion_sim::units`: `HydrogenPerCm3`, `KelvinPerCm3`, `PerCm2`, `Magnitudes`, `Micrometres`,
  through `units.rs`'s `unit!` macro. None exists, and plan 06 adds only `MetalFraction`,
  `HeliumExcess`, `Gauss` and `SolarMassesPerYear`, so this plan owns all five.
- Domain tags `gas.params` and `gas.noise`, both of scope `Galaxy`, in plan 01's `domain_tags!`
  registry (`rng/tags.rs`, under a "Plan 07" heading appended after the Plan 06 event tags, since
  the macro's order fixes `tags::ALL` and `tests/golden/rng/tags.golden` pins it). Parameters are
  drawn on `ObjectKey::galaxy()`, lattice values on `ObjectKey::galaxy_item(n: u64)` with the packed
  word of Design note 8.
- Protocol, by plan 04's "Extending the convention" and nothing else: two request kinds,
  `extinction_map` and `extinction`, each a variant of `RequestBody` and of `ResponseBody` with the
  same `kind` string, both strings added to `REQUEST_KINDS`, failures as `request_error`. The bodies
  are `ExtinctionMapRequest` and `ExtinctionMap`, `ExtinctionRequest` and `ExtinctionResult` (with
  `ExtinctionTarget` and `TargetExtinction`). Also a `gas` group in `GalaxyParameters`, and `Unit`
  gains `PerCm3`, `KPerCm3` and `Mag` (wire strings `per_cm3`, `k_per_cm3`, `mag`). No new
  `ClientMessage` or `ServerMessage` variant, and `PROTOCOL_VERSION` does not change.
- `@hyperion/protocol`: `decodeExtinctionMap(map: ExtinctionMap): DecodedExtinctionMap`, sharing the
  decoder of plan 04's `decodeDensityMap`. That decoder's helpers (`decodeBase64`,
  `readLittleEndianU16`, `bytesPerCode`) are module-private in `packages/protocol/src/densityMap.ts`,
  so the shared part becomes one internal function in that file which both decoders call. The
  accessor is `log10Mag(code)` beside `decodeDensityMap`'s `log10PerLy2(code)`, so the two decoded
  maps are **not** structurally interchangeable and code that serves both takes the accessor as an
  argument. Bindings are regenerated by `just gen-protocol`; requests go through plan 04's
  `RequestClient` unchanged, which derives the response type from the kind, so no per-request
  function is added.
- `compute::quantise_map_with_floor(&RawDensityMap, floor_log10: f64, depth: CodeDepth)` in the
  server, which plan 04's `quantise_map(&RawDensityMap, MapView, CodeDepth)` is rewritten to call
  with its own view-dependent floor, reusing its private `code_of(value, floor, span, max)`.
  `QuantisedMap`'s getters `floor_log10_per_ly2` and `ceiling_log10_per_ly2` are renamed to the
  unit-neutral `floor_log10` and `ceiling_log10`, since one type now carries both quantities; the
  wire field names do not change. And `compute::ExtinctionMapService` beside `DensityMapService`.
- Test helper `ensemble_mean(f, seeds)` in `crates/hyperion-sim/tests/common/mod.rs`, which averages
  a function of the seed with a standard error. Golden files follow plan 01's convention:
  `hyperion_testkit::golden!` with `GoldenWriter`, whose `header(GENERATOR_VERSION.get())` line
  starts every golden, under `crates/hyperion-sim/tests/golden/gas/<name>.golden`.
- This plan's integration tests live in `crates/hyperion-sim/tests/gas.rs` (fast, and the only place
  `tests/common/mod.rs`'s helpers are visible) and `tests/gas_statistics.rs` (P07.T12's slow tests).
  Unit tests sit in `mod tests` inside each `gas` module, so an acceptance filter is
  `--lib galaxy::gas::<module>` or `--test gas`, never a bare name: a bare filter matches test
  _names_ and picks up others (plan 03, Risks, its re-validation record).

## Consumes

Every name below was checked against the code as built at the re-validation recorded under Risks.

- **Plan 01, determinism foundation:** `math::{exp, ln, powf, log10, ln_1p}` (`math` has no `sqrt`:
  `f64::sqrt` is called directly, as plan 01 allows, and the rule is enforced by
  `crates/hyperion-sim/clippy.toml`'s `disallowed-methods`); from `rng`, `Seed`, `Stream`,
  `DomainTag`, `TagScope` and `ObjectKey`, with `Stream::open(seed, tag, object)`,
  `ObjectKey::galaxy()` and
  `ObjectKey::galaxy_item(n: u64)`; the single `domain_tags!` registry in `rng/tags.rs`;
  `Stream::seek(n: u64)`, which seeks a **word** number, not a draw index, so a parameter's fixed
  index is its word offset (Design note 3); the samplers, which are **methods on `Stream`**, not free
  functions: `uniform()`, `uniform_in(lo, hi)`, `standard_normal()` and `normal(mean, sigma)`, each
  standard normal costing exactly two words. `units`, whose newtypes come from its `unit!` macro and
  whose `units::consts` is SI throughout. `coords::GalacticPosition` with `cell()`,
  `offset_metres()`, `to_light_years_f64()`, `from_light_years()`, `in_root_cube()` and
  `to_cylindrical() -> Cylindrical`, whose `radius()` and `height()` are **`Metres`** and whose
  `azimuth()` is `Radians`, so a caller working in light-years converts; `coords::ROOT_HALF_WIDTH_LY`
  (`coords::CellSize` is the generation-cell ladder, 4–128 ly, and is **not** used here: the noise
  lattice of Design note 8 is coarser than any of it). The dev-only `hyperion-testkit`: `golden!`
  with `GoldenWriter` (`header`, `line`, `f64`, `u64_hex`, `as_str`),
  `order::assert_order_independent(keys, f)`, from `stats` the helpers `ks_one_sample`,
  `chi_square_gof`, `assert_p_value`, `assert_poisson_count` and the level `ALPHA`, plus
  `lcg::Lcg` and `float::assert_same_bits`. Slow-test marking
  (`#[ignore = "slow: …"]`), `just test-slow`, `just ci-slow`, `just bench`, `just bless`,
  `version::GENERATOR_VERSION` (a `GeneratorVersion` newtype; goldens record `.get()`).
- **Plan 02, galaxy model:**
  - `GalaxyParams::gas_disc() -> &GasDiscParams`, whose getters are `mass()`, `length()` and
    `height()` — not `radial_scale` or `neutral_height`. The mass is a drawn 10–20% of the **whole**
    thin disc's stellar mass (young and old together) and the scale length 1.5–2 times the thin
    disc's, both uniform; the height is the fixed associated const `GasDiscParams::HEIGHT`, 400 ly,
    with no draw and no tag. Plan 02's D15 says plan 07 owns what they mean, and its "Generator
    version" reserves "plan 07 may refine the gas disc" (see Risks). `BarParams::half_length()`,
    `NuclearDiscParams::length()`, `GalaxyParams::milky_way_like()` and
    `GalaxyParams::from_seed(seed, kind)`.
  - `Fields::arms() -> &arms::ArmGeometry` (`ArmGeometry` is `Copy`, with `pitch() -> Radians`,
    `fade(r)`, `phase_polar(r, theta)`, `point_polar(r, theta) -> ArmPoint`, `ridge_azimuth(r, j)`
    and `phase_range(cell)`), and `arms::SharpArm`, whose factor 1 + f(R) A (g − 1) of plan 02's D10
    is `SharpArm::factor(&ArmPoint)`. **`SharpArm::new` takes
    `(geometry: ArmGeometry, width: LightYears, fraction: f64)` and returns
    `Result<Self, BuildFieldError>`, so it already takes a caller's σ_w and A** — its own
    doc example builds a lane — so P07.T3 needs no change to plan 02. Its bound is
    `SharpArm::sup(radii: ScalarRange, phase: ScalarRange) -> f64`, called directly: the
    `UnimodalFactor` implementor `ArmAcross` is reachable only through `Arm::across`, and `Arm` owns
    its `SharpArm` by value.
  - The gas metallicity: `Component::metallicity(p: &PointLy, age: Years) -> FehDistribution`, whose
    `mean() -> Dex` is read at `Years::ZERO` for the young thin disc's component, the metallicity of
    stars forming now. It reads the cylindrical radius alone, never z, so ζ is constant along a
    vertical line — which is what makes P07.T9's face-on closed form exact. The young thin disc is
    the only component with population `YoungThinDisc` and is `fields.components()[0]`; found
    robustly by `components().iter().find(|c| c.population() == Population::YoungThinDisc)`.
    `metallicity::THIN_DISC_MEAN_RANGE` already clamps the mean to [−1.0, +0.5] dex, so D13's cap is
    free.
  - `PointLy` and `impl From<&coords::GalacticPosition> for PointLy` (by reference only; there is no
    by-value `From` and no method form). From `galaxy::map`: `MapView`, `MapSelection`,
    `MapSpec::new(view, selection, size: [u32; 2], centre: [f64; 2], ly_per_px)`, which returns
    `Result<_, BuildMapSpecError>`, with its getters, `extent()`, `pixel_count()`,
    `pixel_centre(column, row)` and `pixel_span(row)` (the `[z_lo, z_hi]` a row spans, consecutive
    rows sharing an edge bit for bit), and the row-wise shape of
    `render_rows(fields, spec, rows: Range<u32>, out: &mut Vec<f64>)`, which clears `out` and panics
    if the rows reach past the raster. Its width and height are `u32`. `column_density_face_on` and
    `column_density_edge_on` both take a `MapSelection`, and the face-on one takes **no** pixel size:
    this plan's `extinction_face_on(.., pixel_ly)` deliberately differs, because a lane is narrower
    than a pixel. The edge-on machinery of P02.T10.b (`EdgeOnPlan`, its `pixel`, `across_pixel`,
    `render_edge_on_rows`, `MAX_PARTS`, `Part`) is **private to `galaxy::map`**, so P07.T9
    reimplements the plan-means-lines split inside `gas::map` rather than calling it.
  - `galaxy::quad::{gl16, gl32, gl4, gl_panels, gl32_log, gl_log_panels, Gl16Panel, bisect}`. An
    integral in ln R over log-spaced edges is **`gl_log_panels`**, which substitutes u = ln x itself;
    `gl_panels` integrates in x. `tables::gauss_legendre` holds four tables, 4, 8, 16 and 32 points,
    so this plan adds none.
  - `bounds::{CellBox, ScalarRange, UnimodalFactor, BOUND_MARGIN}`; `CellBox::new(min, edge)` refuses
    an edge that is not a power of two, a box outside the root cube and a box across an axis plane,
    and gives `min_corner`, `nearest_corner`, `farthest_corner`, `r_cyl_range`, `centre`,
    `in_plane_half_diagonal` and `contains`. `bounds`'s `COS_SLACK` and `ROUNDING_SLACK` are
    `pub(crate)`, which `galaxy::gas` may read.
  - `fields::vertical::VerticalProfile` and `fields::disc::ExponentialDisc` are read only where this
    plan's own risks name them (the potential's plain gas disc); `ExponentialDisc::new` and
    `vertical::locate` are `pub(crate)`, so they are reachable from `galaxy::gas` but not from
    `crates/hyperion-sim/tests/`.
  - The `Galaxy` handle (`new`, `from_params(seed, params)`, `seed`, `params`, `fields`, `potential`,
    `shares`, `mass_model`, `heap_bytes`), which gains `gas()`. It derives `Debug`, `Clone` and
    `PartialEq` and is `Send + Sync + RefUnwindSafe`, so `GasField` must be too.
  - `crates/hyperion-sim/tests/common/mod.rs`: `assert_within`, `assert_relative`, `sunlike_point`,
    `reference_sphere_integral`, and its existing assertions on the gas disc's fraction, length ratio
    and height, which P07.T1 keeps in step.
- **Plan 03, placement and range query:** `placement::resolve(galaxy: &Galaxy, id: SystemId)`, which
  returns `Result<SystemRecord, ResolveSystemError>`, whose variants are `NoSuchSystem`,
  `LayerNotGenerated(Layer)` and `KindNotGenerated`; and
  `query::position_at(galaxy, record: &SystemRecord, t: UniverseTime)`, which returns a
  `GalacticPosition`. Together they turn an `ExtinctionRequest` into end
  points. `t` is the **sim's** `hyperion_sim::time::UniverseTime`, not the wire type of the same
  name; `SystemRecord::epoch_position()` lends a `&GalacticPosition`; and `SystemId` is built with
  `SystemId::from_raw(u64) -> Result<_, DecodeSystemIdError>` and read back with `raw()`, there
  being no `to_u64`. Both tasks are built (P03.T7 and P03.T9.e). Nothing here needs a `CellCache`,
  whose `with_cell` takes `impl FnOnce` and so is not dyn-compatible.
- **Plan 04, server and protocol:** the request convention (`RequestBody`, `ResponseBody`,
  `REQUEST_KINDS`, `RequestError`, `ErrorCode::BadRequest` with its `field`, the rules of "Extending
  the convention"); `UniverseIdHex` and `SystemIdHex` with `from_u64`/`to_u64`, the wire
  `GalacticPosition { cell_ly, offset_m }` and `UniverseTime { seconds, nanos }`; `DensityMap`'s
  geometry fields and wire conventions (design note 12: row order, code 0, little-endian 16-bit
  codes); `GalaxyParameters`, `ParameterGroup`, `Parameter`, `ParameterOrigin`, `ParameterValue`,
  `Unit` (ten variants, `snake_case` on the wire, pinned by `unit_strings`); `PROTOCOL_VERSION`,
  which does not move. From the server's `compute`: `CpuPool`, `Priority`, `CancelToken`,
  `CancelOnDrop`, `SingleFlight`, `GalaxyCache`, `GalaxyKey`, `ComputeError`, `DensityMapService`,
  `MapKey`, `MapResolution`, `CodeDepth`, `RawDensityMap`, `QuantisedMap`, `quantise_map` and
  `MAP_WIDTH_LY` —
  `CpuPool::try_submit(priority, token, job)` for work that may be refused with `queue_full` and the
  async `submit` for serialising a finished large response; `MapResolution::height_px(view)` takes
  the view; `BAND_ROWS` is private, so P07.T10.a makes it `pub(crate)` or gives the extinction
  service a band constant of its own. From `cache`: `ByteLru`, `SharedByteLru`, `Insertion`,
  `LruCounters`, `HeapBytes` and `ENTRY_OVERHEAD_BYTES`; the `limits` module; from `requests`,
  `Handler`, `Handlers`, `kind`, `is_large` and
  `requests::universe::openable_universe(&AppState, &UniverseIdHex)`, which every handler naming a
  universe calls first; from `convert`, `ConvertRequestError`, `MapRequest`, `density_map` and
  `galaxy_parameters` with its `EXCLUDED_PARAMETERS` and `PARAMETERS_ACCOUNTED_FOR`; `AppState`.
  `decodeDensityMap`; `TestServer`, `TestClient` (`hello`, `request`, `send_request`, `cancel`,
  `close`), `FakeWebSocket`, and `packages/protocol/fixtures/density_map_4x2.json` as the shape a
  cross-language fixture takes.
- **Plan 05, `GALAXY` display:** under `apps/hyperion/src/renderer/src/`, in `displays/galaxy/`:
  `GalaxyPages.tsx` (the `PARAMETERS`, `GALAXY MAP` and `LOCAL CHART` pages), `GalaxyDisplay.tsx`
  (which owns the cursor, the chart centre, the page and `useLocalChart`), `GalaxyMapPanel.tsx`
  (which owns the `POPULATION` choice and holds **no** request), `GalaxyMapView.tsx` (which holds
  `useServerRequest<"density_map">`, the `MAP DATA INVALID` checks and the stale picture),
  `DensityLegend.tsx`, `SystemsPanel.tsx`, `SystemReadout.tsx`, `useLocalChart.ts` with
  `LocalChartState`, `useRangeQuery.ts`, `mapCursor.ts`, `parameterLabels.ts` and
  `parameterReadings.ts`; in `lib/galaxy/`: `mapPicture.ts` (`MAP_RESOLUTIONS_PX`,
  `mapResolutionFor`, `MAX_UPSCALE`, `turnClockwise`, `reducedLevels`, `paintLevels`), `ramp.ts`
  (`parseHexColour`, `buildRamp`, `codeToLevel`, `rampColour`, `rasterise`, `DecodedCodes`) and
  `mapGeometry.ts`, whose `mapGeometry(map: MapRaster)` **already** takes a narrow structural type,
  so this plan widens nothing; `lib/useServerRequest.ts`
  (`useServerRequest(body, timeoutMs?, generation?)`, `RequestState`, `followRequest`,
  `REQUEST_TIMEOUT_MS`); in `components/`, `RequestStatus.tsx`, `StatusLine.tsx`, `StaleMark.tsx` and
  `UnitLabel.tsx`, the last exhaustive over `Unit`; `lib/format.ts` (`formatSci`, `formatNumber`,
  `formatSigned`); `test/{FakeWebSocket.ts, galaxyFixtures.ts}`; and the UX guide's raster-field
  rule. Plan 05 left `MapPopulation` open for a dust value; this plan does not use that opening
  (Design note 19).

## Design notes

Each of these is a decision the brainstorm leaves open. Figures marked "MW" are the values for the
Milky Way fixture. Every figure is a starting value that P07.T12 tunes against the brainstorm's
targets; the targets are binding, these are not.

1. **Module placement.** The field lives in `hyperion_sim::galaxy::gas`, beside `fields`, because it
   is a field of the galaxy and shares the arm parameters. It is reached through `Galaxy::gas()`.

2. **Units.** Density is hydrogen nuclei per cubic centimetre, because every source formula and the
   brainstorm use it. Mass density is 1.4 m_H × n_H (helium included). Pressure is carried as P ÷ k
   in K cm⁻³, the unit the measurements are quoted in. Lengths are light-years at this interface.
   All are newtypes through `units.rs`'s `unit!` macro. `units::consts` is SI throughout, so the two
   new constants are `HYDROGEN_MASS_KG` and `BOLTZMANN_CONSTANT` (CODATA; cite), and centimetres per
   light-year is derived in the gas module from the existing `consts::METRES_PER_LIGHT_YEAR`
   (9,460,730,472,580,800 m exactly) rather than added as a non-SI constant.

3. **Parameters plan 02 does not draw are drawn here**, on the tag `gas.params` with the galaxy as
   the object and one fixed draw index per parameter, so that nothing in plan 02's output moves and
   a parameter added later appends an index. `Stream::seek` takes a **word** number, and every
   parameter here is one uniform, which costs one word, so parameter _k_ of the table is `seek(k)`.
   A parameter added later that needs a normal costs two words and so appends two indices. The
   table's first three rows are plan 02's, not drawn here; on the wire they stay where P04.T14.b put
   them (`gas.mass` under `mass`, `disc.gas.scale_length` under `discs`) and only the rest form
   P07.T10.b's `gas` group, since a dotted parameter key is unique across the whole response.

   | Parameter                             | Rule                                               | MW           |
   | ------------------------------------- | -------------------------------------------------- | ------------ |
   | Gas mass                              | `GasDiscParams::mass()`, 10–20% of the thin disc's | 5.1 × 10⁹ M☉ |
   | Radial scale R_g                      | `GasDiscParams::length()`, 1.5–2 × the thin disc's | 14,000 ly    |
   | Neutral scale height h_n              | `GasDiscParams::HEIGHT`, a generator constant      | 400 ly       |
   | Hole scale R_m                        | 0.8–1.2 × the bar's half-length                    | 16,000 ly    |
   | Warm ionised share of the mass f_w    | 0.20–0.30, uniform                                 | 0.25         |
   | Warm ionised scale height h_w         | 2,500–3,500 ly, uniform                            | 3,000 ly     |
   | Molecular disc share of the mass f_c  | 3–10 × 10⁻⁴, log-uniform                           | 5 × 10⁻⁴     |
   | Molecular disc scale length R_c       | the nuclear disc's scale length                    | 290 ly       |
   | Molecular disc height h_c             | 0.15–0.25 × R_c                                    | 58 ly        |
   | Corona density n_cor                  | 0.5–1.2 × 10⁻³ cm⁻³, log-uniform (note 12)         | 10⁻³         |
   | Pressure floor P_cor ÷ k              | 300–500 K cm⁻³, uniform                            | 400          |
   | Pressure height h_P                   | generator-version constant                         | 1,500 ly     |
   | Pressure speed σ_P                    | generator-version constant                         | 5.5 km/s     |
   | Log-normal width σ_ln                 | 2.0–2.5, uniform                                   | 2.3          |
   | Lane offset d (inward, perpendicular) | 300–600 ly, uniform                                | 450 ly       |
   | Lane width σ_w                        | 150–300 ly, uniform                                | 200 ly       |
   | Lane fraction A of the neutral gas    | 0.08–0.20, uniform                                 | 0.12         |

4. **The smooth field.** With R, z cylindrical in light-years:

   ```text
   n_neutral(R, φ, z) = n_0 · exp(−R_m ÷ R − R ÷ R_g) · exp(−|z| ÷ h_n) · lane(R, φ)
   n_warm(R, z)       = w_0 · exp(−R_m ÷ 2R − R ÷ R_g) · exp(−|z| ÷ h_w)
   n_mol(R, z)        = c_0 · exp(−R ÷ R_c − |z| ÷ h_c)
   n_disc             = n_neutral + n_warm + n_mol
   n_mean             = n_disc + n_cor
   ```

   The factor exp(−R_m ÷ R) is the hole inside the bar, in the form McMillan (2017) uses for the
   Milky Way's gas discs; the neutral disc peaks at √(R_m R_g), near the bar's end. The warm layer
   takes half the hole scale because ionised gas fills the inner galaxy more evenly than neutral gas
   does. n_0, w_0 and c_0 follow from the mass budget: the gas mass of plan 02 is shared as (1 − f_w
   − f_c), f_w and f_c, and each normalisation is mass ÷ (1.4 m_H × the component's volume
   integral). The molecular disc's integral is 4π R_c² h_c. The other two have no elementary radial
   integral, so the radial part is a fixed quadrature in ln R, eight panels with log-spaced edges
   from 1 ly to 20 R_g through plan 02's `galaxy::quad::gl_log_panels`, which substitutes u = ln R
   itself and puts 32 nodes on each panel (`gl_panels` integrates in R, not in ln R), done once in
   `GasField::new`. The corona is outside the budget: it is a halo component, given by its density.
   With MW values this gives 0.70 cm⁻³ of neutral gas and 0.030 cm⁻³ of warm ionised gas in the
   plane at 26,000 ly, 1.1 mag per 3,000 ly there, 0.19 mag to the galactic pole, and 28 mag to the
   centre (13 from the disc, 15 from the molecular disc, 1 from the warm layer). Every one of those
   figures was re-derived at the re-validation and holds to two figures; the gas column they imply at
   26,000 ly, 7.9 M☉ per square parsec, is the subject of an owner question under Risks.

5. **The molecular disc's mass is set by the brainstorm's thirty magnitudes, not by the Milky Way's
   central molecular zone.** The real zone holds 3–5 × 10⁷ M☉, which as a smooth disc would put over
   a hundred magnitudes in front of the centre. The real gas is in a few dense clouds that the line
   of sight to the centre mostly misses. So the smooth disc carries only the diffuse part, some 2–3
   × 10⁶ M☉, and the dense clouds are plan 09's features, which add their own dust locally as the
   brainstorm says. See Risks.

6. **Lanes reuse the arm factor at a shifted radius.** lane(R, θ) = S(R + d ÷ cos p, θ), where S is
   plan 02's `SharpArm` factor 1 + f(R) A (g − 1), built on the shared `ArmGeometry` through
   `SharpArm::new(geometry, width, fraction)` with the lane's own width σ_w and fraction A, evaluated
   as `SharpArm::factor(&geometry.point_polar(R + δR, θ))`, and p is `ArmGeometry::pitch()`. Shifting the radius by δR is a phase
   offset of n δR ÷ (R tan p), so this is the brainstorm's "phase offset" with the offset fixed in
   light-years, for the reason the brainstorm gives for arm widths. Because S averages 1 around the
   circle of radius R + δR, the lane factor averages exactly 1 around the circle of radius R, and
   lanes move no mass. A is the share of the neutral gas gathered into lanes; the peak contrast
   follows from it and is about 3 at 26,000 ly for MW values (four arms at 12°), with 1 − A between
   the arms. The arms trail and the gas overtakes them inside corotation, so the lane sits on the
   inner, concave edge: d is positive inward. The warm layer and the molecular disc carry no lanes.

7. **Noise that is mean-preserving at every point, not only on average.** Lattice values are
   standard normal draws keyed by (seed, `gas.noise`; octave and lattice coordinates), one
   `Stream::standard_normal` per lattice point on its own `ObjectKey::galaxy_item(word)`, so the two
   words each draw costs do not interleave between points. Within a lattice cell the octave's value
   is

   ```text
   g_k(x) = Σ_i w_i G_i ÷ √(Σ_i w_i²)
   ```

   with the eight trilinear weights w_i built from the quintic fade s(t) = t³(6t² − 15t + 10).
   Dividing by the root of the summed squares makes g_k exactly standard normal at every point (a
   sum of independent normals), where plain interpolation would lose up to seven eighths of the
   variance at cell centres and break the mean. Σ w_i² factorises per axis as (1 − s)² + s². The
   octaves combine as g = Σ a_k g_k with Σ a_k² = 1, so g is standard normal and the factor

   ```text
   F(x) = exp(σ_ln g(x) − σ_ln² ÷ 2)
   ```

   has expectation exactly 1 at every x. All arithmetic before the final `math::exp` is `+ − × ÷`
   and `sqrt`, which are IEEE-exact, and t is an offset divided by a power of two.

8. **Octaves.** Five, with lattice spacings 1,024, 512, 256, 128 and 64 ly, and a_k² ∝ λ_k^⅔ (more
   variance at large scales, as turbulence has), normalised. Spacings are powers of two so that
   lattice planes are cell faces of the light-year grid and t is exact. Nothing in the field varies
   faster than 64 ly, which is what bounds the integrator's step. The counter word packs octave (4
   bits) and three lattice coordinates (20 bits each, offset to unsigned) into the `u64` that
   `ObjectKey::galaxy_item(n: u64)` takes, so sixteen octaves are reserved. The root cube is
   ±`coords::ROOT_HALF_WIDTH_LY`, 65,536 ly, so at the coarsest spacing the lattice index runs
   −64..=64 and at the finest −1,024..=1,024: every one fits 20 bits with the offset.

9. **Smoothing to a scale is dropping octaves and their variance.** `SmoothingScale::AtLeast(ℓ)`
   keeps the octaves with λ_k ≥ ℓ and uses σ_eff² = σ_ln² × Σ_kept a_k² in both places of F. That is
   the exact conditional mean of the full field given the coarse octaves, so it is still
   mean-preserving. The research behind the brainstorm drops octaves finer than about 250 ly for a
   field shell; plan 09 passes the shell's own scale.

10. **What the noise multiplies.** n(x) = n_disc(x) × F(x) + n_cor. The corona is an additive,
    un-noised floor: hot gas fills the voids. Dust follows the disc term only, because grains do not
    survive in the corona: the dust-bearing density is n_disc × F × ζ.

11. **Pressure.** P(R, z) ÷ k = P_cor ÷ k + (1.4 m_H σ_P² ÷ k) × n̄_disc(R, 0) × exp(−|z| ÷ h_P),
    where n̄_disc(R, 0) is the azimuthal mean in the plane (so lanes do not modulate it). It is the
    hydrostatic form "pressure is density times a velocity dispersion squared", closed-form, falling
    with height to the corona's floor. σ_P = 5.5 km/s is calibrated so that MW values give the
    measured thermal pressure of 3,800 K cm⁻³ in the plane at 26,000 ly (Jenkins and Tripp 2011, the
    figure the research behind the brainstorm adopted). The noise does not enter the pressure: the
    phases are in rough pressure balance, which is what makes rarefied gas hot.

12. **Phases** are labels derived from density and pressure through the equilibrium temperature T =
    (P ÷ k) ÷ (x n), with x = 2.3 particles per hydrogen nucleus for ionised gas and 1.1 for neutral
    gas: hot above 10⁵ K (n < P ÷ 2.3 × 10⁵ k), warm down to 5,000 K (n < P ÷ 5,500 k), molecular
    above 100 cm⁻³, cold between. With these thresholds a log-normal of σ_ln 2 to 2.5 around 0.75
    cm⁻³ at 3,800 K cm⁻³ puts 18% to 39% of the plane's volume in the hot phase, which is the
    brainstorm's "a fifth to two fifths". The corona itself must come out hot for every seed, which
    needs P_cor ÷ (2.3 k n_cor) above 10⁵ K: that is why the corona's density range stops at 1.2 ×
    10⁻³ cm⁻³ against a floor that can be as low as 300 K cm⁻³. Within the warm phase the neutral
    share is the smooth ratio n_neutral ÷ (n_neutral + n_warm) at that point; hot gas is fully
    ionised; cold and molecular gas are neutral. That rule gives the neutral hydrogen column in
    `Realised` mode. In `Mean` mode there is no local density to classify, so the neutral column is
    the integral of n_neutral + n_mol.

13. **Dust-to-gas is linear in metal abundance.** ζ(x) = 10^[M/H], with [M/H] the gas metallicity
    from plan 02's field at the position: `Component::metallicity(&PointLy::from(p), Years::ZERO)`
    for the young thin disc's component, whose `mean()` plan 02 already clamps to [−1.0, +0.5] dex
    (`metallicity::THIN_DISC_MEAN_RANGE`), so the cap at +0.5 comes for free and the field reads the
    cylindrical radius alone, never the height. One magnitude of visual
    extinction is 1.87 × 10²¹ × ζ⁻¹ hydrogen nuclei per cm² (5.8 × 10²¹ per magnitude of E(B − V),
    Bohlin, Savage and Drake 1978, with R_V = 3.1), the brainstorm's "about 2 × 10²¹".

14. **The integrator.** The segment is clipped to the slab |z| ≤ 8 h_w, outside which nothing but
    the dust-free corona remains (its hydrogen column is added in closed form). Inside, it is
    marched with two-point Gauss–Legendre steps of length min(Δ_noise, Δ_smooth): Δ_noise = 32 ly ×
    2^lod, and Δ_smooth is half the smallest local smooth scale (h_n or the lane width within 5 h_n
    of the plane, h_c within 3 R_c of the centre, h_w ÷ 4 elsewhere). `Quality::Full` is lod = 0.
    `Quality::Budget(N)` takes the smallest lod whose step count is at most N and evaluates the
    noise at `AtLeast(2 Δ_noise)`, so octaves the steps cannot resolve are replaced by their mean
    and not aliased. `NoiseMode::Mean` skips the noise entirely. The result is a pure function of
    its arguments, quality included, so a caller that needs one answer uses one quality. The server
    fixes one per use.

15. **Two-way visibility is exact.** `sightline` orders its end points canonically (by cell, then
    offset, lexicographically) before marching, so A(a, b) and A(b, a) are the same bits. `horizon`
    marches from the origin and may differ from `sightline` in the last bits; it is documented as an
    instrument range, not an identity.

16. **Holes and clouds.** A `Hole` replaces the field inside its sphere by its interior density with
    no dust; the integrator splits steps at the sphere's entry and exit, found from the
    segment–sphere quadratic. A `Cloud` is a Plummer ball, n_c (1 + r² ÷ a²)^(−5/2), added on top.
    Its column along a segment is algebraic: with b the line's impact parameter, c² = a² + b² and s
    the distance along the line from the point of closest approach, the column is n_c a⁵ × [s (3c² +
    2s²) ÷ (3c⁴ (c² + s²)^(3/2))] between the segment's two values of s. So clouds cost no steps and
    no transcendental function. Where holes overlap, the lowest interior density wins; clouds add.
    `GasField::state` takes no modifiers, because the shell test may read only fields and the star's
    own marks. Plan 09's `features::gas_overlay::FeatureGas` implements `GasModifierSource` and
    yields a `Hole` per superbubble and a `Cloud` per cloud and embedded region; with an empty list
    results are bit-identical. The interface sketch gave plan 09 a Gaussian cloud; the Plummer ball
    replaces it because its column is algebraic, and plan 09 as it now reads uses the Plummer form.

17. **Caching.** The sim holds no caches, so `NoiseCache` is a caller-owned direct-mapped table of
    lattice normals keyed by the packed counter word. Consecutive steps along a line share most of
    their lattice corners, which is where the saving is. The server adds a byte-bounded LRU of
    `Sightline` results keyed by (seed, generator version, end points, quality) through plan 04's
    `SharedByteLru` (the `&self` wrapper over `ByteLru`, since the handlers are concurrent), with a
    `HeapBytes` impl for the value, as plan 04's design note 23 keys every cache.

18. **The gas field is static.** It is a snapshot at the epoch like the stellar fields, and is not
    advanced with the clock: over ±H the arm pattern turns by about a light-year at 26,000 ly, far
    below the lane width. End points move (systems drift), the medium does not.

19. **The map product is a separate extinction layer.** `ExtinctionMap` is the visual extinction
    through the whole galaxy along each pixel's line of sight, in magnitudes, from the mean field
    (lanes included, noise not, since a pixel is 256 ly and a column through the disc averages the
    noise). Attenuating the density map on the server was rejected: that map counts systems per
    square light-year, and a dimmed count is not a quantity a console could label. The client can
    draw the extinction layer alone or as a labelled overlay that darkens the density ramp, which is
    what puts dark lanes on the arms' inner edges. It is a request kind of its own
    (`extinction_map`) and not a new selector on `DensityMapRequest` or the `MapPopulation` value
    plan 05 left room for, because that response's floor and ceiling fields are named in systems per
    square light-year. The payload is quantised in log₁₀ of magnitudes like the density map, since
    face-on values span 0.01 to tens of magnitudes, by plan 04's quantiser with an explicit floor
    (`quantise_map_with_floor`), since plan 04's own floor is a number of decades below the ceiling
    (5 dex face-on, 7 edge-on) and this one is a fixed 0.01 mag. The raster reuses plan 04's
    `RawDensityMap` as it stands — its fields are a size, a centre, a pixel size and a `Vec<f32>` of
    log₁₀ values, with no unit and no view in it — so nothing new is needed to carry a grid of
    magnitudes. Each band's pixels are turned into that `f32` by the sim's `math::log10`, not the
    platform's, as plan 04's `DensityMapService` does, so the codes hold on every machine; a pixel
    with no dust becomes `−∞` and so code 0, exactly as an empty density pixel does.

20. **Bands.** `extinction_ratio` takes a wavelength and is the real interface. `Band` is a
    convenience set at the Johnson–Cousins and near-infrared effective wavelengths (U 0.36, B 0.44,
    V 0.55, R 0.66, I 0.81, J 1.25, H 1.65, K 2.2 µm), a mid-infrared point at 10 µm on the extended
    infrared power law, and `Radio`, whose ratio is zero.

## Tasks

Parallelism: T1 → T2 → T3 is one chain. T4 and T7 are independent of it and of each other. T5 needs
T2. T6 needs T2–T5. T8 needs T6 and T7. T9 needs T6. T10.a needs T9, T10.c needs T8, and T10.b needs
T11.a, which has no dependency of its own and is done first so that no unit reaches the wire before
the guide allows it. T11.b needs T10.a, T11.c needs T10.c. T12 needs T8 and can run beside T9–T11.

Gates: every task leaves `just ci` green. Where a task's own tests are marked
`#[ignore = "slow: …"]`, its gate is `just ci-slow`, which is `just ci` plus `just test-slow`; the
tasks that add slow tests are T4.b, T5, T6.b and T12. Benchmarks run under `just bench` and are never
part of either gate.

Owner gates: T11.a needs the owner's confirmation of the four units and the typography ruling it
asks for (see Risks), and P07.T12's Milky Way tuning should follow plan 02's P02.T11, which is
blocked on the owner's density rulings and may move the gas disc's mass (see Risks).

### P07.T1 Gas parameters and units

Build `gas::params`: `GasParams`, `MolecularDisc`, `LaneParams`, `GasParams::from_galaxy` and
`GasParams::milky_way_like`, following Design note 3. The gas mass, R_g and h_n are read from plan
02's `GasDiscParams::{mass, length, HEIGHT}` and never redrawn. Register the domain tag `gas.params`
(scope `Galaxy`) in plan 01's `rng/tags.rs` under a "Plan 07" heading **appended after the Plan 06
event tags**, since the macro's order fixes `tags::ALL`; each parameter is one `Stream::uniform_in` at
its fixed word index, reached with `Stream::seek`. Add the newtypes `HydrogenPerCm3`,
`KelvinPerCm3`, `PerCm2`, `Magnitudes` and `Micrometres` to `units` through its `unit!` macro, and to
`units::consts` the SI constants `HYDROGEN_MASS_KG` and `BOLTZMANN_CONSTANT` (CODATA; cite);
centimetres per light-year is derived in `gas` from the existing `consts::METRES_PER_LIGHT_YEAR`
(Design note 2). Add `pub mod gas;` to `galaxy/mod.rs`'s module list, alphabetically after
`pub mod frame;`. Each range in the table is re-checked against the brainstorm's bullet "Dust and
gas" and, for the hole form, McMillan (2017). Add any author name this task's doc comments cite to
`crates/hyperion-sim/clippy.toml`'s `doc-valid-idents` if `doc-markdown` rejects it.

- Files: `crates/hyperion-sim/src/galaxy/gas/{mod.rs,params.rs}`,
  `crates/hyperion-sim/src/units.rs`, `crates/hyperion-sim/src/rng/tags.rs`,
  `crates/hyperion-sim/src/galaxy/mod.rs`, `crates/hyperion-sim/tests/gas.rs`.
- Tests: every parameter inside its range over 2,000 seeds; the same seed gives the same parameters;
  changing the draw index order is caught by a golden file (`tests/golden/gas/params.golden`, three
  seeds, written with `GoldenWriter`); for every seed the corona alone, n_cor at the pressure floor,
  is hotter than 10⁵ K (Design note 12). The unit tests sit in `mod tests` inside `gas::params`; the
  golden is an integration test in `tests/gas.rs`.
- Acceptance: `cargo test -p hyperion-sim --lib galaxy::gas::params` and
  `cargo test -p hyperion-sim --test gas` pass; plan 02's and plan 03's golden files are unchanged
  (nothing is drawn on an existing stream and the version does not move here). Plan 01's
  `tests/golden/rng/tags.golden` **does** change, by one line for the new tag, and is regenerated
  with `just bless`.

### P07.T2 Smooth components and mass normalisation

Build `gas::smooth`: the four components of Design note 4 without lanes, their normalisations by the
fixed quadrature of Design note 4 (plan 02's `quad::gl_log_panels`, which integrates in ln R; its
Gauss–Legendre tables hold 4, 8, 16 and 32 points already, so no new table), `mean_density`, the
azimuthal mean in the plane, and closed-form vertical columns (2 h × mid-plane density per
exponential layer). Where a column is a difference of two vertical integrals, write the clamp at 0
out as `if difference > 0.0 { difference } else { 0.0 }` and never as `.max(0.0)`, the idiom plan 02's
private `map::across_pixel` documents: `f64::max` may return either of two inputs that compare equal,
so `−0.0` can leak through.

- Files: `crates/hyperion-sim/src/galaxy/gas/smooth.rs`.
- Tests: a brute-force numerical integral of 1.4 m_H × n_disc over the cube returns the gas mass to
  2%, for the MW fixture and 20 seeds. With MW values: the plane at 26,000 ly has 0.6–0.9 cm⁻³ of
  neutral and 0.025–0.035 cm⁻³ of warm ionised gas, and the molecular disc's central density is
  20–80 cm⁻³. For every seed: the neutral density inside R = R_m ÷ 8 is under 1% of its peak (the
  hole), and the corona is n_cor everywhere.
- Acceptance: `cargo test -p hyperion-sim --lib galaxy::gas::smooth` and `--test gas` pass; no `f64`
  transcendental is called outside `math` (`just lint`, which is Clippy's `disallowed_methods` from
  `crates/hyperion-sim/clippy.toml`).

### P07.T3 Lanes from the arm geometry

Add the lane factor of Design note 6 on plan 02's `SharpArm::new(geometry, width, fraction)`, which
already takes a caller's σ_w and A, evaluated through `ArmGeometry::point_polar(R + δR, θ)`. Nothing
in plan 02's arms module changes: the re-validation confirmed the constructor, and `SharpArm`'s own
doc example builds a lane with it.

- Files: `crates/hyperion-sim/src/galaxy/gas/lanes.rs`.
- Tests: the azimuthal mean of the lane factor is 1 to 10⁻⁹ at twenty radii; the lane ridge lies at
  smaller radius than the young-disc arm ridge at the same azimuth, by d ÷ cos p to 5%; the factor
  is 1 well inside the bar's half-length; between the arms it is 1 − A to 1%; at 26,000 ly with MW
  values its peak is 2.5–3.5.
- Acceptance: `cargo test -p hyperion-sim --lib galaxy::gas::lanes` passes; P07.T2's mass test still
  passes with lanes on (`--test gas`).

### P07.T4 Lattice noise

- **P07.T4.a Lattice values and interpolation.** `gas::noise`: register `gas.noise` (scope `Galaxy`)
  under T1's "Plan 07" heading; the counter packing of Design note 8 as the `u64` of
  `ObjectKey::galaxy_item(n)` (rejecting coordinates outside the root cube, ±`ROOT_HALF_WIDTH_LY`;
  the lattice planes on the cube's far faces are inside the 20-bit range), the lattice value from
  `Stream::standard_normal`, the quintic fade, the variance-normalised trilinear interpolation of
  Design note 7. Acceptance: `cargo test -p hyperion-sim --lib galaxy::gas::noise` and `--test gas`
  pass, covering: at a lattice point the value equals the lattice normal exactly; the value is
  continuous across cell faces (difference under 10⁻¹² for points 10⁻⁹ ly either side); over 10⁵
  seeds at one fixed interior point the sample passes `hyperion_testkit::stats::ks_one_sample`
  against N(0, 1) through `assert_p_value` at `stats::ALPHA`; golden values for six points
  (`tests/golden/gas/noise.golden`). `tests/golden/rng/tags.golden` gains its second line and is
  blessed.
- **P07.T4.b Octaves, the log-normal and the smoothing scale.** `OCTAVE_WAVELENGTHS_LY`, the
  amplitudes, `log_normal_factor`, `SmoothingScale`. Acceptance: Σ a_k² = 1 to 10⁻¹⁵; the ensemble
  mean of F over 10⁶ seeds at a fixed point is 1 within four standard errors, the standard error
  taken from the log-normal's own variance e^(σ²) − 1, for σ_ln of 2.0 and 2.5 and for `Full` and
  `AtLeast(250 ly)`; ln F has variance σ_eff² to 2%. These are marked
  `#[ignore = "slow: 10⁶ draws"]` in `tests/gas_statistics.rs` and run under `just test-slow`, so the
  gate is `just ci-slow`; a fast counterpart over 10⁴ seeds with the same tolerance rule runs under
  `just ci`.
- **P07.T4.c Noise cache and benchmark.** `NoiseCache`, with a test that results are bit-identical
  with a cache of any capacity, including one entry, and after `clear`. Criterion benches in a new
  bench target, which needs `[[bench]] name = "gas"` with `harness = false` in
  `crates/hyperion-sim/Cargo.toml` (a shared file; the edit is three lines): one `Full` evaluation
  cold, and the mean cost per evaluation along a 32 ly-step line with a 4,096-entry cache. Targets to
  validate, not promises: 2 µs cold, 0.5 µs on a line. Acceptance: `just bench -- gas` runs them and
  the figures are recorded in the bench file's module doc, as `benches/galaxy.rs` records plan 02's.

### P07.T5 Pressure and phases

`gas::pressure` and `gas::phase` by Design notes 11 and 12, with `GasPhase` and the neutral-share
rule. Re-check the 3,800 K cm⁻³ calibration against Jenkins and Tripp (2011). The floor exists for
the brainstorm's 2–4 Myr maximum shell window; here the check is that P ÷ k never falls below
`pressure_floor`, and at R = 26,000 ly is within 1% of it by |z| = 8 h_P, and P07.T12 pins the
window itself.

- Files: `crates/hyperion-sim/src/galaxy/gas/{pressure.rs,phase.rs}`.
- Tests: MW plane pressure at 26,000 ly within 3,400–4,200 K cm⁻³; monotone non-increasing in |z|;
  floor respected over 10⁴ random positions and 100 seeds (slow: 10⁶ evaluations, so
  `#[ignore = "slow: …"]` in `tests/gas_statistics.rs`, with a fast counterpart over 8 seeds); phase
  thresholds ordered for every pressure; the analytic hot filling factor at the MW plane is 0.17–0.41
  across σ_ln 2.0–2.5 and rises with σ_ln; far above the disc (|z| = 20,000 ly) the phase is `Hot`
  for every seed.
- Acceptance: `cargo test -p hyperion-sim --lib galaxy::gas::pressure`,
  `--lib galaxy::gas::phase` and `--test gas` pass, and `just ci-slow` for the 100-seed sweep.

### P07.T6 The `GasField` facade, state at a site, bound and hooks

- **P07.T6.a Facade and hooks.** Assemble `GasField` (`new`, `mean_density`, `density`,
  `density_with`, `pressure`, `phase`, `dust_per_hydrogen`, `state`), `GasState` with `temperature`
  (Design note 12) and `thermal_sound_speed` (c² = γ P ÷ ρ, γ = 5/3, ρ = 1.4 m_H n), and
  `gas::modifiers` (`GasModifier`, `GasModifierSource`, `NoModifiers`, the point rule of Design note
  16). `state` is what plan 09's P09.T15 and P09.T16 read as `SiteGas`, with
  `SmoothingScale::AtLeast(250 ly)`; plan 09 as it now reads calls `GasField::state` directly and
  names no `gas::state_at` or `gas::state_smoothed`, so those two names are dropped. Add
  `Galaxy::gas()` to `galaxy/mod.rs` between `fields()` and `shares()`, with the field built in
  `Galaxy::from_params` after `Fields::new` (which `GasField::new` borrows) and added to
  `Galaxy::heap_bytes`; `GasField` must derive `Debug`, `Clone` and `PartialEq` and be
  `Send + Sync + RefUnwindSafe`, which the handle's own tests assert, so it holds no `Cell`,
  `RefCell` or `OnceCell`. Revise the `Galaxy` doc's build-time and heap figures for what the field
  adds. The hazard API is `density_with`, documented as the quantity a sublight radiation and erosion
  load is proportional to. Bump `GENERATOR_VERSION` here. Tests: `state` with `AtLeast(250 ly)` at 10⁴
  positions has the
  same mean as `mean_density` within the statistical tolerance and a smaller log-variance than
  `Full`; the pressure in a `GasState` is never below the floor; a point inside a hole returns the
  interior density and outside it is unchanged; a cloud adds its central density at its centre; ζ is
  1 where [M/H] = 0 and never above 10^0.5; a determinism test (two fields from one seed agree bit
  for bit at 1,000 points; different seeds differ); a golden file (`tests/golden/gas/field.golden`)
  of density, pressure and phase at twelve positions for two seeds.
- **P07.T6.b Cell bound for the neutral gas.** `neutral_bound(cell)`, which plan 09 needs to thin
  clouds and star-forming regions against the dust field. It follows the brainstorm's rule under
  "Exact placement by thinning": the vertical factor exp(−|z| ÷ h_n) never rises with |z| and takes
  its nearest-corner value; the radial factor exp(−R_m ÷ R − R ÷ R_g) is unimodal in R with its peak
  at √(R_m R_g) and is bounded from the cell's range of R (`CellBox::r_cyl_range`) by the rule plan
  02's `UnimodalFactor` states; the lane factor is `SharpArm::sup(radii, phase)` over the cell's
  `ArmGeometry::phase_range` at the shifted radii, rounded outward, which already carries plan 02's
  `BOUND_MARGIN`, `COS_SLACK` and `ROUNDING_SLACK` (the last two `pub(crate)`, which `galaxy::gas`
  may read). `SharpArm::sup` is called directly, not through the trait: its only implementor,
  `ArmAcross`, is reachable only from `Arm::across`, and an `Arm` owns its `SharpArm` by value.
  `CellBox::new` refuses an edge that is not a power of two, so the test's 128 ly and 4,096 ly cells
  are legal and any other size must be a power of two too. The molecular disc's nearest-corner value
  is added. The bound is on the mean field; a consumer that thins against the noisy field multiplies
  by its own cap on the log-normal factor. Because this plan's gas layers are exponential in height,
  the ratio of the gas density to an exponential proposal g(z) still peaks at the height nearest the
  plane, so plan 09's Design note 21 bound holds for its clouds as written — unlike the stellar
  discs, which plan 02's R18 says plan 09 must re-bound. If the owner rules that the gas disc is
  cored too (see Risks), that changes and plan 09 must be told.
  Tests: a violation hunt over 10⁶ random points in 10⁴ random 128 ly and 4,096 ly cells, weighted to
  the lanes near the bar's ends and to R near √(R_m R_g), finds none; the bound is within a factor of
  1.5 of the true maximum on cells away from lanes. The hunt is
  `#[ignore = "slow: 10⁶ bound checks over 10⁴ cells"]` in `tests/gas_statistics.rs`, with a fast
  suite of a few cells per size under `just ci`, as plan 02's P02.T8 splits its own hunt.
- Files: `crates/hyperion-sim/src/galaxy/gas/{field.rs,modifiers.rs,bound.rs}`,
  `crates/hyperion-sim/src/galaxy/mod.rs`, `crates/hyperion-sim/src/version.rs`,
  `crates/hyperion-sim/tests/{gas.rs,gas_statistics.rs}`.
- Acceptance: `just ci` green and `just ci-slow` for T6.b's hunt. The `GENERATOR_VERSION` bump moves
  no generated value, so `just bless` changes only the header line of every tracked golden — plan
  01's, plan 02's six `galaxy_*.golden`, plan 03's, and the server's
  `crates/hyperion-server/tests/golden/{galaxy_parameters,density_map_face_on_128}.golden`, which
  carry the same header — and `golden_diff.py` must report "header only, consistent" for all of them.

### P07.T7 Cardelli–Clayton–Mathis law and bands

`gas::ccm`: `extinction_ratio` as a(x) + b(x) ÷ R_V with x = 1 ÷ λ in µm⁻¹, in the paper's four
ranges: infrared 0.3 ≤ x < 1.1 (a = 0.574 x^1.61, b = −0.527 x^1.61), optical and near-infrared 1.1
≤ x < 3.3 (the two seventh-order polynomials in y = x − 1.82), ultraviolet 3.3 ≤ x ≤ 8 (with the
F_a, F_b terms above 5.9) and far ultraviolet 8 < x ≤ 10. Below x = 0.3 the infrared power law is
continued; above x = 10 the function returns an error variant, not an extrapolation. Coefficients
are copied from Cardelli, Clayton and Mathis (1989), equations 1–5, and re-checked digit by digit.
`Band`, `R_V`, `HYDROGEN_COLUMN_PER_MAG`.

- Files: `crates/hyperion-sim/src/galaxy/gas/ccm.rs`.
- Tests: ratio at V is 1 to 1%; at K (2.2 µm) 0.105–0.125, the brainstorm's "about a ninth"; A_B ÷
  A_V − 1 equals 1 ÷ R_V to 3%; continuity at x = 1.1, 3.3, 5.9 and 8 to 2%; monotone rising with x
  from 0.3 to 4.5; `Radio` is exactly zero; golden values at the ten bands
  (`tests/golden/gas/ccm.golden`).
- Acceptance: `cargo test -p hyperion-sim --lib galaxy::gas::ccm` and `--test gas` pass. This task
  has no dependency (not even on T1, once `Magnitudes` and `Micrometres` exist) and can be done
  first; if it runs before T1, it adds those two newtypes itself and T1 reuses them.

### P07.T8 The extinction line integral

- **P07.T8.a Integrator, mean mode, symmetry.** `gas::extinction`: slab clipping, canonical end
  point order, step rule with lod = 0, two-point Gauss–Legendre, `NoiseMode::Mean`, `Sightline` with
  `a_v`, `in_band`, `reddening`, `hydrogen_column`. Work in cell-plus-offset coordinates and form
  each sample position as integer cell plus offset, never as one `f64`: `GalacticPosition::cell()`
  and `offset_metres()` are the pair, and `to_cylindrical()` gives R and z as **`Metres`**, which the
  field converts to light-years once per sample. Acceptance:
  `cargo test -p hyperion-sim --lib galaxy::gas::extinction` and `--test gas` pass, covering: a
  vertical line from the plane to the cube's edge matches the closed-form column to 0.1%; A(a, b) and
  A(b, a) are bit-identical for 10⁴ random pairs; a zero-length segment returns zero; a segment
  wholly outside the slab takes no steps.
- **P07.T8.b Realised mode, quality and cost control.** `NoiseMode::Realised` with the `NoiseCache`;
  `Quality::Budget` with the lod rule and `AtLeast(2 Δ_noise)`. Acceptance: over 256 seeds the mean
  of `Realised` on a fixed 3,000 ly in-plane segment agrees with `Mean` within four standard errors,
  for `Full` and for `Budget(64)`; `Budget(N)` never takes more than N steps; results are identical
  with and without a warm cache.
- **P07.T8.c Modifiers.** Step splitting at hole boundaries and the algebraic Plummer column of
  Design note 16. Acceptance: a line through the centre of a hole of radius r in a uniform test
  field loses exactly the column of 2r; a cloud's column along a long line through its centre is 4 ÷
  3 × core radius × central density to 10⁻⁹, equal in both directions, and matches a brute-force
  numerical integral to 10⁻⁶ on 100 random segments; with an empty modifier list every result is
  bit-identical to the call without modifiers.
- **P07.T8.d Neutral column and horizon.** `neutral_hydrogen_column` by the rule of Design note 12,
  and `horizon`. Acceptance: neutral ≤ total on every line; on in-plane MW lines of 3,000 ly the
  neutral share is 0.8–1.0 in both modes; `horizon` for `Band::V` with a 5 mag limit, looking
  coreward in the MW plane from 26,000 ly in mean mode, is 7,000–13,000 ly; it is longer for `K`
  than for `V` on every tested line, and returns `max_range` for `Radio`.
- **P07.T8.e Benchmarks.** Criterion, in T4.c's `gas` bench target: the line from 26,000 ly to the
  centre, `Realised`, `Full` (target 3 ms); the same at `Budget(64)` (target 0.2 ms); 2,000 lines of
  500 ly from one origin with a shared cache (target 100 ms on one core). Acceptance:
  `just bench -- gas` runs them and the figures are recorded in the bench file's module doc; a miss is
  a finding, not a failure.

Files for all subtasks: `crates/hyperion-sim/src/galaxy/gas/extinction.rs`,
`crates/hyperion-sim/benches/gas.rs`, `crates/hyperion-sim/tests/gas.rs`.

### P07.T9 Extinction map functions

`gas::map`, from the mean field, in the shape of plan 02's `galaxy::map` (P02.T10):
`extinction_face_on`, `extinction_edge_on` and `render_extinction_rows` over plan 02's `MapSpec`
(whose `selection` is ignored, and whose `width_px`/`height_px` are `u32`), so that the server can
split a map into bands of rows exactly as plan 04's `DensityMapService` does. Follow `render_rows`'s
contract: clear `out`, fill `rows.len() × width_px` values row by row, and panic if the rows reach
past the raster. Take each pixel's geometry from `MapSpec::pixel_centre(column, row)` face-on and
`MapSpec::pixel_span(row)` edge-on, whose consecutive rows share an edge bit for bit.

Face-on is closed form per sample (the sum of 2 h × ζ × mid-plane density over the layers, the
molecular disc included; ζ reads the cylindrical radius alone, so it factors out of the vertical
integral exactly), averaged over the pixel on a fixed 4 × 4 sub-grid because a lane is narrower than
a pixel — so `extinction_face_on` takes a `pixel_ly` where plan 02's `column_density_face_on` takes a
`MapSelection` and no pixel size. Edge-on is the mean-mode integrator along +y through the whole cube,
integrated across the pixel's height by the closed form for each exponential layer, as P02.T10.b does
for the stellar discs, since the neutral disc is thinner than a pixel; the gas layers keep their own
exponential closed forms (plan 02's cored `VerticalProfile` belongs to the stellar discs and is not
read here), and each pixel's column is written as a difference of two decreasing exponentials clamped
at 0 with the `if difference > 0.0 { … } else { 0.0 }` idiom, never `.max(0.0)` (P07.T2). Plan 02's
own edge-on machinery (`EdgeOnPlan`, its `pixel`, `across_pixel`, `render_edge_on_rows`, `MAX_PARTS`,
`Part`) is private to `galaxy::map`, so the plan-means-lines split is written afresh in `gas::map`
rather than reused; factoring a line of sight per column of a band, as P02.T10.b does, changes no
value, since a gas line of sight depends on x alone.

- Files: `crates/hyperion-sim/src/galaxy/gas/map.rs`, `crates/hyperion-sim/tests/gas.rs`,
  `crates/hyperion-sim/benches/gas.rs`.
- Tests: the face-on MW value at 26,000 ly, averaged around the circle, is 0.3–0.45 mag (twice the
  polar extinction); the face-on map's lane pixels exceed their inter-arm neighbours at the same
  radius by a factor of at least 1.5; the edge-on central pixel exceeds 25 mag; both views are
  symmetric under z → −z; rows rendered in two bands equal rows rendered in one, bit for bit; a 64 ×
  64 golden map for one seed (`tests/golden/gas/map.golden`).
- Acceptance: `cargo test -p hyperion-sim --lib galaxy::gas::map` and `--test gas` pass; a 512 × 512
  face-on map computes in under 1 s and the edge-on raster at the same resolution, which is 512 × 256
  (plan 04's `MapResolution::height_px(view)` halves the height edge-on), in under 10 s on one core
  (`just bench -- gas`, a finding if missed). Expect the edge-on target to be the finding: plan 02
  measured its own 512 × 256 stellar raster at 64 s on a loaded machine against a 5 s target (its
  R20), and while a gas pixel is far cheaper than a spheroid's two-dimensional quadrature, it is a
  numerical line integral of some hundreds of steps. Record the measured figures either way; plan 04's
  eight workers divide them.

### P07.T10 Protocol and server

Every message here extends plan 04's request convention exactly as its "Extending the convention"
says: a variant on `RequestBody` and on `ResponseBody` with the same `kind`, the string in
`REQUEST_KINDS`, a wire-form test for each inside its `request` or `response` envelope, a handler,
then `just gen-protocol`. No `ClientMessage` or `ServerMessage` variant is added, no body carries a
request ID, errors are `request_error` with plan 04's `ErrorCode` and `field`, and `PROTOCOL_VERSION`
stays as it is (plan 04's design note 15). The re-validation found the full list of places a new kind
touches, none of which may be missed because most are exhaustive matches that will not compile
without the new arm:

- `crates/hyperion-protocol/src/envelope.rs`: the `RequestBody` and `ResponseBody` variants,
  `REQUEST_KINDS`, the enumeration walks `next_request` and `next_response` (wildcard-free matches),
  and the literal list in `request_kinds_are_pinned`. The tests
  `request_kinds_lists_every_variant` and `response_kinds_are_the_request_kinds` then hold for free.
- `crates/hyperion-protocol/src/lib.rs`: the `pub use` re-export lists.
- `crates/hyperion-server/src/requests/mod.rs`: `Handlers::handle`, `kind(body)` and `is_large(body)`
  (a new kind must say whether its response can run to megabytes: `extinction_map`'s can,
  `extinction`'s cannot), the `every_body()` test fixture, which `kind_names_every_body_as_the_wire_does`
  asserts is `REQUEST_KINDS.len()` long, and the unserved filter in
  `the_handlers_refuse_every_kind_until_its_handler_exists` — which lives **here**, in
  `src/requests/mod.rs`, not in `tests/`.
- `crates/hyperion-server/tests/universes.rs`: the `refusals` array of
  `a_save_from_another_generator_version_is_listed_as_a_mismatch_and_refused`, since both kinds name a
  universe.

No TypeScript file changes per kind: `RequestKind`, `RequestOf<K>` and `ResponseFor<K>` are derived
from the generated `RequestBody`, so `useServerRequest<"extinction_map">` works as soon as the Rust
variants exist.

- **P07.T10.a Extinction map.** Kind `extinction_map`.
  `ExtinctionMapRequest { universe: UniverseIdHex, view: MapView, resolution: u16, bits: u8 }`,
  validated as plan 04 validates `DensityMapRequest` (resolutions 128–1,024, 8 or 16 bits).
  `ExtinctionMap` carries `universe`, `view`, `width_px`, `height_px`, `centre_ly`, `ly_per_px` and
  `bits` as `DensityMap` does, then `floor_log10_mag`, `ceiling_log10_mag` and `data_base64`, with
  plan 04's extents, row order and code conventions (its design note 12); code 0 is "at or below the
  floor", the floor is 0.01 mag and the ceiling the grid's maximum. Generalise plan 04's quantiser to
  `quantise_map_with_floor(&RawDensityMap, floor_log10: f64, depth: CodeDepth)` — a `CodeDepth`, not a
  `bits: u8`, since that is what `quantise_map` takes and what `TryFrom<u8>` already validates — and
  have `quantise_map(&RawDensityMap, MapView, CodeDepth)` call it with `span_dex(view)` below the
  ceiling, reusing its private `code_of`, its tests and fixture unchanged. Rename `QuantisedMap`'s
  `floor_log10_per_ly2`/`ceiling_log10_per_ly2` to `floor_log10`/`ceiling_log10` and fix the two call
  sites in `convert::density_map` and the quantiser's tests; the wire field names do not change.
  `compute::ExtinctionMapService` in `compute/extinction_map.rs`, shaped like `DensityMapService`
  (which lives in `compute/density_map.rs` beside the quantiser): it holds the `Arc<GalaxyCache>` and
  fetches its own galaxy, keyed by an `ExtinctionMapKey` of (`GalaxyKey`, view, `MapResolution`) —
  reusing `MapResolution`, whose `height_px(view)` takes the view and whose `TryFrom<u16>` is the
  `resolution` check, and `MAP_WIDTH_LY` for the extent — with the raw grid in a `SharedByteLru` of
  its own byte budget from `ServerConfig`, built through `SingleFlight` as bulk jobs of
  `BAND_ROWS` rows (16; the constant is private to `compute/density_map.rs`, so make it `pub(crate)`)
  calling `render_extinction_rows`, each band's pixels turned to `f32` by the sim's `math::log10`, and
  quantised per request. `AppState` gains the service beside `maps`, and `ServerStats` an accessor for
  its `LruCounters`, as `ServerStats::maps()` does. The handler follows `requests::galaxy::map`:
  `openable_universe` first, then the raw grid, then one `try_submit(Priority::Interactive, token, …)`
  job that quantises and builds the response, with `requests::respond` serialising the frame as a
  second job because `is_large` says so. `decodeExtinctionMap` in `@hyperion/protocol` shares the
  decoder of `decodeDensityMap` through one new internal function in
  `packages/protocol/src/densityMap.ts` and returns `log10Mag(code)`. Tests: wire forms of request and
  response; a fixture `packages/protocol/fixtures/extinction_map_4x2.json` pinned from both languages,
  in the shape of `density_map_4x2.json` (a description, the decoded values, the codes and the map)
  and read by `packages/protocol/src/densityMap.test.ts` and a Rust unit test as that one is; a map
  built with one worker equals one built with four; the second `get` is a cache hit; a golden raster
  `crates/hyperion-server/tests/golden/extinction_map_face_on_128.golden` beside plan 04's.
- **P07.T10.b Gas parameters** (needs T11.a). A `gas` group in `GalaxyParameters` with every entry of
  Design note 3's table that this plan draws, under dotted keys (`gas.hole_scale`, `gas.warm_fraction`,
  `gas.warm_height`, `gas.molecular_fraction`, `gas.molecular_length`, `gas.molecular_height`,
  `gas.corona_density`, `gas.pressure_floor`, `gas.pressure_height`, `gas.pressure_speed`,
  `gas.sigma_ln`, `gas.lane_offset`, `gas.lane_width`, `gas.lane_fraction`), each marked drawn,
  derived or fixed through `convert.rs`'s `drawn`/`derived`/`fixed` helpers, plus `gas.scale_height`
  as `fixed`, which this task moves out of `EXCLUDED_PARAMETERS` (its exclusion reads "a constant of
  the generator, the same for every seed", which no longer stands once the gas field reads it). The
  table's first three rows are **not** re-sent: `gas.mass` is already a `derived` entry in the `mass`
  group and `disc.gas.scale_length` a `derived` entry in `discs`, both from P04.T14.b, and a dotted
  key is unique across the whole response, so re-using `gas.mass` under a second group would collide
  with the existing entry in the client's flat `PARAMETER_LABELS` map. Design note 3's table
  cross-references those two rather than duplicating them. Add the `Unit` values `PerCm3`, `KPerCm3`
  and `Mag`; lengths in light-years, the mass in M☉ and σ_P in km/s use plan 04's existing units.
  `PARAMETERS_ACCOUNTED_FOR` (95 = 80 sent + 15 excluded) rises by the group's size plus one for the
  height moved out of the exclusions, and
  `crates/hyperion-server/tests/golden/galaxy_parameters.golden` is regenerated with `just bless`. On
  the client, **two** switches are exhaustive over `Unit` and both need the three new cases:
  `components/UnitLabel.tsx`'s `unitSymbol` and `parameterReadings.ts`'s private `numberText`. The
  labels go in `parameterLabels.ts`'s `GROUP_LABELS` (a `gas` entry) and `PARAMETER_LABELS`, each
  three words or fewer as the guide's upper-case limit requires, and the group must also join
  `test/galaxyFixtures.ts`'s `everyGalaxyParameter`, which `parameterLabels.test.ts` holds to the
  glossary both ways — or `pnpm test` fails. Tests: wire form of the group; `UnitLabel` renders each
  new unit with an accessible name; `numberText` writes each new unit's precision.
- **P07.T10.c Extinction request.** Kind `extinction`. `ExtinctionRequest` carries `universe`,
  `origin: GalacticPosition`, `time: UniverseTime` and `targets: Vec<ExtinctionTarget>`, with
  `ExtinctionTarget` tagged by `type`: `System { id: SystemIdHex }` or
  `Position { position: GalacticPosition }`, at most 64 targets (a position is some 130 bytes of
  JSON and plan 04's inbound frame limit is 16 KiB; the constant joins `limits.rs`).
  `ExtinctionResult { universe, origin, time, targets: Vec<TargetExtinction> }` in request order,
  `TargetExtinction` tagged by `status`:
  `Ok { a_v_mag, e_b_v_mag, a_k_mag, hydrogen_column_per_cm2, neutral_hydrogen_column_per_cm2 }` or
  `NoSuchSystem`. The handler calls `openable_universe` first, as every handler naming a universe
  does, then checks the time against ±H, the origin and positions against the root cube and the target
  count, each through a `convert::ConvertRequestError::new(field, reason)` so that the refusal is
  `bad_request` with the right `field`; converts the wire `UniverseTime` to the sim's
  `hyperion_sim::time::UniverseTime`, which is a different type of the same name; resolves each system
  target as `SystemId::from_raw(hex.to_u64())` (there is no `SystemIdHex → SystemId` conversion and no
  `SystemId::to_u64`; the getter is `raw()`) and then `placement::resolve` and
  `query::position_at(.., time)`, reading the record's `epoch_position()` as a `&GalacticPosition`, as
  plan 04 requires of every ID from a client, a failure giving that target `NoSuchSystem` and not
  failing the request; runs `sightline` in `Realised` mode at a fixed `Quality::Budget(256)` with
  `NoModifiers`, as one interactive pool job through `CpuPool::try_submit` (which refuses with
  `queue_full` rather than waiting) with one `NoiseCache` owned by the job; and caches results in a
  `SharedByteLru` keyed by (seed, generator version, end points, quality), with a `HeapBytes` impl for
  `Sightline`. `is_large` classifies `ExtinctionResult` as small, so its frame is serialised on the
  runtime. Tests: wire forms; integration tests over the WebSocket with `TestServer` and
  `TestClient::request` in a new `crates/hyperion-server/tests/extinction.rs`: a request from position
  a to position b and the request from b to a return identical figures; an unknown system gives
  `NoSuchSystem` beside good targets; 65 targets give `bad_request` naming `targets`; and the kind
  joins `tests/universes.rs`'s mismatch test.
- Files: `crates/hyperion-protocol/src/{galaxy.rs,envelope.rs,lib.rs}`;
  `crates/hyperion-server/src/`: `compute/{extinction_map.rs,density_map.rs,mod.rs}`,
  `requests/{mod.rs,galaxy.rs}`, `convert.rs`, `limits.rs`, `lib.rs` (`AppState`), `config.rs` (the
  extinction map cache's budget), `stats.rs`; `crates/hyperion-server/tests/`:
  `extinction.rs`, `universes.rs`, `golden/extinction_map_face_on_128.golden`;
  `packages/protocol/src/generated/` by `just gen-protocol`, `packages/protocol/src/densityMap.ts`
  and its test, `packages/protocol/src/index.ts`, `packages/protocol/fixtures/extinction_map_4x2.json`;
  and for T10.b the four client files named there.
- Acceptance: `just gen-protocol-check` and `just ci` green after each subtask.

### P07.T11 Client: map layer and chart readout

- **P07.T11.a UX guide** (no dependency; done before T10.b). Edit `docs/frontend/ux-guidelines.md`,
  whose relevant sections as built are "Numbers, units and time" (its units list and its E-notation
  rule), "Graphs, schematics and spatial displays" (its raster-field rule) and the nomenclature list.
  - **Units.** Allow as astronomers' units beside SI the magnitude (`mag`, which plan 06 does not add:
    its unit list is `MetalFraction`, `HeliumExcess`, `Gauss` and `SolarMassesPerYear`), the column
    density, the number density and the pressure over Boltzmann's constant. Write them in the form the
    guide's existing composed units use (`°/Myr`, `/ly³`, `SYSTEMS/ly²`): **`/cm²`, `/cm³` and
    `K/cm³`**, not `cm⁻²`, `cm⁻³` and `K cm⁻³`. The guide states that B612 "has no superscript digits
    beyond `¹`, `²` and `³` and no superscript minus", so a superscript minus one, two or three cannot
    be set at all, while `²` and `³` can; the composed forms need no new typography rule. Each unit
    needs the owner's confirmation, as plan 05's additions did.
  - **Typography, for the owner.** Whether powers of ten and negative exponents may be set with
    `<sup>`, and subscripts such as the V of A_V with `<sub>`, is a **question for the owner**, not
    this task's to settle: the guide's standing rule is that `10⁻⁴` "cannot be set" and E notation is
    used instead, and plan 05 built `lib/format.ts`'s `formatSci` and `DensityLegend`'s tick labels on
    that ruling. `<sup>` and `<sub>` would work — they shift and scale ordinary glyphs rather than
    calling for superscript characters — but allowing them reverses a shipped rule and would raise the
    question of what becomes of `formatSci` and the legend's ticks. Until the owner rules, this task
    keeps E notation and writes the nomenclature names in parentheses.
  - **Rasters.** Allow a second raster quantity on the galaxy map with its own legend, and an overlay
    only when both legends are shown and the overlay is named on the display. The rule as built allows
    one raster, one hue and one mandatory legend, and already names dust among the fields it covers
    ("such as column density or dust"), so this is an extension of the rule rather than a new one.
  - **Nomenclature.** Add `EXTINCTION`, `DUST OVERLAY`, `A(V)`, `A(K)`, `E(B-V)` and `N(H)` to the
    list, in the parenthesised form until the owner rules on `<sub>`, with `-` as the guide's signed
    form (plan 05's ruling 8) rather than the typographic minus. The guide's "One name per thing" rule
    means nothing on the console may use an abbreviation that is not on this list.
- **P07.T11.b Galaxy map.** A quantity selector (`SYSTEMS`, `EXTINCTION`, and a `DUST OVERLAY` toggle
  available under `SYSTEMS`), keyboard-operable with its key shown, a display control and styled as
  one. It belongs in `GalaxyMapPanel.tsx` beside the `POPULATION` choice it already owns, and is passed
  to both `GalaxyMapView`s as a prop: the panel holds **no** request, and the only
  `useServerRequest` in the map pipeline is in `GalaxyMapView.tsx`, so that is where
  `useServerRequest<"extinction_map">` goes, at `MAP_BITS` and `MAP_TIMEOUT_MS` as the density map's
  is, with the resolution from `mapResolutionFor(backingWidthPx)` and the same turn
  (`turnClockwise` face-on) as plan 05's ruling (2) fixes. `mapGeometry` needs **no** widening: it
  already takes the narrow structural `MapRaster` (`view`, `width_px`, `height_px`, `centre_ly`,
  `ly_per_px`), which an `ExtinctionMap` satisfies unchanged. The extinction layer uses the same
  single-hue ramp, logarithmic in magnitudes, with a new `ExtinctionLegend.tsx` beside
  `DensityLegend.tsx` (whose title, unit and floor text are hard-coded, so it cannot be reused),
  titled `EXTINCTION A(V)`, the unit `mag`, the words `LOG SCALE` and the stated floor. It reuses
  `GalaxyMapView`'s validity checks (the four `MAP DATA INVALID` causes, through `StatusLine` with
  `RETRY`) and its stale handling (`StaleMark`, the `--text-muted` ramp, the trailing `S` and the
  canvas name), which plan 05's rulings (3) and (8) fix, so a map kept after the link drops is marked
  stale as the guide requires. Pending, rejected and timed-out states are plan 05's `RequestStatus`.
  The overlay lowers each pixel's log₁₀ density by 0.4 A_V before the ramp is applied, clamped to the
  ramp's floor. Because plan 05's codes are linear in log₁₀ of the quantity, that is a shift of the
  density map's **code** by −0.4 A_V × (maxCode − 1) ÷ span, clamped at 0 (code 0 being "at or below
  the floor"), so the rule is one pure function over two `DecodedCodes` of the same resolution and
  turn, in `lib/galaxy/mapPicture.ts` beside `turnClockwise` and `reducedLevels` — not in
  `lib/galaxy/ramp.ts`. It is applied once, after the turn and before either paint path, which plan
  05's ruling for T8.d requires ("applies per map pixel before `reducedLevels`") and which is also the
  only place that serves both paths: `GalaxyMapView` paints natively through `ramp.ts`'s
  `rasterise(codes, ramp)` and reduced through `reducedLevels` then `paintLevels`, and the two take
  different inputs. Both legends are shown, and the overlay's label names it and its quantity, as
  T11.a's guide rule requires: `DUST OVERLAY: A(V), WHOLE LINE OF SIGHT`. Note that
  `decodeExtinctionMap`'s accessor is `log10Mag`, not `log10PerLy2`, so any shared helper takes the
  accessor as an argument.
- **P07.T11.c Chart readout.** When a system is selected the chart requests its extinction from the
  chart centre at the chart's time and the readout gains four rows: `A(V)`, `E(B-V)`, `A(K)` in `mag`
  to two decimals, and `N(H)` in `/cm²` in E notation through `formatSci` (there is no formatter that
  writes a mantissa and a separate power of ten, and the guide's standing rule is E notation; if the
  owner allows `<sup>` under T11.a, this row changes with it). The request lives in a new
  `useExtinction.ts` beside `useRangeQuery.ts`, on
  `useServerRequest<"extinction">(body, timeoutMs, generation)` with one `System` target, so a newer
  selection supersedes the older request on the client and cancels it on the server. It is called from
  `useLocalChart.ts`, which already holds the selection (`chosenId`, with `selected` derived each
  render), the chart time and the answer's centre, and its result joins `LocalChartState`; the rows
  are rendered by `SystemReadout.tsx`, which is purely presentational and is rendered by
  `SystemsPanel.tsx` in the display's second column, **not** inside `LocalChartPanel`, so
  `SystemReadoutProps` gains the reading and the `RequestState` as props and `SystemsPanel` passes
  them from `LocalChartState`. Until the response arrives, and for `NoSuchSystem` or a failed request,
  the rows show the guide's missing state — the em dash `—` in `--text-muted`, which
  `SystemReadout`'s own `Reading` already renders for a `null` value — with `RequestStatus` beside
  them, never zero.
- Files: under `apps/hyperion/src/renderer/src/`: in `displays/galaxy/`, `GalaxyMapPanel.tsx`,
  `GalaxyMapView.tsx`, a new `ExtinctionLegend.tsx` beside `DensityLegend.tsx`, `SystemReadout.tsx`,
  `SystemsPanel.tsx`, `useLocalChart.ts` and a new `useExtinction.ts` beside `useRangeQuery.ts`;
  `lib/galaxy/mapPicture.ts`; `test/galaxyFixtures.ts` (`anExtinctionMap`, `anExtinctionResult`);
  `styles.css` for the new legend's block; and their tests.
- Tests: component tests with `FakeWebSocket.serverAnswers` for the selector, the legend's unit, the
  overlay label, the four readout rows, the missing state while pending and a changed selection; a
  pure-function test for the overlay rule, including a code that falls to the floor and one at the
  ceiling.
- Acceptance: `just ci` green, which runs `pnpm typecheck`, `pnpm lint` (`oxlint --type-aware
--deny-warnings`, a root-only script) and `pnpm test` as well as the Rust gate; one file at a time
  with `pnpm --filter hyperion exec vitest run <path>`. By eye, dark lanes lie on the inner edges of
  the young population's arms in the face-on overlay.

### P07.T12 Milky Way and statistical verification

Slow tests at MW parameters and over seeds, marked `#[ignore = "slow: …"]` in
`crates/hyperion-sim/tests/gas_statistics.rs` and run by `just test-slow`. This task also tunes the MW
starting values of Design note 3 until all of them pass, changing parameters and never the targets.

**It should follow plan 02's P02.T11**, which is blocked on the owner's rulings and which may raise
the gas disc's mass to restore the vertical pull when it lowers the stars' surface density (plan 02's
own T11 text, and the brainstorm's open question "The fixture's gas"). Every figure below is computed
from `GasDiscParams::mass()`, so tuning against a mass P02.T11 then moves would have to be redone. If
T12 runs first, its tuned values are provisional and the task is run again after P02.T11.

Because it changes drawn ranges and the MW fixture's values, T12 **bumps `GENERATOR_VERSION` a second
time** and re-blesses every golden, this plan's included; only P07.T6's bump is free of value changes.

- Mean-mode extinction along in-plane lines of 3,000 ly centred at R = 26,000 ly, averaged over 64
  azimuths: 0.8–1.3 mag, the brainstorm's "about one magnitude per 3,000 ly". The `Realised` average
  over the same lines and 32 seeds agrees with the mean-mode figure within four standard errors.
- From (26,000 ly, 0, 0) to the centre, mean mode: A_V of 24–38 mag and A_K of 2.6–4.5 mag, the
  brainstorm's "some thirty" and "about three". The same line in `Realised` mode over 256 seeds: the
  mean agrees within four standard errors, and the median and the 16th and 84th percentiles are
  recorded in the test's output and in this plan's Risks, because with σ_ln above 2 and a molecular
  disc only a lattice cell or two across, a typical seed's line lies well below the mean. A median
  under 10 mag is a finding for the owner, not a failure.
- The shell window's ceiling, which is what the pressure floor is for. With the closed form plan
  09's P09.T15.a will own, copied into the test until then (W = t_PDS × [¾ (v_PDS ÷ β c_net)^(10⁄7)
  - ¼], t_PDS = 1.33 × 10⁴ yr × n^(−4⁄7), v_PDS = 413 km/s × n^(1⁄7), at 10⁵¹ erg and solar
    metallicity, β = 2, c_net² = γP ÷ ρ + (8 km/s)²; Cioffi, McKee and Bertschinger 1988; re-check),
    the largest W over a fixed scan of n from 10⁻³ to 10 cm⁻³ at P = `pressure_floor` is 2–4 Myr for
    the MW fixture and for every seed of 200, and at the MW plane's pressure the largest W is 0.5–1
    Myr and falls at n of 0.1–0.5 cm⁻³. If the floor's drawn range of 300–500 K cm⁻³ misses the
    band, the range moves, not the band.
- Filling factors by Monte Carlo: over 10⁵ points in the plane at R = 20,000–30,000 ly the hot share
  is 0.20–0.40 for MW σ_ln, and above 0.95 at |z| = 20,000 ly; the molecular share in the plane is
  under 2%.
- Mean preservation in space: the volume average of F over a 16,384 ly cube sampled at 10⁶ points is
  1 within the tolerance implied by its correlated variance (documented in the test).
- Over 200 seeds: gas mass within 2% of plan 02's parameter; every parameter in range; plane
  pressure positive and above the floor.
- Determinism: a golden file (`tests/golden/gas/sightlines.golden`) of ten `Sightline`s for two
  seeds, `Full` and `Budget(64)`; order independence (the ten lines computed in two orders and
  singly, with shared and fresh caches, agree bit for bit).
- Acceptance: `just ci-slow` green (which is `just ci` plus `just test-slow`); the tuned MW values are
  written back into `GasParams::milky_way_like` and this plan's table, and every golden is regenerated
  with `just bless` in the same commit as the version bump.

## Verification

- `just ci` after every task; `just ci-slow` (which adds `just test-slow`) after T4.b, T5, T6.b and
  T12; `just bench -- gas` for T4.c, T8.e and T9's map costs, which are findings and never gates. The
  brainstorm's tests that fall to this plan are covered by P07.T4.b (mean preservation), P07.T12 (one
  magnitude per 3,000 ly, thirty and three magnitudes to the centre, a hot filling factor of a fifth
  to two fifths, the 2–4 Myr ceiling on the shell window that the pressure floor sets, determinism,
  order independence), P07.T7 (the infrared ratio and radio) and P07.T8.a (two-way visibility).
- `just test-wasm` at the end of the plan, as plan 02's revision did, so that the goldens hold on a
  second architecture with a 32-bit `usize`. It is not part of either gate and needs wasmtime.
- By eye, in the `GALAXY` display: the extinction layer shows a disc with a hole inside the bar, a
  bright central spot, and lanes that start at the bar's ends and trail; the overlay puts the lanes
  on the inner edges of the young arms; edge-on, the layer is a thin dark band. Selecting systems on
  either side of the plane shows reddening rising towards the plane and the centre.
- A review check that nothing in plans 02 and 03 reads the gas field: stars never read it.

## Generator version

- Adding the field moves no star: all draws are on `gas.params` and `gas.noise`, and no existing
  stream gains a draw. `GENERATOR_VERSION` is bumped in P07.T6 because a universe of the earlier
  version has no gas and readouts would differ; that bump moves no generated value, so every tracked
  golden changes only its header line, `# generator_version = <n>`, which `just bless` rewrites and
  `golden_diff.py` reports as "header only, consistent". The server's two goldens
  (`galaxy_parameters.golden`, `density_map_face_on_128.golden`) carry the same header and move with
  them. Plan 01's `tests/golden/rng/tags.golden` gains a line in P07.T1 and another in P07.T4.a, which
  is a content change and not a header change.
- A **second** bump falls in P07.T12 if its tuning moves any drawn range or any value of
  `GasParams::milky_way_like`, which it is expected to, and then this plan's own goldens move too.
- After that, any change to the parameter rules, the component forms, the octave table, the
  interpolation, σ_P, h_P, the phase thresholds or the integrator's step rule changes output and
  bumps the version. The integrator's rule is output because plan 09's shell test and plan 12's
  magnitudes will read it.
- Reserved: the two domain tags; the request kinds `extinction_map` and `extinction`; draw indices
  0–15 on `gas.params` in the table's order, later parameters appending; octave indices 5–15 in the
  noise counter; the `GasModifier` enum's two variants for plan 09; `SmoothingScale` as the only way
  a consumer selects a scale. The `Unit` values `PerCm3`, `KPerCm3` and `Mag` and the `gas` parameter
  group's keys are reserved on the wire, not in the generator version, and follow plan 04's rule that
  a shipped key is never renamed.

## Risks and open points

- **The corona's density and the pressure floor do not describe one gas.** A corona of 10⁻³ cm⁻³ at
  the 10⁶ K usually quoted has P ÷ k near 2,300 K cm⁻³, not the 300–500 the shell window's 2–4 Myr
  maximum needs. The brainstorm states both figures, so both are kept as independent parameters,
  which implies a corona near 2 × 10⁵ K far from the disc. The brainstorm never states the corona's
  temperature, so nothing it says is contradicted, and of its two figures the one with a stated
  outcome, the 2–4 Myr window borne out by the oldest known remnant, is the one P07.T12 pins. The
  density range is trimmed to 0.5–1.2 × 10⁻³ cm⁻³ so that the corona classifies as hot for every
  seed (Design note 12). A corona whose density falls outward would reconcile them and is a later
  refinement with a version bump. The window test uses plan 09's closed form and its 8 km/s
  turbulent term; at a floor of 400 K cm⁻³ that form gives a ceiling a little under 2 Myr, so
  P07.T12 may move the floor's range down towards 250–400, and plan 09's P09.T15.b then re-pins the
  cap with its production code.
- **The smooth molecular disc is a tenth of the real central molecular zone's mass** (Design note
  5), so that the mean extinction to the centre is the brainstorm's thirty magnitudes, which is the
  outcome the brainstorm states; it gives the central disc no mass. The rest must arrive as plan
  09's clouds. Plan 09's design note 19 places clouds in proportion to this plan's smooth neutral
  density plus a weighted molecular term, because the smooth molecular disc holds 5 × 10⁻⁴ of the
  gas and plain proportion would put only a cloud or two at the centre. Plan 09 solves the weight so
  that its clouds carry nine times `MolecularDisc`'s mass, and P09.T4.c tests it.
- **Thirty magnitudes is the mean, not a typical line.** The noise is the brainstorm's
  (mean-preserving, σ of 2–2.5), and the brainstorm also says the centre lies behind some thirty
  magnitudes. Both hold only on average: half of this plan's 28 mag comes from a molecular disc a
  few lattice cells across, where one log-normal factor with a median a fourteenth of its mean
  decides the realised figure. P07.T12 records the realised distribution. If the owner wants the
  typical seed to read thirty, the remedy is a narrower σ_ln inside the central few hundred
  light-years, which is a change to the brainstorm and not made here.
- **The mean and a typical line differ.** With σ_ln above 2 the median of the factor is a tenth of
  its mean or less, so a single short line is usually well below the mean extinction and
  occasionally far above it. That is the physics of a cloudy medium and the tests are written on
  means, but a console showing 0.002 mag to a star 50 ly away is expected, not a bug.
- **The hole is not in the potential.** Plan 02's potential treats the gas as a plain exponential
  disc. The gas is 15% of the thin disc, so the inner rotation curve is a few per cent high, which
  the research behind the brainstorm already accepted for the stars.
- **Plan 05's reservation is not used.** Plan 05 left `MapPopulation` open for a dust value on the
  density map request. Design note 19 explains why extinction is a request kind of its own. Plan
  05's union stays as it is.
- **Plan 02's interface, settled.** `SharpArm::new(geometry, width, fraction)` already takes a
  caller's width and fraction, so this plan needs nothing new from plan 02's arms module; the gas
  metallicity is `Component::metallicity(&PointLy, Years::ZERO).mean()` for the young thin disc, which
  reads the cylindrical radius alone. Plan 02's potential keeps its own plain double-exponential gas
  disc (its D15), which the 2026-09-21 cored-profile revision did not touch; the two agree in mass,
  scale length and height.
- **Band ownership.** Plan 06's `stellar::photometry` works in V and B − V only and defines no band
  type, so `Band` lives here. A later sensor plan may move it; see Design note 20.
- **Cost on long-range charts.** A chart of thousands of bright stars at 5,000 ly is thousands of
  lines of some 150 steps each. The budgeted quality and the pool keep it under a second, but the
  extinction request is capped at 64 targets for that reason, and plan 12 may want a shared radial
  cache. (This bullet read 256 before the re-validation, against P07.T10.c's 64; 64 is the figure,
  since 256 positions of about 130 bytes of JSON each would exceed plan 04's 16 KiB inbound frame.)
- **Constant scale heights.** The real neutral layer flares outward. A flare is addable later as a
  height that grows with R; nothing here depends on the height being constant except the closed-form
  face-on column, which stays closed-form.
- **Re-validated at 0c4c64e** (2026-09-22), with plans 01 and 02 complete (plan 02 through T10; its
  T11 blocked on owner rulings), plan 03 built through T7, T9 and T12.a, plan 04 through T14.c and
  plan 05 complete. Every Consumes item was checked against a real signature by grep, not from
  memory, and Consumes was rewritten to name them. The substantive corrections: `GasDiscParams`'s
  getters are `mass()`, `length()` and `height()` over the fixed const `HEIGHT`, not `radial_scale` or
  `neutral_height`; `Stream::seek` takes a word number, not a draw index, and the samplers are methods
  on `Stream`; `ObjectKey::galaxy_item` takes a `u64`; `GalacticPosition` has `offset_metres()` and a
  `to_cylindrical()` in **metres**; `coords::CellSize` is dropped as unusable here; the mass
  normalisation's quadrature is `quad::gl_log_panels`, not `gl_panels`, which integrates in R;
  `SharpArm::new` already takes a caller's σ_w and A, so P07.T3's conditional work on plan 02 is
  struck; `Component::metallicity` takes a `&PointLy` and an age and reads R alone, and plan 02 already
  clamps its mean at +0.5 dex; plan 02's edge-on map machinery is private, so P07.T9 writes its own;
  the clamp of a column difference at 0 is written out, never `.max(0.0)`; plan 04's quantiser takes a
  `CodeDepth` and a `MapView` and its `QuantisedMap` getters are density-named, so
  `quantise_map_with_floor` takes a `CodeDepth` and the getters are renamed unit-neutral;
  `MapResolution::height_px` takes the view and `BAND_ROWS` is private; `SystemId` has `from_raw`/`raw`
  and no `to_u64`; the sim's and the wire's `UniverseTime` are different types; the full list of sites a
  new request kind touches is now written out in T10, including the two enumeration walks and the
  refuse-every-kind test, which lives in `src/requests/mod.rs` and not in `tests/`; on the client the
  map request is in `GalaxyMapView` and the selector belongs in `GalaxyMapPanel`, `mapGeometry` needs
  no widening, the overlay rule is a code shift in `lib/galaxy/mapPicture.ts` rather than a level rule
  in `ramp.ts` (plan 05's T8.d ruling puts it before `reducedLevels`, which is also the only place that
  serves both of `GalaxyMapView`'s paint paths), and the chart's readout lives in `SystemsPanel` in
  another column, so its request belongs in `useLocalChart`. Acceptance commands were given to every
  task that had none, bare `cargo test` filters replaced by `--lib galaxy::gas::<module>` and
  `--test gas`, `just ci-slow` named as the gate of T4.b, T5, T6.b and T12, `[[bench]] name = "gas"`
  added to `crates/hyperion-sim/Cargo.toml`'s list, and T10's ordering split (T10.a needs T9, T10.c
  needs T8). The generator-version section now records the second bump T12's tuning forces, and the
  cap on the extinction request was made 64 everywhere. Nothing about what is built changed; the five
  questions below are for the owner.
- **For the owner: the fixture's gas column.** The brainstorm's open questions now include "The
  fixture's gas": plan 02's gas disc gives 6.6 M☉ per square parsec at the Sun's radius against McKee
  et al.'s measured 13.7 ± 1.6, and raising it would restore the vertical pull near the plane when
  P02.T11 lowers the stars' surface density. This plan's own field, with the hole pushing mass outward
  and the warm layer added, gives 7.9 M☉ per square parsec there — still 1.74 times short. The two
  knobs are not equivalent, and the choice is the owner's:
  - Raising the **mass** by 1.74 (plan 02's drawn fraction 0.10–0.20 becoming about 0.17–0.35) raises
    the mid-plane density with it, and the in-plane extinction goes from 1.1 to 1.9 mag per 3,000 ly,
    outside the brainstorm's "about one magnitude per 3,000 ly" and outside P07.T12's 0.8–1.3 bracket.
  - Raising the **mass and the height together** by 1.74 (h_n 400 → about 700 ly, the fraction as
    above) leaves the mid-plane density and so the in-plane extinction untouched at 1.1 mag, and raises
    the polar extinction from 0.19 to 0.33 mag and the 21 cm column with it. This is the physically
    natural reading — the real neutral layer's column is carried well above 123 pc — and it reconciles
    both of the brainstorm's figures, but h_n is plan 02's fixed `GasDiscParams::HEIGHT` and the mass
    enters the potential, so either change moves every star and belongs to plan 02's D15 with a version
    bump and a re-check of its rotation-curve and enclosed-mass brackets.
  - Or leave both as they are and read the 7.9 as the price of meeting the extinction figure, which is
    the outcome the brainstorm states and the one this plan is built on.
- **For the owner: does ruling 1 bind the gas disc?** The 2026-09-21 rulings' first ruling is headed
  "Each disc is exponential in radius and cored in height", and its text and its reason (Bovy 2017's
  measured star counts, the far-field thin and thick heights) are about the stellar discs' age cohorts.
  The brainstorm's "Between the stars" says only "a thin neutral disc a few hundred light-years tall"
  and gives the gas no vertical form, so nothing it says is contradicted by Design note 4's
  exp(−|z| ÷ h_n), and plan 02 kept the gas disc a plain double exponential in the potential. But a
  real gas layer is hydrostatic and cored at the plane for the same reason a stellar one is, and this
  plan's own Design note 11 is a hydrostatic pressure. If the owner rules that the gas is cored too,
  three things follow: P07.T2's closed-form vertical columns and P07.T9's face-on closed form become
  `VerticalProfile::integral_to` differences with its clamp at 0 (which plan 02's R18 and `across_pixel`
  already handle); P07.T6.b's vertical factor still never rises with |z|, so its bound holds; but plan
  09's Design note 21, which bounds density ÷ g(z) at the height nearest the plane, stops holding for
  its clouds as well as for its stellar-disc features, and plan 09 must be told. As the gas stands,
  exponential, that bound does hold for the clouds and plan 09 need re-bound only what follows the
  stellar discs.
- **For the owner: typography.** P07.T11.a's four units are written `mag`, `/cm²`, `/cm³` and `K/cm³`,
  the form the guide's existing `/ly³` and `SYSTEMS/ly²` use, because the guide records that B612 has
  no superscript minus and so `cm⁻²` cannot be set. Whether `<sup>` and `<sub>` may be used — which
  would allow `cm⁻²`, `A_V` and `10⁻⁴`, since they shift ordinary glyphs rather than needing superscript
  characters — reverses the guide's standing E-notation rule, on which plan 05 built `formatSci` and
  `DensityLegend`'s ticks. Until it is ruled on, the nomenclature names are `A(V)`, `A(K)`, `E(B-V)` and
  `N(H)` and `N(H)` is written in E notation.
- **For the owner: the four units themselves**, and the second raster quantity with its overlay, which
  P07.T11.a adds to the guide. Plan 05's own guide additions needed the owner's confirmation and these
  are of the same kind.
- **Pending re-validation.** Nothing in this plan waits on unbuilt code: every Consumes item exists.
  P07.T12's Milky Way figures wait on plan 02's P02.T11, as its own text now says, and P07.T11.a waits
  on the owner.
- **T1 and T2, as built (2026-09-22).** T1 is complete: `units.rs`'s five newtypes with
  `BOLTZMANN_CONSTANT` and `HYDROGEN_MASS_KG` (CODATA, tested), the `gas.params` tag under a "Plan 07"
  heading after plan 06's event tags (`tags.golden` gained exactly one line,
  `gas.params (Galaxy) = 0x80346fd71dc90f62`), `gas/mod.rs` (the cm-per-ly conversions,
  `MASS_PER_HYDROGEN_FACTOR`, `SOLAR_MASSES_PER_LY3_AT_UNIT_DENSITY` = 9.978 × 10⁻⁴,
  `IONISED_PARTICLES_PER_HYDROGEN`), `gas/params.rs` (`GasParams`, `MolecularDisc`, `LaneParams`,
  `from_galaxy`, `milky_way_like`, `neutral_fraction()`, `pressure_speed()`, `PRESSURE_HEIGHT` and
  `PRESSURE_SPEED`), and `tests/gas.rs` with `golden/gas/params.golden` over plan 02's three golden
  seeds. T2 is `gas/smooth.rs`: `SmoothGas` and `GasLayer`, both `pub` with a doctest, taking bare `f64`
  light-years and returning cm⁻³ or cm⁻² as the module doc says of hot paths. It normalises with
  `quad::gl_log_panels` over eight log-spaced panels from 1 ly to 20 R_g (10⁻⁷ relative across the drawn
  ranges), and its closed-form vertical columns are clamped without `f64::max`. No plan 02 or 03 golden
  moved, and no parameter range disagreed with the brainstorm or McMillan (2017). Measured at the
  fixture (version 8), against the brackets and the analytic predictions below:

  | Quantity                       | Measured    | Predicted | Bracket     |
  | ------------------------------ | ----------- | --------- | ----------- |
  | Neutral, plane, 26,000 ly      | 0.673 cm⁻³  | 0.66      | 0.6–0.9     |
  | Warm ionised, plane, 26,000 ly | 0.0297 cm⁻³ | 0.029     | 0.025–0.035 |
  | Molecular centre               | 42.1 cm⁻³   | 41        | 20–80       |
  | Worst hole at R_m ÷ 8          | 0.33%       | 0.2–0.5%  | under 1%    |
  | Mass closure (brute force)     | −1.9 × 10⁻⁶ | —         | 2%          |

  The `galaxy::gas` tests take about 0.9 s together, so none is marked slow. P02.T11 raises the gas
  disc's mass and scale height by the same 1.74 (ruling 1), which leaves the three mid-plane figures
  unchanged; re-read them after it merges, as a check on that ruling's arithmetic.

- **Ruling on design note 3's word indices** (the note was ambiguous: "parameter _k_ of the table is
  `seek(k)`" against seventeen table rows, but "indices 0–15 reserved"). The **eleven drawn parameters
  take words 0–10 in table order**; words 11–15 stay reserved; the derived and constant rows get no word
  at all. Design note 3's wording should be corrected to say that, since only drawn parameters consume
  words.
- **T1's prose "each parameter is one `Stream::uniform_in`" is wrong**: the table makes `f_c` and `n_cor`
  **log-uniform**. Both still cost exactly one word, so the index rule above is unaffected.
- **The Milky Way fixture as built** differs from the table: `R_g` is **14,840 ly** (1.75 × 8,480), not
  14,000, and the gas mass is 5.06 × 10⁹ M☉. Use the code's values.
- **T2's mass test cannot integrate "over the cube" to 2%.** The normalisation integrates to 20 R_g, and
  about **11% of the neutral mass lies outside the ±65,536 ly root cube** (analytic estimate).
  **As measured, 5.6% for the fixture**: the 11% estimate took a cylinder inscribed in the cube and
  missed its corners. At the largest drawn scale lengths up to about 20% lies outside. Integrate
  over the component's own support instead, and report the in-cube fraction as a separate figure.
  - An exact cross-check on `gl_log_panels` for that integral:
    ∫₀^∞ R e^(−a/R − R/b) dR = 2 a b K₂(2 √(a/b)).
  - Analytic predictions for T2's brackets, from the parameters as built and **not yet measured**:
    neutral ≈ 0.66 cm⁻³ (bracket 0.6–0.9), warm ionised ≈ 0.029 (0.025–0.035), molecular centre ≈ 41
    (20–80), and the hole at R_m ÷ 8 is 0.2–0.5% of peak for every seed, against the "under 1%" test.
  - The corona-hot check is load-bearing at its limit: the worst case is
    300 ÷ (2.3 × 1.2 × 10⁻³) = 1.09 × 10⁵ K against design note 12's 10⁵ K floor.
- Clippy's `doc_markdown` rejects bare `R_m`, `h_w`, `σ_ln` and the like in doc comments — backtick
  them. No `clippy.toml` change was needed (`McMillan` and `McKee` are already in `doc-valid-idents`),
  and note that the transcendental bans now also come from a **workspace-root `clippy.toml`** which the
  sim's own file shadows.
