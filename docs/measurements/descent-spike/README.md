# Descent spike results

The recorded runs of the descent spike, plan R05's gate
([`05-terrain-geometry-and-descent-spike.md`](../../agent/plans/rendering-and-planets/05-terrain-geometry-and-descent-spike.md),
Design notes 18 to 22 and 27). R12 folds them into `docs/measurements/rendering/`.

## Files

One run is two files, written by the client's main process at the end of the run
(`apps/hyperion/src/main/results.ts`):

- `<date>-<machine>-<setting>.json`: the results file, schema `hyperion.descent-spike.results`,
  version 6. A second run of the same day, machine and setting gets `-2`, `-3` and so on. A
  profiled run (`--trace-profile on`) is `<date>-<machine>-<setting>-profiled.json`, numbered the
  same way. Once Prettier formats it, a file stays under the repository's 500 KiB limit for added
  files for runs of up to about two hours; the writer warns of one that would not, and still writes
  it in full.
- `<date>-<machine>-<setting>.md`: its summary: the criterion's rows with their verdicts, terrain
  and atmosphere against their estimates, the GPU's clocks, frames by segment, streaming and
  memory.

`<machine>` is the host name, lower-cased, with anything but `a`–`z`, `0`–`9` and `-` dropped.

A native replay of a run's capture (R05.T15, `just replay <capture>`) writes
`<date>-<machine>-<setting>-replay.json` in the same schema. Its frames are timed by the GPU's
completions offscreen, or by presentation with `--present`. Each pass is timed by the replay's own
timestamps. Every figure a replay cannot have (the trace, the main thread, memory, rAF) is null,
with its reason. It reads the GPU's clocks from the same sources as the client, before its first
frame, once a second and after its last, on the memory series' times. The capture (`--capture <dir>`: `capture.json` and `capture.bin`) is hundreds of
megabytes and is never committed.

## What a results file holds

Every figure is `{ "value": …, "reason": null }`, or `{ "value": null, "reason": "…" }` with the
reason it is missing. The writer refuses a file with a null figure that gives no reason.

