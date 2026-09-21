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

`units::Days`. `coords::{SystemVector, SystemVelocity}`: a displacement and a velocity in the system
frame's axes, `f64` metres and metres per second, beside plan 01's `SystemPosition` (a position from
the barycentre). Plan 01 owns `coords` and has no equivalent (its `GalacticDisplacement` and
`GalacticVelocity` are in the galactic frame), so P11.T3.a adds the two types to plan 01's file for
the frames, `src/coords.rs` (or `src/coords/frames.rs` if plan 01 split the module), with the same
frame guard: no conversion to a galactic or body vector without an explicit origin. Plan 14 uses all
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

Plan 14 reuses `orbit` for planets and extends it with open orbits. Plan 09's
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
pub struct RedrawAttempt(/* u8, 0..MAX_REDRAWS */);
pub const MAX_REDRAWS: u8 = 8;
pub const DRAWS_PER_ATTEMPT: u32 = 64;               // plan 06's block in `StarDraws::for_attempt`
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
  `events::{TimeWindow, PoissonBins, MonotonePhase, PhaseClock, LinearClock}`.
  `light_curve(event, dt, band)` takes plan 07's `galaxy::gas::Band` and returns `Watts`; the X-ray
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
P15.T10.b land, the constants hold the scratch values of Design note 13 and are listed in plan 15's
`tables::MANIFEST` with `provisional: true`.

### Domain tags (never renamed)

Entries of plan 01's single `domain_tags!` registry in `rng/tags.rs`, under a "Plan 11" heading,
each added by the task that first draws on it. Scope `System`: `system.multiplicity`,
`system.hierarchy`, `system.substellar`. Scope `Body`, keyed by the `BodyId` of the star an orbit
brings in (Design note 5): `binary.orbit`, `binary.orientation`, `binary.phase`, `binary.kick`,
`binary.ia_mark`, `binary.ce`. Scope `System`, under plan 09's reserved prefix `class.`, for the
catalogue side's candidates: `class.awd`, `class.xrb`, `class.merger`, `class.nsm`. Event keys: one
`DomainTag` of scope `Event` per event tag above, as plan 01's `event_tags!` requires. Streams are
opened with `Stream::open(seed, tag, ObjectKey::from(id))`.

### Protocol and client

- `hyperion-protocol`: `BinaryClassDto`, `OrbitDto`, `HierarchyDto`; plan 06's `SystemSummaryDto`
  gains `hierarchy`, its `StarSummaryDto` gains `body_index` and `binary_class`, and its
  `StellarBriefDto` gains `star_count: u8`. All are additive fields on plan 06's `system_summary`
  request kind and on plan 04's `systems_in_range` rows; this plan adds no request kind.
- `apps/hyperion`: `StarList` component; `formatOrbit` helpers.

### Test helpers

`crates/hyperion-sim/tests/common/binaries.rs`: `sample_systems(galaxy, layer, n)`,
`sample_binaries_by_mass(..)`, `brute_force_class_members(galaxy, class, n)` (prior sampling with no
carve-out: every attempt-0 binary that `carved_class` puts in the class, for checking samplers; plan
15's P15.T10.b calls it, so it is also exported from `stellar::binary::testing` behind the `testing`
feature, as plan 06 does for its samplers).

## Consumes

Names are those of the owning plans' Provides as they stand; the owning plan is authoritative, and
where a name has changed by the time this plan runs only the call sites here change.

- **Plan 01:** `math`; `rng::{Seed, Stream, DomainTag, ObjectKey, Mark, Threshold}`,
  `Stream::open(seed, tag, object)` with random access by `word_at` and `seek`, the `domain_tags!`
  registry in `rng/tags.rs`; the samplers (uniform, normal, log-normal, `PowerLaw` with the exponent
  1 case, `PiecewiseLinear`); integer-threshold decisions; `rng::EventKey` and the `event_tags!`
  registry in `id/event_tags.rs`; `units`;
  `time::{UniverseTime, Span, CLOCK_WINDOW_H, LIGHT_CROSSING_L, SourceHorizon}`;
  `coords::SystemPosition`; `id::{SystemId, BodyId, EventId, EventTag}`; `GENERATOR_VERSION`; from
  `hyperion-testkit`, the `golden!` harness, `stats` (chi-square, Kolmogorov–Smirnov, Poisson
  counts), `order::assert_order_independent`; slow-test marking, `just test-slow`, `just bench`.
