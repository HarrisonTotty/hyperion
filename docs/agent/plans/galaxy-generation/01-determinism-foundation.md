# Plan 01: Determinism foundation

- **Milestone:** M1.
- **Depends on:** none.
- **Brainstorm sections covered:** "Determinism foundation" in full ("Random streams, not a random
  sequence", "Floating point", "Generator version", "Time", "Coordinates" except the frame-selection
  rule, "Identifiers"); the ID tables of "Dense features: clusters and the galactic centre"; the
  event word and the two-step keying of "Events in time"; the dependency and cache rules of "Runtime
  and code shape" that bind the sim crate; "Testing" (golden tests, order independence, and the
  infrastructure for statistical tests); step 1 of "Suggested order of attack", and the reservations
  step 3 demands.

## Goal

When this plan is done `hyperion-sim` has the layer that the brainstorm says "must be right first":
counter-based random streams keyed by seed, domain tag and object ID; hand-written samplers;
integer-threshold decisions; two-step event keys; every transcendental function behind
`hyperion_sim::math` on an exactly pinned `libm`, enforced by Clippy; `GENERATOR_VERSION`; the
universe clock with H, L and the source horizon; the three coordinate frames, the galactic axes and
the named directions; unit newtypes; and the 64-bit system ID with every layout the brainstorm
sketches, canonical and rejecting anything else, with `BodyId`, the event word, the hex wire form
and the derived designation. Beside it stands the test infrastructure every later plan consumes: the
golden-file harness, the order-independence helper, hand-written statistical helpers, the slow-test
marking with `just test-slow`, Criterion under `just bench`, and the CI wiring. Nothing visible
changes in the app; every later plan builds on these names.

## Scope and non-goals

In scope: everything listed under Goal, with golden files pinning each bit layout, keying rule and
sampler output, and statistical tests of each sampler.

Not in scope:

- Anything that reads a density, a potential or a parameter of a galaxy. Plan 02.
- Resolving an ID (candidate count, index check, acceptance test). This plan decodes and validates
  the _form_ of an ID. Whether a well-formed ID names a system is plan 03's `resolve`, and for the
  reserved layer plans 09 and 10.
- The tidal radius and the frame-selection rule of "Coordinates". The tidal radius needs the
  potential tables (plan 02, `potential::PotentialTables::tidal_radius`). The selection rule with
  its hysteresis needs the range query, and plan 03 owns it as `galaxy::frame`. This plan supplies
  only the frame newtypes and the `Frame` enum they select between.
- Wire types. `hyperion-protocol` is plan 04's. This plan fixes the text forms (`SystemId` as 16
  hexadecimal digits and so on) as `Display` and `FromStr` on the sim types, which plan 04 wraps.
- The event constructions themselves (Poisson bins, monotone phase). Plan 06 builds them on the key
  convention fixed here.
- The existing `Simulation` struct in `crates/hyperion-sim/src/lib.rs`. It is left as it is.

## Provides

All paths are under `hyperion_sim` unless a crate is named. Signatures are sketches.

### `hyperion_sim::math`

Free functions over `f64`, each a thin `#[inline]` wrapper of the pinned `libm`: `sin`, `cos`,
`sin_cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`, `tanh`, `asinh`, `acosh`,
`atanh`, `exp`, `exp2`, `exp10`, `exp_m1`, `ln`, `log2`, `log10`, `ln_1p`, `powf`, `cbrt`, `hypot`,
`erf`, `erfc`, `ln_gamma`, `gamma`, and the hand-written `powi(x: f64, n: i32) -> f64`. `f64::sqrt`,
`abs`, `floor`, `ceil`, `round`, `trunc`, `min`, `max`, `copysign`, `rem_euclid` and the four
operators are IEEE-exact and are used directly.

### `hyperion_sim::version`

```rust
pub struct GeneratorVersion(u32);            // Debug, Clone, Copy, Eq, Ord, Hash, Display
pub const GENERATOR_VERSION: GeneratorVersion;   // re-exported at the crate root; starts at 1
impl GeneratorVersion { pub const fn new(v: u32) -> Self; pub const fn get(self) -> u32;
                        pub fn is_supported(self) -> bool; }   // true only for the current one
```

### `hyperion_sim::rng`

```rust
pub struct Seed(u64);                         // re-exported at the crate root; Copy, Eq, Ord, Hash
impl Seed { pub const fn new(value: u64) -> Self; pub const fn get(self) -> u64; }
// Display/FromStr = 16 lower-case hex digits, as for SystemId (ParseSeedError)
pub struct DomainTag { /* name: &'static str, hash: u64, scope: TagScope */ }
pub enum TagScope { Galaxy, Cell, Feature, System, Body, Event, SelfTest }
domain_tags! { /* the single registry, in rng/tags.rs; one entry per tag: */
               /* GALAXY_CELL_CANDIDATES: Cell = "galaxy.cell.candidates"; */ }
pub mod tags { pub const ALL: &[DomainTag]; /* one `pub const NAME: DomainTag` per entry */ }
pub const fn hash_tag_name(name: &str) -> u64;                 // FNV-1a, 64-bit

pub fn threefry2x64_20(key: [u64; 2], counter: [u64; 2]) -> [u64; 2];

pub struct ObjectKey { /* word: u64, sub: u16, scope: TagScope */ }
impl ObjectKey { pub fn galaxy() -> Self; pub fn galaxy_item(n: u64) -> Self;
                 pub fn cell(word: u64) -> Self; pub fn feature(word: u64) -> Self; }
impl From<SystemId> for ObjectKey; impl From<BodyId> for ObjectKey;   // in `id`

pub struct Stream { /* key, counter word 0, sub, next word index */ }     // Clone, Debug
impl Stream {
    pub fn open(seed: Seed, tag: DomainTag, object: ObjectKey) -> Self;
    pub fn next_u64(&mut self) -> u64;
    pub fn word_at(&self, n: u64) -> u64;      // random access, does not advance
    pub fn position(&self) -> u64; pub fn seek(&mut self, n: u64);
    // samplers (rng/sample/*.rs)
    pub fn uniform(&mut self) -> f64;                  // [0, 1)
    pub fn uniform_open_low(&mut self) -> f64;         // (0, 1]
    pub fn uniform_open(&mut self) -> f64;             // (0, 1)
    pub fn uniform_in(&mut self, lo: f64, hi: f64) -> f64;
    pub fn below(&mut self, n: NonZeroU64) -> u64;     // unbiased integer in [0, n)
    pub fn standard_normal(&mut self) -> f64;
    pub fn standard_normal_pair(&mut self) -> (f64, f64);
    pub fn normal(&mut self, mean: f64, sigma: f64) -> f64;
    pub fn log_normal(&mut self, mu_ln: f64, sigma_ln: f64) -> f64;
    pub fn log_normal_dex(&mut self, median: f64, sigma_dex: f64) -> f64;
    pub fn poisson(&mut self, mean: f64) -> u64;
    pub fn power_law(&mut self, law: &PowerLaw) -> f64;
    // decisions (rng/decide.rs)
    pub fn mark(&mut self) -> Mark;
    pub fn decide(&mut self, t: Threshold) -> bool;
    pub fn pick(&mut self, t: &Thresholds) -> Option<usize>;
}
pub const POISSON_PTRS_MIN_MEAN: f64 = 10.0;
pub struct PowerLaw;            // new(exponent, lo, hi) -> Result<_, BuildPowerLawError>
pub struct PiecewisePowerLaw;   // continuous(breaks, exponents), with_coefficients(..),
                                // truncated(lo, hi), integral(lo, hi), moment(lo, hi, p), pdf, cdf,
                                // sample(&mut Stream)
pub struct PiecewiseLinear;     // new(knots, densities), integral, pdf, cdf, sample(&mut Stream)
pub struct Mark(u64);           // a fixed 53-bit uniform kept as an integer
pub struct Threshold(u64);      // from_probability(p), from_ratio(value, bound), ALWAYS, NEVER
pub struct Thresholds;          // from_weights(weights: &[f64], bound: f64)
impl Mark { pub fn is_below(self, t: Threshold) -> bool; pub fn pick(self, t: &Thresholds)
            -> Option<usize>;
            // the same answer as `Thresholds::from_weights(weights, bound)` then `pick`, without
            // allocating and stopping at the first hit: the thinning hot path
            pub fn pick_weighted(self, weights: &[f64], bound: f64) -> Option<usize>; }
// event keys (rng/event.rs)
pub struct EventKey([u64; 2]);
impl EventKey { pub fn derive(seed: Seed, tag: EventTag, subject: EventSubject) -> Self;
                pub fn bin_stream(&self, bin: EventBin) -> Stream;
                pub fn event_stream(&self, bin: EventBin, j: u8) -> Stream; }
```

### `hyperion_sim::units`

Newtypes over `f64` with `value()`, `new()`, arithmetic within the unit, scaling by `f64`, ratio to
`f64`, `total_cmp`, and `From` conversions between units of one dimension:

- SI, used inside: `Metres`, `Seconds`, `Kilograms`, `MetresPerSecond`, `Radians`, `Kelvin`,
  `Watts`.
- Edge units: `LightYears`, `Parsecs`, `Kiloparsecs`, `AstronomicalUnits`, `SolarRadii`, `Years`,
  `Megayears`, `Gigayears`, `SolarMasses`, `JupiterMasses`, `EarthMasses`, `KilometresPerSecond`,
  `Degrees`, `SolarLuminosities`, `PerYear` (angular frequencies such as Ω and κ, in radians per
  Julian year), `PerCubicLightYear`, `PerSquareLightYear`.
- `units::consts`: `SPEED_OF_LIGHT`, `METRES_PER_LIGHT_YEAR`, `METRES_PER_AU`, `METRES_PER_PARSEC`,
  `SECONDS_PER_JULIAN_YEAR`, `GRAVITATIONAL_CONSTANT`, `GM_SUN`, `GM_JUPITER`, `GM_EARTH`,
  `SOLAR_MASS_KG`, `SOLAR_RADIUS_M`, `SOLAR_LUMINOSITY_W`.

### `hyperion_sim::time`

