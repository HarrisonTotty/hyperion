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
- Any code inside `hyperion-sim` other than `tables/`, the `fit.*` entries of `rng/tags.rs`, and the
  one-line repointing of a consumer's constant at its table. Where a fit needs the generator's own
  code (potential, tracks, kick law, binary formulae, stream spray) it calls the owning plan's
  public API and never copies it.
- New physics. A fit chooses constants for forms the brainstorm already fixes.

## Provides

### The crate `hyperion-fit` (`crates/hyperion-fit`)

Created by P02.T6.a and extended here: a binary with a thin `main.rs` over a library, as
`rust-dev.md` requires. It depends on `hyperion-sim`. Nothing may depend on it: a test fails if
`hyperion-sim`, `hyperion-protocol` or `hyperion-server` names it in `Cargo.toml`.

```text
hyperion-fit list                      # tasks, class (fast or slow), table path, state
hyperion-fit run <task> [--since <generator version>] [--smoke] [--threads N] [--out PATH]
hyperion-fit check [--rerun-fast]      # staleness; see Design note 8
hyperion-fit fingerprint <task>        # prints the sim probe values a task depends on
```

`run mge --out <path>` keeps working as plan 02 defined it. `--since` is required whenever the
output goes into the sim's `tables/` (the default) and is not needed with `--out` elsewhere.

```rust
hyperion_fit::task::{FitTask, TaskClass, TaskOutput, RunTaskError, registry}
hyperion_fit::manifest::{Manifest, InputsHash, LockFile, LockEntry, SimFingerprint}
hyperion_fit::emit::{RustTable, Header, HeaderKind, write_table, EmitTableError}
hyperion_fit::parallel::map_reduce_chunks                      // deterministic reduction
hyperion_fit::optimise::{nnls, nelder_mead, levenberg_marquardt}
hyperion_fit::data::{Dataset, Provenance, load_dataset, LoadDatasetError}
hyperion_fit::stats::{total_variation, rms_log_residual, ks_distance}

pub trait FitTask {
    fn name(&self) -> &'static str;                 // "mge", "displaced", ...
    fn class(&self) -> TaskClass;                   // Fast (under 60 s; CI reruns it) or Slow
    fn table_path(&self) -> &'static str;           // relative to crates/hyperion-sim/src/tables/
    fn revision(&self) -> u32;                      // bumped by hand when the algorithm changes
    fn fingerprint(&self) -> SimFingerprint;        // empty for tasks that use only `math`
    fn run(&self, manifest: &Manifest, threads: NonZeroUsize) -> Result<TaskOutput, RunTaskError>;
}
```

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

| Module                     | Shape owned by | Consumer    | Contents                                        |
| -------------------------- | -------------- | ----------- | ----------------------------------------------- |
| `tables::mge`              | plan 02        | 02, 08      | `MGE_EXP`, `MGE_BAR`; this plan adds `MGE_BOXY` |
| `tables::chabrier`         | this plan      | 02, 11      | `HIGH_MASS_BRANCH_SCALE`                        |
| `tables::kick_rank`        | plan 06        | 06 (08, 09) | `SCORE_QUANTILES: [f64; 257]`, four defaults    |
| `tables::helium`           | plan 06        | 06, 09      | `LIFETIME_SLOPE`, `HB_TEMPERATURE_SHIFT`        |
| `tables::displaced_forms`  | this plan      | 08          | see P15.T6                                      |
| `tables::cluster_dynamics` | this plan      | 09          | `BH_LOSS_*`, equipartition, `PULSAR*`           |
| `tables::type_ia_delay`    | this plan      | 09, 11      | delay-first samplers, `DELAY_EDGES`             |
| `tables::lbv`              | this plan      | 09          | `LBV_SAMPLER`, the luminous blue variables'     |
| `tables::binary`           | plan 11        | 11          | `IA_YIELD`, four samplers, `CLASS_SHARES`       |
| `tables::streams`          | this plan      | 10          | `ORPHAN_STREAMS_PER_GLOBULAR`                   |

