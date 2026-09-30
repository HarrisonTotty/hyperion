# Plan R04: Cross-Target Determinism and the Crate Split

- **Milestone:** Rendering milestone RM1 (with R01–R03): a wireframe view at real scale, and the
  determinism checks.
- **Depends on:** none among the rendering plans. It builds on what galaxy plans 01, 04, 14 and 15
  have built (see Consumes).
- **Brainstorm sections covered** (by heading, in
  [the rendering brainstorm](../../brainstorming/rendering-and-planets.md)): constraint 2 of
  "Constraints that decide the design"; "Determinism hazards specific to terrain"; the three things
  "done now" and step 2 of "Suggested order of attack", with the crate move of step 7; the first,
  second and third bullets of "Runtime and code shape" (the crate for the surface, both wasm targets
  in the checks, fixed-width and relaxed SIMD) and the CSP sentences of its last bullet; from
  "Knowledge, and the surface seed", the lean that the surface seed is server-only and plan 14 is
  amended before P14.T23; from "Testing", the height-function determinism item (its checks, not its
  heights), the testkit's `include_str!` arm and the flush-to-zero probes; open question 12; the CSP
  item of "Awaiting the owner".

## Goal

When this plan is done, the determinism checks say what they check and check what the shared terrain
will need. The sim's `math`, `rng` and `units`, with the generator version and the part of `id` that
`rng` needs, live in a crate beneath the sim, `hyperion-base`, with every path the sim exposed today
still in place and every golden file byte for byte unchanged. Beside it stands the
`hyperion-surface` crate, empty of terrain but under its real checks: its own self-contained
`clippy.toml`, a build failure under relaxed SIMD, and a registry for its domain tags that the sim's
collision check covers. All five `clippy.toml` files ban the `algebraic_*` float methods, and a test
holds them to one list. `just ci` runs the fast suites of both WebAssembly targets as well as
native: `wasm32-wasip1` under wasmtime, and `wasm32-unknown-unknown` under `wasm-bindgen-test` on
the V8 that Electron ships, through a `node` shim; a missing tool fails the gate with a pointer to
the recipe that installs it, never a skip. `just ci-slow` adds the slow suite under wasip1. The
testkit compares embedded golden files on the browser target, blessed natively. The sim-determinism
skill carries the terrain hazards, reaches the new crates and routes their changes to the
determinism auditor. The server probes every compute thread for flushed subnormals and refuses to
generate on one that flushes, and a test proves the refusal. Plan 14 is amended so that no surface
seed ever reaches a client, whose wire and parser carry a detail seed instead. And the client's
first WebAssembly module, built from the surface crate, loads in a worker, once the owner has ruled
on the Content Security Policy question, which research reframed (Design note 16). No height
function exists yet: that is [R05](05-terrain-geometry-and-descent-spike.md)'s provisional one and
[R09](09-surface-generator.md)'s real one, both written in the real crate under these checks.

## Scope and non-goals

In scope:

- The three things the brainstorm has done now: plan 14's amendment and the wire and client change
  that go with it, the `algebraic_*` bans, and the false Arm and wasm32 CI claims.
- The crate split: `hyperion-base` (`math`, `units`, `version`, `rng` with the part of `id` it needs
  and its share of the split tag registry) and the `hyperion-surface` skeleton, five `clippy.toml`
  files in all. Done here, before the descent spike, as a ruling of this plan set, so that spike
  terrain code lives in the real crate under the real checks.
- Both wasm targets in the checks: the tools, the recipes, the `node` shim, the testkit's embedded
  golden arm, `just ci` and `just ci-slow`.
- The terrain hazards section of the sim-determinism skill, its `paths:`, the review and validation
  routing, and the `rust-dev.md` pointer and crate-boundary entries.
- The flush-to-zero probes and the server's refusal to generate.
- The client's first WebAssembly load, and the CSP question put to the owner for sign-off.

Non-goals:

- Any height function, noise, coarse field or terrain type. R05 writes the provisional test planet's
  height function in `hyperion-surface`; R09 replaces it and adds the golden height files that these
  checks then carry across native and both wasm targets.
- The `body.surface.detail` tag and the detail seed's value, which R09 registers and computes. This
  plan fixes only the seed's wire form and keeps the surface seed off the wire.
- The height-worker pool, its transfers and its scheduling (R05). This plan's worker loads one
  module and answers one message, to prove the load path.
- Moving any sim module other than those named. `coords`, `time`, `id` beyond its hex parsing, the
  galaxy, the stars and the planets stay in `hyperion-sim`.
