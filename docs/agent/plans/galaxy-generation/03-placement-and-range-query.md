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
`reference_sphere_integral(..)` (a fine midpoint sum, for checking `expected_counts`). The file
exists already with plan 02's helpers; each of these is added by the first task whose integration
tests use it. Unit tests cannot see them.

## Consumes

Names are those of the owning plans' "Provides", which are authoritative. P03.T1 re-checks them
against the code as built before other work starts.

From plan 01, `hyperion_sim`:

- `rng::{Seed, Stream, ObjectKey, tags}` and the single `domain_tags!` registry in `rng/tags.rs`
  (the macro itself is defined in `rng/domain_tag.rs`; an entry reads
  `NAME: Scope = "dotted.name";` under a `// Plan NN` comment). A stream is
  `Stream::open(seed, tag, key)`, where `key` is `ObjectKey::cell(word)` for a cell (`word` =
  `SystemId::cell_word()`, the ID with its index zeroed) or `ObjectKey::from(id)` for a candidate
  (`impl From<SystemId> for ObjectKey`, in `id`); plan 01 asserts, in release builds too, that the
  key's scope is the tag's. Draws: `next_u64`, `word_at(n)` for a word addressed by number,
  `uniform`, `poisson(mean) -> u64` (inversion below a mean of 10, PTRS from 10), `mark`,
  `Mark::from_word`, and `Mark::pick_weighted(weights, bound) -> Option<usize>`, the
  allocation-free form of `Thresholds`. It adds the weights from 0 in index order, stops at the
  first threshold above the mark, and debug-asserts that the whole total, even after an early pick,
  is at most `bound`.
- `units::{LightYears, Metres, Seconds, SolarMasses, Years, KilometresPerSecond}` with their `From`
  conversions, and `units::consts` (`SPEED_OF_LIGHT`, `METRES_PER_LIGHT_YEAR`).
- `time::{UniverseTime, Span, CLOCK_WINDOW_H, ClockWindow}`: `UniverseTime::{EPOCH, since_epoch}`,
  `Span::{as_julian_years_f64, as_seconds_f64}` and `ClockWindow::contains(t)`. There is no
  `as_years` on `UniverseTime`; years since the epoch are `t.since_epoch().as_julian_years_f64()`.
