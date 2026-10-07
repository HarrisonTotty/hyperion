# Plan 11: Multiplicity and Interacting Binaries

- **Milestone:** M4.
- **Depends on:** [06 Stars](06-stellar-stage.md),
  [09 Large features and catalogue classes](09-features-and-catalogue-classes.md),
  [15 Offline fitting](15-offline-fitting.md). It also reads what
  [08 Velocities, kicks and displaced objects](08-velocities-kicks-displaced.md) built, which plan
  09 already depends on.
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/galaxy-generation.md)): the multiplicity bullet of "Systems
  and stars"; the "Interacting binaries" row and the closing paragraphs of "Covering every class of
  star"; the Type Ia bullets of "Supernova remnants: one route, not two" as far as they concern the
  grid's binaries (the explosion mark, "redraw on the same stream"); the rows "Stellar mergers",
  "Neutron-star mergers", "X-ray binaries" and "Accreting white dwarfs" of the class table in
  "Events in time", and the binary events of its two constructions; the binarity sentences of
  "Displaced objects: kicks and runaways" (stripped share, low mode keeps companions, released
  companions, "draw their class accordingly"); the "Recycled objects" bullet of "What is inside a
  cluster today" as far as the binary stage draws conditionally on a member's class; the all-stars
  test of "Sizing the layers" and "The scaling of Chabrier's high-mass branch" under "Open
  questions"; "multiplicity by mass" and "most double neutron stars at low eccentricity" under
  "Testing"; the first sentence of "Orbits and time" (Keplerian elements on rails) for stellar
  orbits; step 9 of "Suggested order of attack".

## Goal

When this plan is done, a system is no longer one star. Every system draws its multiplicity from the
observed fractions for its primary's mass, companions with observed mass ratios, periods and
eccentricities, and a hierarchy that is stable by construction. Each star has a position in the
system frame that is a pure function of time. A binary close enough to interact is run forward once
through the formulae of Hurley, Tout and Pols (2002) to a timeline from which its state at any age
is read, and the named classes fall out of that state: blue stragglers, hot subdwarfs, cataclysmic
variables, X-ray binaries, millisecond pulsars, symbiotic stars, Type Ia progenitors. The grid's
binaries are conditional on what the catalogue owns: explosion as a Type Ia is a thinning mark at
the observed rate, and a binary that comes out exploded, or inside one of the four binary catalogue
classes, redraws on the same stream. Those four classes (stellar mergers, neutron-star mergers,
X-ray binaries, accreting white dwarfs) are registered with plan 09's catalogue, each with a
conditional sampler whose table plan 15 fits, and their events (novae, dwarf novae, X-ray transients
and bursts, Be/X outbursts, luminous red novae, kilonovae) come from plan 06's two constructions.
Binarity agrees with plan 08's class table. The protocol's system summary and the `GALAXY` display
show every star of a system.

## Scope and non-goals

In scope:

- The multiplicity model (one source of truth for every quadrature that needs it), the hierarchy
  draw, stellar orbits as Keplerian elements, star positions in the system frame at any time.
- Brown-dwarf companions, which no other plan owns: plan 13 places the free-floating ones only. They
  are drawn on streams of their own from the same mass-ratio law continued below 0.08 M☉, take their
  state from plan 06's cooling fits, and stay out of every stellar quadrature (Design note 15).
- The binary evolution engine, its timeline, state at any age, and the derived binary classes.
- The Type Ia coupling on the grid side, and the grid's conditioning on the binary catalogue
  classes, with their shares entered in the share matrix.
- The four binary catalogue classes on the catalogue side, including their members on feature-level
  lists, and the conditional draws for cluster members marked as recycled objects.
- Binary events and their light curves.
- Agreement with plan 08: stripped share, low mode, released companions, runaways.
- The all-stars mass-function test, the quadrature that plan 15 fits Chabrier's scaling against, and
  the statistical tests the brainstorm names for binaries.
- Protocol and display: stars, hierarchy and orbits in the system summary.

Non-goals:

- The Type Ia _catalogue entries_ and their shells (delay first, then the binary). Plan 09 owns
  them. This plan supplies the functions that entry needs from the binary stage (Peters's inspiral,
  post-envelope separation) and nothing else.
- Fitting any table. This plan owns the shapes of `tables::binary` and ships scratch values in them;
  plan 15 fits the contents (Design note 13).
- The luminous blue variables' catalogue class. They are single stars, and plan 09 owns the class
  (`catalogue_classes::lbv::LbvProcess`, `ClassId` 4).
- Planets in binaries (stable zones after Holman and Wiegert). Plan 14. This plan exposes the orbit
  elements it needs.
- Observation at retarded time and alerts. Plan 12. Every function here takes a time and is
  symmetric in it, which is all plan 12 needs.
- A `SYSTEM` display. Plan 14. Here the `GALAXY` display's readout lists the stars.

## Provides

All Rust paths are under `hyperion_sim`. Signatures are sketches.

### Additions to plan 01's `units` and `coords`

`units::Days` (P11.T1.a). `units::GravitationalParameter`, μ = GM in m³ s⁻² (P11.T3.a, ruling 33 of
2026-09-22), beside plan 01's `units::consts::{GM_SUN, GM_JUPITER, GM_EARTH,
GRAVITATIONAL_CONSTANT}`, which are bare `f64`s. `coords::{SystemVector, SystemVelocity}`: a
displacement and a velocity in the system frame's axes, `f64` metres and metres per second, beside
plan 01's `SystemPosition` (a position from the barycentre, `[f64; 3]` metres). Plan 01 owns
`coords` and has no equivalent (its `GalacticDisplacement` and `GalacticVelocity` are in the
galactic frame). Plan 01 split the module into files, so P11.T3.a adds the two types to
`src/coords/frames.rs`, beside `SystemPosition` and `BodyPosition`, re-exports them from
`coords/mod.rs`, and keeps that file's frame guard: no conversion to a galactic or body vector
without an explicit origin, and a `compile_fail` doctest against mixing frames. Plan 14 uses all
three.

### `orbit`

```rust
/// Keplerian elements of a relative orbit at the epoch. Angles in the system frame of plan 01.
pub struct KeplerElements { /* semi_major_axis: Metres, eccentricity: Eccentricity,
    inclination, ascending_node, argument_of_periapsis: Radians,
    mean_anomaly_at_epoch: Radians, period: Seconds */ }
pub struct Eccentricity(/* f64 in [0, 1) */);
pub fn solve_kepler(mean_anomaly: Radians, e: Eccentricity) -> Radians;   // eccentric anomaly
impl KeplerElements {
    pub fn relative_state_at(&self, t: UniverseTime) -> (SystemVector, SystemVelocity);
    pub fn periapsis(&self) -> Metres;
    pub fn apoapsis(&self) -> Metres;
}
pub fn roche_lobe_radius(q_donor_over_accretor: f64, separation: Metres) -> Metres; // Eggleton 1983
pub fn peters_merger_time(m1: SolarMasses, m2: SolarMasses, a: Metres, e: Eccentricity) -> Years;
pub fn peters_separation_for(m1: SolarMasses, m2: SolarMasses, t: Years) -> Metres; // circular
```

The sketch carries the period; μ = 4π²a³ ÷ P² follows from it, and ruling 33's `OrbitDto` puts μ on
the wire (P11.T13). `relative_state_at` reduces the mean anomaly from `UniverseTime`'s integer
seconds (`seconds()`, an `i64`, and `subsec_nanos()`) modulo the period before any conversion to
`f64`, and `solve_kepler` runs a fixed number of Newton iterations from a fixed starter and never
exits on a tolerance, so that every platform agrees (P14.T2.a's requirements, taken here from the
start). Plan 14 reuses `orbit` for planets and extends it with open orbits. Plan 09's
`galaxy::motion::KeplerOrbit` is a different thing and stays as it is: a state vector about the
central black hole in the galactic frame, propagated by universal variables. The two share `math`
and nothing else.

### `stellar::multiplicity`

```rust
pub struct MultiplicityModel { /* anchors by primary mass; version default */ }
impl MultiplicityModel {
    pub const fn default_v1() -> Self;
    pub fn multiple_fraction(&self, m1: SolarMasses) -> f64;
    pub fn companion_frequency(&self, m1: SolarMasses) -> f64;
    pub fn companion_count_pmf(&self, m1: SolarMasses) -> [f64; MAX_COMPANIONS + 1];
    pub fn period_distribution(&self, m1: SolarMasses) -> PeriodDistribution;   // log10 days
    pub fn mass_ratio_distribution(&self, m1: SolarMasses, period: Days) -> MassRatioDistribution;
    pub fn eccentricity_distribution(&self, period: Days) -> EccentricityDistribution;
}
pub const MAX_COMPANIONS: usize = 5;                 // stellar companions
pub const MIN_COMPANION_MASS: SolarMasses;           // 0.08, the stellar floor
pub const MIN_SUBSTELLAR_COMPANION_MASS: SolarMasses; // 13 Jupiter masses (Design note 15)
pub const CIRCULARISATION_PERIOD: Days;              // 11.6 d, Raghavan et al. 2010

// Quadratures, deterministic, fixed nodes, stars only (no brown dwarfs). Plan 02's
// `fates::stars_below`, plan 08's `displaced::binarity::stripped_share` and plan 15's P15.T4.b
// call these.
pub fn all_stars_fraction_below(mf: &dyn MassFunction, model: &MultiplicityModel,
    m: SolarMasses) -> f64;
pub fn mean_companion_mass_per_system(mf: &dyn MassFunction, model: &MultiplicityModel)
    -> SolarMasses;
pub fn stripped_share(model: &MultiplicityModel, m1: SolarMasses, comp: &Composition) -> f64;

pub struct SystemHierarchy { /* nodes: Vec<HierarchyNode>, stars: Vec<StarSlot> */ }
pub enum HierarchyNode { Star(StarIndex), Pair { inner: NodeIndex, outer: NodeIndex,
    orbit: KeplerElements } }
pub struct StarSlot { /* body: BodyId, initial_mass: SolarMasses, kind: Star | BrownDwarf */ }
pub enum MultiplicityContext { Free, ForcedSingle,
    ForcedMultiple { max_separation: Option<Metres> } }
pub fn draw_hierarchy(galaxy: &Galaxy, record: &SystemRecord, ctx: MultiplicityContext,
    attempt: RedrawAttempt) -> SystemHierarchy;
pub fn star_positions_at(h: &SystemHierarchy, t: UniverseTime,
    out: &mut Vec<(BodyId, SystemPosition)>);
pub const STAR_BODY_INDEX_END: u16 = 16;             // body indices 0..16: plan 14's slot 0x00
// The redraw attempt lives here, because P11.T2.a uses it before `stellar::binary` exists;
// `stellar::binary` re-exports all three.
pub struct RedrawAttempt(/* u8, 0..MAX_REDRAWS */);
pub const MAX_REDRAWS: u8 = 8;
pub const DRAWS_PER_ATTEMPT: u64 = 64;               // words; equals plan 06's `ATTEMPT_WORDS`
// R06's ask A (P11.T16; decision-r06-census-cost §7): every star's initial mass and every
// star–star pair's least periastron over the attempts `SystemStars::generate` can keep, for the
// sky census's bound star by star (rendering plan R06, R06.T8.g). Reads the draw's words only.
pub struct HierarchyBound { /* stars: initial masses (bit for bit), each with its body and
    attempt; pairs: their two stars and a periastron no larger than the drawn one */ }
pub fn hierarchy_bound(galaxy: &Galaxy, record: &SystemRecord, composition: &Composition)
    -> HierarchyBound;
```

### `stellar::binary`

```rust
pub struct BinaryInput { /* m1, m2: SolarMasses, comp: Composition, orbit: KeplerElements,
    draws: [StarDraws; 2] (plan 06, which hold each star's kick draws) */ }
pub fn can_interact(input: &BinaryInput, until_age: Years) -> bool;       // the cheap pre-test
pub fn evolve(input: &BinaryInput, until_age: Years) -> BinaryTimeline;   // "run forward once"
pub struct BinaryTimeline { /* segments: Vec<Segment>, pooled_ia: Option<PooledIaEvent> */ }
pub enum SegmentKind { Detached, StableTransfer { donor: Component }, CommonEnvelope,
    Merged, Disrupted { by: Component }, Contact }
impl BinaryTimeline {
    pub fn state_at(&self, age: Years) -> BinaryState;      // continuous within a segment
    pub fn merger_age(&self) -> Option<Years>;
    pub fn supernova_ages(&self) -> [Option<Years>; 2];
}
pub struct BinaryState { /* stars: [StarState; 2] (plan 06), orbit: Option<KeplerElements>,
    transfer_rate: Option<SolarMassesPerYear>, kind: SegmentKind */ }
pub enum BinaryClass { None, Algol, Contact, BlueStraggler, HotSubdwarf, RCoronaeBorealis,
    Symbiotic, CataclysmicVariable(CvKind), LowMassXrayBinary(XrbKind),
    HighMassXrayBinary(HmxbKind),
    MillisecondPulsar, DoubleNeutronStar, DoubleWhiteDwarf, TypeIaProgenitor }
pub fn classify(state: &BinaryState) -> BinaryClass;
pub enum CarvedClass { AccretingWdFast, AccretingWdSlow, XrayBinary, StellarMerger,
    NeutronStarMerger }
pub fn carved_class(timeline: &BinaryTimeline, age_at_epoch: Years) -> Option<CarvedClass>;
pub use crate::stellar::multiplicity::{RedrawAttempt, MAX_REDRAWS, DRAWS_PER_ATTEMPT};
pub enum MergedBinaryFate { StaysJudgedOnPairVelocity, StaysJudgedOnKick, Ejected }
pub const CLUSTER_MERGED_BINARY_FATE: MergedBinaryFate; // the default P15.T5.c reviews
```

### `stellar::system` (extends plan 06's `SystemStars`)

```rust
// Plan 06's type, now holding every star: hierarchy, one StarModel per star, a BinaryTimeline per
// interacting pair. Still state at the epoch, still cacheable. `generate` keeps its signature.
impl SystemStars {
    pub fn generate_in(galaxy: &Galaxy, record: &SystemRecord, ctx: MultiplicityContext) -> Self;
    pub fn hierarchy(&self) -> &SystemHierarchy;
    pub fn star_count(&self) -> u8;
    pub fn binary_state_at(&self, pair: NodeIndex, t: UniverseTime) -> Option<BinaryState>;
    pub fn system_mass_at(&self, t: UniverseTime) -> SolarMasses;
    pub fn recoil(&self) -> Option<SystemVelocity>;          // the pair's, after a low-mode kick
}
// `summary_at` and `brief_at` (plan 06) now cover all stars; `SystemSummary` gains
// `hierarchy: HierarchySummary`, and `StarSummary` gains `binary_class: BinaryClass`.
pub struct MultiplicityFates<F: StellarFates>(/* wraps plan 06's fates */); // plan 02's seam
```

### Catalogue classes and events

- In `galaxy::catalogue_classes::binary`: `StellarMergers`, `NeutronStarMergers`, `XrayBinaries`,
  `AccretingWdFast` and `AccretingWdSlow`, implementations of plan 09's `ClassProcess`. The first
  four take the `ClassId` values plan 09 reserved: `STELLAR_MERGER = 2`, `NEUTRON_STAR_MERGER = 3`,
  `XRAY_BINARY = 5`, `ACCRETING_WHITE_DWARF = 6` (the fast hosts). The fifth,
  `ACCRETING_WHITE_DWARF_SLOW = 8`, comes after plan 09's `TIDAL_DISRUPTION_VICTIM = 7` and is
  reserved in plan 09's registry with the other four (Design note 12); that registry, in
  `galaxy/catalogue_classes/mod.rs`, is the only place a value is allocated. Value 4 is plan 09's
  luminous blue variables.
- `stellar::binary::scan::{AwdScanMarks, XrbScanMarks, scan_marks}`: the cheap marks a galaxy-wide
  scan needs (epoch position, velocity, recurrence period, event key, and for a slow host its
  eruption times inside the source horizon), drawn without the rest of the system.
  `scan::engine_calls()` is a test-build counter of calls into `evolve`. Plan 12 consumes these.
- Event tags in plan 01's `event_tags!` registry (`id/event_tags.rs`), in the block 0x0300–0x03FF
  that plan 06 sets aside for this plan: `0x0300 NOVA`, `0x0301 DWARF_NOVA`,
  `0x0302 XRAY_TRANSIENT`, `0x0303 XRAY_BURST`, `0x0304 BEX_GIANT_OUTBURST`,
  `0x0305 LUMINOUS_RED_NOVA`, `0x0306 KILONOVA`. The last two are one-shot: bin 0, number 0, as plan
  09's `SUPERNOVA` is.
- `stellar::binary::events::{BinaryEventKind, BinaryEvent, events_in, active_at, light_curve}`, in
  the shape of plan 06's `stellar::events`, over its
  `events::{EventSeries, TimeWindow, PoissonBins, MonotonePhase, PhaseClock, LinearClock}`: every
  construction takes an `&EventSeries`, built by `EventSeries::new(seed, tag, subject)`.
  `light_curve(event, dt, band)` takes plan 07's `galaxy::gas::ccm::Band` (not re-exported from
  `gas`) and returns `Watts`; the X-ray
  luminosity of an X-ray event is a separate function, `xray_luminosity(event, dt) -> Watts`,
  because plan 07's `Band` has no X-ray value.

### Tables (`tables::binary`: shapes here, contents plan 15's)

```rust
pub enum IaPoolChannel { Merger, Accretion }                 // sub-Chandrasekhar cases included
pub struct IaYieldTable { /* eta: [[f64; 2]; 24], by delay bin of plan 15's
    `tables::type_ia_delay::DELAY_EDGES` and by IaPoolChannel; each in [0, 1] */ }
pub struct ClassSamplerTable { /* per mark of the class, conditional CDFs on 17-point grids, in
    the manner of plan 15's `PRIMARY_MASS_CDF`; the mark list is fixed per class in T8 */ }
pub struct ClassShareTable { /* per class and population: hosts per solar mass formed, and the
    share of them that each of layers A–E gives up */ }
pub const IA_YIELD: IaYieldTable;
pub const AWD_SAMPLER: ClassSamplerTable;   // likewise XRB_SAMPLER, MERGER_SAMPLER, NSM_SAMPLER
pub const CLASS_SHARES: ClassShareTable;
```

The reader types over them are `stellar::binary::ia::IaExplosionMark` and
`stellar::binary::sampler::ClassSampler`. `AWD_SAMPLER` serves both accreting white dwarf classes:
the recurrence period is one of its marks and the split is a test on it. Until P15.T9.b and
P15.T10.b land, the constants hold the scratch values of Design note 13. `tables::MANIFEST` does not
exist yet (plan 15's P15.T2 builds it and registers the tables already committed), so until then
the file says it is provisional in its header, under the README's header convention, as
`tables/mge.rs` does; once the manifest exists it lists them with `provisional: true`.

### Domain tags (never renamed)

Entries of plan 01's single `domain_tags!` registry in `rng/tags.rs`, under a "Plan 11" heading,
each added by the task that first draws on it. Scope `System`: `system.multiplicity`,
`system.hierarchy`, `system.substellar`. Scope `Body`, keyed by the `BodyId` of the star an orbit
brings in (Design note 5): `binary.orbit`, `binary.orientation`, `binary.phase`, `binary.kick`,
`binary.ia_mark`, `binary.ce`. Scope `System`, under plan 09's reserved prefix `class.`, for the
catalogue side's candidates: `class.awd`, `class.xrb`, `class.merger`, `class.nsm`. Event keys: one
`DomainTag` of scope `Event` per event tag above, as plan 01's `event_tags!` requires. Streams are
opened with `Stream::open(seed, tag, ObjectKey::from(id))`. The "Plan 11" heading is appended at the
end of the `domain_tags!` list, whatever the plan number, because the macro's order fixes
`tags::ALL`, which `tests/golden/rng/tags.golden` pins; the task that adds a tag regenerates that
golden with `domain_tags_are_pinned` (in `tests/foundation_golden.rs`).

### Protocol and client

- `hyperion-protocol`: `BinaryClassDto`, `OrbitDto`, `HierarchyDto`; plan 06's `SystemSummaryDto`
  gains `hierarchy`, its `StarSummaryDto` gains `body_index` and `binary_class`, and its
  `StellarBriefDto` gains `star_count: u8`. All are additive fields on plan 06's `system_summary`
  request kind and on plan 04's `systems_in_range` rows; this plan adds no request kind. By ruling
  33 of 2026-09-22, `OrbitDto` carries the whole element set, so that the client can place a body
  and propagate it (plan 14's D18): the period, semi-major axis, eccentricity and inclination, the
  longitude of the ascending node, the argument of periapsis and the mean anomaly at the epoch (all
  angles in radians), and the gravitational parameter μ in m³ s⁻². `HierarchyDto` carries each
  component's mass, so that the client can place the stars about their barycentres. P11.T13 designs
  the exact fields.
- `apps/hyperion`: `StarList` component; `formatOrbit` helpers.

### Test helpers

`crates/hyperion-sim/tests/common/binaries.rs`: `sample_systems(galaxy, layer, n)`,
`sample_binaries_by_mass(..)`, `brute_force_class_members(galaxy, class, n)` (prior sampling with no
carve-out: every attempt-0 binary that `carved_class` puts in the class, for checking samplers; plan
15's P15.T10.b calls it, so it is also exported from `stellar::binary::testing` behind the `testing`
feature, as plan 06 does for its samplers).

## Consumes

Names are those of the owning plans' Provides as they stand; the owning plan is authoritative, and
where a name has changed by the time this plan runs only the call sites here change. Re-validated
against the code at `9d8e775` (see Risks): where an item is built, its real path or signature is
given; where it is not, the task that builds it is named.

- **Plan 01:** `math`; `rng::{Seed, Stream, DomainTag, ObjectKey, Mark, Threshold, Thresholds}`,
  `Stream::open(seed, tag, object)` with random access by `word_at(n)` and `seek(n)`, the
  `domain_tags!` registry in `rng/tags.rs`; the samplers, as `Stream` methods (`uniform`,
  `uniform_in`, `normal(mean, sigma)`, `log_normal(mu_ln, sigma_ln)`, `log_normal_dex`,
  `power_law(&PowerLaw)`) and types (`PowerLaw::new(exponent, lo, hi)`, a density ∝ x^−exponent with
  the exponent-1 case, returning a `Result`; `PiecewiseLinear::new`, also a `Result`, drawn with
  `.sample(&mut stream)`); integer-threshold decisions (`Stream::{mark, decide, pick}`,
  `Threshold::{from_probability, from_ratio}`, `Thresholds::from_weights(weights, bound)`,
  `Mark::{is_below, pick, pick_weighted}`); `rng::EventKey` (`EventKey::derive(seed, tag, subject)`)
  and the `event_tags!` registry in `id/event_tags.rs`; `units`; `time::{UniverseTime, Span}`,
  `CLOCK_WINDOW_H`, `LIGHT_CROSSING_L`, `SourceHorizon` and `ClockWindow`, where `UniverseTime` is
  an `i64` of seconds and a `u32` of nanoseconds (`seconds()`, `subsec_nanos()`,
  `since_epoch() -> Span`); `coords::SystemPosition` (in `coords/frames.rs`);
  `id::{SystemId, BodyId, EventId, EventTag}` (`BodyId::new`, `body_index()`); `GENERATOR_VERSION`
  (a `GeneratorVersion`, 11 at `9d8e775`); from `hyperion-testkit`, the `golden!` harness with
  `GoldenWriter`, through which a new golden is written so that it can be re-blessed, `stats`
  (`chi_square_gof`, `ks_one_sample`, `assert_poisson_count`), `order::assert_order_independent`;
  slow tests marked `#[ignore = "slow: …"]`, run by `just test-slow`; `just bench`.
- **Plan 02:** `imf::{MassFunction, Kroupa, Chabrier}` (with `Chabrier::high_mass_scale`, which
  returns the scale the value was built with: `Chabrier::provisional()`, the default, uses
  `Chabrier::PROVISIONAL_HIGH_MASS_SCALE` = 0.68 until plan 15's P15.T4.b fits
  `tables::chabrier::HIGH_MASS_BRANCH_SCALE`, which does not exist yet); the seam, the trait
  `galaxy::fates::StellarFates` (`lifetime`, `remnant_mass`, `mean_companions`, `breaks`) and the
  free functions over it, `fates::mean_present_mass(f, fates, ages)`,
  `fates::stars_below(f, fates, m)` and `fates::mean_stars_per_system(f, fates)`, with
  `ProvisionalFates` (Design note 1). `Galaxy` holds no fates: `galaxy/params/derive.rs` builds a
  `ProvisionalFates` in `mean_masses` and `build`, which plan 06's P06.T30.a turns into
  `fates_for(population)`. `galaxy::shares::ShareMatrix`;
  `potential::PotentialTables::tidal_radius(&self, m: SolarMasses, p: &PointLy) -> Metres`.
- **Plan 03:** `placement::{SystemRecord, resolve}` (`resolve(galaxy, id)`; the record's
  `epoch_position()`, `primary_initial_mass()`, `age_at_epoch()`, `age_at(t)`, `existence_at(t)`),
  `query::{RangeQuery, SystemSource}`, `check_index_headroom(galaxy)`, the test helper
  `sunlike_point(&Galaxy) -> GalacticPosition` in `crates/hyperion-sim/tests/common/mod.rs`.
- **Plan 06:** built at `9d8e775`: `StarState`, `Phase`, `ObjectKind` (in `stellar::state`,
  re-exported from `stellar`), `Composition`;
  `StarDraws::{for_star, for_attempt, from_parts, median}` (`for_attempt` takes an `attempt: u32`),
  whose attempt block is 64 words (`stellar::draws::ATTEMPT_WORDS`; a normal takes two), and the
  provisional companion-stripped mark on the permanent stream `star.stripped`, which
  `StarDraws::stripped()` returns as a `Mark`; `stellar::sse::{ZCoeffs, WindRecipe}`, `sse::zams`
  and `stellar::remnant::{CompactRemnant, RemnantKind, RemnantRecipe}` (whose `CompactRemnant::new`
  is `pub(crate)`); the generic `events` module (P06.T27): `events::{EventSeries, TimeWindow}` with
  the constructions `PoissonBins` and `MonotonePhase` and their `RateModel`, `PhaseClock` and
  `LinearClock`, where `RateModel::bound` takes the bin's `TimeWindow` and returns
  `events::EventsPerSecond`, and `events::testing::assert_partition_independent`; the rule that body
  index 0 is the primary and companions are numbered from 1; the event-tag block 0x0300–0x03FF. Not
  built yet, by the task that builds it: `stellar::sse::{Track, evolve, lifetime}` with
  `Track::max_radius_until` (P06.T10.c–e) and `turn_off_mass` (P06.T10.e); the helium-star entry
  point of P06.T9, which exists only as the crate-private phase evaluator
  `sse::helium::HeliumStar::new(m)` (ages in Myr from the helium zero-age main sequence, returning a
  `PhasePoint`), inside the private module `sse::helium`; by ruling 34, `Track` gains a constructor
  from a helium-star mass when T4 first needs it, and `HeliumStar` stays crate-private; `StarModel`
  (P06.T29.a); `SystemStars` with `generate`, `summary_at`, `brief_at`, `death_time` and
  `natal_kick`, and `ClockDeath` (P06.T29.b); from `stellar::remnant`, `KickLaw`, `StandardKickLaw`,
  `KickLawParams` (with its provisional `stripped_share`), `NatalKick`, `KickMode` (P06.T19),
  `Stripping`, `CollapseChannel`, `ProgenitorAtDeath` (P06.T10.e, T18), the pulsar spin-down closed
  form (P06.T21.b); `stellar::classify` (P06.T23); `substellar::cooling` (P06.T13);
  `stellar::fates::TrackFates` (P06.T30.b); the protocol's `system_summary` kind with
  `SystemSummaryDto`, `StarSummaryDto` and `StellarBriefDto` (P06.T33).
- **Plan 07:** `galaxy::gas::ccm::Band` (not re-exported from `gas`), for light curves.
- **Plan 08** (not built at `9d8e775`; until P08.T12.c the primary's draws are attempt 0, as the
  slice records in T2.c): on `SystemRecord`, `placement_class() -> PlacementClass` (`Alive`,
  `Retained`, `Displaced { class, kind }` with `DisplacedKind::{Remnant, Runaway, Walkaway}`),
  `mark_attempt()` and `kick_constraint()`, which plan 06's primary draws already honour; the seam
  `displaced::binarity::stripped_share(m, &Composition)` and `ClassTable::stripped_share_used`;
  `displaced::runaway::RunawayModel` (the runaway and walkaway shares this plan must reproduce);
  `kick_bins::speed_bin_shares` (the low-mode share).
- **Plan 09** (not built at `9d8e775`): from `catalogue_classes`, `ClassId`, `ClassProcess`,
  `CatalogueClassSource`, `CatalogueClassCell` and `CatalogueCellKey`, with the reserved values 2,
  3, 5 and 6 and `ClassId::cell_log2_ly()`; the `111` prefix encoding through plan 01's
  `CatalogueSystemId`; the catalogue grid walk;
  `features::members::{MemberRecord, FeatureLevelList}` and the rule that feature-level lists append
  later classes after its own (its design note 16); `features::interior::{MemberClass, ClassKind}`
  with the binary classes and the recycled-object marks of P09.T9.e;
  `features::cluster::ClusterModel` (`sigma(r)`, for the hard–soft boundary);
  `features::shares::FeatureShares::field_factor`, through which layer D already gives up
  `type_ia::ancient_share`; the Type Ia entry (`type_ia::IaProgenitor`) and the requirement its
  P09.T35 records, that this plan's binaries redraw any explosion before +H;
  `catalogue_classes::testing::assert_complementary`.
- **Plan 15** (not built at `9d8e775`: `hyperion-fit` holds only plan 02's `mge` task, and neither
  `tables::type_ia_delay` nor `tables::MANIFEST` exists): `tables::type_ia_delay::DELAY_EDGES`;
  `tables::MANIFEST`; the tasks P15.T4.b, P15.T5.c, P15.T9.b and P15.T10.b, which call this plan's
  code and fill this plan's `tables::binary` (Design note 13).
- **Plans 04 and 05:** the request envelope (`RequestBody`, `ResponseBody`, `REQUEST_KINDS`);
  `SystemsInRange` rows (the wire `SystemRecord`); under `apps/hyperion/src/renderer/src/`,
  `displays/galaxy/{SystemList, SystemReadout}.tsx`, `lib/format.ts`, `components/SunGlyph.tsx`
  and `components/SolarMassUnit.tsx`.

## Design notes

1. **One multiplicity model, and the stripped mark stays the primary draw.** Three earlier stand-ins
   exist. Plan 02's `ProvisionalFates::mean_companions` (0.30, 0.60, 1.0 and 1.3 for primaries of
   0.08–0.5, 0.5–1.5, 1.5–16 and over 16 M☉, Duchêne and Kraus's bins as published; plan 02's R11)
   feeds `mean_present_mass` and `stars_below`, which average a companion over a mass ratio uniform
   on 0.1–1 inside the quadrature. Plan 06's `KickLawParams::stripped_share` is a constant 0.25.
   Plan 08's `displaced::binarity::stripped_share(m, comp)` returns that constant, and it is the one
   function both plan 08's class quadrature and plan 06's provisional mark read. `MultiplicityModel`
   becomes the single source for all of them: `MultiplicityFates` on plan 02's seam, which T1.d
   widens by one provided method for the mass ratio so that the uniform stand-in stays the default
   of `ProvisionalFates`; and this plan's `stripped_share` behind plan 08's seam. Plan 02 states the
   price: the system count N scales every density, so replacing the fates moves every star. That
   bump is taken once, in T1.d. Plan 06 reserved the stripped mark's stream (`star.stripped`) and
   meaning and asks this plan to draw binaries conditional on it. So for a primary of 8 M☉ or more
   the mark is drawn first, with probability `stripped_share(m₁, Z)`, and the innermost orbit is
   then drawn by inverse transform on the period distribution restricted to the interacting range
   when the mark is set, and to its complement when it is not. That is an exact conditional draw,
   needs no redraw, and keeps plan 08's class table, which is computed from the mark, valid.
2. **Anchors.** Multiple fraction and companion frequency are interpolated linearly in log mass
   between the anchors of Duchêne and Kraus (2013, their summary table): (0.09 M☉: 22%, 0.22),
   (0.25: 26%, 0.33), (1.0: 44%, 0.62), (2.7: 50%, 1.0), (11: 60%, 1.0), (30: 80%, 1.3), constant
   outside. The number of companions of a multiple is 1 plus a geometric variate of ratio 1 −
   fraction ÷ frequency, truncated at five companions. For Sun-like primaries that gives 56 : 31 : 9
   : 4, against Raghavan et al.'s 56 : 33 : 8 : 3.
3. **Periods, ratios, eccentricities.** Periods are log-normal in log₁₀(P ÷ day): mean 5.03 and σ
   2.28 for Sun-like primaries (Raghavan et al. 2010), mean 3.85 and σ 1.95 for M dwarfs, and for
   primaries above 2 M☉ a mixture with a close component whose weight rises to 0.7 for O stars
   (Duchêne and Kraus 2013, section 3; Sana et al. 2012). The M dwarf width is from memory and may
   be nearer 1.3 in their summary table. Mass ratios follow q^γ on [0.08 M☉ ÷ m₁, 1] with γ by
   primary mass (0.4 M dwarfs, 0.3 Sun-like, −0.5 for wide pairs of A and B stars, −0.1 for close
   pairs of O stars), plus a twin excess for periods under 100 days. Eccentricity is zero under 11.6
   days, thermal above 10³ days and flat between, capped so that periastron stays outside the
   circularisation separation. Every constant is re-checked against the two papers in P11.T1.
4. **Stable by construction.** A hierarchy is built inside out. Each outer orbit is drawn from the
   same period distribution conditioned on the Mardling and Aarseth (2001) criterion against the
   orbit inside it, by redraw on its own draw numbers, and on its apocentre lying inside half the
   system's tidal radius. The brainstorm asks only for "hierarchical"; a named criterion makes it a
   property test. The source is not in the brainstorm's list and is re-checked in P11.T2.
5. **Body indices and stream keys.** Stars take body indices 0 to 15, the primary at 0 as plan 06
   fixed, the rest depth-first through the hierarchy with a pair's inner member before its outer
   one, and a brown-dwarf companion last. Plan 14 fixes `body_index` as `slot << 8 | sub` with the
   stellar level in slot `0x00`, subs `0x00`–`0x0F`, which is exactly this range, so planets, moons
   and rings never collide with a star. A pair's streams (`binary.*`) are keyed by the `BodyId` of
   the lowest-indexed star of its outer member, "the star the orbit brings in"; with the numbering
   above no two pairs share a key. A system's ID, layer and census band stay those of its primary's
   initial mass, whatever mass transfer does later. _(Ruling 81: above the 1.5–3 M☉ blend, draws
   are keyed by draw slot and bodies numbered after sorting; later `binary.*` tags key by
   `pair_key`, still distinct per pair. See Risks, "Ruling 81 as built".)_
6. **"Run forward once" means a timeline.** `evolve` integrates a binary once, from zero age to its
   age at +H, and returns an ordered list of segments with their boundary ages and parameters. State
   at any age is a lookup plus closed forms inside the segment (single-star evolution of each
   component at its effective mass and age offset, Peters's decay, a constant mean transfer rate).
   That keeps state continuous in time, open to random access and free of replay, as "Events in
   time" demands. The timeline is derived data; the caller may cache it with the system.
7. **Only close pairs run.** `can_interact` compares periastron with the Roche-filling separation
   for each star's largest radius up to the age in question (Eggleton 1983; plan 06's radius bound).
   _(P11.T4.j, ruling p11-channels of 2026-10-06:)_ a pair can interact if it reaches that test on
   the drawn orbit, or after the decay the engine's sinks can make by then: magnetic braking at the
   tidal equilibrium spin, gravitational radiation and the spins' reservoir, bounded from above.
   Everything else is two single stars on an orbit. Wind-fed symbiotics need no orbit change, so
   they are classified from the state of a wide pair. _(P11.T4.i, ruling p11-channels of
   2026-10-06:)_ the engine starts at the first arrival on the main sequence. A star that has not
   arrived by then is its own zero-age main-sequence star to the test and to the engine, its clock
   held at τ = 0 until its own arrival, and is shown on its own pre-main-sequence track until the
   pair touches it.
8. **The grid redraws; it does not veto.** "Events in time" says the cells draw conditional on not
   being in a class, the interacting-binaries row says the formulae run forward conditional on the
   system's class, and the Type Ia text says binaries that come out exploded redraw on the same
   stream. This plan reads all three the same way and applies the redraw to all five carved cases:
   exploded as a Type Ia by +H, and membership of the four binary classes. The class is one
   deterministic test on a binary's own marks, `carved_class`, evaluated identically on both sides:
   the grid rejects an attempt for which it is `Some`, which is rejection sampling and therefore an
   exact draw of the binary's marks conditional on not being in a class; the catalogue keeps only
   entries for which it is `Some(class)` (Design note 10). Nothing is counted twice, because no grid
   system can pass the test and every catalogue entry does. Two reasons rule out the supernova
   class's route, plan 03's `ClaimedByCatalogue`. First, cost: it would put the binary engine inside
   placement and `resolve`, whose budget is a microsecond or two a cell, and existence would be
   decided after placement. Second, counts: these classes' totals are observed, not what the
   formulae yield (Design note 11), and the formulae are known to be off several times over for Type
   Ia. A veto removes the engine's yield from the grid while the catalogue adds the observed count,
   so budgets would not close. With redraw the grid loses no system to the test, and each class's
   observed share is taken out of the share matrix instead (T7), so field plus catalogue equals the
   budget exactly in expectation. The price is that the primaries given up are not tilted towards
   the masses and ages that make the class: an error of the order of the class's share, 10⁻⁴ for the
   largest of the four, and the 2–4% of layer D that the brainstorm itself assigns to redraw.
9. **Redraw mechanics.** Attempt n uses draw numbers n × 64 to n × 64 + 63 on each `binary.*` stream
   and on `system.multiplicity` and `system.hierarchy`, and `StarDraws::for_attempt(seed, body, n)`
   for every companion, which is plan 06's block of 64 words (`stellar::draws::ATTEMPT_WORDS`). This
   redraw never touches the primary's own draws: they stay as plan 08 left them, the track draws at
   `record.mark_attempt()` and the remnant, stripped-mark and kick fields at whichever later attempt
   P08.T12.c's kick loop kept on that one track, because placement, the displaced classes and the
   supernova test have already read them. An attempt is a pure function of (ID, n). After eight
   attempts the last hierarchy is kept with its innermost period moved out of the interacting range;
   at a carve probability under 5% that happens less than once in 10¹⁰ systems.
10. **The catalogue side draws the present first.** Following the Type Ia entry, a binary class
    entry draws what defines it now (for an accreting white dwarf: the two masses, the period, the
    transfer rate; for a merger: its time T), then a history consistent with it from plan 15's
    table. It does not run the engine. It builds its `BinaryTimeline` directly from those marks
    (`BinaryTimeline::from_marks`: a detached history, the segment that holds now, and for a merger
    the inspiral to T and the merged star after), and the entry is kept only if `carved_class` of
    that timeline is its own class, which is how the test is the same function on both sides. The
    marks a galaxy-wide scan needs come first, on their own draws, because ten million hosts must be
    walked in seconds.
11. **Counts are observed.** As with Type Ia, the class totals come from measurement (Pala et al.
    2020; Corral-Santana et al. 2016; Kochanek et al. 2014; about ten neutron-star merger entries),
    through plan 15's share table, not from what the formulae yield. The grid redraws every binary
    that classifies into a class, so no host is outside the catalogue.
12. **Fast and slow accreting white dwarfs** are two class values, split where the nova recurrence
    period equals L. The brainstorm's class table has one row, "All, 6–12 × 10⁶"; the split comes
    from the research notes behind it and is kept for cost. About half the hosts recur more slowly
    than L and erupt at most once or twice inside the source horizon, so a slow host carries its
    eruption times there as its first cheap mark (one call of the monotone phase, made when the
    marks are drawn) and an alert scan skips every slow host that has none, while a fast host must
    be asked for its cycles in the retarded interval. Two class values let plan 12 give each its own
    reach, census and scan, and cost one registry entry. Both are "all hosts", as the class table
    says: every accreting white dwarf is in one of the two, and the 6–12 × 10⁶ is their sum.
13. **Scratch values until the fits land.** P15.T9.b (the explosion mark) and P15.T10.b (the four
    binary samplers) are fitted against this plan's engine, so they cannot exist before T4 and T5,
    while T6–T8 need tables to read. Plan 15 resolves the circle the way it does for every table but
    one: the consumer owns the shape and ships a scratch value. So this plan defines the shapes of
    `tables::binary` and commits scratch contents under a header that says so: η = 1 ÷ 6 in every
    bin of `IA_YIELD`; `CLASS_SHARES` scaled to the brainstorm's counts at Milky Way parameters
    (T7); and hand-made CDF blocks from the brainstorm's figures and T5's prior sample. The order
    is: T4–T5; then T6–T9 on scratch values, with P15.T9.b and P15.T10.b running beside them against
    the engine and `brute_force_class_members`, neither of which reads the tables; then T15 swaps
    the fitted file in, with a bump. Nothing in this plan waits on plan 15.
14. **Defaults of the generator version** set here: common-envelope efficiency α = 1 with the
    binding parameter of Hurley, Tout and Pols; their critical mass ratios; accretion
    Eddington-limited for white dwarfs, neutron stars and black holes (ruling 132.1). `CLUSTER_MERGED_BINARY_FATE` is the fourth of the kick law's defaults that the
    brainstorm's "Open questions" lists, and P15.T5.c reviews it; its value is that a merged pair
    stays a member and is judged on the pair's velocity.
15. **Brown-dwarf companions.** The brainstorm counts brown dwarfs at "one for every four or five
    stars, companions included" and gives the free-floating ones a layer (plan 13, one for every
    five or six stars). The bound ones are the difference, a few per hundred stars, and belong here.
    After the stellar hierarchy is settled, one draw on `system.substellar` decides whether the
    system has a substellar companion, with a probability by primary mass; its mass ratio continues
    Design note 3's power law from 0.08 M☉ ÷ m₁ down to 13 Jupiter masses, its period comes from the
    same distribution thinned below 10³ days for the brown-dwarf desert (Grether and Lineweaver
    2006; re-check, not in the brainstorm's list), and it joins the hierarchy under the same
    stability test or is dropped. Its state is plan 06's `stellar::substellar::cooling`. It is on a
    stream of its own, so adding it moves no star; it never enters `can_interact`; and it is left
    out of every stellar quadrature (mean mass, all-stars fraction), as plan 02's stand-in left
    companions under 0.08 M☉ out. Brown-dwarf primaries stay single, as plan 13 says.
16. **A massive primary dies when plan 06 says.** Plan 08's placement class and plan 09's supernova
    test read the primary's single-star marks: its death time T, remnant and kick. Both were decided
    before this plan runs, so the engine may not move them. For a primary of 8 M☉ or more the engine
    takes the collapse at plan 06's death age, with plan 06's remnant and kick draws; the stripped
    mark has already told plan 06's kick law that a companion took the envelope. What the engine
    decides is the orbit, the companion and what the explosion does to them. A companion's own later
    death is evaluated freely: it is a death the catalogue does not list, found by going there, as
    the brainstorm allows for deaths beyond the clock window. See Risks.

## Tasks

T3.a lands first. T1.a–c, T2.a–b and T3.b are a chain. T4 depends on T1.a–c and T3.a only and can
run beside them. T1.d and T2.c follow T4.a, which supplies the interacting range that both need;
T2.d follows T2.c. T5 follows T4. T6, T7 and T10 follow T5 and T2 and are independent of each other.
T8 follows T5 and T7; its subtasks a to d are independent, and e precedes f. T9 follows T8. T11
needs T2, T5 and T6–T7. T12 runs last in the sim. T13 then T14 follow T11. T15 waits for plan 15 and
can land at any time after T12. Every task that changes generated output bumps `GENERATOR_VERSION`
and regenerates goldens in its own commit; those are T1.d, T2.c, T2.d, T6, T7, T8, T10 and T15.

**The vertical slice** (README, "The vertical slice to the `SYSTEM` display", ruling 33 of
2026-09-22) takes T3.a, T1.a–b (T1.c beside them), T2.a–c, T3.b and a subset of T13 ahead of the
rest, and changes the order above in one place: T2.c lands before T4.a, on two provisional seams
that T1.d and T4.a later replace with a bump (see T2.c). Each of those tasks is built as written
here; what a deferred task would supply is a plain argument, a named provisional constant or a
documented `None`, and the task says so where it applies.

All Rust files are under `crates/hyperion-sim/src/` unless a path says otherwise. Every task that
first draws on a domain tag adds its entry to `rng/tags.rs` under the "Plan 11" heading, so plan
01's collision test covers it. That heading goes at the end of the `domain_tags!` list, because the
macro's order fixes `tags::ALL`, which `tests/golden/rng/tags.golden` pins; the task regenerates
that golden with `domain_tags_are_pinned`. There is no interface-reconciliation task: the names
under Consumes were checked against the owning plans when this plan was validated, and plans this
late are re-validated against the code when their turn comes (README). The re-validation at
`9d8e775` is recorded in Risks.

### P11.T1 The multiplicity model

- **P11.T1.a Fractions and counts.** `units::Days`; `MultiplicityModel`, its anchors (Design note
  2), `multiple_fraction`, `companion_frequency`, `companion_count_pmf`. Doc comments cite Duchêne
  and Kraus (2013) and Raghavan et al. (2010), re-checked. Tests: anchors are reproduced; the PMF
  sums to 1 and its mean equals frequency ÷ fraction to 1% before truncation. Acceptance:
  `cargo test -p hyperion-sim multiplicity::model`. _As built after ruling 74 (round 8, `mult3`):_
  from 2 M☉ up the anchors are Moe and Di Stefano's (2017) Table 13 at 3.5, 7, 12 and 28 M☉,
  extended over the model's laws, and `MAX_COMPANIONS` is 3 (see Risks).
- **P11.T1.b Distributions.** `PeriodDistribution`, `MassRatioDistribution`,
  `EccentricityDistribution` with densities, CDFs and samplers on a supplied stream (Design note 3).
  Tests: Kolmogorov–Smirnov of 10⁵ samples against each CDF at five primary masses; the Sun-like
  period mode lies within 0.1 dex of 10⁵ days; no companion under 0.08 M☉. Acceptance:
  `cargo test -p hyperion-sim multiplicity::dist`. _As built after ruling 74:_ eccentricities are
  Moe and Di Stefano's `e^η` (eqs. 17–18), so `eccentricity_distribution` takes the primary's mass
  as well as the period (see Risks).
- **P11.T1.c Quadratures.** `all_stars_fraction_below`, `mean_companion_mass_per_system`,
  `stripped_share` (the share of primaries whose periastron passes the `can_interact` threshold
  before core collapse; it takes the threshold as a function so that T4.a can supply the real one,
  and until then a test-only closure of periastron under 10 au stands in). Fixed Gauss–Legendre
  nodes, no randomness, stars only. Nothing calls them yet, so no output changes. Tests: under
  Kroupa the fraction below 0.5 M☉ is 0.764 ± 0.005 and under unscaled Chabrier 0.67 ± 0.01, against
  the 20 pc census's 0.69 (Kirkpatrick et al. 2024; the 0.759 once quoted as observed is Kroupa's
  function itself); the gap between `stripped_share` and the provisional 0.25 that plan 06's
  `KickLawParams::stripped_share` and plan 08's `ClassTable::stripped_share_used` will read (neither
  is built at `9d8e775`) is printed per mass for T1.d. Acceptance:
  `cargo test -p hyperion-sim multiplicity::quadrature`.
- **P11.T1.d Replace the stand-ins (version bump; every star moves).** Four edits in one commit. (1)
  Plan 02's `StellarFates` gains a provided method
  `companion_mass_ratio_cdf(&self, m1: f64, q: f64) -> f64`, defaulting to the uniform 0.1–1 that
  `mean_present_mass` and `stars_below` hard-code today (the companion range
  `[max(0.1 m, 0.08 M☉), m]` of `galaxy/fates.rs`, with `MIN_MASS_RATIO` = 0.1), and both read it.
  (2) `MultiplicityFates` wraps plan 06's `TrackFates`, overrides `mean_companions` and that method
  from the model (the mass ratio marginalised over period), and is what plan 06's
  `fates_for(population)` (P06.T30.a, in `galaxy/params/derive.rs`) returns for every population:
  `Galaxy` holds no fates of its own. (3) `stars_below` and
  `all_stars_fraction_below` now agree, which a test pins. (4) Plan 08's seam
  `displaced::binarity::stripped_share` returns this plan's `stripped_share` with T4.a's threshold,
  so plan 06's provisional mark and plan 08's class table both follow; plan 06's constant
  `KickLawParams::stripped_share` stays only as the quadratures' fallback for tests. Bump the
  version and regenerate every golden: this is the one task of the plan that moves primaries. Tests:
  mean present mass per system is 0.55–0.59 M☉ under the default, Chabrier's with plan 15's scale,
  and 0.48 ± 0.03 M☉ under Kroupa (plan 02's bracket; plan 02 measures 0.498–0.503 with its stand-in
  fates, R11), within 3% across the old populations; stars per system 1.33–1.45; `stripped_share`
  averaged over layer E lies in 0.20–0.33, the two mixes of the brainstorm's scratch Monte Carlo,
  and a value outside is a finding against the period distribution, not a reason to move the window;
  plan 06's kick-law tests (P06.T19.d) and plan 08's class-table tests still pass. Acceptance:
  `just ci` and `just test-slow`. T1.d lands after T4.a, which supplies the real threshold.

Files: `units.rs`, `stellar/multiplicity/{mod,model,dist,quadrature,fates}.rs`, a `mod` line in
`stellar/mod.rs`; edits in `galaxy/fates.rs`, `galaxy/params/derive.rs`,
`galaxy/displaced/binarity.rs`, every golden.

### P11.T2 Hierarchies

- **P11.T2.a Types and the draw.** `SystemHierarchy`, `HierarchyNode`, `StarSlot`,
  `MultiplicityContext`, `RedrawAttempt` with `MAX_REDRAWS` and `DRAWS_PER_ATTEMPT` (here in
  `stellar::multiplicity`, because `stellar::binary` does not exist yet; T4.a re-exports them),
  `draw_hierarchy`: multiplicity decision by integer threshold on `system.multiplicity`; companion
  count; for each level, period, mass ratio against the mass of the node inside, eccentricity,
  orientation (isotropic) and mean anomaly at the epoch on `binary.orbit`, `binary.orientation`,
  `binary.phase`; which node a further companion joins on `system.hierarchy`. Register those five
  tags. Draw numbers follow Design note 9. Body indices and stream keys follow Design note 5.
  Orbits are T3.a's `KeplerElements`. Nothing calls it yet. Tests: the numbering rule gives every
  pair of 10⁴ hierarchies a distinct key; attempt n drawn alone equals attempt n drawn after
  attempts 0 to n − 1. _As built after ruling 74:_ a node inside a secondary component is weighted
  by Tokovinin's (2014) correlation of subsystems, 0.275 or 20 (see Risks). _As built after ruling
  81:_ from 3 M☉ up (blended across 1.5–3 M☉) the direct companions are drawn from Moe and Di
  Stefano's Table 13 laws, each newest companion redrawn until the whole test passes, and bodies
  are numbered after sorting; Design note 5's "companion k is body k" holds only below the blend
  (see Risks).
- **P11.T2.b Stability and the tidal cut.** The Mardling–Aarseth condition and the half-tidal-radius
  cut as redraws of the outer orbit only, at most 16 (each on the next draw numbers of the same
  attempt block, which 64 leaves room for), then the companion is dropped (counted by a test, under
  1%). The tidal radius is plan 02's `PotentialTables::tidal_radius`, in metres, at the record's
  `epoch_position()` converted with `PointLy::from`. `ForcedMultiple { max_separation }` truncates
  at a cluster's hard–soft boundary, the separation at which a pair's orbital speed equals plan 09's
  `ClusterModel::sigma(r)` at the member's radius; T8.f supplies it. _Slice:_ plan 09 is not built,
  so `ForcedMultiple` is defined and tested with an explicit `max_separation`, and no caller passes
  it until T8.f; grid systems use `Free`. _As built after ruling 81:_ direct companions from 1.5 M☉
  up (blended) get 42 tries rather than 17, 21 on their draw slot's key and 21 on slot + 8, three
  words each inside the same attempt block (Design note 9). A subsystem gets one try on slot 3 + k,
  and one that fails is truncated (Tokovinin 2014, §4.3), not counted as dropped (see Risks).
- **P11.T2.c Wire into the system stage (version bump).** `SystemStars::generate` calls
  `draw_hierarchy` with `Free` for grid systems (`generate_in` takes the context for everything
  else); each companion gets a `StarModel` from `StarDraws::for_attempt` on its own body index, with
  the system's composition and age. The primary's model is plan 06's, untouched, at plan 08's
  `record.mark_attempt()`. For primaries of 8 M☉ and up the innermost period is drawn conditional on
  plan 06's stripped mark (Design note 1). `summary_at` and `brief_at` cover all stars. Bump the
  version; regenerate goldens: no primary moves, and a golden test pins that the primaries of plan
  06's pinned IDs are bit-identical. _Slice:_ this lands after P06.T29.b and before T1.d and T4.a,
  so three things it reads do not exist yet, and each is a named seam in `stellar/system.rs`:
  - `record.mark_attempt()` is plan 08's; until P08.T12.c the primary's draws are attempt 0
    (`StarDraws::for_star`).
  - The stripped share is `PROVISIONAL_STRIPPED_SHARE` = 0.25 (plan 06, design note 11), and the
    mark it is read against is `StarDraws::stripped()`, a `Mark`, compared with
    `Threshold::from_probability`.
  - The interacting range is a provisional function, T1.c's test stand-in promoted and named: a
    periastron under 10 au.

  T1.d replaces the share with the model's `stripped_share` and T4.a the range with
  `can_interact`'s threshold, each with a bump. Until T4 the pair is two single stars on an orbit.

- **P11.T2.d Brown-dwarf companions (version bump).** Design note 15: the decision and marks on
  `system.substellar`, `MIN_SUBSTELLAR_COMPANION_MASS`, the desert factor, the stability test of
  T2.b, `StarSlot::kind`, state from `stellar::substellar::cooling`, `ObjectKind::Substellar` in
  summaries. The probability by primary mass is a short table whose source the task re-checks and
  cites (candidates: Metchev and Hillenbrand 2009 for Sun-like primaries; Grether and Lineweaver
  2006 for the desert). Tests: no star of any pinned system changes; bound brown dwarfs number
  0.02–0.06 per star over a weighted sample of all layers, which with plan 13's free-floating one
  per five or six gives the brainstorm's one per four or five; fewer than 1% of Sun-like primaries
  have one inside 10³ days; `all_stars_fraction_below` is unchanged to the last bit.
  _As built (round 9c, `bin5b`; output moves, its bump batched into version 16): see Risks,
  "Deviations in P11.T2.d, as built"._

Files: `stellar/multiplicity/{hierarchy,stability,substellar}.rs`, `stellar/system.rs`.

Tests: property tests over 10⁴ IDs: every pair satisfies the criterion, every apocentre is inside
the cut, masses never exceed the primary's, the primary's record is untouched; order independence
through `hyperion_testkit::order::assert_order_independent`; `ForcedSingle` yields one star; golden
hierarchies for six pinned IDs. Acceptance: `cargo test -p hyperion-sim multiplicity::hierarchy` and
the golden suite pass.

### P11.T3 Orbits on rails

- **P11.T3.a Elements, solver, Roche and Peters.** Needs only plan 01, and is the first task of the
  plan to land, because T2.a and T4.a use its types. It takes plan 14's P14.T2.a requirements from
  the start (ruling 33 of 2026-09-22), so that plan 14 inherits a solver at planetary precision
  rather than fixing one:
  - `units::GravitationalParameter` (m³ s⁻²), added here beside plan 01's `units::consts::GM_*`,
    which stay bare `f64`s;
  - `coords::{SystemVector, SystemVelocity}` in `coords/frames.rs`, re-exported from `coords`;
  - `orbit::{KeplerElements, Eccentricity, solve_kepler}`: Newton iteration from a fixed starter, a
    fixed iteration count and never an exit on a tolerance, so that results are bit-reproducible on
    every platform, through `math`; the starter and the count are chosen to meet the residual below
    and documented;
  - `relative_state_at`, which reduces the mean anomaly from `UniverseTime`'s integer seconds and
    nanoseconds modulo the period, in integer or exactly representable arithmetic, before any
    conversion to `f64`, so that a one-day orbit keeps its phase a thousand years out;
  - `periapsis`, `apoapsis`, `roche_lobe_radius` (Eggleton 1983), `peters_merger_time` (with
    Peters's eccentricity integral as a fixed quadrature) and `peters_separation_for`.

  Tests: `solve_kepler` residual |E − e sin E − M| under 10⁻¹³ for e up to 0.999 (P14.T2.a's bound,
  which supersedes 10⁻¹² to 0.99); period closure (state at t and t + P agree to 10⁻⁹ relative);
  symmetric in time; the brainstorm's figure is reproduced: two white dwarfs a thousand years
  before merging have a period of 80–100 s. With it, in the same round, P14.T2.a's constructors
  (`from_semi_major_axis`, `scaled`) and their tests, and the cross-language fixture that P14.T39's
  `orbit.ts` tests read: a golden of 32 orbits (elements, μ, a time, and the position and velocity
  then, at round-trip precision, e from 0 to 0.999, inclinations including 0 and near 180°, times
  on both sides of the epoch), written through `GoldenWriter` under
  `crates/hyperion-sim/tests/golden/orbit/`, its line format documented in its header.

- **P11.T3.b Star positions.** After T2.a. `star_positions_at` walks the hierarchy, places each pair
  about its barycentre and returns plan 01's `SystemPosition`s. Tests: the barycentre of every one
  of 10⁴ hierarchies stays at the origin at ±H to 1 m; a position is the same whatever was asked
  before.

Files: `coords/frames.rs` and `coords/mod.rs`, `units.rs`, `orbit/{mod,kepler,peters,roche}.rs`, a
`pub mod orbit` line in `lib.rs`, `stellar/multiplicity/positions.rs`. Acceptance:
`cargo test -p hyperion-sim orbit` and `multiplicity::positions`.

### P11.T4 The binary evolution engine

Source throughout: Hurley, Tout and Pols (2002), section and equation numbers in doc comments.

A star stripped to its helium core (T4.c, T4.d, T4.e) is a plan 06 `Track` like any other (ruling
34 of 2026-09-22): plan 06's `Track` gains a constructor from a helium-star mass, over P06.T9's
entry point, in the first subtask here that needs it, so that its winds, remnant and death stay in
plan 06's one place. `sse::helium::HeliumStar` stays crate-private, and this plan never wraps it.

- **P11.T4.a Types and the pre-test.** `BinaryInput`, `BinaryTimeline`, `Segment`, `SegmentKind`,
  `BinaryState`, `can_interact` (Design note 7, over plan 06's `Track::max_radius_until`), and
  `state_at` for a timeline of one `Detached` segment. Tests: a wide pair never interacts;
  `state_at` equals two calls of plan 06's single-star function.
- **P11.T4.b Detached evolution.** Orbital change from wind mass loss and wind accretion
  (Bondi–Hoyle, their section 2.1), circularisation and synchronisation reduced to closed forms on
  the segment (circular once the tidal timescale falls under the segment's length), magnetic braking
  (2.4), gravitational radiation by Peters's closed form. Each is a secular rate that `state_at`
  integrates in closed form or on fixed nodes. Tests: angular momentum is conserved without sinks to
  10⁻⁹; a 0.1-day double white dwarf merges at `peters_merger_time`.
- **P11.T4.c Roche-lobe overflow.** Onset search by bisection on the radius bound; dynamical
  stability from the critical mass ratios by donor type (2.6.1); stable transfer at the nuclear or
  thermal rate with their accretion limits, as one segment with a mean rate; rejuvenation of an
  accreting main-sequence star through an effective age; contact.
- **P11.T4.d Common envelope and mergers.** The α–λ energy balance (2.7.1), outcome a tight pair or
  a merger; the merger product by their collision matrix (their table 2), with the product's
  effective age. Draws, where a branch is probabilistic, on `binary.ce`.
- **P11.T4.e Supernovae in a binary.** For a primary of 8 M☉ or more, the collapse falls at plan
  06's death age with plan 06's remnant and kick (Design note 16): `BinaryInput` carries that age
  and the engine treats it as a fixed boundary. For a companion, remnant and kick from plan 06's law
  with `Stripping` and `CollapseChannel` set from the timeline so far. Either way the low mode
  applies as the brainstorm says (always for electron capture and accretion-induced collapse; with
  probability 1 under a 2 M☉ core falling to 0 at 3 for a stripped star). Kick direction on
  `binary.kick`. Post-explosion orbit from their appendix A1; `Disrupted` when unbound; system
  recoil velocity recorded. Accretion-induced collapse of an oxygen–neon white dwarf.
- **P11.T4.f The driver.** `evolve`: event-to-event stepping over the segments above until
  `until_age`, a hard cap of 64 segments (a broken invariant if hit, counted in tests), and
  `pooled_ia` filled when two white dwarfs merge or an accreting white dwarf reaches ignition,
  sub-Chandrasekhar cases included. `merger_age`, `supernova_ages`.

Files: `stellar/binary/{mod,timeline,detached,rlof,common_envelope,supernova,evolve}.rs`.

Tests: six reference binaries from the paper's worked examples reproduce their sequence of phases
(an Algol, a cataclysmic variable, a double neutron star, a common-envelope merger, a blue
straggler, a Type Ia candidate); `state_at` is continuous inside every segment for 10³ random
binaries (no jump above 1% between samples 10⁻⁶ of the segment apart); no main-sequence star is
older than its effective lifetime; total mass never rises; results are identical for any order of
calls. Bench `binary_evolve` under `just bench`: record the median and 99th percentile per
interacting binary; this plan's target is a median under 200 µs, so that a system stays inside the
brainstorm's millisecond. Acceptance: `cargo test -p hyperion-sim binary::` passes after each
subtask, and `just bench` reports the figure after T4.f.

_As built (round 9, `bin11`): T4.a–f, unwired; nothing generated moves. See Risks, "Deviations in
P11.T4, as built"._

- **P11.T4.g The stripped hold and the stop order** (ruling p11-stripped-core, 2026-10-03, its
  owner leans accepted: 129.4a amended, the detached stop order included, batched into the
  version-20 bump with P14 Phase J, `swell`'s core left open; output moves). Hurley, Pols and
  Tout (2000) §6 and Hurley, Tout and Pols (2002) §2.8: a star the binary carries whose mass is at
  or below its core mass is a naked helium star or a white dwarf from that step.
  - `supernova.rs` (`Engine::die`, the pinned hold): the held state is
    `members[0].evaluate(ctx, 0, last_living(age − offset) + offset, mass, τ)`, the binary's
    state and not the track's single-star state. If its envelope is ≤ 0, the member is first
    replaced through `stripped_member` at that age (a helium star lives on with the pin still
    waiting; a collapse is held at the core's state, as in 129.4c). `self.stripped[0]` is set.
    Ruling 129.4a's text gains: "the last living state is the member's, at the binary's mass".
  - `detached.rs` (`integrate`): the envelope ≤ 0 check on a `Shaped` member runs before any
    `stop` is returned, and returns `Stop::Stripped(i)` in its place.
  - `rlof.rs` (`transfer_phase`): after `accretor_events` and `donor_events`, a `Shaped` member
    (accretor or donor) with envelope ≤ 0 is stripped (`strip`), and the pair goes on as
    `quiet_kind` decides.
  - `star.rs`: `Member::Frozen { state, core_radius }`. `frozen_structure` uses the held
    structure's core radius (sse `core_radius`: R_ZHe(Mc), HPT eq. 78; the HeMS radius at τ in
    CHeB; 5 R_WD(Mc) for degenerate cores; BSE §2.7.1), and 0.1 R is removed. A frozen state
    built from `Remains::Collapse` takes the core track's structure.
  - Tests (`stellar/binary/tests.rs`):
    - **The example.** The record 0x81fd865fd000000f pair rebuilt as a `BinaryInput`. No segment
      has a living star with M ≤ Mc in a hydrogen giant phase (HG to TPAGB). The primary is a
      helium star (HeMS/HeHG/HeGB) from 15.457 Myr to its pinned collapse at 15.896 Myr, with
      R < 3 R☉ throughout. No collision or common envelope after 15.69 Myr.
    - **An invariant added to the 10³-pair suite.** At every segment's knots, no living member
      whose phase is HG, FGB, CHeB, EAGB or TPAGB has M ≤ Mc + 10⁻⁹. No `Frozen` member has
      R > R(track at its own mass) or Rc > R. Every `Frozen` member's Rc equals sse
      `core_radius` for its phase and Mc.
    - **Stop order.** A constructed pair where wind removes a `Shaped` star's envelope on the
      step that lands on its phase boundary, and one on its pin: the strip comes first (one
      segment fewer; the collapse's phase just before is HeMS/HeHG/HeGB).
    - Keep 129.4c's and b64352e's tests (`a_held_bare_core_beside_a_main_sequence_star_merges_without_recursing`
      becomes `…_stays_a_helium_star`, golden re-blessed).
  - **Goldens to re-bless** (at version 19, "for the version 20 batch"):
    `stellar/binary_timelines` (expected to move broadly: segment counts from the stop order),
    `stellar/held_bare_core_merger` (renamed if the test is), and any of `stellar/summaries`,
    `stellar/hierarchies`, the planetary and server system goldens that move (expected to be few
    or none). Run `golden_diff.py`, and record the counts of moved digests by cause, as ruling
    129.4's note did.
  - **Statistical checks** (slow, report and assert):
    - `tests/binary_system.rs`: ruling 137's hydrogen-poor share (0.51 now) is re-measured. The
      "Pinned preempting" collapses now count as helium stars, so expect +0.1–0.3 points. Ruling
      123.5's marked-stripped 80% and merged 17% move by under 1 point. Ruling 140.9's ratios do
      not move.
    - `tests/binary_carve.rs`: redraw rates per layer within their current Poisson errors.
    - `binary::` invariants: no pair reaches the segment cap; total mass never rises; no MS
      star older than its lifetime.
    - The R06 census (`tests/stellar_system.rs`, `a_hundred_thousand_systems_live_and_die_in_order`):
      passes.
  - **Acceptance:** `cargo test -p hyperion-sim binary::` and the tests above pass;
    `cargo nextest run -p hyperion-sim --run-ignored only -E 'test(binary_system) | test(binary_carve)'`
    passes with the figures recorded in Risks; determinism-auditor and science-checker reviews
    clean; the version-20 commit (P14 Phase J's T46.f/T47.d) carries this task's goldens.
  - **Finding left open (not this task):** `swell` (`rlof.rs`) places the new giant with
    core = its whole mass. BSE `evolv2` keeps the accretor's core and gives it the accreted
    envelope (`gntage`). Measured to change white-dwarf and helium-star accretors in 1.6% of
    layer-D systems (epoch state 1.1%). Its ruling needs the BSE source checked first.

- **P11.T4.i Start at the first arrival** (ruling p11-channels, 2026-10-06; output moves;
  version-21 batch, re-blessed at 20, held out of integration like T4.h and 9a0950e). Hurley,
  Tout and Pols (2002, §2.8) start both stars on the zero-age main sequence, as COMPAS and SEVN
  do by default. A companion still contracting when its primary expands or explodes is carried
  as its own HPT zero-age main-sequence star.
  - `binary/evolve.rs`:
    - `arrival` is the **first** arrival: the least `main_sequence_arrival` of the members'
      tracks, no later than `until`.
    - `interacts` is false before it.
    - Each star's largest radius in the lobe test is
      `max_radius_until(max(until, arrival_i))`. That is a late star's ZAMS radius until it
      arrives.
    - `own_members_with` builds (or rebuilds a given track) to max(until, the star's own
      `sse::main_sequence_start`) + `reach_margin_years`.
    - Doc comments: `arrival`, `can_interact`, the module's.
  - `binary/star.rs`:
    - `Member::evaluate` and `Member::radius` read a `Member::Track` at a track age held to no
      less than its `main_sequence_arrival`. That is the proxy, at τ = 0, bit for bit
      `own_structure_at(arrival)`.
    - `Member::state_at` is unchanged: the untouched late star is shown on its own pre-MS
      track.
  - `binary/rlof.rs` `carry`: a `Track` member before its arrival becomes
    `Member::MainSequence { mass: its track's mass, tau: 0 }`, through the same held age.
  - Every other engine read of a `Track` member's phase or clock before its arrival is reviewed
    to read the proxy. The known one is `common_envelope.rs::main_sequence_left` through
    `phase_ahead`: a pre-arrival `Track` member has its whole main sequence left, not the time to
    its arrival. `phase_ahead`'s boundary at the arrival stays: a segment ends there, and the
    shown star switches from pre-MS to MS.
  - `galaxy/displaced/binarity.rs` `largest_radius`: the companion's radius is read no earlier
    than its arrival, as `can_interact` reads it. Re-run `hyperion-fit`'s `stripping` task and
    fit-check. Expected unchanged, since the primary's reach is the larger at every tabulated q.
    If it moves, it rides in the batch.
  - Plan 06's Risks pointer on P06.T15.b's convention, and plan 11's design note 7, are
    amended: "the engine starts at the first arrival".
  - **Tests** (`stellar/binary/tests.rs` unless said):
    - `a_late_companion_meets_its_primarys_envelope`. 15 + 1.0 M☉ ([Fe/H] 0, median draws) at
      an orbit the red supergiant fills: a common envelope or merger before plan 06's pinned
      death, `supernova_ages()[0]` equal to that death, and the companion shown as
      `PreMainSequence` until the interaction. Before T4.i: one detached segment and no
      supernova record.
    - `a_late_companions_supernova_is_applied`. 20 + 1.0 M☉ on a wide orbit the primary's
      giant reaches (so the pair passes the pre-test): the supernova is recorded, and the pair is
      bound or not as BSE appendix A1 gives. Before T4.i: no record, and bound on the drawn
      orbit.
    - `a_late_star_is_its_zero_age_self_to_the_engine` (unit): before its arrival
      `Member::evaluate` equals `own_structure_at(arrival)` bit for bit, and `state_at` equals
      the track's pre-MS state.
    - `the_engine_starts_at_the_first_arrival`. `arrival` is the minimum. A pair run to an age
      between the arrivals is tested with the late star's ZAMS radius. A pair whose late star's
      ZAMS radius overfills its drawn lobe interacts at the first arrival.
    - Kept, unchanged in form: `a_pair_run_short_of_the_main_sequence_stays_two_protostars`
      (0.4 Myr is before the first arrival); `stellar_system.rs`'s
      `young_pairs_whose_protostars_overfill_their_orbits_stay_protostars`; the BSE reference
      binaries' phase sequences.
    - Build-age suites (`a_timeline_is_the_same_whatever_age_it_is_run_to`; slow: the 10³ and
      100-pair suites): ages between two arrivals added to their age draws. Bit-for-bit agreement
      for every pair the pre-test passes.
    - `binarity.rs` `the_threshold_is_can_interacts_boundary`: unchanged by T4.i (T4.j amends
      it).
  - **Goldens.**
    - `stellar/binary_timelines` moves in nearly every digest of a run pair with two different
      arrivals. The engine now steps from the first arrival, braking and tides act on the proxy
      from then, and the knots move.
    - Count physical moves as P11.T4.h did, by segment kinds and boundaries with the tracks'
      `Debug` left out. Expected: only pairs whose interaction or supernova falls before the late
      arrival, a few per cent of the 1,000.
    - `stellar/summaries` and any system golden holding such a pair: check with `golden_diff`.
    - The galaxy chain, unless the stripping table moves (above).
  - **Statistics** (slow, record and assert as now). The expected moves are §1.1's, combined
    with T4.j:
    - `binary_system`: ruling 123.5's stripped 80.3% (no move); merged 17%; the merger band
      merged 80–83%; ruling 137's share 0.51.
    - `binary_classes`: double neutron stars 112 ± 11 in the 6–144 window, and the recycled
      pulsars' medians recorded.
    - `binary_carve` within Poisson.
    - The R06 census passes, with its time recorded.
  - **Acceptance:**
    - `cargo nextest run -p hyperion-sim -E 'test(binary::)'`;
    - the slow `binary_system`, `binary_carve` and `binary_classes` suites, the 10³-pair suites
      and the R06 census, with the figures recorded in plan 11's Risks;
    - determinism-auditor and science-checker reviews.

  _As built (Phase J lane, 2026-10-06): see Risks, "P11.T4.i as built"._

- **P11.T4.j A decay-aware pre-test** (ruling p11-channels, 2026-10-06; output moves;
  version-21 batch, after T4.i). Design note 7's lobe test misses pairs that braking (BSE
  eq. 50), gravitational radiation (eq. 48) or tidal spin-up (eq. 34–35, Hut 1981) bring into
  contact before the age asked. Those are the W UMa channel and giants' tidal captures. The
  pre-test now bounds that decay.
  - `binary/evolve.rs` `interacts`:
    - the drawn-orbit test first (unchanged, as `lobe_reached`);
    - otherwise `decay_reaches(input, members, start, until)` (`binary/detached.rs`, beside
      the rates it bounds), the bound of §3.1 items 1–7 with its constants named and cited;
    - each term is dropped when its `BinaryParams` switch is off.
  - `can_interact`'s doc and example: a 0.69-d 1.04 + 0.45 M☉ pair passes at 2 Gyr though its
    stars are inside their lobes.
  - Design note 7 is amended: "can interact … on the drawn orbit, or after the decay the engine's
    sinks can make by then".
  - The build-age contract in plan 11's Risks is strengthened: a pair the pre-test passes over
    at u₁ shows no interaction before u₁ in a run to any later age.
  - `galaxy/displaced/binarity.rs`: `interacting_periastron` and `interacting_share` stay the
    **drawn-orbit** threshold, documented as the lobe part of `can_interact`'s test.
    `the_threshold_is_can_interacts_boundary` keeps "just inside passes `can_interact`" and
    asserts "just outside fails the drawn-orbit test" (crate-private `lobe_reached`), since
    the reservoir passes giants up to about 1.1–1.6 × that threshold. The stripping band is
    unchanged: the captures are late, Case C, at the giants' largest radii, outside "stripped".
  - `binary/supernova.rs` and `binary/evolve.rs` (a fix the gate needs, Finding F1): an orbit
    unbound by a non-sudden death is `SegmentKind::Disrupted { by }`, not `Merged`.
    `Engine::die`'s non-sudden branch today sets `quiet_kind()`, which reads no orbit and no
    gone member as `Merged`. That gives false `merger_age`s, and so false stellar-merger carves,
    and T4.j runs more such pairs.
  - **Tests:**
    - `the_decay_bound_never_passes_over_an_interaction` (slow). The three samples of §1.2,
      6,000 pairs each, fixed seeds, on the T4.i engine, with a `#[cfg(test)]` hook that runs
      the engine past the pre-test.
      - For every pair `can_interact` rejects at u, the forced run holds no stable transfer,
        common envelope, contact or merger (one star left) starting before u and before the
        pair's first supernova. **None allowed.**
      - It records the passes without interaction.
    - `a_thousand_timelines_are_the_same_whatever_age_they_are_run_to`: its missed
      interactions' 2% allowance becomes **zero**.
    - `a_braked_pair_is_run_before_its_contact` (unit). The 1.04 + 0.45 M☉ pair at a = 3.75 R☉
      passes at 2.0 Gyr, reaches contact at about 2.25 Gyr, and a run to 2.0 Gyr equals a run to
      3 Gyr to 2.0 Gyr bit for bit.
    - `the_pre_test_only_widens_with_age` (unit, the 60-pair sample): `can_interact(u₁)`
      implies `can_interact(u₂)` for u₂ > u₁.
    - `a_wide_pair_is_still_passed_over` (unit): 1 + 0.8 M☉ at P = 10⁴ d fails at 13.8 Gyr. The
      bound's P₀ boundary for 1 + 0.8 M☉ at 1 and 10 Gyr is recorded against the science
      check's 0.64 and 1.22 d.
    - `binary_system`'s Rucinski check takes Rucinski's definition (Finding F4): contact pairs
      and main-sequence stars of +1.5 < M_V < +5.5, by the pair's combined M_V. It asserts
      1/1000–1/250 and records the figure against 1/500 per M_V bin.
  - **Goldens.**
    - `stellar/binary_timelines`: the pairs newly passed. About 2% of the lane's sample,
      ~20 digests: the 4 known braked contacts plus pairs now evolved without interacting.
    - The F1 relabel: a few, if any of the 1,000 meets it.
    - System goldens holding a pair under about 2 d, or a giant near its threshold: check with
      `golden_diff`.
    - No galaxy-chain move expected.
  - **Statistics** (expected, §1.2):
    - contact pairs per MS star of the same M_V over +1.5 < M_V < +5.5 ≈ 3 × 10⁻³ (asserted
      10⁻³–4 × 10⁻³), and over +1.5 to +7.5 ≈ 2.1 × 10⁻³ (recorded);
    - per MS star fainter than +1.5 ≈ 2.9 × 10⁻⁴ (recorded);
    - the R06 census time recorded against 234 s: the extra runs are a few per cent;
    - `binary_evolve` and `system_full` re-benched under the lock, recorded as provisional.
  - **Acceptance:** as T4.i, plus the zero-miss gate.

  _As built (Phase J lane, 2026-10-06): see Risks, "P11.T4.j as built"._

- **P11.T4.k Every collapse in a run pair is applied** (ruling p11-supernova-pins, 2026-10-06,
  amended by ruling p11-t4k-faults, 2026-10-06; findings F7 and F8 of P11.T4.i, and A and C of
  T4.k's own gate; output moves; version-21 batch, after T4.j, re-blessed at 20 and held out of
  integration like T4.h–T4.j).
  - **The rule.** A core collapse takes the dying star's last living mass, never its remnant's
    (ruling 129.4a). It is applied through BSE appendix A1, and recorded.
    - A star whose own clock has ended is a collapse now, or the pin's hold, never a living
      track (design note 16; ruling 129.4c).
    - Every phase that advances the engine's age acts on a death or the pin that it reaches.
  - **F7, `binary/supernova.rs` `Engine::pinned_collapse`:**
    - A member on the pin's own track whose track dies now, to the engine clock's resolution
      (`own_death_now`), is living by construction. The pin wins the tie with the track's own
      `Stop::Death`, and the track there already shows its remnant.
    - Its mass before:
      - a `Member::Track` takes the death's progenitor mass (helium core plus envelope);
      - a `Member::Shaped` takes its carried mass at its last living instant (`last_living`),
        read on its closed forms there;
      - both are the readings `Engine::die` takes.
    - The `Member::Track` stays its own track, placed with `offset_for` as `die` places it. It
      shows plan 06's remnant bit for bit (debug-assert `track.remnant() == pin remnant`).
    - Every other form keeps today's reading.
    - Update the docs of the module and of `pinned_collapse`.
    - Plan 11's ruling-129.4a text gains: "the pin's collapse takes the living mass, never the
      remnant's".
  - **F8, `sse/track/binary.rs` `Track::remains_at`:**
    - A strip whose helium star has no life left (its built track is a remnant from its own
      start, `Track::is_remnant_from_its_start`) is `Remains::Ended`, renamed from `Collapse`.
      That is "a bare core with no life left".
      - `track` is the helium track that is dead from its start, and holds its own end.
      - `core` is the helium star's last living state at the stripped mass (`LastLiving`): the
        helium giant at min(clock₀, t_end), or the light helium main-sequence star.
      - `core_radius` is that structure's.
    - Every helium-star arm goes through `stripped_to`. The arms found:
      - the early AGB: a massive core collapses as a neutron star or black hole, and a
        1–3 M☉ star stripped at the end of its early AGB leaves a carbon–oxygen white dwarf of
        its whole mass;
      - the HG and FGB of 2.0–2.5 M☉: a core below the lightest helium star is a helium white
        dwarf at once.
  - **The engine's handling** (`common_envelope.rs` `stripped_member_at`):
    - Any star but a pinned primary with its pin still to come dies at once through `die`.
    - A pinned primary is `Member::Frozen` at `core` until its pin.
    - Debug-assert that no helium star is placed alive when it is dead from its start.
  - **A, `binary/rlof.rs` `transfer_phase`:**
    - A step whose limit is a death (`Stop::Death`) or the pin (`Stop::Pinned`) is not ended
      by the detachment test. It goes through the step's acts, and its stop is acted on, T4.g's
      strip first.
    - Where the stop leaves the donor with nothing living, the transfer ends there, as
      `quiet_kind` decides. Debug-assert that no transfer step starts from a donor that is not
      living.
    - Before T4.k, the "transfer is over" return came first: the donor read inside its lobe as
      its remnant, so the death or the pin was never seen again. That lost the case BB helium
      donors' collapses (55 per 12,000 layer-E prior pairs) and the pinned transfer donors'.
  - **C, `binary/common_envelope.rs` `contact_phase`:**
    - The knots stop at the pin where it lies inside the contact, whatever the age asked.
    - The pin is acted on only where it lies before the pair's age. Otherwise the pair stays in
      contact, as a contact outlasting the age does.
    - `contact_until` is cleared after the collapse.
    - Before T4.k the contact ran to its coalescence past the pin, and the merger product died
      at its own later age (pairs 0223 and 0277 of the thousand).
  - **The backstop, `binary/evolve.rs` `Engine::run`:**
    - Before each phase, a pin still pending at the engine's age (to `own_death_now`'s
      resolution) is acted on.
    - Debug-assert that no pending pin lies behind the engine's age.
  - **Tests** (`stellar/binary/tests.rs`, unless said):
    - The pins ruling's tests, as built:
      - `an_untouched_pinned_primary_collapses_on_its_orbit`;
      - `a_wind_stripped_primary_collapses_in_place`;
      - `a_late_companions_supernova_is_applied`, at 0.9 of the drawn reach;
      - `a_core_stripped_past_its_end_is_held_until_its_pin`;
      - `a_core_stripped_past_its_end_dies_at_once` (pairs 0544 and 0145);
      - sse's `a_core_stripped_past_its_helium_stars_end_has_no_life_left`.
    - `a_carried_primary_collapses_at_its_pin` (the carried tie): one of 0134, 0156, 0446, 0554
      and 0677.
      - The record is at the pin.
      - Its mass before is the shaped member's carried mass at `last_living` of the death, to
        10⁻¹².
      - The remnant is plan 06's, capped at that mass.
    - `a_transfers_last_step_applies_the_pin` (A): pair 0004.
      - The pinned primary, a stable-transfer donor on its own track, has its record at its pin:
        a 14.13 M☉ black hole at 6.4576 Myr, bound.
      - Before T4.k there was no record.
    - `a_transfers_last_step_applies_the_death` (A): pair 0741.
      - The helium-star companion, transferring onto the black hole, has its record at its own
        death: a 1.411 M☉ neutron star at 18.6316 Myr, bound.
      - No merger follows.
      - Before T4.k there was no record, and the pair merged at 985.7 Myr.
    - `a_contact_stops_at_the_pin` (C): pair 0277.
      - The record is at 46.831 Myr, from the main-sequence primary in contact.
      - A run to 30 Myr, inside the contact and before the pin, records nothing, stays in
        contact, and equals the full run up to 30 Myr bit for bit.
    - Invariants in the 10³-pair suite (`check_collapses`, in `check_invariants` and
      `a_thousand_timelines_are_pinned`), on every timeline not capped:
      - no member goes from a living phase to a neutron star or black hole without a supernova
        record at that age (10⁻⁹ relative);
      - every pinned primary living just before its pin has a record at its pin;
      - no segment's member is a track that is a remnant from its own start, unless the engine
        placed it there by its death.
  - **Goldens:**
    - `stellar/binary_timelines`: about 55 of 1,000 digests (the prototype's).
      - By cause: 33 F7 (the pins ruling's 29, plus 0330, 0480, 0782 and 0886, wide pairs that
        T4.j now runs), 7 F8, 5 the carried tie, and 10 A and C.
      - Every one gains a record or moves one (0277: the collapse moves from 69.05 to 46.83 Myr).
      - Count the moves by cause, as T4.i did.
    - `stellar/summaries` and `stellar/hierarchies`: none expected. Run `golden_diff`.
  - **Statistics** (slow; record them, and assert where they are asserted now). The expected
    figures are ruling p11-t4k-faults' probe on layer E's 12,000 prior pairs, against 1e47d16:
    - primary records 5,457 → 6,755;
    - companion records 1,225 → 1,301;
    - bound BH + BH 386 → 460, NS + BH 94 → 119;
    - double neutron stars 112 → 115 (window 6–144);
    - the hydrogen-poor share of all records 0.587 → 0.538;
    - "nothing living left" 168 → 146, plus 2 capped timelines;
    - the shown bound share after the primary's collapse 73.6% → 66.8% (recorded 43.2% →
      42.2%);
    - `binary_system`: ruling 137's share recorded (about 0.48), and ruling 123.5's marks
      unchanged;
    - `binary_carve` within Poisson; the R06 census passes, with its time recorded.
  - **Acceptance:**
    - `cargo nextest run -p hyperion-sim -E 'test(binary::)'`;
    - the slow `binary_system`, `binary_carve` and `binary_classes` suites, the 10³-pair suites
      and the R06 census, with the figures recorded in Risks;
    - the scratch collapse census: 0 faults in the pinned thousand, and none in 22,000 prior
      pairs outside capped timelines;
    - determinism-auditor, science-checker and rust-reviewer reviews.

  _As built (Phase J lane, 2026-10-06): see Risks, "P11.T4.k as built"._

- **P11.T4.l A passed-over pair's collapses** (ruling p11-supernova-pins, 2026-10-06; finding
  F3, P11.T10's first half brought forward; output moves; version 22, its own bump; after
  P11.T4.k and the version-21 refit).
  - **The rule.** A pair that `can_interact` passes over is two single stars on its drawn orbit
    (design note 7). If either star collapses (a sudden death) before the age asked, the drawn
    orbit stands to the collapse.
    - **The pinned primary:** plan 06's age, remnant and kick, with T4.k's living mass. Any other
      star: its own track's death, its remnant, and `companion_kick`.
    - At the collapse, BSE appendix A1 is applied once. Its input is the drawn orbit's relative
      position and velocity at the collapse's universe time (the rails' phase), widened for the
      mass both stars' winds have lost since zero age.
      - The widening is Jeans mode with no accretion (HTP02 §2.2): a M_total is constant, e and
        the true anomaly are kept, so r × f and v ÷ f, with f = M_total,0 ÷ M_total,before.
      - It is a closed form of the stars' own masses at the collapse, so it does not depend on
        the age asked.
      - The orbit shown before the collapse stays the drawn one (design note 7's convention).
        Showing it widened would make a pair's pre-collapse states depend on whether its run
        reaches the collapse.
    - **Bound:** the pair goes on, on the new orbit's fixed elements, with the recoil recorded.
    - **Unbound:** `Disrupted { by }`, both stars their own models.
    - A later collapse in a still-bound pair is applied the same way on the post-collapse orbit.
    - Every collapse writes its `SupernovaRecord`.
    - The stars stay their own tracks bit for bit; only the orbit and the records change.
  - **After a bound collapse,** the pre-test is asked again of the new orbit, from the collapse
    to the age asked: the lobe test at its periastron with each star's largest radius over that
    span, and T4.j's decay bound from the collapse.
    - If it passes, the engine runs from the collapse (`Engine::new` at the collapse's age with
      the members and the new orbit).
    - Otherwise the orbit stays fixed.
    - Measured: 0–3 of 5,607 such pairs interact after their collapse. That is 0 with the
      widened input, 1 with the unwidened drawn orbit, and 3 by the engine route.
    - In pair 6441 (unwidened) a neutron star born at 39 Myr is kicked onto an eccentric orbit
      and meets its companion's giant at periastron: a common envelope, then the helium star
      feeds the neutron star and is consumed.
    - A kick-free collapse cannot bring a periastron in. Instantaneous mass loss leaves the new
      periastron at least the old one (checked over 2 × 10⁵ random phases and eccentricities).
    - Two or three more pairs show `Merged` only as F1's label, for an orbit unbound later by the
      companion's white-dwarf birth. T4.j relabels it `Disrupted`.
  - **Files:**
    - `binary/evolve.rs`: `evolve_with_tracks`'s passed-over branch.
    - `binary/supernova.rs`: a driver for passed-over collapses, reusing `explode` and
      `lose_mass`.
    - `stellar/system.rs`: `run_pairs`' doc (a dead pair is run so that its collapses act on its
      orbit).
    - Plan 11:
      - design note 7 gains "…two single stars on an orbit that their core collapses change (BSE
        appendix A1)";
      - P11.T10 is amended as in this ruling's §7;
      - the version-21 limit in Risks is closed.
  - **Known departure, recorded.**
    - The orbit shown before the collapse is the drawn one, not the wind-widened one, as for
      every passed-over pair. The widening factor is M_total,0 ÷ M_total, for example 1.6 for
      25 + 20 M☉ and 2.5 for a 40 M☉ primary that winds strip to 10 M☉ beside 10 M☉.
    - A1 starts from the widened state, so the orbit after a bound collapse is the widened one,
      and the shown orbit steps there.
    - Measured: the widening moves the statistics by under 1% (the bound share 34.0% → 33.9%),
      and slows the wide pairs' walkaways by about 5%.
    - A white dwarf's birth in a passed-over pair still keeps the drawn orbit. That is F1's
      physics, under its own ruling.
  - **The build-age contract** holds bit for bit:
    - up to a collapse, the drawn orbit does not depend on the age asked;
    - past it, the closed form and any engine run start at the collapse;
    - T4.j's wording, that a pair the pre-test passes over at u₁ shows no interaction before u₁
      in a run to any later age, covers the post-collapse pre-test.
  - **Tests:**
    - `a_wide_pair_is_unbound_by_its_primarys_collapse`. 15 + 3 M☉ at P = 10⁵ d, circular,
      kicks off, so the pair is passed over.
      - The record is at plan 06's death; more than half the pair's mass is lost, so
        `Disrupted { by: Primary }`.
      - The companion's velocity is its orbital velocity in the barycentre frame to 10⁻⁹.
      - There is no recoil, and both stars are their own models.
    - `a_wide_black_hole_pair_stays_bound`. A direct-collapse primary (40 M☉) beside 10 M☉ at
      P = 10⁵ d: bound, with A1's a′ and e′ for the small mass lost, a record, and a recoil.
    - `a_kicked_wide_pair_that_closes_is_run_from_its_collapse`. Find one on the built engine:
      the probe found 0–3 per 5,607, so search 10⁵ layer-E prior pairs. If none appears, test
      the path with a constructed post-collapse orbit: the engine started at a collapse on a
      given eccentric orbit, whose periastron the companion's giant reaches.
    - `the_widening_is_jeans_mode`: for a kick-free collapse, A1's input separation is the drawn
      one × f, e is the drawn one, and the true anomaly is the rails', to 10⁻¹².
    - `a_passed_over_pair_is_the_same_whatever_age_it_is_run_to`: ages before and after its
      collapses, bit for bit to the earlier.
    - The build-age suites gain passed-over pairs with collapses in their age draws.
    - `binary_classes`: the shown double-neutron-star class agrees with the record count
      within 15 pairs (126 against 112).
  - **Goldens:**
    - `stellar/binary_timelines`: the 15 passed-over pairs with a collapse among the 1,000
      (0083, 0221, 0330, 0430, 0480, 0538, 0629, 0648, 0775, 0782, 0836, 0872, 0886, 0887,
      0998), each with its record and segments.
    - System goldens: none expected (checked with the prototype).
    - Run `golden_diff`.
  - **Statistics** (expected, §3; layer E, 12,000 prior pairs):
    - the shown bound share after the primary's collapse 70.3% → 34.0% (neutron stars 17.2%,
      black holes 61.2%);
    - ruling 137's share about 0.48 → about 0.32–0.38 in `binary_system`;
    - bound NS/BH + living star −86% in time;
    - the shown double-neutron-star class −64%;
    - symbiotic NS X-ray binaries −95%;
    - released companions of 2.5 M☉ or more 1,001 → 2,212, 99.7% of the new ones below
      30 km/s;
    - double neutron stars by record 112, no move;
    - Roche-lobe LMXBs and millisecond pulsars unchanged;
    - the R06 census time recorded.
    - Run `just fit-check`. The R06 sky tables are expected not to move: 1 pair in 5,607
      changes its light. If a table does move, refit it in this task.
  - **The wind-spin term** (orchestrator ruling on P11.T4.j's open question, 2026-10-06). T4.j's
    decay bound is kept as built in version 21. T4.l adds the term it lacks, so that the bound is
    provable: a locked star's wind takes 2/3 ṁ R² Ω (HPT eq. 110), which the tides refill from the
    orbit, so S gains (2/3 − k′₂) ΔMᵢ R_max,i² Ω_c for each star (the T4.j probe's wind-spin
    variant: 0 misses, up to 2.9 × the passes). Re-run the zero-miss gate
    (`the_decay_bound_never_passes_over_an_interaction`) and record the passes.
  - **Acceptance:** as T4.k's, plus `GENERATOR_VERSION` 21 → 22 with every golden re-blessed,
    and P11.T10's agreement test, if it then exists, re-baselined.

### P11.T5 Classes from state

Build `BinaryClass`, `classify`: Algol and contact pairs; blue straggler (a main-sequence star above
its population's turn-off mass at that age, from merger or accretion); hot subdwarf (a stripped
helium-burning core of about 0.5 M☉); R Coronae Borealis (a helium–carbon-oxygen white dwarf merger
product); cataclysmic variables by kind (dwarf nova or nova-like by the disc instability line in
transfer rate against period, magnetic by the white dwarf's field draw, AM CVn for helium donors);
symbiotics (a white dwarf or neutron star fed by a giant's wind above a luminosity threshold, for
wide pairs too); low- and high-mass X-ray binaries, Be/X when the donor is plan 06's Be star,
persistent or transient by the irradiated-disc line; millisecond pulsars (spin and field after
recycling as closed forms in accreted mass, composed with plan 06's spin-down; source re-checked and
cited); double neutron stars and double white dwarfs; Type Ia progenitor candidates. `carved_class`
maps these and the merger ages onto the carved classes: a stellar or neutron-star merger whose
`merger_age` falls inside the source horizon; an X-ray binary or accreting white dwarf at any age
inside it, the white dwarfs split by P_rec against L. Exploded as a Type Ia is the fifth carved case
and is T6's. Also `BinaryTimeline::from_marks` (Design note 10), with a test that a timeline built
from the marks read off an evolved timeline gives the same `carved_class` and the same `state_at`
now.

Files: `stellar/binary/{classify,recycling}.rs`.

Tests: each reference binary of T4 gets its expected class at its expected age; classification is a
pure function of state; every class is reached at least once in 10⁶ prior-sampled binaries of mixed
populations (slow). Acceptance: `cargo test -p hyperion-sim binary::classify`, and the slow test
under `just test-slow`.

### P11.T6 The Type Ia coupling

Build `tables/binary.rs` with `IaPoolChannel`, `IaYieldTable` and the scratch `IA_YIELD` (η = 1 ÷ 6
everywhere; Design note 13), marked provisional in its header and, once plan 15's P15.T2 has built
`tables::MANIFEST`, registered there as provisional; `IaExplosionMark`, the
reader; and the mark: a pooled event explodes when an integer draw on `binary.ia_mark` falls under
η's threshold for its delay bin and channel. An unexploded pooled event stays what the engine made
it. In `SystemStars::generate`, a binary whose marked explosion lies at or before +H redraws (Design
notes 8 and 9), which is the requirement P09.T35 records; wire that task's `debug_assert` hook to
`carved_class`. Expose `peters_separation_for` and the post-envelope helper to plan 09's Type Ia
entry, and add a test that the entry's binary, run through `evolve`, merges at its drawn delay to
1%. Bump the version.

Files: `tables/binary.rs`, `tables/mod.rs`, `stellar/binary/ia.rs`, `stellar/system.rs`.

Tests: no grid system has an exploded binary at any t ≤ +H over 10⁵ layer-D systems; the redraw rate
in layer D is recorded and asserted within 0.3–4.5% under Kroupa with the scratch table, and within
the brainstorm's 2–4% once T15 lands (slow); exploded ÷ pooled is η to Poisson accuracy; the pool's
merger rate over the Ia target rate is recorded against the observed five to seven (plan 15 accepts
4.5–7); attempt n is reproducible alone. Acceptance: `cargo test -p hyperion-sim binary::ia` and
`just test-slow`.

### P11.T7 The grid is conditional on the binary classes

Build the redraw on `carved_class` being `Some` in `SystemStars::generate`. Add `ClassShareTable`
and the scratch `CLASS_SHARES` to `tables/binary.rs`, scaled so that Milky Way parameters give 9 ×
10⁶ accreting white dwarfs, 10⁴ X-ray binaries, 9 × 10⁴ stellar mergers in the source horizon (0.35
a year × 264,144 years) and 10 neutron-star mergers. Plan 09's `FeatureShares` gains
`set_class_shares(&ClassShareTable)` (this plan's code in plan 09's file, as plan 09's Provides
records), folded into `field_factor(population, band)` beside `type_ia::ancient_share`, so each
population and layer gives up exactly what T8's classes will hold. Bump the version: about 10⁻⁴ of
grid systems go.

Files: `stellar/system.rs`, `tables/binary.rs`, `galaxy/features/shares.rs`.

Tests: over 10⁵ sampled grid systems none is in a carved class at any age in the source horizon (the
grid half of complementarity; T8 adds the catalogue half); the attempt histogram is recorded and no
system reaches `MAX_REDRAWS`; `field_factor` summed with the class shares returns each population's
budget to 10⁻⁹; `check_index_headroom` still passes. Acceptance:
`cargo test -p hyperion-sim binary::carve`.

_As built (round 9c, `bin5b`; output moves, its bump batched into version 16): see Risks,
"Deviations in P11.T7, as built"._

### P11.T8 The catalogue side

Each subtask implements plan 09's `ClassProcess`: a bound over a cell from the class's row of
`CLASS_SHARES` and the populations' budget densities, a cell size through `ClassId::cell_log2_ly()`
(512 ly for the accreting white dwarfs and X-ray binaries, 4,096 ly for the two merger classes, with
the low cell bits zero), a candidate draw through a `ClassSampler` over the class's scratch
`ClassSamplerTable`, scan marks first, a `BinaryTimeline::from_marks`, the class's test
(`carved_class` equal to the class, Design note 10), and `SystemStars` for the entry. Each registers
its `ClassId` with plan 09's `CatalogueClassSource`, its candidate tag under `class.`, bumps the
version and adds goldens. A candidate that fails the test is thinned like any other, and the scratch
tables must keep that under one in ten, which a test counts.

- **P11.T8.a Accreting white dwarfs.** `ClassSampler`, shared by the subtasks. Two class values,
  `ACCRETING_WHITE_DWARF = 6` for the fast hosts and the new `ACCRETING_WHITE_DWARF_SLOW = 8`
  (Design note 12). Marks: white dwarf mass, donor mass and kind, period, transfer rate, magnetic or
  not; recurrence P_rec = ignition mass ÷ transfer rate, with the ignition mass a fit in white dwarf
  mass and rate whose source T8.a re-checks and cites (candidates: Townsley and Bildsten 2004; Yaron
  et al. 2005). A history (initial masses, age) drawn from the table given the present state. A slow
  host's eruption times inside the source horizon are part of its scan marks. Test: the local
  density at `sunlike_point` for Milky Way parameters is within 30% of 4.8 × 10⁻⁶ per cubic parsec
  (Pala et al. 2020); the expected galaxy total is 6–12 × 10⁶ over ten seeds; the two classes
  partition the hosts, and no slow host has more than two eruptions in the horizon.
- **P11.T8.b X-ray binaries.** Low-mass (neutron-star and black-hole, persistent and transient),
  high-mass (Be/X, supergiant). Hosts follow the old populations for low-mass and the young disc,
  displaced by the pair's recoil, for high-mass. Test: about 10⁴ in total (0.7–1.5 × 10⁴) and 1,300
  ± 400 black-hole transients at Milky Way parameters (Corral-Santana et al. 2016).
- **P11.T8.c Stellar mergers.** One-shot entries with the merger time T as a mark, uniform over the
  source horizon; the pair before T, a luminous red nova from T, the merged star after. One ID on
  both sides of T. Test: an expected 0.2–0.5 a year (Kochanek et al. 2014); evaluated before T the
  entry is a contact or inspiralling pair whose `merger_age` is T, and after T a single merged star
  under the same ID.
- **P11.T8.d Neutron-star mergers.** As T8.c with Peters's inspiral before T and a kilonova after;
  the remnant by total mass. Test: the expected count at Milky Way parameters is 8–12, the
  brainstorm's "about 10", and the realised counts of ten seeds pass
  `hyperion_testkit::stats::assert_poisson_count` against it.
- **P11.T8.e The centre's feature-level index.** Plan 09 flags it and the arithmetic bears it out:
  the index under the spare band value is 13 bits, 8,192 members, and a nuclear cluster of 4–6 × 10⁷
  systems at the galaxy-wide share of 10⁻⁴ expects 2,400–6,000 accreting white dwarfs before any
  allowance for its density or for thinning. Add `check_feature_list_headroom(galaxy)`, called
  beside plan 03's `check_index_headroom`: the expected candidates of all classes on a feature's
  list, plus five standard deviations, must fit. Run it over plan 02's seed sweep, before T8.f puts
  anything on the centre's list. If it fails, this task stops and reports; it does not choose. The
  two ways out both belong to the brainstorm's owner: widen the index into the level and cell bits,
  which are zero under the spare band value (every existing ID keeps its value, but it contradicts
  the brainstorm's "under it the level and cell are zero" and plan 01's canonical rule); or plan
  09's alternative, list at the centre only hosts above a brightness floor, which leaves the dim
  hosts as ordinary members that alerts cannot find from afar. Acceptance: the check passes for
  every seed of the sweep, or the finding is recorded in this plan's Risks with the figures.
- **P11.T8.f Inside features.** The same classes on plan 09's `FeatureLevelList`, appended after
  plan 09's own in the order `STELLAR_MERGER`, `NEUTRON_STAR_MERGER`, `XRAY_BINARY`,
  `ACCRETING_WHITE_DWARF`, `ACCRETING_WHITE_DWARF_SLOW` (plan 09's design note 16), with counts from
  the feature's own parameters: the feature's budget times the class's row of `CLASS_SHARES`, and
  for globulars the encounter-rate scaling of P09.T9.e in its place. `draw_hierarchy` and a
  conditional binary draw for members that plan 09 marks millisecond pulsar, X-ray binary or blue
  straggler; members of plan 09's binary classes get `ForcedMultiple { max_separation }` at the
  hard–soft boundary (T2.b), single classes `ForcedSingle`. Plan 09 leaves the clusters' binary
  fractions as scratch numbers "that plan 11 replaces": open clusters take the model's multiple
  fraction restricted to pairs inside the hard–soft boundary, and globulars keep plan 09's observed
  first- and second-population values, re-checked and cited (Milone et al. 2012). Test: a 47
  Tucanae-like cluster (`features::testing::named_cluster`) shows its marked pulsars as recycled
  binaries or isolated recycled pulsars, none as young pulsars; registering these classes leaves
  every supernova entry's index unchanged.

Files: `galaxy/catalogue_classes/binary/{mod,awd,xrb,merger,nsm,feature}.rs`,
`galaxy/catalogue_classes/mod.rs` (the new `ClassId`), `stellar/binary/scan.rs`,
`stellar/binary/sampler.rs`, `tables/binary.rs` (scratch sampler blocks),
`galaxy/features/members.rs` (T8.e's check).

Shared tests: the catalogue half of complementarity, through plan 09's
`catalogue_classes::testing::assert_complementary`: every entry of every class passes `carved_class`
with its own class at every age in the source horizon, on both sides of a merger's T; each sampler
against `brute_force_class_members` on the marks both define, by chi-square, with the scratch
tables' tolerance stated (slow), which is what gives complementarity its meaning here: the catalogue
holds the same kind of binary that the grid refuses; budgets: field plus features plus catalogue
equals each population's budget in expectation to 10⁻⁶, and plan 09's sampled budget test (P09.T38)
still passes; `scan_marks` for 10⁶ hosts runs with no call into `evolve` (`scan::engine_calls()` is
zero). Acceptance: `cargo test -p hyperion-sim catalogue_classes::binary` and `just test-slow`.

### P11.T9 Binary events and light curves

Register the seven event tags of Provides in `id/event_tags.rs`, numbers explicit, with their
`DomainTag`s of scope `Event`. Build `events_in(system, window)`, `active_at(system, t)` and
`light_curve(event, dt, band)` over plan 06's `events` module, with no construction of this plan's
own. Each tag's series is an `events::EventSeries::new(seed, tag, subject)` (the subject a
`BodyId` or `SystemId` as `EventSubject`), which derives the `EventKey`; a series goes to one
construction only, and a `RateModel`'s `bound` takes the bin's `TimeWindow` and returns
`EventsPerSecond` (P06.T27 as built):

- `MonotonePhase` with a `LinearClock`: classical and recurrent novae with P = P_rec; dwarf novae
  with a period from the transfer rate against the disc-instability rate, skip mark on; X-ray
  transients; Type I X-ray bursts on neutron-star accretors.
- `PoissonBins` with a `RateModel`: Be/X giant (Type II) outbursts. Type I outbursts at periastron
  are a continuous function of orbital phase, not events, and are part of `state_at`.
- One-shot: luminous red nova and kilonova at the entry's T, bin 0 and number 0 of their tags, as
  plan 09's `SUPERNOVA` is.
- Light curves as closed forms per tag: peak, rise, decline (nova decline time from white dwarf
  mass; dwarf nova plateau and decay; fast rise and exponential decay for X-ray transients; a
  plateau of 100–200 days scaled by merger mass for red novae; a power-law fade over days for
  kilonovae), in plan 07's `Band`s, and `xray_luminosity` for the X-ray kinds.

Files: `stellar/binary/events.rs`, `stellar/binary/light_curves.rs`.

Tests: both constructions return the same events in any order of asking and for split intervals
(plan 06's `events::testing::assert_partition_independent`); events are strictly ordered; no event
leaves state that needs replay (state after n eruptions is a closed form in n); the galaxy's nova
rate at Milky Way parameters is the brainstorm's 30–50 a year (slow, from a 1% sample of the
catalogue; a miss is a finding against the scratch sampler's transfer rates and is re-run in T15).
Acceptance: `cargo test -p hyperion-sim binary::events`.

### P11.T10 Agreement with kicks, displaced objects and runaways

Build the conditional multiplicity of layer-D and -E systems on plan 08's `record.placement_class()`
and plan 06's stripped mark (Design note 1): `Displaced { kind: Remnant, .. }` with an ordinary-mode
kick is `ForcedSingle`; `Retained` and low-mode remnants keep their drawn companion and move at the
pair's recoil (10–30 km/s, checked against the velocity plan 08 drew and its `kick_constraint()`);
`Displaced` with kind `Runaway` or `Walkaway` is `ForcedSingle`; a grid binary whose supernova
unbinds it shows the remnant alone, because the released companion is plan 08's independent object.
Low-mass released companions are left out, as the brainstorm says. Bump the version.

Files: `stellar/system.rs`, `stellar/binary/supernova.rs`.

Tests (slow, prior-sampled massive binaries): the stripped share equals `stripped_share` to 0.02;
the low mode is 20 ± 10% of neutron stars and nearly all of its members keep a companion; more than
half of double neutron stars have e < 0.3; the rate of released companions above 2.5 M☉ matches the
supernova-release part of plan 08's `RunawayModel` (walkaways, and the half of runaways not ejected
by encounters) to 25%, most of them under 30 km/s; retention in a cluster with a 50 km/s birth
escape speed is at least a tenth with pairs judged on system velocity; at least half of black holes
are unkicked; and, for Design note 16, `SystemStars::death_time` and `natal_kick` of 10⁴ layer-E
systems equal what plan 06 alone gives. Acceptance: `just test-slow` passes these by name
(`binary_kick_*`).

### P11.T11 System assembly

Build `SystemStars::state_at`: every star's state (from a timeline where one exists, else plan 06),
`BinaryClass` per pair, orbits at t, combined luminosity, system mass now. The tidal radius and any
census field that used the primary's mass alone now use the system's. Measure the rare bright
exception: the share of layer A and B systems whose combined luminosity exceeds that of a 0.75 M☉
star at the same age by a factor of ten, by population, recorded in the doc comment of plan 03's
mass floor and asserted under 10⁻³.

Files: `stellar/system.rs`, doc edits in `galaxy/query`.

Tests: `state_at` continuous in t across ±H except at listed events; a full `SystemStars::generate`
call stays under the brainstorm's millisecond in the `system_full` bench (finding, not failure).
Acceptance: `cargo test -p hyperion-sim stellar::system`.

_As built (round 9c, `bin5b`; output moves, its bump batched into version 16): see Risks,
"Deviations in P11.T11, as built"._

### P11.T12 Statistical suite

Slow tests, fixed seeds, Milky Way parameters unless stated: multiplicity by primary mass in six
bins against the anchors (chi-square); companion count ratios for Sun-like primaries; period and
mass-ratio Kolmogorov–Smirnov from generated systems, not from the samplers; the all-stars test on
generated systems of all five layers weighted by share, brown dwarfs left out, against the 20 pc
census's 69% below 0.5 M☉ (68.8%; Kirkpatrick et al. 2024): under the default, Chabrier's with plan
15's scale (provisionally 0.68, which gives about 71% with plan 02's stand-in companions; P15.T4.b
fits it together with this plan's companions), 69 ± 1%; under Chabrier with `high_mass_scale` 1
about 67%, and under Kroupa 76.4 ± 0.5%, both recorded; a miss under the default is a finding
against the companions or the scale, not a reason to widen the window; blue straggler and hot
subdwarf fractions in an old population against the figures plan 06 uses for its class-fraction
tests; a Hertzsprung–Russell dump of a cluster with binaries for the check by eye.

Files: `crates/hyperion-sim/tests/binaries_statistical.rs`.

Acceptance: `just test-slow` passes; `just ci` stays green.

### P11.T13 Protocol and server

Extend plan 06's `StarSummaryDto` with `body_index` and `binary_class: BinaryClassDto`; add
`OrbitDto` and `HierarchyDto`; `SystemSummaryDto` gains `hierarchy`, evaluated at the request's
time; `StellarBriefDto` gains `star_count`. By ruling 33 of 2026-09-22 the two new types carry what
the client needs to place and propagate every star itself (plan 14's D18), and plan 14's
`BodyOrbitDto` wraps the same `OrbitDto`, so the shape is fixed here, before any client code is
written:

- `OrbitDto`: the whole element set, not only the period, semi-major axis, eccentricity and
  inclination: also the longitude of the ascending node, the argument of periapsis and the mean
  anomaly at the epoch, all in radians, and the gravitational parameter μ in m³ s⁻². Every field
  name carries its unit (`period_s`, `semi_major_axis_m`, `mu_m3_s2`, …), as plan 06's DTOs do.
- `HierarchyDto`: a flat list of nodes, each star's node with its body index and its mass, and each
  pair's with its two children and its `OrbitDto`. The masses are those `star_positions_at` uses, so
  that the client places the stars about each barycentre exactly as the server does.

This task designs the exact fields. Every change is an additive field under plan 04's convention,
on plan 06's `system_summary` kind (P06.T33) and on the `systems_in_range` rows; no request kind is
added. As in P06.T33, an optional field takes `#[serde(default)]`, `skip_serializing_if` and ts-rs's
`optional`, so plan 04's and plan 06's pinned wire forms stay valid byte for byte. The server caches
`SystemStars` in the byte-bounded system cache that plan 06's P06.T34 builds, a `SharedByteLru`
keyed by `(GalaxyKey, SystemId)`, with `HeapBytes` extended to hierarchies and timelines. Run
`just gen-protocol`.

_Slice:_ the vertical slice (README) takes `OrbitDto`, `HierarchyDto` and `body_index` first, since
the `SYSTEM` display needs them and nothing else here. `binary_class`, which needs T5's classes,
and `star_count` come with the rest of this task, each an additive field.

Files: `crates/hyperion-protocol/src/*.rs`, `crates/hyperion-server/src/*` (summary handler, cache
sizing), `packages/protocol/src/generated/*`, `packages/protocol/src/index.ts`.

Tests: wire-form tests for each type; a server integration test requests the summary of a pinned
triple and gets three stars and two orbits, with the elements and μ of each and the stars' masses.
Acceptance: `just ci`.

### P11.T14 Display

The `GALAXY` display's readout lists every star of the selected system (designator suffix A, B, C in
hierarchy order, class, mass in M☉ with the drawn `☉`, state) and each orbit (period and semi-major
axis in the guide's units, eccentricity); the system list gains a star-count column; binary class is
shown in words. The chart symbol is unchanged: shape still encodes type, taken from the brightest
star. No new colour.

Files: `apps/hyperion/src/renderer/src/displays/galaxy/{SystemReadout,SystemList}.tsx` (plan 05's),
`components/StarList.tsx` and its test, `lib/format.ts` (`formatOrbit`).

Tests: Vitest renders a triple and a single; units and digit grouping follow the guide; the list is
keyboard-reachable. Acceptance: `pnpm test`, `just ci`, and a check by eye against a known triple.

### P11.T15 Swap in plan 15's tables (version bump)

When P15.T9.b and P15.T10.b have landed `tables/binary.rs`: clear the `provisional` flags, bump the
version, regenerate goldens, and re-run T6's redraw rate (now 2–4%), T8's counts and sampler
chi-squares at the fitted tolerance, and T9's nova rate. Likewise note P15.T4.b's scale in T12's
Chabrier window and P15.T5.c's verdict on `CLUSTER_MERGED_BINARY_FATE`; if only the alternative
passes its four retention bands, change the default here with the bump. Acceptance: `just ci` and
`just test-slow` green with no provisional entry for `binary` in `tables::MANIFEST`.

### P11.T16 The census's hierarchy bound (R06's ask A)

Decided 2026-10-05 (`decision-r06-census-cost.md`, §7, "Ask A"), for rendering plan R06's
R06.T8.g, the sky census's bound star by star. Built at `GENERATOR_VERSION` 21, beside P11.T17 (ask
B), while the 21 → 22 batch (T4.l, T4.m) waits (owner, 2026-10-07: features first).

`stellar::multiplicity::hierarchy_bound(galaxy, record, composition) -> HierarchyBound` lists, for
every redraw attempt that `SystemStars::generate` can keep:

- each star's initial mass, bit for bit, with its body and attempt, so that the census can read its
  η from its `StarDraws`;
- each star–star pair (the pairs `run_pairs` may send to the engine) with a periastron no larger
  than the drawn orbit's.

The attempts are every one the generator can keep:

- each companion's stability redraws within an attempt;
- the carve redraws of P11.T7, attempts 1–7;
- the single star that `after_last_attempt` keeps.

The bound may list more than the generator keeps, never less. R06.T8.g reads it in this order:

1. the record's composition (`draw_metallicity`);
2. this bound;
3. each pair's `pair_light_bound` (P11.T17) at the bound's periastron;
4. each star's own row of `sky_phase_envelope`.

It reads the generator's existing words only. It opens no stream that `draw_hierarchy` does not, and
it adds no tag and draws no new word; tests pin this. Generated output does not move, so there is no
bump.

Cost: at most 3 µs a record in layers C–E, where `draw_hierarchy_of_composition` cost 28, 103 and
146 µs in the ruling's probe. If 3 µs is infeasible while exact, the task reports with measurements
and options before building. It never trades correctness for speed: the bound never misses a kept
attempt.

Tests:

- (slow) `the_hierarchy_bound_holds_for_generated_systems`: for 10⁵ records of each stellar layer
  (A–E) near the Sun and in the bulge, the generated system's stars and star–star pairs are among
  the bound's. Each mass matches bit for bit, and no drawn periastron is smaller than the bound's.
- The bound adds no tag (`tests/golden/rng/tags.golden` unchanged) and reads no word the draw does
  not read.
- The bound is a pure function: twice the same, in any order.
- A bench, `hierarchy_bound`, per layer near the Sun.

Files: `stellar/multiplicity/bound.rs` (new), `stellar/multiplicity/mod.rs`, `benches/stellar.rs`,
and the slow test. Acceptance: `cargo nextest run -p hyperion-sim stellar::multiplicity`, the slow
test by name, the bench (provisional under shared load). See Risks, "P11.T16's cost, measured".

## Verification

- `just ci` green after every task; `just test-slow` green after T12.
- The brainstorm's tests owned here: multiplicity by mass; the all-stars mass function under both
  functions; most double neutron stars at low eccentricity; the low-mode share with companions kept;
  complementarity for the binary classes across the source horizon; budgets; both event
  constructions in any order; state continuous in time.
- Rates at Milky Way parameters, each the brainstorm's figure: novae 30–50 a year, red novae 0.2–0.5
  a year, about 10⁴ X-ray binaries with about 1,300 black-hole transients, 6–12 × 10⁶ accreting
  white dwarfs, about ten neutron-star merger entries, 2–4% of layer D redrawn for Type Ia, about
  one pooled event in six exploding, from a merger rate five to seven times the Type Ia rate.
- Multiplicity: about a quarter of M dwarfs, nearly half of Sun-like stars and most O and B stars
  have companions; Sun-like periods peak within 0.1 dex of 10⁵ days; 69% of all stars below 0.5 M☉
  under the default against the 20 pc census's 68.8% (76.4% under Kroupa, 67% under unscaled
  Chabrier); bound brown dwarfs at a few per hundred stars.
- Benches: `binary_evolve`, `system_full`, `awd_scan_marks` (per host; the target that makes a
  galaxy-wide scan "seconds" is under 300 ns).
- By eye: a cluster's Hertzsprung–Russell diagram shows a binary sequence and blue stragglers; the
  readout of a known cataclysmic variable reads sensibly.

## Generator version

This plan changes generated output in T1.d, T2.c, T2.d, T6, T7, T8, T10 and T15, each with its own
bump and regenerated goldens. T11 moves output too, as built: a paired star's summary is its pair's
state (round 9c, `bin5b`, whose T2.d, T7 and T11 take one bump in the orchestrator's version 16
batch). T4.g and T4.h move output too, as built: T4.g in P14 Phase J's version 20 batch. T4.h
(the early AGB's core radius and remnant at SSE's τ), 9a0950e's protostar and build-age fixes,
T4.i, T4.j and T4.k (rulings p11-channels, p11-supernova-pins and p11-t4k-faults) took version 21
together, with P14.T47.e and P14.T13.c's Bond albedo, in one bump (Phase J lane, 2026-10-06; Risks,
"The 20 → 21 bump, as built"). Each was committed with its goldens blessed at 20, and the bump
re-blessed them at 21 without moving a value. The batch also staled four fitted tables, which
were refitted at `--since 21` on the same branch and land with it, so that (seed, 21) names one
output:

- plan 06's `stellar_fates_low` and `_mid`, which T4.h left stale by rerun, by a unit in about the
  eighth significant digit (plan 06's Risks), with every golden re-blessed at 21 after them;
- R06's `sky_binary_light_c`, `_d` and `_e`, whose probes run pair evolution, refitted on the new
  fates (R06's Risks, "Generator version 21").

T4.l moves output at version 22, with its own bump, and T4.m rides it (ruling p11-c2-swell). T1.d
moves every star once, because the mean mass per system changes
the system count (plan 02 lists it among its known future bumps). After that no primary moves: IDs,
positions, primary masses, ages, primary draws, death times and kicks are untouched, except that T7
redraws about 10⁻⁴ of grid systems (as built it removes none). It reserves: body indices 0–15 for the stellar level, which is
plan 14's slot `0x00`; the domain tags and the event tags 0x0300–0x0306 listed under Provides, with
the rest of 0x0300–0x03FF free for later binary kinds; 64 draw numbers per redraw attempt; the five
`ClassId` values plan 09's registry holds for this plan (2, 3, 5, 6 and 8); `system.substellar`, so
that brown-dwarf companions can be retuned without touching a star; the shapes of `tables::binary`;
`MergedBinaryFate`'s discriminants. Plan 14 can add planets without moving a star.

## Risks and open points

- **Updated for the 2026-09-21 density rulings.** Chabrier's system function is now the default, and
  the all-stars test's target is the 20 pc census's 69% below 0.5 M☉, not the 75.9% that was
  Kroupa's function itself (T1.c, T1.d, T12, Verification). This plan's companions and plan 15's
  scale are fitted together to that figure, the census's primary band shares (66.5, 12.9, 17.8 and
  2.9%) and a local mean mass of 0.55–0.59 M☉ per system. The census counts 0.32–0.38 stellar
  companions per system, about 0.29 per M primary and 0.6 per FGK primary. Close companions are
  probably incomplete there, so a model above it is a finding to weigh, not an automatic failure.
- **Redraw against veto** (Design note 8). The brainstorm's wording for the non-Ia classes ("the
  cells draw conditional on not being in the class") is read as a conditional draw of the binary's
  marks, with the class's observed share leaving through the share matrix. Plan 09 agrees: it carves
  nothing for these classes and its P09.T35 relies on the redraw for Type Ia. The error is of the
  order of a class's share and is stated in the note.
- **Deaths the engine cannot move** (Design note 16). Holding a massive primary's death at plan 06's
  time ignores the few per cent by which stripping or a merger would shift it. The alternative,
  letting the engine move T, would make plan 08's placement class and plan 09's supernova test
  depend on the binary engine, inside placement. A companion that dies within the supernova interval
  is not a catalogue entry, because the brainstorm's test is on "its primary's death"; it explodes
  in place with its shell and is found only by going there. If that proves visible (a second
  supernova class for companions), it is a change to plan 09's `DeathMarks`.
- **The centre's feature-level index** (T8.e). Scratch arithmetic says 13 bits are tight for the
  centre's accreting white dwarfs, and plan 09 has reported it to the brainstorm's owner. Neither
  way out is this plan's to choose.
- **Engine size.** Hurley, Tout and Pols's algorithm is large. T4 is six days as written and may be
  ten. Its subtasks each leave `just ci` green, so it can be paused.
- **Circular dependency with plan 15**, resolved by scratch values in shapes this plan owns (Design
  note 13) and closed by T15. Until then class marks are right in count and rough in distribution.
- **Multiplicity before this plan.** Plans 02, 06 and 08 each carry a stand-in (Design note 1). T1.d
  replaces all three at once and moves every star, as plan 02 foresaw. Plan 02's stand-in counts
  0.30 companions per M dwarf against 0.33 here, so the shift in N is a few per cent.
- **Brown-dwarf companions** (Design note 15) were owned by no plan; plan 13 names this plan for
  them. Their frequency table is from sources outside the brainstorm's list. Brown-dwarf primaries
  stay single, although a tenth to a fifth of free-floating brown dwarfs are binaries (plan 13's
  risk); `MultiplicityContext` is where that would enter.
- **Sources outside the brainstorm's list** (Mardling and Aarseth 2001; Eggleton 1983; Sana et al.
  2012; Grether and Lineweaver 2006; Metchev and Hillenbrand 2009; Milone et al. 2012; the nova
  ignition-mass fit; the recycling closed form) are cited from memory and must be re-checked when
  coded.
- **Feature-level counts** of recycled objects are plan 09's (P09.T9.e); this plan only draws
  conditionally on them.
- **The fast and slow split** (Design note 12) is this plan's, not the brainstorm's. If plan 12's
  scan proves fast enough without it, the two class values can merge before the first release.
- **Re-validated at `9d8e775` for the vertical slice** (round 7, the `doc` lane). Plans 01–05 and 07
  are built, plan 06 in part (T1, T2, T4–T9, T10.a–b, T11, T27), plans 08, 09 and 15 not at all. The
  plan text now follows the code in these places:
  - `coords` is split into files, so the two vectors go in `coords/frames.rs`.
  - `units::GravitationalParameter` is T3.a's, which also takes P14.T2.a's integer-seconds
    reduction, fixed iteration count and 10⁻¹³ residual to e = 0.999 (ruling 33).
  - `RedrawAttempt`, `MAX_REDRAWS` and `DRAWS_PER_ATTEMPT` move to `stellar::multiplicity`, because
    T2.a builds them before `stellar::binary` exists.
  - Plan 06's attempt block is 64 words (`ATTEMPT_WORDS`), not draws, and the stripped mark is a
    `Mark`.
  - The samplers are `Stream` methods, and `PowerLaw::new` and `PiecewiseLinear::new` return
    `Result`s.
  - Chabrier's scale is `Chabrier::PROVISIONAL_HIGH_MASS_SCALE`; there is no `tables::chabrier`.
  - `mean_present_mass`, `stars_below` and `mean_stars_per_system` are free functions of
    `galaxy::fates`, and `Galaxy` holds no fates, so T1.d goes through P06.T30.a's `fates_for`.
  - Plan 07's band is `galaxy::gas::ccm::Band`, and plan 06's `events` take an `EventSeries`.
  - `tables::MANIFEST` does not exist, so a provisional table says so in its header until P15.T2.
  - New tags go at the end of `domain_tags!`, which `tags.golden` pins in order.
  - T13's `OrbitDto` carries the whole element set and μ, and `HierarchyDto` the stars' masses
    (ruling 33).

  Pending re-validation, because what they read is not built:
  - T1.d (plan 08's seam, P06.T30);
  - T2.b's `ForcedMultiple` (plan 09);
  - T4 (P06.T10.c–e, T18, T19);
  - T6–T8 (plans 09 and 15);
  - T10 (plan 08);
  - the server half of T13 (P06.T33–T34).

- **For the orchestrator to rule: how T4 reaches a helium star.** Consumes asks for "a track from a
  helium-star mass". What exists is `sse::helium::HeliumStar::new(m)`, a crate-private phase
  evaluator in a private module, which counts in Myr from the helium zero-age main sequence and
  returns a `PhasePoint`. Plan 11's stripped companions (T4.c, T4.d) need it as a star with a state
  at any age. The options:
  - (a) P06.T10.c–e's `Track` gains a constructor from a helium-star mass, so plan 11 sees only
    `Track`. This is a change to plan 06's Provides, made while `starA` builds `Track`.
  - (b) `sse` re-exports `HeliumStar` `pub(crate)` and plan 11 wraps it in a segment of its own
    timeline. This duplicates the hand-over logic that `Track` will hold.

  Ruled (ruling 34): option (a), plan 06's `Track` gains a constructor from a helium-star mass when
  P11.T4 first needs it, and `HeliumStar` stays crate-private.

- **The vertical slice** (README, "The vertical slice to the `SYSTEM` display (2026-09-23)"). This
  plan's tasks in it are T3.a (with P14.T2.a), T1.a–b (T1.c beside them), T2.a, T2.b, T3.b, T2.c on
  its provisional seams, and the `OrbitDto`, `HierarchyDto` and `body_index` of T13. Everything else
  waits. Until T4–T11 every pair is two single stars on an orbit, and until T1.d the companions the
  budget counts differ from those drawn by a few per cent.
- **Deviations in P11.T1, as built** (T1.a–c, round 7; T1.d not started, it waits on T4.a).
  - **Names against the code.** The mass functions are `galaxy::imf::{MassFunction, Kroupa,
Chabrier}`, whose masses are bare `f64` M☉; "unscaled Chabrier" is `Chabrier::new(1.0)` and
    the default `Chabrier::provisional()` (scale 0.68; plan 15's table does not exist).
    `Composition` is `stellar::Composition`. `rng::PowerLaw` is a density x^−α, so `q^γ` is
    `PowerLaw::new(−γ, …)`. The quadratures use `galaxy::quad::{gl16, gl32_log}`. Plan 08's
    `ClassTable::stripped_share_used`, plan 06's `KickLawParams` and `orbit::Eccentricity` do not
    exist in this tree, so eccentricities are `f64` in [0, 1) for T2.a to wrap.
  - **Added to Provides.** `units::Days` (an edge unit of the time dimension,
    `consts::SECONDS_PER_DAY`); `LOG_PERIOD_MIN`/`MAX` (log-normals truncated to log₁₀ P of −1 to
    11); `PeriodDistribution::sample_in(stream, lo, hi)`, Design note 1's restricted inverse
    transform; `MultiplicityModel::{companion_mass_ratio_cdf, mean_companion_mass_ratio}`, the law
    marginalised over period that T1.d's `StellarFates` method forwards to. Every sampler draws
    one word. `stripped_share` takes a fourth argument, `interacting_periastron: impl
Fn(SolarMasses, f64, &Composition) -> Metres`, as T1.c's text asks and the sketch omitted.
  - **Count distribution.** The geometric ratio is solved so that the mean _after_ the cap at five
    is CF ÷ MF exactly; the plan's 1 − MF ÷ CF loses up to 4.1% of CF (at 11 M☉). Sun-like
    systems still split 56 : 31 : 9 : 4.
  - **Figures the papers do not support, corrected.** Circularisation at 12 d, Raghavan et al.'s
    "about 12 days" (§5.3.4), not 11.6. The M-dwarf σ(log P) is 1.3 (Duchêne and Kraus, Table 1
    and §3.2.3), not 1.95; the mean 3.85 is their a ≈ 5.3 au. Eccentricities are flat on
    [0, e_max] above P_circ: both papers find the distribution flat and Duchêne and Kraus (§5.1.4)
    "inconsistent with the so-called thermal distribution", so the thermal law above 10³ d is
    dropped. "A close component whose weight rises to 0.7" is Sana et al.'s 0.69 companions of
    q ≥ 0.1 per O star inside 10^3.5 d. The solar-type CF of 62% is Duchêne and Kraus's §3.1.2
    (Raghavan et al.'s split gives 58%), and both counts include brown-dwarf companions.
  - **Massive stars' counts are for q ≥ 0.1.** Duchêne and Kraus's B and O frequencies (1.0 and
    1.3) and Sana et al.'s 0.69 count companions down to q ≈ 0.1 only, while Design note 3 draws
    q down to 0.08 M☉ ÷ m₁. The 11 and 30 M☉ anchors therefore hold the counts extended over the
    model's own mass-ratio law (after ruling 41, 1.40 and 1.81 companions per star, O-star period
    weights 0.44 and 0.56), so that counted above q = 0.1, twins included, the model gives the
    surveys' figures exactly. **Ruled (ruling 37, item 1): stands as built.**
  - **Choices the papers left open, and their rulings.**
    - (1) Between anchors the period distribution is the two anchors' mixture, not interpolated
      parameters, so that bimodal and power-law anchors can mix; (2) a very-low-mass anchor,
      log-normal (3.92, 0.5) from a ≈ 4.5 au, and γ = 4.2 there, both Duchêne and Kraus's Table 1;
      (4) the O-star anchor, Sana et al.'s x^−0.55 on log P 0.15–3.5 plus Öpik's law to 10⁴ au
      (7.76), which counted at q ≥ 0.1 gives 0.30 of O stars a companion inside 10 d (their 30%)
      and 0.43 one over two decades of separation (45 ± 5%). **Ruled (ruling 37, item 2): stand as
      built.** The doc comment now says that a mixture of two unimodal anchors is bimodal between
      them.
    - (3) The A-star anchor. **Ruled (ruling 37, item 3): refitted to measurements.** Its visual
      companions are De Rosa et al.'s VAST log-normal (2014, MNRAS 437, 1216, §6.2: peak 387 au
      projected, σ 0.79 dex), deprojected by +0.13 dex (Duquennoy and Mayor 1991, as Raghavan et al.
      do) and truncated inside a projected 30 au, 0.352 per star; its spectroscopic ones are their
      §6.4 weighted frequency, 0.351 per star (Abt 1965; Carquillat and Prieur 2007; Carrier et al.
      2002), with the shape of Moe and Di Stefano's (2017, ApJS 230, 15) companion frequency per
      decade at 2.7 M☉ (their eqs. 20–23) from log P = 0.2 to the 30 au boundary (log P 4.68).
      The weights are 0.499 and 0.501. It reproduces the survey's 35.1%, 21.9 ± 2.6% (30–800 au;
      model 22.0%) and 33.8 ± 2.6% (30–10⁴ au; model 33.8%). The 1–10 au share becomes 0.19 per
      star at the anchor's companion frequency of 1 (0.14 in the survey's own count, whose total
      is 0.70), against the old 0.06. The 10–15% is sourced: it is Duchêne and Kraus's §5.1.2
      ("the frequency of companions in the 1–10 AU range (10–15%) does not vary significantly
      with stellar mass for M ≤ 1.5 M☉", and among intermediate-mass stars "in reasonable
      agreement"), a figure for primaries up to 1.5 M☉ that the model does not use as a target.
    - (5) and (6), the mass-ratio law and its twins. **Ruled (ruling 37, item 5; ruling 41,
      amending item 4): one source for the law and its excess.** From 0.8 M☉ up, the lower edge
      of Moe and Di Stefano's solar-type interval, the model takes their whole mass-ratio set,
      re-checked against the paper: the broken power law, γ_smallq on q = 0.1–0.3 (eqs. 13–15)
      and γ_largeq on 0.3–1 (eqs. 9–11), each by period and interpolated linearly in M₁ across
      1.2–3.5 and 3.5–6 M☉, and their excess twin fraction on q = 0.95–1 (eqs. 5–7: 0.30 −
      0.15 log₁₀ M₁ below log P = 1, falling linearly to zero at log P = 8 − M₁, 1.5 above
      6.5 M☉), counted among companions of q > 0.3 and weighed into the mixture as
      `F S ÷ (1 − F + F S)`. Below 0.8 M☉ Duchêne and Kraus's single slope stands (4.2, 0.4, 0.3
      at 0.09, 0.25, 1 M☉) with no excess. The close/wide split at log P = 3.5 now shapes only the
      O stars' period anchor. Raghavan et al.'s like-mass check is back at 2σ and passes: 11.4%
      of Sun-like pairs are like-mass, against 10.9 ± 2.1%; 1.8% of binaries under 10⁴ d are
      q ≥ 0.98 twins inside 43 d (Duchêne and Kraus §5.3: 2–3%). **For the orchestrator to
      rule:** their laws are measured down to q = 0.1, and below it the model continues with
      `max(γ_smallq, 0)`, flat where their small-q slope is negative. Extending the negative
      slopes themselves, as ruling 37 item 1 reads for Duchêne and Kraus's gentler ones, would
      put 77% of an O star's wide companions under q = 0.1 and, with the surveyed counts above
      0.1 held, 3.5 companions per O star, most multiples at the cap of five; Duchêne and Kraus
      (§5.1.3, §5.4) find a deficit of extreme mass ratios, not an excess.
    - (7) The eccentricity envelope. **Ruled (ruling 37, item 6): Moe and Di Stefano's, as they
      give it.** Their eq. 3, re-checked: `e_max = 1 − (P ÷ 2 d)^(−2/3)` for P > 2 d, which keeps
      the Roche-lobe fill factors under about 70% at periastron; P₀ = 2 d
      (`ECCENTRICITY_ENVELOPE_PERIOD`, public). Orbits under 12 d stay circular. At 12 d the
      envelope already allows 0.70, and e > 0.6 is open from 7.9 d (so from 12 d), where the
      lane's scaling had barred it below 47 d. `stripped_share`'s eccentricity integral follows:
      the periastron floor of an orbit above 12 d is now the separation of a 2-day orbit.
  - **Measured for T1.d** (after rulings 37 and 41). All stars below 0.5 M☉: 0.7702 under Kroupa,
    0.6810 under Chabrier as published, 0.7177 under the default (census 0.69). **A finding for
    the orchestrator:** the first two now fall just outside the plan's T1.c brackets,
    0.764 ± 0.005 and 0.67 ± 0.01, which are the brainstorm's figures for plan 02's provisional
    companions; Moe and Di Stefano's solar-type law puts more companions at low q. The two still
    bracket the census, which is what the figures are for, and the test asserts that and prints
    the values. Stars per system 1.398, 1.450 and 1.427, inside T1.d's 1.33–1.45. Companions'
    initial mass per system 0.207, 0.296 and 0.238 M☉, against plan 02's stand-in's 0.250, 0.368
    and 0.288, so T1.d's mean present mass will fall by up to 0.05 M☉ at the default.
    `stripped_share` under the 10 au stand-in is 0.28 at 8 M☉, 0.36 at 20, 0.40 at 30 and 0.37
    at 150, against plan 06's provisional 0.25.
- **Deviations in P11.T3.a, as built** (round 7, with plan 14's P14.T2 in the same module). Plan
  01's code differs from the sketch in three places, and the code was followed. `UniverseTime` is
  `i64` seconds plus `u32` nanoseconds, and the phase reduction uses both. `units` had no
  gravitational parameter, so `units::GravitationalParameter` (m³ s⁻²) was added here, with
  `from_solar_masses`, `from_jupiter_masses`, `from_earth_masses` and `from_kilograms`.
  `coords` is split, so `SystemVector` and `SystemVelocity` are in `coords/frames.rs`, with
  `SystemPosition::translated` and `displacement_to` beside them, which P11.T3.b's barycentre
  placement needs. The module is `orbit/{mod, kepler, orientation, phase, peters, roche}.rs`, plus
  plan 14's `open.rs` and `state.rs`. Its changes to the sketch:
  - The three angles are one type, `Orientation::new(i, Ω, ω)`. It returns `Result`, requires i in
    [0, π], reduces Ω and ω into [0, 2π), and precomputes the perifocal basis.
  - `KeplerElements` has no all-fields constructor. It is built by
    `from_period(P, μ, e, orientation, M₀)`, the form for T2's companions, which are drawn by
    period, or by plan 14's `from_semi_major_axis(a, μ, e, orientation, M₀)`. Both return
    `Result<_, BuildOrbitError>` and reduce M₀ into [0, 2π).
  - `KeplerElements` stores μ as well as a and P, because ruling 33 puts μ on the wire. Each
    constructor derives one of a and P from the other.
  - `mean_anomaly_at(t)` is public, since plan 14's D11 reads a planet's phase at a death time.
  - `solve_kepler` returns E in [−π, π] for any M. It uses the cubic starter of Mikkola (1987,
    Celestial Mechanics 40, page 329), then exactly `KEPLER_HALLEY_ITERATIONS` = 2 Halley
    iterations, never stopping on a tolerance. It evaluates f as (1 − e)E + e(E − sin E), with
    E − sin E from the Stumpff series, so that E keeps its relative precision near periapsis at
    high e.
  - The measured worst residual is 8.9 × 10⁻¹⁶ rad for e ≤ 0.999 and 1.3 × 10⁻¹⁵ rad up to
    0.999 999, over 10⁵ grid values and 2 × 10⁷ random ones, and E is within two units in its
    last place of the converged root. The test asks for P14.T2.a's 10⁻¹³ up to 0.999, which
    covers this task's 10⁻¹² up to 0.99.
  - The mean anomaly is reduced exactly modulo the period from the clock's integer seconds, by the
    truncated remainder `math::fmod`, a wrapper of the pinned `libm` added to plan 01's `math`.
    The fraction of a period is centred in [−½, ½), so a phase keeps its relative precision on
    both sides of a whole period, and an anomaly already in [−π, π] is never lifted through 2π.
    The first build did lift it, and a near-parabolic orbit a century before periapsis came out
    7.7 km off.
  - `peters_merger_time(m₁, m₂, a, e) -> Years` computes Peters's eq. 5.14 as Tc × F(e). F comes
    from fixed Gauss–Legendre panels in t = e ÷ √(1 − e²). It agrees with a direct Runge–Kutta
    integration of da/dt and de/dt to 10⁻⁹ (the test's bound; about 10⁻¹⁴ measured), and it is
    within 3% of Mandel's (2021) fit for e up to 0.999 99. It approaches Peters's asymptote
    (768/425)(1 − e²)^(7/2) only slowly: the gap is about 2.06 √(1 − e).
  - `peters_merger_time`, `peters_separation_for` and `roche_lobe_radius` panic, with the panic
    documented, on masses, axes, times or mass ratios that are not finite and positive. A caller
    that passes one has a bug.
  - Eggleton's formula agrees with Paczyński's (1971) to within 3% for q from 0.05 to 0.8. Its
    use at periapsis (Design note 7) is the usual approximation for an eccentric binary, and the
    doc comment says it is an extrapolation.
  - The brainstorm's 80–100 s for two white dwarfs a thousand years before merging holds for
    pairs of 0.6–0.9 M☉ each, whose totals are near or above the Chandrasekhar mass: 83 s at
    0.8 + 0.6 and 98 s at 0.9 + 0.9. A 0.6 + 0.6 M☉ pair gives 76 s.
  - `orbit/functions.golden` pins by bits Peters's time over all its panel counts, its inverse
    and the Roche lobe, beside plan 14's fixture `orbit/states.golden`. Both are new at 11.
- **Deviations in P11.T2.a–b and T3.b, as built** (round 7, the `hier` lane; T2.c and T2.d not
  started). The code is `stellar/multiplicity/{hierarchy,stability,positions}.rs`, with
  `PeriodDistribution::quantile_in` and `share_in` added to `dist.rs` (`sample_in` now calls
  `quantile_in`, bit for bit), four tags in `rng/tags.rs`, and the new golden
  `stellar/hierarchies` (six IDs, blessed at 11). Nothing generated calls `draw_hierarchy`, so no
  existing golden moves.
  - **Where a companion goes.** Companions are added one at a time, each to a node of the
    hierarchy's outer spine: the whole system (a new outermost orbit) or the outer member at any
    level down to its last star (a new orbit inside it). Strictly inside out, as Design note 4
    has it, no outer member could hold a subsystem, and Tokovinin (2014, AJ 147, 87) finds those
    "almost as frequent as in the primary components", with 2 + 2 quadruples at 4%. Only the new
    orbit is ever redrawn, which is what "the outer orbit only" protects, but it may be the inner
    orbit of an existing pair. Joining only the spine puts each new star last in depth-first
    order, so companion k is body k when its orbit is drawn and Design note 5's keys need no
    renumbering. Every shape is reachable in some order of drawing.
  - **The node is drawn with the period, on `binary.orbit`, not on `system.hierarchy`.** A node
    picked once per companion and kept through its redraws dropped 2.2% of companions: a node with
    no real room fails all seventeen tries. Instead each try's first `binary.orbit` word picks the
    node, by integer thresholds on the weights of the nodes' period windows, and the period, by
    inverse transform of the mark's residual inside the node's window. The windows come from the
    criterion's necessary conditions (its smallest axis ratio 2.8 × 0.7 = 1.96, the enclosing
    orbit's known eccentricity, the tidal cut). That is rejection sampling of (node, orbit), so
    it is exact: a companion joins a node in proportion to the chance that an orbit drawn for it
    passes the test. `system.hierarchy` is not opened and is not registered, so four tags were
    added and `tags.golden` gained four lines, not five; the name stays reserved in the heading.
    **For the orchestrator to rule.**
  - **The laws a companion is drawn from.** Its period and mass-ratio laws are the model's at the
    mass of the first star of the node it joins (the system's primary for the whole system), and
    its mass is that star's times q. Moe and Di Stefano (2017, §2 and §5), whose law it is, define
    a tertiary's q "with respect to the … primary" and not "M_B ÷ (M_Aa + M_Ab)"; Tokovinin (2014,
    §4.3) draws inner and outer periods "from the same log-normal distribution". The plan's "mass
    ratio against the mass of the node inside" would let a tertiary outweigh the primary. No star
    outweighs the primary, by construction.
  - **Semi-major axes follow the final masses.** A pair's period, eccentricity, orientation and
    phase are drawn when it forms; its `KeplerElements` are built `from_period` with the pair's
    μ, the total initial mass of its members as the hierarchy finally stands, so a later companion
    that joins a member widens the pair's orbit at the same period. Every test is therefore run on
    the whole hierarchy after each try.
  - **Mardling and Aarseth, re-checked** against the paper (2001, MNRAS 321, 398, §4.1 eq. 90 and
    §4.2): `R_p,out ÷ a_in > 2.8 [(1 + q_out)(1 + e_out) ÷ (1 − e_out)^½]^⅖`, `q_out = m₃ ÷ (m₁ +
m₂)`, C = 2.8 "determined empirically", "holds for q_out ≤ 5"; their reduction factor `f = 1 −
0.3 i ÷ 180°` on the right for inclined and retrograde orbits; a 3 + 1 tests its inner
    triple's outer orbit as the inner binary; a 2 + 2 takes the member of larger semi-major axis
    as the binary and the other as the third body, with `f₁ = 1 + 0.1 min(a ÷ a₂, a₂ ÷ a)`. It is
    applied beyond q_out = 5 too (a light pair about a heavy star), where it grows as q^⅖ against
    the Hill radius's q^⅓, so it errs towards stability.
  - **The tidal cut** is plan 02's radius at the record's epoch position and at the sum of the
    stars' initial masses, the mass the orbits are bound to, not the primary's alone as the frame
    rule reads it; half of it is at most 0.91 of the frame rule's radius, so every star lies in
    its system's frame.
  - **`ForcedMultiple { max_separation }` bounds every semi-major axis**, since a hard–soft
    boundary is a binding energy. A forced multiple whose every companion is dropped comes out
    single (none of 5,000 measured with 1,000 au); P11.T8.f may redraw it at its next attempt.
    **For the orchestrator to rule** whether it should.
  - **Design note 1 is built into `draw_hierarchy`**, so that P11.T2.c calls it unchanged:
    `draw_hierarchy(galaxy, record, MultiplicityContext::Free, RedrawAttempt::FIRST)`. For a
    primary of 8 M☉ or more (`STRIPPED_MARK_MIN_MASS`; since ruling 93.3,
    `stripped_mark_min_mass(&Composition)`, m_cc(Z) − 1 M☉) the mark `StarDraws::for_star(..)
.stripped()`, attempt 0 until plan 08's `mark_attempt`, is read against
    `PROVISIONAL_STRIPPED_SHARE` = 0.25. A set mark makes the system multiple with the primary's
    own orbit's periastron under `PROVISIONAL_INTERACTING_PERIASTRON` = 10 au; an unset one makes
    it multiple with probability (MF − s) ÷ (1 − s) and that periastron at least 10 au. The seams
    are therefore in `stellar::multiplicity`, not `stellar/system.rs` as T2.c says. **For the
    orchestrator to rule.** The primary's initial mass is the record's `primary_initial_mass()`
    (placement's `system.primary_mass`); plan 06's `StarDraws` hold no mass.
  - **Smaller choices.** An orbit with e ≥ 0.9999 is redrawn, since ruling 39 carries such
    orbits as open ones. A dropped companion drops every later one, so body indices have no gap.
    Mark picks on `system.multiplicity`: word 64n decides multiple, word 64n + 1 the count from
    the model's PMF given a multiple, which `ForcedMultiple` reads alone. The node-and-period
    mark's residual counts a window wider than 2⁵² marks in pairs, as `Stream::uniform_open`
    counts 52 bits, so that the share inside the window stays strictly below 1.
  - **Measured.** Companions dropped after sixteen redraws: 0 of 4,239 over the mass function at
    the Sun-like point, 0 of 4,266 in the inner disc at 6,000 ly, 10 of 10,305 (0.10%) for
    primaries log-uniform on 0.08–150 M☉, 9 of 6,333 (0.14%) of them above 8 M☉, against the
    plan's 1%; under `ForcedMultiple` inside 1,000 au, 13 of 6,526. Multiple share and companions
    asked for match the model within 3.29σ at 0.3, 1 and 3 M☉ (0.4405 against 0.44 at 1 M☉).
    Stripped marks: 982 of 4,000 massive primaries (0.2455). The barycentre of 10⁴ hierarchies at
    ±H and the epoch: worst 0.95 m, measured with exact products, with stars up to 2.0 × 10¹⁶ m
    out. The spacing of an `f64` there is 4 m, so the plan's 1 m is met because the heavy stars
    sit near the origin, not guaranteed. The test also asserts the bound that holds at any
    separation, one `f64` epsilon (2.2 × 10⁻¹⁶) of the farthest star's distance. **For the
    orchestrator:** whether the plan's figure should read so. **A finding against Tokovinin (2014, Table 3):** Sun-like triples put their inner
    pair about the primary 1,316 times and in the outer member 1,379 times (his corrected
    simulation 282 : 152), and 2 + 2 are 41% of quadruples (his 74%). He reproduces those only by
    correlating the subsystems of the two components (his ε₊ and ε₋), which independent draws do
    not do.
- **Deviations in T13's slice, as built (round 7, `wire`).** In a new private module `orbit.rs` of
  `hyperion-protocol`, its types re-exported at the crate root, built with P06.T33.
  - `OrbitDto` is `period_s`, `semi_major_axis_m`, `eccentricity`, `inclination_rad`,
    `ascending_node_rad`, `argument_of_periapsis_rad`, `mean_anomaly_at_epoch_rad` and `mu_m3_s2`,
    named after `KeplerElements`'s accessors and matching the client's `KeplerOrbit` (P14.T39). Its
    eccentricity is documented below 0.9999, since ruling 39 carries bound orbits from there up in
    open form; the client's solver stops at the same bound.
  - `HierarchyDto { nodes }` lists `HierarchyNodeDto`s depth first, as `SystemHierarchy::nodes`
    does, tagged by `type`: `star { body_index, mass_msun }` and `pair { inner, outer, orbit }`,
    the children as indices into the list. `mass_msun` is the mass the server places the star by,
    its initial mass, as `star_positions_at` uses. A client sums a member's stars, where the server
    reads `node_mass`, which can differ in the last bit; that is drawing only (plan 14, D18).
  - A system not yet formed has no nodes, as it has no stars.
  - `body_index` and `SystemSummaryDto.hierarchy` are required fields, since they land with the
    `system_summary` kind itself; `binary_class` and `star_count` wait for the rest of T13.
- **Two constructors for plan 14's synthetic hosts (`context`, round 7, P14.T1.d).**
  `stellar/multiplicity/hierarchy.rs` gains the crate-private `SystemHierarchy::single` (an ID, a
  mass and a slot kind) and `SystemHierarchy::binary` (an ID, two masses, the companion's kind, a
  and e), next to the test-only `hand_built`, so that `planetary::context`'s builder can make a
  star or a binary under a chosen ID. Nodes, stars and node masses are laid out as the draw lays
  out a single star and a binary, and the orbit lies in the reference plane at periapsis at the
  epoch. A pair with no third body passes Mardling and Aarseth whatever its orbit. The builder
  checks the draw's other conditions before calling `binary`: the companion no heavier than the
  primary, a period of 0.1–10¹¹ days, an eccentricity inside the envelope at that period (circular
  below 12 days) and under 0.9999, and the apocentre inside `TIDAL_CUT_SHARE` of its sphere of
  influence. So `SystemHierarchy` stays stable by construction outside tests. It departs from the
  draw in one place: a companion under `MIN_COMPANION_MASS` is a `SlotKind::BrownDwarf` slot,
  which the draw makes only from P11.T2.d. Nothing drawn changes.
- **Deviations in P11.T2.c, as built** (round 7, the `srvstars` lane, with P06.T34). The code is
  `stellar/system.rs`; `multiplicity/{mod,hierarchy}.rs` gain only doc lines and a crate-private
  `SystemHierarchy::heap_bytes`, and `sse/track.rs` a crate-private `Track::heap_bytes`.
  - _Built._ `SystemStars::{generate, generate_in, hierarchy, star_count, heap_bytes}`. `generate`
    is `generate_in(.., MultiplicityContext::Free)`. The hierarchy is
    `draw_hierarchy(galaxy, record, ctx, GRID_ATTEMPT)`, and each companion is
    `StarModel::new(slot.initial_mass(), composition, StarDraws::for_attempt(seed, slot.body(), 0),
record.age_at_epoch())`. The primary is built as plan 06 built it, through a named private seam
    `primary_draws` (`for_star`, attempt 0, until P08.T12.c's `mark_attempt`). `GRID_ATTEMPT`
    (`RedrawAttempt::FIRST`) is the second named seam, for T6–T8's redraws. The stripped share and
    the interacting range stay where T2.a–b put them, in `stellar::multiplicity` (ruling 51.2).
  - _Summaries._ `StarSummary` gains `body()`. `SystemSummary` gains `hierarchy()`, an
    `Option<&SystemHierarchy>` that is `None` before birth. The Provides' `HierarchySummary` is
    therefore the drawn hierarchy until T4 gives a pair a state at each time. **For the
    orchestrator to rule** whether T4 adds a type of its own or evolves this one. `StellarBrief`
    gains `star_count()`, and the rest of the brief stays the primary's. `binary_class` waits for
    T5, and `binary_state_at`, `system_mass_at` and `recoil` wait for T4.
  - **The version stays 11** (the orchestrator's brief), although the task asks for a bump. Nothing
    generated that anything reads changes: range rows carry no brief, and `system_summary` is first
    answered in the same change. Only `stellar/summaries` moved. It is rewritten so that the
    primaries come first exactly as before, followed by a new part for the companions of the 6 of
    its 12 systems that are multiple. `golden_diff.py`: "Extended only (1): new values pinned, every
    existing value unchanged … 399 new".
  - _Tests._ `companions_move_no_primary` covers the 12 pinned IDs. Each primary equals plan 06's
    `StarModel` built from the record alone, and its summaries match `ForcedSingle`'s bit for bit.
    `every_companion_is_its_slots_star_with_the_systems_composition_and_age` checks each companion.
    The property test of life and death now holds every star, companions included. The multiple
    share is checked against Σ `multiple_fraction` of the sampled primaries within 3.29σ, on 400
    systems (fast) and 10⁴ (slow), sampled five layers in turn rather than by the mass function.
    Measured: 5,118 multiple of 10,000 against 5,102.0 (z = 0.33), and 203 of 400 against 204.5.
    Over the 10⁴, systems of one to six stars number 4,882 : 2,569 : 1,206 : 647 : 328 : 368. The
    model's PMF sums are 4,898 : 2,629 : 1,180 : 587 : 311 : 395, and the difference is the dropped
    companions. Layer E (8–150 M☉) has 8% sextuples (227 of 2,805). That follows from the capped geometric
    count with a mean near 2.2 for O stars, a property of the model, not of this wiring.
    _Superseded (noted round 9b, `ui13`):_ since rulings 74 and 81 a primary has at most three
    companions (`MAX_COMPANIONS = 3`), so no system has more than four stars. The counts above
    predate that cap. `stellar_system.rs`'s `star_counts_run_from_one_to_four_even_in_layer_e`
    pins the range, and `StellarBriefDto.star_count`'s doc (1 to 4) cites the cap.
  - _A pinned triple_ (superseded: since rulings 74 and 81 `…0009` draws as a binary, and the
    server's test pins `0x4200_2cb2_0000_000d`, three main-sequence dwarfs; the paragraph below
    is the answer as it was), `42002cb200000009` of seed `0x4d2` at the epoch, answered as follows. An
    F3 IV subgiant of 1.681 M☉ (11.3 L☉, 6,743 K) and a K7.5 V star of 0.628 M☉ orbit each other in
    3.96 d (a = 0.0648 au, e = 0, circularised). An F8.5 V star of 1.149 M☉ orbits that pair in
    274.6 d (a = 1.250 au, e = 0.525). The system is 1.516 Gyr old with [Fe/H] −0.014, and its
    cached `SystemStars` is charged 16.2 KB. Until T4 the inner pair is two single stars on an
    orbit (ruling 33).
- **Validated as built (round 8, `val11`: T1.a–c, T2.a–c, T3.a–b, T13's slice, P06.T33–T34).** An
  independent sample of 140,000 hierarchies (seven mass bins at the Sun-like point) and independent
  solutions for the orbits. Nothing generated moved; four tests were added where a constant could
  change unnoticed, and the server's summary test now holds the wire to the sim bit for bit.
  - _Stability._ No pair of 162,000 fails Mardling and Aarseth's eq. 90 written out afresh, and no
    apocentre leaves the half tidal radius; the smallest margin is 1.000 005. The criterion is
    applied beyond its stated q_out ≤ 5 for 0.4% of Sun-like pairs and 10% of O-star pairs.
  - _Orbits._ Against 70-digit decimal solutions, bound states agree to 1.1 × 10⁻¹⁴ (e = 0.9998,
    10⁹ periods out) and open ones to 6 × 10⁻¹⁶ (e from 1 − 10⁻⁷ through the parabola to 3); the
    mean anomaly 10⁹ periods out is within one unit in the last place of the exact rational.
    Eggleton's lobe is within 0.81% of lobes integrated from the equipotential (worst at q = 0.05).
    `tests/orbit_reference.rs` and the Roche test pin these.
  - **For the orchestrator to rule (each moves output):** (1) the massive stars' fractions are
    Duchêne and Kraus's lower limits; counted as Moe and Di Stefano (2017, Table 13) count, their
    single fraction is 0.53 at 9–16 M☉ and 0.44 above 16 against 0.16 and 0.06, and their
    companion frequency 0.59 and 0.70 against 1.6 and 2.1 (Offner et al. 2023, Table 1: MF 93% and
    96%). (2) Sextuples are 8–9% of systems above 8 M☉ because the count is geometric capped at
    five; Moe and Di Stefano's own model stops at quadruples. (3) Tokovinin's (2014) correlated
    subsystems: Sun-like triples split 1,005 : 943 against 282 : 152, and 42% of quadruples are
    2 + 2 against 74%. (4) Eccentricities are flat on [0, e_max] (mean e ÷ e_max 0.46–0.50) against
    Moe and Di Stefano's e^η with η ≈ 0.4 for solar-type and 0.8 for early-type pairs (their eqs.
    17–18). (5) The provisional stripped share, 0.25, removes a third of O stars' close
    companions (0.22 per star inside 10^3.7 d against 0.33 without the mark), until T1.d.
- **Ruling 74 as built (round 8, `mult3`; moves output, version still 11, for the batch of 12).**
  Every decision draws the words it drew before: the weights and laws change, the draws do not.
  - _Massive anchors._ `FRACTION_ANCHORS` keeps Duchêne and Kraus below 2 M☉ and takes Moe and Di
    Stefano's (2017) Table 13 at 3.5, 7, 12 and 28 M☉ (their §9.1 masses): f_mult;q>0.1 0.84, 1.3,
    1.6, 2.1 and F_n=0 0.41, 0.24, 0.16, 0.06. Each is extended over the model's own laws by the
    share s they would count (q > 0.1, log P < 8: 0.785, 0.698, 0.703, 0.718), MF and CF solved so
    that, thinned by s, the counts are Table 13's: MF 0.688, 0.874, 0.919, 0.961 and CF 1.070,
    1.861, 2.275, 2.884. At 28 M☉ the cap binds: every multiple has three companions, the single
    fraction is met and the counted frequency is 2.07. The 2.7, 11 and 30 M☉ anchors and the B
    star's surveyed share are gone; the O-star period anchor keeps its shape.
  - _Cap._ `MAX_COMPANIONS` is 3, so the count PMF has four entries and a hierarchy at most four
    stars and seven nodes. Sun-like systems split 56 : 30.3 : 9.4 : 4.3 (Raghavan 56 : 33 : 8 : 3,
    all within 2σ; Tokovinin's 4.3% quadruples).
  - _Tokovinin's correlation._ A new orbit inside a secondary component weighs 0.275 when the
    primary component is a star and 20 when it is a pair. The first gives Sun-like triples 1.86 :
    1 (1,294 : 696). **For the orchestrator to rule:** the 2 + 2 share cannot reach 74%, because
    companions join the outer spine only, so a 2 + 2 forms only from an L11 triple, 65% of them.
    The second weight takes the share to that reach (65.1%; 1.2 gives 59%, 1,000 gives 66%).
    Reaching 74% needs a third, count-aware weight on the second companion.
  - _Eccentricities._ `p(e) ∝ e^η` under the envelope, η from eqs. 17–18, interpolated linearly
    across 3–7 M☉, held beyond log P = 6 (late) and 5 (early) and at 12 d below it, eq. 17 below
    0.8 M☉. `eccentricity_distribution(m1, period)` takes the primary mass: a change to the
    Provides sketch. `stripped_share`'s eccentricity integral follows.
  - _Measured (`val11`'s sampler, 20,000 per bin)._ Mardling and Aarseth hold for every pair, and
    no apocentre leaves the cut. Dropped companions are at most 0.19% (O stars). No quintuples or
    sextuples. **A finding for the orchestrator:** counted as Moe and Di Stefano count, directly
    about the primary, the massive stars' frequencies are 0.68, 0.81, 0.85 and 0.98 against 0.84,
    1.3, 1.6 and 2.1. The fit assumes every companion orbits the primary directly, but the draw
    puts about half of a massive star's companions in subsystems of its companions (L12 in 75% of
    O systems), where the stability windows leave the most room. Closing it needs the placement,
    not the anchors. Mean e ÷ e_max is 0.55–0.60, against (1 + η) ÷ (2 + η), because stability
    rejects eccentric outer orbits.
  - _Consequences._ Chabrier's function as published gives 1.463 stars per system, just above
    T1.d's 1.33–1.45; the default gives 1.436 and Kroupa 1.406. At plan 15's fitted Chabrier
    scale 0.92 (ruling 138) the default's spine construction gives 1.457, also just above the
    bracket; `quadrature.rs` prints it and asserts only Kroupa's, and T1.d's bracket holds on the
    drawn companions, 1.437 (ruling 140.8). The barycentre test's 1 m is
    exceeded at the positions' resolution (1.48 m with a star 1.5 × 10¹⁶ m out). The P11.T13
    triple is now `0x4200_2cb2_0000_000d`, because `…0009` draws as a binary.
- **Ruling 79's placement weight was built and taken out again (ruling 81.1).** A factor
  `(1.5 M☉ ÷ m₀)^k` on subsystem weights moved O stars' counted frequency only from 0.98 to 1.03
  at k = 0.5, and to 1.18 with subsystems barred, while dropping 16% of companions: the spine
  construction left too few companions near log P = 3 and piled them up at 7.
- **Ruling 81 as built (round 8, `mult3`; moves output, version still 11, for the batch of 12).**
  From 3 M☉ up, and for a share of 1.5–3 M☉ primaries rising linearly in ln M₁ (one mark, word
  64n + 2 of `system.multiplicity`), a system's direct companions are drawn as Moe and Di Stefano
  count them (`stellar/multiplicity/direct.rs`).
  - _Count._ n = 0–3 from Table 13's F_n=0 and f_mult at 1, 3.5, 7, 12 and 28 M☉, interpolated in
    ln M₁: F_n=0 for none, and a capped geometric of mean f_mult ÷ (1 − F_n=0) for a multiple.
    The unset stripped mark still gives the multiple decision (MF − s) ÷ (1 − s).
  - _Each companion._ Its period is drawn from their `f_logP;q>0.1` (eqs. 20–23, converted from
    q > 0.3 by their mass-ratio law) on log P = 0.2–8, times a fitted correction, and its mass
    ratio from their laws on q = 0.1–1 (eqs. 5–7, 9–15), with eccentricity e^η. The flat
    extension below q = 0.1 does not apply to direct companions.
  - _Rejection (as amended)._ Slot by slot, the newest companion is inserted by period and the
    whole hierarchy must pass the whole test. A failure redraws that companion alone, up to 42
    tries: 21 on its slot's key and 21 on slot + 8, since an attempt's block holds 21 tries of
    three words. Tries rejected: 29%, 50%, 59% and 64% at 3.5, 7, 12 and 28 M☉. Direct
    companions dropped: 0.04%, 0.20%, 0.31% and 0.49%. Seventeen tries dropped 1.8% at 28 M☉.
  - _Blend._ Ruling 81.2's "M₁ ≥ 2 M☉" is read through 81.5's blend: at 2 M☉ 41% of systems use
    the direct construction.
  - _Correction._ The fit targets the bin shares of Moe and Di Stefano's own eqs. 20–23 law,
    normalised, not the research's per-decade figures directly; the absolute frequencies follow
    from the count law. `PERIOD_CORRECTION`, 4 masses × 8 decade bins, is solved by the ignored test
    `fit_the_direct_period_correction` (c ← c × target ÷ measured, twelve iterations, every bin
    within 0.2%). It absorbs the provisional stripped mark's set branch above 8 M☉, and is to be
    refitted when P11.T1.d replaces the seam.
  - _Subsystems._ Each direct companion is offered one at its own mass's rate (Table 13 from
    2 M☉, Duchêne and Kraus below) times Tokovinin's ε₋ = 0.5 (innermost) or ε₊ = 1.2. One draw
    from its own laws, kept only if the whole hierarchy passes: Tokovinin's dynamical truncation,
    not a dropped companion. Subsystems never count, and they are only offered while the system
    holds fewer than four stars. This rests on solar-type evidence; the subsystem rate of O and B
    stars is unconstrained.
  - _The cap (ruling 81 as amended): four stars in total, a game-side departure._ An O system
    with three direct companions (about 38%) holds no subsystem, one with two may take one.
    Ruling 81 says one with one may take two; this build offers each direct companion at most one
    subsystem, so it takes at most one. Real sextuples exist (ν Sco, AR Cas; Offner et al. 2023
    §2.1), and the cap is revisited if subsystem statistics are ever measured.
  - _Draw order and numbering (Design note 5)._ Draws are keyed by draw slot (direct 1–3, their
    overflow tries 9–11, subsystems 4–6), and body indices are given after sorting, depth first.
    Slot keys are distinct by construction and never body 0. `system.multiplicity` reads word
    64n + 2 for the blend and words 64n + 4 to 64n + 6 for the subsystem decisions of slots 1–3;
    word 64n + 3 is unused, and `system.hierarchy` is still not read. A subsystem's one try uses
    words 0–2 of slot 3 + k's block. A companion's own star draws (`StarDraws::for_attempt`) are
    keyed by its body index after sorting, not its draw slot: deterministic, but two companions
    can swap star draws if one's period changes.
    So above the blend "companion k is body k when its orbit is drawn" no longer holds, and
    `SystemHierarchy::pair_key` is a distinct body of each pair, not its stream key.
  - _Measured (ruling 81.8; `direct_companions_meet_table_13_counted_as_moe_and_di_stefano_count`,
    10⁴ systems at each of 3.5, 7, 12 and 28 M☉)._ F0 / F1 / F≥2 / f_mult: 0.412/0.403/0.184/0.830,
    0.243/0.399/0.358/1.284, 0.161/0.366/0.472/1.577, 0.059/0.272/0.668/2.079. Per decade at log P
    = 1, 3, 5, 7: 0.080/0.113/0.128/0.101, 0.132/0.198/0.199/0.121, 0.190/0.243/0.230/0.136,
    0.293/0.321/0.292/0.171. Close frequency 0.336, 0.579, 0.754 and 1.039. All are within 2σ of
    Table 13 and most within 1σ. Direct companions dropped: 0.07%, 0.22%, 0.32%, 0.46%. **Compact
    triples are 25% of O stars, against the ruling's 10–20% check**, and 3–13% below: a finding.
    Sun-like targets are unchanged, and the 1 M☉ cross-check passes against Table 13, but F1 is 2.1σ
    above §9.4's 0.27 ± 0.03 (0.33).
  - _After the merge onto `abf2a54`._ Plan 14's supernova-overlap test samples 360 massive
    systems rather than 120, since only 18 of 120 are now single and 120 held 33 surviving pairs
    about exploded hosts (it asks for more than 60). No orbits crossed. P14.T32's
    `fallback_black_hole` is re-pinned to `0x8201_b2e0_0000_0010`, and the server's unbound-body
    system to `0x81fa_b2e0_0000_0002`.
  - **For the orchestrator to rule:** an unset stripped mark no longer holds a direct
    construction's orbits outside 10 au. Under Table 13 nearly every O star has a close
    companion, which the provisional share of 0.25 contradicts. Holding every companion of three
    quarters of the primaries above 8 M☉ outside 10 au rejected most sets and dropped 18–27% of
    their companions. A set mark still asks for an interacting innermost orbit. P11.T1.d's stripped
    share, computed from this model, closes the gap.
  - _Also._ Ruling 74's extended anchors (`FRACTION_ANCHORS` above 2 M☉) now serve only the
    quadratures (T1.c), the blend's spine share and subsystem rates below 2 M☉. P11.T1.d must
    re-derive the quadratures from the direct construction above 1.5 M☉.
- **P11.T14, as built in part (round 9, `ui9`).** `components/StarList.tsx` is in the `GALAXY`
  readout, under the primary's readings, and is built from `lib/system/starList.ts`'s
  `starListRows`.
  - _Stars._ A `STARS` table lists each star under its letter, `A` then `B`, `C` in the hierarchy's
    depth-first order, which is body-index order. Each row reads `CLASS`, `MASS` in `M☉` (the mass
    now; the em dash where no remnant is left) and `STATE`, the kind in words.
  - _Orbits._ An `ORBITS` table lists each pair's orbit, outermost first, named by the letters it
    joins (`AB–C`, `A–B`). Each reads `PERIOD`, `SMA` and `ECC` through the new `formatOrbit`. That
    is `formatPeriod`, then `formatBodyDistance` with no held unit, then four decimals,
    `ECCENTRICITY_DECIMALS`.
  - _Designator._ The `STAR` column holds the letter alone, since the system's designation stands in
    `DESIG` above the list.
  - **For the owner:** `STARS`, `ORBITS`, `STAR`, `STATE`, `ORBIT`, `PERIOD` and the new
    abbreviation `ECC` join the draft nomenclature (`SMA` was proposed with P14.T43).
  - _Not built:_ the system list's star-count column, which needs `StellarBriefDto.star_count` on
    the range rows (P06.T34's blocked briefs), and the binary class in words, which is not on the
    wire (T5, T13). The chart symbol is unchanged.
  - The round's brief placed the star list "in the `SYSTEM` display". This task puts it in the
    `GALAXY` readout, and the `SYSTEM` display's body list already lists the hosts (P14.T43.a), so
    it was built here.
  - _Built (round 9b, `ui13`, ruling 115.5):_ the system list's `STARS` column, which supersedes the
    first half of "Not built" above. It shows the star count, primary included, as one
    right-aligned digit after `CLASS`, or the em dash where the row has no brief. The row's
    accessible name gains `1 star` / `3 stars`, and `DRIVE RANGE` reads `IN` / `OUT` under its
    header. _Deviation:_ that column is 6.5ch, not the ruling's 4ch, because its header word
    `RANGE` is 57 px against 36 px. At list widths up to 30 rem (the `SYSTEMS` column at 1280 × 720)
    the designation takes its own line across each 2 rem row, and the other five columns share the
    second line, set in 1 rem. At 1280 the designation was otherwise cut to `9G…`; this follows the
    UX reviewer's should-fix, ruled by the orchestrator. The binary class in words still waits for
    the wire, and the by-eye check against a known triple is still to be made.
- **Ruling 93.3, as built (round 9, `kick` follow-up).** The primary mass from which the
  companion-stripped mark is read is `stripped_mark_min_mass(&Composition)`, m_cc(Z) − 1 M☉ at
  the system's drawn metallicity (7.20 M☉ at Z = 0.02, never below 5.72), in place of 8 M☉, so
  that plans 06 and 11 mark the same stars. It compares the initial mass, while the track tests
  its window in the mass its early AGB's `m_c_bagb` reads, about 0.1 M☉ lower. `PERIOD_CORRECTION`
  was fitted with the 8 M☉ floor at rows of 3.5, 7, 12 and 28 M☉; at solar metallicity the 7 M☉
  row stays below the new 7.20 M☉ floor, so the table is not refitted now, and it is refitted
  when P11.T1.d replaces the seam.
- **The period correction's fit, as built (slow-test audit, 2026-09-27; no output moves).** The
  ignored test `fit_the_direct_period_correction`, which fitted the table and asserted nothing, is
  gone from the slow suite. The fit is `hyperion-fit`'s `period_correction` task
  (`tasks/period_correction.rs`, Slow, with `manifests/period_correction.toml` and a smoke
  manifest): the same twelve iterations over the same 20,000 systems a row, drawn in chunks on any
  number of threads with integer bin counts, so it gives the old test's table bit for bit. Its
  sample and measurements are the sim's new public `stellar::multiplicity::period_fit` module,
  which the hierarchy tests' samples now share. The task is not in the registry yet, since
  registering commits its table: rerun at version 13 it reproduces the 3.5, 12 and 28 M☉ rows bit
  for bit but moves the 7 M☉ row (its factors from 0.655, 0.903, 1.125, 1.265, 1.243, 1.092, 0.880
  and 0.655 to 0.648, 0.881, 1.116, 1.228, 1.274, 1.103, 0.906 and 0.663), which moves the
  generator's output. The ruling 93.3 note's reading, that the 7 M☉ row stays below the floor, does
  not hold at the fixture's Sun-like point. The refit, in the version 14 batch, registers
  the task, writes `tables/period_correction.rs` and has `direct.rs` read it.
  - _Its replacement_ is the slow test `period_fit::the_period_correction_gives_its_bin_shares`
    (about 10 s): the committed table's bin shares over the fit's own sample, each within the
    plan's 0.2% of Moe and Di Stefano's. The 3.5, 12 and 28 M☉ rows miss by 0.15%, 0.06% and
    0.11%; the 7 M☉ row misses by 1.51% and is held to an interim 0.2–2% (accepted by the
    coordinator) until the refit in the version 14 batch, which must narrow it to 0.2%. _Done in
    P11.T1.d_ (see its entry below).
- **Two pieces of plan 11 landed with plan 06's range briefs (round 9, `briefs`; ruling 90).**
  - `multiplicity::draw_star_count(galaxy, record, ctx, attempt) -> u8` joins the Provides. It equals
    `draw_hierarchy(..).star_count()` for every record, context and attempt (tested over 4,500
    records). It reads the draw's count words, and returns 1 without the tidal limit, the period
    laws or an orbit for a system drawn single; otherwise it draws in full.
    `Draw::companion_count` and `Draw::is_direct` now delegate to free functions they share with it,
    and `draw_hierarchy` is unchanged. P11.T4's binary engine must keep the two equal.
  - P11.T13's `StellarBriefDto.star_count: u8` landed early, with P06.T34, and required rather than
    optional, since no brief had been sent before.
- **Deviations in P11.T4, as built** (round 9, `bin11`; T4.a–f, T5 not started). The engine is
  `stellar/binary/{mod,params,star,timeline,detached,rlof,common_envelope,supernova,evolve}.rs`
  with its tests in `tests.rs`, and ruling 34.1's helium star is `Track::helium_star`,
  `helium_star_full` and `helium_star_from` in a new `sse/track/binary.rs` over P06.T9's entry
  (`Builder::run_from`), with a `mod` line and crate-private re-exports in `sse/track.rs` and
  `sse/mod.rs`. `HeliumStar` stays crate-private. Nothing in `generate` calls the engine.
  - _The primary's fixed death_ (T4.e, design note 16) applies from `stripped_mark_min_mass`,
    m_cc(Z) − 1 M☉ (ruling 93.3), not 8 M☉. `BinaryInput` does not carry the death age: the
    engine reads it from plan 06's full track with the primary's own draws (`evolve.rs`), so the
    two cannot disagree.
  - _`BinaryInput`_ also carries `age_at_epoch`, which places the stars on their orbit at a
    supernova (BSE appendix A1), and a `BinaryParams` (BSE table 3). It is built through `new`,
    which returns a `Result` (`BuildBinaryInputError`).
  - _No draws on `binary.ce` or `binary.kick`._ The kick direction comes from each star's plan 06
    `StarDraws`, so the primary's kick stays plan 06's. The common-envelope code has no
    probabilistic branch. Neither tag is registered yet; both stay reserved.
  - _The secular rates are stepped, not closed forms_ (T4.b). Winds, tides, magnetic braking and
    gravitational radiation are integrated by midpoint steps under BSE section 2.8's limits (BSE
    steps by Euler's rule). Each step is a knot, and `state_at` joins the knots linearly, so
    circularisation and Peters's decay are on nodes. The 0.1-d double white dwarf merges within
    1% of `peters_merger_time` (about 0.4% as measured). Stable transfer is taken implicitly,
    with the rate found by bisection on the step's end overfill.
  - _A star on its own track accretes no wind._ Only a star the binary has touched carries a mass
    path.
  - _Contact._ Two main-sequence stars in contact stay so for the lighter star's thermal
    timescale before they coalesce, where BSE merges them at once. This is HYPERION's choice, so
    that contact pairs exist to be classified.
  - _Figures from the published code where the paper gives none_: a critical q of 3 for a type-1
    main-sequence donor and a core-helium-burning donor (the code's 2001 revision); the square
    root in Zahn's damping (BSE equation 42 as printed lost it); Peters's coefficient from G and
    c rather than BSE's rounded 8.315 × 10⁻¹⁰.
  - _Parameters._ `α_CE` = 1, λ = 0.5 (design note 14, provisional; BSE's table 3 and Model A
    take 3). BSE's own examples run with `with_alpha_ce(3.0)`. `β_W` = 0.5 is table 3's default;
    the published code's input file takes 1/8.
  - _Tests._ Five of the six reference binaries are their own tests. The blue straggler is
    checked inside BSE section 3.1's Algol. The 10³-pair invariant suite is `#[ignore]` and runs
    under `just test-slow`, and the default suite runs 60 pairs.
  - _Bench, and a finding: the target is missed by about 37 times._ `benches/binary.rs` times BSE's
    Algol and cataclysmic variable (`binary/binary_evolve/*`, 36 and 26 ms) and a fixed sample of
    182 pairs that pass `can_interact` (`binary/distribution`), each normalised by `math::exp`.
    On 2026-09-25, under the heavy-test lock at 4.1 GHz and a load average of 2.6, the median was
    7.4 ms (7.4 × 10⁵ calls of `exp`) and the 99th percentile 27 ms (2.7 × 10⁶), against the
    plan's median under 200 µs. The cost is not profiled yet. The likely cause is the midpoint
    steps under BSE section 2.8's limits, each of which evaluates both stars' structures. Plan 11 can't reach the brainstorm's millisecond
    per system until the engine is sped up. Options include coarser step limits, caching each
    segment's structures, or running the engine only for the systems a view asks for.
  - _Open, for the owner._ T2.c says T4.a replaces `PROVISIONAL_INTERACTING_PERIASTRON` with
    `can_interact`'s threshold, with a bump, but "Generator version" lists no bump for T4, and T4
    is built unwired. The seam stays in `multiplicity/hierarchy.rs`. The task that replaces it
    (T6 or T11, where the engine is wired) should be named, with its bump.
- **The engine's speed, as optimised** (round 9, `binspeed`; P11.T4.f's bench). Nothing moved:
  a new golden, `stellar/binary_timelines`, pins the full timeline of 10³ pairs (every segment,
  member, path and distinct track, and 257 states each) and was blessed before the first change;
  the optimised engine matched it bit for bit. It was blessed again after ruling 108 below, which
  moves results by design (499 of the 10³ digests), so it now pins the engine after that ruling.
  The profile (`pprof` sampling `benches/binary.rs`'s sample) had stable transfer at 51% of the
  time, 44% in the bisection for its rate, where each of about 38 trials a step built the donor's
  whole structure; Roche lobes 13%; track builds 16%; and a detached step evaluating each star's
  structure four times, twice over for a star on its own track. Now a trial reads only the
  donor's radius (`Track::radius_at`, `main_sequence_radius`) and shares everything that does not
  depend on the amount tried; a star on its own track is evaluated once (`own_structure_at`,
  `mass_at`); a detached step's end structures are the next step's start's; a transfer step
  reuses its structures for the stability test. Over the bench's 182 pairs, run interleaved with
  the engine as it was under the heavy-test lock (4.2 GHz, load 2.3, `exp` 7.8–8.3 ns), the median
  fell from 6.7 ms (8.1–8.5 × 10⁵ calls of `exp`) to 4.3 ms (5.5 × 10⁵) and the 99th percentile
  from 22 ms (2.7–2.8 × 10⁶) to 15 ms (2.0 × 10⁶). After ruling 108 below it is 4.5 ms (5.7 ×
  10⁵) and 15 ms, about 22 times the plan's 200 µs. Track builds alone (the two stars' tracks, and a massive primary's full
  one) cost about 1 ms per pair, and each stellar evaluation is a few µs of `powf`, so the target
  cannot be met bit for bit. P11.T11 can save the builds by handing the engine the tracks its
  `StarModel`s already hold, which moves nothing. Faster than that needs a result-moving change
  (a secant root for the transfer rate, coarser transfer or detached steps); those are the
  owner's call. Found on the way: `new_star_mass` recursed without end for a merger's core at or
  above the giant branch's base at `M_FGB` (a 13.7 + 3.7 M☉ pair); it now places no star there.
- **Ruling 108 as built** (round 9, `binspeed`; points 1–4, point 5 is P11.T11's). It supersedes
  the _Contact_ and _Parameters_ points of "Deviations in P11.T4, as built" above. Contact
  (`Engine::contact`): a main-sequence pair reached by transfer at the donor's thermal rate
  M ÷ `τ_KH` or faster coalesces on the lighter star's thermal timescale (the old `min` took the
  heavier's); reached more slowly it stays in contact until either star leaves the main
  sequence, then coalesces, or until the pair's age; below q = 0.09 (Rasio 1995) it coalesces at
  once. The masses are held in contact, so q does not fall there. The thermal-rate threshold is
  the lane's reading of "fast (thermal) transfer": a geometric-mean threshold between the thermal
  and nuclear rates called the ruling's own 1.0 + 0.5 M☉, 0.35 d pair fast (its rate over the
  last step was 3.5 × 10⁻⁹ M☉ yr⁻¹, a seventh of its thermal rate), and it stays in contact to the
  horizon under the rule built. `α_CE` = 1 with λ = 0.5 is settled (Claeys et al. 2014), with
  BSE's code default (α 3 with `celamf`'s structure λ) and the later refinement (that λ with α =
  0.25, Zorotovic et al. 2010) recorded in `params.rs`. `β_W` is `WindSpeedFactor::StarTrack`,
  the ruling's continuous form: COSMIC's code adds its rises to the floor, reaching 7.5 and 7.125
  at 120 M☉ and stepping to 7 above. The code's critical ratios differ in two places the engine
  does not follow: it uses Hjellming and Webbink's (1987) q_c = 0.362 + 1 ÷ (3 (1 − Mc ÷ M)) for
  giants (types 3, 5 and 6) where the engine uses BSE equation 57, and Claeys et al. (2014, their
  table 2, after de Mink et al. 2007) put the contact-driven threshold of a main-sequence pair at
  q = 1.6, against the engine's 3 (which `evolv2.f` confirms, with equation 42's square root).
  _The post-common-envelope test_ (108.2) runs 1.0–1.5 M☉ first-giant-branch progenitors with
  0.3 M☉ companions from 70, 100, 300 and 450 d: of the 23 helium white dwarf and main-sequence
  pairs, none lies under 1.9 h (the shortest is 2.7 h) and 21 lie under Nebot Gómez-Morán et
  al.'s (2011) 4.3 d. The two above come from 450 d, where the giant is met at the tip of its
  branch: 7.5 d from 1.0 M☉ and 4.35 d from 1.1 M☉. The test allows at most two, both from the
  widest orbit. A finding for research: with `α_CE` λ = 0.5 the widest first-giant-branch
  progenitors land above the observed range.
- **Ruling 111 as built** (lane `win111`, 2026-09-26, at `GENERATOR_VERSION` 12). _The
  post-common-envelope test_ (point 4) takes its top edge at **4.36 d**, SDSS J1434+5335's 4.357 d
  (Zorotovic et al. 2010, Table A.1), in place of Nebot Gómez-Morán et al.'s 4.3 d, and allows **at
  most one** pair above it, from the widest orbit, in place of two: 4.35 d from 1.1 M☉ at 450 d now
  lies inside, and 7.5 d from 1.0 M☉ at 450 d is the recorded exception, α = 1's known excess
  (Zorotovic et al. fit α 0.2–0.3). _Speed_ (point 6): the bit-exact floor of 4.5 ms is accepted;
  **P11.T11 passes the tracks in** (the `StarModel`s' own, saving about 1 ms a pair, moving
  nothing), and a result-moving solver change is allowed later if final masses and periods move by
  under 1%, with its own bump.
- **Ruling 114 as built** (lane `win111`, 2026-09-26; settles ruling 111.5). It supersedes the
  fast/slow rule of "Ruling 108 as built" above for main-sequence contact during transfer, by
  Nelson and Eggleton's (2001) cases. `Engine::contact` classifies (`ContactRegime`): **AD**, a
  donor rate above 10 M ÷ `τ_KH` (their eq. 8), calls `merge_dynamically` at once, as a q above
  q_crit does in `stability_of`; **AR**, at the thermal rate or faster **and** with
  `t_contact − t_RLOF < 0.1 t_MS`, coalesces on the lighter star's `τ_KH`; anything else is
  **slow**, the W Ursae Majoris channel of ruling 108.1, unchanged; below q = 0.09 it coalesces at
  once. A shallow AR contact, the accretor over its lobe by at most 10%
  (`TEMPORARY_CONTACT_OVERFILL`; de Mink, Pols and Hilditch 2007, §3.2), is temporary:
  `Engine::accretor_contact`, through `Engine::contact_relaxes`, lets `transfer_phase` go on in
  semi-detached transfer, unmarked, and each later step re-tests it, so it merges if the overfill
  passes 10% and turns slow if the rate drops. `t_RLOF` is the age at `roche_onset` and `t_MS` the donor's main-sequence lifetime at its
  mass there (`Engine::overflow_onset`, 0 for a donor off the main sequence); `t_contact` is the
  first step of the transfer at which the accretor filled its lobe (`Engine::first_contact`). The
  lane's readings: "t_MS" as the donor's, at its mass at the onset; AD as `merge_dynamically`
  rather than the thermal-timescale coalescence; a relaxed contact whose rate later falls under
  the thermal rate becomes a slow contact, on which the ruling is silent. Tests:
  `a_shallow_rapid_contact_returns_to_semi_detached_transfer` (5% and 9% overfill relax; slow and
  late contacts do not) and `a_deep_rapid_contact_merges` (20% lasts the lighter star's `τ_KH`,
  then merges; 20 times the thermal rate merges at once). `stellar/binary_timelines` did not move:
  no outcome among its 10³ pairs changes. A departure from BSE, which merges every contact.
  Rucinski's (2002) contact-binary share (1/1000–1/250 of main-sequence stars with M_V > +1.5) is
  P11.T11's check.
- **P11.T4.a's threshold behind plan 08's seam (ruling 120.1; lane `disp08`, 2026-09-27; no output
  moves).** `galaxy::displaced::binarity::interacting_periastron(m1, q, comp)` is `can_interact`'s
  boundary as a periastron, `max_i R_max,i ÷ f(m_i ÷ m_j)` (each star's `Track::max_radius_until`
  the primary's death at the median draws, Eggleton's lobe fraction `f`), and
  `binarity::stripped_share` passes it to this plan's `stripped_share`, building the primary's track
  once. Plan 11's hierarchy still reads `PROVISIONAL_INTERACTING_PERIASTRON`: moving it is T1.d's
  and T2.c's, with their bump. **Finding:** averaged over layer E the share is 0.485 (10 au gave
  0.46–0.51), against ruling 120.1's 0.25–0.33 over neutron-star progenitors, since the pre-test
  counts every interacting pair and Sana et al. (2012) find 20–30% of O stars merge rather than
  being stripped. T1.d's acceptance window (0.20–0.33) will miss by the same amount unless mergers
  are taken out of the share.
- **Ruling 123 behind plan 08's seam (lane `disp08`, 2026-09-27; no output moves).** Stripped is not
  interacting: `galaxy::displaced::binarity::stripped_share` is this plan's `stripped_share` at the
  stripping band's outer edge less at its inner one, `S(a_B) − S(a_merge)` (Case A or B before
  helium exhaustion, mergers out, by the engine's `GAP_Q` and `CODE_Q`), and
  `binarity::stripping_band(m1, q, comp)` returns the band. It is 0.287 over neutron-star
  progenitors, 0.295 over layer E (T1.d's 0.20–0.33 holds) and 0.619 of the interacting share.
  **Design note 1 as amended by ruling 123.5: T1.d and T2.c draw a marked innermost orbit from the
  stripping band and an unmarked one from its complement** (the merger band, Case C and wide pairs),
  instead of the interacting range; when the engine is wired (T6 or T11) about 10³ marked systems
  are checked to be stripped, not merged, before the primary's death, and the merger band to merge.
- **Deviations in P11.T5, as built** (round 9b, `bin5a`, 2026-09-27; unwired, nothing generated
  moves, no tag added). The code is `stellar/binary/{classify,recycling}.rs` and a new `marks.rs`
  for `from_marks`; `rlof::NOVA_RATE` becomes `pub(crate)` (the steady-burning line, shared), and
  `timeline.rs` gains `Context::from_parts` (which `Context::of` now calls) and
  `BinaryTimeline::context`. Every figure was re-checked by a research agent against its source.
  - _`classify(state, &ClassContext)`._ A class also reads the pair's composition (the turn-off),
    the stars' draws (a white dwarf's magnetism mark, a Be star's rotation), `BinaryParams` (the
    wind's speed and focusing), each neutron star's recycled pulsar and, after a merger, the phases
    it merged from. `BinaryTimeline::class_context(age)` builds the context and `class_at(age)`
    classifies. The rules are tried in a fixed order (contact; Roche-lobe overflow by accretor; a
    bound pair; one star), and the first match wins, so a hot subdwarf beside a blue straggler is
    a hot subdwarf, and a neutron star beside a supergiant is an X-ray binary, not symbiotic.
  - _Kinds._ `CvKind {DwarfNova, NovaLike, Magnetic, AmCvn}`, `XrbKind {Persistent, Transient}`
    for low-mass X-ray binaries only, and `HmxbKind {BeX, Supergiant}`, P11.T8.b's split: transfer
    from a donor of 8 M☉ or more (Fortin et al. 2023; intermediate-mass donors are low-mass, as
    Avakyan et al. 2023 count them) runs on the thermal timescale, far above the irradiated line,
    so a transient kind would be empty. `BinaryClass::ALL` lists the 19 values.
  - _Lines and figures._ Dwarf nova against nova-like by Lasota, Dubus and Kruk's (2008) eq. A.1
    and persistent against transient by their eq. A.2 (C = 10⁻³, α = 0.1), at Paczyński's (1977)
    disc radius 0.60 a ÷ (1 + q), or 0.9 of the accretor's lobe for a donor at least as heavy. A
    test holds both lines to Coriat, Fender and Dubus's (2012) power laws in period within 35%.
    Magnetic CVs: 15 of 42 (Pala et al. 2020) by the white dwarf's own `star.magnetism` mark,
    since plan 06 draws no white-dwarf field; a fossil-field progenitor's dwarf is always
    magnetic. Hot subdwarfs 0.32–0.8 M☉ (Han et al. 2002; Heber 2016). Symbiotics include a giant
    filling its lobe onto a white dwarf. R Coronae Borealis stars are the helium giants under
    M_Ch that a helium and a carbon–oxygen or oxygen–neon white dwarf merge into. Plan 06's Be
    star excludes a fossil field, as its `rotation_class` does.
  - _Recycling_ (`recycling.rs`): B = B₀ ÷ (1 + ΔM ÷ 10⁻⁴ M☉), floored at 10⁸ G (Shibazaki et al.
    1989 through Kiel et al. 2008, eq. 9, the primary not re-read; Zhang and Kojima 2006); the
    period is Tauris, Langer and Kramer's (2012) eq. 14 inverted in ΔM, held above their eq. 7's
    equilibrium period at the episode's mean rate and never slower than before. The recycled
    star is plan 06's `NeutronStar::new` again from the end of accretion, so it spins down on
    P06.T21's closed forms; a millisecond pulsar is under 30 ms (Lorimer 2008) and above the
    death line.
  - _Type Ia progenitor._ Double-degenerate: two white dwarfs, not both of helium, that merge
    within the age of the universe by Peters's formula, sub-Chandrasekhar pairs included as the
    brainstorm's pool and the engine include them. Single-degenerate: a carbon–oxygen dwarf fed
    hydrogen at 1.03 × 10⁻⁷ M☉ yr⁻¹ or more. `DoubleWhiteDwarf` is every other bound pair of
    white dwarfs.
  - _`carved_class`_ reads "any age of the horizon" at each segment's first age there, its
    midpoint there, the epoch's age and the horizon's end. The accreting white dwarfs split by
    P_rec at the epoch's age, or at the first accreting age; P_rec uses `nova_ignition_mass`, the
    research's least-squares fit to Yaron et al.'s (2005) table 2 at a 10⁷ K core (0.13 dex rms),
    since no published closed form covers 0.6–1.4 M☉ and 10⁻¹¹–10⁻⁷ M☉ yr⁻¹; P11.T8.a still
    re-checks it. A merger ends the stars' state as a pair; a star destroyed by its own explosion
    (also a `Merged` segment in the engine) is no merger.
  - _`from_marks`_ takes `BinaryMarks::{phase, merger}` (`MarkedPhase`, `MarkedMerger`), each star
    held at its marked state (`Member::Frozen`) on the marked orbit, so `state_at` the marked age
    is the marked state exactly. Until P15.T10.b the history before the phase holds the same
    stars, so the round-trip test takes phases that span the horizon, and mergers.
  - _Provisional, for review:_ `XRB_MIN_LUMINOSITY` 10³⁵ erg s⁻¹, for wind-fed systems only (no
    class definition cuts on luminosity; Lutovinov et al. 2013's survey completeness);
    `SYMBIOTIC_MIN_LUMINOSITY` 10 L☉ (Mikołajewska 2011); `DISC_RADIUS_SHARE` 0.9 (a research
    choice); the nova fit.
  - _The slow test_ (`tests/binary_classes.rs`) draws 360,000 prior pairs stratified by layer
    (200,000 / 100,000 / 20,000 / 10,000 / 30,000, [Fe/H] −1.5 to +0.4), not 10⁶ in the mass
    function's proportions. An interacting layer-E pair costs about 28 ms in the slow-test profile
    under load and a 10⁶ mixed sample about 20 CPU-minutes; the rarest class, the double neutron
    star, comes about twice per 12,000 layer-E pairs, which a mixed 10⁶ holds only 3,000 of. It
    runs on every core, about three minutes. **A finding for research:** 1,560 of 12,000 layer-E
    pairs had two supernovae, 422 stayed bound through both and 585 left two neutron stars, but
    only 2 were bound double neutron stars.
  - **For the orchestrator:** (1) the Type Ia double-degenerate reading, sub-Chandrasekhar
    included (the brainstorm's pool) against the surveys' total above M_Ch (Napiwotzki et al.
    2020); (2) Be/X needs no orbit or accretion condition, so a Be star with a neutron star at any
    separation is carved as an X-ray binary; (3) a neutron star fed by a low-mass giant's wind is
    `Symbiotic`, not carved, where Avakyan et al. (2023) list such systems (GX 1+4) as X-ray
    binaries.
- **P11.T13's `binary_class`, as built** (round 9b, `bin5a`). `StarSummaryDto.binary_class` is a
  `Modelled<BinaryClassDto>`, optional under P06.T33's convention, tagged by `type` with a `kind`
  for cataclysmic variables (`CataclysmicKindDto`) and X-ray binaries (`XrayBinaryKindDto`,
  `HighMassXrayBinaryKindDto`); `BinaryClass::None` is `null`. The server sends it absent until
  P11.T11 evolves each pair; `convert/stellar.rs`'s `binary_class_dto` is `expect(dead_code)`
  outside tests until T11 calls it.
- **P11.T14's class words, as built** (round 9b, `bin5a`). A `BINARY CLASSES` table (`STAR`,
  `BINARY CLASS`) follows `ORBITS` and reads `NONE` when no star is in a class; while the server
  computes none the list says `BINARY CLASSES: NOT YET MODELLED` once, the guide's section state.
  A `BINARY` column in `STARS` was built first and overflowed the readout at 1280 (377 px in 351).
  The words are `lib/galaxy/binaryClass.ts`'s. Checked by eye on seed `4d2` at (0, 26,000, 0) ly
  against the triple `GPF 005JFZ A-3` (M1.5 V, M4 V, M2 V; `AB–C` 24.3 yr at 7.51 AU and `A–B`
  82.5 d at 0.286 AU, both Kepler-consistent), 1920 and 1280, no sideways scroll. **For the
  owner:** `BINARY CLASSES`, `BINARY CLASS` and the class words join the draft nomenclature;
  `TRANSIENT` already names a detected event, so the X-ray binary's `· TRANSIENT` may want another
  word; `BE` and `TYPE IA` could keep the astronomers' case; `HOT SUBDWARF` is both a kind and a
  class; the longest word, `LOW-MASS X-RAY BINARY · TRANSIENT`, wraps at 1280 and is not yet seen
  on screen, since no class reaches the wire before T11.
- **Rulings 129 and 130 as built** (round 9b, `bin5a` follow-up, 2026-09-28; unwired, nothing
  generated moves). 129.4 (the engine's double-neutron-star losses) is another lane's.
  - _129.1:_ `TypeIaProgenitor` (double-degenerate) now also asks for a total above M_Ch or a
    heavier carbon–oxygen or oxygen–neon dwarf of at least `MIN_DETONATABLE_MASS` = 0.85 M☉ (Shen
    et al. 2018); any other bound pair of white dwarfs is `DoubleWhiteDwarf`. The engine's pool
    is unchanged. Tout et al.'s Algol leaves 0.49 + 0.66 M☉ and is now a double white dwarf; its
    test checks the masses and that the merger is still pooled, and a marks test holds the rule's
    four cases.
  - _129.2:_ Be/X asks for a Be star of at least 8 M☉ on an orbit of at most `BEX_MAX_PERIOD` =
    1,000 d (provisional); a pair that fails falls through to the wind rules.
  - _129.3:_ `XrbKind::Symbiotic`, a neutron star or black hole fed by a giant lighter than 8 M☉
    at `SYMBIOTIC_XRB_MIN_LUMINOSITY` = 10³² erg s⁻¹ or more (Yungelson et al. 2019), carved as an
    X-ray binary; `Symbiotic` is white dwarfs only. `BinaryClass::ALL` has 20 values, and
    `XrayBinaryKindDto` gains `symbiotic`.
  - _129.5:_ the re-citations (Lutovinov et al. 2013 section 4.1; Mikołajewska 2011 section 4),
    and `nova_ignition_mass` holds the rate to the fit's 10⁻¹¹–10⁻⁷.
  - _The marks round-trip test_ now takes a phase only where its class holds at the marked age: a
    wind-fed class that crosses its luminosity line later in the horizon cannot come from marks
    that hold the state then.
  - _Ruling 130 in the client._ `lib/galaxy/binaryClass.ts`'s `binaryClassRows` gives one row to
    an innermost pair whose two stars share a class (`A–B`) and one to a star that carries a class
    alone (`A`), in star order, under the key column `STARS`. The words are 130.3–4's and 130.x's.
    Hyphenated words are nowrap spans, and the star list's last column has no trailing padding.
    The guide gains 130's typography bullet, the `BINARY CLASS` row and the amended `STARS` and
    `TRANSIENT` rows. `CV` was not needed (see the width check in the round's report).
- **Deviations in P11.T1.d, as built (round 9b, `fates`, with P06.T30 and the period refit;
  version left at 14 for the orchestrator's batch of 15).**
  - _Edit 1._ `StellarFates::companion_mass_ratio_cdf(m1, q)` defaults to the uniform stand-in.
    Plan 02's quadratures now take a companion's part of any quantity as `∫ g dH` over
    `galaxy::fates::CompanionMasses`, `H(c)` the companions per system below mass c on 240 even
    intervals of ln c, each interval's mean of g from the primaries' running integral. `H` reads
    no age, so `derive.rs` builds it once for all seven populations through the new `_with`
    functions (`mean_present_mass_with`, `mean_formed_mass_with`, `mean_stars_per_system_with`).
    The brute-force test still agrees to 10⁻⁴.
  - _Edit 2._ `stellar::multiplicity::MultiplicityFates` (new `fates.rs`) wraps `TrackFates` with
    `MultiplicityModel::drawn_companion_frequency` and `CompanionLaw`, a table of
    `drawn_companion_mass_ratio_cdfs` at the `TrackFates` grid's masses, split at 0.8 M☉ where
    the law changes (below it the columns are the companion's place in its own range of ln q,
    above it ln q every 1/16 with 0.1, 0.3 and 0.95 among them), within 1.6 × 10⁻³ of the model.
    **Ruling 81's queued item is done here:** the `drawn_*` laws re-derive the quadratures from
    the direct construction above 1.5 M☉, blended in ln M₁ to 3 M☉ as the draw blends: Table 13's
    `f_mult` and Moe and Di Stefano's mass-ratio law over their uncorrected period law, which the
    fitted correction makes the drawn periods follow. The direct companions' subsystems are not
    counted (no closed form gives the survivors of the stability test).
  - _Edit 3._ `stars_below` under `fates_for` equals the new
    `all_stars_fraction_below_as_drawn` to 2.4 × 10⁻⁵ (test tolerance 5 × 10⁻⁴). The T1.c spine
    forms (`all_stars_fraction_below`, `stripped_share`) stay for T1.c's sampler tests.
  - _Edit 4 and ruling 123.5._ `binarity::stripped_share` is `band_share_as_drawn` over the
    stripping band (`direct_band_share` from 3 M☉: the innermost of n independent direct
    companions, density `n f (1 − F)^(n−1)`), and plan 06's mark is read against it everywhere
    through `binarity::is_stripped(mark, m, comp)`: the track, only inside `STRIPPED_WINDOW`
    (`is_companion_stripped(m0, comp, draws)`), the kick law (`with_stripped_mark` gains `m0` and
    `comp`) and the hierarchy. The exact share costs 12 ms a star, so `hyperion-fit`'s new
    `stripping` task tabulates it with the three stage radii (`tables::stripping`, 34 masses over
    5.5–150 M☉ by 11 [Fe/H] over the fitted Z; `StrippingTable`), and `stripped_share`,
    `stripping_band` and `StageRadii::of` read the table inside its domain. At the cells' centres
    the share is within 6 × 10⁻⁴ in the median and 0.073 at worst, across a blue-to-red change of
    the giant branch. `binarity::NEVER_STRIPPED` (the largest mark) keeps the radii's own tracks
    and the fate table's nodes unstripped without asking for the share. The hierarchy's
    `Innermost` is `Stripped(StageRadii)` or `Unstripped { radii, share }`: a marked innermost
    orbit's periastron lies in `(a_merge, a_B]` for its mass ratio, an unmarked one's outside, for
    the direct construction too (`without_wide_innermost` is gone). `PROVISIONAL_STRIPPED_SHARE`
    and `PROVISIONAL_INTERACTING_PERIASTRON` are removed; `KickLawParams::stripped_share` stays for
    the tests' quadratures only. Over 4,000 massive primaries, 1,952 are marked against 1,966
    expected, none loses its companion.
  - _The period correction._ `hyperion-fit`'s `period_correction` task is registered and writes
    `tables::period_correction`, which `direct.rs` and `period_fit::COMMITTED` read. Refit under
    the band: worst bin misses 0.12%, 0.07%, 0.08% and 0.10% at 3.5, 7, 12 and 28 M☉ (the 3.5 M☉
    row, below the mark, is bit-identical); tries rejected 28.9%, 53.7%, 70.9% and 72.8%; direct
    companions dropped 0.04%, 0.22%, 0.40% and 0.57%. The slow test's 7 M☉ interim band is back
    to 0.2%.
  - **Findings for the orchestrator.** (1) The stripped share re-derived from the direct
    construction is 0.455 over layer E and 0.433 over neutron-star progenitors, against
    0.20–0.33 and 0.25–0.33 (stripped ÷ interacting 0.700, inside 0.5–0.75); 0.35 at 8 M☉, 0.42
    at 12, 0.53 at 16.6, 0.50 at 150. Table 13 gives B stars nearly the O stars' close
    companions, and every q ≥ 1/3 pair inside `a_B` counts as stripped. The test holds the
    measured shares provisionally. (2) Consequences of (1), each test holding its measured value
    provisionally: the low-mode share `w` 0.2675 against 1/6–1/4 and the thin disc's retained
    neutron stars 0.271 against 1/6–1/4 (plan 08); cluster retention 0.314 at 100 km/s and 0.286
    at 50 km/s against 18–26% and 15–25% (plan 09's T9.b, ruling 126.3; the doctest reads a fifth
    to a third); under ruling 123.5's band 1.14% of 28 M☉ systems lose a companion (0.57% of
    companions), so the Table 13 test's per-system limit is 1.5% above 20 M☉ and 1% below. (3)
    The fixture's nuclear-disc centre is 19.23 per ly³ against plan 02's 12–19 (held at 19.3):
    the real fates lower its mean mass per system to 0.548 M☉.
  - _Types._ `MultiplicityFates` is concrete over `TrackFates`, not the sketch's
    `MultiplicityFates<F>`: one set of fates is ever wrapped. `StellarFates` also gains a provided
    batch method, `companion_mass_ratio_cdfs(m1, qs, out)`, value for value the single one, which
    `CompanionLaw` overrides with a sweep of its columns (the companions' integral asks for 241
    ratios at some 1,700 primaries; 124 ms a build before it, 27 ms after).
- **Ruling 137, as applied (round 9b, `fates`; tests and windows only, no golden moves).** The
  stripped share is per primary, and finding (1) above is answered: the lane's 0.433 stands.
  `the_band_averaged_stripped_share` asserts 0.33–0.47 over neutron-star progenitors (measured
  0.4329) and 0.35–0.52 over layer E (0.4550), stripped ÷ interacting 0.5–0.75 (0.7001); the
  provisional holds are gone. **Future check (P11.T6/T11, not built):** once the engine adds the
  companions' remnants, the share per exploding star lies in 0.28–0.40 (about 0.32–0.35 expected;
  Sana et al.'s 37% of hydrogen-poor core collapses). Ruling 81.8's 1% is per direct companion:
  the Table 13 test asserts under 1% of companions dropped per bin (0.07%, 0.42% and 0.56% at 3.5,
  12 and 28 M☉) and reports the systems losing one, held under 2% (0.06%, 0.64%, 1.14%); the
  per-system 1.5% above 20 M☉ is gone. The downstream windows of finding (2) follow in plans 08
  and 09, and the nuclear disc's of (3) in plan 02 (11.8–19.6 per ly³, measured 19.23).
- **Ruling 138's consequence for T1.d's figures (round 9b, `fates`).** At the fitted Chabrier scale
  (0.920) the drawn companions give 1.437 stars per system (T1.d's 1.33–1.45 holds); the spine
  construction's count gives 1.457 under the default, which `stars_per_system_lie_in_the_fates_bracket`
  now prints (it asserts Kroupa's 1.406). All stars below 0.5 M☉ over primaries below 8 M☉:
  69.84% (the census check of P15.T4.b, 67.8–71.0%).
- **Findings held for P11.T6/T11 by rulings 140 and 141 (no output moves now).** The model counts
  massive stars, core collapses and Type Ia progenitors on primaries only, per M☉ formed with the
  companions' mass included. (1) Ruling 140.9: once massive companions are counted as O stars and
  core collapses, massive stars per M☉ formed may rise ×1.3–1.5; re-check there the tracer-terms
  star formation rate (then about 2.4–2.7 M☉ a year against Licquia and Newman's 1.65 ± 0.19 and
  Chomiuk and Povich's 1.9 ± 0.4) and the core-collapse rate (about 2.5–2.8 a century against
  Rozwadowska et al.'s 1.63 ± 0.46). (2) Ruling 140.5: M4's 396 neutron stars against Ye et al.
  2019's 150–225, a tension on w per primary-born neutron star, re-checked with the companions'
  neutron stars. (3) Ruling 141.7: the model has 0.76 times Kroupa's 2.5–8 M☉ primaries per M☉
  formed (0.0312 against 0.0409). If the counted 2.5–8 M☉ stars, companions included, differ from
  Kroupa's 0.0409 by more than 10%, the Type Ia delay-time distribution is normalised per
  progenitor formed rather than per M☉ formed, which moves the Type Ia rate, the ancient share and
  the Type Ia entry count.
- **Ruling 129.4 as built** (round 9b, `bin4f`, 2026-09-28; unwired, nothing generated moves; the
  `stellar/binary_timelines` golden moves, re-blessed at version 14 for the version 15 batch).
  - _129.4a, the pinned hold._ `Engine::die` holds a pinned primary whose own track dies first at
    its last living state with the mass it has then (amended by P11.T4.g, 2026-10-03: the last
    living state is the member's, at the binary's mass, and a bare core is stripped to its helium
    star or white dwarf before any hold; amended by P11.T4.k, 2026-10-06: the pin's collapse takes
    the living mass, never the remnant's) (a star on its own track takes its track's
    living mass; the track's mass at the death is its remnant's, which the hold took before), and
    leaves the orbit as it is. Two more faults of the same kind were found and fixed: a detached
    step that lands on a star's own death took the remnant's mass at the step's end, so the orbit
    kept its angular momentum through the mass the death then took again (every companion death
    and white dwarf's birth on its own track); and a dead track re-set at an offset an ulp short of
    its death still gave the living mass at the death's instant, so the next step did the same
    (`die` now places the track at its death or past it, through `offset_for`). The invariant
    suite gains the ruling's test: no detached segment widens its orbit past M_start ÷ M_end,
    within `WIDENING_TOLERANCE` = 0.5. Tides that hand an accretor's spin to the orbit widen by up
    to 23% more over the 10³ pairs (a carbon–oxygen dwarf beside a spun-up main-sequence accretor),
    so the ruling's bare ratio cannot hold; before the fix the suite had segments at ×2–25. A
    regression test runs two of those pairs.
  - _129.4b:_ a helium Hertzsprung-gap or giant donor onto a neutron star or black hole is stable
    at any ratio (Tauris et al. 2015, section 6; Vigna-Gómez et al. 2018, section 2.2.5). A lobe
    inside the donor's core is still a common envelope, as for every donor. BSE's 0.784 stays for
    the other accretors.
  - _129.4c:_ `Track::remains_at` gives `Remains::Collapse` for a helium giant whose carbon–oxygen
    core is at or above M_Ch: a helium-star track entered where the star is at the core's mass,
    which dies at once with plan 06's remnant, and the core's state. The engine explodes it at
    once, companion-stripped (after a common envelope, on the orbit it left; after a Roche lobe
    strips it); a pinned primary whose collapse is still to come is held at the core's state.
    Below M_Ch the white dwarf stays, unclamped. A held core has no envelope, so a common
    envelope it meets again (touching its companion) merges the pair (`common_envelope`), where
    it would otherwise repeat to the segment cap (pair 0911 of the pinned thousand did; a test
    holds it).
  - _The golden_ (ruling 129.4 alone). `stellar/binary_timelines` moves in 278 of its 1,000 digests (24 of them with a
    new segment count). Run with one fix at a time on the old engine, 259 move by 129.4a alone
    (185 with primaries of 8 M☉ or more, 19 below 2 M☉, the white dwarfs born on their own
    tracks), 15 by 129.4b and 17 by 129.4c, 11 of those by both: 280 in all, of which the 278
    are a subset (two came back to the old digests when the death's track age was taken from the
    track itself, an ulp's difference). A
    pre-existing cap stays: pair 0077 (2.23 + 1.16 M☉ at 0.56 d) fills its 64 segments with
    zero-length stable-transfer segments at 1,052.655 Myr, which is not this ruling's.
  - _129.4d:_ `tests/binary_classes.rs` gives a primary of `stripped_mark_min_mass` or more its
    mark exactly where the periastron is in `binarity::stripping_band`, taking the primary's
    draws at the first attempt (`StarDraws::for_attempt`) whose mark agrees.
  - _129.4e, as first built:_ 5 bound double neutron stars in the 12,000 layer-E prior pairs,
    against 6–120, with 132 more whose first neutron star accreted about 1 M☉ in the now stable
    Case BB transfer and became a black hole before the second supernova (the engine before these
    fixes left 1, the census as it was none). Ruling 132 answered it.
- **Ruling 132 as built** (round 9b, `bin4f`, 2026-09-28; unwired, nothing generated moves;
  `stellar/binary_timelines` moves again, re-blessed at version 14 for the version 15 batch).
  - _132.1:_ `BinaryParams::GENERATOR.eddington_limit` is on. `rlof::eddington_rate` takes X from
    the transferred matter (0 for a helium or carbon surface, about 2.9 × 10⁻⁸ M☉ yr⁻¹ onto a
    neutron star in Case BB). The limit also holds a degenerate accretor's wind accretion
    (`detached.rs`), since it is the accretor's: it moves 88 of the golden's digests, and
    nothing measurable in the recycling census below. The X-ray kinds still read the donor's rate. **For P11.T9:** its X-ray
    luminosity must use the accreted rate, never above L_Edd.
  - _132.2:_ the census window is 6–144, and 50–90% of them at e < 0.3. It measures 112 bound
    double neutron stars, 71 (63%) at e < 0.3, none lost to a neutron star grown into a black
    hole. **Finding for P11.T12:** 25 of them merge within the age of the universe (Peters). Scaled
    by VG18's yield (24.04 per Myr from 0.24% of ≥ 8 M☉ binaries, 73% merging), that is about 29
    per Myr in the Galaxy, against Pol et al. (2019)'s 42 (+30 −14). The test fails only above
    twice Pol's upper limit (144 per Myr).
  - _132.3:_ a carried main sequence whose end, (1 − τ) times its lifetime, falls within 10⁻⁹ of
    the age is handed on at τ = 1 exactly (`Engine::main_sequence_tau`, in both the detached and
    the transfer steps). Pair 0077 now ends in 17 segments, its rejuvenated accretor leaving the
    main sequence at 1,052.655 Myr (a test). The invariant suite forbids a segment of no length
    that repeats the kind before it, except a common envelope: two at one instant are two
    envelopes (a giant's hydrogen, then the helium giant it leaves filling the lobe the first
    left; a 5.26 + 5.23 M☉ pair at 902 d does it). A repeated envelope reaches the cap, which the
    suite asserts no pair does.
  - _Recycling_ (a new slow test over layer D and the census's layer-E pairs, read where the
    companion first is a remnant): the median period of pulsars beside a carbon–oxygen or
    oxygen–neon white dwarf or a neutron star is asserted in TLK12's 10–50 ms. **Finding for
    P11.T12**, quartiles (25 / 50 / 75%):
    - beside a CO/ONe white dwarf (241): field 9.1 × 10⁸ / 5.7 × 10⁹ / 1.0 × 10¹⁰ G, period
      5.0 / 24.7 / 66 ms;
    - beside a neutron star (178): field 1.1 / 2.1 / 4.7 × 10¹⁰ G, period 34 / 65 / 126 ms;
    - beside a helium white dwarf (39): field 8.3 × 10⁸ / 1.6 × 10⁹ / 2.3 × 10⁹ G, period
      3.6 / 7.1 / 14 ms.

    The periods are mildly recycled as TLK12 has it. The fields beside heavy companions lie below
    the ruling's computed 3 × 10¹⁰–10¹² G, by about 5 times for the white dwarfs (about 0.1 M☉
    accreted under m_B = 10⁻⁴ M☉, against TLK12's "a few 10⁻² M☉"). P11.T12's field check takes it.

  - _The golden, both rulings._ Against HEAD, 401 of the 1,000 digests move and no pair reaches
    the cap (0077 and 0911 did). Over ruling 129.4's golden, 132.3's end of the main sequence
    alone moves 248 (every carried main sequence that ends is handed on at τ = 1 exactly), the
    limit on transfer 83 more, and on wind accretion 88.
- **The centre's members gain companions here, and their count must be re-derived (ruling 144.4;
  lane `centre09b`, 2026-09-29; nothing moves now).** Plan 09's centre counts its members by the
  class device at their own present mass with no companions (P09.T26–T27): about 5.8 × 10⁷
  systems at a mean of 0.42 M☉ for the Milky Way's 2.5 × 10⁷ M☉, against 4.2 × 10⁷ at the nuclear
  disc's 0.585 M☉, which counts companions. That is right while members have none. When this plan
  gives centre members companions, it must re-derive the count from the cluster's mass with the
  companions included, or the cluster's mass grows by the companions' (×1.38 at the field's
  multiplicity). The field's multiplicity is wrong there anyway: the hard–soft boundary is
  `G m ÷ σ²` ≈ 0.1 au at σ ≈ 100 km/s, so nearly every binary is soft, and at 10⁵ M☉ pc⁻³ (about
  1 pc) evaporation (Binney and Tremaine 2008, §7.5.7, eq. 7.173) takes about 3 Gyr at 1 au, 0.3
  Gyr at 10 au and 30 Myr at 100 au, while at 10 pc a 10 au pair lasts a Hubble time. So inside
  about 2 pc companions wider than about 10 au are ionised, and outside most survive; the eventual
  count lies between 4.2 and 5.8 × 10⁷, nearer the lower by mass. The brainstorm's figure is now
  "4–6 × 10⁷" to cover both.
- **Re-test the kick law on the isolated neutron stars once this plan decides bound and unbound
  (ruling 147.2's risk; lane `kick147`, 2026-09-29; recorded, nothing changed).** Since ruling
  147.2 the kick law's rank table is built on every ordinary-mode neutron star with the
  companion-stripped mark applied (`ReferencePopulation::score`), and test 1 of
  `tests/stellar_kick.rs` asserts Disberg and Mandel's (2025) log-normal on the same set, because
  their young isolated pulsars are the single, merged and widely bound neutron stars and the
  stripped ones whose ordinary kick unbound the pair. The stripped ordinary-mode stars sit high (ln
  v about 6.0 against the whole mode's 5.56) only because the low mode's ramp takes the light
  stripped cores, and Müller et al. (2018) and Willcox et al. (2021), as Disberg and Mandel's §6
  reports them, argue that stripping may itself lower kicks. **At P11.T6 and T11**, when the
  engine decides which pairs a kick unbinds, re-test the log-normal on the unbound and single
  neutron stars only, and rule on the table's population again if they miss it.
- **Deviations in P11.T2.d, as built** (round 9c, `bin5b`, 2026-09-29; output moves, the bump
  batched into version 16). The code is a new `stellar/multiplicity/substellar.rs`, with
  `SystemHierarchy::with_outer_brown_dwarf` in `hierarchy.rs`,
  `MultiplicityModel::substellar_mass_ratio_slope` in `dist.rs` and the tag `system.substellar`
  (scope `System`) appended at the end of `domain_tags!`. `draw_hierarchy` draws the stars as
  before (`draw_hierarchy_with`, which the period correction's fit keeps reading, stars only) and
  then the companion.
  - _Where it goes._ Design note 15 says it "joins the hierarchy under the same stability test".
    It joins only as a new outermost orbit about the whole system: for a single primary, an orbit
    about the primary. Joining an outer member would raise that member's pair's μ, and so move a
    stellar orbit's semi-major axis at its drawn period, and joining inside the primary's pair
    would break the depth-first numbering (Design note 5's "a brown-dwarf companion last"). As
    built, every star, node mass and stellar orbit is the stellar draw's bit for bit, one node
    further on (a test over 10⁴ systems). The price is that a brown dwarf of a multiple system is
    circum-multiple, never about one component. **For the orchestrator to rule** whether it
    should also join an outer member.
  - _The decision_ is one mark at word 64n of attempt n against `substellar_companion_probability`
    (`SUBSTELLAR_ANCHORS`, linear in ln m, constant outside): 0.035 at 0.3 M☉ (Dieterich et al.
    2012, AJ 144, 64), 0.06 at 1 M☉ (Metchev and Hillenbrand 2009, ApJS 181, 62; Kiefer et al.
    2019, A&A 631, A125), 0.03 at 2.5 M☉ (Nielsen et al. 2019, AJ 158, 13) and 0.02 at 8 M☉ (no
    measurement). A research agent read the surveys; only the Sun-like anchor is measured over
    most separations, and the others are extrapolated. **Provisional.**
  - _The desert._ `SUBSTELLAR_DESERT_FACTOR` = 0.3 below `SUBSTELLAR_DESERT_PERIOD` = 10³ d, derived
    against the Sun-like period law from Grether and Lineweaver's (2006) < 1% inside five years
    (f < 0.75), Sahlmann et al.'s (2011, A&A 525, A95) 0.6% (f ≲ 0.45) and Kiefer et al.'s ≥ 2%
    inside 10 au (f ≈ 1 beyond 10³ d); defensible 0.2–0.5. **Provisional.** The thinning is a
    conditional draw, not a rejection: one mark picks the window below or above 10³ d by weights
    `f × share` and `share` of the period law inside the necessary conditions (outside the stellar
    root orbit by the criterion's smallest axis ratio, inside the tidal cut), and the period inside
    it by the mark's residual, so the decision's probability is the companion frequency.
  - _Tries._ Nine tries of seven words (the mark, the mass ratio, the eccentricity, three angles,
    the mean anomaly) on `system.substellar`, then the companion is dropped. The mass ratio is the
    stellar law's lowest segment, `q^γ` with Duchêne and Kraus's γ below 0.8 M☉ and
    `max(γ_smallq, 0)` above, on 13 M_J ÷ m₁ to 0.08 M☉ ÷ m₁; the eccentricity is the stellar law's at the period.
    The stripped mark does not bind it (`Innermost::Free`), and `ForcedMultiple`'s widest
    separation does.
  - _Counted as a star._ `SystemHierarchy::star_count`, `SystemStars::star_count` and the wire's
    `star_count` include the brown dwarf, so every body at the stellar level is counted and the
    summary's list of stars keeps its length: 1 to 5. `draw_star_count` reads the decision word for
    a system with no stellar companion, so a single star stays one word. The protocol's and the
    client's doc comments say 1 to 5. **For the owner:** the `GALAXY` list's `3 stars` and the
    `SYSTEM` readout then count a brown dwarf with the stars.
  - _Measured_ (10⁴ systems over the mass function at the Sun-like point): 404 brown dwarfs over
    14,400 stars, 0.0281 per star (the test's 0.02–0.06); of 10⁴ Sun-like primaries 570 have one
    (5.7%, against the anchor's 6%, the rest dropped by the test), 12 of them inside 10³ d (0.12%,
    under 1%). `all_stars_fraction_below` and its as-drawn form are pinned to the last bit at 0.5
    and 1 M☉ under Kroupa's function (0.766 322 810 095 879 and 0.767 809 616 117 806 below
    0.5 M☉).
  - _Tests._ `hierarchy.rs`'s T2.a–b property tests keep to the stars (`draw_hierarchy_with`);
    `substellar.rs`'s run the whole draw: the companion moves no star, the counts, the desert,
    the whole stability test and order independence with the companion, and the count-only draw.
    Acceptance also takes `cargo test -p hyperion-sim multiplicity::substellar`.
- **Deviations in P11.T7, as built** (round 9c, `bin5b`, 2026-09-29; output moves, the bump batched
  into version 16).
  - _The redraw_ is in `SystemStars::generate_with`: for a grid record under `Free`
    (`stellar::binary::carve::grid_redraws`; a feature member's binaries are plan 09's and
    P11.T8.f's), attempt n draws the hierarchy and each companion's `StarDraws::for_attempt(n)`
    (`SystemStars::at_attempt`), runs its pairs (P11.T11), and the first attempt whose pairs all
    pass `carved_class` is kept. `SystemStars::attempt` and `carved_pair` expose it. The primary's
    model is built once and never redrawn. The Type Ia carve (P11.T6) is not built, so only the
    four binary classes redraw.
  - _After eight carved attempts_ the system keeps its primary alone, at attempt 7, where Design
    note 9 keeps the last hierarchy with its innermost period moved out of the interacting range. A
    single star is never in a class, so the grid half of complementarity stays exact, and moving
    one period would need the engine run again with no guarantee. Unreachable in expectation
    (below 10⁻²⁸ a system at the measured carve rates).
  - _The brief's count._ The range brief (`draw_star_count`, P06.T38.e) runs no engine and reads
    attempt 0, and `SystemStars::brief_at` reports the same first attempt's count, so that a row is
    the same whichever built it. For a redrawn system whose kept attempt has another star count the
    row then disagrees with the summary and `SystemStars::star_count`, about half of the redrawn
    systems. **For the orchestrator to rule:** accept, or have the brief evolve the pairs of a
    multiple system (the engine for every multiple row).
  - _`tables/binary.rs`_ holds `ClassShareTable`, `ClassShareRow` and the scratch `CLASS_SHARES`
    (Design note 13): the same hosts per solar mass formed in every population, scaled to the Milky
    Way fixture's formed mass, `MILKY_WAY_FORMED_MASS` = 9.376 × 10¹⁰ M☉ (its components' born
    systems times 0.957 M☉), for 4.5 × 10⁶ fast and 4.5 × 10⁶ slow accreting white dwarfs, 10⁴
    X-ray binaries, 9 × 10⁴ stellar mergers in the source horizon and 10 neutron-star mergers.
    Layer shares are a guess from the stars that make each class: accreting white dwarfs C 0.75, D
    0.25; X-ray binaries and neutron-star mergers E; stellar mergers A 0.1, B 0.2, C 0.5, D 0.15, E
    0.05. The header says provisional; it is not in `tables::MANIFEST`, which `hyperion-fit` writes
    from its lock file, until plan 15 has a task for it.
  - _The share matrix._ `FeatureShares::set_class_shares` stores, per population and stellar band,
    `Σ r m̄_f w_b ÷ S_b` (`class_share`), and `field_factor` is `1 − φ − class_share`; `Galaxy` sets
    `CLASS_SHARES` when it builds. The shares are about 4 × 10⁻⁴ of band C and 7 × 10⁻⁴ of band D,
    the largest (the accreting white dwarfs). Nothing reads `field_factor` into the fields until
    P09.T2.c, so the share matrix moves no output yet; the bump is the redraw's.
  - _Tests._ `binary::carve` holds a pinned carved system (candidate 21 of the test cell, 12 M☉ at
    30 Myr, an X-ray binary at attempt 0) and the forced contexts. The budget and the table are
    `galaxy::features::shares` and `tables::binary`, and the slow grid half is
    `tests/binary_carve.rs` (`--run-ignored`), so the acceptance takes those filters too.
  - _Measured_ (slow, `tests/binary_carve.rs`, 10⁵ grid systems stratified by layer; 2026-09-30):
    no kept system is carved, and none needs more than 3 attempts (attempts kept 99,756 / 238 / 6).
    Redrawn: A 0 of 30,000, B 0 of 20,000, C 22 of 20,000 (1.1 × 10⁻³), D 186 of 20,000 (9.3 ×
    10⁻³), E 36 of 10,000 (3.6 × 10⁻³). **A finding (provisional; ruling deferred):** the engine
    carves layers C–E some 3–13 times the scratch `CLASS_SHARES` give up (4 × 10⁻⁴ of C, 7 × 10⁻⁴
    of D), and about 5 × 10⁻⁴ of all systems against Design note 8's 10⁻⁴, nearly all of it X-ray
    binaries and accreting white dwarfs. The redraw is exact whatever the rate, but the budgets
    close only when the shares match what the grid refuses (P15.T10.b's fit, or P11.T8's counts).
- **Deviations in P11.T11, as built** (round 9c, `bin5b`, 2026-09-29; output moves, the bump
  batched into version 16).
  - _Which pairs run._ A pair of two stars (a `Pair` of two `Star` nodes, neither a brown dwarf) is
    run through the engine, from zero age to the pair's age at +H, and kept as a `PairTimeline`
    (`SystemStars::pairs`), only if it can interact by +H (`can_interact`, on the models' tracks)
    or a member has a remnant by then. Any other pair is two single stars on their orbit (Design
    note 7) in no class, whose timeline would be one detached segment of its models (a test holds
    that `evolve` gives it that): it keeps its drawn orbit, and skipping it keeps the engine off the
    great majority of pairs. A pair with a member that is itself a pair (a triple's outer orbit) is
    not run: its drawn orbit stands and its members follow their own timelines.
    The inner star is the engine's primary, so design note 16's pinned death also holds a massive
    inner star of a companion's subsystem at its single-star death, not only the system's primary.
  - _Tracks passed in_ (ruling 111.5). `StarModel` now holds its track as an `Arc<Track>`, and
    `evolve_with_tracks` (crate-private) takes the models' tracks; a pinned primary's full track is
    taken from its model only when that is built in full. A test holds every timeline equal to
    `evolve`'s, bit for bit. `stellar/binary_timelines` does not move.
  - _`SystemStars::state_at`_ returns a `SystemState`: each star's state from its pair's timeline
    or its own model, each pair's `PairState` (the engine's orbit, `None` once merged or unbound,
    or the drawn orbit; the pair's `BinaryClass`), the combined luminosity and the system's mass,
    summed in body order. Also `binary_state_at`, `system_mass_at` and `recoil` (the primary's
    pair's). A time past a timeline's end reads the star's own model.
  - _The summary._ `StarSummary::binary_class` is the pair's class, `None` for a star merged into
    its companion. A star its pair has changed is summarised from the pair's state: its remnant
    from its phase and mass, a neutron star's recycled pulsar from the timeline, its rotation
    without the terminal main-sequence state (the model's track is not the star's), and no
    `death_in_window` unless the timeline explodes it at its model's death age (a pinned primary's, design note 16), since the engine records a death without its kind. A merged-away star stays
    in the list as plan 06's `NoRemnant`, so the list keeps its length. The hierarchy in the
    summary is the drawn one; `state_at` has the orbits then. The server sends `binary_class`
    (`null` for none); the client's `BINARY CLASSES: NOT YET MODELLED` state gives way to `NONE`.
  - _The brief_ (`brief_at`, and the range brief) keeps the primary's single-star state, so that a
    range row is the same whichever built it and the range brief stays free of the engine. A
    primary that its pair has changed (an Algol's donor, a merger product) reads so in the summary
    only. **For the orchestrator to rule.**
  - _The tidal radius and census fields._ `planetary::context` already reads the tidal radius at
    the hierarchy's system mass; plan 03's frame rule keeps the primary's mass, as it must before
    any hierarchy is drawn, which the tidal cut's half share covers. The bright exception is
    recorded in `MassFloor`'s doc (`galaxy/query/request.rs`).
  - _Plan 12's lens mass_ (`lensing::lens_mass_at`) reads a grid system's first-attempt single-star
    models (`stellar::system::first_attempt_models`), the range row's cost, not `SystemStars`: with
    the engine a lensing test that sums thousands of lenses ran for minutes. Its layer bound adds a
    brown dwarf's 0.08 M☉. **For the orchestrator to rule** whether lenses should take the engine's
    masses.
  - _Bench._ `benches/stellar.rs` gains `stellar/system_full`: `generate` over every system of the
    50 ly query about the Sun-like point, and over one system with an interacting pair. Not run in
    this lane (the machine is shared; `just bench` takes the heavy-test lock), so the figure is
    open.
  - _An engine case found here_ (`stellar/binary/star.rs`, `shaped_track_age`): a mass-changed
    (`Shaped`) accretor, 7.9 M☉ and 21.7 Myr younger than its pair, was read 46,000 years past its
    own track's death, which tripped the track's range check in debug builds. Release builds held it
    at the track's last age already; the hold is now explicit for `Shaped` members, so debug and
    release agree and no golden moves. **A finding (provisional; ruling deferred):** the engine's
    detached step does not always end a segment at a shaped star's own death. One more instance
    (ruling p11-t4k-faults, 2026-10-06): a capped timeline's last segment, stretched to the age
    asked, reads its shaped member past that member's death (layer-E prior pair 8002 of
    `binary_classes`' sampler; see "P11.T4.g's finding C2" below).
  - _Measured_ (slow, `tests/binary_system.rs`, 2026-09-30; every figure provisional, rulings
    deferred):
    - The bright exception: 4 of 86,185 old-thin-disc layer-A and -B systems (4.6 × 10⁻⁵), none of
      the young disc's 310, the thick disc's 3,403 or the halo's 102: under 10⁻³ (asserted).
    - Rucinski's contact binaries (ruling 114): 1.1 × 10⁻⁴ contact pairs per main-sequence star
      fainter than M_V = +1.5, against 1/1,000–1/250, **ten times low** (0 in layer A, 5 in B, 23 in
      C, 16 in D).
    - Massive stars with their companions (ruling 140.9): 8 M☉ and up rise ×1.52 (0.00866 → 0.0132
      per M☉ formed), 15 M☉ and up ×1.52 (0.00371 → 0.00565), at the top of the ruling's ×1.3–1.5:
      the tracer-terms star formation rate and the core-collapse rate it re-checks are then about
      ×1.5 their primaries-only values.
    - 2.5–8 M☉ stars with their companions (ruling 141.7): 0.0447 per M☉ formed against Kroupa's
      0.0409, +9.2%, inside the ruling's 10%: the delay-time distribution stays per M☉ formed.
    - The stripped mark (ruling 123.5), 4,000 primaries of 8 M☉ and up at 60 Myr: of 1,753 marked,
      1,408 (80%) are stripped by transfer or an envelope and not merged before their deaths, and
      294 (17%) merge; of 986 unmarked with the innermost orbit in the merger band, 783 (79%) merge.
    - The stripped share per exploding star (ruling 137), companions' collapses included: 2,595 of
      5,071 hydrogen-poor (0.51, counted as a naked helium star just before the supernova or an
      envelope under 0.1 M☉), against 0.28–0.40.
    - Not measured here: ruling 140.5's M4 (a cluster's members, P11.T8.f) and ruling 147.2's
      re-test on unbound and single neutron stars (the kick law's reference population, not the
      system stage).
- **Status of `bin5b` at the 2026-09-30 wrap-up (uncommitted, in `target/lanes/integ` on 872704e).**
  - P11.T2.d, T7 and T11: code and tests built; the new slow tests (`tests/binary_carve.rs`,
    `tests/binary_system.rs`) pass on the current code. `GENERATOR_VERSION` not bumped (the v16
    batch). Goldens partly blessed at 15: `rng/tags`, `stellar/hierarchies`, `stellar/summaries`,
    `planetary/systems/hierarchical_triple` and the server's `systems_in_range_briefs` moved; the
    second bless pass (`every_golden_file_carries_the_current_version`) was not re-run.
  - P14.T32's `hierarchical_triple` (`0x4200_2cb2_0000_0003`) now has a bound brown dwarf (4
    bodies), which failed `pinned_ids_satisfy_their_own_predicates`. Resolved at the version-16
    integration: the golden systems' predicates count only `SlotKind::Star` slots as stars, and a
    brown dwarf's orbit about the system is no stellar pair in their separations.
  - Open: the `system_full` bench is written but not run; `just ci` not run (single combined gate).
- **Added by rendering plan R03's R03.T3, by agreement (R03 Design note 8).**
  `stellar::multiplicity::star_states_at(h, t, out: &mut Vec<(BodyId, SystemPosition,
SystemVelocity)>)` in `stellar/multiplicity/positions.rs`: `star_positions_at`'s walk with each
  star's velocity relative to the barycentre beside its position, the positions bit for bit
  `star_positions_at`'s (tested), the velocities the time derivative of the positions (tested by
  central differences). `star_positions_at` is untouched and no golden moved.
- **Fixed after P11.T11 (2026-10-03, found by rendering plan R06.T5.c): the common envelope and
  the merger recursed without end on a held bare core.** Ruling 129.4c's guard in
  `Engine::common_envelope` sends a Frozen donor with no binding energy to a merger
  (`coalesce(None)` → `mix`), but `mix`'s fallback sends any collision involving a giant-like star
  back to a common envelope, with the same state, so a held EarlyAgb bare core (M = M_c = 4.88 M☉)
  touching a 12.65 M☉ main-sequence star (record 0x81fd865fd000000f of seed 0x0926_0000, bulge,
  layer E) recursed until the stack overflowed, in `SystemStars::generate` and so in every path
  that generates a system's stars (scene, system summary, observed range queries, planetary
  context, the sky's census). The existing 129.4c test passed only because its companion was a
  neutron star, whose arm of `mix` comes first. Fix: the guard calls `mix_with(Envelope::Spent)`,
  which merges by the collision matrix (BSE §2.7.3, M₃ = M₁ + M₂; Table 2 gives (5, 1) → 5), and an
  engine flag `in_common_envelope`, set while a common envelope resolves, keeps any merger reached
  from inside one from entering another, as BSE's `comenv` never calls itself (`debug_assert` on
  re-entry). **No generator-version bump**: output moves only for inputs that aborted before (the
  determinism audit traced every path into `mix` and `common_envelope`; no golden moved), and the
  new golden `stellar/held_bare_core_merger` pins the record's stars at the epoch. **Open,
  upstream, routed to the orchestrator:** the science check finds the held state itself
  inconsistent: `die`'s pinned hold (`supernova.rs`) takes the phase and the 1,054 R☉ radius from
  the single-star track and the mass from the binary, so a star stripped to its core is held as an
  EarlyAgb supergiant with no envelope, where Hurley, Pols and Tout (2000, §6) make it a naked
  helium giant of 0.6–3 R☉; `integrate` returns a death stop before its `Stop::Stripped` check on
  the same step; and `frozen_structure` takes the core radius as 0.1 R (105 R☉ here, against
  R_HeGB's 0.6–3 R☉, uncited). With a correct state the pair would stay detached and the main-
  sequence star would overflow its lobe instead. Fixing it moves generated timelines (a bump) and
  touches ruling 129.4a/c, so it waits on the owner.
- **P11.T4.g as built** (Phase J lane, 2026-10-03; ruling p11-stripped-core with the owner's leans;
  goldens blessed at version 19 for the version-20 batch, which P14 Phase J's T46.f/T47.d commit
  bumps).
  - _The hold_ is `Engine::hold` (`supernova.rs`): a carried star's state is
    `Track::structure_at(last_living, M)`, which is `Member::evaluate`'s for a `Shaped` member read
    at the track's age itself (adding and removing the offset could land an ulp on the death); a
    star on its own track keeps `Track::state_at` bit for bit, its core radius from
    `own_structure_at`. A structure with no envelope goes through the new
    `Engine::stripped_member_at(i, track_age)` (`stripped_member` at the engine's age is that at
    `age − offset`), which returns `None` where there is no envelope to lose; then the member is
    frozen as before. `Remains::Collapse` gains `core_radius`, its core's structure's.
  - _Marked stars_ (`marks.rs`) are frozen with a core radius of zero: they are never evolved, and
    nothing reads it.
  - _The golden._ `stellar/binary_timelines` moves in 225 of its 1,000 digests (118 with a new
    segment count). With one change at a time on the new engine: 107 by the hold (R and L at the
    binary's mass for the enveloped holds the ruling found slightly wrong), 6 more by the transfer
    strip, 113 more by the detached stop order, 0 by the core radius. No other golden moved:
    `stellar/held_bare_core_merger` is renamed `stellar/held_bare_core_helium_star` with its test.
    The record's epoch is now two runaway neutron stars (1.485 and 1.372 M☉): the primary's
    supernova at 15.896 Myr disrupts the pair, where the old engine merged it into one.
  - _The example_ runs through `SystemStars::generate` in `tests/stellar_system.rs`, where the
    regression already was, rather than as a rebuilt `BinaryInput`. Between 15.69 Myr and the
    collapse `swell` turns the fed helium star into a core-helium-burning giant with a thin
    envelope again and again (about every 0.03 Myr), each stripped back at once by the transfer
    strip, so the test allows such a giant with M > Mc at each segment's start beside the helium
    phases. R stays under 3 R☉ throughout. `swell`'s core is the finding left open (its measured
    consequence, ruling p11-t4k-faults: "P11.T4.g's finding C2" below).
  - _The invariant_ (`check_no_bare_giant`, in the 60-pair and 10³-pair suites) checks M > Mc,
    the engine's own test, not M > Mc + 10⁻⁹: a donor's wind or transfer can leave an envelope of
    10⁻¹² M☉ at a step. It checks at each segment's start and at the knots of a carried star's mass
    path, not between them: inside a step the core may outgrow the mass until the next step strips
    it, as in BSE. It checks only stars the binary carries (`Shaped`): plan 06's own track ends its
    thermally pulsing AGB at M = Mc. A held member's Rc is checked against sse's values (0,
    R_ZHe(Mc), 5 R_WD(Mc), each held to R; core-helium burning skipped, its τ not being kept); the
    ruling's "R ≤ R(track at its own mass)" is not checked, since `Frozen` holds no track.
  - _The stop order test_ (`a_bare_star_at_its_phase_boundary_is_stripped_first`) builds the
    boundary case: a 5 M☉ star carried 10⁻⁷ M☉ above its core 10 yr before the end of its gap,
    whose core grows 4 × 10⁻⁷ M☉ in those 10 yr while the wind takes 10⁻¹⁰, so no strip is
    predicted; it fails with the old order. The pin case is not built: it is the same branch.
  - _Statistical checks_ (slow, all pass): ruling 137's hydrogen-poor share of core collapses
    0.511 (0.51 before; the expected rise of 0.1–0.3 points did not show at this precision);
    123.5's marked-stripped 1,407 of 1,753 (80.3%) and merged 295 (16.8%); `binary_carve` passes;
    the 10³-pair invariants pass; the R06 census (`a_hundred_thousand_systems_live_and_die_in_order`)
    passes.
  - _Science check findings._ Citations corrected: the core-radius rule is HPT §6.3 (after
    eq. 105) and SSE/BSE `hrdiag`, not BSE §2.7.1; the M ≤ Mc rule is HPT §6 and `hrdiag` as
    `evolv2` calls it, not BSE §2.8. **Open, for the owner:** on the early AGB the sources give
    Rc = min(R_HeHG(Mc, Lx, R_ZHe(Mc), L_THe), R_HeGB(Lx)) (HPT §6.3 after eq. 105; `hrdiag`
    kw = 5), not R_ZHe(Mc) as sse's `core_radius` and the ruling's item 3 say: it grows with L to
    2.4 R_ZHe at 10⁵ L☉ for a 4.88 M☉ core. It feeds the held EAGB stars' Rc (the common
    envelope's core test and tides), though it moved no golden digest here. Also open: the strip
    runs after `contact_check` in a detached step, where `evolv2` strips before its Roche and
    collision tests (small: a bare carried star's radius is near its helium star's).
  - _Version 20's early-AGB core radius_ (ruling p11-stripped-core, amendments 1 and 2,
    2026-10-04). Version 20 ships with Rc = R_ZHe(Mc) on the early AGB, a known departure. P11.T4.h
    (below) builds the sources' rule for version 21.
- **P11.T4.h as built** (Phase J lane, 2026-10-04; ruling p11-stripped-core, amendments 1 and 2).
  The early AGB's core radius and its small-envelope remnant at SSE's τ. **Built for version 21,
  committed with its goldens re-blessed at version 20 and held out of integration** until the 20 →
  21 bump lands with it (with P14.T47.e, `decision-r07-earth-albedo.md`; P11.T4.g's precedent).
  - _The rule_ (HPT §6.3 after eq. 105, eqs 84–88; SSE `hrdiag` kw = 5, lines 728–736; `star`).
    The remnant is the naked helium star of the helium core, `Mc,He`, at Lx = L_THe (L_rel ÷
    L_THe)^τ while τ < 1, then L_rel. L_rel is equation 84's relation at the carbon–oxygen core.
    Its radius is R_HeGB = min(R₁, R₂). τ = 3 (t − t_BAGB) ÷ (t_n − t_BAGB), with t_n SSE's nuclear
    end at the current mass (`EarlyAgb::nuclear_end`). That is when the thermally pulsing AGB's
    relation (A_H,He, from `tscls(13)` and L_DU; for type 5 SSE takes Mt itself, without
    third dredge-up) would bring the core to Mt, capped at `tscls(14)`. `tscls(14)` is when the core
    reaches `Mc,SN`: on the early AGB's relation if `Mc,SN` ≤ `Mc,DU`, otherwise on the pulses'
    at (`Mc,SN` − λ `Mc,DU`) ÷ (1 − λ), and no earlier than t_BAGB. τ is 0 from 100 M☉, where
    SSE's t_n is helium ignition.
    - **Deviation from `star.f` as written:** Mt is held to at least `Mc,BAGB`. SSE's type 5
      never sees Mt ≤ `Mc,BAGB` (`hrdiag` 393–396 makes the star a naked helium star first). The
      track evaluates such a star on its way to the envelope's loss, and there the literal rule
      (t_n ≤ t_BAGB below `Mc,DU`, so τ = 0) stepped τ from ≥ 1 to 0, and the wind with it.
    - That step hung the R06 census and filled the machine's memory: plan 06's envelope
      integrator has no knot cap past its grid (plan 06's Risks; the record is
      0x61f85aa800000001, a 0.84 M☉ companion stripped by its wind on the early AGB). With the
      hold, the census passes in 183 s at about 8 MiB peak.
    - `stellar_system.rs` `a_companion_stripped_by_its_wind_on_the_early_agb_has_a_fate` pins the
      record, and `the_nuclear_end_is_sses` checks that t_n and τ have no step at `Mc,BAGB`.
    - `EarlyAgb` gains `pulse_times` and `t_mc_max`.
    - `track/model.rs` has `early_agb_core(phase, helium, clock, mt)` (and `…_at_tau`). It is
      shared by `early_agb`'s remnant and by `binary::core_radius`'s EAGB arm, unconditionally.
    - `EARLY_AGB_REMNANT_BLEND` is removed. It was the first third of the engine's own EAGB span,
      and SSE's t_n spans the thermally pulsing AGB too.
    - Against SSE's published landmarks, t_n at constant mass is `tscls(14)` to 10⁻¹².
    - At the base of the AGB, Rc = R_ZHe(Mc), where core helium burning leaves it. It rises across
      the phase, to 150 R☉ for 5 M☉ and 3.3 R☉ for 8 M☉ by the phase's end.
    - The science check (against `star.f`, `hrdiag.f`, `zfuncs.f`, `evolv1/2.f` and `comenv.f`)
      found no departures.
  - _The gate, BSE §3.2's cataclysmic variable._ Both tests pass
    (`binary::tests::the_papers_cataclysmic_variable_reproduces_its_sequence`, `classify`'s
    `the_papers_cataclysmic_variable_is_one`). **At the first common envelope (78.914 Myr) τ =
    0.5579.** The giant's Rc is 0.942 R☉ against the core's lobe of 1.904 R☉ on the orbit the
    envelope leaves (a_f = 4.868 R☉), and the core would fill the lobe from τ = 0.6742. The margin
    is thin: 0.12 in τ. A primary that entered the envelope later on its early AGB would coalesce
    (a single 6 M☉ star's early AGB ends at τ = 0.76). The test logs these figures and asserts Rc <
    lobe. With the engine's own blend (the parked WIP), Rc was 69.5 R☉ there.
  - _Goldens_ (`golden_diff.py --base 97c969c`: 28 with changed values, one new, header at 20).
    - New: `stellar/early_agb_remnant`
      (`track::binary::tests::the_early_agb_remnant_at_sses_tau_is_pinned`). It pins t_n, τ, Lc and
      Rc bit for bit at three masses and metallicities, at two masses each, at five points of the
      phase. No older golden samples a thin early AGB.
    - `stellar/binary_timelines`: 961 of 1,000 digests (23 with a new segment count). Most of
      that is the digest hashing each track's full `Debug`, which now prints the two new
      `EarlyAgb` fields. With the tracks' `Debug` left out, 299 pairs move:
      - 115 by the remnant blend alone, none with a new segment count;
      - 251 by the core radius alone, all 23 segment-count changes (tides read R − Rc, the
        common envelope's coalescence test reads Rc);
      - 67 by both.
    - `stellar/summaries`: one death time, 3 × 10⁻¹⁴ relative (a star with a thin envelope on its
      early AGB), from the remnant blend.
    - The galaxy chain, at the last bits, from the remnant blend alone (measured: the core radius
      alone moves none of it). `fates::mean_present_mass` reads plan 06's tracks, so the mean
      system mass moves by about 8 × 10⁻¹⁴, and with it the system count and the population, gas,
      black-hole and cluster masses (≤ 5.5 × 10⁻¹²). Downstream steps amplify that continuously:
      up to 3.5 × 10⁻¹⁰ in `global_list/orbit` after 9,114 steps, 10⁻¹⁰ in `global_list/dwarfs`
      and 2–3 × 10⁻¹¹ in the server's expected counts and the fields' deep tails. Before the
      `Mc,BAGB` hold, τ's step had moved `galaxy_class_table` by up to 3.4 × 10⁻⁹; it is 5.5 ×
      10⁻¹² now. No line count changed outside `binary_timelines`. Moved:
      - `galaxy_params`, `galaxy_fields`, `galaxy_bounds`, `galaxy_map`, `galaxy_potential`,
        `galaxy_handle`, `galaxy_class_table`, `galaxy_velocity`;
      - `galaxy/features/{cells,centre,centre_members,members}`,
        `galaxy/global_list/{dwarfs,orbit}`;
      - `gas/{extinction,field,map,params,sightlines}`;
      - `catalogue_classes/supernova`, `placement/substellar`, `planetary/context`, `query/range`;
      - the server's `galaxy_parameters`, `systems_in_range` and `systems_in_range_briefs`.
    - So T4.h also moves plan 06's single-star output, not only the binary engine's.
  - _Two other plans' tests at their rounding floor_ (orchestrator's rulings, 2026-10-04). The
    galaxy's last-bit change tipped them; their safety checks and the code they test are unchanged:
    - `galaxy::potential::monopole::tests::the_table_holds_the_gaussians_mass_and_density`
      (plan 09). At 2 × 10⁵ ly ΔM ÷ M ≈ 3 × 10⁻¹⁰ over h = 10⁻⁴, so one ulp of ln M is about
      2 × 10⁻⁵ of noise, and the check went from 1.7 × 10⁻⁶ to 1.16 × 10⁻⁵ against 10⁻⁵. Beyond
      10⁵ ly it now takes the Richardson-extrapolated difference at h = 4 × 10⁻³ (6 × 10⁻⁷ at base
      and here), with the tolerance kept.
    - `tests/galaxy_bounds.rs` `envelopes_never_exceed_their_bounds` (plan 02's P02.T8.a).
      Changes are in the tightness half only:
      - The normal range allows 4 |ln corner| ε beyond 10⁻¹²: a k-ulp disagreement in the exponent
        is k |x| ε relative, and `BOUND_MARGIN` leaves 0.9 × 10⁻¹³. It reached 1.08 × 10⁻¹² at
        ln corner = −634.
      - The subnormal range allows a disc 2⌈n0⌉ units beyond its 64: each path rounds the
        subnormal factor once before × n0. It reached 81 units for the nuclear disc, n0 = 105, at
        3.5 × 10⁻³¹¹.
      - Pointers are in plans 02's and 09's Risks.
  - _Tests added_ (`agb.rs`):
    - `the_nuclear_end_is_sses`: SSE's landmarks; t_n falls with the mass; the pulses' relation
      reaches max(Mt, `Mc,BAGB`) at t_n; no step at `Mc,BAGB`.
    - `a_supernova_early_agb_ends_at_its_nuclear_end`: 10–25 M☉ reach τ = 3 at the end; for the
      floored 60 M☉ at Z ≤ 10⁻³, t_n comes after the end; from 100 M☉, τ ≡ 0.
  - _Tests added_ (`track/binary.rs`): `the_early_agb_core_radius_is_the_helium_stars_at_sses_tau`.
    The remnant a hair above the core has Rc's radius, Rc = R_ZHe at the base, Rc rises, and it
    stays under the bound.
  - _Invariant._ `check_no_bare_giant` brackets a frozen EAGB member's Rc by the helium giant's at
    its brightest (`early_agb_core_radius_bound`). The held state keeps no τ to check exactly. The
    bound assumes the carbon–oxygen core inside the helium core, which the 1.05 floor breaks at
    60 M☉ and Z ≤ 10⁻³ at constant mass. No held star has met that case.
  - _Slow suites_ (run capped, all pass):
    - `binary_system`: ruling 137's hydrogen-poor share of core collapses 0.511 (0.511 at T4.g);
      123.5's marked-stripped 1,408 of 1,753 (80.3%) and merged 294 (16.8%).
    - `binary_carve` passes, and so do the 10³-pair invariants.
    - The R06 census passes in 197 s.
- **P11's protostar mergers and build-age dependence, fixed** (Phase J lane, 2026-10-05; rendering
  plan R06's tables, `decision-r06-tables.md`, "For plan 11"). Built for version 21, committed with
  its goldens re-blessed at version 20 and held out of integration until the 20 → 21 bump lands
  with it, P11.T4.h and P14.T47.e.
  - _The protostar mergers._ `evolve.rs`'s `arrival`, where the engine starts stepping, read each
    star's main-sequence start from its track's built segments only. A pair run to an age before a
    star's arrival had none for that star and was stepped from age zero, where the protostars
    (about 10⁻³ M☉ each at the onset) overfill a close orbit: it merged them at once into a
    0.01 M☉ star on the cooling fits (their floor), M_V 17.53, beside a `NoRemnant`. Run to a later
    age, the same pair read back as two protostars.
    - The fix keeps the convention (no star interacts before both have arrived) and makes the
      arrival independent of the build: `Track::main_sequence_arrival` (`sse/track.rs`) is
      `main_sequence_start` where the main sequence is built, and otherwise the same age from the
      build's own law (`phases::main_sequence_start_age`), bit for bit
      (`a_track_built_short_of_its_main_sequence_knows_its_arrival`).
    - The pre-test (`can_interact`) is now false before both stars have arrived, where a star's
      largest radius is its contracting one, so such a pair is not run at all.
    - _Physics._ A convention, as in the codes plan 11 follows. BSE starts both stars on the
      zero-age main sequence (Hurley, Tout and Pols 2002, §2.8), as COMPAS does, and the drawn
      orbits are those observed about main-sequence primaries, a zero-age population (Raghavan et
      al. 2010; Duchêne and Kraus 2013; Sana et al. 2012; Moe and Di Stefano 2017, §2). What the
      pre-main sequence does to a pair is already in them: close pairs form wider and are brought
      in then (Bate, Bonnell and Bromm 2002; Moe and Kratter 2018), and those that merge while
      embedded (Stahler 2010; Tokovinin and Moe 2020) are counted as single stars, so merging the
      drawn pairs whose contracting stars overfill their orbits would count those mergers twice.
      Before the arrival the stars are shown on the drawn orbit, overlapping it where they
      overfill it, and the drawn orbit holds their final masses while they accrete.
    - _Rates_ ([Fe/H] −0.5, the fit's sampler, 2,000 systems a bin; systems holding a 0.010 M☉
      star): before, C bins 0–8: 6, 15, 25, 23, 14, 9, 3, 8, 4; D bins 0–6: 13, 29, 43, 28, 7, 3,
      2; E bins 0, 2, 4: 0, 6, 2. After: none. The repro systems (D cell 80, [Fe/H] 0:
      0x621459680000005c, …90, …91) are protostars, each its own model's.
  - _The build-age dependence._ `evolve(input, u₁)` and `evolve(input, u₂ > u₁)` now agree bit
    for bit at every age to u₁ for every pair the pre-test passes at u₁. _(Strengthened by
    P11.T4.j:)_ a pair the pre-test passes over at u₁ shows no interaction before u₁ in a run to
    any later age (stable transfer, a common envelope, contact or a merger, before the pair's
    first supernova; a passed-over pair's supernova is P11.T10's, finding F3). Four causes, all
    fixed:
    1. The arrival above. It also made whole histories differ for massive primaries whose
       companion's main sequence was built in one run and not the other.
    2. The last step was cut at the run-to age and joined to its knot linearly, where a later
       run's whole step was. Now no step depends on the run-to age (`StepLimit::new` takes none):
       the step that reaches or passes it is the last, and what it lands on lies beyond the
       timeline (`integrate`, `transfer_phase`); the contact phase's knots run to its
       coalescence. Every step is held to `LONGEST_STEP_YEARS`, 10⁹ yr, which the run-to age
       once bounded (two white dwarfs 1 au apart would otherwise step 10¹⁷ yr).
    3. The orbit before the engine's start lay on a path from age zero to the first step, and a
       star below 0.1 M☉'s mass path likewise. Now the drawn orbit stands to the start
       (`OrbitPath::fixed_until`) and the engine's paths begin there.
    4. A track the engine rebuilds (`after_boundary`'s gap, `main_sequence_star`) was built to a
       reach guessed from `main_sequence_start` + `main_sequence_lifetime`, which at 3–8 M☉ falls
       short of the track's own gap by 1.6 × 10⁴–1.8 × 10⁵ yr (0.15% of the main sequence; a
       finding for plan 06). A run whose age fell in that shortfall either stopped on the track's
       last boundary 4,096 times, to its cap, its stars frozen, or, where the gap was not built at
       all, placed the star at its track's age zero, a protostar beside its companion's remnant
       (the determinism audit's sweep: 42 of 2,317 runs to just past an event, in 8 of 60 pairs).
       A later run went on. Now `track_reaching` builds again, past the track's end while the
       phase sought is not in it, then from the placement.
    5. A track built to end within the look-ahead resolution of `phase_ahead` (4 ε of the age)
       past the run-to age had no next segment for a step starting there, and would stall to the
       cap at a handful of representable ages, which the cut at the run-to age had masked. Every
       track the engine reads is now built 16 ε of the age past it (`reach_margin_years`,
       `Engine::reach_span_years`), which moves no state.
    - _Measured after events_ (`a_hundred_timelines_run_to_just_past_their_events_are_the_same`):
      100 pinned-sample pairs, each run to 10⁻³, 1, 10², 10⁴ and 10⁶ yr past every segment start
      of its run to 1.5 × 10¹⁰ yr: all 3,461 runs the pre-test passes agree with it bit for bit.
    - _Measured_ (4,000 sampled close pairs, each run to an age log-uniform in 10⁵–1.6 × 10¹⁰
      yr and to up to four times it): before, 1,335 pairs had stars that differed before the
      earlier age and 1,461 differed among those the pre-test passes there; after, none of the
      latter. 15 with differing stars and 259 with differing orbits remain, all passed over by
      the pre-test at the earlier age (below).
    - _At the system level_ (the R06 probe, `probe_v20.rs::causality`: a fit system rebuilt
      with its bin's upper age and read back; [Fe/H] −0.5, 400 systems a bin): before, C bin 4
      3, C bin 19 0, C bin 10 0, D bin 3 3, D bin 16 5, E bin 10 30; after 0, 0, 0, 0, 4, 14.
      What remains is not build-age dependence: a record rebuilt with another age at the epoch
      is another system. Its carve (design note 8) is judged at its epoch age, so it may take
      another attempt and other companions (E bin 10: 11), and a supernova places its stars on
      their orbit through the epoch age (BSE appendix A1; D bin 16: 4, E bin 10: 1). Two in E
      bin 10 are the pre-test's blind spot. Further bins: C 15, 22, 24: 0, 1, 0; D 10, 20, 23:
      0, 6, 5; E 6, 12, 14: 0, 24, 45, all carve or supernova phase.
  - _Ruled (decision-p11-channels, 2026-10-06): T4.i and T4.j._ The two items it ruled on:
    - **The pre-test's blind spot** (design note 7). `can_interact` reads the drawn orbit, so a
      pair whose orbit magnetic braking (BSE eq. 50), tides or gravitational radiation shrink into
      contact before the run-to age, without its stars reaching the drawn orbit's lobes, is passed
      over: two single stars on the drawn orbit. A run to a later age, which the pre-test passes,
      shows the contact before the earlier age. 4 of 10³ pinned-sample pairs
      (`a_thousand_timelines_are_the_same_whatever_age_they_are_run_to`), e.g. 1.04 + 0.45 M☉ at
      a = 3.75 R☉ (P = 0.69 d), in contact by braking at 2.25 Gyr; the sample is weighted against
      this channel (11% of its primaries are Sun-like, 14% of its periods under 1.3 d, 21% of its
      ages over 1 Gyr). In the field it is BSE's whole W UMa channel (Stępień 2006): every
      convective-envelope pair under P ≈ 1.2–1.5 d reaches contact in the engine within 10 Gyr
      (science check, from eq. 50 at locked spin), and while its primary is on its main sequence
      the pre-test skips it. A likely, perhaps the main, cause of the contact binaries' tenfold
      deficit against Rucinski (P11.T11's Risks). Pairs passed over also keep the drawn orbit
      where the engine would widen it by wind or circularise it (to Δa/a ≈ 1 and Δe ≈ 0.08 in the
      sample). Options:
      - (a) A decay-aware pre-test (the science check's): a_min(t) = [a₀⁵ − 5 (C_mb + C_gw a₀)
        t]^⅕ from eq. 50 at locked spin with R = R_max(until) and the zero-age envelope, braking
        only above 0.35 M☉, eq. 48 for gravitational radiation, a (1 − 3 J_spin ÷ J_orb)⁻¹ spin
        reservoir and a tidal spin-up allowance; the pair interacts if a_min (1 − e) is within
        the lobe test. For 1 + 0.8 M☉ it passes P₀ ≤ 0.64 d at 1 Gyr and ≤ 1.22 d at 10 Gyr, a
        small extra cost. My lean.
      - (b) Run every pair under a fixed period (about 2 d), at any age: simpler, costlier.
      - (c) Keep it and record it.
    - **Both arrivals** (P06.T15.b's convention, kept here). The engine waits for the later
      star's arrival, so a massive primary whose low-mass companion arrives after the primary's
      main sequence never interacts: no common envelope on its giant branch, and its supernova
      leaves the pair on the drawn orbit. A companion arrives too late below 0.29, 0.62, 0.99,
      1.19, 1.58, 1.81 and 2.25 M☉ for primaries of 4, 5, 8, 10, 15, 20 and 40 M☉ (Z = 0.02;
      science check). That is the classic progenitor space of neutron-star and black-hole
      low-mass X-ray binaries through a common envelope (Kalogera and Webbink 1998), and of
      post-common-envelope white dwarfs with M dwarfs from 4–7 M☉ primaries, so the channel's
      yield is zero; and such a pair is carried bound through a supernova that should unbind it.
      Measured: 42 of 1,846 sampled pairs that can interact in their primary's life (26 of 777
      from 8 M☉) have a companion arriving after 90% of it. The science check rates it must-fix,
      pending the owner. Its recommendation, BSE-faithful: start at the first arrival, and carry
      a star that has not arrived as HPT's zero-age main-sequence star of its mass, its clock held
      at τ = 0 until its own arrival, for every interaction test (Roche lobe, collision, common
      envelope, the supernova's orbit, tides, braking), shown on its own pre-main-sequence track
      until the pair touches it (Hurley, Tout and Pols 2002, §2.8; Kalogera and Webbink 1998,
      §1; Moe and Di Stefano 2015 observe B stars with pre-main-sequence companions at 3–8.5 d).
      It moves many massive pairs' histories, the carve and the classes.
  - _Tests added._ `binary::tests`:
    - `a_pair_run_short_of_the_main_sequence_stays_two_protostars` (2.27 + 2.21 M☉ at 1 d, to
      0.4 Myr: one detached segment, two protostars, each its own track's, the drawn orbit,
      equal to a run to 10⁸ yr; it failed before with the merger at zero age);
    - `a_track_built_short_of_the_pairs_age_is_built_again` (8.26 + 7.92 M☉ at 2.31 d to
      37.085 Myr: no cap, equal to a run to 90 Myr, and so are runs to just past each of its
      events to 40 Myr; it capped before, and placed the primary at its track's age zero just
      past its main sequence's end);
    - `a_rebuilt_track_reaches_the_pairs_age` (`track_reaching` at both call sites' placements,
      for a 5 M☉ track whose gap the guess falls short of);
    - `a_timeline_is_the_same_whatever_age_it_is_run_to` (60 pairs) and, slow,
      `a_thousand_timelines_are_the_same_whatever_age_they_are_run_to` (10³: 372 run at both
      ages, bit for bit; 624 passed over, whose stars on their own tracks agree to the later
      run's first event; 4 missed interactions, under the 2% the test allows in this sample);
    - `a_timeline_run_to_just_past_an_event_is_the_same` (8 pairs) and, slow,
      `a_hundred_timelines_run_to_just_past_their_events_are_the_same` (above).
    - `sse::track::tests::a_track_built_short_of_its_main_sequence_knows_its_arrival`.
    - `tests/stellar_system.rs` `young_pairs_whose_protostars_overfill_their_orbits_stay_protostars`
      (the three repro systems).
  - _Goldens_ (`golden_diff.py --base 1845d20`: two moved, none new, header at 20).
    - `stellar/binary_timelines`: all 1,000 digests, no segment count. The digest hashes each
      segment's paths' `Debug`, which now carries the drawn orbit's `FixedOrbit` and paths that
      begin at the engine's start, not at zero, and run past the run-to age. Compared by their
      states alone at the digest's 257 even ages (698 pairs' stars move there):
      - before both arrivals the orbit is the drawn one (it was reconstructed from the axis at
        the protostars' masses: up to the whole period at age zero);
      - in the first engine step the orbit is interpolated from the start (about 10⁻¹² of a);
      - at 1.2 × 10¹⁰ yr, in the last step, uncut, stars move by up to 2 × 10⁻⁴ (the
        interpolation across the step), and elsewhere mostly by units in the last place, where
        the 10⁹-yr cap or the uncut step moved the knots of a long segment (224 sampled states by
        more than 10⁻⁶);
      - two double black holes (pairs 309 and 604) merge by gravitational radiation 0.05–0.95%
        earlier, their only moved segment boundaries: the cap gives their long inspirals more
        steps, nearer Peters's time (309: 8.977 → 8.892 Gyr, Peters 8.76–8.78 Gyr; 604: 9.067 →
        9.062 Gyr, Peters 9.026 Gyr). No supernova moved.
    - `stellar/summaries`: two black holes of one system (mass, core, R and remnant mass) by one
      unit in the last place, from the uncut last step.
    - Nothing else moved: plan 06's single stars, the galaxy chain and the planetary and server
      goldens read no engine state these changes touch.
  - _Slow suites_ (run capped, all pass): `binary_system` (ruling 137's hydrogen-poor share
    0.511; 123.5's marked-stripped 1,408 of 1,753, merged 294; contact pairs 1.13 × 10⁻⁴ per
    faint main-sequence star, unchanged); `binary_carve`; the 10³-pair invariants; the two new
    slow tests; the R06 census (`a_hundred_thousand_systems_live_and_die_in_order`, 234 s).
  - _Reviews._ Science check: the citations hold (HTP 2002 §2.8; Moe and Di Stefano 2017 §2;
    Bate, Bonnell and Bromm 2002; Moe and Kratter 2018), the convention is the defensible choice
    against merging, worded as a convention; fixed: the test's arrival times (5.1 and 5.5 Myr,
    not 7) and the step cap's doc; the two findings above are its. Determinism audit: the
    fallback placement at track age zero and the look-ahead stall (both fixed above), and a test
    of the windows after events (added); no streams, draws, caches or summation orders. Rust
    review: units in the new names (`Years` for the arrival and the fixed orbit's age), one
    `FixedOrbit` for the drawn orbit and its age, the fallback placement, summaries, and the
    contact phase's end for a contact that never coalesces; all fixed.
- **P11.T4.i as built** (Phase J lane, 2026-10-06; ruling p11-channels, §2). The engine starts at
  the first arrival on the main sequence. **Built for version 21, committed with its goldens
  re-blessed at version 20 and held out of integration** until the 20 → 21 bump lands with it,
  P11.T4.h, 9a0950e's fixes and P14.T47.e.
  - _The rule, as built._
    - `evolve.rs`: `arrival` is the least of the members' `Track::main_sequence_arrival`, no later
      than `until`. `interacts` reads each star's largest radius at the held age below, which is a
      late star's zero-age radius. `own_members_with` builds a track (or rebuilds a given one) to
      max(until, `sse::main_sequence_start`) plus `reach_margin_years` of that.
    - `star.rs`: `arrival_ahead_years` and `engine_track_age_years`, the track age held to no less
      than the arrival. `Member::evaluate` reads at that age, and so does `Member::radius` through
      it, so the proxy is bit for bit `own_structure_at(arrival)` at τ = 0. `Member::state_at` is
      unchanged.
    - `rlof.rs` `carry` reads the held age: a `Track` member before its arrival becomes
      `MainSequence { mass, tau: 0 }`.
    - `common_envelope.rs` `main_sequence_left`: a `Track` member before its arrival has its
      whole main-sequence segment left, from its arrival, not the time to it. A contact reads it,
      but every interaction `carry`s its members first, so no run reaches this arm; a unit test
      pins it.
    - Reviewed and unchanged:
      - `phase_ahead`'s boundary at the arrival: a segment ends there, and the shown star switches
        to its main sequence;
      - the detached step's phase share, which is the proxy's kind over the contraction's span;
      - `Engine::new`'s spins, which read the proxy's structure at the start, so a late star
        starts at HPT's zero-age spin (eqs. 107–108) at its zero-age radius;
      - `supernova.rs`, which reads the pair's masses only;
      - `stripped_member`, which no pre-arrival member reaches.
    - **Deviation (beyond the ruling's list):** `Member::mass_at` reads the held age too. The
      contraction keeps the final mass, so the bits are the same as before. It is held so that the
      engine's masses and a carried star's mass agree by construction.
    - **Known, negligible:** a held member's zero-age wind (`detached.rs` `rates`) still takes
      orbital and spin angular momentum while its mass is held. It is ≲ 10⁻¹⁰ M☉ yr⁻¹, over at
      most the companion's arrival.
    - `galaxy/displaced/binarity.rs` `largest_radius` reads a companion at max(age, its arrival),
      on a track built to its main sequence's start. Only `interacting_periastron` and
      `interacting_share` read it. The stripping table reads the primary's radii only:
      `just fit stripping` writes "body unchanged", and `just fit-check` passes (12 fresh, 0
      failures).
    - `sse::main_sequence_start` equals `Track::main_sequence_arrival` bit for bit (now asserted in
      `a_track_built_short_of_its_main_sequence_knows_its_arrival`): a late star's track is always
      built past the age the proxy reads.
  - _Tests_ (`stellar/binary/tests.rs`; the first five fail on the old engine):
    - `a_late_star_is_its_zero_age_self_to_the_engine`: a 1 M☉ star from 0.5 Myr to its arrival.
      `evaluate`, `radius` and `mass_at` are its structure at the arrival, bit for bit, and
      `state_at` is the track's pre-main-sequence state.
    - `a_late_star_is_carried_at_zero_age`, split from the above for Clippy's line limit.
      `main_sequence_left` is the whole main-sequence segment. `carry` at 13.5 Myr gives a
      `MainSequence` at τ = 0 of the track's mass. The start is the 15 M☉ primary's arrival.
    - `the_engine_starts_at_the_first_arrival`: 5 + 1 M☉, arrivals 0.81 and 37.1 Myr.
      - `arrival` is the minimum.
      - At 1.6 Myr the companion is 2.09 R☉ contracting, against 0.89 R☉ at zero age. At 7.6 R☉,
        which only its contracting radius would reach, the pair is passed over.
      - At 0.95 of the companion's zero-age reach (3.35 R☉), inside the primary's reach too, the
        pair is stepped from the first arrival and merges in its first steps.
    - `a_late_companion_meets_its_primarys_envelope`: 15 + 1 M☉ at 1,200 R☉.
      - A common envelope comes before plan 06's pinned death at 14.32 Myr, and the collapse is
        recorded there.
      - The companion, arriving at 37 Myr, is shown before the main sequence until the envelope.
      - From the age the pre-test first passes to the envelope, runs to three ages between the
        arrivals equal the run to 50 Myr, bit for bit (the determinism audit's direct case).
    - `a_late_companions_supernova_is_applied`: 20 + 1 M☉ at 1,200 R☉, with and without kicks.
      - The envelope ejects at 8.89 Myr, to 4.0 R☉.
      - The collapse at 9.87 Myr, to a 4.61 M☉ black hole, is recorded and applied.
      - Without a kick, Blaauw's criterion (BSE A1 with v_k = 0, r = a) gives bound, and the
        collapse does lose mass.
      - **Deviation:** the ruling's "wide orbit the primary's giant reaches" is taken at an orbit
        the giant does fill, about half its drawn reach. At 0.9 of that reach the wind widens the
        orbit faster than the star grows, the pair is never touched, and the engine records no
        collapse at all (finding F7 below). So the test cannot hold there.
    - Build-age suites:
      - `any_age_over` adds an age uniform between the arrivals for every pair with two, drawn on
        a stream of its own, so the samples are unchanged. It counts the first ages and the
        between-arrivals runs apart, and keeps the 2% bound over the first ages.
      - `check_after_events` adds the first arrival plus 10⁻³, 1, 10², 10⁴ and 10⁶ yr, and
        halfway.
      - None of the fast suites' pairs passes the pre-test between its arrivals; the slow ones do
        (below).
    - `a_rejuvenated_accretor_leaves_its_main_sequence_once`: pair 0077 now leaves at 1,052.889
      Myr (1,052.655 before). It is stepped from the primary's arrival, 7 Myr before the
      companion's, and its knots move. Re-pinned.
    - Kept, unchanged in form:
      - `a_pair_run_short_of_the_main_sequence_stays_two_protostars`;
      - `young_pairs_whose_protostars_overfill_their_orbits_stay_protostars` (doc reworded);
      - the BSE reference binaries;
      - `the_threshold_is_can_interacts_boundary`.
  - _Goldens_ (`golden_diff.py --base 3a52ca8`: two moved, none new, header at 20).
    - `stellar/binary_timelines`: 655 of 1,000 digests, every run pair with two different
      arrivals. 595 have one more segment, the detached boundary at the later arrival.
    - Compared by segment kinds, boundaries and supernovae, with the later arrival's split merged
      back (`.git/rm23-scratch/p14j/t4i/compare_phys3.py`):
      - 345 identical, and no passed-over pair moved.
      - 59 pairs (5.9%) whose first interaction or supernova now comes before the later arrival,
        the channel T4.i opens: 15 with a new sequence of kinds, 4 with a new segment count, and
        40 with their boundaries moved (39 by more than 1%).
      - 596 others move through the earlier start alone: tides, winds and braking act on the
        proxy from the first arrival. Massive pairs' arrivals are 0.1–3 Myr apart; a low-mass
        companion's are up to 500 Myr. Of these:
        - 22 change their kinds, mostly a supernova's bound or unbound outcome flipping with the
          orbital phase at the explosion, which a 10⁻⁶ change in the orbit moves (pairs 0010 and
          0031);
        - 28 change their segment count and 2 their supernova outcome;
        - boundaries move by ≥ 1% in 40 (e.g. pair 0036: transfer at 77.1 Myr, not 81.2),
          10⁻⁴–10⁻² in 121, 10⁻⁶–10⁻⁴ in 184, and less in 199.
      - **Against the ruling.** It expected "only pairs whose interaction or supernova falls
        before the late arrival, a few per cent". Those are the 59. The rest's moves come from the
        earlier start's evolution: larger than expected, but none is a new channel.
    - `stellar/summaries`: one system, C 0x42002cb200000000. Its 1.56 M☉ primary is a
      carbon–oxygen white dwarf at the epoch, whose mass moved by 2.5 × 10⁻⁶ through its pair's
      earlier start.
    - Nothing else moved: not the galaxy chain, nor the planetary or server goldens.
  - _Statistics._ Slow and capped.
    - The channels probe is the ruling's `zz_p11ch.rs` `channels`, adapted to the committed
      engine and given a counter of each late pair's primary supernova
      (`.git/rm23-scratch/p14j/t4i/zz_p11ch_t4i.rs`). It was run on 3a52ca8's engine and on
      T4.i's. It reproduces the ruling's T4.i column (`channels_q2.log`) class for class in
      layers C, D and E. Against the ruling's expectations:
      - **Fake LMXBs.** Layer-E late pairs reaching a Roche-lobe LMXB with no primary supernova
        applied: 137 → 6. Symbiotic: 264 → 34. The rest are F7 and F8.
      - **Real common-envelope LMXBs.** Late pairs reaching one with the supernova applied:
        Roche-lobe 10 → 45, symbiotic 10 → 23. By class, the late pairs' Roche-lobe LMXBs are
        NS/BH 16/67 and the symbiotic ones 12/46 (the ruling's 83 and 58).
      - **The bound share after the primary's supernova:** 2,300 of 4,764 (48.3%) → 2,360 of
        5,457 (43.2%), as ruled. For late pairs, 46 supernovae (11 bound) → 741 (53).
      - **Layer D:** dwarf novae 365 → 394 (+8%), nova-likes 11 → 23 (×2.1), magnetic 165 → 177,
        AM CVn 134 → 138. Of its 391 late run pairs, 79 → 349 interact. Layer C does not move.
      - **Double neutron stars:** `binary_classes` has 112 bound in 12,000 (114 before; the
        window is 6–144), 64% at e < 0.3, and 23 merging, about 26 Myr⁻¹ by VG18's yield against
        Pol et al.'s 28–72.
    - Recycled pulsars' medians (`binary_classes`):
      - beside a carbon–oxygen or oxygen–neon white dwarf: 242, at 27.7 ms and 6.70 × 10⁹ G;
      - beside a neutron star: 176, at 70.3 ms and 1.99 × 10¹⁰ G;
      - beside a helium white dwarf: 42, at 8.7 ms and 1.64 × 10⁹ G.
    - `binary_system`:
      - ruling 123.5's marked primaries: stripped 1,407 of 1,752 (80.3%), merged 300 (17.1%);
      - the merger band merged 815 of 986 (82.7%);
      - ruling 137's hydrogen-poor share 0.506 of 5,116 core collapses;
      - contact pairs 1.13 × 10⁻⁴ per faint main-sequence star, unchanged (that move is T4.j's).
    - `binary_carve`:
      - redrawn: C 20, D 137, E 41 (19, 149, 37 before), within Poisson;
      - pairs through the engine: C 3,268, D 11,749, E 8,554.
    - The 10³-pair invariants pass.
    - `a_thousand_timelines_are_the_same_whatever_age_they_are_run_to`:
      - first ages: 376 run at both ages, 620 passed over, 4 missed (372, 624 and 4 before);
      - between the arrivals: 34 run at both ages, 678 passed over, 9 missed. All 9 are pairs
        under 1.4 d that braking or tides now bring into contact or a merger from the first
        arrival: the drawn-orbit pre-test's blind spot, which T4.j removes.
    - `a_hundred_timelines_run_to_just_past_their_events_are_the_same`: 3,504 runs compared just
      past events, and 13 between the arrivals.
    - The late thresholds (`late_thresholds`) are the ruling's table to three figures.
    - The R06 census passes in 188 s under the heavy lock (234 s at 9a0950e, unlocked).
  - _Gate._
    - fmt and Clippy pass, over the workspace.
    - The capped workspace bless under `just _locked` passes (3,651 tests), and so does the
      non-bless gate (3,651 passed, 704 s).
    - `cargo nextest run -p hyperion-sim -E 'test(binary::)'` passes, as do the slow binary
      suites above.
  - _Reviews._
    - Determinism audit: no must-fix. Applied:
      - the direct between-arrivals comparison;
      - the separate counts, with the 2% bound kept over the first ages;
      - `sse::main_sequence_start` pinned to the arrival;
      - the stale "both arrivals" comments.
    - Rust review: applied the units in the helpers' names (`_years`), the stale comments, the
      `main_sequence_left` test, intra-doc links, `lobe_fraction`'s unit, and the tests' messages
      and claims.
    - Science check:
      - It confirmed the citations (HTP 2002 §2.8; COMPAS, Riley et al. 2022 §3.2; SEVN, Iorio et
        al. 2023 §2.1; Dunham et al. 2014), the thresholds, and the Blaauw test's criterion and
        mass accounting.
      - Fixed: `arrival`'s doc wrongly said only a late star's zero-age overfill interacts between
        the arrivals. The thresholds' metallicity, [Fe/H] 0, is now given, and the 20 M☉ one is
        marked as resting on the arrival law's Kelvin–Helmholtz extension (nearer 2 M☉ in MIST).
      - **The late companion's true radius** as its primary leaves the main sequence is 1.0–4.5
        times its zero-age radius (Baraffe et al. 2015 against Tout et al. 1996): 1.2–2 for the
        0.5–1.4 M☉ companions of 10–20 M☉ primaries, and up to 3.4 for 0.1–0.4 M☉ beside 15–20 M☉.
        The ruling's "1.3–2" holds only for the LMXB progenitors. The doc carries the measured
        range: the proxy understates coalescence at a common envelope's exit most for the
        lightest companions. For the orchestrator.
  - _Findings, not this task's (for the orchestrator; the science check confirms both readings;
    both fixed by P11.T4.k, rulings p11-supernova-pins and p11-t4k-faults):_
    - **F7. An untouched pinned primary collapses without a record.**
      - A primary pinned to plan 06's collapse (design note 16) that is still on its own track at
        its death gets no supernova record, and its collapse does not act on the orbit.
      - The step lands on the death exactly, and `Stop::Pinned` wins there. `pinned_collapse` then
        reads the member's structure at the pin, which is the track's remnant, and takes it as
        nothing living. `current(0)` is the remnant's mass there too.
      - E.g. 17 + 12 M☉ at 3,300 R☉ and 25 + 20 M☉ at 3,500 R☉, circular at [Fe/H] 0 and median
        draws, ride their primary's collapse on the pre-collapse orbit. Kick-free, A1 would widen
        the first ×7 or unbind it, and a kick nearly always disrupts it.
      - It affects every run pair whose primary is untouched at its death, late or not.
      - Likely fix: read the living check and the mass from the last living state, as `die`
        does. It moves output: the bound shares, the HMXBs, "bound NS/BH + MS".
    - **F8. A pinned primary stripped just before its pin is held as its remnant.** At 20 + 1 M☉
      and 1,680 R☉, the common envelope at 9.8665 Myr (the pin is at 9.8701 Myr) leaves a
      `Frozen` member shown as a 3.93 M☉ black hole. No collapse is recorded.
      `Remains::Collapse`'s core is documented as the last living state. The hold or the strip
      reads a remnant there.
- **P11.T4.j as built** (Phase J lane, 2026-10-06; ruling p11-channels, §3). The pre-test bounds
  the engine's own orbital decay. **Built for version 21, committed with its goldens re-blessed at
  version 20 and held out of integration** until the 20 → 21 bump lands with it, P11.T4.h,
  9a0950e's fixes, P14.T47.e and P11.T4.i.
  - _The rule, as built._
    - `evolve.rs` `interacts`: nothing before the first arrival; then the lobe test on the drawn
      orbit (`reaches_lobe`, over `largest_radii_rsun`; the same bits as before), and otherwise
      `detached.rs` `decay_reaches(input, members, start_years, until_years, radii_rsun)`, the
      ruling's items 1–7. The constants: `DECAY_PIECES` = 4 and `DECAY_SAFETY` = 1.25 (the
      ruling), with the engine's own `MAGNETIC_BRAKING`, `MAGNETIC_BRAKING_FLOOR`,
      `GRAVITATIONAL_WAVE_RATE`, k′₂ and k′₃. Hut's f₂ and f₅ are now shared with the tide
      (`hut_f2`, `hut_f5`, the same expressions, the same bits).
    - The switches: braking needs `magnetic_braking` and `tides` (the orbit feels braking only
      through the tides), the spins' reservoir `tides`, W `gravitational_radiation`.
    - F1's label (`supernova.rs` `Engine::die`): an orbit a non-sudden death's `lose_mass` unbinds,
      both stars left, is `Disrupted { by }`. The docs of `SegmentKind::Disrupted` and
      `quiet_kind` say so. The physics (instantaneous loss at a white dwarf's birth, ruling
      129.4a) is unchanged, for its own ruling.
    - `binarity.rs`: `interacting_periastron` and `interacting_share` are documented as the lobe
      part of `can_interact`'s test; `the_threshold_is_can_interacts_boundary` asserts "just
      inside passes `can_interact` and the lobe test", "just outside fails the lobe test"
      (`lobe_reached`, `#[cfg(test)] pub(crate)`). The stripping table does not move (it reads
      the primary's radii only).
    - The test hook: `evolve_past_the_pre_test` (`#[cfg(test)]`), through `run_pair`'s
      `PreTest::Past`.
    - The build-age suites' "missed interaction" is now the ruling's (`interaction_before`): a
      stable transfer, common envelope, contact or merger leaving one star, before the age and
      before the pair's first supernova (F3). Their allowances are zero.
  - _Deviations from the ruling's text and its prototype (`probe-engine.patch`)._
    - **The pieces are fixed to the track's segments.** Each segment is cut into 4 equal pieces
      whatever the span, then clipped to it, where the prototype cut [t₀, u] at the segment
      starts. So the braking integral only grows with u. The passes differ by 4 of 18,000
      (sample 0: 2,262 against 2,258); samples 1 and 2 are the probe's to the pair.
    - **The core term.** The ruling's "k′₃ Mc Rc² at their largest" is read where each segment
      opens in the span and at u (`largest_core`), and the total is capped at k′₃ m R² (Rc ≤ R).
      Reading it so needs no more: I ≤ k′₂ m R² + Mc (k′₃ Rc² − k′₂ R²), so the core adds nothing
      while Rc ≤ √(k′₂ ÷ k′₃) R ≈ 0.69 R, a giant's whole life (science check: holds). The
      prototype left the term out.
    - **The coarse filter in two steps**, for its cost on giants: first with I ≤ k′₃ m R² for a
      star past its main sequence (no structure read), then with the core read, then the braking
      integral. Off the main sequence a piece reads no structure (share 1); on it, the structure
      at its start. Built at first with the core read before any coarse step, and with a
      structure read on every piece, the giants' sample cost about three times more (unlocked);
      now 12.5 µs a pair under the lock (below).
    - `decay_reaches` takes the lobe test's radii, computed once.
  - _Tests_ (`stellar/binary/tests.rs` unless said):
    - `the_decay_bound_never_passes_over_an_interaction` (slow): the ruling's three samples
      (`bound_conservative`'s, rebuilt as `decay_sample`), 6,000 pairs each, against
      `evolve_past_the_pre_test`. No miss. Also asserted: no `Merged` segment with both stars
      present, and every pass at the pair's age passes at 1.6 × 10¹⁰ yr.

      | Sample                       | lobe test passes | engine interacts | lobe test misses | pre-test passes | misses | passes without interaction |
      | ---------------------------- | ---------------- | ---------------- | ---------------- | --------------- | ------ | -------------------------- |
      | 0, braking-enriched          | 1,190            | 1,613            | 530              | 2,262           | **0**  | 542                        |
      | 1, the lane's sampler        | 2,449            | 2,347            | 36               | 2,576           | **0**  | 91                         |
      | 2, giants near the threshold | 1,152            | 501              | 160              | 2,761           | **0**  | 1,449                      |

      The ruling's figures (first-arrival column) to the pair, but for sample 0's 4 extra passes.

    - `a_thousand_timelines_are_the_same_whatever_age_they_are_run_to` (slow): first ages 392 run
      at both, 608 passed over, **0 missed** (376, 620, 4 at T4.i); between the arrivals 60, 661,
      **0** (34, 678, 9). The 13 misses are gone.
      `a_timeline_is_the_same_whatever_age_it_is_run_to` (60 pairs) asserts zero too.
    - `a_hundred_timelines_run_to_just_past_their_events_are_the_same` (slow): 3,763 runs just
      past events and 30 between the arrivals agree.
    - `a_braked_pair_is_run_before_its_contact`: 1.04 + 0.45 M☉ at a = 3.75 R☉ (P = 0.689 d),
      [Fe/H] −0.7 (Z = 0.004), median draws: inside its lobes on the drawn orbit, passed at
      2.0 Gyr; transfer from the primary at 2.158 Gyr, contact at 2.216 Gyr, a ≈ 2.3 R☉ before
      it; a run to 2.0 Gyr is the run to 3 Gyr there, bit for bit. (At Z = 0.02: 2.631 and
      2.719 Gyr.)
    - `the_pre_test_only_widens_with_age`: the 60 pairs and four pinned massive wide pairs at 48
      ages and their own: a pass stays a pass; `can_interact` equals `can_interact_with_tracks` on
      full tracks and on tracks built to 1.5 × 10¹⁰ yr (the determinism audit's check).
    - `a_wide_pair_is_still_passed_over`: 1 + 0.8 M☉ at 10⁴ d fails at 13.8 Gyr. The bound's P₀
      boundary for a circular 1 + 0.8 M☉ pair ([Fe/H] 0, median draws) is **0.732 d at 1 Gyr and
      1.537 d at 10 Gyr**, against the science check's 0.64 and 1.22 d at locked spin alone (its
      own closed form with the reservoir and 1.25: 0.71 and 1.40 d); the lobe test alone, 0.295
      and 0.544 d.
    - `the_bound_follows_the_engines_switches`: the braked pair fails without braking or tides; a
      0.3 + 0.3 M☉ pair at 0.2 d (below the braking floor) passes at 13.8 Gyr by gravitational
      radiation alone, merges at about 3.6 Gyr in the engine, and fails without it.
    - `a_giants_tidal_capture_is_run`: 2 + 0.2 M☉ at 1.2 × `interacting_periastron`, circular
      (P ≈ 2,000 d): a common envelope on the thermally pulsing AGB at 1.502 Gyr, the orbit shrunk
      by tides before it; a run to 1.5 Gyr agrees.
    - `a_white_dwarfs_birth_that_unbinds_the_orbit_disrupts_it`: the ruling's F1 trace (10.2 +
      7.88 M☉, 9,373 d, e 0.45, median draws), past the pre-test: `Disrupted { by: Secondary }`
      at 43.87 Myr, both stars present, no merger age. It fails on the old label.
    - `binary_system`'s Rucinski check (below).
  - _Goldens_ (`golden_diff.py --base ce69992`: one moved, none new, header at 20).
    - `stellar/binary_timelines`: **21 of 1,000 digests**, every one a pair the bound now passes
      that the lobe test did not (each from one segment to 2–15); 979 identical, so no run pair
      moved:
      - 3 braked into transfer: 0240 (0.87 + 0.60 M☉, 0.72 d: transfer at 1.131 Gyr, contact at
        1.165), 0349 (0.82 + 0.39, 0.51 d: transfer at 334 Myr, merger at 354 Myr) and 0873
        (0.93 + 0.52, 0.99 d: transfer at 5.970 Gyr, contact at 6.027);
      - 14 evolved without interacting: wide pairs with a giant (11, P 1,188–9,582 d) and three
        short ones braking short of contact by 12 Gyr (0730, 5.81 d; 0790, 1.57 d; 0961, 2.18 d);
      - 3 wide massive pairs (0330, 0480, 0782) unbound by the secondary's supernova. Their
        pinned primary's untouched collapse leaves no record (F7);
      - 1 (0030, 8.18 + 2.36 M☉, 3,657 d) unbound at 40.67 Myr by the primary's oxygen–neon white
        dwarf's birth: F1's new label, `Disrupted { by: Primary }`.
    - The ruling expected about 20 digests: the braked contacts plus pairs now evolved without
      interacting, and F1 "a few, if any". As expected.
    - Nothing else moved: not `stellar/summaries`, the galaxy chain, nor the planetary or server
      goldens.
  - _Statistics_ (slow, capped; against the ruling's §1.2 and §3.3).
    - `binary_system`, Rucinski (Finding F4's definition, asserted 10⁻³–4 × 10⁻³): contact pairs
      per main-sequence star of the same M_V over +1.5 < M_V < +5.5, the pair by its combined
      M_V, **2.84 × 10⁻³ (1/352)**, against the ruling's estimate of 3 × 10⁻³ and Rucinski's
      about 1/500. By bin (+1.5–2.5, 2.5–3.5, 3.5–4.5, 4.5–5.5): 0, 3.15, 2.48 and 3.41 × 10⁻³
      against his 1/662, 1/892, none and 1/425. All contact pairs per main-sequence star of
      +1.5 to +7.5: 2.12 × 10⁻³ (the ruling's 2.1 × 10⁻³); per one fainter than +1.5: 2.85 × 10⁻⁴
      (1.13 × 10⁻⁴ before, ×2.5; the ruling's 2.9 × 10⁻⁴). Contact pairs in layers A–D: 0, 10,
      65, 21 (0, 5, 23, 16), the ruling's to the pair.
    - `binary_system`, marks: stripped 1,407 of 1,752 (80.3%), merged 300 (17.1%), the merger band
      815 of 986 (82.7%), ruling 137's share 0.506 of 5,120 collapses: the ruling's T4.i + T4.j
      figures. The F1 relabel moves none of them.
    - `binary_carve`: redrawn C 20, D 137, E 41; pairs through the engine A 12, B 90, C 3,326,
      D 11,757, E 8,556 (T4.i: 1, 8, 3,268, 11,749, 8,554; the ruling's 12, 89, 3,324, 11,757,
      8,556).
    - `binary_classes`: DNS 112 (in 6–144), 64% at e < 0.3; recycled pulsars' medians unmoved.
    - The R06 census (`a_hundred_thousand_systems_live_and_die_in_order`) passes in **195 s**
      under the heavy lock (188 s at T4.i under the lock; 234 s at 9a0950e, unlocked): +4%, the
      ruling's "a few per cent".
    - **The pre-test's cost**, its tracks built (an uncommitted scratch copy of the gate's
      samples, one thread, best of 3, under the heavy lock; provisional): **1.24, 1.16 and
      12.5 µs a pair** in samples 0, 1 and 2, of which the lobe test alone takes 0.58, 0.83 and
      0.98 µs: the bound adds 0.66, 0.33 and 11.5 µs. The ruling measured 0.7–2 µs on
      main-sequence samples and 15–16 µs on giants.
    - **Benches** under the heavy lock, against ce69992's binaries (provisional):
      `binary/binary_evolve/algol` 26.05 → 26.01 ms and `…/cataclysmic_variable` 12.29 →
      12.30 ms; `binary/distribution` 183 → 191 sample pairs passing the pre-test, median 4,289 →
      4,067 µs, 99th percentile 17.95 → 17.83 ms; `stellar/system_full` over the 50 ly query's 917
      systems 346.8 → 350.0 ms (+0.9%, 382 µs a system), and the interacting pair 4.07–4.14 →
      4.08 ms.
  - _Gate._
    - fmt and workspace Clippy pass.
    - The capped workspace bless under `just _locked` (a nested `--unit` scope, 10G): 3,656 of
      3,657 pass, the one failure the known bless-mode race
      (`every_golden_file_carries_the_current_version`). Then the non-bless run: **3,657 passed**,
      745 s.
    - The slow binary suites (`binary::`, `binary_system`, `binary_carve`, `binary_classes`),
      capped: 11 passed, 305 s. The census as above.
  - _Reviews._
    - Rust review: must-fix applied (units in the new names: `start_years`, `until_years`,
      `offset_years`, `radii_rsun`, `largest_radii_rsun`, `p_c5`); should-fix applied
      (`can_interact`'s one-sentence summary, `own_members_with` private again, tests of the
      switches and of a giant's capture, the clock taken out of the gate test, which the rules
      forbid in tests: the timing above came from an uncommitted scratch copy under the lock);
      considered and applied: no `Vec` in `largest_core`, readability renames, a test doc's
      comparison.
    - Determinism audit: no must-fix. Applied: the equality of `can_interact` across track builds
      (should-fix), "widens with age: tested, not proven", the widening check in the slow gate,
      the capture test's run to an earlier age, `Disrupted`'s doc. Noted: the braking floor read
      on the drawn mass (a star keeps its track's mass before the first interaction).
    - Science check: the algebra, units, constants, HPT §7.2, Rucinski's bins and order, and the
      P₀ boundaries confirmed. Applied: the eccentricity wording (e can grow under tides for a
      star above about 1.3 Ω_eq; the bound still holds for the engine's Roche test at a, but not
      provably in Φ or for a periastron collision), the winds' wording (below), "A3–K5" for the
      +1.5 to +7.5 range, Rucinski's citation and pooled figure, the F1 test's wording.
  - _Findings, for the orchestrator (not this task's):_
    - **The wind's spin is not bounded** (science check, should-fix pending a ruling). A locked
      star's wind carries off 2/3 ṁ R² Ω (HPT eq. 110), which the tides restore from the orbit:
      for a giant heavier than its companion near its lobe, the drain beats the Jeans widening
      (the lobe shrinks 0.09–1.9% per 1% of mass lost at q = 3–10). The ruling's "winds only widen
      the orbit" is therefore not a proof; the bound holds through the reservoir's slack (k′₂ m
      R_max² against k′₂ (M − Mc) R²) and the zero-miss gate. Options: add (2/3 − k′₂) ΔMᵢ
      R_max,i² Ω_c to S (the probe's wind-spin variant: 0 misses, up to 2.9× the passes), or keep
      the doc's statement. Built as ruled, with the doc saying so. The reservoir is also counted
      once, where a star whose I falls and rises (the flash, a blue loop) could draw it twice.
      _Ruled (orchestrator, 2026-10-06):_ T4.j's bound is kept as built in version 21, and the
      missing wind-spin term goes into P11.T4.l, the 21 → 22 task, so that the bound becomes
      provable (T4.l's block, "The wind-spin term").
    - **The ruling's "F0–K" for +1.5 < M_V < +7.5** is A3–K5 (Pecaut and Mamajek 2013, ApJS 208,
      9, extended in Mamajek's online dwarf table, v2022.04.16: A3V M_V 1.70 to K5V 7.28; the
      paper's own table has no M_V).
      _Ruled (orchestrator, 2026-10-06):_ the label is corrected to A3–K5; the channels ruling's
      table carried "F0–K".
    - **Rucinski's shape.** His 1/500 is the scaling his bins are consistent with; pooled over his
      tables the four bins give about 1/750, and the engine's 2.84 × 10⁻³ is 1.4× and 2× those.
      The engine has no contact pair in +1.5 to +2.5, his best-populated bin (18 of his 26
      systems). For the braking law or the contact lifetime (ruling §3.2), not the pre-test.
      _Ruled (orchestrator, 2026-10-06):_ the empty +1.5 to +2.5 bin points at the braking law or
      the contact lifetime, and is an input to the queued binary-population calibration
      investigation (P06, P08, P11, after the 20 → 21 bump), beside F1's physics, F9–F12, F14 and
      the wind-spin term.
    - **F7 reaches more pairs.** The bound runs wide massive pairs whose giants' reservoir reaches
      their threshold; three of the 21 golden moves are such pairs whose pinned primary's
      untouched collapse is not recorded (F7). T4.j runs +6–10% of prior pairs in C–E (ruling
      §3.1), so F7's fix matters more after it.
- **P11.T4.k as built** (Phase J lane, 2026-10-06; rulings p11-supernova-pins and p11-t4k-faults).
  Every collapse in a run pair is applied once, from the star's last living mass. **Built for
  version 21, committed with its goldens re-blessed at version 20 and held out of integration**
  until the 20 → 21 bump lands with it, P11.T4.h, 9a0950e's fixes, P14.T47.e, P11.T4.i, T4.j and
  the Bond albedo.
  - _The rule, as built._
    - F7 and the carried tie (`supernova.rs`): `Engine::pinned_collapse` reads member 0 through
      `own_death_now(pin age)`, which finds the pin's own track at its death: a track (own or
      carried) whose death's age on the track is the pin's and falls at the engine's age, both to
      the clock's resolution (`evolve.rs` `clock_resolution_years`, 4 ε of the age, which
      `phase_ahead` now shares). A `Member::Track` there explodes from the death's progenitor mass
      and stays its own track, placed with `offset_for`; a `Member::Shaped` is read on its closed
      forms at `last_living` at its carried mass. Plan 06's remnant is capped at the mass before
      (`held_to`).
    - F8 (`sse/track/binary.rs`): `Remains::Collapse` is renamed `Remains::Ended`; every
      helium-star arm of `remains_at` goes through `stripped_to`, which returns `Ended` for a
      track dead from its start (`Track::is_remnant_from_its_start`), with `LastLiving`'s state and
      core radius. `common_envelope.rs` `stripped_member_at`: `Ended` dies at once through `die`,
      or is the pinned primary's `Frozen` hold; a debug assertion keeps a dead helium star from
      being placed alive.
    - A (`rlof.rs` `transfer_phase`): a step whose limit is `Stop::Death` or `Stop::Pinned` skips
      the detachment test; after its stop, `end_without_a_living_donor` ends a transfer whose
      donor has nothing living. A debug assertion: no transfer step starts from a donor that is
      neither living nor a white dwarf.
    - C (`common_envelope.rs` `contact_phase`): the knots stop at a pin inside the contact; the pin
      is acted on only before the pair's age, with `contact_until` cleared.
    - The backstop (`evolve.rs` `Engine::run`, `pin_due_now`): before each phase a pin due at the
      engine's age is acted on; a pin behind the engine's age is debug-asserted against, naming the
      pair's composition and draws.
  - _Deviations from the ruling's text._
    - **The carried tie's mass before** is the landing step's carried mass (`mass.last()` of the
      restarted path, the path's at the pin), as `Engine::die` reads a carried star's, not the
      path's at `last_living`. The two differ by the wind over 10⁻¹² of the age (pair 0446: within
      10⁻¹²; up to about 3 × 10⁻¹¹ relative for the other carried pins). The living test is read at
      `last_living` on the closed forms, as ruled.
    - **`own_death_now` checks that the track is the pin's**: its death's age on the track equals
      the pin's (review: determinism audit and plan conformance), so a track dead from its start
      placed at a strip on the pin's age is never taken for the pin's own.
    - **A strip or a death landing on the pin is held, not applied** (review): the hold guards in
      `die` and `stripped_member_at` take a pin at or after the engine's age, to the clock's
      resolution (`>=`, where they read `>`), so that the backstop collapses the primary once with
      plan 06's remnant. Without it such a tie would collapse the primary twice. No sampled pair
      meets it; no digest moves.
    - **The donor debug assertion** allows a white-dwarf donor, which gives mass as a remnant (BSE
      §2.6.5).
    - **The backstop also ends a transfer** whose donor its collapse leaves with nothing living,
      as A does at a step's stop; and the contact phase goes on as `quiet_kind` decides where the
      pin finds nothing living.
    - **`check_collapses`** also skips the pairs the pre-test passes over (the version-21 limit,
      below); its first invariant exempts a compact star a merger leaves inside its companion (a
      Thorne–Żytkow object, BSE §2.7.3); its third checks the neutron-star and black-hole tracks,
      a white dwarf's birth leaving no record (the placement's debug assertion covers it in debug
      builds). Test hooks: `evolve::pinned_death_age_years` and `pinned_track_and_pin`.
    - **Tests.** `a_core_stripped_past_its_end_dies_at_once` takes pairs 0544 and 0145: 0703, the
      pins ruling's choice, now records its primary's carried collapse at 12.51 Myr and no longer
      strips its companion past its end. The carried tie's test takes 0446 and checks the orbit
      after against A1 for that mass before. `a_contact_stops_at_the_pin` checks the build-age
      contract at 30, 46 and 46.9 Myr (determinism audit). Three more (rust review):
      `a_pin_due_at_the_engines_age_is_applied`, `a_pin_left_behind_is_a_fault` (debug builds)
      and `a_transfer_without_a_living_donor_ends`. `a_white_dwarfs_birth_that_unbinds_the_orbit_disrupts_it` (P11.T4.j's F1
      label) moves from the channels ruling's 10.2 + 7.88 M☉ trace, which F7 now unbinds at its
      primary's collapse, to pinned pair 0030 (the primary's oxygen–neon white dwarf at 40.67 Myr).
      The sse test covers the early AGB at 20 and 1.2 M☉ and the Hertzsprung gap at 2.2 M☉, not the
      first giant branch.
    - Earlier entries keep `Remains::Collapse`, the name they were written with. T4.l's golden list
      (measured at ce69992) names 0330, 0480, 0782 and 0886, which T4.j now runs and which moved
      here as F7 pairs: re-measure it in T4.l.
  - _Goldens_ (`golden_diff.py --base 1e47d16`: one file moved, none new, the header at 20).
    - `stellar/binary_timelines`: **55 of 1,000 digests**, the ruling's prototype to the digest:
      - 33 F7: 0076, 0082, 0084, 0086, 0088, 0112, 0191, 0243, 0258, 0268, 0311, 0330, 0346, 0408,
        0429, 0460, 0480, 0490, 0548, 0616, 0761, 0782, 0805, 0831, 0844, 0845, 0886, 0904, 0915,
        0936, 0950, 0985, 0994;
      - 7 F8: 0257, 0539, 0544, 0572, 0701, 0703, 0739;
      - 5 the carried tie: 0134, 0156, 0446, 0554, 0677;
      - 10 A and C: 0004, 0326, 0741, 0829, 0880 (A's records), 0223, 0277 (C), and 0170, 0835,
        0862, which keep their records and segment counts and move in their bits (A acts on a
        death in the landing step rather than after it).
      - Every pair but those three gains a record or moves one (0277: 69.05 → 46.83 Myr); 0257 and
        0703 gain two. 9 change their segment counts (0134, 0223, 0257, 0277, 0326, 0554, 0677,
        0741, 0829).
    - Nothing else moved: not `stellar/summaries`, `stellar/hierarchies`,
      `stellar/held_bare_core_helium_star`, the galaxy chain, nor the planetary or server goldens.
  - _Statistics._
    - The scratch population probe (the channels ruling's sampler, 12,000 layer-E and 10,000
      layer-D prior pairs to 13.8 Gyr), against 1e47d16:
      - layer E: primary records 5,457 → 6,755; companion records 1,225 → 1,301; bound BH + BH
        386 → 460, NS + BH 94 → 119, NS + NS 112 → 115; the hydrogen-poor share of all records
        0.587 → 0.538; "nothing living left" 168 → 148 (146 two-strip white dwarfs and the 2 capped
        pairs); the bound share after the primary's collapse 73.6% → 66.8% shown (43.2% → 42.2%
        recorded);
      - layer D: primary records 208 → 232, "nothing living left" 39 unchanged;
      - the ruling's figures, to the pair.
    - The scratch collapse census (`check_collapses`' invariants, counted): **0 faults in the
      pinned thousand** (103 at 1e47d16), and none in the 22,000 prior pairs outside the capped
      timelines, 1723 and 8002 of layer E (P11.T4.g's C2) and 6695 of layer D (F15), reported
      apart.
    - `binary_system`: ruling 137's hydrogen-poor share **0.500** of 5,323 collapses (0.506 of
      5,120 before; the ruling's estimate about 0.48); ruling 123.5's marks unchanged (1,407 of
      1,752 stripped, 300 merged, the merger band 815 of 986); Rucinski unchanged (2.84 × 10⁻³).
    - `binary_carve`: identical to T4.j's counts.
    - `binary_classes`: bound double neutron stars **115** (in 6–144), 63% at e < 0.3, 26 merging
      (about 30 per Myr by VG18's yield); the recycled pulsars' medians unchanged.
    - The 10³-pair suites pass; the decay bound's zero-miss gate passes (0 misses; passes
      unchanged; the engine interacts in 2,335 and 312 pairs of samples 1 and 2, against 2,347 and
      501, since the collapses it now applies unbind pairs first).
    - The R06 census passes in **188 s** under the heavy lock (195 s at T4.j).
  - _Gate._ fmt and `cargo clippy -p hyperion-sim --all-targets -D warnings`; the targeted
    `binary::` and sse suites (86 tests); the workspace bless under `just _locked` (3,668 of 3,669,
    the one failure the known bless-mode race) and then the non-bless gate (**3,669 passed**,
    729 s); the slow binary suites (11 passed) and the census, all capped.
  - _Reviews._ Determinism audit: no must-fix; applied: the hold guards for a strip on the pin,
    `own_death_now`'s check of the pin's own track, the contact's build-age checks. Rust review:
    applied the must-fix (`LastLiving::helium_giant` takes its star), units in names and types
    (`SolarMasses` in `LastLiving`, `OwnDeath`'s `offset_years`, the test helpers' suffixes),
    intra-doc links and summaries, assertion messages, the shared clock resolution, the merged
    test helpers and the three tests above; not applied: a debug assertion for a death (not the
    pin) left behind, which would fire on the member `die` has just placed at its death. Plan
    conformance: no must-fix; the carried test checks A1, and the deviations above are recorded.
    Science check: fixed the white-dwarf donor's section (BSE §2.6.5), the F8 windows by arm,
    A3–K5's source (Mamajek's dwarf table), the progenitor-mass wording, F13's metallicity and
    island, F14's merging yield against Pol et al.'s intervals, and Tauris et al.'s 39 of 47 for A.
- **Version 21 limit: a passed-over pair's collapses** (ruling p11-supernova-pins, 2026-10-06,
  figures re-measured by ruling p11-t4k-faults; fixed by P11.T4.l at version 22).
  - A pair that the pre-test passes over keeps its drawn orbit through its stars' core collapses,
    with no record, mass loss or kick.
  - These are 42% of layer-E primaries' collapses (5,012 of 11,915 in the 12,000 prior pairs,
    after T4.j's bound) and 36% of layer D's (150 of 421).
  - So version 21 shows:
    - the bound share after the primary's collapse at 66.8%, against about 34% with T4.l;
    - bound NS/BH beside a living star for 26.4 × 10⁶ pair-Myr per 12,000 pairs (T4.l: about
      4.5 × 10⁶, measured at ce69992);
    - double-neutron-star states in 282 pairs, against 115 by record;
    - symbiotic neutron-star X-ray binaries for 25,600 pair-Myr (T4.l: about 1,900);
    - ruling 137's hydrogen-poor share of recorded collapses at 0.54, against about 0.38.
  - Positions are unaffected (the drawn hierarchy). Pair light moves in 1 of 5,607 such pairs.
- **P11.T4.k's known departures** (ruling p11-t4k-faults, 2026-10-06).
  - **A transfer's last step** reads the dying donor's overfill at its death, a remnant's radius,
    so it moves no mass. The donor keeps about one step's transfer (BSE eq. 92's target of 0.5% of
    its mass) in its mass before. Case BB donors mostly do collapse during their transfer (Tauris,
    Langer and Podsiadlowski 2015, MNRAS 451, 2123, Table 1: 39 of 47).
  - **A pinned primary that the binary keeps on its main sequence** (a Case A donor whose binary
    main sequence outlasts plan 06's death) explodes from it at plan 06's age (design note 16).
    - In the detached and transfer phases this was so before T4.k; T4.k extends it to contact.
    - Pairs 0223 and 0277 of the thousand collapse at τ 0.999 and 0.987, 18.5 kyr and 0.68 Myr
      before their binary main sequences would end. Their merger products would have died 10
      and 22 Myr later.
  - **Pinned primaries stripped by the binary keep plan 06's single-star remnant**, capped at
    their mass (design note 16).
    - Binary-stripped stars have smaller carbon–oxygen cores. For case B strips, Schneider,
      Podsiadlowski and Müller (2021, A&A 645, A5, Table 1) move black-hole formation from
      ≳ 34 M☉ to ≳ 67.5 M☉ (the complements of Table 1's neutron-star ranges, at Z = 0.0142; case
      B also has a black-hole island at about 31.5–34 M☉, where the trend runs the other way).
    - Finding F13: plan 06's remnant law should read the stripped mark, as its kick law does.
      Routed to P15.T5.c, with its own bump.
- **Finding F14 (ruling p11-t4k-faults): wide bound double neutron stars.**
  - 115 bound per 12,000 layer-E prior pairs (7.2 × 10⁻⁵ per M☉ formed) is about 4.4 times
    COMPAS's (Vigna-Gómez et al. 2018).
  - Only 21% of them merge, against VG18's 73%. The merging yield, about 26 per Myr, is at the
    low edge of Pol et al.'s 90% intervals (2019, 2020) and inside Colom i Bernadich et al.'s
    (2023).
  - Routed to the binary-population calibration investigation with F9–F12.
- **P11.T4.g's finding C2 (`swell`'s core) has a measured consequence** (ruling p11-t4k-faults;
  ruled by ruling p11-c2-swell, 2026-10-06, and fixed by P11.T4.m at version 22).
  - A helium main-sequence accretor fed hydrogen by a main-sequence donor:
    - is swelled at any positive rate, even while its wind outweighs what it is fed, into a
      core-helium-burning giant whose envelope is the placement's miss (−0.2 to −0.6% of its
      core);
    - T4.g's strip, or a common envelope, makes it a helium star again at once;
    - transfer resumes.
  - The cycle repeats, every 1–66 kyr, until the timeline's 64 segments (`MAX_SEGMENTS`) cap it.
    Its later deaths, pins included, are never reached, and its stretched last segment reads its
    shaped member past that member's death.
  - How often: 2 of 12,000 layer-E prior pairs (1723 and 8002 of `binary_classes`' sampler).
  - The layer-D capped pair, 6695 of 10,000 (4.29 + 2.91 M☉, P 4.19 d, Z 0.0024), is not C2 but
    **finding F15** (ruling p11-c2-swell; fixed by P11.T4.m): `rejuvenated_tau` counts a helium
    star as convectively mixed (τ × m₀ ÷ m₁), where BSE (`evolv2.f` 1889–1900) and COMPAS's
    default keep its fraction burnt. Fed near its end, its steps land short of it and the
    segments shrink to the cap, at 183.1 Myr, its stretched segment showing a hot subdwarf to
    13.8 Gyr.
  - T4.k's collapse invariants skip capped timelines, as the 10³ suites assert none.
  - BSE keeps the accretor's own core and gives it the step's net gain as envelope, behind a gate
    (`evolv2.f` 1480–1491): P11.T4.m's rule, which removes both caps.
- **The local V light is low, and the pair light (a pointer from rendering plan R06, 2026-10-06,
  `decision-r06-t9b-band.md`; for this plan's owner, not a ruling on this plan).** Near the Sun
  the fixture's V light is 26% under Flynn et al. 2006's 0.056 ± 10% L☉ pc⁻³ (plan 02's Risks,
  "The local V light is low"). A calibration finding for plans 02 and 06, and for this plan through
  the pair light: R06's luminosity function is the generator's own quadrature of IMF × SFH ×
  tracks, with R06.T5.d's pair correction drawn by this plan's laws. A fix is a
  `GENERATOR_VERSION` change.
- **The 20 → 21 bump, as built** (Phase J lane, 2026-10-06; decision-p11-t4k-faults §4). P14.T13.c's
  Bond albedo (`p14-albedo`'s two commits, cherry-picked after T4.k) and three merges of
  `rendering-and-planets` (to 33f609c4) come first. Then one `feat(sim)` commit bumps
  `GENERATOR_VERSION` to 21, with its `assert_eq!`, and re-blesses every golden at 21
  (`HYPERION_BLESS=1 cargo nextest run --workspace` under the heavy lock).
  - _The bump moves no value itself._ Against the merge before it, `golden_diff` shows 102
    header-only files and `galaxy_parameters`' own `generator_version` field. The six surface
    goldens keep `TEST_PLANET_VERSION`.
  - _Against 97c969c_, the branch's last version 20, whose goldens integration still holds bit for
    bit: 35 files moved, 1 new, 67 header-only. Only the held commits touch a golden; no merge
    does.

    | Commit              | Goldens moved                                                                                                                                                                                                                                          |
    | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
    | ee47311, P11.T4.h   | 26 galaxy, gas, feature, catalogue, query, placement, planetary-context and server goldens (the galaxy chain at the last bits, through `fates::mean_present_mass`); `stellar/early_agb_remnant` (new); its share of `binary_timelines` and `summaries` |
    | 9a0950e             | `binary_timelines` (all 1,000 digests); `summaries` (two black holes by an ulp)                                                                                                                                                                        |
    | 3a52ca8, P14.T47.e  | `photometry/templates` (528 values); the Earth photometry of `close_binary`, `filler_c`, `solar_like` and `wide_binary`                                                                                                                                |
    | ce69992, P11.T4.i   | `binary_timelines` (655 digests); `summaries` (one white dwarf)                                                                                                                                                                                        |
    | 1e47d16, P11.T4.j   | `binary_timelines` (21 digests)                                                                                                                                                                                                                        |
    | 3b55013d, P11.T4.k  | `binary_timelines` (55 digests)                                                                                                                                                                                                                        |
    | f7b6b4eb, P14.T13.c | 56 values: `derive_body` and five planetary systems                                                                                                                                                                                                    |
    | 1845d20, cdebae44   | none                                                                                                                                                                                                                                                   |
    | fates refit at 21   | none: the refit commit re-blesses every golden after refitting `stellar_fates_low` and `_mid` (P11.T4.h's early-AGB core, through the fates), and no golden moves                                                                                      |

    In all, `binary_timelines` moves all 1,000 digests: 372 in place, and 628 with a new segment
    count in their label. `summaries` moves 29 values and one relabelled death. T47.e and the
    albedo share 8 of `close_binary`'s 72 moved values and 2 of each other system's 18.

  - _Two of R06's pins outside the goldens move too._ Integration's tests hold them, and this
    branch's commits had been blessed without those tests. Each was bisected on integration
    (a50d8a73) with each held commit's code applied in turn, then re-pinned in the bump:
    - `sky::luminosity`'s three build fingerprints move with T4.h alone;
    - `sky_census`'s pinned merger 0x4204_6c99_ff00_000a moves with T4.i alone. Its pair merges at
      58.4 Myr, not 144.2 Myr, and is now a 1.12 M☉ core-helium-burning giant. Census and oracle
      still agree.

    R06's Risks, "Generator version 21", has the detail.

  - _Fitted tables._ At 21, `just fit-check` reported `sky_binary_light_c`, `_d` and `_e` stale,
    since their probes read the binary engine. `hyperion-fit`'s `the_committed_tables_are_fresh`
    failed with them, so integration's `just ci` could not pass at 21 without a refit. Every other
    table was fresh by its probes.
  - _The refit, in 21 (ruled 2026-10-07 on the determinism auditor's must-fix)._ The refit moves the
    sky's output: R06's luminosity tables, and so the three fingerprints again. So that (seed, 21)
    names one output, it was made at `--since 21` on this branch, in one commit after the bump,
    and lands with the batch rather than taking a bump of its own. R06's Risks, "Generator version
    21", has the refit as built.
  - _Stale by rerun: the fates, refitted in 21 too._ `hyperion-fit check --rerun-fast` at 21 also
    found plan 06's `stellar_fates_low` and `_mid` stale by rerun, moved by T4.h alone. The
    orchestrator first kept them for 21 with a refit in the version-22 batch. The same day
    (`status.md`, 2026-10-07) it ruled them refitted at 21 instead:
    - no version-21 output is published;
    - kept, they would leave `just test-slow` and `just ci-slow` red at 21 and could hide a third
      `RerunDiffers`;
    - a later `just fit` at 21 would have moved every `FittedFates` reader without a bump.

    So the refit commit refits both at `--since 21`, then the binary-light tables again on them,
    and re-blesses every golden at 21. Plan 06's Risks has the fates' detail. The row
    "fates refit at 21" of the table above attributes its golden moves.

  - _Landing._ The bump (2ecdeb4a) and the refit land in the same push. The bump alone fails
    `the_committed_tables_are_fresh`, and neither its sky nor its fates are version 21's.
  - _The bump's gate_ (the refit's is in R06's Risks, "Generator version 21": 3,808 of 3,808, and
    `--rerun-fast` with 0 failures). fmt; workspace clippy, natively and on wasm32-wasip1 (base,
    surface, sim, testkit); `just cross-clippy`. Under the heavy lock: the non-bless workspace run,
    3,807 of 3,808 (the fit test above fails); the doctests; the slow binary suites, 11 of 11; the
    R06 census, 187 s.
    The determinism auditor finds the bump itself clean.
- **P11.T16's cost, measured (2026-10-07; pending a ruling, nothing built).** Before building, the
  lane measured whether ask A can be exact at 3 µs. The probe ran on one thread in release, at a load
  of 7–14, so every timing is provisional. It used the Milky Way fixture (seed `0x0926_0000`) and
  4,000 records a layer from the cells nearest (0, 26,000, 68) ly, at attempt 0 and each record's
  own `draw_metallicity`. The probe is `.git/rm23-scratch/p11-bounds/t16/probe_t16.rs` and its logs
  are `probe1.log` and `probe2.log` there. It is not committed.

  | Near the Sun                                      | A     | B     | C     | D     | E     |
  | ------------------------------------------------- | ----- | ----- | ----- | ----- | ----- |
  | Single at attempt 0                               | 69.5% | 60.5% | 50.2% | 37.2% | 12.7% |
  | A single's count-only words, µs                   | 0.23  | 0.20  | 0.21  | 1.09  | 8.69  |
  | A multiple's whole draw, µs                       | 11.8  | 10.5  | 15.1  | 53.3  | 69.6  |
  | `draw_hierarchy_of_composition`, every record, µs | 3.6   | 4.3   | 11.3  | 48.1  | 68.7  |
  | All eight attempts, µs                            | 22.9  | 43.9  | 85.5  | 385   | 540   |
  | Attempt 0 holds a pair (brown dwarfs included)    | 31.8% | 37.8% | 48.0% | 62.0% | 90.8% |
  | Attempt 0 runs a pair through the engine          | 0     | 0     | 13.5% | 58.8% | 90.5% |
  | Redrawn, of 400 generated                         | 0     | 0     | 1     | 2     | 2     |

  The bulge, at (0, 2,000, 300) ly, gives the same figures within 10% (C runs 24.8%).

  The pieces:
  - `DirectPeriods::new` takes 36–42 µs. It is 79 knots of Moe and Di Stefano's law, built once in
    every draw of the direct construction, even with no companion.
  - A spine companion's windowed `PeriodDistribution::quantile_in` takes 3.0–4.7 µs a try: 32 fixed
    Newton–bisection steps on an erfc mixture.
  - `tidal_radius` takes 0.70–0.85 µs a call. It is called for every host window and every `admits`.

  Why an exact bound cannot cost 3 µs at version 21:
  1. A companion's mass is its host's mass times q. q's law varies continuously with log P (γ_small,
     γ_large and the twin share, Moe and Di Stefano's eqs. 9–23) from a host of 0.8 M☉ up. So the
     mass's bits need the period's bits:
     - for the direct construction, the 79-knot table, per primary mass;
     - for the spine, a windowed quantile whose window holds the exact tidal cut and the orbits
       already placed.
  2. The kept try is the one the whole stability test admits (Mardling and Aarseth, the tidal cut,
     the stripped band). Listing every try instead lists up to 42 masses a slot and costs more.
  3. The carve cover. Attempt n + 1 can be kept only if attempt n carves, and that is known only from
     the engine. Without the engine, the only exact exclusion is that attempt n holds no star–star
     pair, and one that cannot interact and holds no remnant by +H is the next sharpest. The
     expected count of further attempts is then p + p² + … + p⁷, with p 0.13–0.48 in C, about 0.6
     in D and 0.9 in E. So an exact bound draws about 1.4–1.6 more attempts for D and 4.7–4.8 for E. Only
     `DirectPeriods` is shared between attempts.

  An exact bound with today's arithmetic therefore costs about:

  | Per record                    | C      | D   | E   |
  | ----------------------------- | ------ | --- | --- |
  | Attempt 0 alone (a floor), µs | 7.6    | 34  | 62  |
  | With the carve cover, µs      | 8.5–13 | 45  | 180 |

  Bit-exact speed-ups might save up to half: the tidal radius once a record, `DirectPeriods`' knots
  that repeat a law computed once, and the count's words alone for a system single at attempt 0.

  Near the Sun the census has C 1.93 × 10⁸, D 6.44 × 10⁷ and E 1.42 × 10⁸ records past the floor.
  The bound alone would cost about 1.2 × 10⁴ CPU-s at attempt 0 and 3 × 10⁴ with the cover. That is
  against R06.T8.g's cold gate of 10,000 CPU-s and T17's budget of 4,000. With T7.b's caps by
  direction (E at 25%), the bound with its cover would cost about 10⁴ CPU-s.

  Options, for the orchestrator to rule:
  - (a) Build it exact at the draw's cost. The bound is the union of the hierarchies that the
    generator's own draw gives at each attempt it may keep. The cover stops at the first attempt that
    holds no star–star pair, and bit-exact speed-ups are applied. The 3 µs target, R06.T8.g's gate
    and T17's budget are re-ruled.
  - (b) As (a), with the cover cut by a sharper exact exclusion built on P11.T17's pair bound. The
    exclusion is for no merger, no transfer and no compact accretor beside a living star in the
    source horizon. The cost then approaches attempt 0's.
  - (c) A generator change in the deferred version-22 batch:
    - the direct period law tabulated by mass node;
    - a closed-form windowed period quantile;
    - companions' masses kept across carve redraws, or a redraw of the carved pair alone.

    An exact bound could then cost a few µs. It moves output and changes how the carve conditions.

  - (d) An `Unbounded` verdict for every multiple. The bound is exact and cheap for systems single at
    attempt 0: 0.2–1.1 µs in A–D and 8.7 µs in E. But the census would still generate 50–87% of
    C–E's records, which fails R06.T8.g's gate of 1%.

  The lane's lean: (a) now, and (b) once P11.T17 lands. The bound stays exact and its cost is
  recorded. (c) belongs with lever 13 if T17's budget is missed.