- AArch64. Nothing runs it, and the plan does not add it; the claims that CI does are corrected
  rather than made true
  ([open question 12](../../brainstorming/rendering-and-planets.md#open-questions) asks only for the
  wasm targets).
- Hosted CI. There is none; a job beside `just ci` would be a gate nobody runs.
- The Electron switches, adapters and the headless SwiftShader harness, which are
  [R01](01-graphics-platform-and-engine.md)'s.

## Provides

Rust paths are sketches, named precisely enough to be grepped.

### `hyperion-base` (new crate, `crates/hyperion-base`)

```rust
// Moved from hyperion-sim, unchanged in behaviour and output:
pub mod math;      // every transcendental, on the pinned libm; `mul_add` is libm's fma;
                   // gains `j0` (T4.c), for R10
pub mod units;     // unit newtypes and `units::consts`
pub mod version;   // GeneratorVersion, GENERATOR_VERSION
pub mod rng {      // Stream, Seed, ObjectKey, DomainTag, TagScope, Mark, Threshold(s), the
                   // samplers, threefry2x64_20, assert_tag_names, hash_tag_name, is_valid_tag_name
    pub mod tags;                       // base's registry: SELFTEST_STREAM
    pub struct RawEventKey([u64; 2]);   // Design note 5
    impl RawEventKey {
        pub fn derive(seed: Seed, tag: DomainTag, counter: [u64; 2]) -> Self;  // Event scope
        pub fn stream(&self, bin_word: u64, slot: u16) -> Stream;
        pub const fn words(self) -> [u64; 2];
    }
    pub const fn assert_registries_disjoint(registries: &[&[DomainTag]]);    // Design note 4
}
impl ObjectKey {
    pub const fn system(raw_system_id: u64) -> Self;                  // Design note 6
    pub const fn body(raw_system_id: u64, body_index: u16) -> Self;
}
#[macro_export] macro_rules! domain_tags { /* as today, emitting `ALL` and its own assertion */ }
pub mod hex { pub enum HexFault { .. }  pub fn parse_lower_hex(text: &str, digits: usize)
    -> Result<u64, HexFault>; }
```

`hyperion-sim` keeps every path it exposes today by re-export:
`hyperion_sim::{math, units, version, GENERATOR_VERSION, GeneratorVersion, Seed}`, and
`hyperion_sim::rng` re-exports all of `hyperion_base::rng` and adds its own `EventKey` and its own
`tags` registry. Nothing in the sim, the fitting crate, the server or their tests and benches
changes its imports.

### `hyperion-surface` (new crate, `crates/hyperion-surface`)

```rust
//! (crate docs: the contract of R09's height function, to come, and its determinism rules)
#[cfg(target_feature = "relaxed-simd")]
compile_error!("relaxed SIMD is banned in hyperion-surface: ...");
pub mod tags;                             // its registry, empty until R09's `surface.*` tags
pub fn generator_version() -> u32;        // what the client's first module reports (T10)
```

Its `clippy.toml` is self-contained, modelled on the sim's. `hyperion-sim` depends on it, so that
the sim's registry can assert every registry disjoint, and so that the flight model's collision
query can call it when there is one.

### Server (`hyperion_server`)

```rust
pub mod compute {
    pub struct FlushProbe { /* flushes_outputs: bool, flushes_inputs: bool */ }
    pub fn probe_flush_to_zero() -> FlushProbe;          // Design note 15; nanoseconds to µs
    pub enum JobError { .., FloatingPointMode }          // new variant
    pub enum SubmitJobError { .., Faulted }              // new variant: a probe has failed
    pub enum StartPoolError { .., FloatingPointMode { worker: usize, probe: FlushProbe } }
    impl CpuPool { pub fn with_probe(.., probe: fn() -> FlushProbe)
        -> Result<Self, StartPoolError>; }
}
```

### Recipes and tools

- `just wasm-tools`: installs what the wasm checks need (both rustup targets, the pinned wasmtime,
  `wasm-bindgen-cli` at the version `Cargo.lock` names).
- `just test-wasm-fast`: the fast suites under wasip1 and the browser target (Design note 11), run
  by `just ci`. `just test-wasm-slow`: the slow suite under wasip1, run by `just ci-slow`.
  `just test-wasm` stays, as both.
- `just test-wasm-browser`: the browser target alone, through the `node` shim.
- `tools/electron-node/node`: the shim that runs Electron as Node.
- `just gen-surface`: the surface crate's module and glue for the client (T10.b).

### Test helpers

- `hyperion_testkit::golden!` gains its embedded arm on `wasm32-unknown-unknown`, and
  `golden::check_embedded(name, expected, actual)`.
- `crates/hyperion-testkit/tests/clippy_bans.rs`, the test that holds every `clippy.toml` to one ban
  list.

### Protocol and client

- `BodyHooksDto { detail_seed: SectionDto<DetailSeedHex> }` in place of `surface_seed`, and
  `DetailSeedHex` in place of `SurfaceSeedHex`; in the client, `BodyHooks.detailSeed`.
- `apps/hyperion/src/renderer/src/wasm/` (T10.b): the module loader and the probe worker, which
  R05's height workers extend.

## Consumes

- **Galaxy plan 01** ([01](../galaxy-generation/01-determinism-foundation.md)): `math`, `units`,
  `rng` (with `rng/tags.rs`, the single registry, and `domain_tags!`), `id`'s hex parsing
  (`id/text.rs`'s `HexFault` and `parse_lower_hex`), `version.rs`, the testkit's `golden!`,
  `GoldenWriter` and `golden::check`, and `foundation_golden.rs` with its eight goldens. These are
  what move or change.
- **Galaxy plan 04** ([04](../galaxy-generation/04-server-and-protocol.md)): the CPU pool,
  `compute::pool::{CpuPool, JobError, SubmitJobError, StartPoolError}`, and the protocol envelope's
  `ErrorCode`.
- **Galaxy plan 14** ([14](../galaxy-generation/14-planetary-systems.md)): `BodyHooksDto` and
  `SurfaceSeedHex` in `crates/hyperion-protocol/src/planetary/record.rs` and `primitives.rs`; the
  client's `bodiesWire.ts`, `model.ts` and `BodyRecordReadings.tsx`; P14.T23 as specified, which
  this plan amends before it lands.
- **Galaxy plan 15** ([15](../galaxy-generation/15-offline-fitting.md)): `hyperion-fit`'s use of
  `hyperion_sim::math` and its `clippy.toml`.
- **The owner:** the ruling on the renderer's CSP (T10.a), which the brainstorm lists under
  "Awaiting the owner" and `.claude/rules/typescript-dev.md` requires.

## Design notes

1.  **The split comes before the spike.** The brainstorm lists the move of `math`, `rng` and `units`
    under step 7, the surface generator, and asks that both wasm targets join the checks in step 2,
    before any terrain code, the descent spike's included. This plan set moves the crate split
    forward to sit with step 2, so that R05's provisional height function is written in
    `hyperion-surface` under its real `clippy.toml` and its real wasm checks rather than in a
    scratch crate moved later. Nothing in the brainstorm is contradicted: step 7's list still holds,
    and is simply done early. The three things the brainstorm has done now (T1–T3) come first
    because they cost least and are independent of the rest.

2.  **What moves, and why `version` goes with it.** `hyperion-base` takes `math`, `units` and `rng`
    as the brainstorm says, the hex parsing of `id` that `rng`'s `Seed` parser reads (`HexFault`,
    `parse_lower_hex`), and `version.rs`. The brainstorm does not name `version`, but the move needs
    it: every golden file carries `GENERATOR_VERSION` in its header, base's own goldens and R09's
    golden height files in the surface crate must write it, and neither crate may depend on the sim,
    which would make a cycle. The version is also what the `libm` pin, now in base, belongs to.
    `coords`, `time`, the rest of `id`, the galaxy, the stars and the planets stay in the sim; R02's
    `BodyFixedPosition` goes into the sim's `coords`, and anything the surface crate needs of
    positions R09 defines there. Three files name `crates/hyperion-sim/src/version.rs` and follow
    it: the justfile's `generator_version` variable, `golden_diff.py`'s `VERSION_FILE` and step 2 of
    the sim-determinism skill. The galaxy plans' many references to `version.rs` stay as history;
    the README's "Code shape" convention, that the sim holds these modules and has one runtime
    dependency, is the roadmap's to restate (notes).

3.  **Every existing path survives by re-export.** `hyperion_sim::math`, `units`, `version`,
    `GENERATOR_VERSION`, `GeneratorVersion`, `Seed` and all of `hyperion_sim::rng` stay valid,
    through `pub use hyperion_base::…`. Some 235 files of the sim, 19 of the fitting crate and 15 of
    the server name these paths, and rewriting them would move no output and add a diff nobody could
    review for what matters. Code in `hyperion-base` and `hyperion-surface` names `hyperion_base`
    paths, since those crates cannot see the sim. The Clippy bans are paths to the banned methods
    (`f64::sin`, `f64::algebraic_add`), and they do not change; the brainstorm's "every ban's path
    changes" is true only of the `reason` strings, which name `hyperion_base::math` in base and the
    surface crate and "`hyperion_sim::math` (`hyperion_base::math`)" in the other three files.

4.  **The registry splits in three, and one assertion still covers them.** The brainstorm says the
    registry splits, since `rng/tags.rs` registers every stage's tags and the mechanism moves to
    base. Base's registry holds the tags base itself opens, today `selftest.stream` alone. The
    surface crate's holds the `surface.*` tags, none until R09 (`surface.channel`, `surface.crater`,
    `surface.scatter`, the ones the brainstorm names), and `selftest.surface.*` tags of `SelfTest`
    scope, such as R05's provisional test planet's. The sim's holds the rest, with `event.selftest`,
    plan 14's `body.surface` and R09's `body.surface.detail`, both of which the server derives in
    the sim. `domain_tags!` moves to base and is exported, and `DomainTag::registered`, which its
    expansion calls from another crate, becomes `#[doc(hidden)] pub`, documented as reachable only
    through the macro. Each registry keeps its own `const` assertion over its names, and the sim's
    gains one over all three, `assert_registries_disjoint` over base's `tags::ALL`, the surface
    crate's `tags::ALL` and its own `ALL`, in that order, which fails compilation on a duplicate
    name or hash across them, so the galaxy README's "the collision test covers them all" stays
    true. That assertion is why the sim depends on the surface crate from the start, as the
    brainstorm has it depend for collision. The golden `rng/tags` prints the three registries in
    that order, which is today's order byte for byte, since `selftest.stream` is today's first entry
    and the surface registry is empty. One registry in base was considered and rejected: base would
    change with every sim plan's tags, rebuilding and re-running the browser target's checks for
    tags the client never opens.

5.  **`EventKey` stays in the sim, over a raw key in base.** `EventKey::derive` takes plan 01's
    `EventTag` and `EventSubject`, and its streams take an `EventBin`, all `id` types that reach
    `SystemId`, `BodyId` and designations. Moving them would take most of `id` into base. Base
    provides `RawEventKey` instead: `derive(seed, tag, counter)` for a domain tag of `Event` scope
    (it panics on any other, as `Stream::open` panics on an `Event` tag) and
    `stream(bin_word, slot)`. The sim's `EventKey` becomes a newtype over it with today's API, so
    its 13 call sites and the `rng/events` golden, which prints `words()`, do not change.
    `Stream::from_words` stays private to base, and the key discipline is no wider than it is now:
    only a registered `Event` tag opens an event stream.

6.  **`ObjectKey` gains public `system` and `body` constructors.** `From<SystemId>` and
    `From<BodyId>` for `ObjectKey` stay in the sim, which the orphan rule allows for a foreign type
    converted from a local one, but `ObjectKey::new` is crate-private in base. The conversions call
    two new constructors, `ObjectKey::system(raw)` and `ObjectKey::body(raw, index)`, with today's
    word, `sub` and scope. This lets code build a key from a raw word that is no valid ID; the
    hazard the key rules guard against is a key from floats, pointers or `usize`, which a `u64`
    argument does not admit, and `Stream::open`'s scope check is unchanged. Their docs send callers
    to the conversions, and the skill's key rule names them.

7.  **Goldens move by rename, byte for byte.** A golden whose inputs are all base types moves to
    `crates/hyperion-base/tests/golden/` with `git mv` and its test with it: `math/functions`,
    `rng/samplers` and `rng/decisions`. Those that read sim types stay: `coords/positions` (cells),
    `id/layouts`, `rng/streams` (keys from IDs and cells), `rng/events` (event subjects) and
    `rng/tags` (all three registries). Base gains its own test that every golden it holds carries
    the current version, as `foundation_golden.rs` has for the sim. `golden_diff.py` compares with
    `--no-renames` today, so it would report a moved golden as one removed and one added; T4.a
    teaches it to pair a deleted golden with an added one of identical content and report it as
    moved, so that its verdict stays "consistent" across the split.