- From `coords`: `GalacticPosition`, `GalacticDisplacement`, `GalacticVelocity`, `LyCell`,
  `CellSize`, `GenCell` and `ROOT_HALF_WIDTH_LY`; `GenCell::of_ly_cell`, `origin`, `in_root_cube`
  and `position_from_words(&self, [u64; 3])`, which draws a position inside a generation cell from
  one word per axis; `GenCell::new(size, [i32; 3])`, which checks only that the cell's light-years
  fit an `i32` (the cube is `in_root_cube`'s);
  `GalacticPosition::{cell, displacement_to, distance_to, translated, in_root_cube}`, where
  `cell()` is the position's `LyCell` and `translated` returns `None` only for a displacement
  that is not finite or would take the cell out of `i32`, never for leaving the cube;
  `GalacticVelocity::displacement_over(Seconds)`, the only velocity × time.
- `id::{SystemId, SystemIdKind, GridId, Layer}`: `SystemId::from_parts(layer, GenCell, index)`,
  which returns `Result<SystemId, BuildSystemIdError>` and rejects a cell outside the root cube, a
  cell of another size and an index too wide for the layer; `SystemId::kind()` giving
  `SystemIdKind::Grid(GridId)` with `layer()`, `cell()` and `index()`; `SystemId::cell_word()`;
  `Layer::{cell_size, cell_size_ly, index_bits, value, ALL}`. `Layer` has seven variants, A–E and
  the two substellar layers, and no stellar-only constant. The reserved value is not a `Layer`, and
  its IDs arrive as the other six `SystemIdKind` variants (`FeatureMember`, `Centre`, `Stream`,
  `DwarfCore`, `Pinned`, `Catalogue`), decoded, not generated here; the central black hole is
  `CentreMemberId::CENTRAL_BLACK_HOLE`.
- `GENERATOR_VERSION` (a `GeneratorVersion`); from the dev-only crate `hyperion-testkit`: `golden!`
  and `GoldenWriter`, `order::assert_order_independent`, and `stats` (`chi_square_gof`,
  `ks_one_sample`, `poisson_interval`, `assert_poisson_count`, `assert_p_value`, `ALPHA`); slow
  tests marked
  `#[ignore = "slow: …"]` under `just test-slow`; Criterion under `just bench`; `just bless`.

From plan 02, `hyperion_sim::galaxy`:

- `Galaxy`: the handle, with `new(seed)`, `seed() -> Seed`, `params()`, `fields()`, `shares()`,
  `mass_function() -> &dyn MassFunction`, `potential()`. There is no `bounds()`: the bounds are
  methods on `Fields`. Pending P02.T9: `galaxy/mod.rs` has no `Galaxy` yet, only `PointLy` and
  `Population`, and the parts are built one by one (`GalaxyParams`, `MassModel`,
  `PotentialTables::in_plane`, `Fields::new(&params, &model)`, `ShareMatrix::uniform`,
  `MassFunctionKind::to_mass_function`).
- `PointLy`, with `From<&GalacticPosition>`: the argument of every density.
- `fields::{Fields, Component, ComponentId, MAX_COMPONENTS}`: a flat list of 15 to 18 density
  components in a fixed order (young thin disc, the old thin disc's five sub-discs, thick disc,
  bulge, long bar, nuclear disc, the halo's components); `MAX_COMPONENTS` is 24;
  `Fields::densities(&PointLy, &mut [f64; MAX_COMPONENTS]) -> f64` in systems per cubic light-year
  before layer shares, which returns the _unweighted_ sum and zeroes the slots past the last
  component; `Fields::{components, component(id)}`;
  `Fields::component_id(usize) -> Option<ComponentId>`, the only way to turn a picked index into a
  `ComponentId` (there is no `ComponentId::new`); `Component::{population, ages, halo_component}`;
  `ages::AgeDistribution::sample(&mut Stream)`, which takes one uniform and returns a signed age at
  the epoch in `Years`, from −H where the component still forms stars.
- `bounds::CellBox` and `Fields::layer_bound(&ShareMatrix, MassBand, &CellBox) -> f64`: a true upper
  bound on the layer's density over the cell. `CellBox::new(min, edge)` rejects an edge that is not
  a power of two, a box reaching outside the root cube and a box across an axis plane (touching one
  is fine), so a generation cell's box cannot fail. `layer_bound` folds share × bound from 0 in
  component order, the same fold as `Fields::layer_density` and as `pick_weighted`'s running total,
  and plan 02's margins (`BOUND_MARGIN`, 2⁻⁴⁰, on every envelope, and the arm factors' slacks) sit
  inside it (plan 02, R17).
- `shares::ShareMatrix`: keyed by band and population, with
  `component_share(band, &Component) -> f64` giving a component its population's share. A
  component's density already carries its share of its population, so a layer's density is Σ_c
  `component_share(band, c)` × density_c, and that sum is what the pick of Design note 3 weights by;
  `Fields::layer_density` is the same sum. Built with P02.T7, seven population columns; P02.T9 adds
  the reserved columns.
- `imf::{MassFunction, MassBand, BandShares, MASS_BAND_EDGES}`: the five band edges (0.08, 0.5,
  0.75, 2.5, 8, 150 M☉) as the single source of truth, `MassBand::{lo, hi}`,
  `MassBand::try_from(Layer)` (error `ConvertLayerError` for a substellar layer),
  `From<MassBand> for Layer`, and `MassFunction::sample_in_band(band, &mut Stream) -> f64`, one
  word, in M☉ as a bare `f64`.
- `potential::PotentialTables::tidal_radius(SolarMasses, &PointLy) -> Metres`.
- `GalaxyParams::milky_way_like()` with `Galaxy::from_params` (pending P02.T9), the fixture the
  brainstorm's Milky Way comparisons use. P02.T11 may still tune it, which re-blesses any golden
  drawn from it.
- `tables::gauss_legendre::{GL16_NODES, GL16_WEIGHTS, GL32_NODES, GL32_WEIGHTS}`: orders 16 and 32
  only. The 4- and 8-point rules of P03.T9.c are new.

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
   53-bit thresholds by plan 01's `Mark::pick_weighted(&weights, B′)`, with B′ = B × (1 + 10⁻¹²)
   (P03.T4.b). The candidate takes the first component whose threshold exceeds the acceptance mark,
   and is thinned if none does. This is the brainstorm's "one uniform draw rejects a candidate or
   picks its class" applied to the grid, in the integer-threshold convention. `pick_weighted`
   requires Σ w ≤ its bound exactly, and B meets that by itself: `layer_bound` folds share × bound
   from 0 in component order, as the weights are folded, with plan 02's margins inside it, so Σ w ≤
   B bit for bit wherever every component's bound holds (plan 02, R17). The padding to B′ sits
   outside B and is not needed for that inequality. It makes plan 01's assertion and P03.T4.b's
   bound check agree: a violation of up to 10⁻¹² beyond a bound that already carries the margins
   trips neither, and a larger one trips P03.T4.b's, which names the cell and the position, first.
   The picked index becomes a `ComponentId` through `Fields::component_id`. The pick is over
   components, not populations, because the old
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
   possible bound × volume plus eight standard deviations exceeds the capacity; and
   `candidate_count` clamps to the capacity with a `debug_assert!`. No search is needed for the
   largest bound. It is not the sum of the components' density peaks, because plan 02's bound
   multiplies each envelope by an arm factor's bound over the cell, as built (plan 02, R17). It is
   at most `layer_bound` over a root octant, `CellBox::new([0, 0, 0], 65_536)`: that box's nearest
   corner is the origin, where every envelope peaks, and its ranges of radius and phase contain
   every cell's, in any octant. The clamp is a pure function of the cell's own draw, so it
   is deterministic, independent of order and cache, and the same in `resolve` as in
   `generate_cell`. It belongs to the generator version. It cannot make a census lopsided: a clamped
   cell would be short of candidates in every query alike, and the headroom check exists so that no
   galaxy that could reach the clamp is ever played. `check_index_headroom` is called by whoever
   builds a `Galaxy` for play, which is plan 04's universe registry; nothing in this plan calls it
   on the hot path. At Milky Way values the fullest layer-A cell expects about 6,000 candidates of
   65,536 under the default mass function (7,500 under Kroupa's).
7. **Ages are signed years at the epoch.** `age_at(t)` adds the clock time. "No system yet" is
   `age_at(t) ≤ 0`. A record resolves at all times; existence is a question asked with a time.
8. **Census stops at the first layer that does not fit.** Layers are tried from E to A with a
   running expected total. The first layer whose addition would exceed the limit is left out with
   every finer layer, even if a finer one alone would fit (layer B is smaller than C under either
   mass function), because the result must be statable as "complete above m".
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
T2–T6; T7 needs T6, and T8 needs T7 (its golden IDs resolve); T9.d needs only T9.a; T9.e needs T5.a
and T9.b; T9.f needs T5.a, T9.b and T9.d; T9.g needs T6 and T9.a–T9.f; T10 needs T7 and T9.g;
T11 and T12.b need T9.g.

Every task that takes a `Galaxy` waits on P02.T9, which builds the handle: T3–T8, T9.c, T9.e–T9.g,
T10, T11 and T12.b. T1, T2, T9.a, T9.b, T9.d and T12.a need nothing from P02.T9–T11 and can start
now. Nothing here consumes P02.T10. P02.T11 may tune `milky_way_like()`, which re-blesses any golden
drawn from the fixture and moves T11's figures, so T11 is best measured after it.

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

Acceptance: `just ci` green; `cargo test -p hyperion-sim --lib galaxy::placement::layers` passes
(a bare `layers` filter matches any test name containing the word).

### P03.T2 Cell keys and geometry

Build: `CellKey` as a `Layer` plus plan 01's `GenCell` of that layer's `cell_size()`, with validated
construction (stellar layer only; `GenCell::new`'s `i32` check, then `GenCell::in_root_cube`, that
is −65,536 ≤ origin and origin + size ≤ 65,536 on every axis), `containing` (`GenCell::of_ly_cell`
on the position's `LyCell` from `GalacticPosition::cell()`, which is floor division by the cell
size), `of(SystemId)` (from `SystemIdKind::Grid`; any other kind, or a substellar layer, is the
matching `ResolveSystemError`), `layer`, `gen_cell`, `origin_ly`, `size_ly`, `cell_box`
(`CellBox::new(origin_ly, size_ly)`, which cannot fail for a generation cell: its edge is a power of
two, it lies in the cube and no plane crosses it, stated in the `expect`), volume in cubic
light-years, `index_capacity` = 2^`Layer::index_bits()`, `candidate_id` (`SystemId::from_parts`,
whose `IndexTooLarge` becomes `None`).

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
The unit tests cannot reach `tests/common`, so the Sun-like point is written out there.

Acceptance:
`cargo test -p hyperion-sim --lib -- galaxy::placement::cell galaxy::placement::headroom` passes;
`just test-slow` passes.

### P03.T4 Candidate evaluation

- **P03.T4.a Position draw.** `candidate_position(galaxy, key, index) -> GalacticPosition` per
  Design note 2: open the candidate's `galaxy.candidate.position` stream, take three words, and call
  `key.gen_cell().position_from_words(..)`. Tests: every position lies inside its cell for all five
  layers, including cells touching the cube's faces; integer parts are uniform (chi-square over 10⁶
  draws per layer); offsets are in [0, 1 ly). Acceptance:
  `cargo test -p hyperion-sim --lib galaxy::placement::candidate::tests::position` (T4.a's tests
  carry the prefix `position`: unit tests sit in `mod tests`, so `placement::candidate::position`
  selects nothing).
- **P03.T4.b Acceptance and component pick.** Evaluate `Fields::densities` at
  `PointLy::from(&position)` into a stack buffer of `MAX_COMPONENTS`, weight each entry by
  `ShareMatrix::component_share(band, component)`, and apply Design note 3 with
  `Mark::pick_weighted`. The index picked becomes the `ComponentId` through
  `Fields::component_id`. Add `debug_assert!(weighted_density <= bound * (1.0 + 1e-12))` with the
  cell and position in the message: this is the brainstorm's bound-check assertion.
  `weighted_density` is the weights folded from 0 in component order, which is `pick_weighted`'s
  running total bit for bit, not the unweighted sum `densities` returns. `pick_weighted` requires Σ
  weights ≤ its bound exactly and debug-asserts the whole total. Plan 02's bound meets that without
  help (Design note 3; plan 02, R17): wherever every component's bound holds, Σ weights ≤ `bound`
  bit for bit. Pass `pick_weighted` the bound padded outside by the assertion's tolerance,
  `bound * (1.0 + 1e-12)`, computed once per cell, so that its assertion never fires before this
  one: a violation of up to 10⁻¹² beyond a bound that already carries plan 02's margins trips
  neither, and a larger one trips this one first. Acceptance changes by at most one part in 10¹²,
  which no test can see. Record the padding and why in the doc comment. Tests: acceptance frequency
  in a cell equals mean density ÷ bound within a binomial interval; the picked components'
  frequencies match the odds at a fixed position (chi-square, using a test-only entry point that
  fixes the position). Acceptance:
  `cargo test -p hyperion-sim --lib galaxy::placement::candidate::tests::accept` (tests prefixed
  `accept`).

Files: `galaxy/placement/candidate.rs`.

### P03.T5 Marks, `SystemRecord` and the candidate's outcome

- **P03.T5.a Marks and the record.** `SystemRecord` (private fields, getters, `from_parts`, `Copy`,
  at most 80 bytes, asserted by a test). It stores its `SystemOrigin` (Design note 18; in M1 always
  `Grid` with the picked `ComponentId`) and, beside it, the component's `Population`, one byte, so
  that `population()` needs no `Galaxy`. The size test leaves room for the origin to grow to eight
  bytes, which is what plan 09's `FeatureMember(FeatureId)` needs. Primary initial mass from
  `MassFunction::sample_in_band(layer band, stream)` on `system.primary_mass`, under the 150 M☉
  limit, wrapped in `SolarMasses` (plan 02 returns a bare `f64` in M☉). Age from the picked
  component's `AgeDistribution` on `system.age`, signed, never clamped at zero. `age_at` (the age
  plus `t.since_epoch().as_julian_years_f64()`), `existence_at` (Design note 7). Tests: masses lie
  in the layer's band for 10⁵ draws per layer; ages lie in the component's range; a record built
  with age −500 yr is
  `NoSystemYet` at the epoch and at +499 yr, and `Exists` at +501 yr; `age_at(±H)` differs from the
  age at the epoch by 1,000 Julian years to within the spacing of `f64` at that age. Acceptance:
  `cargo test -p hyperion-sim placement::record`.
