# Plan 15: Offline fitting toolchain and tables

- **Milestone:** cross-cutting, M2 onward. This plan starts after M1. One table cannot wait for it:
  the Gaussian-sum (MGE) coefficients are read by plan 02's potential in M1, so plan 02's task
  P02.T6.a creates the `hyperion-fit` crate with that one fit. See
  [The first cut, in M1](#the-first-cut-in-m1).
- **Depends on:** plan 01 (`math`, `rng`, golden harness, `just test-slow`) and plan 02
  (`potential`, `fields`, `GalaxyParams::milky_way_like`, and the `hyperion-fit` crate with its
  first-cut MGE task) for the toolchain; then per table as the [Schedule](#schedule) says: 06 for
  the kick ranks and helium, 09 to 11 for the tables that are calibrated against their code.
- **Brainstorm sections covered:** "Open questions" (the whole list of offline fits); the offline
  fitting rule under the roadmap's "Code shape"; and the passages that say what each fit must
  produce: "Galaxy parameters" (the potential as a sum of Gaussians), "Sizing the layers"
  (Chabrier's high-mass branch), "Displaced objects: kicks and runaways" (the form table, the
  own-form shares, the rank table and its defaults), "Supernova remnants: one route, not two" (Type
  Ia yield and explosion mark), "Events in time" (conditional samplers of the catalogue classes),
  "What is inside a cluster today" (black-hole loss, equipartition, pulsars against encounter rate),
  "Streams and accreted structure" (orphan streams per globular), "Covering every class of star"
  (the helium row), and "Testing" (the kick-law tests that pin the four defaults).

## Goal

The workspace crate `hyperion-fit`, which plan 02 created with one task and this plan extends into a
toolchain that turns named sources (catalogues, published relations, and the generator's own code
run at scale) into constant tables committed as Rust source under `crates/hyperion-sim/src/tables/`.
Each table carries a header that says which tool, inputs and revision produced it, is tied to
`GENERATOR_VERSION`, and is checked in CI for staleness without rerunning any fit that takes more
than a minute. When the plan is done every constant the brainstorm's "Open questions" lists has a
task group with its source, format, consumer and acceptance test, and every scratch value that an
earlier plan shipped has been replaced.

### The first cut, in M1

The roadmap places this plan in "M2 onward" and makes it depend on plans 01 and 02, not the other
way round, and the MGE coefficients are still needed in M1. The roadmap's answer, which plan 02
carries out, is that **plan 02's task P02.T6.a creates `crates/hyperion-fit`** in its minimal form:
`Cargo.toml` with `hyperion-sim` as its only dependency, a thin `src/main.rs` that parses
`run mge [--out <path>]` and maps `RunFitError` to an exit code, `src/lib.rs`, and
`src/tasks/mge.rs` with `fit() -> MgeTables` and `render(&MgeTables) -> String`. That task fits
e^(−s) and the long bar's azimuthally averaged profile with 12 to 16 fixed logarithmic widths and
non-negative weights, takes every transcendental from `hyperion_sim::math`, and commits its output
as `crates/hyperion-sim/src/tables/mge.rs` (`MGE_EXP`, `MGE_BAR`, each `[(weight, width); N]`) under
a header naming the tool, its inputs and its version 0. Its test `mge_table_is_reproduced` compares
a fresh render with the committed file byte for byte.

This plan creates nothing that exists. It keeps what plan 02's R8 asks it to keep: the crate, the
command `run mge` with `--out`, the table's path, the item names `MGE_EXP` and `MGE_BAR` and their
shape, and the reproduction test. P15.T1 and P15.T2 grow the crate around the existing task without
changing a byte of the table's body (P15.T2 rewrites its header comment to the common grammar).
P15.T3 then refits with free widths, adds the two-dimensional expansion of the boxy bulge that plan
02's D6 says this plan supplies (`MGE_BOXY`), and replaces the table with a version bump, which plan
02 lists among its known future bumps.

## Scope and non-goals

In scope:

- The `hyperion-fit` crate: command line, deterministic parallel map-reduce, optimisers, the Rust
  source emitter, manifests, hashing, the lock file and the staleness check.
- The `hyperion_sim::tables` registry: the header convention, `TableInfo`, and the rule that links a
  table to `GENERATOR_VERSION`.
- Where external data lives, how it is pinned and how its licence is recorded.
- One task group per fit in the brainstorm's list, in the order the consumers need them.
- `just fit`, `just fit-check`, and the CI step.

Not in scope:

- Using the tables. Each consumer reads its table and owns the tests of the behaviour that results.
  This plan owns the acceptance test of the fit itself.
- The shape of a table whose consumer has already defined it: `tables::mge` (plan 02),
  `tables::kick_rank` and `tables::helium` (plan 06), `tables::binary` (plan 11). This plan fills
  them and takes over the files. It defines the shapes of the rest.
- Hand-entered constants that are not fits, such as plan 02's `tables::gauss_legendre`. They carry
  an `@static` header and the staleness check only verifies that nobody marks them generated.
- Any code inside `hyperion-sim` other than `tables/`, the `fit.*` entries of its `rng/tags.rs`,
  and the one-line repointing of a consumer's constant at its table. Where a fit needs the
  generator's own code (potential, tracks, kick law, binary formulae, stream spray) it calls the
  owning plan's public API and never copies it.
- New physics. A fit chooses constants for forms the brainstorm already fixes.

## Provides

### The crate `hyperion-fit` (`crates/hyperion-fit`)

Created by P02.T6.a and extended here: a binary with a thin `main.rs` over a library, as
`rust-dev.md` requires. It depends on `hyperion-sim`, with the sim's `testing` feature, and on
`hyperion-surface` (R09.T7.f's `TableCrate`), and reaches `hyperion-base` through the sim's
re-exports (re-validated). Nothing may depend on it: `no_runtime_crate_depends_on_hyperion_fit`
(`tests/toolchain.rs`) fails if `hyperion-base`, `hyperion-sim`, `hyperion-surface`,
`hyperion-protocol` or `hyperion-server` names it in `Cargo.toml`.

```text
hyperion-fit list                      # tasks, class (fast or slow), table path, state
hyperion-fit run <task> [--since <generator version>] [--smoke] [--threads N] [--out PATH]
                        [--data DIR] [--manifest PATH]
hyperion-fit check [--rerun-fast] [--threads N]   # staleness; see Design note 8
hyperion-fit fingerprint <task>        # prints the sim probe values a task depends on
hyperion-fit orbits [--smoke | --manifest PATH] [--threads N] [--out DIR] [--max-parts N]
```

`run mge --out <path>` keeps working as plan 02 defined it. `--since` is required whenever the
output goes into its crate's `tables/` (the default) and is not needed with `--out` elsewhere.
`--smoke` and `--manifest` need `--out`: a committed table is always made from
`manifests/<task>.toml` (P15.T1 as built). `orbits` is P15.T6.b's (re-validated at 898a3c94;
`cli.rs`'s `Command`, which also holds rendering plan R08's `atmosphere-reference`).

```rust
hyperion_fit::task::{FitTask, TaskClass, TaskOutput, RunTaskError, TableCrate, REGISTRY,
                     registry, find}
hyperion_fit::manifest::{Manifest, InputsHash, LockFile, LockEntry, SimFingerprint}
hyperion_fit::emit::{RustTable, TableItem, Header, HeaderKind, Workspace, Destination,
                     write_table, EmitTableError}
hyperion_fit::parallel::map_reduce_chunks                      // deterministic reduction
hyperion_fit::optimise::{golden_section, nelder_mead}          // P15.T8.a's
hyperion_fit::optimise::{nnls, least_squares, levenberg_marquardt, LeastSquaresFit}
    // P15.T3.a's, as built (T3.b added `upper` and `least_squares`):
    // nnls<R: AsRef<[f64]>>(rows: &[R], rhs: &[f64]) -> Vec<f64>
    // least_squares<R: AsRef<[f64]>>(rows: &[R], rhs: &[f64]) -> Option<Vec<f64>>
    // levenberg_marquardt(model: impl FnMut(&[f64], &mut [f64], Option<&mut [f64]>),
    //     start: &[f64], lower: &[f64], upper: &[f64], residuals: usize, iterations: u32)
    //     -> LeastSquaresFit
    // LeastSquaresFit { params: Vec<f64>, sum_of_squares: f64, steps: u32 }
hyperion_fit::data::{Dataset, Provenance, load_dataset, LoadDatasetError}
hyperion_fit::check::{check, CheckReport}
hyperion_fit::pipeline::{prepare, rerender}
hyperion_fit::cli::{Cli, run}

pub trait FitTask: Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;                 // "mge", "displaced_forms", ...
    fn class(&self) -> TaskClass;                   // Fast (under 60 s; CI reruns it) or Slow
    fn table_crate(&self) -> TableCrate { TableCrate::Sim }
    fn table_path(&self) -> &'static str;           // relative to its crate's src/tables/
    fn revision(&self) -> u32;                      // bumped by hand when the algorithm changes
    fn items(&self) -> &'static [&'static str];     // the table's items, for the smoke check
    fn fingerprint(&self) -> SimFingerprint;        // empty for tasks that use only `math`
    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError>;
}
```

As built at 898a3c94 (re-validated): `write_table(emission, destination) -> Result<Written,
EmitTableError>`, its `Destination` either the workspace's tables or `--out`. The sketch's
`hyperion_fit::stats::{total_variation, rms_log_residual, ks_distance}` is not built: the displaced
task keeps its own total-variation distance (`tasks::displaced_forms::cells::total_variation`) and
a bounded Nelder–Mead (`displaced_forms::nelder_mead`), and a task's tests that need a
Kolmogorov–Smirnov test take it from `hyperion-testkit`'s `stats` (a dev-dependency). `tasks::mge`
has a private `nnls`, by projected coordinate descent.

### `hyperion_sim::tables`

```rust
pub struct TableInfo {
    pub name: &'static str,
    pub revision: u32,                  // the table's own revision, from its header
    pub since_generator_version: u32,   // GENERATOR_VERSION at which this revision took effect
    pub provisional: bool,              // true while a scratch stand-in is in place
}
pub const MANIFEST: &[TableInfo];       // fitted tables only, in name order
```

`TableInfo` is `hyperion_base::version::TableInfo` since R09.T7.f, re-exported here. A crate
beneath the sim lists its own fitted tables in its own `tables::MANIFEST` of the same type: the
surface crate does, for `cold_trap`'s `bowl_permanent_shadow`. `hyperion-fit` compares every such
`MANIFEST` with `tables.lock`, through `TableCrate` (decision-r09-t7f.md): `task::TableCrate`
(`Sim`, `Sky` reserved for the sky crate's split, `Surface`), and `check::REPOSITORY_MANIFESTS`,
which lists the sim's and the surface crate's (re-validated).

| Module                          | Shape owned by | Consumer    | Contents                                                  |
| ------------------------------- | -------------- | ----------- | --------------------------------------------------------- |
| `tables::mge`                   | plan 02        | 02, 08      | `MGE_EXP`, `MGE_BAR`; this plan adds `MGE_BOXY`           |
| `tables::chabrier`              | this plan      | 02, 11      | `HIGH_MASS_BRANCH_SCALE`                                  |
| `tables::kick_rank`             | plan 06        | 06 (08, 09) | `SCORE_QUANTILES: [f64; 257]`, four defaults              |
| `tables::helium`                | plan 06        | 06, 09      | `LIFETIME_SLOPE`, `HB_TEMPERATURE_SHIFT`                  |
| `tables::displaced_forms`       | this plan      | 08          | see P15.T6                                                |
| `tables::cluster_dynamics`      | this plan      | 09          | `BH_LOSS_*`, equipartition, `PULSAR*`                     |
| `tables::type_ia_delay`         | this plan      | 09, 11      | delay-first samplers, `DELAY_EDGES`                       |
| `tables::lbv`                   | this plan      | 09          | `LBV_SAMPLER`, the luminous blue variables'               |
| `tables::binary`                | plan 11        | 11          | `IA_YIELD`, four samplers, `CLASS_SHARES`                 |
| `tables::streams`               | this plan      | 10          | `ORPHAN_STREAMS_PER_GLOBULAR`                             |
| `tables::saturation`            | plan 14        | 14          | P14.T49.b's saturation curves                             |
| `tables::substance_albedo`      | plan 14        | 14, R09     | P14.T49.c's per-band albedos at grain classes             |
| `tables::thermo`                | plan 14        | 14          | P14.T49.d's log K_f(T)                                    |
| `tables::condensate_extinction` | plan 14        | 14          | P14.T49.e's κ_ext and ω at 550 nm, from R08's index files |

Phase L of plan 14 (decision-composition, 2026-10-09) adds the four fit tasks above
(`saturation_curves`, `substance_albedos`, `thermo_equilibrium`, `condensate_extinction`) as
subtasks of its own, with this plan's emitter, manifest and staleness check. `condensate_extinction`
reads committed datasets outside the crate (rendering plan R08's `materials/` files), whose hashes
its manifest records, so a changed index file makes it stale.

The owner of a shape states it in its own Provides, and this plan's task group restates what the fit
must fill. P15.T2 checks each against the code as built, and the owner's code wins.

**Built at 898a3c94** (re-validated). The module table's rows as built: `tables::mge` also holds
`MGE_HOLED_EXP` (P02.T12.b), `tables::helium` also `LOG_Z_NODES`, and of `tables::binary`'s items
only `CLASS_SHARES` exists. Registered: `mge` (provisional, P02.T6.a's first cut),
`chabrier` (fitted, P15.T4.b), `kick_rank` (provisional, P06.T19.b's), `displaced_forms`
(provisional, on the smoke histograms), `cluster_dynamics` (three provisional blocks, P15.T8.a–c)
and `type_ia_delay` (fitted, P15.T9.a). Hand-made, with no `MANIFEST` row or lock entry: `helium`
(P06.T17's identity), `binary` (P11.T7's `ClassShareTable`, `CLASS_SHARES` and
`MILKY_WAY_FORMED_MASS`) and `streams` (P10.T3.b's 1.5), which P15.T7.b, P15.T9.b with T10.b, and
P15.T11.a register. Not built: `MGE_BOXY` (P15.T3.b), `tables::lbv` (P09.T43's scratch, then
P15.T10.a), and `IA_YIELD` and the samplers, whose shapes are P11.T6's and T8's. `task::REGISTRY`
holds 47 tasks in name order. Beside this plan's eight (`chabrier`, `cluster_bh`,
`displaced_forms`, `equipartition`, `kick_rank`, `mge`, `pulsars`, `type_ia_delay`) and Phase L's
(`condensate_extinction` and `_2` to `_7`, one task in seven parts, `saturation_curves`,
`substance_albedos`, `thermo_equilibrium`), it runs other plans' fits, whose tables and shapes
their plans own: plan 06's `stellar_fates_high`, `_low` and `_mid` and `wd_cooling`; plan 08's
`lifetime_bracket`; plan 09's `cluster_retention`; plan 11's `binary_cooling_floor`,
`binary_pair_light_c` to `_e`, `binary_reach`, `period_correction` and `stripping`; plan 13's
`giant_cooling`; plan 14's `liquid_optics`; R06's `limb_darkening`, `sky_binary_light_c` to `_e`,
`sky_envelope`, `sky_phase_envelope`, `star_colour` and its five `star_colour_reddening` tables;
and the surface crate's `bowl_permanent_shadow` (R09) and `space_weathering` (R10).
_Since P15.T11.a (2026-10-23; Risks, "T11.a as built"), `streams` is registered, provisional, with
its `MANIFEST` row and lock entry, and `task::REGISTRY` holds 48 tasks, this plan's nine with
`streams`._

### Recipes and CI

- `just fit <task>`: `cargo run --release -p hyperion-fit -- run <task> --since <current>`, writing
  into the sim's `tables/` and updating `crates/hyperion-fit/tables.lock`. As built (re-validated),
  `just fit <task> *args`, with `<current>` the justfile's `generator_version`, read from
  `crates/hyperion-base/src/version.rs`; a task whose `table_crate()` is `Surface` writes into that
  crate's `tables/`.
- `just fit-check`: `cargo run -p hyperion-fit -- check`. Part of `just ci`. It reruns no fit. Its
  cost is the fingerprints' probes, a few seconds at most, since the displaced table's probes build
  plan 02's full potential once. As built, `just ci` runs it as `_fit-check-built`, the debug binary
  that `_test-build` made.
- `just test-slow` additionally runs `hyperion-fit check --rerun-fast` (as built, the justfile's
  `slow_then_check`, after the slow tests and timed). `just ci-slow` runs `test-slow`.

## Consumes

- **Plan 01:** `hyperion_sim::math` (every transcendental in a fit goes through it, so a table is
  bit-identical on every platform); `rng::{Seed, Stream, ObjectKey, tags}` with
  `Stream::open(seed, tag, key)`. Since R04.T4.d, `math`, `rng`, `units` and `version` live in
  `hyperion-base` and `hyperion_sim` re-exports them, so these paths hold (re-validated). The fit's
  Monte Carlo draws use the sim's generator on tags named `fit.<task>.<purpose>`, which each task
  group adds to the sim's registry, `crates/hyperion-sim/src/rng/tags.rs`, one of three since
  R04.T4.d (base's holds `selftest.stream` alone and the surface crate's the `surface.*` tags). They
  go under a "Plan 15" heading appended at the end of its `domain_tags!` list, since the macro's
  order fixes `tags::ALL`, which the golden `rng/tags` pins (re-validated), with scope `Galaxy` and
  the key `ObjectKey::galaxy_item(chunk index)`, so the collision test covers them: the sim's
  `assert_registries_disjoint` over all three registries. No `fit.` tag exists yet (re-validated):
  the built tasks draw through their consumers' functions (`score_quantiles` opens plan 06's
  `stellar.reference`), and `displaced_forms`' births through a counter-based SplitMix64 of their
  own (`tasks::displaced_forms::births::Draws`), which opens no tag. Also `GENERATOR_VERSION` (a
  `GeneratorVersion`, read with `.get()`); from `hyperion-testkit`, `golden!`, `stats` and
  `float::assert_same_bits`; the slow-test marking (`#[ignore = "slow: …"]`) and `just test-slow`.
- **Plan 02:** the crate `crates/hyperion-fit` as P02.T6.a leaves it (`run mge [--out]`,
  `RunFitError`, `tasks::mge::{fit, render, MgeTables}`, the test `mge_table_is_reproduced`), as
  P15.T1–T2 grew it: `render` returns a `RustTable`, `MgeTables` has a third field, `holed_exp`,
  and the test compares `pipeline::rerender` with the committed file through `tests/common`'s
  `assert_reproduced` (re-validated). Also `tables/mge.rs` with `MGE_EXP` and `MGE_BAR`, and since
  P02.T12.b `MGE_HOLED_EXP`, the holed thin disc's, with signed weights on `MGE_EXP`'s widths,
  which the nuclear disc's inner part reuses since R26; each is `[(f64, f64); 14]` (re-validated).
  `GalaxyParams::milky_way_like` and `Galaxy::from_params`, which takes the seed and returns a
  `Result`: `from_params(seed, params) -> Result<Galaxy, BuildGalaxyError>` (re-validated). From
  `potential`, `MassModel` (`v_circ_sq`, `potential`, `vertical_force`, `enclosed_mass`) and
  `PotentialTables` (`v_circ`, `omega`, `kappa`, `potential`, `escape_speed`, `bar_pattern_speed`,
  `bar_corotation`); as built, `MassModel::enclosed_mass` holds the spherical components only and
  `expanded_enclosed_mass` adds the Gaussians (P02's R15), and both types have `force`, returning
  `CylindricalForce` (ruling 101.4; the tables' far field conservative since P02.T6.f), beside
  `PotentialTables::{v_circ_sq, escape_speed_in_plane}` (re-validated). `potential::mge`'s
  Gaussian builders, and the moment-matched bulge, the crate-private `bulge_spheroid`, which T3.c
  replaces. `fields` (component densities) and `ages::AgeDistribution`;
  `imf::{MassFunction, Kroupa, Chabrier}`, whose provisional scale of 0.68 P15.T4 replaces
  (replaced: P15.T4.b's 0.920 in `tables::chabrier`, re-validated), and `fates::mean_present_mass`,
  with `imf::Truncated`, `fates::CensusFates` and `GalaxyParams::census_system_masses_under`, which
  P15.T4.b added.
- **Plan 06:**
  `stellar::remnant::reference::{ReferencePopulation, score_quantiles, kick_observables}` (P15.T5
  calls the same `score_quantiles(pop, n, seed)` that made the provisional table, and the six
  kick-law tests of P06.T19.d through `kick_observables`; as built, `ReferencePopulation::scores`
  and `quantiles_of` too, and `KickObservables` feeds seven tests, since ruling 96.3 added the
  escape share: re-validated), `KickLawParams` (with `max_speed_km_s`, ruling 96.2),
  `StandardKickLaw`, `KickRankTable`, `stellar::sse` tracks (`Track::full`), `stellar::lifetime`
  (a function, `stellar::sse::lifetime`, re-exported: re-validated), `StarDraws::median`,
  `SystemStars::lbv_window`, the shapes `tables::kick_rank::SCORE_QUANTILES` and
  `tables::helium::{LIFETIME_SLOPE, HB_TEMPERATURE_SHIFT}`, and the provisional and identity files
  P06.T19.b and P06.T17 commit, whose ownership passes to this plan. As built (re-validated):
  `tables::kick_rank` also holds the law's defaults, `LOW_RAMP`, `BH_FACTOR`, `EC_WINDOW_SINGLE`
  and `EC_WINDOW_STRIPPED`, which `KickLawParams::default` reads, while the track reads its own
  copies of the windows, `stellar::remnant::collapse`'s `SINGLE_STAR_WINDOW` and
  `COMPANION_STRIPPED_WINDOW`; `tables::helium` also holds `LOG_Z_NODES`, and the track reads it
  through `stellar/sse/track/excess.rs`'s crate-private `HeliumTable` and `HeliumHook`.
- **Plan 08:** nothing. Plan 08 consumes `tables::displaced_forms`. The orbit integrator here is the
  fit's own and reads only plan 02's potential.
- **Plan 09:** the cluster class code and its scratch constants (black-hole loss, equipartition,
  pulsar counts), its scratch Type Ia samplers, which P15.T8 and P15.T9.a replace. As built
  (re-validated): P15.T8.a moved the constants, unchanged, into `tables::cluster_dynamics`'s three
  provisional blocks, which `features::cluster` and `features::interior::profile` read, and
  P15.T9.a replaced the samplers with the fitted `tables::type_ia_delay`, which
  `catalogue_classes::type_ia` reads. P15.T5.c reads P09.T9.b's neutron-star retention,
  `features::interior::retention` (`retention`, and `retention_tabulated` over plan 09's fitted
  `tables::cluster_retention`, ruling 139.5). P15.T10.a's consumer is P09.T43's `LbvProcess`, not
  built. The Baumgardt–Hilker copy, `features::testing`, is behind the sim's `testing` feature.
- **Plan 10:** the stream generator, for P15.T11; its orphan multiplier. As built (re-validated):
  the multiplier is `tables::streams::ORPHAN_STREAMS_PER_GLOBULAR` (P10.T3.b, hand-made,
  provisional 1.5), which `global_list::orphans` reads; the generator is
  `global_list::Debris::generate`, which makes stream specifications, and the spray and the tubes
  that P15.T11.b observes (`global_list::spray`, `TubeTable::build`) are P10.T5–T6's, not built:
  `TubeTable` is a placeholder.
- **Plan 11:** the shapes `IaYieldTable`, `ClassSamplerTable`, `ClassShareTable` and `IaPoolChannel`
  and the scratch contents at `tables/binary.rs` (`IA_YIELD`, `AWD_SAMPLER`, `XRB_SAMPLER`,
  `MERGER_SAMPLER`, `NSM_SAMPLER`, `CLASS_SHARES`);
  `stellar::binary::{MergedBinaryFate, CLUSTER_MERGED_BINARY_FATE}`, the default P15.T5.c reviews;
  the binary engine run forward; `multiplicity::stripped_share` and the all-stars mass-function
  quadrature that P15.T4 fits against; `stellar::binary::testing::brute_force_class_members`, which
  plan 11 exports behind `hyperion-sim`'s `testing` feature because a file under the sim's
  `tests/common/` cannot be reached from another crate. `hyperion-fit` enables that feature in its
  own dependency on the sim. As built (re-validated):
  - `tables/binary.rs` holds only `ClassShareTable`, `ClassShareRow`, the scratch `CLASS_SHARES`
    and `MILKY_WAY_FORMED_MASS` (P11.T7). `IaYieldTable`, `IA_YIELD`, `ClassSamplerTable` and the
    four samplers are P11.T6's and T8's, not built.
  - `IaPoolChannel` is `stellar::binary::IaPoolChannel` (`timeline.rs`, P11.T4), beside the
    engine's pooled candidates, `PooledIaEvent`. The engine is
    `stellar::binary::evolve(&BinaryInput, until_age) -> BinaryTimeline`.
  - `MergedBinaryFate` and `CLUSTER_MERGED_BINARY_FATE` exist, and nothing reads them.
  - `multiplicity::stripped_share` takes a fourth argument, the interacting periastron. The fit
    reads the two-argument `galaxy::displaced::binarity::stripped_share(m, composition)`
    (`kick_rank`'s fingerprint). The quadrature P15.T4.b used is
    `multiplicity::all_stars_fraction_below_as_drawn`.
  - `stellar::binary::testing` is private to the sim's own tests, and `brute_force_class_members`
    is P11.T8's, not built.

Plans 09 to 11 were written in parallel with this one. Each task that calls their code begins by
reading their Provides as built. This list was re-validated against the code at 898a3c94
(2026-10-22): the items marked "(re-validated)" were corrected, and the rest were checked and stand
(Risks, "Re-validated at 898a3c94").

## Design notes

1. **Dependency direction.** `hyperion-fit` depends on `hyperion-sim`; the reverse is forbidden and
   tested. The bootstrap problem (plan 02 needs a table before this plan starts) is solved by plan
   02 creating the crate with its one task, as above. (Re-validated: it also depends on
   `hyperion-surface`, for the surface crate's tables, and the test guards base and the surface
   crate too; see Provides.)
2. **Deterministic given its inputs.** No wall clock, no OS randomness, no `HashMap` iteration in
   any output path. Threads are allowed, but work is cut into chunks whose boundaries depend only on
   the manifest, each chunk's result is a pure function of its index, and results are reduced in
   index order. The thread count therefore cannot change a bit of output, and a test runs a smoke
   manifest with one and with four threads and compares bytes. Floating-point text is Rust's
   shortest round-trip form, which is the same on every platform.
3. **Ordinary crates are allowed here and only here.** Expected: `clap`, `rayon` (used only inside
   `map_reduce_chunks`), `serde`, `toml`, `serde_json`, `csv`, `sha2`, `thiserror`. All go in
   `[workspace.dependencies]`. Linear algebra and optimisers are written by hand (non-negative least
   squares after Lawson and Hanson, Nelder–Mead, Levenberg–Marquardt), about 400 lines, because an
   optimiser crate that changes its iteration order between releases would make a table
   irreproducible in a way the lock file cannot see. `Cargo.lock` pins the rest. (Re-validated:
   `optimise` holds P15.T8.a's golden-section search and Nelder–Mead; P15.T3.a adds the other two.)
4. **Tables are Rust source, not data files.** The sim does no I/O, so a table is a `const`. The
   emitter writes one file per table: the header, then items in a fixed order, numeric literals with
   `_` separators so that Clippy's `unreadable_literal` passes without an `expect`, and
   `#[rustfmt::skip]` on each array so that `cargo fmt --check` accepts long rows. Every item gets a
   `///` comment with its unit (dimensionless unless stated) and source.
5. **Header.** Every table file opens with:

   ```text
   //! <one-sentence summary>.
   //!
   //! @generated by hyperion-fit <crate version>, task `<name>` revision <n>. Do not edit.
   //! inputs-sha256: <64 hex digits>
   //! manifest: crates/hyperion-fit/manifests/<name>.toml
   //! data: <dataset>@<sha256 prefix>, ...            (or `none`)
   //! sim-fingerprint: <64 hex digits>                 (or `none`)
   //! since-generator-version: <n>
   //! source: <citations the fit was made against>
   //! acceptance: <the measured figures of the acceptance test>
   ```

   A scratch stand-in committed by a consumer plan has `@provisional by <tool or plan task>` in
   place of the `@generated` line and needs only `since-generator-version` and `source`. A
   hand-entered constant file has `@static`.

6. **What `inputs-sha256` covers.** The SHA-256 of, in order: the task name, the task revision, the
   manifest file's bytes, and the recorded hash of each dataset the manifest names. It does not
   cover source code, because hashing source would mark an hour-long table stale for a changed
   comment. A change to a task's algorithm must bump its `revision()`. Review enforces that, and
   `--rerun-fast` catches a forgotten bump for fast tasks.
7. **Sim fingerprint.** A task that runs the generator's own code depends on that code's numbers. It
   declares a fingerprint: a short list of probe values (for the displaced table, circular speed and
   potential at eight fixed points of the manifest's Milky Way model; for the kick ranks, core mass
   and remnant mass at eight initial masses). The lock file stores the values, and `check`
   recomputes them and compares with a relative tolerance of 10⁻⁹. A refactor that changes no number
   leaves the table fresh; a change to the potential marks it stale.
8. **Staleness.** `hyperion-fit check` fails, naming the table and the reason, if for any generated
   table: the file is missing; its header's `inputs-sha256` differs from the hash computed now; its
   body's SHA-256 differs from `tables.lock` (a hand edit); its fingerprint is outside tolerance;
   its `since-generator-version` exceeds `GENERATOR_VERSION`; or `tables::MANIFEST` disagrees with
   the lock file. With `--rerun-fast` it also reruns every `TaskClass::Fast` task into a temporary
   directory and compares bytes. Nothing slow is ever rerun by CI. Provisional tables are listed as
   warnings until P15.T12 makes them errors.
9. **Scratch values come from the consumers.** A consumer must never wait on a fit to compile. Plans
   02, 06, 09, 10 and 11 each ship the brainstorm's rounded value, either at the table's final path
   with a provisional header (02's `mge`, 06, 11) or as a named constant in their own code (02's
   Chabrier scale, 09, 10). A task here that replaces a constant of the second kind first moves it
   to its table at the same value, which changes no output, and then fits it. (Re-validated: plans
   09 and 10 shipped theirs at the tables' paths, hand-made with no fit header: `cluster_dynamics`,
   which P15.T8.a took over, and `streams`, which P15.T11.a registers.)
10. **Versioning with `GENERATOR_VERSION`.** A table belongs to the generator version. A commit that
    changes the body of a table which generated output reads must bump `GENERATOR_VERSION`, set the
    table's `since-generator-version` to the new value and regenerate goldens, all together. Three
    guards: the emitter refuses to overwrite a table unless `--since` is given and equals the sim's
    current constant, and refuses if the new body differs from the old while `--since` equals the
    old table's value; `check` refuses a table whose `since` is ahead of the constant; and the
    golden tests fail when a star moves. A fit whose output is byte-identical to the committed table
    changes nothing and needs no bump. A table that no record function reads may re-emit a changed
    body at the version in force through the first-emission path, until its first reader's commit
    (decision-p14-t49h-liquids §5).
11. **Manifests** are TOML files in `crates/hyperion-fit/manifests/`: seeds, sample sizes, grids,
    parameter ranges, dataset names. Each slow task also has `<name>.smoke.toml`, a reduced run of a
    few seconds that the crate's tests execute end to end into a temporary directory, checking the
    header grammar and that the emitted item list matches the task's declared items.
12. **External data** lives under `crates/hyperion-fit/data/<dataset>/` with a `PROVENANCE.toml`
    giving the citation, the URL, the retrieval date, the release, the licence or terms of use as
    found, and the SHA-256 of each file. Two classes:
    - _Committed_: small tables (under the 500 kB limit of the `check-added-large-files` hook) whose
      terms allow redistribution, or that hold only measured facts, with the citation kept. The
      Baumgardt–Hilker globular cluster tables are expected to fall here.
    - _Fetched_: large or restrictively licensed sets. Only `PROVENANCE.toml`, the reduction script
      and its small output are committed; raw files go to `crates/hyperion-fit/data/cache/`, which
      is git-ignored. The CMC Cluster Catalog falls here: its snapshots run to gigabytes, and the
      fit needs one reduced CSV of a few hundred rows.

    The first sub-task of each group that needs data records the terms before committing a byte. If
    redistribution is not clearly allowed, the dataset is _fetched_ and only the reduced numbers are
    committed, as facts with their citation. `load_dataset` verifies hashes and fails with
    `LoadDatasetError::HashMismatch`. The repository has no licence file of its own yet; the data
    directories must not be the reason one is chosen.

13. **Fits are made at Milky Way parameters unless the brainstorm says a family is needed.** The
    displaced forms are universal once dimensionless, so one table serves, with the halo dependence
    carried on a three-point grid in escape speed ÷ circular speed. The own-form shares are fitted
    over the bar's parameters because the brainstorm asks for that.
14. **Acceptance figures are stored** in the header and in `tables.lock`, so a reviewer sees the
    quality of a fit in the diff.

## Tasks

Every task starts after M1. P15.T1 to P15.T3 are sequential and open M2. After them the groups
P15.T4 to P15.T11 are independent of each other and can run in parallel, each gated only by the code
its [Schedule](#schedule) row names. P15.T12 closes the plan.

Re-validated at 898a3c94: P15.T1 and P15.T2 are built, and the groups ran without waiting for
P15.T3 (T4, T5.a, T6.a–g, T7.a, T8 and T9.a are built), because no group reads T3's tables. So T3
is one of the parallel groups, gated by the toolchain and plan 02 alone, and its swap, T3.c, rides
version 25 (S1). Within a group the subtasks run in letter order, but P15.T5.d comes before
P15.T5.b, whose harness needs its seams.

### P15.T1 Task registry and command line

**Build.** Extend the crate that P02.T6.a created; create nothing that exists. Add the dependencies
of Design note 3 to `[workspace.dependencies]` and to the crate's `Cargo.toml`
(`[lints] workspace = true` is already there). `src/main.rs` stays thin: parse arguments with
`clap`, call `hyperion_fit::cli::run`, map the library's error enum to an exit code. No `anyhow`:
`rust-dev.md` keeps it for the server binary, and plan 02's `RunFitError` grows variants instead
(`UnknownTask`, `Task(RunTaskError)`, `Emit(EmitTableError)`, `Check(..)`). New: `task.rs` with
`FitTask`, `TaskClass`, `TaskOutput`, `RunTaskError` and `registry()`, and `cli.rs` with the four
commands. Plan 02's `tasks::mge` is wrapped as the first `FitTask` (`Fast`, revision 0) with its
`fit` and `render` unchanged, so `run mge --out <path>` writes the same bytes as before and
`mge_table_is_reproduced` still passes.
`parallel::map_reduce_chunks(n_items, chunk, threads, map, reduce)` with the index-ordered reduction
of Design note 2.

**Files.** `Cargo.toml` (workspace dependencies), `crates/hyperion-fit/Cargo.toml` and, under its
`src/`, `main.rs`, `lib.rs`, `cli.rs`, `task.rs`, `parallel.rs` and `tasks/mge.rs`; `justfile`
(`fit`).

**Tests.** `map_reduce_gives_identical_bytes_for_one_and_four_threads` (sums 10⁶ pseudo-random `f64`
and compares bit patterns); `no_runtime_crate_depends_on_hyperion_fit` (reads the three manifests
with `toml`); `list_prints_registered_tasks`.

**Acceptance.** `just ci` green; `cargo run -p hyperion-fit -- list` prints one row, `mge`;
`tables/mge.rs` is untouched (`git diff --exit-code` on it).

**As built (round 9, lane `fit15`).**

- `main.rs` parses with `clap` (`cli::Cli`) and calls `cli::run`. Plan 02's hand-written
  `Command` enum and `Command::parse` are gone. `RunFitError` is a `thiserror` enum in `lib.rs`
  with the variants `UnknownTask`, `Manifest`, `Task(RunTaskError)`, `Emit(EmitTableError)`,
  `Check(CheckReport)`, `OtherManifestNeedsOut`, `DataNeedsOneDataset` and `Output`. Command-line
  errors exit with 2, everything else with 1.
- `FitTask` has the sketched methods plus `items()`, the item names the smoke run checks, and
  the supertraits `Sync + Debug`. `registry()` returns `&'static [&'static dyn FitTask]`, the
  static `task::REGISTRY` in name order, and `task::find(name)` looks one up. Five tasks are
  registered, so `list` prints five rows: `chabrier` (fast, provisional, P15.T4.a),
  `giant_cooling` (fast), `kick_rank` (slow, provisional), `mge` (fast, provisional) and
  `wd_cooling` (fast).
- `mge` is registered at revision 1, not the plan's 0, which keeps the "version 1" that P02.T12.b's
  refit gave its header. Its `fit()` is unchanged. Its `render()` now returns the table's contents, a
  `RustTable`, and no longer a `String`. `mge_table_is_reproduced` compares
  `pipeline::rerender`, the bytes `run mge --since <current>` would write, with the committed
  file.
- `run` has two options beyond the sketch. `--data DIR` keeps `wd_cooling`'s option: it reads
  the task's one dataset from DIR. `--manifest PATH` runs another manifest and, like `--smoke`,
  needs `--out`: a committed table is always made from `manifests/<task>.toml`. P15.T5.a's
  production run uses it.
- `parallel::map_reduce_chunks(n_items, chunk, threads, map, reduce)`: `map` receives a chunk's
  item range, and `reduce` is an `FnMut(T)` called in index order. Chunks are mapped in waves of
  eight per thread. It returns `Result<(), BuildThreadPoolError>`.
- `pipeline.rs` (not in the plan) holds what `cli` and `check` share: `prepare` runs a task and
  builds its header, `rerender` works out what a run would write without writing, and the
  manifest paths.
- Dependencies added: `rayon` 1.11, `sha2` 0.10, `thiserror` 2 and `toml` 1.1; `clap` and
  `serde` were already there. `serde_json` and `csv` are not added, because nothing reads JSON
  or CSV yet.
- **Adding a task** (plan 06's `stellar_fates`, which the `briefs` lane adds, for one) touches none of the
  toolchain. Add `tasks/<name>.rs` with a unit struct implementing `FitTask`, whose `run` reads
  its manifest's `[params]` and returns `TableItem`s. Add its `pub mod` line and one
  `task::REGISTRY` entry, in name order, and `manifests/<name>.toml`, plus `<name>.smoke.toml`
  if the task is slow. Then `just fit <name>` writes the table, its lock entry and its
  `MANIFEST` row, and you add the table's `pub mod` line to the sim's `tables/mod.rs`.
  `task.rs`'s module documentation says the same.

### P15.T2 Emitter, manifests, lock file and the staleness check

**Build.**

- `emit`: `RustTable` (module doc, header, a list of items: scalars, arrays of scalars, arrays of
  tuples, arrays of structs with named `f64` fields, nested arrays) and `write_table`. Float
  formatting: shortest round-trip, then `_` every three digits on both sides of the point, exponent
  kept; a non-finite value is `EmitTableError::NonFinite`. A file is normally one task's. Where two
  tasks fill one file (`tables/binary.rs`, P15.T9.b and P15.T10.b), each task's header block and
  items sit between its own pair of marker comments, and the emitter rewrites only that block.
- `manifest`: `Manifest::load`, `InputsHash::compute` (Design note 6), `SimFingerprint`, `LockFile`
  (`crates/hyperion-fit/tables.lock`, TOML, entries in name order: name, revision, inputs hash, body
  hash, fingerprint values, since-generator-version, provisional, acceptance figures).
- `hyperion_sim::tables::mod.rs`: `TableInfo` and `MANIFEST`, which the emitter rewrites between two
  marker comments. The provisional tables that exist by now (`mge`, and `kick_rank` and `helium` if
  plan 06's P06.T17 and P06.T19.b have landed; otherwise their own tasks register them) are
  registered as provisional. Their headers, which plan 02 and plan 06 wrote before this grammar
  existed, are rewritten to the `@provisional` form of Design note 5. That touches only `//!` lines:
  no body changes, so there is no bump, and the `mge` task's `render` is updated in the same commit
  so that `mge_table_is_reproduced` keeps passing. Each table's items are checked against the format
  its task group states here, and a difference is settled before any fit.
- `check` with every rule of Design note 8, and `--rerun-fast`.
- `justfile`: `fit-check`, added to `ci`; `test-slow` gains `--rerun-fast`. `.gitignore`:
  `crates/hyperion-fit/data/cache/`.

**Files.** `crates/hyperion-fit/src/{emit,manifest,cli}.rs`, `crates/hyperion-fit/tables.lock`,
`crates/hyperion-sim/src/tables/mod.rs`, `justfile`, `.gitignore`.

**Tests.** `emitted_literals_round_trip` (strip separators, parse, compare bits, over 10⁴ values
including subnormals and powers of ten); `emitted_file_passes_header_grammar`;
`check_reports_hand_edited_table`, `check_reports_changed_manifest`,
`check_reports_fingerprint_drift`, `check_rejects_since_version_above_current`, each against a
temporary directory with a toy task; `emitter_refuses_changed_body_without_new_since`;
`two_tasks_share_a_file_without_touching_each_other`.

**Acceptance.** `just ci` green including `just fit-check`. The first real table (P15.T3) is the
proof that emitted source passes `cargo fmt --check` and Clippy pedantic; until then a toy table is
checked once by hand in the pull request.

**As built (round 9, lane `fit15`).**

- **The header** follows Design note 5, with three details.
  - `data` entries are in backticks (`` `montreal_cooling@66d339259a79` ``), because Clippy's
    `doc_markdown` rejects a bare snake-case identifier in a doc comment.
  - A long value continues on lines indented by two more spaces.
  - A provisional table made by a task keeps every line and says both who made it and why it is
    provisional: `@provisional by hyperion-fit 0.1.0, task `mge` revision 1 (P02.T6.a)`.
- **The body** is everything after the leading `//!` lines. Its SHA-256 is the lock file's body
  hash, so rewriting a header to the grammar leaves the body, and the table, unchanged.
- **Tables written before the emitter.** `mge`, `giant_cooling`, `wd_cooling` and `kick_rank` keep
  their bodies verbatim as `TableItem::Source`. Their rustfmt-shaped layout (no
  `#[rustfmt::skip]`, and `MASS_NODES` packed several to a line) cannot come out of the generic
  items without changing bytes. New tables use the structured items. `chabrier` is the first: a
  scalar, so it needs no `#[rustfmt::skip]`.
- **The toy check by hand.** A toy table of every item kind passed `rustfmt --check` and Clippy
  pedantic at `-D warnings`.
- **Shared files** use the markers `// @begin-table <name>` and `// @end-table <name>`, with the
  block's header in `//` comments. A task gets a block when another registered task has the same
  `table_path()`.
- **Hashes.**
  - `inputs-sha256`: SHA-256 over the name, the revision (`u32`, little-endian), the manifest's
    bytes and each dataset's recorded hash, each field prefixed by its length as a
    little-endian `u64`.
  - A dataset's recorded hash: the SHA-256 of its `PROVENANCE.toml` file list, as
    `<name> <sha256>` lines.
  - `sim-fingerprint`: the SHA-256 of the probe values written as shortest round-trip decimals,
    one per line. The crate's `clippy.toml` forbids hashing float bits.
- **Manifests** hold `task`, `datasets` and `[params]`.
  - `mge`, `giant_cooling` and `wd_cooling` take their parameters from constants in their code.
    Their manifests record those constants, and `Manifest::expect_params` refuses a manifest that
    disagrees.
  - `kick_rank` and `chabrier` read their parameters from the manifest.
- **Fingerprints.** Every task but `chabrier` reads constants of the sim, so each has probes (the
  sketch had none for a task using only `math`):
  - `mge`: `THIN_DISC_HOLE_LENGTHS`;
  - `giant_cooling`: `JUPITER_MASS_KG`, `SOLAR_LUMINOSITY_W` and `SOLAR_RADIUS_M`;
  - `wd_cooling`: the last two;
  - `kick_rank`: the carbon–oxygen core and remnant masses at 8, 10, 12, 15, 20, 30, 50 and
    100 M☉, on `Track::full` with the median draws at Z = 0.02, and the score's scatter.
- **Data.** The `data` module (`Dataset`, `Provenance`, `load_dataset`, `LoadDatasetError`) is
  built here, because the inputs hash needs the recorded hashes. Two datasets are registered:
  - `data/giant_cooling/PROVENANCE.toml`: committed, under CC BY 4.0;
  - `data/montreal_cooling/PROVENANCE.toml`: fetched, its 23 files' SHA-256 recorded. The raw
    files move from `target/data/montreal_cooling/` to the ignored
    `data/cache/montreal_cooling/`, and `wd_cooling::DEFAULT_DATA_DIR` follows them. Plan 06's
    P06.T20.a text still names the old path.
- **`check`** has every rule of Design note 8, and also fails in three more cases:
  - the lock file and the header disagree (on `since`, provisional, inputs hash, task, revision
    or fingerprint hash);
  - a registered task has no lock entry;
  - a lock entry names a task that is not registered.
- **`--rerun-fast`** reruns each fast task in memory (`pipeline::rerender`) rather than into a
  temporary directory, and compares the same bytes. A fast task whose fetched dataset is absent
  is skipped with a warning; this is `wd_cooling` on any machine without the Montreal files.
- **Today's tables.** The five tables have `since-generator-version` 11: every body last changed
  while the generator was at 11. The headers of `mge`, `giant_cooling`, `wd_cooling` and
  `kick_rank` are rewritten, and their bodies are byte-identical to HEAD's (the SHA-256 of each
  body is unchanged). `giant_cooling` and `wd_cooling` are registered as generated, being real
  fits. `mge` and `kick_rank` are provisional.
- **Recipes.** `just fit <task>` reads `GENERATOR_VERSION` from `version.rs` with `sed`.
  `fit-check` is part of `ci`, after `test`. `test-slow` ends with `check --rerun-fast`.
- **Output of `just fit-check`** on today's tables:
  `fresh giant_cooling`, `fresh wd_cooling`, warnings for the provisional `chabrier`, `kick_rank`
  and `mge`, and `2 fresh, 3 warnings, 0 failures`.

### P15.T3 Gaussian-sum coefficients of each density profile

**Source.** The profiles of the brainstorm's "Populations" themselves. The method is the
multi-Gaussian expansion (Emsellem, Monnet and Bacon 1994; Cappellari 2002; neither is in the
brainstorm's list, so re-check when coding). The closed form used for acceptance is Freeman (1970).

**Consumer.** Plan 02, `galaxy::potential` (it runs in M1 on P02.T6.a's first cut, and this replaces
it at the start of M2); plan 08's Jeans tables read the same bodies as tracers, the bar's through
`MGE_BAR` and the bulge's in closed form (ruled 2026-10-23, P15.T3.c).

**Format** (`tables::mge`, lengths in units of the profile's own scale). `MGE_EXP` and `MGE_BAR`
keep plan 02's shape, `[(f64, f64); N]` of (weight, width). New:

```rust
/// Σ weight × exp(−(R² + z² ÷ q²) ÷ 2 width²) ≈ exp(−(R^c + z^c)^(1 ÷ c)).
/// R and z are in scale lengths.
pub struct BoxyTerm { pub weight: f64, pub width: f64, pub q: f64 }
pub const MGE_BOXY: [(f64, [BoxyTerm; 16]); 3];            // boxiness c = 3.0, 3.5, 4.0
```

Re-validated at 898a3c94: `MGE_HOLED_EXP` (P02.T12.b) has the same shape as the other two, with
signed weights on `MGE_EXP`'s widths, and all three hold 14 terms (`tasks::mge::WIDTHS`). An
emitted table imports its row types from its consumer, as `tables::displaced_forms` imports
`galaxy::displaced::forms`, so `BoxyTerm` is defined in `galaxy::potential::mge`, which T3.c adds,
and T3.b's scratch output names it there.

- **P15.T3.a Refit the one-dimensional expansions.** The task already lives at `src/tasks/mge.rs`
  (P02.T6.a) and is a `FitTask` since P15.T1. Give it a manifest, then refit with free widths in
  place of plan 02's fixed logarithmic ones: weights by non-negative least squares on 400
  logarithmically spaced points of s in [10⁻³, 12], widths polished by Levenberg–Marquardt in log
  width. (Re-validated: the manifest, `manifests/mge.toml`, exists since P15.T2 and records the
  first cut's constants, which `Manifest::expect_params` holds to the code; the refit reads its
  parameters from a manifest, as `kick_rank` and `chabrier` do. `optimise::nnls` and
  `optimise::levenberg_marquardt` are new here. Plan 02's one-dimensional quadrature is
  `potential::mge::Gaussian::v_circ_sq`, summed over `double_exponential`'s Gaussians as its test
  `a_thin_double_exponential_peaks_near_two_point_two_scale_lengths` does (the sim's
  `tests/galaxy_potential.rs`). `MGE_HOLED_EXP` is refitted with `MGE_EXP` by
  P02.T12.b's construction, `MGE_EXP`'s weights less a non-negative fit of the deficit
  e^(−s) (1 − e^(−x ÷ s)) on the same widths (`fit_holed_exp`), because plan 02's test holds its
  widths to `MGE_EXP`'s bit for bit; it serves the thin discs and the nuclear disc's inner part,
  P02's R25 and R26.)
  **Acceptance:** maximum relative error of e^(−s) under 0.5% on 0.05–10 with at most 14
  terms; the three-dimensional mass of the spheroidal exponential to 0.05%; the circular speed of a
  thin exponential disc built from the table, by the one-dimensional quadrature plan 02 uses, within
  0.5% of Freeman's Bessel-function form over 0.2–8 scale lengths (Bessel functions by series and
  asymptotic forms in the task's test module, through `math`); the bar profile (plan 02's: the
  azimuthal average of the long bar's surface density at width ratio 0.1, level to 0.85 of the
  half-length with a Gaussian end of 0.15 half-lengths) to 3% of its central value and its mass to
  0.3%, and its cumulative mass within 5% everywhere; plan 02's own table tests (1%, 0.1%, 3%, 0.5%,
  and the 7% cumulative check) still pass, and `mge_table_is_reproduced` passes against the new
  file. (Re-validated: those are the sim's `tests/galaxy_potential.rs`, with
  `mge_exp_reproduces_the_exponential`, `mge_bar_reproduces_the_bar_profile`,
  `the_expansions_have_non_negative_weights_and_ascending_widths`, and P02.T12.b's
  `mge_holed_exp_reproduces_the_holed_profile`, 1% and its mass fraction to 10⁻⁵.) A local 2% is
  out of reach: plan 02's R14 shows by linear programming that no non-negative
  sum of centred Gaussians fits the profile's flat top and sharp end better than 12% locally inside
  one half-length, and that the best possible cumulative-mass error is 4.4%. Beating that needs
  another basis (off-centre or negative terms), which plan 02's potential code does not support.
  (Re-validated: since P02.T12.b it takes signed weights for the holed disc alone, through
  `holed_double_exponential`, and reads `MGE_EXP` and `MGE_BAR` through `nonzero`, which drops
  every term that is not positive.)
- **P15.T3.b The boxy bulge in two dimensions.** For each boxiness c, fit 16 terms with free width
  and q on a polar grid of 48 × 12 points by Levenberg–Marquardt from the one-dimensional solution,
  weights kept non-negative by a projected step. Plan 02 then drops its equal-second-moments
  spheroid (its D6) for this expansion, interpolating linearly in c. **Acceptance:** mass-weighted
  rms relative density error under 3%; enclosed mass within spheres of 0.5, 1, 2 and 4 scale lengths
  within 1% of a direct integral of the profile; midplane radial force within 2% of a brute-force
  integral at six radii; at Milky Way values the rotation curve changes by under 3% anywhere and
  plan 02's enclosed-mass test from 1 pc to 2 kpc still passes. (Re-validated, and ruled in
  decision-p15-revalidation on 2026-10-23: the last two read the expansion in plan 02's potential,
  which T3.c puts there, and they run at T3.c, beside plan 02's
  `the_fixture_s_enclosed_masses_are_the_milky_ways` (`tests/galaxy_milky_way.rs`, slow). The
  curve's change is measured from the crate-private `bulge_spheroid`, so T3.b's own tests, in
  `hyperion-fit`, hold the first three checks, and T3.a–b touch no file of the sim.)
- **P15.T3.c Swap in.** A held checkpoint of version 25 (S1; decision-bump-plan-after-22,
  2026-10-14; updated 2026-10-15): one commit with the table at `--since 25` and plan 02's
  `MassModel` reading `MGE_BOXY` in place of its moment-matched spheroid, its goldens blessed at
  header 24 on its lane branch. It lands only with the S1 train, whose bump commit takes 24 → 25.
  Every star moves, because the potential sets the sub-disc heights.
  T3.a and T3.b are developed and reviewed with `--out` to a scratch path, so `just ci` stays green
  until this commit. (Re-validated at 898a3c94. To land before T3.c without moving the committed
  table, their runs take a manifest of their own, with `--manifest` and `--out`, as P15.T5.a's
  `kick_rank.production.toml` does, while `mge.toml` and the task's revision still reproduce the
  committed first cut for `mge_table_is_reproduced`, `just fit-check` and `--rerun-fast`. Their
  tests check the fitted tables in memory, as `the_expansions_follow_their_profiles` does today.
  T3.c moves the refit's parameters into `mge.toml`, bumps the task's revision (Design note 6) and
  runs plan 02's table tests on the new file. `MassModel::new` builds the bulge in
  `galaxy/potential/model.rs` from `bulge_spheroid` and `potential::mge::spheroidal_exponential`;
  plan 08's kinematics read `bulge_spheroid` too, for the bulge's tracer
  (`kinematics::spheroid::bulge_tracer_axes`), and `MGE_BAR` for the bar's
  (`kinematics::ln_bar_surface_density`). Ruled 2026-10-23 (decision-p15-revalidation Q8): the
  body the potential expands is the bulge's own profile made axisymmetric in the plane,
  exp(−((R ÷ s)^c + (|z| ÷ h)^c)^(1 ÷ c)), with c its boxiness, h its vertical scale and
  s = √((a² + b²) ÷ 2) of its in-plane scales. Its unit body is the bulge's, so it has the bulge's
  mass, both its second moments and its boxiness exactly, with no fit (D6's rule, ⟨R²⟩ matched).
  `MGE_BOXY`'s terms are in units of s and h, so a term's flattening is q h ÷ s in physical units.
  The bulge's tracer follows in the same commit, as plan 08's design note 10 asks:
  `bulge_tracer_axes` returns (s, h) and the boxiness, and the bulge's `JeansTable` and
  `bulge_projected_sigma` read that body's closed-form ln-density in place of the spheroidal
  exponential's, not `MGE_BOXY`'s Gaussians; `bulge_spheroid` goes with its last reader. T3.c
  states σ_e and the fixture's black-hole mass before and after (P08.T4.d), and sets the fixture's
  offset again to keep Sgr A*'s mass if it must, as P15.T4.b did. Plan 02's
  `the_expansions_have_non_negative_weights_and_ascending_widths` gains `MGE_BOXY`, since a reader
  through `nonzero` drops a negative weight silently. Plan 08's bulge tests and the σ_e doctest run
  at the swap; a miss that is no blocker is recorded, and T3.c lands.)

**Files.** `src/optimise.rs`, `src/tasks/mge.rs`, `manifests/mge.toml`, `tables/mge.rs`,
`galaxy/potential/model.rs` (T3.c's swap). Re-validated: also T3.a–b's own manifest, and
`galaxy/potential/mge.rs` (T3.c's `BoxyTerm` and its Gaussians). Ruled 2026-10-23: also
`galaxy/kinematics/spheroid.rs` and `kinematics/mod.rs`, for T3.c's tracer, with plan 08's tests.
The tests stay in the crate's `tests/mge.rs`, with `tests/common`'s `assert_reproduced` (since
the commit 7696bdbe).

**Acceptance.** `just ci` and `just test-slow` green; the task is `Fast` (under ten seconds) and is
rerun by `--rerun-fast`.

### P15.T4 Chabrier high-mass branch scale

**Source.** The volume-complete census, as "Sizing the layers" uses it since the 2026-09-21 density
rulings: within 20 pc (Kirkpatrick et al. 2024, table 4) 66.5% of primaries lie below 0.5 M☉, with
12.9, 17.8 and 2.9% in the bands above, and 68.8% of all stars, companions and white-dwarf
progenitors included; the 10 pc sample (Reylé et al. 2021) agrees. The mean present-day mass per
system with a star or white dwarf is 0.55–0.59 M☉. The former target, 75.9% of all stars, was
Kroupa's function itself. The observed multiplicity is plan 11's (Duchêne and Kraus 2013; Raghavan
et al. 2010).

**Consumer.** Plan 02, `imf::Chabrier`. **Format** (`tables::chabrier`):
`pub const HIGH_MASS_BRANCH_SCALE: f64;` multiplying the branch above 1 M☉.

- **P15.T4.a Move the constant.** Plan 02's 0.68 becomes the table, provisional, same value.
- **P15.T4.b Fit (M4, after plan 11).** With plan 11's multiplicity model and its all-stars
  quadrature, solve for the scale s by bisection on the fraction below 0.5 M☉. The quadrature is
  deterministic, so no sampling is needed. **Acceptance:** fraction below 0.5 M☉ within 1 point of
  the census's 68.8%; primary band shares within the census's Poisson errors; s inside 0.6–1.0 (0.68
  gives about 71% with plan 02's stand-in companions and 1.0 about 67%; otherwise the task fails and
  the discrepancy is reported, never clamped); the local mid-plane mean present-day mass per system
  under the scaled function within 0.55–0.59 M☉ by plan 02's `mean_present_mass`. If no s meets all
  three with plan 11's companions, the task reports which one fails. Bumps the version.

**Files.** `src/tasks/chabrier.rs`, `manifests/chabrier.toml`, `tables/chabrier.rs`,
`galaxy/imf.rs`.

**As built, P15.T4.a (round 9, lane `fit15`).**

- The move is registered as task `chabrier` (fast, revision 0). It emits its manifest's
  `scratch_scale`, 0.68, as a provisional `TableItem::Scalar`.
- `Chabrier::PROVISIONAL_HIGH_MASS_SCALE` keeps its name, which plan 11 uses, and now reads
  `tables::chabrier::HIGH_MASS_BRANCH_SCALE`.
- The move is bit-identical: `imf`'s test `assert_same_bits(Chabrier::default().high_mass_scale(),
0.68)` still passes, and no golden moves.
- P15.T4.b replaces the run with the bisection, bumps the revision, and drops `scratch_scale` from
  the manifest.

**As built, P15.T4.b (round 9b, lane `fates`, with P11.T1.d; ruling 138).**

- Task `chabrier` revision 1 (fast): the multinomial maximum-likelihood s on the 20 pc census's
  primary counts, 1,491 / 288 / 400 / 64 in bands A–D of 2,243 (our tally of Kirkpatrick et al.
  2024, Table 4, in the manifest), the model's band shares renormalised over 0.08–8 M☉; a fixed
  72-step golden-section search of 0.3–1.6 and 60 bisections for each 1σ end. **s = 0.9201** (1σ
  0.8507–0.9927; 0.8957 and 0.9352 under Cummings's and El-Badry's IFMRs). The scratch 0.68 is
  retired and the table is no longer provisional.
- The checks, over systems whose primary lies below 8 M☉ (`imf::Truncated`, `fates::CensusFates`
  and `GalaxyParams::census_system_masses_under`, new), all pass: band shares 67.7 / 12.0 / 16.8 /
  3.45%, misses −0.73, +1.19, +1.15, −1.53 Poisson errors, χ² 5.62 on 2 dof (≤ 9.21, each ≤ 2);
  all stars below 0.5 M☉ 69.84% (67.8–71.0%); the local mid-plane mean in stars and white dwarfs
  0.5517 M☉ (0.54–0.60). Reported: the all-inclusive local mean 0.6115 M☉, of which neutron stars
  and black holes 0.0530 M☉; the primaries' mean present mass 0.4027 M☉ against the census's
  0.437. The first attempt's bisection on the all-stars share (s = 0.9015, failing two unlike
  comparisons) is superseded.
- **For the owner (ruling 138.6):** Chabrier's (2005) system function (m_c 0.25 M☉, x 1.35) fits
  the census's primaries better, χ² 4.4 as published and 1.7 at s = 0.88; it is not adopted,
  because it also moves plan 13's substellar branch.
- **Consequences, as built.** The all-inclusive windows are re-derived at the fitted s: the Milky
  Way fixture's mean 0.59–0.61 M☉ (0.5985 galaxy-wide, `derive::tests`; `tests/galaxy_params.rs`)
  and 0.60–0.62 locally (0.6115, `tests/galaxy_fields.rs`, which also asserts the census-like
  0.5517 in 0.54–0.60); the seeds' old populations 0.57–0.62 and young disc 0.85–0.90
  (`tests/common`, 0.581–0.611 and 0.873 over 3,000 seeds). The system count is 8.55 × 10¹⁰ for
  the fixture. P02.T11's benchmark is unchanged at the new N: 0.00201 systems per ly³ in the plane
  and 0.00198 at the Sun's height (0.0018–0.0021), 0.0426 M☉ pc⁻³ (0.0415 ± 0.004). Ruling 137.7's
  nuclear-disc bracket, re-derived: Kroupa's 13.5–22.5 × 0.816 = **11.0–18.4 per ly³**, measured
  17.93. The fixture's black-hole offset is set again to −0.3800 dex (σ_e about 117.3 km/s) to keep
  Sgr A*'s mass. `ProvisionalFates`' tests and the brainstorm's band-share table rows keep the
  scratch 0.68 explicitly (`Chabrier::new(0.68)`), since those figures are the stand-in's; the
  table's default row awaits the owner's edit (fitted A–E 67.1 / 11.9 / 16.7 / 3.4 / 0.95%; 16.68, so 16.7, ruling 140.10).
- **Findings held provisionally (ruling 138's consequences, windows stated at 0.68):** bound
  clusters born 373 per Myr (240–360), 158,664 associations (10,000–150,000), one ionising star
  per 270 M☉ formed (400–600), M4's 396 neutron stars (100–350, also ruling 137's w), the rogue
  planets' saturation threshold 37.49 per star (35.9 ± 3%), the barred orbits' Jacobi drift
  1.09 × 10⁻⁴ (10⁻⁴), the old thin disc's dead primaries under the default 10.7% (the T30 window
  is Kroupa's and holds). The spine construction's stars per system under the default is 1.457,
  above 1.45; T1.d's bracket holds on the drawn companions (1.437). _Ruled (ruling 140, lane
  `win140`; tests and text only, no output moves):_ every window but the Jacobi drift was derived
  through 0.68, and each is re-derived at the fitted scale and asserted, the holds removed: bound
  clusters 300–480 per Myr, associations 1.2–2.1 × 10⁵, ionising stars 3.0–4.8 per 10³ M☉ formed,
  M4 170–600 neutron stars (Ye et al. 2019's 150–225 recorded as a tension for P11.T6/T11), the
  rogue planets' threshold 37.5 per star ± 3% (the 2,000-seed windows hold, see plan 13's P13.T2),
  and the brainstorm's default row 67 / 12 / 17 / 3.4 / 0.95%. The Jacobi drift was numerical:
  T6.a's fast test now runs its 20 barred orbits through `integrate_checked(…, 1e-4, 6)` and
  asserts 10⁻⁴ (ruling 140.7). The 1.457 joins plan 11's Risks (ruling 140.8). Ruling 141 does
  the same for T18.a's Type Ia rate: 0.42–0.53 a century (0.462 measured) and the ancient shares
  3.3–4.5%.

**Round 9b, lane `fates` (with P06.T30 and P11.T1.d), for the tables it touches.** `stripping`
(slow, revision 0; `tables::stripping`, plan 11's stripping table) and `period_correction` (slow;
`tables::period_correction`, plan 11's refit) join the registry. `kick_rank` gains two fingerprint
probes, a 7.7 M☉ star with its stripped mark set, since the reference population's tracks now read
plan 11's share; its rerun kept the body. The mean masses move the fixture's potential (v_c
224.018 to 223.961 km/s), so `displaced_smoke` was regenerated (`hyperion-fit orbits --smoke`,
PROVENANCE sha256 updated) and `displaced_forms` refitted: mean misplaced share 0.9700 (from
0.9742), the neutron stars' half-density height 173 pc (from 223, against 110–130), bar
elongation 0.34 (from −1.12). The new and refitted tables say `since 14`; the orchestrator's bump
to 15 restamps them.

**Ruling 137.6.** The neutron stars' half-density height, 173 pc against P15.T6.e's 110–130, is
a smoke-fit finding held for ruling 120.3's orbit production run, not a consequence of the
stripped share (more low-mode stars lower it). The window's source is owed, and it must be stated
at a low-mode share w (the fixture's is now 0.2675).

### P15.T5 Kick-law rank table and its four defaults

**Source.** The measured log-normal of young isolated pulsars (Disberg and Mandel 2025), the
progenitor ordering of Mandel and Müller (2020) with the 45% scatter of Disberg, Mandel and Hirai
(2026), and the generator's own tracks from plan 06. The defaults are pinned by the brainstorm's
"Testing" list: Willcox et al. (2021), Igoshev et al. (2021), Pfahl et al. (2002) and Ivanova et al.
(2008), the double neutron stars' eccentricities, Nagarajan and El-Badry (2025).

**Consumer.** Plan 06, `stellar::remnant::KickRankTable` (M2). The kick law is plan 06's
(`stellar::remnant::KickLaw`); plans 08 and 09 read it through plan 06. **Format:** plan 06's,
`tables::kick_rank::SCORE_QUANTILES: [f64; 257]`, the ordinary kick score at ranks i ÷ 256 over its
`ReferencePopulation`, strictly increasing, which `KickRankTable` interpolates linearly. P06.T19.b
commits it provisionally from 10⁶ draws. The law's four defaults are the brainstorm's ramp,
black-hole factor and window widths, which sit in the table beside the quantiles as four constants
that `KickLawParams::default` reads (`LOW_RAMP` (2.0, 3.0) M☉, `BH_FACTOR` 0.75,
`EC_WINDOW_SINGLE` 0.1 M☉ and `EC_WINDOW_STRIPPED` 1.0 M☉; plan 06's Provides), and plan 11's
`CLUSTER_MERGED_BINARY_FATE` (`stellar::binary`), which is not in the table. (Re-validated at
898a3c94: the plan said the table held none. The `kick_rank` task writes the four by copying the
sim's own constants, in `tasks/kick_rank.rs`'s `render`. The track reads its own copies of the
windows, `stellar::remnant::collapse`'s `SINGLE_STAR_WINDOW` and `COMPANION_STRIPPED_WINDOW`, which
`kick.rs`'s `the_defaults_are_the_brainstorms_and_the_tracks` holds equal.)

- **P15.T5.a Production rank table.** Call plan 06's `reference::score_quantiles(pop, n, seed)`, the
  function that made the provisional table, with n = 10⁷ in chunks through `map_reduce_chunks`
  (exact quantiles by a merged sort of chunk histograms on a fixed fine grid, so the result does not
  depend on chunking or threads). If `score_quantiles` cannot be called per chunk, this task splits
  it into a per-chunk scorer and a quantile step in plan 06's module, with plan 06's tests.
  Fingerprint: core and remnant masses at eight initial masses. **Acceptance:** mapping 10⁷ fresh
  reference draws through the table gives ln(speed ÷ km/s) with mean 5.60 ± 0.01 and standard
  deviation 0.68 ± 0.01 before the clamp; the knots are strictly increasing; the largest change from
  plan 06's provisional table is reported. This task writes the file with `--out` to a scratch path
  and its figures into the pull request; **plan 06's P06.T19.e is the commit that swaps it in**,
  with `--since` the bumped version, the regenerated goldens and a rerun of P06.T19.d. One bump,
  owned by plan 06.
- **P15.T5.b Defaults review.** The four defaults are fields of plan 06's `KickLawParams` (the ramp
  over core masses of 2–3 M☉, the black holes' 0.75, the two electron-capture window widths) and
  plan 11's fate of merged binaries in clusters, which P15.T5.c reviews. The harness evaluates
  plan 06's test observables on a grid of each default about its value (one at a time, the rest
  held) and emits a sensitivity table, the acceptance figures of a registered table of its own. No
  default moves in this task (ruled 2026-10-23, decision-p15-revalidation Q2). **Acceptance:** at
  the committed defaults every observable of P06.T19.d passes, but the two that ruling 147.2 holds
  at measured values, which the harness reports as held; for each default the harness reports the
  interval over which each passing observable keeps passing, and whether the default lies inside
  it and not within a tenth of its width from an edge. A default at or near an edge is a finding,
  recorded with the table, and is not moved here. Re-validated at 898a3c94, and ruled 2026-10-23:
  - The harness cannot vary the windows through `kick_observables`: the track reads `collapse`'s
    constants, and the `ec_window_*` fields only record them (P06's "Deviations in T19.a–d", "For
    P15.T5.b"). Nor can it vary the rank table's population, which `ReferencePopulation::score`
    takes from `KickLawParams::default()` with only its scatter replaced, so a moved ramp or window
    meets a table built at the defaults. P15.T5.d adds both seams first.
  - Plan 06's six observables are seven tests since ruling 96.3 added the escape share
    (`tests/stellar_kick.rs`, slow: `kick_observables` over 20,000 stars with
    `KickLawParams::default()` and `KickRankTable::generator()`). Ruling 147.2 held two of them at
    measured values outside their windows: isolated pulsars under 50 km/s on the sky, 0.0708
    against 3–7%, and the escape share, 0.1159 against 12–20%. Both are accepted as findings and
    stay held (decision-p15-revalidation Q2): each misses by under one and a half of its binomial
    errors at 20,000 stars, and neither is a wrong state or an absurd value.
  - _The rank table at a grid point_ (Q1). Each grid point of the ramp or of a window, the centre
    included, rebuilds the table at that point's membership through P15.T5.d's seam, by
    `score_quantiles` on the committed table's seed. The production manifest takes a score count
    of 10⁵, about 1.4 CPU-minutes a point, or up to 10⁶ if the builder finds that too coarse,
    stated. The black-hole factor's points hold the centre's rebuild, since the membership never
    reads the factor. The ramp's two ends move together. The header states the harness's
    resolution, the centre's observables on its own rebuild against those on the committed table.
  - _The table_ (Q3). A registered task, `kick_defaults` (slow, revision 0, with
    `manifests/kick_defaults.toml` and `kick_defaults.smoke.toml`), writes
    `tables/kick_defaults.rs`: each default's value, the grid, and the interval of each passing
    observable. No generator code reads it. Its fingerprint is `kick_rank`'s probes and the four
    defaults, not the committed rank table, whose rebuilds it makes itself. It is `@generated`,
    emitted at the version in force through the first-emission path, and moves nothing, so it
    takes no bump; `tables/kick_rank.rs` keeps its body until P06.T19.e. The smoke run's item check
    uses `tests/common`'s `declared`.
  - A default that a later ruling moves changes the table's constant, which the `kick_rank` task
    then emits from its manifest rather than by copying the sim's constant, and for a window
    `collapse`'s copy too, with a bump on the first galaxy train after its lane's gate.
- **P15.T5.c Fate of merged binaries in clusters (M4).** With plan 09's cluster retention and plan
  11's engine: neutron-star retention of 18–26% at a birth escape speed of 100 km/s, 15–25% at 50
  and 5–17% at 20 (ruling 126.3, amending 106.4 and 96.3 for clusters: the low mode is judged on
  its pair's systemic speed, σ 12 km/s), and under 1% in the most massive open clusters, under plan
  11 (the merged object stays a member and is judged on the pair's velocity) and under the
  alternative. **Acceptance:**
  the default passes all four bands; if only the alternative does, the finding goes to plan 11.
  Re-validated at 898a3c94, and ruled 2026-10-23 (decision-p15-revalidation Q4 and Q5):
  - Nothing reads `CLUSTER_MERGED_BINARY_FATE` (`stellar/binary/mod.rs`). P09.T9.b's retention,
    `features::interior::retention` (`retention`, and `retention_tabulated` over
    `tables::cluster_retention`, which plan 09's slow `cluster_retention` task fits), is a
    closed-form quadrature over plan 08's kick bins. It judges every low-mode neutron star on its
    pair's systemic speed (`PAIR_SYSTEMIC_SIGMA`, 12 km/s), merged or not, and never runs the
    engine of plan 11. That is the default's reading, `StaysJudgedOnPairVelocity`, applied to every
    low-mode neutron star, so retention as built is the default's measurement.
  - Ruling 137.3 (P09's Risks, "Ruling 137.3, as applied") states ruling 126.3's windows at a
    low-mode share w of 0.181 and shifts them with the measured w. At the fixture's w of 0.2675
    they are 26–34%, 24–34% and 10–22%, and under 1% in the open cluster, and the code measures
    the four as 31.4%, 28.6%, 16.2% and 0.43% (`neutron_stars_are_retained_as_the_brainstorm_says`,
    `retention.rs`). These windows replace ruling 126.3's above. So the default passes all four
    bands, the acceptance's first clause; the alternatives are consulted only if it fails, and are
    not measured.
  - What T5.c builds: a `const` assertion in `retention.rs` that `CLUSTER_MERGED_BINARY_FATE` is
    `MergedBinaryFate::StaysJudgedOnPairVelocity`, the one reading the quadrature applies, so that
    changing the default fails to build until a reader of the other branches exists; and its
    record. No output moves, so it takes no bump. A plan 11 task that models merged products in
    clusters explicitly adds the other branches.
  - P11's finding F13 ("P11.T4.k as built": plan 06's remnant law should read the stripped mark,
    as its kick law does) leaves this plan, whose Scope keeps plan 06's law out. It is a science
    refinement of which binary-stripped stars make black holes, not a wrong class under the
    documented single-star law (P11's design note 16), so it is a deferred correction, with no
    task and no bump. T5.c does not wait on it.
- **P15.T5.d The kick law's seams for the defaults review** (re-validated at 898a3c94; new, and
  before P15.T5.b). In plan 06's module, with plan 06's tests, as P15.T5.a allowed for
  `score_quantiles`. The track's two electron-capture windows become a run-time option that
  defaults to `collapse`'s constants (for example in `stellar/sse/track.rs`'s `TrackOptions`, read
  by `track/phases.rs`'s `captures_electrons`). `ReferencePopulation` takes a `KickLawParams` for
  its membership, in place of `KickLawParams::default()` with its scatter, and passes its windows to
  its tracks, and `kick_observables` passes its params' windows on. At the defaults every output is
  bit-identical. **Files:** `stellar/sse/track.rs`, `stellar/sse/track/phases.rs`,
  `stellar/remnant/reference.rs`. **Tests:** at the defaults the committed table is still
  reproduced (`the_committed_body_is_the_rendering_of_its_quantiles`, the fit's
  `tests/kick_rank.rs`), as is `two_runs_give_the_same_table_bit_for_bit` (`tests/stellar_kick.rs`);
  a narrower single-star window moves the captures at its edge in the reference population only.
  **Acceptance:** `just ci` green, and `golden_diff.py` (the sim-determinism skill's) finds no
  golden moved. Half a day.

**Files.** `src/tasks/kick_rank.rs`, `src/tasks/kick_defaults.rs`, `manifests/kick_*.toml` with
their smoke manifests, `tables/kick_rank.rs`, and `tables/kick_defaults.rs` with its `MANIFEST` row
(P15.T5.b, ruled 2026-10-23); P15.T5.d's three files of plan 06; and P15.T5.c's guard in the sim's
`galaxy/features/interior/retention.rs`. The fits are `Slow`: 10⁷ tracks are tens of minutes on
one core, so CI never reruns them and relies on the hash, the fingerprint and the smoke run.

**As built, P15.T5.a (round 9, lane `fit15`).**

- `tasks::kick_rank::KickRankTask` (slow, provisional P06.T19.b) runs `fit_parallel`. Its
  `scores_parallel` scores the sample stars in rounds, each of twice the members still wanted,
  in chunks of 1,024 through `map_reduce_chunks`. It joins the chunks in index order and cuts
  at the n-th member. That gives exactly the scores `score_quantiles` sorts, and a test checks
  the quantiles bit for bit on one thread and on four.
- Plan 06's module was not touched. It already exposes the per-chunk scorer
  (`ReferencePopulation::scores`) and the quantile step (`quantiles_of`).
- The quantiles come from one sort of all the scores (80 MB at 10⁷), not from chunk histograms
  merged on a fine grid. The sort is exact and independent of the chunking.
- **Manifests:**
  - `kick_rank.toml`: the committed table's 10⁶ scores, with 10⁵ fresh ones for the acceptance
    figures;
  - `kick_rank.production.toml`: 10⁷ scores, with 10⁶ fresh ones, not the plan's 10⁷, because
    ±0.01 on the moments needs far fewer and the second 10⁷ tracks would double the run;
  - `kick_rank.smoke.toml`: 300 scores.
- **Acceptance figures.** Fresh scores go through the new table; ln(v ÷ km/s) is taken with the
  rank clamped to 0.001–0.999 and without the truncation at 1,000 km/s. The report also gives
  the largest shift of a knot's rank from the committed table. The lane reads "before the clamp" as before the truncation, which ruling 96.2 added after the plan was written. The rank clamp alone narrows the spread by a few thousandths (0.6758 over 10⁵ fresh scores of the committed table), inside the ±0.01. This reading is the orchestrator's to confirm. The committed header's `source` was corrected afterwards ("ApJL", which Clippy's `doc_markdown` rejects, became "ApJ Letters") by re-rendering it from the committed quantiles and the recorded acceptance figures, which is what a rerun writes, rather than by a second half-hour run.
- **The hand-over to P06.T19.e.** A committed table is always made from `kick_rank.toml`. So
  P06.T19.e moves the production parameters into `kick_rank.toml` (its inputs hash changes with
  them, as it should), bumps the generator version, and either reruns `just fit kick_rank` or
  commits the scratch file after correcting its `manifest:` line.

### P15.T6 Displaced-population form table

**Source.** Orbit integration in the model's own potential (plan 02), about 2 × 10⁷ orbits,
including a rotating bar for the bar-, bulge- and nuclear-disc-born. The kick law sets only the
weights (plan 08's quadrature), never the forms. Literature checks: Sartore et al. (2010) for the
neutron stars' bound share and height, Boodram and Heinke (2022) for kicks smoothing a barred bulge.

**Consumer.** Plan 08, `galaxy::displaced` (M3).

**Format** (`tables::displaced_forms`). Lengths in thin-disc scale lengths R_d, speeds in the
circular speed v_c at 3 R_d, times in R_d ÷ v_c (11 Myr for the Milky Way).

```rust
pub const SPEED_EDGES: [f64; 7] = [0.25, 0.5, 0.85, 1.3, 1.75, 2.2, 2.8];
pub const AGE_EDGES: [f64; 6] = [0.1, 0.3, 1.0, 2.0, 4.0, 8.0];
pub const ESCAPE_RATIO_NODES: [f64; 3] = [2.1, 2.5, 2.9];      // v_esc ÷ v_c at 3 R_d, in the plane
pub const COROTATION_RATIO_NODES: [f64; 3] = [1.0, 1.2, 1.4];  // bar corotation ÷ half-length

/// weight × exp(−R ÷ h_r) × exp(−(|z| ÷ h)^β) ÷ h, with h = h_0 exp(R ÷ r_flare).
pub struct FlaredLayer { pub weight: f64, pub h_r: f64, pub h_0: f64, pub r_flare: f64,
                         pub beta: f64 }
/// weight × (1 + (R² + z² ÷ q²) ÷ a²)^(−γ ÷ 2).
pub struct CoredPowerLaw { pub weight: f64, pub a: f64, pub q: f64, pub gamma: f64 }
/// In units of v_c. `outbound` is the share of objects with z × v_z > 0.
pub struct ClassKinematics { pub mean_phi: f64, pub sigma_r: f64, pub sigma_phi: f64,
                             pub sigma_z: f64, pub outbound: f64 }
pub struct DiscBornForm { pub layer: FlaredLayer, pub spheroid: CoredPowerLaw,
                          pub in_cube: [f64; 3],            // bound and inside, by escape ratio
                          pub unbound_in_cube: [f64; 3],    // unbound but still inside
                          pub kinematics: ClassKinematics,
                          pub misplaced: f64 }              // total-variation distance of the fit
pub struct OldBornForm { pub own_share: [f64; 3],           // by corotation ratio
                         pub spheroid: CoredPowerLaw, pub in_cube: [f64; 3],
                         pub kinematics: ClassKinematics, pub misplaced: f64 }
pub const DISC_BORN: [[DiscBornForm; 7]; 8];                // [speed bin][age bin]
pub const THICK_DISC_BORN: [OldBornForm; 8];                // own_share zero
pub const HALO_BORN: [OldBornForm; 8];                      // own_share zero
pub const BULGE_BORN: [OldBornForm; 8];
pub const BAR_BORN: [OldBornForm; 8];
pub const NUCLEAR_DISC_BORN: [OldBornForm; 8];              // u against its own circular speed
pub const HYPERVELOCITY: CoredPowerLaw;                     // ancient Type Ia survivors
```

`layer.weight + spheroid.weight = 1`. A class's forms describe its bound members; the unbound still
inside the cube are fitted with the fastest speed bin of the same age bin, which is where plan 08
puts them. For the two ballistic age bins (τ < 0.3) plan 08 uses the brainstorm's analytic layer and
ignores `layer`; the fitted values are kept for the record, and `kinematics`, `in_cube` and
`unbound_in_cube` are used. Node arrays are read by linear interpolation, clamped at the ends.

- **P15.T6.a Orbit integrator.** Fixed-step kick-drift-kick leapfrog in plan 02's tabulated
  axisymmetric potential (bilinear force interpolation on the table's grid), step 1 ÷ 200 of the
  local circular period with a floor. A rotating-frame variant adds a bar: the bar-plus-bulge mass
  takes a quadrupole of manifest-given amplitude rotating at Ω_p, with Coriolis and centrifugal
  terms in the standard rotating-frame leapfrog. If plan 10's stream integrator is public by then
  and adequate, reuse it for the axisymmetric case. **Acceptance:** energy (the Jacobi integral with
  the bar) conserved to 10⁻⁴ relative over 10 Gyr for 1,000 test orbits; a circular orbit stays
  circular to 10⁻³ in radius; one and four threads give identical bytes.
- **P15.T6.b Births and the run.** Births from the thin birth layer (the young disc's envelope
  without arms on the thin disc's radial profile, with the declining formation history), the thick
  disc, the halo mixture, the bulge, the bar and the nuclear disc; initial velocity circular plus
  the kick for the discs, and drawn from an isotropic Jeans dispersion for the rest (the fit's own
  statement; plan 08 does not exist at the first run, and the forms are insensitive to it at the 1%
  level, which T6.e checks). Kick speeds log-uniform in u from 0.02 to 6 so that every bin fills
  whatever the law; directions isotropic; ages from the population's history. Each orbit is
  integrated to the epoch and recorded (R, |z|, cylindrical velocity, in-cube flag, bound flag) into
  per-bin histograms on 48 × 40 logarithmic cells in R and |z|. About 1.2 × 10⁷ disc-born orbits and
  1.6 × 10⁶ per old population, set in the manifest. The histograms go to `data/cache/displaced/`
  and their hash into the lock file. **Acceptance:** every bin holds at least 2 × 10⁴ orbits; a 1%
  smoke run twice gives identical bytes. Hours on one core, tens of minutes on sixteen; never run in
  CI.
- **P15.T6.c Disc-born forms, with the young bins regularised.** Fit `FlaredLayer` plus
  `CoredPowerLaw` to each of the 56 histograms by minimising the total-variation distance,
  Nelder–Mead from a fixed start. The scratch fits behind the brainstorm ran to slopes of 10⁵ and
  core radii of 10³ R_d wherever the spheroid carried little weight, in bins holding 10⁻⁵ of the
  objects; those are unconstrained directions and not results. So: γ is boxed to [2, 8], a to [0.2,
  12], q to [0.02, 1], β to [0.8, 3], r_flare to [1, 50]; a spheroid fitted at under 0.05 of the
  weight is set to zero and the layer refitted alone; and along each speed row the parameters of
  neighbouring age bins are tied by a quadratic penalty on their differences in log, with a strength
  chosen so that the weighted mean misplaced share rises by no more than 0.3 points. **Acceptance:**
  weighted mean misplaced share at most 7% (the brainstorm's 6.6% plus the allowance); no parameter
  on a box edge in any bin holding more than 10⁻⁴ of the weight; each bin's misplaced share against
  its noise floor (from two half-samples) reported in the header; parameters vary monotonically or
  smoothly along each row (no sign change of the second difference larger than the half-sample
  scatter).
- **P15.T6.d Old-population forms and own-form shares.** One `CoredPowerLaw` per speed bin for the
  thick disc and halo. For bulge, bar and nuclear disc, the brainstorm's mixture (own form ×
  `own_share` plus a spheroid) fitted on a three-dimensional histogram in the bar's frame, at each
  of the three corotation ratios and, within each, at two bar masses, to confirm that the share
  depends on the ratio and only weakly on the mass (if it does not, a second node array is added and
  plan 08 told). **Acceptance:** at a ratio of 1.2 the shares reproduce the brainstorm's within 0.08
  (bar 0.95, 0.61, 0.20, 0.05; bulge 0.91, 0.76, 0.55, 0.25; nuclear disc 0.88, 0.67, 0.43); shares
  fall monotonically with speed; misplaced share at most 6% for the bulge and 13% for the bar; the
  bar's elongation relative to the unkicked control within 0.05 of 0.99, 0.79, 0.5 and 0.25 over the
  first four bins and under 0.05 beyond u of 1.75; the length along the bar within 10% of the
  control's in every bin.
- **P15.T6.e Kinematics, in-cube shares and universality.** Per class the mean rotation, three
  dispersions and outbound share from the same orbits; `in_cube` and `unbound_in_cube` at the three
  escape ratios from three halo masses. Then repeat T6.b at a tenth of the size in four more
  potentials spanning the parameter ranges, and once more at Milky Way values with plan 08's
  velocity laws for the births if plan 08 has landed. **Acceptance:** the Milky Way table misplaces
  at most 9% in every other potential (the scratch work found 7.6–8.1%) and changes by under 1 point
  with plan 08's births; reweighted by plan 06's kick law at Milky Way values the table gives 13–14%
  of neutron stars unbound (accept 12–15%), 71–73% of neutron stars inside the root cube (accept
  69–75%) and 99% of black holes (accept 98–99.6%); the slowest class has `mean_phi` above 0.95 and
  the fastest below zero; the neutron stars' half-density height is 110–130 pc; phase mixing is
  complete by τ of 8–30 (the last age bin's form at τ of 8–30 and beyond 30 agree within the noise
  floor).
- **P15.T6.f Hypervelocity row.** The density of unbound survivors on straight lines: the old
  populations' density convolved with 1 ÷ (4π r² v), fitted by one `CoredPowerLaw`; its shape does
  not depend on v, so it serves both survivor populations of ruling 128.1 (0.26 at 1,000–1,500
  km/s, 0.04 at 2,000–2,500). **Acceptance:** misplaced share under 10%; at Milky Way rates the
  number inside the cube is 10⁴–10⁵, of them 1,500–3,500 fast and 1.5–4.5 × 10⁴ slow; the
  rate-weighted share of slow launches below 1.5 times the local escape speed, where the straight
  line is poor, is reported as a finding.
- **P15.T6.g Orbits through the centre: the time-transformed leapfrog** (decision-p15-t6g-orbits,
  2026-10-12; slow-pass 2 triage F2 and F1). It replaces T6.a's fixed step, and ruling 140.7's
  halving as the method (the halving stays as the check), for every orbit, plain and barred, of every
  class. From version 16's potential on, two of T6.a's 1,000 barred test orbits never converged under
  the halving (368 and 740: 1.41 and 0.143 after six), because no fixed step resolves an orbit that the
  bar throws into the black hole's radius of influence and the cluster's cusp (Gerhard and Binney
  1985, MNRAS 216, 467: a central mass makes such orbits chaotic, so they belong in the table).
  - **The integrator.** The time-transformed leapfrog of Mikkola and Aarseth (2002, Celest. Mech.
    Dyn. Astron. 84, 343) in the bar's rotating frame, with the time transformation dt = ds ÷ Ω(r): Ω
    is the circular frequency `v_c(r) ÷ r` of the plane's tables at the spherical radius r = |x|, so
    that every step is `step_fraction` (1 ÷ 200) of the local circular period, as T6.a first asked. A
    step of fictitious length h = 2π × `step_fraction` is K(h ÷ 2) D(h) K(h ÷ 2). Each kick adds
    `−∇Φ δt` to the momentum, with δt = (h ÷ 2) ÷ Ω(r) at the current position, and adds
    `δt Ω′(r) (x · (p + p′) ÷ 2) ÷ r` to the auxiliary W (p and p′ the momentum before and after the
    kick). The drift is T6.a's exact rotate-and-drift of the kinetic-plus-rotation flow over the
    physical time h ÷ W. W starts at Ω(r₀). The step that would pass the orbit's age is replaced by
    one plain kick-drift-kick step of the time left, so the orbit ends at its age exactly. Ω and Ω′
    are read from a table of ln Ω against ln r built once per potential from
    `PotentialTables::v_circ_sq` and its derivative (cubic Hermite with exact slopes, at least 40
    knots a decade from 10⁻⁶ to 4 × 10⁶ ly, continued as r^−3/2 inside and with its last slope
    outside), so that a step costs one force, one table read and one rotation. The method is
    time-symmetric, not symplectic, and needs no step floor: `min_step_years` leaves the three orbit
    manifests, and `step_fraction` and `nuclear_step_fraction` keep their values as fractions of the
    local period. Nothing is softened: the black hole stays a point mass and the cusp the potential's.
    Every operation is IEEE-754 arithmetic, `sqrt` or `hyperion_sim::math` (`ln`, `exp`, `sin_cos`,
    `hypot`), never `mul_add` or a `std` transcendental, and an orbit stays a pure function of its
    manifest and number, so the smoke histograms' digest is the same on Linux, macOS and Windows.
  - **Not chosen** (the decision's measurements): the logarithmic-Hamiltonian leapfrog with U = −Φ
    (Mikkola and Tanikawa 1999, MNRAS 310, 745; Preto and Tremaine 1999, AJ 118, 2532), whose step
    follows 1 ÷ U, and −Φ changes
    only 1.6-fold from 3,500 ly to 1 ly while the period falls 5,200-fold; Bulirsch–Stoer or IAS15
    (Rein and Spiegel 2015, MNRAS 446, 1424) for the plunging orbits, whose order and error control assume a smooth
    force where the tables' force has derivative jumps at every grid line; a fixed step per orbit,
    which cannot know a chaotic orbit's pericentre.
  - **The check stays.** An orbit whose Jacobi integral (its energy without the bar) has drifted by
    more than `drift_tolerance` is integrated again from its start at half the step fraction, up to
    `max_halvings` times. Production keeps 10⁻³ and three (ruling 120.3: δ ln r ≈ (|E| ÷ v_c²) δE ÷ E,
    3–8 times the relative drift, so 10⁻³ moves an apocentre by a few thousandths in ln r against
    cells 0.20 wide); the smoke manifest 10⁻² and one; T6.a's tests 10⁻⁴ and six.
  - **The drift's scale** (decision-p15-t6g-orbits §2). Every drift (the check's, the tests',
    the record's worst and the unresolved count's) is |J − J₀| ÷ max(|J₀|, |Φ(x₀)|), Φ(x₀) the
    full potential at the start. An energy error moves a bound orbit by δ ln r ≈ δJ ÷ v_c² and a
    free one by δr ÷ r ≈ δJ ÷ 2T, neither of which involves J itself; dividing by |J₀| measured
    a cancellation for the marginally bound and unbound (the five diagnosed of the reduced run's
    13 "unresolved" fast kicks, J₀ from −541 to +947 (km/s)², off by 1–2 (km/s)²). For bound orbits the scale is
    |Φ₀| (|Φ₀| ÷ v_c² ≈ 3–9 at the fixture's radii, so 10⁻³ bounds δ ln r by about 0.009); for
    fast kicks it is |J₀| ≈ T₀.
  - **Capture.** Within 1 ly of the centre, where the black hole holds about 96% of the enclosed mass,
    an orbit whose specific angular momentum |x × p| is below 4GM•/c (c = 1 ly per Julian year) falls
    into the black hole at its next pericentre (Shapiro and Teukolsky 1983, _Black Holes, White Dwarfs
    and Neutron Stars_, §12.4; the marginally bound orbit at 4GM/c², Bardeen, Press and Teukolsky
    1972, ApJ 178, 347, eqs. 2.13 and 2.19). Its integration stops, and its class's record counts
    it as captured, with its time; it takes no histogram cell, and every share keeps it in its
    denominator, so the class table counts it as gone. No other relativistic term: the first
    post-Newtonian corrections are of order GM• ÷ (c² r), under 10⁻³ outside 10⁻³ ly, and the
    histograms begin at 16 ly. No tidal disruption: the table's objects are compact remnants, and
    even a white dwarf's tidal radius about Sgr A* (about 1.7 × 10⁹ m for 0.6 M☉) lies inside the
    horizon (1.27 × 10¹⁰ m). (Corrected 2026-10-13 from P15.T6.g's science check, with the
    equations cited: the order read r_s ÷ r, which is 1.3 × 10⁻³ at 10⁻³ ly, and the radius
    1.4 × 10⁹ m.)
  - **Unresolved orbits.** An orbit still over `drift_tolerance` after its last halving is not
    classified: it takes no histogram cell, no bound or unbound count and no place in any share's
    numerator or denominator, and its class's record counts it as unresolved, with its drift and its
    closest approach to the centre. Every share is therefore of the class's resolved orbits, those kept
    within the tolerance and those captured. Production allows at most 10⁻⁴ of each class unresolved
    (21 of a disc class, 20 of an old one, 10 of a nuclear-disc one), and T6.c–f refuse histograms
    over it; a class over it is rerun with `max_halvings` six, and one still over goes to a decision
    agent, not to a waiver. The smoke run records its counts without the limit. The table's header
    gives each class's captured and unresolved counts.
  - **Acceptance.** T6.a's, on the new method: the energy without the bar to 10⁻⁴ relative over
    10 Gyr for its 1,000 test orbits on the first pass, and the Jacobi integral with the bar to 10⁻⁴
    for its 1,000 on the checked integration (1 ÷ 200, 10⁻⁴, six halvings), at most 20 refined, with
    the first pass's worst reported; (all on the drift's scale above; the figures are restated under it); orbits 368 and 740 of the barred set asserted by number; circular
    orbits at 3,000–50,000 ly to 10⁻³ in radius; one and four threads the same bytes. New: 16 in-plane
    orbits, released at zero radial speed from 300 and 3,000 ly with a tangential speed of 10⁻², 10⁻³,
    10⁻⁴ and 10⁻⁵ of v_c, in the plain and the barred potential, each hold 10⁻⁴ over 10 Gyr on the
    checked integration or are captured; an orbit released at 0.5 ly with |x × p| 0.9 of 4GM•/c is
    reported captured, and one with 1.1 of it is not; the barred set's median first-pass drift, on the drift's scale, falls 3–5 fold per halving of the
    step fraction from 1 ÷ 50 to 1 ÷ 400, a leapfrog's second order predicting 4
    (decision-p15-t6g-orbits §3: 4.11, 3.81 and 4.11, each at least 3.4 bootstrap standard deviations
    inside the bracket); the worst orbit's falls are reported, not asserted, since a different orbit is
    worst at each step. Re-time the 4,950-orbit
    reduced run and restate the production run's hours in T6.b's record. (The decision's scratch
    measurement, at version 21's potential: on the barred set the checked integration kept at most
    8.6 × 10⁻⁵ with 3 refined, orbits 368 and 740 at 3.5 and 5.5 × 10⁻⁵ on the first pass, the plain
    set's worst 6.5 × 10⁻⁵, and 0.59 of the fixed step's steps at about 1.6 times the time a step.)
    Before the production run, rerun the 4,950-orbit reduced run on the drift's scale and with
    P02.T6.f; it must leave at most 10⁻⁴ of each class unresolved.
  - **Then re-emit the provisional table** (the triage's F1; the smoke pin c07edb86 left stale). Run
    `cargo run --profile slow-test -p hyperion-fit -- orbits --smoke --out
crates/hyperion-fit/data/cache/displaced_smoke`; update `data/displaced_smoke/PROVENANCE.toml`'s
    `sha256`, `retrieved` and `release` ("rerun with P15.T6.g's time-transformed leapfrog, in the
    potential of P11.T4.h's early-AGB remnant blend"); re-emit `tables/displaced_forms.rs` with
    `hyperion-fit run displaced_forms` (which writes `tables.lock`); re-bless the goldens that read
    `galaxy::displaced` at header 21 and check `golden_diff`; then run
    `the_smoke_fit_runs_end_to_end_on_fresh_smoke_histograms`,
    `energy_and_jacobi_hold_over_ten_gyr_for_a_thousand_orbits`, the new tests and `just fit-check`,
    and record the acceptance line's move in this plan's Risks. The integrator and the re-emission are
    the task's two commits.
  - **Generator version.** The task moves generated output through the re-emitted table only, so it
    lands with the 21 → 22 batch: committed with no bump, held out of integration until p14-batch's
    bump (as c07edb86 was for 21), and cherry-picked just before the bump commit, so that the goldens
    are re-blessed once at 22. If it is not through its gate and review when that bump is otherwise
    ready, the batch goes without it, and the task, re-emission included, waits as one unit for
    22 → 23. The re-emission never lands alone: the integrator changes every smoke orbit, and the
    manifests' change makes the committed table stale for `fit-check`.
  - **Files.** `src/tasks/displaced_forms/{orbits,mod,histogram}.rs` (the integrator, the capture, the
    record's captured and unresolved counts), the shares' readers in
    `src/tasks/displaced_forms/{table,fit_disc,fit_old,kinematics}.rs`,
    `manifests/displaced_orbits{,.smoke,.one_percent}.toml`, `data/displaced_smoke/PROVENANCE.toml`,
    `crates/hyperion-sim/src/tables/displaced_forms.rs`, `tables.lock`.

**Files.** `src/tasks/displaced/{mod,orbits,births,histogram,fit_disc,fit_old,kinematics}.rs`,
`manifests/displaced.toml`, `manifests/displaced.smoke.toml`, `tables/displaced_forms.rs`.
Re-validated at 898a3c94, as T6.a–f built them: under `src/tasks/displaced_forms/`, `mod`,
`orbits`, `births`, `histogram`, `table`, `fit_disc`, `fit_old`, `kinematics`, `hypervelocity`,
`cells` and `nelder_mead`; the orbit run's `manifests/displaced_orbits{,.smoke,.one_percent}.toml`,
the fit's `manifests/displaced_forms{,.smoke,.one_percent}.toml`, `data/displaced_smoke/` and
`data/displaced_one_percent/`, and `tables/displaced_forms.rs`; the production run adds
`data/displaced/`.

No scratch stand-in exists for this table. It is the long pole of M3 and starts as soon as plan 02's
potential is numerically stable, in parallel with M2.

**T6.a–b as built (lane `disp08`, 2026-09-26, at `GENERATOR_VERSION` 12; no table yet).**

- **Files and names.** `src/tasks/displaced_forms/{mod,orbits,births,histogram}.rs` (the task is
  named for its table, `displaced_forms`), `manifests/displaced_forms.toml` and
  `displaced_forms.smoke.toml`. The task is not in the registry: a `FitTask` returns a table, which
  T6.c–f fit. Until then `hyperion-fit orbits [--smoke | --manifest PATH] [--threads N] [--out
DIR]` runs the orbits and writes `displaced_orbits.txt` (text, integer counts and shortest
  round-trip floats) to `data/cache/displaced/` or `--out`, printing its SHA-256 for
  `tables.lock`.
- **The force (ruling 101.4).** Plan 02's potential gained `PotentialTables::force` (the exact
  gradient of the (R, |z|) grid's bicubic Hermite interpolant, so the field is conservative) and the
  direct `MassModel::force`, returning `CylindricalForce { radial, vertical }`. The integrator
  steps with the tables' force, not bilinear force interpolation, which is not a gradient.
- **T6.a, the bar and the splitting.** The bar is Dehnen's (2000) quadrupole in Monari et al.'s
  (2016) three-dimensional form, `A U(r ÷ R_b)(x′² − y′²) ÷ r²`, `A = bar_strength × v_c(R_b)²`,
  `R_b` the fixture's half-length, `Ω_p` the sim's pattern speed (or the manifest's
  `corotation_ratio`). Inside `R_b`, Dehnen's `s³ − 2` is direction-dependent at the centre (a force
  growing as `1 ÷ r`, which threw test orbits by 20% in the Jacobi integral), so `U(s) = 5s³ − 6s²`,
  the cubic meeting `−s⁻³` with value and slope at `s = 1`: HYPERION's regularisation, a finding for
  the owner only if T6.d's shares depend on it. The rotating frame's step is `K(h ÷ 2) R(h) D(h)
K(h ÷ 2)`: the kinetic and rotation terms commute, so their joint flow (drift, then rotation by
  `−Ω_p h`) is exact, and the Strang splitting is symplectic with one force a step. Each orbit's
  fixed step is 1 ÷ 200 of the circular period at its pericentre (from its energy and angular
  momentum in the plane's potential), at least `min_step_years`: "the local circular period" read
  as the orbit's, since a step that followed the radius would not be symplectic.
- **T6.a's acceptance.** The test orbits run from 200 ly to 40,000 ly, every fourth in the
  nuclear disc, bulge and bar. Without the bar the energy holds to 4.7 × 10⁻⁵ over 10 Gyr for all
  1,000 (slow; 20 in the fast suite). With the bar (strength 0.05) the fixed step of the first pass
  holds 982 of 1,000, but one inner orbit torqued onto the black hole lost 79% of its Jacobi
  integral: so each orbit is integrated through `integrate_checked`, again from its start at half
  the step while its Jacobi integral has drifted by more than `drift_tolerance` (10⁻⁴), up to
  `max_halvings` (6) times. 18 of 1,000 needed it and the worst kept is 8.7 × 10⁻⁵ (at version 15's
  potential, ruling 140.7: 17 refined, the worst kept 9.93 × 10⁻⁵, the first pass's worst 0.377;
  the fast suite's 20 barred orbits are checked the same way, not on the first pass). Circular orbits
  at 3,000–50,000 ly stay circular to 10⁻³ over 10 Gyr; one and four threads give the same bits.
  **Superseded as the method by P15.T6.g** (decision-p15-t6g-orbits, 2026-10-12): from version 16's
  potential on, orbits 368 and 740 never converged under the halving (1.41 and 0.143 after six),
  and the 14 refined orbits that did converge did so by chance (slow-pass 2 triage).
- **T6.b's births** follow the text, with one change: the halo, bulge and bar draw their velocities
  from plan 08's velocity ellipsoid of the component they were born in, not an isotropic Jeans
  dispersion, since plan 08's laws exist (so T6.e's "plan 08's births" is this run). Old sources
  are born by exact rejection on logarithmic cells of the cube's octant (each envelope never rises
  with |x|, |y|, |z|); the thin source in the thin birth layer (holed thin radial profile, the young
  disc's vertical profile), its time since death from the thin history restricted to the age bin;
  the old sources' from their populations' histories. Kicks are log-uniform within each speed bin
  (0.02–6 over the eight). An unkicked control per barred source serves T6.d. An orbit is bound if
  its epoch energy cannot reach the sim's escape boundary, twice `r₂₀₀` (ruling 91).
- **T6.b's histograms**: per class, the bound and the unbound inside the cube on 48 × 40 log cells
  in R (16 ly to 2¹⁸) and |z| (1 ly to 2¹⁶), counts inside and outside, the bound members'
  velocity moments in `v_c` (`v_φ` spinward) and outbound count, a 24 × 24 × 20 bar-frame histogram
  for the barred sources, and the steps taken. The smoke run twice, and on one and three threads,
  gives the same bytes. The smoke manifest holds 1–3 orbits a class on coarse steps, not the
  plan's 1% (about 40 minutes on three threads), so that the fast suite can run it three times;
  the 1% run is the production manifest with its counts divided by 100.
- **T6.e's baseline.** With plan 08's velocity laws already in T6.b's births, T6.e's check that the
  table "changes by under 1 point with plan 08's births" needs the other run: a tenth-size run
  with isotropic Jeans births for the halo, bulge and bar, which T6.e adds.
- **The production run is not run: about 100 hours on three threads.** A reduced run of 50 orbits
  per class (4,950 orbits, the production manifest with the counts cut) took 98 s on three threads
  (niced, under the heavy-test lock, 2.8–3.3 GHz, load 5–10): 5.6 × 10⁸ steps, 526 ns a step and
  thread. Scaled by class to the manifest's counts the run is 2.1 × 10¹² steps, about 103 hours:
  the nuclear disc 71.5% (a few × 10⁵ steps an orbit, its periods being a few Myr over 10 Gyr, and
  its barred orbits refining most), the bulge 12.0%, the bar 6.3%, the halo 4.2%, the thick disc
  4.1%, the thin disc 1.9%. The production manifest keeps a Jacobi drift of 10⁻³ and three
  halvings: T6.a's 10⁻⁴ and six halvings put a 200-per-class run at 5.2 × 10⁹ steps against 1.2 ×
  10⁹ without refinement. Command: `nice -n 19 cargo run --profile slow-test -p hyperion-fit --
orbits --threads 3` (writes `crates/hyperion-fit/data/cache/displaced/displaced_orbits.txt`, whose
  SHA-256 it prints for `tables.lock`). Each class's record counts its orbits integrated again at
  a smaller step, those kept still over the tolerance after the last halving, and the worst drift
  kept, so that T6.c–e can judge the histograms by them (**for T6.c**: the nuclear disc's classes
  refine most); 10⁻³ is acceptable for production (decision-p15-t6g-orbits: it moves an apocentre
  by a few thousandths in ln r, against histogram cells 0.20 wide). A lever, for the owner: the
  nuclear disc's classes at a tenth of the count (still 2 × 10⁴ a class) would cut it to about 37 hours. The production run starts only from a commit holding P15.T6.g and P02.T6.f
  (decision-p15-t6g-orbits §2), so that its committed histograms can be regenerated from committed
  code. P15.T6.g restates the run's hours (about 25 on three threads) under ruling 120.3–4's
  estimate below.

**Ruling 120.3–4 as built (lane `disp08`, 2026-09-27; the production run not started).**

- **The manifest.** The nuclear disc takes `nuclear_orbits_per_class` = 10⁵ (other sources keep
  their counts), drift tolerance 10⁻³ with three halvings, and each class's record counts the
  orbits refined and those still over the tolerance, with the worst drift. `nuclear_step_fraction`
  stays 1 ÷ 200: a reduced run of 150 orbits in each of the nuclear disc's eight classes refined
  15.2% of them at 1 ÷ 200 and 22.3% at 1 ÷ 100 (47% more, against the ruling's 10%; read as
  7.1 points it would pass), and kept 13 against 24 over the tolerance, for 0.65 of the steps.
- **Resumable parts.** The run is cut into parts of `part_orbits` (4,096; 4,703 parts), each
  computed in the manifest's chunks and written whole (to `part-NNNNNN.txt.partial`, then renamed)
  under the output directory's `parts/`, headed by the part's number and the manifest's SHA-256; a
  later invocation reads the finished parts back (exact: counts, and floats in shortest round-trip
  form) and computes the rest, and the totals are merged in part order, so the bytes do not depend
  on restarts, threads or `--max-parts`. A part of another manifest is refused. The smoke test
  stops after one part, then after two more, then finishes, and compares bytes with the
  uninterrupted run.
- **Estimate: about 61 hours on three threads.** The nuclear disc's 8 × 10⁵ orbits take 8.4 × 10⁵
  steps each (the reduced run: 1.0 × 10⁹ steps for 1,200 orbits in 171 s, about 510 ns a step and
  thread, niced, load 6–11): 32 hours. The other sources' 6.1 × 10¹¹ steps at 526 ns: 29 hours. At 1
  ÷ 100 the nuclear disc would take 21 hours, 50 in all. Command, run under nice from the
  repository root and restartable at any point:
  `nice -n 19 cargo run --profile slow-test -p hyperion-fit -- orbits --threads 3`; add
  `--max-parts N` to stop after N parts, and run the same command again to resume. It writes
  `crates/hyperion-fit/data/cache/displaced/displaced_orbits.txt` and prints its SHA-256 for
  `tables.lock`. **Restated by P15.T6.g** (below): about 25 hours on three threads, 3.2 × 10¹¹
  steps.

**T6.c–f as built (lane `disp08b`, 2026-09-27, at `GENERATOR_VERSION` 13; the table is
provisional, fitted to the smoke histograms; no output moves).**

- **The task.** `tasks::displaced_forms::DisplacedFormsTask` (slow, registered), modules
  `displaced_forms/{table,fit_disc,fit_old,kinematics,hypervelocity,cells,nelder_mead}.rs`; the
  plan's `orbits`, `births` and `histogram` are T6.a–b's. The orbit run's manifests are renamed,
  bytes unchanged (so the production run's parts and header hash still match):
  `manifests/displaced_orbits.toml`, `displaced_orbits.smoke.toml`, and a new
  `displaced_orbits.one_percent.toml` (the production counts over 100). `displaced_forms.toml` and
  `displaced_forms.smoke.toml` are now the **fit's** manifests: one dataset, an orbit run's
  histograms, declared `fetched` with a `PROVENANCE.toml` (`data/displaced_smoke/` now; the
  production run's `data/displaced/` when it lands, whose default directory is the orbits
  command's own `crates/hyperion-fit/data/cache/displaced/`), and `orbit_manifest`, whose SHA-256 the histograms'
  header must carry. `histograms = "smoke" | "one_percent" | "production"` decides `@provisional`.
  The Nelder–Mead, the cell quadrature and the misplaced share live in the task's modules, not in
  the plan's `hyperion_fit::{optimise, stats}` (nothing else uses them yet).
- **Table shape.** As the format above, with names from the sim: rows are
  `forms::{FlaredLayerParams, CoredPowerLawParams, ClassKinematics, DiscBornRow, OldBornRow}`
  (`DiscBornForm` is taken by P08.T10's density), and `forms::ESCAPE_RATIO_NODES` and `at_nodes`
  are the node arrays' reading. **`OldBornRow` gains `unbound_in_cube`**, which Design note 23 of
  plan 08 needs for every source. A sim test holds the table's edges and nodes to the module's.
- **T6.c.** Each class's bound in-cube (R, |z|) histogram is fitted (the fastest speed bin's
  with every speed bin's unbound-in-cube members of its age bin added, where plan 08 puts them; so
  too the thick disc's and halo's fastest; the barred sources' bar-frame histograms record only
  the bound, so theirs are fitted without) by the total-variation
  distance to the forms' cell integrals over the cube (Gauss–Legendre in ln of each cell's
  coordinate, the corner annulus beyond L in the arc's angle; cells tile the cube to 10⁻⁶). The
  boxes are the plan's, with `h_r` in [0.05, 50] and `h_0` in [10⁻⁴, 20] `R_d` added; the
  parameters move as `lo + (hi − lo)(1 + sin t) ÷ 2` (log scale where positive) so an edge is
  reachable and detectable; a spheroid under 0.05 is dropped and the layer refitted alone; the
  row penalty is λ Σ (ln θ − ln θ′)² in Gauss–Seidel sweeps, the largest λ of the manifest's
  ladder (10⁻⁴–10⁻¹) whose weighted mean rises ≤ 0.3 points. **Weights** of the weighted mean are
  the neutron stars' band-averaged speed-bin shares (P08.T8.b as built, in the manifest) × the
  thin history's share of each age bin × the class's in-cube share. **The noise floor** is the
  expected misplaced share of Poisson counts from the fitted model (`E|X − λ|` exactly), since
  the summed histograms have no half-samples; the table's notes list misplaced ÷ floor per class.
  The smoothness check is reported as a count of second-difference sign changes.
- **T6.d.** Thick disc and halo: one spheroid a speed bin, boxes a [0.005, 20] `R_d`, q [0.02,
  1], γ [0.5, 10] (the nuclear disc's core is a few hundredths of `R_d`). Bulge, bar, nuclear
  disc: `s ×` the field population's own density (plan 08 places the retained share with it)
  plus a spheroid, on the 24 × 24 × 20 bar-frame histogram (the bar lies along x in the fields
  and in the orbits' frame at the epoch). Elongation is `1 − √(⟨y²⟩ ÷ ⟨x²⟩)` within two bar
  half-lengths, over the control's. **One run, one corotation ratio (the fixture's 1.24) and one
  bar mass: the measured share fills all three nodes**; the runs at 1.0 and 1.4 and a second bar
  mass need the orbit command to run the barred sources alone, which it cannot yet.
- **T6.e.** Kinematics and in-cube shares straight from each record; **one halo mass, so the
  measured value fills all three escape-ratio nodes.** Not built: the four universality
  potentials, the isotropic-Jeans baseline, the other halo masses; the kick-law reweighting of
  "13–14% unbound" cannot be read from the histograms, which count the unbound only inside the
  cube (so plan 08's class table offers only `unbound_in_cube_share`; the total is P08.T14.4's, from the kick law against the escape speed), and "phase mixing by τ 8–30" needs the
  last age bin split at 30. These are for the production follow-up.
- **T6.f.** The sources are every field component weighted by its mean `t^−1.1` from 40 Myr over
  its age distribution (Maoz and Graur 2017), a reading of "the old populations' density" that
  gives each population its ancient events; the azimuthal kernel is closed (`2π ÷ √(A² − B²)`),
  softened at 1% of the source's distance. Rate 0.54 per century (Li et al. 2011), channel 0.3,
  2,200 km/s: 17,365 inside the cube, 10.7 Myr each; misplaced 5.3% (acceptance 10⁴–10⁵ and
  under 10%). **Under ruling 128.1** (lane `disp08b`, 2026-09-28): the two populations and the
  rate are the sim's `class_table::{SURVIVOR_POPULATIONS, MILKY_WAY_IA_RATE_PER_YEAR}`, held by
  the fingerprint, not manifest parameters; the table gains `HYPERVELOCITY_MEAN_EXIT_LY`, the
  rate-weighted mean path to the cube's face (**78,662 ly**), from which the class table computes
  each population's count. **29,123 inside: 26,849 slow (1.5–4.5 × 10⁴) and 2,273 fast
  (1,500–3,500)**; misplaced still 5.3%; **0.017** of slow launches leave below 1.5 times the
  local escape speed (the full potential's), a finding: straight lines serve all but a fiftieth.
- **The 1% run as a check** (`displaced_orbits.one_percent.toml`, 192,608 orbits, 1.8 CPU-hours;
  fitted with `displaced_forms.one_percent.toml` to a scratch file, never committed). At this size
  every fit sits at its noise floor (misplaced ÷ floor 1.0–1.7), so the figures are preliminary
  **findings for the production run to confirm, not rulings**: disc-born weighted mean misplaced
  0.202 (floor-limited, against ≤ 0.07; λ 10⁻³); 6 classes over 10⁻⁴ of the weight with a
  parameter on a box edge; worst old-source misplaced thick 0.84, halo 0.81, bulge 0.89, bar 0.93,
  nuclear 0.61 (bar-frame cells of 2,000 orbits); **own-form shares at the fixture's ratio 1.24:
  bar 0.15, 0.11, 0.04, 0.00 against the brainstorm's 0.95, 0.61, 0.20, 0.05; bulge 0.30, 0.34,
  0.05, 0.00 against 0.91, 0.76, 0.55, 0.25; nuclear disc 0.62, 1.00, 0.00 against 0.88, 0.67,
  0.43** (outside ±0.08 in every bin but noise-limited: the fit trades a noisy own form for a smooth
  spheroid at no cost in misplaced share); bar elongation over the control 0.75, 0.59, 0.61, 0.26
  over the first four bins (plan 0.99, 0.79, 0.5, 0.25 within 0.05), 0.16, −0.00, −0.04, 1.28
  beyond (plan < 0.05), length within 0.68 of the control's (plan 10%); the slowest thin class's
  mean rotation 0.947 `v_c` (plan above 0.95), the fastest's −0.599 (below zero, as planned); **the
  neutron stars' half-density height at 26,000 ly 155 pc against 110–130**.
- **Cost.** The provisional fit is 3.6 CPU-minutes (2.6–5.3 minutes wall on three threads under
  load 13–18); the 1% fit 7.4 CPU-minutes; the production fit costs the same per evaluation, so
  about as long.
- **Rerunning on the production histograms:** the steps are in `table.rs`'s module documentation
  (copy the file, add `data/displaced/PROVENANCE.toml`, point the manifest's three lines at
  production, `just fit displaced_forms`, rerun plan 08's form, bound and class-table tests and
  re-bless the class table's golden).

**T6.g as built (lane `p15-orbits`, 2026-10-12, at `GENERATOR_VERSION` 21; held for the 21 → 22
batch with its re-emission, which waits for P02.T6.f).**

- **The integrator** (`orbits.rs`).
  - `OrbitPotential::new(tables, black_hole, bar)` takes the black hole's mass for the capture
    rule, since `PotentialTables` does not expose it.
  - `CircularFrequency` is the ln Ω table: 512 knots, 40.5 a decade, from ln 10⁻⁶ to ln (4 × 10⁶)
    ly. Its `Ω′` is the interpolant's own derivative.
  - `integrate(state, duration_years, fraction) -> Pass` and `integrate_checked(state,
duration_years, fraction, tolerance, refinements) -> Checked` take the step fraction, not a
    step. `step_for`, its pericentre bisection and `RunParams::min_step_years` are gone.
    `RunParams::from_manifest` refuses a step fraction or tolerance that is not positive and
    finite.
  - A pass ends `Ending::{Aged, Captured { after_years }, Broken}`. `Broken` comes from a step
    that is not positive, or at once from a duration that is not finite. It has never been seen
    in a run, and its drift counts as infinite, so that no pass can loop.
  - A step shorter than half the clock's last digit loses its time to rounding. This happens late
    in a run, at pericentres near 5 × 10⁻⁶ ly. It is harmless, and it is not treated as `Broken`:
    a guard that did so broke a legitimate stress-orbit pass.
  - The capture rule is checked at the start and after every step but the plain one of the time
    left.
  - A captured pass's drift is its drift at the capture, so a capture is kept only from a pass
    within the tolerance. Both stress orbits from 300 ly at 10⁻⁵ `v_c` take one halving for this:
    2.1 × 10⁻⁴ mid-plunge at 1 ÷ 200.
  - `OrbitPotential::{drift, drift_scale}` measure every drift on §2's scale. `Checked` reports
    the first pass's drift and the kept pass's closest approach.
- **The record** (`histogram.rs`).
  - `over_tolerance` becomes `unresolved`, with `unresolved_drift` and `unresolved_closest_ly`
    (the class's worst and closest, aggregated), beside `captured` and `captured_years` (the sum
    of the times from birth to capture).
  - `ClassHistogram::resolved()` is the share denominator. Each class line has 34 tokens, and the
    reader refuses a record whose outcomes do not add up to its orbits.
  - `kinematics::in_cube` and `fit_disc`'s class weights divide by `resolved()`; `fit_old` reads
    no share.
  - `table::check_unresolved` and `UNRESOLVED_LIMIT` (one in 10⁴, `orbits / 10_000`, rounded down)
    refuse every histogram but the smoke run's. The 1% run (2,143, 2,000 and 1,000 a class) and a
    50-a-class reduced run therefore allow none.
  - The table's notes name each class with a capture or an unresolved orbit, and its acceptance
    line gives the totals. Its source line cites Mikkola and Aarseth, and Bardeen, Press and
    Teukolsky with Shapiro and Teukolsky for the capture.
- **Tests.**
  - The one-and-four-threads test runs the barred potential, so it also covers the rotation and
    the Ω table.
  - The capture test's orbit at 0.9 of 4GM•/c from 0.5 ly is captured at its start. A third orbit,
    at 0.9 from 1.5 ly, is captured on its way in.
  - The stress test holds every kept pass to 10⁻⁴, captured or not.
  - New fast tests: the drift's scale, a pass that cannot step, the manifest's positive fractions
    and the unresolved limit.
- **Measured on the drift's scale** (the slow tests, at version 21's potential, before P02.T6.f).
  - Plain: worst 6.08 × 10⁻⁵.
  - Barred: first pass 1.29 × 10⁻⁴, kept 8.55 × 10⁻⁵, 3 refined, 7.161 × 10⁷ steps, closest
    approach 3.9 ly. Orbits 368 and 740 at 3.44 and 5.48 × 10⁻⁵ on their first pass (closest 19.8
    and 14.1 ly).
  - The median first-pass drift is 6.33 × 10⁻⁵, 1.54 × 10⁻⁵, 4.04 × 10⁻⁶ and 9.84 × 10⁻⁷ at 1 ÷ 50
    to 1 ÷ 400: falls of 4.11, 3.81 and 4.11 (§3's figures). The worst orbit's falls are 3.70,
    6.06 and 3.63.
  - The 16 stress orbits all hold. The two from 300 ly at 10⁻⁵ `v_c` are captured on their first
    infall (after 0.97 Myr). Four need one halving, those two among them.
  - The capture test's orbit at 1.1 of 4GM•/c passes 6.5 × 10⁻⁶ ly a thousand times in 10⁶ years,
    with a drift of 2.6 × 10⁻⁵.
  - On the old |J₀| scale, the decision's §1 figures were reproduced bit for bit: 6.495 × 10⁻⁵,
    1.308 × 10⁻⁴, 8.552 × 10⁻⁵, 3 refined, 3.47 and 5.53 × 10⁻⁵. The exception is 1 ÷ 400's worst,
    3.60 × 10⁻⁵ from the table against the decision's 3.65 × 10⁻⁵ from Ω directly.
  - Fast-suite cost, provisional (dev profile, one test alone, load about 33): the smoke-run test
    39 s, the 20-orbit test 11 s. Integration's last record of T6.a's versions was 92 s and 35 s.
- **The reduced run re-timed** (4,950 orbits: the production manifest at 50 a class, three threads,
  niced, at the lock-held setting; provisional, before P02.T6.f).
  - On the drift's scale: 38 s at load 12; 1.453 × 10⁸ steps (T6.b's run took 5.6 × 10⁸); 785 ns
    a step and thread; 54 refined, none unresolved, none captured.
  - Scaled by class to the production counts: 3.17 × 10¹¹ steps. The nuclear disc takes 58%, the
    bulge 15%, the bar 9%, the thick disc 8%, the halo 6% and the thin disc 4%.
  - **About 23 hours on three threads** at that rate, against ruling 120.3–4's 61. A timed rerun
    under the heavy lock is pending.
  - **§2(c)'s check, on a base holding P02.T6.f** (lane `p15-orbits`, 2026-10-13, P02.T6.f's
    47ba6178 picked in; provisional timing): 41 s at load 12, 1.453 × 10⁸ steps, 847 ns a step and
    thread; 54 refined, none unresolved and none captured, so every class is within the limit of
    one in 10⁴. The worst drift kept is 9.85 × 10⁻⁴, in `thin_s7_a6`. Scaled to production: 3.17 ×
    10¹¹ steps, about 25 hours on three threads. The production run may start from a commit
    holding P15.T6.g and P02.T6.f.
- **The unresolved limit, ruled** (decision-p15-t6g-orbits §2, 2026-10-12).
  - On the old |J₀| scale the same run left 13 orbits unresolved and 456 refined. All 13 were fast
    kicks (speed bins 4–7) with closest approaches of 190–26,000 ly, 1–4 of 50 in the bin-6
    classes; with six halvings 5 remained.
  - Five were diagnosed. They were marginally bound or unbound, with J₀ of −541, −24, +947, +252
    and −348 (km/s)² against |Φ| ≈ 1.6 × 10⁵ at the Sun, and their absolute drift was 1–2 (km/s)².
  - The escapers' drift stopped falling where `PotentialTables::forces` leaves its grid at 2¹⁸ ly
    in one coordinate, where it is not its potential's gradient. P02.T6.f corrects that.
  - §2 ruled the drift's scale above, P02.T6.f for the far field, and a production run only from a
    commit holding both.
- **For main, not ruled.**
  - §2's text and the manifests' first draft gave `|Φ₀| ÷ v_c²` as 3–9, so that 10⁻³ bounds
    δ ln r by about 0.009. The fixture's `galaxy_potential` golden gives 3 at the Sun, 9 at
    3,260 ly, 14 at 1,000 ly and 30 at 100 ly, so the bound is 0.003–0.03, still under a seventh
    of a cell. The code and the manifests now say so.
  - The P15.T6.g bullet's "r_s ÷ r, under 10⁻³ outside 10⁻³ ly" holds for GM•/(c² r); r_s ÷ r is
    1.3 × 10⁻³ at 10⁻³ ly.
  - Its white dwarf's tidal radius, 1.4 × 10⁹ m, is about 1.7 × 10⁹ m for 0.6 M☉. That is still
    inside the horizon.
  - The science check could not see Shapiro and Teukolsky's §12.4, so Bardeen, Press and Teukolsky
    (1972, eqs. 2.13 and 2.19) is now cited beside it.
- **Landed in the 21 → 22 batch** (lane `p14-bump`, 2026-10-14). F1 was emitted at `--since 21`
  on the lane and restamped to 22 by the 21 → 22 bump commit (decision-displaced-since); the body
  is F1's. The smoke orbits were made again on the bump tree (`displaced_orbits.txt`'s SHA-256
  88db652d…, F1's), the table re-emitted by the prebuilt tool at `--since 22` with only the three
  `since` fields moving (the header, `tables.lock` and the `MANIFEST` row), and `fit-check` finds
  it "provisional, since generator version 22" with no failure. `PROVENANCE.toml` keeps its
  "at GENERATOR_VERSION 21", where the data was made.

### P15.T7 Helium correction

**Source.** The brainstorm names none beyond "fitted offline". The fit needs published
helium-enhanced stellar models reaching Y of about 0.43 at globular-cluster metallicities. T7.a
selects the grid and records it; see [Risks and open points](#risks-and-open-points).

**Consumer.** Plan 06's hook P06.T17 (`tables::helium`, committed there as the identity, every
coefficient zero); plan 09 supplies ΔY for second-population members (M3). **Format:** plan 06's, as
its Provides and P06.T17 give it: `LIFETIME_SLOPE`, the coefficients of s(m, Z), a quadratic in log
mass at four metallicities interpolated in log Z, which multiplies the main-sequence and
giant-branch timescales by exp(s × ΔY); and `HB_TEMPERATURE_SHIFT`, linear in envelope mass, which
shifts log T_eff on the horizontal branch by its value × ΔY at constant luminosity. The first is the
brainstorm's correction to lifetimes; the second sets the blue end of the horizontal branch. If the
grid shows that the tip core mass must move too, that is a change of shape and goes to plan 06.
(Re-validated at 898a3c94: the file holds `LOG_Z_NODES: [f64; 4]`, the log₁₀ Z of the four
metallicities, [Fe/H] −2.2, −1.6, −1.0 and −0.5; `LIFETIME_SLOPE: [[f64; 3]; 4]`, the quadratic's
coefficients in log₁₀ m at each of them; and `HB_TEMPERATURE_SHIFT: [f64; 2]`, in envelope mass;
all zero. The track reads them through `stellar/sse/track/excess.rs`, whose `HeliumHook::of` builds
no hook at ΔY ≤ 0, so grid stars are untouched by the code path, not only by the form. The file
keeps P06.T17's hand-written header, with no `MANIFEST` row or lock entry, as P15.T2 left it: T7.b
registers it.)

- **P15.T7.a Select and record the source** and its terms, as a _committed_ or _fetched_ dataset.
- **P15.T7.b Fit.** Least squares on the grid's turn-off ages and zero-age horizontal-branch
  temperatures relative to plan 06's tracks at ΔY = 0. **Acceptance:** lifetime ratio reproduced
  to 3% for 0.6–0.9 M☉, ΔY up to the grid's reach (about 0.15; "The grid's reach" below) and
  [Fe/H] from −2.2 to −0.5; every correction exactly the identity at ΔY = 0 by its form, so grid
  stars stay bit-identical (plan 06's golden); ΔY = 0.1 shortens the lifetime of a 0.8 M☉ star by
  about a third (0.25–0.4), the sign plan 06 documents; a synthetic cluster of 10⁶ M☉ with ΔY up
  to 0.18 shows a horizontal branch whose blue end, at the grid's top ΔY, is hotter than its first
  population's by the grid's amount to 10%. Re-validated at 898a3c94:
  - _The data._ `data/basti_helium/` holds only `PROVENANCE.toml` and `urls.txt`, though the
    provenance says the reduction script and its output are committed, and the T7.a record's
    research notes are not in the repository. So T7.b first writes the script and its reduced
    output there, from the raw files in `data/cache/basti_helium/`, reading only log age, mass,
    log L and log T_eff, as the provenance says: the turn-off ages of the 0.6–0.9 M☉ tracks and
    the zero-age horizontal branch's temperatures by envelope mass, at each Z and Y. With that
    output committed the task runs where the raw files are absent, so `--rerun-fast` reruns it
    (the Schedule's `fast`), unlike `wd_cooling`.
  - _The grid's reach_ (ruled 2026-10-23, decision-p15-revalidation Q6). Y = 0.30, 0.35 and 0.40
    (at Z = 0.0198, 0.35 and 0.40 only) against a canonical Y of 0.245–0.256 over Z from 10⁻⁴ to
    the value 0.008 (`urls.txt`'s `zahbz104y245aes` to `zahbz803y256aes`): ΔY reaches about 0.15,
    while the generator draws up to 0.18 (`features::interior::abundances::HELIUM_EXCESS_MAX`). The
    acceptance asserts to the grid's reach. The correction is not clamped: both forms are smooth
    in ΔY, so 0.15–0.18 is a 20% extrapolation, and the header's acceptance line states the
    fitted lifetime factor and shift at 0.18 beside the trend of the grid's own ratios.
  - _The hook._ `HeliumTable`, `HeliumHook` and `Track::build_with_helium` are crate-private, so
    `hyperion-fit` cannot build a track under a candidate table, which the synthetic cluster's
    figure for the header needs. T7.b first exposes such a builder behind the sim's `testing`
    feature, which `hyperion-fit` enables, as P06.T31 does for `stellar::testing`; no output
    moves.
  - _Other fitted tables_ (ruled 2026-10-23, decision-p15-revalidation Q7). Rendering plan R06
    builds `sky_envelope` and `sky_phase_envelope` at ΔY = 0 (R06's "T6.b's fitted envelope, as
    built" and "Deviations in T8.m, as built"), and their fingerprints probe ΔY = 0 alone, so
    `just fit-check` will not flag them. T7.b refits neither, and the phase envelope gains no
    helium axis, so its storage needs no ruling:
    - no star the sky's census reads has a helium excess until R06.T16.a brings feature members
      in: a grid record's metallicity draw has none (`explosion_site` asserts it), and feature
      members are `not_modelled` in every sky reply;
    - the phase envelope's reader gives a star with a helium excess the table's brightest value,
      which skips nothing, so it stays a bound under any `tables::helium`;
    - the brightness envelope reads no composition, so whichever of T7.b's swap and R06.T16.a
      lands second refits `sky_envelope` with helium-excess nodes in its build, up to
      `HELIUM_EXCESS_MAX`, in its same 198 × 193 storage, bumps that task's revision and adds a
      ΔY > 0 probe to its fingerprint.

    T7.b's swap states every fitted table built at ΔY = 0 that a helium-rich star can reach, with
    its guard: the fates, plan 11's reach and pair light, plan 08's lifetime bracket, and the two
    sky tables, as above.

  - _Landing_ (decision-p15-revalidation Q9). The fitted body moves the tracks of every cluster
    member with a helium excess, so it is a held swap. T7.b first lands what moves nothing: the
    reduction script and its output, the `testing` builder, and the task `helium` (fast, at
    revision 0) on `manifests/helium.toml`, which emits P06.T17's identity as a provisional table,
    its body bit for bit (only its header is rewritten to Design note 5's grammar), while
    `manifests/helium.production.toml` makes the fit with `--manifest` and `--out`, its figures in
    the record. The swap moves the fit's parameters into `helium.toml`, takes revision 1, is
    emitted at the train's version with the goldens, and rides the first galaxy train after its
    gate: version 25 (S1) if ready when bump-25's wiring starts, else the next.
  - The reproduction test uses `tests/common`'s `assert_reproduced`.

**Files.** `src/tasks/helium.rs`, `manifests/helium.toml`, `data/<grid>/`, `tables/helium.rs`. The
swap bumps the version, although no grid star changes, because members with a helium excess do.
Re-validated: the grid is `data/basti_helium/` (T7.a), and T7.b's builder adds the sim's
`stellar/sse/track/excess.rs` (or `track.rs`); registering the table adds its task to
`task::REGISTRY` and the table's row to `tables::MANIFEST` (P15.T1's "Adding a task"). As ruled
on 2026-10-23, the fit has its own `manifests/helium.production.toml` until the swap.

### P15.T8 Cluster dynamics constants

**Consumer.** Plan 09 (M3). **Format** (`tables::cluster_dynamics`): `BH_LOSS_BETA`,
`BH_LOSS_PSI_SLOPE`, `BH_CLOCK_FACTOR` (ruling 126.4: 2.5, on age ÷ t★ in the black-hole law
only), `BH_RELAXATION_PREFACTOR`; `EQUIPARTITION_EXPONENT`; `PULSARS_AT_47_TUC_GAMMA`,
`PULSAR_GAMMA_EXPONENT`, `PULSAR_CORE_COLLAPSE_CAP`. T8.a first moves plan 09's scratch constants
here unchanged (2.8 × 10⁻³, 147, 2.5, 0.138; η = 1, δ = ½ mass segregation; 40, 0.7, and a cap equal to the
count at 47 Tucanae's Γ), whatever plan 09 has called them.

- **P15.T8.a Black-hole loss against the CMC Cluster Catalog** (Kremer et al. 2020). Dataset:
  _fetched_; a reduction script extracts, per model and snapshot, mass, half-mass radius, number and
  mass of black holes and age. Fit the closed form of Breen and Heggie (2013) as parametrised by
  Antonini and Gieles (2020), f(t) = [(1 + ψ₁ f₀) e^(−β ψ₁ t ÷ t★) − 1] ÷ ψ₁ floored at zero, with
  t★ = c √(M r_h³ ÷ G) ÷ (⟨m⟩ ln Λ), for β (`BH_LOSS_BETA`), ψ₁ (`BH_LOSS_PSI_SLOPE`) and c
  (`BH_RELAXATION_PREFACTOR`), with the clock factor `BH_CLOCK_FACTOR` on t ÷ t★ (ruling 126.4), the
  four constants plan 09's P09.T9.c reads; the decay rate
  β ψ₁ (0.41 at the scratch values) is derived and is not emitted. **Acceptance:** rms error in
  log₁₀ of the retained black-hole number under 0.3 dex over models that retain any at 12 Gyr; at
  least 85% of models predicted empty are empty; applied to the Baumgardt–Hilker catalogue at 12
  Gyr: none in dynamically old clusters, tens to a few hundred in a typical massive one, thousands
  in ω Centauri, and about a fifth of the catalogue (15–25%) beyond the core-collapse line of 14
  relaxation times with no black holes (Trager et al. 1995).
- **P15.T8.b Mass-segregation exponent against multimass King models.** The tool integrates the
  models itself (Poisson's equation with one lowered-Maxwellian component per mass class, in the
  manner of Gunn and Griffin 1979 and Da Costa and Freeman 1976; neither is in the brainstorm's
  list, so re-check) over concentrations 0.7–2.3 and the mass-function slopes of "What is inside a
  cluster today", and fits η in the class profile (1 + r² ÷ r_c²)^(−3 q^η ÷ 2) for q below 1, with η
  = 1 above. η = 2δ, δ the models' velocity-scale exponent (Gieles and Zocchi 2015, MNRAS 454,
  576, eqs. 24–29), and η = 1 is δ = ½, not velocity equipartition (ruling 139.2). **Acceptance:**
  half-mass radius of each class below the turn-off reproduced to 10%; η between 0.8 and 1.0
  (ruling 139.2: Peuten et al. 2017 find δ ≃ 0.5 in N-body models, Hénault-Brunet et al. 2019 fit
  0.44 at 47 Tucanae).
- **P15.T8.c Pulsars against encounter rate.** Datasets: Bahramian et al. (2013) for Γ, the
  Baumgardt–Hilker tables for ρ_c and r_c, and a census of pulsars per cluster (the brainstorm cites
  none; the task records the one it uses). Poisson regression of count on Γ with a completeness term
  in distance. **Acceptance:** exponent within 0.5–0.9; about 40 (30–50) at 47 Tucanae's Γ; the
  implied total over the catalogue within 2,000–8,000.

**Files.** `src/tasks/{cluster_bh,equipartition,pulsars}.rs`, `manifests/cluster_*.toml`,
`data/{baumgardt_hilker,cmc,bahramian_2013,gc_pulsars}/`, `tables/cluster_dynamics.rs`. The
Baumgardt–Hilker tables also back plan 09's own Milky Way tests. Those keep a copy of the reduced
numbers, with the citation, in the sim's test data: the sim never depends on `hyperion-fit`, not
even as a dev-dependency.

### P15.T9 Type Ia yield table and samplers

**Source.** The observed delay-time distribution (Maoz and Graur 2017: 1.3 × 10⁻³ per M☉ formed,
t^−1.1 from 40 Myr), the merger rate (Maoz, Hallakoun and Badenes 2018), the channel shares of the
brainstorm (Shen et al. 2018; Foley et al. 2013), Peters (1964), and the binary formulae of Hurley,
Tout and Pols (2002) as plan 11 builds them; Claeys et al. (2014) for the known shortfall.

- **P15.T9.a Delay-first samplers (M3, consumer plan 09; plan 11 reads `DELAY_EDGES`).**
  `tables::type_ia_delay`: `DELAY_EDGES: [f64; 25]` (logarithmic, 40 Myr to 13.7 Gyr),
  `YIELD_PER_SOLAR_MASS: [f64; 24]`, `CHANNEL_SHARE: [[f64; 4]; 24]` (both destroyed, surviving
  donor, hydrogen donor, Iax), `PRIMARY_MASS_CDF` and `SECONDARY_MASS_CDF` as
  `[[[f64; 17]; 4]; 24]`, `LAYER_SHARE: [[f64; 2]; 24]` (layers C and D; C is expected to be zero),
  `ANCIENT_LOSS_PER_SOLAR_MASS: f64`. From single-star lifetimes and the initial-to-final mass
  relation alone: masses with lifetime(M₂) no greater than the delay, and for mergers the
  post-envelope separation solved from Peters's time so that lifetimes and inspiral add up.
  **Acceptance:** the yield integrates to 1.3 × 10⁻³ per M☉ to 1%; a fifth of delays under 0.1 Gyr
  and about 62% under 1 Gyr; no primary under 2.5 M☉; at Milky Way values 0.4–1 events a century and
  an exploded share of layer D within 2–4.5%.
- **P15.T9.b Yield and explosion mark (M4, consumer plan 11).** Fill plan 11's
  `tables::binary::IA_YIELD` (an `IaYieldTable`: η by the 24 delay bins of `DELAY_EDGES` and by
  `IaPoolChannel`, scratch value 1 ÷ 6 everywhere): run plan 11's engine forward on 10⁷ layer-C and
  layer-D binaries, pool the candidate events, and set the explosion probability per delay bin and
  pool channel to target ÷ pool, with a cap of 1. **Acceptance:** the mark averages about one in six
  (0.1–0.25); bins at the cap hold under a tenth of the yield, with the shortfall reported in the
  header; the pool's merger rate is 4.5–7 times the Ia rate; rerunning the fit with the fitted mark
  in place returns it within its error (the mark thins a pool it does not shape). (Re-validated at
  898a3c94: pending; it waits on P11.T6, which defines `IaYieldTable` and `IA_YIELD`.
  `IaPoolChannel` is `stellar::binary::IaPoolChannel` (P11.T4), and the engine pools its candidates
  as `PooledIaEvent`. P11's deferred finding in "P11.T11.a as built" is for this review of the
  engine's white-dwarf channels: an oxygen–neon white dwarf grown to collapse by accretion, and
  a carbon–oxygen one of 1.12 M☉, end as neutron stars, not as Type Ia.)

**Files.** `src/tasks/{type_ia_delay,type_ia_yield}.rs`, manifests, `tables/type_ia_delay.rs`,
`tables/binary.rs`.

### P15.T10 Conditional samplers of the other catalogue classes

**Source.** The rates of the brainstorm's "Events in time" table (Kochanek et al. 2014; Pala et al.
2020; Corral-Santana et al. 2016, and its neutron-star merger count), against plan 06's tracks and
plan 11's engine.

- **P15.T10.a Luminous blue variables (M3, consumer plan 09).** `tables::lbv::LBV_SAMPLER`: the
  conditional density of initial mass, and per mass node the age window in which a star sits near
  the Humphreys–Davidson limit, from plan 06's `SystemStars::lbv_window`. **Acceptance:** over a
  10⁶-star layer-E sample the class holds every star with a window inside the source horizon and
  nothing else; the sampler's mass distribution passes a Kolmogorov–Smirnov test at the 1% level
  against the brute-force members. (Re-validated at 898a3c94: pending; it waits on P09.T43's
  `LbvProcess` and its scratch `tables::lbv`. `SystemStars::lbv_window` exists.)
- **P15.T10.b Binary classes (M4, consumer plan 11).** Fill `AWD_SAMPLER`, `XRB_SAMPLER`,
  `MERGER_SAMPLER`, `NSM_SAMPLER` and `CLASS_SHARES` of `tables::binary` in plan 11's shapes
  (`ClassSamplerTable`, `ClassShareTable`), one sub-task per class. **Acceptance per class:** the
  galaxy-wide count at Milky Way values inside the brainstorm's range (about 10⁴ X-ray binaries with
  about 1,300 black-hole transients; 6–12 × 10⁶ accreting white dwarfs; 0.2–0.5 luminous red novae a
  year; about 10 neutron-star merger entries); and for each mark a two-sample Kolmogorov–Smirnov
  test at the 1% level between hosts drawn through the table and plan 11's
  `stellar::binary::testing::brute_force_class_members` over 10⁷ systems. (Re-validated at
  898a3c94: pending; it waits on P11.T8, which defines `ClassSamplerTable`, the four samplers and
  `brute_force_class_members`. Plan 11's Provides puts that in the sim's `tests/common/binaries.rs`,
  exported from `stellar::binary::testing` behind the `testing` feature, but `stellar::binary`'s
  `testing` module is private to the sim's own tests today. `ClassShareTable` and `CLASS_SHARES`
  exist (P11.T7). The two-sample test is `hyperion-testkit`'s `stats::ks_two_sample`.)

**Files.** `src/tasks/{lbv,binary_classes}.rs`, manifests, `tables/lbv.rs`, `tables/binary.rs`.
`tables/binary.rs` is filled by two tasks (T9.b and T10.b) with disjoint item lists. Each task's
items sit between its own pair of marker comments under its own header block, the emitter rewrites
only its own block, and the lock file holds one entry and one body hash per block. P15.T2's emitter
and `check` support this from the start, and a test covers two toy tasks sharing a file.

### P15.T11 Orphan streams per globular cluster

**Source.** The census of known streams: over 120 reported, wide and thin together, and far from
complete (Bonaca and Price-Whelan 2025, §2.2; Mateu 2023 lists 95); P15.T11.b counts the thin,
globular-like ones among them. **Consumer.** Plan 10 (M3). **Format** (`tables::streams`):
`ORPHAN_STREAMS_PER_GLOBULAR: f64`, the brainstorm's 1.5, uncertain threefold.

- **P15.T11.a Move plan 10** to the table, unchanged. Re-validated at 898a3c94: P10.T3.b already
  committed the table at its path, the brainstorm's 1.5, hand-made with no fit header, `MANIFEST`
  row or lock entry, and `global_list::orphans` already reads it (`expected_count`), so nothing is
  left to repoint. T11.a registers it, as P15.T4.a did for `chabrier`: a task `streams` (fast,
  revision 0) emits its manifest's scratch value, 1.5, as a provisional `TableItem::Scalar`, with
  its `manifests/streams.toml`, its `task::REGISTRY` entry, its lock entry and its `MANIFEST` row
  (P15.T1's "Adding a task"). The value's bits are unchanged, so no bump, and nothing generated
  reads the stream specifications yet (P10's "T3.a–c as built"). **Acceptance:** `just ci` green,
  with `just fit-check` warning that `streams` is provisional; a test holds
  `ORPHAN_STREAMS_PER_GLOBULAR` to 1.5 bit for bit (`assert_same_bits`) and the committed file to
  the task's rerendering (`tests/common`'s `assert_reproduced`); no golden moves. Half a day.
- **P15.T11.b Fit.** With plan 10's generator at Milky Way values, for a multiplier k on a grid from
  0.5 to 4.5: generate the streams, observe from the plane at 8 kpc, apply a detectability cut (mass
  above 10³ M☉ within 30 kpc, axis density at least ten times the smooth halo's, more than 15° from
  the plane; the cut's constants are manifest parameters), count, and choose k to match the known
  globular-like streams with the Poisson error. **Acceptance:** k inside 0.5–4.5 with a stated
  one-sigma interval; at the chosen k globular tubes hold 0.3–1% of the halo's budget. The value
  stays a parameter of the generator version whatever the fit says, and the header states how far
  the detectability cut moves it. Bumps the version if the value moves. (Re-validated at
  898a3c94: pending; it waits on P10.T5–T6. The generator as built, `global_list::Debris`, makes
  stream specifications only: the spray (`global_list::spray`) and the tubes (`TubeTable::build`,
  `locate`) that this fit observes are not built, and `TubeTable` is a placeholder.)

**Files.** `src/tasks/streams.rs`, `manifests/streams.toml` and its smoke manifest,
`data/stream_census/` (the counts used, _committed_ as facts with their citations),
`tables/streams.rs`, and plan 10's constant repointed at the table (T11.a).

### P15.T12 Close-out

No provisional table remains; `check` treats `provisional: true` as an error from here on; every
header carries its acceptance figures. `hyperion-fit list` prints name, revision, class, runtime and
acceptance for every table, pasted into the pull request. **Acceptance:** `just ci` and
`just test-slow` green with zero provisional tables. (Re-validated at 898a3c94: `check` sees only
registered tables, so the hand-made `binary`, `helium` and `streams` must be registered first, by
P15.T9.b and T10.b (with P11.T15), T7.b and T11.a. The registry also runs other plans' tasks,
Provides' "Built at 898a3c94", whose provisional tables, if any remain, this error catches too.)

### Schedule

| Table                 | Task         | Consumer | Needed in | Needs  | Class | Scratch from      |
| --------------------- | ------------ | -------- | --------- | ------ | ----- | ----------------- |
| **MGE, first cut**    | **P02.T6.a** | **02**   | **M1**    | 01     | fast  | itself            |
| Toolchain             | P15.T1, T2   | all      | start M2  | 01, 02 | n/a   | n/a               |
| MGE, final and boxy   | P15.T3       | 02, 08   | M2        | 02     | fast  | P02.T6.a          |
| Kick ranks, defaults  | P15.T5.a, b  | 06       | M2        | 06     | slow  | P06.T19.b         |
| Kick-law seams        | P15.T5.d     | 06       | M2        | 06     | n/a   | n/a               |
| Helium correction     | P15.T7       | 06, 09   | M3        | 06     | fast  | P06.T17           |
| Displaced forms       | P15.T6       | 08       | M3        | 02, 06 | slow  | none: start in M2 |
| Cluster dynamics      | P15.T8       | 09       | M3        | data   | fast  | plan 09           |
| Type Ia delay         | P15.T9.a     | 09       | M3        | 06     | fast  | plan 09           |
| LBV sampler           | P15.T10.a    | 09       | M3        | 06, 09 | fast  | plan 09           |
| Orphan streams        | P15.T11      | 10       | M3        | 10     | slow  | plan 10           |
| Chabrier branch scale | P15.T4       | 02       | M4        | 11     | fast  | plan 02           |
| Merged-binary fate    | P15.T5.c     | 09, 11   | M4        | 09, 11 | slow  | plan 11           |
| Type Ia yield, mark   | P15.T9.b     | 11       | M4        | P11.T6 | slow  | plan 11           |
| Binary class samplers | P15.T10.b    | 11       | M4        | P11.T8 | slow  | plan 11           |

Only the displaced table has no scratch stand-in, because its consumer cannot do anything useful
without it. Everything else lets its consumer proceed on a scratch value and costs one version bump
when the fit lands.

Re-validated at 898a3c94, the Needs column by task: P15.T5.b runs after T5.d; T5.c needs nothing
more, since P09.T9.b's retention is the default fate's reading, and it moves nothing, so its row's
M4 and slow no longer hold (decision-p15-revalidation Q4); P15.T9.b needs P11.T6, and T10.b
P11.T8; T10.a needs P09.T43, its consumer; T11.b needs P10.T5–T6. T3.c rides version 25 (S1), and
T6's production run starts from 25's commit and lands with 26 (S2). T7.b's swap rides the first
galaxy train after its gate. Built: T1, T2, T4, T5.a (its production run is P06.T19.e's), T6.a–g,
T7.a, T8.a–c (provisional) and T9.a.

## Verification

- Every task group's acceptance test runs where its class allows: fast tasks as tests of the task on
  its real manifest and again under `--rerun-fast`; slow tasks on their smoke manifest in
  `cargo test`, with the full run's figures in the header and the lock file.
- `just fit-check` in CI proves that no table is stale, hand-edited, ahead of the generator version
  or missing from `tables::MANIFEST`.
- Determinism: the thread-count test of P15.T1, a smoke rerun test per slow task, and `--rerun-fast`
  on CI's machine against tables made on a developer's.
- The consumers' statistical and Milky Way tests are the end-to-end check that a table does its job.
  This plan's acceptance figures are the same numbers those tests assert, so a table that passes
  here does not fail there.

## Generator version

Every table belongs to the generator version. A commit that changes a table which generated output
reads bumps `GENERATOR_VERSION` by the rule of Design note 10. Known bumps from this plan: P15.T3.c,
P15.T4.b, P15.T5.a (through P06.T19.e), any moved default of P15.T5.b, P15.T7.b (only members with a
helium excess change), P15.T8, P15.T9, P15.T10 and P15.T11.b. Moving a constant into a table at the
same value is not a bump. P15.T6 lands with plan 08's own bump at version 26 (S2); T6.b's
production run runs once, from version 25's landed commit, under the potential freeze: from that
commit until 26 lands, nothing may change plan 02's potential tables or the birth populations the
run samples (decision-bump-plan-after-22, 2026-10-14; updated 2026-10-15). P15.T6.g's re-emission
landed with the 21 → 22 batch ("Landed in the 21 → 22 batch", T6.g as built; re-validated).

Re-validated at 898a3c94, at version 23, and ruled 2026-10-23 (decision-p15-revalidation Q9).
P15.T4.b's bump is taken (its table says since 14). P15.T9.a moved no wired output and took no
bump (its record), so P15.T9's is T9.b's. P15.T5.a's rides P06.T19.e at version 25 (S1, as the
Generator version of plan 06 says). P15.T5.b, T5.c, T5.d, T7.b's first landing and T11.a move
nothing and take no bump. Each held swap rides the first galaxy train after its lane's gate, as
decision-bump-plan-after-22 does for its own:

- T3.c, at 25 (S1), as an anchor;
- T7.b's swap, at 25 as a rider if ready when its wiring starts, else the next train;
- T8's adoption, at the first train after its deferred ruling, if that ruling moves a value;
- T10.a, at the first train after P09.T43 and its own gate (the bump plan lists 28);
- T9.b and T10.b, with P11.T15, whose swap they are, at 28;
- T11.b, if its value moves, at the first train after P10.T5–T6 and its gate (expected 28);
- a default of T5.b that a later ruling moves, at the first train after its gate.

P11's finding F13 is a deferred correction, with no train (decision-p15-revalidation Q5).

Reserved so that later tables move nothing they need not: table paths and item names are final from
the scratch stage on; the node arrays of `displaced_forms`, so that a finer dependence on the halo
or the bar adds nodes and not fields; `HYPERVELOCITY`, which plan 08 registers at zero weight until
plans 09 and 11 feed it; and the domain-tag prefix `"fit."`, which no generator code uses (in the
sim's registry, where no `fit.` tag exists yet: re-validated).

## Risks and open points

- **Updated for the 2026-09-21 density rulings.** Chabrier's system function is now the default, and
  P15.T4 fits its scale to the 20 pc census instead of the 75.9% that was Kroupa's function itself,
  so the scale may land well above the old 0.6–0.75 window. The halo's smooth components now have
  inner slopes of 2.2–2.8, which makes the smooth halo at 15–18 kpc three to four times denser, so a
  typical globular stream stands at roughly 7–15 times it, not 25–45. P15.T11.b's tenfold
  detectability cut then removes more model streams, and the fitted multiplier k moves with it. The
  cut is a manifest parameter, and its value is checked against how the known streams were found
  before the fit runs. If plan 02 puts the discs' cored vertical profiles into the potential, P15.T3
  gains a one-dimensional expansion of those profiles beside `MGE_EXP`.
- **The roadmap's "M2 onward" hides an M1 dependency.** Resolved as the roadmap says: P02.T6.a
  creates the crate with the first-cut MGE task, and this plan extends both after M1. The crate plan
  02 leaves has no registry, emitter or lock file, so between M1 and P15.T2 the only guard on
  `tables/mge.rs` is plan 02's byte-for-byte test, which is enough for one fast table.
- **No named source for the helium correction**, although the brainstorm says each fit "has a named
  source to fit against". Nor is there one for the pulsar census of P15.T8.c or for multimass King
  models. Source selection is the first sub-task of each, the scratch value stands meanwhile, and
  the owner should confirm the choices.
- **Catalogue terms of use are not known in advance**, and no web research was done for this plan.
  Design note 12 works either way.
- **Circular calibration.** P15.T4.b, T5.c, T9.b, T10.b and T11.b are calibrated against code that
  itself reads scratch values of the same tables. Each such fit is of a normalisation the forward
  code does not feed back into, and each task tests that by refitting with the fitted value in
  place.
- **The displaced fit is the long pole**, and its fingerprint ties it to plan 02's potential. A late
  numerical change to the potential costs a rerun of hours. The fit is dimensionless and universal
  to 4–5%, so the manifest may carry an explicit, reviewed fingerprint waiver for this one table
  provided the universality check of P15.T6.e passes against the changed potential.
- **The bar in the fit is not the sim's density.** The sim's potential tables are axisymmetric; the
  rotating quadrupole exists only in `hyperion-fit`, and its amplitude is a manifest parameter. The
  brainstorm's own-form shares came from one such model at one pattern speed; T6.d's grid over the
  corotation ratio is the extension it asks for.
- **Table shapes are split between plans.** Four shapes belong to consumers. If a fit needs a field
  its consumer did not foresee, the consumer's shape changes in that task with the consumer's tests,
  not here.
- **Hand-written optimisers** may converge worse than a library's. Acceptance is on the fit's
  quality, so a task that misses may change its method without touching anything else.
- **Build cost.** The crate adds `rayon`, `clap` and the rest to every `cargo clippy --workspace`.
  If CI time suffers, exclude the crate from the workspace's default members and lint it in the
  `just fit-check` step.
- **For the owner: Chabrier's 2005 system function (ruling 138.6).** P15.T4.b fits the 2003 system
  function's branch above 1 M☉ (s = 0.920). Chabrier's (2005) revision, m_c 0.25 M☉ and x 1.35,
  fits the 20 pc census's primary counts better: χ² 4.4 as published and 1.7 at s = 0.88, against
  5.6 for the fitted 2003 form. It is noted and not adopted, because it also moves plan 13's
  substellar branch; adopting it is a change to plan 02's mass function and a version bump.
- **T7.a, T8.a–c, T9.a as built (lane `fit15a`, 2026-09-30, at `GENERATOR_VERSION` 15; no
  bump, no wired output moved).**
  - **T9.a done.** Task `type_ia_delay` (fast, 1.4 s release), `tables/type_ia_delay.rs`, test
    `tests/type_ia_delay.rs`. `DELAY_EDGES` start at plan 06's 42.55 Myr (`type_ia::MIN_DELAY`,
    ruling 136.7), not 40 Myr. Pair model: the galaxy's mass function × the share of mass ratios
    on 0.1–1 left open by the channel's window (new `type_ia::{primary_range, secondary_window}`;
    `MIN_MASS_RATIO`, `DONOR_MAX` made public; a living donor's window now also respects q ≥ 0.1).
    `PRIMARY_MASS_CDF` holds quantiles at ranks k/16 of the primary's place in `ln m` on its range
    at the drawn delay, not masses; `SECONDARY_MASS_CDF` quantiles of the place in the window
    (the identity: flat q). `CHANNEL_SHARE` is ruling 136.9's in every bin (lifetimes and the IFMR
    carry no channel information; left to T9.b). `LAYER_SHARE` C is 0 by construction (primary ≥
    2.5 M☉). Acceptance passes: yield 1.000000, 0.187 under 0.1 Gyr, 0.617 under 1 Gyr, 0.462 a
    century, ancient share 3.60–4.07%, quantiles within 0.0155. Swapped into
    `IaProgenitor::draw` (new `type_ia::delay_bin`); `supernova.golden` (unwired) re-blessed:
    masses move, channels do not.
  - **T7.a done.** Source: old BaSTI α-enhanced with its helium-enhanced extension (Y 0.30, 0.35,
    0.40; [Fe/H] −2.62 to −0.29; tracks and ZAHB), fetched: `data/basti_helium/PROVENANCE.toml`
    (88 files, 38.4 MB) and `urls.txt`; raw in `data/cache/basti_helium/`. Research notes with the
    survey and the columns T7.b reads: `target/lanes/srvbodies/target/fit15a/helium/NOTES.md` (not
    committed). T7.b not started.
  - **T8 file moved.** `tables/cluster_dynamics.rs` is now three blocks (`cluster_bh`,
    `equipartition`, `pulsars`), each `@provisional` holding plan 09's scratch values bit for
    bit; the fitted values and misses are in each block's acceptance. New `optimise.rs`
    (golden section, Nelder–Mead). `hyperion-fit` enables the sim's `testing` feature for the
    Baumgardt–Hilker copy. New committed datasets (reduced facts + `reduce.py`): `cmc` (Kremer et
    al. 2020 Table A1 from the arXiv source), `bahramian_2013` (VizieR), `gc_pulsars` (Freire).
  - **T8.a built, acceptance missed (provisional; ruling deferred).** β and c enter only as β ÷ c,
    so c stays 0.138 and k 2.5; the best fit runs to ψ₁ → ∞ (a pure exponential, βψ₁ 0.101) at
    0.476 dex (scratch 1.716 dex); 19 of 34 predicted empty are empty; on the catalogue ω Cen
    12,100 and 0.6% core-collapsed. Scratch kept.
  - **T8.b built, acceptance missed (provisional; ruling deferred).** Multimass King models (δ ½,
    40 models, c 0.81–2.08): best η 0.17 with radii within 28%; η = 1 gives 92%. Light classes'
    half-mass radii saturate in King models; the class profile's form cannot match to 10% at any
    η. Task is `Slow` (31 s release); its smoke manifest `equipartition.smoke.toml` exists but
    the smoke-run test is not written yet.
  - **T8.c built, acceptance partly missed (provisional; ruling deferred).** γ 0.632 and A 38.0
    pass; the implied total 1,539 misses 2,000–8,000. Scratch kept.
  - **Pulsar-cap test fixed** (`interior/checks.rs`): asserts `held == min(law, NS)`, counts the
    clusters where the law exceeds the neutron stars (3), and pins the cap's removal at ≤ 1,400
    (measured 1,299, all NGC 5694, Γ 334 × 47 Tuc from its 0.02 pc catalogue core; provisional).
  - **Not done:** T7.b; a reproduction/smoke test for the three T8 blocks beyond `just
fit-check`; review-changes on this lane's diff.
- **P15.T6.g's drift scale and far field** (lane `p15-orbits`, 2026-10-12; decision-p15-t6g-orbits
  §2 and §3). The reduced run's unresolved fast kicks were marginally bound orbits measured on |J₀|,
  and escapers crossing a far field that was not conservative. Every drift is now measured on
  max(|J₀|, |Φ(x₀)|). P02.T6.f makes the far field conservative. The production run starts only
  from a commit holding both, after the reduced run is repeated there. The convergence check is
  the median's fall, 3–5 fold. Details are in "T6.g as built".
- **P15.T6.g's re-emission moved the provisional table's acceptance line** (lane `p15-orbits`,
  2026-10-13, at header 21, on a base holding P02.T6.f). The smoke histograms, 7fbfd48f… at
  version 16, are now 88db652d…: 211 orbits, none captured or unresolved, 6 refined. Without
  P02.T6.f the digest was 8163fd3f…, with the same table body.
  - Disc-born weighted mean misplaced 0.9661 → 0.9718 (unregularised 0.9641 → 0.9716, λ 0.01 →
    0.001).
  - Classes with a parameter on a box edge 52 → 54, of them over 10⁻⁴ of the weight 19 → 18.
  - Second-difference sign changes 66 → 85.
  - The nuclear disc's own-form shares 1, 0, 0, 0 → 1, 0, 1, 0.
  - Bar elongation 0.80, −2.04, 1.04, 0.80 → 0.79, 0.00, 1.13, −20.39; length within 0.76 → 0.98.
  - The slowest thin class's rotation 0.889 → 0.882 `v_c`; the neutron stars' half-density height
    238 → 102 pc.
  - The hypervelocity row is unchanged.
  - The `galaxy_class_table` golden moves 70 values at header 21 (for example `ThinDisc.gone`
    0.214 → 0.134), all read from the table.
  - These moves come from the smoke run's handful of orbits per class, from c07edb86's potential
    shift and from the integrator, not from a physical change. Refits on one and on three threads
    give the committed bytes.
  - The table's `since` is 21, because the emitter writes only the current version. It keeps 21
    over an unchanged body. At the 21 → 22 bump, main makes it 22 or rules that 21 stands.
- **T8's follow-up tests as built (lane `lane/61-p15-t8-cluster-tests`, 2026-10-20, at
  `GENERATOR_VERSION` 22; no table moved, no bump).** One file, `tests/cluster_dynamics.rs`, by
  main's ruling of 2026-10-20 (issue #61 §3: rerun and compare bytes, tiered by the task's class).
  Times are nextest's in the dev profile on an 8-thread laptop at load 3.4–3.7, four test threads
  (a shared machine, so provisional).
  - `cluster_bh_block_is_reproduced` (fast, 1.1–1.3 s) and `pulsars_block_is_reproduced` (fast,
    0.2 s): `rerender` on the committed manifest gives the whole shared file byte for byte, its body
    unchanged. Both reproduced it on the first run.
  - `the_committed_blocks_hold_plan_nines_scratch_values` (fast, under 0.01 s): each block's body is
    its task's `render` of `SCRATCH` or `SCRATCH_ETA`, `@provisional` by P15.T8.a, b or c, its
    `task` the block's name. A ruling that adopts fitted values updates this test in its own lane.
  - `the_equipartition_smoke_run_passes_the_header_grammar_and_is_the_same_on_any_thread_count`
    (fast, 2.1–2.4 s): the smoke manifest through `cli::run` on one thread and on four, the same
    block both times, its header's task, manifest and acceptance, and its declared items. The run is
    `@provisional` by P15.T8.b (η 0.1588 over the two models, radii within 0.169). The task ignores
    `--threads`, so the pair proves determinism and the command line, not a thread split.
  - `equipartition_block_is_reproduced` (slow, `#[ignore]`, 21.0 s): `rerender` on the full
    manifest's 40 models, as above.
  - **Closes** the record's "Not done" item "a reproduction/smoke test for the three T8 blocks"
    (T7.a, T8.a–c, T9.a as built). T7.b and the review of lane `fit15a`'s diff stay open.
  - **Deviations.** The scratch test also pins `SCRATCH` and `SCRATCH_ETA` bit for bit at the
    values P15.T8 quotes from plan 09 (the testkit's `assert_same_bits`), so that a changed scratch
    constant cannot pass with its own re-emission. The `cluster_bh` test builds no galaxy of its
    own: the task builds the Milky Way at the manifest's seed inside `rerender`, as in
    `type_ia_delay_table_is_reproduced`. On a mismatch the panic gives the first differing line's
    committed and rerendered text, not only its number.
  - _Deferred findings (2026-10-13 rule)._ The declared-items helper is now copied in
    `tests/cluster_dynamics.rs`, `tests/kick_rank.rs` and `tests/displaced_forms.rs`, and inline in
    `tests/toolchain.rs`; the reproduction tests' first-differing-line panic is copied in eleven
    other test files. Both belong in `tests/common/mod.rs` (rust-dev § Tests), for a lane whose
    scope holds those files: this one's held two paths.
- **Re-validated at 898a3c94 (2026-10-22).** Against `origin/rendering-and-planets` at 898a3c94,
  at `GENERATOR_VERSION` 23 and `PROTOCOL_VERSION` 3: this plan's first re-validation, anchored on
  9d955b26, where it arrived on this branch. Docs only; no code, table or other plan changed. The
  brainstorm's two commits since (0c115993, the contact pairs after Rucinski; 78a8c520,
  composition) touch no fit here.
  - _What changed, and why: the code as built._
    - Provides: the crate's dependencies and its guard test; the command line's as-built options
      and `orbits`; the Rust sketch as built (`TableCrate`, `REGISTRY`, `find`, `items()`,
      `table_crate()`; `optimise` holds golden-section search and Nelder–Mead, NNLS and
      Levenberg–Marquardt are P15.T3.a's, and there is no `stats` module); a paragraph under the
      module table on its rows as built (`MGE_HOLED_EXP`, `LOG_Z_NODES`, `tables::binary`), on what
      is built and on the registry's 47 tasks; the recipes as the justfile has them.
    - Consumes: R04.T4.d's base crate and three registries (the `fit.*` tags go in the sim's, at
      its end); `Galaxy::from_params`'s seed and `Result`; `MGE_HOLED_EXP`; what
      `MassModel::enclosed_mass` holds; the forces; the kick law's defaults in `tables::kick_rank`
      and the track's own windows; plan 09's retention and tables; plan 10's table and generator;
      plan 11's shapes as built. Design notes 1, 3 and 9: their references to code.
    - Tasks: the ordering paragraph (T3 is one of the parallel groups; T5.d comes before T5.b).
      T3.a–c: the manifest exists; `MGE_HOLED_EXP` is refitted with `MGE_EXP` on its widths,
      since plan 02's test holds them equal; how T3.a–b land before T3.c without staling the
      committed table; the potential's signed weights; the other readers T3.c meets. T5's Format
      (the defaults are in the table), T5.b (what its harness cannot vary; seven tests) and T5.c
      (nothing reads the fate; ruling 137.3; finding F13). T6's Files line. T7: the table's three
      items, the data, the grid's reach, the crate-private hook and R06's two tables. T11.a: it
      registers the table that P10.T3.b committed, with an acceptance it lacked. Pointers in T9.b,
      T10, T11.b and T12; the Schedule's T5.d row and its note; Generator version.
  - _New subtask._ P15.T5.d, the kick law's seams for the defaults review: the windows the track
    reads and the rank table's membership become run-time inputs that default to today's constants,
    so that T5.b's grid can vary them. P06's "For P15.T5.b" says to vary them through the
    constants, which are compile-time. It comes before T5.b, by the ordering paragraph and the
    Schedule; no task is renumbered.
  - _Records left as written, now stale in part._
    - T1's record: five tasks registered, now 47 (Provides).
    - T2's "Today's tables" and its `just fit-check` output. The provisional set at 898a3c94 is
      `cluster_bh`, `equipartition` and `pulsars` at 15, `kick_rank` at 16, `mge` at 11 and
      `displaced_forms` at 22.
    - T5.a's record. Ruling 147.2 rebuilt the committed table at task revision 1 (P06's
      "Ruling 147.2, as built"), and no run of `kick_rank.production.toml` is recorded, so the
      production run is made at revision 1, and P06.T19.e reruns it rather than committing an older
      scratch file.
    - T7.a's record: its research notes are not in the repository, and T7.b's text now stands
      without them.
    - T8's tests record: its deferred finding on the shared helpers is closed by 7696bdbe
      (2026-10-20), which moved `declared`, `first_difference` and `assert_reproduced` to
      `tests/common/mod.rs`.
    - This section's first bullet: plan 02 did not put the discs' cored vertical profiles into the
      potential (its D6 keeps every disc exponential in height there), so T3 gains no expansion of
      them.
  - _Pending re-validation._ T3.c waits on T3.a–b and rides S1 (25). T5.b waits on T5.d. T6's
    production run waits on version 25's commit, and T6.c–f's production fits on it. T9.b waits on
    P11.T6, T10.a on P09.T43, T10.b on P11.T8, T11.b on P10.T5–T6, and T12 on all of them and on
    the deferred ruling on T8.a–c's blocks. Executable now (ruled 2026-10-23): T3.a and T3.b;
    T5.d, then T5.b, with T5.c's guard; T7.b's first landing, its swap held for a train; and T11.a.
  - _Awaiting a ruling, unchanged:_ the three cluster blocks of T8.a–c, provisional, their fitted
    values and misses in their acceptance lines.
  - _Open points, ruled 2026-10-23_ (decision-p15-revalidation, one ruling each; the texts above
    carry them):
    1. T5.b's grid points of the ramp and the windows rebuild the rank table, the centre included,
       at 10⁵ scores; the black-hole factor's hold the centre's rebuild.
    2. Ruling 147.2's two held figures are accepted as findings and stay held; T5.b moves no
       default.
    3. T5.b's sensitivity table is a registered `kick_defaults` table that nothing reads, with no
       bump.
    4. Retention as built is the default fate's reading and passes; T5.c adds a guard, measures no
       alternative, and takes no bump.
    5. F13 is a deferred correction outside this plan, with no task and no bump.
    6. T7.b asserts to the grid's reach (ΔY about 0.15) and states its extrapolation to 0.18.
    7. T7.b refits neither sky table: the phase envelope keeps its read-side rule, and
       `sky_envelope` gains helium-excess nodes with whichever of T7.b's swap and R06.T16.a lands
       second.
    8. The bulge's tracer follows the potential to the axisymmetrised boxy body, whose in-plane
       scale is √((a² + b²) ÷ 2), in T3.c.
    9. Each held swap rides the first galaxy train after its lane's gate (Generator version).
  - _Merges._ The sweep was made at the lane's base, 898a3c94, three commits past the brief's
    607346de. The lane then merged `origin/rendering-and-planets` at 93396d5f, two R08.T8.e commits
    in the client's atmosphere code and plan R08. None of these touches what this plan consumes,
    and the versions stay 23 and 3, so the sweep stands.
- **T3.a–b as built (lane `lane/94-p15-t3`, 2026-10-23, at `GENERATOR_VERSION` 23; no table
  moved, no bump).** Fit-crate code and tests only, by main's ruling of 2026-10-23 on P15.T3's
  brief (issue #94). The refit runs from a manifest of its own, `manifests/mge.refit.toml`
  (`method = "free_widths"`), with `--manifest` and `--out` to a scratch path. `mge.toml`, the
  task's revision 1 and its three `items()` still make the committed first cut, whose code path
  is unchanged. `MgeTask::run` sends a manifest that names `method` to `tasks::mge::refit::run`,
  which refuses any other method, a key it does not read, or a value its tables fix, with a
  `ManifestParamError`. A manifest without `method` goes through `expect_params` as before.
  - _Method (T3.a)._
    - `optimise::nnls`: Lawson and Hanson's (1974, ch. 23) active-set NNLS, its free columns
      solved by Householder QR; a column under 2⁻⁴⁰ of its norm once the others are taken out
      counts as dependent. Their subroutine gives up after 3n inner passes; here 3n outer steps
      of at most n inner passes bound the work, and the last feasible point is returned.
      `optimise::levenberg_marquardt`: Marquardt's (1963) scaling kept as Moré (1978) keeps it
      and MINPACK's `lmder` implements it, the damped step by QR of `[J; √λ D]` (λ from 10⁻³, ×10
      or ÷10, within 10⁻¹² to 10¹²), lower bounds by a projected step that holds a bound
      parameter whose descent direction points out (Bertsekas's 1982 set I⁺, without his
      ε-enlargement). It stops when no step lowers the sum even at 10¹², which changes no bit,
      since every later iteration would repeat the failed trial. Both are public, with unit tests
      against known solutions. Bounds are met by explicit comparisons, not `f64::max`, so a
      signed zero cannot reach a table.
    - `MGE_EXP`: variable projection (Golub and Pereyra 1973). Levenberg–Marquardt moves the 14
      ln widths from the first cut's log-spaced 0.04–3.5. At every evaluation the weights are
      re-solved by `nnls` on 400 log-spaced `s` on [10⁻³, 12], each row over `e^(−s)`. The
      Jacobian is by forward differences of 10⁻⁷ in ln width. At the end the widths are sorted
      and the weights solved once more. The manifest's 300 iterations are not reached: the
      body at 1,000 is the same, and at 100 it differed in the seventh digit.
    - `MGE_HOLED_EXP`: P02.T12.b's construction on the refit's widths, samples and weighting.
  - _Figures (T3.a), on the refit; all pass._ Against plan 15: `MGE_EXP` within 0.010% of
    `e^(−s)` on 0.05–10 (0.5%), its spherical mass within 0.0014% (0.05%); the thin disc from it
    within 0.016% of Freeman's (1970) speed on 0.2–8 scale lengths, worst at 0.25 (0.5%);
    `MGE_BAR` within 2.96% of its central value, worst at u = 0.89 (3%), its mass within 0.295%
    (0.3%), and its cumulative mass within 4.93%, worst at the edge u = 1.06 (5%). Against plan
    02's table tests: `MGE_EXP` within 1% on 0.05–8 and its mass within 0.1%; `MGE_HOLED_EXP`
    within 0.083% of `e^(−s)` on 0.05–8 (1%), its mass fraction within 2.8 × 10⁻⁶ (10⁻⁵), its
    widths `MGE_EXP`'s bit for bit, some weight negative; `MGE_BAR` within 0.03, 0.5% and the 7%
    cumulative check; non-negative weights and strictly ascending widths for both.
  - _Deviations (T3.a)._
    1. **The bar's method.** Least squares on the profile and its mass row alone gave 2.83%,
       0.307% and 6.91%, and with rows added for the mass inside each edge of plan 02's
       cumulative test (0.06–2.5, in relative error) no fixed weighting met all three: the best
       was 2.90%, 0.64% and 5.07%. So the bar runs in six rounds. Each polishes the widths in
       least squares, then solves the weights for the least largest error by Lawson's algorithm
       (1961; Rice and Usow 1968, Math. Comp. 22, 118), 300 non-negative solves with each row
       scaled by its figure's tolerance (0.03, 0.003, 0.05). The next round's polish weights its
       rows by the last minimax solve's weights, and the round with the least largest scaled
       error is kept. The worst scaled error is 0.987, so the three figures pass with about 1%
       of their tolerances to spare: R14's bound at work. This is the "Hand-written optimisers"
       risk's licence; the basis is unchanged.
    2. **Fixed keys.** The refit reads `widths`, `bar_width`, `bar_level` and `bar_end` from its
       manifest and refuses any value but 14 and the bar's nominal shape, which `bar_profile`
       is written for. It does not generalise the profile, so the first cut's code is untouched.
    3. **Freeman's check** is in `tests/mge.rs`, with the Bessel functions by series below y = 9
       and asymptotic series from there (twelve digits where the disc reads them, y ≤ 4; the
       reference values are mpmath's at 30 digits). The disc's height is 10⁻⁵ of its scale
       length: a tenth of that moves the speed by 1.9 × 10⁻⁵ at most. The plan puts the Bessel
       functions in the task's tests, so the run's acceptance line leaves Freeman's figure out
       and `refit::passes` does not read it; the acceptance line says the test holds it.
    4. **The cumulative mass's edges.** Plan 15's "within 5% everywhere" is measured at plan 02's
       edges, u = 0.06, 0.08, …, 2.5 half-lengths, the profile's mass by Simpson's rule at 0.01,
       as the brief directs. Inside 0.06 the error is bounded by the central-value figure, and
       beyond 2.5 it settles to the mass figure. The fit's own cumulative rows read their step,
       their number and their first edge from the manifest (`bar_cumulative_step`,
       `bar_cumulative_pairs`, `bar_cumulative_from`), set to the same edges.
    5. **Freeman's disc** is built in `tests/mge.rs` by `double_exponential`'s term rule (positive
       terms, mass ∝ wᵢsᵢ²wⱼsⱼ, width sᵢL, axis ratio sⱼH ÷ sᵢL) on the refit's `MGE_EXP`, not by
       `double_exponential` itself, which reads the committed table; T3.a touches no file of the
       sim. Its speed is the sim's `Gaussian::v_circ_sq`.
  - _The run (T3.a)._ `run mge --manifest crates/hyperion-fit/manifests/mge.refit.toml`, with
    `--out` to a scratch path, takes 3.7 s in the dev profile (an 8-thread laptop at load 6.3,
    shared), so the task stays `Fast`.
  - _Tests (T3.a)._ Unit tests in `optimise.rs`: NNLS (the free solution, a held variable, all
    held, an overlapping Gaussian basis, Kuhn–Tucker and determinism) and Levenberg–Marquardt
    (Rosenbrock, a lower bound, a non-finite trial, determinism), with both optimisers' bits
    pinned by `the_optimisers_keep_their_bits`. In `tasks::mge::refit`: Lawson's minimax
    constant and its non-negative weights, the NaN rule of the figures' maxima, and
    `log_points` against the first cut's spacing bit for bit. In `tests/mge.rs`:
    `the_refit_meets_plan_fifteen_s_figures` (which also holds that the polish of `MGE_EXP`
    stops before its cap), `the_refit_passes_plan_two_s_table_tests`,
    `the_refit_disc_s_circular_speed_follows_freeman`, `the_refit_is_deterministic`,
    `the_refit_manifest_runs_end_to_end`, `a_manifest_that_is_neither_cut_nor_refit_is_refused`,
    `the_refit_refuses_values_out_of_range` and
    `the_bessel_functions_match_their_tables_and_their_wronskian`. All are fast, under nextest in
    the dev profile on 4 test threads at load 4–6: the four that refit take 4.0–4.6 s, the
    determinism test 8.2 s (two refits), the others under 0.2 s.
  - _Method (T3.b)._ `tasks::mge::boxy`, run with the one-dimensional refit from the same
    manifest. For each c of 3.0, 3.5 and 4.0 it fits the unit body exp(−(R^c + |z|^c)^(1 ÷ c)),
    R and z in its scale lengths (the ruling's point 8), with 16 terms of free weight, width and
    q.
    - The grid is 48 log-spaced spherical radii on [0.001, 40] by 12 latitudes at the
      Gauss–Legendre nodes of [0, π ÷ 2] (rule computed by Newton's method on Pₙ).
    - The residuals, in order: the density in relative error weighted by each grid point's
      share of the mass; the latitude average (the monopole) of each radius out to 12, in
      relative error, weighted 3; the inward pull in the plane at 0.25, 0.5, 1, 2, 4 and 8 scale
      lengths, in relative error, weighted 10; the central density Σw against 1, weighted 1; and
      the whole mass Σ w (2π)^(3÷2) σ³ q against the body's, weighted 3. The force and mass rows
      are the ruling of 2026-10-23 (issue #94): weight the fit towards the enclosed mass and the
      force at six radii.
    - The seed is the refit's `MGE_EXP` at q = 1, the c = 2 body, its 14 weights solved by
      `nnls` on the residuals; then the two terms carrying the most mass (w σ³) are each split
      into two of half the weight at q = 1.1 and 1 ÷ 1.1, so that all 16 start with weight.
    - Levenberg–Marquardt moves the weights, ln widths and ln q together, 300 iterations, with
      an analytic Jacobian for the density rows and forward differences of 10⁻⁶ for the forces.
      The weights are held at 0 or above by the projected step, the widths within [0.001, 40]
      and q within [0.05, 20]. At the end the terms are sorted by width.
    - The references: the body's mass inside a sphere by a direct integral in latitude of the
      radial γ(3, N r) ÷ N³, which the tests hold to the spherical exponential's closed form at
      c = 2 to 10⁻¹²; its whole mass the same at infinity, held to 3! V(c) with the sim's
      `BoxyBulge::unit_volume` to 10⁻¹²; and its midplane pull by the multipole expansion of its
      potential (Binney and Tremaine 2008, §2.4) to order 64, which the tests hold to a
      brute-force integral of the force over all space along rays from the field point
      (`boxy::midplane_force`) to 10⁻⁶ at c = 3, 3.5 and 4, and both rules to the closed form at
      c = 2 (10⁻⁶ and 10⁻⁸).
  - _Figures (T3.b), on the refit, per c = 3, 3.5 and 4._
    - Mass-weighted rms relative density error: 13.3%, 17.5% and 20.7%, against plan 15's 3%.
      Missed: the ruled structural miss (below). The best a 1,250-term dictionary reaches on the
      same metric is 12.8% and 19.2% at c = 3 and 4.
    - Mass inside spheres of 0.5, 1, 2 and 4 scale lengths: within 0.136%, 0.197% and 0.249%
      at worst (1%); the whole mass within 0.136%, 0.192% and 0.245%.
    - Midplane force at the six radii: within 0.19%, 0.27% and 0.34% at worst (2%); held within
      2% at 17 more radii between 0.2 and 12 scale lengths.
    - Plan 02's checks for each row: every weight positive, widths strictly ascending, q
      positive (0.05–20).
    - The scratch header is `@provisional` by P15.T3.b, its acceptance line stating every figure
      and the miss.
  - _Deviations (T3.b)._
    1. **The density's 3% is out of reach** of a non-negative sum of axis-aligned centred Gaussians.
       Such a sum is log-convex in (R², z²), so on each circle r = const its ln ρ̂ at the diagonal
       is at most the mean of its two axis values, while the body's exceeds that mean by r (1 −
       2^(1/c − 1/2)) for every c > 2 (and is concave along the circle for 2 < c ≤ 4). Main ruled it
       a structural miss on 2026-10-23 (issue #94): weights stay non-negative, the `nonzero` reader
       and plan 02's non-negative test stay as they are, the fit leans towards the mass and the
       force, and `MGE_BOXY` is judged on those figures and on T3.c's two Milky Way checks.
    2. **Rows beyond the 48 × 12 grid's**: the monopole, the six forces, the central density and
       the whole mass. Without the last two, a first fit hid a spike of 16,000 times the central
       density inside the grid's first radius and a term of width 4 × 10¹⁰ outside its last; the
       whole mass is what T3.c's normalisation reads. The grid reaches 40 scale lengths so that
       the mass-weighted density rows hold back any faint excess out there.
    3. **Levenberg–Marquardt gains upper bounds** (`upper`, a changed signature, Provides above),
       for the boxy terms' widths and q, which a term with zero weight could otherwise send off
       to 10⁴⁷. **`optimise::least_squares`** is new and public. The one-dimensional polish's
       Jacobian now takes each shifted residual by `least_squares` on the columns NNLS left free
       at the point, the variable-projection derivative, instead of a fresh NNLS: the run fell
       from 6.2 s to 4.3 s, and every T3.a figure is the same to its printed digits (the bodies
       move). T3.a's note that the body at 100 iterations differed in the seventh digit was
       made before this change; `the_refit_meets_plan_fifteen_s_figures` still holds that the
       polish stops before its cap.
    4. **The references live in `tasks::mge::boxy`**, not in the test, so that the run's
       acceptance line can state every T3.b figure; the tests hold them to closed forms and to
       each other. The run reads the multipole rule, about 1 ms a point; the brute-force one
       takes about 0.2 s a point and is read by the tests alone.
    5. **The 300 iterations bind** for each boxy row. At 600 the figures move in their fourth
       digit and the run takes 8.1 s.
    6. **The seed splits after its weights are solved.** Solved after the split, the two halves
       of each split term, nearly the same column, left one at weight 0, whose width and q then
       had no gradient and stayed put: two of each row's 16 terms did nothing. Now every term
       carries weight.
    7. **A bound binds.** At c = 3.5 and 4 the narrowest term rests on the width floor, 0.001
       (with q 10.0 and 8.6), so those rows depend on `boxy_widths`; their flattest terms, q 0.053
       and 0.051, sit near the 0.05 floor.
    8. **The force's 2% is held against the multipole rule** at 1.9%, and the multipole rule to
       the brute-force integral at 10⁻⁶, so the plan's 2% against the brute force holds.
  - _The run (T3.a–b)._ `run mge` with `--manifest crates/hyperion-fit/manifests/mge.refit.toml`
    and `--out` to a scratch path under `target/` takes 4.3 s in the dev profile (8-thread
    laptop at load 5.7, shared). Its body is 8,849 bytes with SHA-256
    `b650018948a5ca7bf2144a0aa07d9a6f1173877e427a1c69565c9a4235d3a0f6`, the same in two runs.
    Its header is `@provisional` by P15.T3.b.
  - _Tests (T3.b)._ In `tests/mge.rs`, all fast, timed under nextest in the dev profile on 4 test
    threads at load 4.5–5.7: `the_boxy_references_hold_to_their_closed_forms_and_to_each_other` (5.2
    s), `mge_boxy_meets_plan_fifteen_s_mass_and_force_figures_and_holds_its_density_miss` (6.7 s),
    `mge_boxy_is_deterministic` (4.7 s; the boxiest row twice, compared as `Debug` text so that a
    zero's sign counts), and `the_refit_manifest_runs_end_to_end`, now with `MGE_BOXY`'s 48 terms,
    its `use` line and the header provisional by P15.T3.b (6.0 s).
    `the_refit_refuses_values_out_of_range` gains seven boxy cases: another number of terms, falling
    boxinesses, a split of q below 1 or outside the axis ratios, too few latitudes (two would leave
    fewer residuals than parameters) and a grid past 100 scale lengths. The T3.a tests now take
    2.4–4.9 s. In `optimise.rs`: the lower and upper bounds, and `least_squares` refusing a
    dependent or a zero column. `just fit-check` gives `41 fresh, 6 warnings, 0 failures` at the
    base and at the head, `mge` still provisional since 11.
  - _Deferred findings (2026-10-13 rule)._
    - The boxy grid's monopole, force and mass weights (3, 10, 3) trade the mass figure
      against the force figure: at 1, 3 and 10 for the force the worst force is 1.4%, 0.94% and
      0.33% and the worst mass 0.04%, 0.15% and 0.25% (c = 4). The values are the manifest's;
      T3.c's Milky Way checks may prefer another balance.
    - `MGE_EXP`'s smallest width is 0.0024 scale lengths. With T3.c's thin disc that puts a
      term near 20 ly, which the potential's quadrature takes, but nothing tests that narrow a
      term there yet. `MGE_BOXY`'s narrowest terms are 0.0010–0.0016 scale lengths, 2–3 ly at
      the Milky Way's s ≈ 1,907 ly, and its flattest reach q h ÷ s ≈ 0.02, a sheet about 10 ly
      thick; their mass is negligible, but nothing tests T3.c's quadrature on terms that narrow.
  - _For T3.c._
    - Move the refit's parameters from `mge.refit.toml` into `mge.toml`, `method` included, and
      bump the task's revision (Design note 6), so that the committed table is the refit's.
    - Add `MGE_BOXY` to `items()` (after `MGE_BAR`); the emitted body already starts with
      `use crate::galaxy::potential::mge::BoxyTerm;`.
    - Define `BoxyTerm { pub weight: f64, pub width: f64, pub q: f64 }` in
      `galaxy/potential/mge.rs`, its fields public, since the table builds it as a literal.
    - Build the bulge from `MGE_BOXY`, interpolating linearly in c between rows by the density,
      not term by term: the rows are sorted by width, so term k of one row has no counterpart in
      the next. Take both bracketing rows, their weights scaled by 1 − t and t, 32 Gaussians,
      each of width σ s and axis ratio q h ÷ s (the ruling's point 8), read through `nonzero`,
      and normalised to the bulge's mass.
    - Run plan 02's table tests on the new file. Its non-negative and ascending test,
      `the_expansions_have_non_negative_weights_and_ascending_widths`, gains `MGE_BOXY`; its
      1%, 0.1%, 3%, 0.5% and 7% checks hold on the refit's tables by
      `the_refit_passes_plan_two_s_table_tests`.
    - Make `mge_table_is_reproduced` reproduce the refit (it then runs the boxy fit as well,
      about 4 s), and keep `the_refit_*` tests or fold them into it.
    - The two Milky Way checks: the rotation curve against `bulge_spheroid`'s within 3% and
      `the_fixture_s_enclosed_masses_are_the_milky_ways`; the boxy figures here suggest the
      enclosed mass moves well under 1%.
- **T11.a as built (lane `lane/98-p15-t11a`, 2026-10-23, at `GENERATOR_VERSION` 23; no value
  moved, no bump).** The registration of the table that P10.T3.b committed by hand, by main's
  ruling of 2026-10-23 on P15.T11.a's brief (issue #98). `galaxy::global_list::orphans` reads the
  table as before, and nothing else of the sim changed.
  - _Registered._ Task `streams` (`tasks::streams::StreamsTask`): revision 0 (`VERSION`),
    `TaskClass::Fast`, the sim's `streams.rs`, one item, `ORPHAN_STREAMS_PER_GLOBULAR`, and an
    empty fingerprint. `run` checks the manifest, reads its one parameter,
    `scratch_streams_per_globular`, with `Manifest::f64`, refuses a value that is not positive and
    finite with a `ManifestParamError`, and emits it as a `TableItem::Scalar`, `@provisional` by
    P15.T11.a. `manifests/streams.toml` holds `scratch_streams_per_globular = 1.5`; there is no
    smoke manifest. `task::REGISTRY` holds 48 tasks, `streams` between `stellar_fates_mid` and
    `stripping`.
  - _The emission._ `cargo run -q -p hyperion-fit -- run streams --since 23`, in the dev profile
    (the bytes of `just fit streams`), which reported "body unchanged". The header is
    ``@provisional by hyperion-fit 0.1.0, task `streams` revision 0 (P15.T11.a)``, with
    `data: none`, `sim-fingerprint: none`, `since-generator-version: 23` (the hand-made table had
    none, so it took the current version) and the acceptance "scratch: the brainstorm's 1.5,
    unfitted until P15.T11.b". The lock entry's body SHA-256 is
    `afca2ac98e6e0ab584d93c38c354e16ac4717184497ce3c096d9cb6b1829cdac`, its inputs SHA-256
    `5b9e18c7…`; the `MANIFEST` row is revision 0, since 23, provisional.
  - _The body is P10.T3.b's, byte for byte_, the item's doc comment included: its SHA-256 is the
    same at the base. Only `//!` lines changed. The summary is the committed one, the fit header is
    new, and the notes are rewritten: provisional by P15.T11.a, what P15.T11.b fits it against, and
    a parameter of the generator version whatever the fit says.
  - `just fit-check` gives `41 fresh, 6 warnings, 0 failures` at the base (37a38068) and
    `41 fresh, 7 warnings, 0 failures` at the head, the seventh `streams`, provisional since 23.
  - _Tests_, under nextest in the dev profile on an 8-thread laptop at load 5, four test threads (a
    shared machine, so provisional). In `tests/streams.rs`: `streams_table_is_reproduced` (0.007
    s) and `the_committed_multiplier_is_the_brainstorms_one_and_a_half` (0.006 s; 1.5 by
    `assert_same_bits` against the test's own literal). In `tasks::streams`, each under 0.02 s:
    `the_table_is_the_manifests_multiplier`, `the_run_emits_the_scratch_value_provisionally`,
    `the_run_refuses_a_manifest_without_the_multiplier`,
    `the_run_refuses_a_multiplier_that_is_not_positive_and_finite` (0, −1.5, ∞ and NaN) and
    `the_run_refuses_another_tasks_manifest`. The registry's pinning test lists `streams`, and
    `the_committed_tables_are_fresh` (15–19 s) and `every_committed_table_declares_its_tasks_items`
    pass with it.
  - _Deviations._
    1. **The notes' census.** Checked against both sources: "over 120" is Bonaca and
       Price-Whelan's (2025, New Astron. Rev. 100, 101713, §2.2) count of the streams reported,
       wide and thin together, and Mateu (2023, MNRAS 520, 5225) lists 95. "Uncertain threefold" is
       this plan's, not a source's. The notes say so, where the committed ones said "the census of
       thin streams (over 120 known and far from complete; Bonaca and Price-Whelan 2025, Mateu
       2023), uncertain threefold".
    2. **The header's `source`** names the brainstorm's section and file, not its line (795 at the
       base), which moves as the brainstorm is edited.
  - _Deferred findings (2026-10-13 rule)._ P15.T11's Source line, "The census of thin streams:
    over 120 known (Bonaca and Price-Whelan 2025; Mateu 2023)", has the wording of deviation 1: the
    120 counts every stream and is Bonaca and Price-Whelan's alone. It is a wording fix for a lane
    whose scope holds that line. The brainstorm's own sentence cites them alone and is right.
  - _Records left as written, now stale in part._ Provides' "Built at 898a3c94" still lists
    `streams` as hand-made, for P15.T11.a to register, and `task::REGISTRY` at 47 tasks; this record
    supersedes both.
  - _For T11.b._
    - Replace `scratch_streams_per_globular` with the fit's parameters (the grid of k on 0.5–4.5,
      the detectability cut's constants), and bump `VERSION` to 1: the inputs hash covers the
      manifest and the revision, not the code (Design note 6).
    - The item's doc comment is fixed text that quotes 1.5, so `render` must write the fitted
      value and its source into it. `the_committed_multiplier_is_the_brainstorms_one_and_a_half`
      pins 1.5, so a moved value changes that test, and its name, with the bump.
    - A fit that runs plan 10's generator gains `fingerprint()` probes of the code it runs, as
      `displaced_forms` and `kick_rank` do. If it runs over a minute it is `Slow`, and gains
      `manifests/streams.smoke.toml`, a smoke test, and a slow `streams_table_is_reproduced`, as
      `equipartition_block_is_reproduced` is.
    - `provisional` becomes `None`, or `Some("P15.T11.b")` while the acceptance misses.
    - `data/stream_census/` should mark each stream globular-like or not, explicitly: Bonaca and
      Price-Whelan's census includes dwarf-galaxy streams, and mass alone does not split them.
