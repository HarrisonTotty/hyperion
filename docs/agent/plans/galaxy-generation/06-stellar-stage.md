# Plan 06: Stars: evolution, remnants and classes

- **Milestone:** M2.
- **Depends on:** 03 (placement, `SystemRecord`, range query), 15 (offline tables: the kick rank
  table and the helium correction). Only the last task of the kick law, P06.T19.e, waits on plan 15,
  and plan 15's rank-table task waits on P06.T18 and P06.T19.a–b; every other task runs on the
  provisional tables this plan commits, so nothing blocks (see Consumes and Risks). It builds on 01
  and 02 through 03, and on 04 and 05, which are complete by M2, for the protocol and display tasks.
  Per task the edges are narrower: only T3 and T29.b read plan 03, and through T29.b the tasks that
  handle systems (T31, T32, T34–T37); T30 needs plan 02's `Galaxy` (P02.T9) and not plan 03; T1,
  T2, T27 and all of phase B need plan 01 alone (see the ordering note under Tasks).
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

Units added to plan 01's `units`: `MetalFraction`, `HeliumExcess`, `Gauss`, `SolarMassesPerYear`.
Plan 01 already has `SolarLuminosities`, `SolarRadii`, `Kelvin` and `Years`, and plan 02 added
`Dex` (with `DexPerKiloparsec`), in which its `FehDistribution` is written.

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
0 < p < 1 (P06.T1.b, the ID plans 08 and 10 cite). Plan 01's `math` has none, and its testkit has
only `stats::normal_cdf`.

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

- `stellar::testing::{sample_population, hr_sample}`, behind a `testing` feature and `cfg(test)`:
  draws stars of one population at given galaxy parameters without placement. `hyperion-sim` has
  no `[features]` yet; P06.T27.d, the first task with a `testing` module, declares it.
- `crates/hyperion-sim/tests/data/sse/`: reference vectors from the published SSE code.
- `events::testing::assert_partition_independent`: the union of an event listing over any partition
  of a window equals the listing over the whole. Order of asking is checked with plan 01's
  `hyperion_testkit::order::assert_order_independent`.
- Golden files follow plan 01's convention: `hyperion_testkit::golden!` and
  `crates/hyperion-sim/tests/golden/stellar/<name>.golden`.

## Consumes

- **Plan 01:** `math` (every `ln`, `exp`, `powf`, `log10`, `sin`, `cos`, `erfc` here goes through
  it, and every fused multiply-add through `math::mul_add`, since `f64::mul_add` is disallowed);
  `rng::{Seed, Stream, DomainTag, TagScope, ObjectKey}` with `Stream::open(seed, tag, object)`,
  which asserts that the tag's scope is the key's, `seek` and `word_at`, and the single
  `domain_tags!` registry in `rng/tags.rs`, to which this plan's tags are added with scope `System`
  (`system.metallicity`), `Body` (every `star.*` tag), `Galaxy` (`stellar.reference`) or `Event`,
  each by the task that first opens a stream under it (the file's rule); the samplers (uniform,
  normal, log-normal, Poisson, power law; normals always take two words, Box–Muller), which have no
  exponential, Maxwellian or isotropic direction, so this plan composes those from uniforms and
  normals; integer-threshold decisions (`Mark`, `Threshold`, `Thresholds`), and the two-step event
  key `rng::EventKey` (`derive(seed, tag: EventTag, subject: EventSubject)`, `bin_stream(bin)`,
  `event_stream(bin, j: u8)`); `units`; `time` (`UniverseTime`, `Span`, `CLOCK_WINDOW_H`,
  `LIGHT_CROSSING_L`, `SourceHorizon`, `ClockWindow`); `coords` (`coords::UnitVector` along the
  galactic axes for kick and spin directions, built with `UnitVector::from_components`); `id`
  (`SystemId`, `BodyId::new(system, body_index: u16)`, `EventId::new(subject, word)`, `EventTag`,
  `EventBin`, `EventWord::new(tag, bin, number: u8)`, `EventSubject`, the event word's layout of a
  16-bit tag, a signed 40-bit number and an 8-bit index, and the `event_tags!` registry, whose
  entries each name a `DomainTag` of scope `Event`); `GENERATOR_VERSION` (5 at re-validation, a
  constant in `version.rs` whose unit test pins its value; there is no changelog); from the
  `hyperion-testkit` crate the `golden!` harness, `order::assert_order_independent` and `stats`
  (chi-square, Kolmogorov–Smirnov, Poisson interval, `normal_cdf`); slow-test marking
  (`#[ignore = "slow: …"]`), `just test-slow`, `just bench`, `just bless`. Plan 01 has no normal
  quantile, and P06.T1.b adds one to `math` under plan 01's rules for that module.
- **Plan 02:** `galaxy::Galaxy` (P02.T9, not yet built: T3, T29.b, T30 and T31 wait for it);
  `Component::metallicity(&PointLy, age: Years)`, which returns a `FehDistribution` (`mean()` and
  `sigma()` in `Dex`, for a population or halo component); the age distributions
  (`AgeDistribution`, ages in `Years`) and `imf::{MassFunction, Kroupa, Chabrier, MassFunctionKind}`
  for test sampling and count tests; `galaxy::quad::bisect` for this plan's root-finding;
  `galaxy::fates::{StellarFates, ProvisionalFates, mean_present_mass,
mean_present_mass_of_mixture}`, the seam through which the mean-mass quadrature reads lifetimes
  and remnant masses. `StellarFates: Debug + Send + Sync` takes masses as bare `f64` in M☉, returns
  `lifetime` in `Years` and `remnant_mass` in M☉, and has a provided `breaks()` that lists the
  masses where a fate has a kink or a jump, which the quadrature uses as panel edges.
  `ProvisionalFates` holds Raiteri, Villata and Navarro's (1996) lifetimes at Z = 0.02 (plan 02's
  R11).
- **Plan 03:** `galaxy::placement::{SystemRecord, Existence, resolve, ResolveSystemError}` (the
  record's `id`, `epoch_position`, `origin` (`SystemOrigin::Grid(ComponentId)` for every record of
  this plan, so `component()` is `Some`), `population`, `primary_initial_mass`, `age_at_epoch`,
  `age_at(t)` and `existence_at(t)`); `galaxy::query::{RangeQuery, RangeResult, SystemHit}`.