| Part         | Holds                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `run`        | The machine (CPU, threads, memory, governor, load average at the start (three averages, or none with the reason: Windows keeps none), Chromium's GPU and driver), the app's, Electron's, Chromium's, Node's and V8's versions, the platform, the launch mode, the setting, the seed, every option and every switch applied, the pass timer (`full`, `quantized` or `absent`), whether the window was shown, T, the warm-up, the canvas size, whether the run is provisional, and the trace's windows (each window's script-time span, file size and buffer use; each boundary's excluded interval, its frames and the stall's largest rAF interval; the traced time; whether the run is profiled; the trace's format)                                                                                                           |
| `levels`     | T6's level table as the run used it: per level ε_n (m) and k_n                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `frames`     | Frame intervals after the 10 s warm-up, whole and per segment, from presentation times in the trace and from `requestAnimationFrame`: count, 50th, 95th and 99th percentiles (nearest rank), maximum, missed frames (above 1.5 T) and hitches (above 3 T); the frames Chromium dropped; the frames left out at the trace's window boundaries                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `gpu`        | The timer, its tolerance a pass (65.5 µs on a `quantized` timer, else 0), untimed passes, each pass's GPU time (50th, 95th, 99th percentiles), the 95th percentile of each frame's sum, over the frames whose pass times are complete, with the incomplete ones counted (dropped and partial) and bounding the rows' verdicts; terrain's and the atmosphere's own per-frame sums (50th, 95th, 99th percentiles) against their estimates, and whether each is over its estimate, findings that no verdict reads (`rows`); the GPU's clocks at 1 Hz and their maximum, from `nvidia-smi` or the kernel's sysfs, or null with the reason; and the GPU process's main-thread time and slices, which list only the names whose category was recorded (`GPUTask` always, `WebGPU` and `VulkanQueueSubmitHook` only in a profiled run) |
| `mainThread` | Our code's time a frame at the 95th percentile, the renderer main thread's split into busy, our code (the per-frame `spike.frame` spans) and idle, the engine chunk's sampled self time (present in profiled runs only), and GC pauses per thread                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `streaming`  | Per segment: patches a second requested, baked and made resident; the predicted demand and the mean patches selected, under the hard bound and under min(hard, 4σ_n); baked against each demand; seconds `TERRAIN: STREAMING` showed                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `uploads`    | Bytes through the engine's `writeBuffer` and `writeTexture`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `pipelines`  | Pipelines created after the warm-up, with their labels                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `memory`     | The 1 Hz series as columns of whole KiB with the sample times in ms, a whole-run null with its reason for a reading never available, and -1 with listed gaps for a reading missing at some samples (the app's processes, the GPU process, Chromium's tracing service apart, the renderer's private bytes, the GPU process's DRM fdinfo, `nvidia-smi`), their peaks, and the headline GPU memory with its source: `nvidia-smi` less its baseline before launch on NVIDIA, else DRM fdinfo, else the adapter's own tally                                                                                                                                                                                                                                                                                                          |
| `criteria`   | Design note 21's rows for the whole descent (the frame rows, the two headroom rows, the rest of the frame, which is terrain and atmosphere summed per frame, and memory) and the frame rows per segment, each with its limit, value, tolerance and verdict (`pass`, `fail`, `marginal` or `not-measured`), and the overall verdict                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

How the figures are read:

- **T** is the display's vsync period from its refresh rate (`Display.displayFrequency`): 16.68 ms
  at 59.94 Hz on the high setting, twice the period on the low setting's 30 fps. A hidden run has
  no window, so T, the presentation times and every row read against T are null with the reason
  "no window shown"; such a run proves the harness and the file, not the criterion.
- **The timer** (decisions-r06-r07.md item 8): R01 lifts Dawn's 65.5 µs timestamp quantisation
  only in its Linux `vulkan` mode. Elsewhere the timer is `quantized`: a GPU-time row then carries
  ±65.5 µs a pass (±k × 65.5 µs for a sum of k passes) and is `marginal` within that of its limit.
  Frame intervals, which no timer affects, are the pass criterion.
- **Incomplete pass times** (`decision-r05-trace-windows-2.md`, addendum B): a frame some of whose
  timer resolves never reported, dropped by the timer while every read-back buffer was in flight
  or lost to a failed read, is left out of the GPU rows' sums and counted in `gpu.incompleteFrames`:
  `dropped` (no pass time at all) and `partial`, among `frames`, the frames after the warm-up
  outside the trace's boundary exclusions. Each pass's own percentiles keep every time that
  arrived. A GPU row passes only if it passes with them all above its limit, fails only if it
  fails with them all below it, and is otherwise not measured, with their count as the reason.
  After the trace's last stop the run waits up to 1 s for the reads still in flight before it
  takes its report (R05.T14.k). A frame whose every missing read was still in flight then is
  counted the same way, with the reason "read in flight at the report", not the GPU's backlog.
- **The GPU's clocks** (`decision-r05-trace-windows-2.md`, addendum B, ruling 2; R05.T14.k): a GPU
  row measures each pass at the clocks the driver chose for the spike's load, never pinned or
  normalised. `gpu.clocks` records them at 1 Hz on the memory series' times, in its column form
  (whole MHz, and the performance state's number, P0 being 0), with the maximum graphics clock and
  the source. On NVIDIA, under Linux and Windows, they come from the `nvidia-smi -q -x` the memory
  sampler already runs. Under Linux, an Intel GPU's come from i915's sysfs, `gt_act_freq_mhz`
  against `gt_RP0_freq_mhz` (or `gt_boost_freq_mhz`), with no memory clock or state, and an AMD
  GPU's from amdgpu's `pp_dpm_sclk` and `pp_dpm_mclk`. Any user can read either, on the DRM card
  whose PCI vendor and device are the run's GPU's. On macOS (where `powermetrics` needs root), and
  on Windows for another vendor's GPU, they are null with the reason. A GPU row (`headroom-gpu`,
  `terrain-atmosphere`) whose median graphics clock after the warm-up was below 90% of the
  maximum carries the note "measured at a median N of M MHz (the driver's choice at this load)".
  The summary's **GPU clocks** line gives the graphics clock's median, 5th and 95th percentiles
  against the maximum. The files written before R05.T14.k have none ("not recorded before results
  version 5").
- **The rest of the frame** (`decision-r05-high-atmosphere.md`, R05.T14.l): Design note 21 judges
  terrain and atmosphere together, as the row `terrain-atmosphere`: the 95th percentile, over the
  complete frames, of each frame's terrain and atmosphere passes summed, against the two
  estimates' sum, 6 ms (5 + 1) on high and 18 ms (14 + 4) on low. It is a GPU row, so the
  incomplete frames bound it and the clock note applies. Each pass row's own percentiles are in
  `gpu.rows` beside its estimate, with `overEstimate`; the summary prints them, and a row over its
  estimate is "a finding for T19 and R12", never a verdict. The files written before R05.T14.l
  keep each row's 95th percentile (from version 5's `terrain` and `atmosphere` rows) and, for a row
  of one pass, its 50th and 99th. Their joint row is not measured: "not recorded before results
  version 6", since none kept each frame's sum, or, in the high run's file, which had no timed
  pass, "no timed terrain or atmosphere pass".
- **The selection bound** (decisions-r05.md item 6): selection uses the hard bound; the file also
  records demand and patch counts under min(hard, 4σ_n), so that a failure on streaming demand alone
  that the calibrated bound would meet is called ours to fix, not a fired rule.
- **Memory:** GB is 10⁹ bytes. On NVIDIA the DRM fdinfo figures are null with their reason, since
  the driver writes no `drm-*` memory keys (hardware decision item 2). The tracing service is the
  measurement's own and is reported apart from the app.
- **A quiet machine** (Design note 27): a run started with the load average at or above 1 is marked
  provisional, and does not count towards the verdict. Windows keeps no load average, so a run or a
  replay there is always provisional, its quiet rule unchecked; its `loadAverage` is none, with the
  reason "Windows keeps no load average" (results version 6; version 5 wrote zeros), and a demand
  record's Windows cells record an empty list.
  The governor is Linux's alone, and null with the reason elsewhere (R05.T20).
- **The trace** is a Perfetto protobuf stream over CDP, taken in windows of script time
  (R05.T14.e, T14.i) and decoded by the spike's own decoder (T14.h). A window that fails makes
  every trace figure null with its reason. A window fails when it is missing or empty, lost data,
  is short of its span because its buffer filled, or its frame spans disagree with the renderer's.
  The frames left out at boundaries are at most 5% of the descent and of any segment, and none
  in the approach or any later segment: the last boundary's exclusion (its gap plus the guard) must
  end before the approach begins. A profiled run (`--trace-profile on`) adds V8's CPU profiler and
  `gpu`: it is a diagnostic, its memory includes the profiler's samples, and it is not judged. The
  trace is recorded on the spike window's own debugger, which sends only `Tracing` and `IO`
  commands and is attached before the first window and detached after the last. Each window's
  buffer is 768 MiB, or 1.5 GiB in a profiled run. The recipe gives Electron a `TMPDIR` under
  `target/descent-spike/`, on disk, where Chromium spools each window's stream before the client
  reads it.

## Making a run

`just descent-spike` builds the client, starts a local server with `--num-workers 2` and runs the
client with `--descent-spike` and the options given (R05.T13.c). `--out <dir>` sets where the files
go (this directory by default). `just descent-spike --smoke` runs 10 s hidden and writes no file:
it checks its trace's three windows as a run's are, each decoded and its frame spans matched to the
renderer's frames, and exits 1 with the first failed window's reason (R05.T14.i). Its log lists
every clock sample (`descent spike: GPU clocks sampled …`), and a run's log the summary's clock
line. The runs that count are visible, on a quiet machine:

- **The development machine's low-setting run** (R05.T14.c, by hand for the owner): with the
  projector in its 1080p 59.94 Hz mode, nothing else running and the load average under 1:

  ```sh
  just descent-spike --setting low
  ```

  The lanes prove the harness and the file with hidden runs only, whose presentation figures are
  null with "no window shown". R05.T14.f's three hidden runs (`2026-10-06-effect-low`, `-high` and
  `-low-profiled`) proved the windowed protobuf trace on this machine: every window decoded and
  matched frame by frame, at most 26% of a window's buffer, and 2.16% (low) and 1.36% (high) of the
  descent left out.