- **Plan 02:** `imf::{MassFunction, Kroupa, Chabrier}` (with `Chabrier::high_mass_scale`, which
  reads plan 15's `tables::chabrier::HIGH_MASS_BRANCH_SCALE`, provisionally 0.68); the seam
  `galaxy::fates::StellarFates` with `mean_companions`, `ProvisionalFates`, `mean_present_mass` and
  `stars_below` (Design note 1); `ShareMatrix`; `potential::PotentialTables` (tidal radius).
- **Plan 03:** `placement::{SystemRecord, resolve}`, `query::{RangeQuery, SystemSource}`,
  `check_index_headroom`, the test helper `sunlike_point`.
- **Plan 06:** `stellar::sse::{Track, evolve, lifetime, turn_off_mass}` with
  `Track::max_radius_until`, and the helium-star entry point of P06.T9 (a track from a helium-star
  mass); `StarState`, `Phase`, `Composition`; `StarDraws::{for_star, for_attempt, from_parts}`,
  whose attempt block is 64 draws; `StarModel`;
  `SystemStars::{generate, summary_at, brief_at, death_time, natal_kick}`, `ClockDeath`; from
  `stellar::remnant`, `KickLaw`, `StandardKickLaw`, `KickLawParams` (with its provisional
  `stripped_share`), `NatalKick`, `KickMode`, `Stripping`, `CollapseChannel`, `ProgenitorAtDeath`,
  the pulsar spin-down closed form, and the provisional companion-stripped mark on the permanent
  stream `star.stripped`;
  `events::{TimeWindow, PoissonBins, RateModel, MonotonePhase, PhaseClock, LinearClock}`;
  `stellar::classify`; `stellar::substellar::cooling`; `stellar::fates::TrackFates`; the rule that
  body index 0 is the primary and companions are numbered from 1; the event-tag block 0x0300–0x03FF;
  the protocol's `system_summary` kind with `SystemSummaryDto`, `StarSummaryDto` and
  `StellarBriefDto`.
- **Plan 07:** `galaxy::gas::Band`, for light curves.
- **Plan 08:** on `SystemRecord`, `placement_class() -> PlacementClass` (`Alive`, `Retained`,
  `Displaced { class, kind }` with `DisplacedKind::{Remnant, Runaway, Walkaway}`), `mark_attempt()`
  and `kick_constraint()`, which plan 06's primary draws already honour; the seam
  `displaced::binarity::stripped_share(m, &Composition)` and `ClassTable::stripped_share_used`;
  `displaced::runaway::RunawayModel` (the runaway and walkaway shares this plan must reproduce);
  `kick_bins::speed_bin_shares` (the low-mode share).
- **Plan 09:** from `catalogue_classes`, `ClassId`, `ClassProcess`, `CatalogueClassSource`,
  `CatalogueClassCell` and `CatalogueCellKey`, with the reserved values 2, 3, 5 and 6 and
  `ClassId::cell_log2_ly()`; the `111` prefix encoding through plan 01's `CatalogueSystemId`; the
  catalogue grid walk; `features::members::{MemberRecord, FeatureLevelList}` and the rule that
  feature-level lists append later classes after its own (its design note 16);
  `features::interior::{MemberClass, ClassKind}` with the binary classes and the recycled-object
  marks of P09.T9.e; `features::cluster::ClusterModel` (`sigma(r)`, for the hard–soft boundary);
  `features::shares::FeatureShares::field_factor`, through which layer D already gives up
  `type_ia::ancient_share`; the Type Ia entry (`type_ia::IaProgenitor`) and the requirement its
  P09.T35 records, that this plan's binaries redraw any explosion before +H;
  `catalogue_classes::testing::assert_complementary`.
- **Plan 15:** `tables::type_ia_delay::DELAY_EDGES`; `tables::MANIFEST`; the tasks P15.T4.b,
  P15.T5.c, P15.T9.b and P15.T10.b, which call this plan's code and fill this plan's
  `tables::binary` (Design note 13).
- **Plans 04 and 05:** the request envelope (`RequestBody`, `ResponseBody`); `SystemsInRange` rows;
  `displays/galaxy/{SystemList, SystemReadout}.tsx`, `lib/format.ts`, `SunGlyph`.

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
   initial mass, whatever mass transfer does later.
6. **"Run forward once" means a timeline.** `evolve` integrates a binary once, from zero age to its
   age at +H, and returns an ordered list of segments with their boundary ages and parameters. State
   at any age is a lookup plus closed forms inside the segment (single-star evolution of each
   component at its effective mass and age offset, Peters's decay, a constant mean transfer rate).
   That keeps state continuous in time, open to random access and free of replay, as "Events in
   time" demands. The timeline is derived data; the caller may cache it with the system.
7. **Only close pairs run.** `can_interact` compares periastron with the Roche-filling separation
   for each star's largest radius up to the age in question (Eggleton 1983; plan 06's radius bound).
   Everything else is two single stars on an orbit. Wind-fed symbiotics need no orbit change, so
   they are classified from the state of a wide pair.
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
   for every companion, which is plan 06's block of 64. This redraw never touches the primary's own
   draws: they stay as plan 08 left them, the track draws at `record.mark_attempt()` and the
   remnant, stripped-mark and kick fields at whichever later attempt P08.T12.c's kick loop kept on
   that one track, because placement, the displaced classes and the supernova test have already read
   them. An attempt is a pure function of (ID, n). After eight attempts the last hierarchy is kept
   with its innermost period moved out of the interacting range; at a carve probability under 5%
   that happens less than once in 10¹⁰ systems.
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
    binding parameter of Hurley, Tout and Pols; their critical mass ratios; their accretion
    efficiency. `CLUSTER_MERGED_BINARY_FATE` is the fourth of the kick law's defaults that the
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

All Rust files are under `crates/hyperion-sim/src/` unless a path says otherwise. Every task that
first draws on a domain tag adds its entry to `rng/tags.rs` under the "Plan 11" heading, so plan
01's collision test covers it. There is no interface-reconciliation task: the names under Consumes
were checked against the owning plans when this plan was validated, and plans this late are
re-validated against the code when their turn comes (README).

### P11.T1 The multiplicity model

- **P11.T1.a Fractions and counts.** `units::Days`; `MultiplicityModel`, its anchors (Design note
  2), `multiple_fraction`, `companion_frequency`, `companion_count_pmf`. Doc comments cite Duchêne
  and Kraus (2013) and Raghavan et al. (2010), re-checked. Tests: anchors are reproduced; the PMF
  sums to 1 and its mean equals frequency ÷ fraction to 1% before truncation. Acceptance:
  `cargo test -p hyperion-sim multiplicity::model`.
- **P11.T1.b Distributions.** `PeriodDistribution`, `MassRatioDistribution`,
  `EccentricityDistribution` with densities, CDFs and samplers on a supplied stream (Design note 3).
  Tests: Kolmogorov–Smirnov of 10⁵ samples against each CDF at five primary masses; the Sun-like
  period mode lies within 0.1 dex of 10⁵ days; no companion under 0.08 M☉. Acceptance:
  `cargo test -p hyperion-sim multiplicity::dist`.
- **P11.T1.c Quadratures.** `all_stars_fraction_below`, `mean_companion_mass_per_system`,
  `stripped_share` (the share of primaries whose periastron passes the `can_interact` threshold
  before core collapse; it takes the threshold as a function so that T4.a can supply the real one,
  and until then a test-only closure of periastron under 10 au stands in). Fixed Gauss–Legendre
  nodes, no randomness, stars only. Nothing calls them yet, so no output changes. Tests: under
  Kroupa the fraction below 0.5 M☉ is 0.764 ± 0.005, against the observed 0.759; under unscaled
  Chabrier 0.67 ± 0.01; the gap between `stripped_share` and plan 08's
  `ClassTable::stripped_share_used` is printed per mass for T1.d. Acceptance:
  `cargo test -p hyperion-sim multiplicity::quadrature`.
- **P11.T1.d Replace the stand-ins (version bump; every star moves).** Four edits in one commit. (1)
  Plan 02's `StellarFates` gains a provided method
  `companion_mass_ratio_cdf(&self, m1: f64, q: f64) -> f64`, defaulting to the uniform 0.1–1 that
  `mean_present_mass` and `stars_below` hard-code today, and both read it. (2) `MultiplicityFates`
  wraps plan 06's `TrackFates`, overrides `mean_companions` and that method from the model (the mass
  ratio marginalised over period), and `Galaxy` uses it for every population. (3) `stars_below` and
  `all_stars_fraction_below` now agree, which a test pins. (4) Plan 08's seam
  `displaced::binarity::stripped_share` returns this plan's `stripped_share` with T4.a's threshold,
  so plan 06's provisional mark and plan 08's class table both follow; plan 06's constant
  `KickLawParams::stripped_share` stays only as the quadratures' fallback for tests. Bump the
  version and regenerate every golden: this is the one task of the plan that moves primaries. Tests:
  mean present mass per system is 0.48 ± 0.03 M☉ under Kroupa (plan 02's bracket; plan 02 measures
  0.498–0.503 with its stand-in fates, R11) and 0.55–0.60 under Chabrier, within 3% across the old
  populations; stars per system 1.33–1.45; `stripped_share` averaged over layer E lies in 0.20–0.33,
  the two mixes of the brainstorm's scratch Monte Carlo, and a value outside is a finding against
  the period distribution, not a reason to move the window; plan 06's kick-law tests (P06.T19.d) and
  plan 08's class-table tests still pass. Acceptance: `just ci` and `just test-slow`. T1.d lands
  after T4.a, which supplies the real threshold.

