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
  `systemd-run --user` works, the heavy test runs are capped in memory.
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
| `--sky-cache`           | `HYPERION_SKY_CACHE_MB`        | `64`, provisional until R06.T8.n             | Cache of the sky census's cells, in MiB      |
| `--sky-tables`          | `HYPERION_SKY_TABLES_MB`       | `160`                                        | Cache of each galaxy's sky tables, in MiB    |
| `--serve-sky`           | `HYPERION_SERVE_SKY`           | off                                          | Serve `sky` requests                         |
| `--stop-on-stdin-close` | `HYPERION_STOP_ON_STDIN_CLOSE` | off                                          | Stop gracefully when standard input closes   |

`--stop-on-stdin-close` is for a server run as another program's child. The parent stops it by
closing its standard input, which on Windows is the only graceful stop a parent has, and if the
parent dies the pipe closes, so the server never outlives it. It is off by default: with it, a
server started from a terminal stops at an end of input typed there (Ctrl-D on Unix, Ctrl-Z and
Enter on Windows), and one started with standard input closed, as systemd starts it, stops at once.
Its variable takes `y`, `yes`, `t`, `true`, `on` or `1`, or `n`, `no`, `f`, `false`, `off` or `0`,
in any case, and refuses anything else.

`--serve-sky` turns on the `sky` request, the stars, band and limits a view's sky is drawn from.
It is off by default until the sky's census near the Sun is fast enough to serve (rendering plan
R06, R06.T8.g), and the server then answers `sky` as `unsupported`, as before the sky was served.
Its variable takes the same values as `--stop-on-stdin-close`'s.

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
`just test-wasm-fast`, the fast suites on WebAssembly. Without the WebAssembly suites it took about
three minutes on a quiet machine; they add about two more (measured under shared load, to be
re-timed quiet).

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
  compared natively, on wasip1 and in the browser. `just test-wasm-slow`, part of `just ci-slow`,
  runs their slow tests and doctests there; `just test-wasm` runs both. A missing tool fails them
  with a pointer to `just wasm-tools`; they never skip. Nothing checks AArch64.
- `just test-wasm-browser` runs the tests of the crates the client ships or tests with
  (`hyperion-base`, `hyperion-surface`, `hyperion-testkit`; not the sim, which no browser loads) as
  `wasm32-unknown-unknown`, the client's target, under `wasm-bindgen-test` on the V8 that Electron
  ships: `tools/electron-node/node` runs Electron as Node, and the recipe's first line shows the
  Electron and V8 versions it ran on. It fails if any test that runs natively, outside a
  `native_only` module, is missing there, since a plain `#[test]` is silently dropped on that
  target (each test module imports `wasm_bindgen_test::wasm_bindgen_test as test` instead).
- `just test`, `just test-slow`, `just bench` and the WebAssembly checks build first, then run
  under one lock shared by every worktree of the clone (`.git/hyperion-heavy-tests.lock`). A second
  run waits for the first to finish, and says so, because two suites at once each take twice as
  long, and the load fails the timing-sensitive server tests.

### Git hooks

`just hooks` installs hooks defined in `.pre-commit-config.yaml`. On commit they run file hygiene
checks, `cargo fmt`, `cargo clippy`, `prettier`, `oxlint`, `tsc`, and the protocol-bindings check
(each only when relevant files changed); on push they run both test suites. `just pre-commit` runs
the commit-stage hooks against every file.

### Changing the protocol

Edit the types in `crates/hyperion-protocol`, run `just gen-protocol`, re-export any new type
from `packages/protocol/src/index.ts`, and commit the regenerated bindings. `just ci` fails if they
are stale.
