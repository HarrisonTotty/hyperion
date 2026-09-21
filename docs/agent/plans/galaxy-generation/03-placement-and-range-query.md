# Plan 03: Placement and the Range Query

- **Milestone:** M1.
- **Depends on:** [01 Determinism foundation](01-determinism-foundation.md),
  [02 Galaxy model](02-galaxy-model.md).
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/galaxy-generation.md)): "Placing star systems"; "Exact
  placement by thinning" (the algorithm; the bound functions are plan 02's); the placement parts of
  "Sizing the layers"; the ID-resolution paragraph of "Identifiers" ("A well-formed ID does not
  always name a system"); the position-drawing and frame-selection paragraphs of "Coordinates"; the
  primary-mass bullet of "Systems and stars"; "Star formation continues" under "Events in time"
  (ages from −H, "no system yet"); "The range query" in full; the cache, purity and benchmark
  bullets of "Runtime and code shape"; the items of "Testing" named under
  [Verification](#verification); step 3 of "Suggested order of attack" (what must be reserved).

## Goal

When this plan is done, `hyperion-sim` can place every field star system of a galaxy and answer
"which systems lie within R light-years of this point at time t?". Five independent grids, layers A
to E with cells of 8 to 128 ly, are filled by exact thinning against plan 02's bounds. Each accepted
candidate becomes a `SystemRecord`: ID, epoch position, population, primary initial mass and age. A
`SystemId` read from anywhere resolves in constant time to its record or to "no such system". The
range query walks the layers from coarsest to finest, decides a complete census or nothing per layer
from expected counts before generating anything, pads for motion, filters the unborn, and merges
non-grid sources through a hook that later plans fill. The frame-selection rule is a pure function
over the query's results. All of it is pure, cache-agnostic, benchmarked and covered by golden,
statistical and order-independence tests.

## Scope and non-goals

In scope:

- The layer table (A–E), cell keys, candidate counts, candidate streams, thinning, the population
  pick, primary initial mass, age, `SystemRecord`, whole-cell generation, `resolve`.
- The range query: request and result types, cell walk, expected counts over the sphere, the census
  rule, mass floor, time argument, padding, unborn filter, drift hook, merge hook, result ordering.
- The frame-selection rule (distance ÷ tidal radius, with hysteresis).
- A cache interface the caller implements, and the byte size of what it would hold.
- Benchmarks and the tests listed under [Verification](#verification).

Not in scope:

- Density fields, bounds, the share matrix, the mass function, age distributions, tidal radius:
  plan 02. This plan calls them and never re-derives them.
- Streams, samplers, IDs, coordinates, units, time, the golden harness: plan 01.
- Protocol messages, the server's LRU cache and CPU pool: plan 04.
- Velocities (plan 08), stellar state (plan 06), features, catalogue classes, the central black
  hole's scanning rule (plan 09), the global list (plan 10), substellar layers (plan 13), pinned
  content (a later overlay plan). This plan leaves a hook for each and says where.
- Metallicity and multiplicity, which the brainstorm's pipeline puts in the system stage below
  placement.

## Provides

All under `hyperion_sim::galaxy`. Signatures are sketches.

### `galaxy::placement`

```rust
/// One row of the layer table. `STELLAR_LAYERS` is ordered coarsest (E) to finest (A).
pub struct LayerSpec { /* layer: Layer, cell_ly: u32, band: MassBand */ }
pub const STELLAR_LAYERS: [LayerSpec; 5];
pub fn layer_spec(layer: Layer) -> Option<&'static LayerSpec>;   // None for non-stellar layers
pub fn layer_for_initial_mass(mass: SolarMasses) -> Option<Layer>; // None outside 0.08–150 M☉

/// A generation cell of one stellar layer: plan 01's `coords::GenCell` plus the layer that owns it
/// (a cell size alone does not name a layer: brown dwarfs reuse 16 ly).
pub struct CellKey { /* layer: Layer, cell: GenCell */ }
impl CellKey {
    pub fn new(layer: Layer, cell: [i32; 3]) -> Result<Self, BuildCellKeyError>;
    pub fn layer(&self) -> Layer;
    pub fn gen_cell(&self) -> GenCell;
    pub fn cell_box(&self) -> CellBox;        // plan 02's bound argument; never straddles a plane
    pub fn containing(layer: Layer, position: &GalacticPosition) -> Result<Self, BuildCellKeyError>;
    pub fn of(id: SystemId) -> Result<Self, ResolveSystemError>;
    pub fn origin_ly(&self) -> [i32; 3];
    pub fn size_ly(&self) -> u32;
    pub fn index_capacity(&self) -> u32;      // 2^(16 + 3k)
    pub fn candidate_id(&self, index: u32) -> Option<SystemId>;
}

pub fn candidate_count(galaxy: &Galaxy, key: CellKey) -> u32;

pub enum CandidateOutcome { Accepted(SystemRecord), Thinned, ClaimedByCatalogue }
pub fn evaluate_candidate(galaxy: &Galaxy, key: CellKey, index: u32) -> CandidateOutcome;

/// A system as placed: state at the epoch, never a position at some other time.
pub struct SystemRecord { /* id, epoch_position, origin, population, primary_initial_mass,
                            age_at_epoch */ }
impl SystemRecord {
    pub fn from_parts(..) -> Self;            // for non-grid sources and tests
    pub fn id(&self) -> SystemId;
    pub fn layer(&self) -> Layer;
    pub fn epoch_position(&self) -> &GalacticPosition;
    pub fn origin(&self) -> SystemOrigin;     // where the record came from (Design note 18)
    pub fn component(&self) -> Option<ComponentId>; // `Some` for `SystemOrigin::Grid` only: plan
                                              // 02's flat index, valid for this galaxy's `Fields`
    pub fn population(&self) -> Population;   // stored, so no `Galaxy` and no component is needed
    pub fn primary_initial_mass(&self) -> SolarMasses;
    pub fn age_at_epoch(&self) -> Years;      // signed; down to −H
    pub fn age_at(&self, t: UniverseTime) -> Years;
    pub fn existence_at(&self, t: UniverseTime) -> Existence;
}
pub enum Existence { NoSystemYet, Exists }
/// What made a record. `Grid` is the only variant in M1. Plan 09 adds `FeatureMember(FeatureId)`,
/// `CentreMember` and `CatalogueSystem(ClassId)`, and plan 10 `GlobalListMember`; none of them
/// carries a component, and no grid record changes when they arrive.
#[non_exhaustive]
pub enum SystemOrigin { Grid(ComponentId) }

pub fn generate_cell(galaxy: &Galaxy, key: CellKey, out: &mut Vec<SystemRecord>);
pub fn cell_heap_bytes(systems: &[SystemRecord]) -> usize;

pub fn resolve(galaxy: &Galaxy, id: SystemId) -> Result<SystemRecord, ResolveSystemError>;
pub enum ResolveSystemError { NoSuchSystem, LayerNotGenerated(Layer), KindNotGenerated }

/// Called once by whoever builds a `Galaxy` for play (plan 04's universe registry).
pub fn check_index_headroom(galaxy: &Galaxy) -> Result<(), ExceedIndexCapacityError>;

/// The caller's cache. `hyperion-sim` holds none; `NoCache` regenerates into a scratch buffer.
pub trait CellCache {
    fn with_cell<R>(&mut self, galaxy: &Galaxy, key: CellKey,
                    f: impl FnOnce(&[SystemRecord]) -> R) -> R;
}
pub struct NoCache { /* scratch: Vec<SystemRecord> */ }
```

Domain tags added to plan 01's single `domain_tags!` registry in `rng/tags.rs`, under a "Plan 03"
heading (never renamed afterwards): `galaxy.cell.candidates` (scope `Cell`), and with scope
`System`: `galaxy.candidate.position`, `galaxy.candidate.accept`, `system.primary_mass`,
`system.age`, and, reserved and unused until plan 08, `system.velocity`. The constants are
`tags::GALAXY_CELL_CANDIDATES` and so on.

### `galaxy::query`

```rust
pub struct RangeQuery { /* centre, radius, time, limit, mass_floor, substellar, cell_budget */ }
impl RangeQuery {
    pub fn builder(centre: GalacticPosition, radius: LightYears) -> RangeQueryBuilder;
}
impl RangeQueryBuilder {
    pub fn time(self, t: UniverseTime) -> Self;               // default: the epoch
    pub fn limit(self, limit: NonZeroU32) -> Self;            // default DEFAULT_CENSUS_LIMIT
    pub fn mass_floor(self, floor: MassFloor) -> Self;        // default MassFloor::LayerA
    pub fn substellar(self, request: SubstellarRequest) -> Self; // default None
    pub fn cell_budget(self, cells: NonZeroU32) -> Self;      // default DEFAULT_CELL_BUDGET
    pub fn build(self) -> Result<RangeQuery, BuildRangeQueryError>;
}
pub enum MassFloor { LayerE, LayerD, LayerC, LayerB, LayerA }  // plan 13 adds two steps below
pub enum SubstellarRequest { None, BrownDwarfs, BrownDwarfsAndRoguePlanets }

pub struct SystemHit { /* record: SystemRecord, position (at t), distance: LightYears */ }
pub struct RangeResult { /* systems: Vec<SystemHit>, census: Census, stats: QueryStats */ }
pub struct Census { /* complete_down_to: Option<Layer>, stopped_by: CensusStop,
                       expected: LayerCounts */ }
impl Census {
    pub fn complete_down_to(&self) -> Option<Layer>;          // None: nothing fits
    pub fn complete_above(&self) -> Option<SolarMasses>;      // lower edge of that layer's band
    pub fn stopped_by(&self) -> CensusStop;
    pub fn expected(&self) -> &LayerCounts;                   // per layer, grid plus sources
}
pub enum CensusStop { MassFloor, Limit, CellBudget }
pub struct LayerCounts(/* [f64; 7], indexed by `Layer::value()`; the two substellar entries stay
                          zero until plan 13 */);
pub struct QueryStats { /* cells_visited, systems_examined, padded_radius */ }

pub fn range_query<C: CellCache>(galaxy: &Galaxy, cache: &mut C,
    sources: &[&dyn SystemSource], query: &RangeQuery) -> RangeResult;

// Building blocks, public for plans 04, 09, 10, 12 and the frame rule.
pub struct QuerySphere { /* centre, radius, time, padded_radius */ }
pub fn cells_in_sphere(layer: Layer, sphere: &QuerySphere) -> impl Iterator<Item = CellKey>;
pub fn count_cells_in_sphere(layer: Layer, sphere: &QuerySphere) -> u64;
pub fn expected_counts(galaxy: &Galaxy, centre: &GalacticPosition, radius: LightYears)
    -> LayerCounts;
pub const PAD_SPEED: KilometresPerSecond;                     // 1,000 km/s; this plan owns it
pub fn pad_speed(layer: Layer) -> KilometresPerSecond;        // `PAD_SPEED` for every layer in M1
pub fn pad_for(t: UniverseTime, speed: KilometresPerSecond) -> LightYears;

/// Drift hook. Zero until plan 08 draws velocities on "system.velocity".
pub fn epoch_velocity(galaxy: &Galaxy, record: &SystemRecord) -> GalacticVelocity;
pub fn position_at(galaxy: &Galaxy, record: &SystemRecord, t: UniverseTime) -> GalacticPosition;

/// Merge hook: anything that is not the grid. Empty in M1.
pub trait SystemSource {
    fn expected_in_sphere(&self, galaxy: &Galaxy, sphere: &QuerySphere) -> LayerCounts;
    fn systems_in_sphere(&self, galaxy: &Galaxy, sphere: &QuerySphere, layers: LayerSet,
                         out: &mut Vec<SystemHit>);
    fn suppresses(&self, galaxy: &Galaxy, record: &SystemRecord, t: UniverseTime) -> bool;
}
```

### `galaxy::frame`

```rust
pub struct FrameCandidate { /* id: SystemId, distance: Metres, tidal_radius: Metres */ }
pub const FRAME_HYSTERESIS: f64 = 0.1;
pub fn select_frame(candidates: &[FrameCandidate], current: Option<SystemId>) -> Option<SystemId>;
pub fn frame_at<C: CellCache>(galaxy: &Galaxy, cache: &mut C, sources: &[&dyn SystemSource],
    ship: &GalacticPosition, t: UniverseTime, current: Option<SystemId>) -> Option<SystemId>;
```

### Test helpers

In `crates/hyperion-sim/tests/common/mod.rs`: `sunlike_point(&Galaxy) -> GalacticPosition` (in the
plane, 26,000 ly from the centre, on the +y axis, clear of the bar), `brute_force_in_sphere(..)`
(generates every intersecting cell with `NoCache` and filters by distance, with no census), and
`reference_sphere_integral(..)` (a fine midpoint sum, for checking `expected_counts`).

## Consumes

Names are those of the owning plans' "Provides", which are authoritative. P03.T1 re-checks them
against the code as built before other work starts.

From plan 01, `hyperion_sim`:

- `rng::{Seed, Stream, ObjectKey, tags}` and the single `domain_tags!` registry in `rng/tags.rs`. A
  stream is `Stream::open(seed, tag, key)`, where `key` is `ObjectKey::cell(word)` for a cell
  (`word` = `SystemId::cell_word()`, the ID with its index zeroed) or `ObjectKey::from(id)` for a
  candidate; plan 01 debug-asserts that the key's scope is the tag's. Draws: `next_u64`,
  `word_at(n)` for a word addressed by number, `uniform`, `poisson` (inversion and PTRS), `mark`,
  and `Mark::pick_weighted(weights, bound)`, the allocation-free form of `Thresholds`.
- `units::{LightYears, Metres, SolarMasses, Years, KilometresPerSecond}` and `units::consts`
  (`SPEED_OF_LIGHT`, `METRES_PER_LIGHT_YEAR`).
- `time::{UniverseTime, Span, CLOCK_WINDOW_H, ClockWindow}`.
- From `coords`: `GalacticPosition`, `GalacticDisplacement`, `GalacticVelocity`, `LyCell`,
  `CellSize`, `GenCell` and `ROOT_HALF_WIDTH_LY`; `GenCell::of_ly_cell`, `origin`, `in_root_cube`
  and `position_from_words([u64; 3])`, which draws a position inside a generation cell from one word
  per axis; `GalacticPosition::{displacement_to, distance_to, translated, in_root_cube}`.
- `id::{SystemId, SystemIdKind, GridId, Layer}`: `SystemId::from_parts(layer, GenCell, index)`,
  `SystemId::kind()` giving `SystemIdKind::Grid(GridId)` with `layer()`, `cell()` and `index()`,
  `SystemId::cell_word()`, `Layer::{cell_size, cell_size_ly, index_bits, ALL}`. `Layer` has seven
  variants, A–E and the two substellar layers; the reserved value is not a `Layer`, and its IDs
  arrive as the other `SystemIdKind` variants (decoded, not generated here).
- `GENERATOR_VERSION`; from the dev-only crate `hyperion-testkit`: `golden!` and `GoldenWriter`,
  `order::assert_order_independent`, and `stats` (`chi_square_gof`, `ks_one_sample`,
  `poisson_interval`, `assert_poisson_count`, `assert_p_value`, `ALPHA`); slow tests marked
  `#[ignore = "slow: …"]` under `just test-slow`; Criterion under `just bench`; `just bless`.

From plan 02, `hyperion_sim::galaxy`:

- `Galaxy`: the handle, with `seed() -> Seed`, `params()`, `fields()`, `shares()`,
  `mass_function()`, `potential()`. There is no `bounds()`: the bounds are methods on `Fields`.
- `PointLy`, with `From<&GalacticPosition>`: the argument of every density.
- `fields::{Fields, Component, ComponentId, MAX_COMPONENTS}`: a flat list of density components in a
  fixed order (young thin disc, the old thin disc's five sub-discs, thick disc, bulge, long bar,
  nuclear disc, the halo's components); `Fields::densities(&PointLy, &mut [f64; MAX_COMPONENTS])` in
  systems per cubic light-year before layer shares; `Fields::component(id)`;
  `Component::{population, ages, halo_component}`; `ages::AgeDistribution::sample(&mut Stream)`,
  which takes one uniform and returns a signed age at the epoch in `Years`, from −H where the
  component still forms stars.
- `bounds::CellBox` and `Fields::layer_bound(&ShareMatrix, MassBand, &CellBox) -> f64`: a true upper
  bound on the layer's density over the cell, summed over components in index order.
- `shares::ShareMatrix`: keyed by band and population, with
  `component_share(band, &Component) -> f64` giving a component its population's share. A
  component's density already carries its share of its population, so a layer's density is Σ_c
  `component_share(band, c)` × density_c, and that sum is what the pick of Design note 3 weights by;
  `Fields::layer_density` is the same sum.
- `imf::{MassFunction, MassBand, BandShares, MASS_BAND_EDGES}`: the five band edges (0.08, 0.5,
  0.75, 2.5, 8, 150 M☉) as the single source of truth, `MassBand::try_from(Layer)`, and
  `MassFunction::sample_in_band(band, &mut Stream)`.
- `potential::PotentialTables::tidal_radius(SolarMasses, &PointLy) -> Metres`.
- `GalaxyParams::milky_way_like()` with `Galaxy::from_params`, the fixture the brainstorm's Milky
  Way comparisons use.

## Design notes

Decisions where the brainstorm is silent. None contradicts it.

1. **Cell stream.** A cell's candidate count is drawn on tag `galaxy.cell.candidates` with
   `ObjectKey::cell(word)`, where `word` is the `cell_word()` of the cell's candidate 0
   (`SystemId::from_parts(layer, cell, 0)`): the ID with its index zeroed. The word equals candidate
   0's ID, and that is safe because the tag's scope is `Cell` and every candidate tag's is `System`,
   which plan 01 asserts when a stream is opened.
2. **Candidate draws.** On `galaxy.candidate.position`, keyed by the candidate's ID, words 0, 1 and
   2 go to plan 01's `GenCell::position_from_words`, one word per axis: the top log₂(cell size) bits
   are the integer light-year inside the cell and the next 52 bits the offset fraction. Cell sizes
   are powers of two, so this is exactly uniform, and no `f64` ever spans a whole cell. The layout
   is plan 01's (its Design note 23) and is not restated in code here. On `galaxy.candidate.accept`,
   word 0 is the single acceptance word, taken as a `Mark`.
3. **One draw rejects or picks.** With weights w_c = `component_share(band, c)` ×
   density_c(position) in component index order and the cell's bound B = `Fields::layer_bound`,
   which is Σ `component_share(band, c)` × bound_c, the cumulative sums Σ w ÷ B are turned into
   53-bit thresholds by plan 01's `Mark::pick_weighted(&weights, B)`. The candidate takes the first
   component whose threshold exceeds the acceptance mark, and is thinned if none does. This is the
   brainstorm's "one uniform draw rejects a candidate or picks its class" applied to the grid, in
   the integer-threshold convention. The pick is over components, not populations, because the old
   thin disc's sub-disc and the halo's component decide the age distribution (`Component::ages`).
   The population is the component's parent (`Component::population`), and the halo's component mark
   is `Component::halo_component`. Plan 02's share matrix is keyed by population; a component's
   density already holds its share of its population, so weighting by the population's share is
   exact.
4. **Marks have streams of their own.** Mass on `system.primary_mass`, age on `system.age`, both
   keyed by the candidate's ID, drawn only for accepted candidates. Adding metallicity, velocity or
   multiplicity later moves nothing.
5. **Carve-out hook.** After the marks are drawn, `evaluate_candidate` calls a private
   `catalogue_claims(galaxy, &record) -> bool`, which returns `false` until plan 09. The outcome
   enum already has `ClaimedByCatalogue`, and `resolve` already maps it to `NoSuchSystem`. The hook
   sits after the marks because a class is a test on a candidate's own marks.
6. **Index overflow.** A Poisson draw can in principle exceed a cell's index capacity. Two defences:
   `check_index_headroom` runs once per galaxy and fails if, for any stellar layer, the largest
   possible bound × volume plus eight standard deviations exceeds the capacity (the largest bound is
   the sum of each component's global peak, so no search is needed); and `candidate_count` clamps to
   the capacity with a `debug_assert!`. The clamp is a pure function of the cell's own draw, so it
   is deterministic, independent of order and cache, and the same in `resolve` as in
   `generate_cell`. It belongs to the generator version. It cannot make a census lopsided: a clamped
   cell would be short of candidates in every query alike, and the headroom check exists so that no
   galaxy that could reach the clamp is ever played. `check_index_headroom` is called by whoever
   builds a `Galaxy` for play, which is plan 04's universe registry; nothing in this plan calls it
   on the hot path. At Milky Way values the fullest layer-A cell expects about 7,500 candidates of
   65,536.
7. **Ages are signed years at the epoch.** `age_at(t)` adds the clock time. "No system yet" is
   `age_at(t) ≤ 0`. A record resolves at all times; existence is a question asked with a time.
8. **Census stops at the first layer that does not fit.** Layers are tried from E to A with a
   running expected total. The first layer whose addition would exceed the limit is left out with
   every finer layer, even if a finer one alone would fit (layer B is smaller than C under Kroupa),
   because the result must be statable as "complete above m".
9. **The limit is on expected counts only.** The realised count fluctuates around it and is never
   truncated, since truncation would break the census. Callers size buffers with headroom. The
   default limit is 4,096.
10. **Cell budget.** The brainstorm notes that a 500 ly sphere touches a million cells. In the halo
    such a query has a tiny expected count and would pass the census rule while costing seconds. So
    a second, purely geometric stop: a layer whose intersected cell count would take the running
    total past `cell_budget` (default 2²⁰) is left out like a layer over the limit, and the census
    reports `CensusStop::CellBudget`. It depends only on the query, so it is as deterministic as the
    count rule. It does not weaken "a complete census or nothing, per layer": the budget only ever
    leaves out whole layers, before anything is generated, and the result says which rule stopped
    it. The cell counts are taken over the padded sphere, so they depend on the query's radius,
    centre and time and on nothing else.
11. **Expected counts use a fixed quadrature**, given under P03.T9.c. The unpadded sphere is
    integrated at the epoch, since the process is stationary. The rule's node counts depend only on
    the radius and on whether the sphere crosses z = 0. It belongs to the generator version in the
    weak sense that changing it can change which layers a query returns, never a star.
12. **Query time is limited to the clock window.** `build` rejects |t| > H. Spatial searches use
    present positions, and the brainstorm guarantees those only within H. Retarded evaluation back
    to −(H + L) is evaluation of found objects, which is plan 12's.
13. **Padding speed is per layer behind one function.** `pad_speed(layer)` returns `PAD_SPEED` for
    every layer now. Plan 08 raises it for the unbound class in layer E, which is the brainstorm's
    "only the unbound class needs more".
14. **Result order** is by distance at t, then by ID, using `total_cmp`. It is independent of walk
    order and cache state.
15. **Whole cells only.** `generate_cell` always produces every accepted system of the cell, which
    is what a cache must hold. Skipping candidates outside the sphere before the density evaluation
    would not change output and is kept as a lever if the cold benchmark misses.
16. **Frame rule placement.** `galaxy::frame` is new to the interface sketch. A ship is inside a
    sphere when distance ÷ radius ≤ 1. With no current frame the smallest ratio wins, lower ID on an
    exact tie. With a current frame whose ratio is still ≤ 1, another system takes over only when
    its ratio ≤ (1 − 0.1) × the current one. If the current ratio exceeds 1 the rule runs as if
    there were no current frame, and `None` means the galactic frame. Until plans 06 and 11 give a
    present-day system mass, the tidal radius uses the primary initial mass.
17. **Substellar request.** The enum exists now so that plan 13 changes no signature. Until then
    `build` rejects anything but `None` with `BuildRangeQueryError::SubstellarLayersUnavailable`.
    `LayerCounts` has seven entries from the start, indexed by `Layer::value()`, and the census rule
    walks `STELLAR_LAYERS` plus whatever the substellar request adds, which is nothing in M1. Plan
    13 therefore fills two entries that were always zero and changes neither the type nor any M1
    census: a test pins that the two substellar entries are exactly zero for every M1 query.
18. **A record says where it came from.** The brainstorm's members of the galactic centre, of
    streams and of dwarf cores belong to no density component, so a mandatory `ComponentId` on the
    record would force later plans to store a false one. The record instead stores a `SystemOrigin`,
    `#[non_exhaustive]`, whose only M1 variant is `Grid(ComponentId)`; `component()` is a
    convenience that returns `Some` for that variant alone. The population is stored beside it for
    every origin. Plans 09 and 10 add their variants (listed under Provides) and move nothing: a
    grid record's bytes, draws and wire form are the same before and after. Code that reads a
    component's laws (plan 06's metallicity draw, plan 08's velocity draw) takes the component from
    `Grid` and is never called for another origin.

## Tasks

P03.T1 comes first. After it, T2–T6 are sequential; T9.a–T9.c and T12.a can run in parallel with
T2–T6; T7, T8 and T10 need T6; T9.d–T9.g need T6 and T9.a–T9.c; T11 and T12.b need T9.g.

### P03.T1 Reconcile interfaces, module skeleton and layer table

Build: read plans 01 and 02's "Provides" as built and adjust the names under [Consumes](#consumes)
if the code differs. Create the module tree with `//!` docs. Add `placement::layers` with
`LayerSpec`, `STELLAR_LAYERS`, `layer_spec`, `layer_for_initial_mass`. Cell sizes come from
`Layer::cell_size_ly()` and bands from plan 02's `MassBand`; nothing is restated as a literal. Add
the six domain tags to `rng/tags.rs` under a "Plan 03" heading, with the scopes given under
[Provides](#provides), and re-bless plan 01's `rng/tags.golden`, whose diff must show added lines
only. Define the error enums (`BuildCellKeyError`, `ResolveSystemError`, `ExceedIndexCapacityError`,
`BuildRangeQueryError`) following `rust-dev.md`.

Files: `crates/hyperion-sim/src/galaxy/{placement,query}/mod.rs`, `galaxy/placement/layers.rs`,
`galaxy/frame.rs` (empty module with docs), `galaxy/mod.rs`, `src/rng/tags.rs`,
`tests/golden/rng/tags.golden`.

Tests: the table has cells 8, 16, 32, 64, 128 ly for A–E and bands 0.08–0.5, 0.5–0.75, 0.75–2.5,
2.5–8, 8–150 M☉; `layer_for_initial_mass` at each edge (lower edge inclusive) and outside the range;
plan 01's tag-collision test still passes.

Acceptance: `just ci` green; `cargo test -p hyperion-sim layers` passes.

### P03.T2 Cell keys and geometry

Build: `CellKey` as a `Layer` plus plan 01's `GenCell` of that layer's `cell_size()`, with validated
construction (stellar layer only; `GenCell::in_root_cube`, that is −65,536 ≤ origin and origin +
size ≤ 65,536 on every axis), `containing` (`GenCell::of_ly_cell` on the position's `LyCell`, which
is floor division by the cell size), `of(SystemId)` (from `SystemIdKind::Grid`; any other kind, or a
substellar layer, is the matching `ResolveSystemError`), `layer`, `gen_cell`, `origin_ly`,
`size_ly`, `cell_box` (`CellBox::new(origin_ly, size_ly)`, which cannot fail for a generation cell,
stated in the `expect`), volume in cubic light-years, `index_capacity` = 2^`Layer::index_bits()`,
`candidate_id` (`SystemId::from_parts`).

Files: `galaxy/placement/cell.rs`.

Tests: negative coordinates floor correctly (−1 ly is in cell −1); the planes x, y, z = 0 are cell
faces in every layer; `CellKey::of(key.candidate_id(i))` round-trips; a coarse cell contains exactly
the eight cells of the next finer layer; keys outside the cube are rejected; capacity is 65,536 for
A and 2²⁸ for E.

Acceptance: `cargo test -p hyperion-sim placement::cell` passes.

### P03.T3 Candidate counts and index headroom

Build: a private `layer_bound(galaxy, key) -> f64` that calls plan 02's
`galaxy.fields().layer_bound(galaxy.shares(), band, &key.cell_box())` and re-derives nothing: the
bound and its summation order are plan 02's and part of the output. `candidate_count` =
Poisson(bound × volume) on the cell stream (Design note 1), clamped to `index_capacity` (Design note
6). `check_index_headroom`.

Files: `galaxy/placement/cell.rs`, `galaxy/placement/headroom.rs`.

Tests: over 10⁵ layer-A cells at the Sun-like point the mean candidate count matches bound × volume
within plan 01's Poisson interval; the count is identical on repeated calls; a synthetic bound above
capacity clamps (unit test on the private clamp function); `check_index_headroom` passes for
`milky_way_like()` and, as a slow test, for 200 seeds; it fails for a hand-built over-dense set of
parameters if plan 02 allows constructing one, otherwise the comparison function is unit-tested.

Acceptance: tests pass; `just test-slow` passes.

### P03.T4 Candidate evaluation

- **P03.T4.a Position draw.** `candidate_position(galaxy, key, index) -> GalacticPosition` per
  Design note 2: open the candidate's `galaxy.candidate.position` stream, take three words, and call
  `key.gen_cell().position_from_words(..)`. Tests: every position lies inside its cell for all five
  layers, including cells touching the cube's faces; integer parts are uniform (chi-square over 10⁶
  draws per layer); offsets are in [0, 1 ly). Acceptance:
  `cargo test -p hyperion-sim placement::candidate::position`.
- **P03.T4.b Acceptance and component pick.** Evaluate `Fields::densities` at
  `PointLy::from(&position)` into a stack buffer of `MAX_COMPONENTS`, weight each entry by
  `ShareMatrix::component_share(band, component)`, and apply Design note 3 with
  `Mark::pick_weighted`. The index picked is the `ComponentId`. Add
  `debug_assert!(weighted_density <= bound * (1.0 + 1e-12))` with the cell and position in the
  message: this is the brainstorm's bound-check assertion. `pick_weighted` requires Σ weights ≤
  its bound exactly and debug-asserts it, so pass it the bound padded by the same tolerance,
  `bound * (1.0 + 1e-12)`, computed once per cell. A sum within rounding of the bound then never
  trips plan 01's assertion, and acceptance changes by at most one part in 10¹², which no test can
  see. Record the padding in the doc comment. Tests: acceptance frequency in a cell
  equals mean density ÷ bound within a binomial interval; the picked components' frequencies match
  the odds at a fixed position (chi-square, using a test-only entry point that fixes the position).
  Acceptance: `cargo test -p hyperion-sim placement::candidate::accept`.

Files: `galaxy/placement/candidate.rs`.

### P03.T5 Marks, `SystemRecord` and the candidate's outcome

- **P03.T5.a Marks and the record.** `SystemRecord` (private fields, getters, `from_parts`, `Copy`,
  at most 80 bytes, asserted by a test). It stores its `SystemOrigin` (Design note 18; in M1 always
  `Grid` with the picked `ComponentId`) and, beside it, the component's `Population`, one byte, so
  that `population()` needs no `Galaxy`. The size test leaves room for the origin to grow to eight
  bytes, which is what plan 09's `FeatureMember(FeatureId)` needs. Primary initial mass from
  `MassFunction::sample_in_band(layer band, stream)` on `system.primary_mass`, under the 150 M☉
  limit. Age from the picked component's `AgeDistribution` on `system.age`, signed, never clamped at
  zero. `age_at`, `existence_at` (Design note 7). Tests: masses lie in the layer's band for 10⁵
  draws per layer; ages lie in the component's range; a record built with age −500 yr is
  `NoSystemYet` at the epoch and at +499 yr, and `Exists` at +501 yr; `age_at(±H)` differs from the
  age at the epoch by 1,000 Julian years to within the spacing of `f64` at that age. Acceptance:
  `cargo test -p hyperion-sim placement::record`.
- **P03.T5.b Outcome and carve-out hook.** `CandidateOutcome` and `evaluate_candidate`: position,
  acceptance and pick (T4), then the marks, then the private `catalogue_claims` hook (Design note
  5). Tests: a test-only build of the hook that claims every candidate with an even index shows
  `ClaimedByCatalogue` for exactly those among the otherwise accepted; `evaluate_candidate` is
  identical on repeated calls and in any order of indices. Acceptance:
  `cargo test -p hyperion-sim placement::candidate::outcome`.

Files: `galaxy/placement/record.rs`, `galaxy/placement/candidate.rs`.

### P03.T6 Whole-cell generation and the cache interface

Build: `generate_cell` (clears `out`, reserves from the candidate count, pushes accepted records in
index order), `cell_heap_bytes` (`len × size_of::<SystemRecord>()`), the `CellCache` trait, and
`NoCache`. A `//!` section states the contract the brainstorm sets for caches: they hold accepted
systems as state at the epoch, are bounded by bytes, and eviction is always safe.

Files: `galaxy/placement/generate.rs`, `galaxy/placement/cache.rs`.

Tests (integration, `tests/placement_order.rs`): order independence, that is generating cell A then
B equals B then A equals B alone, compared record for record; a toy `BTreeMap`-backed `CellCache` in
the test returns the same slices as `NoCache`; a toy cache that evicts on every call changes
nothing.

Acceptance: `cargo test -p hyperion-sim --test placement_order` passes.

### P03.T7 Resolving IDs

Build: `resolve` per "Identifiers": canonical decode is plan 01's; here, the ID must be a grid ID of
a stellar layer (`LayerNotGenerated(layer)` for the two substellar layers until plan 13, and
`KindNotGenerated` for every reserved-layer kind until plans 09 and 10, since plan 01's `Layer` has
no variant for the reserved value), the cell must be inside the cube, `index < candidate_count`, and
`evaluate_candidate` must accept. No cell is generated. Doc comment states that IDs from saves or
the protocol always go through this function.

Files: `galaxy/placement/resolve.rs`.

Tests: every record of 1,000 generated cells resolves to an identical record; an index equal to the
candidate count, and `u32::MAX` masked to the layer's width, give `NoSuchSystem`; a thinned
candidate's ID gives `NoSuchSystem`; a brown-dwarf ID gives `LayerNotGenerated` and the central
black hole's ID (`f000000700000000`) gives `KindNotGenerated`; resolving does not depend on whether
the cell was generated before. The doc example on `resolve` shows an ID from a save being resolved
and its record read.

Acceptance: `cargo test -p hyperion-sim placement::resolve` passes.

### P03.T8 Placement verification

- **P03.T8.a Density against the field, per seed** (slow). For three seeds plus `milky_way_like()`
  and for each layer, count systems in test boxes and compare with the layer density integrated over
  the box by a fine midpoint sum, using plan 01's Poisson interval. Boxes: the Sun-like point; high
  above the plane; the bulge at 1,000 ly; the nuclear disc's edge; a box straddling a young arm
  ridge just outside the bar's end, sized to cut across several 128 ly cells. For the arm box also
  compare counts in slabs either side of each cell face, to show no cell-shaped patches.
- **P03.T8.b Marks.** Kolmogorov–Smirnov of primary masses per layer against the mass function
  restricted to the band (both `Kroupa` and `Chabrier`); of ages per component against its
  distribution; chi-square of component frequencies in a box against the integrated odds.
- **P03.T8.c Bound exercise.** A debug-build test that generates every layer's cells along the young
  arm ridges from the bar's end outward for 2,000 ly, for ten seeds, so that the assertion of T4.b
  runs where violations are likeliest. It complements plan 02's hunting test, which checks the bound
  function directly.
- **P03.T8.d Golden cells.** With plan 01's harness: for two seeds, one cell per layer at the
  Sun-like point, one layer-A and one layer-E cell in the bulge, and one layer-E cell on an arm
  ridge, recording candidate count and every record in full. Ten golden IDs, including two that
  resolve to `NoSuchSystem`. This task adds the first generated stars, so it bumps
  `GENERATOR_VERSION` once.

Files: `crates/hyperion-sim/tests/{placement_stats.rs, placement_golden.rs}`,
`tests/golden/placement/*`, `tests/common/mod.rs`.

Acceptance: `just test` and `just test-slow` pass; changing draw order in T4.a by hand makes the
golden test fail (checked once, not committed).

### P03.T9 The range query

- **P03.T9.a Request and result types.** Everything under `galaxy::query` in
  [Provides](#galaxyquery) except the functions. `build` validates: radius finite and > 0 and at
  most the root cube's diagonal; centre inside the cube; |t| ≤ H (Design note 12); substellar
  request (Design note 17). Files: `galaxy/query/request.rs`, `galaxy/query/result.rs`. Tests: each
  rejection returns its variant; defaults are as documented; `complete_above` returns 0.5 M☉ for
  layer B. Acceptance: `cargo test -p hyperion-sim query::request`.
- **P03.T9.b Cell walk and sphere intersection per layer.** `QuerySphere`; `cells_in_sphere`
  iterates the cell-coordinate bounding box of the padded sphere, clipped to the root cube, and
  keeps a cell when the squared distance from the centre to the cell's box is at most the padded
  radius squared. Distances are taken from integer light-year differences (`i64`) minus the centre's
  fractional part, so nothing loses precision far from the origin. `count_cells_in_sphere` does the
  same without allocating. Files: `galaxy/query/walk.rs`. Tests: against a brute-force scan of the
  bounding box; a 50 ly sphere at a generic point meets 1,400 ± 15% layer-A cells and 300 ± 20% in
  layers B–E together (the brainstorm's figures); spheres crossing the axis planes and the cube's
  faces; a sphere smaller than a cell. Acceptance: `cargo test -p hyperion-sim query::walk`.
- **P03.T9.c Expected counts over the sphere.** `expected_counts` integrates each component's
  density over the unpadded sphere once, then forms each layer's count as Σ_c
  `component_share(band, c)` × I_c. Quadrature, fixed by Design note 11: split the z range [z₀ − R,
  z₀ + R] at z = 0 when it lies strictly inside, because every disc has a kink there; divide each
  part into n equal panels, n = ⌈width ÷ 64 ly⌉ held between 1 and 16; Gauss–Legendre in z on each
  panel; at each z node integrate over the disc of radius √(R² − (z − z₀)²) about (x₀, y₀) with
  Gauss–Legendre in radius (weight r) and equally spaced azimuths offset by half a step. Orders are
  4 × 4 × 8 (z, radius, azimuth) for R ≤ 256 ly and 8 × 8 × 16 above. Nodes and weights are
  constants with their source cited. Files: `galaxy/query/expected.rs`. Tests: a constant density
  gives 4πR³ ÷ 3 to 10⁻¹²; a pure exponential in |z| matches its closed form to 10⁻⁶; against
  `reference_sphere_integral` the layer totals agree within 2% for R of 10, 50, 500 and 5,000 ly at
  the Sun-like point, above the plane, straddling the plane and in the bulge, and within 5% at the
  nuclear disc's centre; the result is bit-identical on repeated calls. Acceptance:
  `cargo test -p hyperion-sim query::expected`, slow cases under `just test-slow`.
- **P03.T9.d Census rule.** A pure function from the grid's `LayerCounts`, the sources'
  `LayerCounts`, per-layer cell counts, the limit, the cell budget and the mass floor to a `Census`
  and the set of layers to walk, per Design notes 8–10. If even layer E does not fit, the census is
  `complete_down_to: None` and nothing is generated. Files: `galaxy/query/census.rs`. Tests: table
  tests, including B fitting alone after C fails (still excluded), the floor stopping before the
  limit, the cell budget stopping first, sources' counts tipping a layer over, nothing fitting, and
  the two substellar entries of `LayerCounts` being exactly zero in and out (Design note 17).
  Acceptance: `cargo test -p hyperion-sim query::census`.
- **P03.T9.e Time: padding, drift hook, unborn filter.** `PAD_SPEED` (1,000 km/s, doc comment
  quoting the brainstorm: above any escape speed), `pad_for` = speed ÷ c × |t| in light-years (0.33
  ly a century), `pad_speed(layer)` (Design note 13), `epoch_velocity` returning zero with a doc
  comment naming plan 08 and the reserved tag, `position_at` = epoch position + velocity × t through
  plan 01's coordinate arithmetic, and the per-system test: distance at t ≤ R and
  `existence_at(t) == Exists`. Files: `galaxy/query/motion.rs`. Tests: pad at 100 yr is 0.3336 ly to
  four figures and adds 2% to 50 ly; `position_at` with a test-only non-zero velocity moves a record
  by v × t, symmetric in the sign of t; a record born at +200 yr is absent at +100 yr and present at
  +300 yr. Acceptance: `cargo test -p hyperion-sim query::motion`.
- **P03.T9.f Merge hook.** The `SystemSource` trait with its contract in the doc comment: a source
  reports its expected members per layer by the mass band they would fall in, errs high if it must,
  returns hits already tested at t, and may suppress grid systems inside a pinned volume. Later
  plans implement it for feature members (09), the catalogue classes (09), the global list (10), and
  pinned content; the central black hole's scanning rule lives inside plan 09's source. The trait
  may still change before the first release. Files: `galaxy/query/source.rs`. Tests: a fake source
  adds its hits to the right layers and its expected count to the census; a fake suppressor removes
  the grid systems inside a ball and nothing else; with `&[]` the query equals the grid alone.
  Acceptance: `cargo test -p hyperion-sim query::source`.
- **P03.T9.g Assembly.** `range_query`: build the sphere and its pad; expected counts from the grid
  and the sources; cell counts per layer; the census; then for each admitted layer, E to A, walk the
  cells through the `CellCache`, test each record at t, apply suppression, and push hits; merge the
  sources' hits for the admitted layers; sort (Design note 14); fill `QueryStats`. Buffers are
  reserved from the expected total. Files: `galaxy/query/mod.rs`. Tests: see T10. Acceptance: the
  doc example on `range_query` (a 20 ly query at the Sun-like point with `NoCache`) passes as a
  doctest.

### P03.T10 Query verification

Build, in `tests/query.rs` and `tests/query_stats.rs`:

- Equality with `brute_force_in_sphere` for a dozen centres and radii whose census admits every
  layer, at t = 0 and t = ±H.
- **Determinism of the census regardless of cache state:** the same query against `NoCache`, a cold
  toy cache, a warm one, and one that evicts at random (fixed seed) gives equal `RangeResult`s,
  census included.
- Order independence: two overlapping queries in either order, and each alone, agree on every shared
  system.
- Census behaviour in place: at the Sun-like point a 50 ly query with the default limit is complete
  down to A with about 1,600 systems at the reference density scaled to the seed; in the bulge the
  same query stops above A and says why; at the very centre nothing fits.
- Every returned ID resolves, and every returned system satisfies the distance and existence tests.
- Statistical (slow): the realised count per layer over 200 disjoint spheres lies in the Poisson
  interval of `expected_counts`.
- Golden: two pinned queries, recording the census, the IDs in order and the distances.

Acceptance: `just test` and `just test-slow` pass.

### P03.T11 Benchmarks

Build Criterion benches, all on `milky_way_like()` with fixed seeds:

- `placement/sparse_fine_cell`: `generate_cell` for layer-A cells at the Sun-like point, and at the
  rim. Target from the brainstorm: 1–2 µs a cell.
- `placement/cell_by_layer`: one cell of each layer at the Sun-like point; a layer-A bulge cell.
- `placement/resolve`: one ID in a sparse cell and one in the fullest layer-A cell (constant time:
  the two should be within a small factor).
- `query/expected_counts`: R = 50 and 5,000 ly. Budget: under 0.3 ms at 50 ly.
- `query/range_50ly_cold`: `NoCache`, the Sun-like point, default limit. Target: under 5 ms. The
  brainstorm notes that this and the sparse-cell target stand or fall together, since the query
  touches about 1,400 fine cells.
- `query/range_50ly_warm`: a pre-filled toy cache.
- `query/range_500ly_floor_d`: a long-range query with a mass floor.

Record the measured figures and the machine in the bench file's `//!` header. A miss is a finding to
report, not a CI failure.

Files: `crates/hyperion-sim/benches/{placement.rs, range_query.rs}`, `Cargo.toml` bench entries.

Acceptance: `just bench` runs them; results recorded.

### P03.T12 Frame selection

- **P03.T12.a The rule.** `FrameCandidate`, `FRAME_HYSTERESIS`, `select_frame` per Design note 16.
  Ratios are compared with `total_cmp`, exact ties by ID. Tests: smallest ratio wins; exact tie goes
  to the lower ID; a rival at 0.95 of the current ratio does not take over and one at 0.89 does; a
  ship moving along the boundary of two equal spheres changes frame at most once; outside every
  sphere the result is `None`; a current frame that the ship has left is dropped. Files:
  `galaxy/frame.rs`. Acceptance: `cargo test -p hyperion-sim frame`.
- **P03.T12.b `frame_at`.** For each stellar layer, search radius = 1.25 × the tidal radius of the
  layer's largest mass at the ship's position (the tidal radii are about 3 ly for A and 21 ly for E
  at the Sun-like point), walk that layer with `cells_in_sphere` with no census, evaluate positions
  at t through `position_at`, take each system's tidal radius from `PotentialTables::tidal_radius`
  at its own position, merge the sources' systems, and apply `select_frame`. Tests: a ship placed
  0.1 ly from a generated system gets that system; a ship in a void gets `None`; the result is the
  same at t and with any cache. Acceptance: `cargo test -p hyperion-sim --test frame`.

## Verification

The plan is done when:

- `just ci`, `just test-slow` and `just bench` run clean.
- The brainstorm's Testing items owned here pass: **order independence** (T6, T10), **density
  against the field, per seed** (T8.a), the **bound-check debug assertion** and its exercise along
  arm ridges near the bar (T4.b, T8.c), **golden cells and IDs** (T8.d, T10), and **determinism of
  the census decision regardless of cache state** (T10).
- The benchmarks of T11 are recorded against the targets: a sparse fine cell in 1–2 µs, a 50 ly
  query at Sun-like density under 5 ms cold.
- By eye, once plans 04 and 05 land: the local chart at the Sun-like point shows no cell-shaped
  patches, the census line reads `COMPLETE ABOVE …` as the radius grows, and the count falls when
  the chart centre rises out of the plane.

## Generator version

This plan creates the first generated stars, so `GENERATOR_VERSION` is bumped in P03.T8.d, and again
by any later task here that changes a draw, the clamp or the threshold construction. The bound
(plan 02) is part of the output: a change there moves every star and regenerates these goldens.

Reserved so that later plans move no star they need not:

- **The time argument** on the query, `position_at`, `age_at` and `existence_at`, from the start.
- **Velocity draws on a stream of their own:** tag `system.velocity`, registered and unused. Plan 08
  fills `epoch_velocity`; no position at t = 0 changes.
- **Marks on their own streams:** mass and age each have a tag, so metallicity, multiplicity and the
  rest add tags and move nothing.
- **The carve-out hook** in `evaluate_candidate` and the `ClaimedByCatalogue` outcome. Plan 09's use
  of it, and the field giving up the share φ, are a version bump the brainstorm already expects.
- **The share matrix's shape** (band × population column, read per component through
  `component_share`), consumed as a matrix although M1's columns are identical, so displaced classes
  add columns.
- **The merge hook** `SystemSource`, for features, the global list, catalogue classes and pinned
  content.
- **The substellar request**, the mass floor's room for two more steps, and the two zero entries of
  `LayerCounts` (Design note 17), for plan 13.
- **The record's origin** (`SystemOrigin`, Design note 18), so that plans 09 and 10 add variants for
  members that have no component and change no grid record.
- **Per-layer padding speed**, for plan 08's unbound class.

## Risks and open points

- **The 1–2 µs cell target is tight.** A sparse cell costs one bound evaluation over a dozen or more
  components, a Poisson draw, and about one candidate with a full density evaluation. If the bench
  misses, the levers that do not change output are: evaluating components in a fixed order with
  early exit once the threshold is passed (the cumulative sum need only reach the acceptance word),
  reusing buffers, and Design note 15's pre-filter for uncached queries. Anything that changes the
  bound's value is plan 02's and bumps the version.
- **Interface names** under [Consumes](#consumes) were reconciled with plans 01 and 02 as drafted.
  The most load-bearing: plan 02 exposes densities per component and the layer bound in one call
  each; its age sampler returns signed ages; `MassFunction::sample_in_band` exists; plan 01's
  `Layer` knows its cell size and index width, and `GenCell::position_from_words` fixes the position
  draw. P03.T1 re-checks them against the code.
- **Addition: the index clamp** (Design note 6) is not in the brainstorm, which says only that the
  index absorbs the field's densities at every layer. A clamp that was ever reached would leave a
  cell silently short of stars in release builds. That is why the headroom check is a hard error at
  eight standard deviations and why the clamp carries a `debug_assert!`, which the slow-test profile
  keeps on. Plan 04 must call `check_index_headroom` when it builds a universe's `Galaxy`.
- **Ambiguity: what the census limit bounds.** The brainstorm decides from expected counts but
  speaks of "the caller's limit" as if it bounded the result. Resolved as Design note 9: expected
  counts only, no truncation. Plan 04 must allow for Poisson excess when sizing messages.
- **Ambiguity: layers after the first that does not fit.** Resolved as Design note 8.
- **Addition: the cell budget** (Design note 10) is not in the brainstorm. It is needed to keep a
  sparse long-range query from running for minutes, and it follows the same "whole layer or nothing,
  decided before generating" shape.
- **Unborn systems are too rare to find by sampling** (a few thousand in the galaxy), so the unborn
  filter is tested on constructed records and on the age sampler's plumbing, not on a wild sample.
- **Expected-count accuracy in the young disc.** Sharp arms are narrower than the quadrature's
  spacing for large radii. The young population is under 1% of every layer in M1, so the error in a
  layer's total is small, but once plan 08 makes layer E's columns differ this should be re-checked.
- **Frame rule has no consumer in M1.** It is included because the brainstorm ties it to the range
  query; its tidal radius uses the primary initial mass until later plans supply a system mass, and
  near the galactic centre plan 09's feature rule ("the smaller radius wins") must be layered on.
- **Suppression and expected counts.** A pinned volume's suppression is not subtracted from the
  grid's expected count. That errs high, which can only drop a layer early, the same direction the
  brainstorm accepts for features.
- **`displacement_to` cost.** Plan 01 routes `mul_add` through `libm::fma` for determinism, so
  `GalacticPosition::displacement_to` costs about 54 ns (three axes) against about 12 ns with
  hardware FMA. Keep it unless the range-query benchmark shows it on the profile. If it does, a
  plain multiply and add is bit-identical to the fused form whenever the two cells differ by 31 ly
  or less per axis, so a fast path on that condition is exact; add it with a test that compares both
  paths across the boundary.