- **P03.T5.b Outcome and carve-out hook.** `CandidateOutcome` and `evaluate_candidate`: position,
  acceptance and pick (T4), then the marks, then the private `catalogue_claims` hook (Design note
  5). Tests: a test-only build of the hook that claims every candidate with an even index shows
  `ClaimedByCatalogue` for exactly those among the otherwise accepted; `evaluate_candidate` is
  identical on repeated calls and in any order of indices. Acceptance:
  `cargo test -p hyperion-sim --lib galaxy::placement::candidate::tests::outcome` (tests prefixed
  `outcome`).

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
black hole's ID (`CentreMemberId::CENTRAL_BLACK_HOLE`, `f000000700000000`) gives
`KindNotGenerated`; resolving does not depend on whether the cell was generated before. The doc
example on `resolve` shows an ID from a save being resolved and its record read.

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
  layer B (`MassBand::lo`). Acceptance:
  `cargo test -p hyperion-sim --lib -- galaxy::query::request galaxy::query::result`.
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
  z₀ + R] at z = 0 when it lies strictly inside, because the bulge and the bar have a kink there
  (the discs, cored in height, no longer do); divide each part into n equal panels, n = ⌈width ÷ 64
  ly⌉ held between 1 and 16; Gauss–Legendre in z on each panel; at each z node integrate over the
  disc of radius √(R² − (z − z₀)²) about (x₀, y₀) with Gauss–Legendre in radius (weight r) and
  equally spaced azimuths offset by half a step. Orders are 4 × 4 × 8 (z, radius, azimuth) for R ≤
  256 ly and 8 × 8 × 16 above. Nodes and weights are constants with their source cited; plan 02's
  `tables::gauss_legendre` has only 16 and 32 points, so the 4- and 8-point rules are added beside
  them in the same form. Files: `galaxy/query/expected.rs`, `src/tables/gauss_legendre.rs`,
  `tests/query_expected.rs`, `tests/common/mod.rs` (`sunlike_point`, `reference_sphere_integral`,
  added to plan 02's helpers there). Tests, as unit tests: a constant density gives 4πR³ ÷ 3 to
  10⁻¹²; a pure exponential in |z| matches its closed form to 10⁻⁶; the result is bit-identical on
  repeated calls. As integration tests, since `tests/common` is not visible to unit tests: against
  `reference_sphere_integral` the layer totals agree within 2% for R of 10, 50, 500 and 5,000 ly at
  the Sun-like point, above the plane, straddling the plane and in the bulge, and within 5% at the
  nuclear disc's centre. Acceptance: `cargo test -p hyperion-sim --lib galaxy::query::expected` and
  `cargo test -p hyperion-sim --test query_expected`, slow cases under `just test-slow`.
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
  plan 01's coordinate arithmetic (`GalacticVelocity::displacement_over` of the span since the
  epoch in `Seconds`, then `GalacticPosition::translated`), and the per-system test: distance at
  t ≤ R and `existence_at(t) == Exists`. Files: `galaxy/query/motion.rs`. Tests: pad at 100 yr is
  0.3336 ly to four figures and adds 2% to 50 ly; `position_at` with a test-only non-zero velocity
  moves a record
  by v × t, symmetric in the sign of t; a record born at +200 yr is absent at +100 yr and present at
  +300 yr. Acceptance: `cargo test -p hyperion-sim query::motion`.