Files: `units.rs`, `stellar/multiplicity/{mod,model,dist,quadrature,fates}.rs`; edits in
`galaxy/fates.rs`, `galaxy/displaced/binarity.rs`, every golden.

### P11.T2 Hierarchies

- **P11.T2.a Types and the draw.** `SystemHierarchy`, `HierarchyNode`, `StarSlot`,
  `MultiplicityContext`, `RedrawAttempt`, `draw_hierarchy`: multiplicity decision by integer
  threshold on `system.multiplicity`; companion count; for each level, period, mass ratio against
  the mass of the node inside, eccentricity, orientation (isotropic) and mean anomaly at the epoch
  on `binary.orbit`, `binary.orientation`, `binary.phase`; which node a further companion joins on
  `system.hierarchy`. Draw numbers follow Design note 9. Body indices and stream keys follow Design
  note 5. Nothing calls it yet. Tests: the numbering rule gives every pair of 10⁴ hierarchies a
  distinct key; attempt n drawn alone equals attempt n drawn after attempts 0 to n − 1.
- **P11.T2.b Stability and the tidal cut.** The Mardling–Aarseth condition and the half-tidal-radius
  cut as redraws of the outer orbit only, at most 16 (each on the next draw numbers of the same
  attempt block, which 64 leaves room for), then the companion is dropped (counted by a test, under
  1%). `ForcedMultiple { max_separation }` truncates at a cluster's hard–soft boundary, the
  separation at which a pair's orbital speed equals plan 09's `ClusterModel::sigma(r)` at the
  member's radius; T8.f supplies it.