8.  **Profiles, inlining and the benches.** `[profile.dev.package."*"]` optimises dependencies that
    are not workspace members, so `hyperion-base` and `hyperion-surface` each get an `opt-level = 2`
    entry beside the sim's; without it the fast suite would run the sim's maths at `opt-level = 0`,
    which the root `Cargo.toml` measured at about twice the time. Every `math` function is already
    `#[inline]`; the `units` operators and the small `rng` methods rely on rustc's cross-crate
    inlining of small functions. The foundation benches
    (`crates/hyperion-sim/benches/foundation.rs`: `math`, streams, samplers, decisions) are run
    before and after T4.a and T4.b, and a function that loses more than a few per cent gains
    `#[inline]`, which moves no bits since Rust neither contracts nor reassociates floats. A
    regression is a finding, as the galaxy README's test conventions say, not a failure.

9.  **Five `clippy.toml` files, one list, and a test that holds them to it.** Clippy takes the
    nearest file and does not merge them, so each is self-contained: the root's binds the server,
    the protocol and the testkit, and the sim, the fitting crate, base and the surface crate each
    carry their own. All five ban the transcendental methods, `mul_add` and, from T1, the ten
    `algebraic_*` methods (`algebraic_add`, `algebraic_sub`, `algebraic_mul`, `algebraic_div` and
    `algebraic_rem` on `f64` and `f32`; checked present on rustc 1.98.1, and resolved by Clippy's
    `disallowed-methods`, in a scratch crate on 2026-09-29). The four crate files also ban reading
    float bits (`to_bits`, `to_le_bytes`, `to_be_bytes`, `to_ne_bytes`), which the root file omits
    on purpose, since the testkit prints bits and the server compares them. The surface crate's file
    must not fall back to the root's, because that ban is exactly what stops a noise function
    hashing a float. `crates/hyperion-testkit/tests/clippy_bans.rs` reads the five files as text and
    asserts each holds its set, and that every crate under `crates/` other than the server, the
    protocol and the testkit has a `clippy.toml` of its own, so a sixth determinism crate cannot be
    added under the root file by accident. The testkit hosts it because it depends on no workspace
    crate and is already where cross-crate test machinery lives.

10. **Relaxed SIMD is a build failure in both crates the client ships.** The brainstorm puts the
    `compile_error!` under `target_feature = "relaxed-simd"` in the surface crate. Base is compiled
    into the same module, and a `-C target-feature=+relaxed-simd` build applies to every crate in
    it, so the surface crate's line already stops any client bundle; base carries the same line so
    that its own test builds for the browser target cannot be made that way either. On rustc 1.98.1
    the feature name is recognised for wasm32 and the error fires (checked on 2026-09-29 in a
    scratch crate built for `wasm32-wasip1` with and without the flag). Fixed-width `simd128` stays
    allowed, as the brainstorm says, provided no sum is reassociated; the skill's hazard section
    says so, since no lint can.

11. **Which suites run where, and failing rather than skipping.** Under wasip1, `just ci` runs the
    fast suites of `hyperion-base`, `hyperion-surface`, `hyperion-testkit` and `hyperion-sim`, and
    `just ci-slow` adds their slow suites (`--ignored`, under the `slow-test` profile), which is
    what `just test-wasm` runs today plus the two new crates. The brainstorm asks for the "fast
    goldens" in `just ci`. A whole fast suite includes every fast golden, and it is the simplest set
    to state, so it is the default. If T7.c measures the sim's fast suite under wasmtime adding more
    than half of `just ci`'s wall time (about three minutes today, by the README), the sim's share
    narrows to its golden binaries (`--test '*golden*'`; cargo's target flags accept globs, checked
    on 2026-09-29) plus the unit tests that write goldens, and the rest of its fast suite moves to
    `just ci-slow`. The measurement and the choice go in the plan's "as built" notes. Under the
    browser target, `just ci` runs the fast suites of base, the surface crate and the testkit, the
    crates the client ships or tests with, and not the sim's: the sim is never loaded by a browser,
    and its 56 golden-writing test files would each need the attribute change of Design note 12 for
    no parity the brainstorm asks. Every recipe first checks its tools (wasmtime;
    `wasm-bindgen-test-runner` at the version `Cargo.lock` names; both rustup targets; Electron's
    binary) and, if one is missing, fails with a message that names `just wasm-tools`, never skips,
    since a gate that passes by skipping is not a gate. `rust-toolchain.toml` lists both targets, so
    `rustup` installs them with the toolchain. The runner variables are set in the recipes, not in
    `.cargo/config.toml`, so that a bare `cargo test --target wasm32-unknown-unknown` cannot
    silently run under the system's Node.

12. **The browser target's harness** (researched 2026-09-29, on rustc 1.98.1 with wasm-bindgen
    0.2.129 and `wasm-bindgen-test` 0.3.79, under Electron through the shim; sources:
    `wasm-bindgen-test`'s README and runtime, `wasm-bindgen-cli`'s `src/wasm_bindgen_test_runner.rs`
    and `node.rs`, and scratch crates). Four findings shape the wiring. First,
    `#[wasm_bindgen_test]` and `#[wasm_bindgen]` compile under the workspace's
    `unsafe_code = "forbid"` with no relaxation: the exports they generate carry the spans of an
    external proc macro, where `unsafe_code` is not reported, and Clippy pedantic flags nothing
    generated. Second, a plain `#[test]` compiles on that target and is then silently dropped,
    neither run nor counted, so each test module or file that runs there opens with the import
    below, which leaves the bodies unchanged, and the recipe asserts that the browser run's `--list`
    count equals the native one's for each crate, so a file that forgets the line fails the gate.
    Third, Node is the runner's default mode, so no `wasm_bindgen_test_configure!` is written; it
    has no timeout in that mode, so the recipe wraps each run in `timeout`. Fourth, `-- --ignored`
    runs nothing there (the runner forwards only `--include-ignored`), only one name filter is
    accepted, and doctests do not run on the target (wasm-bindgen issue 4921), so the recipe runs
    `--lib --tests` and the browser target has no slow suite, which the brainstorm does not ask for.
    `wasm-bindgen-test` pins its own `wasm-bindgen` exactly, so the workspace pins
    `wasm-bindgen-test = "=0.3.79"` (or the version current when T8 lands), as a target-specific
    dev-dependency of the three crates, and `just wasm-tools` installs `wasm-bindgen-cli` at the
    version `Cargo.lock` names, which the runner requires. On that target `std::env` reads nothing,
    `std::fs` returns `Unsupported`, `println!` is discarded and `Instant::now()` panics; the
    testkit uses none of the last, and tests that time themselves stay native.

    ```rust
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    use wasm_bindgen_test::wasm_bindgen_test as test;
    ```

