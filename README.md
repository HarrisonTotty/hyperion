# HYPERION

A spaceship bridge simulation game: players crew the stations of a ship as it navigates an
actual-size, procedurally generated galaxy.

## Galaxy generation

One 64-bit seed produces a **full-size barred spiral galaxy**: about **100 billion star systems**
across roughly 100,000 ly. Nothing is stored. It is all generated on demand, and the same seed
always gives the same galaxy.

**Structure**

- Thin, thick and nuclear discs, a boxy bulge, a bar, spiral arms, a dark-matter halo and a central
  black hole
- Each seed draws its own mass, size, arms and history, within ranges seen in real galaxies

**Every class of star**

- Protostars, T Tauri stars, main sequence, giants, supergiants, Wolf-Rayet stars and LBVs
- White dwarfs, pulsars, magnetars and black holes
- Variable stars and interacting binaries (X-ray binaries, cataclysmic variables, millisecond
  pulsars)
- Supernova kicks fling neutron stars across the galaxy, and runaway stars flee their clusters

**Large structures**

- Globular and open clusters, OB associations, nebulae and supernova remnants
- Stellar streams and the cores of absorbed dwarf galaxies
- A nuclear cluster whose stars orbit the black hole in real time

**Between the stars**

- Brown dwarfs, about 21 rogue planets per star, lone black holes, and dust and gas
- Dust dims each band differently, so optical, IR and radio sensors see to different distances

**Planets**

- Realistic architectures (compact chains, Solar-System-like systems, hot Jupiters) with stable
  orbits
- Radii, atmospheres, habitable zones, moons, rings and belts all come from physics

**A living galaxy**

- One universe clock: stars drift, evolve, are born and die during play
- Novae, supernovae, flares and pulsar glitches all happen
- Sensors see the past: a supernova 10,000 ly away takes 10,000 years to show up

**Navigation**

- Free 3D travel limited by drive range, with no star lanes
- Rotatable 3D star chart and galaxy density map on the bridge

The full design is in
[`docs/agent/brainstorming/galaxy-generation.md`](docs/agent/brainstorming/galaxy-generation.md).

## Layout

| Path                       | What                                                                  |
| -------------------------- | --------------------------------------------------------------------- |
| `crates/hyperion-server`   | Game server binary (axum, WebSocket at `/ws`)                         |
| `crates/hyperion-sim`      | Deterministic simulation / procgen core — no I/O                      |
| `crates/hyperion-base`     | Beneath the sim: maths on the pinned `libm`, units, generator version |
| `crates/hyperion-surface`  | The shared height function, native and WebAssembly (no terrain yet)   |
| `crates/hyperion-protocol` | Wire protocol types; source of truth for the TypeScript bindings      |
| `apps/hyperion`            | Bridge client (Electron + React, built with electron-vite)            |
| `packages/protocol`        | `@hyperion/protocol` — TS bindings generated from the protocol crate  |
| `docs/`                    | Design docs, agent plans, UX guidelines                               |

## Prerequisites

