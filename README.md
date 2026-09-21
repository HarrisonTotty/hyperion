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
just server    # run the game server on 127.0.0.1:7878 (override with HYPERION_ADDR)
just client    # run the Electron client with hot reload
```

The client connects to `ws://127.0.0.1:7878/ws`; override with `VITE_HYPERION_SERVER_URL`.

## Checks

| Command      | Rust                      | TypeScript                    |
| ------------ | ------------------------- | ----------------------------- |
| `just check` | `cargo check`             | `tsc --noEmit` (TypeScript 7) |
| `just lint`  | `cargo clippy` (pedantic) | `oxlint --type-aware`         |
| `just fmt`   | `cargo fmt`               | `prettier`                    |
| `just test`  | `cargo test`              | `vitest`                      |

`just ci` runs everything CI runs, including a check that the generated protocol bindings are
up to date.

- `just test-slow` runs the slow statistical tests, marked `#[ignore = "slow: ..."]`, under the
  `slow-test` profile (release speed with debug assertions on); CI runs it after `just test`.
- `just bench` runs the Criterion benchmarks (`just bench -- <filter>` narrows them); a regression
  is a finding to raise, never a CI failure.
- `just bless` rewrites the golden files under `crates/*/tests/golden/` after a deliberate
  `GENERATOR_VERSION` bump; it refuses to run under `CI`.
- `just test-wasm` runs the sim's and the testkit's tests, goldens and slow tests included, as
  `wasm32-wasip1` under wasmtime. It needs `rustup target add wasm32-wasip1` and wasmtime, so it is
  not part of `just ci`. CI runs it, and the same tests on AArch64, so that generated output is
  checked bit for bit on three architectures.

### Git hooks

`just hooks` installs hooks defined in `.pre-commit-config.yaml`. On commit they run file hygiene
checks, `cargo fmt`, `cargo clippy`, `prettier`, `oxlint`, `tsc`, and the protocol-bindings check
(each only when relevant files changed); on push they run both test suites. `just pre-commit` runs
the commit-stage hooks against every file.

### Changing the protocol

Edit the types in `crates/hyperion-protocol`, run `just gen-protocol`, re-export any new type
from `packages/protocol/src/index.ts`, and commit the regenerated bindings. CI fails if they are
stale.