13. **The shim, and proving it ran.** `tools/electron-node/node` is a two-line shell script that
    runs `$HYPERION_ELECTRON` with `ELECTRON_RUN_AS_NODE=1` and every argument, and fails if the
    variable is unset. `just test-wasm-browser` resolves Electron's binary with
    `pnpm --filter hyperion exec node -p "require('electron')"` (the system Node, which the README
    already requires, resolving the path only), exports it, prepends `tools/electron-node` to
    `PATH`, and before any test runs `node -p process.versions.electron` through the shim, failing
    unless it prints Electron's version. The runner calls `node --expose-gc` on a CommonJS file,
    which Electron accepts as Node (the brainstorm's probe, and the research above). The check then
    runs on the V8 the client ships, as the brainstorm requires, and an Electron upgrade changes the
    V8 the goldens are checked on with no edit here.

14. **The embedded golden arm.** On `wasm32-unknown-unknown`, `golden!` expands to
    `golden::check_embedded` given the name, the file's text through
    `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/", $name, ".golden"))` and the
    actual text, and elsewhere to today's `golden::check`, each arm behind a `#[cfg]` inside the
    expansion, which is resolved in the calling crate; a `cfg`-stripped block is removed before its
    inner macros expand, so the native arm still accepts a computed name (verified in a scratch
    crate). On the browser target the name must be a literal, which every call site in the crates
    that run there already is; the sim's one computed name (`planetary_golden.rs`) never runs there.
    `check_embedded` shares the header, commit-hook and line-diff checks with `check` and never
    blesses: a bless request there panics with "golden files are blessed natively", and a golden
    that does not exist yet fails to compile on that target, so new goldens are blessed natively
    first, as the brainstorm says.

15. **The flush-to-zero probe reads bits, lives in the server, and is proved by a preloaded
    library** (researched 2026-09-29; sources: Intel SDM vol. 1 §10.2.3 for MXCSR's FTZ bit 15 and
    DAZ bit 6; the Arm ARM's FPCR for FZ, and FIZ and AH under FEAT_AFP; C11 §7.6 for a thread
    starting with its creator's floating-point environment; Rust 1.98.1's `core::arch` docs, which
    call changing MXCSR's DAZ or rounding bits undefined behaviour; GCC 13's release notes; scratch
    experiments on this machine). The brainstorm's probe is right in substance, with one correction:
    the output probe must compare the result's bits with zero, not the result with `0.0`, because
    under DAZ alone the comparison reads the subnormal as zero, so the output probe would fail too
    and the two modes could not be told apart. It multiplies by `0.5`, which is what LLVM makes of
    `/ 2.0` in any case and is exact. The four probes are `f64::MIN_POSITIVE * 0.5` and
    `f32::MIN_POSITIVE * 0.5` (a subnormal result, flushed under FTZ or AArch64's FZ) and the
    smallest subnormal times 2⁵² in `f64` and 2²³ in `f32` (a normal result from a subnormal input,
    zero under DAZ, FZ or FIZ), each input behind `black_box`, measured to reach the FPU at
    `opt-level = 3`. The two widths share their control bits on both architectures, so the second is
    redundant; both are kept because the brainstorm asks for both and they cost nothing. The probe
    lives in `hyperion_server::compute`, not in base: reading bits needs `to_bits`, which the
    sim-style files ban and the root file allows for exactly this kind of use, and the server is its
    only caller, since WebAssembly requires subnormals and the client's module has nothing to probe.
    The pool probes each worker before it takes its first job, and refuses to start, naming the
    thread and the mode, if one fails; and it probes after every job, on the success and the panic
    paths alike, before the reply is sent, so that no value computed under a flushing mode ever
    leaves the worker: the job answers `JobError::FloatingPointMode`, the pool is marked faulted,
    and every later submission fails with `SubmitJobError::Faulted`. A request that meets either
    answers `internal`, with a message saying the server refuses to generate, since plan 04 keeps
    the error codes fixed. Nothing runs on a worker between jobs, so the two probes cover every job.
    The server loads no native library today; the rule "probe after loading one" becomes a doc
    comment on the pool and a line in the skill, for whichever plan first loads one. The probe costs
    about 1.6 µs unflushed (measured under shared load, provisional), on the microcode path
    subnormals take, which is nothing beside a job. The test needs FTZ turned on without `unsafe`,
    which no crate can offer soundly. It re-runs its own test binary as a child with `LD_PRELOAD`
    naming a shared object built at test time by `cc -shared -fPIC` from a few lines of C whose
    constructor ORs FTZ, DAZ or both into MXCSR, and the child asserts the probe's failure on its
    main thread and on a thread it spawns. `cc` is rustc's default linker on
    `x86_64-unknown-linux-gnu`, so it is present wherever the workspace links. A fourth variant
    builds an empty file with `-shared -mdaz-ftz`, which links `crtfastmath.o` back in, the exact
    hazard the brainstorm describes. The test is gated to Linux on x86-64; an AArch64 twin would set
    FPCR in the same way and is left until something runs AArch64.

16. **The first module, the worker, and what the CSP actually governs** (researched 2026-09-29, on
    the repo's Electron 44.4.3 with a real electron-vite 6 and Vite 8 build of a wasm-bindgen crate,
    sandboxed, `file://` and dev server both; sources: WHATWG HTML, "initialize worker global
    scope's policy container", which inherits the creator's policy only for local schemes; W3C CSP3
    §4.5.1, which checks the compiling global's own policy, and its `worker-src` → `child-src` →
    `script-src` → `default-src` fallback; Vite's WebAssembly and worker features; Electron's
    security checklist, item 18). The research contradicts one sentence of the brainstorm. The
    page's meta-tag CSP decides only whether a worker may be created (`'self'` allows a same-origin
    module file, so no `blob:` and no `worker-src` are needed, as the brainstorm says); the running
    worker takes its policy from its own script's response, and a `file://` script or Vite's dev
    server sends none, so a worker compiles WebAssembly under today's CSP unchanged, in dev and in
    the built app. `'wasm-unsafe-eval'` is needed only to compile on the page's own thread, which
    fails today with a `CompileError` citing `script-src 'self'`, or once worker scripts carry a CSP
    header of their own, which only a custom scheme served through `protocol.handle` can give them.
    That route also ends `file://`'s access to the whole disk, which a CSP-less worker has (a
    `file://` fetch works there), and Electron's checklist prefers it. So the question for the owner
    is no longer one change but three options (T10.a), and this plan does not pick among them.
    Whatever the ruling, the loading path is the same: the surface crate is also a `cdylib` with
    `wasm-bindgen` as a dependency on the wasm target only, exporting from `src/wasm.rs` (which
    compiles under `forbid(unsafe_code)`, Design note 12, where a raw `#[unsafe(no_mangle)]` export
    does not); `just gen-surface` builds it for `wasm32-unknown-unknown` in release and runs
    `wasm-bindgen --target web` into `apps/hyperion/src/renderer/src/generated/surface/`, which is
    ignored by git, Prettier and oxlint, since a machine-built binary is not source, and which
    `just check`, `lint`, `test`, `client` and `build` make first, as the TypeScript side needs its
    `.d.ts`; the renderer's Vite config sets `worker.format: "es"`; a module worker created with
    `new Worker(new URL("./surface.worker.ts", import.meta.url), { type: "module" })` awaits the
    glue's `init()`, which Vite rewrites to a hashed asset that Electron serves from `file://` as
    `application/wasm`, so streaming compilation works; and the worker's logic sits in a pure
    `handleRequest` module that a Node-environment vitest test drives after `initSync` from the
    bytes on disk (jsdom refuses the file URL). The worker needs `lib: ["webworker"]` for its
    `postMessage` with transfers, through a `tsconfig.worker.json` or a typed wrapper, which T10.b
    settles under TypeScript 7. `nodeIntegrationInWorker` stays off: with the sandbox on, the worker
    was seen to have no `require` or `process`. The first module's one job is to report
    `generator_version()`, and the loader compares it with the server's `server_generator_version`,
    which the client already reads, and reports a mismatch as a fault of the client's module: a
    stale `gen-surface` build would otherwise draw terrain the server does not collide with, once
    there is terrain.

17. **The detail seed's wire form, and no protocol bump.** `BodyHooksDto` holds
    `detail_seed: SectionDto<DetailSeedHex>` rather than a bare seed, the field-level section form
    that `BodyRecordDto`'s own fields already take, because the hooks section becomes `ok` when
    P14.T23 lands with the bulk composition while the detail seed waits for R09's tag: until then
    the field is `not_modelled`. `SurfaceSeedHex` is renamed `DetailSeedHex`, with the same
    16-hex-digit form. No client has ever parsed a hooks value, since every hooks section is
    `not_modelled` today, so the rename breaks no deployed reader and `PROTOCOL_VERSION` stays at 2,
    as adding or reshaping a field no one sends does under plan 04's rules. The `SYSTEM` readout's
    `SURFACE SEED` row becomes `DETAIL SEED`, which shows the guide's em dash until R09 fills it; it
    is the same row, renamed, so no new display string needs the owner. The surface seed stays in
    the sim and the server, where P14.T23 computes it on `body.surface`, and no DTO carries it.

## Tasks

Each task leaves `just ci` passing. T1–T3 are independent of each other and of the rest. T4.a
precedes every task after it; T7 and T8 need T4 and T5; T9 needs T4.a; T10.b needs T5, T8 and the
owner's ruling in T10.a.

### R04.T1 Ban the `algebraic_*` float methods, and hold the `clippy.toml` files to one list

Add the ten `algebraic_*` entries of Design note 9 to `clippy.toml`,
`crates/hyperion-sim/clippy.toml` and `crates/hyperion-fit/clippy.toml`, each with the reason "may
fuse or reassociate: the same inputs may give different results even within one run", citing the
Rust documentation of the methods (`library/core/src/primitive_docs.rs`; re-check the quotation
against rustc 1.98.1's documentation, as the galaxy README's Figures rule asks of any cited source).
Extend each file's header comment to say why. Write `crates/hyperion-testkit/tests/clippy_bans.rs`
(Design note 9), with `#![cfg(not(all(target_family = "wasm", target_os = "unknown")))]`, since it
reads files.

- _Files:_ the three `clippy.toml` files; `crates/hyperion-testkit/tests/clippy_bans.rs`.
- _Tests:_ `every_clippy_toml_bans_the_shared_list`, `crate_files_also_ban_float_bits`,
  `every_determinism_crate_has_its_own_clippy_toml`; each fails when an entry is deleted from a copy
  of a file (a unit test over an in-memory text, not the real file).
- _Accept:_ `cargo test -p hyperion-testkit --test clippy_bans`; `just lint`; and, by hand and
  recorded in the task's commit message, that adding `let _ = 1.0_f64.algebraic_add(1.0);` to any
  function of the sim, the fitting crate and the server makes `just lint` fail.

### R04.T2 Say what the determinism checks run

Correct the claim that CI checks 64-bit Arm and wasm32. The sim-determinism skill's opening
paragraph says "CI checks all three architectures" and its step 5 says "CI will run it"; the headers
of the nine `crates/hyperion-sim/tests/planetary_*golden.rs` files (`planetary_golden`, `_context_`,
`_derive_`, `_fate_`, `_giants_`, `_masses_`, `_moons_`, `_placement_`, `_small_bodies_`) say "CI
checks the same file on 64-bit Arm and on wasm32". Each is made to say what runs: `just ci` checks
native x86-64, `just test-wasm` checks `wasm32-wasip1` by hand, and nothing checks AArch64. The
skill's step 5 says to report `just test-wasm` as not run when wasmtime is missing, never that CI
will run it. T7 and T8 change the same sentences again when the wasm targets join `just ci`, so the
words are chosen to be edited in one place: the skill states the targets, and each golden header
points to the skill rather than repeating the list.