The owner of a shape states it in its own Provides, and this plan's task group restates what the fit
must fill. P15.T2 checks each against the code as built, and the owner's code wins.

### Recipes and CI

- `just fit <task>`: `cargo run --release -p hyperion-fit -- run <task> --since <current>`, writing
  into the sim's `tables/` and updating `crates/hyperion-fit/tables.lock`.
- `just fit-check`: `cargo run -p hyperion-fit -- check`. Part of `just ci`, and a step of the
  `rust` job in `.github/workflows/ci.yml`. It reruns no fit. Its cost is the fingerprints' probes,
  a few seconds at most, since the displaced table's probes build plan 02's full potential once.
- `just test-slow` additionally runs `hyperion-fit check --rerun-fast`.

## Consumes

- **Plan 01:** `hyperion_sim::math` (every transcendental in a fit goes through it, so a table is
  bit-identical on every platform); `rng::{Seed, Stream, ObjectKey, tags}` with
  `Stream::open(seed, tag, key)`: the fit's Monte Carlo draws use the sim's generator on tags named
  `fit.<task>.<purpose>`, which each task group adds to plan 01's single registry in `rng/tags.rs`
  under a "Plan 15" heading, with scope `Galaxy` and the key `ObjectKey::galaxy_item(chunk index)`,
  so the collision test covers them; `GENERATOR_VERSION`; from `hyperion-testkit`, `golden!` and
  `stats`; the slow-test marking and `just test-slow`.
- **Plan 02:** the crate `crates/hyperion-fit` as P02.T6.a leaves it (`run mge [--out]`,
  `RunFitError`, `tasks::mge::{fit, render, MgeTables}`, the test `mge_table_is_reproduced`) and
  `tables/mge.rs` with `MGE_EXP` and `MGE_BAR`; `GalaxyParams::milky_way_like` and
  `Galaxy::from_params`; from `potential`, `MassModel` (`v_circ_sq`, `potential`, `vertical_force`,
  `enclosed_mass`) and `PotentialTables` (`v_circ`, `omega`, `kappa`, `potential`, `escape_speed`,
  `bar_pattern_speed`, `bar_corotation`); `fields` (component densities) and
  `ages::AgeDistribution`; `imf::{MassFunction, Kroupa, Chabrier}`, whose provisional scale of 0.68
  P15.T4 replaces, and `fates::mean_present_mass`.
- **Plan 06:**
  `stellar::remnant::reference::{ReferencePopulation, score_quantiles, kick_observables}` (P15.T5
  calls the same `score_quantiles(pop, n, seed)` that made the provisional table, and the six
  kick-law tests of P06.T19.d through `kick_observables`), `KickLawParams`, `StandardKickLaw`,
  `stellar::sse` tracks, `stellar::lifetime`, `StarDraws::median`, `SystemStars::lbv_window`, the
  shapes `tables::kick_rank::SCORE_QUANTILES` and
  `tables::helium::{LIFETIME_SLOPE, HB_TEMPERATURE_SHIFT}`, and the provisional and identity files
  P06.T19.b and P06.T17 commit, whose ownership passes to this plan.
- **Plan 08:** nothing. Plan 08 consumes `tables::displaced_forms`. The orbit integrator here is the
  fit's own and reads only plan 02's potential.
- **Plan 09:** the cluster class code and its scratch constants (black-hole loss, equipartition,
  pulsar counts), its scratch Type Ia samplers, which P15.T8 and P15.T9.a replace.
