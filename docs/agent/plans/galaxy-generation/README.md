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

## The vertical slice to the `SYSTEM` display (2026-09-23)

The owner asked for the `SYSTEM` display (plan 14, phase I) ahead of the roadmap's order, which puts
it six plans away. From round 7 on, lanes build a vertical slice: only what the first working
display needs, taken from plans 06, 11, 13 and 14 in dependency order (ruling 33 of 2026-09-22). Two
rules hold throughout. Each task is built as its plan specifies, not as a stub: the slice chooses
which tasks, not how thinly. And nothing deferred may be contradicted or need tearing out: where a
task reads something a deferred plan provides, it takes a plain argument, a named provisional
constant or a documented `None`, and its plan's task text says so in a _Slice:_ note. A later task
that fills such a seam bumps the version where it moves output.

**What the first display shows.** `SYSTEM` is the third tab (`F3`). `OPEN SYSTEM`, beside the
`GALAXY` local chart's readout, opens the selected system at the chart's time, held; with nothing
selected it reads `NO SYSTEM SELECTED`, and every other data state goes through plan 05's
`RequestStatus`, plus `NOT YET FORMED` and `NO BODIES`, with the last data kept and marked stale
when the link is lost. It shows every star of plan 11's hierarchy, the grid primary (0.08–150 M☉)
and its drawn companions, moving about their barycentres, each with its phase and kind, its MK class
without peculiar suffixes, its initial mass and mass now, L, R and T_eff, and for a remnant its
kind, mass and white-dwarf cooling age; what plan 06 does not model yet (variability, rotation,
activity, spins, kicks, binary class) is the guide's em dash. It shows every primordial planet of
every stable zone, S-type and P-type, in its state at the display time: not yet formed; present,
with adiabatic expansion and circularisation; destroyed by engulfment; or unbound after a supernova,
with a zero kick. The orbit map is drawn in the `SYSTEM BARYCENTRIC` frame on the `SYSTEM PLANE`,
with the galactic triad, orbits as solid `--text-muted` ellipses and the selected one in `--text`,
switchable annuli in `--text-muted` for the stable-zone limits, the snow line and the habitable
zone, `BODIES NOT TO SCALE`, a 1-2-5 scale bar in km, Mm, Gm and AU, and the presets `INNER`, `ALL`,
`TOP`, `SIDE`, `FRONT` and `OBLIQUE`. The display time reads `DISPLAY TIME UT +… yr ddd/hh:mm:ss`
and steps from 1 h to 100 yr, with `RESET` and `CLOCK WINDOW LIMIT`, re-requesting past a year or a
body's `valid_until`. The body list gives hosts, then planets by semi-major axis, with kind, a and
state in words, and the readout, from `body_detail`, gives designation, ID and label, kind, planet
class and state, mass in M⊕, radius in km, density and gravity, a, P, e, i and the distance from the
primary now, T_eq and bulk composition, and for the system its architecture class and zones. What is
absent is said, from the state the server tags on every section (ruling 34): `ok`, `not_resolved`
(`NOT RESOLVED`), `not_modelled` (`NOT YET MODELLED`) or `not_applicable` (no row, so a gas giant
shows no surface). `MOONS, RINGS, BELTS AND COMETARY HALO: NOT YET MODELLED` stands as a system note
composed from those tags, and the surface, atmosphere, habitability and resources sections read
`NOT YET MODELLED`. There is no events panel and no `RUN`/`HOLD`, and brown dwarfs and rogue planets
cannot be reached.

**The tasks, by plan.** A dependency that only a deferred part of a task needs is not a reason to
wait for it.