- **P11.T2.c Wire into the system stage (version bump).** `SystemStars::generate` calls
  `draw_hierarchy` with `Free` for grid systems (`generate_in` takes the context for everything
  else); each companion gets a `StarModel` from `StarDraws::for_attempt` on its own body index, with
  the system's composition and age. The primary's model is plan 06's, untouched, at plan 08's
  `record.mark_attempt()`. For primaries of 8 M☉ and up the innermost period is drawn conditional on
  plan 06's stripped mark (Design note 1). `summary_at` and `brief_at` cover all stars. Bump the
  version; regenerate goldens: no primary moves, and a golden test pins that the primaries of plan
  06's pinned IDs are bit-identical.
- **P11.T2.d Brown-dwarf companions (version bump).** Design note 15: the decision and marks on
  `system.substellar`, `MIN_SUBSTELLAR_COMPANION_MASS`, the desert factor, the stability test of
  T2.b, `StarSlot::kind`, state from `stellar::substellar::cooling`, `ObjectKind::Substellar` in
  summaries. The probability by primary mass is a short table whose source the task re-checks and
  cites (candidates: Metchev and Hillenbrand 2009 for Sun-like primaries; Grether and Lineweaver
  2006 for the desert). Tests: no star of any pinned system changes; bound brown dwarfs number
  0.02–0.06 per star over a weighted sample of all layers, which with plan 13's free-floating one
  per five or six gives the brainstorm's one per four or five; fewer than 1% of Sun-like primaries
  have one inside 10³ days; `all_stars_fraction_below` is unchanged to the last bit.