- Rust (toolchain pinned in `rust-toolchain.toml`; `rustup` installs it automatically)
- Node.js ≥ 22.12 and pnpm 12
- [`just`](https://github.com/casey/just)
- [`uv`](https://docs.astral.sh/uv/) — runs [pre-commit](https://pre-commit.com) for the git hooks
- For the WebAssembly checks in `just ci`: the `wasm32-wasip1` and `wasm32-unknown-unknown`
  targets (listed in `rust-toolchain.toml`), [wasmtime](https://wasmtime.dev) at the version the
  justfile pins, `wasm-bindgen-cli` at the version of `wasm-bindgen` that `Cargo.lock` names, and
  [cargo-nextest](https://nexte.st). `just wasm-tools` installs them.

## Development

```sh
just install   # pnpm install
just hooks     # install the git pre-commit and pre-push hooks (once per clone)
just server    # run the game server on 127.0.0.1:7878 (`just server --help` for options)
just client    # run the Electron client with hot reload (`just client --help` for options)
```

Options to `just client` reach the client: `just client --address 10.0.0.5 --port 9100`.

### Platforms

HYPERION builds and runs on Linux, macOS and Windows. The recipes are bash scripts.

- **Linux** is the reference: the checks and every recorded measurement run there. Where
  `systemd-run --user` works, the heavy test runs are capped in memory. `just seed-target` clones
  with `cp --reflink`, or on btrfs takes a snapshot of a source that `just target-subvolume` has
  made a subvolume.
- **macOS** needs the Xcode Command Line Tools (`xcode-select --install`, for clang, git and
  Python 3) and the prerequisites above, all of which Homebrew has. The recipes run on macOS's own
  bash 3.2 and BSD tools. Where macOS lacks a util-linux or coreutils tool that a recipe uses
  (`flock`, `setsid`, `timeout`), the justfile puts a stand-in from `tools/portable/`, written in
  the system Perl, last on `PATH`. The heavy-test lock works as on Linux, but with no systemd the
  runs are not capped in memory. `just seed-target` clones with APFS's clonefile(2).
- **Windows**: run the recipes from WSL 2, a Linux system where they run as on Linux, or from Git
  Bash. PowerShell and `cmd.exe` cannot run them. `just cross-clippy`, and so `just ci`, needs WSL,
  since its stand-in C compiler is a bash script. Neither has been tried on Windows yet.

### Seeing a generated system in `VIEW`

Until sessions exist, the ship is a stand-in that the server starts at the galactic centre, in no
system, and no console moves it. So a plain `just server` and `just client` show `VIEW`'s kept
test scene under `TRAINING`, with `SCENE NOT AVAILABLE: the ship is in no system`. To see the
server's scene of a real system:

```sh
just server        # in one terminal
just place-ship    # in another, once the server is up
just client        # then F2, OPEN the universe place-ship names, and F4 for VIEW
```

`just place-ship` is a client of the running server, like the bridge. It opens the universe
`Dev Fixture` (creating it with seed `4d2` the first time), picks the system nearest
`0,26000,0` ly that has a planet, and sends `scene_ship` to put the ship 0.01 au behind that
planet along its orbit, at rest in the planet's frame, with the clock at 0 s running at 1×. The
seat camera, `VIEW`'s default, looks along the ship's nose, which for the stand-in is its velocity
in the system frame, so the planet is straight ahead. Options choose the rest
(`just place-ship --help`):

| Option                       | Default                                 | What                                                       |
| ---------------------------- | --------------------------------------- | ---------------------------------------------------------- |
| `--universe <NAME or ID>`    | `Dev Fixture`                           | Universe; a name not found is created with `--seed`        |
| `--seed <HEX>`               | `4d2`                                   | Seed of a universe it creates                              |
| `--system <ID or DESIG>`     | nearest with a planet                   | System, by ID, or by designation within the search         |
| `--near <X,Y,Z>`, `--radius` | `0,26000,0` ly, `40` ly                 | Where the system is searched for                           |
| `--look-at <TARGET>`         | `planet`                                | `planet` (the first), `barycentre`, or a body ID           |
| `--distance <AU>`            | 0.01 from a body, 1 from the barycentre | Distance from the target                                   |
| `--time <S>`, `--rate <N>`   | `0`, `1`                                | Scene clock: time from the epoch, and 0 or 1, 10 … 100,000 |
| `--address`, `--port`        | as `just client`                        | The server, also from `HYPERION_SERVER_ADDR` and `_PORT`   |

`--look-at barycentre` puts the ship at rest 1 au along galactic +z from the barycentre, looking
down at it: a single system's star, but empty space between the stars of a wide multiple. The
setting is not saved, so run `just place-ship` again after each server start, and it moves the
scene of every client of that universe.

### Server configuration

The server takes these options, each of which can instead be set by its environment variable.
An option given on the command line wins over its variable.

| Option                  | Variable                       | Default                                      | What                                         |
| ----------------------- | ------------------------------ | -------------------------------------------- | -------------------------------------------- |
| `--address`             | `HYPERION_ADDR`                | `127.0.0.1`                                  | IP address to listen on                      |
| `--port`                | `HYPERION_PORT`                | `7878`                                       | Port to listen on                            |
| `--data-dir`            | `HYPERION_DATA_DIR`            | `./hyperion-data`                            | Where universes are saved                    |
| `--num-workers`         | `HYPERION_WORKERS`             | available parallelism less one, at least one | Generation worker threads                    |
| `--cell-cache`          | `HYPERION_CELL_CACHE_MB`       | `256`                                        | Cache of generated cells, in MiB             |
| `--map-cache`           | `HYPERION_MAP_CACHE_MB`        | `64`                                         | Cache of galaxy density maps, in MiB         |
| `--system-cache`        | `HYPERION_SYSTEM_CACHE_MB`     | `128`                                        | Cache of generated systems' stars, in MiB    |
| `--body-cache`          | `HYPERION_BODY_CACHE_MB`       | `128`                                        | Cache of generated planetary systems, in MiB |
| `--brief-cache`         | `HYPERION_BRIEF_CACHE_MB`      | `64`                                         | Cache of range briefs' star models, in MiB   |
| `--sky-cache`           | `HYPERION_SKY_CACHE_MB`        | `0`, off                                     | Cache of the sky census's cells, in MiB      |
| `--sky-tables`          | `HYPERION_SKY_TABLES_MB`       | `160`                                        | Cache of each galaxy's sky tables, in MiB    |
| `--serve-sky`           | `HYPERION_SERVE_SKY`           | on                                           | Serve `sky` requests                         |
| `--stop-on-stdin-close` | `HYPERION_STOP_ON_STDIN_CLOSE` | off                                          | Stop gracefully when standard input closes   |

`--stop-on-stdin-close` is for a server run as another program's child. The parent stops it by
closing its standard input, which on Windows is the only graceful stop a parent has, and if the
parent dies the pipe closes, so the server never outlives it. It is off by default: with it, a
server started from a terminal stops at an end of input typed there (Ctrl-D on Unix, Ctrl-Z and
Enter on Windows), and one started with standard input closed, as systemd starts it, stops at once.
Its variable takes `y`, `yes`, `t`, `true`, `on` or `1`, or `n`, `no`, `f`, `false`, `off` or `0`,
in any case, and refuses anything else.

`--serve-sky` serves the `sky` request, the stars, band and limits a view's sky is drawn from.
It is on by default (since rendering plan R13's R13.T2, the interim sky). `--serve-sky=false`, or
the variable set to a false value, turns it off: the server then answers `sky` as `unsupported`,
as before the sky was served, and opening a universe builds no sky tables in the background. Its
variable takes the same values as `--stop-on-stdin-close`'s, and so does the option after an equals
sign (`--serve-sky=off`); given alone, `--serve-sky` is on.

`--sky-cache` is 0 by default, which turns the sky's cell cache off: each sky's census reads no
cache and keeps no cell. For now a sky served from the cache, asked again or after a jump, costs
about what it costs without it, since its census still generates every star system, so the cache
would hold up to 1,280 MiB for no saving a player would feel (rendering plan R06, R06.T8.o).
`1280` is the size that holds a sky near the Sun and the cells a 1,000 ly jump adds, the value to
set for a warm sky. The default returns to it once a sky asked again costs at most half of the
first.

The data directory is created with the first universe. Each universe is one directory,
`universes/<id>/`, holding a small `universe.json` with its name, seed and generator version;
nothing generated is saved. To delete a universe, stop the server and remove its directory.

### Client configuration

The client takes the server to link to, as an option or as its variable. It links to
`ws://<address>:<port>/ws`, so by default a server on this machine.

| Option      | Variable               | Default     | What                                  |
| ----------- | ---------------------- | ----------- | ------------------------------------- |
| `--address` | `HYPERION_SERVER_ADDR` | `127.0.0.1` | IP address or host name of the server |
| `--port`    | `HYPERION_SERVER_PORT` | `7878`      | Port the server listens on            |

The variables name the server, not the client, so they are deliberately not the server's own
`HYPERION_ADDR` and `HYPERION_PORT`: an address to listen on and an address to connect to are not
the same thing. Both options are read at launch, so no rebuild is needed to point the client
somewhere else, and the `LINK` display shows the endpoint in use.

## Checks

| Command      | Rust                      | TypeScript                    |
| ------------ | ------------------------- | ----------------------------- |
| `just check` | `cargo check`             | `tsc --noEmit` (TypeScript 7) |
| `just lint`  | `cargo clippy` (pedantic) | `oxlint --type-aware`         |
| `just fmt`   | `cargo fmt`               | `prettier`                    |
| `just test`  | `cargo test`              | `vitest`                      |

`just ci` is the gate before a commit: the four checks above, a check that the fitted tables are
fresh, a check that the generated protocol bindings are up to date, `just cross-clippy`, and
`just test-wasm-fast`, the fast suites on WebAssembly. vitest runs beside its builds, at 4 workers
in a scope capped at 5 GB and 4 cores, and takes no lock; the Rust suites run in one hold of the
heavy-test lock, wasip1 and the browser target side by side. On 2026-10-10 it took 57 minutes on the
recommended machine with nothing else running and its builds warm, 51 of them in the suites, most
of those the sim's (below). A task's gate is `just smart-ci`, the part of it that the change needs
(below); integration runs the whole.

- `just cross-clippy`, part of `just ci`, runs Clippy over every target of the workspace and of
  `tools/gpu-replay` for the other two platforms, of Linux (x86-64), macOS (Apple silicon) and
  Windows (x86-64, MSVC): macOS and Windows from Linux, Windows and Linux from a Mac. Code gated
  to one platform (`cfg(unix)`, `target_os = "linux"`) can leave an import or a helper unused on
  another, which only that platform's Clippy sees. Clippy never links, so it needs no SDK: only the
  platforms' standard libraries, which rustup installs from `rust-toolchain.toml`, and a stand-in
  C compiler that it writes under `target/cross/`. It builds in `target/cross` and
  `target/tools-cross` and runs beside the other builds; on a warm tree it takes about a second.
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

### `just smart-ci`: the part of `just ci` a change needs

`just smart-ci` is a task's gate: the steps of `just ci` that its change reaches, chosen from its
diff. The change is every path that differs between the merge-base of HEAD and
`rendering-and-planets` (or `just smart-ci <ref>`) and the working tree, untracked files included,
so it works before a commit and after. It runs `ci`'s own recipes with `ci`'s flags, features,
locks and phases, scoped to the crates, packages and files concerned, and of the test suites only
the tests that reach the changed code. `just smart-ci --plan` prints the plan, each step with the
paths that caused it, and how many of each crate's tests run and why, and runs nothing; a run
prints it first. `--explain` lists every chosen test with the chain of items from it to the change.
Integration keeps the full `just ci`.

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

| A change to                                                                                                                  | runs                                                                                                                                                                                                                                                                                |
| ---------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Markdown                                                                                                                     | Prettier on those files                                                                                                                                                                                                                                                             |
| a package's sources (`src/`)                                                                                                 | tsc, oxlint, and the vitest files that import or read the change (every one of the package if its setup file does)                                                                                                                                                                  |
| a package's configuration, `package.json`, `tsconfig`                                                                        | tsc, oxlint and every vitest file. No TypeScript change runs no TypeScript check                                                                                                                                                                                                    |
| a crate's code                                                                                                               | that crate's rustfmt and Clippy (native, wasm32 for the browser crates, and macOS and Windows for a crate whose code differs between the platforms: the server, or a crate the guard finds such code in), with the features `just ci` gives it, and the tests that reach the change |
| a crate's `Cargo.toml`                                                                                                       | that crate's checks and every test, and the Clippy and tests of the crates that depend on it                                                                                                                                                                                        |
| a crate's goldens                                                                                                            | the tests of the crate that read goldens, those that name the file, and the vitest files that read it                                                                                                                                                                               |
| a crate's other data (fitted inputs, fixtures)                                                                               | its tests, the tests that name the file, and the vitest files that read it                                                                                                                                                                                                          |
| `crates/hyperion-protocol`, the bindings                                                                                     | the bindings' check and the TypeScript side                                                                                                                                                                                                                                         |
| `packages/protocol/fixtures/`                                                                                                | the TypeScript side and the tests of the crates that read the fixture                                                                                                                                                                                                               |
| the surface module's sources (`hyperion-surface`, `hyperion-base`)                                                           | `gen-surface` and the vitest files that import the module; tsc and oxlint when its exports change                                                                                                                                                                                   |
| the sim's sources (`crates/hyperion-sim/src/`), the testkit's `golden.rs`                                                    | smart-ci's own tests too, whose guard holds the tests that `wasm_sky` leaves out of the wasip1 runs to pinning no value                                                                                                                                                             |
| WGSL, the engine, the smoke harness                                                                                          | `just test-render`'s build, and its run in the same hold of the heavy-test lock as the suites                                                                                                                                                                                       |
| `.config/nextest.toml`, `rustfmt.toml`, `clippy.toml`, Prettier's or oxlint's configuration                                  | the checks it configures                                                                                                                                                                                                                                                            |
| `justfile`, `Cargo.toml`, `Cargo.lock`, `pnpm-lock.yaml`, `rust-toolchain.toml`, `.cargo/`, `tools/portable/`, unknown paths | the full `just ci`                                                                                                                                                                                                                                                                  |

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
its documentation is that crate's data. A changed file's removed items still count, so that a test
naming one runs. Where the reading cannot be sure, it widens: a file it cannot follow, a file in no
module tree, a changed crate attribute, global allocator or panic handler, and a module outside the
crate's directory run their crate's every test, and an unresolved path falls back to every item of
its name. The graph's facts are cached in `target/smart-ci/`, keyed by each file's git object id and the
reader's own source, so a stale entry is never used; a build takes about 3 s with the cache, 10 to
40 s without. On the TypeScript side, the vitest files are chosen from the import graph (imports,
`export ... from`, dynamic imports, workers' `new URL(...)`, `vi.mock`, `import.meta.glob`, `?raw`
and other assets) and from the paths that tests read at run time.
`just smart-ci --all-tests` runs every test of the changed crates and every vitest file instead, as
smart-ci did before the graph.

By default the Clippy of only the changed crates runs, not of the crates that depend on them:
integration's full `just ci` catches a break there. Their tests are the graph's to choose: every
test of theirs that reaches the change runs. So `just smart-ci --dependents` no longer adds their
tests; it adds their Clippy, which builds all of their code and not only the code their tests
reach, for a change to what other crates use (a public item's name or signature). With
`--all-tests` it adds every test of theirs as well, as before the graph. `just ci` runs smart-ci's tests
(`tools/smart-ci/test_smart_ci.py`, which runs `test_rust_graph.py`'s too): the graph's traps, each
on a small workspace of its own, and the agreement with `ci`, which fails if `ci` gains a step, or a
step gains a command, that smart-ci does not know.

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

### Git hooks

`just hooks` installs hooks defined in `.pre-commit-config.yaml`. On commit they run file hygiene
checks, `cargo fmt`, `cargo clippy`, `prettier`, `oxlint`, `tsc`, and the protocol-bindings check
(each only when relevant files changed); on push they run both test suites. `just pre-commit` runs
the commit-stage hooks against every file.

### Changing the protocol

Edit the types in `crates/hyperion-protocol`, run `just gen-protocol`, re-export any new type
from `packages/protocol/src/index.ts`, and commit the regenerated bindings. `just ci` fails if they
are stale.
