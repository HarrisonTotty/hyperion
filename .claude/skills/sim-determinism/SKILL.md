---
name: sim-determinism
description: Keeps HYPERION's procedural generation bit-for-bit reproducible across runs, call orders and CPU architectures. It covers random streams and domain tags, word consumption, iteration and summation order, integer widths, golden files, GENERATOR_VERSION bumps and statistical-test seeds. Use whenever writing or changing generator code in crates/hyperion-sim or crates/hyperion-testkit, adding random draws, when a golden test fails, or before bumping the generator version.
paths:
  - "crates/hyperion-sim/**"
  - "crates/hyperion-testkit/**"
---

# Simulation determinism

A universe is `(seed, generator_version)`. The same pair must give the same bits on x86-64,
AArch64 and wasm32, in any call order, for as long as saves exist. CI checks all three
architectures. The sim's `clippy.toml` already bans platform maths (go through
`hyperion_sim::math`), `f64::mul_add` and float `to_bits`. This skill covers what no lint can see.

## Streams and draws

- Randomness comes only from `Stream::open(seed, TAG, key)`. Each property group gets its own
  domain tag, declared inside `domain_tags!` in `crates/hyperion-sim/src/rng/tags.rs` under the
  plan's heading, as `NAME: <scope variant> = "a.b.c";` (for example
  `GALAXY_PARAMS_STELLAR_MASS: Galaxy = "galaxy.params.stellar_mass";`), by the task that first
  opens it.
- A tag is never renamed or removed. Its name is hashed into every key it opens.
- **The number and order of words drawn from a stream is part of the output.** Adding a draw,
  reordering draws, or making a draw conditional moves every later value from that stream. A new
  property gets a new tag, not an extra draw on an existing stream. A rejection loop is fine as
  long as it consumes words deterministically.
- Keys come from integers only, through the `ObjectKey` constructors. Never derive a key from float
  bits, pointers, the index of an unordered collection, or `std::hash` (`RandomState` is seeded
  per process).
- Random decisions go through the integer thresholds in `rng::decide` (`Threshold`, `Thresholds`,
  `Mark`). Don't compare a float uniform with a float probability.

## Order independence

Generating A then B equals generating B then A, which equals generating B alone. Generators are
pure functions of seed, key and inputs.

- **Banned**: state that depends on call history, such as memo tables filled on first use,
  counters, or lazily initialised globals. Caches that do this belong to the caller (the server).
- **Allowed**: immutable values precomputed in a constructor from its inputs alone, such as
  quadrature nodes or tables. If a precomputed path stands in for a direct computation, add a test
  that the two agree bit for bit, so that a later edit can't split them.

For anything that may sit behind a cache, prove order independence with
`hyperion_testkit::order::assert_order_independent`.

## Arithmetic whose form is output

- Float addition is not associative, so **summation order is output** (plan 02, design note 18).
  Sum components in their fixed, declared order. Never sum an unordered collection or reduce in
  parallel. An algebraically equal rewrite changes bits: factoring out, reordering terms,
  `x * x * x` for `powi(x, 3)`, multiplying by a precomputed `1 / x`. Refactor such code only as a
  deliberate output change, with a bump.
- Never let `HashMap` or `HashSet` iteration order reach output. Use a `BTreeMap`, or sort first,
  with `total_cmp` for floats.
- Nothing that reaches output, a key or a hash may depend on `usize`, which has 32 bits on wasm.
  Use `u64` and `u32`. `as usize` on a `u64` truncates there.
- A float-to-integer `as` saturates and turns NaN into 0. Handle NaN and the range explicitly.

## When a golden test fails

Goldens (`crates/*/tests/golden/**/*.golden`) pin output as float bits, so a failure means output
moved.

1. Decide whether the change is meant to move generated output. The task or the plan's
   **Generator version** section usually says. If it isn't, the failure is a bug: find the stream,
   order or arithmetic change that caused it, and don't bless.
2. If it is meant to: bump `GENERATOR_VERSION` in `crates/hyperion-sim/src/version.rs`, once per
   task, in the commit that moves the output (plans say tasks bump "as they land"). Check
   `git diff HEAD -- crates/hyperion-sim/src/version.rs` first so that you don't bump twice. A
   work-in-progress checkpoint may lag. The task isn't done until its bump and regenerated goldens
   are in.
3. Run `just bless`. It refuses to run under `CI`.
4. Account for every golden that moved:

   ```bash
   python3 ${CLAUDE_SKILL_DIR}/scripts/golden_diff.py            # against HEAD; --base <ref> for a range
   ```

   It separates header-only churn (every golden carries the version), extensions (new labels, old
   values unchanged) and changed values, and says whether the version and the goldens agree. The
   change must explain each changed value. A change it can't explain is a leak: something moved
   that shouldn't have, so go back to step 1 for it. An extension isn't automatically safe: if a
   new label pins a value the base already generated, and its computation changed, that is moved
   output too.
5. Run `just test-wasm` if wasmtime is installed. If it isn't, say that it wasn't run; CI will run
   it.
6. If the bump trips a statistical test, run that test under three other seeds. Two failures in
   three is a real defect. Otherwise change the seed in the same commit, with a note (plan 01,
   design note 28). Never loosen α, shrink the sample, or widen a bracket or tolerance the plan
   states in order to pass. If the plan's bracket is wrong, that is a deviation to record with its
   reasoning, which the plan-conformance reviewer checks.

## Adding goldens and statistical tests

- Write goldens with `hyperion_testkit::golden!(name, text)`, building the text with
  `GoldenWriter`: `header(GENERATOR_VERSION.get())`, then `f64` (bits and decimal), `u64_hex` and
  `line`. Never pin a float by its decimal form alone. `just bless` creates new golden files.
- Statistical tests use fixed seeds, α = 10⁻³, and the helpers in `hyperion_testkit::stats`.
  Anything slow gets `#[ignore = "slow: <what>"]` and runs under `just test-slow`.
- Every generator keeps a test that the same seed gives the same result twice.
