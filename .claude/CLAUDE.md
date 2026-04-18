# HYPERION Spaceship Bridge Simulation Game

HYPERION is an incredibly detailed, modular, and customizable spaceship bridge simulation game. Players operate different positions on a spaceship (like in Star Trek) that they design and upgrade, and are dropped into a procedurally-generated galaxy with generated alien races, factions, languages, and history. Think Dwarf Fortress meets No Man's Sky meets Artimis.

## Relevant Documentation

Documentation can be found in the `docs/` directory. In particular:

* `architecture.md` - Provides a high-level overview of the game's architecture.
* `game-design.md` - Provides information about how the game is intended to be played.
* `modules.md` - Provides an in-depth guide into the game's ship module system.
* `ship-positions.md` - Detailed information about the role and capabilities of each ship position.
* `plans/` contains detailed action plans for implementing new features.
* `brainstorming/` contains brainstorming documents for new features/ideas.

## Common Commands

Development tasks are wrapped in a `justfile` at the repo root. Prefer these over raw `cargo` invocations so the workflow stays consistent. Run `just` with no arguments to see all recipes.

* `just build` / `just build-release` - Compile the project.
* `just run -- <args>` - Run the binary, forwarding args after `--`.
* `just test` - Run unit, integration, and doc tests.
* `just format` / `just format-check` - Apply or verify `rustfmt`.
* `just lint` - Run `clippy` with `-D warnings` (matches the rule in `.claude/rules/development.md`).
* `just typecheck` - Fast `cargo check` across all targets.
* `just check` - Full CI gate: format-check, lint, typecheck, and tests.