- **Plan 04:** the request convention (`RequestId`, `RequestBody`, `ResponseBody`, `REQUEST_KINDS`,
  `RequestError`, `ErrorCode`, the reserved kind `system_summary` and the rules of "Extending the
  convention"); `SystemIdHex`, `UniverseIdHex`, the wire `UniverseTime`; `SystemsInRangeRequest`,
  `SystemsInRange` and its row (the protocol's `SystemRecord`); the universe registry,
  `compute::CpuPool` (`submit(Priority, CancelToken, job)`), `cache::{SharedByteLru, HeapBytes}`
  (plan 04's design note 23 leaves the systems-level cache for this plan to instantiate, keyed with
  `(seed, generator_version)` like every other cache: as built, `compute::GalaxyKey`);
  `RequestClient` and `RequestChannel`; the test helpers `TestServer` and `TestClient` (in
  `crates/hyperion-server/tests/common/mod.rs`) and `FakeWebSocket`. As built through P04.T13, the
  wire types live in `hyperion-protocol`'s `envelope.rs` (`RequestBody`, `ResponseBody`,
  `REQUEST_KINDS`, `RequestError { code, message, field: Option<String> }`, `ErrorCode`),
  `galaxy.rs` (the range types, and `Unit`, which only `ParameterValue` carries; other quantities
  put the unit in the field name) and `primitives.rs`; `PROTOCOL_VERSION` is 2; an absent optional
  goes on the wire as `null`, and no field uses `serde(default)` or `skip_serializing_if` yet; the
  server's `requests::Handlers` answers every kind, `systems_in_range` included, with
  `unsupported` until P04.T14, and `requests::{kind, is_large}` match every `RequestBody` variant.
- **Plan 05:** the general spatial view, `spatial/marks.ts` (`PointMark`, `SymbolShape`, whose
  values `diamond`, `square` and `triangle` plan 05's D14 defines and reserves for later types),
  `spatial/symbols.ts` (`symbolOutline`, `SIZE_CLASS_REM`), `useServerRequest(body, timeoutMs?,
generation?)` with its exhaustive `ErrorCode` switch `settledState` in `lib/useServerRequest.ts`,
  and `RequestStatus` (`components/RequestStatus.tsx`), `SystemList`, `SystemReadout`,
  `SymbolLegend`, `chartModel.ts`, `lib/galaxy/{model.ts, wire.ts}` (`ChartSystem`), `SunGlyph`,
  `UnitLabel` (keyed by the protocol's `Unit`), `lib/format.ts`, and the test helpers
  `galaxyFixtures.ts` and `RecordingContext2D`. Of these, `SystemList`, `SystemReadout`,
  `SymbolLegend`, `chartModel.ts` and `RecordingContext2D` come with P05.T9–T11, not yet built.
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

T1, T2, T27 and phase B need plan 01 alone, so they can start before plan 03 exists; T3 is the only
task of phases A–F that reads a `SystemRecord`, and it also needs plan 02's `Galaxy` (P02.T9).
Within phase A, T1.a comes first; T1.b needs nothing of this plan, and T2 needs T1.a. T27 needs
nothing of this plan. Phase B starts once T1.a lands: T4, T5, T6 and T7 in sequence, then T8 and T9
in parallel; T11, T10.a and T10.b need only T1.a and can run beside any of these; T10.c–e need all
of them and T2; then T12.b and T12.c. T12.a is an offline run of the published SSE code with no
code dependency. Phases C, D and E can run in parallel with each other once T10 is done, except
where a task names another: T16 needs T20.a, the cooling law it hands over to; T18 needs T8's
`m_c_bagb`; T19.a's law needs T18 and T1.b; T24.a needs T9 and T10; T24.b needs T28.f; T26.c–d need
T27.c. T29.a, the `StarModel`, needs T10 and phase D; T26.d and every T28 kind take one, so they
wait for it. Within phase F, T28 needs T27 and T29.a; T28.a needs T25; T28.e needs T24.a; T28.g
needs the rest of T28. T29.b needs plan 03, T3 and B–F. T30 needs T10, T18 and P02.T9, not plan 03.
T31 and T32 need T29.b. Phase H needs T29.b; its protocol task can be written against the types of
T1 as soon as they exist; T34 also needs plan 04's P04.T14 (the range handler) and T35–T37 plan
05's P05.T9–T11 (the chart, list, readout and legend). T19.e is the only task that waits on another
plan (plan 15's P15.T5.a) and is done last; the plan is otherwise complete without it.

Where a task's acceptance says only that its tests pass, the command is
`cargo test -p hyperion-sim -- <module paths of its Files>`, for instance
`cargo test -p hyperion-sim -- stellar::sse::hg stellar::sse::gb` for T6.

Every task that turns a figure into code re-checks it against the named source and cites it in the
doc comment. Every public item gets rustdoc with units and valid ranges, per `rust-dev.md`.

### Phase A: foundations

#### P06.T1 Module skeleton, state types, units and the normal quantile

- **P06.T1.a Module skeleton, state types and units.**
  - **Build:** the `stellar` module tree (`state`, `composition`, `sse`, `substellar`, `premain`,
    `remnant`, `classify`, `photometry`, `variability`, `rotation`, `nebula`, `events`, `system`,
    `fates`, `draws`, `testing`) with `//!` docs. `Phase`, `StarState`, `Composition`, `ObjectKind`
    as under Provides. `StarState::effective_temperature` is derived from L and R by
    Stefan–Boltzmann with T☉ = 5,772 K (IAU 2015 nominal values; cite): `units::consts` has the
    nominal L☉ and R☉ but no T☉, which this task adds beside them. The missing unit newtypes
    (`MetalFraction`, `HeliumExcess`, `Gauss`, `SolarMassesPerYear`) through `units.rs`'s `unit!`
    macro; `Dex` is plan 02's. Add `stellar` to `lib.rs` and to its crate doc's list of modules.
    Replace nothing of the existing `Simulation` stub.
  - **Files:** `crates/hyperion-sim/src/stellar/mod.rs`, `state.rs`, `composition.rs`, and a
    `//!`-only file for each other module of the tree (`sse/mod.rs`, `remnant/mod.rs`,
    `classify/mod.rs` and `events/mod.rs`, the directories later tasks fill; the rest single
    files); `crates/hyperion-sim/src/units.rs`, `lib.rs`.
  - **Tests:** `Composition::from_fe_h(0)` gives Z = 0.02; clamping at both ends; the Sun's L and R
    give 5,772 K to 1 K; `Phase::is_remnant` and `is_living` partition the variants (exhaustive
    match).
  - **Accept:** `cargo test -p hyperion-sim -- stellar::state stellar::composition units` passes;
    `just ci` green.
- **P06.T1.b Normal quantile.** `math::normal_quantile(p: f64) -> f64` for 0 < p < 1, which plan
  01's `math` lacks, under plan 01's rules for the module (its P01.T2): hand-written, no new
  dependency, every transcendental through the existing wrappers of the pinned `libm`. Acklam's
  rational approximation (central and tail branches, split at p = 0.02425; `math::ln` and
  `f64::sqrt` only), then one Halley step on Φ(x) − p with Φ from `math::erfc` and the density from
  `math::exp`, which brings the relative error below 10⁻¹³. It debug-asserts 0 < p < 1 and
  documents the domain. Add its golden values to plan 01's `tests/golden/math/functions.golden`
  through one more entry of the `UNARY` table in `tests/foundation_golden.rs` and `just bless`, with
  arguments that cross both branch points: 0.5, 0.02425 ± 2⁻⁵⁵, 0.97575, 0.001, 0.999, 10⁻¹⁰, 1 −
  2⁻⁵³; no existing line changes. It needs nothing else of this plan, and T19.a's kick law and plans
  08 and 10 call it.
  - **Files:** `crates/hyperion-sim/src/math.rs`, `tests/foundation_golden.rs`, the golden.
  - **Tests:** the golden; `hyperion_testkit::stats::normal_cdf(normal_quantile(p))` returns p to
    10⁻¹² at 1,000 points; antisymmetry about ½ to 10⁻¹²; strictly increasing across both branch
    points.
  - **Accept:** `cargo test -p hyperion-sim math` and the foundation golden pass.

#### P06.T2 Per-star draws and reserved streams

- **Build:** `StarDraws::for_star(seed, BodyId)`: one struct of fixed draws, each read from its own
  domain tag (the list under "Generator version"), none depending on time or on another draw.
  `for_attempt(seed, body, attempt)` reads the same tags with the draw counter offset by attempt ×
  64, so attempt 0 is `for_star` and a redraw never touches another tag (plan 01's normals take two
  words each, so a block holds 32 tries of a redrawn normal). Fields are typed (`UnitUniform` and
  `StandardNormal`, newtypes this task defines, since plan 01's samplers return bare `f64`, and
  `coords::UnitVector`), not bare `f64`. `StarDraws::from_parts` for quadratures and tests, and
  `StarDraws::median()`. Directions use two uniforms (z and azimuth) in galactic axes, through
  `UnitVector::from_components`. Register every `star.*` tag of "Generator version" (scope `Body`)
  in plan 01's `domain_tags!` registry, `rng/tags.rs`, under a "Plan 06" heading; the file's rule
  is that a tag is added by the task that first opens a stream under it, so `system.metallicity`
  waits for T3, `stellar.reference` for T19.b and the event tags for T27.a. Streams are opened with
  `Stream::open(seed, tag, ObjectKey::from(body))`, which asserts the `Body` scope, and redraws use
  `Stream::seek`.
- **Files:** `stellar/draws.rs`, `rng/tags.rs`.
- **Tests:** golden values for three pinned `(seed, BodyId)`; adding a field with a new tag leaves
  the pinned values unchanged (the test reads fields by tag); order independence (A then B equals B
  alone, through `hyperion_testkit::order::assert_order_independent`); the registry's compile-time
  collision assertion covers the new tags.
- **Accept:** golden file `tests/golden/stellar/star_draws.golden` committed and passing;
  `cargo test -p hyperion-sim -- stellar::draws rng::tags star_draws` passes (the golden test's
  name contains `star_draws`).

#### P06.T3 Metallicity draw per system

- **Needs:** plan 03's `SystemRecord` and plan 02's `Galaxy` (P02.T9), and P02.T7.e's
  metallicity as revised for the 2026-09-21 rulings; the only task of phases A–F that does.
- **Build:** `stellar::system::draw_metallicity(galaxy, record)`. It is defined for grid records
  only (`SystemOrigin::Grid`; plan 09's members bring their own `Composition` and never reach it),
  so `record.component()` is `Some(c)`; a `None` is a `debug_assert!` and falls back to the
  population's first component. The record's component is `galaxy.fields().component(c)`, and its
  `metallicity(&point, record.age_at_epoch())`, with `point` the `PointLy` of the record's epoch
  position, returns the `FehDistribution` for that population or halo component, place and age.
  [Fe/H] = its `mean()` plus its `sigma()` (both `Dex`) times one standard normal on the tag
  `system.metallicity` (scope `System`, registered here), opened with `ObjectKey::from(SystemId)`.
  A system not yet born at the epoch (negative age) reads the field at age zero. Helium excess is
  zero for every grid system. Returns `Composition`.
- **Files:** `stellar/system.rs`, `rng/tags.rs`.
- **Tests:** over 10⁵ sampled thin-disc records at Milky Way parameters, the radial gradient fits
  the galaxy's drawn one (`GalaxyParams::metallicity_gradient`, about −0.05 dex per kpc) within the
  field's own stated tolerance; the halo's two main components separate in [Fe/H]; K–S against the
  field's normal at one fixed position; golden for pinned IDs.
- **Accept:** the slow test passes under `just test-slow`; goldens pass.

### Phase B: the Hurley, Pols and Tout backbone

All of phase B implements Hurley, Pols and Tout (2000, MNRAS 315, 543; "HPT" below) from the paper.
Section numbers are those of the journal version. The published SSE Fortran is used only to produce
reference output (T12) and to settle a suspected misprint, which the doc comment then records. No
code is copied from it. Every formula here is a pure function of mass, metallicity and age, so the
phase reads plan 01's `math` and `units` and T1.a's types, from T10.c on T2's `StarDraws`, and
nothing of plans 02 or 03.

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
  bench reports the cost (target under 5 µs). The bench is this plan's first: it creates
  `crates/hyperion-sim/benches/stellar.rs` and its `[[bench]]` entry (`harness = false`, as
  `foundation` and `galaxy` have) in the crate's `Cargo.toml`, which T32 extends.
- **P06.T4.c ZAMS luminosity and radius.** `zams::luminosity(m, &ZCoeffs)` and `zams::radius` (Tout
  et al. 1996 equations 1 and 2). Files: `stellar/sse/zams.rs`. Tests: 1 M☉ at Z = 0.02 gives about
  0.70 L☉ and 0.89 R☉; both are continuous and L is monotone in mass over 0.1–100 M☉ for five
  metallicities. Accept: tests pass.

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
  ignition `t_he_i`. Tests: the tip of the giant branch for 1 M☉ at Z = 0.02 is 2,700–3,000 L☉ with
  a core near 0.48 M☉ (HPT's formulae give 2,752 L☉ and 0.477 M☉; BaSTI 2,985 L☉ and 0.478 M☉); the
  tip luminosity rises with metallicity (Salaris and Cassisi 1997, MNRAS 289, 406: 1,977 L☉ at Z =
  10⁻⁴ to 2,742 at 0.006). Corrected in round 6's validation from "near 2,500" and "falls".
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
  age, and finds all three bit-identical, which is "the grid does not depend on the query". The
  white-dwarf ending and the convergence test read the end of the track, which T10.d's hand-over
  and T10.e's lifetime complete, so they land with T10.e; T10.c alone passes the rest.
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
  `evolve` and `lifetime` convenience functions, and `turn_off_mass(age, comp)` by bisection
  (plan 02's `galaxy::quad::bisect`) on `t_zams + t_ms`, with `t_zams` zero until T15.b adds the
  pre-main sequence. `lifetime` is what plan 08's placement calls for layers D and E, up to twice per
  accepted record, so it integrates only what the death time needs (mass and core mass under the
  wind, no radius, luminosity output or remnant stage) and stores no track; it must return exactly
  `Track::lifetime` of the full build, which a test pins over 10⁴ random inputs. Tests: lifetime is
  continuous and monotone decreasing in mass apart from the documented jump across `m_hef`; the
  property test "no living phase at an age beyond the lifetime, no remnant before it" over 10⁵
  random inputs.
- **Files:** `stellar/sse/wind.rs`, `track.rs`, `evolve.rs`. **Accept:** tests pass; bench targets
  of "Verification" reported.
- **Speed note (ruling 77).** HPT's formulae under `stellar/sse/` raise their positive bases with
  `math::powf_positive` (exp(y ln x) on the pinned `libm`), not `math::powf`; a base that can reach
  zero keeps `math::powf`. Everything outside `stellar/sse/` stays on `math::powf`. `lifetime` stays
  bit-equal to `Track::lifetime`, and its 5 µs target passes to a fitted lifetime table built with its
  first bulk consumer (P06.T30 or plan 08's placement). See "The integrator's speed, as optimised"
  and "`powf_positive` in the stellar formulae" under Risks.

#### P06.T11 Remnant structure from the backbone

- **Build:** the white dwarf kinds by core mass at envelope loss (He, CO, ONe), white dwarf radius
  (HPT section 6.2.1), neutron star radius (10 km in HPT; use 11.5 km with a citation to a current
  measurement, and keep 10 under `RemnantRecipe::Hurley2000`), black hole radius 2GM ÷ c², and the
  original remnant mass formula from the core mass at supernova, kept only under
  `RemnantRecipe::Hurley2000` for validation.
- **Files:** `stellar/remnant/mod.rs`, `structure.rs`.
- **Tests:** a 0.6 M☉ white dwarf has a radius of 0.012–0.013 R☉; radius falls with mass and
  vanishes at the Chandrasekhar mass; under the original recipe the remnant turns from a neutron
  star to a black hole at the core mass where the formula passes the largest neutron-star mass.
  The endings by initial mass (a 20 M☉ star at Z = 0.02 leaves a neutron star and 40 M☉ a black
  hole, matching SSE) need the tracks of T10 and are checked by T12.b, so this task needs only
  T1.a.
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
  Myr at 1 M☉ (Mamajek 2009), scaled by m^(−0.1) below 1 M☉ (Luhman et al. 2005's brown-dwarf and
  M-star disc fractions) and by m^(−1.06) above it (halving by 2 M☉, as Ribas et al. 2015 and
  Mamajek measure), held to 0.3–15 Myr (ruling 38). It decides classical against weak-lined T Tauri
  (T24), bounds FU Orionis activity (T28.d), and is there for plan 14. By ruling 33 of 2026-09-22 it
  is the one lifetime of a star's circumstellar disc, so that the star's T Tauri class and its
  planets' formation see the same disc: plan 14's `disc::derive` takes it as an argument (P14.T3.a)
  and draws no lifetime of its own, except for a circumbinary disc, whose rank plan 14 draws on
  `planet.disc` and puts through this same law at the pair's total mass. Mamajek (2009, AIP Conf.
  Proc. 1158, 3) measures an e-folding time of about 2.5 Myr for the disc fraction, which is the
  survival function of an exponential. The law, a function of the mass and of the `UnitUniform` rank
  `StarDraws::disc_lifetime()` that draws nothing itself, is built in `stellar/premain.rs` ahead of
  the rest of T15, because plan 14 is its first caller (the `planet` lane, round 7; see its as-built
  record in Risks); its tests (the median, the clamps, monotonicity in the rank) went with it.
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

- **P06.T19.a Interface and ordinary mode.** The normal quantile this subtask once began with is
  P06.T1.b, which needs nothing of this plan and lands early. The law: `KickLaw`, `KickLawParams`,
  `StandardKickLaw`. Score x = (M_CO − M_rem) ÷ M_rem × ξ with ξ normal about 1 with σ = 0.45,
  redrawn deterministically (next draw numbers on `star.kick.score`) until positive. Rank r =
  F_x(x) from `KickRankTable`, clamped to 0.001–0.999, speed = exp(5.60 + 0.68 × Φ⁻¹(r)) km/s,
  which spans 33–2,200 km/s (Disberg and Mandel 2025, ApJL 989, L8, for μ and σ; Disberg, Mandel
  and Hirai 2026 for the 45%). Φ⁻¹ is `math::normal_quantile` (T1.b). Direction isotropic from
  `star.kick.direction`. `KickDraws` with `of(&StarDraws)` and `from_parts`, so that plan 08's
  quadrature can drive the law from explicit variates. Until T19.b lands the table, the unit tests
  of this subtask use a two-knot test table.
- **P06.T19.b Reference population and provisional rank table.**
  `remnant::reference::ReferencePopulation`: Kroupa primaries of 8–150 M☉, Z = 0.02, iron-core
  collapses of single and wind-stripped progenitors that leave a neutron star, sample i drawn on the
  tag `stellar.reference` (scope `Galaxy`, registered in `rng/tags.rs` by this subtask, object
  `ObjectKey::galaxy_item(i)`) from the given seed
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
- **Files:** `stellar/remnant/kick.rs`, `reference.rs`, `tables/kick_rank.rs`, `tables/mod.rs`,
  `rng/tags.rs`.
- **Accept:** `just test-slow` passes the six tests; a golden pins the kicks of three pinned IDs.

#### P06.T20 White dwarfs: cooling and spectral types

- **P06.T20.a Cooling.** Luminosity from cooling age, by mass and core composition (He, CO, ONe),
  under the modern recipe by a fit to the Montreal evolutionary sequences of Bédard et al. (2020,
  ApJ 901, 93), through `hyperion-fit run wd_cooling` into `tables/wd_cooling.rs` (ruling 57.2;
  first built as the two-piece modified Mestel law of Hurley and Shara 2003, ApJ 589, 179, which
  failed the check below and is kept as the §6.3 perturbation's target). `Hurley2000` keeps HPT's
  equation 90. T_eff from L and the radius of T11. The cooling age counts from the end of the
  post-AGB bridge, whose end luminosity the law is matched to; T16.a, which hands over to this law
  and so lands after it, moves the origin there from T10.d's direct hand-over at envelope loss.
  Check against the 0.6 M☉ CO thick-hydrogen sequence of Bédard et al. (2020), at models held out
  of the fit: T_eff within 10% from 0.01 to 10 Gyr; and continuity in mass across the fitted range.
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
- **P06.T26.d Light factor with cycle-keyed irregularity** (needs T27.c and T29.a's `StarModel`).
  `light_factor_at(star: &StarModel, t) -> f64`:
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
  `number => CONST = tags::CONST`, with the numbers listed under "Generator version"; all ten land
  here, ahead of the T28 kinds that open them, because their numbers are reserved in order. Write
  the number blocks of "Conventions fixed here" into `event_tags.rs`'s module doc, which today says
  only that later stages "allocate the blocks their plans set aside", for plans 09, 11 and 14;
  re-export the tags as `events::tags`. An event's marks come from `EventKey::event_stream`, so no
  event kind needs a second tag; `TimeWindow` (half-open, on `UniverseTime`); helpers that build an
  `EventId` from a subject, tag, `EventBin` and index (`EventId::new(subject, EventWord::new(tag,
bin, j))`). Add `events` to `lib.rs` and its crate doc.
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
  `events::testing` is the crate's first `testing` module, so this subtask declares the `testing`
  feature in the crate's `Cargo.toml` (it has no `[features]` yet), with the module under
  `cfg(any(test, feature = "testing"))`.
- **Files:** `crates/hyperion-sim/src/events/{mod.rs, tags.rs, bins.rs, phase.rs, testing.rs}`,
  `rng/tags.rs`, `id/event_tags.rs`, `lib.rs`, `benches/events.rs` with its `[[bench]]` entry and
  the feature in `crates/hyperion-sim/Cargo.toml`. The phase needs plan 01 alone and nothing of
  this plan, so it can run beside T1–T12.
- **Accept:** `cargo test -p hyperion-sim events` and the slow statistical tests pass; bench numbers
  recorded in the task's commit message.

#### P06.T28 Single-star event kinds (`stellar::events`)

Each kind is a `RateModel` or a `PhaseClock` built from T29.a's `StarModel`, a mark sampler, and a
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

`StarModel` needs no record and `SystemStars` does, so the task is split: T29.a lands after T10 and
phase D and before T26.d and T28, which take a `StarModel`; T29.b needs plan 03 and T3.

- **P06.T29.a `StarModel`.**
  - **Build:** `StarModel::new(m0, Composition, StarDraws, age_at_epoch: Years)`: the track from
    mass, composition and those draws, built to the age at the epoch + H and completed to death if
    the star is already dead or dies within the window (design note 19), then the remnant stage, a
    private `remnant_stage(&Track, &StarDraws)` that reads only the `star.remnant.*`,
    `star.stripped` and `star.kick.*` fields. `state_at(t: UniverseTime)` evaluates the track at
    the age at the epoch plus t (design note 23). By ruling 34 of 2026-09-22 `StarModel` is how
    plan 14 reads every star, never `Track`: it also exposes `lifetime`, `death`,
    `max_radius_until` and `max_luminosity_until`, for every star, the cooling-fit stars below
    0.1 M☉ included (T13), whose radius falls monotonically with age.
  - **Files:** `stellar/system.rs`.
  - **Tests:** `state_at(UniverseTime::EPOCH)` equals `Track::state_at(age_at_epoch)` bit for bit;
    a star dead at the epoch has its full track and a remnant; order independence.
  - **Accept:** `cargo test -p hyperion-sim stellar::system` passes.
- **P06.T29.b `SystemStars`.**
  - **Build:** `SystemStars::generate` (metallicity, draws, and T29.a's model). `generate` is three
    separate steps, because plan 08's P08.T12.c repeats only the last: the primary's draws
    (`StarDraws::for_star` here; plan 08 makes it `for_attempt(.., record.mark_attempt())`), the
    track, and T29.a's remnant stage. Plan 08's kick loop calls that last step again with the same
    fields of later attempts, on the one built track, until the record's `kick_constraint()` is
    met, so an attempt costs a remnant and a kick and never a track; no track draw is ever redrawn.
    With no constraint, which is every record of this plan, the step runs once. `summary_at(t)`: age
    = `record.age_at(t)`; plan 03's `record.existence_at(t)` of `NoSystemYet` gives
    `SystemExistence::NotYetBorn` and no stars. `brief_at(t)`: state and classification only.
    `death_time()`: T = lifetime − age at the epoch as a `UniverseTime`, `BeyondClockRange` when the
    seconds do not fit an `i64`. `natal_kick()`, `lbv_window()`. `ObjectKind` from phase and
    class. Nothing here changes existing output, so the version stays (there is no changelog to
    extend).
  - **Files:** `stellar/system.rs`.
  - **Tests:** golden summaries (`tests/golden/stellar/summaries.golden`) for a dozen pinned IDs
    across all five layers at t = 0 and ±500 years; order independence; determinism across two
    runs; a star whose T falls at +100 years is living at +99 and a remnant at +101, under the same
    ID; the property test of the brainstorm, over 10⁵ random IDs and times: no star in a living
    phase is older than its lifetime, every remnant is older, no state has a non-finite or
    non-positive L, R or T_eff (black holes and `NoRemnant` excepted).
  - **Accept:** tests and goldens pass.

#### P06.T30 Real lifetimes and remnant masses in the mean mass per system

This task needs T10, T18 and plan 02's `Galaxy` (P02.T9), which holds the table; it needs nothing
of plan 03. Its brackets also assume that plan 02 has switched the default mass function to
Chabrier's (ruling 2 of 2026-09-21): as built, `MassFunctionKind`'s default and
`GalaxyParams::milky_way_like` are still Kroupa's.

- **P06.T30.a The seam.** Plan 02's `galaxy::fates::StellarFates` takes a mass and no metallicity,
  and `mean_present_mass(f, fates, ages)` is called once per population with that population's age
  distribution, the halo's through `mean_present_mass_of_mixture` over its components (both in
  `galaxy/params/derive.rs`'s `mean_masses`, which builds one `ProvisionalFates` for all seven).
  Leave the trait as it is and give each population its own fates value. Add
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
  potential tables are. `mean_companions` delegates to `ProvisionalFates`. `TrackFates` implements
  the trait's `breaks()` with the masses where its table has a kink or a jump, which plan 02's
  quadrature takes as panel edges. `fates_for` switches to it; `build`'s `mean_formed_mass` reads
  only `mean_companions` and may keep `ProvisionalFates`. Bump `GENERATOR_VERSION` in `version.rs`,
  with the unit test there that pins its value; regenerate every golden file in the same commit
  with `just bless`.
- **Tests:** at Milky Way parameters the mean present-day mass per system under the default,
  Chabrier's system function, is 0.55–0.59 M☉ (0.48 ± 0.02 under Kroupa's), varies by under 5%
  between the old populations, and the young disc's is 30–50% higher; the share of dead primaries is
  8 ± 2% in the old thin disc, 11 ± 2% in the thick disc and 13 ± 3% in the halo (the research
  figures behind the brainstorm's 0.48); the system count for a 5 × 10¹⁰ M☉ galaxy is near 0.9 ×
  10¹¹ (10¹¹ under Kroupa's).
- **Files:** `galaxy/fates.rs`, `galaxy/params/derive.rs` (the call sites of plan 02's P02.T4 and
  P02.T5, in `mean_masses` and `build`), `stellar/fates.rs`, `version.rs`, all goldens.
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
  `PROTOCOL_VERSION` (2) does not change: plan 04's design note 15 says a new kind or optional
  field does not bump it, because an older server answers `unsupported`. The two optional fields
  are the protocol's first with `serde(default)` and `skip_serializing_if` (plan 04 sends absent
  optionals as `null`), so they also take ts-rs's `optional` attribute, and the generated
  TypeScript marks them optional. The TypeScript request client needs no change, but the client's
  exhaustive switch over `ErrorCode`, `settledState` in `lib/useServerRequest.ts`, gains the
  `unknown_system` case in this task, so that `just ci` stays green after `just gen-protocol`. The
  server's exhaustive matches over `RequestBody` (`requests::{kind, is_large}` and `Handlers`) gain
  the new kind, which answers `unsupported` until T34; its size class is small.
- **Files:** in `crates/hyperion-protocol/src/`: `envelope.rs` (the bodies, `REQUEST_KINDS`,
  `ErrorCode`), `galaxy.rs` (the range request and row), and the new DTOs there or in a new
  `stellar.rs`; `crates/hyperion-server/src/requests/mod.rs`;
  `apps/hyperion/src/renderer/src/lib/useServerRequest.ts`; then `just gen-protocol` and the
  regenerated `packages/protocol/src/generated`.
- **Tests:** a wire-form pin for the `system_summary` request inside its `request` envelope, for its
  response inside `response`, for a `request_error` with `unknown_system`, and for each new DTO; the
  tests that pin `REQUEST_KINDS` (`request_kinds_are_pinned`, and `request_kinds_lists_every_variant`
  through its `next_request` and `next_response` helpers) and `error_code_strings` are updated; an
  old-form range request without `include_stellar` still parses, and a range response without
  briefs serialises exactly as plan 04 pinned it.
- **Accept:** `just gen-protocol-check` and `just ci` green.

#### P06.T34 Server

- **Blocked in part (ruling 77.3):** the `system_summary` handler is built. The range handler's
  `stellar` briefs are blocked on speed: a brief needs each star's state at the requested time, so
  a track per system, and tracks are still 4–7 times over the Verification's `generate` targets
  after ruling 77.1. That misses the Verification's 15 ms for 1,600 briefs. The fitted lifetime
  table alone does not unblock them, since it gives the lifetime and not the state. What does is
  open, and queued for a ruling: tracks within their targets, or a fitted table of brief states.
- **Needs:** plan 04's P04.T14, which lands the range handler, the universe registry and the caches
  in `AppState` (through P04.T13 every kind still answers `unsupported`), and plan 03's `resolve`.
- **Build:** the range handler fills `stellar` when asked, on the CPU pool inside the range job,
  from a byte-bounded `SharedByteLru` of `SystemStars` keyed by `(GalaxyKey, SystemId)`, as plan
  04's design note 23 keys every cache with `(seed, generator_version)` (epoch state, as the
  brainstorm requires of caches; `SystemStars` implements `HeapBytes`; budget from a new
  `HYPERION_SYSTEM_CACHE_MB`, default 128, beside `HYPERION_CELL_CACHE_MB` and
  `HYPERION_MAP_CACHE_MB` in `config.rs`).
  The `system_summary` handler is an interactive pool job: it validates the time against ±H as plan
  04's limits do for the range query (`bad_request` with `field: "time"`), resolves the ID through
  plan 03's `resolve` (any `ResolveSystemError` gives `request_error` with `unknown_system` and
  `field: "system"`), generates or fetches the model and evaluates at the requested time. A
  `SystemIdHex` that fails to parse is already plan 04's `bad_request`.
- **Files:** `crates/hyperion-server/src/` (handlers under `requests/` and cache wiring as P04.T14
  lays them out), `config.rs`, and integration tests in `crates/hyperion-server/tests/`, whose
  `common/mod.rs` holds `TestServer` and `TestClient`.
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
  says what is hidden (`412 OF 1630 SHOWN — LIVING`, digits grouped from five as the guide and
  plan 05's `lib/format.ts` have it), per the guide's honest-data rule. Legend
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
  under a millisecond"; a miss is a finding): `ZCoeffs::new` under 5 µs; a lifetime for a primary of
  layer D or E in about 5 µs (plan 08's 5 ms query budget counts on it: its design note 28 and
  P08.T16), met not by `stellar::lifetime`, which keeps bit-equality with `Track::lifetime` at
  0.5–0.75 ms, but by a **fitted lifetime table** (`hyperion-fit`, in log M, Z and the draws that
  matter, with a stated tolerance against `Track::lifetime`), built with its first bulk consumer,
  P06.T30 or plan 08's placement (ruling 77.3); `generate` for a main-sequence dwarf under 10 µs,
  for a giant under 60 µs, for a remnant (full track) under 150 µs, so that a triple of evolved stars under plan 11 still leaves plan 14
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

- **Updated for the 2026-09-21 density rulings.** The default mass function is now Chabrier's system
  function, so T30.b's bracket is the default's 0.55–0.59 M☉, with Kroupa's 0.48 kept as a second
  case. The thin discs' age–metallicity relation is now flat to 8 Gyr, so the thin discs' entry in
  T30.a's `reference_fe_h` follows whatever mean P02.T7.e sets at the reference radius.
- **Re-validated at 70c6052**, against plan 01, plan 02's T1–T8, plan 04's T1–T13 and plan 05's
  T1, T3 and T5–T7 as built. Since the plan was written the brainstorm changed only by the
  2026-09-21 rulings, which touch T30.b (applied above) and the thin disc's metallicity that T3
  reads from P02.T7.e; no design note contradicts them. Edits:
  - _Order._ The ordering note had phases A and F wait on plan 03. Only T3 and T29.b read it; T1,
    T2, T27 and phase B need plan 01 alone, and T30 needs P02.T9, not plan 03. T16 needs T20.a (the
    note had the reverse, though T16 hands over to T20's law). T26.d and T28 take a `StarModel`,
    which only T29 built, so T29 is split into T29.a (`StarModel`, no record) and T29.b. T11's
    20 and 40 M☉ endings need tracks and are left to T12.b, so T11 needs only T1.a; T10.c's
    end-state tests land with T10.e.
  - _The normal quantile_ moved from T19.a to a new T1.b, the ID plans 08 and 10 already cite; it
    needs plan 01 alone.
  - _Names._ `Dex` is plan 02's. `UnitUniform` and `StandardNormal` are new in T2, and the
    direction type is `coords::UnitVector`. `rng/tags.rs` has each tag added by the task that
    first opens it, so `system.metallicity` moves to T3 and `stellar.reference` to T19.b.
    `FehDistribution` is in `Dex`. The P02.T4 and P02.T5 call sites are
    `galaxy/params/derive.rs`. `GENERATOR_VERSION` has no changelog, and its value is pinned by a
    test in `version.rs`. The sim crate has no `testing` feature (T27.d declares it). Bench files
    and their `[[bench]]` entries are created by T4.b and T27.d. T4.c has a file, T1.a an
    acceptance filter that selects its tests, and T2 a command. The exhaustive `ErrorCode` switch
    is `settledState`. T33's modules are `envelope.rs` and `galaxy.rs`, and its optional fields are
    the protocol's first. T34's cache key is `GalaxyKey`, and T35.b's count groups digits from
    five.
  - _Pending re-validation._ T3 and T29.b wait on P03.T5.a (`SystemRecord`, `Existence`) and P03.T7 (`resolve`)
    and P02.T9 (`Galaxy`). T30 waits on P02.T9 and on plan 02 switching the default mass function
    to Chabrier's (the code's default and `milky_way_like` are still Kroupa's). T34 waits on
    P04.T14. T35–T37 wait on P05.T9–T11 (`SystemList`, `SystemReadout`, `SymbolLegend`,
    `chartModel.ts`, `ChartControls.tsx`, `useRangeQuery.ts`, `RecordingContext2D`).
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
  here (T1.b); `SymbolShape` gains `ringed-circle`; `ErrorCode` gains `UnknownSystem`, which plan
  14 expects from here; the event-tag number blocks under Provides are proposed here and must be
  honoured by plans 09, 11 and 14, none of which names numbers yet. Plan 09 asks that draws accept
  an attempt number (`StarDraws::for_attempt`) and plans 11 and 14 ask for the monotone helpers on
  `Track`; both are provided.
- **Deviations in T1.a, as built.** `ObjectKind` lives in `stellar::state`, re-exported as
  `stellar::ObjectKind`, because T1.a makes `system.rs` a `//!`-only file; no consumer names a path
  to it. `StarState` is built by `StarState::new(StarStateParts)`, a struct of the eight required
  inputs with public fields (none is optional, so no builder); it derives the envelope mass (M −
  M_c, at least +0) and T_eff, and debug-asserts the documented ranges (age ≥ 0, a remnant all
  core, `NoRemnant` massless and dark). T_eff is 0 K at zero luminosity; `surface_gravity()` is
  `Option<Dex>`, log₁₀ g in cm s⁻², `None` at zero mass or radius. T☉ is
  `units::consts::SOLAR_EFFECTIVE_TEMPERATURE_K`. Added beyond the sketch:
  `Composition::{fe_h, helium_excess, SOLAR}`, the constants `Z_SOLAR`, `Z_FIT_MIN` and
  `Z_FIT_MAX` of `stellar::composition`, `Phase::ALL`, `ObjectKind::ALL`. `stellar::testing` is
  `#[cfg(test)] pub(crate)` until the crate has its `testing` feature (T27.d, another lane);
  whichever of T27.d and T1.a merges second, and at the latest T31, whose integration test calls
  it, moves it under `cfg(any(test, feature = "testing"))` and makes it `pub`.
- **Deviations in T2, as built.** The decision draws are `rng::Mark`s, not `UnitUniform`s
  (brainstorm, "Floating point": decisions compare integers; the `KickDraws` sketch already has a
  "mode mark"): `star.magnetism`, `star.stripped`, `star.remnant.type`, `star.remnant.fallback`,
  `star.kick.mode` and the three `star.wd.*`. The median mark is 2⁵², below a threshold of
  probability p exactly when p > ½. T25 takes the fossil field's strength rank from the magnetism
  mark as (mark + ½) ÷ k below its threshold k; T30.b builds its type and fallback nodes as marks.
  `star.kick.score` and `star.ns.spin` are held as the first `REDRAW_TRIES` = 8 normals of the
  attempt's block (16 of its 64 words), in draw order; T19.a and T21.a take the first their
  condition accepts and must document a fallback for all eight failing (about once in 10¹⁵ and 4 ×
  10¹² stars). `star.ns.geometry` holds the spin axis (words 0–1), then the magnetic inclination
  as a rank (word 2). `from_parts` takes `StarDrawsParts`, a struct with public fields and a
  `MEDIAN` constant for struct-update syntax; the median's directions are galactic north (a
  convention). Added: `ATTEMPT_WORDS`, `REDRAW_TRIES` (a const assertion keeps their product in
  the block; changing either is a version change), `UnitUniform::{new, value, HALF}`,
  `StandardNormal::{new, value, ZERO}`, a getter per field and `parts()`. The golden test is
  `star_draws_are_pinned` in `tests/stellar_draws.rs`. T2 also extends
  `tests/golden/rng/tags.golden` by the 22 tags (`domain_tags_are_pinned`, which the acceptance
  command does not select); merging another lane's tags means regenerating that file with that
  test.
- **Deviations in T4, as built.** The tables are `pub(crate)` constants `coeffs_data::{A, B,
ZAMS_L, ZAMS_R}` of type `[[f64; 5]; N]` indexed by coefficient number, with the paper's digits;
  row 0, the closed-form rows (a17, a33, b2, b3, b17, b26, b45, b47, b50) and the unused b8 and
  b35 are zero. Their types fix the lengths, so there is no length test; an FNV-1a checksum of the
  bits pins each table and a test pins which rows are empty. They were read from the papers by
  script and then checked number by number against the SSE package's data statements (`zdata.h`,
  `zcnsts.f`), which agree to the last digit: a use of the SSE source beyond settling misprints,
  for the owner to confirm. The paper's text layer loses b45's parentheses: b45 = 1 − (2.47162ρ −
  5.401682ρ² + 3.247361ρ³), as printed and in SSE. Equation 21a is a58 M^a60 ÷ (a59 + M^a61) as
  the journal prints it (the arXiv preprint misprints a59 M^a61); `ZCoeffs::alpha_r_power_law`
  holds the one copy, which T5's αR shares. Equation numbers in doc comments are the journal's,
  which splits some of the preprint's (9a/9b, 19a/19b, 21a/21b, 22a/22b). The a68/a66 special case
  makes a64 jump in Z near 0.016, as in the paper and SSE; it keeps αR continuous in mass.
  `M_hook` and `M_HeF` are quadratics in ζ with minima inside the fitted range (Z ≈ 0.0025 and
  0.00073, as SSE's values confirm), so "each critical mass is monotone in Z" cannot hold: the test
  asserts that `M_FGB` rises, all three are continuous at 200 points, and `M_hook` and `M_HeF` turn
  once each. `M_FGB` keeps the paper's rounded constants (13.048, 0.0012), within 0.18% of SSE's
  16.5 Z^0.06 ÷ (1 + (10⁻⁴ ÷ Z)^1.27). Tout et al.'s fits are their equations 1 and 2, with the
  coefficients from their equations 3 and 4 (Tables 1 and 2). `ZCoeffs::new` clamps Z to the
  fitted range and panics on NaN. Added: `ZCoeffs::{z, zeta, m_hook, m_hef, m_fgb}`; tests against
  SSE output (critical masses at the five T12 metallicities, ZAMS L and R at four points), with the
  SSE run's provenance in `stellar/sse/mod.rs`; a continuity sweep of every aₙ and bₙ in Z. Bench:
  `ZCoeffs::new` 1.81 µs against the 5 µs target.
- **Deviations in T5, as built.** `stellar::sse::ms` is `pub(crate)` (T10 is its first caller;
  until then it carries `cfg_attr(not(test), expect(dead_code))`, which T10 removes, as it does on
  `ZCoeffs::{a, b, alpha_r_power_law}`). `t_bgb`, `t_hook`, `t_ms` (in `Megayears` from the ZAMS),
  `l_tms`, `l_bgb` (`SolarLuminosities`) and `r_tms` (`SolarRadii`) take `SolarMasses`; L and R
  along the main sequence come from `MainSequence::new(m, &c).at(t)`, which returns the new
  `sse::PhasePoint { luminosity, radius, core_mass }` that every later phase returns too. Equation
  6's x is the SSE code's form, max(0.95, max(0.95 − (10/3)(Z − 0.01), min(0.99, 0.98 − (100/7)(Z −
  0.001)))): the printed max(0.95, min(0.95 − 0.03(ζ + 0.30103), 0.99)) is a different fit, not a
  misprint; the two agree for Z ≤ 0.0003, at 0.001 and for Z ≥ 0.01, and at Z = 0.004 (0.962
  against 0.970) the printed form moves `t_ms` by 0.8% and late main-sequence L by 0.02–0.06 dex
  against SSE, beyond T12.b's 0.02 dex; for the owner to confirm, since the phase consults SSE
  only for misprints. Misprint settled against SSE: equation 22b's denominator is a74 − 1.0
  (printed a74 − 1.06 in the journal too). Equation 23's low-mass branch takes |M − a78| as SSE
  does (no numerical effect), and its last branch ends at a75 + 0.1 as the journal prints. R_TMS is held at 1.5 R_ZAMS below 0.5 M☉ as printed
  (SSE holds it up to a17, where a test shows it never binds). Equation 24's degenerate floor uses
  X = 0.76 − 3Z (Pols et al. 1998) at every mass; it binds only near 0.1 M☉, where it exceeds Tout
  et al.'s R_ZAMS (0.135 against 0.130 R☉ at Z = 0.02), so the τ = 0 and 1 tests compare with the
  floored radii and T15.b's blend must meet `MainSequence::at(0)`, not `zams::radius`. Continuity is
  asserted with a test-only bisection jump detector (`stellar/sse/continuity.rs`) instead of a
  Lipschitz bound, which the hooks' (M − `M_hook`)^0.4 and ^0.5 rises and R_TMS's steep ramp at Z =
  10⁻⁴ would break, applied both to L and R and to each coefficient of equations 16–23 at its own
  scale. The 0.75 M☉ test sweeps 200 metallicities. Added: L and R against SSE's `hrdiag` at nine
  (Z, M, t) points to 10⁻⁹ (all 5 × 31 × 400 main-sequence rows of the run agree to 10⁻¹⁴).
- **Deviations in T6, as built.** `stellar::sse::{hg, gb}` are private modules of `pub(crate)`
  items with `ms`'s dead-code expectation, which T10 removes. The gap is `HertzsprungGap::new(m,
&c).at(t)` (`t_start` = t_MS, `t_end` = t_BGB); the branch below `M_FGB` is
  `FirstGiantBranch::new(m, &c).at(t)` (`t_hei`); both return `PhasePoint` and debug-assert their
  time range. Equation 37 is `GiantBranch` (`m_x`, `l_x`, `luminosity`, `core_mass`, `times` →
  `GiantTimes`, `core_mass_at`, `time_of_luminosity`); R_GB is `gb::radius`, L_HeI `gb::l_hei`. The
  gap's end and equation 44 need parts of HPT 5.3–5.4, so `gb.rs` also holds, for T7 and T8 to
  reuse: `mc_hei`, `mc_bgb` (eq. 44), `r_hei` (eq. 50), `r_mhe_intermediate` (eq. 55 from `M_HeF`
  up; T7 adds the branch below), `blue_fraction_massive` (eq. 58 above `M_FGB`; T7 adds the rest),
  `agb_radius` (eq. 74) and `mc_bagb` (eq. 66), the `m_c_bagb` of T8, T18.b and T28.f, which T8.b
  re-exports from `sse` under that name since `gb` is private. Two choices follow SSE, for the owner
  to confirm: p, q and log D change form over 2.0–2.5 M☉, not `M_HeF`–2.5 as printed (same at Z =
  0.02; at Z = 0.001 the printed form leaves a 2 M☉ giant up to 0.06 dex fainter at a given age,
  beyond T12.b's 0.02 dex); and above `M_FGB` a star with no blue phase (τ_bl zero below 10⁻¹⁰, as
  SSE's `tblf`) ignites helium at R_AGB(L_HeI), not R_mHe, so that core helium burning, which then
  starts on R_AGB (eq. 64), begins where the gap ends (every star above `M_FGB` at Z ≳ 0.022, where
  1 − b47 < 0, and elsewhere where R_mHe ≥ R_AGB). Equation 44's c₁ is the printed 9.20925 × 10⁻⁵
  (SSE: 0.09796164⁴), so the SSE tests (4 gap and 6 branch points) hold L and R to 10⁻⁹ and Mc to
  10⁻⁷. T6.c re-checked: equation 49 puts the 1 M☉, Z = 0.02 tip at 2,752 L☉ (SSE agrees; Fig. 11
  about 2,800) with a 0.477 M☉ core, and the bolometric tip **rises** with metallicity (1,933 L☉ at
  Z = 10⁻⁴ to 2,814 at 0.03; Cassisi and Salaris 1997 agree), so the tests assert 2,752 ± 1 L☉ and a
  rise over 200 metallicities; the plan's "near 2,500" and "falls" are for the owner to amend. Left
  to T10.d, whose Build text should gain it: HPT section 6.3's small-envelope perturbation of L and
  R (equations 97–100), which SSE applies from the gap to the AGB whenever μ < 1, including massive
  stars in the gap with no mass loss (μ ≈ 0.86 at 20 M☉), and which needs T9's helium ZAMS and the
  white dwarf's L and R; the SSE test points avoid it. Also for T10: the phase structs take one
  mass, while SSE evaluates R_GB and R_AGB at the current mass, and equation 30 keeps the larger of
  a mass-losing gap star's previous core and the formula's. Not done, from review: SSE reference
  rows for the gap above `M_FGB` (only self-consistency is tested there), and computing equation
  44's C and f_bl(`M_FGB`) once per Z rather than per star (T10's bench decides).
- **Deviations in T1.b, as built.** `math::normal_quantile` departs from Acklam's reference form in
  three places, each to keep its error at a few ulps: above ½ it is −Q(1 − p), with 1 − p exact,
  so the upper tail is as accurate as the lower and the mirror is exact bit for bit (the upper
  branch point moves to where 1 − p crosses 0.02425, so 0.97575 as an `f64` takes the tail
  branch); for 0.25 ≤ p ≤ ½ the Halley step evaluates Φ(x) − p as ½ erf(x ÷ √2) − (p − ½), since
  the erfc form loses x's relative precision as x → 0 at ½; and in the tail the Newton step is
  scaled by p, (Φ − p) ÷ p × √(2π) exp(x²/2 + ln p), which cannot overflow for subnormal p. The
  10⁻¹³ bound holds for normal p (3.3 × 10⁻¹⁶ at worst against 200-bit values). Below 2⁻¹⁰²² the
  step cannot resolve Φ(x₀) − p and Acklam's unrefined value remains: 1.8 × 10⁻⁹ at 2⁻¹⁰⁷⁴.
  Release builds return −∞ for p ≤ 0, +∞ for p ≥ 1 and a NaN unchanged, rather than the
  hardware's default NaN, whose sign differs by architecture; callers inverting a draw take it
  from `Stream::uniform_open` (`Stream::uniform` can return 0). The golden's arguments are a
  superset of the task's: 0.97575 ± 2⁻⁵⁵ rounds to 0.97575, so the upper branch point is crossed
  at 0.97575 ± 2⁻⁵³, and 0.02425, 0.25 (the erf switch), `f64::MIN_POSITIVE` and 5 × 10⁻³²⁴ are
  added. Strict increase is tested on steps of 2⁻⁵⁰ in p across 0.02425, 0.25, ½ and 0.97575:
  neighbouring doubles move the quantile by less than an ulp there, so it is monotone only to
  within its rounding. The round trip through `normal_cdf` is relative, with the complement checked
  above ½; since `normal_cdf` is the Φ the step drives to p, accuracy is also checked against a
  test-only transcription of Wichura's AS 241 (PPND16) to 10⁻¹³ relative.
- **Deviations in T27, as built.** The sketch's `(key: &EventKey, tag: EventTag)` pairs are one
  `events::EventSeries { subject, tag, key }`, built by `EventSeries::new(seed, tag, subject)`
  (which calls `EventKey::derive`), because an `EventId` needs the subject and a key does not
  carry it; every `events_in`, `active_at`, `event`, `phase_at` and `cycle_at` takes
  `&EventSeries`, and plans 09, 11 and 14 build one where their text says `EventKey::derive`. A
  tag's series goes to one construction only: bin k's count stream is cycle k's skip stream.
  `RateModel::bound` takes the bin's `TimeWindow` (so a model needs no Δ), and both methods return
  the new unit `events::EventsPerSecond` (`per_day`, `per_julian_year`), kept in `events` rather
  than `units.rs` while another lane edits that file. Phases are a split `Phase { cycles: i64,
fraction: f64 }`: `PhaseClock::base_phase` and `phase_at` return one and `time_at` takes one,
  because an `f64` of cycles resolves only 6 × 10⁻⁵ at P = 16 s over the source horizon;
  `LinearClock` takes a `Span` period of at least 16 s and inverts in integer arithmetic to within
  max(1 ns, 2⁻⁵² P). `PoissonBins::new(bin_seconds, look_back_bins)` rejects Δ outside 16 s to
  2⁴⁴ s; bins and cycles beyond the 40-bit numbers hold no events; a bin's mean above 64 fails a
  debug assertion; thinning is one integer `Threshold::from_ratio` decision per candidate, whose
  time is its word's top 53 bits scaled to the bin in integer nanoseconds. `MonotonePhase::new(a,
ℓ, J)` takes ℓ in whole cycles and 1–48 octaves with ℓ × 2^(J − 1) ≤ 2⁶², and the skip comes
  through `with_skip(SkipMark)`; added `octaves_to_span`, `LinearClock::cycles_in`,
  `noise_bound_cycles`, `amplitude_cycles` and `event_by_id`. Octave j's lattice value at index i is
  the first word of `event_stream(i, j + 1)`, so a phase kind draws all of a cycle's marks from
  `event_stream(n, 0)`. Roots are an integer-nanosecond bisection, not `galaxy::quad::bisect`
  (whose `f64` seconds resolve a millisecond at the horizon); Φ in `f64` is not monotone within a
  few ulps of n (over microseconds at periods of years), so the bracket, the margin, the form of
  `noise_bound_cycles` and the midpoint rule are output, and a ten-year case in the golden pins
  them. A per-call `Lattice` cache keeps amplitudes and the last lattice values; a test checks it
  bit for bit against cold bisection. `Phase::new` asserts a finite fraction in release builds too.
  "Exact arithmetic" is read as integer lattice indices and fractions from integer remainders, with
  3s² − 2s³ in plain `f64`. Tests: `events::testing` holds `partition`,
  `assert_partition_independent` and `ConstantRate` and does not depend on `hyperion-testkit`; the
  tests permute calls with `assert_order_independent` over the pieces, and the thread checks are
  separate tests compiled out on wasm32-wasip1, which has no threads (x86-64 and AArch64 run them).
  The diffusion test measures the variance, over 400 subjects, of the drift from cycle 0, tₙ − t₀ −
  nP, at lags 4–4,096 below a top octave of 8,192 (tₙ − nP alone is stationary across subjects);
  its fixed ratios (each fourfold lag at least doubles, 1,024 × the lag at least 64 ×) sit about
  five standard deviations from both the expected 4 and the control's 1. The million-cycle test is
  `#[ignore = "slow: …"]` (about 55 s under `slow-test`). Goldens `events/bins` (with a thinned
  rate) and `events/phase` are written from unit tests, under `tests/golden/events/`, not
  `stellar/`. Acceptance also needs `cargo test -p hyperion-sim -- events event_tags
domain_tags_are_pinned`, since `events` alone misses the registry tests and the tags golden.
  Bench (x86-64, criterion): ten day-long bins with thinning, 52 µs; one horizon-spanning root
  (P = 3 yr, 16 octaves), 30 µs.
- **Deviations in T7, as built.** `stellar::sse::cheb` is a private module of `pub(crate)` items
  with `ms`'s dead-code expectation, which T10 removes: `CoreHeliumBurning::new(m, &c)` with
  `t_start` (`t_HeI`), `t_end` (`t_HeI` + `t_He`) and `at(t)` → `PhasePoint`, and the landmarks
  `t_hei`, `t_he`, `l_bagb`, `l_min_he`, `l_zahb`, `r_zahb`, `r_mhe_low` (eq. 55 below `M_HeF`) and
  `blue_fraction` (eq. 58 in every regime, T6's `gb::blue_fraction_massive` above `M_FGB`). The
  helium ZAMS pieces are `helium::{zams_luminosity, zams_radius, main_sequence_lifetime}` (eqs.
  77–79). `gb.rs` gains `RadiusLaw` (R_GB and R_AGB with their mass dependence evaluated once, to
  which `radius` and `agb_radius` now delegate, bit for bit), so that the phases keep `at(t)`
  without a `ZCoeffs`. Below `M_HeF`, `m` is the mass after the flash, where HPT section 7.1 reset
  M₀ to Mₜ; the envelope sets the horizontal branch through eq. 52's µ. Two places follow SSE, for
  the owner to confirm: eq. 58's exponent is 0.4805428, not the printed 0.414, which moves `τ_bl` by
  up to 0.035 and R by up to 0.12 dex (L 0.06 dex) at Z = 0.001, 4–5 M☉; and below `M_HeF`, `R_x` =
  `R_ZAHB` is evaluated with the growing core, and ξ and `R_min` with it, where the paper fixes
  `R_x` at the start (the fixed form moves R by up to 0.030 dex at 0.7 M☉, Z = 10⁻⁴, and L by 0.009
  dex). A misprint settled against SSE: b17's exponent is 0.6371760; both printings give 2.862149,
  b′16's second coefficient, which moved `L_min,He` by up to a third at Z = 0.03. That is a one-line
  fix in T4's `coeffs.rs`; b17 has no other caller, so nothing generated moves. Eq. 53's 1.6479 is
  kept as printed (SSE 1.647903, under 10⁻⁵ in `L_ZAHB`); `L_ZAHB`(`M_HeF`) = `L_min,He`(`M_HeF`)
  whatever the core, so SSE's choice of core in eq. 55's constant is immaterial. The SSE rows hold L
  and R to 10⁻⁵, not 10⁻⁹, because T4's printed `M_FGB` constants and the printed 1.6479 move them
  by up to 4 × 10⁻⁶ there (3 × 10⁻³ dex at Z = 10⁻⁴; with SSE's two constants patched in, every
  unperturbed row of the run agreed to 5 × 10⁻¹⁴); massive stars are compared with a run that
  disables SSE's μ < 1 perturbation, which SSE applies to all of them in this phase. Plan figures
  that are wrong for HPT's formulae, SSE agreeing with the formulae: the 5 M☉ loop at Z = 0.02 spans
  4,010–4,665 K and never reaches 5,500–6,500 K (at Z = 0.004 it spans 4,475–7,260 K, and the test
  asserts both); at Z = 0.0005 a horizontal branch with 0.1–0.2 M☉ of envelope sits at 15,400–9,000
  K, and 6,000–7,500 K takes 0.25–0.30 M☉ (6,910 K for the 0.8 M☉ star that lost nothing). `t_He`(1
  M☉) is 131.5 Myr at Z = 0.02. Continuity is tested from T6 at ignition above `M_HeF` (to 10⁻⁹ and
  by age sweeps), in τ, and in mass across `M_HeF` and 12 M☉ and, for Z ≤ 0.002, `M_FGB`; for Z >
  0.002 HPT's declared jump at `M_FGB` is excluded and checked to exist (only 0.007 in `τ_bl` at Z =
  0.004).
- **Deviations in T8, as built.** `stellar::sse::agb`: `EarlyAgb::new(m, &c)` (`t_start` = `t_BAGB`,
  `t_end`, `end` → `EarlyAgbEnd::{ThermalPulses, Supernova}`, `co_core_mass(t)`, `at(t)` with the
  helium core as `core_mass`, and `thermal_pulses()` → `Option<ThermallyPulsingAgb>`, so that no
  mismatched or impossible pulsing phase can be built), `ThermallyPulsingAgb` (`t_start` = `t_DU`,
  `t_end`, `end` → `CoreEnd::{Supernova, WhiteDwarf}`, `time_of_core_mass`, `at`,
  `interpulse_period`), `mc_du` (eq. 69), `mc_sn` (eq. 75), and the constants
  `HELIUM_RATE_MSUN_PER_LSUN_MYR`, `COMBINED_RATE_MSUN_PER_LSUN_MYR` and `CHANDRASEKHAR_MSUN`. The
  ends are at constant mass; the envelope's loss under a wind is T10's root. `sse` re-exports
  `m_c_bagb` (T6's `gb::mc_bagb`) and `interpulse_period` for T18.b and T28.f. `gb.rs` gains
  `GiantBranch::{with_rate, times_from}`, the second for eq. 72, which the 1 M☉ thermally pulsing
  AGB needs because its core starts above `M_x`. `A_He` is SSE's 8.0 × 10⁻⁵, not eq. 68's 7.66 ×
  10⁻⁵, which makes the early AGB 4.4% longer and its luminosity up to 0.11 dex off SSE (2.5 M☉, Z =
  0.02), for the owner to confirm; `A_H,He` is the printed ≈ 1.27 × 10⁻⁵, as SSE has it. `Mc,SN` is
  eq. 75 as printed, not SSE's max(…, 1.05 `Mc,CO`(`t_BAGB`)): where the relation's carbon–oxygen
  core at the base of the AGB already exceeds `Mc,SN` (40–80 M☉) the early AGB ends at once, at most
  0.24% of the lifetime before SSE's; at 60 M☉ and Z = 10⁻⁴ and 10⁻³ SSE's carbon–oxygen core at the
  base of the AGB exceeds its helium core and it runs a thermally pulsing AGB with third dredge-up
  for 0.41 and 0.31 Myr more (8.2% and 6.4% of its lifetime), an artefact not followed, for the
  owner to confirm, since it exceeds T12.b's 1% at those two grid points. HPT give no interpulse
  period; it is Wagenhuber and Groenewegen (1998, A&A 340, 183) eq. 11 with all three terms, so
  `interpulse_period(mc, mc_first, envelope, &c)` → `Years` takes the core at the first pulse and
  the envelope as well as the core, with their α_MLT = 1.5. Plan figure re-checked: `m_c_bagb`
  reaches 1.6 M☉ at 6.31 M☉ and 2.25 M☉ at 8.20 M☉ at Z = 0.02. SSE, at constant mass on 200 ages
  over each star's AGB: L and R to 10⁻¹¹ on 8,690 early and 3,824 thermally pulsing rows where μ ≥ 1
  (all 13,577 and 8,527 with the perturbation off), `t_DU`, `L_DU` and the time to `Mc,SN` to 10⁻¹².
  The helium core's fall at `t_DU` in the second dredge-up is HPT's declared discontinuity; L and R
  are continuous there and at `t_BAGB`.
- **Deviations in T9, as built.** `stellar::sse::helium::HeliumStar`: `new(m)` at zero age (plan
  11's stripped stars, and envelope loss in the gap or on the giant branch), and
  `from_core_helium_burning(&cheb, t)` and `from_early_agb(&early, t)`, which return the star and
  its age (eq. 76; the helium giant's age from eq. 84's relation at the early AGB's carbon–oxygen
  core, no earlier than `t_HeMS`); `t_ms`, `t_end`, `end` (T8's `CoreEnd`), `phase_at(t)` → `Phase`
  (the helium Hertzsprung gap while R₁ < R₂, the giant branch after) and `at(t)`. `gb.rs` gains
  `GiantBranch::helium_giant` (eq. 84's relation). `A_He` is T8's. Below 0.214 M☉, where 1.45 M −
  0.31 is not positive, `Mc,max` is M, as in SSE; SSE's rule that a helium main-sequence star below
  the core at helium ignition of an `M_HeF` star (about 0.33 M☉) is at once a helium white dwarf is
  not in HPT and is left to T10.d. Plan figure: a 4 M☉ helium star's main sequence lasts 1.514 Myr
  ("about 1 Myr"). "Continuous in L to 1% when the envelope reaches zero" holds, to 10⁻⁹, for a star
  whose envelope is gone on the zero-age horizontal branch, and for the helium star against the core
  luminosity that section 6.3's perturbation takes a thinning envelope to. Without that perturbation
  the core-helium-burning luminosity exceeds the helium star of its core by 0.13–1.75 dex (0.49 dex
  at 1 M☉, Z = 0.02, halfway through), which is the gap T10.d must close. SSE: helium main sequence,
  gap and giant branch L and R to 10⁻¹³ on 7,493, 455 and 11 rows where μ ≥ 1 (the perturbation of
  helium giants near their end is T10.d's); the entries from core helium burning to 10⁻⁷ (eq. 44's
  rounded c₁ in the core) and from the early AGB to 10⁻¹² with the perturbation off. The SSE runs
  that T7–T9's tests read are the build of `stellar/sse/mod.rs`, run again on 2026-09-23 on finer
  grids and a second time with the perturbation's branch disabled; each test says which it reads.
- **Deviations in T10.a and T10.b, as built.** `sse::wind` is private; `WindRecipe` is `pub`
  (`Default` is `Modern`) and re-exported as `sse::WindRecipe`. `wind::rate(recipe, &StarState,
&Composition, ReimersEta) -> SolarMassesPerYear` is `pub(crate)`; η is the new `pub(crate)`
  newtype `ReimersEta` (`new`, `value`, `HURLEY` = 0.5), which T10.c builds from
  `StarDraws::eta` by design note 7. Added for T10.d's HPT §6.3 perturbation:
  `wind::small_envelope_mu(m, mc, l)`, equation 97. Both recipes read Z ÷ Z☉ as
  `Composition::z_fit` ÷ 0.02. Post-AGB, pre-main-sequence, substellar and remnant phases have no
  wind. `Hurley2000` is SSE's `mlwind` for a single star with `hewind` = 1, the paper's, and it
  agrees with `mlwind` to 8 × 10⁻¹⁵ over 131,040 states (types 1–9, four Z, every term), zeros
  included. It follows SSE in four places where the printed text moves the rate further than
  T12.b's tolerances (ruling 10), for the owner to confirm. First, Nieuwenhuijzen and de Jager
  ramps on over 4,000–4,500 L☉; the printed switch is up to 0.7 dex high at 4,100 L☉. Second,
  Reimers applies from the Hertzsprung gap; "the GB and beyond" would zero every gap star below
  4,000 L☉. Third, P₀ ≤ 2,000 d rather than log P₀ ≤ 3.3, 0.059 dex where it binds. Fourth, the
  LBV term applies to types 2–6 and not on the main sequence. Both the paper and SSE **add** the
  LBV term to the largest of the other four, so the plan's "the maximum of the applicable terms"
  holds for those four only. SSE's distributed `evolve.in` sets `hewind` = 0.5, so T12.a runs
  with `hewind` = 1.0 or T12.b's helium-star rates differ by 2. `Modern` follows Belczynski et
  al. (2010, §2.2). From 12,500 K a hydrogen-rich star loses mass at Vink's rate **alone**, in
  place of every HPT term, small-envelope term included: design note 6's "Hurley's own choices
  elsewhere", and Belczynski's "for H-rich low mass stars, for which the above prescriptions do
  not apply". Vink's equations 24 and 25 were checked digit by digit against the paper,
  Belczynski's equations 6–7 and MESA's `winds.f90`. They are applied beyond their calibrated
  grid (log L 5.0–6.0, 20–60 M☉, Z ÷ Z☉ 1/30–3), as Belczynski applies them from about 3 M☉.
  Across 22,500–27,500 K, Ṁ is (1 − w) cool + w hot with w linear in T, both fits taken at the
  star's state; Vink's own jump position (about 25.9 kK at solar Z, eqs 14–15) and the second
  jump near 15 kK are not modelled. From 11,500 to 12,500 K HPT's rate is handed over to Vink's
  the same way, in a band we chose after MESA's "Dutch" 10,000–11,000 K. T is held at 50,000 K
  above the fits, and v∞ ÷ v_esc at 2.6 and 1.3, with no Z^0.13 correction, so Ṁ ∝ Z^0.85 as
  design note 6 says. Evolved hydrogen-rich stars beyond the Humphreys–Davidson limit lose
  1.5 × 10⁻⁴ M☉ yr⁻¹ in place of every other term. That step, about 6 times the rate just inside
  the limit for a 60 M☉ star of 10⁶ L☉, is the recipe's one discontinuity, kept as design note 6
  specifies; the continuity test covers the five temperature boundaries (11,500, 12,500, 22,500,
  27,500 and 50,000 K) inside the limit. Helium stars lose max(Reimers, 10⁻¹³ L^1.5 (Z ÷
  Z☉)^0.86). A giant cooler than 11,500 K that is stripped to a helium star therefore sees its
  rate fall about 95 times at Z = 10⁻⁴ and 7 at 0.002, as in the codes followed. The plan's O star
  (40 M☉, 40 kK, 10⁵·⁷ L☉) loses 3.43 × 10⁻⁶ M☉ yr⁻¹.
- **Deviations in T11, as built.** `RemnantRecipe` (`pub`, `Default` `MandelMuller2020`) lives
  in `stellar::remnant` and is re-exported as `sse::RemnantRecipe`. `RemnantKind` and
  `CompactRemnant` (`pub`, getters `kind` and `mass`, `pub(crate) new` debug-asserting the mass)
  exist now, in Provides' shape, because HPT's remnant mass returns one; T18.d reuses them.
  `remnant::structure` is `pub(crate)` and holds `CHANDRASEKHAR_MASS` (1.44),
  `OXYGEN_NEON_MC_BAGB` (1.6) and `HURLEY_MAX_NEUTRON_STAR_MASS` (1.8). The white dwarf's kind
  is `white_dwarf_kind(DegenerateCore)`: `Helium` for a degenerate helium core, and
  `CarbonOxygen { mc_bagb }` below 1.6 M☉ or oxygen–neon from it. So carbon–oxygen against
  oxygen–neon is decided by the core mass at the base of the AGB (HPT §5.4, SSE), not at the
  envelope loss; for a helium star the caller passes its initial mass, HPT §6.1. The radii are
  `white_dwarf_radius(recipe, m)` (equation 91, floored at the recipe's neutron-star radius),
  `neutron_star_radius(recipe)` and `black_hole_radius(recipe, m)`: 4.24 × 10⁻⁶ M under
  `Hurley2000`, otherwise 2GM ÷ c² from the nominal constants, 0.12% larger. HPT's "10 km" is
  kept as their 1.4 × 10⁻⁵ R☉, 9.74 km in the nominal R☉. `hurley_supernova_remnant(mc_sn)` is
  equation 92, a neutron star up to 1.8 M☉. **The neutron-star radius is 12.2 km, not 11.5.**
  Koehn et al. (2025, Phys. Rev. X 15, 021014) combine nuclear theory and experiment with the
  NICER radii of PSR J0030+0451 (Riley et al. 2019, Miller et al. 2019) and PSR J0740+6620
  (Salmi et al. 2024, Dittmann et al. 2024), GW170817 and GW190425, and give R₁.₄ = 12.20 (+0.50
  −0.48) km at 95%, which excludes 11.5. The other combined analyses of the NICER data centre at
  12.0–12.45 km (Miller et al. 2021, Raaijmakers et al. 2021, and Rutherford et al. 2024 with PSR
  J0437−4715), and those of 2025–2026 that add PSR J0614−3329 at 11.8–11.9 km. Only
  gravitational waves with nuclear theory alone reach 11.0 km (Capano et al. 2020). T21.b's
  spin-down constant, written with 11.5 km, should read `neutron_star_radius`: k ∝ R⁶ is 1.43
  times larger. Equation 91's relation vanishes at M_Ch as the plan asks. The radius itself is
  floored at the neutron star's, as HPT print it, which binds within 3 × 10⁻⁶ M☉ of M_Ch. SSE's
  two guards below 0.002 M☉ are left out. SSE's distributed `evolve.in` has `nsflag` = 1
  (Belczynski et al. 2002 masses) and `mxns` = 3, so T12.a must run with `nsflag` = 0 and `mxns`
  = 1.8 to compare against `RemnantRecipe::Hurley2000`. Against SSE's `hrdiag` (run of
  2026-09-23), white-dwarf radii at ten masses, neutron-star and black-hole radii, and the
  supernova remnants of 8–80 M☉ stars at three Z all agree to 10⁻⁹.
- **Validation of T1.a, T1.b, T2, T4–T6 and T27 (val06, round 6).**
  - _Coefficients._ Every row of HPT's Appendix (journal pages 566–569, ADS scan) and of Tout et
    al.'s Tables 1 and 2 (page 258) was read against the page images, and separately against
    SSE's `zdata.h` through `zcnsts.f`'s mapping: all 143 table rows agree (SSE stores the rows of
    b21 and b22 in the other order). The closed forms do not all agree. b17's exponent is printed
    2.862149 in the journal and the preprint, which is b′16's β one row above; SSE has 0.6371760,
    a number the paper never prints. The printed form moves `L_min,He` (equation 51) by +0.08 dex
    at Z = 0.004 and −0.17 dex at 0.03, so b17 now takes SSE's exponent as a misprint settled,
    with a test against SSE's `lHef` (the same fix, to the same value, as `starA`'s); nothing reads
    b17 before T7. For T7: equation 58's (M ÷ `M_FGB`)^0.414 is 0.4805428 in SSE's `tblf` (the
    printed exponent raises `τ_bl` by up to 0.035), and equation 53's 1.6479 is 1.647903 there.
  - _Agreement with SSE, reproduced_ over all of `probe_hrd.csv` with a harness since removed:
    main sequence (52,029 rows) L 1.0 × 10⁻¹⁴ and R 1.3 × 10⁻¹⁴ relative; gap (770 rows) L
    2.0 × 10⁻¹⁴, Mc 4.3 × 10⁻⁸ (equation 44's c₁), and R 1.3 × 10⁻³ dex at Z = 10⁻⁴, 5 M☉, above
    `M_FGB`, where equation 50's µ reads the rounded `M_FGB` (7 × 10⁻¹⁴ with SSE's); branch (1,795
    rows) L 4.2 × 10⁻¹³, R 2.6 × 10⁻¹³, Mc 4.3 × 10⁻⁸. The recorded 10⁻¹³ for the gap holds only
    below `M_FGB`. Eighteen rows (15 gap, 3 branch) carry SSE's µ < 1 perturbation, T10.d's, and
    differ by up to 0.018 dex in R. Added to the tests: the nine unperturbed gap rows above `M_FGB`
    (all at Z = 10⁻⁴), four main-sequence rows in η's Z ≤ 0.0009 branch, and 24 points of SSE's
    landmark functions (`L_HeI`, `R_GB`, `R_AGB`, `R_mHe`, `τ_bl`, `Mc,BAGB`, `Mc,BGB`, `Mc,HeI`,
    `R_HeI`), which the gap's end had been checked against only through the same functions.
  - _The five choices for the owner_, as the worst deviation from SSE over the grid's unperturbed
    rows; all five meet the 0.02 dex rule, and the recommendation is to confirm them as built.
    - Equation 6's printed x: `t_MS` −0.83% (Z = 0.004), main-sequence R +0.030 dex and L
      +0.016 dex within 13.8 Gyr (L +0.055 dex and gap L −0.18 dex beyond it), 207 rows past
      0.02 dex. SSE's form stands.
    - p, q and log D from `M_HeF`: branch L −0.038 dex (Z = 0.001, 2 M☉), R −0.023 dex, Mc
      −0.95%, `t_HeI` +0.055%. SSE's 2.0 M☉ stands.
    - Ignition at `R_AGB` without a blue phase: the printed `R_HeI` differs by −0.70 to +2.31 dex
      at 15 of the 155 points (M ≥ 40 at Z = 0.02, M ≥ 15 at 0.03, 60–100 at 0.004, 100 at 0.001);
      every gap row it touches is perturbed, where it is off by 1.14 dex. SSE's form stands.
    - Equation 44's printed c₁: Mc 4.3 × 10⁻⁸, L and R unchanged. The printed form stands.
    - `M_FGB`'s rounded constants: gap R 1.3 × 10⁻³ dex, `L_min,He` 8 × 10⁻⁵ dex, `τ_bl`
      1.4 × 10⁻³, and, measured by `starA` together with equation 53's printed 1.6479, core helium
      burning 1.1 × 10⁻³ dex in L and 3.1 × 10⁻³ in R (ruling 29). The printed form stands.
  - _T6.c's figures._ The plan's "near 2,500 L☉" and "falls" should read: a 1 M☉ star at
    Z = 0.02 reaches the tip at 2,700–3,000 L☉ (log L = 3.45 ± 0.05; BaSTI, Pietrinferni et al.
    2004, ApJ 612, 168, Table 3: 2,985 L☉ with a 0.478 M☉ core; HPT's equation 49 gives 2,752 L☉
    and 0.477 M☉), and the bolometric tip brightens with metallicity (Salaris and Cassisi 1997,
    MNRAS 289, 406, Table 1: 1,977 L☉ at Z = 10⁻⁴ to 2,742 at 0.006, against HPT's 1,933 to 2,814
    at 0.03); only the I-band tip fades. Cassisi and Salaris 1997 (MNRAS 285, 593) gives no tip
    luminosity: the paper meant is Salaris and Cassisi 1997.
  - _The normal quantile_ against roots found to 45 digits at 49,187 points (each branch point ±6
    ulps, the tails, 2⁻¹⁰⁷⁴–2⁻¹⁰²², random): 3.4 × 10⁻¹⁶ relative (2.45 ulps) for normal p, and
    3.9 × 10⁻⁹ for subnormal p, at 6.5 × 10⁻³¹⁹ (the doc said 3.5 × 10⁻⁹; corrected). The mirror
    is exact bit for bit at all 37,058 points where 1 − (1 − p) = p, except p = ½, +0 against −0,
    which no odd function avoids (the doc is corrected).
  - _T2._ Tags, scopes and word budgets match "Generator version" and the plan's text (at most 16
    of 64 words; eight tries all fail at 8.9 × 10⁻¹⁶ and 2.5 × 10⁻¹³). The median mark's boundary
    is exact: ceil(p × 2⁵³) > 2⁵² exactly when p > ½, and ½ + 2⁻⁵³ already accepts. The test
    checked 0.500001 and one field, so a median mark one too high passed it; it now checks every
    field of the median and the next double above ½.
  - _T27._ New tests: the partition rule over every one-second piece of ±1,000 s at 16 s bins and
    periods, with cuts a nanosecond either side of an event, across `ClockWindow::START` and `END`
    and `SourceHorizon::START`, and across the last 40-bit bin and cycle numbers; and
    `LinearClock`'s fraction against the exact ratio, to 2⁻⁵⁰ across the source horizon. All pass.
    A rate above its bound fails only in debug builds, as the plan asks. In release it is clipped
    to the bound, a mean above 64 passes, a count above 255 is clamped (events are lost without a
    word), and a NaN rate gives no events; a NaN, negative or overflowing bound panics in release
    too.
  - _Tests that could not fail_ (97 deliberate breaks, 38 survived). The jump detector's 0.05 dex
    gate hid every jump below about 0.03 dex, and its bisection lost a jump that ran against the
    slope. It now flags any interval that departs from the cubic through its neighbours, cuts each
    into 32 pieces so that curvature cannot outweigh a jump, bisects every piece past the tolerance
    towards its outlying quarter, and tells a cusp from a jump by whether the change halves over 2²⁰
    of width. It finds 10⁻⁴ dex anywhere in a sweep, which the continuity tests alone now show for
    every jump the breaks planted; the main-sequence sweep takes about 10 s in debug under load. The
    diffusion test was one-sided, so octave amplitudes of 2ʲ passed; it now bounds each fourfold
    lag's growth by 8. Coefficient digits below the SSE tests' 10⁻⁹, b46's finishing step and b17
    were pinned by the checksum alone. The new golden `stellar/sse` pins every aₙ, bₙ and critical
    mass and points of all three phases at the five metallicities, bit for bit; it passes on
    wasm32-wasip1. Left as equivalent: γ's a75 + 0.1 bound (its clamp makes the forms identical),
    `events_in`'s −1 ns on the window's end, the phase's one-cycle margins, the quantile's erf/erfc
    switch anywhere in 0.1–0.4, and √(2π) one ulp off.
  - _Determinism._ No platform transcendental bypasses `math`, nothing `usize`-dependent reaches
    output, and the `Lattice` cache lives for one call. Latent: a release build lets a NaN into a
    `StarState`, whose range checks are debug assertions.
- **P06.T35.b's outline, as built alone (round 7, `ui`).** Only the `ringed-circle` value of `SymbolShape` and its outline are built, for plan 14's `SYSTEM` display; `starSymbols.ts`, the `STARS` filter and the legend wait for the stellar wire types. `SymbolOutline` gains a third kind, `{ kind: "ringed-circle", discRadius }`, with `discRadius` `RINGED_DISC_SHARE` = 1/3 of the ring's radius; the painter fills the disc alone, then adds the ring to the path and strokes both once, so open and filled differ only by the fill (tested in the draw list and on a recorded canvas). **Deviation, for the orchestrator to rule:** the task asks that it read open against filled at the smallest `SIZE_CLASS_REM`, which no ringed circle can: at size class 0 (8 px at 100%) the ring's and the disc's 1.5 px outlines take 6 px, leaving 2 px for the gap round the disc and the open disc's hole, which cannot both be a pixel wide. The third splits it evenly, 2 px each at size class 2 and 1.2 px at 80%, and the test asserts both at least the outline's width at class 2 — the smallest class a giant is expected to take, since D17 ties size to the initial-mass layer and giants should come from layer C (0.75–2.5 M☉) and above in a galaxy of the Milky Way's age; `starSymbols.ts` should confirm that when it lands, or plan 06 should give giants a size class floor.
- **Deviations in T15.c, as built (`planet`, round 7; rulings 33 and 38).** Only the disc-lifetime
  law is built, because plan 14's disc is its first caller: `stellar::premain::disc_lifetime` of a
  mass and a rank, in `Megayears`, with `disc_lifetime_mean(mass)` and the constants
  `DISC_LIFETIME_MEAN_SOLAR`, `DISC_LIFETIME_LOW_MASS_EXPONENT`, `DISC_LIFETIME_HIGH_MASS_EXPONENT`,
  `DISC_LIFETIME_MIN` and `DISC_LIFETIME_MAX`; its values are pinned by the golden
  `stellar/disc_lifetime`. It draws nothing: the rank is the star's `StarDraws::disc_lifetime()`.
  The lifetime is −τ ln(1 − u), held to 0.3–15 Myr, with τ = 2.5 Myr at 1 M☉; the median rank gives
  1.733 Myr there, Mamajek's half-life of 1.7. The source, re-checked: Mamajek (2009, AIP Conf.
  Proc. 1158, 3, Fig. 1 and eq. 1), an e-folding time of 2.5 Myr for the disc fraction of 22
  clusters; Ribas et al.'s (2015, Table A.2) all-star fit of 2.7 ± 0.7 Myr for inner-disc excesses
  agrees. The mass scaling was first built as the plan's m^−½, which gave 1.6–2.2 Myr at 1.3–2.5 M☉
  against Mamajek's 1.2 and 11 Myr for brown dwarfs against about 3. Ruled (ruling 38, point 6): it
  follows measurements. Below 1 M☉, τ ∝ m^−0.1: Luhman et al. (2005, ApJ 631, L69) find brown
  dwarfs' disc fractions of 42% and 50% in IC 348 and Chamaeleon I against 33% and 45% for their
  M0–M6 stars, lifetimes 1.28 and 1.15 times as long across a factor of six in mass; the law gives
  3.4 Myr at 0.05 M☉. Above 1 M☉, τ ∝ m^−1.06, halving by 2 M☉ (1.20 Myr there): Ribas et al. (2015,
  Table 3) measure lifetimes 2.09 and 2.2 times longer below 2 M☉ than above at 1–3 and 3–11 Myr,
  and Mamajek 1.2 Myr above 1.3 M☉. The mean reaches the 0.3 Myr floor near 7 M☉. P06.T15.c's text
  now says so. T24's T Tauri class must read this function. T15.a and T15.b are not built, and
  `Track` is untouched.
- **Deviations in T3, as built.** `stellar::system::draw_metallicity(&Galaxy, &SystemRecord) ->
Composition` as specified: \[Fe/H\] = `mean + sigma × z` through `Stream::normal` (two words of
  `system.metallicity`, scope `System`, keyed by `ObjectKey::from(SystemId)`), `Composition::from_fe_h`
  with no helium excess, the field read at `max(age, 0)`; a record without a component fails a
  `debug_assert!` and reads its population's first component. The tag sits under a new "Plan 06,
  the system's own draws" heading after the `star.*` tags, so `tags.golden` gains one line in the
  middle, not at the end. Tests (`tests/stellar_metallicity.rs`, units in `system.rs`): the
  gradient over 2.7 × 10⁵ thin-disc records of a strip of layer-E cells along +y (12,800–44,800
  ly), slow by plan 01's convention although it runs in about a second, is the fixture's −0.05
  dex/kpc within 3.29 standard errors of the fitted slope (measured −0.04979 ± 0.00020), and the
  scatter the field's 0.20 within 3.29 of its own standard error (0.2001). The field states no
  tolerance, so the bracket is its scatter's sampling error at α = 10⁻³. Records older than
  `THIN_DISC_FLAT_AGE` are left out, so the test holds whichever disc carries the decline beyond
  8 Gyr. The halo test builds records from parts in the fixture's in-situ and dominant-merger
  components, 10⁴ each, and finds means −0.5988 and −1.1991, a separation of 0.6004 against 0.6
  (± 0.0042), and a two-sample K–S p below 10⁻³⁰⁰. The K–S test at one place (old thin disc,
  9,000/21,000/−120 ly, 3.2 Gyr) has p = 0.72. The golden `stellar/system_metallicity` pins 25
  records: the first three of seven cells across the layers, three young-disc and three halo
  records found by placement, and one young-disc record 500 years unborn built from parts, since
  the unborn sliver (H = 1,000 years) is too rare to find. **For the orchestrator:** ruling 7 is
  not in the field as built. `galaxy/fields/metallicity.rs` still gives the thin discs the
  decline of 0.1 dex per Gyr beyond 8 Gyr, and the thick disc a fixed −0.55. T3 reads whatever the
  field says, so moving the decline is plan 02's change, with a bump.
- **Deviations in T13, as built.** `stellar::substellar::cooling(mass: SolarMasses, age: Years,
comp: &Composition) -> Result<StarState, EvaluateCoolingError>`. It covers
  `substellar::{MIN_MASS, MAX_MASS}` = 0.01–0.1 M☉ inclusive. `MassOutsideFits(mass)` is returned
  outside that range or for NaN, and `AgeOutsideLife(age)` for a negative or non-finite age. Below
  `MIN_AGE` (1 Myr) it gives the 1 Myr state with the true age. Plan 13's sketch has a bare
  `StarState`, so its calls take `?` or an `expect`. Added: `hydrogen_burning_limit(&Composition) ->
SolarMasses`, which plan 13 and T29 need to tell `ObjectKind::Dwarf` from `Substellar`. The
  returned phase is `Phase::Substellar` at every age and on both sides of the limit, since the
  phase's own doc already covers "the latest M dwarfs" and no such object leaves it. Mass is
  constant, the core is zero, the mass-loss rate is zero and the phase fraction is zero. The
  seam for `starA`'s `evolve` is `m0 < 0.1 → cooling(m0, age, comp)`, which cannot fail for grid
  primaries (0.08 M☉ and up). Five departures from the plan's sketch follow, **for the
  orchestrator to rule**.
  - _A contraction regime._ Burrows et al.'s power laws are late-time fits ("characterize older
    SMOs", §II), and alone they miss BHAC15 at 0.1 Gyr: 3,238 against 2,525 K at 0.05 M☉, and
    −2.42 against −2.68 dex at 0.09 M☉. Before degeneracy the object contracts on its Hayashi
    track by the n = 3/2 polytrope's Kelvin–Helmholtz law, R³ = t_KH m² (T☉/T_H)⁴ ÷ 7t, whose
    one parameter T_H = 3,020 K (m ÷ 0.1)^0.09 is fitted to BHAC15. It joins the Burrows branch
    by p-norms of order 4, the smaller L and the larger R. The Burrows radius is their
    equation 5, at equation 3's gravity and equation 2's temperature.
  - _Radius and temperature are joined, not luminosity and radius._ With L and R joined, a star
    settling on the floor rose by up to 200 K in temperature at Z = 10⁻⁴. With R and T each a
    p-norm of order 20 and L = R²T⁴, T, R and L never rise with age, and T never falls with
    mass, by construction. Plan 13's P13.T5.a asks for exactly that.
  - _The floor._ R_hb = R₀.₁ (m ÷ 0.1)^1.2 x^0.05 and T_hb = T₀.₁ x^0.18, with x = (m − m_e) ÷
    (0.1 − m_e), fitted to BHAC15's 10 Gyr isochrone. It is pinned at 0.1 M☉ to Tout's ZAMS
    luminosity and to the radius the backbone starts from, HPT's equation 24 floor (0.1346 R☉ at
    Z = 0.02; `zams::radius` alone gives 0.1305). That floor is computed again in
    `backbone_at_tenth`, in `sse::ms`'s arithmetic, because `MainSequence` is private to `sse`.
    At merge, add a test of `cooling(0.1)` against `evolve(0.1)` at 1 and 10 Gyr.
  - _Metallicity._ Burrows's κ̂ is read as Z ÷ Z☉ (on `z_fit`) above a metal-free floor
    4^(−1/0.35) = 0.0190, the quarter of solar luminosity their text gives at zero metallicity.
    The hydrogen-burning limit follows their equation 7, m_e ∝ κ̂^(−1/9), with its own floor
    (0.068 ÷ 0.083)⁹. It is calibrated at 0.068 M☉ solar, where the floor's weight must vanish
    for BHAC15's 0.07 M☉ model to keep burning, and at 0.083 M☉ metal-poor. Baraffe et al.'s
    (1997, A&A 327, 1054) Tables II–V start at 0.083 M☉ at every \[M/H\] from −2.0 to −1.0, and
    call that the limit. At −2.0 their model is at the edge (1,779 K, log L −4.27), but at −1.0 it
    still has 2,359 K, so the limit there lies lower. The fit gives 0.065 M☉ at Z = 0.03, 0.079 at
    \[M/H\] = −1 (their 0.083 M☉ model's luminosity within 0.12 dex, relative to 0.1 M☉), 0.0826
    at −2.0 and 0.083 at Z = 10⁻⁴.
  - _The 10 Gyr point at 0.05 M☉._ BHAC15's grid stops at 1,300 K, so it comes from ATMO 2020
    (Phillips et al. 2020, A&A 637, A38), the same group's cold extension: 782 K and −5.670. The
    fit gives 759 K and −5.698.
- **Measured for T13**, against BHAC15's isochrones (`BHAC15_iso.2mass`). ΔT and Δlog L at
  0.05 M☉ are +10 K and 0.000 at 0.1 Gyr, and +98 K and +0.125 at 1 Gyr (from the track). The
  0.125 is Burrows's equation 1 itself, which ATMO 2020 matches to 0.02 dex. At 0.075 M☉ they are
  +8, −120 and +69 K, and −0.002, −0.093 and +0.034. At 0.08 M☉, +4, −74 and −25 K, and −0.004,
  −0.060 and −0.020. At 0.09 M☉, +6, −57 and −58 K, and +0.003, −0.008 and −0.012. The worst are
  120 K and 0.125 dex, against the plan's 150 K and 0.15. Over BHAC15's grid from 30 Myr to
  10 Gyr and 0.03–0.1 M☉ the fit is within 190 K and 0.16 dex. At 0.1 M☉ it meets the backbone
  within 0.53% in L and 0.02% in R from 1 Gyr on, at all five metallicities (the plan's 2%; the
  test holds 1% and 0.03%). A
  0.05 M☉ object is 2,535 K at 0.1 Gyr (M), 1,481 K at 1 Gyr (L) and 931 K at 5 Gyr (T), on
  Pecaut and Mamajek's scale (2022.04.16: M/L at 2,310 K, L/T at 1,310 K). Continuity in age is
  a log–log Lipschitz bound (|Δln L| ≤ 2|Δln t|, |Δln R| ≤ |Δln t| ÷ 3). The golden
  `stellar/substellar_cooling` pins 252 states.
  - _Plan figures that were wrong._ The limit's range is wider than 0.072–0.078 M☉ at its
    metal-poor end. At solar metallicity Burrows et al. give 0.07–0.075 M☉ and BHAC15 0.07, but
    the limit is 0.083 M☉ at \[M/H\] = −2.0 (Baraffe et al. 1997) and 0.092 M☉ at zero
    metallicity (Burrows et al., after Saumon et al. 1994). A 0.05 M☉ object is not "2,800 K at
    0.1 Gyr": BHAC15 give 2,525 K and ATMO 2020 2,548 K, and 2,800 K is its temperature at
    10–25 Myr. Deuterium burns above 13 Jupiter masses, not below (Burrows et al. §II), and the
    fit ignores it at every mass.
  - _Known limits._ Near 13 Jupiter masses the fit is up to 0.6 dex faint at 30–100 Myr against
    ATMO 2020, where deuterium burns (P13.T5.a may add it). At 0.1 M☉ it is 0.42 dex brighter
    than the backbone at 0.1 Gyr, as BHAC15's star is, until P06.T15.b's pre-main sequence, whose
    Hayashi segment should use this contraction law. Young objects near 0.075 M☉ are 2,900–
    3,000 K below 10 Myr (BHAC15 2,967 K, ATMO 2020 3,050 K), which P13.T5.a's "none earlier than
    M6" (2,810 K) must allow. The metal-poor floors follow Baraffe et al. (1997) in luminosity to
    0.2 dex, but are 230–440 K cooler. The reason is the backbone's: HPT's equation 24 gives
    0.143 R☉ at 0.1 M☉ and Z = 10⁻⁴, against their 0.108. That is a finding against the
    backbone's floor at low Z, for T12.
- **Deviations in T18.a–c, as built (round 7, `remnant`).** Built as functions of plain arguments in
  `stellar::remnant::collapse` (`pub mod`), for T18.d to wire into `Track::death`; nothing generated
  calls them yet, so no golden moved, and the new golden `stellar/collapse` pins them by bits at 11.
  - _API._ `RemnantDraws::{of(&StarDraws), from_parts(type, fallback, mass)}` holds the three
    reserved draws. `core_collapse(co_core, helium_core, RemnantDraws) -> CoreCollapse` returns
    `NeutronStar { mass }`, `BlackHole { mass }` (partial fallback), `DirectCollapse { mass }`
    (complete fallback, the helium core), `PulsationalPairInstability` (40.5 M☉) or
    `PairInstabilitySupernova`, and `CoreCollapse::remnant()` gives the `CompactRemnant`.
    `pair_instability(helium_core) -> PairInstability::{None, Pulsational, Disruptive, Collapse}`,
    which `core_collapse` applies first. For electron capture there is
    `ElectronCaptureWindows::new(&ZCoeffs)`, with `iron_core_mass()` (m_cc), `window(width)`,
    `single()` and `companion_stripped()`, each an `InitialMassWindow` with `lower`, `upper` and
    `contains`, and `electron_capture_remnant()`. The Table 1 figures, the 1.26 M☉ mass, the 2.25 M☉
    core at the base of the AGB, the window widths and Belczynski's four figures are `pub const`s.
  - _T18.d's call._ In the EAGB and helium-star ends:
    `core_collapse(p.co_core_mass(), p.helium_core_mass(), RemnantDraws::of(draws))`.
    `DirectCollapse` and `PulsationalPairInstability` map to `DeathKind::DirectCollapse`, and
    `PairInstabilitySupernova` to `DeathKind::PairInstability`. At the thermally pulsing AGB's end:
    `windows.single()` (or `companion_stripped()`) `.contains(m)`, with the windows built once per
    track beside `lightest_helium_star`.
  - _Figures re-checked_ against the arXiv source of MM20 (2006.08360). M₁–M₄ are 2, 3, 7 and 8. The
    probabilities are (M_CO − M₁) ÷ (M₃ − M₁) and (M_CO − M₁) ÷ (M₄ − M₁). The mass laws are
    1.2 ± 0.02, 1.4 + 0.5 (…) ± 0.05, 1.4 + 0.4 (…) ± 0.05 and 0.8 M_CO ± 0.5, and the hold is
    1.13–2.0 M☉. All are as the plan has them, but the recipe is MM20's **section 3**, not 2.
    Electron capture's 1.26 M☉ is also section 3. Belczynski et al. (2016, A&A 594, A97,
    arXiv:1607.03116, section 3 and eq. 1, model M10) give 45–65 M☉ → 45 (1 − 0.1) = 40.5 M☉ and
    65–135 M☉ → nothing, and section 2 gives collapse from 135 M☉.
  - _Redraws._ MM20 redraw out-of-range masses. The one normal is mapped through the quantile of
    the truncated normal instead: the same distribution, one draw, monotone in the draw (tested
    against a rejection sample). A black hole of partial fallback is also held below its helium
    core, the mass of complete fallback. MM20 print no such bound, but COMPAS, where they
    implemented the recipe, applies it (`GiantBranch::CalculateFallbackBHMassMullerMandel`, `dev`
    branch). MM20's models span M_CO ≈ 1.4–9 M☉ (their Fig. 1); complete fallback above that is
    their rule carried on.
  - _The envelope and the total mass do not enter._ MM20 take any hydrogen envelope to be unbound,
    so T18.d need pass neither.
  - _m_cc(Z)_ is bisected on `m_c_bagb` over 0.1–100 M☉ to adjacent doubles. It is the lowest
    double reaching 2.25 M☉, the early AGB's own test, and matches eq. 66 inverted by hand to
    10⁻¹⁴. It is 8.203 M☉ at Z = 0.02, 8.32 at 0.03, 6.83 at 10⁻⁴, and lowest, 6.72, near
    3 × 10⁻⁴. Both windows lie in the oxygen–neon band (a core of at least 1.88 M☉ at the base of
    the AGB) at every Z.
  - **For the orchestrator to rule: the window's mass.** m_cc is found at constant mass. On
    `starA`'s tracks main-sequence winds lower the mass `m_c_bagb` reads (HPT section 7.1), so iron
    cores begin at m0 = 8.305 M☉ at Z = 0.02 (6.836 at [Fe/H] = −2.3). An initial-mass window
    [8.103, 8.203) then leaves a 0.1 M☉ gap of stars that neither capture electrons nor make iron
    cores. The track runs them through HPT's own electron capture, 2.2% of an 8–150 M☉ sample.
    Recommended: T18.d tests the window against the mass the track's `m_c_bagb` reads, so that the
    window meets the iron cores.
  - **For the orchestrator to rule: the stripped window and design note 11.** Note 11 draws the
    companion-stripped mark only for stars of 8 M☉ or more, but the stripped window [m_cc − 1,
    m_cc) lies below 8.2 M☉ at every Z, so it would be nearly empty. Recommended: T19.c draws the
    mark from m_cc − 1.0 M☉, where the widest window begins.
  - _Also for the orchestrator._ MM20 use 1.38 M☉ in place of M_Ch in eq. 75, which the track does
    not; that moves only M_CO below M₁, where the neutron star is 1.2 M☉ either way. The 40.5 M☉
    black hole carries Belczynski's 10% neutrino loss while MM20 neglect it, so the black hole of
    a 44.9 M☉ helium core is 44.9 M☉ and that of 45.0 is 40.5, a step design note 10 accepts.
    T19's `ec_window_single` and `ec_window_stripped` must read `SINGLE_STAR_WINDOW` and
    `COMPANION_STRIPPED_WINDOW`, so that there is one copy.
  - _Population tests left to T18.d._ The 38 ± 5% black holes, 70–80% complete fallback and 2–6%
    electron captures need real cores. The one stand-in in the tree, HPT's constant-mass cores
    (eqs 66 and 75), passes them (36.9%, 75.0%, 2.6%), but for its own reasons. It puts solar
    stars above about 80 M☉ into pair instability and overstates complete fallback. A forecast on
    `starA`'s tracks (a scratch run, not committed; 3 × 20,000 Kroupa stars at Z = 0.02, η and
    the remnant draws drawn, tracks capped at 100 M☉) gives 36.4–37.2%, **70.3–70.9%** and
    2.4–2.6%. So T18.d's complete-fallback test sits one sampling σ above its floor: stars of
    23–56 M☉ end on a Wolf–Rayet plateau of M_CO 6.0–8.1 M☉, short of M₄.
  - _Tests._ The type is monotone in the draw and in M_CO. The shares follow both linear
    probabilities (χ², α = 10⁻³). Neutron stars lie in 1.13–2.0 M☉ at |z| up to 40, and follow
    the three branches' mean and σ. Black holes of 2–5 M☉ exist, and partial fallback lies between
    2.0 M☉ and the helium core. The pair-instability ranges are closed below. m_cc and the windows
    are checked at 41 metallicities.
  - _Outside `collapse.rs`._ The `pub mod collapse;` line is appended to `remnant/mod.rs`. Three
    `expect` attributes that the new public code leaves unfulfilled are removed: on
    `CompactRemnant::new`, on `ZCoeffs::b` and on the `m_c_bagb` re-export. `starA`'s tree already
    removes the first two.
- **Deviations in T10.c–e, as built (round 7, `starA`).**
  - _API._ `stellar::sse::{Track, TrackOptions, Bridges, MIN_INITIAL_MASS, MAX_INITIAL_MASS,
evolve, lifetime, turn_off_mass}`, with `evolve` and `lifetime` re-exported as `stellar::{evolve,
lifetime}`. `Track::{to_age, full, to_age_with, full_with, state_at, lifetime, death, remnant,
max_radius_until, max_luminosity_until, built_until, initial_mass, composition, options}`. The
    `_with` forms take `TrackOptions` (wind recipe, remnant recipe and `Bridges::{Physical,
Instant}`); `TrackOptions::hurley2000()` is T12.b's (both HPT recipes, no bridges).
    `pub(crate)`: `Track::pulse_phase_at` (T8.b's cumulative pulses, for T24.b and T28.f, with
    their dead-code expectation) and `track::lifetime_of`. `window_where` is T24's and not built.
    `stellar::remnant` gains `Death`, `DeathKind` (with `ThermonuclearDisruption`, carbon ignition
    in a degenerate core that leaves nothing), `ProgenitorAtDeath`, `Stripping` and
    `SupernovaType`, and the `pub(crate)` modules `white_dwarf` (HPT eq. 90) and `neutron_star`
    (eq. 93), which both recipes use until T20 and T21 (ruling 33). `stellar::sse::envelope` holds
    HPT §6.3's perturbation (eqs 97–105), which ruling 29 makes a requirement.
  - _The knot coordinate_ (ruling 40). The main sequence and the helium main sequence keep design
    note 1's τ grid (16 knots). From the Hertzsprung gap on, knots sit at fixed values of u = 1 − (1
    − x)(1 − y): x is the phase's progress, in time for the gap, core helium burning and the
    thermally pulsing AGB and in core mass on the giant branches, and y is the share of the entry
    envelope lost. The 16 knots (32 on the pulsing AGB) are clustered as 1 − (1 − k ÷ (n − 1))³,
    and each interval is integrated in u from the state reached, so the coordinate is still fixed
    before any query. Fixed fractions of the phase's time stall on massive stars'
    luminous-blue-variable bursts, which strip an envelope in a sliver of the phase, and at the
    giant-branch tip. Offsets in log L and log R decay over the first 2% of a phase where HPT's
    formulae step: the core's appearance at a massive star's gap, the second dredge-up, and an
    early-AGB star whose remnant is still passing to the helium giants.
  - _The initial mass in the gap is frozen_ at the main sequence's end (ruling 40). HPT ask that
    it follow the current mass; the effect on any age is under 10⁻⁴.
  - _SSE's forms, by ruling 40._ `R_mHe` at the initial mass above `M_FGB` (`gb::IgnitionRadius`;
    the current-mass form keeps 25 M☉ of a 60 M☉ star at Z = 10⁻⁴). The supernova core is held to
    at least 1.05 × `Mc,CO`(`t_BAGB`), while printed eq. 75 still decides supernova against
    pulses (ruling 29 amended). A helium star's carbon–oxygen white dwarf has the star's whole
    mass, because HPT say only that the star "becomes a CO WD". Below 0.689 M☉ that leaves the
    unburnt helium on the dwarf, and the luminosity steps at the hand-over by log₁₀(M ÷ (1.45 M −
    0.31)), as in SSE: 0.26–0.34 dex for the lightest helium star that burns helium. The step is
    at the death, which the continuity tests exclude, but P06.T16's bridge will not remove it.
    The early AGB's remnant passes from the end of the helium main sequence to the helium giants'
    relation over the first third of the early AGB's own span, where SSE uses a third of its
    nuclear time; this is provisional, for T12.b to decide. HPT's eq. 90 cools white dwarfs with A
    = 4, 15 and 17 (ruling 33). A black hole's luminosity is exactly zero, and
    `StarState::luminosity` says that no consumer may take its logarithm unguarded. Inside the
    track, the only logarithms of L are of living phases.
  - _Seams._ `phases::collapse_remnant` is P06.T18.d's: HPT's remnant stands in under both recipes
    until then. `evolve` is where P06.T13's `substellar::cooling` takes over below 0.1 M☉; until
    the orchestrator wires it (ruling 33), masses below 0.1 M☉ are evaluated at 0.1. The AGB hands
    straight to the white dwarf until T16, and that step is declared to the continuity test, as are
    the unbridged flash of `Bridges::Instant` and a helium star too light to burn helium. SSE turns
    such a star into a helium white dwarf at once (`zpars(10)`, 0.31–0.35 M☉), and HPT do not print
    the rule.
  - _Supernova types._ IIP above 2 M☉ of hydrogen envelope, after Heger et al. (2003, §4.1 and
    Fig. 2), who assume that split. IIL from 0.1 M☉, the plan's default. IIb below that, Ib above
    0.14 M☉ of helium outside the carbon–oxygen core, and Ic below. The 0.14 is the top of the
    0.06–0.14 M☉ that Hachinger et al. (2012, §4.3) find can hide in low-mass SNe Ic, extrapolated
    to heavier cores. **For the orchestrator to rule:** the 0.1 M☉ IIb bound matches SN 2011dh's
    envelope of about 0.1 M☉ (Bersten et al. 2012), but it calls the prototype, SN 1993J, Type IIL:
    its envelope was 0.20 ± 0.05 M☉ (Woosley et al. 1994).
  - _Dead code._ T10 made the phase modules' blanket expectations unnecessary, and they are gone.
    What only tests call is now `#[cfg(test)]`: the constant-mass `at` of the gap, the giant
    branch, core helium burning and the pulsing AGB, `l_zahb`, `r_zahb`, `t_hook`,
    `GiantBranch::{m_x, l_x}`, `HeliumStar::{from_core_helium_burning, end, phase_at}` and
    `ReimersEta::HURLEY`. Accessors that nothing calls are removed, and so is `r_mhe_low`, whose
    wrapper had no caller. `agb::interpulse_period` keeps an expectation naming T28.f.
    `DegenerateCore::Helium` is now what the track builds helium white dwarfs from.
  - _Tests._ They are as the task lists. Added: finiteness over 120 random tracks under both
    recipes (ruling 30), a helium star's whole-mass white dwarf, and the envelope's loss continuous
    to 1% in L and R. The lifetime-in-mass sweep runs 2 metallicities × 60 intervals in the fast
    suite (2.5 s) and 5 × 400 as a slow test. Slow tests, at the slow-test profile, one thread
    each, load 5–7: 200 random tracks × 2 recipes × 2,000 ages continuous, 8.4 s; 10⁵ life and
    death inputs, 228 s; 10⁴ fast lifetimes equal to the full track's bit for bit, 28 s; the 5 × 400
    sweep, 54 s. All pass.
  - _Against SSE_, run with `evolve.in`'s options changed as ruling 26 asks and steps a hundred
    times finer (`pts` × 0.01), over the 16 masses × 5 Z. Phase-start ages agree to 9.5 × 10⁻⁵,
    except a 0.1 M☉ helium white dwarf's 8.1 × 10⁻⁴ at 7 × 10¹² years. Masses at phase starts agree
    to 0.85%, the worst at 1 M☉ on the pulsing AGB, where SSE's own steps jitter by ±0.3%. Cores at
    phase starts agree to 9.5 × 10⁻⁴ and remnant masses to 5.3 × 10⁻⁴ M☉. Three routes differ, each
    by a sliver of a phase: 47 years of core helium burning (60 M☉, Z = 10⁻³), a pulsing AGB of no
    length (0.8 M☉, Z = 0.004), and SSE's 160 years as a helium giant before the supernova (20 M☉,
    Z = 0.03). Within phases, L and R at equal phase fractions differ by up to 1.7 dex, but only
    where the state moves steeply with time. That is the gap of massive stars, whose LBV wind strips
    the envelope in a burst, and the pulsing AGB's end. T12.a's samples must avoid those places.
  - _Speed_ is a finding, not a failure (ruling 40). Measured on 2026-09-23 with the bench profile
    (`benches/stellar.rs`) at load 2.3 rising to 5.2 and 2.9–3.0 GHz. `math::exp` took 6.9 ns
    before the groups, the idle figure, and 18.1 ns after, once the load rose.

    | Call                                   | Time             | `math::exp` calls | Target                          |
    | -------------------------------------- | ---------------- | ----------------- | ------------------------------- |
    | `lifetime`, 4 M☉ (layer D)             | 2.0 ms           | 295,000           | 5 µs                            |
    | `lifetime`, 20 M☉ (layer E)            | 0.95 ms          | 137,000           | 5 µs                            |
    | `Track::to_age`, 0.4 M☉ at 5 Gyr       | 10.7 µs          | 1,550             | 10 µs (a dwarf's `generate`)    |
    | `Track::to_age`, 2 M☉ giant at 1.2 Gyr | 0.49 ms          | 71,000            | 60 µs (a giant's `generate`)    |
    | `Track::full`, 1, 5 and 20 M☉          | 1.5, 1.8, 1.3 ms | 187,000–261,000   | 150 µs (a remnant's `generate`) |
    | `state_at`, main sequence and AGB      | 0.20, 1.8 µs     | 28, 260           | —                               |

    A second run at load 7–9 agreed to 10–40%. A dwarf meets its target, and every evolved track
    misses by 8–12 times. `lifetime` misses by 200–400 times, because it integrates the same grid
    as the full track and drops only the samples and the remnant, as it must to stay equal to it
    bit for bit. Each derivative of the envelope integration evaluates the phase's closed forms,
    the wind, and five core-mass or progress probes, and a track takes about a thousand of them.
    Nothing on the slice's path calls `lifetime` in bulk. Plan 08's placement will, and its target
    stays open until then.
- **Deviations in T12, as built (round 7, `starA`).**
  - _T12.a._ `crates/hyperion-sim/tests/data/sse/z{0.0001,0.001,0.004,0.02,0.03}.csv`, about 32 KB
    each, with a provenance `README.md`. The run is SSE's `evolv1` with ruling 26's options, and
    with steps a hundred times finer than distributed (`pts` × 0.01, ruling 40); the header and the
    README give both step settings. A "phase change" is the first logged step of each SSE stellar
    type, and the row of a remnant's type is the death. The plan's "20 samples along each track"
    are steps at fixed fractions of a phase's time, not at ages, chosen from SSE alone. Candidates
    sit at 0.05, …, 0.95 of each phase that lasts at least 10⁻³ of the lifetime. A candidate is
    kept where log L and log R move by at most 0.05 dex across ±1% of the phase, and where SSE's
    run at ten-times-coarser steps agrees to 0.005 dex, so that SSE is converged there. The
    candidates are then taken round-robin over the phases. Of 6,327 candidates, 5,988 pass, and
    every star has 20. Without the convergence filter, one early-AGB sample (20 M☉, Z = 0.03, at
    0.40 of the phase, where item 4's blend ends) was 0.0205 dex off in R. There SSE's own R moves
    by 0.075 dex between its two step settings, and its core moves towards ours.
  - _T12.b._ `tests/sse_reference.rs`, on the public API only. Phases are matched by SSE's type,
    with the helium Hertzsprung gap and giant branch merged. A phase lasting under 10⁻⁴ of the
    lifetime may be missing from the other code; three are: 47 years of core helium burning (60 M☉,
    Z = 10⁻³), a pulsing AGB of no length (0.8 M☉, Z = 0.004), and SSE's 160 years as a helium giant
    (20 M☉, Z = 0.03). All 80 stars pass at the plan's tolerances. The worst deviations are:

    | Quantity          | Worst           | Where                                  | Tolerance |
    | ----------------- | --------------- | -------------------------------------- | --------- |
    | Phase-start age   | 9.5 × 10⁻⁵      | core helium burning, 1 M☉, Z = 0.03    | 1%        |
    | Lifetime          | 8.1 × 10⁻⁴      | 0.1 M☉, Z = 0.004                      | 1%        |
    | Phase-start mass  | 0.76%           | early AGB, 20 M☉, Z = 0.02             | 1%        |
    | Phase-start core  | 9.5 × 10⁻⁴      | Hertzsprung gap, 100 M☉, Z = 0.02      | 1%        |
    | log L at a sample | 2.8 × 10⁻³ dex  | core helium burning, 0.8 M☉, Z = 0.004 | 0.02 dex  |
    | log R at a sample | 1.07 × 10⁻² dex | core helium burning, 0.8 M☉, Z = 0.004 | 0.02 dex  |
    | Remnant mass      | 5.3 × 10⁻⁴ M☉   | black hole, 40 M☉, Z = 10⁻⁴            | 0.02 M☉   |

    Every remnant is of SSE's kind. **Ruling 29's two SSE artefacts do not arise with the wind:**
    both 60 M☉ stars (Z = 10⁻⁴ and 10⁻³) lose their envelope in the gap and never reach the AGB,
    in either code. A test keeps that true, and no point is exempted. Ruling 40's item 4, the early
    AGB's remnant blend over a third of the phase, passes the tolerance and stands.

  - _T12.c._ The initial–final mass relation of HPT's Fig. 18 was read from the journal's 799-dpi
    bitmap to about ±0.003 M☉: 17 points at Z = 0.02 and 16 at 0.004, from 1.25 to 7.5 M☉. Under
    `TrackOptions::hurley2000` the tracks agree to 0.0025 M☉ (the plan asks 0.05). Under the
    generator's recipes, at [Fe/H] = 0 and 0.85–7.2 M☉ in 0.05 M☉ steps, the white dwarfs follow
    Cummings et al.'s (2018, ApJ 866, 21) adopted MIST fit, eqs. 4–6, to 0.08 M☉ (design note 9).
    The worst is +0.069 M☉ at 7.2 M☉, and −0.05 M☉ near 1 M☉, within their 0.06 M☉ scatter.
    HPT tabulate no main-sequence lifetimes. Their Fig. 5 plots Pols et al.'s detailed-model
    `t_BGB` at Z = 10⁻⁴ and 0.03, which eq. 4 fits to 4.8%. `ms::t_bgb` agrees with 24 of those
    models, 0.5–4 M☉, to within 4.3%, and the test (`sse::evolve`'s unit test, since `t_bgb` is
    crate-private) allows 0.03 dex. Above 4 M☉ the two metallicities' markers overlap. The model
    near 0.63 M☉ is left out, because its mass would need three digits at log t ∝ −3.7 log M.
  - `cargo test -p hyperion-sim --test sse_reference` takes 0.45 s, so it is not marked slow.
- **Deviations in T23 and T20.b, as built (round 7, `class`).** Built without the T24/T25 extras (ruling 33). The integrator (T10.c–e) was another lane's, so every test builds its `StarState`s directly.
  - _Files and API._ Besides `classify/{mod,pm13,luminosity}.rs` and `photometry.rs`, there are four new files:
    - `classify/sk81.rs` holds the class boundaries' source table.
    - `classify/scales.rs` holds the giant and supergiant temperature scales.
    - `remnant/wd_spectral.rs` holds T20.b. It is not `white_dwarf.rs`, which `starA` owns.
    - `tests/stellar_classify.rs` writes a new golden, `stellar/classify`.

    `remnant/mod.rs` gains one `pub mod wd_spectral;` line, and the sim's `clippy.toml` gains one word, `McElroy`, in its list of valid identifiers.
    - `classify` takes `(&StarState, &Composition, &StarDraws, &ClassExtras)`. `ClassExtras` is an empty `#[non_exhaustive]` struct: `ClassExtras::NONE` is its only value, and `classify` destructures it, so a field added by T21, T24 or T25 fails to compile until it is read. `PeculiarClass` is an uninhabited enum, so `Classification::peculiar_class()` is always `None`, and the `Display` match on it is empty.
    - Added: `SpectralLetter`; `SpectralCode`, the continuous code, ten per class with O0 at 0, written to the nearest half subtype, half up; `NeutronStarClass`; and `subtype_from_teff(Kelvin) -> Option<SpectralCode>`.
    - `LuminosityClass` names its variants (`Hypergiant` for Ia⁺ through `Dwarf` for V) and adds `Subdwarf` and `ExtremeSubdwarf`, written as prefixes. The Ia⁺ class is written `Ia+`.
    - `SpectralType` is `Sequence(SpectralCode)`, `WhiteDwarf(WhiteDwarfType)`, `NeutronStar(NeutronStarClass)`, `BlackHole` or `NoRemnant`.
    - Photometry: `bolometric_correction_v(Kelvin)`, `colour_b_v(Kelvin)` and `absolute_magnitude_v(&StarState)` each return `Option<Magnitudes>`. Also added: `absolute_bolometric_magnitude` and `SOLAR_ABSOLUTE_BOLOMETRIC_MAGNITUDE`.

  - _T23.a._
    - The table is Mamajek's maintained version 2022.04.16, whole: 118 rows from O3V to Y4V, SHA-256 in the header. The alternative was Table 5 (O9V–M9V) with the extension stitched on. The version differs from Table 5 by up to 700 K (O9V), and its BC_V is on the IAU 2015 scale (−0.085 for the Sun).
    - Interpolation is linear in log T_eff, and the tables are checksummed. The Sun reads G1.98, written G2V, with M_V = 4.825.
    - BC_V is tabulated down to L5V and B − V down to M9V; cooler objects get `None`.
    - Above O3V, BC_V follows the Rayleigh–Jeans slope of 7.5 mag per dex, and B − V is held. From 45 to 150 kK this falls within 0.1 mag of the Montreal DA models' fall in BC_V.
    - White dwarfs, neutron stars and black holes get no M_V. The dwarf table's BC_V is 0.6–3.3 mag off the Montreal DA models from 4,000 to 3,000 K.
  - _T23.b._ The plan asks for log g boundaries throughout. They are used only between V, IV and the rest.
    - III, II, Ib, Iab and Ia are separated by luminosity at the star's temperature. The source is Straižys and Kuriliene (1981), whose Tables III, IV and VII were transcribed and cross-checked by their own formula for log g to 0.045 dex.
    - The reason: their class III gravities are those of 2.1–3.5 M☉ tracks. On log g alone, Arcturus-like giants (4,300 K, log g 1.7) would read II, which fails the plan's own test.
    - Ia⁺ uses the Humphreys–Davidson limit in HPT's form, as the winds do.
    - The tie rule extends to the AGB: a core-helium-burning or AGB star is at least III.
    - Subtype scales:
      - V is exact on the dwarf scale.
      - III: Martins et al. (2005) for O; the dwarf scale times Zorec et al.'s (2009) III/V ratio for B–A1; the dwarf scale for A2–F5; a log T_eff bridge from F5 to G5; van Belle et al.'s (2021) fit for G5–M5.5; and Richichi et al. (1999) for M6–M9.
      - The supergiant scale (Ib, Iab, Ia and Ia⁺ share it): Martins for O; Markova and Puls (2008) for B0–B7; Firnstein and Przybilla (2012) for B8–A3; Humphreys and McElroy (1984) for F–G; Levesque et al. (2005) for K1–M5; then parallel to the giants.
      - IV and II take the subtype halfway between their neighbours' at the star's temperature.
      - Giants and supergiants are typed no later than M9.5.
    - de Burgos et al.'s (2024) B supergiants were set aside: they run 1–1.3 kK hotter at B1–B2 and would put those supergiants above the giants.
  - _T23.c._
    - sd and esd are classes, with the plan's [Fe/H] thresholds of −1.0 and −1.7. These have no source: Lépine et al. (2007) define the classes by TiO/CaH, not [Fe/H]. They apply to main-sequence dwarfs of class V only.
    - L, T and Y are written without a class. Every neutron star is `NS`.
    - `NoRemnant` is written `NONE`.
  - _T20.b._
    - The helium-atmosphere fraction follows the measurements and is not monotone. It is held at 24% above 75 kK, the share Bédard et al. (2020) find "born with hydrogen-deficient atmospheres"; their 87% above 90 kK is only hydrogen-rich stars crossing those temperatures faster. It falls to 8% at 30 kK (their Fig. 19), then rises to 32% at 5.5–7 kK (Kilic et al. 2025, Table 4). Reversals are pooled.
    - As a result, above 29,854 K a helium-rich star can float up to DA. The plan's test ("never back to DA") is asserted from there down.
    - The DB/DC boundary is set at 11,000 K instead of 12,000, where Kilic's DBs give way.
    - The DQ fraction peaks at 38% (8–9 kK) and falls, where the plan had it rising.
    - Metal lines use the 40 pc census's optical rates, constant with age: 8.0% of DAs and 15.8% of the rest (O'Brien et al. 2024). The plan asked for 0.25–0.5 by cooling age, which is Koester et al.'s (2014) intrinsic rate, measured in the UV. It would triple the DAZ fraction against every census, and O'Brien et al. find no trend with cooling age.
    - The census test uses uniform ages over 0–9 Gyr on the Montreal 0.6 M☉ DA sequence and is cut at 5,000 K, where the censuses are complete. It gives DA 70.2%, DC + DQ + DZ 28.5% and DB 1.2%. The DB bracket was 2–10%, below every measured sample (1.1–2.0%), and is corrected to 1–3%.
  - _Known limits, for T24, T25 and plan 07._
    - Helium stars and hot central stars are typed by temperature alone (a 90 kK helium star reads O3V) until T24.a.
    - Luminous low-mass AGB stars read II by their luminosity: the Mira-like case reads M8II, where catalogues give M7 III.
    - BC_V is the dwarfs' at every gravity, so a late M giant's M_V is up to 1.7 mag too bright (Straižys and Kuriliene 1981, Table III, at M6 III).
- **Deviations in T33, as built (round 7, `wire`).** Built in one change with P11.T13's slice and
  P14.T35.a, in a new private module `stellar.rs` beside `orbit.rs` and `planetary.rs`, their types
  re-exported at the crate root as every module's are; `envelope.rs` and `galaxy.rs` gain the kind,
  the error codes and the two optional range fields.
  - _The field set._ `SystemSummaryDto`: `universe`, `system`, `time`, `existence`
    (`SystemExistenceDto`: `not_yet_born`, `exists`), `age_myr`, `fe_h_dex`, `stars` and plan 11's
    `hierarchy`. `StarSummaryDto`: `body_index`, `kind`, `phase` (`PhaseDto`, one value per
    `Phase`), `class`, `initial_mass_msun`, `mass_msun`, `core_mass_msun`, `luminosity_lsun`,
    `radius_rsun`, `teff_k`, `absolute_v_mag`, `colour_b_v_mag`, `mass_loss_rate_msun_per_yr`,
    `remnant` and `death_time`; then, absent until their tasks land, `rotation_period_d` and
    `activity_log_lx_lbol` (T25), `variability` (T26), `planetary_nebula` (T16) and
    `active_events` (T28). `age_myr` and `initial_mass_msun` are not in the task's list: T29.b
    computes both, and the `SYSTEM` display shows them at times other than the chart row's.
    `body_index` and `hierarchy` are required, since they land with the kind and no older form
    exists.
  - _Three states per value._ Absent: this generator version does not compute it (ruling 34's
    single value, the em dash). `null`: computed, and the object has none, plan 04's convention for
    an `Option`. Otherwise the value. A value that is absent now and can be `null` once computed (a
    star that does not vary, one with no nebula) is the new `hyperion_protocol::Modelled<T>`
    (`NotModelled`, `Null`, `Value`), with hand-written serde impls and
    `#[ts(as = "Option<Option<T>>", optional)]`, so TypeScript reads `name?: T | null`; Clippy's
    `option_option` ruled out a bare `Option<Option<T>>`. One that always has a value once
    computed is a skipped `Option`. **For the orchestrator to rule.**
  - _`RemnantDto`_ is tagged by `type`: `white_dwarf { cooling_age_myr, natal_kick? }`,
    `neutron_star { pulsar?, natal_kick? }`, `black_hole { dimensionless_spin?, natal_kick? }`,
    `no_remnant`. The white dwarf's type is not repeated: it is the star's `class`, where T23 and
    T20.b write Sion's type (`DA4.2`), and its composition is the `phase`. `PulsarDto` is
    `spin_period_s`, `period_derivative_s_per_s`, `magnetic_field_g`, `alive` and `magnetar`;
    `NatalKickDto` is `speed_km_s` and `KickModeDto` (`ordinary`, `low`, `fallback_none`,
    `white_dwarf`).
  - _Shapes for later tasks, fixed now from their text._ `VariabilityDto` is `kind`, `period_d` and
    `amplitude_mag` (peak to peak, V), with `VariableKindDto` holding the 21 kinds T26.a–c name;
    `PlanetaryNebulaDto` is T16.b's `radius_ly`, `expansion_speed_km_s`, `age_yr`,
    `ionised_mass_msun` and `excitation_class`; `StarEventDto` is `kind` (`StarEventKindDto`,
    T28's seven), `onset` and `duration_s`, and T28 adds each kind's magnitude, and the event ID if
    plan 12 needs it, as fields. Activity is log₁₀ L_X ÷ L_bol, the quantity T25's Rossby law
    gives, since `ActivityLevel` has no shape yet.
  - _Light-less objects._ `StellarBriefDto`'s `log_luminosity_lsun` and `teff_k` are
    `Option<f32>`, `null` for a black hole and for `NoRemnant`: log₁₀ 0 is −∞, which `serde_json`
    writes as `null` and cannot read back as an `f32`. `StarSummaryDto.teff_k` is `null` for them
    too, rather than the sim's 0 K.
  - _Radii of neutron stars and black holes_ stay `radius_rsun` on the wire, one field in one unit
    for every object; ruling 36's km is the client's scale step. **For the orchestrator to rule**,
    against a `radius_km` on those remnants.
  - _`include_stellar`_ is a `bool` with `serde(default, skip_serializing_if = "std::ops::Not::not")`
    and `ts(as = "Option<bool>", optional)`, since ts-rs's `optional` takes only an `Option`. A
    request of plan 04's form parses as `false` and serialises unchanged, and a row without a brief
    has no `stellar` key. Until T34 the server ignores the flag and every row's `stellar` is `None`,
    which a test in `convert.rs` pins so that T34 changes it knowingly.
  - _Server._ `Handlers` answers `system_summary` with `unsupported` under its own ID
    (`not_served_yet`), `kind` names it, and `is_large` puts it with the small responses;
    `tests/websocket.rs` checks the answer and that the connection carries on.
  - _Error codes._ `unknown_system` and plan 14's `unknown_body` both land here, after
    `unknown_universe`, with their `settledState` cases and a hook test, and both join
    `RequestStatus.tsx`'s `REFUSALS` (a set, not an exhaustive switch, so nothing failed to
    compile), which would otherwise have read them as faults. Both files are the `ui` lane's.
  - The server's `galaxy_parameters` and `systems_in_range` goldens are unchanged
    (`golden_diff.py`: "No golden files changed").
- **Deviations in T18.d, T20.a and T29, as built (round 7, `model`).** No existing golden moved.
  Three goldens are new at 11: `stellar/summaries`, `stellar/endings` (T18.d's and T20.a's endings
  on real tracks, bit for bit) and `stellar/white_dwarf_cooling`. **Ruled (ruling 57):** the
  version stays 11, as `SYSTEM-VIEWER-PATH.md` §3 has it ("land T20.a before T29.b's goldens and it
  costs no bump"). But the default recipe's `Track` output does move: white-dwarf luminosities, the
  §6.3 perturbation's target, iron-core remnants, and the IIb bound. Through the perturbation's
  target, the wind also moves the lifetimes and white-dwarf masses of stars with thin envelopes. No
  golden at HEAD pinned any of it, and nothing seeded read it; ruling 33 had expected a bump.
  - _T18.d's wiring._ The builder carries the star's `RemnantDraws`. Every iron-core death (the
    early AGB's supernova and an oxygen–neon helium star's) goes through `phases::iron_core_fate`:
    HPT's equation 92 under `Hurley2000`, so T12.b is untouched, and `collapse::core_collapse` under
    `MandelMuller2020`. A neutron star or partial fallback keeps `CoreCollapse { supernova }`;
    complete fallback and pulsational pair instability are `DirectCollapse`; a pair-instability
    supernova is `PairInstability` with `NoRemnant`. The fate records the supernova type, so that
    `Track::fate_with(RemnantDraws)` redraws the remnant on the built track (T29's remnant stage,
    which plan 08 repeats). The single-star window is tested at the end of the thermal pulses on the
    early AGB's initial mass, the mass `m_c_bagb` reads (ruling 45): inside it the star collapses
    by electron capture into the 1.26 M☉ neutron star however its pulses end. At Z = 0.02 that is
    [8.103, 8.203) M☉ in that mass, about [8.20, 8.30) M☉ initially, and it meets the iron cores
    with no gap. The windows are found only when a star reaches the pulses' end. The stripped
    window waits for T19.c's mark: the track never sets `Stripping::Companion`.
  - **Below the window. Ruled (ruling 57).** On the tracks as built, the pulses of stars 0.13 M☉
    (Z = 0.02) to 0.34 M☉ (Z = 10⁻⁴) below the window grew oxygen–neon cores to `Mc,SN` = 1.44 M☉
    with up to 5.9 M☉ of envelope still on. That left 1.44 M☉ dwarfs at the neutron-star radius
    floor, 2.0–2.2% of an 8–150 M☉ sample. Under the default an oxygen–neon white dwarf is now
    capped at `collapse::OXYGEN_NEON_CAPTURE_MASS`, 1.37 M☉. That is the electron-capture mass of
    Miyaji et al. (1980) and Nomoto (1984), about 1.375 M☉. Outside the window the AGB ends when the
    core reaches the cap, on the thermal pulses or, where `Mc,DU` is already above it, on the early
    AGB, and the envelope goes then. This follows Doherty et al. (2015, MNRAS 446, 2599): a
    super-AGB star below the window loses its envelope first. `Hurley2000` keeps HPT's electron
    capture there. Over 5–9 M☉ at five metallicities the heaviest white dwarf is exactly 1.37 M☉,
    at 0.002 96 R☉ (2,062 km), against the floor's 1.75 × 10⁻⁵ R☉; 125 of the scan's dwarfs sit at
    the cap. A test holds that no dwarf passes the cap or comes within 20 times the floor. The
    three shares and T12.b are unchanged. In `stellar/endings` the rows of 7 and 8.1 M☉ at Z = 0.02
    moved, and no other golden did. At 7 M☉ the shorter span of the pulses moves the knots: the
    death comes 530 years earlier and the dwarf is 6 × 10⁻⁵ M☉ lighter. At 8.1 M☉ the 1.44 M☉
    dwarf becomes 1.37 M☉.
  - _Shares on real tracks_ (Kroupa 8–150 M☉ at Z = 0.02, own η and remnant draws, tracks held to
    100 M☉ until T14, 20,000 stars). Seed `0x0618d00000000001`, which the slow tests use, gives 35.98%
    black holes, 71.59% complete fallback and 2.68% electron capture. Seeds 2 and 3 give
    36.37–36.67%, 70.39–70.87% and 2.52–2.70%. All are in band: complete fallback sits 0.4–1.6
    points above its floor, as the forecast said. Neutron stars span 1.132–1.997 M☉, and 1,661
    black holes are of 2–5 M☉. The tests are slow, 20 s each at the slow-test profile.
  - _The IIb bound is 0.5 M☉_ (ruling 46.1). It is the upper end of the envelopes inferred for SNe
    IIb, from Sravan, Marchant and Kalogera (2019, ApJ 885, 130, §2.3): ≲ 0.5 M☉ for every one with
    a detected progenitor (1993J, 2011dh, 2011fu, 2016gkg), and below it for larger samples.
    SN 1993J (0.20 ± 0.05 M☉), 2011dh (about 0.1) and Cas A (a 1993J twin by its light echo, Krause
    et al. 2008) are IIb. The lower bound stays zero. The sample gives 90.9% IIP, 1.0% IIL, 3.4%
    IIb, 4.7% Ib and no Ic, as single stars should.
  - _T20.a._ `white_dwarf::{hurley_shara_luminosity, luminosity, formation_luminosity,
cooling_origin}`, with the paper's 300, 1.18 and 6.48, the 9,000 Myr break, and HPT's A of 4, 15
    and 17 (HS03's 20:80 C:O and 80:20 O:Ne give 15.2 and 16.8). The late factor is SSE's
    (9,000.1 A)^5.3, because the printed (9,000 A)^5.3 leaves a 6 × 10⁻⁵ step at 9 Gyr. §6.3's
    perturbation reads the recipe's law at t = 0, as SSE does with `wdflag` > 0. Under the default,
    the law's clock starts where it gives the star's last luminosity, above −0.1 Myr, so L is
    continuous at every hand-over. **Ruling 46.2:** the light helium stars' 0.26–0.34 dex step is
    therefore gone now, at T10.d's direct hand-over, and not only once T16 lands. The oxygen–neon
    dwarfs' 0.06 dex step goes with it, and only the radius still steps. `Hurley2000` keeps both
    steps; a test shows each. When T16 lands, the match moves to the bridge's end.
  - **The 10% check fails.** Against Bédard et al.'s (2020) 0.6 M☉ thick-H sequence (the Montreal
    `seq_060_thick.txt`), Hurley and Shara's T_eff is within 10% only at 0.01–0.02 Gyr and 2–3 Gyr.
    It is 13–20% cool from 0.05 to 1 Gyr and 11–17% cool from 5 to 10 Gyr, worst −20.0% at 0.2 Gyr.
    Equation 90 is 9–45% cool. The test pins these deviations and that the law beats equation 90 at
    all 13 ages; it does not claim the plan's bracket. **Ruled (ruling 57):** HS03 stands
    provisionally, with these deviations pinned; a fit to the Montreal sequences replaces it in
    version 12's batch.
  - _T29.a._ `stellar::system::{StarModel, BuildStarModelError, MAX_STAR_MASS}`. It has
    `StarModel::new(m0, Composition, StarDraws, age_at_epoch) -> Result` for 0.01–150 M☉. It also
    has `state_at(t) -> Option<StarState>`, `None` while the age is not positive, as
    `existence_at` has it. The remaining methods are `lifetime()`, `death()` and `remnant()`, each
    an `Option` that is `None` below 0.1 M☉; `max_radius_until(t)` and `max_luminosity_until(t)`,
    zero before formation; `natal_kick()`, `None` until T19; and `age_at(t)`, `initial_mass()`,
    `composition()`, `draws()` and `age_at_epoch()`. The track is built to `age_at(+H)`. A star
    living past +H finds its death on demand by `sse::fate_of`, which is `Track::full`'s bit for
    bit and costs a whole build, 1–2 ms for an evolved star. Stars above 100 M☉ are evolved at 100
    until T14. `remnant::{NatalKick, KickMode}` are the Provides shape, with no law.
  - _T29.b._ `SystemStars::{generate, record, stars, primary, summary_at, brief_at, death_time,
natal_kick, lbv_window}`, with `SystemSummary`, `StarSummary`, `StellarBrief`,
    `SystemExistence`, `ClockDeath` and `object_kind`. `StarSummary` holds the state, `ObjectKind`,
    classification (T23 with `ClassExtras::NONE`), M_V, B − V, the remnant once dead, and the death
    if it falls in [−H, +H]. Variability, rotation, magnetism, nebula and events wait for their
    tasks. `brief_at` is `None` before birth, and its log L is `None` where L = 0. `lbv_window` is
    `None` until T24.a. `death_time` is `BeyondClockRange` for T past `i64` seconds (the lightest
    dwarfs, whose lifetimes pass 2.9 × 10¹¹ years) and for objects below 0.1 M☉. `ObjectKind` for helium stars uses T24.a's floor of
    10⁴·⁹ (Z ÷ 0.02)^−0.4 L☉: Wolf–Rayet above it, hot subdwarf below.
  - _Tests._ 10⁵ random systems across the layers near the solar circle at random clock times hold
    the brainstorm's property (a slow test; 400 in the fast suite).
  - _Outside the owned files._ `remnant/mod.rs` gains `mod kick` beside the `class` lane's
    `pub mod wd_spectral` (applied from `class-P06T23T20b.patch`, with its `McElroy` in `clippy.toml`
    and its Risks bullet above). `sse/mod.rs` re-exports `fate_of`.
- **P06.T35.b's registry, as built alone (round 7c, `ui`).** `lib/galaxy/starSymbols.ts` exports
  `starSymbol(kind: ObjectKindDto): SymbolShape | null`, an exhaustive `switch` over the wire's 13
  kinds: circle for a protostar, pre-main-sequence star, dwarf, subgiant, hot subdwarf and
  `substellar` (a brown dwarf), ringed circle for a giant, supergiant and Wolf-Rayet star, diamond
  for a white dwarf, triangle for a neutron star, square for a black hole, and `null` for
  `no_remnant`, which is listed and not drawn (the owner's draft of the symbol set). `ObjectKind` is
  the wire's `ObjectKindDto`; no client alias was added. `starSizeClass(shape, layer)` gives the mass
  layer's class and raises a ringed circle to `RINGED_CIRCLE_MIN_SIZE_CLASS` (2), ruling 35.4's
  floor, which a test pins for every layer. Its first caller is plan 14's orbit map; the chart's
  `STARS` filter, legend entries and class column, `wire.ts`'s brief and `include_stellar` still wait
  for P06.T34, which fills the brief. P06.T36's readout was built for the `SYSTEM` display's host,
  not the chart's `SystemReadout` (plan 14's round-7c bullet), and its words (`objectKindLabel`,
  `phaseLabel`, `remnantLabel`) are in `lib/system/words.ts` for the chart to reuse. Luminosity and
  radius have their formatters, `formatLuminosityLsun` and `formatRadiusRsun` (three significant
  figures, E notation below 0.001), with `KM_PER_RSUN` (695,700, IAU 2015 B3) and `formatRadiusKm` for
  ruling 36's compact remnants, and their drawn units, `SolarUnit` (`L☉`, "solar luminosities";
  `R☉`, "solar radii").
- **Deviations in T34, as built (round 7, `srvstars`), without the range briefs.** The code is
  `hyperion-server`'s `requests/system.rs` (`summary`), `convert/stellar.rs` (`SummaryRequest`,
  `system_summary`, `unknown_system`, `orbit_dto`) and `compute/systems.rs` (`SharedSystemCache`).
  `config.rs` gains `--system-cache` (`HYPERION_SYSTEM_CACHE_MB`, default 128 MiB), there is a new
  `ServerStats::systems()`, and the README's table of options gains the row.
  - _The cache._ It is a `SharedByteLru<(GalaxyKey, SystemId), SystemStars>`, with
    `impl HeapBytes for SystemStars` through the sim's new `SystemStars::heap_bytes` (every track's
    segments, knots and samples by capacity, and the hierarchy's lists). Measured charges: 10.6 KB
    for a pair of living dwarfs, 16.2 KB for the pinned triple, 21 KB for a quadruple of dwarfs, and
    30–80 KB where a star is a white dwarf with its whole track. So 128 MiB holds some 1,600–12,000
    systems. Entries are epoch state, so a request at another time is a hit. A hit skips `resolve`,
    since only IDs that resolved are stored. There is no `SingleFlight`: two concurrent misses both
    generate, and the second insert replaces an equal value (`SharedByteLru`'s rule).
  - _The order of checks_ is the universe, then `time` (plan 04's `query_time`: `bad_request`
    naming `time`), then the ID, then the galaxy, then one interactive pool job. The job looks up
    the cache, and on a miss resolves and generates, then summarises at the time and converts. The
    small frame is serialised on the runtime. **For the orchestrator to rule:** a well-formed
    16-digit `SystemIdHex` whose bits fail `SystemId::from_raw` is neither `resolve`'s refusal nor a
    parse failure. It is answered `unknown_system` naming `system`, as the code's doc ("a
    well-formed system ID that names no system") reads.
  - _Wire rules (ruling 54)._ Rotation, activity, variability, a nebula, active events, a pulsar's
    detail and a black hole's spin are absent. A natal kick is sent when the sim has one (none
    before T19). `teff_k` is `null` where L = 0. A white dwarf's cooling age is its age less its
    age at death. The hierarchy's nodes come from `SystemSummary::hierarchy`, empty before birth.
    A star node's mass is its initial mass. An orbit's fields are `KeplerElements`' accessors, bit
    for bit.
  - _Not this round: `include_stellar`'s briefs._ A brief builds a track, 1–2 ms (ruling 46), so a
    brief per row of a 20,000-system answer is seconds. The flag is accepted and every row still
    has no `stellar`. `convert.rs` documents this, and its test is renamed
    `a_request_for_briefs_gets_rows_without_them_until_the_integrator_is_fast`. The test "a range
    request with `include_stellar` returns a brief on every row" waits with the briefs.
  - _Tests._ `tests/system_summary.rs` holds five tests:
    - four pinned systems (one to four stars) equal the sim's field by field, companions included,
      at three times;
    - P11.T13's triple returns three stars and two orbits, with the elements, μ = G ΣM and the
      masses;
    - `bad_request` naming `time`, and `unknown_system` naming `system`, for `u64::MAX` and for the
      index after a cell's last candidate; a malformed ID is `bad_request`;
    - a summary cancelled while its cold galaxy builds ends with exactly one terminal message:
      `cancelled`, or the answer on a machine that finishes the build first;
    - the same request twice gives the same frame, a miss then a hit, and a second universe of the
      seed shares the entry.

    There are unit tests for the conversion and the cache. `websocket.rs`'s "unsupported until its
    handler lands" test, and `Handlers`' `not_served_yet`, are removed. The server's
    `galaxy_parameters` and `systems_in_range` goldens are unchanged.
- **T20.a refitted to the Montreal sequences, as built (round 8, `wdcool`; ruling 57.2).** Under
  the modern recipe a white dwarf now cools by `stellar::remnant::cooling`, which reads
  `tables/wd_cooling.rs`, the output of `hyperion-fit run wd_cooling` (task version 0).
  - _Data and licence._ The input is the 23 thick-hydrogen (DA) sequences of Bédard, Bergeron,
    Brassard and Fontaine (2020, ApJ 901, 93), 0.2–1.3 M☉ in steps of 0.05, from
    <https://www.astro.umontreal.ca/~bergeron/CoolingModels/>, retrieved 2026-09-24. The page states
    no licence. It asks users of its tables to acknowledge the site and cite the papers, and the
    table's header does both. The raw files are therefore **not committed**. The fit reads them from
    `target/data/montreal_cooling/` (or `--data`), and the header records their FNV-1a digest,
    `0x632cda247f9d4def`. `hyperion-fit`'s reproduction test and residual test run only where the
    files are present, and say so on stderr otherwise. The sim's own tests quote the held-out models
    they check, as T20.a's first check quoted thirteen.
  - _The fit._ For each sequence, the table holds log₁₀(t + 0.1 Myr) at 96 luminosities evenly
    spaced over log₁₀ L = −7.5 to 2.5. That is the age as a function of the luminosity, which stays
    gentle through a crystallised dwarf's Debye plunge, where L(t) does not. The values are least
    squares for linear interpolation in log L, with a 10⁻⁶ curvature penalty. Every model whose
    number is a multiple of 5 is held out. Past a sequence's brighter end the column goes on
    straight. Past its fainter end, about 1,500 K, it follows Mestel's L ∝ t^−1.4. Carried on at
    the last models' slope, a 1.3 M☉ dwarf would have been 46 dex fainter at 10 Gyr. A dwarf in the
    Debye regime really fades faster than Mestel's law, so the extension bounds L from above. Between
    sequences, the clock is linear in mass at fixed log L, and L(t) is that column inverted. Every
    column falls strictly, so the inverse exists and L falls with age at every mass. **Residuals**
    in log L at the models' ages: the 4,221 fitted models are within 0.009 dex (rms 0.0010); the
    1,046 held-out models are within 0.028 dex (rms 0.0020), the worst being 1.05 M☉'s last model,
    at 11.4 Gyr, in the Mestel extension.
  - _The 0.6 M☉ check_ runs at all 35 held-out models of `seq_060_thick.txt` from 8.8 Myr to
    10.2 Gyr, with T11's radius. T_eff is within 10% everywhere and within 5.5% at the worst,
    +5.4% at 8.8 Myr. All of that is the radius: the young model is 1.11 times T11's cold radius,
    and L is within 0.01 dex. The error is under 2.2% past 0.5 Gyr. Hurley and Shara's law was
    −20% at worst. Luminosities at held-out models of 0.2, 0.45, 0.9 and 1.3 M☉ are within 0.02 dex.
  - _Continuity in mass._ At 1 Myr–3 Gyr no step of 0.001 M☉ moves L by 0.015 dex. At 10 Gyr the
    steps reach 0.12 dex, smoothly, between 1.10 and 1.15 M☉, where one sequence is still plunging
    and the next has ended, below 2,000 K. Over 10⁻⁷ M☉ no step exceeds 10⁻⁴ dex, and L meets each
    sequence at its mass. Interpolation itself was checked by holding whole sequences out, 0.1 M☉
    apart, which is twice the table's spacing. Above 3,000 K it is within 0.05 dex from 0.45 to
    0.95 M☉. It is off by 0.07–0.19 dex below 0.45 M☉ at 10–50 Myr, where the sequences start at
    different luminosities, by 0.08–0.11 dex at 1.0–1.1 M☉, and by 0.3–1.8 dex at 1.15–1.25 M☉ in
    the Debye plunge.
  - _Where Montreal has no sequence._ **For the orchestrator to rule.**
    - Helium and oxygen–neon cores read the carbon–oxygen table at their law time × A ÷ A_CO,
      Mestel's heat-capacity scaling (Mestel 1952; the A of HPT §6.2.1 and HS03 §2). A is 4 for
      helium, 16.67 for HS03's 80:20 O:Ne by mass, and 13.71 for Montreal's 50:50 C:O (the harmonic
      mean, since the ions count). A helium dwarf therefore takes 3.4 times as long as a
      carbon–oxygen one to reach a given L, and an oxygen–neon one 0.82 times as long. Both factors
      come from the scaling alone. Detailed models agree in direction only: Althaus et al. (2013,
      A&A 557, A19) for helium, mostly by residual hydrogen burning, and Camisassa et al. (2022,
      MNRAS 511, 5198) for carbon–oxygen against oxygen–neon. Their sequences would do better, if
      their terms allow.
    - Masses outside 0.2–1.3 M☉ read the nearest sequence, which covers oxygen–neon dwarfs up to
      the 1.37 M☉ cap.
    - The law no longer reads Z. HS03's Z^0.4 was a fit, and the sequences take no progenitor
      metallicity. They omit its small real effects: residual hydrogen burning at low Z (Renedo et
      al. 2010) and ²²Ne sedimentation (Camisassa et al. 2016). So metal-poor dwarfs moved most: at Z = 0.001, +0.73 dex at 1 Gyr for a 1 M☉
      star's dwarf.
  - _The perturbation's target stays Hurley and Shara's law at t = 0._ **For the orchestrator to
    rule.** This is `formation_luminosity`, 23 L☉ for 0.6 M☉. The Montreal sequences start where
    their models were started (0.2 L☉ at 0.2 M☉, 56 L☉ at 0.6 M☉), not at formation, so they
    cannot stand in. Every track is therefore unchanged up to its death. `cooling_origin` inverts
    the new law at the star's last luminosity: a 0.6 M☉ dwarf starts 0.18 Myr into the law, and a
    brighter hand-over starts before zero, above −0.1 Myr. **Ruling 46.2 holds.**
    `a_white_dwarf_takes_over_at_its_stars_luminosity_and_then_fades` (1, 2, 3 M☉ CO, 7 M☉ ONe)
    and the light helium star's test still show under 10⁻⁶ dex at the hand-over. The origin test
    covers all three cores at 0.3–1.37 M☉ and from 10⁻⁴ to 5 × 10⁴ L☉, to 10⁻⁹.
  - _Outside the owned files._ `track/tests.rs` is `speed`'s. The fading test's per-stride floor
    goes from 0.8 to 0.7, with a comment. The 7 M☉ oxygen–neon dwarf's Debye plunge drops 25% in
    one stride at 5 Gyr, from 10⁻⁵ to 10⁻⁷ L☉ in 0.6 Gyr, as the 1.3 M☉ sequence does. Three doc
    comments in `track.rs`, `track/model.rs` and `track/tests.rs` now name the Montreal law. No
    code in `track/` changed: `luminosity`, `formation_luminosity` and `cooling_origin` keep their
    signatures, and `cooling_origin`'s `z` is `_z`, unread.
  - _Goldens (at 11; the orchestrator bumps to 12)._ Three moved, and only white dwarfs' values
    in them.
    - `stellar/white_dwarf_cooling`: 72 origins now invert the new law, and 63 `Montreal at`
      lines are new. The Hurley and Shara and HPT lines are unchanged.
    - `stellar/endings`: 32 values, the luminosity at +1 Myr and +1 Gyr of every white dwarf. At
      1 Gyr each is brighter by 0.17–0.90 dex. At 1 Myr the dwarfs of 1.0–1.37 M☉ are 0.15–0.67 dex
      fainter, and the lighter ones 0.29–0.70 dex brighter. Masses and deaths are unchanged.
    - `stellar/summaries`: the three white dwarfs' L, T_eff and B − V at their three times, and
      their Sion types (DA5.3 to DA4.5, DA8.2 to DA7.9, and DC11.0 to DA9.9, since the atmosphere
      draw's threshold moves with T_eff). Their masses, radii and ages are unchanged.
- **The integrator's speed, as optimised (round 8, `speed`; ruling 46).** No output bit moved: no
  golden changed, and a scratch fingerprint of 480 tracks under all four option sets (segments,
  knots, samples, fates, 60 states and maxima each, `to_age` prefixes, `lifetime`, `evolve`, doubled
  resolution) is identical before and after.
  - _Profile_ (in-process sampling at 2 kHz, and a count of every `math` call by site). `libm`'s
    `pow` is 74–77% of a track's time; one costs about 57 ns, 4–5 `math::exp`. A full 5 M☉ track made
    19,176 `pow` and 5,000 other transcendental calls; the rest of the integrator is under 15%.
    About a quarter of the calls repeated an earlier one's arguments. Before: core helium burning
    20%, the pulsing AGB 20%, the early AGB 15%, the maxima's samples 11% of a full track.
  - _What changed._ Only one side of a `min` of two power laws is evaluated where the crossing
    decides it (`coeffs::LesserPowerLaw`, `lesser_side`; equation 37's L, the radius scales of 46
    and 74, the hook's ΔL); the envelope integration evaluates each core mass once and shares it
    with the envelope, its rate, the progress and the interpulse period, and reads it from the
    evaluated state where that is the same number; a fixed luminosity's powers in a radius law are
    kept (`gb::LuminosityPowers`); core helium burning evaluates only the radius formulae its age
    needs; the rebuilt main sequences hand their lifetime to the integration; `(1 + X)^(5/3)` is a
    `ZCoeffs` constant; the electron-capture window's root is found only near the window; the peak
    search keeps its states; `Model::CoreHeliumBurning` is boxed. Each has a bit-equality test.
  - _Result_ (A/B in one process group at load 13–17 and 2.3–2.9 GHz, `math::exp` 11–12 ns; `pow`
    calls before → after): `lifetime` 4 M☉ 129k → 87k `exp` (14,750 → 11,018 `pow`), 20 M☉ 88k →
    66k; `to_age` of a 2 M☉ giant 55k → 39k; `full` of 1, 5 and 20 M☉ 194k, 260k, 160k → 137k,
    158k, 119k; `evolve` at 20 M☉ 4 Myr 42k → 41k. A factor of 1.4–1.75, still 5–8 times over the
    track targets, and `lifetime` 100–130 times over its 5 µs.
  - **For the orchestrator to rule: the targets cannot be met without moving output.** `lifetime`
    must equal `Track::lifetime` bit for bit, and so integrate the whole grid (the wind reads L and
    R at every step); 5 µs is about 650 `exp`, fewer than one phase's knots. Measured options:
    - `math::powf` as `exp(y ln x)` for finite positive x (every caller in the sim): tracks a
      further 1.1–1.8 times faster (`full` 20 M☉ ×0.57); worst moves over 280 stars: lifetime 1 ×
      10⁻¹¹, remnant mass 2 × 10⁻⁹ M☉, log L and log R 5 × 10⁻⁹ dex. Every golden would move in
      its last bits; a version-12 change.
    - `STEPS_PER_KNOT` 4 → 2: ×0.49; lifetime up to 1.5 × 10⁻³ (p99 1.7 × 10⁻⁴), remnant mass up to
      0.14 M☉, log L p99 7.5 × 10⁻⁴ dex but 0.18 at worst, log R up to 1.2 dex where a stripping
      burst moves. Fewer knots (8/16) is worse for less gain. Not recommended.
    - A fitted lifetime table through `hyperion-fit` for plan 08's bulk calls, which gives up
      equality with `Track::lifetime` and is the only route to about 5 µs.
- **`powf_positive` in the stellar formulae (round 8, `speed2`; ruling 77.1).**
  `math::powf_positive(x, y)` = `libm::exp(y × libm::log(x))`, debug-asserted to a finite positive x and a finite y. Its
  relative error is within 2⁻⁵² (1 + 1.5 |y ln x|) to first order (log and exp under an ulp each,
  one rounding of the product), 1.7 × 10⁻¹⁴ at |y ln x| = 50; a unit test checks 2⁻⁵² (2 + 1.5 |y
  ln x|) against `powf` over 2 × 10⁵ draws of x in 10⁻⁵–10⁸ and y in ±11.
  - _Where._ Every `pow` of `stellar/sse/` with a base that cannot reach zero, the winds and the
    envelope perturbation included. These keep `powf`: the zero-age main sequence's τ^η at τ = 0 (a
    branch), µ^b of the horizontal branch at µ = 0 (`envelope_power`), |M − a78|^a79, the
    core-helium-burning times' `complement` and τ_bl's `depth`, τ_bl^ξ and the blue loop's λ base,
    `mc_intermediate`'s fourth root (its sum can be negative below `M_HeF`, and the NaN falls to the
    cap), and the degenerate radius floor (1 + X)^(5/3), which `stellar::substellar` recomputes with
    `powf` and pins bit for bit (P06.T13). The relation-luminosity test no longer probes Mc = 0 and
    −0.1, outside the new domain.
  - _Deviations, as built._ Ruling 77.1 expected `speed`'s bit-equality tests to become tolerance
    tests against `powf`. None broke, so they stay bit-equality tests, and the tolerance against
    `powf` is tested once, in `math`'s unit test; the 280-star change below was measured by a
    scratch test and is not kept. `powf_positive` is `math`'s third hand-written function (plan 01
    lists `math` as thin `libm` wrappers; its module doc now names it). The orchestrator made ruling 77.3's
    corrections at merge: the Verification's lifetime target now names the fitted table, and
    P06.T34's range briefs are marked blocked.
  - _Bit-equality tests._ All of `speed`'s still hold bit for bit, because each shortcut and its
    unshortened form now call the same power: the lesser power law against the printed `min`, the
    relation luminosity, the radius law at cached powers, the main sequence alone, the gap and core
    helium burning at a current mass, the rebuilt main sequence's lifetime, the electron-capture
    window, and the fast `lifetime` against `Track::lifetime`. `CROSSING_MARGIN`'s 10⁻⁹ still
    decides: the laws' ratio there is at least 10⁻¹², against a power error under 2 × 10⁻¹⁴.
  - _Output change_ over `speed`'s 280 stars (80 on T12's grid, 200 random; default options):
    lifetime 1.0 × 10⁻¹¹ relative, remnant mass 2.4 × 10⁻⁹ M☉, log L 3.8 × 10⁻⁹ and log R 4.9 ×
    10⁻⁹ dex at 100 fractions of each life. At seven fractions of every phase the median is 2 ×
    10⁻¹⁴ dex and the 99th percentile 7.8 × 10⁻⁹, but the worst is 9.3 × 10⁻⁷ dex in L and 6.9 ×
    10⁻⁷ in R, all on short thermally pulsing AGBs (0.8 M☉ at Z = 0.004, 6.8 × 10⁴ years), where the
    phase's end moves with the envelope and L and R move steeply. No segment count changed, and
    T12.b passes unchanged.
  - _Speed_ (A/B of the two bench binaries in one lock, the base built with `powf_positive` as
    `libm::pow`; round two at load 5.1–5.2 and 2.8–2.9 GHz, `math::exp` 11 ns on both sides): `powf`
    8.0 `exp`, `powf_positive` 4.1; `lifetime` 4 M☉ 1.24 → 0.75 ms (×0.61), 20 M☉ 1.10 → 0.49 ms
    (×0.44); `to_age` of a 2 M☉ giant 445 → 288 µs (×0.65); `full` of 1, 5 and 20 M☉ 1.46, 1.68,
    1.13 → 0.93, 1.12, 0.63 ms (×0.64, ×0.67, ×0.56); `evolve` at 20 M☉ 4 Myr 371 → 168 µs
    (×0.45). Round one agreed within its noisier `exp`. Tracks remain 4–7 times over the 150 µs and
    60 µs targets.
  - _Goldens_ (re-blessed at 11; the bump waits for the version-12 batch). Largest relative change
    per golden: `stellar/sse` 2.6 × 10⁻¹⁴, `collapse` 3 × 10⁻¹⁶, `endings` 4.5 × 10⁻⁹, `summaries`
    3.1 × 10⁻¹⁰ (death times by 5 × 10⁻¹²), planetary `systems/subgiant` 1.9 × 10⁻¹⁶,
    `systems/red_giant` 1.7 × 10⁻⁹ (an engulfment a fraction of a second earlier), and
    `systems/fallback_black_hole` 1.3 × 10⁻⁷ in a position. `planetary/fate` moves most: a
    circumbinary orbit after the 20 M☉ supernova changes a by 2.3 × 10⁻⁷ and e by 1.2 × 10⁻⁵, as
    a nearly unbound orbit amplifies the remnant mass's change, and its mean anomaly by 4.8 × 10⁻³
    rad after some 2,300 orbits. Every golden outside the stellar stage and its planetary readers is
    unchanged.
