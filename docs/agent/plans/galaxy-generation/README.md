# Galaxy Generation: Roadmap

Action plans for building what
[the galaxy generation brainstorm](../../brainstorming/galaxy-generation.md) designs. This file is
the index: what each plan covers, the order they run in, and the conventions they share. The
brainstorm is the specification. A plan says how to build a part of it, never what to build, and
where a plan and the brainstorm disagree the brainstorm wins until it is revised.

## Outcome

A working galaxy creation and exploration app:

- **Creation.** From the bridge client an operator creates a universe from a seed. The server
  generates its parameters and holds it as `(seed, generator_version)`.
- **Exploration.** The `GALAXY` display shows the galaxy map face-on and edge-on, the drawn
  parameters, and a rotatable 3D local chart of the systems around any chosen point and time. Each
  later milestone deepens what can be explored: stars, gas and dust, moving and displaced objects,
  clusters and the galactic centre, streams, binaries, events and alerts, substellar objects, and
  finally planetary systems with a `SYSTEM` display.

The app works at the end of every milestone. No plan leaves `just ci` failing.

## Plans

| Plan                                       | Title                                   | Milestone | Depends on         |
| ------------------------------------------ | --------------------------------------- | --------- | ------------------ |
| [01](01-determinism-foundation.md)         | Determinism foundation                  | M1        | none               |
| [02](02-galaxy-model.md)                   | Galaxy model: parameters to fields      | M1        | 01                 |
| [03](03-placement-and-range-query.md)      | Placement and the range query           | M1        | 01, 02             |
| [04](04-server-and-protocol.md)            | Server, universes and protocol          | M1        | 01, 02, 03         |
| [05](05-galaxy-display.md)                 | The `GALAXY` display                    | M1        | 04                 |
| [06](06-stellar-stage.md)                  | Stars: evolution, remnants and classes  | M2        | 03–05; 15 (table)  |
| [07](07-gas-and-dust.md)                   | Gas and dust field, extinction          | M2        | 02; display: 03–05 |
| [08](08-velocities-kicks-displaced.md)     | Velocities, kicks and displaced objects | M3        | 06, 07, 15         |
| [09](09-features-and-catalogue-classes.md) | Large features and catalogue classes    | M3        | 06, 07, 08, 15     |
| [10](10-global-list-streams.md)            | The global list: streams, dwarf cores   | M3        | 09                 |
| [11](11-multiplicity-and-binaries.md)      | Multiplicity and interacting binaries   | M4        | 06, 09, 15         |
| [12](12-retarded-observation-alerts.md)    | Retarded-time observation and alerts    | M4        | 09, 11             |
| [13](13-substellar-layers.md)              | Brown dwarfs and rogue planets          | M5        | 06                 |
| [14](14-planetary-systems.md)              | Planetary systems and the `SYSTEM` view | M5        | 06, 11, 13         |
| [15](15-offline-fitting.md)                | Offline fitting toolchain and tables    | M2 onward | per table, below   |

Plan 15 is not a stage but a supplier. Each of its tables is fitted against code that another plan
has already written, so the dependency runs per table and both ways: a consumer first commits a
provisional table in the final shape, plan 15 fits the production table against the consumer's code,
and one commit swaps it in with a bump of the generator version. The kick-law rank table (plan 06),
the displaced forms (plan 08), the cluster constants and Type Ia delays (plan 09) and the binary
class samplers (plan 11) all work this way. The first milestone needs one fit, the Gaussian-sum
coefficients of the potential, which plan 02 makes itself when it creates the `hyperion-fit` crate.

Plans 01–05 are the first milestone, which the brainstorm's Decisions fix: galactic structure and a
visualiser. It is the only milestone whose plans are expected to be executed exactly as written.
Later plans are as detailed as the brainstorm allows and are re-validated when their turn comes,
because the code they build on will exist by then.

### Milestones