- _Files:_ `.claude/skills/sim-determinism/SKILL.md`; the nine planetary golden test files.
- _Accept:_ `grep -rn "CI checks\|CI will run" crates .claude` prints nothing; `cargo fmt --check`;
  `cargo test -p hyperion-sim --test planetary_golden` (headers are comments; nothing moves).

### R04.T3 Amend plan 14: a detail seed on the wire, the surface seed off it

The brainstorm's lean under
[Knowledge, and the surface seed](../../brainstorming/rendering-and-planets.md#knowledge-and-the-surface-seed):
plan 14 is amended before P14.T23 lands. Edit
[galaxy plan 14](../galaxy-generation/14-planetary-systems.md): P14.T23 computes `surface_seed` on
`body.surface` for the server's own use and never puts it in a DTO; the hooks section's wire form
carries `detail_seed` (Design note 17), `not_modelled` until R09 registers `body.surface.detail` and
computes it; the Provides sketch of `hooks::BodyHooks`, the DTO list and the note on `BodyHooksDto`
say the same; each edit is marked "amended by R04.T3 (rendering plans)" with a link to this plan.
Then make the wire and the client match: rename `SurfaceSeedHex` to `DetailSeedHex`, reshape
`BodyHooksDto`, regenerate the bindings, and follow it in the client's parser, model and readout.

- _Files:_ `docs/agent/plans/galaxy-generation/14-planetary-systems.md`;
  `crates/hyperion-protocol/src/{primitives.rs, lib.rs, planetary/record.rs}`;
  `packages/protocol/src/index.ts` and `packages/protocol/src/generated/` (by `just gen-protocol`);
  `apps/hyperion/src/renderer/src/lib/system/{bodiesWire.ts, model.ts}` and their tests;
  `apps/hyperion/src/renderer/src/displays/system/BodyRecordReadings.tsx` and its test.
- _Tests:_ `body_hooks_wire_form` pins `{"detail_seed":{"state":"not_modelled"}}` and the `ok` form
  `{"detail_seed":{"state":"ok","value":"0123456789abcdef"}}` (or whatever `SectionDto`'s existing
  wire form is, which the test reads from its neighbours); the withheld-keys test lists
  `detail_seed`; `a_detail_seed_is_sixteen_hex_digits` replaces the surface seed's; in the client, a
  hooks section parses with a detail seed, a malformed one is a fault, and the readout's
  `DETAIL SEED` row shows the em dash for `not_modelled`.
- _Accept:_ `just gen-protocol-check`; `cargo test -p hyperion-protocol planetary`;
  `pnpm --filter hyperion exec vitest run` on `src/renderer/src/lib/system` and
  `src/renderer/src/displays/system`; `just ci`; and this prints nothing:

  ```sh
  grep -rn "surface_seed\|surfaceSeed\|SurfaceSeedHex\|SURFACE SEED" \
    crates/hyperion-protocol packages/protocol/src apps/hyperion/src
  ```

### R04.T4 The base crate

#### R04.T4.a Create `hyperion-base` with `math`, `units` and `version`

Create `crates/hyperion-base` (`Cargo.toml` with `libm.workspace = true` as its only dependency,
`[lints] workspace = true`, `bench = false`, `hyperion-testkit` as a dev-dependency) and its
`clippy.toml`, the sim's list with base's reasons (Design note 3) and the `algebraic_*` entries.
Move `math.rs`, `units.rs` and `version.rs` with `git mv`, rewrite their `crate::` paths and their
module docs (the rule "no code in `hyperion-sim` calls a transcendental method" becomes "no code in
the determinism crates"), and add the relaxed-SIMD `compile_error!` (Design note 10) to base's
`lib.rs`, whose `//!` docs state its boundary: no I/O, clocks, threads or caches, `libm` its only
runtime dependency. In the sim, `pub use hyperion_base::{math, units, version}` and the crate-root
re-exports; the sim's `Cargo.toml` depends on `hyperion-base` and drops its direct `libm`. Add
`hyperion-base` to `[workspace.dependencies]` and
`[profile.dev.package.hyperion-base] opt-level = 2` (Design note 8). Move `math/functions.golden`
and `math_function_values_are_pinned` into `crates/hyperion-base/tests/foundation_golden.rs`, with
base's own `every_golden_file_carries_the_current_version`. Point the justfile's
`generator_version`, `golden_diff.py`'s `VERSION_FILE` and the skill's step 2 at
`crates/hyperion-base/src/version.rs`, and teach `golden_diff.py` to report moves (Design note 7).
Add both new crates' rows to the README's crate table here and in T5.

- _Files:_ `crates/hyperion-base/{Cargo.toml, clippy.toml, src/lib.rs, src/math.rs, src/units.rs}`,
  `crates/hyperion-base/{src/version.rs, tests/foundation_golden.rs}`,
  `crates/hyperion-base/tests/golden/math/functions.golden`;
  `crates/hyperion-sim/{Cargo.toml, src/lib.rs, tests/foundation_golden.rs}`; root `Cargo.toml`;
  `justfile`; `.claude/skills/sim-determinism/{SKILL.md, scripts/golden_diff.py}`; `README.md`.
- _Tests:_ everything that passed before passes unchanged; `golden_diff.py` gains a case for a moved
  golden (its own test, if the script has one, or a documented manual run).
- _Accept:_ `just ci`; `git diff -M --stat HEAD` lists `math/functions.golden` as a rename at 100%;
  `python3 .claude/skills/sim-determinism/scripts/golden_diff.py` ends "consistent", with no changed
  value and the golden reported as moved; `cargo tree -p hyperion-base -e normal` lists only `libm`;
  `cargo test -p hyperion-testkit --test clippy_bans`; `just bench -- math` before and after,
  recorded in the task's commit message.

#### R04.T4.b Move `rng`, split the registry, and move the hex parsing

Move `rng/` with `git mv` into base, less the sim's registry and `EventKey`: `stream.rs`, `key.rs`,
`domain_tag.rs`, `decide.rs`, `threefry.rs`, `sample/`, and `tags.rs` reduced to base's registry
(Design note 4). Move `HexFault` and `parse_lower_hex` from `id/text.rs` to `hyperion_base::hex`,
public and documented as the parser behind every 16-digit text form; `id` imports them. Build
`RawEventKey` (Design note 5), `ObjectKey::system` and `ObjectKey::body` (Design note 6),
`assert_registries_disjoint`, and export `domain_tags!`. In the sim, `rng/mod.rs` becomes the
re-export of `hyperion_base::rng::*` with `event.rs` (`EventKey` over `RawEventKey`) and `tags.rs`
(the sim's registry, `SELFTEST_STREAM` re-exported from base so every `tags::SELFTEST_STREAM` path
holds, and the disjointness assertion, over base's registry and its own until T5 adds the surface
crate's). Unit tests that read sim types (`stream.rs`'s keys from `GenCell` and `SystemId`,
`event.rs`'s) stay in the sim, in `rng/mod.rs`'s test module or `tests/foundation_order.rs`; those
that read only base types move. Move `rng/samplers.golden` and `rng/decisions.golden` with their
tests into base's `foundation_golden.rs`. `domain_tags_are_pinned` iterates base's registry, then
the sim's.

- _Files:_ `crates/hyperion-base/src/{rng/, hex.rs, lib.rs}`, `crates/hyperion-base/tests/`;
  `crates/hyperion-sim/src/{rng/mod.rs, rng/event.rs, rng/tags.rs, id/mod.rs, id/text.rs}`,
  `crates/hyperion-sim/tests/foundation_golden.rs`, and any test whose imports moved.
- _Tests:_ `registries_are_disjoint` (a `const` assertion, plus a runtime test naming the registries
  so a failure reads well); `a_raw_event_key_refuses_a_tag_of_another_scope` (`should_panic`);
  `event_keys_match_before_and_after` is the unchanged `rng/events` golden; the `compile_fail`
  doctests of `assert_tag_names` move with it, and one is added for `assert_registries_disjoint`.
- _Accept:_ `just ci`; `golden_diff.py` consistent with two more moves and no changed value;
  `git diff -M --stat` shows the moved sources as renames; `cargo tree -p hyperion-base -e normal`
  still lists only `libm`; `just bench -- rng` and `-- samplers` before and after, recorded.

#### R04.T4.c A `j0` wrapper in base's `math`

[R10](10-terrain-on-generated-worlds.md) (its Design note 7) needs the Bessel function of the first
kind of order zero for its slope uncertainty. Add `math::j0(x: f64) -> f64`, `#[inline]`, exactly
`libm::j0`, documented as the others are (its domain, and the pinned `libm` as its source), and pin
it in a new golden, `math/bessel.golden`, over a fixed set of arguments that includes 0, its first
zero near 2.404 8, negative arguments (it is even) and large ones; a new file, so that the moved
`math/functions.golden` stays byte for byte what it was. `f64` has no `j0` method, so no
`clippy.toml` entry is needed; the module docs' list of wrappers gains it.

- _Files:_ `crates/hyperion-base/src/math.rs`, `crates/hyperion-base/tests/foundation_golden.rs`,
  `crates/hyperion-base/tests/golden/math/bessel.golden` (by `just bless`, as a new file).
- _Tests:_ `j0_values_are_pinned`; `j0_is_even` on a sample; `j0(0) == 1` exactly.
- _Accept:_ `cargo test -p hyperion-base`; `golden_diff.py` reports one extension and no changed
  value; later, T8.c compares the new golden on all three targets.

### R04.T5 The `hyperion-surface` skeleton

Create `crates/hyperion-surface`: `Cargo.toml` depending on `hyperion-base` alone,
`[lints] workspace = true`; a self-contained `clippy.toml`, the sim's list with base's reasons and
the `algebraic_*` entries (Design note 9); `src/lib.rs` whose `//!` docs state the crate's contract
and boundary (what R05's and R09's height functions will be: pure functions of the coarse field, a
position and the detail seed, with no I/O, clocks, threads, caches of their own or `f32` in the
authoritative path, and no dependency on the sim), the relaxed-SIMD `compile_error!`,
`generator_version()`; and `src/tags.rs`, an empty `domain_tags!` registry whose docs reserve the
`surface.` prefix, and `selftest.surface.` for `SelfTest` tags. Make the sim depend on it and add
its registry to the disjointness assertion. Add
`[profile.dev.package.hyperion-surface] opt-level = 2`.

- _Files:_ `crates/hyperion-surface/{Cargo.toml, clippy.toml, src/lib.rs, src/tags.rs}`; root
  `Cargo.toml`; `crates/hyperion-sim/{Cargo.toml, src/rng/tags.rs}`; `README.md`.
- _Tests:_ `surface_tags_carry_the_surface_prefix`: every name in the surface registry begins
  `surface.`, or `selftest.surface.` for a tag of `SelfTest` scope such as R05's test planet's
  (vacuous until R05, and then binding); `the_generator_version_is_the_sims` (in the sim's tests:
  `hyperion_surface::generator_version()` equals `GENERATOR_VERSION.get()`).
- _Accept:_ `just ci`; `cargo test -p hyperion-testkit --test clippy_bans` now sees five files; the
  build below fails with the `compile_error!`'s message, and so does the same for `-p hyperion-base`
  (T7.b makes this a recipe step):

  ```sh
  RUSTFLAGS="-C target-feature=+relaxed-simd" cargo build -p hyperion-surface \
    --target wasm32-wasip1 --target-dir target/relaxed-simd-check
  ```

### R04.T6 The terrain hazards in the skill, and the routing to the auditor

Add a section "Hazards across targets" to `.claude/skills/sim-determinism/SKILL.md`, after
"Arithmetic whose form is output", carrying the brainstorm's
[Determinism hazards specific to terrain](../../brainstorming/rendering-and-planets.md#determinism-hazards-specific-to-terrain)
as rules an implementer can act on: transcendentals only through `math`, the surface crate's
self-contained `clippy.toml` and why the root file is not enough; fused multiply-add only where
`math::mul_add` is written, `a * b + c` in hot noise code, and the `algebraic_*` ban; octave sums in
a fixed order; relaxed SIMD banned by `compile_error!`, fixed-width `simd128` allowed without
reassociation; `min` and `max` not exact at a signed zero (rustc 1.98.1's constant folding orders −0
below +0 while the x86-64 instruction returns the second operand), so the height path uses its own
sign-fixing `min` and `max` until `f64::minimum` (rust-lang/rust issue 91079) is stable; a NaN's
sign leaking through `total_cmp`, `is_sign_*` and `copysign`, so heights are asserted finite before
they are sorted, compared or emitted; flush-to-zero from outside, and the server's probes (T9);
integer seed derivation and counter-based noise on `Stream`, with `u64` cell and cache keys, never
`usize`; no `f32` in the authoritative path. Re-check each cited source when writing it (the Rust
documentation of `min`, `max`, `total_cmp` and the `algebraic_*` methods; GCC 13's release notes and
llvm-project pull request 80475 on `crtfastmath.o`), as the Figures rule asks. Extend the skill's
`paths:` and description to `crates/hyperion-base/**` and `crates/hyperion-surface/**`. Route their
changes to the determinism auditor: `.claude/skills/review-changes/SKILL.md`'s routing list,
`.claude/agents/determinism-auditor.md` (its description, and step 1's list of `clippy.toml` files
to diff, which becomes all five), `.claude/skills/validate/scripts/select_checks.py`'s
`DETERMINISM_CRATES`, and the simulation-code bullet of `.claude/skills/implement-task/SKILL.md`. In
`.claude/rules/rust-dev.md`, the numeric-safety section gains one bullet pointing to the skill's new
section, and the crate-boundaries section gains entries for `hyperion-base` (beneath the sim, the
fitting crate and the surface crate; no I/O, clocks, threads, caches; `libm` its only runtime
dependency) and `hyperion-surface` (depends on base alone, and from T10.b on `wasm-bindgen` on the
wasm target only; compiles to `wasm32-unknown-unknown`; never depends on the sim);
`.claude/agents/rust-reviewer.md`'s crate-boundary check follows.

- _Files:_ `.claude/skills/sim-determinism/SKILL.md`; `.claude/skills/review-changes/SKILL.md`;
  `.claude/agents/determinism-auditor.md`; `.claude/agents/rust-reviewer.md`;
  `.claude/skills/validate/scripts/select_checks.py`; `.claude/skills/implement-task/SKILL.md`;
  `.claude/rules/rust-dev.md`.
- _Accept:_ `grep -n "hyperion-base\|hyperion-surface" <each file>` finds each change;
  `python3 .claude/skills/validate/scripts/select_checks.py --base HEAD` on a scratch edit to
  `crates/hyperion-base/src/math.rs` (reverted after) lists the determinism checks;
  `npx prettier --check` on the Markdown files.

### R04.T7 `wasm32-wasip1` in the gate

#### R04.T7.a Fix what wasip1 already finds

`just test-wasm` has not been run for some time, and a run of the sim's fast suite under wasip1 on
2026-09-29 (wasmtime 49.0.1) found three failures, each of which aborts its whole test binary, since
a panic on `wasm32-wasip1` aborts the process and hides every later test in it:

1. `galaxy::snr::caps::tests::cut_ends_a_window_at_its_cap`
   (`crates/hyperion-sim/src/galaxy/snr/caps.rs`, about line 302) catches an expected debug
   assertion with `std::panic::catch_unwind`, which cannot unwind on wasip1; the abort took the rest
   of the sim's lib unit tests with it. Gate it with `#[cfg(panic = "unwind")]`: the cap is still
   tested natively, and the test asserts a debug check, not generated output, so wasip1 loses no
   parity by skipping it.
2. `satellites::moons_inside_hill_spheres_and_rings_inside_roche_limits`
   (`crates/hyperion-sim/tests/planetary_properties.rs`, `check_all`) fans out with
   `std::thread::scope`, and wasip1 has no threads. Run it on one thread where
   `cfg(target_family = "wasm")`, with the same work and the same assertions.
3. `rocky_core_mass_fractions_follow_plotnykov_and_valencia`
   (`crates/hyperion-sim/tests/planetary_rocky_properties.rs`, its local `quantile`) computes
   `(values.len() - 1) * ppm` in `usize`, which overflows 32 bits once the sample passes about
   4,300: the very class of bug the wasm run exists to catch, in test code. Do the index arithmetic
   in `u64` and convert with `usize::try_from`, and grep the other test helpers for the same pattern
   (`* ppm`, `len() *`), fixing any found.

Then rerun the whole fast suite under wasip1 (and the slow suite, whose results the measurement had
not reached) and fix whatever else surfaces in the same way, since a binary that aborted early hid
its later tests. None of these touches generated output.

- _Files:_ `crates/hyperion-sim/src/galaxy/snr/caps.rs`,
  `crates/hyperion-sim/tests/{planetary_properties.rs, planetary_rocky_properties.rs}`, and whatever
  the rerun finds.
- _Accept:_ `just test-wasm` passes in full (needs T7.b's tools, or wasmtime installed by hand);
  `cargo test -p hyperion-sim` unchanged natively; `golden_diff.py` reports nothing.

#### R04.T7.b The tools, and a check that fails rather than skips

Add `targets = ["wasm32-wasip1", "wasm32-unknown-unknown"]` to `rust-toolchain.toml`. Write
`just wasm-tools`: `rustup target add` for both targets, `cargo install wasmtime-cli --locked` at a
version pinned in the justfile, and `cargo install wasm-bindgen-cli --locked` at the version read
from `Cargo.lock` (T8.b makes that entry exist; until then this step is a no-op with a message).
Write `_wasm-preflight`, which checks each tool the recipe that calls it needs and exits non-zero
naming `just wasm-tools` for the first one missing. Split `test-wasm` into `test-wasm-fast` and
`test-wasm-slow`, each adding `-p hyperion-base -p hyperion-surface`, keeping `test-wasm` as both,
and put each run under `_locked`, as `test` is. Update the README's `test-wasm` paragraph and its
prerequisites. Make the relaxed-SIMD negative build of T5 a step of `test-wasm-fast`, which fails if
the build succeeds.

- _Files:_ `rust-toolchain.toml`, `justfile`, `README.md`.
- _Accept:_ `just wasm-tools` then `just test-wasm-fast` passes; with `WASMTIME=/nonexistent`,
  `just test-wasm-fast` fails and prints `just wasm-tools`; `just test-wasm-slow` passes.

#### R04.T7.c Measure, then join `just ci` and `just ci-slow`

Time `just ci` as it stands, the fast wasip1 suite of each crate, and the slow suite, on the
development machine while it is quiet: no other agent, lane or test run, with the load average read
from `/proc/loadavg` before and after each run and below 1.0 at the start. Record each figure with
its load averages. The figures taken on 2026-09-29, while writing this plan, were measured under
shared load and are provisional; they are a sense of scale only and never the figure the rule is
applied to. Apply Design note 11's rule, to the quiet figures alone, to choose the sim's share, and
wire `test-wasm-fast` into `ci` and `test-wasm-slow` into `ci-slow`. Update the skill's opening
paragraph and step 5, and the README's check list, to say what now runs (T2's sentences), and the
`validate` skill's `select_checks.py`, whose separate `just test-wasm` gate step and its skip when
wasmtime is missing are replaced by `just ci`'s own check. In the same edit, teach the skills'
task-ID patterns the rendering plans' `R` prefix, since today they match only `P..` and would not
pick up this plan set's tasks: `select_checks.py`'s `TASK_RE` and its normaliser, and
`plan_task.py`'s `ID_RE`, `LOOSE_ID_RE`, `HEADING_TASK_RE`, `INLINE_TASK_RE`, its normaliser and its
`--list` hint (`.claude/skills/implement-task/scripts/plan_task.py`, lines 43–46, 107, 286 and 644
on 2026-09-29), with the plan directory chosen by the prefix (`galaxy-generation/` for `P`,
`rendering-and-planets/` for `R`). A grep of `.claude/skills/` and `.claude/agents/` for `P\d{2}\.T`
and `P(\d` finds no other pattern today; it is rerun then.

- _Files:_ `justfile`, `README.md`, `.claude/skills/sim-determinism/SKILL.md`,
  `.claude/skills/validate/scripts/select_checks.py`,
  `.claude/skills/implement-task/scripts/plan_task.py`, `.claude/agents/validator.md`; this plan's
  "as built" notes (the timings).
- _Accept:_ `just ci` and `just ci-slow` pass and run the wasip1 suites; the quiet timings are
  recorded with their load averages; `plan_task.py R04.T7.c` prints this task and
  `select_checks.py R04.T7.c` accepts its ID; `grep -n "test-wasm" justfile` shows both in the
  gates.

### R04.T8 `wasm32-unknown-unknown` under `wasm-bindgen-test`, on Electron's V8

#### R04.T8.a The testkit's embedded golden arm

Build `golden::check_embedded` and `golden!`'s two arms (Design note 14), sharing the existing
checks with `check`. Add the target-specific dev-dependency on `wasm-bindgen-test` (pinned in
`[workspace.dependencies]`) and the `as test` import to the testkit's test modules.

- _Files:_ `crates/hyperion-testkit/{Cargo.toml, src/golden.rs}`, root `Cargo.toml`.
- _Tests:_ natively, `check_embedded` accepts equal text, reports the first differing line, rejects
  a header mismatch and refuses to bless; `golden!` with a computed name still compiles natively.
- _Accept:_ `cargo test -p hyperion-testkit`;
  `cargo build --target wasm32-unknown-unknown -p hyperion-testkit --tests`.

#### R04.T8.b The shim and the recipe

Write `tools/electron-node/node` and `just test-wasm-browser` (Design note 13), with the runner
variable set in the recipe, `timeout` around each run, `--lib --tests` under the `slow-test` profile
(debug wasm is slow, and the goldens need no debug assertions to compare), the preflight, and the
`--list` count comparison of Design note 12. Extend `just wasm-tools` to install `wasm-bindgen-cli`
at the locked version.

- _Files:_ `tools/electron-node/node`, `justfile`, `README.md`.
- _Accept:_ `just test-wasm-browser` prints Electron's version first and passes on the testkit; with
  the shim's directory left off `PATH`, it fails at the version check; with one test file's
  `as test` line removed, it fails at the count comparison.

#### R04.T8.c Base and the surface crate on the browser target, and into `just ci`

Add the dev-dependency and the `as test` import to every test module and file of `hyperion-base` and
`hyperion-surface`; put `test-wasm-browser` for the three crates into `test-wasm-fast`, and so into
`just ci`. The moved goldens of T4 (`math/functions`, `rng/samplers`, `rng/decisions`) are then
compared on all three targets. Update the skill's opening paragraph to name the browser target.

- _Files:_ the two crates' `Cargo.toml` and test files; `justfile`;
  `.claude/skills/sim-determinism/SKILL.md`.
- _Accept:_ `just ci` runs and passes the browser target's suites; changing one character of
  `math/functions.golden` makes the native, wasip1 and browser runs all fail (checked by hand,
  reverted, recorded); its time is added to T7.c's record.

### R04.T9 The flush-to-zero probes, and the server's refusal to generate

#### R04.T9.a The probe and the pool's checks

Build `probe_flush_to_zero` and `FlushProbe` (Design note 15) in
`crates/hyperion-server/src/compute/float_mode.rs`, `#[inline(never)]`, citing the Intel SDM and the
Arm ARM for the bits it detects and the brainstorm's bullet for why it exists. Give `CpuPool` a
probe: `CpuPool::new` uses the real one and `CpuPool::with_probe` takes any `fn() -> FlushProbe`,
for tests. Each worker probes before its first job and reports to the constructor, which waits for
every report and returns `StartPoolError::FloatingPointMode { worker, probe }` on the first failure,
after stopping the workers. After every job, before the reply, the worker probes again; on a failure
the reply is `JobError::FloatingPointMode`, a pool-wide flag is set, the fault is logged once with
the thread's name, and every later `try_submit` and bulk submission returns
`SubmitJobError::Faulted`, while queued jobs answer `JobError::FloatingPointMode` as workers take
them. Map both to `ErrorCode::Internal` in `requests/mod.rs` with the message "the server's
floating-point mode flushes subnormals, so it refuses to generate". Document on `CpuPool` that code
loading a native library must probe the loading thread and then every worker.

- _Files:_ `crates/hyperion-server/src/compute/{float_mode.rs, pool.rs, mod.rs, error.rs}`,
  `crates/hyperion-server/src/requests/mod.rs`.
- _Tests:_ `the_probe_passes_on_a_default_thread` (and on a spawned one);
  `a_pool_whose_probe_fails_at_start_refuses_to_start`;
  `a_job_after_which_the_probe_fails_answers_floating_point_mode_and_faults_the_pool` (a probe that
  fails from its Nth call); `a_faulted_pool_refuses_submissions`; the request mapping answers
  `internal` with the message.
- _Accept:_ `cargo test -p hyperion-server compute::float_mode compute::pool`; `just ci`.

#### R04.T9.b The probe fails under a flushing thread

The brainstorm's test: the probes fail, in `f64` and `f32`, on a thread whose flush or
denormals-are-zero mode is on. In `crates/hyperion-server/tests/flush_to_zero.rs`, gated on
`cfg(all(target_os = "linux", target_arch = "x86_64"))`: a child test that returns at once unless a
marker variable is set, and when it is, asserts the probe's expected result on its main thread and
on a spawned thread, and that `CpuPool::new` refuses to start; and a parent test that, for FTZ
(`0x8000`), DAZ (`0x0040`) and both, writes the constructor's C source to a temporary directory,
builds it with `cc -shared -fPIC -O2`, re-runs `std::env::current_exe()` with `--exact` naming the
child, the marker and `LD_PRELOAD`, and asserts that the child passed. A fourth case builds an empty
file with `cc -shared -fPIC -mdaz-ftz`. A missing `cc` fails the test with a message saying so; it
is never skipped.

- _Files:_ `crates/hyperion-server/tests/flush_to_zero.rs`.
- _Tests:_ as above; plus one run by hand, recorded, in which the child's expectation is inverted
  and the parent is seen to fail, so the test is shown able to fail.
- _Accept:_ `cargo test -p hyperion-server --test flush_to_zero` passes in about a second and a
  half; `just ci`.

### R04.T10 The client's first WebAssembly load

#### R04.T10.a The CSP question, for the owner

Put to the owner, with Design note 16's findings and sources, the three options, and record the
ruling in this plan and in the brainstorm's "Awaiting the owner" item (the brainstorm's edit through
a researching agent, as edits to it are made):

1.  **Add `'wasm-unsafe-eval'` now**, the brainstorm's lean, the policy below. Not needed for
    workers under `file://`; it states the intent and allows compilation on the render thread.

    ```text
    default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline';
      connect-src 'self' ws: wss:
    ```

    (one line in `index.html`; wrapped here).

2.  **Change nothing.** Workers compile under today's policy; the render thread never compiles
    WebAssembly, and the rule that it does not is written in the loader.
3.  **Serve the renderer through a privileged custom scheme** (`protocol.handle`), sending the CSP
    as a header on the page and on every worker script, with `'wasm-unsafe-eval'` in it, which is
    then necessary: workers gain a real policy and lose `file://`'s reach over the disk. A larger
    change, to the main process and the dev-server path, that would become its own task.

The task ends when the owner signs off on one. Nothing is changed before then, and the
`.claude/rules/typescript-dev.md` rule stands whichever is chosen.

- _Files:_ this plan; the brainstorm's "Awaiting the owner" item, once ruled.
- _Accept:_ the ruling is recorded, with its date, in both.

#### R04.T10.b The surface module in a worker

Blocked on T10.a. Build what Design note 16 describes, under the chosen option: `wasm-bindgen` as a
wasm-only dependency of `hyperion-surface` and its `crate-type`, `src/wasm.rs` exporting
`generator_version`; `just gen-surface` and the recipe dependencies; the ignore entries; the Vite
worker format;
`apps/hyperion/src/renderer/src/wasm/{surface.worker.ts, handleRequest.ts, loadSurfaceModule.ts}`
and their tests; the generator-version comparison and its fault; and, if the owner chose option 1 or
3, the CSP change itself. Update the crate-boundary entry of `rust-dev.md` (T6) to say that the
surface crate's one non-base dependency is `wasm-bindgen`, on the wasm target only.

- _Files:_ `crates/hyperion-surface/{Cargo.toml, src/wasm.rs}`; `justfile`; `.gitignore`,
  `.prettierignore`, `.oxlintrc.json`; `apps/hyperion/electron.vite.config.mts`;
  `apps/hyperion/src/renderer/src/wasm/`; `apps/hyperion/vitest.config.mts` if the test needs the
  Node project; `apps/hyperion/src/renderer/index.html` under options 1 and 3;
  `.claude/rules/rust-dev.md`.
- _Tests:_ in Node, `handleRequest` after `initSync` answers the generator version, and equals the
  sim's (through a constant the test reads from `gen-surface`'s output or from the protocol); the
  loader reports a fault when the server's version differs; `cargo test -p hyperion-surface`
  natively and on the browser target.
- _Accept:_ `just ci`; by hand and recorded, `just client` and the built app from
  `pnpm --filter hyperion build` each log the module's generator version from its worker with no CSP
  violation in the console.

### R04.T11 Verification pass

Run the plan's Verification list on the development machine, record the timings and the hand checks
in this plan's "as built" notes, and update the README's crate table, prerequisites and check list
to their final form.

- _Files:_ this plan; `README.md`.
- _Accept:_ `just ci-slow` passes; every item of Verification is recorded.

## Verification

The plan is done when, on the development machine, with the tools `just wasm-tools` installs:

- `just ci` passes and runs, besides what it ran before, the fast suites under `wasm32-wasip1` and
  `wasm32-unknown-unknown` (T7, T8), the ban-list test (T1), the flush-to-zero tests (T9) and the
  client's module test (T10.b); and with wasmtime, `wasm-bindgen-test-runner` or the target removed
  from `PATH` or the toolchain, it fails with a message naming `just wasm-tools`, never passing by
  skipping.
- `just ci-slow` passes, the slow suite under wasip1 included.
- Every golden file is byte for byte what it was before T4.a, which
  `python3 .claude/skills/sim-determinism/scripts/golden_diff.py --base <commit before T4.a>`
  confirms with its "consistent" verdict and the moves it reports; `GENERATOR_VERSION` is unchanged.
- The same golden files are compared on native, wasip1 and the browser target: the base crate's on
  all three, the sim's on native and wasip1, which is the parity the brainstorm's height-function
  item needs, ready for R09's files.
- `just test-wasm-browser` shows, in its first line, that the tests ran on Electron's V8 (the shim's
  check), not the system's Node.
- The five `clippy.toml` files carry one ban list, and adding an `algebraic_*` call anywhere in the
  workspace fails `just lint` (checked by hand, recorded).
- The flush-to-zero test is seen to pass, and, with its expectation inverted by hand, to fail.
- The skill, the rule file, the review routing and the validation script name both new crates.
- `just ci`'s wall time before and after the plan, and the slow wasip1 suite's time, measured on a
  quiet machine with their load averages, are recorded in the plan's "as built" notes (open question
  12 asks what the extra runs add).
