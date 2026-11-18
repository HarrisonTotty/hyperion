# Descent spike: thewired, low, 2026-11-18

- **Overall:** not-measured (provisional: not a quiet machine)
- **Machine:** Intel(R) Core(TM) i7-8665U CPU @ 1.90GHz, 8 threads; GPU 32902:16032; governor powersave; load average 1.44, 1.76, 2.02
- **Versions:** app 0.1.0, Electron 45.1.0, Chromium 156.0.8078.49
- **Launch:** linux, vulkan mode, timer full, seed 7, world terrain-test, style wireframe, sizes on, window hidden, canvas 805 × 433 px
- **T:** — (no window shown); warm-up 10 s; pacing display, measured vsync 16.70 ms
- **Trace:** 9 perfetto-proto windows, 1199.0 s traced after the warm-up, unprofiled; 8 boundaries left out 809 frames (largest stall 183.30 ms); largest file 80 MiB, buffer use up to 9 %
- **Incomplete pass times:** none of 37548 frames after the warm-up
- **GPU clocks:** from i915-sysfs; graphics median 1150 MHz (100 %), p5 300, p95 1150, after the warm-up, of 1150 MHz
- **Options:** `--setting low` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile off` `--world terrain-test` `--style wireframe` `--sizes on` `--terrainVertexPath face-differences` `--terrainNormals mesh`

## The criterion (Design note 21)

| Row                                                                                        | Limit    | Value     | Verdict      | Note            |
| ------------------------------------------------------------------------------------------ | -------- | --------- | ------------ | --------------- |
| 50th percentile ≤ T + 0.5 ms                                                               | —        | 16.70 ms  | not-measured | no window shown |
| 95th percentile ≤ 35 ms                                                                    | —        | 116.60 ms | not-measured | no window shown |
| 99th percentile ≤ 2T                                                                       | —        | 200.00 ms | not-measured | no window shown |
| ≤ 1% of intervals above 1.5 T                                                              | 1 %      | —         | not-measured | no window shown |
| none above 3 T                                                                             | 0        | —         | not-measured | no window shown |
| main thread ≤ 0.8 T at the 95th percentile                                                 | —        | 17.70 ms  | not-measured | no window shown |
| GPU pass sum ≤ 0.8 T at the 95th percentile                                                | —        | 67.47 ms  | not-measured | no window shown |
| terrain and atmosphere GPU time, summed per frame, ≤ 18 ms (14 + 4) at the 95th percentile | 18.00 ms | 1.21 ms   | pass         |                 |
| GPU resident ≤ 1 GB                                                                        | 1.0 GB   | 0.227 GB  | pass         |                 |

## Terrain and atmosphere against their estimates

Each pass row's sum in each complete frame, beside the brainstorm's estimate: findings, which no verdict reads. The criterion judges the two together.

| Row        | Estimate | p50     | p95     | p99     | Finding                  |
| ---------- | -------- | ------- | ------- | ------- | ------------------------ |
| terrain    | 14.00 ms | 0.40 ms | 1.21 ms | 2.22 ms |                          |
| atmosphere | 4.00 ms  | —       | —       | —       | no timed atmosphere pass |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 37548 intervals, p50 16.70 ms, p95 116.60 ms, p99 200.00 ms, max 2833.20 ms. Frames left out at the trace's window boundaries: 809.

| Segment             | Intervals                                                                   | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 3000 intervals, p50 16.70 ms, p95 16.70 ms, p99 16.80 ms, max 16.80 ms      | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 31020 intervals, p50 16.70 ms, p95 116.60 ms, p99 183.20 ms, max 283.40 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 1901 intervals, p50 50.00 ms, p95 150.00 ms, p99 366.60 ms, max 1250.00 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 113 intervals, p50 216.70 ms, p95 849.90 ms, p99 2099.90 ms, max 2116.60 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 220 intervals, p50 216.70 ms, p95 983.20 ms, p99 1116.60 ms, max 2833.20 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 46 intervals, p50 216.70 ms, p95 683.20 ms, p99 766.70 ms, max 766.70 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 1247 intervals, p50 33.30 ms, p95 83.30 ms, p99 100.00 ms, max 583.40 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 0.0       | 0.0   | 0.0      | 3204.5          | 3204.5                   | 0             | 0                      | 0.0           |
| descent arc         | 0.5       | 0.7   | 0.7      | 3790.0          | 3790.0                   | 61            | 61                     | 0.0           |
| approach and flare  | 113.6     | 15.8  | 15.8     | 12821.3         | 12821.3                  | 265           | 265                    | 47.2          |
| low fast pass       | 151.6     | 36.1  | 36.1     | 9220.7          | 9220.7                   | 361           | 361                    | 30.0          |
| slowdown            | 82.7      | 39.7  | 39.7     | 4835.6          | 4835.6                   | 384           | 384                    | 59.9          |
| vertical descent    | 1.4       | 6.0   | 6.0      | 1073.1          | 1073.1                   | 258           | 258                    | 0.0           |
| hover and touchdown | 0.0       | 0.3   | 0.3      | 1.1             | 1.1                      | 114           | 114                    | 0.0           |

## Memory (peaks)

- GPU headline: 217 MiB from drm-fdinfo
- App processes: 2047 MiB; tracing service (the measurement's own): 130 MiB
- Renderer private: 1092 MiB
- DRM fdinfo resident: 217 MiB
- nvidia-smi less its baseline: — (no nvidia-smi baseline (not NVIDIA)); the GPU process alone: — (nvidia-smi does not list the GPU process)
- Adapter tally: 35 MiB

Uploads 3526 MiB; 2 pipelines created after warm-up; untimed passes — (no pass counted over a generated world).

## Sizes (R10.T14.b)

Counted once a second of script time, by unbudgeted re-selections of each pass's own inputs: F at the pass's margins, F₀ at margin 0, AR all round, and F′ and AR′ with the diagnostic occluder; each view's counts leave the forced region out, which is counted once. The views are found in each selection's input by their streaming weight, then their viewport.

- **Second view:** instrument-1 on spike-craft, wireframe, weight 0.25; viewport 274 × 140 px from 0.0 s, 80 × 146 px from 0.1 s.
- **The header's lowest sampled height:** -9,538.1 m.
- **The lit and wireframe passes' forced sets** (T14.c's finding 4): — (the lit and wireframe passes never stood in the same frame).

### The wireframe cache: spike (wireframe, weight 1), instrument-1 (wireframe, weight 0.25)

1,128 samples; attached to 1 pass; 520 slots of 38,028 B.

| Count                     | spike                                              | instrument-1                                       | union                                                  |
| ------------------------- | -------------------------------------------------- | -------------------------------------------------- | ------------------------------------------------------ |
| F                         | 967 (879.0 s, descent arc, 29,335 m up)            | 943 (1,051.1 s, approach and flare, 15,651 m up)   | 967 (879.0 s, descent arc, 29,335 m up)                |
| F₀ (margin 0)             | 962 (879.0 s, descent arc, 29,335 m up)            | 885 (1,050.0 s, approach and flare, 16,239 m up)   | 962 (879.0 s, descent arc, 29,335 m up)                |
| AR (all round)            | 1,728 (1,048.1 s, approach and flare, 17,470 m up) | 1,695 (1,050.0 s, approach and flare, 16,239 m up) | 1,728 (1,048.1 s, approach and flare, 17,470 m up)     |
| F′ (diagnostic occluder)  | 967 (879.0 s, descent arc, 29,335 m up)            | 943 (1,051.1 s, approach and flare, 15,651 m up)   | 967 (879.0 s, descent arc, 29,335 m up)                |
| AR′ (diagnostic occluder) | 1,728 (1,048.1 s, approach and flare, 17,470 m up) | 1,695 (1,050.0 s, approach and flare, 16,239 m up) | 1,728 (1,048.1 s, approach and flare, 17,470 m up)     |
| Forced, once              |                                                    |                                                    | 108, 0 seen (997.0 s, approach and flare, 56,693 m up) |

| Form                                 | Slots   | Bytes         | Slots, diagnostic | Bytes, diagnostic | Against the as-built 520 slots |
| ------------------------------------ | ------- | ------------- | ----------------- | ----------------- | ------------------------------ |
| Ruled: Σ max(1.3 AR, 3 F) + forced   | 5,838.0 | 222,007,464 B | 5,838.0           | 222,007,464 B     | × 11.23                        |
| (a) its largest same-sample value    | 5,366.0 | 204,058,248 B | 5,366.0           | 204,058,248 B     | × 10.32                        |
| (b) max(1.3 AR∪, 3 F∪) + forced      | 3,009.0 | 114,426,252 B | 3,009.0           | 114,426,252 B     | × 5.79                         |
| (c) the frustum rule, Σ 3 F + forced | 5,838.0 | 222,007,464 B | 5,838.0           | 222,007,464 B     | × 11.23                        |

- **A/B:** the diagnostic occluder at its deepest -9,538.1 m, against the as-built -66,604.0 m; the price in slots, each a share of the sized all-round peak Σ 1.3 AR (4,449.9 slots): the rule's own, rule − rule′, 0.0 slots, 0.0 %, the reading the 10 % test takes; the union's all-round term 1.3 (AR − AR′) 0.0 (0.0 %), its frustum term 3 (F − F′) 0.0 (0.0 %).
- **The all-round peak's residency:** the resident share's peak 100.0 % (417.2 s, descent arc, 194,905 m up), the swept share's 100.0 % (333.4 s, descent arc, 224,952 m up), the cap at AR∪'s peak × 0.30; resident at 100 % at some sample. The descent never turns, its views look along the track and VIEW's LOOK turns are not flown: a "never" is the descent's.
- **The forced region:** first selected at 990.8 s; every forced patch resident first at 1,169.1 s, and at every sample after; touchdown at 1,199.9 s (99 forced, 99 of them resident); a lead of 30.8 s (touchdown − every forced patch resident); the contact by the session's rule at 990.8 s (104 forced, 0 of them resident).
- **Box heights by level** (the frustum count's seen leaves): L2 max 243,412.6 m, median at F∪'s peak —; L4 max 168,117.9 m, median at F∪'s peak —; L5 max 135,159.8 m, median at F∪'s peak —; L6 max 85,242.2 m, median at F∪'s peak —; L7 max 48,279.6 m, median at F∪'s peak 44,016.2 m; L8 max 29,606.8 m, median at F∪'s peak 25,970.9 m; L9 max 19,535.8 m, median at F∪'s peak 14,764.5 m; L10 max 15,376.5 m, median at F∪'s peak —; L11 max 13,178.5 m, median at F∪'s peak —; L12 max 7,546.4 m, median at F∪'s peak —; L13 max 7,167.6 m, median at F∪'s peak —; L14 max 7,018.6 m, median at F∪'s peak —; L15 max 6,706.8 m, median at F∪'s peak —; L16 max 6,674.1 m, median at F∪'s peak —; L17 max 6,654.7 m, median at F∪'s peak —.
- **k_n = ε_n P₀ ÷ e_n**, 1 level table met; the last (from 0.1 s): L0 49.42, L1 80.96, L2 138.01, L3 200.74, L4 282.75, L5 359.13, L6 382.05, L7 451.28, L8 539.52, L9 530.64, L10 464.68, L11 365.54, L12 304.93, L13 280.19, L14 279.03, L15 276.57, L16 272.83, L17 0.00.
- **The counting's cost:** p50 8.40 ms, p95 31.20 ms, max 72.80 ms a sample.

| Segment             | Frames | Drawn, mean / max | Standing in | Missing | Stand-in share  | Limited frames | Made resident / s |
| ------------------- | ------ | ----------------- | ----------- | ------- | --------------- | -------------- | ----------------- |
| orbit coast         | 3,580  | 0.0 / 0           | 0.0 / 0     | 0.0 / 1 | 0.0 % / 0.0 %   | 0              | 0.0               |
| descent arc         | 31,829 | 61.3 / 266        | 0.0 / 14    | 0.0 / 0 | 0.4 % / 100.0 % | 7,007          | 0.7               |
| approach and flare  | 1,901  | 240.9 / 407       | 26.3 / 304  | 0.0 / 0 | 7.8 % / 84.1 %  | 1,901          | 15.8              |
| low fast pass       | 113    | 381.3 / 442       | 309.7 / 360 | 0.0 / 0 | 85.7 % / 90.7 % | 113            | 39.3              |
| slowdown            | 220    | 435.4 / 519       | 316.5 / 404 | 0.0 / 0 | 81.0 % / 92.7 % | 220            | 40.1              |
| vertical descent    | 46     | 397.7 / 468       | 7.8 / 59    | 0.0 / 0 | 4.8 % / 32.6 %  | 38             | 6.2               |
| hover and touchdown | 1,247  | 78.5 / 117        | 0.1 / 21    | 0.0 / 0 | 0.1 % / 15.9 %  | 0              | 0.3               |
| after the script    | 1,006  | 78.0 / 78         | 0.0 / 0     | 0.0 / 0 | 0.0 % / 0.0 %   | 0              | —                 |

The pass's own counts at the samples, the largest in each segment:

| Segment             | Samples | Selected | Limited samples | Selected resident | Slots used | Pinned forced | Pinned drawn |
| ------------------- | ------- | -------- | --------------- | ----------------- | ---------- | ------------- | ------------ |
| orbit coast         | 60      | 0        | 0               | 0                 | 1          | 0             | 1            |
| descent arc         | 850     | 260      | 413             | 260               | 520        | 0             | 392          |
| approach and flare  | 73      | 369      | 73              | 247               | 520        | 108           | 366          |
| low fast pass       | 28      | 398      | 28              | 69                | 520        | 104           | 122          |
| slowdown            | 56      | 443      | 56              | 260               | 520        | 102           | 348          |
| vertical descent    | 10      | 301      | 9               | 292               | 520        | 99            | 378          |
| hover and touchdown | 50      | 132      | 0               | 114               | 520        | 99            | 96           |
| after the script    | 1       | 114      | 0               | 114               | 520        | 99            | 69           |

### The height workers by segment

3 workers wrapped; the flight's 6,440 bakes of 6,440 posted, 384 never made resident, 5.4 a second.

| Segment             | Bakes / s | Baked | Never resident | Post to reply, p50 / p95 (ms) | Ground speed ÷ height, mean / max (s⁻¹) |
| ------------------- | --------- | ----- | -------------- | ----------------------------- | --------------------------------------- |
| orbit coast         | 22.4      | 1     | 0              | 37.0 / 37.0                   | 0.024 / 0.024                           |
| descent arc         | 0.8       | 637   | 10             | 37.2 / 58.7                   | 0.032 / 1.000                           |
| approach and flare  | 16.5      | 1,992 | 102            | 82.8 / 231.7                  | 1.142 / 4.708                           |
| low fast pass       | 42.7      | 1,209 | 127            | 80.9 / 160.6                  | 1.000 / 1.000                           |
| slowdown            | 42.5      | 2,522 | 142            | 82.5 / 117.5                  | 0.481 / 0.799                           |
| vertical descent    | 6.0       | 61    | 1              | 150.2 / 286.6                 | 0.000 / 0.000                           |
| hover and touchdown | 13.2      | 18    | 2              | 303.7 / 402.2                 | 0.000 / 0.000                           |

### Memory: itemized, the engine's tally and the driver

- The wireframe cache: 520 slots × 38,028 B = 18.9 MiB
- The session's coarse-field-gpu: 0.2 MiB at its peak (1.0 s)
- The cascades: none allocated (the low setting has no shadow cascades: the engine allocated nothing under shadows); T9.a's spec 64.0 MiB
- Itemized GPU (the caches, the session's masks and maps, the cascades): 19.1 MiB; with the workers' field copies (3.3 MiB at their peak): 22.4 MiB, against 1 GB (low); the pool's own copy beside them 0.0 MiB; 3 workers started; 1,191 samples at 1 Hz
- The engine's tally, the app's own GPU figure: 35.4 MiB at its whole peak (1,050.0 s); each category at its own peak: other 1.6 MiB (1,050.0 s), coarse-field-gpu 0.2 MiB (0.4 s), geometry-clusters 13.1 MiB (1,050.0 s), atmosphere-tables 0.0 MiB (before the flight), height-cache 19.1 MiB (0.0 s), render-targets 1.5 MiB (0.0 s)
- The driver: nvidia-smi's peak less its pre-launch baseline — (no nvidia-smi baseline (not NVIDIA)); the GPU process's peak — (nvidia-smi does not list the GPU process)
- At the run's end: the device — (no nvidia-smi reading at the run's last memory sample), the GPU process — (nvidia-smi does not list the GPU process at the run's last memory sample), the rest of the card — (no nvidia-smi reading at the run's last memory sample) (the pre-launch baseline — (no nvidia-smi baseline (not NVIDIA)), a drift of — (no nvidia-smi reading at the run's last memory sample))
- Less the driver: the itemized GPU — (no nvidia-smi baseline (not NVIDIA)), the engine's tally — (no nvidia-smi baseline (not NVIDIA))

### The binding

- maxStorageBufferBindingSize: the adapter's 4,294,967,292 B, requested 1,073,741,824 B, the device's 1,073,741,824 B
- The wireframe cache's 520 slots of 520 requested, on its own path, no fallback: its ceiling 31,767 slots at the requested binding, 3,970 slots at WebGPU's default 128 MiB

### The readout and the load

- VIEW's terrain readout every 250 ms, not shown: 4,394 publishes, 4,393 holding an answer; 4,302 asked of the workers, 4,302 answered, 0 failed; 0 of its guards' calls to fail the session counted and withheld
- The 1-minute load: 1.44 at 0 s, 1.29 at 60 s, 1.18 at 120 s, 1.62 at 180 s, 1.57 at 240 s, 1.05 at 300 s, 1.05 at 360 s, 0.99 at 420 s, 0.93 at 480 s, 0.51 at 540 s, 0.44 at 600 s, 0.30 at 660 s, 0.32 at 720 s, 0.46 at 780 s, 0.31 at 840 s, 0.91 at 900 s, 1.40 at 960 s, 1.54 at 1,020 s, 1.92 at 1,080 s, 2.47 at 1,140 s, 1.87 at 1,200 s, 1.20 at 1,230 s