```rust
pub struct UniverseTime { /* seconds: i64, nanos: u32 in 0..1e9 */ }   // Copy, Eq, Ord, Hash
pub struct Span { /* same representation, signed */ }
pub const CLOCK_WINDOW_H: Span;        // 1,000 Julian years = 31,557,600,000 s
pub const LIGHT_CROSSING_L: Span;      // 2^18 Julian years = 8,272,635,494,400 s
pub struct SourceHorizon;              // START = -(H + L), END = +H, contains(t)
pub struct ClockWindow;                // START = -H, END = +H, contains(t)
impl UniverseTime { pub const EPOCH: Self; pub fn new(seconds: i64, nanos: u32)
    -> Result<Self, BuildUniverseTimeError>; pub fn from_julian_years(y: i64) -> Option<Self>;
    pub fn checked_add(self, s: Span) -> Option<Self>; pub fn checked_sub(self, s: Span)
    -> Option<Self>; pub fn checked_since(self, earlier: Self) -> Option<Span>;
    pub fn since_epoch(self) -> Span;
    pub fn seconds(self) -> i64; pub fn subsec_nanos(self) -> u32; }   // the wire form, plan 04
impl Span { pub fn new(seconds: i64, nanos: u32) -> Result<Self, BuildSpanError>;
            pub fn seconds(self) -> i64; pub fn subsec_nanos(self) -> u32;
            pub fn as_seconds_f64(self) -> f64; pub fn as_julian_years_f64(self) -> f64;
            pub fn from_seconds_f64(s: f64) -> Option<Self>; /* checked arithmetic, neg, abs */ }
```

### `hyperion_sim::coords`

```rust
pub const ROOT_HALF_WIDTH_LY: i32 = 65_536;
pub struct LyCell { /* x, y, z: i32 */ }    // Eq, Ord, Hash; new([i32; 3]), to_array() -> [i32; 3]
pub struct GalacticPosition { /* cell: LyCell, offset: [f64; 3] metres in [0, 1 ly) */ }
pub struct GalacticDisplacement; pub struct GalacticVelocity;    // f64 metres, metres per second
pub struct SystemPosition; pub struct BodyPosition;              // f64 metres from the frame origin
pub enum Frame { Galactic, System(SystemId), Body(BodyId) }
pub enum CellSize { Ly4, Ly8, Ly16, Ly32, Ly64, Ly128 }          // log2_ly(), ly(), k()
pub struct GenCell { /* size: CellSize, x, y, z: i32 */ }        // of_ly_cell, origin, in_root_cube
impl GenCell { pub fn position_from_words(&self, words: [u64; 3]) -> GalacticPosition; }
impl GalacticPosition { pub fn new(cell, offset) -> Result<_, BuildGalacticPositionError>;
    pub fn cell(&self) -> LyCell; pub fn offset_metres(&self) -> [f64; 3];  // wire form, plan 04
    pub fn from_light_years(ly: [f64; 3]) -> Option<Self>; pub fn to_light_years_f64(&self)
    -> [f64; 3]; pub fn displacement_to(&self, other: &Self) -> GalacticDisplacement;
    pub fn translated(&self, d: GalacticDisplacement) -> Option<Self>;
    pub fn distance_to(&self, other: &Self) -> Metres; pub fn in_root_cube(&self) -> bool;
    pub fn to_cylindrical(&self) -> Cylindrical; pub fn directions(&self) -> Option<Directions>; }
pub struct Cylindrical { /* radius: Metres, azimuth: Radians from +x towards +y, height */ }
pub struct Directions { /* coreward, rimward, spinward, antispinward, north, south: unit */ }
```

### `hyperion_sim::id`

```rust
pub struct SystemId(u64);        // validated; Copy, Eq, Ord, Hash; Display/FromStr = 16 hex digits
pub enum Layer { A, B, C, D, E, BrownDwarf, RoguePlanet }        // values 0..=6
pub const RESERVED_LAYER_VALUE: u8 = 7;
impl Layer { pub fn value(self) -> u8; pub fn cell_size(self) -> CellSize;
             pub fn cell_size_ly(self) -> u32; pub fn cell_bits_per_axis(self) -> u32;
             pub fn index_bits(self) -> u32; pub fn letter(self) -> char;
             pub const ALL: [Layer; 7]; }
impl SystemId { pub fn from_raw(raw: u64) -> Result<Self, DecodeSystemIdError>;
    pub fn from_parts(layer: Layer, cell: GenCell, index: u32) -> Result<Self, BuildSystemIdError>;
    pub fn raw(self) -> u64; pub fn kind(self) -> SystemIdKind; pub fn layer(self) -> Option<Layer>;
    pub fn cell_word(self) -> u64;  /* the ID with its index zeroed: a cell's ObjectKey word */
    pub fn designation(self) -> Designation; }
pub enum SystemIdKind { Grid(GridId), FeatureMember(FeatureMemberId), Centre(CentreMemberId),
    Stream(StreamMemberId), DwarfCore(DwarfCoreMemberId), Pinned(PinnedId),
    Catalogue(CatalogueSystemId) }                  // each converts `Into<SystemId>` infallibly
pub struct GridId;               // layer(), cell() -> GenCell, index() -> u32
pub struct FeatureRef;           // cell: FeatureCell (5 bits per axis, 4,096 ly), index < 2^14
pub enum MemberSlot { InCell { band: Layer, level: u8, cell: [u8; 3], index: u16 },
                      FeatureLevel { index: u16 } }
pub struct BodyId { /* system: SystemId, body_index: u16 */ }     // "0123456789abcdef.0003"
pub struct EventTag(u16); pub struct EventBin(i64 /* within ±2^39 */);
pub struct EventWord(u64);       // tag(), bin(), number() -> u8; new(tag, bin, j); from_raw
pub enum EventSubject { System(SystemId), Body(BodyId) }
pub struct EventId { /* subject, word */ }                        // "<subject>:<16 hex digits>"
event_tags! { /* registry in id/event_tags.rs: number, name, DomainTag of scope Event */ }
pub struct Designation;          // Display, FromStr; SystemId::try_from(&Designation)
```

Errors (`ParseSeedError` lives in `rng`): `DecodeSystemIdError`, `BuildSystemIdError`,
`ParseSystemIdError`, `ParseBodyIdError`, `ParseEventIdError`, `DecodeEventWordError`,
`ParseDesignationError`, each a concrete enum.

### Crate `hyperion-testkit` (new, a dev-dependency only)

```rust
hyperion_testkit::golden!("rng/streams", actual: &str);        // tests/golden/rng/streams.golden
pub struct golden::GoldenWriter;  // header(version), line(..), u64_hex, f64 (bits and decimal)
pub fn order::assert_order_independent<K, V: PartialEq + Debug>(keys: &[K], f: impl Fn(&K) -> V);
pub mod stats { pub const ALPHA: f64 = 1e-3;
    pub fn regularised_gamma_q(a: f64, x: f64) -> f64; pub fn normal_cdf(z: f64) -> f64;
    pub fn poisson_pmf(k: u64, mean: f64) -> f64; pub fn poisson_cdf(k: u64, mean: f64) -> f64;
    pub fn chi_square_gof(observed: &[u64], expected: &[f64]) -> ChiSquare;  // merges thin bins
    pub fn ks_one_sample(samples: &mut [f64], cdf: impl Fn(f64) -> f64) -> KolmogorovSmirnov;
    pub fn ks_two_sample(a: &mut [f64], b: &mut [f64]) -> KolmogorovSmirnov;
    pub fn poisson_two_sided_p(observed: u64, mean: f64) -> f64;
    pub fn poisson_interval(mean: f64, alpha: f64) -> (u64, u64);
    pub fn assert_p_value(name: &str, p: f64, alpha: f64);
    pub fn assert_poisson_count(name: &str, observed: u64, mean: f64, alpha: f64); }
```

### Commands and conventions

- `just test-slow`, `just bench`, `just bless`; `just ci` gains `test-slow`.
- A slow test is marked `#[ignore = "slow: <reason>"]`. Nothing is ignored for any other reason.
- Golden files: `crates/<crate>/tests/golden/<name>.golden`, first line `# generator_version = <n>`.
- Cargo profile `slow-test`: release optimisation with debug assertions and overflow checks on.

## Consumes

Nothing from other plans. Existing repository pieces: the workspace `Cargo.toml` and its lints,
`justfile`, `.github/workflows/ci.yml`, `.claude/rules/rust-dev.md`.

## Design notes

Decisions the brainstorm leaves open. Each is part of the generator version unless it says
otherwise.

