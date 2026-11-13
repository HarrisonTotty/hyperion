# The rendering budget, measured

Plan R12's consolidated record of the renderer's performance against the brainstorm's
[Performance budget](../../agent/brainstorming/rendering-and-planets.md#performance-budget). The
record is [`runs.v1.jsonl`](runs.v1.jsonl): one validated run record (`@hyperion/perf`'s
`PerfRunRecord`) a line, append-only. Its records are perf mode's runs (`just perf-run`), R05's
descent-spike results files folded into it, and other plans' benchmarks entered by hand, each added
by `just perf-record <file>`. The sections between `<!-- generated:… -->` markers are written from
it by `just perf-table`, and `pnpm test` fails when they differ from what it would write; the rest
of this file is prose.

## How to read it

- **The machines** are the README's Target hardware. The RTX 3080 desktop is the recommended
  specification: the discrete column is judged on it, beside the brainstorm's RTX 4060-class
  estimates, and a pass on it is a pass on a faster part. The UHD 620 laptop is the minimum
  specification: its figures are that chassis's throttled sustained state. The owner's M5 Mac is
  a third machine, its columns shown once it has records (a lean pending the owner).
- **Frame intervals** are R05's: presentation times from the trace, with the
  `requestAnimationFrame` fallback (R05 Design note 21). A missed frame is an interval above 1.5 T.
- **GPU time** is R01's per-pass timestamps. Chromium quantises them to 65,536 ns unless perf mode
  lifts the mask: a cell reads `QUANTISED` where its record's timer was quantised, `NOT AVAILABLE`
  where it had none, and `≤` where a pass was bracketed, an upper bound.
- **Left out of the tables:** a record whose display was virtual or whose window was hidden, its
  frames paced by Chromium's timer rather than a panel's vertical blank. `(forced)` marks a record
  whose preconditions were overridden.
- **A change** is stated by `just perf-compare <id>` against the last comparable record, with its
  median ratio and both ranges (Design note 7): a finding, never a failure.

## Frame time

<!-- generated:frame-time -->
<!-- prettier-ignore-start -->
GPU time a frame by row: measured p50 / p95 / p99 in ms, the median over the record's
repetitions, with the record's ID; `—` where no record has the row yet.

| Pass (the brainstorm's row)           | Discrete estimate                         | RTX 3080, high, measured p50 / p95 / p99, ms | UHD 620 estimate                               | UHD 620, low, measured p50 / p95 / p99, ms |
| ------------------------------------- | ----------------------------------------- | -------------------------------------------- | ---------------------------------------------- | ------------------------------------------ |
| Terrain geometry and patches          | 3–5 ms                                    | —                                            | 8–14 ms                                        | —                                          |
| Atmosphere, per frame                 | 0.5–1 ms                                  | —                                            | 2–4 ms                                         | —                                          |
| Atmosphere, per-planet tables         | under 0.1 ms, when the atmosphere changes | —                                            | about 1 ms, on the same occasions              | —                                          |
| Volumetric clouds                     | 1.5–3 ms at quarter resolution            | —                                            | cut: a two-dimensional layer at about 1 ms     | —                                          |
| Ocean                                 | 1–2 ms, Gerstner                          | —                                            | 2–3 ms                                         | —                                          |
| Shadows                               | 1.5–3 ms                                  | —                                            | 0.3–0.6 ms                                     | —                                          |
| Star field and galactic band          | under 0.2 ms near the Sun                 | —                                            | under 0.5 ms                                   | —                                          |
| Exposure histogram                    | 0.3–0.5 ms                                | —                                            | about 1 ms                                     | —                                          |
| Bloom and tone mapping                | under 1 ms                                | —                                            | 2–3 ms                                         | —                                          |
| Scatter instances                     | 1–2 ms                                    | —                                            | off, or a token density under 1 ms             | —                                          |
| Rings, when in view                   | 0.5–1 ms                                  | —                                            | under 0.5 ms                                   | —                                          |
| Station wireframe view                | under 3 ms at 1080p                       | —                                            | 4–9 ms at 1080p, against its own 16.7 ms frame | —                                          |
| Cockpit instruments (measured only)   | —                                         | —                                            | —                                              | —                                          |
| Consoles' compositing (measured only) | —                                         | —                                            | —                                              | —                                          |
| Other passes (measured only)          | —                                         | —                                            | —                                              | —                                          |
<!-- prettier-ignore-end -->
<!-- generated:frame-time -->

## Memory

<!-- generated:memory -->
<!-- prettier-ignore-start -->
Peak bytes by item from the allocation tally, then the kernel's resident total (Design note 6):
the median over the record's repetitions, in MB of 10⁶ bytes.

| Item (the brainstorm's row)             | Discrete estimate                                   | RTX 3080, high, measured peak | UHD 620 estimate                            | UHD 620, low, measured peak |
| --------------------------------------- | --------------------------------------------------- | ----------------------------- | ------------------------------------------- | --------------------------- |
| Star cubemap                            | about 300 MB with mips                              | —                             | 34 MB with mips                             | —                           |
| Height-texture cache                    | about 400 MB                                        | —                             | about 64 MB                                 | —                           |
| Atmosphere tables                       | about 0.4 MB a planet on an Earth, under 2 MB       | —                             | smaller tables, within the discrete figures | —                           |
| Atmosphere tables, per view             | 0.43 MB a view, summed over suns                    | —                             | within the discrete figures                 | —                           |
| Coarse field, GPU copy                  | at most 384 KiB at level 8                          | —                             | the same                                    | —                           |
| Render targets                          | about 70 MB at 1080p                                | —                             | about 30 MB at 720p                         | —                           |
| Shadows                                 | four 2,048² cascades, 64 MB                         | —                             | none beyond the patch cache                 | —                           |
| Clouds                                  | 3D noise volumes, 10–20 MB                          | —                             | a two-dimensional layer, under 16 MB        | —                           |
| Ocean (measured only)                   | —                                                   | —                             | —                                           | —                           |
| Rings (measured only)                   | —                                                   | —                             | —                                           | —                           |
| Scatter (measured only)                 | —                                                   | —                             | —                                           | —                           |
| Stills (measured only)                  | —                                                   | —                             | —                                           | —                           |
| Other (measured only)                   | —                                                   | —                             | —                                           | —                           |
| Total, GPU                              | about 0.75–1.25 GB                                  | —                             | about 150–300 MB                            | —                           |
| Coarse field, the workers' copies (CPU) | 4–15 MB a worker; about 45 MB for an Earth in three | —                             | the same                                    | —                           |
<!-- prettier-ignore-end -->
<!-- generated:memory -->

## Streaming and the CPU

<!-- generated:streaming -->
<!-- prettier-ignore-start -->
Patches a second sustained against the demand, by segment, and the time spent under
`TERRAIN: STREAMING` (Design note 9); then the CPU's figures: the renderer's main thread a
frame, the height workers' µs a point with its gradient (the budget's 10 µs) and the
pipelines created late.

No record has streaming figures yet.

No record has CPU figures yet.
<!-- prettier-ignore-end -->
<!-- generated:streaming -->

## Scenes against their criteria

Each scene's result against its criterion (Design note 9), written by R12.T8 and T9 as their runs
land. None yet.

## The ladder audit

Per ladder row, the plan and task of each setting and the tests that exercise it, written by
R12.T7.b. Not yet written.

## Ladder changes

R12.T11's log: each value change with its before and after records. None yet.

## Runs

<!-- generated:runs -->
<!-- prettier-ignore-start -->
No record yet: `just perf-record <file>` adds one.
<!-- prettier-ignore-end -->
<!-- generated:runs -->