- The client's first module loads in its worker in `just client` (dev) and in the built app
  (`pnpm --filter hyperion build`, then run from `out/`), checked by hand and recorded, once the
  owner has ruled on the CSP.

## Generator version

This plan changes no generated output and does not bump `GENERATOR_VERSION`. The crate split moves
code, not arithmetic: every golden file is byte for byte what it was, the moved ones by rename, and
`golden_diff.py` must report "consistent" with no changed value after each of T4.a, T4.b and T5. A
task that finds a golden moved has broken the move and is not done. The `algebraic_*` bans, the
probes, the recipes and the documentation touch no generator. The detail seed's rename changes a
wire field that no body fills.

It reserves, so that later plans need not:

- **Registries.** The surface crate's registry, for `surface.*` tags only (R09's `surface.channel`,
  `surface.crater`, `surface.scatter` and whatever else its synthesis opens). In the sim's registry,
  the names `body.surface` (P14.T23, for the server-only surface seed) and `body.surface.detail`
  (R09, for the detail seed), each of `Body` scope, both named here so that neither is taken for
  anything else before its task.
- **Key constructors.** `ObjectKey::system` and `ObjectKey::body` are the only raw constructors in
  base; R09's body–face–level–cell key is a new constructor beside them, with a new `TagScope` if
  its word layout needs one, added by R09 in base.