1. **Generator: Threefry2x64-20.** Both candidates pass the brainstorm's tests. Threefry wins on
   portability and auditability: it uses only 64-bit wrapping add, rotate and xor, so the Rust is
   twenty lines with no 128-bit multiply (Philox needs the high half of a 64 × 64 product, which
   WebAssembly and older targets emulate); Random123 publishes known-answer vectors for exactly this
   variant, so the implementation is pinned against an outside authority and not only against
   itself; 20 rounds is the authors' conservative default where 13 already pass BigCrush; and it is
   the variant in widest use (JAX, NumPy's sibling implementations), so its behaviour on structured
   counters is the best studied. Philox2x64-10 is a few nanoseconds faster per block on x86-64,
   which the brainstorm has already called nothing next to a density evaluation.
2. **Keying.** Key words are `(seed, tag hash)`. Counter word 0 is the object's 64-bit word. Counter
   word 1 is `sub << 48 | block`, where `sub` is a 16-bit sub-object number and `block` a 48-bit
   draw number. For a system, cell, feature or the galaxy `sub` is 0. For a body `sub` is its
   `body_index`, which is how "its body index shares the second counter word with the draw number".
   A block yields two output words, so a stream's word `n` is output `n & 1` of block `n >> 1`, and
   a stream holds 2⁴⁹ words. The generator version is not folded into the key, so a version bump
   moves only what its code change moves.
3. **A tag fixes what its counter word names.** Every tag is declared with a `TagScope`, and
   `Stream::open` debug-asserts that the `ObjectKey`'s scope matches. This is what makes it safe for
   a cell's word (an ID with the index zeroed) to equal the ID of that cell's candidate 0, and for
   body index 0 to share `sub = 0` with its system: the two are never opened under the same tag.
4. **One tag registry.** The interface sketch names a free-standing `domain_tag!`. The collision
   test can only be total if every tag is listed in one place, so this plan provides a block macro,
   `domain_tags!`, used once, in `rng/tags.rs`. It emits one `pub const` per tag, the slice
   `tags::ALL`, and a `const` assertion that fails compilation on a duplicate name, a duplicate hash
   or a malformed name. An entry reads `CONST_NAME: Scope = "tag.name";`. Every plan adds its tags
   to that file, in a section headed by its plan number, in the task that first opens a stream under
   them; `DomainTag` has no public constructor, so there is no other way to make one, and that
   includes the `fit.*` tags of `hyperion-fit` (plan 15). The tags behind event tags are ordinary
   entries of scope `Event`, which `event_tags!` refers to by constant (P01.T6.e), so `tags::ALL` is
   total. Tag names match `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+`.
5. **Tag hash: FNV-1a, 64-bit**, over the name's UTF-8 bytes, as a `const fn`. It only has to be
   injective over a few hundred names, which the assertion proves; Threefry does the mixing.
6. **Words to floats.** A uniform uses the top 53 bits of a word: `u53 = word >> 11`.
   `uniform = u53 × 2⁻⁵³` lies in [0, 1) and `uniform_open_low = (u53 + 1) × 2⁻⁵³` in (0, 1]. The
   open form uses the top 52 bits, `uniform_open = ((word >> 12) + 0.5) × 2⁻⁵²`, and lies in (0, 1).
   All three are exact in `f64`.
7. **Thresholds are 53-bit.** `Threshold::from_probability(p) = ceil(p × 2⁵³)` as a `u64`, and a
   decision is `u53 < threshold`. Multiplying by 2⁵³ is exact, so the threshold is a pure function
   of `p`'s bits; it gives exactly the outcome of `uniform() < p`; `p = 1` gives 2⁵³, above every
   `u53`, so it always accepts; `p = 0` never does. A 64-bit threshold cannot represent "always".
8. **Normal by Box–Muller**, not a rejection method, so that a normal always consumes exactly two
   words. `ln` takes `uniform_open_low`, which is never 0. The tail is cut at 8.57σ (probability
   10⁻¹⁷), which is documented.
9. **Poisson switch at a mean of 10**, the lower limit of Hörmann's PTRS. Below it, inversion by
   sequential search on one word.
10. **Power law through `exp_m1` and `ln_1p`**, which is stable as the exponent approaches 1, with
    the exact limit form inside `|1 − α| < 10⁻⁸`. The rogue planets' mass function falls "nearly as
    1 ÷ mass", so the neighbourhood of 1 is not a corner case.
11. **"Piecewise" is two samplers**: a piecewise power law (the mass functions) and a piecewise
    linear density on knots (tabulated age distributions and anything fitted offline). Other closed
    forms, such as a truncated exponential, are inverse transforms that their owners write from
    `uniform` and `math`.
12. **Layer values.** A–E are 0–4, brown dwarfs 5, rogue planets 6, and 7 is the reserved value.
    Mass-band fields inside reserved-layer IDs then use the same numbers, with 7 as the band field's
    spare value. See Risks for the wording this reads.
13. **Spare bits sit directly under the layer field**, bits 60–58, and the index is always
    low-aligned. The rogue-planet layout is then the same layout with the cell field grown upwards
    into the spare bits, and one mask extracts the index of any grid ID.
14. **Axis order and offsets.** Cell coordinates are stored x, then y, then z, from the high bits
    down, each as `coordinate + 2^(bits − 1)`. IDs therefore sort by layer, x, y, z, index.
15. **Padding in the `10` layouts.** The brainstorm's fields do not fill the 57 bits after the
    sub-kind. Fields are low-aligned, the padding sits directly under the sub-kind and must be zero.
    Sub-kind `11` is rejected.
16. **Inner cells are not canonical.** In a nested grid a cell of level j ≥ 1 whose three
    coordinates all lie in the inner half (4–11 of 16, or 8–23 of 32 at the centre) covers volume
    that level j − 1 owns. An ID naming it is rejected, because one system must have one ID.
17. **The spare band value** (7) means a feature-level member in every nested layout: catalogue
    feature, centre and dwarf core. Level and cell must then be zero. The brainstorm states this for
    catalogue features and makes the central black hole "member zero", which implies it for the
    centre. For a stream, band 7 is rejected: the brainstorm gives streams no feature-level members.
18. **Catalogue-system cells.** "Coarser classes zero the low bits" depends on a class registry that
    plan 09 owns. This plan decodes the field widths only and offers
    `CatalogueSystemId::is_aligned_to(cell_log2_ly)`; plan 09's resolve applies it. IDs from saves
    and the protocol always go through resolve, so canonicity still holds end to end.
19. **Pinned content** (`110`) is an opaque 58-bit number. The overlay brainstorm will give it
    structure; any value is well-formed until then.
20. **Years are Julian years** (31,557,600 s), the year of the light-year's definition, so that one
    light-year is exactly `c` × one year and H and L are exact integers of seconds.
21. **`UniverseTime` is normalised like `std::time::Duration` but signed**: nanoseconds are always
    0–999,999,999 and the instant is `seconds + nanos × 10⁻⁹`, so −0.5 s is `(−1, 500_000_000)`. The
    derived `Ord` is then chronological. All arithmetic is checked.
22. **System and body frames are translations**: axes parallel to the galactic axes, origin at the
    barycentre or the body's centre. Rotating body-fixed frames belong to plan 14.
23. **A position draw uses one word per axis**: the top `log2(cell size)` bits are the integer
    light-year inside the cell and the next 52 bits the offset fraction, so a position is never one
    `f64` across a coarse cell. 52 bits, not 53, because `(1 − 2⁻⁵³) × 1 ly` rounds to within one
    unit in the last place of a whole light-year and the offset must stay strictly below it.
24. **Text forms.** `SystemId`: exactly 16 lower-case hexadecimal digits; upper case is rejected so
    that one ID has one string. `BodyId`: the system, a full stop, four digits. `EventId`: the
    subject, a colon, 16 digits.
25. **Designations** are derived from the ID alone, are bijective with it, and are not part of the
    generator version: the format may change without moving a star, because saves hold IDs. Format
    in P01.T6.g.
26. **Test helpers live in a new crate, `hyperion-testkit`**, not in a module of the sim. A module
    would need a feature flag and a self-referential dev-dependency to reach integration tests. The
    crate depends only on the pinned `libm`, never on the sim (a cycle would duplicate the sim's
    types inside its own unit tests), and nothing depends on it except as a dev-dependency.
27. **Slow tests are `#[ignore = "slow: …"]`**, run by
    `cargo test --profile slow-test -- --ignored`. It needs no feature flag, shows in every test
    listing, and because CI runs all ignored tests nothing can be parked silently. The profile keeps
    debug assertions on, because the brainstorm's bound checks are debug assertions and the slow
    tests are where they bite.
28. **Statistical tests use fixed seeds and α = 10⁻³.** With fixed seeds a test is deterministic: it
    cannot flake, it can only start failing when generated output changes. When a deliberate version
    bump trips one, the rule is to run it under three other seeds; two failures out of three is a
    real defect, otherwise the seed is changed in the same commit with a note.
29. **The Clippy list is per crate**, in `crates/hyperion-sim/clippy.toml`, so that the server, the
    testkit and `hyperion-fit` stay free to use `std`. Clippy takes the nearest `clippy.toml` and
    does not merge, so a future root file must repeat nothing the sim's file needs.
30. **`libm` with `default-features = false`.** The `arch` feature only swaps in hardware
    instructions for correctly rounded functions, but turning it off leaves one code path to reason
    about. The sim uses `f64::sqrt` from `std`, so nothing is lost.
31. **Float bits are never hashed**, enforced by adding `f64::to_bits` and `f32::to_bits` to the
    sim's disallowed methods. `ObjectKey` constructors take integers only. The golden writer, which
    prints bits, lives in the testkit.

## Tasks

Order and parallelism: T1 first. Then T2, T3, T4.a and T7.a–b can run in parallel. T4.b and T5
follow T3; T6 follows T5, and T6.e also needs T7.b; T7.c follows T6.a and T6.e; T8 and T9 follow
T7.c, and T8's subtasks are independent of each other after T8.a; T10 follows T6.e and T7.c; T11 and
T12 close the plan. Each task leaves `just ci` green.

Every task obeys `.claude/rules/rust-dev.md`: doc comments with units and sources on every public
item, `#[must_use]`, no `as` without an `#[expect]` and a reason, no `unwrap()` outside tests.

### P01.T1 Test infrastructure

#### P01.T1.a The `hyperion-testkit` crate and the golden harness

Build: a new library crate. `golden::check(manifest_dir, name, actual)` reads
`<manifest_dir>/tests/golden/<name>.golden` and compares it with `actual` byte for byte. On a
mismatch it panics with the path, the first differing line number, both lines, and the sentence "if
this change is intended, bump GENERATOR_VERSION and run `just bless`". With `HYPERION_BLESS=1` it
writes the file instead (creating directories) and passes; if `CI` is also set it panics, so CI can
never bless. The macro `golden!(name, actual)` supplies `env!("CARGO_MANIFEST_DIR")` from the
calling crate. `GoldenWriter` builds the text: `header(version: u32)` writes
`# generator_version = <n>`; `u64_hex(label, v)`; `f64(label, v)` writes
`label = 0x<16 hex digits>  # <shortest round-trip decimal>` and panics on a NaN, whose bits are
unspecified; `line(&str)`. `golden::check` also fails if the file's header version differs from the
version the test passes in, which is what forces goldens to be regenerated in the commit that bumps
the version.

Files: `crates/hyperion-testkit/{Cargo.toml,src/lib.rs,src/golden.rs}`; root `Cargo.toml`
(`hyperion-testkit` and `libm = { version = "=0.2.16", default-features = false }` under
`[workspace.dependencies]`; take the newest 0.2.x at execution time and record it here); `justfile`
(`bless`: `HYPERION_BLESS=1 cargo test --workspace`).

Tests: a match passes; a mismatch panics with the line number (`#[should_panic(expected = ..)]`); a
header mismatch panics; bless writes into a temporary directory; bless under `CI` panics; NaN
panics.

Acceptance: `cargo test -p hyperion-testkit` passes; `just ci` green.

#### P01.T1.b Special functions and chi-square

Build `stats`: `regularised_gamma_q(a, x)` by the series for `x < a + 1` and the modified Lentz
continued fraction otherwise (Numerical Recipes, 3rd ed., §6.2), using `libm::lgamma`; `normal_cdf`
from `libm::erfc`; `poisson_pmf` as `exp(k ln μ − μ − lnΓ(k + 1))`;
`poisson_cdf(k, μ) = Q(k + 1, μ)`; `chi_square_gof(observed, expected)`, which first merges adjacent
bins from each end until every expected count is at least 5, then returns the statistic, the degrees
of freedom (bins − 1) and `p = Q(dof ÷ 2, χ² ÷ 2)`; `assert_p_value`.

Tests, against published values: Q(0.5, 0.5) = 0.317311, Q(5, 10) = 0.029253, Q(50, 40) = 0.929665
(re-derive each from a table or an independent tool and cite it in the test); χ² = 11.07 at 5
degrees of freedom gives p = 0.0500; `poisson_cdf` sums `poisson_pmf` to 10⁻¹²; a fair die sample
passes and a loaded one (one face at 0.2) fails at N = 10⁵.

Acceptance: `cargo test -p hyperion-testkit stats` passes.

#### P01.T1.c Kolmogorov–Smirnov and Poisson intervals

Build: `ks_one_sample` sorts with `total_cmp`, computes D, and returns
`p = 2 Σ_{j≥1} (−1)^(j−1) exp(−2 j² λ²)` with Stephens's `λ = (√n + 0.12 + 0.11 ÷ √n) D`, summed
until a term is under 10⁻¹²; `ks_two_sample` with `n = n₁ n₂ ÷ (n₁ + n₂)`;
`poisson_two_sided_p(n, μ) = min(1, 2 min(P(X ≤ n), P(X ≥ n)))`; `poisson_interval(μ, α)`, the
narrowest `[lo, hi]` with P(X < lo) ≤ α ÷ 2 and P(X > hi) ≤ α ÷ 2, found by stepping the CDF; and
`assert_poisson_count`.

Tests: λ = 1.36 gives p = 0.049; a linear congruential sample against the uniform CDF passes, the
same sample squared fails; `poisson_interval(100, 0.05)` is `(81, 120)` (check against an exact
table); a count inside passes and one outside fails with a message naming both.

Acceptance: `cargo test -p hyperion-testkit stats` passes.

#### P01.T1.d Order-independence helper

Build `order::assert_order_independent(keys, f)`: evaluates `f` over the keys forwards, backwards,
in a fixed pseudo-random permutation (an inline 64-bit LCG, seed constant), and each key alone in a
fresh call, and asserts all four agree per key, naming the key's position on failure. This is the
brainstorm's "generating A then B equals generating B then A equals generating B alone". Pure
functions pass trivially; the helper exists for later plans, whose generators sit behind caches.

Tests: a pure closure passes; a closure with a hidden counter (`Cell<u64>`) fails.

Acceptance: `cargo test -p hyperion-testkit order` passes.

#### P01.T1.e Slow tests, benchmarks and CI wiring

Build:

- Root `Cargo.toml`: `[profile.slow-test]` with `inherits = "release"`, `debug-assertions = true`,
  `overflow-checks = true`; `criterion` under `[workspace.dependencies]` (newest release,
  `default-features = false`).
- `crates/hyperion-sim/Cargo.toml`: `[dev-dependencies]` `hyperion-testkit`, `criterion`; a
  `[[bench]] name = "foundation"`, `harness = false`.
- `crates/hyperion-sim/benches/foundation.rs`: a Criterion main with one placeholder group, filled
  by later tasks.
- `justfile`: `test-slow` (`cargo test --workspace --profile slow-test -- --ignored`), `bench *args`
  (`cargo bench --workspace {{ args }}`), and `ci` gains `test-slow` after `test`.
- `.github/workflows/ci.yml`: the Rust job gains `just test-slow`. Benchmarks are compiled by the
  existing `--all-targets` steps and never run in CI: a regression is a finding, not a failure.
- One slow test in the testkit (`#[ignore = "slow: chi-square at N = 10^7"]`) so the recipe has
  something to run.
- A sentence each in the root `README.md`'s command list for `test-slow`, `bench` and `bless`.

Tests: the slow test itself.

Acceptance: `just test-slow` runs at least one test and passes; `just test` reports it ignored;
`just bench -- --test` completes; `just ci` green.

### P01.T2 `math`: pinned `libm`, the Clippy list, golden function values

Build:

- `crates/hyperion-sim/Cargo.toml`: `libm.workspace = true`, the sim's only runtime dependency.
- `src/math.rs`: the wrappers listed under Provides. `powi` is hand-written: exponentiation by
  squaring over the bits of `|n|` from the lowest, multiplying the accumulator in that fixed order,
  and `1 ÷ result` for negative `n` (with `i32::MIN` handled through `unsigned_abs`). Module docs
  state the rule, the reason (brainstorm, "Floating point") and that `+ − × ÷` and `sqrt` are used
  directly.
- `crates/hyperion-sim/clippy.toml`: `disallowed-methods`, each entry with a `reason` pointing at
  `hyperion_sim::math`. For both `f64` and `f32` the list is: sin, cos, tan, sin_cos, asin, acos,
  atan, atan2, sinh, cosh, tanh, asinh, acosh, atanh, exp, exp2, exp_m1, ln, log, log2, log10,
  ln_1p, powf, powi, cbrt, hypot; and `to_bits`, with the reason "float bits are never hashed; print
  bits through the testkit only".
- Golden function values, `tests/golden/math/functions.golden`, written by
  `tests/foundation_golden.rs`: for every wrapper six to ten arguments chosen to cross its
  algorithm's branches (for `exp`: 0, 1, −1, 10⁻¹⁰, 709, −745, 0.5 ln 2 ± ε; for `ln`: 1, 2, 10, a
  subnormal, 1 ± 2⁻⁵²; for the circular functions: 0, 1, π ÷ 4 and π as `f64`, 10⁶, 10²²; for
  `powf`: (2, 0.5), (10, −0.35), (0.08, −1.3), (150, −2.3), the mass-function exponents; for
  `ln_gamma`: 1, 2, 0.5, 10.5, 3 × 10⁵; and so on), no NaN results.

