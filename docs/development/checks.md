# Checks

| Command      | Rust                      | TypeScript                    |
| ------------ | ------------------------- | ----------------------------- |
| `just check` | `cargo check`             | `tsc --noEmit` (TypeScript 7) |
| `just lint`  | `cargo clippy` (pedantic) | `oxlint --type-aware`         |
| `just fmt`   | `cargo fmt`               | `prettier`                    |
| `just test`  | `cargo test`              | `vitest`                      |

`just ci` is the gate before a commit: the four checks above, a check that the fitted tables are
fresh, a check that the generated protocol bindings are up to date, `just cross-clippy`,
`just doc-check`, and `just test-wasm-fast`, the fast suites on WebAssembly. vitest runs beside
its builds, at 4 workers in a scope capped at 5 GB and 4 cores, and takes no lock; the Rust suites
run in one hold of the heavy-test lock, wasip1 and the browser target side by side. On 2026-10-10
it took 57 minutes on the recommended machine with nothing else running and its builds warm, 51 of
them in the suites, most of those the sim's (below). A task's gate is `just smart-ci`, the part of
it that the change needs (below); integration runs the whole.

- `just cross-clippy`, part of `just ci`, runs Clippy over every target of the workspace and of
  `tools/gpu-replay` for the other two platforms, of Linux (x86-64), macOS (Apple silicon) and
  Windows (x86-64, MSVC): macOS and Windows from Linux, Windows and Linux from a Mac. Code gated
  to one platform (`cfg(unix)`, `target_os = "linux"`) can leave an import or a helper unused on
  another, which only that platform's Clippy sees. Clippy never links, so it needs no SDK: only the
  platforms' standard libraries, which rustup installs from `rust-toolchain.toml`, and a stand-in
  C compiler that it writes under `target/cross/`. It builds in `target/cross` and
  `target/tools-cross` and runs beside the other builds; on a warm tree it takes about a second.