- **P03.T9.f Merge hook.** The `SystemSource` trait with its contract in the doc comment: a source
  reports its expected members per layer by the mass band they would fall in, errs high if it must,
  returns hits already tested at t, and may suppress grid systems inside a pinned volume. Later
  plans implement it for feature members (09), the catalogue classes (09), the global list (10), and
  pinned content; the central black hole's scanning rule lives inside plan 09's source. The trait
  may still change before the first release. Files: `galaxy/query/source.rs`. Tests: a fake source's
  expected counts, taken through T9.d's census function, tip a layer over the limit and reach
  `Census::expected`. The tests that run a query move to T9.g, which is the first task with
  `range_query`. Acceptance: `cargo test -p hyperion-sim --lib galaxy::query::source`.
- **P03.T9.g Assembly.** `range_query`: build the sphere and its pad; expected counts from the grid
  and the sources; cell counts per layer; the census; then for each admitted layer, E to A, walk the
  cells through the `CellCache`, test each record at t, apply suppression, and push hits; merge the
  sources' hits for the admitted layers; sort (Design note 14); fill `QueryStats`. Buffers are
  reserved from the expected total. Files: `galaxy/query/mod.rs`, `tests/query.rs`. Tests, from
  T9.f: a fake source adds its hits to the right layers; a fake suppressor removes the grid systems
  inside a ball and nothing else; with `&[]` the query equals the grid alone. The rest are T10's.
  Acceptance: `cargo test -p hyperion-sim --test query` passes, and the doc example on
  `range_query` (a 20 ly query at the Sun-like point with `NoCache`) passes as a doctest.

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
  rim. Target from the brainstorm: 1–2 µs a cell. Plan 02 measured the two calls that dominate it
  (R16, R17, on a loaded i7-8665U): `Fields::layer_bound` 543–694 ns and `Fields::densities` about
  540–570 ns (415 in the bulge, 585 at 35,000 ly), so a sparse cell's bound and one candidate's
  densities already take 1.1–1.3 µs before the Poisson draw, the candidate's four words and the
  marks. Expect the upper half of the range, not 1 µs; the first risk below lists the levers.
- `placement/cell_by_layer`: one cell of each layer at the Sun-like point; a layer-A bulge cell.
- `placement/resolve`: one ID in a sparse cell and one in the fullest layer-A cell (constant time:
  the two should be within a small factor; each is one bound, one Poisson draw and one candidate).
- `query/expected_counts`: R = 50 and 5,000 ly. Budget: under 0.3 ms at 50 ly, which holds on plan
  02's figures: two panels of 4 × 4 × 8 nodes, 256 calls of `Fields::densities`, about 0.15 ms. At
  5,000 ly, 16 panels a part of 8 × 8 × 16 nodes (32 when the sphere crosses the plane) make
  16,000–33,000 calls, some 9–19 ms, with no budget.