- **Plan 10:** the stream generator, for P15.T11; its orphan multiplier.
- **Plan 11:** the shapes `IaYieldTable`, `ClassSamplerTable`, `ClassShareTable` and `IaPoolChannel`
  and the scratch contents at `tables/binary.rs` (`IA_YIELD`, `AWD_SAMPLER`, `XRB_SAMPLER`,
  `MERGER_SAMPLER`, `NSM_SAMPLER`, `CLASS_SHARES`);
  `stellar::binary::{MergedBinaryFate, CLUSTER_MERGED_BINARY_FATE}`, the default P15.T5.c reviews;
  the binary engine run forward; `multiplicity::stripped_share` and the all-stars mass-function
  quadrature that P15.T4 fits against; `stellar::binary::testing::brute_force_class_members`, which
  plan 11 exports behind `hyperion-sim`'s `testing` feature because a file under the sim's
  `tests/common/` cannot be reached from another crate. `hyperion-fit` enables that feature in its
  own dependency on the sim.

Plans 09 to 11 were written in parallel with this one. Each task that calls their code begins by
reading their Provides as built.

## Design notes

1. **Dependency direction.** `hyperion-fit` depends on `hyperion-sim`; the reverse is forbidden and
   tested. The bootstrap problem (plan 02 needs a table before this plan starts) is solved by plan
   02 creating the crate with its one task, as above.
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
   irreproducible in a way the lock file cannot see. `Cargo.lock` pins the rest.
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
   to its table at the same value, which changes no output, and then fits it.
10. **Versioning with `GENERATOR_VERSION`.** A table belongs to the generator version. A commit that
    changes the body of a table which generated output reads must bump `GENERATOR_VERSION`, set the
    table's `since-generator-version` to the new value and regenerate goldens, all together. Three
    guards: the emitter refuses to overwrite a table unless `--since` is given and equals the sim's
    current constant, and refuses if the new body differs from the old while `--since` equals the
    old table's value; `check` refuses a table whose `since` is ahead of the constant; and the
    golden tests fail when a star moves. A fit whose output is byte-identical to the committed table
    changes nothing and needs no bump.
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
- `justfile`: `fit-check`, added to `ci`; `test-slow` gains `--rerun-fast`. `ci.yml`: a
  `just fit-check` step in the `rust` job. `.gitignore`: `crates/hyperion-fit/data/cache/`.

**Files.** `crates/hyperion-fit/src/{emit,manifest,cli}.rs`, `crates/hyperion-fit/tables.lock`,
`crates/hyperion-sim/src/tables/mod.rs`, `justfile`, `.github/workflows/ci.yml`, `.gitignore`.

**Tests.** `emitted_literals_round_trip` (strip separators, parse, compare bits, over 10⁴ values
including subnormals and powers of ten); `emitted_file_passes_header_grammar`;
`check_reports_hand_edited_table`, `check_reports_changed_manifest`,
`check_reports_fingerprint_drift`, `check_rejects_since_version_above_current`, each against a
temporary directory with a toy task; `emitter_refuses_changed_body_without_new_since`;
`two_tasks_share_a_file_without_touching_each_other`.

**Acceptance.** `just ci` green including `just fit-check`. The first real table (P15.T3) is the
proof that emitted source passes `cargo fmt --check` and Clippy pedantic; until then a toy table is
checked once by hand in the pull request.

### P15.T3 Gaussian-sum coefficients of each density profile

