# Prerequisites and platforms

## Prerequisites

- Rust (toolchain pinned in `rust-toolchain.toml`; `rustup` installs it automatically)
- Node.js ≥ 26.0 (`engines.node` in `package.json`) and pnpm 12. The client's tests use
  `Float16Array` (Node 24) and the global `ErrorEvent` (Node 25), and vitest and jsdom accept no
  Node 25, so the floor is the 26 line. `pnpm test`, `pnpm typecheck` and `just smart-ci` stop at
  once on an older Node (`tools/node-floor/check.cjs`). Node 25 and later ship no Corepack: install
  pnpm on its own (`npm install --global pnpm@12`), and it runs the version `packageManager` names.
  The client itself runs on the Node and Chromium that its Electron bundles.
- [`just`](https://github.com/casey/just)
- [`uv`](https://docs.astral.sh/uv/) — runs [pre-commit](https://pre-commit.com) for the git hooks
- For the WebAssembly checks in `just ci`: the `wasm32-wasip1` and `wasm32-unknown-unknown`
  targets (listed in `rust-toolchain.toml`), [wasmtime](https://wasmtime.dev) at the version the
  justfile pins, `wasm-bindgen-cli` at the version of `wasm-bindgen` that `Cargo.lock` names, and
  [cargo-nextest](https://nexte.st). `just wasm-tools` installs them.

## Platforms

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