- `query/range_50ly_cold`: `NoCache`, the Sun-like point, default limit. Target: under 5 ms. The
  brainstorm notes that this and the sparse-cell target stand or fall together, since the query
  touches about 1,400 fine cells. On plan 02's figures, at the reference density: about 1,700 bounds
  and 3,100 candidates' densities at roughly 0.6 µs each, some 3 ms, plus the marks, distances
  and sort.
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
  `galaxy/frame.rs`. Acceptance: `cargo test -p hyperion-sim --lib galaxy::frame` (a bare `frame`
  filter also runs plan 01's `coords::frames` tests).
- **P03.T12.b `frame_at`.** For each stellar layer, search radius = 1.25 × the tidal radius of the
  layer's largest mass at the ship's position (for `milky_way_like()` at version 9 onward the tidal
  radii are about 3.4 ly for A and 22.5 ly for E at the Sun-like point, from 4.2 ly for 1 M☉; they
  were 3.5, 23 and 4.4 before P02.T11 retuned the fixture), walk that layer
  with `cells_in_sphere` with no census, evaluate positions
  at t through `position_at`, take each system's tidal radius from `PotentialTables::tidal_radius`
  at its own position, merge the sources' systems, and apply `select_frame`. Tests: a ship placed
  0.1 ly from a generated system gets that system; a ship in a void gets `None`; the result is the
  same at t and with any cache. Files: `galaxy/frame.rs`, `tests/frame.rs`. Acceptance:
  `cargo test -p hyperion-sim --test frame`.

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

- **The 1–2 µs cell target is tight, and 1 µs is out of reach.** A sparse cell costs one bound
  evaluation over 15 to 18 components, a Poisson draw, and about one candidate with a full density
  evaluation. Plan 02 measured the bound at 543–694 ns and the densities at about 540–570 ns (R16,
  R17), 1.1–1.3 µs together before any draw. The T7 revision's tabulated vertical profiles change
  the densities' cost, so T11 re-measures both. If the bench misses 2 µs, the levers that do not
  change output are: evaluating components lazily in index order and stopping once the running sum
  passes the acceptance mark, reusing buffers, and Design note 15's pre-filter for uncached queries.
  The first lever is bit-identical, since the running sum is the same fold, but it needs a lazy
  pick from plan 01: `pick_weighted` stops early but takes every weight up front, and in debug
  builds adds the rest to check the total. It also gives up the work `Fields::densities` shares
  between components, so it pays only where the first components usually decide. Anything that
  changes the bound's value is plan 02's and bumps the version.
- **Interface names** under [Consumes](#consumes) were reconciled with plans 01 and 02 as drafted,
  then checked against the code as built (the re-validation below). The most load-bearing: plan 02
  exposes densities per component and the layer bound in one call each; its age sampler returns
  signed ages; `MassFunction::sample_in_band` exists; plan 01's `Layer` knows its cell size and
  index width, and `GenCell::position_from_words` fixes the position draw. P03.T1 re-checks what
  has landed since, the `Galaxy` handle first.
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
- **Updated for the 2026-09-21 density rulings.** The default mass function is now Chabrier's system
  function, so the fullest layer-A cell at Milky Way values drops to about 6,000 candidates (Design
  note 6), and a 50 ly query at the reference density generates about 3,100 candidates, not 2,900,
  for the same 1,600 systems. The discs are cored in height, so the z = 0 split of P03.T9.c now
  serves the bulge and the bar alone. It stays, because the quadrature belongs to the version.
- **Re-validated at 70c6052** (2026-09-21), with plan 01 and plan 02's T1–T8 built, T9–T11 not, and
  the T7 revision for the density rulings under way. Consumes now names what the code has:
  `Seconds`, `UniverseTime::{EPOCH, since_epoch}`, `Span::as_julian_years_f64`,
  `GalacticPosition::cell`, `GalacticVelocity::displacement_over`, `Fields::component_id` (there
  is no `ComponentId::new`), `MassBand::{lo, hi}`, `sample_in_band`'s bare `f64`,
  `CentreMemberId::CENTRAL_BLACK_HOLE` and Gauss–Legendre tables of 16 and 32 points only. Plan
  01's scope check on opening a stream is a release `assert!`. Design note 3 and P03.T4.b now state
  how the weights, B and the padding relate (plan 02, R17; `bounds.rs`, "A layer's bound"): Σ w ≤ B
  holds bit for bit, and the padding, kept, only orders the two assertions. Design note 6's
  largest bound is `layer_bound` over a root octant, not the sum of density peaks, since the
  as-built bound carries arm-factor bounds. Acceptance filters corrected for T1, T3, T4.a, T4.b,
  T5.b, T9.a, T9.c and T12.a: unit tests sit in `mod tests`, and the bare filters `layers` and
  `frame` matched other tests. T9.c's comparisons against `reference_sphere_integral` become
  integration tests,
  T9.f's tests that run a query move to T9.g, the first task with `range_query`, and T12.b names
  `tests/frame.rs`. Ordering: T8 needs T7, T10 needs T9.g, and the needs of T9.d–T9.f are stated.
  T11 and the first risk above use plan 02's measured costs, and T12.b's tidal radii are the
  fixture's as built. Pending re-validation: T3–T8, T9.c, T9.e–T9.g, T10, T11 and T12.b wait on
  P02.T9 (the `Galaxy` handle, used here as its Provides sketches it); the densities' cost and
  Design note 6's candidate counts wait on the T7 revision.
- **Deviations in T1, T2, T9.a, T9.b, T9.d, T12.a, as built** (built before P02.T9; nothing in them
  takes a `Galaxy`).
  - **T1.** The six tags are registered now, as the task says, though no stream opens under them
    until T3–T5 (plan 08 for `system.velocity`); `rng/tags.rs`'s module doc still says a tag is
    added by the task that first opens it. Submodules are private and re-exported, so the names are
    `galaxy::placement::LayerSpec`, `galaxy::placement::CellKey`, `galaxy::query::Census` and so on.
    `layer_spec` lends a row of a private `static` copy of `STELLAR_LAYERS`.
    `layer_for_initial_mass` takes each band as `[lo, hi)`, E as `[8, 150]` M☉ and NaN as `None`;
    plan 02's `sample_in_band` draws from the closed band, so a primary can sit on its band's upper
    edge, and a record's layer is always its ID's, never `layer_for_initial_mass` of its mass (T5.a,
    T8.b). The error variants, which the plan left open: `BuildCellKeyError` is
    `NotStellarLayer(Layer)`, `CoordinateOutOfRange(BuildGenCellError)` or
    `OutsideRootCube(GenCell)`; `ExceedIndexCapacityError` is one struct variant,
    `LayerTooDense { layer, largest_mean, capacity }`, where T3 puts the largest bound × volume in
    `largest_mean`; `BuildRangeQueryError` is `RadiusNotFinite`, `RadiusNotPositive`,
    `RadiusBeyondRootCube`, `CentreOutsideRootCube`, `TimeOutsideClockWindow(UniverseTime)` or
    `SubstellarLayersUnavailable`. The placement errors sit in `placement/mod.rs`,
    `BuildRangeQueryError` in `query/mod.rs`. `galaxy/mod.rs`'s `//!` still describes plan 02 alone
    ("No star is placed here"): the lane could only append to that shared file, so the integration
    commit rewords it.
  - **T2.** Beyond the sketch: `CellKey::volume_ly3() -> f64`, exact, for T3's bound × volume, and
    `CellKey::cell_word() -> u64`, the raw ID of the cell's candidate 0 and the one source of Design
    note 1's word. T3, T6 and T7 key the cell stream with `ObjectKey::cell(key.cell_word())`; a test
    pins it equal to `SystemId::cell_word()` of every candidate. `containing` outside the cube gives
    `OutsideRootCube(cell)`, and it accepts exactly the positions `GalacticPosition::in_root_cube`
    accepts. `of` gives `LayerNotGenerated(layer)` for a substellar grid ID and `KindNotGenerated`
    for every reserved kind, with no cube check: a grid ID's cell fields span the cube exactly.
    "Stellar" is `layer_spec(layer).is_some()`, so plan 13 widens `new`, `containing` and `of`
    through `layer_spec` alone. `candidate_id` maps `IndexTooLarge` to `None` and panics on the
    other `BuildSystemIdError` variants, which a key cannot produce.
  - **T9.a.** `SystemHit` and `RangeResult` are not built. A `SystemHit` holds a `SystemRecord`,
    which is T5.a's, so the ordering note is wrong to let T9.a run beside T2–T6 for these two.
    `SystemHit` moves to T9.f, whose `systems_in_sphere` is its first use, and `RangeResult` to
    T9.g, which returns it; both stay in `query/result.rs` as Provides sketches them. `LayerSet`,
    named only in `SystemSource`'s signature, is a `u8` set over `Layer::value()` (`EMPTY`, `with`,
    `contains`, `is_empty`, `iter` in `Layer::ALL` order, `FromIterator`). `Census::layers()` gives
    the walked set: the walk order (E to A, then the brown dwarfs and the rogue planets) down to
    `complete_down_to`. `LayerCounts` has `ZERO`, `from_array`, `to_array`, `get`, `set` and an
    elementwise `Add`; `from_array` and `set` panic on a count that is NaN, infinite or negative.
    `Census` has private fields and a `pub(super)` constructor, which debug-asserts that a census
    admitting nothing was stopped by the limit or the budget. `QueryStats` keeps `pub(super)` fields
    for T9.g to add to. `RangeQuery` is `Clone`, not `Copy`, and `build` checks the rules in the
    task's order (a test pins it), the diagonal as r² > 3 × 131,072², exact on the right.
  - **T9.b.** `QuerySphere::new(centre, radius, time, pad) -> Result<_, BuildQuerySphereError>`
    (`InvalidRadius`, `InvalidPad`), with `padded_radius = radius + pad`. The pad is the caller's,
    because `pad_for` and `pad_speed` are T9.e's: it must be at least `pad_for(t, pad_speed(layer))`
    for every layer walked with the sphere, and T9.g passes that. `cells_in_sphere` returns
    `impl Iterator<Item = CellKey> + use<>`, so it borrows nothing, and yields cells in `CellKey`
    order; both functions yield nothing for the substellar layers. A cell that only touches the
    padded sphere is kept on every side (the bounding box reaches one light-year past
    `[c − R, c + R]` both ways). The kept cells of a z column are contiguous, so
    `count_cells_in_sphere` bisects each column's ends on the same test and visits no cell. At the
    test's generic point a 50 ly sphere meets 1,436 layer-A cells and 304 in B–E (Steiner's formula
    gives 1,429 and 307), inside the task's brackets.
  - **T9.d.** The census is `decide_census(query, grid, sources, cells_in_layer) -> Census`, with
    `query: &RangeQuery` and `cells_in_layer: impl FnMut(Layer) -> u64`. The limit, the budget and
    the floor come from the query, so the two `NonZeroU32`s cannot be swapped and plan 13 finds the
    substellar request there. The cell counts are a function, called once for each layer the walk
    reaches and never for the rest, so a query stopped at D never counts layer A's cells; T9.g
    passes `count_cells_in_sphere` over the padded sphere. The walked set is `Census::layers()`, not
    a second return value. At one layer the limit is tested before the budget, and a total equal to
    the limit fits. `decide_census` is public (T9.f's test and its doc example use it;
    crate-private, it would be dead code until T9.g) and is a building block to add to Provides.
  - **T12.a.** `FrameCandidate::new(id, distance, tidal_radius)` returns a `Result` whose error,
    `BuildFrameCandidateError`, is `InvalidDistance` unless the distance is finite and ≥ 0 and
    `InvalidTidalRadius` unless the radius is finite and > 0; the candidate has getters, `ratio()`
    and `contains_ship()`; a distance of −0 is stored as +0. A current frame missing from the
    candidates is dropped like one the ship has left, and an ID listed twice counts at its smallest
    ratio; a test runs every order of five candidates. For T12.b: `PotentialTables::tidal_radius` is
    zero at the exact centre, which `FrameCandidate::new` refuses, so `frame_at` must skip that
    point or leave it to plan 09's rule.