Files: `stellar/multiplicity/{hierarchy,stability,substellar}.rs`, `stellar/system.rs`.

Tests: property tests over 10⁴ IDs: every pair satisfies the criterion, every apocentre is inside
the cut, masses never exceed the primary's, the primary's record is untouched; order independence
through `hyperion_testkit::order::assert_order_independent`; `ForcedSingle` yields one star; golden
hierarchies for six pinned IDs. Acceptance: `cargo test -p hyperion-sim multiplicity::hierarchy` and
the golden suite pass.

### P11.T3 Orbits on rails

- **P11.T3.a Elements, solver, Roche and Peters.** Needs only plan 01, and is the first task of the
  plan to land, because T2.a and T4.a use its types. Build `coords::{SystemVector, SystemVelocity}`
  and `orbit::{KeplerElements, Eccentricity, solve_kepler}` (Newton iteration from a fixed starter,
  fixed iteration count so that results are bit-reproducible, through `math`), `relative_state_at`,
  `roche_lobe_radius`, `peters_merger_time` (with Peters's eccentricity integral as a fixed
  quadrature) and `peters_separation_for`. Tests: `solve_kepler` residual under 10⁻¹² for e up to
  0.99; period closure (state at t and t + P agree to 10⁻⁹ relative); symmetric in time; the
  brainstorm's figure is reproduced: two white dwarfs a thousand years before merging have a period
  of 80–100 s.
- **P11.T3.b Star positions.** After T2.a. `star_positions_at` walks the hierarchy, places each pair
  about its barycentre and returns plan 01's `SystemPosition`s. Tests: the barycentre of every one
  of 10⁴ hierarchies stays at the origin at ±H to 1 m; a position is the same whatever was asked
  before.

Files: `coords.rs`, `orbit/{mod,kepler,peters,roche}.rs`, `stellar/multiplicity/positions.rs`.
Acceptance: `cargo test -p hyperion-sim orbit` and `multiplicity::positions`.

### P11.T4 The binary evolution engine

Source throughout: Hurley, Tout and Pols (2002), section and equation numbers in doc comments.

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
everywhere; Design note 13), registered in `tables::MANIFEST` as provisional; `IaExplosionMark`, the
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
  the index under the spare band value is 13 bits, 8,192 members, and a nuclear cluster of 4–5 × 10⁷
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
own:

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