Tests: the golden; a sanity test that the pinned values agree with reference constants to one unit
in the last place (`exp(1)` = `0x4005BF0A8B145769`, `ln(2)` = `0x3FE62E42FEFA39EF`, `sin(1)` =
`0x3FEAED548F090CEE`, `cos(1)` = `0x3FE14A280FB5068C`); `powi(x, n)` equals repeated multiplication
bit for bit for n = 0–8 and equals `1 ÷ powi(x, −n)` for negative n; `powi(2, 1023)` is finite and
`powi(2, 1024)` is infinite.

Acceptance: `just ci` green; adding `let _ = 1.0_f64.sin();` to the sim makes `just lint` fail with
the configured reason (check by hand once, do not commit); the golden passes.

### P01.T3 `version` and `units`

Build `version.rs` as under Provides, with the crate-root re-export and a doc comment restating the
brainstorm's rule (any change to output bumps it, goldens regenerate in the same commit, before the
first release bumps are free). `GENERATOR_VERSION` is 1.

Build `units.rs`: a private `unit!` macro generating each newtype with `new`, `value`, `Debug`,
`Clone`, `Copy`, `PartialEq`, `PartialOrd`, `Default`, `Add`, `Sub`, `Neg`, `Mul<f64>`, `Div<f64>`,
`Div<Self, Output = f64>`, `total_cmp`, and a doc comment stating the unit. No `Eq` or `Hash`: they
hold floats. `From` conversions in both directions within a dimension, always through the SI unit
and a named constant. Constants with their sources in the doc comments: c = 299,792,458 m/s (exact);
the au = 149,597,870,700 m (IAU 2012 B2, exact); the Julian year = 31,557,600 s; the light-year =
9,460,730,472,580,800 m (exact, and exactly representable in `f64`: test it); the parsec = 648,000 ÷
π au; G = 6.67430 × 10⁻¹¹ (CODATA 2018); GM☉ = 1.3271244 × 10²⁰, GM of Jupiter = 1.2668653 × 10¹⁷,
GM of Earth = 3.986004 × 10¹⁴ m³ s⁻², R☉ = 6.957 × 10⁸ m, L☉ = 3.828 × 10²⁶ W (IAU 2015 B3 nominal
values); masses in kilograms are GM ÷ G. Re-check each against its resolution when writing it.

Files: `src/version.rs`, `src/units.rs`, `src/lib.rs` (module declarations, crate docs).

Tests: the light-year constant equals `c × year` exactly in integer arithmetic and round-trips
through `f64`; each conversion pair round-trips to 10⁻¹⁵ relative; 1 pc = 3.2615637… ly;
`GENERATOR_VERSION.is_supported()`; a `compile_fail` doctest showing `Metres + LightYears` does not
compile.

Acceptance: `cargo test -p hyperion-sim units version` passes; `just ci` green.

### P01.T4 `time`

#### P01.T4.a `UniverseTime` and `Span`

Build as under Provides and Design note 21. `new` rejects `nanos ≥ 10⁹`, for `Span` as for
`UniverseTime`; both represent a negative value as floored whole seconds plus non-negative
nanoseconds, and `seconds()` and `subsec_nanos()` return exactly those two fields, so that
`new(t.seconds(), t.subsec_nanos()) == t`. Plan 04's wire conversion is built on these getters and
on `Span::new`. `Span::from_seconds_f64` returns `None` for non-finite or out-of-range input and
otherwise floors to the nanosecond. `as_seconds_f64` is documented as lossy beyond 2⁵³ ns and is
what closed forms take. `Display` prints `T+<seconds>.<9 digits> s` or `T-…`.

Tests: ordering across zero (`(−1, 999_999_999) < (0, 0)`); add and subtract carry and borrow
nanoseconds in both signs; overflow returns `None`; `t.checked_add(s)?.checked_since(t) == s` over a
table of edge values; `new(x.seconds(), x.subsec_nanos()) == x` for both types over the same table,
negative values included; the brainstorm's claim that an `f64` of seconds resolves about a
millisecond at the far end of the horizon (the gap between adjacent `f64` values at 8.3 × 10¹² s is
2⁻¹⁰ s), which is the reason for the type.

#### P01.T4.b H, L and the horizons

Build `CLOCK_WINDOW_H`, `LIGHT_CROSSING_L`, `ClockWindow`, `SourceHorizon` as under Provides. Doc
comments state that both constants belong to the generator version and quote the brainstorm's
definitions.

Tests pin: H = 31,557,600,000 s; L = 8,272,635,494,400 s; the horizon runs from −8,304,193,094,400 s
to +31,557,600,000 s; L in light-years (262,144) exceeds the root cube's diagonal, 131,072 × √3 =
227,023.4 ly, computed from `coords::ROOT_HALF_WIDTH_LY` once T5 lands (until then from a literal);
`contains` is inclusive at both ends.

Files: `src/time.rs`. Acceptance: `cargo test -p hyperion-sim time` passes; `just ci` green.

### P01.T5 `coords`

#### P01.T5.a Galactic positions, displacements and generation cells

