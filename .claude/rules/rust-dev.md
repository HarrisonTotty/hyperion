---
paths:
  - "**/*.rs"
  - "**/Cargo.toml"
---

# Rust Development

Applies to every crate under `crates/`. Formatting is owned by `rustfmt`; never hand-format or
argue with its output.

## Workflow

- After changing Rust code run `just check`, `just lint` and `just test`. Iterate on one crate with
  `cargo test -p <crate> <filter>`.
- Clippy `all` + `pedantic` run with `-D warnings`, so a warning is a build failure. Fix the code
  rather than silencing the lint.
- When a lint is genuinely wrong, suppress it on the smallest item with
  `#[expect(clippy::lint_name, reason = "...")]`. Never use `#[allow]` without a reason, and never
  add crate-wide suppressions.
- Add dependencies to `[workspace.dependencies]` in the root `Cargo.toml` and reference them with
  `dep.workspace = true`. Every crate keeps `[lints] workspace = true`.
- After changing types in `hyperion-protocol`, run `just gen-protocol` and commit the regenerated
  bindings.

## Crate boundaries

- `hyperion-sim` does no I/O, reads no clocks, spawns no threads and has no async code. Time enters
  through `dt` arguments and randomness through the seed.
- `hyperion-sim` must be deterministic: never let `HashMap`/`HashSet` iteration order influence
  output (use `BTreeMap`, or sort first), and never seed from the OS or the wall clock.
- `hyperion-protocol` holds only wire types and their serde/ts-rs derives, with no behaviour.
- Binaries stay thin: `main.rs` parses configuration and calls into `lib.rs`, so integration tests
  can reach the logic.

## Errors and panics

- Return `Result` for anything that can fail in normal operation: malformed client input, network
  errors, missing files. Panic only for a broken invariant, which is a bug.
- No `unwrap()` outside tests. Use `expect("...")` only when failure is impossible, and have the
  message state why, for example `.expect("hardcoded address is valid")`.
- Library crates (`hyperion-sim`, `hyperion-protocol`) define concrete error enums that implement
  `std::error::Error` and are `Send + Sync + 'static`. Never use `()` or `String` as an error type.
  `anyhow` is for the server binary's top level only.
- `Display` text for errors is lowercase with no trailing punctuation. Name error types
  verb-object-error: `ParseSeedError`, not `SeedParseError`.
- Propagate with `?` and add context at the boundary where it is known. Never discard an error
  with `let _ =` or `.ok()` without a comment saying why it is safe to ignore.
- `Drop` implementations never panic and never block. Offer an explicit `close()`-style method
  that returns `Result` when teardown can fail.

## Type and API design

- Make invalid states unrepresentable. Validate once in a constructor and carry the proof in the
  type, instead of re-checking raw values downstream.
- Wrap quantities that share a primitive in newtypes (`Kilometres(f64)`, `SystemId(u64)`) so the
  compiler rejects mix-ups. Physical quantities always carry their unit in the type or the name.
- Do not take `bool` or a bare `Option` as a parameter when a two-variant enum would read better
  at the call site.
- Struct fields are private by default; expose getters. Getters have no `get_` prefix: `seed()`,
  `seed_mut()`.
- Conversions follow `as_` (free, borrowed to borrowed), `to_` (expensive), `into_` (consumes
  `self`). Implement `From`/`TryFrom`, never `Into`/`TryInto`.
- Public types derive every applicable trait of `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`,
  `PartialOrd`, `Ord`, `Default`. `Debug` is mandatory. Derive `Copy` only for small plain values
  whose copy semantics are permanent.
- Acronyms are one word in type names: `Uuid`, `WsSession`, not `UUID`, `WSSession`.
- Take ownership when the function needs to own the value and borrow otherwise. Never borrow and
  then clone internally; let the caller decide where the copy happens.
- Accept `&str`, `&[T]` and `&Path` instead of `&String`, `&Vec<T>` and `&PathBuf`. Prefer
  `impl IntoIterator<Item = T>` to a concrete collection when the function only iterates.
- Mark constructors, getters and pure functions `#[must_use]`.
- Use a builder once construction needs more than about four inputs or has optional settings.
- Do not put trait bounds on a struct definition that a derive already implies; put bounds on the
  `impl` blocks that need them.
- Use `pub(crate)` for anything not needed outside the crate. Match exhaustively on enums we own
  instead of using a `_` arm, so new variants produce compile errors.

## Numeric safety

- Integer overflow panics in debug builds and wraps in release. Wherever overflow is reachable,
  use `checked_*`, `saturating_*` or `wrapping_*` and handle the result deliberately.
- Avoid `as` for numeric conversions. Use `From` for lossless conversions and `TryFrom` for
  fallible ones. A remaining `as` needs an `#[expect]` whose reason explains why truncation or
  precision loss cannot occur.
- Never compare floats with `==`. Compare against an explicit tolerance, or use `total_cmp` for
  ordering.

## Unsafe

- `unsafe_code` is `forbid` for the whole workspace. Do not write `unsafe`, and do not relax the
  lint without asking first. If performance seems to demand it, look for a safe crate and measure.
- If an exception is ever granted: every `unsafe fn` documents its contract under `# Safety`, and
  every `unsafe` block is preceded by a `// SAFETY:` comment explaining why the contract holds.

## Async and concurrency (`hyperion-server`)