- `just doc-check`, part of `just ci`, runs rustdoc over every crate of the workspace
  (`cargo doc --workspace --no-deps`) with its warnings as errors: an intra-doc link that does not
  resolve, that names an item that is not public, or whose name is ambiguous, a bare URL, a stray
  HTML tag, a redundant link target. Link a public item by a path rustdoc resolves from the
  documented item's module (``[`super::Y`]``, ``[`crate::x::Y`]``), and name one that is not
  public in backticks with no link. It builds in the workspace's own build directory, right after
  Clippy, whose dependency metadata it reuses: about 11 s from `cargo clean --doc`, well under a
  second warm. It documents the host's target alone, so code behind another target's cfg (the
  surface crate's `wasm` module) goes unchecked.
- `just ci-slow` is `just ci` plus `just test-slow` and `just test-wasm-slow`. The slow tests take
  far longer than the rest, so run it before a push that changes the sim, and after a
  `GENERATOR_VERSION` bump.
- `just test-slow` runs the slow statistical tests, marked `#[ignore = "slow: ..."]`, under the
  `slow-test` profile (release speed with debug assertions on). It uses
  [cargo-nextest](https://nexte.st) (`cargo install cargo-nextest --locked`), which runs the tests
  of every binary side by side instead of one binary at a time; `just test-slow <filter>` narrows
  it.
- `just bench` runs the Criterion benchmarks (`just bench -- <filter>` narrows them); a regression
  is a finding to raise, never a failure.
- `just bless` rewrites the golden files under `crates/*/tests/golden/` after a deliberate
  `GENERATOR_VERSION` bump; it refuses to run under `CI`.
- `just repin <test name>…` re-pins the figures those tests pin (`hyperion_testkit::pins`, kept in
  `crates/*/tests/pins/`), blessing by test name as goldens are; `just repin --check` fails on a
  stale pin.
- `just test-wasm-fast`, part of `just ci`, checks generated output bit for bit on WebAssembly as
  well as on native x86-64: it runs the fast tests of `hyperion-base`, `hyperion-surface`,
  `hyperion-sim` and `hyperion-testkit`, goldens included, as `wasm32-wasip1` under wasmtime, where
  `usize` is 32 bits, with cargo-nextest (one process per test, since wasip1 has no threads); and
  it checks that a build with relaxed SIMD fails, runs Clippy for the browser target over the
  crates that run there, and runs `just test-wasm-browser` (below), so that the same goldens are
  compared natively, on wasip1 and in the browser. Its wasip1 run, and smart-ci's, leave out the
  sky table modules' tests that compare one wasip1 computation with another (the justfile's
  `wasm_sky`): `just test-wasm-sky` runs them there, and integration's slow pass runs it every day a
  commit lands. A test that pins a value on every target stays in `ci`, and smart-ci's tests fail
  if `wasm_sky` selects one. `just test-wasm-slow`, part of `just ci-slow`, runs `just test-wasm-sky`
  and their slow tests and doctests there; `just test-wasm` runs both. A missing tool fails them
  with a pointer to `just wasm-tools`; they never skip. Nothing checks AArch64.
- `just test-wasm-browser` runs the tests of the crates the client ships or tests with
  (`hyperion-base`, `hyperion-surface`, `hyperion-testkit`; not the sim, which no browser loads) as
  `wasm32-unknown-unknown`, the client's target, under `wasm-bindgen-test` on the V8 that Electron
  ships: `tools/electron-node/node` runs Electron as Node, and the recipe's first line shows the
  Electron and V8 versions it ran on. It fails if any test that runs natively, outside a
  `native_only` module, is missing there, since a plain `#[test]` is silently dropped on that
  target (each test module imports `wasm_bindgen_test::wasm_bindgen_test as test` instead).
- `just test`, `just test-slow`, `just bench` and the WebAssembly checks build first, then run
  under one lock shared by every worktree of the clone (`.git/hyperion-heavy-tests.lock`); vitest,
  at 4 workers in its capped scope, is not a heavy run and runs outside it. A second
  run waits for the first to finish, and says so, because two suites at once each take twice as
  long, and the load fails the timing-sensitive server tests. A run with `HYPERION_HEAVY_PRIORITY=1`
  (integration's) goes first: while it waits, every other waiter steps aside, so it takes the lock
  at its next release.

## `just smart-ci`: the part of `just ci` a change needs

`just smart-ci` is a task's gate: the steps of `just ci` that its change reaches, chosen from its
diff. The change is every path that differs between the merge-base of HEAD and
`rendering-and-planets` (or `just smart-ci <ref>`) and the working tree, untracked files included,
so it works before a commit and after. It runs `ci`'s own recipes with `ci`'s flags, features,
locks and phases, scoped to the crates, packages and files concerned, and of the test suites only
the tests that reach the changed code. `just smart-ci --plan` prints the plan, each step with the
paths that caused it, and how many of each crate's tests run and why, and runs nothing; a run
prints it first. `--explain` lists every chosen test with the chain of items from it to the change.
A plan that runs vitest or tsc first checks the Node floor, and on an older Node stops there, in one
line, before anything builds. Integration keeps the full `just ci`.

Where builds share a machine's build slots (a command that runs its arguments while it holds one),
set `SMART_CI_SLOT` to that command instead of wrapping smart-ci in it. smart-ci then holds a slot
only for its setup phase and then its side, vitest and main phases, which build, and releases it
before the heavy-test lock:
a wrapper would keep the slot through the wait for the lock and the suites. A full plan runs `ci`'s
own phases the same way, and `just smart-ci --full` runs that plan whatever the diff, in place of
`build-slot just ci`. Unset, as on any machine without build slots, the phases run directly. With
`SMART_CI_PRIORITY=1` (integration's runs, over its unpushed commits or with `--full`), any plan's
suites that take the heavy-test lock take it with that priority, and every recipe line carries the
time it started (`JUST_TIMESTAMP=true`). Each nextest run writes a JUnit report of its tests' start
times and durations to `target/nextest/<profile>/junit.xml`: `default` for the native suites,
`wasip1` for the wasip1 ones.

**Small suites run without the heavy-test lock.** The lock is for full suites and runs over 4 GB.
A plan's suites run without it when they are small by the timings recorded from earlier runs: at
most 300 of nextest's tests and 300 s of test time, doctests and browser suites included (39% of
the last 112 commits' plans, which at 4 test threads take about 2 minutes at most). They still run
capped, at 4 test threads in a scope of 5 GB and 4 cores where systemd allows it, and hold no build
slot. Each run of the suites through smart-ci records nextest's time for each test that passed,
rustdoc's for each crate's doctests and the browser runner's for each crate's suite in
`target/smart-ci/test-times.json`; a test with no recorded time counts as large, so the first run
in a fresh build directory takes the lock, and so does a test that reads a fixture built once a run
(the sky's luminosity tables), whose recorded time leaves its build out. `--full` and the smoke
harness always take it. The plan says which, and why.

| A change to                                                                                                                  | runs                                                                                                                                                                                                                                                                                                                                                                    |
| ---------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Markdown                                                                                                                     | Prettier on those files                                                                                                                                                                                                                                                                                                                                                 |
| a package's sources (`src/`)                                                                                                 | tsc, oxlint, and the vitest files that import or read the change (every one of the package if its setup file does)                                                                                                                                                                                                                                                      |
| a package's configuration, `package.json`, `tsconfig`, the Node floor's preflight (`tools/node-floor/`)                      | tsc, oxlint and every vitest file (and smart-ci's tests, for the preflight). No TypeScript change runs no TypeScript check                                                                                                                                                                                                                                              |
| a crate's code                                                                                                               | that crate's rustfmt and Clippy (native, wasm32 for the browser crates, and macOS and Windows for a crate whose code differs between the platforms: the server, or a crate the guard finds such code in), with the features `just ci` gives it, its rustdoc (`just doc-check`, with the features `cargo doc --workspace` gives it), and the tests that reach the change |
| a crate's `Cargo.toml`                                                                                                       | that crate's checks and every test, and the Clippy, rustdoc and tests of the crates that depend on it                                                                                                                                                                                                                                                                   |
| a crate's goldens and pins                                                                                                   | the tests of the crate that read goldens (or pins), those that name the file, and the vitest files that read it                                                                                                                                                                                                                                                         |
| a crate's other data (fitted inputs, fixtures)                                                                               | its tests, the tests that name the file, and the vitest files that read it (a Markdown file its root includes as docs is data, and runs its rustdoc too)                                                                                                                                                                                                                |
| `crates/hyperion-protocol`, the bindings                                                                                     | the bindings' check and the TypeScript side                                                                                                                                                                                                                                                                                                                             |
| `packages/protocol/fixtures/`                                                                                                | the TypeScript side and the tests of the crates that read the fixture                                                                                                                                                                                                                                                                                                   |
| the surface module's sources (`hyperion-surface`, `hyperion-base`)                                                           | `gen-surface` and the vitest files that import the module; tsc and oxlint when its exports change                                                                                                                                                                                                                                                                       |
| the sim's sources (`crates/hyperion-sim/src/`), the testkit's `golden.rs` and `pins.rs`                                      | smart-ci's own tests too, whose guard holds the tests that `wasm_sky` leaves out of the wasip1 runs to pinning no value                                                                                                                                                                                                                                                 |
| WGSL, the engine, the smoke harness                                                                                          | `just test-render`'s build, and its run in the same hold of the heavy-test lock as the suites                                                                                                                                                                                                                                                                           |
| `.config/nextest.toml`, `rustfmt.toml`, `clippy.toml`, Prettier's or oxlint's configuration                                  | the checks it configures                                                                                                                                                                                                                                                                                                                                                |
| `justfile`, `Cargo.toml`, `Cargo.lock`, `pnpm-lock.yaml`, `rust-toolchain.toml`, `.cargo/`, `tools/portable/`, unknown paths | the full `just ci`                                                                                                                                                                                                                                                                                                                                                      |

**Clippy for the other platforms.** `just ci` runs `cross-clippy` over the whole workspace. smart-ci
runs it only for the crates whose code differs between Linux, macOS and Windows: those in
`CROSS_CLIPPY_CRATES` (`tools/smart-ci/smart_ci.py`, the server today) and tools/gpu-replay. Every
other crate is the same code on all three after cfg expansion, which native Clippy lints. A guard
keeps the list honest. It finds a crate's cfg predicates that take different values on the three
desktop targets, its `std::os` and `core::os` paths, its dependencies on `libc`, `nix`,
`windows-sys` or `winapi`, its platform tables in `Cargo.toml` and any build script. It reads every
target `--all-targets` lints. A crate it flags gets its cross-Clippy in the plan, and `just ci`
(smart-ci's tests) fails until the crate is in the list.

**The tests a change reaches.** `tools/smart-ci/rust_graph.py` builds a dependency graph of the
workspace's Rust code, item by item (each function, type, trait, impl and its members, constant,
static and macro), from the source alone, and a test runs when it reaches a changed item through
it, in its own crate or in any crate that depends on it: nextest and the wasip1 suites test by test
(`-E` filters), doctests by file, the browser suites by crate. Names are resolved as rustc resolves
them: module trees and `#[path]`, imports, globs and re-exports, the extern prelude, a turbofish's
types, a format string's `{name}`, a path in an attribute's string (`serialize_with = "f"`). A
method call names no item, so an impl is attached to its type, and a changed method runs the tests
that both hold its type and name it (a macro's body and its arguments count as naming it); an impl
of another crate's trait (operators, `for`, `?`, derives call it unnamed) stays whole, and so does
a member that is a macro call. Macros' bodies are their callers' code, and an item-position call
defines what the body defines; `cfg` counts every configuration, and a changed `cfg` on an import
changes the import; files read as data count by their path's string, by a directory's name (from
any crate), or by a listing of their directory, and a Markdown file that a crate root includes as
its documentation is that crate's data. A test that reads the Rust sources as text by listing a
directory reaches a source walker, an item whose code lists a directory (`read_dir`, as a function
or as the method `dir.read_dir()`, or `WalkDir`) and holds the string `rs` or one ending in `.rs`:
a changed `.rs` file of a crate, added or deleted ones too, runs every such test whose scope holds
that crate, in any crate and whatever the crates' dependencies, in the native step alone, whose
causes name "the tests that read the Rust sources". The scope is read from the strings of the code
the test reaches: `src` names its own crate, `../<crate dir>` or a crate's directory name that
crate, and a directory computed rather than named (`parent()`, `..` in another form, `crates`, an
environment variable other than `CARGO_MANIFEST_DIR`), or named by nothing, is every crate's, as
the surface crate's tag check's is; `--explain` shows each test's scope. A test that the change
reaches through its code runs whatever its scope. A test that reads Rust sources by listing a
directory names the `rs` extension in the function that lists it, so that smart-ci finds it. After
the tests chosen, the plan lists the slow tests (`#[ignore]`d ones) that the graph reaches, which
no step runs, so that a lane whose change reaches a slow test runs it by name. A changed file's
removed items still count, so that a test naming one runs. Where the reading cannot be sure, it
widens: a file it cannot follow, a file in no module tree, a changed crate attribute, global
allocator or panic handler, and a module outside the crate's directory run their crate's every
test, and an unresolved path falls back to every item of its name. The graph's facts are cached in
`target/smart-ci/`, keyed by each file's git object id and the reader's own source, so a stale
entry is never used; a build takes about 3 s with the cache, 10 to 40 s without. On the TypeScript
side, the vitest files are chosen from the import graph (imports, `export ... from`, dynamic
imports, workers' `new URL(...)`, `vi.mock`, `import.meta.glob`, `?raw` and other assets) and from
the paths that tests read at run time.
`just smart-ci --all-tests` runs every test of the changed crates and every vitest file instead, as
smart-ci did before the graph: another crate's tests that read the Rust sources (the surface crate's
tag check, for a change to the sim) run only without it.

By default the Clippy and rustdoc of only the changed crates run, not of the crates that depend on
them: integration's full `just ci` catches a break there. Their tests are the graph's to choose:
every test of theirs that reaches the change runs. So `just smart-ci --dependents` no longer adds
their tests; it adds their Clippy, which builds all of their code and not only the code their tests
reach, and their rustdoc, for a change to what other crates use (a public item's name, signature or
path, which their docs may link). With `--all-tests` it adds every test of theirs as well, as
before the graph. `just ci` runs smart-ci's tests (`tools/smart-ci/test_smart_ci.py`, which runs
`test_rust_graph.py`'s too): the graph's traps, each on a small workspace of its own, and the
agreement with `ci`, which fails if `ci` gains a step, or a step gains a command, that smart-ci does
not know.

**Gate stamps: the git hooks don't repeat a passing gate.** A run of `just smart-ci` that passes
writes a stamp keyed by the tree it checked: the git tree id of the working tree, uncommitted and
untracked files included (what `git add -A` would commit), so that any change of content gives
another key. Stamps live in `<git common dir>/smart-ci/stamps/<tree>.json`, shared by every
worktree of the clone, and record the run's base, its plan's steps with the paths that caused each,
the changed paths, what it ran with (the toolchain, the cargo, rustc, Clippy, nextest and
`HYPERION_*` environment, cargo's configuration in `CARGO_HOME`, the node_modules installed) and the
time. A failed run writes none and withdraws the tree's earlier one. The hooks' expensive checks run
through `tools/smart-ci/stamp.py hook`, which skips a check, printing why, when a stamp of the exact
tree being committed (the index's) or pushed covers it and the working tree the check would read is
that tree, and otherwise runs it as before:

- the workspace Clippy and the bindings' check (on commit), only when the plan ran the hook's own
  command: a full plan, or `just smart-ci --hook-checks`, which runs `cargo clippy --workspace`, and
  the bindings' check over `--workspace`, in place of the changed crates' runs, inside the gate's
  build slot. smart-ci's usual `-p` selection is not the hook's check: it leaves out the crates that
  depend on the change, which a workspace run builds again and can newly fail, and outside
  `--workspace` the dependencies resolve with fewer features. A plan with a Rust or protocol change
  says so in a note;
- oxlint and tsc (on commit), when the plan ran them (both are whole-tree runs);
- `just test` (on push), when smart-ci passed on the pushed commit's tree: the tests that reach the
  change, by its graph, ran, and the rest are as the base's earlier gates left them. This is the
  premise of smart-ci's gate itself, and stricter than a push with `--no-verify` after it.

A stamp counts only when its base is an ancestor of `origin/rendering-and-planets` (a full plan's
whatever its base): pushed by integration after its smart-ci gate, and checked later by the owner's
full CI. A base on picks not yet pushed counts once they are. It counts only with the
toolchain (rustc, cargo, node, pnpm), environment and configuration it recorded, and not when its
change edits `tools/smart-ci/`, whose rules would decide their own coverage. The other hooks
(formatting, whitespace, large files, session links, the commit message) always run.
`SMART_CI_NO_STAMPS=1` runs every check whatever the stamps, as `just pre-commit` does, and
`python3 -I tools/smart-ci/stamp.py check <commit>` prints whether a tree has a valid stamp, against
which base, and which hooks it covers.

Measured on 2026-10-10 on the recommended machine, with nothing else running and the builds warm:
the full `just ci` took 3,400 s, 3,085 s of it in the suites (native 1,278 s, wasip1 1,418 s,
vitest 49 s), and a Markdown change under `just smart-ci` 1.3 s. With the graph (measured on
2026-10-11 on the shared machine, so provisional), the native tests of the changed crate and the
crates that depend on it, all of them against those the change reaches:

- a sim `planetary/` change (a phase function): 1,269 s for all 3,616 tests, 233 s for the 463
  that reach it;
- a sim `sky/` change (the eye's glare): 1,277 s, 471 s for 217 (the sky's slow tests among them);
- a `hyperion-surface` change (the relief's octaves): 1,360 s for 3,880, 4 s for 22;
- a client change to `lib/format.ts`, which most displays import: 77 of the 366 vitest files, half
  of the suite's file-seconds (261 of 524 s) but little of its wall time (63 s against 57 to 73 s),
  since its slowest file is among them and vitest's start dominates; a leaf file runs a handful.

The wasip1 suites are narrowed by the same filters. Replayed over 12 recent code commits at CI-93's
per-test times, the native and wasip1 suites' test-seconds fell by 71% (127,262 to 36,994), and
each commit's own changed tests were among those chosen.

## Git hooks

`just hooks` installs hooks defined in `.pre-commit-config.yaml`. On commit they run file hygiene
checks, `cargo fmt`, `cargo clippy`, `prettier`, `oxlint`, `tsc`, and the protocol-bindings check
(each only when relevant files changed); on push they run both test suites. `just pre-commit` runs
the commit-stage hooks against every file.

## Changing the protocol

Edit the types in `crates/hyperion-protocol`, run `just gen-protocol`, re-export any new type
from `packages/protocol/src/index.ts`, and commit the regenerated bindings. `just ci` fails if they
are stale.

## Kernel digest pins

After resolving a digest conflict, run `just pin-digests`: it rewrites the WGSL digest pins of
`discReflectance.test.ts` and `thickKernels.test.ts` from the kernels' sources, with no test run
(`just pin-digests --check` only compares).
