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
pub struct TrackOptions;                  // recipes, bridges, windows; Default is the generator's
impl TrackOptions { pub const fn with_capture_windows(self, w: CaptureWindowWidths) -> Self; }
pub struct CaptureWindowWidths;           // the electron-capture windows' widths (P15.T5.d)
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
    impl ReferencePopulation {            // a law's membership and windows (P15.T5.d)
        pub fn new(params: KickLawParams) -> Result<Self, BuildCaptureWindowWidthsError>;
    }
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
  `BH_FACTOR` (0.75), `EC_WINDOW_SINGLE` (0.1 M☉) and `EC_WINDOW_STRIPPED` (1.0 M☉). Plan 11's
  `CLUSTER_MERGED_BINARY_FATE`, the default P15.T5.c reviewed, is not among them: it stays in
  `stellar::binary` (plan 11's design note 14).
- `tables::helium`: `LIFETIME_SLOPE`, the coefficients of s(m, Z), a quadratic in log mass at four
  metallicities, interpolated in log Z; and `HB_TEMPERATURE_SHIFT`, linear in envelope mass, held at
  `HB_ENVELOPE_MAX` (the fit's upper envelope mass) and floored at zero (decision-p15-t7b-leans
  §8, applied by P15.T7.b's swap). Every coefficient zero is the identity.

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
                      HotSubdwarf, StrippedHeliumStar /* P06.T24.c */, WhiteDwarf, NeutronStar,
                      BlackHole, NoRemnant, Substellar }
```

`stellar::fates::TrackFates` implements plan 02's `galaxy::fates::StellarFates` for the mean-mass
quadrature. That trait takes a mass and no metallicity, so a `TrackFates` is built at one
metallicity, `TrackFates::at(fe_h: Dex)`, and plan 02's quadrature, which already runs once per
population, is handed the one for that population's reference [Fe/H] (P06.T30). `mean_companions`
delegates to plan 02's `ProvisionalFates` until plan 11.

### Range briefs (`stellar::brief`, `stellar::fates`, `tables::stellar_fates`; P06.T38, ruling 89)

```rust
// stellar::sse: the knot-free main sequence alone, bit-equal to Track::state_at (T38.b)
pub fn main_sequence_state(m0: SolarMasses, comp: &Composition, draws: &StarDraws, age: Years)
    -> Option<StarState>;              // None unless on a knot-free main sequence at `age`
// stellar::fates: one node of the fate table, one Track::full (T38.c), and its reader (T38.d)
pub struct FateNode { route: FateRoute, log_death_age, a, b }  // a, b: a white dwarf's mass
                                       // and cooling origin, or an iron core's M_CO and M_He
impl FateNode { pub fn of(m0: SolarMasses, comp: &Composition, eta: StandardNormal) -> Self; }
pub struct FittedFates<'t> { fe_h, panels: [FatePanel<'t>; 3], splits: [f64; 2] }
                                       // over tables::stellar_fates_{low,mid,high} (generator())
impl FittedFates<'_> {           // every η a StandardNormal draw
    pub fn lifetime_fitted(&self, m0: SolarMasses, comp: &Composition, eta) -> Option<Years>;
    pub fn lifetime_bracket(&self, m0: SolarMasses, comp: &Composition, eta)
                -> Option<(Years, Years)>;     // validated, not proved (P06.T38.d's tests)
    pub fn fate_fitted(&self, m0: SolarMasses, comp: &Composition, eta)
        -> Option<FittedFate>;         // route, remnant inputs, error bounds
}
// stellar::brief: per-row routing for a range query (T38.e)
pub struct BriefModel;                 // epoch state, time-independent, cacheable
impl BriefModel {
    pub fn new(galaxy: &Galaxy, record: &SystemRecord) -> Self;   // the generator's table
    pub fn with_fates(galaxy: &Galaxy, record: &SystemRecord, fates: &FittedFates) -> Self;
    pub fn brief_at(&self, t: UniverseTime) -> Option<StellarBrief>;
    pub fn stated_error_at(&self, t: UniverseTime) -> Option<StatedError>; // table rows only
    pub fn route(&self) -> BriefRoute;  // Table | MainSequence | Exact
    pub fn star_count(&self) -> u8;
    pub fn heap_bytes(&self) -> usize;
}
pub fn range_brief(galaxy: &Galaxy, record: &SystemRecord, t: UniverseTime)
    -> Option<StellarBrief>;
// stellar::multiplicity (plan 11's code; ruling 90): the count alone, equal to the full draw's
pub fn draw_star_count(galaxy: &Galaxy, record: &SystemRecord, ctx: MultiplicityContext,
    attempt: RedrawAttempt) -> u8;     // == draw_hierarchy(..).star_count()
