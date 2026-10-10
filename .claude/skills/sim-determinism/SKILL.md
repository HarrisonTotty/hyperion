---
name: sim-determinism
description: Keeps HYPERION's procedural generation bit-for-bit reproducible across runs, call orders, CPU architectures and WebAssembly. It covers random streams and domain tags, word consumption, iteration and summation order, integer widths, the hazards of code run natively and as WebAssembly (the shared terrain), golden files, GENERATOR_VERSION bumps and statistical-test seeds. Use whenever writing or changing generator code in crates/hyperion-sim, crates/hyperion-base, crates/hyperion-surface or crates/hyperion-testkit, or fitted tables in crates/hyperion-fit, adding random draws, when a golden test fails, or before bumping the generator version.
paths:
  - "crates/hyperion-sim/**"
  - "crates/hyperion-base/**"
  - "crates/hyperion-surface/**"
  - "crates/hyperion-testkit/**"
  - "crates/hyperion-fit/**"
---

# Simulation determinism

A universe is `(seed, generator_version)`. The same pair must give the same bits on x86-64,
AArch64 and wasm32, in any call order, for as long as saves exist. What checks it today: `just ci`
runs the goldens natively on x86-64 and, through `just test-wasm-fast`, the fast suites of base,
the surface crate, the sim and the testkit as `wasm32-wasip1` under wasmtime, and those of base,
the surface crate and the testkit (not the sim, which no browser loads) as the client's
`wasm32-unknown-unknown` under `wasm-bindgen-test` on Electron's V8, where `golden!` compares the
golden files embedded at compile time (they are blessed natively); `just ci-slow` adds
their slow suites there (`just test-wasm-slow`); a missing tool fails either gate, naming
`just wasm-tools`; and nothing checks AArch64. Every determinism crate's own
`clippy.toml` (the sim's, `hyperion-base`'s, `hyperion-surface`'s and `hyperion-fit`'s) already bans
platform maths (go through `hyperion_base::math`, which the sim re-exports as
`hyperion_sim::math`), `f64::mul_add`, the `algebraic_*` methods and float `to_bits`. The exact
`libm` pin in the root `Cargo.toml` is part of the output too: changing it is a generator-version
change. This skill covers what no lint can see.

## Streams and draws

- Randomness comes only from `Stream::open(seed, TAG, key)`, or, for events, from
  `EventKey::derive(seed, EVENT_TAG, subject)` and its `bin_stream` / `event_stream`
  (`Stream::open` panics on an `Event`-scope tag). Each property group gets its own domain tag,
  declared inside `domain_tags!` under the plan's heading, as `NAME: <scope variant> = "a.b.c";`
  (for example `GALAXY_PARAMS_STELLAR_MASS: Galaxy = "galaxy.params.stellar_mass";`), by the task
  that first opens it, in one of three registries: `crates/hyperion-sim/src/rng/tags.rs` for every
  stage of the sim (event tags, `body.surface` and `body.surface.detail` included);
  `crates/hyperion-surface/src/tags.rs` for the `surface.*` tags the surface crate opens, and
  `selftest.surface.*` for its `SelfTest` tags; `crates/hyperion-base/src/rng/tags.rs` only for
  what base itself opens (`selftest.stream`). The sim asserts the three disjoint at compile time.
  An event tag also takes a number in `crates/hyperion-sim/src/id/event_tags.rs`, backed
  by an `Event`-scope domain tag.
- A tag is never renamed or removed. Its name is hashed into every key it opens.
- **The number and order of words drawn from a stream is part of the output.** Adding a draw,
  reordering draws, or making a draw conditional moves every later value from that stream. A new
  property gets a new tag, not an extra draw on an existing stream. A rejection loop is fine as
  long as it consumes words deterministically.
- Keys come from integers only, through the `ObjectKey` constructors or the `From<SystemId>` and
  `From<BodyId>` conversions (which call `ObjectKey::system` and `ObjectKey::body`). Never derive a
  key from float bits, pointers, the index of an unordered collection, or `std::hash`
  (`RandomState` is seeded per process).
- Random decisions go through the integer thresholds `rng::{Threshold, Thresholds, Mark}`
  (`rng/decide.rs`), usually via `Stream::decide`, `Stream::pick` or `Stream::mark`. Don't compare a
  float uniform with a float probability.

## Order independence

Generating A then B equals generating B then A, which equals generating B alone. Generators are
pure functions of seed, key and inputs.

- **Banned**: state that depends on call history, such as memo tables filled on first use,
  counters, or lazily initialised globals. Caches that do this belong to the caller (the server).