| Milestone | What the app gains                                                                                                |
| --------- | ----------------------------------------------------------------------------------------------------------------- |
| M1        | Create a galaxy from a seed; galaxy map; local chart with mass, age and population; queries take a time.          |
| M2        | Every system's stars have a state, class and events; charts show living stars and remnants; dust lanes on the map |
| M3        | Systems move; kicked remnants and runaways; clusters, nebulae, supernova remnants, the galactic centre; streams   |
| M4        | Multiple stars and interacting binaries; sensors see the past; alerts for novae, supernovae and the rest          |
| M5        | Brown dwarfs and rogue planets; planetary systems, moons, belts and rings; the `SYSTEM` display                   |

## Conventions every plan follows

### Plan layout

Each plan has these sections in this order:

1. **Header**: milestone, depends on, brainstorm sections covered (by heading).
2. **Goal**: what exists when the plan is done, in a paragraph.
3. **Scope and non-goals.**
4. **Provides**: the public Rust or TypeScript interfaces, protocol messages and test helpers that
   other plans consume. Signatures are sketches, named precisely enough to be grepped.
5. **Consumes**: what it needs from other plans, by plan number and name.
6. **Design notes**: decisions the plan makes that the brainstorm leaves open, each with its reason.
   Nothing here may contradict the brainstorm.
7. **Tasks.** Each task has an ID (`P03.T4`), a title, what to build, the files it touches, its
   tests, and acceptance criteria that can be checked by running something. A task is at most about
   a day of focused work. Anything larger is split into subtasks (`P03.T4.a`), each with its own
   acceptance criteria. Tasks are ordered so that each one compiles and passes `just ci` on its own.
8. **Verification**: how the plan as a whole is shown to be done: statistical tests, benchmarks,
   checks by eye.
9. **Generator version**: whether the plan changes generated output, and what it reserves so that
   later plans need not.
10. **Risks and open points.**

### Code shape

- The sim stays in `hyperion-sim`, as modules: `rng`, `math`, `units`, `time`, `coords`, `id`,
  `galaxy`, `stellar`, `planetary`, and others as plans add them. It gains exactly one runtime
  dependency, `libm`, pinned with `=`. It does no I/O, spawns no threads and holds no caches of its
  own: the caller owns caches.
- Every transcendental function goes through `hyperion_sim::math`, enforced by Clippy's
  `disallowed_methods`.
- `hyperion-protocol` holds wire types only. After changing it, `just gen-protocol`.
- `hyperion-server` owns universes, caches, the CPU pool and persistence.
- Offline fitting lives in a separate workspace crate, `hyperion-fit`, which is never a dependency
  of the sim or the server. Plan 02 creates it with the one fit the first milestone needs, and plan
  15 extends it. Its output is Rust source for constant tables, committed under
  `crates/hyperion-sim/src/tables/` with a header naming the tool, its inputs and its version.
- The frontend follows `docs/frontend/ux-guidelines.md`. Edits to the guide are tasks in the plan
  that needs them.
- `.claude/rules/rust-dev.md` and `.claude/rules/typescript-dev.md` bind every task.

### Generator version

`GENERATOR_VERSION` is a constant in `hyperion-sim`. Any task that changes generated output bumps it
and regenerates the golden files in the same commit. Before the first release bumps are free, but
each plan still lists what it reserves (ID prefixes, streams, arguments) so that the next plan moves
no star it need not.

### Tests

- Golden files live in `crates/hyperion-sim/tests/golden/`. Plan 01 provides the harness and the
  statistical helpers (chi-square, Kolmogorov–Smirnov, Poisson interval checks), hand-written, with
  fixed seeds, in a dev-only workspace crate, `hyperion-testkit`.
- Random streams are opened with `Stream::open(Seed, DomainTag, ObjectKey)`. Every domain tag is an
  entry in plan 01's single registry, `rng/tags.rs`, and each plan's tasks add their own entries
  there, so the collision test covers them all.
- Slow statistical tests and Milky Way comparisons run under `just test-slow`, which `just ci-slow`
  runs and `just ci` does not. Plan 01 defines how they are marked.
- Benchmarks use Criterion as a dev-dependency and run under `just bench`. The brainstorm's targets
  are recorded in the plan that owns the code, and a regression is a finding, not a failure.

### Figures

Numbers in the brainstorm are rounded. A task that turns one into code re-checks it against the
source the brainstorm cites and records the citation in the doc comment.