| Plan | Tasks in the slice                                                                                                                                                                                                                                                                                               |
| ---- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 06   | T3; T10.c–e (with the dispatch below 0.1 M☉ to T13); T13; T18.a–d; T20.a; T23.a–d (no T24/T25 extras); T29.a–b; T33, T34 and T35.b in part (`system_summary`, its DTOs and `unknown_system`; its handler and the `SystemStars` cache; `starSymbols.ts`, `ringed-circle`); T15.c's disc-lifetime law ahead of T15 |
| 11   | T3.a (with P14.T2.a); T1.a–b (T1.c beside them); T2.a–c; T3.b; T13 in part (`OrbitDto`, `HierarchyDto`, `body_index`)                                                                                                                                                                                            |
| 13   | T5.b; T5.c; T8.a (an owner's draft); T8.b (without `formatSubstellarMass`); T8.c's `triangle-down` outline                                                                                                                                                                                                       |
| 14   | T1.a–d; T2.a–c; T3–T9; T10.a; T11.a–d; T12; T15; T16.a–b; T28.a–c; T30.a–c; T32 in part (fifteen goldens); T34; T35 and T36 without `body_events`; T37–T41; T42.a–c; T43.a–b; T44.a                                                                                                                              |

**The relaxations, each with the task that removes it.**

| Plan | Relaxation                                                                                                                                                                                                                | Removed by                           |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ |
| 06   | T29.a's remnant stage reads only the `star.remnant.*` fields, and `natal_kick()` is `None`: no kicks, so P14.T28.c unbinds planets with a zero kick                                                                       | P06.T19 (bump)                       |
| 06   | White dwarfs shine by Hurley, Pols and Tout's own cooling law (their §6.2.1), under both recipes, for T10.d's hand-over and §6.3's perturbation; `Hurley2000` keeps it for good                                           | P06.T20.a under `Modern` (bump)      |
| 06   | T23.d classifies without T24's and T25's extras, so no peculiar suffixes                                                                                                                                                  | P06.T24, T25 (bump)                  |
| 06   | T10.d–e use the plan's interim forms for what T14, T15 and T16 add: `t_zams` = 0, a declared discontinuity at the post-AGB hand-over, the formulae as they stand above 100 M☉                                             | P06.T15.b, T16.a, T14 (bumps)        |
| 06   | The system count N stays on the provisional fates, with no real lifetimes in the mean mass                                                                                                                                | P06.T30.b (moves every star once)    |
| 11   | T2.b defines `ForcedMultiple` but only `Free` is used, since plan 09's `ClusterModel` is not built                                                                                                                        | P11.T8.f                             |
| 11   | T2.c draws the innermost orbit of a primary of 8 M☉ or more on two named provisional seams: `PROVISIONAL_STRIPPED_SHARE` = 0.25 against `StarDraws::stripped()`, and an interacting range of periastron under 10 au       | P11.T1.d and T4.a (bumps)            |
| 11   | The primary's draws are attempt 0, since plan 08's `mark_attempt()` does not exist                                                                                                                                        | P08.T12.c                            |
| 11   | No binary evolution: every pair is two single stars on an orbit, and plan 14's zones are the zones at birth                                                                                                               | P11.T4–T11                           |
| 11   | The companions the budget counts differ from those drawn by a few per cent                                                                                                                                                | P11.T1.d (moves every star)          |
| 13   | Nothing is placed, so `HostKind` is always `Stellar`, and brown dwarfs and rogue planets cannot be reached                                                                                                                | P13.T3, P14.T27                      |
| 13   | If T5.b needs a fit, it follows plan 02's `mge` convention, since plan 15's toolchain does not exist                                                                                                                      | P15.T2 (registers it)                |
| 14   | `SystemContext`'s sphere of influence is the galactic tidal radius at the epoch position, the encounter environment `None`, the strip radius 0.49 × that radius: no pericentre stripping within about 10 ly of the centre | P09.T28.c and P14.T29                |
| 14   | T3, T4 and T8 take host parameters and zone limits as plain arguments, not a context or an `OrbitZone`                                                                                                                    | P14.T30.a (adapts)                   |
| 14   | No [α/Fe] and no X-ray and ultraviolet history, and so no atmospheres, surfaces, habitability or resources: those sections read `NOT YET MODELLED`                                                                        | P14.T1.a with T13, T23–T26           |
| 14   | T16.a runs T11, T12 and T15 only, with a Bond albedo of 0.3                                                                                                                                                               | P14.T13, T14                         |
| 14   | `generate` equals `generate_planets`: no moons, rings, belts, halo or second-generation planets, so the system note says so                                                                                               | P14.T17–T22, T28.e                   |
| 14   | T28.a has no protoplanetary-disc body in belt slot `0xE0`                                                                                                                                                                 | P14.T28.a with T21 (bump)            |
| 14   | T32 pins fifteen of its twenty-four goldens, with no events                                                                                                                                                               | The tasks that make the rest (bumps) |
| 14   | `BeltDto`, `BodyEventDto` and `body_events` are deferred; `BodyKindDto` and `BodyStateDto` have every variant from the start                                                                                              | P14.T21, T31, T35–T36, T43.c         |
| 14   | The display opens held and steps; there is no `RUN` or `HOLD`                                                                                                                                                             | P14.T44.b                            |
| All  | The guide entries the display needs (P14.T38.a, P13.T8.a, P06.T35.a's units, the two phrases) are drafts for the owner, and the client is built to them                                                                   | The owner's edit of the guide        |

Two relaxations are knowingly inaccurate until their plans land, and the display must not hide them:
close binaries evolve as two single stars on an orbit (until P11.T4–T11), and a remnant unbinds its
planets with no kick (until P06.T19). Both fall short of the brainstorm for a while; neither changes
what the finished plans build.

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