```

Every `None` of the fast path and the reader sends the caller to the exact integrator. Plan 08's
placement and P06.T30.b read `FittedFates` too.

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
   fits. **Between HPT's seven calibration metallicities** (10⁻⁴, 3 × 10⁻⁴, 10⁻³, 0.004, 0.01, 0.02
   and 0.03, the Pols et al. 1998 models HPT fitted) the giant's and the asymptotic giant's radius
   laws (HPT equations 46 and 74) are evaluated at the calibration metallicities around Z, and
   log R is interpolated in log Z by a monotone cubic (ruling 92 of 2026-09-22). This departs from
   the printed form, which rulings 10, 29 and 40 otherwise follow: the Appendix's min and max clamps
   on b1–b3 switch between the calibration points and put a sawtooth into giants' radii (up to
   +0.22/−0.14 dex), temperatures (metal-poor giants up to 700 K cooler than solar ones) and white
   dwarf masses (±0.08 M☉, three reversals), which no detailed model has. At a calibration
   metallicity the laws are the printed ones, bit for bit, so T12's comparison with SSE holds.
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
    plan 11. Until then each star of m_cc(Z) − 1 M☉ or more (ruling 45.2) draws
    `Stripping::Companion` with probability `KickLawParams::stripped_share` (default 0.25, between
    the 20% and 33% mixes of the brainstorm's scratch Monte Carlo) on its own stream. The mark
    widens the electron-capture window and selects the low-mode ramp. It changes the track in the
    wide window (ruling 93.1): a marked star in [m_cc − 1, m_cc) dies by electron capture, not as a
    white dwarf, so the track reads `star.stripped`; its envelope is still a single star's. Plan
    08's kick loop keeps attempt 0's mark, so no attempt rebuilds the track (ruling 93.2). Plan 11 replaces the constant with the
    quadrature over periods and mass ratios and draws its binaries conditional on the mark, so the
    mark's stream and meaning are reserved now.
12. **Electron-capture windows are in initial mass** and end at the lowest initial mass that makes
    an iron core, `m_cc(Z)` (in Hurley's terms, the mass whose core at the base of the AGB is 2.25
    M☉): `[m_cc − 0.1, m_cc)` for a single star, `[m_cc − 1.0, m_cc)` for a companion-stripped one.
    The brainstorm gives widths only; see Risks. The widths are a track option since P15.T5.d,
    `TrackOptions::with_capture_windows` with `stellar::sse::CaptureWindowWidths`, and the
    generator's are `collapse`'s `SINGLE_STAR_WINDOW` and `COMPANION_STRIPPED_WINDOW`, which
    `TrackOptions::default()` carries.
13. **Neutron-star birth laws.** The brainstorm says birth spin and field "are drawn" and gives no
    distribution. The field is one log-normal with decay, so magnetars are the high tail of the
    birth field and fall out of the draw, as the brainstorm's row asks, and are not a separate roll.
    Both come from the decaying-field population synthesis of Popov et al. (2010, MNRAS 401, 2675),
    in the timing convention B = 3.2 × 10¹⁹ G √(P Ṗ) (ruling 110.1): log₁₀ B₀ at the equator
    normal about 12.95 with σ 0.6 (their polar 13.25 less log 2), and birth periods normal about
    250 ms with σ 100 ms. P06.T21.a re-checks that the pair reproduces the observed period and
    period-derivative plane, weighting the living pulsars by beaming × radio luminosity
    (Faucher-Giguère and Kaspi 2006, ApJ 643, 332): median P 0.4–0.9 s and median log Ṗ −15.2 to
    −14.2 (ATNF v2.6.1: 0.63 s, −14.66).
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
    open: circle (protostar, pre-main-sequence, dwarf, subgiant, hot subdwarf, stripped helium
    star from P06.T24.c, substellar), circle with an outer ring (giant, supergiant, Wolf-Rayet),
    diamond (white dwarf), triangle (neutron star), square (black hole). Plan 05's D14 already defines the outlines of the last three in
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
`m_c_bagb`; T19.a's law needs T18 and T1.b; T39 needs T14; T24.a needs T9, T10 and T39; T24.b needs T28.f; T26.c–d need
T27.c. T29.a, the `StarModel`, needs T10 and phase D; T26.d and every T28 kind take one, so they
wait for it. Within phase F, T28 needs T27 and T29.a; T28.a needs T25; T28.e needs T24.a; T28.g
needs the rest of T28. T29.b needs plan 03, T3 and B–F. T30 needs T10, T18 and P02.T9, not plan 03.
T31 and T32 need T29.b. Phase H needs T29.b; its protocol task can be written against the types of
T1 as soon as they exist; T34 also needs plan 04's P04.T14 (the range handler) and T35–T37 plan
05's P05.T9–T11 (the chart, list, readout and legend). T19.e is the only task that waits on another
plan (plan 15's P15.T5.a) and is done last; the plan is otherwise complete without it. T38, from
ruling 89, needs T29.b and ruling 77.1's `powf_positive`. T38.a completes T32's benches, and
T38.b–e need it. T38's subtasks run in order, and T38.a is a stop point. T34's range briefs, T35.b,
T37 and plan 13's P13.T7 wait on T38.e. Phase I holds rendering plan R06's accepted asks: T41
needs T38.e, T42 needs T10.b and T39, and T43 needs T20.a and T23.a; T42 and T43 each land their
pure part (a) before their swap (b).

Where a task's acceptance says only that its tests pass, the command is
`cargo test -p hyperion-sim -- <module paths of its Files>`, for instance
`cargo test -p hyperion-sim -- stellar::sse::hg stellar::sse::gb` for T6.

Every task that turns a figure into code re-checks it against the named source and cites it in the
doc comment. Every public item gets rustdoc with units and valid ranges, per `rust-dev.md`.

### Phase A: foundations

#### P06.T1 Module skeleton, state types, units and the normal quantile

- **P06.T1.a Module skeleton, state types and units.**
  Records: [06-stellar-stage/records/P06.T1.a.md](06-stellar-stage/records/P06.T1.a.md).
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
  Records: [06-stellar-stage/records/P06.T1.b.md](06-stellar-stage/records/P06.T1.b.md).
  - **Files:** `crates/hyperion-sim/src/math.rs`, `tests/foundation_golden.rs`, the golden.
  - **Tests:** the golden; `hyperion_testkit::stats::normal_cdf(normal_quantile(p))` returns p to
    10⁻¹² at 1,000 points; antisymmetry about ½ to 10⁻¹²; strictly increasing across both branch
    points.
  - **Accept:** `cargo test -p hyperion-sim math` and the foundation golden pass.

#### P06.T2 Per-star draws and reserved streams

Records: [06-stellar-stage/records/P06.T2.md](06-stellar-stage/records/P06.T2.md).

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

Records: [06-stellar-stage/records/P06.T3.md](06-stellar-stage/records/P06.T3.md).

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

Records: [06-stellar-stage/records/P06.T4.md](06-stellar-stage/records/P06.T4.md).

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
  `foundation` and `galaxy` have) in the crate's `Cargo.toml`, which T32 extends. _As built
  (ruling 92):_ `ZCoeffs::new` also fixes where Z falls among HPT's seven calibration
  metallicities (`ZCoeffs::radius_in_z`): within 10⁻¹² of one, relatively, it is `Calibrated` and
  the giant radius laws are the Appendix's at Z; otherwise a `ZBlend` holds the stencil (three
  nodes in the end intervals, four elsewhere), its spacings in ζ and the Hermite basis at ζ. The
  nodes' coefficients are literals (`sse::calibration::CALIBRATION`), held to `ZCoeffs::new` bit for
  bit by a test.
- **P06.T4.c ZAMS luminosity and radius.** `zams::luminosity(m, &ZCoeffs)` and `zams::radius` (Tout
  et al. 1996 equations 1 and 2). Files: `stellar/sse/zams.rs`. Tests: 1 M☉ at Z = 0.02 gives about
  0.70 L☉ and 0.89 R☉; both are continuous and L is monotone in mass over 0.1–100 M☉ for five
  metallicities. Accept: tests pass.

#### P06.T5 Main sequence

Records: [06-stellar-stage/records/P06.T5.md](06-stellar-stage/records/P06.T5.md).

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

Records: [06-stellar-stage/records/P06.T6.md](06-stellar-stage/records/P06.T6.md).

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
- **Radii between the calibration metallicities, as built (round 9, `zsmooth`; ruling 92).**
  Equation 46 (and T8.a's equation 74) is HPT's printed law only at the seven calibration
  metallicities; between them `gb::RadiusLaw` holds the law at each node of a three- or four-node
  stencil (`sse::calibration`) and ln R is the monotone piecewise cubic Hermite in log Z through
  their radii (design note 5): Brodlie's weighted harmonic mean of the secants inside (Fritsch and
  Butland 1984), `pchip`'s shape-preserving rule at the two ends (Moler 2004), with the basis fixed
  per Z and the slopes formed per evaluation, since the node radii depend on M and L. Tests: the laws are the printed ones bit for bit at every
  calibration metallicity; both are continuous in Z; the red giant at 1 M☉ and 100 L☉ cools
  monotonically with [Fe/H] (4,858 K at −2.2 to 4,123 K at +0.176). See Risks for the rest.

#### P06.T7 Core helium burning

Records: [06-stellar-stage/records/P06.T7.md](06-stellar-stage/records/P06.T7.md).

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

Records: [06-stellar-stage/records/P06.T8.md](06-stellar-stage/records/P06.T8.md).

- **P06.T8.a Early AGB:** core mass at the base of the AGB `m_c_bagb`, the helium and carbon–oxygen
  core growth, L from the core mass–luminosity relation, R_AGB. Second dredge-up at the end of the
  phase. _As built:_ R_AGB is interpolated in Z between HPT's calibration metallicities, each node's
  equation 74 with its own `M_HeF` (ruling 92; see T6).
- **P06.T8.b Thermally pulsing AGB:** core growth with third dredge-up (λ), L, R, and the three ends
  of the phase: envelope loss, the core reaching `m_c_sn` (supernova from the AGB, 1.6–2.25 M☉ cores
  at the base giving ONe white dwarfs or electron capture) or Chandrasekhar mass (HPT section 5.4).
  Expose `m_c_bagb` and the interpulse period as a function of core mass (T28.f needs both).
- **Files:** `stellar/sse/agb.rs`.
- **Tests:** continuity with T7 at the end of helium burning; with constant mass, a 1 M☉ core grows
  monotonically; `m_c_bagb` crosses 1.6 M☉ near 6.5 M☉ and 2.25 near 8 M☉ at Z = 0.02 (re-check).
- **Accept:** tests pass.

#### P06.T9 Naked helium stars

Records: [06-stellar-stage/records/P06.T9.md](06-stellar-stage/records/P06.T9.md).

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

Records: [06-stellar-stage/records/P06.T10.md](06-stellar-stage/records/P06.T10.md).

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

Records: [06-stellar-stage/records/P06.T11.md](06-stellar-stage/records/P06.T11.md).

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

Records: [06-stellar-stage/records/P06.T12.md](06-stellar-stage/records/P06.T12.md).

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

Records: [06-stellar-stage/records/P06.T13.md](06-stellar-stage/records/P06.T13.md).

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

Records: [06-stellar-stage/records/P06.T14.md](06-stellar-stage/records/P06.T14.md).

- **Build:** above 100 M☉ the formulae are evaluated as they stand, times correction factors for
  ZAMS luminosity, ZAMS radius and main-sequence lifetime, each a quadratic in log₁₀(m ÷ 100) that
  is exactly 1 at 100 M☉. The task picks one published grid of very massive star models that reaches
  150 M☉ at two metallicities or more (candidates: Yusof et al. 2013; Köhler et al. 2015), fits the
  three quadratics to it, and records grid, fit and residuals in the doc comment. The Eddington
  factor Γ_e at X = 0 (the least opacity a photosphere can have) is checked to stay below 0.75 on
  the main sequence at every metallicity (ruling 124.1); after the main sequence its excursions are
  flagged and pinned, and P06.T39's wind removes them.
- **Files:** `stellar/sse/vms.rs`; the tracks' tests in `stellar/sse/track/stages_tests.rs`.
- **Tests:** continuity at 100 M☉ to 10⁻¹²; L, R and lifetime at 120 and 150 M☉ within 10% of the
  chosen grid; lifetime stays above 2 Myr; count check in T31 (about a thousand alive at once).
- **Accept:** tests pass.
- _As built (round 9, `track06`)._ `stellar/sse/vms.rs`: `luminosity_factor`, `radius_factor` and
  `lifetime_factor`, each 1 + a x + b x² in x = log₁₀(m ÷ 100 M☉), exactly 1 at and below
  100 M☉. The grid is Yusof et al.'s (2013, MNRAS 433, 1114) non-rotating Geneva models, Tables 2
  (ZAMS log L, log T_eff) and 3 (τ_H), at 120 and 150 M☉ and Z = 0.014 and 0.006; it has no
  100 M☉ model, so each quadratic's two coefficients are least squares over the four points. Fits
  (a, b): L (0.0583, −0.901), worst residual 0.15%; R (−2.23, 8.97), 5.6% (the grid's own spread
  in Z); lifetime (−2.896, 8.99), 3.0%, held so that the lifetime still falls with mass to 150 M☉
  (the unconstrained fit dips to 0.765 at 138 M☉). The luminosity and radius factors enter
  `zams::luminosity` and `zams::radius` and so correct the zero-age main sequence alone (the
  terminal-age values are HPT's, so the correction fades along the main sequence and the
  Hertzsprung gap starts where HPT's does); the lifetime factor enters HPT's `t_BGB`, and with it
  `t_hook`, `t_MS` and the gap. `MAX_INITIAL_MASS` is 150 M☉, so `StarModel`, the binary engine,
  the kick law's reference population and `FateNode::of` no longer evolve 100–150 M☉ stars as
  100 M☉ ones, and the fate table's high panel has 72 masses to log₁₀ m₀ = 2.18. _Findings_ (see
  Risks): the grid differs from HPT by 8–16% in radius and 19–22% in lifetime already at 120 M☉,
  so the corrections act mostly between 100 and 120 M☉; the tracks' own main sequences, whose
  winds lengthen the effective-age clock, last 1–16% longer than τ_H (3.09 against 2.67 Myr at 120 M☉ and Z = 0.014); the Eddington factor passes 1 on
  the late main sequence at Z ≤ 0.001 above about 130 M☉, and after the main sequence at every
  metallicity. The count check waits for T31.

#### P06.T15 Protostars and the pre-main sequence

- **P06.T15.a Protostar phase.** For age < t_p = 0.5 Myr: m(t) = m_f × (1 − (1 − t ÷ t_p)²), which
  reaches half the final mass at 0.146 Myr, the observed length of Class 0 (Dunham et al. 2014;
  re-check). `ProtostarClass::{Class0, ClassI}` by whether the envelope (m_f − m) outweighs the
  star. L = photospheric (from T15.b at the current mass, on the birthline) + accretion, G m ṁ ÷ R.
  R on the birthline from a closed-form fit (Palla and Stahler 1999 as a starting point; record the
  choice).
- **P06.T15.b Contraction.** `t_zams(m, Z)`: the arrival time, a fit in log m to Baraffe et al.
  (2015) below 1.4 M☉ and to a Kelvin–Helmholtz time from the ZAMS values above (about 40 Myr at 1
  M☉, several hundred Myr at 0.2, under t_p above about 6–7 M☉ (ruling 124.6), in which case the star is on the main
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
  Records: [06-stellar-stage/records/P06.T15.c.md](06-stellar-stage/records/P06.T15.c.md).
- **Files:** `stellar/premain.rs`, `stellar/sse/track.rs`.
- **Tests:** continuity at t_p and at `t_zams` (values to 10⁻⁶, slopes to 5%); a 1 M☉ star at 2 Myr
  has 1–3 L☉ and 3,900–4,500 K; in a sample of the young disc (ages −H to 100 Myr) most stars below
  0.5 M☉ are pre-main-sequence, as the brainstorm states; stars of negative age are rejected by the
  type (`SystemExistence::NotYetBorn` is decided in T29).
- **Accept:** tests pass; the continuity sweep of T10.d now starts at age zero.
- _As built (round 9, `track06`; T15.a and T15.b)._ `stellar::premain` gains `PROTOSTAR_DURATION`
  (t_p = 0.5 Myr), `protostar_mass`, `ProtostarClass::{Class0, ClassI}`, `protostar_class` and
  `t_zams(m, &Composition)`, and the crate-private closed forms `Protostar` and `Contraction`
  that the track's two new leading segments evaluate (`Model::Protostar`,
  `Model::PreMainSequence`). Dunham et al. (2014) confirm 0.15–0.16 Myr for Class 0 and about
  0.5 Myr for Class 0+I.
  - _Arrival._ t_zams = g(m) G m² ÷ (R L) at HPT's zero-age main sequence of the star's Z, with
    log₁₀ g a cubic in log₁₀ m fitted to Baraffe et al.'s (2015) arrivals (log R within 0.01 dex
    of its minimum; 554 Myr at 0.1 M☉, 230 at 0.2, 140 at 0.5, 38.4 at 1, 17.4 at 1.4; residuals
    0.017 dex rms, 0.037 at most) and held at its 1.4 M☉ value above: the Kelvin–Helmholtz time,
    scaled. Against MIST it gives 15.2 against 13.9 Myr at 1.5 M☉, 0.81 against 1.0 at 5 and
    0.21 against 0.19 at 10; a 1 M☉ star at Z = 0.002 arrives at 19.6 Myr against MIST's 21.1 at
    [Fe/H] = −1. The arrival falls below t_p near 6 M☉, not 8.
  - _Birthline._ Palla and Stahler (1999) would not download; the table is Baraffe et al.'s 0.5 Myr
    radii and temperatures at 0.1–1.4 M☉ (the age at which their tracks start and accretion ends),
    Palla and Stahler's (1993, Table 1) birthline at 2–5 M☉ (with its deuterium swelling near
    4 M☉), and HPT's zero-age main sequence at 8 M☉, where Palla and Stahler (1990) say it meets
    the birthline; linear in log m between, held beyond.
  - _Protostar._ Its photosphere is the birthline's at its current mass, shifted by the accreted
    share w = m ÷ m_f times the contraction's own −⅓ log₁₀(t ÷ t_p) in log R, so that at t_p it
    meets the contraction in value and in slope; L adds G m ṁ ÷ R. The luminosity's slope at t_p
    does not match (the accretion stops there), which the test leaves out; R's does. The mass is
    held above 10⁻³ M☉ at the onset. A star that arrives before t_p accretes onto its main
    sequence: the protostar's photosphere ends at the main sequence's state at t_p, which starts
    from τ₀ = (t_p − t_zams) ÷ t_MS (held to 0.5), and it has no contraction segment.
  - _Contraction._ Hayashi at the birthline's temperature with R = R_birthline (t ÷ t_p)^−⅓
    (Baraffe et al.'s 1 M☉ star: 0.593 from 1 to 5 Myr, the law 0.585), then from the Henyey onset,
    at a share 0.687 − 0.7 log₁₀ m (held to 0.3–0.9) of the span in ln t (fitted to where
    Baraffe et al.'s tracks start to heat), a cubic Hermite blend in ln t of log L and log R to
    the main sequence's first state and slopes, which the contraction reads from the main
    sequence's segment, built first and handed on. A 1 M☉ star at 2 Myr has 1.27 L☉ and 4,397 K.
    **Deviation:** stars below 0.5 M☉ get the blend too, over the last 10% of the span in ln t,
    since their slopes must meet the main sequence's as well; the plan gives the Henyey segment to
    stars above 0.5 M☉ only. A star of negative age has no protostar class and no mass
    (`protostar_class` is `None`, `protostar_mass` zero), as `StarModel` has no state then.
  - _Seams._ HPT's clock starts at the main sequence's start: `turn_off_mass` inverts arrival +
    t_MS; `main_sequence_state` and `Track::knot_free_main_sequence` start at the arrival
    (`Track::built_from`), and the brief's main-sequence route needs the window's start past it;
    builds that keep no track skip both stages. Under `Bridges::Instant` (P06.T12.b's SSE
    comparison) there is neither stage: the track starts on the zero-age main sequence at age
    zero. **Deviation:** `max_radius_until` and `max_luminosity_until` count from the arrival
    (before it they are the present values): a contracting star's radius would otherwise engulf
    every ultra-short-period planet of plan 14 and fill plan 11's close companions' lobes, where
    binary codes start at the zero-age main sequence. For the same reason plan 11's engine
    (`stellar/binary/evolve.rs`, `arrival`) starts stepping where the first star has arrived, the
    pair detached before, and carries a star that has not yet arrived as its own zero-age
    main-sequence star (P11.T4.i, ruling p11-channels of 2026-10-06; it waited for both arrivals
    before).

#### P06.T16 The post-AGB bridge and planetary nebulae

- **P06.T16.a Bridge** (design note 8; ruling 124.2–3): a `PostAgb` segment after envelope loss
  from either AGB phase: log L falling linearly in time from the AGB's by 0.2939 − 0.8304 log₁₀ M_f
  dex, T_eff rising from the AGB value through 25,000 K to the knee, whose radius is ρ R_WD(0 K)
  with log₁₀ ρ = 0.1863 − 1.8306 log₁₀ M_f (both held at their 0.83 M☉ values above it), over the
  crossing time `t_cross(m_core)` (log₁₀ t = 2.0690 − 1.4443 log₁₀(M − 0.517), at most 10⁵ years),
  then the white dwarf's fade on Miller Bertolami's median shape to 10 L☉ over t₁ (log₁₀ t₁ =
  4.6189 − 3.9917 log₁₀ M_f, years; ruling 127.1), and the hand-over to T20's cooling law matched
  at 10 L☉, the white dwarf's radius R_WD[1 + (ρ − 1)(L ÷ L_knee)^0.4]. Removes the declared discontinuity left by T10.d. (The first form, constant
  L to the cold white dwarf's radius, is withdrawn by ruling 124.2: it knees 0.4 dex hot.)
- **P06.T16.b Nebula.** `nebula::planetary_nebula(&Track, age) -> Option<PlanetaryNebula>`: present
  when the death was `EnvelopeLoss` from the AGB, the central star is above 25,000 K, and the time
  since ejection is under the visibility time, R_max ÷ 1.3 v with R_max = 0.9 pc and v drawn once in
  20–40 km/s, the edge moving at 1.3 v (ruling 124.5: 16,900–33,900 years, the brainstorm's "some
  20,000"). Gives radius, expansion speed,
  age, ionised mass (a fixed fraction of the envelope lost in the last superwind knots) and an
  excitation class from the central star's temperature.
- **Files:** `stellar/sse/track.rs`, `stellar/nebula.rs`.
- **Tests:** continuity through the bridge; the knee's T_max within 0.1 dex of Miller Bertolami's
  (2016) 24 sequences and R ÷ R_WD at log L = 2 within 25% of theirs; no white dwarf of M_f ≤
  0.85 M☉ above 10^5.65 K; the fade from the knee to log L = 2 in 0.3–30 kyr for 0.58–0.83 M☉
  (ruling 127.1), the time to log L = 1 within 0.2 dex of each of Miller Bertolami's sequences,
  and L and R continuous at t₁;
  nebula radius never above 0.9 pc (2.94 ly); stars with cores below about 0.53 M☉ (0.527–0.534)
  show no nebula; at today's star formation the Galaxy's count is at least 3,500 (ruling 127.2:
  0.296 deaths a year × 0.70 × 19.6 kyr), 0.65–0.75 of the AGB deaths show a nebula, and each
  nebula's mean visible time is 16–26 kyr (ruling 124.4); the Galaxy's own count, 5,000–50,000 (expected
  20,000–40,000), waits for T31's sampler.
- **Accept:** tests pass.
- _As built (round 9, `track06`; T16.a)._ `stellar/sse/track/post_agb.rs`, `Model::PostAgb`,
  `Entry::PostAgb`. Under `Bridges::Physical` the thermally pulsing AGB keeps its giant's L and R
  to the loss of its envelope, without HPT §6.3's perturbation towards the white dwarf (which faded
  the star to about 20 L☉ over its last 10⁴–10⁵ years), and every white dwarf the AGB leaves
  (envelope loss, and the oxygen–neon cap) crosses first: L held at the AGB's last, log T_eff
  linear in time from the AGB's to 25,000 K at `ionising_years` and on to the knee at
  `crossing_years`, both power laws in the core mass fitted to Miller Bertolami's (2016) Table 3
  and CDS tracks at Z = 0.02 and 0.01 (log₁₀ t = 1.716 − 9.466 log₁₀ M, rms 0.28 dex, and 1.834 −
  7.867 log₁₀ M, rms 0.30, held to 95% of the crossing): 15 kyr at 0.55 M☉, 1.5 kyr at 0.7. The
  knee is where the star has the white dwarf's radius, so L and R are both continuous and the
  cooling law is matched to the crossing's L (ruling 46.2). The death, and the remnant's birth,
  are at the knee, so every AGB white dwarf dies 10²–10⁵ years later; `lifetime_of` and
  `remnant_of` reach the same death and the same last luminosity without the segment. Under
  `Bridges::Instant` the perturbation and the direct hand-over stand. _Findings:_ the knee is
  0.3–0.4 dex hotter than Miller Bertolami's highest temperatures, because the zero-temperature
  radius is 2–4 times smaller than a hot core's, and the fade after it runs on the Montreal law's
  extrapolation above its brightest node (a 1.27 M☉ oxygen–neon dwarf is 1.3 MK a century after);
  and without the perturbation the AGB's last winds act at the giant's radius, so white dwarfs are
  lighter, by 0.002–0.014 M☉ (0.5197 to 0.5121 M☉ at 1 M☉ and Z = 0.02, 0.5557 to 0.5418 at
  Z = 0.004), still within design note 9's Cummings et al. check.
- _As built (round 9, `track06`; T16.b)._ `stellar::nebula::{planetary_nebula, PlanetaryNebula,
MAX_RADIUS, IONISING_TEMPERATURE, MIN_EXPANSION_SPEED, MAX_EXPANSION_SPEED}`; the track keeps the
  star's `nebula` rank. The shell is the mass lost over the thermally pulsing AGB's last 16 of 31
  knot intervals (about 0.04 M☉ at 1 M☉, 0.25 at 2, 1 at 7), 30% of it ionised (Frew and Parker
  2010 give 0.005–3 M☉); the excitation class, 0–12, inverts Reid and Parker's (2010, eq. 5)
  log T_eff = 4.439 + 0.1174 E − 0.00172 E². _Deviations:_ the plan's "cores below about 0.53 M☉
  show no nebula" does not hold with Miller Bertolami's crossing: cores below 0.446–0.487 M☉ (for
  the slowest and fastest shells) show none, and his own 0.528 M☉ sequence at Z = 0.02 reaches
  25,000 K 13.6 kyr after the AGB (a finding, Risks); the Milky Way count uses a constant star
  formation of 1.65 M☉ a year and Kroupa's function in place of T31's sampler, not built, and
  counts deaths of 0.9–8 M☉ stars (a 0.8 M☉ star of solar metallicity does not die within the
  Galaxy's age), not 0.8–8: a mean visible time of 21,000 years (Jacob et al.'s
  2013 21,000 ± 5,000) and 6,200 nebulae, inside the plan's 5,000–50,000 but a third of the
  estimated 20,000, which the constant star formation's undercount of old stars may explain.
  Not wired: `SystemStars` and the server's `planetary_nebula` stay `NotModelled`
  (`srv/convert/stellar.rs` is not this lane's).
- _As built after ruling 127 (round 9, `track06`), superseding the bullets above and below where
  they differ._ `post_agb::{fade_years, fade_log_l, bridged_origin}`: a white dwarf off the bridge
  fades on MB16's median shape from its knee to 10 L☉ over t₁, and its Montreal law starts at its
  10 L☉ point at t₁ (`bridged_origin`, `cooling_origin` at 10 L☉ less t₁). The remnant record
  carries the knee's log L (`RemnantModel::knee`, `Model::Remnant { knee }`), which the radius's
  inflation reads, so the "negative origin" test of the bullet below is gone. The fate table gains
  two routes, `BridgedCarbonOxygenWhiteDwarf` and `BridgedOxygenNeonWhiteDwarf` (codes 6 and 7),
  whose `b` is the knee's log L; the origin is rebuilt from the mass. `fates::MIN_ORIGIN_MYR` is
  removed: only bridged dwarfs had negative origins, and they no longer store one, so unbridged
  dwarfs are held at zero as before P06.T16. `lifetime_of` and `remnant_of` reach the same knee
  and origin. The fade from the knee to log L = 2 takes 10^−1.53 t₁, 2.6–10.8 kyr at 0.83–0.58 M☉, inside ruling 127.1's
  0.3–30 kyr. **Not carried:** plan 11's engine, when a companion feeds a bridged white dwarf
  (`remnant_clock`), runs it on the Montreal law from the 10 L☉ origin, without the fade.
- _As built after ruling 124 (round 9, `track06`), superseding the two bullets above where they
  differ._
  - _The knee (124.2)._ `post_agb::{knee_fade_dex, knee_inflation, inflated_radius}`: the crossing
    fades in log L linearly in time by Δ, heats through 25,000 K to the knee at ρ R_WD, and the
    white dwarf's radius is R_WD[1 + (ρ − 1)(L ÷ L_knee)^0.4] (`model.rs`,
    `young_white_dwarf_radius`), L_knee the cooling law's luminosity at its origin. A white dwarf is
    taken to have left a knee when its origin is negative under the default recipe (the law was
    matched above its brightest node), which the fate table's remnant also carries, so the table
    route needs no new column; helium dwarfs and `Hurley2000` keep the cold radius. Tests: the
    knee's T_max within 0.1 dex at all 24 of MB16's sequences, R ÷ R_WD at log L = 2 within 25%, no
    dwarf of ≤ 0.85 M☉ above 10^5.65 K. **A finding against the ruling's fade test:** the fade
    from the knee to log L = 2 takes 46–102 kyr for 0.58–0.80 M☉ dwarfs, against its 0.3–10 kyr
    (MB16 0.85–6.9): above 10^2.5 L☉ the Montreal law runs on its log–log extrapolation towards its
    −0.1 Myr pole. The test pins 10–200 kyr.
  - _The crossing (124.3)._ `crossing_years` and `ionising_years` are offset power laws, log₁₀ t =
    a + b log₁₀(M − M₀) in years: (0.517, 2.0690, −1.4443), held at 10⁵ years, and (0.519,
    2.1550, −1.1362), held at 95% of the crossing. The cut-off is 0.527–0.534 M☉; HYPERION's
    1 M☉ solar star (a 0.512 M☉ core) shows no nebula.
  - _The nebula (124.5)._ `MAX_RADIUS` is 0.9 pc (2.94 ly) and `EDGE_SPEED_FACTOR` 1.3: the edge
    moves at 1.3 v, so a nebula is visible for 16.9–33.9 kyr.
  - _The count (124.4)._ The constant-rate count is a floor with a mean visible time of 16–26 kyr:
    measured 19,600 years, but **4,050 nebulae, under the ruling's floor of 5,000** (a finding): 72
    of the 240 stars, those of about 0.9–1.1 M☉ with the lighter cores rulings 92 and 99 accepted,
    are lazy under 124.3. The test asserts 3,500 until that is ruled.
  - _The Eddington factor (124.1)._ `very_massive_main_sequences_stay_below_the_eddington_limit`
    reads Γ_e at X = 0 (below 0.75), and `post_main_sequence_eddington_excursions_are_pinned` pins
    the maxima (within 20%) and times above 1 (within a factor of two) of the research's six tracks,
    for P06.T39 to remove.
  - _The blend below 0.5 M☉ (124.7)._ `the_blend_below_half_a_solar_mass_stays_near_the_hayashi_temperature`:
    within 200 K and 7% of the birthline temperature at Z = 0.02, and of the range between the
    birthline's and the zero-age main sequence's at lower Z (whose zero-age main sequence is up to
    300 K hotter).

#### P06.T17 The helium-excess hook

- **Build:** `Composition::helium_excess` enters `Track` through `tables::helium`, in the shape
  under Provides, which plan 15's P15.T7 restates: the main-sequence and giant-branch timescales are
  multiplied by exp(s(m, Z) × ΔY), with s from `LIFETIME_SLOPE` (a quadratic in log mass at four
  metallicities, interpolated in log Z), and log T_eff on the horizontal branch is shifted by
  `HB_TEMPERATURE_SHIFT` (linear in envelope mass) × ΔY, with the radius adjusted to keep L. This
  task commits `tables/helium.rs` with every coefficient zero, README's header, and marked
  provisional. The code path applies the correction only when ΔY > 0, so grid stars are
  bit-identical whatever plan 15 later fits. Document the expected signs for plan 15: ΔY = 0.1
  shortens the lifetime of a 0.8 M☉ star by about a half (0.42–0.55; decision-p15-t7b-leans L3,
  which retired the earlier estimate of a third) and moves the horizontal branch bluewards.
- **Files:** `stellar/composition.rs`, `stellar/sse/track.rs`,
  `crates/hyperion-sim/src/tables/helium.rs`, `tables/mod.rs`.
- **Tests:** ΔY = 0 gives bit-identical tracks with and without the hook compiled in (golden); a
  test-only non-identity table changes lifetime and horizontal-branch temperature in the stated
  directions.
- **Accept:** tests pass; plan 15 can replace the table file alone.
- _As built (round 9, `track06`)._ `tables/helium.rs` (`LOG_Z_NODES` at [Fe/H] −2.2, −1.6, −1.0,
  −0.5; `LIFETIME_SLOPE`, `HB_TEMPERATURE_SHIFT`, all zero), committed by hand and marked
  provisional, not in `MANIFEST` or the lock (P15.T7 registers it). `stellar/sse/track/excess.rs`:
  `HeliumTable`, and `HeliumHook::of` built only for ΔY > 0. The main sequence's lifetime (in
  both its duration and the rebuilt effective-age rate, bit for bit) and the Hertzsprung gap's and
  first giant branch's spans (`Span::stretched`) take exp(s ΔY), s at the mass the phase's
  formulae read; core helium burning's log T_eff takes the shift at constant L; `turn_off_mass`
  reads the factor too. Tests: at ΔY = 0 a track is bit-identical whatever the table, and the
  committed table is the identity at ΔY = 0.1 too; a test table (s = −4, shift 0.5) shortens a
  0.8 M☉ star's life at [Fe/H] = −1.5 by a third and heats its horizontal branch.

### Phase D: remnants and kicks

#### P06.T18 Remnant type and mass (Mandel and Müller 2020)

Records: [06-stellar-stage/records/P06.T18.md](06-stellar-stage/records/P06.T18.md).

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
  Records: [06-stellar-stage/records/P06.T18.d.md](06-stellar-stage/records/P06.T18.d.md).
- **Files:** `stellar/remnant/collapse.rs`.
- **Tests:** over a Kroupa sample of 8–150 M☉ at Z = 0.02: black holes are 38 ± 5% of compact
  remnants (the research figure behind "about four fifths" of layer-E remnants staying); black holes
  of 2–5 M☉ exist; complete fallback is 70–80% of black holes; neutron star masses lie in 1.13–2.0;
  the type is monotone in the uniform draw (a lower draw never turns a black hole into a neutron
  star); the electron-capture share of single-star neutron stars is 2–6%.
- **Accept:** `cargo test -p hyperion-sim stellar::remnant::collapse` passes.

#### P06.T19 The kick law

Records: [06-stellar-stage/records/P06.T19.md](06-stellar-stage/records/P06.T19.md).

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
  Since ruling 127.1 a white dwarf off the bridge follows Miller Bertolami's fade to 10 L☉ first,
  and this law is matched there (Bédard et al. 2020 warn that cooling ages under 10⁵ years depend
  on their initial models); the law itself is unchanged.
  Check against the 0.6 M☉ CO thick-hydrogen sequence of Bédard et al. (2020), at models held out
  of the fit: T_eff within 10% from 0.01 to 10 Gyr; and continuity in mass across the fitted range.
  Records: [06-stellar-stage/records/P06.T20.a.md](06-stellar-stage/records/P06.T20.a.md).
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

Records: [06-stellar-stage/records/P06.T21.md](06-stellar-stage/records/P06.T21.md).

- **P06.T21.a Birth draws.** Spin period normal about 250 ms with σ 100 ms, redrawn until above 10
  ms (`star.ns.spin`); log₁₀ of the equatorial dipole field in gauss normal about 12.95 with σ 0.6
  (`star.ns.field`); magnetic inclination and spin axis isotropic (`star.ns.geometry`). Both from
  Popov et al. (2010), their polar field less log 2 (ruling 110.1).
- **P06.T21.b Spin-down in closed form.** Field decay B(t) = B₀ ÷ (1 + t ÷ τ_d) with τ_d = 10⁴ yr ×
  (10¹⁵ G ÷ B₀), to a floor of 10¹² G or B₀ if lower (Colpi, Geppert and Page 2000, ApJ 529, L29,
  eq. 3 with α = 1, normalised to Beniamini et al. 2019's τ_B = 10⁴ yr; ruling 110.4). Magnetic
  dipole braking P Ṗ = k B², k in the timing convention (R = 10 km, I = 10⁴⁵ g cm², so k = 9.77 ×
  10⁻⁴⁰ s G⁻²; ruling 110.1, which keeps 12.2 km for the decay luminosity), gives P(t)² = P₀² +
  2k ∫B² dt, and the integral of this decay law is elementary. Mean glitch activity is included as
  a factor (1 − 0.01) on ν̇ while |ν̇| < 10⁻¹⁰·⁵ Hz s⁻¹, for pulsars and magnetars alike (Fuentes
  et al. 2017, A&A 608, A131, §5; ruling 110.3), so glitches leave no mark that needs replaying.
  `PulsarState` at any age: period, period derivative, field, characteristic age, spin-down
  luminosity and whether the pulsar is alive.
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
  Records: [06-stellar-stage/records/P06.T21.e.md](06-stellar-stage/records/P06.T21.e.md).
- **Files:** `stellar/remnant/neutron_star.rs`.
- **Tests:** P is continuous and non-decreasing in age; with constant field the closed form equals
  √(P₀² + 2kB²t); a 10¹²·⁵ G pulsar born at 300 ms dies after 10⁷–10⁸ years; the share of neutron
  stars born above 4.4 × 10¹³ G is 8–40% (the draw of T21.a's tail holds 12.4%, 12.2% measured; Popov et al. 2010 "about
  10%"; ruling 110.1), none of them counts as a magnetar by T21.c's second condition before
  spin-down has slowed it, and the expected number of active magnetars at two core collapses a
  century is 20–300 (about 90; ruling 110.2); design note 13's weighted medians lie in their
  windows; the beaming fraction at 1 s is 10–20% over random
  directions; the pulsar wind nebula flag lasts 10³–10⁵·⁵ years across the birth distribution.
- **Accept:** tests pass.

#### P06.T22 Black holes

Records: [06-stellar-stage/records/P06.T22.md](06-stellar-stage/records/P06.T22.md).

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

Records: [06-stellar-stage/records/P06.T23.md](06-stellar-stage/records/P06.T23.md).

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

#### P06.T39 Winds near the Eddington limit (ruling 124.1)

Records: [06-stellar-stage/records/P06.T39.md](06-stellar-stage/records/P06.T39.md).

Runs before P06.T24.a, whose WNh class it feeds.

- **Build:** a wind term that grows with the electron-scattering Eddington factor Γ_e (at the
  star's surface hydrogen, X = 0 as the bound): continuous with the recipe's rate at Γ_e = 0.7, and
  above it log Ṁ steepening as 3.99 log Γ_e + 0.78 log L (Vink et al. 2011, as Gräfener et al.
  2011, A&A 535, A56, eq. 7, quote it). It strips the stripping instant's last envelope (Γ_e(X = 0) of 1.03–1.19 for 11–46 kyr today) in decades and the metal-poor 150 M☉ red supergiants'
  (log L 6.9–7.7, up to 8.0 for 37–88 kyr) in a few thousand years, handing over to the helium-star
  laws, which is Yusof et al.'s (2013) endpoint. T24.a reads Γ_e for its WNh stars, not the surface
  composition (Gräfener et al. 2011: "the Wolf-Rayet stage should be identified by large Eddington
  parameters").
- **Files:** `stellar/sse/wind.rs`, `stellar/sse/track*`.
- **Tests:** no living state spends over 10³ years at Γ_e(X = 0) > 1, over 100–150 M☉ at five
  metallicities and 80 M☉ at Z = 10⁻⁴; continuity of the rate at Γ_e = 0.7; the pinned excursions
  of `post_main_sequence_eddington_excursions_are_pinned` replaced by this test.
- **Accept:** tests pass. **Moves output** (the post-main-sequence evolution of stars above about
  80 M☉, low-Z remnants, the fate table's high panel and the kick rank table): its own bump.
- **Done (round 9, `events06a`), provisionally at version 15 for the orchestrator's v16 batch.**
  `sse::wind::{eddington_factor, EDDINGTON_KINK}` and the steepened modern rate; the test is
  `the_eddington_wind_bounds_the_excursions_or_pins_them`, which misses its bound below Z ≈ 0.006
  and pins those rows (see "P06.T39, T24.a, T26.d and T31, as built" under Risks).

#### P06.T24 Classes beyond the MK grid

`PeculiarClass` and the rules, all derived from state and track, none rolled. T24.a covers the
massive and stripped stars, T24.b the rest, and T24.c corrects T24.a's hot subdwarfs.

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
- **P06.T24.b Carbon and S stars, and young stars** (needs T28.f). **Built (lane
  `p06-t24b-t28g`, 2026-10-31), held at 26 (`PECULIAR_CLASSES_VERSION`).**
  Records: [06-stellar-stage/records/P06.T24.b.md](06-stellar-stage/records/P06.T24.b.md).
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
- **P06.T24.c The stripped helium stars** (a technical correction to T24.a,
  `decision-stripped-star-class.md` of 2026-10-21; the brainstorm sets no edge for subdwarfs).
  Records: [06-stellar-stage/records/P06.T24.c.md](06-stellar-stage/records/P06.T24.c.md).
  - Hot subdwarfs end at 1.7 M☉ (`HOT_SUBDWARF_MAX_MASS`), an edge in mass: Götberg et al. (2018,
    A&A 615, A78) find spectra "undistinguishable from subdwarfs" only up to 1.7 M☉, and Drout et
    al. (2023, Science 382, 1287) put the subdwarfs at ≲ 1.5 M☉ and the "intermediate-mass helium
    stars" at 2–8 M☉. Provisional at every Z: Götberg's grid is at Z = 0.014.
  - A naked helium star above the edge, from 20,000 K and at or below the Wolf-Rayet floor, is a
    stripped helium star: `He-sdO` from 40,000 K and `He-sdB` below, on the hot subdwarfs' letter
    rule (Götberg et al. 2018, sections 4 and 4.2; Heber 2016, section 2), with no `f`. Its kind
    is `STRIPPED HELIUM STAR`, a circle on the chart, sent as `stripped_helium_star`
    (`PROTOCOL_VERSION` 2 → 3, since `ObjectKindDto` is closed). The Wolf-Rayet floor stays its
    upper end. The classifier tries Wolf-Rayet, then hot subdwarf, then stripped helium star.
  - Tests: 1.6 and 1.8 M☉ on the helium main sequence are a hot subdwarf and a stripped helium
    star; a 2.3 M☉ helium star at about 65,000 K is `He-sdO3`; above the floor the Wolf-Rayet class
    is unchanged; a 0.5 M☉ helium star is still `sdB`; look-72's `820E6EE00000008D` primary is a
    stripped helium star in its observed and now summaries.
  - Accept (T24.c, beyond the line below):
    `cargo test -p hyperion-sim stellar::system stellar::state`,
    `cargo test -p hyperion-sim --test observe_brief`,
    `cargo test -p hyperion-protocol object_kind_strings protocol_version_is_three`, and the
    client's vitest files that list every kind.
- **Files:** `stellar/classify/peculiar.rs`.
- **Accept:** `cargo test -p hyperion-sim stellar::classify::peculiar` passes after each subtask.
- **T24.a done (round 9, `events06a`).** The deviations and findings are under Risks ("P06.T39,
  T24.a, T26.d and T31, as built"). T24.b's T28.f is done (2026-10-28): its n is
  `thermal_pulse::pulse_number_at`, the pulses since the first. **T24.c done (lane `stripped-class`,
  2026-10-21; at version 22, no bump):** see Risks, "Deviations in T24.c, as built".

#### P06.T25 Rotation and magnetism

Records: [06-stellar-stage/records/P06.T25.md](06-stellar-stage/records/P06.T25.md).

- **Build:** one draw each, as the brainstorm's row says, plus the spin axis (`star.spin_axis`).
  - `u_rot` (`star.rotation`) is a rank. Above 1.3 M☉ it maps onto the equatorial velocity
    distribution for the mass (bimodal for late B and A stars; Zorec and Royer 2012; record), as a
    fraction of critical velocity from the state's mass and radius, conserved through the main
    sequence. Below 1.3 M☉ it maps onto the initial period spread, which converges by Skumanich
    braking, Skumanich-like, P_rot ∝ t^0.566, MH08's fitted exponent, with their colour-dependent
    coefficient (Mamajek and Hillenbrand 2008, Table 10, whole; ruling 110.6),
    with saturation for fully convective stars. Both are closed forms in age.
  - `u_mag` (`star.magnetism`): a fossil field in main-sequence stars from 1.4 M☉, with an
    incidence rising with mass: 0.5% at 1.4–1.8 M☉, linear to 11% at 3.6 M☉, flat to 5 M☉, then
    down to 7% at 15 M☉ and above (Sikora et al. 2019, MNRAS 483, 2300, §6; Grunhut et al. 2017,
    MNRAS 465, 2432; ruling 110.5), with a log-normal strength; these are forced to the slow
    rotation mode.
  - Derived: `Be` for B-type main-sequence stars above 0.7 of critical; `Ap`/`Bp` for fossil-field
    stars of 7,000–20,000 K; `Am` for non-magnetic A stars (7,000–10,000 K) under 120 km/s (as a
    single-star stand-in for tidal braking; plan 11 adds the binaries); `ActivityLevel` from the
    Rossby number with the convective turnover time of Wright et al. (2011): saturated L_X ÷ L_bol =
    10⁻³ below Ro = 0.13, falling as Ro^(−2.7) above. The flare rate of T28.a reads the activity.
- **Files:** `stellar/rotation.rs`.
- **Tests:** the Sun's age and mass give a 22–30 day period and a low activity level for the median
  draw; Be stars are 10–25% of B-type main-sequence stars; Ap and Bp 1–3% of main-sequence stars
  of 1.4–5 M☉ (Sikora et al.'s 52 of 3,254; ruling 110.5);
  rotation period continuous and rising with age for cool dwarfs.
- **Accept:** tests pass.

#### P06.T26 Variability

Records: [06-stellar-stage/records/P06.T26.md](06-stellar-stage/records/P06.T26.md).

- **P06.T26.a The instability strip.** Blue and red edges as lines in (log T_eff, log L) (record the
  source). A star inside pulsates, named by phase and mass: δ Scuti (main sequence and Hertzsprung
  gap, 1.5–2.5 M☉), RR Lyrae (low-mass core helium burning), classical Cepheid (core helium burning
  blue loop or gap crossing, 3–20 M☉), type II Cepheid (post-horizontal-branch and AGB low-mass
  stars: BL Her, W Vir, RV Tau by period). Period from mean density, P = Q × (ρ̄ ÷ ρ̄☉)^(−½), with Q
  by kind (0.033–0.04 d), amplitude zero at the edges, so variability switches on and off
  continuously as a star crosses. A classical Cepheid's amplitude peaks 300 K inside the blue edge
  and falls linearly to the red (Bono, Castellani and Marconi 2000, ApJ 529, 293, §4; ruling 110.7);
  the other kinds' peak mid-strip.
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
- **T26.d done (round 9, `events06a`)**: `variability::light_factor_at(star, cycles, t)`, with
  the star's `star.var.cycle` series passed in beside it (see Risks).
- **Files:** `stellar/variability.rs`.
- **Tests:** a 5 M☉ blue-loop star gets a Cepheid period of 3–10 days, and the sample's Cepheids
  follow a period–luminosity slope within 15% of the observed one; RR Lyrae periods of 0.3–0.9 days;
  δ Scuti 0.02–0.3 days; Mira periods of 150–600 days; `light_factor_at` is continuous in t and the
  same whatever order times are asked in; a star just outside the strip has amplitude zero.
- **Accept:** tests pass.

### Phase F: events in time

#### P06.T27 Generic event machinery (`hyperion_sim::events`)

Records: [06-stellar-stage/records/P06.T27.md](06-stellar-stage/records/P06.T27.md).

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
  Records: [06-stellar-stage/records/P06.T28.a.md](06-stellar-stage/records/P06.T28.a.md).
- **P06.T28.b Glitches.** Crab-like: Poisson bins (Δ = 30 days), rate proportional to |ν̇| for
  pulsars younger than 10⁷ years, sizes from a power law, a recovery that decays within the
  look-back. Vela-like (characteristic age 10⁴–10⁵ years and one fixed mark): monotone phase with P
  of about three years scaled by |ν̇|, narrow size distribution near Δν ÷ ν = 10⁻⁶. The frequency
  offset is a sawtooth in the phase's fractional part with zero mean, so spin-down stays the closed
  form of T21.b. **T28.b done (lane `23-p06-t28b-glitches`, 2026-10-15; at version 22, no bump: new
  output on the two glitch tags and the new domain tag `star.ns.glitch_class`, moving nothing
  existing).** The record is under Risks ("Deviations in T28.b, as built" and "Deferred findings
  (2026-10-13 rule) (P06.T28.b)").
  Records: [06-stellar-stage/records/P06.T28.b.md](06-stellar-stage/records/P06.T28.b.md).
- **P06.T28.c Magnetar bursts and giant flares** (rates ruled 2026-10-27, decision-galaxy-25-train
  §5.1; new output on the registered tags 0x0103–0x0105, moving nothing existing, so no bump).
  Every rate is zero unless `PulsarState::is_magnetar` holds, and zero once the field sits at its
  floor. Each scales with the field's fractional decay rate: an outburst releases crustal stress
  that the decaying field builds, its energy scales with B², and the field releases energy at
  L_B ∝ B |Ḃ|, so the event rate per magnetar scales as |Ḃ| ÷ B. Under T21's law (Colpi et al.
  2000, α = 1, with B₀τ = 10¹⁹ G yr by ruling 110.4) that is |d ln B ÷ dt| = 1 ÷ (τ + t) =
  B(t) ÷ (10¹⁹ G yr): it falls with the decaying field, and is higher for the stronger fields.
  The figures marked [S] were cited from memory in the ruling: the task's science check reads each
  at source before commit, and a source that differs moves the figure to the source's value
  within the same form, recorded, with no new ruling unless it is a blocker.
  Records: [06-stellar-stage/records/P06.T28.c.md](06-stellar-stage/records/P06.T28.c.md).
  - **Episodes** (`star.ev.magnetar_episode`, Poisson bins, Δ = 30 days):
    r_ep = N_EP × B(t) ÷ (10¹⁹ G yr), with N_EP = 2,000 episodes per e-fold of the field (ours).
    That is 0.2 a year at 10¹⁵ G, 0.02 at 10¹⁴ G and 0.009 at 4.4 × 10¹³ G.
    - The check: Coti Zelati et al. 2018 (MNRAS 474, 961) catalogue 23 outbursts from 17
      magnetars over 1998–2016 [S], about 0.05 per magnetar-year among the ~30 known, whose
      dipole fields lie mostly between 10¹⁴ and 10¹⁵ G (Olausen and Kaspi 2014's McGill catalogue
      [S]). The law gives 0.05 at 2.5 × 10¹⁴ G.
    - The bound per bin is the rate at the bin's start, since the field only falls, thinned to
      zero where the star is not a magnetar.
    - Marks: the episode's burst count N_b, log-uniform on [3, 1,000] (ours; median 55, mean
      172), against over 400 bursts in a day from SGR J1550−5418 in January 2009 (Kaneko et al.
      2010 and van der Horst et al. 2012 [S]), hundreds from SGR 1935+2154 in April 2020 (Younes
      et al. 2020 [S]) and about 440 bursts in 5 years of Fermi GBM, nearly all from 8 magnetars
      (Collazzi et al. 2015 [S]); its e-folding τ_b, log-uniform on 0.3–10 days (ours).
    - Recorded, not tested: an episode's energy, about R³B² ÷ (3 N_EP), roughly 10⁴⁴ erg at
      5 × 10¹⁴ G, above X-ray outbursts' 10⁴⁰–10⁴³ erg: a radiative efficiency of 10⁻⁴–10⁻¹,
      plausible.
  - **Short bursts** (`star.ev.magnetar_burst`, Poisson bins, Δ = 1 hour): inside an episode,
    r_b(Δt) = (N_b ÷ τ_b) e^(−Δt ÷ τ_b) for Δt ≥ 0 since its onset. Each hour bin looks back over
    the 30-day bins whose onsets lie within 10 τ_b,max (100 days, so 4 bins), and the episodes are
    regenerated from their own series, which is pure: no event list is ever an input. The bound is
    the sum, over the looked-back episodes, of N_b ÷ τ_b at the bin's start. Marks: energy
    dN/dE ∝ E^(−5/3) on 10³⁷–10⁴¹ erg (Göğüş et al. 1999, ApJ 526, L93; 2000, ApJ 532, L121;
    Cheng et al. 1996, Nature 382, 518 [S]); duration log-normal about 0.1 s, σ 0.35 dex (Göğüş et
    al. 2001, ApJ 558, 228, §3.1 [S]; the ruling's 0.3, moved at source 2026-10-29).
  - **Giant flares** (`star.ev.magnetar_giant`, Poisson bins, Δ = 1 Julian year):
    r_gf = N_GF × B(t) ÷ (10¹⁹ G yr).
    - N_GF is derived by the task from the model's own Milky Way census of active magnetars
      (T21's, 84), so that the Galaxy's rate is 0.044 a year: two Galactic giant flares, SGR
      1900+14 (1998-08-27) and SGR 1806−20 (2004-12-27), in about 45 years of all-sky coverage
      since 1979 [S]. SGR 0526−66's 1979 flare was the LMC's, and is not counted. The expected
      N_GF is about 30, from Σ |d ln B ÷ dt| ≈ 84 × 1.5 × 10⁻⁵ a year, which gives a 10¹⁵ G
      magnetar one flare in about 330 years.
    - This replaces "about one per 50–100 years per young magnetar; re-check", which describes
      only the youngest, and over the population would overshoot the Galactic record three to
      five times.
    - Energy: dN/dE ∝ E^(−1.7) [S] on 10⁴⁴–10⁴⁷ erg. Cross-check, recorded: Burns et al. 2021
      (ApJL 907, L28), a volumetric rate of about 3.8 × 10⁵ Gpc⁻³ yr⁻¹ above 4 × 10⁴⁴ erg [S].
  - **Tests**, besides T28's shared tests below: no episode, burst or giant flare while the star
    is not a magnetar; each rate non-increasing in age while it is one; every burst after its
    episode's onset and within 10 τ_b of it; the Galaxy's giant-flare rate at Milky Way values
    within 0.02–0.09 a year; energies inside their ranges; each [S] figure read at source, with
    its basis in its doc comment.
  - T28.c lands before T28.g's wiring. If it lands after, adding it to `active_events` moves
    summary output at a later train.
  - **Done (lane `p06-events`, 2026-10-28; at version 23, no bump), with burst bins of 15 minutes
    and N_GF = 67.29 from T21's census:** the record is under Risks ("Deviations in T28.c, as
    built" and "Deferred findings (2026-10-13 rule) (P06.T28.c)"). The [S] figures were read at
    source by research (2026-10-29) before landing: σ moved to 0.35 dex (Göğüş et al. 2001, §3.1)
    and `MAX_BURST_DURATION` to 3 s, and Coti Zelati et al.'s span is 1998–2016 (their first
    outburst is SGR 1627−41's of 1998 June 15); the rest are confirmed or consistent, Cheng et
    al.'s index only at second hand.
- **P06.T28.d FU Orionis outbursts.** Poisson bins (Δ = 500 years, look-back 1 bin): one per 5,000
  years while a protostar (age < 0.5 Myr), one per 112,000 years afterwards while the disc of T15.c
  lasts, the rate stepping smoothly over 0.05 Myr so the bound stays tight. Marks: luminosity factor
  10–100, rise of 1–10 years, decay of 20–200 years. **Done (lane `22-p06-t28d-fuor`, 2026-10-15;
  at version 22, no bump), to main's ruling of 2026-10-15, which replaces this text's rates and
  marks and the parent's 240–830 a year:** the record is under Risks ("Deviations in T28.d, as
  built" and "Deferred findings (2026-10-13 rule) (P06.T28.d)"); T28.g composes
  `Outburst::luminosity_factor_at` into `summary_at`.
  Records: [06-stellar-stage/records/P06.T28.d.md](06-stellar-stage/records/P06.T28.d.md).
- **P06.T28.e LBV giant eruptions.** Poisson bins (Δ = 100 years, look-back 1 bin) while inside
  T24.a's LBV window. The rate is a generator default, provisionally one per 30,000 years per star
  in the window, which with some 500 such stars gives a Milky Way galaxy about 30 in ±H (the
  research figure). T31 measures the real count and retunes the constant so that the galaxy-wide
  figure is 15–60, regenerating this task's golden. Marks: ejected mass of 0.1–10 M☉ (already in the
  mean wind; design note 14), peak of 10⁶·⁵–10⁷·⁵ L☉, duration of 1–20 years. **Done (lane
  `p06-events`, 2026-10-28; at version 23, no bump), retuned here to one per 12,000 years from
  T31's 181 LBVs (30.2 in ±H):** the record is under Risks ("Deviations in T28.e, as built" and
  "Deferred findings (2026-10-13 rule) (P06.T28.e)"); T28.g composes
  `Eruption::luminosity_factor_at` into `summary_at`.
  Records: [06-stellar-stage/records/P06.T28.e.md](06-stellar-stage/records/P06.T28.e.md).
- **P06.T28.f Thermal pulses.** Monotone phase on a clock whose base phase is the integral of 1 ÷
  τ_ip(M_c) along the thermally pulsing AGB segment, which T10.c stores at the segment's knots,
  interpolated in age, plus the clock time since the epoch times the epoch's pulse frequency (design
  note 23). Pulse number n(t) feeds T24's C/O. Transient: the luminosity dip and peak of a pulse
  cycle as a function of the cycle's fractional phase. **Done (lane `p06-events`, 2026-10-28; at
  version 23, no bump), with the base phase the track's own pulse count read at the age's two
  parts and a noise anchored so that pulse 0 falls at the phase's start:** the record is under
  Risks ("Deviations in T28.f, as built" and "Deferred findings (2026-10-13 rule) (P06.T28.f)");
  T24.b reads `thermal_pulse::pulse_number_at` for its n, and T28.g composes
  `ThermalPulseModel::luminosity_factor_at` into `summary_at`. _Amended 2026-10-29 (main's ruling
  on the lane's question)._ The base phase is the track's integrated pulse count, the integral of
  1 ÷ τ_ip(M_c, M_env) (Wagenhuber and Groenewegen 1998, A&A 340, 183, equation 11) that P06.T10.c
  stores at the segment's knots, read at the age's two parts: the age at the epoch, and the clock
  time since it, as P14.T45.d reads the mass. It replaces n(a₀) + f(a₀)(t − epoch), which is that
  count's tangent at the epoch. Both keep design note 23's intent, since the fast part comes from
  the clock time. The tangent departs from the count by ½ ḟ (t − epoch)²: under 10⁻³ cycles over
  ±H for most stars, but up to some 2 × 10⁻² cycles for the short periods of 6–7 M☉ stars, and
  many cycles across the source horizon, where it would put pulses outside the phase. Reading the
  count itself gives P06.T24.b's C/O, the census and the listings the same pulses.
  Records: [06-stellar-stage/records/P06.T28.f.md](06-stellar-stage/records/P06.T28.f.md).
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
- **T28.a done (lane `14-p06-t28a-flares`, 2026-10-14; at version 22, no bump: new output on a
  new kind that moves nothing existing, as "Generator version" says of every task but T30.b and
  T19.e).** The record is under Risks ("Deviations in T28.a, as built" and "Deferred findings
  (2026-10-13 rule) (P06.T28.a)"). T28.b and T28.d are done (2026-10-15, above) and T28.c, T28.e
  and T28.f (2026-10-28, above), and T28.g is open; T28.g composes
  `Flare::luminosity_factor_at` into `summary_at`.

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

Records: [06-stellar-stage/records/P06.T30.md](06-stellar-stage/records/P06.T30.md).

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
  Records: [06-stellar-stage/records/P06.T30.b.md](06-stellar-stage/records/P06.T30.b.md).
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
    planetary nebulae (T16) 5,000–50,000 (expected 20,000–40,000), compared by birth rate (ruling
    124.4): AGB white-dwarf births 1.2–3.0 a year (Moe and De Marco 2006: 2.4 ± 0.5) and a nebula
    share of those births of 0.6–0.95 (0.73 ± 0.10); living radio pulsars 10⁵–10⁶, of which beamed at a given place 10–20%;
    core collapses per century 1–8 across seeds and about 2 at Milky Way values.
- **Files:** `stellar/testing.rs`, `crates/hyperion-sim/tests/stellar_statistics.rs`.
- **Accept:** `just test-slow` passes; a band that fails is a finding to resolve in the model or to
  widen with a recorded reason, never silently.
- **Done (round 9, `events06a`)**, with the slow tests in `stellar/testing.rs` itself rather than
  `tests/stellar_statistics.rs` (see Risks for why and for the bands measured).

#### P06.T32 Benchmarks

- **Build:** Criterion benches: `ZCoeffs::new`; `stellar::lifetime` at 1, 5, 12 and 40 M☉ with
  `ZCoeffs` already built and with it built inside the call; `SystemStars::generate` for a layer-A
  main-sequence star, a red giant, a white dwarf and a neutron star; `brief_at` and `summary_at` on
  a built model; briefs for the 1,600 systems of a 50 ly query at the reference density.
- **Files:** `crates/hyperion-sim/benches/stellar.rs`.
- **Accept:** `just bench` runs them; targets under "Verification"; results recorded.
- **Completed by P06.T38.a (round 9, `briefs`).** The system and brief benches landed there, and
  their results are recorded under it. `lifetime` is benched at 1, 4, 5, 12, 20 and 40 M☉ (at load
  4–13, `math::exp` 7.8–8.1 ns): 606, 562, 583, 535, 445 and 446 µs, 55,000–78,000 `exp`, about 100
  times the 5 µs target, which the fitted table of T38.c–d is to meet. _Deviation:_ each is timed
  with `ZCoeffs` built inside the call only, since no public entry takes one already built;
  `ZCoeffs::new` alone is 1.4–1.7 µs, so the difference is under 0.3%.

#### P06.T38 Fitted fates and range briefs (ruling 89)

Records: [06-stellar-stage/records/P06.T38.md](06-stellar-stage/records/P06.T38.md).

A range brief is routed per row. Every exact-routed row is bit-identical to `SystemStars::brief_at`;
a dead primary read through the fate table agrees with `system_summary` in kind and class exactly,
and in L and T_eff within the table's stated error (ruling 89.2 as amended by ruling 90).
Main-sequence primaries (about 90% of rows) take a state-only fast path; living evolved stars and
stars near death take the exact integrator; dead stars take a **fate table over (m₀, Z, η)**, since
a remnant's state is a closed form of four numbers
(`Model::Remnant { phase, mass, birth, origin }`). The table is ruling 77.3's fitted lifetime
table: one artefact, one `Track::full` per node, giving the death age and the fate. It is over
mass, Z and η, never age, so it keeps the brainstorm's "no tables binned by age". No existing
output moves and no bump is needed; a moved golden is a finding, but for this task's own briefs
goldens, whose table-routed rows move within their stated error (ruling 90.4). The design study is
`_orchestration/research/briefs/ADVICE.md`.

- **P06.T38.a Measure first.** A stop point: report before building b–e. Say so if the draws, the
  hierarchy or the classification cost more than about 5 µs a row, since then the fast path alone
  misses the budget, and if the table's error exceeds about 10⁻³, since then the guard's fallback
  share grows, with how much.
  - **Build:** P06.T32's missing benches: `SystemStars::generate` for a layer-A dwarf, a giant, a
    white dwarf and a neutron star; `brief_at` and `summary_at` on a built model; `draw_hierarchy`,
    `StarDraws::for_star`, `draw_metallicity` and `classify` alone; briefs for the 1,600 systems
    of a 50 ly query through today's exact path. A route census (main sequence, living evolved,
    dead) of 50 ly queries at the solar circle, in the thick disc and in the bulge. A scratch fate
    survey, not kept: dense sweeps in log m₀ of t_death, route, remnant mass, cooling origin and
    M_CO at 5 Z × 3 η; every jump and kink located against the known `ZCoeffs` loci (m_hef, m_fgb,
    m_hook, the electron-capture and pair-instability edges); cubic interpolation error at node
    spacings of 0.01, 0.02 and 0.04 dex, and quadratic error in η.
  - **Provides:** the benches; the grid, node count, table size and tolerances that T38.c builds.
  - **Files:** `crates/hyperion-sim/benches/stellar.rs`; the survey in scratch only.
  - **Tests:** none kept but the benches.
  - _Accept:_ `just bench stellar` runs them; the measurements (per-row costs, route shares, the
    survey's errors) are recorded under this task.
  - _Measured (round 9, `briefs`, at `ea20910`, version 11)._ Benches in two runs at load 2.3–8,
    `math::exp` 7.8–7.95 ns; instruction counts from a scratch `perf_event_open` counter
    (user-space instructions, identical in both runs; one `math::exp` is 73), with the census run at
    load 6–9 and `math::exp` 11 ns. The census uses the benches' seed and fixture at the epoch.
    - _Benches._ `generate`: a layer-A dwarf 26.6–32.6 µs, a giant 62–77 µs, a white dwarf
      0.66–1.06 ms, a neutron star 1.11–1.44 ms. `brief_at` 0.17–1.1 µs and `summary_at` 0.44–3.6
      µs on a built system. `draw_metallicity` 66–82 ns, `StarDraws::for_star` 1.34–1.66 µs,
      `draw_hierarchy` (the dwarf's system, with a companion) 7.1–7.8 µs, `classify` 0.32–0.57 µs
      for a living star and under 0.07 µs for a remnant. The 969 rows of the 50 ly query through
      today's exact path: 89–95 ms, 92–98 µs a row, **147–157 ms per 1,600 rows** (11.5–12.2 M
      `exp`).
    - _Route census._ Solar circle, 50 ly (969 rows, complete to layer A): below 0.1 M☉ 7.7%,
      knot-free main sequence 84.2%, main sequence with loss none, living evolved 0.1%, dead 7.95%
      (64 white dwarfs, 11 neutron stars, 2 black holes; one within 10⁻³ of its death age). Thick
      disc (z = 4,500 ly; 50 ly holds 12 rows, so 200 ly, 1,224 rows, 44% thick disc and 7% halo):
      7.7%, 81.1%, none, 0.65%, 10.6%. Bulge (R = 2,000 ly, z = 1,000 ly), 50 ly: 1,245 rows,
      complete only to layer D under the default limit, **all dead** (975 white dwarfs, 161 neutron
      stars, 109 black holes), 5.7 M instructions a row, 1.4–1.8 s for the query.
    - _Per-row pieces_ (instructions, solar circle): metallicity 460; `for_star` 10.4 k;
      classification 4.1 k (main sequence), 2.3 k (dead), 11.6 k below 0.1 M☉ (with the cooling
      fits); the primary's model 55.7 k on the main sequence and 2.84 M dead; companions 138 k a row
      on average, which a brief does not need. **The hierarchy grows with the primary's mass:**
      12.5 k a row in layer A, 17 k in B, 43.5 k in C, 227 k in D and 272–335 k in E (about 3, 4,
      10, 52 and 62–87 µs at the census's load), 26.7 k on average at the solar circle.
    - _The 5 µs stop rule._ The draws, the metallicity and the classification are under it. **The
      hierarchy is not:** 7–8 µs for the benched dwarf's system, 5–6 µs a row on average, and 10–90
      µs for primaries of layers C–E, which are the dead rows. With the fast path (`ZCoeffs::new`
      1.4–1.7 µs plus the closed form) and a 3 µs table row, 1,600 solar-circle rows project to
      about 12–13 ms on one idle thread, 56% of it the hierarchy; the bulge's all-dead rows to about
      45 µs each, 0.9 s at the 20,000-row cap.
    - _Fate survey_ (scratch, not kept): 21,550 full tracks over 0.7–100 M☉ at 0.0025 dex, [Fe/H]
      −2, −1, −0.5, 0 and +0.3, and η at −3, −1.5, 0, +1.5 and +3σ; and 21,550 more at 25 [Fe/H]
      0.125 dex apart at the median η. Route loci: helium to carbon–oxygen white dwarfs at 0.71–0.94
      M☉ (the one locus that moves with η), oxygen–neon white dwarfs from 4.9–6.5, electron capture
      from 6.6–8.3, iron cores from 6.7–8.5, the supernova type's steps at 14–38 M☉; none of pair
      instability or thermonuclear disruption below 100 M☉. Kinks sit at m_hef (t_death's largest),
      at 6.3–8.0 M☉ (M_He) and at 22–24 M☉ for Z ≥ 0.02 (the stripping onset); none at m_hook or
      m_fgb.
    - _Interpolation errors_ (relative; median and p99). Cubic in log m at 0.01 dex: white dwarfs'
      t_death 1.4 × 10⁻⁶ and 4.1 × 10⁻³, mass 8.1 × 10⁻⁵ and 3.5 × 10⁻³, cooling origin 1.2 × 10⁻⁴
      and 2 × 10⁻²; iron cores' t_death 5.7 × 10⁻⁷ and 1.3 × 10⁻⁴, M_CO 6.8 × 10⁻⁶ and 4.5 × 10⁻³.
      Cells straddling a route change are 6.6%, 12.6% and 23% of the IMF weight at 0.01, 0.02 and
      0.04 dex; cells flagged (a straddle or any column over 10⁻³) 21.5%, 48% and 84%. Quadratic in
      η: t_death p99 2.5 × 10⁻⁴, white dwarf mass 1.2 × 10⁻⁴ and 8.5 × 10⁻³ (18% over 10⁻³); the
      iron cores' columns do not depend on η. **In Z the white dwarf mass is structured:** at fixed
      m₀ it moves by 5–10% within 0.125–0.25 dex (1.5 M☉: 0.663, 0.671 and 0.615 M☉ at [Fe/H] −1.45,
      −1.33 and −1.20; 0.625, 0.687 and 0.637 at −0.70, −0.58 and −0.45). Cubic at 0.25 dex gives
      it 7.8 × 10⁻³ and 7.2 × 10⁻² (90% of points over 10⁻³), and even a 0.125 dex fit leaves a
      residual of 2.2 × 10⁻³. The iron cores' M_CO is 6.5 × 10⁻⁵ and 2.7 × 10⁻² at 0.25 dex.
      Cubic in log m at 0.02 and 0.04 dex: white dwarfs' t_death 9.6 × 10⁻⁶ and 1.8 × 10⁻², then
      7.8 × 10⁻⁵ and 4.0 × 10⁻²; mass 2.6 × 10⁻⁴ and 6.2 × 10⁻³, then 6.7 × 10⁻⁴ and 1.2 × 10⁻²;
      cooling origin 4.6 × 10⁻⁴ and 3.4 × 10⁻², then 2.0 × 10⁻³ and 3.7 × 10⁻²; iron cores' t_death
      2.8 × 10⁻⁶ and 4.2 × 10⁻⁴, then 1.4 × 10⁻⁵ and 5.2 × 10⁻⁴; M_CO 2.9 × 10⁻⁵ and 1.2 × 10⁻²,
      then 2.3 × 10⁻⁴ and 1.2 × 10⁻².
    - _What it means for T38.c._ At the design study's grid (about 0.02 dex in mass, 10 Z nodes, 3
      η) the medians meet 10⁻³ for death ages and iron cores, and the p99s are the kink cells, which
      the build flags. White dwarfs, the dead rows' 80–85%, miss it: their mass error is near 10⁻²
      from the Z axis alone. That moves T_eff by about 0.4% and the temperature index (50,400 ÷
      T_eff, to one decimal) by 0.04–0.08, so the guard would trip for an estimated 80–100% of them.
      Each fallback is the primary's full track, 0.6–0.7 ms. The 1,600 solar-circle rows, with some
      106 white dwarfs, would then cost about 70–90 ms instead of 12–13 ms, 5–6 times the budget.
      **The 10⁻³ condition is met for iron cores and death ages and missed for white dwarfs.**
      Reported for a ruling before b–e.
    - _Deviations, as built._ The fixture's 50 ly query at the Sun-like point holds 969 rows, not
      1,600, so the brief bench runs those and the per-1,600 figures are scaled. The thick-disc
      census uses 200 ly, since 50 ly holds 12 rows, and the bulge's is complete only to layer D.
      The survey sweeps five η values and adds the 25-node Z sweep, beyond 5 Z × 3 η, to measure
      the Z axis. The giant, white dwarf and neutron star exemplars come from a 150 ly query down
      to layer D if the 50 ly query has none. `classify` is benched for every exemplar.
- **P06.T38.b The main-sequence fast path.**
  - **Build:** the builder's knot rule (loss at the entry rate under 10⁻⁶ of the mass) made
    shareable; a state-only evaluation of `ZCoeffs`, the main-sequence timescales and the closed
    form at the age, skipping the maxima samples and the golden-section searches `to_age` builds.
  - **Provides:** `sse::main_sequence_state(m0, &Composition, &StarDraws, age) ->