- **T1–T7 validated adversarially (round 5, at 13fee1d, `GENERATOR_VERSION` 8).** Absolute
  figures below are the version-8 fixture's and move with plan 02's round-5 rulings; ratios, word
  counts and bit-identity do not.
  - **Design note 6's figures, version 8:** the fullest layer-A cell expects 6,911 candidates under
    the default and 8,642 under Kroupa's (the ratio, 1.2505, is the same at every seed), not about
    6,000 and 7,500. With eight standard deviations, 7,576 of 65,536; B to E sit 45 to 900 times
    under their capacity. Every layer's capacity is 128 systems per cubic light-year, so layer A
    overflows at 184 systems per cubic light-year under the default and 168 under Kroupa's, and a
    fixed 16-bit index would overflow layer E at 4.3 and 4.9: the brainstorm's 180, 170, 4 and 5.
    The octant's bound dominated all of about 5.1 million cells swept over every octant, peaking at
    0.99675 of it in the eight cells touching the origin. The builder's densest centre is refused at
    layer A with a mean of 65,822. A new test sweeps cells in every octant against the octant's
    bound: an 8 ly box at the origin in place of the octant had passed every test, goldens included.
  - **The record's growth room is smaller than P03.T5.a says.** A record is 72 bytes. An origin
    with an eight-byte-aligned payload rounds up to 16 bytes and the record to 88, and so does
    plan 01's `FeatureRef`, which is 16 bytes in memory, so plan 09's `FeatureMember(FeatureId)`
    as `FeatureId` is sketched does not fit the 80-byte cap. Only a payload of at most seven bytes
    at four-byte alignment does. The size test's `bytes + 8 − 1 ≤ 80` ignored alignment. It now
    measures a mirror of the record's fields. For the owner: pack `FeatureMember`'s payload, or
    raise the cap to 88 bytes before plan 09.
  - **`check_index_headroom` is called by plan 04's `GalaxyCache`** (`compute/galaxies.rs`), not by
    the universe registry that Design note 6 and the third risk above name.
  - **Tests that could not fail, now fixed.** Keying the cell stream by another word, swapping the
    position's axes, taking the mark from word 1, crossing the mass and age tags, or padding the
    bound by 10⁻³ each passed every T1–T7 test and failed only T8.d's golden. A test now rebuilds
    every draw from Design notes 1–4 alone, opening the streams by hand, reading the words by
    number and writing out the thresholds. `resolve_refuses_a_thinned_candidate` needed a thinning
    in one layer-E cell, which about three cells in ten lack (1.2 a cell), so it now walks cells
    until it has a dozen. `placement_order`'s order test compares records as `Debug` text, which
    tells −0 from +0.
  - **Re-derived:** words per candidate are 3 (position), 1 (mark), and 1 each for mass and age
    when accepted. 2,846 records and 601 thinnings, rebuilt independently, agreed bit for bit.
    `generate_cell`, `NoCache` (cold, and after the neighbouring cells in reverse order) and
    `resolve` on a freshly built galaxy gave the same bits for 15 cells. The centre's layer-E cell
    draws 294,948 candidates and keeps 138,146 systems, 9.5 MiB at 72 bytes each.