**Source.** The profiles of the brainstorm's "Populations" themselves. The method is the
multi-Gaussian expansion (Emsellem, Monnet and Bacon 1994; Cappellari 2002; neither is in the
brainstorm's list, so re-check when coding). The closed form used for acceptance is Freeman (1970).

**Consumer.** Plan 02, `galaxy::potential` (it runs in M1 on P02.T6.a's first cut, and this replaces
it at the start of M2); plan 08's Jeans tables read the same expansions as tracers.

**Format** (`tables::mge`, lengths in units of the profile's own scale). `MGE_EXP` and `MGE_BAR`
keep plan 02's shape, `[(f64, f64); N]` of (weight, width). New:

```rust
/// Σ weight × exp(−(R² + z² ÷ q²) ÷ 2 width²) ≈ exp(−(R^c + z^c)^(1 ÷ c)).
/// R and z are in scale lengths.
pub struct BoxyTerm { pub weight: f64, pub width: f64, pub q: f64 }
pub const MGE_BOXY: [(f64, [BoxyTerm; 16]); 3];            // boxiness c = 3.0, 3.5, 4.0
```

- **P15.T3.a Refit the one-dimensional expansions.** The task already lives at `src/tasks/mge.rs`
  (P02.T6.a) and is a `FitTask` since P15.T1. Give it a manifest, then refit with free widths in
  place of plan 02's fixed logarithmic ones: weights by non-negative least squares on 400
  logarithmically spaced points of s in [10⁻³, 12], widths polished by Levenberg–Marquardt in log
  width. **Acceptance:** maximum relative error of e^(−s) under 0.5% on 0.05–10 with at most 14
  terms; the three-dimensional mass of the spheroidal exponential to 0.05%; the circular speed of a
  thin exponential disc built from the table, by the one-dimensional quadrature plan 02 uses, within
  0.5% of Freeman's Bessel-function form over 0.2–8 scale lengths (Bessel functions by series and
  asymptotic forms in the task's test module, through `math`); the bar profile (plan 02's: the
  azimuthal average of the long bar's surface density at width ratio 0.1, level to 0.85 of the
  half-length with a Gaussian end of 0.15 half-lengths) to 2% and its mass to 0.3%; plan 02's own
  table tests (1%, 0.1%, 3%, 0.5%) still pass, and `mge_table_is_reproduced` passes against the new
  file.
- **P15.T3.b The boxy bulge in two dimensions.** For each boxiness c, fit 16 terms with free width
  and q on a polar grid of 48 × 12 points by Levenberg–Marquardt from the one-dimensional solution,
  weights kept non-negative by a projected step. Plan 02 then drops its equal-second-moments
  spheroid (its D6) for this expansion, interpolating linearly in c. **Acceptance:** mass-weighted
  rms relative density error under 3%; enclosed mass within spheres of 0.5, 1, 2 and 4 scale lengths
  within 1% of a direct integral of the profile; midplane radial force within 2% of a brute-force
  integral at six radii; at Milky Way values the rotation curve changes by under 3% anywhere and
  plan 02's enclosed-mass test from 1 pc to 2 kpc still passes.
- **P15.T3.c Swap in.** One commit: the table with `--since` the bumped version, plan 02's
  `MassModel` reading `MGE_BOXY` in place of its moment-matched spheroid, `GENERATOR_VERSION`
  bumped, goldens regenerated. Every star moves, because the potential sets the sub-disc heights.
  T3.a and T3.b are developed and reviewed with `--out` to a scratch path, so `just ci` stays green
  until this commit.

**Files.** `src/optimise.rs`, `src/tasks/mge.rs`, `manifests/mge.toml`, `tables/mge.rs`,
`galaxy/potential/model.rs` (T3.c's swap).

**Acceptance.** `just ci` and `just test-slow` green; the task is `Fast` (under ten seconds) and is
rerun by `--rerun-fast`.

### P15.T4 Chabrier high-mass branch scale

**Source.** The observed single-star mass function (Kroupa 2001; Chabrier 2003) and the observed
multiplicity (Duchêne and Kraus 2013; Raghavan et al. 2010), as "Sizing the layers" uses them: of
all stars, companions included, 75.9% lie below 0.5 M☉.

**Consumer.** Plan 02, `imf::Chabrier`. **Format** (`tables::chabrier`):
`pub const HIGH_MASS_BRANCH_SCALE: f64;` multiplying the branch above 1 M☉.

- **P15.T4.a Move the constant.** Plan 02's 0.68 becomes the table, provisional, same value.
- **P15.T4.b Fit (M4, after plan 11).** With plan 11's multiplicity model and its all-stars
  quadrature, solve for the scale s by bisection on the fraction below 0.5 M☉. The quadrature is
  deterministic, so no sampling is needed. **Acceptance:** fraction below 0.5 M☉ within 0.3 points
  of 75.9%; s inside 0.6–0.75 (otherwise the task fails and the discrepancy is reported, never
  clamped); mean present-day mass per system under the scaled function within 0.53–0.57 M☉ by plan
  02's `mean_present_mass`. Bumps the version.

**Files.** `src/tasks/chabrier.rs`, `manifests/chabrier.toml`, `tables/chabrier.rs`,
`galaxy/imf.rs`.

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
commits it provisionally from 10⁶ draws. The law's four defaults are not in the table: three are
fields of `KickLawParams::default` and the fourth is plan 11's `CLUSTER_MERGED_BINARY_FATE`.

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
  plan 11's fate of merged binaries in clusters. The harness evaluates plan 06's six test
  observables on a grid of each default about its value (one at a time, the rest held) and emits a
  sensitivity table into `tables.lock` as acceptance figures. A default moves only if an observable
  fails at the brainstorm's value, in the order ramp, windows, factor, and the move is a change to
  `KickLawParams::default` with a bump. **Acceptance:** at the committed defaults all six
  observables of P06.T19.d pass; for each default the harness reports the interval over which they
  keep passing, and the default lies inside it and not within a tenth of its width from an edge.
- **P15.T5.c Fate of merged binaries in clusters (M4).** With plan 09's cluster retention and plan
  11's engine: neutron-star retention of 18–26% at a birth escape speed of 100 km/s, 13–19% at 50,
  8–12% at 20, and under 1% in the most massive open clusters, under plan 11 (the merged object
  stays a member and is judged on the pair's velocity) and under the alternative. **Acceptance:**
  the default passes all four bands; if only the alternative does, the finding goes to plan 11.

**Files.** `src/tasks/kick_rank.rs`, `src/tasks/kick_defaults.rs`, `manifests/kick_*.toml` with
their smoke manifests, `tables/kick_rank.rs`. All three are `Slow`: 10⁷ tracks are tens of minutes
on one core, so CI never reruns them and relies on the hash, the fingerprint and the smoke run.

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
  populations' density convolved with 1 ÷ (4π r² v), fitted by one `CoredPowerLaw`. **Acceptance:**
  misplaced share under 10%; at Milky Way rates and the brainstorm's 30% channel share the number
  inside the cube is some tens of thousands (10⁴–10⁵).

**Files.** `src/tasks/displaced/{mod,orbits,births,histogram,fit_disc,fit_old,kinematics}.rs`,
`manifests/displaced.toml`, `manifests/displaced.smoke.toml`, `tables/displaced_forms.rs`.

No scratch stand-in exists for this table. It is the long pole of M3 and starts as soon as plan 02's
potential is numerically stable, in parallel with M2.

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

- **P15.T7.a Select and record the source** and its terms, as a _committed_ or _fetched_ dataset.
- **P15.T7.b Fit.** Least squares on the grid's turn-off ages and zero-age horizontal-branch
  temperatures relative to plan 06's tracks at ΔY = 0. **Acceptance:** lifetime ratio reproduced to
  3% for 0.6–0.9 M☉, ΔY up to 0.18 and [Fe/H] from −2.2 to −0.5; every correction exactly the
  identity at ΔY = 0 by its form, so grid stars stay bit-identical (plan 06's golden); ΔY = 0.1
  shortens the lifetime of a 0.8 M☉ star by about a third (0.25–0.4), the sign plan 06 documents; a
  synthetic cluster of 10⁶ M☉ with ΔY up to 0.18 shows a horizontal branch whose blue end is hotter
  than its first population's by the grid's amount to 10%.

**Files.** `src/tasks/helium.rs`, `manifests/helium.toml`, `data/<grid>/`, `tables/helium.rs`. Bumps
the version when it lands, although no grid star changes, because members with a helium excess do.

### P15.T8 Cluster dynamics constants

**Consumer.** Plan 09 (M3). **Format** (`tables::cluster_dynamics`): `BH_LOSS_BETA`,
`BH_LOSS_PSI_SLOPE`, `BH_RELAXATION_PREFACTOR`; `EQUIPARTITION_EXPONENT`; `PULSARS_AT_47_TUC_GAMMA`,
`PULSAR_GAMMA_EXPONENT`, `PULSAR_CORE_COLLAPSE_CAP`. T8.a first moves plan 09's scratch constants
here unchanged (2.8 × 10⁻³, 147, 0.138; full equipartition; 40, 0.7, and a cap equal to the count at
47 Tucanae's Γ), whatever plan 09 has called them.

- **P15.T8.a Black-hole loss against the CMC Cluster Catalog** (Kremer et al. 2020). Dataset:
  _fetched_; a reduction script extracts, per model and snapshot, mass, half-mass radius, number and
  mass of black holes and age. Fit the closed form of Breen and Heggie (2013) as parametrised by
  Antonini and Gieles (2020), f(t) = [(1 + ψ₁ f₀) e^(−β ψ₁ t ÷ t★) − 1] ÷ ψ₁ floored at zero, with
  t★ = c √(M r_h³ ÷ G) ÷ (⟨m⟩ ln Λ), for β (`BH_LOSS_BETA`), ψ₁ (`BH_LOSS_PSI_SLOPE`) and c
  (`BH_RELAXATION_PREFACTOR`), exactly the three constants plan 09's P09.T9.c reads; the decay rate
  β ψ₁ (0.41 at the scratch values) is derived and is not emitted. **Acceptance:** rms error in
  log₁₀ of the retained black-hole number under 0.3 dex over models that retain any at 12 Gyr; at
  least 85% of models predicted empty are empty; applied to the Baumgardt–Hilker catalogue at 12
  Gyr: none in dynamically old clusters, tens to a few hundred in a typical massive one, thousands
  in ω Centauri, and about a fifth of the catalogue (15–25%) beyond the core-collapse line of 14
  relaxation times with no black holes (Trager et al. 1995).
- **P15.T8.b Partial-equipartition exponent against multimass King models.** The tool integrates the
  models itself (Poisson's equation with one lowered-Maxwellian component per mass class, in the
  manner of Gunn and Griffin 1979 and Da Costa and Freeman 1976; neither is in the brainstorm's
  list, so re-check) over concentrations 0.7–2.3 and the mass-function slopes of "What is inside a
  cluster today", and fits η in the class profile (1 + r² ÷ r_c²)^(−3 q^η ÷ 2) for q below 1, with η
  = 1 above. **Acceptance:** half-mass radius of each class below the turn-off reproduced to 10%; η
  between 0.3 and 1.
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
  in place returns it within its error (the mark thins a pool it does not shape).

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
  against the brute-force members.
- **P15.T10.b Binary classes (M4, consumer plan 11).** Fill `AWD_SAMPLER`, `XRB_SAMPLER`,
  `MERGER_SAMPLER`, `NSM_SAMPLER` and `CLASS_SHARES` of `tables::binary` in plan 11's shapes
  (`ClassSamplerTable`, `ClassShareTable`), one sub-task per class. **Acceptance per class:** the
  galaxy-wide count at Milky Way values inside the brainstorm's range (about 10⁴ X-ray binaries with
  about 1,300 black-hole transients; 6–12 × 10⁶ accreting white dwarfs; 0.2–0.5 luminous red novae a
  year; about 10 neutron-star merger entries); and for each mark a two-sample Kolmogorov–Smirnov
  test at the 1% level between hosts drawn through the table and plan 11's
  `stellar::binary::testing::brute_force_class_members` over 10⁷ systems.

**Files.** `src/tasks/{lbv,binary_classes}.rs`, manifests, `tables/lbv.rs`, `tables/binary.rs`.
`tables/binary.rs` is filled by two tasks (T9.b and T10.b) with disjoint item lists. Each task's
items sit between its own pair of marker comments under its own header block, the emitter rewrites
only its own block, and the lock file holds one entry and one body hash per block. P15.T2's emitter
and `check` support this from the start, and a test covers two toy tasks sharing a file.

### P15.T11 Orphan streams per globular cluster

**Source.** The census of thin streams: over 120 known (Bonaca and Price-Whelan 2025; Mateu 2023)
and far from complete. **Consumer.** Plan 10 (M3). **Format** (`tables::streams`):
`ORPHAN_STREAMS_PER_GLOBULAR: f64`, the brainstorm's 1.5, uncertain threefold.

- **P15.T11.a Move plan 10** to the table, unchanged.
- **P15.T11.b Fit.** With plan 10's generator at Milky Way values, for a multiplier k on a grid from
  0.5 to 4.5: generate the streams, observe from the plane at 8 kpc, apply a detectability cut (mass
  above 10³ M☉ within 30 kpc, axis density at least ten times the smooth halo's, more than 15° from
  the plane; the cut's constants are manifest parameters), count, and choose k to match the known
  globular-like streams with the Poisson error. **Acceptance:** k inside 0.5–4.5 with a stated
  one-sigma interval; at the chosen k globular tubes hold 0.3–1% of the halo's budget. The value
  stays a parameter of the generator version whatever the fit says, and the header states how far
  the detectability cut moves it. Bumps the version if the value moves.

**Files.** `src/tasks/streams.rs`, `manifests/streams.toml` and its smoke manifest,
`data/stream_census/` (the counts used, _committed_ as facts with their citations),
`tables/streams.rs`, and plan 10's constant repointed at the table (T11.a).

### P15.T12 Close-out

No provisional table remains; `check` treats `provisional: true` as an error from here on; every
header carries its acceptance figures. `hyperion-fit list` prints name, revision, class, runtime and
acceptance for every table, pasted into the pull request. **Acceptance:** `just ci` and
`just test-slow` green with zero provisional tables.

### Schedule

| Table                 | Task         | Consumer | Needed in | Needs  | Class | Scratch from      |
| --------------------- | ------------ | -------- | --------- | ------ | ----- | ----------------- |
| **MGE, first cut**    | **P02.T6.a** | **02**   | **M1**    | 01     | fast  | itself            |
| Toolchain             | P15.T1, T2   | all      | start M2  | 01, 02 | n/a   | n/a               |
| MGE, final and boxy   | P15.T3       | 02, 08   | M2        | 02     | fast  | P02.T6.a          |
| Kick ranks, defaults  | P15.T5.a, b  | 06       | M2        | 06     | slow  | P06.T19.b         |
| Helium correction     | P15.T7       | 06, 09   | M3        | 06     | fast  | P06.T17           |
| Displaced forms       | P15.T6       | 08       | M3        | 02, 06 | slow  | none: start in M2 |
| Cluster dynamics      | P15.T8       | 09       | M3        | data   | fast  | plan 09           |
| Type Ia delay         | P15.T9.a     | 09       | M3        | 06     | fast  | plan 09           |
| LBV sampler           | P15.T10.a    | 09       | M3        | 06     | fast  | plan 09           |
| Orphan streams        | P15.T11      | 10       | M3        | 10     | slow  | plan 10           |
| Chabrier branch scale | P15.T4       | 02       | M4        | 11     | fast  | plan 02           |
| Merged-binary fate    | P15.T5.c     | 09, 11   | M4        | 09, 11 | slow  | plan 11           |
| Type Ia yield, mark   | P15.T9.b     | 11       | M4        | 11     | slow  | plan 11           |
| Binary class samplers | P15.T10.b    | 11       | M4        | 11     | slow  | plan 11           |

Only the displaced table has no scratch stand-in, because its consumer cannot do anything useful
without it. Everything else lets its consumer proceed on a scratch value and costs one version bump
when the fit lands.

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
same value is not a bump. P15.T6 lands with plan 08's own bump.

Reserved so that later tables move nothing they need not: table paths and item names are final from
the scratch stage on; the node arrays of `displaced_forms`, so that a finer dependence on the halo
or the bar adds nodes and not fields; `HYPERVELOCITY`, which plan 08 registers at zero weight until
plans 09 and 11 feed it; and the domain-tag prefix `"fit."`, which no generator code uses.

## Risks and open points

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
