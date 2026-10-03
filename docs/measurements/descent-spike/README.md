# Descent spike results

The recorded runs of the descent spike, plan R05's gate
([`05-terrain-geometry-and-descent-spike.md`](../../agent/plans/rendering-and-planets/05-terrain-geometry-and-descent-spike.md),
Design notes 18 to 22 and 27). R12 folds them into `docs/measurements/rendering/`.

## Files

One run is two files, written by the client's main process at the end of the run
(`apps/hyperion/src/main/results.ts`):

- `<date>-<machine>-<setting>.json`: the results file, schema `hyperion.descent-spike.results`,
  version 1. A second run of the same day, machine and setting gets `-2`, `-3` and so on.
- `<date>-<machine>-<setting>.md`: its summary: the criterion's rows with their verdicts, frames
  by segment, streaming and memory.

`<machine>` is the host name, lower-cased, with anything but `a`–`z`, `0`–`9` and `-` dropped.

## What a results file holds

Every figure is `{ "value": …, "reason": null }`, or `{ "value": null, "reason": "…" }` with the
reason it is missing. The writer refuses a file with a null figure that gives no reason.

| Part         | Holds                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `run`        | The machine (CPU, threads, memory, governor, load average at the start, Chromium's GPU and driver), the app's, Electron's, Chromium's, Node's and V8's versions, the platform, the launch mode, the setting, the seed, every option and every switch applied, the pass timer (`full`, `quantized` or `absent`), whether the window was shown, T, the warm-up, the canvas size, whether the run is provisional, and the trace's span |
| `levels`     | T6's level table as the run used it: per level ε_n (m) and k_n                                                                                                                                                                                                                                                                                                                                                                      |
| `frames`     | Frame intervals after the 10 s warm-up, whole and per segment, from presentation times in the trace and from `requestAnimationFrame`: count, 50th, 95th and 99th percentiles (nearest rank), maximum, missed frames (above 1.5 T) and hitches (above 3 T); the frames Chromium dropped                                                                                                                                              |
| `gpu`        | The timer, its tolerance a pass (65.5 µs on a `quantized` timer, else 0), untimed passes, each pass's GPU time (50th, 95th, 99th percentiles), the 95th percentile of each frame's sum, and the GPU process's main-thread time                                                                                                                                                                                                      |
| `mainThread` | Our code's time a frame at the 95th percentile, the renderer main thread's split into busy, our code, the engine chunk's sampled self time and idle, and GC pauses per thread                                                                                                                                                                                                                                                       |
| `streaming`  | Per segment: patches a second requested, baked and made resident; the predicted demand and the mean patches selected, under the hard bound and under min(hard, 4σ_n); baked against each demand; seconds `TERRAIN: STREAMING` showed                                                                                                                                                                                                |
| `uploads`    | Bytes through the engine's `writeBuffer` and `writeTexture`                                                                                                                                                                                                                                                                                                                                                                         |
| `pipelines`  | Pipelines created after the warm-up, with their labels                                                                                                                                                                                                                                                                                                                                                                              |
| `memory`     | Samples at 1 Hz (the app's processes, the GPU process, Chromium's tracing service apart, the renderer's private bytes, the GPU process's DRM fdinfo, `nvidia-smi`), their peaks, and the headline GPU memory with its source: `nvidia-smi` less its baseline before launch on NVIDIA, else DRM fdinfo, else the adapter's own tally                                                                                                 |
| `criteria`   | Design note 21's rows for the whole descent (the frame rows, the two headroom rows, terrain, atmosphere and memory) and the frame rows per segment, each with its limit, value, tolerance and verdict (`pass`, `fail`, `marginal` or `not-measured`), and the overall verdict                                                                                                                                                       |

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
- **The trace** is marked truncated when its span is shorter than the scripted descent (the 2 GiB
  trace buffer filled).

## Making a run

`just descent-spike` builds the client, starts a local server with `--num-workers 2` and runs the
client with `--descent-spike` and the options given (R05.T13.c). `--out <dir>` sets where the files
go (this directory by default). The runs that count are visible, on a quiet machine:

- **The development machine's low-setting run** (R05.T14.c, by hand for the owner): with the
  projector in its 1080p 59.94 Hz mode, nothing else running and the load average under 1:

  ```sh
  just descent-spike --setting low
  ```

  The lanes prove the harness and the file with hidden runs only, whose presentation figures are
  null with "no window shown".

- **The UHD 620 runs** (R05.T16) and **the discrete runs** (R05.T17): their commands are in the
  plan's tasks.