- **T8–T12 validated adversarially (place lane, 2026-09-23).** No generator defect, and nothing
  below moves generated output. Absolute figures are version 8's fixture; ratios, agreements and
  bit-identity are not.
  - _T8.a's recorded agreement was the wrong statistic._ "0.02–1.3% in every block, layer and seed"
    does not hold. Recounted by position (each block and a one-cell border generated) against an
    independent two-point Gauss–Legendre rule on quarter-cells, formed from `densities` ×
    `component_share`, the 100 block counts deviate by 0.008–8.4%. That spread is Poisson: the
    layer-A and B blocks hold 360–3,500 systems, |z| ≤ 2.27, and Σz² = 86.6 on 100 degrees of
    freedom. Summed per layer they agree to −0.004% (E) to +0.153% (A), within 1.4σ. The midpoint
    reference agrees with the other rule to 5 × 10⁻⁴ at the nuclear disc and 10⁻⁴ elsewhere.
  - _What stayed green under a deliberate perturbation of the code._ T8.a's blocks, fast and slow,
    missed thinning 1% short in layer A, which the candidate unit test and the query golden caught.
    The fast blocks also missed 2% too many candidates in B and thinning 1% short in E, which the
    slow blocks caught. T8.a's slabs, per galaxy, passed a 3% deficit in the middle quarter of every
    cell, which only the totals caught. Three mark errors passed T8.d and every golden: a mass drawn
    for the wrong candidate past index 127 (nothing else caught it), ages from the wrong component
    past index 127 (T8.b's KS caught it), and components 0 and 1 swapped past index 64 (T8.b's χ²
    caught it). Three query errors passed T10's integration tests: dropping the pad for motion (the
    query golden caught it by its cell count), dropping the unborn filter (`motion`'s unit test
    caught it) and expected counts 2% high (the golden and the slow checks caught it). A `frame_at`
    search margin of 0.5 instead of 1.25 passed everything. The rest was caught: 3% in C, the last
    candidate dropped, the count less one, a cell shifted by one cell or 8 ly, and every
    perturbation of the walk, the sort, the census and the frame rule.
  - _Fixed in the tests._ A new golden, `placement/digests` (version 8, 35 lines), pins every record
    of the 16 pinned cells as an FNV-1a digest per cell. T8.a pools each layer over every block and
    galaxy, which resolves about 1% in A and B and 0.2–0.3% in C–E; the 1% case still passes, at
    −0.87% against ±0.97%. It also pools the arm slabs over four galaxies, where the 3% patch now
    fails at p = 1.6 × 10⁻⁷. T10's clock-window test pins each layer's padded cell count. T12.b
    compares `frame_at` with a brute-force search at 28 ships, plus 8 more towards the centre as a
    slow test. Each of these was checked to fail under the perturbation it answers. T12.a gains a
    unit test of its degenerate inputs: a rival at exactly 0.9 of the current ratio takes over and
    one an ulp above does not, and a ship on two systems at once (both ratios 0) goes to the lower
    ID and stays.
  - _Fixed in the code: `frame_at` took any time_ and padded each layer for |t| of drift. At 10⁵
    years it took 1.4 s, growing as t³, and a time near the source horizon would all but hang. It
    now returns `Result<Option<SystemId>, FindFrameError>`, refusing
    `TimeOutsideClockWindow(t)` as `RangeQuery::build` does. This deviates from the Provides
    sketch; nothing consumes `frame_at` yet.
  - _Held as claimed._ T8.a's slab conditioning is sound: given the total, the eighths are
    multinomial, and `chi_square_gof` takes bins − 1 degrees of freedom. The total is tested by the
    block and pooled checks, and a uniform 1% bias fails those while leaving the slabs green, as it
    should. `slow-test` keeps debug assertions: a bound 3% too tight trips T8.c at a layer-E arm
    cell in 0.2 s.
  - _Held as claimed, T9 and T12._ The walk equals exact integer arithmetic over 675 spheres
    centred on cell corners, cell faces and the cube's faces. Over 2,997 fractional spheres it keeps
    every cell nearer than R − 10⁻⁹ and none beyond R + 10⁻⁹. The query equals a per-layer brute
    force, strictly ordered, in ten boundary cases (on every layer's corner with R = 0.5 to 300,
    the cube's corner and face, the centre) at t = 0 and ±H. The same cell comes out bit for bit by
    every route tried: a shuffled order, a reused buffer, `resolve` in reverse, a cache warmed by
    neighbours, and a query against the union of eight sub-queries. `expected_counts` agrees with a
    24-point spherical rule to 1.7 × 10⁻⁴ (4.7 × 10⁻³ at 5,000 ly in the bulge). `frame_at` equals
    the brute force at 118 ships at two times each, 100 of the 236 answers naming a system.
  - _T11's figures were load-inflated; the misses stand, but smaller._ These were timed in one
    process against `math::exp` at a load of 7–10 and 3.3–4.2 GHz (7.3–7.9 ns an exp), and the
    bench headers carry the table. A sparse fine cell costs **290 exp, 2.14–2.23 µs**, against 3.31
    µs recorded and a 1–2 µs target. The cold 50 ly query costs **1.02 M exp, 7.2–7.3 ms**, against
    21.8 ms recorded and a 5 ms target. The first risk's arithmetic and plan 02's R21 (119 exp,
    "inside the budget") count only the bound (88) and the densities (74). They omit the primary's
    mass draw, 85 exp under Chabrier, as dear as a density evaluation. That draw is the cheapest
    lever, but it is plan 02's and would move every mass.
- **Rulings 23 and 25: a query independent of its sources' order, and the cache's rule (srv lane,
  generator version 11).** No output moves while no source exists: with `&[]` the sum is 0 in
  every layer, as the fold gave. `golden_diff` sees nothing from it.
  - _Order-independent sums (ruling 23)._ `range_query` no longer folds the sources' expected
    counts in list order. `sum_source_counts`, crate-private in `query/mod.rs`, sorts each
    layer's per-source contributions with `f64::total_cmp` and adds them from the smallest,
    starting at 0. The census then depends on the multiset of contributions only, and
    sources need no identity key. `decide_census`'s documentation now says so. In `query/mod.rs`,
    `the_census_does_not_depend_on_the_order_of_the_sources` runs three sources of 0.1, 0.2 and
    0.3 (layer E, and ten times that in C) over 12 ly of the Sun-like point in all six orders. The
    test first asserts that a plain fold does give the census different bits in some order, so that
    it can tell the two sums apart, and then asserts that every order gives a bit-identical census
    and the same systems. With the old fold back in place it fails, on layer E's 0.697 420 117 775
    853 2 against …3.
  - _Unique IDs (ruling 23)._ `SystemSource`'s contract gains the clause that a source's IDs are
    its own, disjoint from the grid's and from every other source's, which `SystemIdKind` makes
    natural. A `debug_assert!` over the merged, sorted hits checks that no ID appears twice. The
    hits are ordered by distance first, so two copies need not be neighbours, and the check
    (`ids_are_unique`) sorts a copy of the IDs. A source that hands back the grid's nearest system
    trips it (`a_source_that_repeats_a_grid_id_trips_the_debug_check`, `#[cfg(debug_assertions)]`
    and `should_panic`). It must land before plan 09's first source, and now it has. `frame_at`
    merges source hits into `select_frame` without that check, which is left as it is:
    `select_frame` is documented to count a repeated ID's smallest ratio and not to depend on the
    candidates' order.
  - _The cache's rule (ruling 25)._ `CellCache`'s documentation now says that an implementation
    keeping cells between calls keys them by galaxy as well as by cell, as plan 04's
    `CellCacheHandle` does with its seed and generator version; the trait is unchanged. The trait's
    own example, `LastCell`, kept its cell between calls keyed by `CellKey` alone, the very pattern
    the rule forbids. It now keys by seed as well, and shows that another galaxy's cell of the
    same key is lent as its own. The test and bench caches
    (`tests/{frame,query,placement_order}.rs`, `benches/range_query.rs`) each serve one galaxy and
    are left as they are.
- **Ruling 24: `frame_at` gets a golden (srv lane, blessed at version 11).** `tests/frame.rs`'s
  `the_frames_of_the_solar_circle_ships_are_pinned` writes `tests/golden/frame/frames.golden`. It
  holds the 28 ships of the brute-force test (now built by `solar_circle_ships`, with the same
  LCG draws), each with its position's bits and the frame `frame_at` picks at the epoch and at the
  clock window's end: an ID and designation, or `none`. Of the 56 answers, 28 name a system, as the
  brute-force test counts. The answers are computed apart from the brute-force search, so that the
  golden turns red on its own. With `SEARCH_MARGIN` at 0.5 in place of 1.25 it does, at line 11
  (`ship[01]` at the epoch, `0x42002cb200000000` becomes `none`), and so does the brute-force test.