### P11.T12 Statistical suite

Slow tests, fixed seeds, Milky Way parameters unless stated: multiplicity by primary mass in six
bins against the anchors (chi-square); companion count ratios for Sun-like primaries; period and
mass-ratio Kolmogorov–Smirnov from generated systems, not from the samplers; the all-stars test on
generated systems of all five layers weighted by share, brown dwarfs left out: 76.4 ± 0.5% below 0.5
M☉ under Kroupa, against the observed 75.9%; under Chabrier with `high_mass_scale` 1 about 67%, and
with plan 15's scale (provisionally 0.68; P15.T4.b fits it against `all_stars_fraction_below` and
lands inside the brainstorm's 0.65–0.7) 75.9 ± 1%; a miss under Kroupa is a finding against the
anchors, not a reason to widen the window; blue straggler and hot subdwarf fractions in an old
population against the figures plan 06 uses for its class-fraction tests; a Hertzsprung–Russell dump
of a cluster with binaries for the check by eye.

Files: `crates/hyperion-sim/tests/binaries_statistical.rs`.

Acceptance: `just test-slow` passes; `just ci` stays green.

### P11.T13 Protocol and server

Extend plan 06's `StarSummaryDto` with `body_index` and `binary_class: BinaryClassDto`; add
`OrbitDto` (period, semi-major axis, eccentricity, inclination) and `HierarchyDto` (a flat list of
nodes); `SystemSummaryDto` gains `hierarchy`, evaluated at the request's time; `StellarBriefDto`
gains `star_count`. Every change is an additive field under plan 04's convention, on plan 06's
`system_summary` kind and on the `systems_in_range` rows; no request kind is added. The server
caches `SystemStars` in the byte-bounded system cache that plan 06 instantiated from plan 04's
`ByteLru`, with `HeapBytes` extended to hierarchies and timelines. Run `just gen-protocol`.

Files: `crates/hyperion-protocol/src/*.rs`, `crates/hyperion-server/src/*` (summary handler, cache
sizing), `packages/protocol/src/generated/*`, `packages/protocol/src/index.ts`.

Tests: wire-form tests for each type; a server integration test requests the summary of a pinned
triple and gets three stars and two orbits. Acceptance: `just ci`.

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
  have companions; Sun-like periods peak within 0.1 dex of 10⁵ days; 76.4% of all stars below 0.5 M☉
  under Kroupa against the observed 75.9%, 67% under unscaled Chabrier; bound brown dwarfs at a few
  per hundred stars.
- Benches: `binary_evolve`, `system_full`, `awd_scan_marks` (per host; the target that makes a
  galaxy-wide scan "seconds" is under 300 ns).
- By eye: a cluster's Hertzsprung–Russell diagram shows a binary sequence and blue stragglers; the
  readout of a known cataclysmic variable reads sensibly.

## Generator version

This plan changes generated output in T1.d, T2.c, T2.d, T6, T7, T8, T10 and T15, each with its own
bump and regenerated goldens. T1.d moves every star once, because the mean mass per system changes
the system count (plan 02 lists it among its known future bumps). After that no primary moves: IDs,
positions, primary masses, ages, primary draws, death times and kicks are untouched, except that T7
removes about 10⁻⁴ of grid systems. It reserves: body indices 0–15 for the stellar level, which is
plan 14's slot `0x00`; the domain tags and the event tags 0x0300–0x0306 listed under Provides, with
the rest of 0x0300–0x03FF free for later binary kinds; 64 draw numbers per redraw attempt; the five
`ClassId` values plan 09's registry holds for this plan (2, 3, 5, 6 and 8); `system.substellar`, so
that brown-dwarf companions can be retuned without touching a star; the shapes of `tables::binary`;
`MergedBinaryFate`'s discriminants. Plan 14 can add planets without moving a star.

## Risks and open points

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