- Never block the runtime: no `std::thread::sleep`, blocking file or network I/O, or long
  computations inside async code.
- Use `tokio::task::spawn_blocking` for short blocking calls. Long-running CPU work, such as
  procedural generation, goes on a dedicated thread or a CPU pool with a bounded queue. A running
  `spawn_blocking` task cannot be aborted.
- Use `std::sync::Mutex` for short critical sections and never hold its guard across an `.await`;
  scope the guard in a block so it drops first. Use `tokio::sync::Mutex` only when the lock must
  span an `.await`.
- Prefer one task that owns the state, fed by channels, over state shared behind locks.
- Use bounded channels. An unbounded channel needs a comment explaining what limits its growth.
- Every spawned task has an owner that keeps its `JoinHandle` and a shutdown path. Assume a future
  can be dropped at any `.await`, and keep state consistent across those points.
- Log with `tracing` macros and structured fields (`tracing::info!(%addr, "listening")`), never
  `println!` or `eprintln!`.

## Performance

- Choose the right algorithm and data structure first, then measure before micro-optimising.
  Optimise hot paths only, and leave a comment citing the measurement for any non-obvious
  optimisation.
- Default to `Vec` and `HashMap`. Use `BTreeMap` for sorted or range access, `VecDeque` for
  queues, and never `LinkedList`.
- Call `with_capacity` or `reserve` when the size is known or can be bounded.
- In loops, hoist collections and `String` buffers outside and `clear()` them to reuse the
  allocation. Use `clone_from` instead of assigning a fresh `clone()`.
- Use the entry API (`map.entry(k).or_insert_with(..)`) instead of a lookup followed by an insert.
- Prefer iterator chains to index loops. Do not `collect()` into an intermediate `Vec` only to
  iterate it again.
- Treat every `clone()` as something to justify. Borrow, move, or share with `Arc` instead. Use
  `Cow<'_, str>` when a value is usually borrowed and only occasionally owned.
- Avoid `format!` and `to_string()` in hot paths when a literal or `write!` into an existing
  buffer would do.
- Use generics (static dispatch) on hot paths. Use `dyn Trait` for heterogeneous collections or
  to limit code size on cold paths.
- Keep frequently instantiated types small: box large, rarely used enum variants.

## Documentation

- Every public item has a `///` doc comment, and every crate and public module opens with `//!`
  docs describing its role and boundaries.
- The first line is a single-sentence summary, because rustdoc reuses it in listings. Follow it
  with a blank line and then the detail. Do not restate what the signature already says.
- Use these sections, in this order, whenever they apply: `# Errors` (each failure condition),
  `# Panics` (each reachable panic), `# Safety` (caller obligations), `# Examples`.
- Give an example for any public API in `hyperion-sim` and `hyperion-protocol` whose use is not
  obvious from its signature. Examples show why you would call the item, not only how.
- Examples are doctests and must compile and pass. Use `?` rather than `unwrap()`, hiding the
  scaffolding with `#` lines and ending with `# Ok::<(), ErrorType>(())`.
- Use `no_run` for examples needing a network or a long runtime, and `should_panic` or
  `compile_fail` when that is the point. Do not use `ignore`; mark non-Rust blocks as `text`.
- Link related items with intra-doc links (``[`Simulation::step`]``) instead of bare names.
- State the unit, valid range and reference frame of every physical quantity, and cite the source
  of physical constants and astrophysical models.
- Ordinary `//` comments explain why, never what. Do not leave commented-out code or a `TODO`
  without an issue reference.

## Tests

- Unit tests live in the file under test, in `#[cfg(test)] mod tests` with `use super::*;`. They
  may exercise private functions.
- Integration tests live in `crates/<crate>/tests/` and use only the public API. Shared helpers
  go in `tests/common/mod.rs`, never `tests/common.rs`.
- Every behaviour change comes with a test, and every bug fix with a regression test that fails
  before the fix.
- Name tests for the behaviour they verify, without a `test_` prefix:
  `ping_is_answered_with_matching_nonce`.
- Use `assert_eq!` and `assert_ne!` rather than `assert!(a == b)`, so failures print both values.
  Add a message when the assertion is not self-explanatory.
- `unwrap()` and `expect()` are fine in tests. Assert on a specific error variant, not merely
  `is_err()`. `#[should_panic]` always carries `expected = "..."`.
- Tests are deterministic and independent: fixed seeds, no wall-clock time, no `sleep` for
  synchronisation, no fixed ports (bind `127.0.0.1:0`), no dependence on test order.
- Async tests use `#[tokio::test]`. Wrap any wait on the network or a channel in
  `tokio::time::timeout` so that a failure cannot hang the suite.
- `hyperion-sim` keeps a determinism test: the same seed and inputs give identical state across
  two runs.
- Every `hyperion-protocol` message has a test pinning its JSON wire form, because the TypeScript
  client depends on it.

## Sources

Distilled from the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), the
[Rust Book](https://doc.rust-lang.org/book/) (chapters 9 and 11), the
[rustdoc book](https://doc.rust-lang.org/rustdoc/), the
[`std` documentation](https://doc.rust-lang.org/std/), the
[Tokio documentation](https://tokio.rs/tokio/tutorial) and the
[Rust Performance Book](https://nnethercote.github.io/perf-book/). Consult them when a case is not
covered here.
