# Descent spike: effect, high, 2026-10-21

- **Overall:** not-measured (provisional: not a quiet machine)
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 6.03, 4.43, 6.29
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, world terrain-test, style wireframe, sizes on, window hidden, canvas 1398 × 793 px
- **T:** — (no window shown); warm-up 10 s
- **Trace:** 9 perfetto-proto windows, 1187.3 s traced after the warm-up, unprofiled; 8 boundaries left out 1896 frames (largest stall 66.70 ms); largest file 197 MiB, buffer use up to 22 %
- **Incomplete pass times:** none of 68505 frames after the warm-up
- **GPU clocks:** from nvidia-smi; graphics median 420 MHz (20 %), p5 225, p95 540, after the warm-up, of 2115 MHz; memory median 405 MHz; performance states P5 to P8
- **Options:** `--setting high` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile off` `--world terrain-test` `--style wireframe` `--sizes on` `--terrainVertexPath baked-offsets` `--terrainNormals double`

## The criterion (Design note 21)

| Row                                                                                      | Limit   | Value    | Verdict      | Note                                                                                     |
| ---------------------------------------------------------------------------------------- | ------- | -------- | ------------ | ---------------------------------------------------------------------------------------- |
| 50th percentile ≤ T + 0.5 ms                                                             | —       | 16.70 ms | not-measured | no window shown                                                                          |
| 95th percentile ≤ T + 1 ms                                                               | —       | 16.80 ms | not-measured | no window shown                                                                          |
| 99th percentile ≤ 2T                                                                     | —       | 33.40 ms | not-measured | no window shown                                                                          |
| ≤ 1% of intervals above 1.5 T                                                            | 1 %     | —        | not-measured | no window shown                                                                          |
| none above 3 T                                                                           | 0       | —        | not-measured | no window shown                                                                          |
| main thread ≤ 0.8 T at the 95th percentile                                               | —       | 12.40 ms | not-measured | no window shown                                                                          |
| GPU pass sum ≤ 0.8 T at the 95th percentile                                              | —       | 3.76 ms  | not-measured | no window shown; measured at a median 420 of 2115 MHz (the driver's choice at this load) |
| terrain and atmosphere GPU time, summed per frame, ≤ 6 ms (5 + 1) at the 95th percentile | 6.00 ms | —        | not-measured | no timed terrain or atmosphere pass                                                      |
| GPU resident ≤ 3 GB                                                                      | 3.0 GB  | 0.352 GB | pass         |                                                                                          |

## Terrain and atmosphere against their estimates

Each pass row's sum in each complete frame, beside the brainstorm's estimate: findings, which no verdict reads. The criterion judges the two together.

| Row        | Estimate | p50 | p95 | p99 | Finding                  |
| ---------- | -------- | --- | --- | --- | ------------------------ |
| terrain    | 5.00 ms  | —   | —   | —   | no timed terrain pass    |
| atmosphere | 1.00 ms  | —   | —   | —   | no timed atmosphere pass |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 68505 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 1133.20 ms. Frames left out at the trace's window boundaries: 1896.

| Segment             | Intervals                                                                 | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 2896 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 66.80 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 50253 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 1133.20 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 6804 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 1083.30 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 1729 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 50.00 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 3402 intervals, p50 16.70 ms, p95 16.80 ms, p99 50.00 ms, max 83.40 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 558 intervals, p50 16.70 ms, p95 33.30 ms, p99 50.00 ms, max 66.60 ms     | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 2862 intervals, p50 16.70 ms, p95 16.80 ms, p99 49.90 ms, max 66.70 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 0.0       | 0.0   | 0.0      | 16910.6         | 16910.6                  | 0             | 0                      | 0.0           |
| descent arc         | 0.8       | 0.9   | 0.9      | 23882.5         | 23882.5                  | 148           | 148                    | 120.6         |
| approach and flare  | 121.0     | 3.2   | 3.2      | 58541.5         | 58541.5                  | 293           | 293                    | 86.3          |
| low fast pass       | 204.1     | 0.0   | 0.0      | 27662.2         | 27662.2                  | 365           | 365                    | 30.0          |
| slowdown            | 101.0     | 6.0   | 6.0      | 14506.7         | 14506.7                  | 358           | 358                    | 60.0          |
| vertical descent    | 4.8       | 9.5   | 9.5      | 3219.4          | 3219.4                   | 219           | 219                    | 8.8           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 3.2             | 3.2                      | 99            | 99                     | 0.0           |

## Memory (peaks)

- GPU headline: 336 MiB from nvidia-smi
- App processes: 3803 MiB; tracing service (the measurement's own): 212 MiB
- Renderer private: 705 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 336 MiB; the GPU process alone: 282 MiB
- Adapter tally: 75 MiB

Uploads 6702 MiB; 0 pipelines created after warm-up; untimed passes — (no pass counted over a generated world).

## Sizes (R10.T14.b)

Counted once a second of script time, by unbudgeted re-selections of each pass's own inputs: F at the pass's margins, F₀ at margin 0, AR all round, and F′ and AR′ with the diagnostic occluder; each view's counts leave the forced region out, which is counted once. The views are found in each selection's input by their streaming weight, then their viewport.

- **Second view:** instrument-1 on spike-craft, wireframe, weight 0.25; viewport 571 × 140 px from 0.0 s, 376 × 140 px from 0.1 s.
- **The header's lowest sampled height:** -9,538.1 m.
- **The lit and wireframe passes' forced sets** (T14.c's finding 4): — (the lit and wireframe passes never stood in the same frame).

### The wireframe cache: spike (wireframe, weight 1), instrument-1 (wireframe, weight 0.25)

1,205 samples; attached to 1 pass; 520 slots of 139,428 B.

| Count                     | spike                                              | instrument-1                                       | union                                                  |
| ------------------------- | -------------------------------------------------- | -------------------------------------------------- | ------------------------------------------------------ |
| F                         | 1,219 (571.3 s, descent arc, 139,675 m up)         | 1,219 (571.3 s, descent arc, 139,675 m up)         | 1,219 (571.3 s, descent arc, 139,675 m up)             |
| F₀ (margin 0)             | 1,219 (571.3 s, descent arc, 139,675 m up)         | 1,219 (571.3 s, descent arc, 139,675 m up)         | 1,219 (571.3 s, descent arc, 139,675 m up)             |
| AR (all round)            | 1,759 (1,054.0 s, approach and flare, 14,008 m up) | 1,759 (1,054.0 s, approach and flare, 14,008 m up) | 1,759 (1,054.0 s, approach and flare, 14,008 m up)     |
| F′ (diagnostic occluder)  | 1,219 (638.4 s, descent arc, 115,609 m up)         | 1,016 (602.1 s, descent arc, 128,604 m up)         | 1,219 (638.4 s, descent arc, 115,609 m up)             |
| AR′ (diagnostic occluder) | 1,759 (1,054.0 s, approach and flare, 14,008 m up) | 1,759 (1,054.0 s, approach and flare, 14,008 m up) | 1,759 (1,054.0 s, approach and flare, 14,008 m up)     |
| Forced, once              |                                                    |                                                    | 107, 0 seen (993.0 s, approach and flare, 69,875 m up) |

| Form                                 | Slots   | Bytes           | Slots, diagnostic | Bytes, diagnostic | Against the as-built 520 slots |
| ------------------------------------ | ------- | --------------- | ----------------- | ----------------- | ------------------------------ |
| Ruled: Σ max(1.3 AR, 3 F) + forced   | 7,421.0 | 1,034,695,188 B | 6,812.0           | 949,783,536 B     | × 14.27                        |
| (a) its largest same-sample value    | 7,314.0 | 1,019,776,392 B | 6,333.0           | 882,997,524 B     | × 14.07                        |
| (b) max(1.3 AR∪, 3 F∪) + forced      | 3,764.0 | 524,806,992 B   | 3,764.0           | 524,806,992 B     | × 7.24                         |
| (c) the frustum rule, Σ 3 F + forced | 7,421.0 | 1,034,695,188 B | 6,812.0           | 949,783,536 B     | × 14.27                        |

- **A/B:** the diagnostic occluder at its deepest -9,538.1 m, against the as-built -66,604.0 m; the price in slots, each a share of the sized all-round peak Σ 1.3 AR (4,573.4 slots): the rule's own, rule − rule′, 609.0 slots, 13.3 %, the reading the 10 % test takes (over 10 %: a finding for main); the union's all-round term 1.3 (AR − AR′) 0.0 (0.0 %), its frustum term 3 (F − F′) 0.0 (0.0 %).
- **The all-round peak's residency:** the resident share's peak 100.0 % (426.1 s, descent arc, 191,727 m up), the swept share's 100.0 % (263.1 s, descent arc, 250,141 m up), the cap at AR∪'s peak × 0.30; resident at 100 % at some sample. The descent never turns, its views look along the track and VIEW's LOOK turns are not flown: a "never" is the descent's.
- **The forced region:** first selected at 990.8 s; every forced patch resident first at 1,174.0 s, and at every sample after; touchdown at 1,199.9 s (99 forced, 99 of them resident); a lead of 25.9 s (touchdown − every forced patch resident); the contact by the session's rule at 990.8 s (105 forced, 0 of them resident).
- **Box heights by level** (the frustum count's seen leaves): L1 max 404,228.6 m, median at F∪'s peak —; L4 max 168,296.7 m, median at F∪'s peak —; L5 max 135,148.2 m, median at F∪'s peak —; L6 max 84,750.5 m, median at F∪'s peak —; L7 max 48,279.8 m, median at F∪'s peak 44,016.2 m; L8 max 29,606.8 m, median at F∪'s peak 25,955.3 m; L9 max 18,209.8 m, median at F∪'s peak 14,933.6 m; L10 max 8,268.8 m, median at F∪'s peak —; L11 max 5,003.0 m, median at F∪'s peak —; L12 max 4,004.2 m, median at F∪'s peak —; L13 max 3,615.4 m, median at F∪'s peak —; L14 max 3,466.4 m, median at F∪'s peak —; L15 max 3,400.9 m, median at F∪'s peak —; L16 max 3,332.7 m, median at F∪'s peak —; L17 max 3,313.3 m, median at F∪'s peak —.
- **k_n = ε_n P₀ ÷ e_n**, 1 level table met; the last (from 0.1 s): L0 49.42, L1 80.96, L2 138.01, L3 200.74, L4 282.75, L5 359.13, L6 382.05, L7 451.28, L8 539.52, L9 530.64, L10 464.68, L11 365.54, L12 304.93, L13 280.19, L14 279.03, L15 276.57, L16 272.83, L17 0.00.
- **The counting's cost:** p50 7.00 ms, p95 21.20 ms, max 34.60 ms a sample.

| Segment             | Frames | Drawn, mean / max | Standing in | Missing | Stand-in share  | Limited frames | Made resident / s |
| ------------------- | ------ | ----------------- | ----------- | ------- | --------------- | -------------- | ----------------- |
| orbit coast         | 3,463  | 0.0 / 0           | 0.0 / 0     | 0.0 / 1 | 0.0 % / 0.0 %   | 0              | 0.0               |
| descent arc         | 52,149 | 147.7 / 260       | 0.4 / 27    | 0.0 / 1 | 1.6 % / 100.0 % | 29,086         | 0.9               |
| approach and flare  | 6,804  | 237.8 / 260       | 29.7 / 332  | 0.0 / 0 | 8.6 % / 82.8 %  | 6,804          | 3.2               |
| low fast pass       | 1,729  | 63.6 / 75         | 308.5 / 372 | 0.0 / 0 | 84.5 % / 88.2 % | 1,729          | 0.0               |
| slowdown            | 3,402  | 39.8 / 59         | 323.4 / 380 | 0.0 / 0 | 90.5 % / 94.3 % | 3,402          | 6.0               |
| vertical descent    | 558    | 23.5 / 54         | 165.8 / 303 | 0.0 / 0 | 61.2 % / 92.4 % | 246            | 9.5               |
| hover and touchdown | 2,862  | 18.2 / 19         | 0.0 / 0     | 0.0 / 0 | 0.0 % / 0.0 %   | 0              | 0.0               |
| after the script    | 2,531  | 18.0 / 18         | 0.0 / 0     | 0.0 / 0 | 0.0 % / 0.0 %   | 0              | —                 |

The pass's own counts at the samples, the largest in each segment:

| Segment             | Samples | Selected | Limited samples | Selected resident | Slots used | Pinned forced | Pinned drawn |
| ------------------- | ------- | -------- | --------------- | ----------------- | ---------- | ------------- | ------------ |
| orbit coast         | 60      | 0        | 0               | 0                 | 1          | 0             | 1            |
| descent arc         | 897     | 260      | 500             | 260               | 520        | 0             | 399          |
| approach and flare  | 97      | 373      | 97              | 259               | 520        | 108           | 386          |
| low fast pass       | 30      | 404      | 30              | 70                | 520        | 102           | 122          |
| slowdown            | 60      | 412      | 60              | 76                | 520        | 101           | 93           |
| vertical descent    | 10      | 327      | 5               | 122               | 520        | 99            | 57           |
| hover and touchdown | 50      | 99       | 0               | 99                | 520        | 99            | 25           |
| after the script    | 1       | 99       | 0               | 99                | 520        | 99            | 25           |

### The height workers by segment

3 workers wrapped; the flight's 2,553 bakes of 2,553 posted, 865 never made resident, 2.2 a second.

| Segment            | Bakes / s | Baked | Never resident | Post to reply, p50 / p95 (ms) | Ground speed ÷ height, mean / max (s⁻¹) |
| ------------------ | --------- | ----- | -------------- | ----------------------------- | --------------------------------------- |
| orbit coast        | 23.8      | 1     | 0              | 39.9 / 39.9                   | 0.024 / 0.024                           |
| descent arc        | 1.0       | 855   | 6              | 178.4 / 349.5                 | 0.040 / 1.000                           |
| approach and flare | 6.5       | 776   | 395            | 420.8 / 739.2                 | 0.963 / 4.708                           |
| low fast pass      | 9.2       | 275   | 275            | 649.5 / 715.4                 | 1.000 / 1.000                           |
| slowdown           | 9.2       | 550   | 188            | 648.3 / 733.0                 | 0.509 / 0.805                           |
| vertical descent   | 11.4      | 96    | 1              | 522.8 / 670.2                 | 0.000 / 0.000                           |

### Memory: itemized, the engine's tally and the driver

- The wireframe cache: 520 slots × 139,428 B = 69.1 MiB
- The session's coarse-field-gpu: 0.1 MiB at its peak (0.0 s)
- The cascades: none allocated (the wireframe alone was drawn, and the cascades are drawn for photorealistic views only: the engine allocated nothing under shadows); T9.a's spec 64.0 MiB
- Itemized GPU (the caches, the session's masks and maps, the cascades): 69.3 MiB; with the workers' field copies (3.3 MiB at their peak): 72.6 MiB, against 2–3 GB (discrete); the pool's own copy beside them 0.0 MiB; 3 workers started; 1,260 samples at 1 Hz
- The engine's tally, the app's own GPU figure: 75.0 MiB at its whole peak (977.2 s); each category at its own peak: other 1.1 MiB (977.2 s), coarse-field-gpu 0.1 MiB (before the flight), atmosphere-tables 0.0 MiB (before the flight), height-cache 69.2 MiB (0.0 s), render-targets 4.7 MiB (0.0 s)
- The driver: nvidia-smi's peak less its pre-launch baseline 336.0 MiB; the GPU process's peak 282.0 MiB
- At the run's end: the device 464.0 MiB, the GPU process 274.0 MiB, the rest of the card 190.0 MiB (the pre-launch baseline 136.0 MiB, a drift of 54.0 MiB)
- Less the driver: the itemized GPU -266.7 MiB, the engine's tally -261.0 MiB

### The binding

- maxStorageBufferBindingSize: the adapter's 2,147,483,644 B, requested 1,073,741,824 B, the device's 1,073,741,824 B
- The wireframe cache's 520 slots: its ceiling 10,589 slots at the requested binding, 1,323 slots at WebGPU's default 128 MiB

### The readout and the load

- VIEW's terrain readout every 250 ms, not shown: 4,975 publishes, 4,974 holding an answer; 4,583 asked of the workers, 4,583 answered, 0 failed; 0 of its guards' calls to fail the session counted and withheld
- The 1-minute load: 6.03 at 0 s, 5.81 at 60 s, 6.45 at 120 s, 5.91 at 180 s, 4.52 at 240 s, 4.64 at 300 s, 6.21 at 360 s, 4.05 at 420 s, 6.72 at 480 s, 6.99 at 540 s, 4.56 at 600 s, 6.65 at 660 s, 11.66 at 720 s, 14.41 at 780 s, 11.74 at 840 s, 9.48 at 900 s, 8.43 at 960 s, 9.15 at 1,020 s, 9.41 at 1,080 s, 10.15 at 1,140 s, 9.41 at 1,200 s, 8.01 at 1,230 s
