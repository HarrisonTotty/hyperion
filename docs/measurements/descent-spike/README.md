# Descent spike results

The recorded runs of the descent spike, plan R05's gate
([`05-terrain-geometry-and-descent-spike.md`](../../agent/plans/rendering-and-planets/05-terrain-geometry-and-descent-spike.md),
Design notes 18 to 22 and 27). R12 folds them into `docs/measurements/rendering/`.

## Files

One run is two files, written by the client's main process at the end of the run
(`apps/hyperion/src/main/results.ts`):

- `<date>-<machine>-<setting>.json`: the results file, schema `hyperion.descent-spike.results`,
  version 4. A second run of the same day, machine and setting gets `-2`, `-3` and so on. A
  profiled run (`--trace-profile on`) is `<date>-<machine>-<setting>-profiled.json`, numbered the
  same way. Once Prettier formats it, a file stays under the repository's 500 KiB limit for added
  files for runs of up to about two hours; the writer warns of one that would not, and still writes
  it in full.
- `<date>-<machine>-<setting>.md`: its summary: the criterion's rows with their verdicts, frames
  by segment, streaming and memory.

`<machine>` is the host name, lower-cased, with anything but `a`–`z`, `0`–`9` and `-` dropped.

A native replay of a run's capture (R05.T15, `just replay <capture>`) writes
`<date>-<machine>-<setting>-replay.json` in the same schema. Its frames are timed by the GPU's
completions offscreen, or by presentation with `--present`. Each pass is timed by the replay's own
timestamps. Every figure a replay cannot have (the trace, the main thread, memory, rAF) is null,
with its reason. The capture (`--capture <dir>`: `capture.json` and `capture.bin`) is hundreds of
megabytes and is never committed.

## What a results file holds

Every figure is `{ "value": …, "reason": null }`, or `{ "value": null, "reason": "…" }` with the
reason it is missing. The writer refuses a file with a null figure that gives no reason.

| Part         | Holds                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `run`        | The machine (CPU, threads, memory, governor, load average at the start, Chromium's GPU and driver), the app's, Electron's, Chromium's, Node's and V8's versions, the platform, the launch mode, the setting, the seed, every option and every switch applied, the pass timer (`full`, `quantized` or `absent`), whether the window was shown, T, the warm-up, the canvas size, whether the run is provisional, and the trace's windows (each window's script-time span, file size and buffer use; each boundary's excluded interval, its frames and the stall's largest rAF interval; the traced time; whether the run is profiled; the trace's format) |
| `levels`     | T6's level table as the run used it: per level ε_n (m) and k_n                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `frames`     | Frame intervals after the 10 s warm-up, whole and per segment, from presentation times in the trace and from `requestAnimationFrame`: count, 50th, 95th and 99th percentiles (nearest rank), maximum, missed frames (above 1.5 T) and hitches (above 3 T); the frames Chromium dropped; the frames left out at the trace's window boundaries                                                                                                                                                                                                                                                                                                            |
| `gpu`        | The timer, its tolerance a pass (65.5 µs on a `quantized` timer, else 0), untimed passes, each pass's GPU time (50th, 95th, 99th percentiles), the 95th percentile of each frame's sum, and the GPU process's main-thread time and slices, which list only the names whose category was recorded (`GPUTask` always, `WebGPU` and `VulkanQueueSubmitHook` only in a profiled run)                                                                                                                                                                                                                                                                        |
| `mainThread` | Our code's time a frame at the 95th percentile, the renderer main thread's split into busy, our code (the per-frame `spike.frame` spans) and idle, the engine chunk's sampled self time (present in profiled runs only), and GC pauses per thread                                                                                                                                                                                                                                                                                                                                                                                                       |
| `streaming`  | Per segment: patches a second requested, baked and made resident; the predicted demand and the mean patches selected, under the hard bound and under min(hard, 4σ_n); baked against each demand; seconds `TERRAIN: STREAMING` showed                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `uploads`    | Bytes through the engine's `writeBuffer` and `writeTexture`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `pipelines`  | Pipelines created after the warm-up, with their labels                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `memory`     | The 1 Hz series as columns of whole KiB with the sample times in ms, a whole-run null with its reason for a reading never available, and -1 with listed gaps for a reading missing at some samples (the app's processes, the GPU process, Chromium's tracing service apart, the renderer's private bytes, the GPU process's DRM fdinfo, `nvidia-smi`), their peaks, and the headline GPU memory with its source: `nvidia-smi` less its baseline before launch on NVIDIA, else DRM fdinfo, else the adapter's own tally                                                                                                                                  |
| `criteria`   | Design note 21's rows for the whole descent (the frame rows, the two headroom rows, terrain, atmosphere and memory) and the frame rows per segment, each with its limit, value, tolerance and verdict (`pass`, `fail`, `marginal` or `not-measured`), and the overall verdict                                                                                                                                                                                                                                                                                                                                                                           |

How the figures are read:

- **T** is the display's vsync period from its refresh rate (`Display.displayFrequency`): 16.68 ms
  at 59.94 Hz on the high setting, twice the period on the low setting's 30 fps. A hidden run has
  no window, so T, the presentation times and every row read against T are null with the reason
  "no window shown"; such a run proves the harness and the file, not the criterion.
- **The timer** (decisions-r06-r07.md item 8): R01 lifts Dawn's 65.5 µs timestamp quantisation
  only in its Linux `vulkan` mode. Elsewhere the timer is `quantized`: a GPU-time row then carries
  ±65.5 µs a pass (±k × 65.5 µs for a sum of k passes) and is `marginal` within that of its limit.
  Frame intervals, which no timer affects, are the pass criterion.
- **The selection bound** (decisions-r05.md item 6): selection uses the hard bound; the file also
  records demand and patch counts under min(hard, 4σ_n), so that a failure on streaming demand alone
  that the calibrated bound would meet is called ours to fix, not a fired rule.
- **Memory:** GB is 10⁹ bytes. On NVIDIA the DRM fdinfo figures are null with their reason, since
  the driver writes no `drm-*` memory keys (hardware decision item 2). The tracing service is the
  measurement's own and is reported apart from the app.
- **A quiet machine** (Design note 27): a run started with the load average at or above 1 is marked
  provisional, and does not count towards the verdict.
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
renderer's frames, and exits 1 with the first failed window's reason (R05.T14.i). The runs that
count are visible, on a quiet machine:

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
  The baseline, over three seeds, and one high run for comparison, not judged:

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
