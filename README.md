# HYPERION

A spaceship bridge simulation game: players crew the stations of a ship as it navigates an
actual-size, procedurally generated galaxy. It is a mixture of Artemis's bridge crew and No Man's
Sky's universe.

- **Everything is procedurally generated**, with realistic parameters: today the galaxy, its stars
  and their planets; by design, alien races, factions, languages and civilisations' histories too.
- **Realism throughout.** Planets' sizes, atmospheres and orbits follow astrophysical
  expectation, and each station's console aims at a flight simulator's attention to detail.
- **Local language models**, such as Gemma 4, are to write detailed descriptions where needed.
- **Rust and TypeScript.** The game server (`hyperion-server`) is written in Rust; the bridge
  client (`hyperion`) is an Electron and React application in TypeScript.

One 64-bit seed produces a full-size barred spiral galaxy, about 100 billion star systems across
roughly 100,000 ly, generated on demand: the same seed always gives the same galaxy.
[Galaxy generation](docs/galaxy-generation.md) lists what it holds.

## Prerequisites

- Rust (toolchain pinned in `rust-toolchain.toml`; `rustup` installs it automatically)
- Node.js ≥ 26.0 and pnpm 12 (`npm install --global pnpm@12`)
- [`just`](https://github.com/casey/just)
- [`uv`](https://docs.astral.sh/uv/) — runs [pre-commit](https://pre-commit.com) for the git hooks
- The WebAssembly targets and tools: the client's build needs `wasm-bindgen`, and the checks the
  rest. `just wasm-tools` builds and installs them, which takes a while.

Why the Node floor is 26, the WebAssembly checks' tools, and what macOS and Windows need are in
[Prerequisites and platforms](docs/development/platforms.md).

## Quick start

```sh
just install      # pnpm install
just wasm-tools   # install the WebAssembly targets and tools (once per machine)
just hooks        # install the git pre-commit and pre-push hooks (once per clone)
just server       # run the game server on 127.0.0.1:7878 (`just server --help` for options)
just client       # run the Electron client with hot reload (`just client --help` for options)
```

Options to `just client` reach the client: `just client --address 10.0.0.5 --port 9100`.
`just server` and `just client` build what they run; `just build` builds the release artifacts.

The ship starts in no system, so `VIEW` first shows its test scene. To see a generated system, run
`just place-ship` once the server is up
([Running](docs/development/running.md#seeing-a-generated-system-in-view)).

A change's gate is `just smart-ci`, the part of `just ci` that it needs
([Checks](docs/development/checks.md)).

## Layout

| Path                       | What                                                                  |
| -------------------------- | --------------------------------------------------------------------- |
| `crates/hyperion-server`   | Game server binary (axum, WebSocket at `/ws`)                         |
| `crates/hyperion-sim`      | Deterministic simulation / procgen core — no I/O                      |
| `crates/hyperion-base`     | Beneath the sim: maths on the pinned `libm`, units, generator version |
| `crates/hyperion-surface`  | The shared height function and terrain, native and WebAssembly        |
| `crates/hyperion-protocol` | Wire protocol types; source of truth for the TypeScript bindings      |
| `crates/hyperion-fit`      | Offline fitting: emits the constant tables the generator reads        |
| `crates/hyperion-testkit`  | Test helpers: golden files, order independence, statistical tests     |
| `apps/hyperion`            | Bridge client (Electron + React, built with electron-vite)            |
| `packages/protocol`        | `@hyperion/protocol` — TS bindings generated from the protocol crate  |
| `tools/`                   | Development tools: `smart-ci`, `repin`, `gpu-replay`, the Node floor  |
| `docs/`                    | Design docs, agent plans, UX guidelines                               |

## Documentation

| Where                                                              | What                                                                     |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| [`docs/galaxy-generation.md`](docs/galaxy-generation.md)           | What the generated galaxy holds, in brief                                |
| [`docs/development/platforms.md`](docs/development/platforms.md)   | The prerequisites in full; Linux, macOS and Windows                      |
| [`docs/development/running.md`](docs/development/running.md)       | Seeing a system in `VIEW`; the server's and the client's options         |
| [`docs/development/checks.md`](docs/development/checks.md)         | `just ci`, `just smart-ci`, the git hooks, the protocol, the digest pins |
| [`docs/frontend/ux-guidelines.md`](docs/frontend/ux-guidelines.md) | The bridge consoles' visual and UX guidelines                            |
| [`docs/agent/brainstorming/`](docs/agent/brainstorming/)           | The design: galaxy, rendering and planets, life, the single-player game  |
| [`docs/agent/plans/`](docs/agent/plans/)                           | The action plans, each set with a roadmap `README.md`                    |
| [`docs/agent/handoff/README.md`](docs/agent/handoff/README.md)     | Where development stands and what comes next                             |
| [`docs/measurements/`](docs/measurements/)                         | Recorded runs of the by-hand and timed checks                            |
| [`NOTICE`](NOTICE)                                                 | Third-party software and data, and their licences                        |