- **Allowed, as a documented exception**: a value computed lazily, on first read, inside the value
  that owns it, when all of these hold:
  - it is a pure, deterministic function of state that is already fixed, and that state never
    changes afterwards;
  - it draws on no RNG stream, and its computation never reads the lazy value itself (so it
    cannot re-enter);
  - it gives the same bits whenever it is computed, and whichever caller reads it first;
  - `PartialEq` and `Debug` are unaffected: equality ignores whether it has been computed yet, and
    `Debug` prints what it always printed;
  - its documentation says it is lazy, and a golden or a bit-exact test pins the value.

  The example is `galaxy::params::BlackHoleParams`: `GalaxyParams` holds the black hole's σ_e and
  mass in a `OnceLock`, solved from the immutable parameters when first read (plan 02, P02.T6.e as
  built). A `OnceLock` keeps the owner `Send + Sync` and works on `wasm32-wasip1`; `Cell`,
  `RefCell` and `OnceCell` do not.
- **Allowed**: immutable values precomputed in a constructor from its inputs alone, such as
  quadrature nodes or tables. If a precomputed path stands in for a direct computation, add a test
  that the two agree bit for bit, so that a later edit can't split them.

For anything that may sit behind a cache, prove order independence with
`hyperion_testkit::order::assert_order_independent`.

## Arithmetic whose form is output

- Float addition is not associative, so **summation order is output** (galaxy-generation plan 02,
  D18).
  Sum components in their fixed, declared order. Never sum an unordered collection or reduce in
  parallel. An algebraically equal rewrite changes bits: factoring out, reordering terms,
  `x * x * x` for `powi(x, 3)`, multiplying by a precomputed `1 / x`. Refactor such code only as a
  deliberate output change, with a bump.
- Never let `HashMap` or `HashSet` iteration order reach output. Use a `BTreeMap`, or sort first,
  with `total_cmp` for floats.
- Nothing that reaches output, a key or a hash may depend on `usize`, which has 32 bits on wasm.
  Use `u64` and `u32`. `as usize` on a `u64` truncates there.
- A float-to-integer `as` saturates and turns NaN into 0. Handle NaN and the range explicitly.

## Hazards across targets

The shared terrain (`hyperion-surface`, with `hyperion-base` beneath it) runs natively on the
server and as WebAssembly in the client, and both must produce the same `f64` for the same point,
or the client draws ground the server does not collide with (the rendering brainstorm's
"Determinism hazards specific to terrain"). Everything above holds there too; these hazards are
specific to code that runs on more than one target:

- **Transcendentals only through `hyperion_base::math`.** Base's and the surface crate's own
  `clippy.toml` files ban the platform methods, and each is self-contained: the workspace root's
  file allows reading float bits (the testkit prints them), so a crate that fell back to it could
  hash a float in a noise function without a lint firing. A new determinism crate gets a file of
  its own; `crates/hyperion-testkit/tests/clippy_bans.rs` fails if it has none.
- **Fused multiply-add only where `math::mul_add` is written.** Rust never contracts `a * b + c`
  on its own, so in hot noise code write the plain expression and it stays two roundings on every
  target. The `algebraic_*` methods are banned everywhere: Rust's documentation says "the same
  inputs may produce different results even within a single program run" (`primitive_docs.rs`,
  "Algebraic operators", rustc 1.98.1).
- **Octave sums in a fixed order.** Sum octaves, channels and contributions in their declared
  order, never through an iterator adaptor that may split or reorder the work.
- **No relaxed SIMD.** A build with `+relaxed-simd` fails in both client crates (a
  `compile_error!`), any `target_feature(enable …)` attribute is rejected by a source test, and the
  20 relaxed intrinsics of `core::arch::wasm32` are Clippy-banned for the browser target: their
  NaN, signed-zero and rounding results are the engine's choice. Fixed-width `simd128` is allowed,
  provided no sum is reassociated to fit the lanes; no lint can see that, so review for it.
- **`min` and `max` are not exact at a signed zero.** Rust's documentation says that when the
  inputs compare equal, such as `+0.0` and `-0.0`, "either input may be returned
  non-deterministically" (`f64::max`, rustc 1.98.1); constant folding orders −0 below +0 while the
  x86-64 instruction returns the second operand. The height path uses its own sign-fixing `min`
  and `max` until `f64::minimum` and `maximum` are stable (rust-lang/rust issue 91079).