Option<StarState>`, `None` unless the star is on a knot-free main sequence at `age`.
  - **Files:** `stellar/sse/{evolve.rs, track.rs, track/build.rs, mod.rs}`.
  - **Tests:** bit-equal to `Track::state_at` wherever `Some`, over 10⁵ random (m₀, Z, η, age),
    ruling 77.3's pattern for `lifetime`; `None` exactly where the track has knots or has left the
    main sequence.
  - _Accept:_ `cargo test -p hyperion-sim -- stellar::sse` and
    `just test-slow main_sequence_fast_path`; every golden unchanged.
  - - _As built (round 9, `briefs`)._ `Track::knot_free_main_sequence` (crate-private) builds the
      main-sequence segment alone through the builder's own `main_sequence_segment`, which
      `Builder::main_sequence` now calls, so the knot rule is shared by construction and nothing else
      of the builder moved. `main_sequence_state` answers only before the segment's end, since at the
      end the full track's next segment starts. `track/build.rs`, which the task lists, is unchanged; `track/phases.rs` holds the shared segment. Tests: 2,000 random stars in `just test` and 10⁵
      slow, each branch counted (answered, with knots, at or past the end, age zero), plus the
      refusals outside 0.1–100 M☉ and at negative or non-finite ages; no golden moved. Cost: 2.9 µs
      for a 0.4 M☉ dwarf (bench, `math::exp` 7.2–7.9 ns), about 27,000 instructions against `to_age`'s
      6.8 µs.
- **P06.T38.c The node evaluator and the table task.** Held by ruling 90, unblocked by ruling 99
  once ruling 92's giant radii smoothed the white dwarfs' metallicity structure.
  - **Build:** in the sim, a public node evaluator (one `Track::full`, the six columns), since
    `fate_of` and the remnant segment are crate-private and `hyperion-fit` calls only public API
    (plan 15, design note 1). In `hyperion-fit`, `tasks/stellar_fates.rs`: the grid T38.a fixed,
    per-cell validation at the cell centres (error bounds and unsmooth flags), rendered through
    `tasks/render.rs` with a header as `wd_cooling.rs` has (tool, version 0, source, grid,
    acceptance figures, since-generator-version); `Command::RunStellarFates` and
    `DEFAULT_STELLAR_FATES_OUT` in `lib.rs`, and the dispatch in `main.rs`. Split the output by Z
    row if it passes about 450 kB. Land it after the bump to 12, so its header reads 12.
  - **Provides:** `stellar::fates::FateNode::of(m0, &Composition, eta)`;
    `crates/hyperion-sim/src/tables/stellar_fates.rs` and its `tables/mod.rs` line.
  - **Files:** `stellar/fates.rs`, `stellar/sse/` (the re-export), `tables/stellar_fates.rs`,
    `tables/mod.rs`, and in `crates/hyperion-fit/src/`: `lib.rs`, `main.rs`, `tasks/mod.rs` and
    `tasks/stellar_fates.rs`.
  - **Tests:** a byte-for-byte reproduction of the table (slow); a fingerprint test in `just test`,
    12 fixed nodes across every route rebuilt with `FateNode::of` and compared bit for bit with the
    table, so that a stellar change moving a node fails ordinary CI (plan 15, design note 7).
    - _Accept:_ `just ci` and `just test-slow stellar_fates` green (as built:
      `just test-slow the_committed_panels_are_reproduced`, and `just fit-check`).
  - _As built (round 9, `briefs`; ruling 99), on fit15's registry (plan 15, P15.T1–T2)._
    - **The re-survey** on the merged tree (21,550 tracks at five \[Fe/H\] and five η, and 40
      \[Fe/H\] at the median η) found the white dwarfs' mass error in \[Fe/H\] at 0.25 dex down to
      a median of 4.9 × 10⁻⁵ (1.1% over 10⁻³) and the iron cores' M_CO to 7.4 × 10⁻⁶ (10.5% over
      10⁻³), as ruling 99.3 said. Cubic in η over five nodes left the white dwarfs' mass at 8.2%
      over 10⁻³; the iron cores do not depend on η (10⁻¹⁵).
    - **The grid was chosen on real rows, not the survey alone.** A scratch harness built
      candidate tables in memory and routed the dead rows of the 50 ly solar-circle query, a
      200 ly thick-disc query and a 50 ly bulge query through them. The \[Fe/H\] spacing decides
      the white dwarfs' mass error and so how often the guard fails: 11 nodes gave 24 ms per
      1,600 solar-circle rows, 21 gave 18.7, and HPT's seven calibration metallicities as nodes
      with 0.125 dex between them 16.9. The mass step (0.005 against 0.01 dex) and the η nodes
      (four, five or seven) moved it by under 1 ms; a safety factor of 2 gave 19.5 ms but a
      class mismatch in the bulge, so it stays 3.
    - **Three panels, three tasks, three files** (`stellar_fates_low`, `_mid` and `_high`), since
      the toolchain writes one file per task and the repository takes no file over 500 kB:
      low 0.74–2.63 M☉ (56 masses, 285 kB), mid 2.34–9.12 M☉ (60 masses, 304 kB), both at
      Δlog m₀ = 0.01 and η = −3, −1, 1 and 3; high 5.75–100 M☉ at Δlog m₀ = 0.02 and η = 0 alone
      (63 masses, 111 kB). All share 24 \[Fe/H\] nodes. That is 12,648 nodes; with the
      validation's four points a cell, 49,816 full tracks, 17 s on four threads. The reader switches
      panel at 2.5 and 8 M☉, and each panel's nodes run past its splits. The tasks are `Fast`.
    - **Columns:** the route (He, CO or ONe white dwarf, electron capture, iron core, no remnant),
      log₁₀ death age, and two values: a white dwarf's mass and cooling origin, or an iron core's
      progenitor M_CO and M_He. The iron core's remnant is drawn afterwards from the star's own
      draws by Mandel and Müller's `core_collapse`, as the track does. Values are stored to eight
      significant digits, and without digit separators so that the files fit
      (`#[expect(clippy::unreadable_literal)]`). The large arrays are `pub static`, since Clippy
      refuses so large a `const`, and \[Fe/H\]'s array expects `approx_constant`, since log₁₀(0.01
      ÷ 0.02) is −log₁₀ 2.
      - **Validation:** each cell of mass and metallicity is checked at four points, a quarter and
        three quarters across it in mass and in \[Fe/H\], paired four ways, at the middle of each η
        cell (the high panel: at η ±2.4, 0 and 1.2, and it answers for no |η| over 2.4). A bound is
        three times the worst error, at least 10⁻⁴, rounded up to two digits. A cell fails where a point takes
        another route, no stencil avoids a change of route, or a bound exceeds 0.02 (0.2 for iron
        cores, whose guard reads only the remnant's kind). Unusable cells: 35% of the low panel's
        (the He/CO line and m_hef), 9.6% of the mid's, 6.5% of the high's. Usable bounds (median,
        99th percentile): death age 1.1 × 10⁻⁴ and 3.1–16 × 10⁻³; white dwarf mass 1.1 × 10⁻³ and
        1.1 × 10⁻² (low), 5.8 × 10⁻⁴ and 8.2 × 10⁻³ (mid).
    - **Tests:** `the_committed_panels_are_reproduced` (slow, byte for byte through
      `pipeline::rerender`) and a small fit equal on one and four threads, in `hyperion-fit`;
      `just fit-check` holds the three to their inputs hash, body hash and fingerprint (two
      probe stars a panel); the sim's `the_committed_nodes_are_the_tracks` rebuilds twelve nodes
      (four a panel) with `FateNode::of` and compares them with the table to its stored digits,
      in `just test`, which is the plan's twelve nodes to the stored precision rather than bit for
      bit.
      - _Deviations:_ no `render.rs`, `Command::RunStellarFates` or `DEFAULT_STELLAR_FATES_OUT`, which
        plan 15's registry replaced; three tasks and files, not one, from one
        `StellarFatesTask { panel }` in three statics; `FateNode::of` takes η as a `StandardNormal`;
        four columns, not six, since an iron core's remnant is redrawn from M_CO and M_He and needs
        no supernova byte or envelope; the node check compares to the stored eight digits, not bit
        for bit; the header reads 11, since the bump to 12 has not come, and the bump must re-emit
        the three with `--since 12` (or refit them if a track moves).
      - _As built (slow-test audit, 2026-09-27):_ `the_committed_panels_are_reproduced` is
        deleted. It repeated `hyperion-fit check --rerun-fast`, which reruns every fast task,
        these three panels included, and compares the bytes; `just test-slow` runs that check,
        now under the heavy-test lock. The byte-for-byte reproduction this task asks for is that
        check's.
- **P06.T38.d `FittedFates`, the reader.** Built with T38.c.
  - **Build:** interpolation in fixed-order arithmetic through `math`; `None` outside the domain
    (m₀ below 0.7 or above 100 M☉, η beyond ±3σ, a non-zero `helium_excess`, an unsmooth cell).
  - **Provides:** `FittedFates::{lifetime_fitted, lifetime_bracket, fate_fitted}` and
    `FittedFate` (the route, the remnant inputs and the error bounds).
  - **Files:** `stellar/fates.rs`, `stellar/mod.rs`; the Verification's lifetime line names it
    met.
  - **Tests:** over random stars against `Track::lifetime` and `Track::fate_with` (10⁴ on a
    subsample in `just test`, 10⁵ slow): the bracket contains the exact lifetime every time, and
    the columns are within the stated tolerances.
  - _Accept:_ `cargo test -p hyperion-sim -- stellar::fates` and its slow test green.
  - _As built (round 9, `briefs`)._ `FittedFates { fe_h, panels: [FatePanel; 3], splits }`, with
    `generator()` over the three files, and `fate_fitted`, `lifetime_fitted`, `lifetime_bracket`
    and `fate_unbounded` (what `hyperion-fit` validates by). The interpolant is Lagrange in each
    axis: cubic, or quadratic or linear where no cubic stencil keeps to one route. It answers
    `None` outside 0.74–100 M☉, outside the η nodes, for a helium excess, where no stencil keeps to
    one route, and in an unusable cell. A stripped star is not the table's inside
    `STRIPPED_WINDOW` (5.5–11 M☉), where the mark can move its fate (ruling 93); a test checks
    that outside it the mark moves none. `FittedFate::remnant` gives the remnant of the star's own
    remnant draws with the columns shifted by their bounds, the guard's corners. - **Tests:** `the_table_is_within_its_bounds`: over 400 random stars in `just test` the route
    is always the track's, every star lies within three times its bounds, and at most one in
    fifty past a bound itself; the slow test holds 10⁴ stars to the same and one in a hundred (as run: 7,638 answered, 13, or
    0.17%, past a bound, none past three times it or by another route).
    _Deviation:_ 400 and 10⁴ stars, not 10⁴ and 10⁵, since each is a full track; and the
    bracket is not held "every time" at its own width: the bounds are validated, not proved
    (before the floor of 10⁻⁴, the four-point validation and the high panel's |η| ≤ 2.4, iron
    cores were found three to eleven times past theirs, and a 99 M☉ star's death age six), so
    the range brief takes a star as dead only past three times the death age's bound, and at
    least 3 × 10⁻³ of its life (`DEAD_MARGIN_FLOOR`), the margin the tests hold always.
    - `lifetime_fitted` costs about 1 µs; plan 08 and P06.T30.b may read it. The Verification's
      lifetime line names it.
- **P06.T38.e The brief router.** _Ruling 90:_ built before T38.c–d with **dead stars on the exact
  path** until the table exists, and with the hierarchy's count-only path **required**, as a new
  plan 11 function beside `draw_hierarchy`, which stays unchanged.
  - **Build:** route each primary: below 0.1 M☉, the cooling fits; on a knot-free main sequence,
    T38.b; otherwise by the certified bracket: before it, the exact `Track::to_age`; after it, the
    table's remnant through the track's own remnant closed form (`remnant_state`, crate-internal,
    the function the remnant segment already calls), with the star's own remnant draws on the
    table's progenitor (`iron_core_fate`); inside it, the exact `Track::full`. A guard evaluates
    kind and class at the corners of the error box (T_eff, L × (1 ± ε), M_CO ± ε) and falls back
    to the exact path wherever they could differ. Only the primary is built and only the tags the
    brief reads are opened; the star count comes from the hierarchy (a count-only path, tested
    equal to `star_count()`, if T38.a finds the hierarchy dominant). A system where plan 11's
    `can_interact` holds, once P11.T4 exists, routes exact. _Amended by plan 11's P11.T11.a_
    (`decision-observed-class-findings.md` §A): the router no longer runs without the binary
    engine everywhere. A gate first certifies the primary as its own single-star model through the
    window (a single system, or one whose first attempt's pairs cannot change its stars or be
    carved); those rows take the routes here, bit for bit, and every other row takes
    `BriefRoute::Paired`, the system's `SystemStars` with the engine, whose brief is the summary's
    primary.
  - **Provides:** `stellar::brief::{BriefModel, range_brief}`.
  - **Files:** `stellar/brief.rs` (new), `stellar/mod.rs`, `stellar/system.rs` (the shared
    `object_kind` and `classify` call), `tests/golden/stellar/briefs.golden`.
  - **Tests:** every exact route bit-equal to `SystemStars::brief_at`; table routes with equal
    kind and class, and L and T_eff within tolerance, over 10⁵ random records (slow); order and
    cache independence (no cache, warm, random eviction, four chunks); `None` before birth; a golden
    of a dozen pinned IDs across the five layers and every route, at t = 0 and ±500 yr; the bench
    of 1,600 briefs.
  - _Accept:_ `just ci` and `just test-slow brief` green; 1,600 cold briefs under 15 ms on one
    thread once T38.d routes the dead rows (ruling 90), and until then measured and recorded with
    the load and `math::exp`.
  - - _As built (round 9, `briefs`; ruling 90)._ `BriefRoute` has two routes, `MainSequence` (a
      knot-free main sequence lasting past +H, its one-segment track) and `Exact` (the primary's
      `StarModel`: the cooling fits, living evolved stars, stars leaving the main sequence within the
      window, and every dead star). `BriefModel::new(galaxy, record)` takes no fates until T38.d.
      `StellarBrief::of` (crate-private, in `system.rs`) is the one brief function that both
      `SystemStars::brief_at` and the router call. A main-sequence row reads the primary's η alone
      (`StarDraws::eta_for_attempt`, crate-private, tested equal to `for_attempt(..).eta()`), since
      neither the main sequence nor `classify` of a living star reads another draw. The star count is
      `multiplicity::draw_star_count`, which reads the draw's count words and returns 1 without the
      tidal limit, the period laws or any orbit for a system drawn single, and otherwise draws the
      hierarchy in full. `Draw::companion_count` and `Draw::is_direct` delegate to free functions it
      shares; `draw_hierarchy` is unchanged. Tests: the count equals the full draw's over 4,500
      records of every mass, two positions, four contexts and two attempts; the brief is bit-equal to
      `SystemStars::brief_at` over 180 pinned records at six times in `just test`, 300 random systems
      in the integration test and 3 × 10⁴ slow (250 s at 10⁵, so cut); a model built twice is equal
      and answers the same in any order; `stellar/briefs.golden` pins 12 systems of layers A–E, one
      below 0.1 M☉ and one subgiant, 12 distinct, at −500, 0 and +500 yr. The main-sequence route reads η at the
      same seam as `primary_draws` (`system::primary_eta`), which P08.T12.c changes with it. No existing golden moved.
  - _Cost_ (instruction counts from the scratch counter, one `math::exp` = 73; wall at load 6–9,
    `math::exp` 11.9 ns; the bench at load 2.5–12, 7.2–7.9 ns). Per solar-circle row: main sequence
    38,900 instructions (532 `exp`, about 4.2 µs idle) against 111,000 before; living 43,800;
    dead 3.19 M (the full track, unchanged). **1,600 cold rows: 50 ms idle** (3,964 `exp` a row),
    87.5% of it the 8% dead rows; without them 6.3 ms. The bench's 969 rows: 85 ms routed against
    135 ms exact in one run at load 12. **Warm** (built models): 4,360 instructions a row, 0.84 µs,
    **0.76 ms per 1,600**. A model is charged 2.2 kB on the main sequence and 25 kB dead. The thick
    disc's 200 ly (10.6% dead) is 97 ms per 1,600 at 11.9 ns; the bulge's all-dead rows 49,000 `exp`
    each, so 20,000 of them are 7.7 s on one thread and about 1.1 s over the pool's seven workers.
    **The 15 ms budget is missed by the dead rows alone,** as ruling 90 expected; T38.c–d are
    what meets it.
  - _The table route, as built (ruling 99)._ `BriefRoute::Table`: a star the table finds dead
    through the whole window (its age at −H past the death age plus its bound) takes its
    remnant's closed form at the table's fate (`Track::remnant_only`, whose state is the full
    track's bit for bit at the same remnant). It is asked before the main sequence, whose build
    for a dead massive star would be wasted. The guard:
    - a white dwarf's class is a function of its temperature and draws alone: the temperature's
      range over the box and the window, to first order (three partial differences at −H, the
      cooling to +H at the centre), must keep the class the same at both ends and the centre;
    - an iron core's remnant kind must be the same at the four corners of its two cores;
      - a neutron star's class (`NS`, `PSR` or `MAG` since P06.T21.c) must be the same at both ends
        of the window for the death age at its centre and three bounds either side, since the
        pulsar's age since formation decides it; electron capture takes that check alone, and no
        remnant none.
        On the rebase onto d2787a2 (version 12) the remnant routes pass a neutron star's pulsar class
        to `classify` (`remnant_extras`, as `StarModel::class_extras_at` does), and the main sequence's
        rotation draws are read after the dead star's early return. The three tables' bodies are
        unchanged at version 12.
        A star that fails, or whose mark matters, takes `remnant_of`, a build to the death that keeps
        no other segment and no samples (`Keep::Remnant`, new), whose remnant is the full track's bit
        for bit (tested), and only a star alive at −H after all its `StarModel`.
        `BriefModel::stated_error_at` gives the table's brief its stated error: the range of log L and
        `T_eff` over the corners of the box at three times the bounds (`STATED_FACTOR`), widened by
        at least 10⁻⁴ dex and 10⁻⁴ relative. Every table-routed row of the tests lies inside it (31,536
        in the slow test); at the bounds' own width 0.5% did not. A star is taken as dead past three times the death age's
        bound. `BriefModel::with_fates` takes another table, for studying one on real rows.
        _Also:_ the builder now finds the lightest helium star only when a helium main sequence is
        entered, not in `Builder::new`, which cut the T38.b fast path by a twentieth; no output moves.
        T38.b's note that `track/build.rs` is unchanged no longer holds.
      - **Cost** (instruction counts; `math::exp` = 73; at an idle 7.9 ns): per 1,600 solar-circle
        rows **16.6 ms** (96,000 instructions a row), of which the twelve exact dead rows are 51%,
        the main sequence 34%, the table's 65 rows 11%. The wall time was 18.9 ms for the 969 rows
        at load 6 and 4.2 GHz. Warm: 0.6 µs a row, 1 ms per 1,600. A table row costs 116,000
        instructions for a white dwarf and 326,000 for a neutron star, most of it plan 11's
        hierarchy of a massive star. The thick disc's 200 ly query: 29 ms per 1,600 (its white
        dwarfs meet more unusable cells); the bulge's all-dead rows: 148 ms per 1,600, 82% of them
        table-routed (1.8 s at the cap of 20,000 on one thread, about 0.3 s on the pool).
      - **Fallback share** of the dead rows: solar circle 16% (6 guard, 4 unusable cells or no
        stencil, 1 near death, 1 neutron star at the NS/BH line); thick disc 28%; bulge 18%
        (stripped marks 5%, white-dwarf guard 10%). **The 15 ms target is missed by 11%** at the
        solar circle: the white dwarfs' mass error, about 10⁻³ against a temperature index rounded to 0.1,
        trips the guard for one white dwarf in nine, and each costs a full track to its death.
      - **Accuracy:** no class or kind mismatch over the 1,175 table-routed rows of the three
        queries, nor over the slow test's 31,536, every one inside its stated error.
        The golden `stellar/briefs.golden` moved on the five systems that are now table-routed
        (ruling 90.4): log L by up to 4 × 10⁻⁴ dex and `T_eff` by up to 0.02%, every class and kind
        unchanged. `systems_in_range_briefs.golden` moved on 27 dead rows: log L by up to 2.5 ×
        10⁻⁴ dex (median 2 × 10⁻⁵) and `T_eff` by up to 3.6 × 10⁻⁴ (median 2.5 × 10⁻⁵), kinds and
        classes unchanged.
  - _Re-measured after the rebase onto 6ac4458_ (kick's P06.T19 and zsmooth's ruling 92).
    Instructions per solar-circle row: main sequence 38,700 (unchanged), living 44,600, dead 4.25 M
    (×1.33 over 3.19 M). **1,600 cold rows: about 65 ms idle**, 90% of it the dead rows; warm
    unchanged at 4,360 instructions a row. The bulge's dead rows cost 59,000 `exp` each. Both new
    goldens were re-blessed: only white-dwarf rows moved (L, T_eff and some Sion indices, such as
    DA8.4 to DA8.5 and DC19.1 to DC17.6), none on the main-sequence route.

### Phase H: protocol, server and display

#### P06.T33 Protocol

Records: [06-stellar-stage/records/P06.T33.md](06-stellar-stage/records/P06.T33.md).

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

Records: [06-stellar-stage/records/P06.T34.md](06-stellar-stage/records/P06.T34.md).

- **Blocked in part (ruling 77.3), unblocked by T38 (ruling 89):** the `system_summary` handler is
  built. The range handler's `stellar` briefs wait on T38.e: a brief needs each star's state at the
  requested time, and tracks are still 4–7 times over the Verification's `generate` targets after
  ruling 77.1, which misses the Verification's 15 ms for 1,600 briefs. Ruling 89 settles what
  unblocks them: neither tracks within their targets nor a table of brief states, but T38's routed
  briefs (a main-sequence fast path, the exact integrator, and the fate table, whose columns give a
  dead star's state as well as its lifetime). The remainder, built after T38.e:
  `convert.rs::system_record` takes an `Option<StellarBriefDto>` and loses its hard-disable;
  `convert/stellar.rs::brief_dto` (ruling 54's null `teff_k` and log L where L = 0, plan 11's
  `star_count`); `requests/galaxy.rs`'s range job builds `range_brief`s when asked, in
  index-ordered chunks on the pool above a row threshold; `compute/systems.rs` gains a
  `SharedBriefCache` of `BriefModel`s keyed by `(GalaxyKey, SystemId)`, budgeted by
  `--brief-cache` / `HYPERION_BRIEF_CACHE_MB` in `config.rs` and the README's table. Its tests in
  `tests/systems_in_range.rs`: a brief on every row, the same request twice identical cold and
  warm, one chunk against four identical, and a new server golden with briefs;
  `systems_in_range.golden` stays byte-identical.
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
  says what is hidden (`412 OF 1630 SHOWN: LIVING`, digits grouped from five as the guide and
  plan 05's `lib/format.ts` have it), per the guide's honest-data rule. Legend
  entries for the five shapes. The list gains a class column.
  Records: [06-stellar-stage/records/P06.T35.b.md](06-stellar-stage/records/P06.T35.b.md).
- **Files:** `docs/frontend/ux-guidelines.md`; under `apps/hyperion/src/renderer/src/`:
  `spatial/{marks.ts, symbols.ts}`, `lib/galaxy/{starSymbols.ts, wire.ts, model.ts}`, in
  `displays/galaxy/`: `chartModel.ts`, `ChartControls.tsx`, `SystemList.tsx`, `SymbolLegend.tsx` and
  `useRangeQuery.ts`; `test/galaxyFixtures.ts`, and their tests.
- **Tests:** vitest: mapping is exhaustive over `ObjectKind` (a `switch` with no default); filter
  logic; legend renders every shape with an accessible name; the list row shows class text.
- **Accept:** `pnpm typecheck`, `pnpm lint`, `pnpm test` green.

#### P06.T36 Client: system readout

Records: [06-stellar-stage/records/P06.T36.md](06-stellar-stage/records/P06.T36.md).

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
  _Its frame, pegs and marks are drawn by `paint.ts` at R07.T16.f's widths
  (decision-thin-line-contrast)._
- **Files:** `apps/hyperion/src/renderer/src/displays/galaxy/HrDiagram.tsx`,
  `lib/galaxy/hrProjection.ts`, tests (with plan 05's `RecordingContext2D`).
- **Tests:** projection maps the Sun to the expected pixel; picking tolerance of at least `1rem`;
  renders with zero points; the caption counts.
- **Accept:** TypeScript checks and tests green; the by-eye checks of "Verification".

### Phase I: rendering plan R06's asks

Rendering plan R06's asks A1, A2 and A4 ("Asked by rendering plan R06", under Risks), accepted on
2026-10-19 (signoff-12 item 29) and numbered here by decision-galaxy-25-train §1.7 (2026-10-27).
A3 needs no task: its rule is this plan's, as built in `stellar/photometry.rs`. P06.T40 stays
reserved for finding F13 (Risks, "Deferred findings (2026-10-13 rule) (P11's F13 …)"). A2 and A4
move stellar output, so each lands its pure part first and holds its swap on a lane branch:
stellar evaluation takes no `Galaxy`, feeds process-wide statics such as
`galaxy::fates::fates_for`'s `FATES`, and moves fitted tables, so it cannot sit behind a hold on
integration.

#### P06.T41 `BriefModel` over the retarded interval (ask A1)

- **Build:** a constructor such as `BriefModel::new_for(galaxy, record, earliest_emitted)` whose
  routes are tested from the earliest emitted time to +H. The sky places every star at its emitted
  time, up to some 2.6 × 10⁵ years before the query's (the light time across the galaxy), where
  `BriefModel`'s routes are tested only over the clock window today. Over the wider interval each
  route keeps the module's contract: the exact, main-sequence and paired routes give
  `SystemStars::brief_at`'s brief bit for bit, and the fate table's route its kind and class
  exactly and its L and T_eff within `BriefModel::stated_error_at`. `BriefModel::new` and its
  clock window are unchanged.
- **Files:** `stellar/brief.rs` and its tests.
- **Tests:** every route against `SystemStars::brief_at` at times from `earliest_emitted` to +H,
  with `earliest_emitted` at the clock window's start and 2.6 × 10⁵ years before it; a route
  certified over the clock window but not over the wider interval falls back to a dearer one.
- **Accept:** tests pass. **No output moves** and there is no bump (signoff-12 item 29).
- **Its reader.** The ask names R06.T8.b's brief as its one call site, switched once T41 lands.
  As built, R06.T8.b reads no brief: its flux bound reads R06's brightness envelope
  (`sky/census/cell.rs`'s module text), so no call site switches today.
- **Held** (decision-galaxy-25-train §11, 2026-10-27). With no reader, T41 is not briefed, and it
  is not a close-out item of this plan: plan 06 closes without it. A task that needs a brief at
  an emitted time outside the clock window, at a cost the exact path can't meet (plan 12's
  observed mode, or a later R06 cost task), names T41, and T41 is briefed with it.

#### P06.T42 The Humphreys–Davidson limit in the modern winds (ask A2)

Ruled 2026-10-19 (signoff-12 item 29): the tracks must not keep cool supergiants past the limit.
No cool supergiant is observed above log L ≈ 5.5 (Davies, Crowther and Beasor 2018, MNRAS 478,
3138: 5.4 in the SMC, 5.5 in the LMC; Humphreys and Davidson 1979's line at 5.7–5.9, revised to
5.66, as they quote it). Under `WindRecipe::Modern` a star past the limit loses the constant
`MODERN_LBV`, 1.5 × 10⁻⁴ M☉/yr (Belczynski et al. 2010, eq. 8), about five times below the
Nieuwenhuijzen–de Jager rate it replaces on the cool side, so the tracks keep cool supergiants up
to about 0.5 dex above the observed ceiling for some 2 × 10⁵ years. Flooring at the cool-side rate
alone changes nothing at log L 5.8–5.9, where that rate (1.1–1.6 × 10⁻⁴) is about the constant.
Hurley, Pols and Tout's term, about 9 M☉/yr at log L 5.80 and 120 at 5.90 at 4,000 K, a wall at
log L 5.78–5.80, does the enforcing.

- **P06.T42.a The rate, pure** (no output moves, no bump). Past the limit the Modern rate is
  max(`MODERN_LBV`, the cool- or hot-side rate) plus Hurley, Pols and Tout's LBV term,
  Ṁ_LBV = 0.1 (10⁻⁵ R L^½ − 1)³ (L ÷ 6 × 10⁵ − 1) M☉/yr where L > 6 × 10⁵ L☉ and
  10⁻⁵ R L^½ > 1 (HPT 2000 §7.1; `wind.rs`'s `hurley_lbv`), as COMPAS's `HURLEY_ADD` combines
  them. The term is added after the Eddington boost (P06.T39), not inside `modern_below_kink`, and
  the doc says that the rate departs from Belczynski et al.'s "full amount of LBV mass loss". It
  lands as a function that nothing on the record calls.
  Records: [06-stellar-stage/records/P06.T42.a.md](06-stellar-stage/records/P06.T42.a.md).
  - **Files:** `stellar/sse/wind.rs`.
  - **Tests:** below the limit the function gives today's Modern rate to the bit; past it the rate
    is at least `MODERN_LBV` and at least the side rate; the term's figures above, at 4,000 K.
  - **Accept:** tests pass; nothing on the record calls the rate, so no golden moves.
- **P06.T42.b The swap** (output moves; held on a lane branch, it rides version 25 (S1) if it is
  ready when that train's wiring starts, else the next galaxy train; decision-galaxy-25-train
  §1.7). `WindRecipe::Modern` reads T42.a's rate past the limit.
  Records: [06-stellar-stage/records/P06.T42.b.md](06-stellar-stage/records/P06.T42.b.md).
  Its builder:
  - re-checks the remnant masses, since the added term undoes part of Belczynski et al.'s
    black-hole calibration at solar metallicity;
  - checks `envelope_knots` against the term's stiffness (about 10⁴ M☉/yr deep past the line),
    where a knot-cap fault was found before (Risks, "The envelope integrator's past-the-grid
    branch had no knot cap");
  - re-measures R06's brightness envelope and caps, which read the tracks with no change in R06.
  - **What moves:** every track past the limit, and through them `FATES`, `kick_rank`'s
    fingerprint (core and remnant masses at eight initial masses, 8–150 M☉, so P06.T19.e re-runs
    its production manifest at 25), and the pair-light and sky tables. Refits follow
    `just fit-check`'s flags in the train's wiring.
  - **Accept:** T42.a's tests pass against the swapped recipe; the three checks above recorded in
    Risks.

#### P06.T43 White dwarfs' V and cool giants' bolometric corrections (ask A4)

Accepted 2026-10-19 (signoff-12 item 29): a white dwarf with no V is dropped from the sky's
census, and a late M giant reads up to 1.7 mag too bright at M6 III by `photometry.rs`'s own note.
Both are wrong skies.

- **P06.T43.a V and BC_V, pure** (no output moves, no bump). An absolute V for white dwarfs from
  the Montreal grids (Bédard et al. 2020, ApJ 901, 93), and a gravity-dependent BC_V for cool
  giants (Fluks et al. 1994 or Worthey and Lee 2011 for M giants; Levesque et al. 2005's column
  for supergiants), as functions that nothing on the record calls. With them, the check of the
  module's "Straižys and Kuriliene (1981, Ap&SS 80, 353, Table III)" citation for the M giants'
  corrections.
  Records: [06-stellar-stage/records/P06.T43.a.md](06-stellar-stage/records/P06.T43.a.md).
  - **Files:** `stellar/photometry.rs` and its tests.
  - **Tests:** a white dwarf's V at the grid's nodes, to the grid's printed precision; the cool
    giants' BC_V against the chosen source's table at three gravities; the M6 III case of the
    module's note recorded before and after.
  - **Accept:** tests pass, and the citation check's result recorded in Risks; nothing on the
    record calls the functions, so no golden moves.
- **P06.T43.b The swap** (output moves; held on a lane branch, it rides version 25 (S1) if it is
  built when that train's wiring starts, else the next galaxy train; decision-galaxy-25-train
  §1.7). `absolute_magnitude_v` reads T43.a's V and BC_V, and `absolute_v_of_state` drops A4's
  interim, a white dwarf dark in V. R06's census and luminosity function follow with no other
  change.
  Records: [06-stellar-stage/records/P06.T43.b.md](06-stellar-stage/records/P06.T43.b.md).
  - **What moves:** the census (white dwarfs gain a V, and late M giants dim),
    `binary_cooling_floor`'s fresh-build test, and the sky tables. Refits follow
    `just fit-check`'s flags in the train's wiring.
  - **Accept:** T43.a's tests pass through `absolute_magnitude_v`; the census's change recorded in
    Risks.

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
  matter, with a stated tolerance against `Track::lifetime`), ruling 77.3's table, built by
  P06.T38.c–d as the fate table (`FittedFates::lifetime_fitted`, about 1 µs, with each cell's
  validated bound; met); `generate` for a main-sequence dwarf under 10 µs,
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
bumps it again when plan 15's rank table replaces the provisional one (only kicks change). T19.e
rides version 25 (S1), a held swap that lands with the train (decision-bump-plan-after-22,
2026-10-14; updated 2026-10-15). P06.T42.b (ask A2's winds) and P06.T43.b (ask A4's V) move
stellar output too: each is a swap held on a lane branch, which rides version 25 (S1) if it is
ready when that train's wiring starts, else the next galaxy train (decision-galaxy-25-train §1.7,
2026-10-27). P06.T24.b's young, S and carbon classes are held at 26 behind
`PECULIAR_CLASSES_VERSION`, a `StellarRules` field. Every other task adds output on new streams
and moves nothing that existed. Any later
change to a formula,
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
  `star.ns.spin`; `star.ns.field`; `star.ns.geometry`; `star.ns.phase`; `star.ns.glitch_class`
  (P06.T28.b, the Vela-like mark; appended at the registry's end, after plan 09's
  `centre.ev.tidal_disruption`, because the macro's order fixes `ALL`); `star.bh.spin`;
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

Task records as built: [06-stellar-stage/records/](06-stellar-stage/records/), one file per task, each linked from its entry under Tasks.

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
  smooth the draws' effect on old remnants and is not the default. _Round 9:_ ruling 89 makes such
  a table, over (m₀, Z, η) with the remnant draws applied on top, the default for dead range rows
  (T38.c–e); `system_summary` and every other reader keep the full track.
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
- **Cost of briefs on large queries.** A bulge query returns tens of thousands of rows, nearly all
  old and many dead. `include_stellar` is optional for that reason, and the server cache absorbs
  repeats. If it is still too slow the chart can request briefs for the coarse layers only.
  _Round 9:_ T38 routes these rows (ruling 89). T38.a measured a 50 ly bulge query as all dead and
  its hierarchy draw at 50–90 µs a row, about 0.9 s at the 20,000-row cap even through the table.
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
- **Rulings 96 and 93.1–3, as built (round 9, `kick` follow-up).**
  - _96.2._ `KickLawParams::max_speed_km_s` = 1,000: the ordinary map is the log-normal truncated
    there, exp(μ + σ Φ⁻¹(r Φ(b))), b = (ln 1,000 − μ) ÷ σ, so the clamp spans 32–990 km/s and
    the median rank gives 264 km/s. Test 1's K–S reads the truncated CDF.
  - _96.3._ The tests are retargeted: isolated pulsars under 100 km/s in 7–13% (beside Willcox's
    5 ± 2% on the sky, kept); retention under 20 km/s 15–25%; black holes under 12 M☉ 30–70%
    unkicked and 12–50% above 100 km/s; the double-neutron-star toy's carbon–oxygen core, e < 0.3
    in 50–90%; and a new test, the escape share with a 230 km/s rotation against 570 km/s, 12–20%
    (`KickObservables::{isolated_under_100_share, escape_share}`, averaged exactly over isotropic
    directions).
  - _96.4._ The black-hole factor's documentation cites Nagarajan and El-Badry (2025) and Atri et
    al. (2019), with Mandel and Müller's 0.5 as the prescription it departs from.
  - _93.1–2_ are in design note 11 above and in plan 08's T8.
  - _93.3._ Plan 11's `STRIPPED_MARK_MIN_MASS` is replaced by
    `multiplicity::stripped_mark_min_mass(&Composition)`, the companion-stripped window's lower
    end at the system's metallicity (7.20 M☉ at Z = 0.02), read at the system's
    `draw_metallicity`, with a 5.7 M☉ floor below which no root is found.
  - _Measured_ (seed `0x0619d00000000001`, 20,000 stars): reference ln v 5.561 ± 0.630 (K–S
    p = 0.66); isolated pulsars 6.79% under 50 km/s on the sky and 8.65% under 100 km/s; low mode
    19.1%; retention 19.1 / 19.6 / 24.2% under 20 / 50 / 100 km/s; 77.5% of 9,924 surviving
    double neutron stars with e < 0.3 (7.5% with the helium core); black holes 71.0% unkicked,
    and under 12 M☉ 67.1% unkicked and 14.4% above 100 km/s; 15.0% of neutron stars leave the
    disc with the rotation. The 3D mean falls to 269 km/s. Only `stellar/kicks` moved: the two
    ordinary speeds. No multiplicity golden moved, and the T32 search reproduces every pinned ID.
- **SSE's giant radii interpolated in Z, as built (round 9, `zsmooth`; ruling 92).** A departure
  from the printed HPT form that rulings 10, 29 and 40 otherwise follow, made under the owner's
  delegation on a research agent's advice (`_orchestration/research/wd-ifmr/NOTES.md`) and logged
  for the owner's review. It amends design note 5, T4.b, T6.c and T8.a.
  - _Why._ HYPERION matched the published SSE code (to 0.006 M☉ in white dwarf mass) and SSE is
    converged, so there was no bug. The min and max clamps on b1–b3 in the Appendix switch at
    [Fe/H] −2.010, −1.301, −0.859, −0.592, −0.567 and −0.340, between HPT's calibration
    metallicities; the radii of equations 46 and 74 depart there from their own node values by up
    to +0.22/−0.14 dex, and through the Vassiliadis and Wood superwind the white dwarf mass swung
    ±0.08 M☉ about its trend. Detailed models (Meng, Chen and Han 2008; Romero, Campos and
    Kepler 2015) and HPT's own calibration points fall monotonically.
  - _What._ `stellar/sse/calibration.rs` (new): `CALIBRATION_Z`, the seven metallicities'
    ζ and radius coefficients as literals (`CALIBRATION`, held to `ZCoeffs::new` bit for bit by a
    test, so that a metallicity between two costs no evaluation of the Appendix), `RadiusInZ`
    (`Calibrated` within 10⁻¹² of a node, else `Between(ZBlend)`), and `ZBlend`, the stencil of
    three or four nodes and the Hermite basis at ζ, fixed in `ZCoeffs::new`. The slopes are Fritsch
    and Carlson's (1980) monotone ones, Brodlie's weighted harmonic mean inside and the
    shape-preserving three-point formula at the two ends (Fritsch and Butland 1984; Moler's
    `pchip`); they depend on the node radii and are formed at each evaluation, since no weights
    fixed in advance of the data can keep every data set monotone. `gb::RadiusLaw` is the printed
    law at a calibration metallicity and, between two, the law at each stencil node with its scale
    as ln A, ln R being the cubic; `gb::LuminosityPowers` keeps each node's powers. `ZCoeffs` gains
    `radius_in_z` and `radius_coeffs`; `LesserPowerLaw` gains `from_parts` and `exponents`.
    `cheb::ZeroAgeHorizontalBranch` no longer holds its own copy of the giant's law: `radius` takes
    the phase's (the same law at the same mass), which keeps `BluePhase` small.
  - _Acceptance._ At every calibration metallicity both laws are the printed ones bit for bit
    (test), so T12.b at 10⁻⁴, 10⁻³, 0.004, 0.02 and 0.03 and T12.c pass unchanged
    (`tests/sse_reference.rs`).
  - _The ruling's tests_ (all pass). `calibration.rs`: the table is `ZCoeffs::new` bit for bit, a
    calibration metallicity reached through [Fe/H] snaps to its node, the stencils and basis, the
    cubic keeps lines and monotone data and meets the node values. `gb.rs`: both laws are the
    printed ones bit for bit at the seven metallicities, continuous in Z (the jump finder over
    log Z at five (M, L)), and a law at cached powers is the law at the luminosity bit for bit off
    the nodes too; `metal_poor_giants_are_hotter`: at 1 M☉ and 100 L☉, T_eff falls monotonically
    from 4,858 K at [Fe/H] −2.2 to 4,123 K at +0.176 (0.0125 dex steps; it was 4,921 K, then 4,180 K
    at −0.9 and 4,950 K at −0.6), and R_GB at 2,000 L☉ and R_AGB of 1.4 M☉ at 5,000 L☉ rise
    monotonically. `tests/sse_reference.rs::white_dwarf_masses_fall_with_metallicity` (slow, about
    380 full tracks, generator recipes, median draws): 18 initial masses from 0.7 to 3 M☉, each
    white dwarf non-increasing at every point of a 0.125 dex grid from −2.2 to +0.3; the secant
    slope from −2.2 to +0.176 is −0.080 M☉ per dex at 1.5 M☉, −0.090 at 1.75 and −0.099 at 2 (the
    ruling's window is −0.03 to −0.10; Romero et al. give about −0.05, so 2 M☉ sits near the edge).
    Before, 1.5 M☉ went 0.664 → 0.673 M☉ from −1.425 to −1.3 and 0.566 → 0.677 from −0.925 to
    −0.55. `cheb.rs`'s horizontal branch at Z = 0.0005 now pins 6,975 K at 0.8 M☉ (SSE's printed
    form gives 6,912 K; see T7's bullet).
  - _Cost_ (A/B of scratch binaries against `58e3d48`, the integrator as ruling 46 left it after
    rulings 46 and 77.1, in one lock; instructions retired, since the machine ran at load 19–23 and
    2.0–2.1 GHz with `math::exp` at 23–25 ns, where times were ±30%). Off the calibration
    metallicities, a radius costs up to four node laws, ln R each, and the cubic: at [Fe/H] −0.5,
    `Track::full` of 1, 2, 5 and 20 M☉ ×1.66, 1.48, 1.30 and 1.17; `lifetime` ×1.59, 1.52, 1.37,
    1.18; `to_age` at 95% of the life ×1.58, 1.61, 1.36, 1.13; at −1.5, `full` ×1.41, 1.30, 1.35,
    1.14. At a calibration metallicity (solar) +1.5–3%, and `ZCoeffs::new` +2.3% (5,876 → 6,012
    instructions). Main-sequence states read neither law. In `math::exp` units, in the same run,
    `full` of 1 M☉ at −0.5 went from 67,000 to 91,000, so ruling 46's targets (150 µs, about
    20,000 `exp`; `lifetime` 5 µs) are further off; per giant step, where the radius was about 3
    transcendental calls, it is now about 14.
  - _Iron cores_ (ruling 92.4; the P06.T38.a Z sweep rerun: 25 [Fe/H] × 860 masses, median draws).
    Their Z structure is not from these laws and barely moves: the carbon–oxygen core's residual
    from a 5-point cubic at 0.125 dex has p99 9.5 × 10⁻³ → 7.3 × 10⁻³; a cubic on a 0.25 dex axis
    misses 10⁻³ at 19.1% → 17.7% of points (p99 2.7 × 10⁻² → 1.9 × 10⁻²); sign reversals of the
    slope stay at a median of 1 (max 5 → 4). The kinks sit near [Fe/H] −2.1 for 8–14 M☉ and near
    +0.05 for 40–85 M☉ (Z ≈ 0.022, where 1 − b47 < 0 ends the blue phase above `M_FGB`, see
    `gb::r_hei`). The white dwarfs, by contrast, move from 89.8% to 8.7% of points over 10⁻³ at
    0.25 dex (median 7.8 × 10⁻³ → 2.1 × 10⁻⁴), and 67% → 2.2% at 0.125 dex; death ages are
    unchanged in structure. This is input for ruling 92.6's re-survey of the fate table (T38.c–d),
    which stays open.
  - _Goldens_ (re-blessed at 11; the bump waits for the version-12 batch, ruling 92.5). Four moved,
    each an off-node star whose giant or gap radius, or whose wind through it, now reads the cubic:
    `stellar/summaries` (7 of 18 systems, all evolved or dead: white dwarf masses by up to 1.6%,
    death times by up to 0.9 Myr, and their L, R, T_eff and B − V); `planetary/systems/subgiant`
    (T_eff 5,288 → 5,271 K through R_EHG, the mass by 6 × 10⁻⁷ M☉ and the planets' μ with it);
    `planetary/systems/red_giant` (T_eff 4,484 → 4,509 K, so both engulfments come about 2 Myr later
    and the wind's mass loss moves the survivors' orbits by 6 × 10⁻⁵); and
    `planetary/systems/fallback_black_hole` (the 32.4 M☉ progenitor's winds read R: the black hole
    is 9.94 M☉, not 10.14, which moves the unbinding times and the survivors' orbits). The
    `stellar/sse` backbone golden, pinned at the calibration metallicities, did not move. The T32
    search still finds all fifteen pinned systems, so nothing is re-pinned.
  - _Files._ In `stellar/sse/`: `calibration.rs` (new), `coeffs.rs`, `gb.rs`, `cheb.rs` and a `mod`
    line in `mod.rs`; none of the `briefs` lane's fast path (`evolve.rs`, `track.rs`,
    `track/build.rs`). `ZCoeffs::{giant_radius_scale, agb_radius_scale}` are now test-only, and
    `cheb::ZeroAgeHorizontalBranch::{at_mass, radius}` changed signature.
- **Ruling 110, as built (round 9, `rem07`).** Points 1, 3, 5 and 7 move output, in one batch.
  - _T21 (points 1–4)._ `neutron_star::TIMING_RADIUS` (10 km) sets k = 9.77 × 10⁻⁴⁰ s G⁻², which
    withdraws ruling 27's consequence for k (the radius bullet above); the decay luminosity keeps
    `neutron_star_radius`'s 12.2 km. The field is the equatorial one
    (`birth_field`, `PulsarState::field`, the wire's `magnetic_field_g`), log B ~ N(12.95, 0.6),
    and P₀ ~ N(0.25, 0.10) s. `GLITCH_AGES` is gone: `GLITCH_NU_DOT_LIMIT` (10⁻¹⁰·⁵ Hz s⁻¹) sets
    one onset per star, where the unglitched |ν̇| = k B² ÷ P³ reaches it (closed form on the
    floor, bisection in B ÷ B₀ to the last bit while the field decays; zero for the many stars
    born below it, which then glitch from birth, so the constant-field test reads √(P₀² + 2 × 0.99
    kB²t)). Measured: 12.2% born above 4.4 × 10¹³ G (window 8–40%), 84 active magnetars (20–300),
    and design note 13's re-check weighted by the beam's swept sky fraction × P^−1.5 Ṗ^0.5: median P
    0.646 s (0.4–0.9) and log Ṗ −14.84 (−15.2 to −14.2), with 40% of stars alive at a uniform age
    to 100 Myr.
  - _T25 (points 5–6)._ `rotation::FOSSIL_FIELD_INCIDENCE` and `fossil_field_incidence` replace
    `FOSSIL_FIELD_SHARE`; `FOSSIL_FIELD_MIN_MASS` is 1.4 M☉. The incidence reads the main-sequence
    star's mass, not its initial mass, which no caller of `fossil_field` holds; they differ by the
    main sequence's winds. Ap and Bp (every fossil-field star, weighted by Salpeter × t_MS ∝
    M^−4.85 over 1.4–5 M☉) are 1.6% (window 1–3%; Sikora's 1.6%). The fossil-field stars stand in
    for the class, whose 7,000–20,000 K cuts only the coolest 1.4–1.5 M☉ stars, at 0.5% incidence. MH08's law is no longer
    provisional.
  - _T26 (points 7–8)._ The Cepheid profile is linear from the blue edge to its peak 300 K inside
    it, then linear to the red edge, in kelvin; where the strip is under 600 K wide (log L below
    about 2.25) the peak sits mid-strip (the lane's choice). Type II Cepheids, which share the
    strip, keep 4x(1 − x): Bono et al.'s models are classical Cepheids'. The 5 M☉ test stays at
    [Fe/H] = −0.5; the backbone's short solar loop is recorded against P06.T14–T16 (Anderson et
    al. 2016, Table A.1: 5 M☉ at Z = 0.014 enters the strip at P 2.95–4.81 d).
- **The spectral scale of young objects (ruling 135.2, from `sub13b`'s P13.T5.a).** The classifier
  types every living object on the dwarf scale (Pecaut and Mamajek 2013, Mamajek's table
  v2022.04.16; M6V 2,810 K, M5.5 written from 2,994 K down, M6 from 2,869 K). Young objects near the
  hydrogen-burning limit, still contracting at ruling 42.1's Hayashi temperature, come out M5.5
  from 5 to 80 Myr, which BHAC15 supports. A catalogue on Luhman et al.'s (2003) young scale, M6 at
  2,990 K and warmer than the dwarf scale by about a subtype, would call them M6.5–M7; Herczeg and
  Hillenbrand's (2014) newer young scale, M6 at 2,860 K, gives M5.5 too. The dwarf scale is kept.
- **Ruling 147.2, as built (lane `kick147`, 2026-09-30; moves output, provisional bless at 15 for
  the version 16 batch).** `ReferencePopulation::score`'s members are the iron-core neutron stars
  that take the ordinary mode: the companion-stripped mark is applied (`binarity::is_stripped`,
  through `kick::stripped_mark_applied`, which `StandardKickLaw::with_stripped_mark` now calls)
  and the star's own mode draw is read by `KickLawParams::takes_low_mode`, which the law's kick
  reads too. `tables::kick_rank` is task revision 1 (10⁶ members, fresh-score acceptance mean
  5.6000, sd 0.6774; largest rank shift 0.062), its fingerprint gains the stripped share at 12, 20
  and 40 M☉, and `cluster_retention` is re-emitted. Test 1 asserts on every ordinary-mode neutron
  star: n 9,131, ln v mean 5.557 and sd 0.631, K–S p 0.87; reported, unstripped 6,979 at 5.436
  and 0.607, companion-stripped 2,152 at 5.952 and 0.540. The fresh ranks are uniform (p 0.52).
  Retention 0.2740 / 0.2783 / 0.3232, low mode 0.2740; test 5 0.771; test 6 0.711 unkicked,
  light 0.670 and 0.136; under 100 km/s 0.0903.
  - Isolated pulsars under 50 km/s on the sky 0.0708, against 3–7% (expected about 0.04): held at
    the measured value. Ruled 2026-10-23 (decision-p15-revalidation Q2): a finding, accepted and
    deferred, its window unchanged; P06.T19.e re-pins it at its re-measured value.
  - Neutron stars above the escape speed 0.1159, against 12–20% (expected about 0.15): held at the
    measured value, and ruled as the line above.

### Asked by rendering plan R06

Drafted by R06.T1 (2026-10-02); accepted 2026-10-19 by the sign-off agent under the owner's
delegation (`signoff-12.md`, item 29), A2 amended. Rendering plan R06
(`docs/agent/plans/rendering-and-planets/06-the-sky.md`, "Named asks of galaxy plan 06" and Design
note 19) consumes each with an interim of its own until it lands here; none changes this plan's
output until its task is built, and each that does is a generator-version change made here, not in
R06.

- **A1, `BriefModel` over the retarded interval** (R06's open question 17). The sky places every
  star at its emitted time, up to some 2.6 × 10⁵ years before the query's (the light time across
  the galaxy), while `BriefModel`'s routes are tested only over the clock window. Ask: a
  constructor such as `BriefModel::new_for(galaxy, record, earliest_emitted)` whose routes are
  tested from the earliest emitted time to +H. **R06's interim:** for an emitted time outside the
  clock window the census reads `SystemStars::generate(..).brief_at`, as plan 12's observed mode
  does (P12.T0's finding), which is exact and dearer. **Switched by:** R06.T8.b's brief, the one
  call site, once A1 lands. _Accepted 2026-10-19 (`signoff-12.md`, item 29); no output moves.
  Its task is P06.T41._
- **A2, the Humphreys–Davidson limit** (R06's open questions 13 and 17). Under the default
  `WindRecipe::Modern` (`stellar/sse/wind.rs`), a star beyond the limit loses the constant
  `MODERN_LBV` (1.5 × 10⁻⁴ M☉/yr), below the Nieuwenhuijzen–de Jager rate it replaces on the cool
  side (8.1 × 10⁻⁴ at log L 6.3 and 4,000 K), so crossing the limit slows the wind and the tracks
  keep cool supergiants above log L 5.8 for some 2 × 10⁵ years, about 1.3 mag brighter at the top
  than any observed cool supergiant. Ask: a ruling on whether the tracks keep them. R06's research
  lean (medium-high): enforce the limit through mass loss, not a photometry clamp, by flooring the
  Modern rate at the cool-side rate and adding Hurley, Pols and Tout's LBV term at their log L 5.78
  line (Humphreys and Davidson 1979 and 1994; Davies, Crowther and Beasor 2018 place red
  supergiants' upper limit near log L 5.5, which is left as it is); layer E's brightest then
  reaches M_V ≈ −9.7. **R06's interim:** the census follows the tracks as they stand, and the cap
  rule reads R06's brightness envelope, so a ruling moves the caps with no change in R06.
  **Switched by:** none needed (R06.T6.b's envelope and R06.T7's caps re-read the tracks).
  _Ruled 2026-10-19 (`signoff-12.md`, item 29): the tracks must not keep them. Past the limit the
  Modern rate is max(`MODERN_LBV`, the cool- or hot-side rate) plus Hurley, Pols and Tout's LBV
  term (HPT 2000 §7.1), as COMPAS's `HURLEY_ADD`; the builder re-checks the remnant masses, adds
  the term after the Eddington boost, checks `envelope_knots`, and R06's envelope and caps are
  re-measured. Its task is P06.T42: the rate lands pure (T42.a), and the swap, held on a lane
  branch, rides version 25 (S1) if ready, else the next galaxy train (decision-galaxy-25-train
  §1.7, 2026-10-27)._
- **A3, protostars dark in V** (R06's open question 13). `absolute_magnitude_v` treats a Class 0/I
  protostar (`premain::protostar_class`) as a living star with a V magnitude, but its envelope's
  extinction (A_V ≳ 100 for Class 0, tens to about 100 for Class I; André, Ward-Thompson and
  Barsony 1993; Whitney et al. 2003) hides it in V. Ask: Class 0/I has no absolute V magnitude;
  Class II takes no circumstellar term, the birth cloud's extinction being plan 07's. R06's lean
  (high). **R06's interim:** the census and the luminosity function apply the rule themselves in
  one function, `sky::photometry::is_dark_in_v`. **Switched by:** R06.T5.a's `sky::photometry`,
  which then calls this plan's rule. _Accepted 2026-10-19 (`signoff-12.md`, item 29): plan 06's
  rule, as built in `stellar/photometry.rs` since CS.T1; the pole-on Class I sources stay a
  recorded finding._
- **A4, V magnitudes of white dwarfs and of M giants.** `photometry.rs` gives white dwarfs no
  absolute V, and reads the bolometric correction at the star's temperature whatever its gravity,
  so a late M giant is up to 1.7 mag too bright at M6 III (the module's own note). Ask: an absolute
  V for white dwarfs from the Montreal cooling grids (Bédard et al. 2020, ApJ 901, 93), and a
  gravity-dependent BC_V for cool giants (R06's lean, medium-high: Fluks et al. 1994 or Worthey and
  Lee 2011 for M giants, Levesque et al. 2005's column for supergiants). The ask includes a check
  of the module's "Straižys and Kuriliene (1981, Ap&SS 80, 353, Table III)" citation for the M
  giants' corrections, as the rendering roadmap's asks table records. **R06's interim:** a white
  dwarf is left out of the census and counted in its `without_photometry` tally, which the `sky`
  response reports (and the view labels `WD NOT MODELLED`); M giants are as bright as this plan
  says. **Switched by:** R06.T5.a's `sky::photometry::absolute_v_of_state`, which then reads this
  plan's V for white dwarfs; R06's census and luminosity function follow it with no other change.
  _Accepted 2026-10-19 (`signoff-12.md`, item 29); a bump. Its task is P06.T43: V and BC_V land
  pure (T43.a), and the swap in `absolute_magnitude_v`, held on a lane branch, rides version 25
  (S1) if built, else the next galaxy train (decision-galaxy-25-train §1.7, 2026-10-27)._
  _Switched under the 26 arm by P06.T43.b (`V_MAGNITUDE_VERSION`): an M6 III giant reads 1.339 mag
  fainter in V; the interim stands to version 25._
- **The envelope integrator's past-the-grid branch had no knot cap (found by P11.T4.h,
  2026-10-04; resolved 2026-10-05 by a guard that moves no output, see "Resolved, as built"
  below).** In `track/build.rs` `envelope_knots`, a knot past the clustered grid steps in age to
  twice the envelope's remaining life at its present rate of loss. Nothing capped that branch, so
  if each step falls short the loop pushes a knot every pass, without end.
  - Each step falls short when the rate of loss drops as the mass nears the core. P11.T4.h's work
    in progress caused that: the early AGB's remnant τ stepped from ≥ 1 to 0 at M = Mc, and its
    wind with it. It hung the R06 census and filled the machine's memory (about 5 MiB/s).
  - Repro: record 0x61f85aa800000001 of seed 0x0600_0029_b000_5eed (the census's galaxy), whose
    0.84 M☉ companion loses its envelope to its wind on the early AGB; run `StarModel::lifetime`
    with the step restored.
  - P11.T4.h removed the step; `stellar_system.rs`
    `a_companion_stripped_by_its_wind_on_the_early_agb_has_a_fate` pins the record.
  - _Resolved, as built (2026-10-05)._ The guard fails loudly, naming the star, instead of
    growing, and is bit-identical on every current record.
    - _What the stall was_ (re-measured with the step restored). It was not a Zeno series of
      shrinking steps.
      - The envelope sat at 3.3 × 10⁻¹⁵ M☉ on 0.5102 M☉, about 30 units in the mass's last place,
        at 2.59 × 10¹⁰ yr.
      - Each knot's 2e ÷ f, 2.65 × 10⁻⁶ yr, rounded up to one unit in the age's last place, 3.81 ×
        10⁻⁶ yr.
      - The midpoints, below the core, saw a wind too weak to move the mass.
      - So each knot moved the age by one unit, 1.4 × 10⁵ yr short of the span's end: about 4 ×
        10¹⁰ knots, or some 3 TB.
    - _The bound._ `MAX_KNOTS_PAST_GRID` = 28 covers steps that resolve the phase's laws.
      - The worst wind whose rate falls no faster than the envelope is a rate in proportion to it.
        It leaves 0.153 of the envelope per interval of 4 midpoint steps.
      - From 150 M☉ such a wind reaches 2⁻⁵⁴ of a 10⁻³ M☉ core in 27 short intervals; one more
        makes 28. A test checks this against the resolution and its doubling.
    - _The stall allowance._ Past the 28 knots the build may go on while the age stays within
      `EnvelopeClock::still_years` of the 28th knot's. That covers stalls at the age's rounding.
      - `still_years` is 2 × 2⁻⁵² × the clock's larger end (Myr) × 10⁶ × the stretch: how long the
        phase's clock can stand still, in years of age.
      - A core read through that clock can stand still where the clock, on the track of the
        star's effective initial mass, runs ahead of the age. Once the clock moves, the core
        catches up by more than the envelope left.
      - A wind continuous over the step cannot stall: it moves the mass by more than the envelope.
      - Each knot moves the age by at least one unit in its last place, so the window allows at
        most 1 + `still_years` ÷ (2⁻⁵³ t) knots more: 23 at a clock 5.5 times the age.
      - The window is checked in age, not by dividing by it, so a zero or NaN age cannot switch
        the guard off. That was the determinism auditor's point against a first draft that counted
        knots.
    - _Why the allowance is needed_ (measured on the R06 census, then the instrumentation removed).
      - 161 envelope phases take more than one knot past the grid, between 5.2 × 10⁵ and 10⁶ of
        them in all: 136 take 2, 18 take 3, 5 take 4 and 2 take 5. All are on the thermally pulsing
        AGB or in core helium burning.
      - All of them have clocks 3.5–7 times the age at the phase's start, envelopes of
        10⁻¹⁶–10⁻¹³ M☉, and steps of one or two units of the age.
      - The 5-knot core-helium-burning case has a clock 5.5 times the age, and its core stood still
        for about 8 units of the age.
      - A flat 28 would have left the galaxy as a whole a margin only up to clocks of about 7 times
        the age.
    - _The error._ `envelope_knots` returns `IntegrateEnvelopeError::Stalled`. It carries the knots,
      the phase and its entry, the last knot's age, mass and envelope, and the span's end, in
      `Years` and `SolarMasses`.
      - It is propagated with `?` through `envelope_segment`, the six envelope phases and
        `Builder::phase` to `Builder::run_from`.
      - `run_from` panics. A stall is a bug in a phase's laws, and the track constructors and
        their 157 call sites stay infallible.
      - The message names the star by the mass it enters its life with, its composition, its
        Reimers η, the options, the resolution and the companion-stripped mark.
      - With the step restored, record 0x61f85aa800000001 fails in 0.7 s under a 1 GB cap, at 45
        knots, 28 and 17 more within the allowance. The message reads: "the track of a star
        entering its life with 0.8411980660928929 M☉ (Composition { z: MetalFraction(0.025711…),
        fe_h: Dex(0.109089…), helium_excess: HeliumExcess(0.0) }, ReimersEta(0.467527…),
        TrackOptions { wind: Modern, remnant: MandelMuller2020, bridges: Physical }, Resolution {
        knots: 16, pulsing_knots: 32, steps: 4 }, companion-stripped mark false) stalled: the
        EarlyAgb phase entered at 2.589473034261505e10 yr with 0.5112103126692104 M☉ still had an
        envelope after 45 knots past its grid: 3.3306690738754696e-15 M☉ of 0.5101685949439533 M☉
        at 2.5899760324016014e10 yr, 1.405475198135376e5 yr before its span's end".
    - _Tests_, in `build.rs`, on synthetic laws:
      - A wind that drops a million-fold at the core, on a clock 5 times the age, trips the guard
        at 44 knots, 28 + 1 + 15, after 8 derivative evaluations a knot. The age moves one unit a
        knot, and the span's end lies over 10¹⁰ knots away. Without the guard the same case ran
        2 × 10⁶ knots unchanged; the test's laws now refuse more than 10⁴ evaluations, so a guard
        that failed fails the test instead of filling memory.
      - The same star with a continuous wind ends in one knot.
      - A core read through a clock 13.5 times the age stalls for more than 28 knots and ends
        within the allowance.
      - The proportional wind from 150 M☉ ends in 26 knots.
    - _Output._ `golden_diff` shows 0 moved, and `GENERATOR_VERSION` is unchanged.
    - _Also found._ `MAX_KNOT_FACTOR` can never fire.
      - Each interval on the grid passes at least one more of its values, so the grid branch takes
        at most n knots, and `built.len() > 2n` is never true there.
      - It is left as it is, since removing it would move nothing.
- **A track built short of its main sequence knows its arrival** (Phase J lane, 2026-10-05, for
  plan 11's protostar mergers; plan 11's Risks, "P11's protostar mergers and build-age dependence,
  fixed"). `Track::main_sequence_arrival` is `main_sequence_start` where the main sequence is
  built, and otherwise the same age from the build's law, `phases::main_sequence_start_years`,
  which `Builder::main_sequence_start` now takes its start from (bit for bit; tested at 0.1–150
  M☉, two metallicities and four build ages). Plan 11's engine started where both stars had
  arrived (where the first has since P11.T4.i) and had read the arrival from the built segments
  alone. Nothing generated by this plan moves.
  - _Found there, for this plan:_ a track's own main sequence ends 1.6 × 10⁴–1.8 × 10⁵ yr after
    `sse::main_sequence_start` + `sse::main_sequence_lifetime` at 3–8 M☉ ([Fe/H] 0, median draws;
    0.14% of the main sequence at 5 M☉, none at 0.8–2 M☉), and before it from 12 M☉ (the
    accreting τ₀). Nothing reads the closed form as the end but plan 11's track reaches, which
    now check the track itself (`binary::evolve::track_reaching`).
- **The local V light is low (a pointer from rendering plan R06, 2026-10-06,
  `decision-r06-t9b-band.md`; for this plan's owner, not a ruling on this plan).** Near the Sun
  the fixture's V light is 0.0417 L☉ pc⁻³, 26% under Flynn et al. 2006's 0.056 ± 10%, uniformly
  across magnitudes, so the deficit is light per unit mass in the turnoff stars and clump giants
  (M_V 4 to 0), and the column's light is 21% (0.26 mag) low. A calibration finding for plans 02
  and 06 (and plan 11 for the pair light): the tracks' turnoff and clump light is one of the four
  candidates, with plan 02's SFH, young disc height and Chabrier's scale. The measurement, and the
  comparison of the fixture's Φ(M_V) with Hipparcos/CNS5 that parts them, are in plan 02's Risks,
  "The local V light is low". A fix is a `GENERATOR_VERSION` change.
- **`stellar_fates_low` and `_mid` went stale by rerun at generator version 21, and were refitted
  at 21 (Phase J lane, 2026-10-07).** At 21, before the refit, `hyperion-fit check --rerun-fast`
  reproduced neither committed file. `just fit-check` and `hyperion-fit`'s
  `the_committed_tables_are_fresh` (which runs `Rerun::None`) still passed, since neither task's
  sim fingerprint probes moved.
  - _What moved._ No route or death age moved. Only these white-dwarf nodes did, 85 of their 111
    moved values by a unit in the eighth significant digit and 4 by more than 10⁻⁶ relative:
    - `_low`: 101 of 5,376 nodes, 98 `BridgedCarbonOxygenWhiteDwarf` and 3
      `CarbonOxygenWhiteDwarf`. The mass `a` moved in 32 of them, by at most 2 × 10⁻⁸ M☉. `b`
      moved in 76: the knee's log₁₀ L by at most 1.1 × 10⁻⁵ dex, and the cooling origin by at
      most 6.3 × 10⁻⁸ Myr;
    - `_mid`: 3 of 5,760 nodes, all `BridgedOxygenNeonWhiteDwarf`, in `b` alone, by at most
      10⁻⁷ dex.

    Each cell's validated bound is 3 times its worst error at its validation points: a median of
    5.9 × 10⁻⁴ in `a` and 3.1 × 10⁻⁴ in `b` in `_low` (the files' acceptance lines). The moves
    sit far inside it.

  - _The cause: P11.T4.h alone_ (ee47311, landed as c07edb86; the early AGB's core and
    small-envelope remnant at SSE's τ), which the AGB cores of `FateNode::of`'s full tracks read.
    Both reruns were made in scratch worktrees. At 97c969c (version 20) the rerun reproduces both
    committed bodies. At ee47311 it gives version 21's bodies bit for bit.
  - _The ruling, changed the same day_ (the orchestrator, `status.md`, 2026-10-07). It first kept
    both tables as committed for 21, with a refit in the version-22 batch. It changed that to a
    refit at 21, before the batch lands, for four reasons:
    - No version-21 output has been published, so re-blessing at 21 makes no (seed, 21)
      ambiguity.
    - Kept, the tables would leave `just test-slow` (which ends with `check --rerun-fast`) and
      `just ci-slow` red for every lane for as long as 21 lasts. That could hide a third
      `RerunDiffers`, which is the forgotten-bump guard.
    - `hyperion-fit run` would have accepted a later refit at 21, since their `since` was 13. It
      would have moved every reader of `FittedFates` under (seed, 21) without a bump.
    - The 22 batch would have refitted them anyway.
  - _The refit, as built._ `hyperion-fit run stellar_fates_low --since 21` and `_mid` took 2 s and
    1 s on 16 threads under the heavy lock. Their bodies are the attribution reruns' to the byte.
    R06's `sky_binary_light_*` tables were then refitted again on them (R06's Risks, "Generator
    version 21"), and every golden re-blessed at 21.
  - _What the refit moved downstream: nothing measured._
    - The bless passed 3,808 of 3,808, and `golden_diff` against the bump finds no golden changed.
    - R06's three luminosity build fingerprints held at their values from the first refit.
    - The binary-light refit on the new fates wrote the same three bodies to the byte.

    The moved white-dwarf nodes reach none of them. Since the refit, `check --rerun-fast` reports
    no stale table.

- **Deferred findings (2026-10-13 rule) (P11's F13; decision-p15-revalidation Q5, 2026-10-23).**
  Recorded and deferred, with no task and no bump: the remnant law should read the stripped mark,
  as the kick law does. Binary-stripped stars have smaller carbon–oxygen cores, and Schneider,
  Podsiadlowski and Müller (2021, A&A 645, A5, Table 1) move black-hole formation for case B
  strips from about 34 to about 67.5 M☉. Today a pinned primary stripped by its binary keeps this
  plan's single-star remnant, capped at its mass (plan 11's design note 16): a documented
  simplification, not a wrong class. If a ruling schedules it, it takes P06.T40, rides the first
  galaxy train after its gate, and reruns P06.T19.d, P09.T9.b's retention test and the
  `cluster_retention` fit.