- **The wire.** `BodyHooksDto.detail_seed` and `DetailSeedHex`, for R09 to fill.
- **Checks.** R09's golden height files live in `crates/hyperion-surface/tests/golden/` and are
  compared on all three targets by this plan's recipes with no further wiring, since
  `test-wasm-fast` and `test-wasm-browser` run every test of the surface crate.

## Risks and open points

- **The owner's CSP ruling gates T10.b**, and through it R05's height workers. Research found that
  workers compile under today's policy (Design note 16), so no option leaves the client without
  WebAssembly; the risk is delay, not feasibility. If the owner picks option 3, the custom scheme
  becomes its own task before T10.b.
- **A flushing mode switched on and off within one job** is invisible to probes run between jobs;
  only reading MXCSR could see it, and that needs `unsafe`. The brainstorm's mitigation, the LLM out
  of process, is what closes it; the skill says so.
- **Rust code run under a preloaded FTZ is formally undefined behaviour** (the `core::arch` docs on
  MXCSR). T9.b shows what the compiled probe does in that state, not what the language guarantees;
  that same undefinedness is why the server refuses to generate.
- **Cross-crate inlining.** Moving `units` and `rng` out of the sim could slow the sim if rustc does
  not inline their small functions across the crate boundary; T4's bench comparisons catch it, and
  `#[inline]` is the cure (Design note 8).
- **Paths that name `hyperion-sim` for moved files.** Galaxy plans, the skill and scripts name
  `crates/hyperion-sim/src/{math, units, version, rng/tags}.rs`. T4 fixes the live tooling; the
  plans are history, and the roadmap's conventions are asked to follow (notes).
- **The validation and implementation skills know only `P..` task IDs**, so they will not find this
  plan's `R04.T…` tasks until their scripts learn the prefix. Not this plan's to fix; recorded for
  the roadmap.

- **The browser harness's quirks** (Design note 12): a plain `#[test]` silently dropped, `--ignored`
  running nothing, one filter per run, no timeout. The count comparison and `timeout` guard the
  first and last; a `wasm-bindgen-test` upgrade that changes any of them is caught by the recipe
  failing, and the pins move together.
- **Time.** Each wasm target adds a build and a run to `just ci`, and the slow wasip1 suite may add
  far more to `just ci-slow` than the native slow suite's twenty minutes. Design note 11's rule
  bounds the first; the second is recorded and accepted, since `ci-slow` is the full gate. The
  pre-push hook runs neither.
- **AArch64 stays unchecked.** The claims are corrected, not made true; if a server is ever run on
  Arm, a run there is the check to add.