- **The UHD 620 runs** (R05.T16, by hand for the owner, on the laptop, each on a quiet machine).
  The clocks come from i915's sysfs and need no privilege; on this part they also follow the
  package's shared power budget, so the CPU's load shows in them. The baseline, over three seeds,
  and one high run for comparison, not judged:

  ```sh
  just descent-spike --setting low --seed 7
  just descent-spike --setting low --seed 0
  just descent-spike --setting low --seed 1
  just descent-spike --setting high --seed 7
  ```

  The variants, each one factor changed from the first baseline, then a capture of a baseline span
  and its native replay:

  ```sh
  just descent-spike --setting low --seed 7 --companion-load 2
  just descent-spike --setting low --seed 7 --cold-cache
  just descent-spike --setting low --seed 7 --ridged on
  just descent-spike --setting low --seed 7 --dawn-safety off
  just descent-spike --setting low --seed 7 --workers 3
  just descent-spike --setting low --seed 7 --capture target/descent-spike/capture-t16
  just replay target/descent-spike/capture-t16
  ```

- **The discrete runs** (R05.T17, by hand for the owner, on the development machine with the
  projector in its 1080p 59.94 Hz mode). Every timed run pins `--workers 3`:

  ```sh
  just descent-spike --setting high --workers 3 --seed 7
  just descent-spike --setting high --workers 3 --seed 0
  just descent-spike --setting high --workers 3 --seed 1
  just descent-spike --setting high --workers 3 --seed 7 --dawn-safety off
  just descent-spike --setting high --workers 3 --seed 7 --vertex-path face-differences
  just descent-spike --setting high --workers 3 --seed 7 --normals mesh
  just descent-spike --setting high --workers 2 --seed 7
  just descent-spike --setting high --workers 3 --seed 7 --capture target/descent-spike/capture-t17
  just replay target/descent-spike/capture-t17 --present
  ```

  Only if a timed run misses a frame row or the main-thread headroom row, one profiled run on that
  run's seed, a diagnostic that is not judged:

  ```sh
  just descent-spike --setting high --workers 3 --seed <the seed> --trace-profile on
  ```

