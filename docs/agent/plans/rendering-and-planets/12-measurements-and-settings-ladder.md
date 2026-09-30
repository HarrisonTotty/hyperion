# Plan R12: Measurements and the Settings Ladder

- **Milestone:** Rendering milestone RM6 (with R11).
- **Depends on:** every earlier rendering plan, R01–R11, as built; through them galaxy plans 04, 06,
  12 and 14. The runs need, in particular, R05's scripted descent, its metrics harness and its
  results files, R07's several views and per-view budgets, R10's terrain on generated worlds and
  R11's clouds, oceans and rings. The main screen's run waits on R07's main screen, which waits on
  the single-player sessions, ship state and closed-loop commands that have no plan yet.
- **Brainstorm sections covered** (by heading, in
  [the brainstorm](../../brainstorming/rendering-and-planets.md)):
  [Performance budget](../../brainstorming/rendering-and-planets.md#performance-budget), all of it,
  as the plan that replaces its estimates; the performance runs and the resident-memory run of
  [Testing](../../brainstorming/rendering-and-planets.md#testing) ("With a GPU, by hand, and
  recorded"); the station wireframe's 60 fps lean of
  [Two deployments, one scene](../../brainstorming/rendering-and-planets.md#two-deployments-one-scene);
  "whether the cockpit and its instruments fit the 33 ms frame together" of
  [Several views in one client](../../brainstorming/rendering-and-planets.md#several-views-in-one-client);
  "The budget" under [Decisions](../../brainstorming/rendering-and-planets.md#decisions); step 10 of
  [Suggested order of attack](../../brainstorming/rendering-and-planets.md#suggested-order-of-attack).

## Goal

When this plan is done, every figure in the brainstorm's performance budget — the frame-time table
on both columns, the memory table, the height workers' cost and the station wireframe's row — has a
measured counterpart on the UHD 620 and on an RTX 4060-class discrete GPU, recorded with the
machine, the driver, the power and thermal state, the setting, the commit and the generator version
that produced it, in a results file under version control, from which a generated table sets
estimate beside measurement. The three performance runs the brainstorm's Testing section names exist
as scripted, seeded scenes that anyone can re-run: the single-player cockpit measured together with
its wireframe instruments, a station's wireframe during a descent measured alone, and the resident
memory against the ceiling, each worker's copy of the coarse field included. A regression shows as a
number, with its effect size, beside the last comparable run. The ladder's settings are adjusted to
what the runs show, by a stated rule, and every earlier plan's low setting has been audited against
the budget's three rules, the third above all: that it was built alongside its high setting and is
still exercised, so that it has not rotted.

## Scope and non-goals

In scope:

- The perf-run mode of the client: a command-line entry that loads a scene from a catalogue, checks
  the machine is quiet, warms up to a steady state, runs a stated number of repetitions and writes
  one record per scene.
- What R05's harness does not already measure: the itemisation of its allocation tally by the
  budget's memory table, the shared-memory bound and the display server's line beside its fdinfo
  reader, NVIDIA's graphics-process memory, the power and thermal state with the throttle counters,
  and the machine's identity.
- The scene catalogue: the cockpit descent, the station wireframe descent, a ringed gas giant's
  approach, the arrival with its coarse field posted to three workers, and the nuclear-disc sky as
  an informative run; the pinned bodies each needs, with a test that the pins still hold.
- The consolidated results record (`runs.v1.jsonl`), its validator and table writer, the generated
  budget table (`budget.md`), the fold of R05's and the other plans' own recorded runs into it, and
  a comparison tool that states a regression as a number.
- The consolidated runs on the UHD 620 and on the discrete reference machine.
- The ladder's adjustment, by a rule, in the values of the settings the owning plans built; and a
  drafted revision of the brainstorm's budget tables for the owner.
- The audit of every earlier plan's low setting against the budget's three rules, mechanical where
  it can be and written where it cannot.

Non-goals:

- **Any feature's low setting.** R01–R11 each build their own feature's low setting and record their
  own benchmarks; this plan measures them together, and where a low setting is missing or untested
  it files the finding with the owning plan rather than building it.
- **The descent spike's pass criterion** (R05 Design note 21) and open question 2's decision rule
  (R05 Design note 22), which are R05's. The consolidated runs use them unchanged, and a discrete
  result that meets open question 2's condition goes back to R05's rule, not to a decision here.
- **The measurement machinery R05 builds**: `contentTracing` and its reducer, the frame-interval and
  per-pass timers, the pipeline shim, the fdinfo reader. This plan extends them and does not
  duplicate them.
- **Still images** and their soak at the batch cap, which are R11's.
- **Golden images and performance gates in CI.** The runs are by hand and recorded, as the
  brainstorm's Testing section requires. What runs in `just ci` is the arithmetic: the record's
  validator, the table's agreement with the record, the parsers, the comparison rule and the pins.
- **The 4K main screen** as a scene of this plan's first pass. It waits on R07's main screen and so
  on the sessions (Risks).
- **The single-player server's default pool cap.** The runs pass their cap explicitly and record it;
  the default belongs to the sessions work, which receives the measured recommendation as an ask.

## Provides

TypeScript paths are under `apps/hyperion/` unless a path says otherwise. Signatures are sketches.

### The run record (`perf/record.ts`)

```ts
export const PERF_RECORD_VERSION = 1;
export interface PerfRunRecord {
  readonly version: 1;
  readonly id: string; // `${date}-${machine}-${scene}-${n}`
  readonly date: string; // ISO 8601, UTC
  readonly commit: string; // git rev-parse HEAD; `dirty` flags a changed tree
  readonly dirty: boolean;
  readonly generatorVersion: number;
  readonly source: "perf-run" | "descent-spike" | "plan-benchmark"; // T1's fold of R05's files
  readonly scene: { readonly id: PerfSceneId; readonly version: number; readonly seed: string };
  readonly setting: SettingSnapshot; // R05's `QualitySetting` and every later plan's entries
  readonly views: readonly PerfViewRecord[]; // style, size, internal scale, rate (R07 note 14)
  readonly machine: MachineRecord; // Design note 5
  readonly state: PowerThermalRecord; // Design note 7
  readonly harness: HarnessRecord; // Design notes 4, 7, 8
  readonly runs: readonly PerfRepetition[]; // Design note 7
  readonly notes: string;
}
export interface HarnessRecord {
  readonly gpuTiming: boolean; // R01's `--hyperion-gpu-timing` on
  readonly timestampQuantumNs: number; // 65,536 when quantised, 1 when not
  readonly repetitions: number;
  readonly warmUp: { readonly seconds: number; readonly steady: boolean }; // `NOT STEADY` if false
  readonly forced: boolean; // a precondition was overridden with `--force`
  readonly pacing: "every-vsync" | "every-second-vsync"; // R05 Design note 21
  readonly server: {
    readonly placement: "local" | "remote" | "co-resident" | "none";
    readonly numWorkers: number | null;
  };
  readonly heightWorkers: number;
  readonly display: "physical" | "virtual";
}
export function parsePerfRecord(line: string): PerfRunRecord; // throws PerfRecordError
export function foldSpikeResult(json: unknown): PerfRunRecord; // R05's results file, T1
export class PerfRecordError extends Error {
  /* line, field, reason */
}
```

### Budget rows and the itemised ledger (`src/renderer/src/view/perf/`)

```ts
export type BudgetRow =
  | "terrain"
  | "atmosphere-frame"
  | "atmosphere-tables"
  | "clouds"
  | "ocean"
  | "shadows"
  | "stars"
  | "exposure-histogram"
  | "bloom-tonemap"
  | "scatter"
  | "rings"
  | "station-wireframe"
  | "compositing"
  | "other";
export const PASS_ROWS: Readonly<Record<string, BudgetRow>>; // every timed pass label, note 4
export type MemoryItem =
  | "star-cubemap"
  | "height-cache"
  | "atmosphere-tables"
  | "coarse-field-gpu"
  | "render-targets"
  | "shadows"
  | "clouds"
  | "other";
export function itemise(tally: AllocationTally): Readonly<Record<MemoryItem, number>>; // over R05's
```

`MemoryItem`'s values are the entries this plan adds to R01's `MemoryCategory` union
(`view/engine/memory.ts`), so a creation site names its item through the category R01's
`createBuffer` and `createTexture` already require; sizes come from R01's `textureBytes`, which
this plan consumes rather than redefines.

### Perf mode (`src/main/perf/`)

- `--perf-run <scene>`, `--perf-out <path>`, `--perf-reps <n>`, `--perf-control` and `--force` on
  the client's command line (`src/main/cli.ts`), with `HYPERION_PERF_*` variables.
- `samplers.ts`: `sampleGpuMemory(pid)` over R05's `main/fdinfo.ts` with the shared bound (Design
  note 6), `sampleDisplayServer(pid)`, `sampleNvidia()` over `nvidia-smi -q -x`, `sampleThermal()`,
  `sampleThrottle()`.
- `machine.ts`: `describeMachine(summary: AdapterSummary) -> MachineRecord`; `preconditions.ts`:
  `checkQuiet() -> PreconditionReport`.

### Scenes and tools

- `src/renderer/src/view/perf/scenes.ts`: `PerfSceneId`, `PERF_SCENES`, each with its version,
  seeds, pinned universe, system and body, camera script (R05's `descentProfile.ts` or a new
  script), views and criteria.
- `perf/table.ts` (run by Node's type stripping): writes the generated sections of `budget.md` from
  `runs.v1.jsonl`. `perf/compare.ts`: states a record against the last comparable one.
- Recipes: `just perf-run scene=<id> reps=<n>`, `just perf-record <file>`, `just perf-table`,
  `just perf-compare <id>`.

### Recorded results (`docs/measurements/rendering/`)

- `runs.v1.jsonl`: one validated `PerfRunRecord` per line, append-only, the folded R05 spike files
  included.
- `budget.md`: hand-written prose around generated sections: the frame-time table and the memory
  table as the brainstorm lays them out, estimate beside measurement; the streaming and CPU table;
  each scene's result against its criterion; the ladder audit; the log of ladder changes.

## Consumes

Names are those the owning plans' Provides carry as written on 2026-09-29; where a name differs as
built, T0 changes only the call sites here.

- **R01** ([01](01-graphics-platform-and-engine.md)): `graphicsSwitches` with
  `GraphicsLaunchOptions.gpuTiming` and `GPU_TIMING_SWITCH` (`--hyperion-gpu-timing`, which merges
  `timestamp_quantization` into `disable-dawn-features` through `mergeSwitchValue`);
  `GraphicsStatus`, which states whether the timer is quantised; `AdapterSummary` and
  `GpuCapabilities` (`timestampQuery`, `subgroups`, `shaderF16`, `rg11b10Renderable`);
  `RenderEngine` and `RenderView`, with `onAllocation`, `MemoryCategory` and `textureBytes`
  (`view/engine/memory.ts`, R01.T8.d); the `GraphicsFault` a run ends on; `selectKernel`'s path,
  which each record states; the headless smoke harness `just test-render` over `WGSL_CATALOGUE` with
  `CapabilityOverrides`, which T7.a extends to every setting; R01.T11's three-canvas figures, folded
  in by T1.
- **R02:** the `view/` directory, the camera presets and a scripted camera pose; `ViewLabelBlock`,
  which states the setting; the exposure model, whose state each record carries.
- **R03:** the scene subscription and the binary frames, whose transfer time the arrival scene
  reports.
- **R04:** the `hyperion-surface` wasm build (`just gen-surface`) loaded by the height workers; the
  owner's ruling on the CSP question (R04.T10.a), which gates the first worker.
- **R05** ([05](05-terrain-geometry-and-descent-spike.md)): `QualitySetting` and its low setting in
  one place (Design note 26); the scripted descent `view/spike/descentProfile.ts` with `demand.ts`
  and its fixed-step mode; `spikeScene.ts`; the metrics harness `view/spike/metrics.ts`,
  `percentiles.ts` and `pipelineShim.ts` (frame intervals from presentation times with the
  `requestAnimationFrame` fallback, missed and hitching frames, per-pass GPU time from
  `timestampWrites`, main-thread time split, patches a second against the demand, upload bytes,
  pipeline stalls, GC pauses); the adapter's allocation tally (R05.T11.a); the main-process
  `main/spike.ts`, `main/fdinfo.ts` (summing distinct client IDs) and `main/reduceTrace.ts`; its
  results files `docs/measurements/descent-spike/<date>-<machine>-<setting>.json`; the pass
  criterion of Design note 21 and its pacing; open question 2's rule (Design note 22);
  `HeightWorkerPool` and `postField`; `PatchCache` and its budget; `terrainAnnunciation`.
- **R06:** the sky request and its census as arrival work on the server's bulk priority; the star
  cubemap bake and the sprite budget, with their settings.
- **R07:** `photorealisticPasses(setting)` and its `PassList`, whose labels join `PASS_ROWS`; the
  per-view budget policy and the internal-scale controller of its Design note 14; AgX, the exposure
  histogram and bloom; later, the main screen and its client role.
- **R08:** atmospheres at both settings, the per-planet tables and R05's `TABLE_SIZES` per
  `QualitySetting`, which R08 widens, among them.
- **R09:** the coarse pass on the server as arrival work, the coarse-field chunks and their request,
  the height function's bench (µs a point with its gradient).
- **R10:** terrain on generated worlds, the wireframe's depth-only terrain at 4 px with contours,
  horizon-map shadows, the height-texture cache sizes.
- **R11:** the ladder entries its T1 adds to the view's settings (`decoration`, `scatter`, `clouds`,
  `ocean`, `rings`) and the benchmarks its Design note 1 records.
- **Galaxy plan 14** ([14](../galaxy-generation/14-planetary-systems.md)): the bodies the scenes
  pin, by ID, with their ocean, cloud and ice fractions, atmospheres and rings, through
  `body_detail`.
- **Galaxy plan 04:** the server's `--num-workers` (`HYPERION_WORKERS`,
  `crates/hyperion-server/src/config.rs`, which defaults to the available parallelism less one) and
  its `CpuPool` priorities; `TestClient` for the pins test.

What exists today, checked against the tree on 2026-09-29: none of R01–R11's code. The client has no
`view/` directory, no WebGPU and no command-line switches (`apps/hyperion/src/main/index.ts` appends
none); its command line (`src/main/cli.ts`, on `commander`) has `--address` and `--port` only. The
repository records Criterion bench figures in doc comments and plan "as built" notes; there is no
results file and no `docs/measurements/` directory. The development machine is a ThinkPad X1 Yoga
4th generation: an i7-8665U (four cores, eight threads, 15 W, PL1 15 W with τ = 28 s from
`intel-rapl-mmio:0`, PL2 51 W) with its UHD 620 (`8086:3ea0`, i915, `gt_RP0_freq_mhz` 1,150 MHz,
`gt_RP1_freq_mhz` = `gt_RPn_freq_mhz` = 300 MHz), 16 GB, Linux 6.18, Mesa 26.2.3, Xorg with i3 and
no compositor, TLP 1.10.2 in `performance/AC` mode with the `powersave` governor, the
`balance_performance` energy preference and turbo on; `intel_gpu_top` is installed; the package
energy counter is root-only. Under the owner's other work the package sat at 96–97 °C against a 100
°C trip, and the thermal-throttle counters showed 17,010 s of package throttling in 35,900 s of
uptime.

## Design notes

1. **Where the results live.** R05 records its spike runs under `docs/measurements/descent-spike/`,
   so the consolidated record goes beside it, in `docs/measurements/rendering/`: measurements of the
   product outlive the plans, and `apps/` is the client's tree. The raw record is JSON lines, one
   scene a line, append-only and versioned in its file name as plan 12's Knowledge files are
   (`contacts.v1.jsonl`); the human table is generated from it, so that the two cannot disagree, and
   a Vitest in `pnpm test` fails when `budget.md`'s generated sections differ from what the writer
   would produce. R05's own results files stay where they are; T1's fold turns each into a record
   with `source: "descent-spike"`, and the other plans' benchmarks recorded in their "as built"
   notes become `plan-benchmark` records by hand, each citing its plan and task. If
   `docs/measurements/` is not yet in `.claude/CLAUDE.md`'s documentation layout when T1 lands, T1's
   commit adds it, flagged for the owner.

2. **What "replaced" means for the brainstorm.** The brainstorm says the implementing plan should
   replace its estimates with measured figures and keep them under version control. The record and
   its table do the keeping. The brainstorm's own tables are then revised to cite the measured table
   and to state which estimates held, as a drafted brainstorm revision that the owner signs off
   (T10), not an edit this plan makes on its own authority: the brainstorm is the specification, and
   several of its figures (the 30 fps at 720p, the 1 GB ceiling, _τ_) are leans that a measurement
   can argue with but not overrule.

3. **The settings are R05's `QualitySetting` and the entries later plans add.** The brief gives the
   first low setting to R05, which defines `QualitySetting` and gathers its low setting in one place
   (its Design note 26); R07 keys its passes by it, R08 has its `settings.ts`, and R11.T1 adds its
   ladder entries to "the view's settings". This plan reads them all into one `SettingSnapshot` per
   record and changes only their values (T11); it adds no feature and no setting. Where T0 finds the
   entries scattered with no common list, the snapshot is assembled from each owner's module and the
   finding goes to R05 as a named ask for a single list.

4. **Pass time per budget row, and the timer's quantum** (researched 2026-09-29). The frame-time
   table is per pass, so every pass label the harness times — R05's terrain and atmosphere passes,
   R07's `PassList`, and each later plan's — maps to one `BudgetRow` in `PASS_ROWS`, and a Vitest
   fails on a pass label without one. GPU time is R05's, from each pass's `timestampWrites`.
   Chromium quantises WebGPU timestamps to 65,536 ns by default: Dawn masks the low word with
   `kTimestampQuantizationMask = 0xFFFF0000` (`src/dawn/common/Constants.h`, applied in
   `CommandEncoder.cpp` and `QueryHelper.cpp`), which Chrome's _What's new in WebGPU_ 121 rounds to
   100 µs; on Gen9 the unquantised tick is 83.3 ns (Mesa's `timestamp_frequency` of 12 MHz,
   `src/intel/dev/intel_device_info.c`). A probe of Chromium 152 under R01's switch set on the UHD
   620 found `timestamp-query` exposed and every timestamp a multiple of 65,536 ns, and found that
   `--disable-dawn-features=timestamp_quantization` lifts the mask while changing no feature, limit,
   WGSL language feature or adapter; `--enable-webgpu-developer-features` also lifts it but unmasks
   the adapter info, and `--enable-unsafe-webgpu` adds four features and two WGSL features (Chromium
   `gpu/command_buffer/service/webgpu_decoder_impl.cc` and `dawn_instance.cc` at main; Electron
   44.4.3 probes of 2026-09-29, not in the repository). So perf mode passes R01's `gpuTiming` option
   (`--hyperion-gpu-timing`), as R01 Design note 4 and R05 Design note 18 already do, and nothing
   else; the machine's identity comes from `app.getGPUInfo("complete")`, not from the developer
   flag. The record states the quantum, and per-pass rows read `QUANTISED` wherever it is not 1 ns.
   One control run per scene omits the switch; since the mask sits in the resolve shader, the
   control only confirms that the frame-interval median and the kernel's busy time are unchanged.

5. **What a record says about the machine.** A number without its machine is not comparable, so a
   record carries the CPU model and core count; R01's `AdapterSummary` and `GpuCapabilities` and the
   kernel path chosen; the device ID, driver and versions from `app.getGPUInfo("complete")` (Mesa or
   NVIDIA), the kernel, Electron and Chromium; the display's size and refresh, whether it is
   `physical` or `virtual`, the window manager and compositor, and whether the path is X11 or
   XWayland; the switches in force; the server's placement and `--num-workers`; the height workers'
   count. On the discrete machine it also carries the card's published figures (Design note 11). Two
   records compare only when scene, scene version, machine identity, power profile and setting
   agree; the comparison tool refuses otherwise and names the field.

6. **Memory: the itemised tally and the kernel total** (researched 2026-09-29). WebGPU has no memory
   query, and Chromium's memory-infra dumps show no WebGPU allocation (a `contentTracing` run over
   `disabled-by-default-memory-infra` saw nothing above 8.9 MB while 256 MiB of textures were
   resident; `webgpu_decoder_impl.cc` registers no dump provider), so the budget's memory table is
   met two ways. The itemised figure is R05's allocation tally, grouped by `MemoryItem`, each
   creation site naming its item, with R01's `textureBytes` computing sizes from format, size,
   layers and mips. The total is the kernel's, read in the main process through R05's `fdinfo.ts`,
   summed per unique `drm-client-id` over the GPU process's `i915` clients, since the process holds
   several (ANGLE's and Skia's compositor device, and Dawn's) and duplicated descriptors share an
   ID, as the kernel's `Documentation/gpu/drm-usage-stats.rst` says
   (<https://docs.kernel.org/gpu/drm-usage-stats.html>). `drm-total` counts allocations whose pages
   need not exist; `drm-resident` counts pages that do (the probe: 256 MiB of textures showed 293.6
   MB total but 28.9 MB resident until cleared). So Σ`drm-resident-system0` is the headline against
   the ceiling, an upper bound because swapchain images shared between the GPU process's clients are
   counted in each; the lower bound Σresident − ½·Σ`drm-shared-system0` is stated beside it (about 6
   MB apart in the probe). A peak resident above the ceiling fails; a peak total above it is a
   finding, since an untouched allocation can become resident at any time. Dawn frees a destroyed
   resource on a later device tick, so the kernel peak ignores the 2 s after a large release in the
   tally, and tally and kernel peaks are stated apart. Under X11 with no compositor, Xorg's fdinfo
   is readable, and its resident memory and `drm-engine-render` delta are a "display server" line,
   outside the GPU process's busy time. GPU busy time is the GPU process's `drm-engine-render`
   nanoseconds differenced per frame; busy time less the timed passes is the `compositing` row, the
   consoles' cost the budget leaves out of its table. On NVIDIA, `--query-compute-apps` lists
   compute processes only, and a Vulkan GPU process may be a graphics process (`G`), so the
   per-process "GPU Memory Usage" comes from `nvidia-smi -q -x` (`<processes><process_info>`), with
   the device's `memory.used` less an idle baseline taken before launch as a cross-check (NVIDIA's
   `nvidia-smi` documentation). CPU memory is each process's from `app.getAppMetrics()`, which does
   not hold GEM pages (163 MB before and 166 MB after 256 MiB became resident), and each height
   worker reports its WebAssembly memory's byte length and its coarse-field copy, so the 15 MB field
   in three workers is a line of its own. On the UHD 620 GPU and CPU memory are one pool, so the two
   are reported apart and never summed into a "free" figure.

7. **A run, and what is compared** (researched 2026-09-29). The machine is measured as a player has
   it, at its default power profile, but in a known state, because unrecorded environment
   differences produce wrong data (Mytkowicz, Diwan, Hauswirth and Sweeney, ASPLOS 2009,
   doi:10.1145/1508244.1508275) and this laptop throttles thermally under sustained load.
   Preconditions, checked by the runner, which refuses otherwise unless `--force` marks the record:
   on AC (`/sys/class/power_supply/AC/online`); a one-minute load average below 1.0 before launch,
   with no builds, lanes or tests running; package temperature below 60 °C at the start, waiting up
   to five minutes for it; and the power profile recorded as found (TLP mode, governor, EPP,
   `no_turbo`, RAPL PL1, PL2 and τ). The warm-up is at least two minutes, 4.3 τ, so the PL1 average
   has converged to about 1.4%, and then continues until a sliding 30 s window changes by less than
   5% in median GPU frequency and 2 °C in package temperature from the one before, for at most ten
   minutes, after which the record says `NOT STEADY` (Kalibera and Jones, ISMM 2013,
   doi:10.1145/2464157.2464160, establish warm-up by inspecting the sequence rather than assuming a
   count). Five repetitions follow back to back, with no cool-down, since the sustained state is the
   one wanted; a monotone trend across them (all four successive differences of per-repetition mean
   frame time of one sign, p ≈ 0.083) is flagged. Throttling is recorded, not excluded: each
   repetition carries the delta of
   `/sys/devices/system/cpu/cpu*/thermal_throttle/{package,core}_throttle_total_time_ms` and the
   GPU's share of time at `gt_RP0_freq_mhz`, and a repetition is marked only when its throttle
   fraction differs from the scene's median by more than 20 percentage points. The UHD 620 runs are
   paced to every second vsync of the 60 Hz display, as R05's criterion is.

   The tables report R05's percentiles: the median over repetitions of each repetition's p50, p95
   and p99, the range, the scene's frame count, and p99 over the pooled frames beside the median of
   per-repetition p99s, since a 60 s repetition at 30 fps rests its p99 on 18 frames. They are not
   what is compared: on a vsync-paced display intervals are multiples of 16.7 ms, so p95 and p99 are
   step functions that a median-and-range comparison barely sees. The comparison uses continuous
   metrics per repetition — mean GPU busy time per frame, mean main-thread time per frame — and the
   missed-frame fraction (intervals above 1.5 T, R05's convention). A change is `REGRESSION` or
   `BETTER` only when all five new values lie beyond all five old ones (complete separation, the
   exact Mann–Whitney extreme, p = 2/252 ≈ 0.008 two-sided) and the ratio of medians exceeds ε, 3%
   for the means and 1 percentage point for the missed-frame fraction; otherwise `SAME`. The tool
   prints the median ratio and both ranges, as an effect size with its uncertainty. It is a finding,
   never a failure, as the galaxy README has it for benchmarks. T8 revises ε to twice the observed
   within-scene coefficient of variation and raises the repetitions to ten where that coefficient
   exceeds 3% (Georges, Buytaert and Eeckhout, OOPSLA 2007, doi:10.1145/1297027.1297033, size the
   count from measured variance).

8. **The scenes, and why these.** The brainstorm names three runs; the frame-time table needs every
   row lit at least once; the memory table needs its peak.
   - `cockpit-descent`: single-player on one machine. The photorealistic view full-window with two
     wireframe instrument panels, under R07's per-view policy, a local server on the same machine
     with `--num-workers 2`, and R05's scripted descent onto a pinned generated world with an
     atmosphere, an ocean, clouds and ice, entering from approach so that the arrival work — the
     sky's census and the coarse pass, for which R05 could only substitute a companion load — lands
     during it, as the budget warns. It lights terrain, atmosphere, clouds, ocean, shadows, stars,
     the histogram, bloom, scatter and the instruments' share, and answers "whether the cockpit and
     its instruments fit the 33 ms frame together". The camera follows the script on the seat
     preset's pose; no ship exists, and none is needed for a camera that is a display control.
   - `station-wireframe-descent`: the wireframe alone at 1080p, depth-only terrain at 4 px with
     contours, the same descent, three height workers and no server on the machine, since a station
     runs none. The server runs on a second machine on wired Ethernet; if there is none, it runs on
     the same machine pinned to one core with `--num-workers 1`, and the record's placement says
     `co-resident`, which the table shows but does not count as the station's figure.
   - `ringed-giant-approach`: a pinned ringed gas giant from 10¹⁰ m to inside its rings' annulus,
     across the 10⁹ m boundary and the slab-to-particle criterion, which lights the rings row and
     the gas giant's atmosphere.
   - `arrival-memory`: arrival at a pinned Earth-sized world whose surveyed coarse field is near 15
     MB, posted to three workers, then the descent's deepest point held for a minute: the resident
     memory's peak against the ceiling.
   - `nuclear-disc-sky`: a camera sweep in the nuclear disc, where the star bake is redone as the
     camera moves. Informative: the budget's star-field row is the solar neighbourhood's, which
     `cockpit-descent` measures, and this run records what the other end costs.

   Each scene pins universe seed, system and body IDs at the current generator version, and a server
   test asserts that each pin still has what its scene needs; a generator bump that moves a pin
   re-pins it in the same commit, as goldens are re-blessed, and bumps the scene's version, so that
   records from before and after never compare.

9. **Criteria, per scene.** Taken from R05's Design note 21 and the brainstorm, never set here:
   `cockpit-descent` at low meets R05's 720p30 column with the instruments open, and at high its
   1080p60 column on the discrete target; `station-wireframe-descent` meets R05's criterion rows at
   T = 16.7 ms at 1080p on the UHD 620, the brainstorm's lean beyond the owner's floor; the memory
   rows are R05's. The discrete ceiling is stated as 2 to 3 GB; a resident peak above 2 GB is a
   finding and one above 3 GB fails. Patches a second sustained are set against the demand at each
   moment of the script, and the fraction of the descent spent under `TERRAIN: STREAMING` is
   recorded beside them.

10. **The ladder's adjustment rule.** Adjusting is choosing values inside each ladder row's stated
    policy, never moving the policy. On the UHD 620, if a low-setting scene misses its criterion,
    values move in this order until it meets it: first the bounds of R07's internal-scale controller
    while instruments are open, which the brainstorm names as the lever; then scatter's token
    density to off, "the first thing after clouds to go", clouds being already a 2D layer; then,
    among the remaining rows, the one whose measured median most exceeds the upper end of its
    estimate, within its own row's knobs (bloom levels, table sizes, Gerstner components down to the
    stated eight, cubemap face size). Nothing the three rules protect is touched: the finest level
    under grounded bodies, authoritative rocks, the setting's statement and annunciations. On the
    discrete target, whose estimates already sum over budget at their upper ends, the measurements
    decide which pass gives, by the same excess rule over volumetric clouds, scatter density, the
    cascades and ring particles. A change that would move a figure the brainstorm states — _τ_,
    720p, 30 fps, a 4 px wireframe tolerance, glint retained at low, the memory ceilings — is not
    made; it is drafted as a brainstorm revision for the owner. Each change is its own commit with a
    run recorded before and after.

11. **The discrete machine** (researched 2026-09-29). A rented cloud GPU cannot give delivered frame
    intervals: cloud parts (L4, A10G, T4) have no scanout, so frames are paced by Chromium's
    timer-driven begin-frame source or a virtual display rather than a panel's vertical blank; a
    container needs the NVIDIA graphics capability for the Vulkan ICD; and none of them is an
    RTX 4060. The reference is a physical desktop with an RTX 4060-class card, a 1080p60 monitor and
    X11. A cloud run may check the NVIDIA code paths — the `nvidia-smi -q -x` parser, the per-pass
    timestamps — with `display: virtual`, and `perf/table.ts` excludes such records from the tables.
    The brainstorm's hardware figures, cited from memory, were checked: the UHD 620's 24 EUs (Mesa's
    `cfl_gt2`) at 1.15 GHz give 0.44 TFLOP/s of FP32, with 37.5 GB/s of memory bandwidth shared with
    the CPU (Intel ARK, i7-8665U); the RTX 4060's 3,072 CUDA cores at 2.46 GHz give 15.1 TFLOP/s,
    and 17 Gbps on 128 bits gives 272 GB/s (NVIDIA's product page; the memory speed from memory,
    which T9's `nvidia-smi -q` settles); the PlayStation 4's 1.84 TFLOP/s holds (Sony, 20 February
    2013). So the ratios hold: 34 times the arithmetic and 7.3 times the bandwidth.

12. **The audit's three rules.** The budget's rules are: the setting is stated on the display at all
    times, with `TERRAIN: DETAIL LIMITED` whenever it draws the surface below what the camera's
    position warrants; the patches under every grounded body in view are drawn at the finest level
    whatever the setting and style; and the low setting is built alongside the high one. The first
    two are properties with tests in their owning plans (R02, R05, R10), and the audit checks that
    those tests exist and run at every setting. The third is history and use: the audit reads, for
    each ladder row, the task and commit that built each setting and whether they landed together,
    and makes rot mechanical to catch by having R01's headless smoke harness render every
    `WGSL_CATALOGUE` entry at every setting, so that a low path that no longer compiles or completes
    a frame fails `just test-render`, which every task touching `view/engine/` or a catalogued
    shader runs as its gate (R01.T9.e), rather than waiting to be noticed. Findings go to the owning
    plan as named asks; this plan fixes none.

## Tasks

T0 comes first. T1 and T2 are independent; T3 needs T0 and T2. T4 needs T1–T3. T5 needs T4.a. T6
needs T1. T7 needs T0 and can run beside T1–T6. T8 needs T4–T6; T9 needs T8's tooling and a discrete
machine; T10 and T11 need T8 and T9; T12 closes.

### R12.T0 Reconcile with R01–R11 as built

Run the `revalidate-plan` skill over this plan. For every name under Consumes, find its signature in
the tree and correct the call sites; fold in each earlier plan's "as built" deviations. Build the
ladder inventory in this plan's Risks section: for every row of the brainstorm's ladder (clouds,
terrain detail, ocean, shadows, atmosphere, scatter, rings, resolution) and every setting the view
holds beyond them, the owning plan and task of each setting, the module that holds it, its test
names, and the benchmarks the owning plan recorded. List every pass label the harness can time.

Files: this plan. Acceptance: `npx prettier --check` on this plan; every Consumes name resolves by
`grep` in the tree or is listed as missing with its owner.

### R12.T1 The run record, the results store and the fold

`perf/record.ts` with `PerfRunRecord`, `HarnessRecord` and `parsePerfRecord` (every field of Design
notes 4, 5 and 7; an unknown `version` refused; units in field names: `_ms`, `_ns`, `_bytes`,
`_per_s`); `foldSpikeResult`, which turns one of R05's results files into a record with
`source: "descent-spike"`; `perf/table.ts`, which writes `budget.md`'s generated sections between
`<!-- generated:<name> -->` markers from the records, with the brainstorm's estimates as a constant
table beside them, each citing the brainstorm's row, and which excludes `display: virtual` records
and marks `forced` ones; `docs/measurements/rendering/runs.v1.jsonl` and `budget.md` (prose and
empty generated sections); the recipes `just perf-record <file>` (validate or fold, append,
regenerate) and `just perf-table`; the layout line of Design note 1 if it is missing.

Files: `apps/hyperion/perf/{record,fold,table}.ts` and tests, `docs/measurements/rendering/*`,
`justfile`, `.claude/CLAUDE.md` if needed.

Tests: a record round-trips; each missing or mistyped field is a `PerfRecordError` naming it; a
fixture of R05's results file folds into a valid record; the table writer is a pure function of the
records, orders rows as the brainstorm's tables do and leaves out a `virtual` record; `budget.md`
agrees with `runs.v1.jsonl` (the sync test). Acceptance: `pnpm test`, `just ci`, and
`just perf-table` leaves the tree unchanged.

### R12.T2 Machine, power and thermal state

`src/main/perf/machine.ts` (`describeMachine`: CPU from `os.cpus()`, `app.getGPUInfo("complete")`,
kernel from `os.release()`, the display from `screen`, the window manager, compositor and X11 or
XWayland from the environment, versions from `process.versions`, R01's `AdapterSummary` from the
renderer). `sampleThermal` and `sampleThrottle` in `src/main/perf/samplers.ts`:
`/sys/class/drm/card*/gt_act_freq_mhz` and `gt_RP0_freq_mhz`,
`/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq`, `/sys/class/thermal/thermal_zone*/temp`,
`/sys/devices/system/cpu/cpu*/thermal_throttle/{package,core}_throttle_total_time_ms`, sampled at 10
Hz and summarised per repetition (minimum and median GPU frequency, share of time at RP0,
throttle-time deltas and fraction, maximum temperature). `preconditions.ts`: `checkQuiet()` reads AC
(`/sys/class/power_supply/*/online`), the load average (`os.loadavg()`), the package temperature,
and the profile as found: `tlp-stat -s` where it runs unprivileged, the governor, EPP and
`intel_pstate/no_turbo`, and RAPL PL1, PL2 and τ from `/sys/class/powercap/intel-rapl-mmio:0`,
falling back to `intel-rapl:0`. The package energy counter is root-only, so package power is `null`
unless the owner has made it group-readable (an optional udev rule, documented, not required). A
missing file is `null`, never a guess.

Files: `apps/hyperion/src/main/perf/{machine,samplers,preconditions}.ts` and tests, with fixture
files under `src/main/perf/fixtures/` copied from this machine's `/sys`.

Tests: the parsers against the fixtures; a missing file gives `null`; the throttle fraction and the
RP0 share on synthetic traces; `checkQuiet` refuses on battery, on a load average of 1.0 or more, or
above 60 °C, and names which. Acceptance: `pnpm test`, `just ci`.

### R12.T3 Memory: itemised and kernel

- **R12.T3.a The itemisation.** `view/perf/ledger.ts` with `itemise` over R05's allocation tally and
  R01's `textureBytes`, whose sizes it re-checks (packed formats including `rgb9e5ufloat` and
  `rg11b10ufloat`, `rgba16float`, `depth32float`, cube layers, mip chains); every creation site of
  the tally names a `MemoryItem` as its R01 `MemoryCategory`. Tests, which also re-check the
  brainstorm's memory table as arithmetic from formats: a 1,024² `rgb9e5ufloat` cubemap with mips is
  6 × 4 B × 1,024² × 4⁄3 ≈ 33.6 MB ("34 MB with mips"), and 3,072² is ≈ 302 MB ("about 300 MB");
  four 2,048² `depth32float` cascades are 64 MiB ("64 MB"); a release returns the item to zero; the
  peak never falls; a creation without an item fails the Vitest over the tally's call sites.
  Acceptance: `pnpm test`.
- **R12.T3.b The kernel's figures.** `sampleGpuMemory(pid)` over R05's `fdinfo.ts`, adding
  `drm-shared-system0` and `drm-total-stolen-system0` to what it parses, summing per unique
  `drm-client-id`, and returning resident, total, shared, the lower bound of Design note 6 and the
  busy nanoseconds; the settle window after a large tally release; `sampleDisplayServer(pid)` over
  Xorg's fdinfo where readable, else `null`; `sampleNvidia()` over `nvidia-smi -q -x`'s
  `process_info` entries filtered to the GPU process, and the device's `memory.used` less the
  baseline taken before launch; `app.getAppMetrics()` per process; the workers' reports of
  `WebAssembly.Memory.buffer.byteLength` and coarse-field bytes over R05's pool messages. Tests: the
  fdinfo parser against the text captured on this machine (`drm-driver: i915`, the `system0` keys
  `total`, `shared`, `resident`, `active` and `purgeable`, and the `stolen-system0` keys); two
  descriptors with one client ID count once; the lower bound on a fixture with shared memory; an
  fdinfo of another driver is skipped; the XML parser against a `process_info` fixture of type `G`
  and one of `C+G`; an unreadable fdinfo gives `null`. Acceptance: `pnpm test`, and by hand on this
  machine a sample of the running client's GPU process shows a non-zero resident figure and Xorg's
  line.

Files: `apps/hyperion/src/renderer/src/view/perf/ledger.ts`, R05's tally call sites,
`apps/hyperion/src/main/perf/samplers.ts`, R05's `main/fdinfo.ts`, tests and fixtures.

### R12.T4 Perf mode

- **R12.T4.a The command line and the switch.** `--perf-run <scene>`, `--perf-out <path>`,
  `--perf-reps <n>`, `--perf-control` and `--force` in `src/main/cli.ts` (and their variables, in
  its table); perf mode calls R01's `graphicsSwitches` with `gpuTiming: true` unless
  `--perf-control` is given, and adds no other switch; the renderer receives the scene ID over the
  preload's configuration path. Tests: the CLI parses and refuses an unknown scene; the switch list
  in perf mode is R01's with `gpuTiming` and nothing more, never
  `--enable-webgpu-developer-features` or `--enable-unsafe-webgpu`; a control run's list lacks
  `timestamp_quantization`. Acceptance: `pnpm test`, `just ci`.
- **R12.T4.b The runner.** `view/perf/runPerfScene.ts` over R05's `metrics.ts` and the main
  process's `spike.ts` tracing and reducer: `checkQuiet` first (refusing unless `--force`, which the
  record carries); the scene; the warm-up of Design note 7 to a steady state or `NOT STEADY`; the
  repetitions, each with R05's metrics, the pass times by `PASS_ROWS`, the itemised and kernel
  memory, the thermal and throttle summary and the trend check; one `PerfRunRecord` over IPC to the
  main process, which appends it to `--perf-out` and quits. A `GraphicsFault` or a refused adapter
  ends the run with a record whose `notes` say so. The Vitest of Design note 4 over every pass label
  T0 listed. The recipe `just perf-run scene=<id> reps=<n>` builds the client, sets the commit and
  generator version in the environment and runs it. Tests: the runner against a fake harness, fake
  samplers and a fake clock produces a record that `parsePerfRecord` accepts; warm-up frames are
  absent from the percentiles; a trace that never settles ends at ten minutes as `NOT STEADY`; a
  monotone five is flagged. Acceptance: `pnpm test`, `just ci`, and by hand one repetition of R05's
  own descent on this machine gives a record that `just perf-record` accepts.

Files: `apps/hyperion/src/main/{cli,index}.ts`, `src/main/perf/`, `src/preload/`,
`src/renderer/src/view/perf/runPerfScene.ts`, `justfile`, tests.

### R12.T5 The scene catalogue

- **R12.T5.a Scenes.** `view/perf/scenes.ts` with the five scenes of Design note 8, each a version,
  seeds, the pinned IDs, its camera script (R05's `descentProfile.ts` for the descents, new pure
  scripts in the same shape for the giant's approach and the sky sweep), its views (style, size,
  rate, under R07's per-view policy) and its criterion from Design note 9. The cockpit's views are
  one photorealistic view at the window's size and the two instrument panels at the sizes R07's
  cockpit layout uses. Tests: every scene names a criterion; every view's style exists; the scripts
  are deterministic in R05's fixed-step mode (two evaluations at one time agree to the bit).
  Acceptance: `pnpm test`.
- **R12.T5.b The pins.** A search, recorded in the test's doc comment with its command, for a
  universe seed and system near the Sun holding an Earth-sized world with an atmosphere, an ocean
  fraction above 0.3, clouds and polar ice, and a ringed gas giant; and
  `crates/hyperion-server/tests/perf_scenes.rs`, which asks `body_detail` through `TestClient` for
  each pin and asserts what its scene needs (the fractions, the atmosphere's surface pressure, the
  ring), with a failure message that says to re-pin and bump the scene's version. A Vitest checks
  that `scenes.ts` and the test's constants agree by reading the test file's text. Acceptance:
  `cargo test -p hyperion-server --test perf_scenes`, `just ci`.

### R12.T6 The comparison tool

`perf/compare.ts` and `just perf-compare <id>`: find the previous record of the same scene, scene
version, machine identity, power profile and setting; refuse and name the differing field otherwise;
per continuous metric of Design note 7, print the old and new medians, both ranges, the median ratio
and the verdict (`SAME`, `BETTER`, `REGRESSION`); print the percentiles for reading, without a
verdict. It never exits non-zero on a regression. Tests: complete separation with the ratio beyond ε
gives `REGRESSION` (and the mirror `BETTER`); complete separation within ε gives `SAME`; overlapping
values give `SAME`; the missed-frame fraction uses its absolute ε; a different machine or power
profile is refused with the field named. Acceptance: `pnpm test`, `just ci`.

Files: `apps/hyperion/perf/compare.ts` and tests, `justfile`.

### R12.T7 The audit of the low settings

- **R12.T7.a Mechanical.** A Vitest over the settings T0 inventoried: every ladder row of the
  brainstorm has a high and a low value, each accepted by its feature. An extension of R01's smoke
  harness: a variant per setting that renders every `WGSL_CATALOGUE` entry at that setting on
  SwiftShader, with and without `CapabilityOverrides.withholdSubgroups`, asserting R01's frame
  properties, so that a rotted setting fails `just test-render`. A check, by test name, that R02's
  label-block test states the setting and that R05's and R10's grounded-body tests run at both
  settings and both styles. Acceptance: `pnpm test`, `just ci`, and `just test-render` passes at
  every setting.
- **R12.T7.b Written.** The ladder audit section of `budget.md`, hand-written: per row, the plan and
  task of each setting, whether they landed in one task (from `git log`), the tests that exercise
  each, the quality limits that R08 and R11 list for the audit, and a finding where one is missing,
  filed as a named ask in the owning plan's Risks. Acceptance: `npx prettier --check` on
  `budget.md`; every row of the brainstorm's ladder has an audit line.

Files: `apps/hyperion/src/renderer/src/view/perf/ladder.test.ts`, R01's `src/smoke/` and
`renderer/smoke.html`, `docs/measurements/rendering/budget.md`.

### R12.T8 The UHD 620 runs

Before the runs, stop every build, lane and test on the machine: the owner's other work is the
largest noise source seen here (load 21 at 97 °C). On AC at the default power profile, the display
at 1080p60: every scene at the low setting, five repetitions and one control run (Design note 4),
and `cockpit-descent` at the high setting for reference, where it runs. The local server for
`cockpit-descent` runs with `--num-workers 2` and two height workers. Fold R05's spike results files
and R01.T11's figures, and enter the other plans' recorded benchmarks as `plan-benchmark` records.
Report each scene's coefficient of variation and throttle fraction, and set ε and the repetition
count from them (Design note 7), re-running any scene whose coefficient exceeds 3% at ten
repetitions. Run `just perf-record` on each output and `just perf-table`. By hand, and recorded.

Files: `docs/measurements/rendering/{runs.v1.jsonl,budget.md}`, this plan's Design note 7 if ε
changes. Acceptance: the records validate; `just ci` passes (the sync test); `budget.md` shows a
measured UHD 620 value, or `NOT AVAILABLE` or `QUANTISED` with its reason, for every row of the
brainstorm's UHD 620 column and memory table, and each scene's verdict against its criterion; the
control runs' medians fall within the timed runs' ranges, or the finding is written in `budget.md`;
the figures are labelled as this chassis's throttled sustained state.

### R12.T9 The discrete runs

On a physical desktop with an RTX 4060-class GPU, a 1080p60 monitor, X11, the NVIDIA driver and the
same switch set, with the same preconditions: first R01's timestamp probe page once, to confirm the
quantum and the switch on the NVIDIA path (the Chromium source was read at main, not the 152
branch); then one `nvidia-smi -q -x` with the client running, to settle whether its GPU process is
`G` or `C+G`; then every scene at the high setting, five repetitions and a control, and
`cockpit-descent` at the low setting too, so that the low setting's cost on the target is known. The
record carries NVIDIA's published figures for the card (CUDA cores, boost clock, TGP, memory size
and bus width), the card's own `clocks.max.graphics` and memory clock from `nvidia-smi -q`, whose
product with the bus width gives its bandwidth, and `display: physical`. The station scene's server
runs on this machine for the UHD 620's station run, and T8's station run is repeated with the server
off the UHD 620. By hand, and recorded.

Files: as T8. Acceptance: as T8, for the discrete column; the station wireframe's figure on the UHD
620 with the server placement `remote`.

### R12.T10 Replace the budget's estimates

In `budget.md`, the measured tables and prose: which estimates held; which pass gave on the discrete
target (the lower ends summed to about 9 ms and the upper to about 18 ms); whether the UHD 620 low
setting landed nearer the lower end of its 16 to 28 ms, as the budget requires for the 5 ms it
leaves the instruments and compositing, with compositing measured (Design note 6); the height
workers' measured µs a point against the 10 µs budget and patches a second against the demand; the
memory peaks against the ceilings, with their bounds. Then draft the brainstorm revision: the
frame-time and memory tables' "estimate, not a measurement" sentences replaced by a link to
`budget.md` and the measured figures beside the estimates; every other figure the runs contradicted,
each with its record ID; the hardware figures of Design note 11 (272 GB/s in both places, 1.15 GHz,
the 37.5 GB/s shared bandwidth) with their sources in place of "cited from memory and not
re-checked". The revision goes to the owner; the owner signs off.

Files: `docs/measurements/rendering/budget.md`, a drafted revision of
`docs/agent/brainstorming/rendering-and-planets.md`. Acceptance: `npx prettier --check` on both;
each changed figure in the draft cites a record ID present in `runs.v1.jsonl` or a source; the owner
signs off.

### R12.T11 Adjust the ladder

- **R12.T11.a Low.** Apply Design note 10's rule to every UHD 620 scene that missed its criterion:
  each value change a commit in the owning plan's settings module, with the scene that missed re-run
  before and after and both records kept. Stop when every low scene meets its criterion or the rule
  runs out of values; in the latter case write the finding and the drafted brainstorm revision for
  the owner, and, if the scene is the descent, hand it to R05's open question 2 rule, under which a
  UHD 620 failure redesigns the low setting.
- **R12.T11.b High.** The same for the discrete target, deciding which pass gives.

Files: the owning plans' setting values, `runs.v1.jsonl`, `budget.md`'s ladder change log.
Acceptance: `just ci`; every change in the log has a before and an after record; `perf-compare` on
each after-record states the change with its verdict.

### R12.T12 Verification pass

Re-run every scene on both machines at the settings T11 left, record, regenerate the table, and
write the recommendations the sessions work needs (the single-player pool cap and the height-worker
count that the cockpit run supports). Record in this plan's Risks the figures that still stand as
estimates and why.

Acceptance: `just ci`; `budget.md` has a measured value, or a stated reason for its absence, for
every row of both budget tables and both columns; the sessions ask is written in this plan's Risks.

## Verification

- **Every estimate has a measurement** on both GPUs or a stated reason, in `budget.md`, generated
  from `runs.v1.jsonl` and checked in sync by `pnpm test` (T1, T8–T10).
- **The three named runs** exist as scenes anyone can re-run with `just perf-run`: the cockpit with
  its instruments together, the station wireframe during a descent alone, and the resident memory
  against the ceiling with each worker's coarse-field copy (T5, T8, T9).
- **Regressions are numbers:** `just perf-compare` states each change against the last comparable
  run, with its median ratio and ranges (T6).
- **The ladder is adjusted** by Design note 10's rule, with before and after records for every
  change (T11).
- **No low setting has rotted:** `just test-render` renders every catalogued shader at every
  setting, and the written audit covers every ladder row (T7).
- **By eye:** during `cockpit-descent` on the UHD 620, the label block states the setting
  throughout, and `TERRAIN: DETAIL LIMITED` and `TERRAIN: STREAMING` appear and clear as the
  record's streaming fraction says.

## Generator version

No change to generated output and no bump. The scenes pin universe seeds and body IDs at the current
generator version; a bump that moves a pin re-pins it, bumps the scene's version and leaves earlier
records valid but incomparable, which the comparison tool enforces. Every record carries the
generator version it ran at. The plan reserves nothing in the generator.

## Risks and open points

- **No discrete GPU.** The project has none, and T9 needs one borrowed; a rented cloud instance is
  not a stand-in (Design note 11). Until one is found the discrete column stays estimates, and the
  table says so.
- **The brainstorm's step 3 sentence** that the forced switches give per-pass GPU time uncoarsened
  is wrong, as R01 and R05 also found (Design note 4); T10's draft corrects it. Its Sources line
  crediting `--enable-unsafe-webgpu` with lifting the quantisation is right, but that flag is the
  broadest of three routes, and the narrow one is the Dawn toggle.
- **NVIDIA's process type** (Design note 6): whether Chromium's GPU process shows as `G` or `C+G` is
  unverified; T9's first `nvidia-smi -q -x` settles it, and the XML path reads either.
- **Thresholds provisional until T8.** The warm-up window's 5% and 2 °C, the 20-point throttle
  margin, ε of 3% and 1 point, and five repetitions are engineering choices (medium confidence),
  revised from T8's measured spread.
- **This chassis throttles thermally under sustained load.** The X1 Yoga's figures are its throttled
  sustained state and are labelled so; another UHD 620 laptop may do better or worse, which is why a
  record names the machine and not only the GPU.
- **Frame intervals are R05's.** Presentation times from the trace, with the `requestAnimationFrame`
  fallback; this plan inherits R05's definition and says so in `budget.md`.
- **The station's server placement.** With one machine the station run is co-resident and does not
  count (Design note 8); T9's machine removes that until the project has a second one of its own.
- **The main screen at 4K** waits on R07's main screen and so on the sessions, ship state and
  closed-loop commands, which have no plan. When it exists, a `main-screen-4k` scene joins the
  catalogue to measure the internal scale the brainstorm says it may render below native at.
- **The RTX 4060's memory speed** (17 Gbps, so 272 GB/s) is from memory; T9's `nvidia-smi -q`
  settles it.
- **Settings scattered** (Design note 3): if T0 finds no single list of the view's settings, the
  snapshot is assembled per module and R05 is asked for one; this plan does not build it.
- **`just test-render` is not in `just ci`** (R01.T9.e), so T7.a's anti-rot check runs where R01's
  gate runs it, on tasks that touch the engine or a shader; if the owner moves it into `ci`, the
  check moves with it.
- **The ladder inventory** is written by T0.
- **Sessions ask.** The single-player server's default pool cap and the height-worker count are
  handed to the sessions work by T12.
