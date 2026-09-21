# Plan 06: Stars: evolution, remnants and classes

- **Milestone:** M2.
- **Depends on:** 03 (placement, `SystemRecord`, range query), 15 (offline tables: the kick rank
  table and the helium correction). Only the last task of the kick law, P06.T19.e, waits on plan 15,
  and plan 15's rank-table task waits on P06.T18 and P06.T19.a–b; every other task runs on the
  provisional tables this plan commits, so nothing blocks (see Consumes and Risks). It builds on 01
  and 02 through 03, and on 04 and 05, which are complete by M2, for the protocol and display tasks.
- **Brainstorm sections covered:** "Systems and stars" (stellar state; multiplicity is plan 11);
  "Covering every class of star" (every row of the gap table except interacting binaries; of the
  helium row only the hook); "Events in time" ("Evolution is free", "A system's own events" and the
  rule on marks that need history, for single stars); "Displaced objects: kicks and runaways" (the
  kick law only: the paragraph "The stellar stage draws the kick from one law behind one
  interface"); the planetary nebula sentence of "Large features"; "Testing" (property tests,
  statistical tests, "Time", "The kick law"); "Runtime and code shape" (the system summary message);
  the caveats of "Sizing the layers" that wait on "what each star is now"; "Open questions" (the
  kick law's four defaults).

## Goal

When this plan is done every system the grid places has a star with a state at any clock time: its
phase, mass, luminosity, radius, temperature, spectral type and luminosity class, its peculiar class
where it has one, its variability, rotation and magnetism, and, once it has died, its remnant with
mass, natal kick, cooling or spin-down. All of it is a pure function of seed, ID and time,
continuous in age, with no age-binned tables: `state = evolve(m, Z, age at epoch + t)`. A star's
death is a clock time. Single-star events (flares, glitches, magnetar bursts, FU Orionis outbursts,
giant eruptions, thermal pulses) come from two generic, random-access, order-independent
constructions that plans 09, 11 and 14 reuse. The galaxy's mean mass per system is recomputed from
the real lifetimes and remnant masses. The server answers a system summary request, the range result
says what each star is now, and the `GALAXY` chart draws living stars and remnants with different
symbols, filters on them, and plots a Hertzsprung–Russell diagram of the query, which is where layer
E's arms finally stand out.

## Scope and non-goals

In scope:

- `hyperion_sim::stellar`: the Hurley, Pols and Tout (2000) formulae for all phases; the wraps
  around them (below 0.1 M☉, 100–150 M☉, before the main sequence, modern winds, modern remnant
  masses, the post-AGB bridge); remnants; classification; variability; rotation and magnetism;
  planetary nebulae; the per-system metallicity draw; the helium-excess hook.
- The kick law behind one interface, its draws, and the kick-law tests of the brainstorm's Testing
  section.
- `hyperion_sim::events`: Poisson bins and the monotone phase, generic. `stellar::events`: the
  single-star kinds.
- Replacing plan 02's provisional lifetimes and remnant masses in the mean-mass quadrature, with a
  generator version bump.
- Protocol: a stellar brief on each range-result row and a system summary message. Server handlers.
  Client: symbols, filter, readout, HR diagram, UX guide edits.

Not in scope:

- Multiplicity, companions, binary orbits and every interacting binary (plan 11). This plan draws a
  provisional companion-stripped mark because the kick law needs it; plan 11 replaces it.
- What a kick does to a position: displaced classes, class tables, runaways, velocities (plan 08).
  The supernova catalogue class, shells, light curves, the LBV catalogue class (plan 09). This plan
  provides the death time, death kind, kick and LBV window those plans test.
- Retarded-time observation and alerts (plan 12). Everything here takes a time and is indifferent to
  whether it is the present or a retarded one.
- The brown dwarf and rogue planet layers (plan 13). The cooling fits they need are built here.
- The fit of the helium correction and the production kick rank table (plan 15).
- Extinction and apparent magnitudes (plan 07). This plan gives absolute magnitudes only.

## Provides

All paths are under `hyperion_sim` unless a crate is named. Signatures are sketches.

### `stellar` core types (`stellar::state`, `stellar::composition`)

```rust
pub struct Composition { /* z: MetalFraction, fe_h: Dex, helium_excess: HeliumExcess */ }
impl Composition {
    pub fn from_fe_h(fe_h: Dex, helium_excess: HeliumExcess) -> Self; // Z = 0.02 × 10^[Fe/H]
    pub fn z(&self) -> MetalFraction;        // as drawn
    pub fn z_fit(&self) -> MetalFraction;    // clamped to 0.0001..=0.03 for the formulae
}
pub enum Phase {
    Protostar, PreMainSequence, MainSequence, HertzsprungGap, FirstGiantBranch,
    CoreHeliumBurning, EarlyAgb, ThermallyPulsingAgb, HeliumMainSequence, HeliumHertzsprungGap,
    HeliumGiantBranch, PostAgb, HeliumWhiteDwarf, CarbonOxygenWhiteDwarf, OxygenNeonWhiteDwarf,
    NeutronStar, BlackHole, NoRemnant, Substellar,
}
impl Phase { pub fn is_remnant(self) -> bool; pub fn is_living(self) -> bool; }
pub struct StarState { /* phase, age, mass, core_mass, envelope_mass, luminosity, radius,
    effective_temperature, mass_loss_rate, phase_fraction; getters; surface_gravity() */ }
```

Units added to plan 01's `units`: `Dex`, `MetalFraction`, `HeliumExcess`, `Gauss`,
`SolarMassesPerYear`. Plan 01 already has `SolarLuminosities`, `SolarRadii`, `Kelvin` and `Years`.

### The backbone (`stellar::sse`)

```rust
pub struct ZCoeffs;                       // every Z-dependent coefficient, built once per Z
impl ZCoeffs { pub fn new(z_fit: MetalFraction) -> Self; }
pub struct Track;                         // one star's evolution: phase segments with mass knots
impl Track {
    pub fn to_age(m0: SolarMasses, comp: &Composition, draws: &StarDraws, age_max: Years) -> Self;
    pub fn full(m0: SolarMasses, comp: &Composition, draws: &StarDraws) -> Self;
    pub fn state_at(&self, age: Years) -> StarState;
    pub fn lifetime(&self) -> Option<Years>;          // None while the build has not reached death
    pub fn death(&self) -> Option<Death>;
    pub fn window_where(&self, pred: PhasePredicate) -> Option<AgeInterval>; // e.g. the LBV window
    pub fn max_radius_until(&self, age: Years) -> SolarRadii;          // monotone; plans 11, 14
    pub fn max_luminosity_until(&self, age: Years) -> SolarLuminosities; // monotone; plan 14
}
pub fn evolve(m0: SolarMasses, comp: &Composition, draws: &StarDraws, age: Years) -> StarState;
pub fn lifetime(m0: SolarMasses, comp: &Composition, draws: &StarDraws) -> Years;
// both re-exported as `stellar::{evolve, lifetime}`; a quadrature with no star passes
// `&StarDraws::median()`
pub fn turn_off_mass(age: Years, comp: &Composition) -> SolarMasses;   // inverse of t_ms; plan 09
pub enum WindRecipe { Hurley2000, Modern }        // Modern is the generator default
pub enum RemnantRecipe { Hurley2000, MandelMuller2020 }
```

`stellar::substellar::cooling(mass, age, comp) -> StarState` is the fit below 0.1 M☉ that plan 13
shares.

### Draws (`stellar::draws`)

`StarDraws::for_star(seed: Seed, star: BodyId) -> StarDraws` holds every fixed draw of one star,
each from its own domain tag (listed under "Generator version"; scope `Body`, registered in plan
01's `domain_tags!` registry in `rng/tags.rs` under a "Plan 06" heading).
`StarDraws::for_attempt(seed, star, attempt: u32)` is the same on a later block of each stream's
draw counter, so that a conditional sampler (plan 09's catalogue classes, plan 11's binaries) can
redraw on the same stream, as the brainstorm asks; attempt 0 is `for_star`.
`StarDraws::from_parts(..)` builds the struct from explicit variates and `StarDraws::median()` is
the fixed median star (η = 0.5, every rank a half), for quadratures and tests. Everything else in
`stellar` is a pure function of `(m0, Composition, StarDraws, age)`, so the quadratures of plans 02,
08 and 15 can integrate over draws of their own without an ID.

### Conventions fixed here for later plans

- **Body index 0 is the primary star** of every system. Plan 01 builds `BodyId` and leaves the
  meaning of the indices open; this plan is the first to open a stream under a `BodyId`, so it fixes
  index 0. Plan 11 numbers companions from 1 and plan 14 numbers planets, moons and rings after the
  stars (its `STAR_BODY_INDEX_END`). A free-floating object of plan 13 is likewise body 0 of its
  system.
- **Event-tag number blocks** in plan 01's 16-bit `event_tags!` registry (`id/event_tags.rs`), where
  0 is invalid and `0x0001` is plan 01's self-test tag: `0x0100`–`0x01FF` single stars (this plan),
  `0x0200`–`0x02FF` features and the galactic centre (plan 09), `0x0300`–`0x03FF` binaries (plan
  11), `0x0400`–`0x04FF` bodies (plan 14). `0x0002`–`0x00FF` and everything from `0x0500` stay
  unallocated. Within a block numbers are explicit, ascending in order of registration and never
  reused. Each event tag names a `DomainTag` of scope `Event` declared in `rng/tags.rs`, as plan
  01's P01.T6.e requires.

### Remnants and kicks (`stellar::remnant`)

```rust
pub struct Death { /* age: Years, kind: DeathKind, progenitor: ProgenitorAtDeath */ }
pub enum DeathKind {
    EnvelopeLoss,                         // becomes a white dwarf, through PostAgb where it applies
    ElectronCapture, CoreCollapse { supernova: SupernovaType }, DirectCollapse,
    PairInstability,                      // leaves NoRemnant
}
pub enum SupernovaType { IIP, IIL, IIb, Ib, Ic, None }     // from the envelope at death
pub struct ProgenitorAtDeath { /* co_core_mass, helium_core_mass, envelope_mass, stripping */ }
pub enum Stripping { None, Wind, Companion }
pub enum CollapseChannel { IronCore, ElectronCapture, AccretionInduced }

pub trait KickLaw {
    fn kick(&self, channel: CollapseChannel, progenitor: &ProgenitorAtDeath,
            remnant: &CompactRemnant, draws: &KickDraws) -> NatalKick;
}
pub struct StandardKickLaw;               // the generator version's law; params in KickLawParams
pub struct KickLawParams { /* ln_mu: 5.60, ln_sigma: 0.68, score_scatter: 0.45, low_sigma_km_s: 5.0,
    low_ramp: (2.0, 3.0), bh_factor: 0.75, ec_window_single: 0.1, ec_window_stripped: 1.0,
    wd_sigma_km_s: 1.0, rank_clamp: (0.001, 0.999), stripped_share: 0.25 (provisional; plan 11);
    Default is the generator version's law */ }
pub struct KickDraws { /* score normals, mode mark, three low-mode normals, direction */ }
impl KickDraws { pub fn of(draws: &StarDraws) -> Self; pub fn from_parts(..) -> Self; }
pub struct CompactRemnant { /* kind: RemnantKind, mass: SolarMasses */ }  // from `Track::death`
pub enum RemnantKind { WhiteDwarf, NeutronStar, BlackHole, None }        // and the remnant draws
pub struct NatalKick { /* speed: MetresPerSecond, direction: UnitVector (galactic axes),
    mode: KickMode */ }
pub enum KickMode { Ordinary, Low, FallbackNone, WhiteDwarf }
pub fn ordinary_score(co_core: SolarMasses, remnant: SolarMasses, xi: f64) -> f64;
pub struct KickRankTable;                 // F_x, interpolating `tables::kick_rank`
pub mod reference {
    pub struct ReferencePopulation;       // the population F_x is tabulated over (P06.T19.b)
    pub fn score_quantiles(pop: &ReferencePopulation, n: u64, seed: Seed) -> [f64; 257];
    pub struct KickObservables { /* the six figures of P06.T19.d */ }
    pub fn kick_observables(params: &KickLawParams, table: &KickRankTable, n: u64, seed: Seed)
        -> KickObservables;               // pure; the slow tests and plan 15's P15.T5.b call it
}
```

Table shapes fixed by this plan and filled by plan 15, which restates them in its P15.T5 and P15.T7
(README's header convention now, plan 15's `@provisional` header grammar once its registry exists):

- `tables::kick_rank`: `SCORE_QUANTILES: [f64; 257]`, the ordinary kick score at ranks i ÷ 256 over
  the reference population, strictly increasing; and the law's four defaults as constants that
  `KickLawParams::default` reads, `LOW_RAMP: (f64, f64)` (2.0, 3.0 M☉ of carbon–oxygen core),
  `BH_FACTOR` (0.75), `EC_WINDOW_SINGLE` (0.1 M☉) and `EC_WINDOW_STRIPPED` (1.0 M☉). Plan 15 adds
  plan 11's `CLUSTER_MERGED_BINARY_FATE` beside them.
- `tables::helium`: `LIFETIME_SLOPE`, the coefficients of s(m, Z), a quadratic in log mass at four
  metallicities, interpolated in log Z; and `HB_TEMPERATURE_SHIFT`, linear in envelope mass. Every
  coefficient zero is the identity.

### Classification and derived properties

- `stellar::classify::{SpectralType, LuminosityClass, PeculiarClass, Classification, classify}`;
  `Classification` implements `Display` (`G2V`, `M5III`, `WC7`, `DA4.2`, `sdM3`, `T6`).
- `stellar::photometry::{bolometric_correction_v, absolute_magnitude_v, colour_b_v}`.
- `stellar::variability::{Variability, VariableKind, variability, light_factor_at}`.
- `stellar::rotation::{Rotation, Magnetism, ActivityLevel, SpinAxis}`.
- `stellar::nebula::{PlanetaryNebula, planetary_nebula}`.

### `math` addition

`math::normal_quantile(p: f64) -> f64`, the inverse of the standard normal distribution function for
0 < p < 1 (P06.T19.a). Plan 01's `math` has none, and its testkit has only `stats::normal_cdf`.

### Events (`events`, generic; `stellar::events`, single stars)

```rust
// events
pub use crate::id::{EventTag, EventBin};  // plan 01's; this plan's tags are entries of its
pub use crate::rng::EventKey;             // `event_tags!` registry in `id/event_tags.rs`
pub mod tags { /* re-exports */ }
pub struct TimeWindow { /* start, end: UniverseTime; half-open */ }
pub struct PoissonBins { /* bin_seconds: i64, look_back_bins: u32 */ }
pub trait RateModel { fn bound(&self, bin: i64) -> f64; fn rate(&self, t: UniverseTime) -> f64; }
pub struct BinEvent { /* id: EventId, time: UniverseTime, marks: Stream */ }
impl PoissonBins {
    pub fn events_in(&self, key: &EventKey, tag: EventTag, w: TimeWindow, r: &impl RateModel,
                     out: &mut Vec<BinEvent>);
    pub fn active_at(&self, key: &EventKey, tag: EventTag, t: UniverseTime, r: &impl RateModel,
                     out: &mut Vec<BinEvent>);   // looks back `look_back_bins`
    pub fn event(&self, key: &EventKey, id: EventId, r: &impl RateModel) -> Option<BinEvent>;
}
pub trait PhaseClock { fn base_phase(&self, t: UniverseTime) -> f64;
                       fn time_at(&self, phase: f64) -> UniverseTime; }
pub struct LinearClock { /* period, origin */ }
pub struct MonotonePhase { /* amplitude, lattice_cycles, octaves, skip: Option<SkipMark> */ }
pub struct CycleEvent { /* id: EventId, cycle: i64, time: UniverseTime, marks: Stream */ }
impl MonotonePhase {
    pub fn phase_at(&self, key: &EventKey, c: &impl PhaseClock, t: UniverseTime) -> f64;
    pub fn events_in(&self, key: &EventKey, tag: EventTag, c: &impl PhaseClock, w: TimeWindow,
                     out: &mut Vec<CycleEvent>);
    pub fn event(&self, key: &EventKey, tag: EventTag, c: &impl PhaseClock, cycle: i64)
                 -> Option<CycleEvent>;          // None if the skip mark removes it
    pub fn cycle_at(&self, key: &EventKey, c: &impl PhaseClock, t: UniverseTime) -> (i64, f64);
}
// stellar::events
pub enum StarEventKind { Flare, Glitch, MagnetarBurst, MagnetarGiantFlare, FuOrionisOutburst,
                         GiantEruption, ThermalPulse }
pub struct StarEvent { /* id: EventId, kind, onset: UniverseTime, duration, magnitude fields */ }
pub fn events_in(star: &StarModel, w: TimeWindow, out: &mut Vec<StarEvent>);
pub fn active_at(star: &StarModel, t: UniverseTime, out: &mut Vec<StarEvent>);
```

### System assembly (`stellar::system`)

```rust
pub fn draw_metallicity(galaxy: &Galaxy, record: &SystemRecord) -> Composition;
pub struct StarModel;      // m0, Composition, StarDraws, Track: state at the epoch, cacheable
pub struct SystemStars;    // today one StarModel (the primary); plan 11 adds companions
impl SystemStars {
    pub fn generate(galaxy: &Galaxy, record: &SystemRecord) -> Self;
    pub fn summary_at(&self, t: UniverseTime) -> SystemSummary;
    pub fn brief_at(&self, t: UniverseTime) -> StellarBrief;          // cheap: for range rows
    pub fn death_time(&self) -> ClockDeath;   // T = lifetime − age at the epoch
    pub fn natal_kick(&self) -> Option<NatalKick>;
    pub fn lbv_window(&self) -> Option<(UniverseTime, UniverseTime)>;
}
pub enum ClockDeath { At(UniverseTime, DeathKind), BeyondClockRange, AlreadyRemnantAtBirth }
pub enum SystemExistence { NotYetBorn, Exists }   // From<placement::Existence> (plan 03's
                                                  // `NoSystemYet`, `Exists`); decided by
                                                  // `SystemRecord::existence_at`
pub struct SystemSummary { /* existence, composition, stars: Vec<StarSummary> */ }
pub struct StarSummary { /* state, classification, photometry, variability, rotation, magnetism,
    remnant detail, nebula, active events, death within the clock window */ }
pub struct StellarBrief { /* kind: ObjectKind, class: Classification, log_luminosity, teff */ }
pub enum ObjectKind { Protostar, PreMainSequence, Dwarf, Subgiant, Giant, Supergiant, WolfRayet,
                      HotSubdwarf, WhiteDwarf, NeutronStar, BlackHole, NoRemnant, Substellar }
```

`stellar::fates::TrackFates` implements plan 02's `galaxy::fates::StellarFates` for the mean-mass
quadrature. That trait takes a mass and no metallicity, so a `TrackFates` is built at one
metallicity, `TrackFates::at(fe_h: Dex)`, and plan 02's quadrature, which already runs once per
population, is handed the one for that population's reference [Fe/H] (P06.T30). `mean_companions`
delegates to plan 02's `ProvisionalFates` until plan 11.

### Protocol (`hyperion-protocol`, mirrored in `@hyperion/protocol`)

Everything here extends plan 04's request convention as its "Extending the convention" prescribes:
no new `ClientMessage` or `ServerMessage` variant, one new variant on each of `RequestBody` and
`ResponseBody` under the kind plan 04 reserved, its string in `REQUEST_KINDS`, failures as
`request_error`.

- `StellarBriefDto` as an optional field `stellar: Option<StellarBriefDto>` on each row of plan 04's
  `SystemsInRange` (its wire `SystemRecord`), and an optional request field `include_stellar: bool`
  on `SystemsInRangeRequest`, both `#[serde(default)]`.
- Kind `system_summary`: `RequestBody::SystemSummary(SystemSummaryRequest)` with
  `SystemSummaryRequest { universe: UniverseIdHex, system: SystemIdHex, time: UniverseTime }`, and
  `ResponseBody::SystemSummary(SystemSummaryDto)`, which echoes `universe`, `system` and `time`.
  With it `StarSummaryDto`, `RemnantDto`, `VariabilityDto`, `StarEventDto`, `ObjectKindDto`.
- `ErrorCode::UnknownSystem` (`unknown_system`): a well-formed `SystemIdHex` that plan 03's
  `resolve` refuses. Plan 14 reuses it.

### Client (`apps/hyperion`)

- `lib/galaxy/starSymbols.ts`: the mapping `ObjectKind → SymbolShape` and its legend entries;
  `SymbolShape` gains `"ringed-circle"`; a `STARS` filter (`ALL`, `LIVING`, `REMNANTS`) on the local
  chart; `SystemReadout` extended with the summary through plan 05's
  `useServerRequest<"system_summary">`; `displays/galaxy/HrDiagram.tsx` and its pure projection
  helpers in `lib/galaxy/hrProjection.ts`.

### Test helpers

- `stellar::testing::{sample_population, hr_sample}`, behind the `testing` feature and `cfg(test)`:
  draws stars of one population at given galaxy parameters without placement.
- `crates/hyperion-sim/tests/data/sse/`: reference vectors from the published SSE code.
- `events::testing::assert_partition_independent`: the union of an event listing over any partition
  of a window equals the listing over the whole. Order of asking is checked with plan 01's
  `hyperion_testkit::order::assert_order_independent`.
- Golden files follow plan 01's convention: `hyperion_testkit::golden!` and
  `crates/hyperion-sim/tests/golden/stellar/<name>.golden`.

## Consumes

- **Plan 01:** `math` (every `ln`, `exp`, `powf`, `log10`, `sin`, `cos`, `erfc` here goes through
  it); `rng::{Seed, Stream, DomainTag, TagScope, ObjectKey}` with `Stream::open(seed, tag, object)`,
  `seek` and `word_at`, and the single `domain_tags!` registry in `rng/tags.rs`, to which this
  plan's tags are added with scope `System` (`system.metallicity`) or `Body` (every `star.*` tag);
  the samplers (uniform, normal, log-normal, Poisson, power law), integer-threshold decisions
  (`Mark`, `Threshold`), and the two-step event key `rng::EventKey` (`derive(seed, tag, subject)`,
  `bin_stream(bin)`, `event_stream(bin, j)`); `units`; `time` (`UniverseTime`, `Span`,
  `CLOCK_WINDOW_H`, `LIGHT_CROSSING_L`, `SourceHorizon`); `coords` (galactic axes for kick and spin
  directions); `id` (`SystemId`, `BodyId`, `EventId`, `EventTag`, `EventBin`, `EventSubject`, the
  event word's layout of a 16-bit tag, a signed 40-bit number and an 8-bit index, and the
  `event_tags!` registry, whose entries each name a `DomainTag` of scope `Event`);
  `GENERATOR_VERSION`; from the `hyperion-testkit` crate the `golden!` harness,
  `order::assert_order_independent` and `stats` (chi-square, Kolmogorov–Smirnov, Poisson interval,
  `normal_cdf`); slow-test marking, `just test-slow`, `just bench`, `just bless`. Plan 01 has no
  normal quantile, and P06.T19.a adds one to `math` under plan 01's rules for that module.
- **Plan 02:** `galaxy::Galaxy`; `Component::metallicity(&PointLy, age)`, which returns a
  `FehDistribution` (mean and sigma of [Fe/H] for a population or halo component); the age
  distributions and `imf::MassFunction` for test sampling and count tests;
  `galaxy::fates::{StellarFates, ProvisionalFates, mean_present_mass}`, the seam through which the
  mean-mass quadrature reads lifetimes and remnant masses.
- **Plan 03:** `galaxy::placement::{SystemRecord, Existence, resolve, ResolveSystemError}` (the
  record's `id`, `epoch_position`, `origin` (`SystemOrigin::Grid(ComponentId)` for every record of
  this plan, so `component()` is `Some`), `population`, `primary_initial_mass`, `age_at_epoch`,
  `age_at(t)` and `existence_at(t)`); `galaxy::query::{RangeQuery, RangeResult, SystemHit}`.
- **Plan 04:** the request convention (`RequestId`, `RequestBody`, `ResponseBody`, `REQUEST_KINDS`,
  `RequestError`, `ErrorCode`, the reserved kind `system_summary` and the rules of "Extending the
  convention"); `SystemIdHex`, `UniverseIdHex`, the wire `UniverseTime`; `SystemsInRangeRequest`,
  `SystemsInRange` and its row (the protocol's `SystemRecord`); the universe registry,
  `compute::CpuPool`, `cache::{ByteLru, HeapBytes}` (plan 04's design note 23 leaves the
  systems-level cache for this plan to instantiate, keyed with `(seed, generator_version)` like
  every other cache); `RequestClient` and `RequestChannel`; the test helpers `TestServer`,
  `TestClient` and `FakeWebSocket`.
- **Plan 05:** the general spatial view, `spatial/marks.ts` (`PointMark`, `SymbolShape`, whose
  values `diamond`, `square` and `triangle` plan 05's D14 defines and reserves for later types),
  `spatial/symbols.ts` (`symbolOutline`), `useServerRequest` and `RequestStatus`, `SystemList`,
  `SystemReadout`, `SymbolLegend`, `chartModel.ts`, `lib/galaxy/{model.ts, wire.ts}`, `SunGlyph`,
  `UnitLabel`, `lib/format.ts`, and the test helpers `galaxyFixtures.ts` and `RecordingContext2D`.
- **Plan 15:** nothing that a task here waits for except in P06.T19.e. Plan 15 fills
  `tables::kick_rank` and `tables::helium` in the shapes this plan commits (see Provides) and takes
  the files over, and its tools call this plan's
  `stellar::remnant::reference::{ReferencePopulation, score_quantiles, kick_observables}`,
  `KickLawParams`, `StandardKickLaw`, `stellar::sse`, `stellar::sse::lifetime` and
  `SystemStars::lbv_window`. So the order is: P06.T17 and P06.T19.b commit the identity and
  provisional tables; P06.T18 and P06.T19.a–d land; plan 15's P15.T5.a and P15.T5.b run against
  them, P15.T5.a writing the production table to a scratch path; P06.T19.e is the commit that swaps
  it in, with the version bump. The helium fit (P15.T7) is scheduled for M3 and replaces the
  identity table without any change here. Plan 15's registry (its P15.T2) registers tables that
  already exist as provisional, so this plan does not wait for plan 15's toolchain either.

## Design notes

1. **A track is built on a grid that does not depend on the query.** The Hurley formulae are closed
   forms in mass and age only while mass is constant. With mass loss the published SSE code steps
   through time with an adaptive step, which would make the state at an age depend on how the steps
   fell, and so fail "continuous in age". Here a star's `Track` is a sequence of phase segments. A
   segment with mass loss carries a fixed number of knots, found by a fixed-order integration of the
   wind recipe with a fixed number of steps between knots. The knots sit at fixed values of a
   coordinate that runs from 0 to 1 whatever the mass does. On the main sequence, in the Hertzsprung
   gap and on the helium main sequence it is HPT's own fractional age τ of the phase, which their
   effective-age rule preserves when the mass changes, so the integration is of dM ÷ dτ = Ṁ ×
   t_phase(M). From the giant branch onward, where the formulae keep the mass the phase was entered
   with, the phase's duration is fixed at entry and the coordinate is the fraction of it (HPT
   section 7.1; P06.T10.c re-checks which phases update the initial mass). A segment that mass loss
   ends early (the envelope is gone before the nominal end) stops at the root of the envelope mass
   on the knot interpolant, found by a bisection of fixed iteration count. `state_at` interpolates
   mass, effective initial mass and effective age between knots and evaluates the phase's closed
   forms. Nothing in the grid reads the age asked for or the clock: it is a pure function of
   `(m0, Composition, StarDraws)`, so the state is a continuous function of age and the same
   whatever was asked before. The knots are the nodes of a quadrature over one star's own history,
   not a table binned by age, and nothing is shared between stars, which is what the brainstorm's
   rule forbids. A segment without significant mass loss (the main sequence below about 10 M☉, where
   SSE's own recipe gives none) has no knots and costs nothing to build. The knot and step counts
   are constants of the generator version, sized so that a full track costs a few hundred
   evaluations of the closed forms (see the benchmark targets) and checked for convergence in
   P06.T10.c.
2. **Tracks build lazily and the prefix is exact.** `Track::to_age(a)` stops once it has passed `a`.
   Its segments are bit-identical to the same segments of `Track::full`, because each segment
   depends only on those before it. Three quarters of stars are M dwarfs on the main sequence, and
   for them a track is `ZCoeffs` plus three timescales.
3. **Every junction is continuous, except a death that explodes or collapses.** Where the formulae
   jump (the helium flash from the tip of the giant branch to the horizontal branch; the end of the
   AGB to a white dwarf) a bridge segment interpolates log L, log R and mass over a stated physical
   time: 10⁴ years for the flash, and the post-AGB crossing of note 8. The only discontinuities in
   state are `ElectronCapture`, `CoreCollapse`, `DirectCollapse` and `PairInstability`, which are
   deaths with a clock time, and the continuity test excludes exactly those instants.
4. **Age zero is the onset of collapse**, as the brainstorm says, so the formulae's clock starts at
   the star's arrival on the zero-age main sequence, `t_zams(m, Z)`, and `lifetime` includes it.
5. **Metal fraction.** Z = 0.02 × 10^[Fe/H], the solar value the formulae were fitted with. [Fe/H]
   is kept as drawn for the consoles and for plan 14; the formulae see Z clamped to 0.0001–0.03.
   Alpha enhancement is a mark of the population and halo component (plan 02) and does not enter the
   fits.
6. **Modern winds are the set current population-synthesis codes use**, of which the brainstorm
   names Vink et al. (2001): Vink for hot hydrogen-rich stars (12,500–50,000 K, both sides of the
   bi-stability jump, scaled as Z^0.85); 1.5 × 10⁻⁴ M☉ per year beyond the Humphreys–Davidson limit
   (Belczynski et al. 2010); Hamann and Koesterke (1998) scaled by Z^0.86 (Vink and de Koter 2005)
   for helium stars; and Hurley's own choices elsewhere (Kudritzki and Reimers on the giant branch,
   Vassiliadis and Wood on the AGB, Nieuwenhuijzen and de Jager for cool luminous stars). The
   original recipe stays as `WindRecipe::Hurley2000` so that the backbone can be validated against
   the published SSE output.
7. **Reimers η is drawn per star**, normal with mean 0.5 and σ 0.07 (McDonald and Zijlstra 2015 give
   0.477 ± 0.070; re-check), held positive. Without a spread every star of one cluster lands on one
   point of the horizontal branch and no RR Lyrae population has the right width. The helium hook
   moves the same branch bluewards.
8. **The post-AGB bridge** is what planetary nebulae need and what makes the end of the AGB
   continuous. After envelope loss the star crosses to high temperature at constant luminosity in a
   time that falls steeply with core mass (about 10⁴ years at 0.55 M☉ to under 10³ at 0.7; the task
   fits a closed form to Miller Bertolami 2016 and records it), then joins the white dwarf cooling
   law. Slow, low-mass cores reach ionising temperatures after their nebula has dispersed, so "lazy"
   stars have no planetary nebula without any extra rule.
9. **White dwarf mass is the core mass the track ends with**, not a separate relation, so lifetime,
   nebula and remnant agree. It is tested against the semi-empirical relation of Cummings et al.
   (2018) to 0.08 M☉; a failure is a finding against the AGB wind, not a reason to bolt on a second
   relation.
10. **Pair instability.** Mandel and Müller (2020) stop below it and the brainstorm is silent. At
    the lowest metallicities a 100–150 M☉ star can end with a helium core above 45 M☉. The generator
    default follows Belczynski et al. (2016, A&A 594, A97; re-check the core-mass limits): a helium
    core of 45–65 M☉ pulses down to a 40.5 M☉ black hole, and one of 65–135 M☉ leaves `NoRemnant`.
    This touches a few thousand long-dead halo and thick-disc systems. Flagged under Risks.
11. **The companion-stripped mark is provisional.** The low kick mode belongs to electron capture,
    to accretion-induced collapse and to companion-stripped progenitors, but companions arrive in
    plan 11. Until then each star of 8 M☉ or more draws `Stripping::Companion` with probability
    `KickLawParams::stripped_share` (default 0.25, between the 20% and 33% mixes of the brainstorm's
    scratch Monte Carlo) on its own stream. The mark widens the electron-capture window and selects
    the low-mode ramp; it does not change the track. Plan 11 replaces the constant with the
    quadrature over periods and mass ratios and draws its binaries conditional on the mark, so the
    mark's stream and meaning are reserved now.
12. **Electron-capture windows are in initial mass** and end at the lowest initial mass that makes
    an iron core, `m_cc(Z)` (in Hurley's terms, the mass whose core at the base of the AGB is 2.25
    M☉): `[m_cc − 0.1, m_cc)` for a single star, `[m_cc − 1.0, m_cc)` for a companion-stripped one.
    The brainstorm gives widths only; see Risks.
13. **Neutron-star birth laws.** The brainstorm says birth spin and field "are drawn" and gives no
    distribution. The field is one log-normal with decay, so magnetars are the high tail of the
    birth field and fall out of the draw, as the brainstorm's row asks, and are not a separate roll:
    log₁₀ B₀ normal about 13.25 with σ 0.6 and the decaying-field population synthesis of Popov et
    al. (2010, MNRAS 401, 2675); birth periods normal about 300 ms with σ 150 ms (Faucher-Giguère
    and Kaspi 2006, ApJ 643, 332). The two come from different syntheses, so P06.T21.a re-checks
    that the pair still reproduces the observed period and period-derivative plane and records what
    it settles on. Flagged under Risks.
14. **An event may change what a console reads, never what the track is.** `summary_at` applies the
    transient factor of active events (an FU Orionis outburst's luminosity, a glitch's recovering
    frequency step) on top of the track's state. Mean effects are already in the closed forms: the
    LBV wind rate includes the mean mass of giant eruptions, spin-down includes mean glitch
    activity, and carbon enrichment is a closed form in the thermal pulse number.
15. **Bins and cycles are integers of seconds.** A Poisson bin is `[kΔ, (k + 1)Δ)` with Δ a whole
    number of seconds, so bin membership is exact integer arithmetic on `UniverseTime`. The event
    word's 8-bit index caps a bin at 255 events: a kind chooses Δ so that its bound × Δ is at most
    64, which puts the overflow probability below 10⁻⁶⁰, and a count above 255 is clamped with a
    debug assertion. The 40-bit number covers the source horizon for Δ or a period down to 16 s.
16. **The monotone phase** is Φ(t) = φ(t) + N(φ(t)), with φ the clock's base phase (t ÷ P for a
    fixed period) and N a sum of J octaves of value noise: N(x) = Σⱼ a × 2^(j ÷ 2) × vⱼ(x ÷ (ℓ ×
    2ʲ)), each vⱼ a cubic interpolation of hashed lattice values in [−1, 1] under the object's event
    key. The amplitudes grow as a random walk's would, so the phase diffuses up to the largest
    octave, which is sized to span the source horizon; the slopes fall as 2^(−j ÷ 2), so |N′| is at
    most 10.3 × a ÷ ℓ, and the constructor rejects parameters for which that exceeds 0.9. Φ is then
    strictly increasing and events cannot change order. Event n is the root of Φ(t) = n, bracketed
    by the amplitude bound and found by bisection to one nanosecond, which is deterministic. This is
    the midpoint-displacement idea of the research notes in a form that needs no recursion.
17. **Symbols.** Size stays tied to the mass layer and fill to the side of the reference plane, as
    the brainstorm fixes, so shape is the only free channel. Five outlines, each readable filled or
    open: circle (protostar, pre-main-sequence, dwarf, subgiant, hot subdwarf, substellar), circle
    with an outer ring (giant, supergiant, Wolf-Rayet), diamond (white dwarf), triangle (neutron
    star), square (black hole). Plan 05's D14 already defines the outlines of the last three in
    `symbols.ts`; the ringed circle is a new `SymbolShape` value, `ringed-circle`, whose inner disc
    alone takes the fill so that it still reads filled or open. A system that left no remnant is
    listed but not drawn. The spectral class goes in the list and readout in words, so shape is
    never the only signal.
18. **The HR diagram is a panel of the `GALAXY` display**, not a tool, because the brainstorm wants
    no separate developer viewer. It plots the current range result and so costs no new request.
19. **Remnants at the epoch need their past.** A star already dead at the epoch still builds its
    full track, because the white dwarf's mass and cooling age and the neutron star's age come from
    it. That is the expensive case (see benchmarks), and the server's cache holds the result.
20. **Black-hole spins.** The brainstorm's row says "mass and spin" and no more. A black hole from a
    single star is born slowly rotating if angular momentum is carried efficiently from core to
    envelope: a dimensionless spin near 0.01 (Fuller and Ma 2019, ApJL 881, L1). The default is a
    half-normal with σ = 0.1, which keeps most spins small and leaves a tail. Plan 11 owns the
    routes to high spin (tidal spin-up, accretion). Flagged under Risks.
21. **White-dwarf atmosphere fractions.** The brainstorm gives the scheme, an atmosphere draw "whose
    odds depend on temperature", and no numbers. The helium-atmosphere fraction against temperature
    follows the spectral-evolution measurements of Cunningham et al. (2020, MNRAS 492, 3540) below
    20,000 K and Bédard et al. (2020, ApJ 901, 93) above; the polluted fraction of 25– 50% follows
    Koester, Gänsicke and Farihi (2014, A&A 566, A34). P06.T20.b re-checks each figure and records
    the one it uses. Flagged under Risks.
22. **Event rates.** The brainstorm gives rates for FU Orionis outbursts only. The others are
    generator defaults from the sources it lists: flare frequency distributions by activity level
    from Kepler and TESS surveys of M and Sun-like dwarfs (the task picks and records one; Davenport
    2016 and Günther et al. 2020 are candidates); glitch activity from Fuentes et al. (2017) and
    glitch statistics from Melatos et al. (2008); FU Orionis rates as the brainstorm states them
    (Contreras Peña et al. 2019); magnetar burst and giant-flare rates and the giant-eruption rate
    of luminous blue variables have no source in the brainstorm, so the task names one or documents
    the figure as ours. Flagged under Risks.
23. **Age is an `f64` of years, and fast phenomena never read it.** A 10 Gyr age resolves about a
    minute, which plan 01 records as this plan's concern. Evolution is slow, so `state_at` takes
    plan 03's `SystemRecord::age_at(t)`. Everything periodic or sudden (pulse phase, pulsation
    phase, event bins and cycles) takes the `UniverseTime` itself, with frequencies and rates
    evaluated at the epoch age, so no fast phase is ever formed by subtracting two large ages.

## Tasks

Phases A and F depend only on plans 01–03 and can start together, except that T28 needs T10. Within
phase B the order is T4, T5, T6, then T7–T9 and T11 in parallel, then T10, then T12. Phases C, D and
E can run in parallel with each other once T10 is done, except where a task names another: T19.a
needs T18; T20 needs T16; T24.a needs T9 and T10; T24.b needs T28.f; T28.a needs T25; T28.e needs
T24.a; T28.g needs the rest of T28. Phase G needs B–F. Phase H needs T29; its protocol task can be
written against the types of T1 as soon as they exist. T19.e is the only task that waits on another
plan (plan 15's P15.T5.a) and is done last; the plan is otherwise complete without it.

Every task that turns a figure into code re-checks it against the named source and cites it in the
doc comment. Every public item gets rustdoc with units and valid ranges, per `rust-dev.md`.

### Phase A: foundations

#### P06.T1 Module skeleton, state types and units

- **Build:** the `stellar` module tree (`state`, `composition`, `sse`, `substellar`, `premain`,
  `remnant`, `classify`, `photometry`, `variability`, `rotation`, `nebula`, `events`, `system`,
  `fates`, `draws`, `testing`) with `//!` docs. `Phase`, `StarState`, `Composition`, `ObjectKind` as
  under Provides. `StarState::effective_temperature` is derived from L and R by Stefan–Boltzmann
  with T☉ = 5,772 K (IAU 2015 nominal values; cite). The missing unit newtypes. Replace nothing of
  the existing `Simulation` stub.
- **Files:** `crates/hyperion-sim/src/stellar/mod.rs`, `state.rs`, `composition.rs`,
  `crates/hyperion-sim/src/units.rs`, `lib.rs`.
- **Tests:** `Composition::from_fe_h(0)` gives Z = 0.02; clamping at both ends; the Sun's L and R
  give 5,772 K to 1 K; `Phase::is_remnant` and `is_living` partition the variants (exhaustive
  match).
- **Accept:** `cargo test -p hyperion-sim stellar::state` passes; `just ci` green.

#### P06.T2 Per-star draws and reserved streams

- **Build:** `StarDraws::for_star(seed, BodyId)`: one struct of fixed draws, each read from its own
  domain tag (the list under "Generator version"), none depending on time or on another draw.
  `for_attempt(seed, body, attempt)` reads the same tags with the draw counter offset by attempt ×
  64, so attempt 0 is `for_star` and a redraw never touches another tag. Fields are typed
  (`UnitUniform`, `StandardNormal`, `UnitVector`), not bare `f64`. `StarDraws::from_parts` for
  quadratures and tests, and `StarDraws::median()`. Directions use two uniforms (z and azimuth) in
  galactic axes. Register every `star.*` tag of "Generator version" (scope `Body`) and
  `system.metallicity` (scope `System`) in plan 01's `domain_tags!` registry, `rng/tags.rs`, under a
  "Plan 06" heading; streams are opened with `Stream::open(seed, tag, ObjectKey::from(body))` and
  redraws use `Stream::seek`.
- **Files:** `stellar/draws.rs`, `rng/tags.rs`.
- **Tests:** golden values for three pinned `(seed, BodyId)`; adding a field with a new tag leaves
  the pinned values unchanged (the test reads fields by tag); order independence (A then B equals B
  alone, through `hyperion_testkit::order::assert_order_independent`); the registry's compile-time
  collision assertion covers the new tags.
- **Accept:** golden file `tests/golden/stellar/star_draws.golden` committed and passing.

#### P06.T3 Metallicity draw per system

- **Build:** `stellar::system::draw_metallicity(galaxy, record)`. It is defined for grid records
  only (`SystemOrigin::Grid`; plan 09's members bring their own `Composition` and never reach it),
  so `record.component()` is `Some(c)`; a `None` is a `debug_assert!` and falls back to the
  population's first component. The record's component is `galaxy.fields().component(c)`, and its
  `metallicity(&point, record.age_at_epoch())`, with `point` the `PointLy` of the record's epoch
  position, returns the `FehDistribution` for that population or halo component, place and age.
  [Fe/H] = its mean plus its sigma times one standard normal on the tag `system.metallicity`, keyed
  by `SystemId`. A system not yet born at the epoch (negative age) reads the field at age zero.
  Helium excess is zero for every grid system. Returns `Composition`.
- **Files:** `stellar/system.rs`.
- **Tests:** over 10⁵ sampled thin-disc records at Milky Way parameters, the radial gradient fits
  −0.05 dex per kpc within the field's own stated tolerance; the halo's two main components separate
  in [Fe/H]; K–S against the field's normal at one fixed position; golden for pinned IDs.
- **Accept:** the slow test passes under `just test-slow`; goldens pass.

### Phase B: the Hurley, Pols and Tout backbone

All of phase B implements Hurley, Pols and Tout (2000, MNRAS 315, 543; "HPT" below) from the paper.
Section numbers are those of the journal version. The published SSE Fortran is used only to produce
reference output (T12) and to settle a suspected misprint, which the doc comment then records. No
code is copied from it.

#### P06.T4 Metallicity coefficients and the zero-age main sequence

- **P06.T4.a Coefficient tables.** Transcribe the appendix coefficients of HPT (each `a_n`, `b_n` as
  its polynomial in ζ = log₁₀(Z ÷ 0.02)) and the ZAMS coefficients of Tout et al. (1996; MNRAS 281,
  page 257) into `const` arrays. Files: `stellar/sse/coeffs_data.rs`. Tests: array lengths; a
  checksum test over each table so that an accidental edit fails. Accept: compiles, checksums
  pinned.
- **P06.T4.b `ZCoeffs::new`.** Evaluate every coefficient for a given Z, with the special cases and
  clamps the appendix lists, and the critical masses of HPT section 5: `m_hook`, `m_hef` (helium
  flash), `m_fgb`. Files: `stellar/sse/coeffs.rs`. Tests: at Z = 0.02, `m_hook` ≈ 1.02, `m_hef` ≈
  1.99, `m_fgb` ≈ 13.0 M☉ from the closed forms (re-check in the paper); each critical mass is
  monotone and continuous in Z across 0.0001–0.03 at 200 points. Accept: tests pass; a Criterion
  bench reports the cost (target under 5 µs).
- **P06.T4.c ZAMS luminosity and radius.** `zams::luminosity(m, &ZCoeffs)` and `zams::radius` (Tout
  et al. 1996 equations 1 and 2). Tests: 1 M☉ at Z = 0.02 gives about 0.70 L☉ and 0.89 R☉; both are
  continuous and L is monotone in mass over 0.1–100 M☉ for five metallicities. Accept: tests pass.

#### P06.T5 Main sequence

- **P06.T5.a Timescales.** `t_bgb`, `t_hook`, `t_ms` (HPT section 5.1). Tests: `t_ms`(1 M☉, 0.02) is
  within 3% of 11.0 Gyr, and 0.75 M☉ exceeds 13.8 Gyr at every Z (the brainstorm's reason for the
  B/C boundary); monotone decreasing in mass.
- **P06.T5.b Terminal main sequence and base of the giant branch.** `l_tms`, `r_tms`, `l_bgb`.
  Tests: continuity in mass at 200 points per metallicity; `l_tms > l_zams`.
- **P06.T5.c Evolution along the main sequence.** L(τ) and R(τ) with α_L, β_L, α_R, β_R, γ, the hook
  terms ΔL and ΔR, and the low-mass radius floor. Tests: equals ZAMS at τ = 0 and the terminal
  values at τ = 1 to 10⁻⁹; continuous in τ and in mass, including across `m_hook` and the piecewise
  boundaries of each α and β (a sweep asserting a Lipschitz bound, which is how transcription slips
  show); the Sun at 4.57 Gyr gives 1.0 ± 0.05 L☉ and 1.0 ± 0.03 R☉.
- **Files:** `stellar/sse/ms.rs`. **Accept:** all of the above pass under
  `cargo test -p hyperion-sim stellar::sse::ms`.

#### P06.T6 Hertzsprung gap and first giant branch

- **P06.T6.a Giant-branch core mass–luminosity relation** and its inverse, the parameters p, q, B,
  D, the crossover `m_x`, `t_inf1`, `t_inf2` and `t_x` (HPT section 5.2). Core mass as a function of
  time along the branch in both regimes. Tests: L(Mc) and Mc(L) are inverses to 10⁻¹⁰; Mc(t) is
  continuous at `t_x`.
- **P06.T6.b Hertzsprung gap.** Core mass at the end of the main sequence and its growth, L and R
  interpolated in τ between the terminal main sequence and the base of the giant branch (or helium
  ignition for masses above `m_fgb`). Tests: continuous with T5 at `t_ms` and with T6.a at `t_bgb`.
- **P06.T6.c Giant radius** R_GB(M, L) and the luminosity at helium ignition `l_he_i`, time of
  ignition `t_he_i`. Tests: the tip of the giant branch for 1 M☉ at Z = 0.02 is near 2,500 L☉ with a
  core near 0.47 M☉ (re-check against HPT's figures); the tip luminosity falls with metallicity.
- **Files:** `stellar/sse/hg.rs`, `gb.rs`. **Accept:** tests pass.

#### P06.T7 Core helium burning

- **Build:** zero-age horizontal branch luminosity and radius as functions of core and envelope
  mass, the minimum luminosity and radius for intermediate masses, the blue-loop fraction and its
  timing (`τ_x`, `τ_y`), the helium-burning lifetime `t_he`, core growth, and L and R through the
  phase (HPT section 5.3). The low-mass branch depends on envelope mass, which is what lets mass
  loss on the giant branch (η of design note 7, and the helium hook) set the colour of the
  horizontal branch.
- **Files:** `stellar/sse/cheb.rs`.
- **Tests:** continuity with T6 at ignition for masses above `m_hef` (below it the flash bridge of
  T10.d applies); a 5 M☉ star at Z = 0.02 makes a blue loop that crosses 5,500–6,500 K; a 0.8 M☉, Z
  = 0.0005 star with 0.1–0.2 M☉ of envelope sits at 6,000–7,500 K (the RR Lyrae region), and bluer
  with less envelope; `t_he`(1 M☉) is 100–140 Myr.
- **Accept:** tests pass.

#### P06.T8 Asymptotic giant branch

- **P06.T8.a Early AGB:** core mass at the base of the AGB `m_c_bagb`, the helium and carbon–oxygen
  core growth, L from the core mass–luminosity relation, R_AGB. Second dredge-up at the end of the
  phase.
- **P06.T8.b Thermally pulsing AGB:** core growth with third dredge-up (λ), L, R, and the three ends
  of the phase: envelope loss, the core reaching `m_c_sn` (supernova from the AGB, 1.6–2.25 M☉ cores
  at the base giving ONe white dwarfs or electron capture) or Chandrasekhar mass (HPT section 5.4).
  Expose `m_c_bagb` and the interpulse period as a function of core mass (T28.f needs both).
- **Files:** `stellar/sse/agb.rs`.
- **Tests:** continuity with T7 at the end of helium burning; with constant mass, a 1 M☉ core grows
  monotonically; `m_c_bagb` crosses 1.6 M☉ near 6.5 M☉ and 2.25 near 8 M☉ at Z = 0.02 (re-check).
- **Accept:** tests pass.

#### P06.T9 Naked helium stars

- **Build:** helium main sequence (ZAMS L and R, lifetime, evolution), helium Hertzsprung gap and
  giant branch with their own core mass–luminosity relation and radius, and the rule for which
  evolved helium stars become helium giants (HPT section 6.1). Entry points: from a track whose
  envelope the wind removes during helium burning or later (the Wolf-Rayet route), and a constructor
  from a helium-star mass for plan 11's stripped stars.
- **Files:** `stellar/sse/helium.rs`.
- **Tests:** L and R continuous across the phases; helium main-sequence lifetime of a 4 M☉ helium
  star about 1 Myr (re-check); the entry from core helium burning is continuous in L to 1% when the
  envelope reaches zero.
- **Accept:** tests pass.

#### P06.T10 Mass loss and the track integrator

- **P06.T10.a Hurley's wind recipe** (HPT section 7.1): Kudritzki–Reimers with η, Vassiliadis–Wood
  on the AGB with its superwind cap, Nieuwenhuijzen–de Jager scaled by Z^½, the Wolf-Rayet-like term
  for small envelopes (μ), the LBV term beyond L > 6 × 10⁵ L☉ and 10⁻⁵ R L^½ > 1, and helium-star
  winds. One function `wind::rate(recipe, &StarState, &Composition, η)` returning
  `SolarMassesPerYear`. Tests: each term against a hand-computed value; the rate is the maximum of
  the applicable terms, as the paper states.
- **P06.T10.b Modern winds** (design note 6): Vink et al. (2001) equations 24 and 25 with the
  bi-stability jump handled by linear interpolation in temperature between 22,500 and 27,500 K so
  that the rate is continuous; the LBV rate; Hamann–Koesterke with the Vink–de Koter scaling. Tests:
  a 40 M☉, 40,000 K, 10⁵·⁷ L☉ star at solar Z loses 10⁻⁶–10⁻⁵ M☉ per year; the Z scaling; continuity
  across every temperature boundary.
- **P06.T10.c The fixed-grid integrator** (design notes 1 and 2). `Track` as `Vec<Segment>`; a
  segment holds its phase, age range and up to `KNOTS` (16 by default, 32 on the thermally pulsing
  AGB, spaced geometrically towards its end where the superwind acts) knots of (age, mass, effective
  initial mass, effective age offset, core mass). Integration within a segment is a fixed count of
  midpoint steps per knot interval (`STEPS_PER_KNOT` = 4), in the coordinate design note 1 fixes for
  the phase. The effective-age bookkeeping when mass changes on the main sequence and the rule that
  the giant-branch formulae keep the initial mass follow HPT section 7.1. A segment whose total loss
  at the entry rate would be under 10⁻⁶ of its mass takes no knots. A segment that ends by envelope
  loss ends at the root of the envelope mass on the knot interpolant, by 40 bisections. On the
  thermally pulsing AGB each knot also carries the cumulative interpulse phase, the integral of 1 ÷
  τ_ip(M_c) from the segment's start (T8.b), which T24.b and T28.f read. `Track::to_age` and
  `Track::full`. Tests: prefix equality, bit for bit, between `to_age(a)` for ten ages and `full`;
  mass never increases after the protostar phase; with `WindRecipe::Hurley2000` and η = 0.5, a 1 M☉
  star at Z = 0.02 ends as a CO white dwarf of 0.50–0.56 M☉; convergence: doubling both `KNOTS` and
  `STEPS_PER_KNOT` moves the final mass and the lifetime by under 0.5% for the 16 masses of T12 at
  two metallicities (if not, the counts are raised here, before any golden pins them); a test that
  builds one track, evaluates it at 1,000 ages in two different orders and against a fresh track per
  age, and finds all three bit-identical, which is "the grid does not depend on the query".
- **P06.T10.d `state_at`, bridges and envelope loss.** Segment lookup by binary search;
  `max_radius_until` and `max_luminosity_until` from per-segment running maxima stored at build time
  plus the closed form inside the current segment; interpolation of the knot quantities (monotone
  cubic in the segment's fraction); the helium-flash bridge and the hand-over to the post-AGB bridge
  of T16 (until T16 lands, a direct hand-over to the white dwarf marked as a declared discontinuity
  in the continuity test, removed by T16); loss of the whole envelope on the giant branch (helium
  white dwarf or helium star) and during helium burning. Tests: continuity sweep over 200 random (m,
  Z, η) with 2,000 ages each, concentrated near junctions: |Δ log L| and |Δ log R| per step below a
  Lipschitz bound times the step, outside the declared deaths.
- **P06.T10.e Lifetime and death.** `Track::lifetime`, `Track::death` with `ProgenitorAtDeath` (core
  masses and envelope at the last living instant) and `SupernovaType` from the envelope: hydrogen
  envelope above 2 M☉ IIP, 0.1–2 IIL, under 0.1 IIb, none Ib, and Ic when the helium-star wind has
  also removed most helium (thresholds are generator defaults; the task records its source).
  `evolve` and `lifetime` convenience functions, and `turn_off_mass(age, comp)` by bisection on
  `t_zams + t_ms`. `lifetime` is what plan 08's placement calls for layers D and E, up to twice per
  accepted record, so it integrates only what the death time needs (mass and core mass under the
  wind, no radius, luminosity output or remnant stage) and stores no track; it must return exactly
  `Track::lifetime` of the full build, which a test pins over 10⁴ random inputs. Tests: lifetime is
  continuous and monotone decreasing in mass apart from the documented jump across `m_hef`; the
  property test "no living phase at an age beyond the lifetime, no remnant before it" over 10⁵
  random inputs.
- **Files:** `stellar/sse/wind.rs`, `track.rs`, `evolve.rs`. **Accept:** tests pass; bench targets
  of "Verification" reported.

#### P06.T11 Remnant structure from the backbone

- **Build:** the white dwarf kinds by core mass at envelope loss (He, CO, ONe), white dwarf radius
  (HPT section 6.2.1), neutron star radius (10 km in HPT; use 11.5 km with a citation to a current
  measurement, and keep 10 under `RemnantRecipe::Hurley2000`), black hole radius 2GM ÷ c², and the
  original remnant mass formula from the core mass at supernova, kept only under
  `RemnantRecipe::Hurley2000` for validation.
- **Files:** `stellar/remnant/mod.rs`, `structure.rs`.
- **Tests:** a 0.6 M☉ white dwarf has a radius of 0.012–0.013 R☉; radius falls with mass and
  vanishes at the Chandrasekhar mass; under the original recipe a 20 M☉ star at Z = 0.02 leaves a
  neutron star and 40 M☉ a black hole, matching SSE.
- **Accept:** tests pass.

#### P06.T12 Validation against the published SSE output

- **P06.T12.a Reference vectors.** A developer runs the public SSE code once, offline, for m ∈ {0.1,
  0.3, 0.5, 0.8, 1, 1.5, 2, 3, 5, 8, 10, 15, 20, 40, 60, 100} M☉ and Z ∈ {0.0001, 0.001, 0.004,
  0.02, 0.03} with its default wind (η = 0.5) and remnant options, and commits a compact CSV per
  metallicity: for each phase change its age, mass, core mass, log L, log R; plus 20 samples along
  each track. A header names the SSE version, its options and the date. Only output is committed,
  never SSE source. Files: `crates/hyperion-sim/tests/data/sse/*.csv`, `tests/data/sse/README.md`
  (provenance only).
- **P06.T12.b Comparison tests.** With `WindRecipe::Hurley2000`, `RemnantRecipe::Hurley2000`, η =
  0.5, no pre-main-sequence offset and no bridges: phase-change ages within 1%, masses and core
  masses within 1% (2% on the thermally pulsing AGB, where SSE's adaptive step and the fixed grid
  differ most), log L and log R within 0.02 dex at the samples, the same remnant kind, remnant mass
  within 0.02 M☉. Any systematic excess is fixed in the formulae, not by loosening. Files:
  `crates/hyperion-sim/tests/sse_reference.rs`.
- **P06.T12.c Paper checks.** Independent of the code: the tabulated or plotted values of HPT that
  can be read exactly (main-sequence lifetimes, the initial–final mass relation figure at Z = 0.02
  to 0.05 M☉) and Cummings et al. (2018) per design note 9, under the modern recipe.
- **Accept:** `cargo test -p hyperion-sim --test sse_reference` passes; marked slow if over 10 s.

### Phase C: around the backbone

#### P06.T13 Below 0.1 M☉: cooling fits (shared with plan 13)

- **Build:** `substellar::cooling(mass, age, comp)`: the analytic fits of Burrows et al. (2001,
  their equations for L, T_eff and R as power laws in age, mass and opacity), valid 0.01–0.08 M☉,
  with deuterium burning ignored below 13 Jupiter masses (plan 13 may refine). For 0.075–0.1 M☉ a
  hydrogen-burning floor: L = smoothmax(L_cool(m, t), L_hb(m, Z)), where `L_hb` is a fit to the main
  sequence of Baraffe et al. (2015) pinned so that at 0.1 M☉ it equals the HPT ZAMS value, which
  makes the state continuous in mass across 0.1 M☉; the same for radius. The hydrogen-burning limit
  is where `L_hb` reaches zero weight, 0.072–0.078 M☉ by metallicity. `evolve` dispatches here below
  0.1 M☉; such stars never leave this phase within any age the fields draw.
- **Files:** `stellar/substellar.rs`.
- **Tests:** against Baraffe et al. (2015) at 0.05, 0.075, 0.08 and 0.09 M☉ and 0.1, 1 and 10 Gyr:
  T_eff within 150 K, log L within 0.15 dex; a 0.05 M☉ object passes through M, L and T temperatures
  as it ages (2,800 K at 0.1 Gyr class, under 1,300 K by 5 Gyr); continuity in mass across 0.1 M☉ to
  2% in L at 1 and 10 Gyr; continuity in age.
- **Accept:** tests pass; plan 13 can call the function with no change.

#### P06.T14 Extension to 100–150 M☉

- **Build:** above 100 M☉ the formulae are evaluated as they stand, times correction factors for
  ZAMS luminosity, ZAMS radius and main-sequence lifetime, each a quadratic in log₁₀(m ÷ 100) that
  is exactly 1 at 100 M☉. The task picks one published grid of very massive star models that reaches
  150 M☉ at two metallicities or more (candidates: Yusof et al. 2013; Köhler et al. 2015), fits the
  three quadratics to it, and records grid, fit and residuals in the doc comment. The Eddington
  factor is checked to stay below 1 over the whole range.
- **Files:** `stellar/sse/vms.rs`.
- **Tests:** continuity at 100 M☉ to 10⁻¹²; L, R and lifetime at 120 and 150 M☉ within 10% of the
  chosen grid; lifetime stays above 2 Myr; count check in T31 (about a thousand alive at once).
- **Accept:** tests pass.

#### P06.T15 Protostars and the pre-main sequence

- **P06.T15.a Protostar phase.** For age < t_p = 0.5 Myr: m(t) = m_f × (1 − (1 − t ÷ t_p)²), which
  reaches half the final mass at 0.146 Myr, the observed length of Class 0 (Dunham et al. 2014;
  re-check). `ProtostarClass::{Class0, ClassI}` by whether the envelope (m_f − m) outweighs the
  star. L = photospheric (from T15.b at the current mass, on the birthline) + accretion, G m ṁ ÷ R.
  R on the birthline from a closed-form fit (Palla and Stahler 1999 as a starting point; record the
  choice).
- **P06.T15.b Contraction.** `t_zams(m, Z)`: the arrival time, a fit in log m to Baraffe et al.
  (2015) below 1.4 M☉ and to a Kelvin–Helmholtz time from the ZAMS values above (about 40 Myr at 1
  M☉, several hundred Myr at 0.2, under t_p above about 8 M☉, in which case the star is on the main
  sequence when accretion ends). Between t_p and `t_zams`: Hayashi contraction at nearly fixed
  temperature with R ∝ t^(−⅓), then for m > 0.5 M☉ a Henyey segment of rising temperature, blended
  so that L, R and their first derivatives meet the ZAMS values at `t_zams`. `Track` gains the two
  leading segments and offsets the HPT clock (design note 4).
- **P06.T15.c Disc lifetime draw.** One draw on `star.disc_lifetime`: exponential with a mean of 2.5
  Myr scaled by m^(−½), held to 0.3–15 Myr (record the source). It decides classical against
  weak-lined T Tauri (T24), bounds FU Orionis activity (T28.d), and is there for plan 14.
- **Files:** `stellar/premain.rs`, `stellar/sse/track.rs`.
- **Tests:** continuity at t_p and at `t_zams` (values to 10⁻⁶, slopes to 5%); a 1 M☉ star at 2 Myr
  has 1–3 L☉ and 3,900–4,500 K; in a sample of the young disc (ages −H to 100 Myr) most stars below
  0.5 M☉ are pre-main-sequence, as the brainstorm states; stars of negative age are rejected by the
  type (`SystemExistence::NotYetBorn` is decided in T29).
- **Accept:** tests pass; the continuity sweep of T10.d now starts at age zero.

#### P06.T16 The post-AGB bridge and planetary nebulae

- **P06.T16.a Bridge** (design note 8): a `PostAgb` segment after envelope loss from either AGB
  phase: constant L, T_eff rising from the AGB value to the knee temperature of the white dwarf of
  that mass over the crossing time `t_cross(m_core)`, then the hand-over to T20's cooling law with
  matching L. Removes the declared discontinuity left by T10.d.
- **P06.T16.b Nebula.** `nebula::planetary_nebula(&Track, age) -> Option<PlanetaryNebula>`: present
  when the death was `EnvelopeLoss` from the AGB, the central star is above 25,000 K, and the time
  since ejection is under the visibility time, R ÷ v with R_max = 0.8 pc and v drawn once in 20–40
  km/s (about 20,000–40,000 years, the brainstorm's "some 20,000"). Gives radius, expansion speed,
  age, ionised mass (a fixed fraction of the envelope lost in the last superwind knots) and an
  excitation class from the central star's temperature.
- **Files:** `stellar/sse/track.rs`, `stellar/nebula.rs`.
- **Tests:** continuity through the bridge; nebula radius never above 2.7 ly; stars with cores below
  about 0.53 M☉ show no nebula; the expected number alive in a Milky Way galaxy (death rate of 0.8–8
  M☉ stars × mean visible time, from T31's sampler) is 5,000–50,000, against the 20,000 or so
  estimated for the Milky Way.
- **Accept:** tests pass.

#### P06.T17 The helium-excess hook

- **Build:** `Composition::helium_excess` enters `Track` through `tables::helium`, in the shape
  under Provides, which plan 15's P15.T7 restates: the main-sequence and giant-branch timescales are
  multiplied by exp(s(m, Z) × ΔY), with s from `LIFETIME_SLOPE` (a quadratic in log mass at four
  metallicities, interpolated in log Z), and log T_eff on the horizontal branch is shifted by
  `HB_TEMPERATURE_SHIFT` (linear in envelope mass) × ΔY, with the radius adjusted to keep L. This
  task commits `tables/helium.rs` with every coefficient zero, README's header, and marked
  provisional. The code path applies the correction only when ΔY > 0, so grid stars are
  bit-identical whatever plan 15 later fits. Document the expected signs for plan 15: ΔY = 0.1
  shortens the lifetime of a 0.8 M☉ star by roughly a third and moves the horizontal branch
  bluewards.
- **Files:** `stellar/composition.rs`, `stellar/sse/track.rs`,
  `crates/hyperion-sim/src/tables/helium.rs`, `tables/mod.rs`.
- **Tests:** ΔY = 0 gives bit-identical tracks with and without the hook compiled in (golden); a
  test-only non-identity table changes lifetime and horizontal-branch temperature in the stated
  directions.
- **Accept:** tests pass; plan 15 can replace the table file alone.

### Phase D: remnants and kicks

#### P06.T18 Remnant type and mass (Mandel and Müller 2020)

- **P06.T18.a Core collapse outcomes** from the carbon–oxygen core mass at death, with the paper's
  break points M₁ = 2, M₂ = 3, M₃ = 7, M₄ = 8 M☉ (re-check every figure against MNRAS 499, 3214,
  section 2 and table 1). Always a neutron star below M₁. P(black hole) rises linearly from 0 at M₁
  to 1 at M₃, decided by one fixed uniform (`star.remnant.type`) against that threshold, so the line
  between neutron stars and black holes is a probability over about 10–25 M☉ of initial mass. Among
  black holes, P(complete fallback) rises linearly from 0 at M₁ to 1 at M₄
  (`star.remnant.fallback`): the mass is then the helium core's (the whole star's if it kept no
  envelope to lose), the death is `DirectCollapse` and there is no kick. Otherwise the black hole's
  mass is normal about 0.8 M_CO with σ = 0.5 M☉, held above the maximum neutron star mass. Neutron
  star masses: normal about 1.2 M☉ (σ 0.02) below M₁; about 1.4 + 0.5 (M_CO − M₁) ÷ (M₂ − M₁) (σ
  0.05) between M₁ and M₂; about 1.4 + 0.4 (M_CO − M₂) ÷ (M₃ − M₂) (σ 0.05) above; held to 1.13–2.0
  M☉. One standard normal (`star.remnant.mass`) serves whichever branch applies.
- **P06.T18.b Electron capture** (design note 12): `m_cc(Z)` by root-finding on `m_c_bagb` once per
  `ZCoeffs`; inside the window the death is `ElectronCapture`, the remnant a 1.26 M☉ neutron star,
  the channel `CollapseChannel::ElectronCapture`. Below the window the star ends as an ONe or CO
  white dwarf through the AGB.
- **P06.T18.c Pair instability** (design note 10).
- **P06.T18.d `CompactRemnant`** type joining the three, and `RemnantRecipe::MandelMuller2020` as
  the generator default in `Track::death`.
- **Files:** `stellar/remnant/collapse.rs`.
- **Tests:** over a Kroupa sample of 8–150 M☉ at Z = 0.02: black holes are 38 ± 5% of compact
  remnants (the research figure behind "about four fifths" of layer-E remnants staying); black holes
  of 2–5 M☉ exist; complete fallback is 70–80% of black holes; neutron star masses lie in 1.13–2.0;
  the type is monotone in the uniform draw (a lower draw never turns a black hole into a neutron
  star); the electron-capture share of single-star neutron stars is 2–6%.
- **Accept:** `cargo test -p hyperion-sim stellar::remnant::collapse` passes.

#### P06.T19 The kick law

- **P06.T19.a Normal quantile, interface and ordinary mode.** Two parts, in order.
  - The quantile, which plan 01's `math` lacks: `math::normal_quantile(p: f64) -> f64` for 0 < p <
    1, under plan 01's rules for the module (its P01.T2): hand-written, no new dependency, every
    transcendental through the existing wrappers of the pinned `libm`. Acklam's rational
    approximation (central and tail branches, split at p = 0.02425; `math::ln` and `f64::sqrt`
    only), then one Halley step on Φ(x) − p with Φ from `math::erfc` and the density from
    `math::exp`, which brings the relative error below 10⁻¹³. It debug-asserts 0 < p < 1 and
    documents the domain. Add its golden values to plan 01's `tests/golden/math/functions.golden`
    through `tests/foundation_golden.rs` and `just bless`, with arguments that cross both branch
    points: 0.5, 0.02425 ± 2⁻⁵⁵, 0.97575, 0.001, 0.999, 10⁻¹⁰, 1 − 2⁻⁵³; no existing line changes.
    Files: `crates/hyperion-sim/src/math.rs`, `tests/foundation_golden.rs`, the golden. Tests: the
    golden; `hyperion_testkit::stats::normal_cdf(normal_quantile(p))` returns p to 10⁻¹² at 1,000
    points; antisymmetry about ½ to 10⁻¹²; strictly increasing across both branch points. Accept:
    `cargo test -p hyperion-sim math` and the foundation golden pass.
  - The law: `KickLaw`, `KickLawParams`, `StandardKickLaw`. Score x = (M_CO − M_rem) ÷ M_rem × ξ
    with ξ normal about 1 with σ = 0.45, redrawn deterministically (next draw numbers on
    `star.kick.score`) until positive. Rank r = F_x(x) from `KickRankTable`, clamped to 0.001–0.999,
    speed = exp(5.60 + 0.68 × Φ⁻¹(r)) km/s, which spans 33–2,200 km/s (Disberg and Mandel 2025, ApJL
    989, L8, for μ and σ; Disberg, Mandel and Hirai 2026 for the 45%). Φ⁻¹ is
    `math::normal_quantile`. Direction isotropic from `star.kick.direction`. `KickDraws` with
    `of(&StarDraws)` and `from_parts`, so that plan 08's quadrature can drive the law from explicit
    variates. Until T19.b lands the table, the unit tests of this subtask use a two-knot test table.
- **P06.T19.b Reference population and provisional rank table.**
  `remnant::reference::ReferencePopulation`: Kroupa primaries of 8–150 M☉, Z = 0.02, iron-core
  collapses of single and wind-stripped progenitors that leave a neutron star, sample i drawn on the
  tag `stellar.reference` (scope `Galaxy`, object `ObjectKey::galaxy_item(i)`) from the given seed
  and turned into a star through `StarDraws::from_parts`, so it needs no ID and no galaxy;
  `score_quantiles(pop, n, seed)` sorts n scores into the 257 quantiles at ranks i ÷ 256 of
  `SCORE_QUANTILES` (see Provides), and `KickRankTable` interpolates them linearly. It is written as
  a per-chunk scorer and a quantile step, so that plan 15 can run it in chunks. A provisional table
  made by this function with n = 10⁶ is committed as `tables/kick_rank.rs` with README's header
  (tool, inputs, version), marked provisional, with the four defaults as its constants, which
  `KickLawParams::default` reads. Plan 15's P15.T5.a calls the same function with 10⁷ draws and owns
  the file afterwards (T19.e). Tests: the quantiles are strictly increasing; ranks of 10⁵ fresh
  reference scores are uniform (K–S); two runs give the same table bit for bit.
- **P06.T19.c Low mode, black holes, white dwarfs.** Low mode: Maxwellian with σ = 5 km/s (three
  normals on `star.kick.low`). It always applies to `ElectronCapture` and `AccretionInduced`; to
  `Stripping::Companion` with probability clamp(3 − M_CO ÷ M☉, 0, 1) on `star.kick.mode`. Black
  holes: the ordinary map × 0.75, with M_rem the black hole's mass; none after complete fallback.
  White dwarfs: Maxwellian with σ = 1 km/s. The provisional stripped mark of design note 11
  (`star.stripped`).
- **P06.T19.d Kick-law tests** (the brainstorm's Testing list), all slow tests with fixed seeds on a
  Kroupa sample of 8–150 M☉ primaries at Z = 0.02 with the provisional stripped mark. The six
  figures are computed by the public, pure `reference::kick_observables`, which plan 15's P15.T5.b
  also calls; the tests assert its output against the bands below. Each of the four defaults of
  "Open questions" (the 2–3 M☉ ramp, 0.75, the window widths; the fate of merged binaries is plan
  11's) is a field of `KickLawParams`, and each test names the default it pins.
  1. Ordinary-mode neutron star speeds: moments of ln v within 5.60 ± 0.12 and 0.68 ± 0.10, and K–S
     against the log-normal, truncated at the clamp, at plan 01's `stats::ALPHA`.
  2. Isolated pulsars (every ordinary-mode neutron star, and the low-mode ones from single stars,
     which are the electron captures of the 0.1 M☉ window; low-mode neutron stars from
     companion-stripped stars stay with their companions and are left out): 5 ± 2% have a transverse
     speed under 50 km/s, the mean over isotropic viewing directions of the speed projected on the
     sky (Willcox et al. 2021).
  3. Low-mode share of all neutron stars within 20 ± 10% (Igoshev et al. 2021).
  4. Retention: the share of neutron stars with speed under 50 km/s, judging low-mode ones on a pair
     recoil of a third of their kick, is at least 10%. Report also the values at 20 and 100 km/s
     against the brainstorm's 8–12% and 18–26%.
  5. Double neutron stars, by a test-only toy: circular pre-supernova pairs of a 1.4 M☉ neutron star
     and a stripped helium star, separations log-uniform over 1–10 R☉, the kick of this law and
     instantaneous mass loss (Hills 1983; Brandt and Podsiadlowski 1995). More than half of the
     surviving pairs have e < 0.3. Plan 11 repeats the test on real binaries.
  6. Black holes: at least half unkicked; among those under 12 M☉, 50–70% unkicked and 12–26% above
     100 km/s (Nagarajan and El-Badry 2025).
- **P06.T19.e Swap in plan 15's rank table** when it lands: replace the file, bump the generator
  version, regenerate goldens, rerun T19.d.
- **Files:** `stellar/remnant/kick.rs`, `reference.rs`, `tables/kick_rank.rs`, `tables/mod.rs`.
- **Accept:** `just test-slow` passes the six tests; a golden pins the kicks of three pinned IDs.

#### P06.T20 White dwarfs: cooling and spectral types

- **P06.T20.a Cooling.** Luminosity from cooling age by the two-piece modified Mestel law of Hurley
  and Shara (2003, ApJ 589, 179), which depends on mass and core composition (He, CO, ONe); T_eff
  from L and the radius of T11. The cooling age counts from the end of the post-AGB bridge, whose
  end luminosity the law is matched to. Check against one published cooling sequence for 0.6 M☉ CO
  (the task chooses and cites; Bédard et al. 2020 is a candidate): T_eff within 10% from 0.01 to 10
  Gyr.
- **P06.T20.b Spectral type** by fixed draws against thresholds that move with temperature, the
  pattern the brainstorm's determinism section prescribes. `u_atm` (`star.wd.atmosphere`) against
  the helium-atmosphere fraction f_He(T_eff): low near the DB gap (30,000–45,000 K), about 10% at
  20,000 K, rising to about 30% below 10,000 K as convection mixes thin hydrogen layers, so a given
  star can turn from DA to a helium type as it cools and never back. Hydrogen atmosphere: DA above
  5,000 K, DC below. Helium atmosphere: DO above 45,000 K, DB 12,000–45,000 K, then DQ with
  probability rising to about 0.3 (`star.wd.carbon`) and DC otherwise. `u_metal` (`star.wd.metals`)
  against a pollution fraction of 0.25–0.5 by cooling age appends Z (DAZ, DBZ) or, for a cool helium
  atmosphere, gives DZ. The numeric suffix is 50,400 K ÷ T_eff. The task records the sources of the
  fractions (the Montreal and Gaia 100 pc samples) since the brainstorm gives the scheme only.
- **Files:** `stellar/remnant/white_dwarf.rs`.
- **Tests:** L and T_eff continuous and falling with age; in a thin-disc sample the types come out
  DA 65–85%, DC and DQ and DZ together 10–30%, DB 2–10%; a fixed star's type sequence over 10 Gyr
  never goes from a helium type back to DA.
- **Accept:** tests pass.

#### P06.T21 Neutron stars

- **P06.T21.a Birth draws.** Spin period normal about 300 ms with σ 150 ms, redrawn until above 10
  ms (`star.ns.spin`); log₁₀ of the dipole field in gauss normal about 13.25 with σ 0.6
  (`star.ns.field`); magnetic inclination and spin axis isotropic (`star.ns.geometry`). Sources to
  re-check: Faucher-Giguère and Kaspi (2006) for the period, Popov et al. (2010) for the field with
  decay.
- **P06.T21.b Spin-down in closed form.** Field decay B(t) = B₀ ÷ (1 + t ÷ τ_d) with τ_d = 10⁴ yr ×
  (10¹⁵ G ÷ B₀), to a floor of 10¹² G or B₀ if lower. Magnetic dipole braking P Ṗ = k B², k from R =
  11.5 km and I = 10⁴⁵ g cm², gives P(t)² = P₀² + 2k ∫B² dt, and the integral of this decay law is
  elementary. Mean glitch activity is included as a factor (1 − 0.01) on ν̇ for pulsars with
  characteristic ages of 10³–10⁵ years (Fuentes et al. 2017; re-check), so glitches leave no mark
  that needs replaying. `PulsarState` at any age: period, period derivative, field, characteristic
  age, spin-down luminosity and whether the pulsar is alive.
- **P06.T21.c Death line, magnetars, beaming.** Alive as a radio pulsar while B ÷ P² > 0.17 × 10¹² G
  s⁻² (Bhattacharya et al. 1992). Magnetar while B > 4.4 × 10¹³ G and the decay luminosity exceeds
  the spin-down luminosity. Beam half-angle ρ = 5.4° × (P ÷ s)^(−½) (or the Tauris and Manchester
  1998 beaming fraction, whichever the task finds reproduces 10–20% at 1 s);
  `PulsarBeam::sweeps(direction: UnitVector) -> bool` is true when the angle between the direction
  and the spin axis is within ρ of the inclination, for either pole.
- **P06.T21.d Pulse phase.** `pulse_phase(t)` for |t| ≤ H: phase at the epoch from one draw, plus ν
  t + ½ ν̇ t² + ⅙ ν̈ t³ with ν, ν̇, ν̈ evaluated at the epoch, the product ν t computed in split integer
  and fractional parts so the error stays below 10⁻³ cycles over the clock window. Beyond H the
  function still returns, with the error documented.
- **P06.T21.e Pulsar wind nebula flag:** present while the spin-down luminosity is above 10³⁶ erg/s
  (10⁴–10⁵ years, as the brainstorm's shell section expects). Plan 09 draws it.
- **Files:** `stellar/remnant/neutron_star.rs`.
- **Tests:** P is continuous and non-decreasing in age; with constant field the closed form equals
  √(P₀² + 2kB²t); a 10¹²·⁵ G pulsar born at 300 ms dies after 10⁷–10⁸ years; the share of neutron
  stars born above 4.4 × 10¹³ G is 15–40% (the draw of T21.a gives 26%; Beniamini et al. 2019
  estimate about 0.4 with wide errors; re-check), none of them counts as a magnetar by T21.c's
  second condition before spin-down has slowed it, and the expected number of active magnetars at
  two core collapses a century is 20–300; the beaming fraction at 1 s is 10–20% over random
  directions; the pulsar wind nebula flag lasts 10³–10⁵·⁵ years across the birth distribution.
- **Accept:** tests pass.

#### P06.T22 Black holes

- **Build:** `BlackHole { mass, spin }`. Mass from T18. Dimensionless spin from one draw
  (`star.bh.spin`): small for single-star collapse, a half-normal with σ = 0.1 held below 0.998
  (efficient core–envelope coupling; Fuller and Ma 2019 as the suggested source, recorded by the
  task; see Risks). Luminosity zero: "dark unless something feeds them", and feeding is plan 11's.
  Schwarzschild radius and the innermost stable orbit for the readout.
- **Files:** `stellar/remnant/black_hole.rs`.
- **Tests:** spin in [0, 0.998); ISCO is 6GM ÷ c² at zero spin; state constant in time.
- **Accept:** tests pass.

### Phase E: classification and derived properties

#### P06.T23 MK classification

- **P06.T23.a Dwarf scale and photometry.** The dwarf table of Pecaut and Mamajek (2013, ApJS 208,
  9, table 5 and its maintained extension to L, T and Y): spectral subtype, T_eff, BC_V, B−V, as a
  `const` table in `stellar/classify/pm13.rs` with a checksum test. `subtype_from_teff` by
  interpolation in log T_eff, giving a continuous subtype (`G2.3`) that formats to the nearest
  half-class for O–M. `photometry::{bolometric_correction_v, absolute_magnitude_v, colour_b_v}` with
  M_bol☉ = 4.74.
- **P06.T23.b Luminosity class** from surface gravity and luminosity at the star's temperature:
  boundary curves log g(T_eff) between V, IV, III, II, Ib, Iab and Ia, and Ia⁺ above 10⁵·⁷ L☉ within
  0.2 dex of the Humphreys–Davidson limit. Giants and supergiants are cooler than dwarfs of the same
  type, so classes III and I apply an offset to the dwarf scale; the brainstorm names only Pecaut
  and Mamajek, so the task picks the giant and supergiant calibrations, records them, and keeps the
  dwarf scale exact for class V (see Risks). The phase breaks ties: a core-helium-burning star at
  clump gravity is III, never IV.
- **P06.T23.c Subdwarfs, L–T–Y, white dwarfs and others.** `sd` prefix for a main-sequence star with
  [Fe/H] < −1.0, `esd` below −1.7; L, T, Y from the same table for `Substellar` and the coolest
  dwarfs; white dwarfs by T20.b; neutron stars and black holes have no spectral type and format as
  `NS`, `PSR`, `MAG` and `BH`.
- **P06.T23.d `classify(&StarState, &Composition, &StarDraws, extras) -> Classification`** and its
  `Display`. Composition peculiarities from T24 and T25 append (`G8III Ba` is plan 11's; here `Ap`,
  `Am`, `Be`, `e` for emission in classical T Tauri and Herbig stars).
- **Files:** `stellar/classify/{mod.rs, pm13.rs, luminosity.rs}`, `stellar/photometry.rs`.
- **Tests:** the Sun is G2V with M_V = 4.81 ± 0.05; Vega-like (9,600 K, log g 4.0) A0V; a 4,300 K,
  log g 1.7 star K-type III; 3,600 K, 10⁵ L☉ is M-type Iab or Ia; subtype is monotone in T_eff;
  every state from a 10⁵-star sample classifies without panic and round-trips through `Display` and
  a test parser.
- **Accept:** tests pass.

#### P06.T24 Classes beyond the MK grid

`PeculiarClass` and the rules, all derived from state and track, none rolled. T24.a covers the
massive and stripped stars, T24.b the rest.

- **P06.T24.a Wolf-Rayet stars, hot subdwarfs and luminous blue variables.**
  - Wolf-Rayet: a star above a luminosity floor of 10⁴·⁹ L☉ × (Z ÷ 0.02)^(−0.4) (record the source)
    that is hot and nearly or fully stripped. WNh while a hydrogen envelope under 10% of the mass
    remains; WN as a naked helium star until the mass lost as a helium star exceeds the layer above
    the helium core's largest convective extent (a closed-form fraction of the helium star's initial
    mass, fitted to published helium-star models and recorded); WC after that; WO in the helium
    Hertzsprung gap or later with T_eff above 10⁵ K. A numeric subtype from temperature (WN2–9,
    WC4–9).
  - Naked helium stars below the floor: hot subdwarfs (`sdO`, `sdB`), reachable for single stars
    only through giant-branch envelope loss, and for plan 11's stripped stars.
  - Luminous blue variable: inside the HPT criterion L > 6 × 10⁵ L☉ and 10⁻⁵ R L^½ > 1 and hotter
    than 8,000 K. `Track::window_where(PhasePredicate::Lbv)` gives the age interval, which plan 09's
    catalogue class tests against the source horizon.
  - Tests: a 60 M☉ star at Z = 0.02 passes through LBV, WN and WC in that order and at Z = 0.0001
    never becomes WC; a giant-branch star stripped of its envelope classifies as `sdB`;
    `window_where(PhasePredicate::Lbv)` brackets exactly the ages at which the state passes the
    criterion, at 1,000 sampled ages.
- **P06.T24.b Carbon and S stars, and young stars** (needs T28.f).
  - Carbon and S stars: on the thermally pulsing AGB, C/O = (C/O)₀ + δ(m, Z) × max(0, n − n_min),
    with n the thermal pulse number from T28.f's clock (a closed form in the event number), δ from
    the dredge-up efficiency λ of T8.b, zero outside the mass range where dredge-up works and hot
    bottom burning does not (about 1.5–4 M☉ at solar Z, lower at low Z; record the source). M below
    C/O = 0.95, S (through MS and SC) for 0.95–1.0, C above, formatted `C-N`.
  - Young stars: `TTauri { classical: bool }` for pre-main-sequence stars under 2 M☉ (classical
    while the disc of T15.c lasts), `HerbigAeBe` for 2–8 M☉.
  - Tests: carbon stars appear only inside the mass range and only after n_min pulses; the C/O of a
    fixed star is non-decreasing in time; a 1 M☉ star at 2 Myr is a T Tauri star, classical or not
    by its disc draw, and a 4 M☉ one at 0.3 Myr is `HerbigAeBe`; count checks in T31.
- **Files:** `stellar/classify/peculiar.rs`.
- **Accept:** `cargo test -p hyperion-sim stellar::classify::peculiar` passes after each subtask.

#### P06.T25 Rotation and magnetism

- **Build:** one draw each, as the brainstorm's row says, plus the spin axis (`star.spin_axis`).
  - `u_rot` (`star.rotation`) is a rank. Above 1.3 M☉ it maps onto the equatorial velocity
    distribution for the mass (bimodal for late B and A stars; Zorec and Royer 2012; record), as a
    fraction of critical velocity from the state's mass and radius, conserved through the main
    sequence. Below 1.3 M☉ it maps onto the initial period spread, which converges by Skumanich
    braking, P_rot ∝ t^½ with a colour-dependent coefficient (Mamajek and Hillenbrand 2008; record),
    with saturation for fully convective stars. Both are closed forms in age.
  - `u_mag` (`star.magnetism`): a fossil field in 7–10% of main-sequence stars above 1.5 M☉, with a
    log-normal strength; these are forced to the slow rotation mode.
  - Derived: `Be` for B-type main-sequence stars above 0.7 of critical; `Ap`/`Bp` for fossil-field
    stars of 7,000–20,000 K; `Am` for non-magnetic A stars (7,000–10,000 K) under 120 km/s (as a
    single-star stand-in for tidal braking; plan 11 adds the binaries); `ActivityLevel` from the
    Rossby number with the convective turnover time of Wright et al. (2011): saturated L_X ÷ L_bol =
    10⁻³ below Ro = 0.13, falling as Ro^(−2.7) above. The flare rate of T28.a reads the activity.
- **Files:** `stellar/rotation.rs`.
- **Tests:** the Sun's age and mass give a 22–30 day period and a low activity level for the median
  draw; Be stars are 10–25% of B-type main-sequence stars; Ap and Bp 5–10% of A and B stars;
  rotation period continuous and rising with age for cool dwarfs.
- **Accept:** tests pass.

#### P06.T26 Variability

- **P06.T26.a The instability strip.** Blue and red edges as lines in (log T_eff, log L) (record the
  source). A star inside pulsates, named by phase and mass: δ Scuti (main sequence and Hertzsprung
  gap, 1.5–2.5 M☉), RR Lyrae (low-mass core helium burning), classical Cepheid (core helium burning
  blue loop or gap crossing, 3–20 M☉), type II Cepheid (post-horizontal-branch and AGB low-mass
  stars: BL Her, W Vir, RV Tau by period). Period from mean density, P = Q × (ρ̄ ÷ ρ̄☉)^(−½), with Q
  by kind (0.033–0.04 d), amplitude largest mid-strip and zero at the edges, so variability switches
  on and off continuously as a star crosses.
- **P06.T26.b Long-period variables.** AGB and tip giants: Mira above a luminosity and amplitude
  threshold, semiregular (SRa, SRb) below, irregular for supergiants (SRc, Lc). Period from Ostlie
  and Cox (1986): log P = −2.07 + 1.94 log R − 0.9 log M (days, solar units).
- **P06.T26.c Other strips,** since every named kind must be able to fall out: β Cephei and slowly
  pulsating B stars, γ Doradus, ZZ Ceti (DA, 10,500–12,500 K), V777 Her (DB, 22,000–29,000 K), GW
  Vir, α Cygni supergiants, the S Doradus cycles of LBVs (years to decades, on the monotone phase),
  and rotational modulation (BY Dra, α² CVn) from T25's period and activity. Each is a region test
  and a period rule.
- **P06.T26.d Light factor with cycle-keyed irregularity.** `light_factor_at(star, t) -> f64`:
  pulsation phase from a `PhaseClock` whose frequency is the epoch's plus its first derivative from
  the track (so evolution changes the period and the phase stays continuous), through
  `MonotonePhase::cycle_at`; the cycle's amplitude and shape marks come from the stream keyed by
  cycle number under the star's `star.var.cycle` event key. Regular pulsators use zero noise
  amplitude; Miras a few per cent of period jitter and 10–30% of amplitude scatter; semiregulars
  more.
- **Files:** `stellar/variability.rs`.
- **Tests:** a 5 M☉ blue-loop star gets a Cepheid period of 3–10 days, and the sample's Cepheids
  follow a period–luminosity slope within 15% of the observed one; RR Lyrae periods of 0.3–0.9 days;
  δ Scuti 0.02–0.3 days; Mira periods of 150–600 days; `light_factor_at` is continuous in t and the
  same whatever order times are asked in; a star just outside the strip has amplitude zero.
- **Accept:** tests pass.

### Phase F: events in time

#### P06.T27 Generic event machinery (`hyperion_sim::events`)

- **P06.T27.a Tags, windows and IDs.** Declare this plan's event domain tags (scope `Event`) in
  `rng/tags.rs` and register each in plan 01's `event_tags!` (`id/event_tags.rs`) as
  `number => CONST = tags::CONST`, with the numbers listed under "Generator version"; write the
  number blocks of "Conventions fixed here" there as a comment for plans 09, 11 and 14; re-export
  the tags as `events::tags`. An event's marks come from `EventKey::event_stream`, so no event kind
  needs a second tag; `TimeWindow` (half-open, on `UniverseTime`); helpers that build an `EventId`
  from a subject, tag, `EventBin` and index.
- **P06.T27.b Poisson bins** (design note 15). Bin k = floor(t ÷ Δ) in integer seconds. The key is
  `EventKey::derive(seed, tag, subject)`. From `key.bin_stream(k)`, in a fixed order: the count, by
  plan 01's Poisson sampler with mean `bound(k) × Δ`, then for each j a time fraction and a thinning
  uniform; event j is accepted if u < `rate(t) ÷ bound(k)`. Each accepted event carries
  `key.event_stream(k, j)` for its marks, so marks never shift the times. `events_in` visits the
  bins that touch the window and sorts its output by time; `active_at` looks back `look_back_bins`;
  `event(id)` regenerates one event from its ID alone and returns `None` for an ID that names a
  rejected or absent event, as `resolve` does for systems. A debug assertion that `rate ≤ bound`.
- **P06.T27.c Monotone phase** (design note 16). `LinearClock` with split arithmetic accurate to
  10⁻⁶ cycles over the source horizon; the trait for clocks whose period drifts; value noise by
  hashing integer lattice points under the event key and interpolating with the cubic 3s² − 2s³ in
  exact arithmetic; the constructor's slope check; root-finding by bisection; `SkipMark` (a
  probability) as an integer threshold on the cycle's hash; `cycle_at` for continuous phases.
- **P06.T27.d Tests and benches.** For both constructions: the union of `events_in` over any
  partition of a window equals `events_in` over the whole, in any order of calls and across threads
  (`events::testing::assert_partition_independent`, with the order of calls permuted through
  `hyperion_testkit::order::assert_order_independent`); `event(id)` agrees with the listing; bin
  counts pass a Poisson interval check and waiting times K–S against the exponential for a constant
  rate; a thinned sinusoidal rate reproduces its profile (chi-square); phase events are strictly
  ordered, none lost or doubled over 10⁶ cycles, including negative times; the variance of (t_n −
  nP) grows with n up to the top octave (the phase diffuses), which a plain jittered lattice
  included as a control fails; goldens for pinned keys. Benches: events in a window of ten bins, and
  one phase root.
- **Files:** `crates/hyperion-sim/src/events/{mod.rs, tags.rs, bins.rs, phase.rs, testing.rs}`,
  `benches/events.rs`.
- **Accept:** `cargo test -p hyperion-sim events` and the slow statistical tests pass; bench numbers
  recorded in the task's commit message.

#### P06.T28 Single-star event kinds (`stellar::events`)

Each kind is a `RateModel` or a `PhaseClock` built from a `StarModel`, a mark sampler, and a
transient effect for `summary_at`. Each records its sources; the brainstorm gives FU Orionis rates
and the references Contreras Peña et al. (2019), Melatos et al. (2008) and Fuentes et al. (2017).

- **P06.T28.a Flares** (Poisson bins, Δ = 1 day). Stars with convective envelopes (below about 1.3
  M☉, and pre-main-sequence stars). Cumulative rate above energy E a power law of index about −1,
  normalised by T25's activity level, with an energy floor per star that keeps the bound at or under
  64 a day. Marks: energy (power law), duration and peak factor from the energy.
- **P06.T28.b Glitches.** Crab-like: Poisson bins (Δ = 30 days), rate proportional to |ν̇| for
  pulsars younger than 10⁷ years, sizes from a power law, a recovery that decays within the
  look-back. Vela-like (characteristic age 10⁴–10⁵ years and one fixed mark): monotone phase with P
  of about three years scaled by |ν̇|, narrow size distribution near Δν ÷ ν = 10⁻⁶. The frequency
  offset is a sawtooth in the phase's fractional part with zero mean, so spin-down stays the closed
  form of T21.b.
- **P06.T28.c Magnetar bursts and giant flares.** Poisson bins at two levels: active episodes (Δ =
  30 days, rate falling with the decaying field), and within an episode short bursts (Δ = 1 hour,
  look-back to the episode's bin); giant flares as a separate rare kind (about one per 50–100 years
  per young magnetar; re-check), energy 10⁴⁴–10⁴⁷ erg.
- **P06.T28.d FU Orionis outbursts.** Poisson bins (Δ = 500 years, look-back 1 bin): one per 5,000
  years while a protostar (age < 0.5 Myr), one per 112,000 years afterwards while the disc of T15.c
  lasts, the rate stepping smoothly over 0.05 Myr so the bound stays tight. Marks: luminosity factor
  10–100, rise of 1–10 years, decay of 20–200 years.
- **P06.T28.e LBV giant eruptions.** Poisson bins (Δ = 100 years, look-back 1 bin) while inside
  T24.a's LBV window. The rate is a generator default, provisionally one per 30,000 years per star
  in the window, which with some 500 such stars gives a Milky Way galaxy about 30 in ±H (the
  research figure). T31 measures the real count and retunes the constant so that the galaxy-wide
  figure is 15–60, regenerating this task's golden. Marks: ejected mass of 0.1–10 M☉ (already in the
  mean wind; design note 14), peak of 10⁶·⁵–10⁷·⁵ L☉, duration of 1–20 years.
- **P06.T28.f Thermal pulses.** Monotone phase on a clock whose base phase is the integral of 1 ÷
  τ_ip(M_c) along the thermally pulsing AGB segment, which T10.c stores at the segment's knots,
  interpolated in age, plus the clock time since the epoch times the epoch's pulse frequency (design
  note 23). Pulse number n(t) feeds T24's C/O. Transient: the luminosity dip and peak of a pulse
  cycle as a function of the cycle's fractional phase.
- **P06.T28.g Assembly.** `stellar::events::{events_in, active_at}` over all kinds; `StarEvent`;
  transient factors composed in `summary_at` (design note 14).
- **Files:** `stellar/events/`: `mod.rs`, `flares.rs`, `glitches.rs`, `magnetar.rs`,
  `fu_orionis.rs`, `lbv.rs`, `thermal_pulse.rs`.
- **Tests:** per kind, the mean count over 10⁴ stars matches the rate integral (Poisson interval);
  FU Orionis onsets galaxy-wide come to 240–830 a year at Milky Way values (from T31's sampler);
  order independence through the public functions; `bound` is never exceeded (a sweep across each
  kind's parameter range); no function in `stellar` takes a list of past events as input (a
  compile-level property: `summary_at` has no such parameter, stated in its doc and checked by a
  test that evaluates a star at t = +900 years cold and after evaluating 100 earlier times, with
  equal results).
- **Accept:** tests pass under `just test` and `just test-slow`.

### Phase G: system assembly and galaxy integration

#### P06.T29 `SystemStars`, summaries and hooks for later plans

- **Build:** `StarModel` and `SystemStars::generate` (metallicity, draws, a track built to the age
  at the epoch + H and completed to death if the star is already dead or dies within the window;
  design note 19). `generate` is three separate steps, because plan 08's P08.T12.c repeats only the
  last: the primary's draws (`StarDraws::for_star` here; plan 08 makes it
  `for_attempt(.., record.mark_attempt())`), the track from mass, composition and those draws, and
  the remnant stage, a private `remnant_stage(&Track, &StarDraws)` that reads only the
  `star.remnant.*`, `star.stripped` and `star.kick.*` fields. Plan 08's kick loop calls that last
  step again with the same fields of later attempts, on the one built track, until the record's
  `kick_constraint()` is met, so an attempt costs a remnant and a kick and never a track; no track
  draw is ever redrawn. With no constraint, which is every record of this plan, the step runs once.
  `summary_at(t)`: age = `record.age_at(t)`; plan 03's `record.existence_at(t)` of `NoSystemYet`
  gives `SystemExistence::NotYetBorn` and no stars. `brief_at(t)`: state and classification only.
  `death_time()`: T = lifetime − age at the epoch as a `UniverseTime`, `BeyondClockRange` when the
  seconds do not fit an `i64`. `natal_kick()`, `lbv_window()`. `ObjectKind` from phase and class.
  Extend `GENERATOR_VERSION`'s changelog comment.
- **Files:** `stellar/system.rs`.
- **Tests:** golden summaries (`tests/golden/stellar/summaries.golden`) for a dozen pinned IDs
  across all five layers at t = 0 and ±500 years; order independence; determinism across two runs; a
  star whose T falls at +100 years is living at +99 and a remnant at +101, under the same ID; the
  property test of the brainstorm, over 10⁵ random IDs and times: no star in a living phase is older
  than its lifetime, every remnant is older, no state has a non-finite or non-positive L, R or T_eff
  (black holes and `NoRemnant` excepted).
- **Accept:** tests and goldens pass.

#### P06.T30 Real lifetimes and remnant masses in the mean mass per system

- **P06.T30.a The seam.** Plan 02's `galaxy::fates::StellarFates` takes a mass and no metallicity,
  and `mean_present_mass(f, fates, ages)` is called once per population with that population's age
  distribution. Leave the trait as it is and give each population its own fates value. Add
  `fates::reference_fe_h(Population) -> Dex`, the constants of plan 02's P02.T7.e with no dependence
  on position or on the system count, so that nothing becomes circular: old and young thin disc 0.0,
  thick disc −0.55, bulge 0.0, long bar 0.0, nuclear disc +0.1, halo −1.2 (its dominant component).
  Where plan 02 passes one `ProvisionalFates` to every population, pass `fates_for(population)`
  instead, which still returns `ProvisionalFates`: no output change, no version bump, plan 02's
  goldens unchanged.
- **P06.T30.b `stellar::fates::TrackFates`.** `TrackFates::at(fe_h)`: lifetime and present mass
  (living mass from the track, or remnant mass with the draws integrated out by a fixed 8-point
  quadrature over the type and fallback uniforms, other draws at `StarDraws::median()`) as functions
  of m at that metallicity, tabulated once on a fixed grid of 96 masses, log-spaced over 0.08–150 M☉
  with the band edges as nodes, and interpolated; the quadrature reads the table. The table depends
  on the generator version alone, so it is built once per `Galaxy` for the four distinct reference
  metallicities (about 400 full tracks, some tens of milliseconds) and held by the `Galaxy`, as the
  potential tables are. `mean_companions` delegates to `ProvisionalFates`. `fates_for` switches to
  it. Bump `GENERATOR_VERSION`; regenerate every golden file in the same commit with `just bless`.
- **Tests:** at Milky Way parameters the mean present-day mass per system under Kroupa is 0.48 ±
  0.02 M☉ (0.55–0.60 under Chabrier), varies by under 5% between the old populations, and the young
  disc's is 30–50% higher; the share of dead primaries is 8 ± 2% in the old thin disc, 11 ± 2% in
  the thick disc and 13 ± 3% in the halo (the research figures behind the brainstorm's 0.48); the
  system count for a 5 × 10¹⁰ M☉ galaxy is near 10¹¹.
- **Files:** `galaxy/fates.rs`, `galaxy/params.rs` (the call sites of plan 02's P02.T4 and P02.T5),
  `stellar/fates.rs`, all goldens.
- **Accept:** `just ci` green with regenerated goldens; the version bump is in the same commit.

#### P06.T31 Statistical tests: class fractions by population

- **Build:** `stellar::testing::sample_population(galaxy, population, n, seed)`: draws (m, age,
  [Fe/H]) from plan 02's mass function, age distribution and metallicity field at a fixed position,
  without placement, and evaluates summaries at t = 0. Slow tests at Milky Way parameters, each with
  its source in a comment and tolerances wide enough for a fixed-seed sample of 10⁶:
  - Thin disc near 26,000 ly, all stars: main-sequence types M 70–80%, K 10–14%, G 5–8%, F 2–4%, A
    0.4–1%, B 0.05–0.2%, O under 10⁻⁵; white dwarfs 5–9% of objects (the 10 pc sample of Reylé et
    al. 2021); giants 0.3–1%.
  - Old populations (thick disc, bulge, halo): nothing living above the turn-off (0.8–1.0 M☉ by
    metallicity and age); red giants and horizontal-branch stars present at the 1% level; halo RR
    Lyrae per unit mass within a factor of three of the observed specific frequency.
  - Layer E: the living share in the old thin disc is under 1% and in the young disc most of those
    under 30 Myr; neutron stars to black holes about 62:38.
  - Galaxy-wide expected counts from population budgets × sampled fractions: protostars 0.6–4 × 10⁶;
    stars above 100 M☉ 300–3,000; Wolf-Rayet stars 500–8,000; LBVs 100–2,000 (from which T28.e's
    provisional rate is retuned to 15–60 giant eruptions in ±H); classical Cepheids 5,000–50,000;
    planetary nebulae (T16); living radio pulsars 10⁵–10⁶, of which beamed at a given place 10–20%;
    core collapses per century 1–8 across seeds and about 2 at Milky Way values.
- **Files:** `stellar/testing.rs`, `crates/hyperion-sim/tests/stellar_statistics.rs`.
- **Accept:** `just test-slow` passes; a band that fails is a finding to resolve in the model or to
  widen with a recorded reason, never silently.

#### P06.T32 Benchmarks

- **Build:** Criterion benches: `ZCoeffs::new`; `stellar::lifetime` at 1, 5, 12 and 40 M☉ with
  `ZCoeffs` already built and with it built inside the call; `SystemStars::generate` for a layer-A
  main-sequence star, a red giant, a white dwarf and a neutron star; `brief_at` and `summary_at` on
  a built model; briefs for the 1,600 systems of a 50 ly query at the reference density.
- **Files:** `crates/hyperion-sim/benches/stellar.rs`.
- **Accept:** `just bench` runs them; targets under "Verification"; results recorded.

### Phase H: protocol, server and display

#### P06.T33 Protocol

- **Build:** in `hyperion-protocol`: `ObjectKindDto` (snake-case strings), `StellarBriefDto`
  (`kind`, `class: String`, `log_luminosity_lsun: f32`, `teff_k: f32`), the optional `stellar` field
  on the range row and `include_stellar` on the request (both `#[serde(default)]`, and `stellar`
  skipped when `None`, so plan 04's wire forms stay valid byte for byte); the kind `system_summary`
  exactly as plan 04's "Extending the convention" prescribes: `RequestBody::SystemSummary` and
  `ResponseBody::SystemSummary` with the same `kind` string, `"system_summary"` added to
  `REQUEST_KINDS`, no new `ClientMessage` or `ServerMessage` variant and no request ID inside the
  body, since the envelope carries it; `ErrorCode::UnknownSystem`. The response carries `universe`,
  `system`, `time`, existence, [Fe/H], and per star: kind, phase, class, mass, core mass, L, R,
  T_eff, M_V, B−V, mass-loss rate, rotation period, activity, variability (kind, period, amplitude),
  remnant detail (white dwarf type and cooling age; pulsar period, Ṗ, field, alive, magnetar; black
  hole spin; natal kick speed and mode), planetary nebula, active events, and death within the clock
  window as a time. Units are in field names and are ones the UX guide allows or gains in T35.a.
  Times use plan 04's wire `UniverseTime`, IDs its `SystemIdHex` and `UniverseIdHex`.
  `PROTOCOL_VERSION` does not change: plan 04's design note 15 says a new kind or optional field
  does not bump it, because an older server answers `unsupported`. The TypeScript request client
  needs no change, but the client's exhaustive switches over `ErrorCode` (plan 05's `RequestStatus`)
  gain the `unknown_system` case in this task, so that `just ci` stays green after
  `just gen-protocol`.
- **Files:** `crates/hyperion-protocol/src/lib.rs` (or the module plan 04 split it into), then
  `just gen-protocol` and the regenerated `packages/protocol/src/generated`.
- **Tests:** a wire-form pin for the `system_summary` request inside its `request` envelope, for its
  response inside `response`, for a `request_error` with `unknown_system`, and for each new DTO; the
  test that pins `REQUEST_KINDS` is updated; an old-form range request without `include_stellar`
  still parses, and a range response without briefs serialises exactly as plan 04 pinned it.
- **Accept:** `just gen-protocol-check` and `just ci` green.

#### P06.T34 Server

- **Build:** the range handler fills `stellar` when asked, on the CPU pool inside the range job,
  from a byte-bounded `ByteLru` of `SystemStars` keyed by `(seed, generator_version, SystemId)`, as
  plan 04's design note 23 keys every cache (epoch state, as the brainstorm requires of caches;
  `SystemStars` implements `HeapBytes`; budget from a new `HYPERION_SYSTEM_CACHE_MB`, default 128).
  The `system_summary` handler is an interactive pool job: it validates the time against ±H as plan
  04's limits do for the range query (`bad_request` with `field: "time"`), resolves the ID through
  plan 03's `resolve` (any `ResolveSystemError` gives `request_error` with `unknown_system` and
  `field: "system"`), generates or fetches the model and evaluates at the requested time. A
  `SystemIdHex` that fails to parse is already plan 04's `bad_request`.
- **Files:** `crates/hyperion-server/src/` (handlers and cache wiring as plan 04 laid them out).
- **Tests:** integration tests over a real socket with plan 04's `TestServer` and `TestClient`:
  summary of a pinned system equals the sim's; a malformed ID gives `bad_request` and a well-formed
  ID that names no system gives `unknown_system`, each as a `request_error` for the request's ID; a
  cancelled summary ends with exactly one terminal message; a range request with `include_stellar`
  returns a brief on every row; the same request twice is identical (cache hit or not).
- **Accept:** `cargo test -p hyperion-server` passes.

#### P06.T35 Client: symbols, filter, legend, UX guide

- **P06.T35.a UX guide.** Edit `docs/frontend/ux-guidelines.md`: the star chart's symbol set (design
  note 17) under the spatial-display rules; beside the SI units the guide already has (K, s), the
  astronomers' units L☉ and R☉ (drawn with plan 05's `☉` component), `dex` for [Fe/H], `mag` for
  magnitudes, `d` for rotation and pulsation periods and `G` for magnetic fields, each needing the
  owner's confirmation as plan 05's additions did; the HR diagram as a permitted scatter plot with a
  reversed temperature axis, under the guide's rules for graphs.
- **P06.T35.b Symbols and filter.** `lib/galaxy/starSymbols.ts`: `ObjectKind → SymbolShape`;
  `spatial/marks.ts` and `spatial/symbols.ts` gain the `ringed-circle` value and its outline beside
  the circle, diamond, square and triangle plan 05 defined (a pure path function, unit-tested
  without a canvas; at the smallest `SIZE_CLASS_REM` it still reads open against filled, and every
  exhaustive switch over `SymbolShape` gains the case). `lib/galaxy/wire.ts` and `model.ts` carry
  the brief into `ChartSystem`. The chart requests `include_stellar`. A `STARS` selector (`ALL`,
  `LIVING`, `REMNANTS`) with a single-key binding filters marks and list alike, and the count line
  says what is hidden (`412 OF 1,630 SHOWN — LIVING`), per the guide's honest-data rule. Legend
  entries for the five shapes. The list gains a class column.
- **Files:** `docs/frontend/ux-guidelines.md`; under `apps/hyperion/src/renderer/src/`:
  `spatial/{marks.ts, symbols.ts}`, `lib/galaxy/{starSymbols.ts, wire.ts, model.ts}`, in
  `displays/galaxy/`: `chartModel.ts`, `ChartControls.tsx`, `SystemList.tsx`, `SymbolLegend.tsx` and
  `useRangeQuery.ts`; `test/galaxyFixtures.ts`, and their tests.
- **Tests:** vitest: mapping is exhaustive over `ObjectKind` (a `switch` with no default); filter
  logic; legend renders every shape with an accessible name; the list row shows class text.
- **Accept:** `pnpm typecheck`, `pnpm lint`, `pnpm test` green.

#### P06.T36 Client: system readout

- **Build:** selecting a system requests `system_summary` at the chart's time through plan 05's
  `useServerRequest<"system_summary">`, whose `RequestChannel` makes the latest selection win; the
  readout (an `output` element, as plan 05 built it) gains: class and kind in words, phase, mass now
  beside initial mass, L, R, T_eff, [Fe/H], M_V, rotation, variability, remnant detail, nebula,
  active events, and `DIES IN 312 yr` when death falls inside the clock window. Missing values are
  em dashes; pending, rejected, timed-out and link-down states are plan 05's `RequestStatus`, never
  a zero. A system not yet born reads `NOT YET FORMED`. Every new label joins the guide's
  nomenclature list.
- **Files:** `displays/galaxy/SystemReadout.tsx`, a thin `displays/galaxy/useSystemSummary.ts` over
  `useServerRequest` with an explicit return type, `test/galaxyFixtures.ts` (`aSystemSummary`).
- **Tests:** vitest with `FakeWebSocket.serverAnswers("system_summary", …)`: renders a living star,
  each remnant kind, an unborn system, a rejection with `unknown_system`; a reply to a superseded
  request is dropped.
- **Accept:** TypeScript checks and tests green; checked by eye against a running server.

#### P06.T37 Client: HR diagram panel

- **Build:** `HrDiagram`: a Canvas 2D scatter of the current range result's briefs, log T_eff on a
  reversed x axis (from 200,000 to 1,000 K, so that hot white dwarfs and brown dwarfs fit; points
  beyond an axis are pegged at the edge with the guide's off-scale mark and counted in the caption)
  against log L ÷ L☉ from −6 to 6.5, a title above, thin `--line` grid, a label and unit on each
  axis and values at the major ticks, as the guide requires of a graph, marks in `--text`, the
  selected system with the chart's reticle, reachable systems in `--accent` as on the chart,
  spectral class letters along the top axis. Projection and picking as pure functions. Compact
  remnants without a photosphere (neutron stars, black holes) are counted in a caption, not plotted.
  Mark size follows the chart's size classes, and the panel says `SYMBOLS NOT TO SCALE` as the chart
  does. Selecting a point selects the system everywhere. The canvas has an accessible name and is
  not focusable: keyboard access is through the shared list. It redraws on demand, never on a loop.
- **Files:** `apps/hyperion/src/renderer/src/displays/galaxy/HrDiagram.tsx`,
  `lib/galaxy/hrProjection.ts`, tests (with plan 05's `RecordingContext2D`).
- **Tests:** projection maps the Sun to the expected pixel; picking tolerance of at least `1rem`;
  renders with zero points; the caption counts.
- **Accept:** TypeScript checks and tests green; the by-eye checks of "Verification".

## Verification

- **Backbone:** T12's comparison with published SSE output passes at the stated tolerances for all
  16 masses × 5 metallicities, under the original recipes.
- **Properties:** no main-sequence (or any living) star older than its lifetime; mass non-increasing
  after the protostar phase; all state finite and positive; classification total.
- **Continuity in time:** the sweep of T10.d, extended by T13, T15 and T16 to all ages from zero,
  and through T29 on `summary_at` with events off: no jump except at a death by collapse or
  explosion. T10.c's test that a track's answers do not depend on what was asked before, and its
  convergence test, are what show the fixed grid honours "continuous in age with no age-binned
  tables".
- **Order independence:** `StarDraws`, `SystemStars::generate`, both event constructions and every
  event kind return the same result in any order of asking and alone; determinism test across two
  runs.
- **Kick law:** the six tests of T19.d.
- **Class fractions by population** and expected galaxy-wide counts: T31. The mean mass per system:
  T30.
- **Benchmarks** (this plan's targets, set inside the brainstorm's "a full system with its bodies in
  under a millisecond"; a miss is a finding): `ZCoeffs::new` under 5 µs; `stellar::lifetime` about 5
  µs for a primary of layer D or E (plan 08's 5 ms query budget counts on it: its design note 28 and
  P08.T16); `generate` for a main-sequence dwarf under 10 µs, for a giant under 60 µs, for a remnant
  (full track) under 150 µs, so that a triple of evolved stars under plan 11 still leaves plan 14
  half the millisecond for the bodies; `brief_at` on a built model under 2 µs; briefs for a cold 50
  ly query of 1,600 systems adding under 15 ms to plan 03's 5 ms; ten Poisson bins under 2 µs; one
  phase root under 3 µs.
- **By eye,** in the `GALAXY` display against a Milky Way seed:
  1. HR diagram of a 50 ly query in the thin disc: a main sequence dense at the bottom, a few
     giants, a white dwarf sequence at lower left, nothing in forbidden regions, no seams at 0.1,
     `m_hook`, `m_hef` or 100 M☉.
  2. The same in the halo or thick disc: a turn-off near 0.8 M☉, a giant branch and a horizontal
     branch. In the young disc: a pre-main-sequence band above the lower main sequence.
  3. Chart with a mass floor of 8 M☉ and a radius of 2,000 ly across an arm: with `ALL`, a nearly
     uniform field of triangles and squares; with `LIVING`, the few circles and ringed circles lie
     along the arm. This is the caveat of "Sizing the layers" resolved.
  4. Stepping the chart's time by ±1,000 years: a selected evolved star's readout changes smoothly;
     a star with `DIES IN` becomes a remnant past that time and back again before it.

## Generator version

This plan changes generated output once for certain: T30.b changes the mean mass per system, hence
the system count and every candidate draw, and bumps `GENERATOR_VERSION` with all goldens. T19.e
bumps it again when plan 15's rank table replaces the provisional one (only kicks change). Every
other task adds output on new streams and moves nothing that existed. Any later change to a formula,
coefficient, recipe default, knot count or bridge time of this plan changes stellar output and is a
version bump.

Reserved so that later plans move no star:

- **Body index 0** of every system is its primary star; plan 11 numbers companions from 1 and plan
  14 numbers planets after the stars (see "Conventions fixed here" under Provides).
- **Domain tags** (never renamed; in `rng/tags.rs` under "Plan 06"; scope `Body` unless stated):
  `system.metallicity` (scope `System`); `stellar.reference` (scope `Galaxy`); `star.eta`;
  `star.rotation`; `star.magnetism`; `star.spin_axis`; `star.disc_lifetime`; `star.stripped`
  (provisional meaning, permanent stream: plan 11 conditions on it); `star.remnant.type`;
  `star.remnant.fallback`; `star.remnant.mass`; `star.kick.score`; `star.kick.mode`;
  `star.kick.low`; `star.kick.direction`; `star.wd.atmosphere`; `star.wd.carbon`; `star.wd.metals`;
  `star.ns.spin`; `star.ns.field`; `star.ns.geometry`; `star.ns.phase`; `star.bh.spin`;
  `star.nebula`.
- **Event tags** (plan 01's 16-bit registry, where `0x0001` is its self-test tag). Each is a domain
  tag of scope `Event` and an `event_tags!` entry; an event's marks come from
  `EventKey::event_stream`, so no kind has a second tag. `0x0100` `star.ev.flare`, `0x0101`
  `star.ev.glitch`, `0x0102` `star.ev.glitch_cycle`, `0x0103` `star.ev.magnetar_episode`, `0x0104`
  `star.ev.magnetar_burst`, `0x0105` `star.ev.magnetar_giant`, `0x0106` `star.ev.fu_orionis`,
  `0x0107` `star.ev.lbv_eruption`, `0x0108` `star.ev.thermal_pulse`, `0x0109` `star.var.cycle` (the
  cycle-keyed irregularity of T26.d, S Doradus cycles included). The rest of `0x0100`–`0x01FF` is
  this plan's; `0x0200`–`0x02FF` is reserved for plan 09 (features and the centre), `0x0300`–
  `0x03FF` for plan 11 (binaries), `0x0400`–`0x04FF` for plan 14 (bodies). Zero is invalid.
- **Arguments:** `Composition::helium_excess` (zero for every grid star; plans 09 and 10 set it for
  second-population members); `Stripping` and `CollapseChannel::AccretionInduced` in the kick law
  (plan 11 supplies real values); `WindRecipe` and `RemnantRecipe` as version-level switches.
- **Tables:** `tables/kick_rank.rs` and `tables/helium.rs`, whose ownership passes to plan 15 in the
  shapes committed here.
- **Protocol:** the kind `system_summary`, `ErrorCode::UnknownSystem`, and the optional fields
  `include_stellar` and `stellar`, none of which changes `PROTOCOL_VERSION`.

## Risks and open points

- **Transcription.** HPT has some 200 coefficients and known misprints. Mitigations: checksummed
  tables, continuity sweeps across every piecewise boundary, and T12's comparison with SSE output.
  The SSE source is consulted only to settle a misprint, and its licence is unclear, so no code is
  copied and only its numerical output is committed.
- **Track cost.** A remnant needs its full track (design note 19). If the 150 µs target is missed by
  a wide margin, the fallback is a once-per-galaxy table of end states over (m, Z), of the kind
  T30.b builds for the quadrature, used for stars dead longer than the source horizon. That is a
  table in mass and metallicity, not in age, so it stays within the brainstorm's rule, but it would
  smooth the draws' effect on old remnants and is not the default.
- **Ambiguity: the electron-capture windows.** The brainstorm gives widths (0.1 and about 1 M☉) and
  no position or variable. Read here as intervals of initial mass ending at the lowest mass for
  iron-core collapse (design note 12). Test 2 of T19.d pins the single-star width.
- **Ambiguity: single stars and the low mode.** The kick law and its tests belong to this plan, but
  the low mode's main progenitors need companions, which are plan 11's. Resolved by the provisional
  stripped mark with a default share of 0.25 (design note 11) and a test-only toy binary for the
  double neutron star test. Plan 11 must replace the share with the quadrature the brainstorm
  describes and keep the stream. Until then companion-stripped stars follow unstripped tracks, so
  their M_CO, and with it their place on the 2–3 M☉ ramp, is that of a single star.
- **Circular dependency with plan 15.** The rank table is tabulated "from the generator's own
  tracks", so plan 15's tool needs T18 and T19.a–d before it can run, while README lists 15 as a
  dependency of this plan and gives plan 15 no dependency on this one. Resolved by the provisional
  table and T19.e: the header keeps README's "03, 15", but the only edge from 15 to here ends at
  T19.e, and plan 15's own schedule already shows the reverse edge (its "Needs code from: 06").
  Milestone M2 can close on the provisional table if plan 15's P15.T5.a slips; the swap is then one
  version bump in M3. README's table would be more exact with 15 depending on 06 per table.
- **The fixed grid and "continuous in age".** Design note 1 reads the brainstorm's ban on "tables
  binned by age" as a ban on shared tables indexed by age, not on the nodes of one star's own mass
  integral. The grid never reads the query time, state is continuous between and across knots, and
  the cost fits the millisecond. If the owner reads the rule more strictly, the alternative is a
  closed-form mass history per phase, which exists only for the simplest wind laws.
- **Silent in the brainstorm, decided here and open to revision,** each with a design note that
  names the source to re-check: pair instability (note 10, Belczynski et al. 2016); black hole spins
  (note 20, Fuller and Ma 2019); the scatter in Reimers η (note 7, McDonald and Zijlstra 2015); the
  white dwarf atmosphere fractions (note 21, Cunningham et al. 2020, Bédard et al. 2020, Koester et
  al. 2014); neutron star birth distributions and field decay (note 13, Popov et al. 2010,
  Faucher-Giguère and Kaspi 2006, which come from different syntheses and may not combine); event
  rates other than FU Orionis (note 22; magnetar bursts, giant flares and giant eruptions of
  luminous blue variables have no source in the brainstorm at all). Decided in tasks, with a
  candidate source each: the post-AGB crossing time (note 8); the giant and supergiant temperature
  scales beside Pecaut and Mamajek's dwarf scale (T23.b); the protostar's mass growth law and the
  disc lifetime (T15); supernova subtypes from the envelope (T10.e); the rotation distributions
  (T25); the instability strip's edges (T26). Each task must record the source it uses. None
  contradicts the brainstorm, which gives the scheme in each case and not the numbers.
- **"Continuous in time" against real jumps.** The helium flash and the end of the AGB are bridged
  over short physical times (note 3). Supernovae and collapses stay discontinuous, as deaths with a
  clock time, which is how the brainstorm treats them.
- **Very massive stars.** The choice of comparison grid for 100–150 M☉ is left to T14 because no
  source is cited; with about a thousand such stars alive the exposure is small.
- **Cost of briefs on large queries.** A bulge query returns tens of thousands of rows, nearly all
  old and many dead. `include_stellar` is optional for that reason, and the server cache absorbs
  repeats. If it is still too slow the chart can request briefs for the coarse layers only.
- **Interface sketch and neighbours.** Checked against plans 01–05 as validated, and the drafts of
  09, 11, 14 and 15. Points to confirm: plan 01 says the meaning of body indices is plan 14's, while
  this plan fixes index 0 as the primary star and plan 11 numbers companions from 1, so plan 14 must
  number planets after the stars (it does, through plan 11's `STAR_BODY_INDEX_END`); plan 02's
  `StellarFates` has no metallicity argument and keeps none (T30.a); `math` gains a normal quantile
  here (T19.a); `SymbolShape` gains `ringed-circle`; `ErrorCode` gains `UnknownSystem`, which plan
  14 expects from here; the event-tag number blocks under Provides are proposed here and must be
  honoured by plans 09, 11 and 14, none of which names numbers yet. Plan 09 asks that draws accept
  an attempt number (`StarDraws::for_attempt`) and plans 11 and 14 ask for the monotone helpers on
  `Track`; both are provided.