The seeds 7, 0 and 1 are the three whose scripted descent was checked clear of the terrain, with
ridges off and on (`decision-r05-descent-clearance.md`). A capture run carries the capture's own
costs, so it is not one of the timed runs.

**The first visible run of each kind is checked before the next** (R05.T14.f): T14.c's run, then
T16's first and T17's first. Its summary's **Trace** line must name no failed window and a buffer
use of at most 50%. The frames left out at the boundaries must be at most 5% of the descent and of
each segment, with none in the approach or a later segment. From the repository's root, on a POSIX
shell, this prints the three and `PASS` or `FAIL`:

```sh
node -e 'const r = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")); const t = r.run.trace.value; if (t === null) { console.log("FAIL no trace: " + r.run.trace.reason); process.exit(1); } const failed = t.windows.filter((w) => w.figures.value === null).map((w) => "window " + (w.index + 1) + ": " + w.figures.reason); const buffer = Math.max(...t.windows.map((w) => w.figures.value?.bufferPercent ?? 0)); const pct = (n, kept) => (100 * n) / (n + kept); const f = r.frames; const segments = f.segments.map((s) => [s.segment, pct(s.excludedFrames, s.raf.value?.count ?? 0)]); const busy = f.segments.slice(f.segments.findIndex((s) => s.segment === "approach and flare")).reduce((n, s) => n + s.excludedFrames, 0); console.log("failed windows: " + (failed.length === 0 ? "none" : failed.join("; "))); console.log("largest buffer use: " + buffer.toFixed(1) + " % (at most 50)"); console.log("left out: " + pct(f.excludedFrames, f.raf.value.count).toFixed(2) + " % of the descent (at most 5); worst segment " + Math.max(...segments.map(([, p]) => p)).toFixed(2) + " % (at most 5); " + busy + " frames from the approach on (none)"); const ok = failed.length === 0 && buffer <= 50 && pct(f.excludedFrames, f.raf.value.count) <= 5 && segments.every(([, p]) => p <= 5) && busy === 0; console.log(ok ? "PASS" : "FAIL"); process.exit(ok ? 0 : 1)' docs/measurements/descent-spike/<file>.json
```

A `FAIL` stops the runs of that kind: the windows are re-ruled before any more are made. A profiled
run's exclusions are recorded, not budgeted, so the check does not apply to it.