Build `LyCell`, `GalacticPosition` (canonical: each offset component in `[0, 1 ly)`, finite; `new`
rejects anything else, `translated` renormalises by carrying whole light-years into the cell with
checked `i32` arithmetic; `cell()` and `offset_metres()` return the two fields, so that
`new(p.cell(), p.offset_metres()) == p`, which is what plan 04's wire conversion uses),
`GalacticDisplacement`, `GalacticVelocity`. `displacement_to` subtracts the integer cells first,
converts that difference to metres, and then adds the offset difference, so that nearby positions
far from the origin lose nothing. `CellSize` and `GenCell`: `GenCell::of_ly_cell(cell, size)` is an
arithmetic shift right by `log2_ly` on each axis, which is "divided by the cell size, rounded down";
`origin()` is the low corner as an `LyCell`; `in_root_cube()` is −65,536 ≤ coordinate × size and
(coordinate + 1) × size ≤ 65,536 on every axis. `position_from_words` per Design note 23: for each
axis `ly = word >> (64 − log2_ly)` and `fraction = (word << log2_ly) >> 12` (52 bits), offset =
`fraction × 2⁻⁵² × METRES_PER_LIGHT_YEAR`, with a debug assertion that the offset is below one
light-year.

Tests: resolution (the gap between adjacent offsets just under one light-year is 2 m, the
brainstorm's "about 2 m anywhere"); `of_ly_cell` on negative coordinates (−1 → −1 at every size, −8
→ −1 and −9 → −2 at 8 ly); the planes x, y, z = 0 are cell faces at every size;
`position_from_words` with all-ones words stays inside the cell and with zero words is the origin; a
property loop over 10⁴ LCG-generated positions: `a.translated(a.displacement_to(b))` is within 4 m
of `b` for separations under 100 ly anywhere in the cube; `from_light_years` and
`to_light_years_f64` round-trip to 10⁻⁹ ly; `new(p.cell(), p.offset_metres())` returns `p` for every
position of that loop.

#### P01.T5.b Frames, axes and named directions

Build `SystemPosition`, `BodyPosition`, `Frame`, with conversions that take the frame's origin
explicitly (`SystemPosition::to_galactic(&self, barycentre: &GalacticPosition)` and its inverse,
likewise body to system). No conversion exists between frames without an origin, which is the
compile-time guard the brainstorm asks of the newtypes. Module docs define the axes as the
brainstorm does: origin at the galactic centre, +x along the bar's long axis, +z galactic north,
rotation counter-clockwise seen from the north, arms trailing. `to_cylindrical` gives R, the azimuth
from +x towards +y through `math::atan2`, and z. `directions()` returns `None` when R = 0 and
otherwise coreward = (−x, −y, 0) ÷ R, spinward = (−y, x, 0) ÷ R, north = (0, 0, 1) and their
opposites, with R from the full position (cell and offset) in metres. Docs note that (rimward,
spinward, north) is right-handed and that the directions are local.

Tests: at (+R, 0, 0) spinward is +y; at (0, +R, 0) spinward is −x and coreward is −y; coreward ·
spinward = 0 and both are unit to 10⁻¹⁵; `None` on the z axis; rimward × spinward = north; a
`compile_fail` doctest adding a `SystemPosition` to a `BodyPosition`.

Files: `src/coords.rs` (or `src/coords/{mod,galactic,frames}.rs`), golden
`tests/golden/coords/positions.golden` (twelve `position_from_words` results across all six sizes).
Acceptance: `cargo test -p hyperion-sim coords` passes; `just ci` green.

### P01.T6 `id`

Bit 63 is the most significant. Every field not listed is zero, and a non-zero bit there is
rejected. All extraction goes through two private helpers, `field(raw, hi, lo)` and
`with_field(raw, hi, lo, value)`, tested on their own.

#### P01.T6.a Grid layout for the five stellar layers

| Layer | Value | Cell (ly) | k   | Spare   | Cell x  | Cell y  | Cell z  | Index  | Index capacity |
| ----- | ----- | --------- | --- | ------- | ------- | ------- | ------- | ------ | -------------- |
| A     | 0     | 8         | 0   | [60:58] | [57:44] | [43:30] | [29:16] | [15:0] | 65,536         |
| B     | 1     | 16        | 1   | [60:58] | [57:45] | [44:32] | [31:19] | [18:0] | 524,288        |
| C     | 2     | 32        | 2   | [60:58] | [57:46] | [45:34] | [33:22] | [21:0] | 4,194,304      |
| D     | 3     | 64        | 3   | [60:58] | [57:47] | [46:36] | [35:25] | [24:0] | 33,554,432     |
| E     | 4     | 128       | 4   | [60:58] | [57:48] | [47:38] | [37:28] | [27:0] | 268,435,456    |

The layer is bits [63:61] in every layout. In general, with a = 14 − k bits per axis and n = 16 + 3k
index bits, z starts at bit n, y at n + a and x at n + 2a. A stored coordinate is the generation
cell's coordinate plus 2^(a − 1), so it is unsigned and every bit pattern of the cell field is
valid.

Build `Layer`, `GridId`, `SystemId::{from_raw, from_parts, raw, kind, layer, cell_word}`,
`DecodeSystemIdError::{SpareBitsSet, ..}`,
`BuildSystemIdError::{CellOutsideRootCube, CellSizeMismatch, IndexTooLarge}`. `from_parts` requires
the `GenCell`'s size to be the layer's.

Tests: a table of hand-computed IDs (layer A, cell (0, 0, 0), index 0 is `0x0200_0800_2000_0000`;
layer E, cell (−512, −512, −512), index 0 is `0x8000_0000_0000_0000`; layer E, cell (511, 511, 511),
index 2²⁸ − 1 is `0x83FF_FFFF_FFFF_FFFF`; recompute each by hand when writing the test); round trip
over every corner cell and extreme index of every layer; any of bits 60–58 set is `SpareBitsSet`; an
index of 2^n is `IndexTooLarge`; `Layer::index_bits` and `cell_size_ly` match the table; IDs sort by
layer, then x, y, z, then index.

#### P01.T6.b The substellar layers

| Layer        | Value | Cell (ly) | k   | Spare   | Cell x  | Cell y  | Cell z  | Index  |
| ------------ | ----- | --------- | --- | ------- | ------- | ------- | ------- | ------ |
| Brown dwarfs | 5     | 16        | 1   | [60:58] | [57:45] | [44:32] | [31:19] | [18:0] |
| Rogue planet | 6     | 4         | −1  | none    | [60:46] | [45:31] | [30:16] | [15:0] |

The rogue-planet layer has 15 bits per axis (45 cell bits), takes the three spare bits, and keeps a
16-bit index: 65,536 per 64 ly³ cell, the brainstorm's 1,024 per cubic light-year.

Build: extend `Layer` handling; the layout follows the cell size, not the layer value, so the code
is one function of `CellSize` with the k = −1 case explicit.

Tests: as T6.a for both layers; every 64-bit pattern under layer 6 decodes (nothing to reject) and
round-trips, checked on 10⁵ LCG patterns; the brown-dwarf layout equals layer B's with the layer
bits changed; capacity per cubic light-year computed from `index_bits` and `cell_size_ly` is 128 for
A and 1,024 for the rogue planets.

#### P01.T6.c Reserved layer, prefix `0`: member of a catalogue feature

Under layer value 7, bit 60 is the first prefix bit.

| Field           | Bits    | Width | Notes                                                   |
| --------------- | ------- | ----- | ------------------------------------------------------- |
| Layer           | [63:61] | 3     | 7                                                       |
| Prefix          | [60]    | 1     | `0`                                                     |
| Feature cell x  | [59:55] | 5     | 4,096 ly cells, stored as coordinate + 16               |
| Feature cell y  | [54:50] | 5     |                                                         |
| Feature cell z  | [49:45] | 5     |                                                         |
| Feature index   | [44:31] | 14    | Candidate number in the feature cell                    |
| Mass band       | [30:28] | 3     | 0–6 as `Layer`; 7 marks a feature-level member          |
| Level           | [27:25] | 3     | 0–7                                                     |
| Cell in level x | [24:21] | 4     | 0–15; the feature's centre is the corner of 7 and 8     |
| Cell in level y | [20:17] | 4     |                                                         |
| Cell in level z | [16:13] | 4     |                                                         |
| Index           | [12:0]  | 13    | Candidate number in the cell and band, or member number |

Rules: band 7 requires level and cell to be zero (`FeatureLevelFieldsSet`), and the index then
counts feature-level members from 0 ("member zero"). For level ≥ 1, a cell with all three
coordinates in 4–11 is rejected (`InnerCellOwnedByLowerLevel`).

Build `FeatureCell`, `FeatureRef` (with `object_word()`: the ID with bits [30:0] zero, the feature's
`ObjectKey` word), `MemberSlot`, `FeatureMemberId`, a shared private
`NestedSlotLayout { level_bits, cell_bits_per_axis }` used again in T6.d.

Tests: hand-computed examples; round trip over a grid of field extremes; each rule's rejection with
its specific error variant; level 0 accepts every cell; at level 3 exactly 16³ − 8³ = 3,584 cells
are accepted.

#### P01.T6.d Prefixes `10`, `110` and `111`

Prefix `10` is bits [60:59]; the sub-kind is [58:57]: `00` centre, `01` stream, `10` dwarf core,
`11` rejected (`UnknownSubKind`).

| Sub-kind   | Zero    | Fields, high to low                                                                                 |
| ---------- | ------- | --------------------------------------------------------------------------------------------------- |
| Centre     | [56:35] | band [34:32], level [31:28] (0–11), cell x [27:23], y [22:18], z [17:13] (0–31), index [12:0]       |
| Stream     | [56:54] | number [53:42], band [41:39] (0–6), along [38:25], across a [24:19], across b [18:13], index [12:0] |
| Dwarf core | [56:33] | number [32:31], band [30:28], level [27:25], cell x [24:21], y [20:17], z [16:13], index [12:0]     |

The centre has twelve levels of 32 cells per axis: levels 12–15 are rejected (`LevelOutOfRange`),
the inner-cell rule uses 8–23, and band 7 is its feature-level list, whose member 0 is the central
black hole. The dwarf core follows the catalogue-feature rules exactly, and its low 31 bits have the
same positions. A stream rejects band 7 (Design note 17).

Prefix `110` is bits [60:58]; [57:0] is an opaque 58-bit pinned number.

Prefix `111` is bits [60:58]:

| Field  | Bits    | Width | Notes                                                          |
| ------ | ------- | ----- | -------------------------------------------------------------- |
| Class  | [57:52] | 6     | Registry owned by plan 09                                      |
| Cell x | [51:44] | 8     | 512 ly cells, stored as coordinate + 128                       |
| Cell y | [43:36] | 8     |                                                                |
| Cell z | [35:28] | 8     |                                                                |
| Index  | [27:4]  | 24    | Candidate number                                               |
| Member | [3:0]   | 4     | 0 is the entry itself; others are further members of the entry |

Build `CentreMemberId`, `StreamMemberId`, `DwarfCoreMemberId`, `PinnedId`, `CatalogueSystemId` (with
`is_aligned_to(cell_log2_ly: u32)`, true when the low `cell_log2_ly − 9` bits of each axis are
zero), and the complete `SystemIdKind` dispatch, matched exhaustively.

Tests: hand-computed examples for each layout, including the central black hole, which is
`0xF000_0007_0000_0000` (layer 7, prefix `10`, sub-kind `00`, band 7, everything else zero;
recompute by hand); every padding bit set in turn is rejected with `PaddingBitsSet`; each rule's
error variant; round trips over field extremes; bit-budget assertions as `const` checks (each
layout's widths sum to 64).

#### P01.T6.e `BodyId`, the event word, the event-tag registry

Event word:

| Field | Bits    | Width | Notes                                                  |
| ----- | ------- | ----- | ------------------------------------------------------ |
| Tag   | [63:48] | 16    | From the registry; 0 is never valid                    |
| k     | [47:8]  | 40    | Bin or cycle number, two's complement, −2³⁹ to 2³⁹ − 1 |
| j     | [7:0]   | 8     | Number within the bin                                  |

Build `BodyId` (every `u16` is a valid index; what the indices mean is plan 14's), `EventBin` (`new`
rejects values outside the signed 40-bit range; `from_field` sign-extends), `EventTag`, `EventWord`,
`EventSubject`, `EventId`, and `event_tags!` in `id/event_tags.rs`: each entry is
`number => CONST_NAME = tags::EVENT_CONST`, where the right-hand side is a `DomainTag` of scope
`Event` already declared in `rng/tags.rs` (so the one collision assertion sees it, Design note 4).
Numbers are explicit and never reused, and the macro emits the `EventTag` constant,
`EventTag::name()` (the domain tag's name), `EventTag::domain_tag()`, and a `const` assertion that
the numbers are unique and non-zero and that every domain tag named has scope `Event` and is named
once. `EventWord::from_raw` rejects an unregistered tag. One entry is registered now,
`0x0001 => SELF_TEST = tags::EVENT_SELFTEST` (`"event.selftest"`, added to `rng/tags.rs` by this
task), never emitted by a generator, so that goldens and integration tests have a registered tag.

Tests: k = −1 encodes as forty ones and decodes to −1; k = ±2³⁹ boundaries; an unregistered tag and
tag 0 are rejected with distinct variants; round trips.

#### P01.T6.f Text forms

Build `Display`, `LowerHex` and `FromStr` for `SystemId` (16 lower-case digits, then `from_raw`, so
a malformed layout is rejected at the door with `ParseSystemIdError::Decode(..)`), `BodyId`
(`<system>.<4 digits>`), `EventWord` (16 digits), `EventId` (`<subject>:<word>`). Parsers reject
upper case, a `0x` prefix, whitespace, and wrong lengths, each with its own variant.

Tests: round trips for every ID kind; each rejection; a `u64` above 2⁵³ survives the text form (the
reason the protocol does not send a number).

#### P01.T6.g Designations

Derived from the ID alone, bijective, human-readable, in the manner of Elite's `Sector AB-C d12-3`.
The alphabet is Crockford base 32 (`0123456789ABCDEFGHJKMNPQRSTVWXYZ`). A **sector** is a 4,096 ly
cube, 32 per axis, named by three characters (x, y, z). Layer letters are A–E, `F` for brown dwarfs
and `G` for rogue planets.

| Kind                  | Format                                                        | Example                   |
| --------------------- | ------------------------------------------------------------- | ------------------------- |
| Grid                  | `<sector> <xx><yy><zz> <layer>-<index>`                       | `H7K 4C0RFZ A-7`          |
| Feature member        | `<sector> F<feature index> <band><level>-<x><y><z>-<index>`   | `H7K F212 C3-8F2-40`      |
| Feature-level member  | `<sector> F<feature index> M<index>`                          | `H7K F212 M0`             |
| Centre                | `CENTRE <band><level as one base-32 digit>-<x><y><z>-<index>` | `CENTRE A9-2HJ-12`        |
| Centre, feature level | `CENTRE M<index>`                                             | `CENTRE M0`               |
| Stream                | `STREAM <number> <band>-<along>-<a>.<b>-<index>`              | `STREAM 87 A-5121-3.23-9` |
| Dwarf core            | `DWARF <number> …` as a feature member                        | `DWARF 1 B2-47C-3`        |
| Pinned                | `PIN <number>`                                                | `PIN 42`                  |
| Catalogue system      | `<sector> K<class>-<x><y><z>-<index>` and `/<member>` if ≠ 0  | `H7K K3-052-118/1`        |

In a grid designation the sector is the top five bits of each stored cell coordinate, and `<xx>` is
the remaining 9 − k bits (10 for rogue planets) of that axis as two base-32 digits. In a catalogue
system's, the sector is the top five bits of each 8-bit coordinate and `<x>` the low three as one
digit. Indices and numbers are decimal without padding. A body appends ` /<body index>`.

Build `Designation` (holds the `SystemId`, formats on `Display`), `FromStr` that parses back to the
ID through `from_raw`, `ParseDesignationError`. Docs state that the format is not part of the
generator version and that proper names are a later overlay.

Tests: round trip `id → text → id` for every kind over field extremes and 10⁴ LCG-generated valid
IDs; uniqueness on the same sample (no two IDs share a designation); out-of-range digits (a cell
digit beyond the layer's bits) are rejected; the examples in the table, recomputed by hand.

#### P01.T6.h ID goldens and benchmarks

Build `tests/golden/id/layouts.golden`: some sixty lines of `parts → raw hex → designation` covering
every layout and every layer, written through the public API. Benchmarks in `benches/foundation.rs`:
`SystemId::from_raw` on a mixed batch (target under 5 ns each), `from_parts` for layer A, and
designation formatting.

Files for T6: `src/id/{mod,bits,layer,grid,reserved,body,event,event_tags,text,designation}.rs`.
Acceptance for each subtask: `cargo test -p hyperion-sim id` passes and `just ci` is green; for T6.h
also `just bench -- id` prints the three results.

### P01.T7 `rng` core

#### P01.T7.a Threefry2x64-20

Build `threefry2x64_20(key, counter)` exactly as Salmon et al. (2011) and Random123's `threefry.h`
define it: key schedule `ks = [k0, k1, 0x1BD11BDAA9FC1A22 ^ k0 ^ k1]`; `x0 = c0 + ks[0]`,
`x1 = c1 + ks[1]`; twenty rounds of `x0 += x1; x1 = rotl(x1, R[r mod 8]); x1 ^= x0` with
`R = [16, 42, 12, 31, 16, 32, 24, 21]`; after every fourth round, with `s = (r + 1) ÷ 4`,
`x0 += ks[s mod 3]` and `x1 += ks[(s + 1) mod 3] + s`. All additions wrap. Re-check the constants
against the paper when writing.

Tests, the Random123 known-answer vectors (counter, key → output):

| Counter                                | Key                                    | Output                                 |
| -------------------------------------- | -------------------------------------- | -------------------------------------- |
| `0`, `0`                               | `0`, `0`                               | `c2b6e3a8c2c69865`, `6f81ed42f350084d` |
| all ones, all ones                     | all ones, all ones                     | `e02cb7c4d95d277a`, `d06633d0893b8b68` |
| `243f6a8885a308d3`, `13198a2e03707344` | `a4093822299f31d0`, `082efa98ec4e6c89` | `263c7d30bb0f0af1`, `56be8361d3311526` |

Also: for a fixed key, 10⁵ consecutive counters give distinct outputs (the bijection, spot-checked).
Benchmark: one block, target under 20 ns.

#### P01.T7.b Domain tags

Build `TagScope`, `DomainTag`, `hash_tag_name` (FNV-1a: offset basis `0xcbf29ce484222325`, prime
`0x100000001b3`), the `domain_tags!` macro and `rng/tags.rs` per Design note 4. The file opens with
the rules: a tag is never renamed or removed; a new property group gets a new tag; each plan adds
its tags under its own heading, as `CONST_NAME: Scope = "tag.name";`. Plan 01 registers only
`SELFTEST_STREAM` = `selftest.stream` (scope `SelfTest`) here and `EVENT_SELFTEST` =
`event.selftest` (scope `Event`) in P01.T6.e.

Tests: `hash_tag_name("star.mass")` = `0x76f046feb1feaee9`, `"planet.orbits"` =
`0x00b517d62a08cb6e`, `"moon.count"` = `0xa00dd982cc043ce1` (recompute independently); a runtime
test that no two entries of `tags::ALL` share a name or a hash (the brainstorm's collision test,
which the `const` assertion also enforces at compile time); `compile_fail` doctests for a duplicate
name and for a malformed name; golden `tests/golden/rng/tags.golden` listing name, scope and hash
for every tag, so that a rename or removal shows in review as a changed line and not only as an
addition.

#### P01.T7.c `Seed`, `ObjectKey` and `Stream`

Build per Design notes 2 and 3. `Seed` is a plain newtype with `new`, `get` and the 16-digit text
form of Provides. `impl From<SystemId> for ObjectKey` (scope `System`) and
`impl From<BodyId> for ObjectKey` (scope `Body`, `sub` = the body index) are written in this task,
in `src/id/mod.rs`, because they need both modules. Exact keying:

| Word           | Contents                                                                                                                                                                   |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Key word 0     | The universe seed                                                                                                                                                          |
| Key word 1     | The domain tag's hash                                                                                                                                                      |
| Counter word 0 | The object's word: a `SystemId`'s raw value, a body's system's raw value, a cell word (an ID with its index zeroed), a feature word, or 0 or an item number for the galaxy |
| Counter word 1 | `sub << 48 \| block`: `sub` is 0, or the `body_index` for a body; `block` is the 48-bit draw number                                                                        |

Stream word `n` is output `n & 1` of block `n >> 1`. `next_u64` caches the second word of a block.
`word_at` and `seek` give random access. Reaching block 2⁴⁸ panics, documented under `# Panics` as
unreachable in practice. `Stream` is a plain value: no interior state beyond its position, `Clone`.

Tests: `word_at(n)` equals the n-th `next_u64` for n up to 1,000; two streams that differ in any one
of seed, tag, object word or `sub` share no word among their first 1,000 (checked for adjacent
cells, consecutive candidates, consecutive body indices: the structured inputs the brainstorm
names); `sub` and `block` do not alias (body 1, block 0 differs from body 0, block 2⁴⁸ − 1's
neighbour: assert on the counter words through a private accessor); a scope mismatch panics in debug
(`#[should_panic(expected = "scope")]`, under `cfg(debug_assertions)`); golden
`tests/golden/rng/streams.golden`: the first eight words for twenty (seed, tag, key) triples.
Benchmark: open a stream and draw four words (a thinning candidate's budget), target under 50 ns.

Files for T7: `src/rng/{mod,threefry,tags,key,stream}.rs`. Acceptance for each subtask:
`cargo test -p hyperion-sim rng` passes; `just ci` green; the goldens exist and pass.

### P01.T8 Samplers

Each sampler states in its doc comment how many words it consumes. Each has a fast statistical test
at N = 10⁵ in `just test` and a slow one at N = 10⁷ in `tests/sampler_statistics.rs`, both with
fixed seeds and `stats::ALPHA`, and contributes lines to `tests/golden/rng/samplers.golden` (the
first sixteen values as bits, from `selftest.stream`).

#### P01.T8.a Uniforms and bounded integers

Build: `uniform = (w >> 11) as f64 × 2⁻⁵³`; `uniform_open_low = ((w >> 11) + 1) as f64 × 2⁻⁵³`;
`uniform_open = ((w >> 12) as f64 + 0.5) × 2⁻⁵²`, which is exact and lies strictly inside (0, 1);
`uniform_in(lo, hi) = lo + (hi − lo) × uniform`, documented as able to return `hi` by rounding only
when `hi − lo` is below the spacing of floats at `hi`, and debug-asserting `lo < hi`; `below(n)` by
Lemire's multiply-and-reject method on 128-bit products, unbiased, consuming one word per attempt.
One word each otherwise. The integer-to-float `as` conversions are exact below 2⁵³ and carry an
`#[expect]` saying so.

Tests: the extremes (`w = 0` and `w = u64::MAX`) give 0 and 1 − 2⁻⁵³, 2⁻⁵³ and 1, 2⁻⁵³ and 1 − 2⁻⁵³;
chi-square over 256 bins; KS against the uniform CDF; `below(6)` chi-square; `below(2⁶³ + 1)`
rejects and stays in range.

#### P01.T8.b Normal and log-normal

Build `standard_normal_pair`: `u1 = uniform_open_low`, `u2 = uniform`, `r = sqrt(−2 ln u1)`,
`θ = 2π u2`, result `(r cos θ, r sin θ)` through `math`. `standard_normal` returns the first of the
pair and discards the second, so it always costs two words. `normal`,
`log_normal(μ, σ) = exp(μ + σ z)`,
`log_normal_dex(median, σ_dex) = median × exp(ln 10 × σ_dex × z)`. Sigma must be finite and
non-negative (debug assertion; zero returns the mean).

Tests: `u1` at its smallest gives a finite 8.57; KS against `stats::normal_cdf`; the pair's two
halves are uncorrelated (sample correlation within 4 ÷ √N); log-normal's median and the KS of its
logarithm; the brainstorm's use case as a doctest (a concentration with 0.11 dex of scatter).

#### P01.T8.c Poisson by inversion

Build for `mean < POISSON_PTRS_MIN_MEAN`: one word; `u = uniform`, `p = exp(−mean)`, `s = p`,
`k = 0`; while `u ≥ s` and `k < 256`: `k += 1`, `p *= mean ÷ k`, `s += p`. A mean of 0 returns 0
without drawing. A negative, NaN or infinite mean panics (`# Panics`: a broken invariant in the
caller).

Tests: chi-square against `stats::poisson_pmf` at means 0.01, 0.3, 1.2, 5 and 9.99; at 0.01 the zero
class alone is checked by `assert_poisson_count` on the number of non-zero draws; the cap is
unreachable (u = 1 − 2⁻⁵³ at mean 9.99 stops below 60).

#### P01.T8.d Poisson by PTRS, the dispatch, and the power check

Build for `mean ≥ 10` Hörmann's (1993) PTRS, in the form NumPy uses:

```text
b = 0.931 + 2.53 * sqrt(mean)        a = -0.059 + 0.02483 * b
inv_alpha = 1.1239 + 1.1328 / (b - 3.4)
v_r = 0.9277 - 3.6224 / (b - 2)
loop:
    U = uniform_open - 0.5           V = uniform_open_low
    us = 0.5 - |U|
    k = floor((2 * a / us + b) * U + mean + 0.43)
    if us >= 0.07 and V <= v_r: return k
    if k < 0 or (us < 0.013 and V > us): continue
    if ln(V) + ln(inv_alpha) - ln(a / (us * us) + b)
           <= -mean + k * ln(mean) - ln_gamma(k + 1): return k
```

Two words per attempt. Because U is open, `us` is never 0. The float `k` is tested for sign before
its conversion to `u64`. Supported means run to 2³¹, documented; the brainstorm needs a few hundred
thousand. Re-check the constants against the paper when writing. `poisson` dispatches on
`POISSON_PTRS_MIN_MEAN`.

Tests: chi-square at means 10, 10.01, 50, 1,000 and 3 × 10⁵ (bins pooled to expected counts ≥ 5 by
the helper); sample mean and variance within 5 standard errors; continuity across the switch: the
two methods' samples at mean 10 pass `ks_two_sample`. The power check the brainstorm implies: a
test-only sampler that rounds a normal of mean and variance 1.2 **fails** the same chi-square at N =
10⁵, which proves the test would catch the shortcut. Benchmarks: mean 1.2 (target under 40 ns) and
mean 1,000 (target under 150 ns).

#### P01.T8.e Power law

Build `PowerLaw::new(exponent α, lo, hi)` for a density ∝ x^−α on `[lo, hi]`, rejecting `lo ≤ 0`,
`hi ≤ lo` and non-finite input: the explicit upper limit the brainstorm requires is not optional.
With `g = 1 − α` and `ℓ = ln(hi ÷ lo)`: for `|g| ≥ 10⁻⁸`,
`x = lo × exp(ln_1p(u × exp_m1(g ℓ)) ÷ g)`; otherwise the exponent-1 form `x = lo × exp(u ℓ)`. One
word. Also `integral()`, `pdf`, `cdf`. The result is clamped to `[lo, hi]` against last-place
rounding.

Tests: KS against the analytic CDF at α = 2.3 on [8, 150], α = 1.3 on [0.08, 0.5], α = 1 exactly, α
= 1 ± 10⁻⁹ and 1 ± 10⁻⁶ (continuity: the same word gives values within 10⁻⁶ relative across the
switch), α = −0.5 (rising), α = 0 (uniform); extremes of `u` give `lo` and just under `hi`.

#### P01.T8.f Piecewise samplers

Build `PiecewisePowerLaw`: segments with breaks `b₀ < … < b_m`, exponents, and coefficients either
chained for continuity (`continuous`) or given (`with_coefficients`, which Chabrier's scaled
high-mass branch will need); `integral(lo, hi)`, `moment(lo, hi, p)` (∫ x^p × density, with the
logarithmic case), `truncated(lo, hi)`, `pdf`, `cdf`. Sampling: word 1 picks the segment through
`Thresholds` built once from the segment integrals, word 2 draws inside it by T8.e's formula. Two
words always. `PiecewiseLinear`: knots and non-negative densities, linear between knots; word 1
picks the segment, word 2 gives `t = u (f₀ + f₁) ÷ (f₀ + sqrt(f₀² + u (f₁² − f₀²)))` and
`x = x₀ + t (x₁ − x₀)`, which needs no division by `f₁ − f₀`. Constructors return `Result` and
reject unsorted breaks, negative or all-zero densities and non-finite input.

Tests: a Kroupa-shaped law (α = 0.3, 1.3 and 2.3 with breaks at 0.08 and 0.5 M☉ on [0.01, 150]; take
the exponents from Kroupa 2001) passes KS against its own `cdf`; its band shares over the edges
0.08, 0.5, 0.75, 2.5, 8 and 150 from `integral` are within half a point of the brainstorm's 76, 9.8,
11, 2.3 and 0.64%; `truncated` to a band samples only inside it and passes KS; a discontinuous
two-segment law puts the right share in each segment (`assert_poisson_count`); `PiecewiseLinear` on
a triangle and on a profile with a zero-density segment, KS against `cdf`.

#### P01.T8.g Sampler benchmarks and the slow suite

Build the Criterion group `samplers` (uniform, normal, log-normal, both Poisson regimes, power law,
both piecewise samplers) and complete `tests/sampler_statistics.rs` with every N = 10⁷ test marked
`#[ignore = "slow: …"]`.

Files for T8: `src/rng/sample/{mod,uniform,normal,poisson,power_law,piecewise}.rs`,
`tests/sampler_statistics.rs`, `tests/golden/rng/samplers.golden`, `benches/foundation.rs`.
Acceptance for each subtask: its fast tests pass under `just test`, its slow tests under
`just test-slow`, its golden lines pass; for T8.g `just bench -- samplers` prints every result and
the whole slow suite finishes in under two minutes on a developer machine.

### P01.T9 Integer-threshold decisions

Build `Mark`, `Threshold`, `Thresholds` per Design note 7. `Threshold::from_probability(p)`:
`ceil(p × 2⁵³)`, debug-asserting 0 ≤ p ≤ 1 and clamping in release.
`Threshold::from_ratio(value, bound)`: the thinning form, debug-asserting `0 ≤ value ≤ bound` and
`bound > 0`. That is the brainstorm's "debug assertion that the density never exceeds the thinning
bound", placed where every caller passes through it; in release a violation saturates to `ALWAYS`.
`Thresholds::from_weights(weights, bound)`: running sums in index order (the order is part of the
output), threshold i = `ceil(sumᵢ ÷ bound × 2⁵³)`, debug-asserting the total does not exceed the
bound; `pick` returns the first i with `mark < thresholdᵢ`, or `None`, which is rejection.
`Threshold::from_ratio(value, bound)` is `from_probability(value ÷ bound)`, and
`Mark::pick_weighted(weights, bound)` forms the same running sums and thresholds one at a time and
returns at the first hit, so it allocates nothing and is bit-for-bit `from_weights` then `pick`.
This is the brainstorm's "one uniform draw rejects a candidate or picks its class by the odds".
`Stream::mark` draws one word and keeps `w >> 11`; a `Mark` can be compared against many moving
thresholds, the brainstorm's "one fixed uniform compared against a threshold that moves". Module
docs repeat the brainstorm's caution: this is a convention, reproducibility still rests on `libm`.

Tests: p = 1 always accepts and p = 0 never, over the extreme marks;
`mark.is_below(from_ probability(p))` equals `uniform < p` on the same word for 10⁵ LCG words and
probabilities, including p = 2⁻⁵³ and p just under 1; `pick` frequencies by chi-square, with the
rejected share as a class; `pick_weighted` equals `from_weights` then `pick` for 10⁴ LCG weight
vectors and marks; `from_ratio` with value > bound panics in debug; golden lines for thresholds of
0.1, 1 ÷ 3, 4 × 10⁻⁵ and 1 − 2⁻⁵³.

Files: `src/rng/decide.rs`. Acceptance: `cargo test -p hyperion-sim decide` passes; `just ci` green.

### P01.T10 Event keys, in two steps

Build `rng/event.rs`. Step 1, `EventKey::derive(seed, tag, subject)`: one Threefry block with key
`(seed, tag.domain_tag().hash)` and counter `(subject's system raw value, sub << 48)`, where `sub`
is 0 for a system and the `body_index` for a body; the two output words are the event key. Event
tags have scope `Event` and are never opened as ordinary streams, so block 0 under them is not
shared with anything. Step 2: a `Stream` whose key words are the event key and whose counter is:

| Word           | Contents                                                                                                                      |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Counter word 0 | The bin or cycle number k, its `i64` reinterpreted as `u64` (two's complement)                                                |
| Counter word 1 | `slot << 48 \| block`: slot 0 is the bin's own stream (count, times, thinning); slot j + 1 is event j's own stream, j = 0–255 |

`bin_stream(bin)` and `event_stream(bin, j)` return ordinary `Stream`s, so every sampler works on
them. An `EventId`'s word (tag, k, j) therefore names exactly one stream, and the bin's contents are
"a pure function of (seed, ID, tag, k)" as the brainstorm requires. `Stream` gains a private
constructor from raw key words; the public `open` is unchanged.

Tests: bins k = −3…3 asked in forward, reverse and shuffled order give identical words
(`assert_order_independent`), the brainstorm's "both event constructions return the same events in
any order of asking" at the level this plan owns; k = −1 and k = 2⁴⁰ − 1 are different streams (no
aliasing through the 40-bit field: the counter uses the sign-extended value); slot 0 and event 0
differ; a body's key differs from its system's; golden `tests/golden/rng/events.golden`. Benchmark:
derive a key and open one bin, target under 60 ns.

Acceptance: `cargo test -p hyperion-sim event` passes; `just ci` green.

### P01.T11 Foundation-wide golden, order-independence and determinism tests

Build `tests/foundation_golden.rs` (collects every golden above under one header check against
`GENERATOR_VERSION`) and `tests/foundation_order.rs`: for 64 IDs spanning every layout, and six
tags' worth of streams (using `selftest.stream` with distinct seeds, since only self-test tags exist
yet), a fixed recipe per ID (four uniforms, a normal, a Poisson at 1.2 and at 40, a power law, a
threshold pick) is order-independent under `assert_order_independent`, and two complete runs are
equal, which is the sim's determinism test that `rust-dev.md` requires. A doctest on `rng::Stream`
shows the brainstorm's central promise: drawing a new property under a new tag leaves an existing
property's values unchanged.

Acceptance: `just ci` green; deleting any golden file makes `just test` fail with the harness's
message; changing one Threefry rotation constant fails the known-answer test and every `rng` golden
(check by hand once).

### P01.T12 A second architecture in CI

Build: a CI job `rust-aarch64` on `ubuntu-24.04-arm` that runs
`cargo test -p hyperion-sim -p hyperion-testkit` and `just test-slow`. The goldens, which include
`libm` function values and sampler outputs as bits, then pass on x86-64 and AArch64 on every push,
which turns the brainstorm's portability claim into a check. If the runner is not available to the
repository, the fallback is a `wasm32-wasip1` build of the sim's tests run under `wasmtime`; record
which was used.

Acceptance: both Rust jobs green on a pull request.

## Verification

The plan is done when:

- `just ci` is green, including `just test-slow`, on both CI architectures.
- Each claim is pinned by a named test:

| Claim                                                   | Pinned by                                                   |
| ------------------------------------------------------- | ----------------------------------------------------------- |
| The generator is Threefry2x64-20                        | Random123 known-answer vectors (T7.a)                       |
| Key and counter word contents, `sub` sharing word 1     | `rng/streams.golden`, no-aliasing tests (T7.c)              |
| Tags never collide and are never renamed                | `const` assertion, runtime test, `rng/tags.golden` (T7.b)   |
| Sampler outputs bit for bit                             | `rng/samplers.golden` (T8)                                  |
| Sampler distributions                                   | chi-square, KS, Poisson-count tests, fast and slow (T8)     |
| A rounded normal is not good enough                     | the power check (T8.d)                                      |
| The normal avoids ln 0; the power law's exponent 1      | extremes and continuity tests (T8.b, T8.e)                  |
| Decisions equal `uniform < p`, and p = 1 always accepts | T9                                                          |
| Event keys in two steps, random access in k             | `rng/events.golden`, order test (T10)                       |
| `libm` behaviour                                        | `math/functions.golden`, reference constants (T2), the lint |
| H, L, the source horizon, L against the cube's diagonal | T4.b                                                        |
| Frame resolution, cell alignment, position draws        | T5.a, `coords/positions.golden`                             |
| Axes and named directions                               | T5.b                                                        |
| Every ID layout, canonical encoding and each rejection  | T6.a–e, `id/layouts.golden`                                 |
| Text forms and designations are bijective               | T6.f, T6.g                                                  |
| Order independence and run-to-run determinism           | T11                                                         |
| Cross-platform reproducibility                          | T12                                                         |

- `just bench` reports, on the developer machine, recorded in the pull request: a Threefry block
  under 20 ns; opening a stream and drawing four words under 50 ns; Poisson at a mean of 1.2 under
  40 ns and at 1,000 under 150 ns; `SystemId::from_raw` under 5 ns; an event key and one bin under
  60 ns. These are this plan's own targets, set so that the foundation takes well under a fifth of
  the brainstorm's 1–2 µs for a sparse fine cell (one Poisson draw and one to three candidates of
  two blocks each). Missing one is a finding to raise, not a failure.
- By eye: `cargo doc -p hyperion-sim --open` reads as a specification of the layouts; the tables
  above appear in the module docs of `id` and `rng`.

## Generator version

This plan sets `GENERATOR_VERSION` to 1 and generates nothing a player sees, but every choice in it
is part of every later output: the block function, the key and counter layout, the tag hash, the
word-to-float rules, each sampler's algorithm and word consumption, the threshold rule, the event
key convention, the pinned `libm` version, H and L, and the ID layouts. Changing any of them later
moves every star.

Reserved now so that later plans move nothing they need not, which is what step 3 of the
brainstorm's order of attack demands of the first milestone:

- **The time argument's type**: `UniverseTime`, with H, L and the source horizon as constants. Plans
  02–04 carry it from the start (ages from −H, the query's time, the protocol's time).
- **The ID prefixes**: layer value 7 with `0`, `10` (and its three sub-kinds), `110` and `111`, all
  decoded and validated now though nothing generates them until plans 09 and 10; layer values 5 and
  6 with their layouts, until plan 13; the body index and the event word, until plans 06 and 14.
- **The event-key convention**: two-step keys, the (k, slot, block) counter, the event-tag registry
  with number 0 invalid and numbers never reused.
- **Streams of their own**: the tag registry and its scope rule are the mechanism. The velocity
  draw's tag, `system.velocity`, is registered by plan 03 alongside its placement tags and stays
  unused until plan 08, so adding velocities moves no position, mass or age.
- **Spare capacity**: bits 60–58 of the stellar and brown-dwarf layouts; sub-kind `11` under prefix
  `10`; the padding of the `10` layouts; 58 opaque bits under `110`; catalogue classes up to 64;
  event tags up to 65,535; `sub` values for non-body objects.

The designation format and the text forms are not part of the generator version.

## Risks and open points

- **Which layer value is reserved.** The brainstorm says the two substellar layers "take the last
  layer values", which read literally would make them 6 and 7 and the reserved value 5. This plan
  reads it as "the last values left": brown dwarfs 5, rogue planets 6, reserved 7, so that the 3-bit
  band field of member IDs ("the same bands as the layers, and one spare value") uses the same
  numbers with 7 special in both. Nothing else in the brainstorm depends on the choice. It is fixed
  by the first golden file.
- **Where the spare bits sit.** The brainstorm's table lists "Spare" last, which suggests the low
  end. Design note 13 puts them under the layer field. The brainstorm calls its table a sketch, and
  the field widths are unchanged.
- **Catalogue-system index width.** The research notes behind the brainstorm give 28 bits; the
  brainstorm gives index 24 and member 4. The brainstorm wins, and the widths sum to 64.
- **Rules the brainstorm implies but does not state**, adopted because "one system has one ID":
  inner cells of nested levels are rejected (Design note 16); band 7 applies to the centre and dwarf
  cores and not to streams (17); centre levels 12–15 are rejected. Plans 09 and 10 should confirm
  them against their grids before generating members; changing them later is cheap, since nothing
  generates these IDs before then.
- **Alignment of coarse catalogue classes** is checked by plan 09's resolve and not by decode
  (Design note 18), so a raw ID with stray low cell bits is well-formed here and resolves to "no
  such system" there.
- **The `domain_tags!` registry** differs from the interface sketch's `domain_tag!`. Plans that
  wrote `domain_tag!("…")` register the same names in `rng/tags.rs` with a scope instead. The single
  file will see merge conflicts when plans run in parallel; sections per plan keep them trivial.
- **`hyperion-testkit` is a new crate**, not in the roadmap's code-shape list. It is a
  dev-dependency only, and the sim's runtime dependencies remain `libm` alone.
- **`libm` without `arch`** has not been measured here. If the soft `exp`, `ln` or `pow` prove slow
  in plan 03's benchmarks, enabling `arch` must first be shown to leave every golden unchanged on
  both CI architectures.
- **Age plus clock time in `f64`.** A 10 Gyr age in seconds has a spacing of about a minute. That is
  plan 06's concern (evolution is slow; fast phenomena take the clock time directly), recorded here
  because `Span::as_seconds_f64` invites the mistake.
- **The AArch64 runner** may not be available to a private repository; T12 names the fallback.
- **Statistical thresholds.** α = 10⁻³ over a few dozen fixed-seed tests gives a few per cent chance
  that some seed needs changing at the first run. Design note 28 says how that is handled without
  weakening a test.