- **A NaN's sign is not portable.** Rust does not guarantee a NaN's bit pattern across arithmetic,
  so `total_cmp`, `is_sign_positive`, `is_sign_negative` and `copysign` can differ between targets
  on a NaN (`f64::is_sign_positive`'s documentation). Assert heights finite before they are sorted,
  compared or emitted.
- **Flush-to-zero from outside.** WebAssembly keeps subnormals, but a native thread can be put in
  a flushing mode by a library it loads: GCC before 13 linked `crtfastmath.o`, which sets FTZ and
  DAZ at startup, into shared objects built with `-ffast-math` (GCC 13's release notes), and Clang
  links it when `-mdaz-ftz` is given (llvm-project pull request 80475). The server probes every
  compute thread before its first job and after each, and refuses to generate on one that
  flushes (R04.T9); code that loads a native library probes the loading thread and every worker. A
  mode switched on and off within one job is invisible to the probes; the brainstorm's answer is to
  run the language model out of process.
- **Integer seeds and counter-based noise.** Derive seeds with integer arithmetic, and draw noise
  from `Stream` by counter (a lattice point's key, its word number), so that the value at a point
  never depends on which points were computed first. Cell and cache keys are `u64`, never `usize`,
  which is 32 bits on WebAssembly.
- **No `f32` in the authoritative path.** The GPU draws in `f32`, but every height, normal or
  material the server also computes is `f64` end to end; an `f32` rounding in the middle differs
  from the GPU's and from nothing the server knows.

## When a golden test fails

Goldens (`crates/*/tests/golden/**/*.golden`) pin output as float bits, so a failure means output
moved.

1. Decide whether the change is meant to move generated output. The task or the plan's
   **Generator version** section usually says. If it isn't, the failure is a bug: find the stream,
   order or arithmetic change that caused it, and don't bless.
2. If it is meant to: bump `GENERATOR_VERSION` in `crates/hyperion-base/src/version.rs`, once per
   task, in the commit that moves the output (plans say tasks bump "as they land"). Update the
   `assert_eq!(GENERATOR_VERSION.get(), N)` test in the same file. So that you don't bump twice,
   check whether the task already bumped, including in its earlier checkpoint commits:
   `golden_diff.py --base <commit before the task's first commit>` shows the rise. The task isn't
   done until its bump and regenerated goldens are in.
3. Run `just bless`. It refuses to run under `CI`.
4. Account for every golden that moved (run from the repository root):

   ```bash
   python3 .claude/skills/sim-determinism/scripts/golden_diff.py                    # tree vs HEAD
   python3 .claude/skills/sim-determinism/scripts/golden_diff.py --base <ref>       # tree vs <ref>
   python3 .claude/skills/sim-determinism/scripts/golden_diff.py --base C^ --head C # commit C
   ```

   It separates header-only churn (every golden carries the version), extensions (new labels, old
   values unchanged) and changed values. It checks every golden's header against the version, and
   ends with a verdict: problems (exit 1), checks to make by hand, or consistent. The surface
   crate's goldens directly in `crates/hyperion-surface/tests/golden/` pin R05's provisional test
   planet, which belongs to no universe: their header is `TEST_PLANET_VERSION` from
   `crates/hyperion-surface/src/lib.rs`, which a change to them bumps instead of
   `GENERATOR_VERSION`, and the script checks them against it; R09's, in its subdirectories
   (`wire/`, `height/`), carry `GENERATOR_VERSION` like every other golden. The change must
   explain each changed value. A change it can't explain is a leak: something moved that shouldn't
   have, so go back to step 1 for it. An extension isn't automatically safe: if a
   new label pins a value the base already generated, and its computation changed, that is moved
   output too.
5. Run `just ci-slow`. Its `just ci` half compares the fast goldens as `wasm32-wasip1`
   (`just test-wasm-fast`) and its slow half the slow suites there too (`just test-wasm-slow`).
   Both fail, naming `just wasm-tools`, when a tool is missing; never report them as skipped.
6. If the bump trips a statistical test, run that test under three other seeds. Two failures in
   three is a real defect. Otherwise change the seed in the same commit, with a note
   (galaxy-generation plan 01, design note 28). Never loosen α, shrink the sample, or widen a
   bracket or tolerance the plan states in order to pass. If the plan's bracket is wrong, it is
   either a technical correction, recorded in the plan as a deviation with its reasoning, or, when
   the figure comes from the brainstorm, a specification question for the owner. The
   plan-conformance reviewer checks for both.

## Adding goldens and statistical tests

- Write goldens with `hyperion_testkit::golden!(name, text)`, building the text with
  `GoldenWriter`: `header(GENERATOR_VERSION.get())`, then `f64` (bits and decimal), `u64_hex` and
  `line`. Never pin a float by its decimal form alone. `just bless` creates new golden files.
- Statistical tests use fixed seeds, α = 10⁻³, and the helpers in `hyperion_testkit::stats`.
  Anything slow gets `#[ignore = "slow: <what>"]` and runs under `just test-slow`.
- Every generator keeps a test that the same seed gives the same result twice.
- In `hyperion-base`, `hyperion-surface` and `hyperion-testkit`, whose tests also run on
  `wasm32-unknown-unknown` (rendering plan R04, Design note 12): every test module and file opens
  with `#[cfg(all(target_arch = "wasm32", target_os = "unknown"))] use
  wasm_bindgen_test::wasm_bindgen_test as test;`, since a plain `#[test]` is silently dropped
  there; a test that reads files goes in a module named `native_only`, compiled out there; every
  `should_panic` test states `expected` and lives in the crate's `tests/panics.rs`; and a golden's
  name in `golden!` is a string literal. `just test-wasm-browser` fails when a test is missing
  there.
