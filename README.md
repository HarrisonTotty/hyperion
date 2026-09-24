# HYPERION

A spaceship bridge simulation game: players crew the stations of a ship as it navigates an
actual-size, procedurally generated galaxy.

## Layout

| Path                       | What                                                                 |
| -------------------------- | -------------------------------------------------------------------- |
| `crates/hyperion-server`   | Game server binary (axum, WebSocket at `/ws`)                        |
| `crates/hyperion-sim`      | Deterministic simulation / procgen core — no I/O                     |
| `crates/hyperion-protocol` | Wire protocol types; source of truth for the TypeScript bindings     |
| `apps/hyperion`            | Bridge client (Electron + React, built with electron-vite)           |
| `packages/protocol`        | `@hyperion/protocol` — TS bindings generated from the protocol crate |
| `docs/`                    | Design docs, agent plans, UX guidelines                              |

## Prerequisites

- Rust (toolchain pinned in `rust-toolchain.toml`; `rustup` installs it automatically)
- Node.js ≥ 22.12 and pnpm 12
- [`just`](https://github.com/casey/just)
- [`uv`](https://docs.astral.sh/uv/) — runs [pre-commit](https://pre-commit.com) for the git hooks

## Development

```sh
just install   # pnpm install
just hooks     # install the git pre-commit and pre-push hooks (once per clone)
just server    # run the game server on 127.0.0.1:7878 (`just server --help` for options)
just client    # run the Electron client with hot reload (`just client --help` for options)
```

Options to `just client` reach the client: `just client --address 10.0.0.5 --port 9100`.

### Server configuration

The server takes these options, each of which can instead be set by its environment variable.
An option given on the command line wins over its variable.

| Option           | Variable                   | Default                                      | What                                      |
| ---------------- | -------------------------- | -------------------------------------------- | ----------------------------------------- |
| `--address`      | `HYPERION_ADDR`            | `127.0.0.1`                                  | IP address to listen on                   |
| `--port`         | `HYPERION_PORT`            | `7878`                                       | Port to listen on                         |
| `--data-dir`     | `HYPERION_DATA_DIR`        | `./hyperion-data`                            | Where universes are saved                 |
| `--num-workers`  | `HYPERION_WORKERS`         | available parallelism less one, at least one | Generation worker threads                 |
| `--cell-cache`   | `HYPERION_CELL_CACHE_MB`   | `256`                                        | Cache of generated cells, in MiB          |
| `--map-cache`    | `HYPERION_MAP_CACHE_MB`    | `64`                                         | Cache of galaxy density maps, in MiB      |
| `--system-cache` | `HYPERION_SYSTEM_CACHE_MB` | `128`                                        | Cache of generated systems' stars, in MiB |

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

`just ci` is the gate before a commit: the four checks above plus a check that the generated
protocol bindings are up to date. It takes about three minutes.

- `just ci-slow` is `just ci` plus `just test-slow`. The slow tests add about nineteen minutes, so
  run it before a push that changes the sim, and after a `GENERATOR_VERSION` bump.
- `just test-slow` runs the slow statistical tests, marked `#[ignore = "slow: ..."]`, under the
  `slow-test` profile (release speed with debug assertions on).
- `just bench` runs the Criterion benchmarks (`just bench -- <filter>` narrows them); a regression
  is a finding to raise, never a failure.
- `just bless` rewrites the golden files under `crates/*/tests/golden/` after a deliberate
  `GENERATOR_VERSION` bump; it refuses to run under `CI`.
- `just test-wasm` runs the sim's and the testkit's tests, goldens and slow tests included, as
  `wasm32-wasip1` under wasmtime, where `usize` is 32 bits. It needs wasmtime and the target
  (`rustup target add wasm32-wasip1`), so it is part of neither `ci` nor `ci-slow`. Run it, and the
  same tests on AArch64, to check generated output bit for bit on three architectures.

### Git hooks

`just hooks` installs hooks defined in `.pre-commit-config.yaml`. On commit they run file hygiene
checks, `cargo fmt`, `cargo clippy`, `prettier`, `oxlint`, `tsc`, and the protocol-bindings check
(each only when relevant files changed); on push they run both test suites. `just pre-commit` runs
the commit-stage hooks against every file.

### Changing the protocol

Edit the types in `crates/hyperion-protocol`, run `just gen-protocol`, re-export any new type
from `packages/protocol/src/index.ts`, and commit the regenerated bindings. `just ci` fails if they
are stale.
