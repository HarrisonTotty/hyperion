# Descent spike: effect, high, 2026-10-21

- **Overall:** not-measured (provisional: not a quiet machine)
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 3.91, 3.51, 4.05
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, world terrain-test, style photorealistic, sizes on, window hidden, canvas 1398 × 793 px
- **T:** — (no window shown); warm-up 10 s
- **Trace:** 9 perfetto-proto windows, 1139.1 s traced after the warm-up, unprofiled; 8 boundaries left out 4729 frames (largest stall 66.70 ms); largest file 559 MiB, buffer use up to 58 %
- **Incomplete pass times:** 0 dropped and 10 partial of 66659 frames after the warm-up, left out of the GPU rows' sums, whose verdicts hold whatever their times
- **GPU clocks:** from nvidia-smi; graphics median 1965 MHz (93 %), p5 1170, p95 1980, after the warm-up, of 2115 MHz; memory median 9501 MHz; performance states P0 to P5
- **Options:** `--setting high` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile off` `--world terrain-test` `--style photorealistic` `--sizes on` `--terrainVertexPath baked-offsets` `--terrainNormals double`

## The criterion (Design note 21)

| Row                                                                                      | Limit   | Value    | Verdict      | Note                                                                                  |
| ---------------------------------------------------------------------------------------- | ------- | -------- | ------------ | ------------------------------------------------------------------------------------- |
| 50th percentile ≤ T + 0.5 ms                                                             | —       | 16.70 ms | not-measured | no window shown                                                                       |
| 95th percentile ≤ T + 1 ms                                                               | —       | 16.80 ms | not-measured | no window shown                                                                       |
| 99th percentile ≤ 2T                                                                     | —       | 33.40 ms | not-measured | no window shown                                                                       |
| ≤ 1% of intervals above 1.5 T                                                            | 1 %     | —        | not-measured | no window shown                                                                       |
| none above 3 T                                                                           | 0       | —        | not-measured | no window shown                                                                       |
| main thread ≤ 0.8 T at the 95th percentile                                               | —       | 15.60 ms | not-measured | no window shown                                                                       |
| GPU pass sum ≤ 0.8 T at the 95th percentile                                              | —       | 9.48 ms  | not-measured | no window shown                                                                       |
| terrain and atmosphere GPU time, summed per frame, ≤ 6 ms (5 + 1) at the 95th percentile | 6.00 ms | 3.24 ms  | pass         | 10 frames with incomplete pass times left out; the verdict holds whatever their times |
| GPU resident ≤ 3 GB                                                                      | 3.0 GB  | 0.838 GB | pass         |                                                                                       |

## Terrain and atmosphere against their estimates

Each pass row's sum in each complete frame, beside the brainstorm's estimate: findings, which no verdict reads. The criterion judges the two together.

| Row        | Estimate | p50     | p95     | p99     | Finding                                                                        |
| ---------- | -------- | ------- | ------- | ------- | ------------------------------------------------------------------------------ |
| terrain    | 5.00 ms  | 1.38 ms | 3.24 ms | 3.36 ms |                                                                                |
| atmosphere | 1.00 ms  | —       | —       | —       | 10 frames' pass times were incomplete (the GPU was more than 27 frames behind) |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 66659 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 66.80 ms. Frames left out at the trace's window boundaries: 4729.

| Segment             | Intervals                                                               | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 2985 intervals, p50 16.70 ms, p95 16.80 ms, p99 16.80 ms, max 66.70 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 48338 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 66.80 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 6622 intervals, p50 16.70 ms, p95 33.30 ms, p99 50.00 ms, max 66.70 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 1717 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 50.10 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 3475 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 66.70 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 587 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.00 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 2934 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 66.70 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 0.0       | 0.0   | 0.0      | 16910.6         | 16910.6                  | 0             | 0                      | 0.0           |
| descent arc         | 2.2       | 2.0   | 2.0      | 23882.5         | 23882.5                  | 365           | 365                    | 201.0         |
| approach and flare  | 129.9     | 6.8   | 6.8      | 58541.5         | 58541.5                  | 666           | 666                    | 115.6         |
| low fast pass       | 204.1     | 0.0   | 0.0      | 27662.2         | 27662.2                  | 642           | 642                    | 30.0          |
| slowdown            | 101.0     | 3.9   | 3.9      | 14506.7         | 14506.7                  | 316           | 316                    | 60.0          |
| vertical descent    | 2.4       | 8.1   | 8.1      | 3219.4          | 3219.4                   | 228           | 228                    | 10.0          |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 3.2             | 3.2                      | 99            | 99                     | 0.7           |

## Memory (peaks)

- GPU headline: 799 MiB from nvidia-smi
- App processes: 4206 MiB; tracing service (the measurement's own): 497 MiB
- Renderer private: 948 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 799 MiB; the GPU process alone: 745 MiB
- Adapter tally: 472 MiB

Uploads 10410 MiB; 2 pipelines created after warm-up; untimed passes — (no pass counted over a generated world).

## Sizes (R10.T14.b)

Counted once a second of script time, by unbudgeted re-selections of each pass's own inputs: F at the pass's margins, F₀ at margin 0, AR all round, and F′ and AR′ with the diagnostic occluder; each view's counts leave the forced region out, which is counted once. The views are found in each selection's input by their streaming weight, then their viewport.

- **Second view:** instrument-1 on spike-craft, photorealistic, weight 0.25; viewport 571 × 140 px from 0.0 s, 376 × 140 px from 0.3 s.
- **The header's lowest sampled height:** -9,538.1 m.
- **The lit and wireframe passes' forced sets** (T14.c's finding 4): — (the lit and wireframe passes never stood in the same frame).

### The lit cache: spike (photorealistic, weight 1), instrument-1 (photorealistic, weight 0.25)

1,226 samples; attached to 1 pass; 1,286 slots of 311,024 B.

| Count                     | spike                                              | instrument-1                                       | union                                                  |
| ------------------------- | -------------------------------------------------- | -------------------------------------------------- | ------------------------------------------------------ |
| F                         | 2,841 (861.5 s, descent arc, 35,608 m up)          | 2,613 (525.1 s, descent arc, 156,225 m up)         | 2,853 (899.1 s, descent arc, 22,154 m up)              |
| F₀ (margin 0)             | 2,841 (861.5 s, descent arc, 35,608 m up)          | 2,613 (525.1 s, descent arc, 156,225 m up)         | 2,841 (861.5 s, descent arc, 35,608 m up)              |
| AR (all round)            | 3,993 (1,054.0 s, approach and flare, 14,008 m up) | 3,993 (1,054.0 s, approach and flare, 14,008 m up) | 3,993 (1,054.0 s, approach and flare, 14,008 m up)     |
| F′ (diagnostic occluder)  | 2,841 (861.5 s, descent arc, 35,608 m up)          | 2,378 (899.1 s, descent arc, 22,154 m up)          | 2,853 (899.1 s, descent arc, 22,154 m up)              |
| AR′ (diagnostic occluder) | 3,993 (1,054.0 s, approach and flare, 14,008 m up) | 3,993 (1,054.0 s, approach and flare, 14,008 m up) | 3,993 (1,054.0 s, approach and flare, 14,008 m up)     |
| Forced, once              |                                                    |                                                    | 107, 0 seen (991.0 s, approach and flare, 76,357 m up) |

| Form                                 | Slots    | Bytes           | Slots, diagnostic | Bytes, diagnostic | Against the as-built 1,286 slots |
| ------------------------------------ | -------- | --------------- | ----------------- | ----------------- | -------------------------------- |
| Ruled: Σ max(1.3 AR, 3 F) + forced   | 16,469.0 | 5,122,254,256 B | 15,764.0          | 4,902,982,336 B   | × 12.81                          |
| (a) its largest same-sample value    | 15,678.0 | 4,876,234,272 B | 15,258.0          | 4,745,604,192 B   | × 12.19                          |
| (b) max(1.3 AR∪, 3 F∪) + forced      | 8,666.0  | 2,695,333,984 B | 8,666.0           | 2,695,333,984 B   | × 6.74                           |
| (c) the frustum rule, Σ 3 F + forced | 16,469.0 | 5,122,254,256 B | 15,764.0          | 4,902,982,336 B   | × 12.81                          |

- **A/B:** the diagnostic occluder at its deepest -9,538.1 m, against the as-built -66,604.0 m; the price in slots, each a share of the sized all-round peak Σ 1.3 AR (10,381.8 slots): the rule's own, rule − rule′, 705.0 slots, 6.8 %, the reading the 10 % test takes; the union's all-round term 1.3 (AR − AR′) 0.0 (0.0 %), its frustum term 3 (F − F′) 0.0 (0.0 %).
- **The all-round peak's residency:** the resident share's peak 11.1 % (433.0 s, descent arc, 189,229 m up), the swept share's 100.0 % (263.2 s, descent arc, 250,129 m up), the cap at AR∪'s peak × 0.32; resident at 100 % never. The descent never turns, its views look along the track and VIEW's LOOK turns are not flown: a "never" is the descent's.
- **The forced region:** first selected at 990.8 s; every forced patch resident first at 1,177.0 s, and at every sample after; touchdown at 1,199.9 s (99 forced, 99 of them resident); a lead of 22.9 s (touchdown − every forced patch resident); the contact by the session's rule at 990.8 s (105 forced, 0 of them resident).
- **Box heights by level** (the frustum count's seen leaves): L1 max 404,228.6 m, median at F∪'s peak —; L4 max 168,335.4 m, median at F∪'s peak —; L5 max 135,203.7 m, median at F∪'s peak —; L6 max 84,750.5 m, median at F∪'s peak —; L7 max 48,279.8 m, median at F∪'s peak —; L8 max 29,606.8 m, median at F∪'s peak 26,074.2 m; L9 max 18,209.8 m, median at F∪'s peak 15,081.1 m; L10 max 8,508.2 m, median at F∪'s peak 7,211.3 m; L11 max 3,843.1 m, median at F∪'s peak —; L12 max 2,245.1 m, median at F∪'s peak —; L13 max 1,866.3 m, median at F∪'s peak —; L14 max 1,717.4 m, median at F∪'s peak —; L15 max 1,651.9 m, median at F∪'s peak —; L16 max 1,619.1 m, median at F∪'s peak —; L17 max 1,599.7 m, median at F∪'s peak —.
- **k_n = ε_n P₀ ÷ e_n**, 1 level table met; the last (from 0.6 s): L0 49.42, L1 80.96, L2 138.01, L3 200.74, L4 282.75, L5 359.13, L6 382.05, L7 451.28, L8 539.52, L9 530.64, L10 464.68, L11 365.54, L12 304.93, L13 280.19, L14 279.03, L15 276.57, L16 272.83, L17 0.00.
- **The counting's cost:** p50 16.60 ms, p95 32.10 ms, max 49.60 ms a sample.

| Segment             | Frames | Drawn, mean / max | Standing in | Missing | Stand-in share   | Limited frames | Made resident / s |
| ------------------- | ------ | ----------------- | ----------- | ------- | ---------------- | -------------- | ----------------- |
| orbit coast         | 3,538  | 0.0 / 0           | 0.0 / 0     | 0.0 / 1 | 0.0 % / 0.0 %    | 0              | 0.0               |
| descent arc         | 53,067 | 349.1 / 643       | 22.6 / 641  | 0.0 / 1 | 10.3 % / 100.0 % | 29,646         | 2.0               |
| approach and flare  | 6,622  | 579.4 / 643       | 75.6 / 610  | 0.0 / 0 | 11.3 % / 86.2 %  | 6,394          | 6.8               |
| low fast pass       | 1,717  | 285.6 / 317       | 389.2 / 421 | 0.0 / 0 | 60.6 % / 65.6 %  | 1,717          | 0.0               |
| slowdown            | 3,475  | 20.1 / 283        | 307.7 / 530 | 0.0 / 0 | 98.6 % / 100.0 % | 124            | 3.9               |
| vertical descent    | 587    | 7.5 / 26          | 191.9 / 264 | 0.0 / 0 | 77.4 % / 98.9 %  | 0              | 8.1               |
| hover and touchdown | 2,934  | 18.2 / 20         | 0.0 / 1     | 0.0 / 0 | 0.0 % / 1.0 %    | 0              | 0.0               |
| after the script    | 8,076  | 18.0 / 18         | 0.0 / 0     | 0.0 / 0 | 0.0 % / 0.0 %    | 0              | —                 |

The pass's own counts at the samples, the largest in each segment:

| Segment             | Samples | Selected | Limited samples | Selected resident | Slots used | Pinned forced | Pinned drawn |
| ------------------- | ------- | -------- | --------------- | ----------------- | ---------- | ------------- | ------------ |
| orbit coast         | 60      | 0        | 0               | 0                 | 1          | 0             | 1            |
| descent arc         | 898     | 643      | 506             | 643               | 1,286      | 0             | 934          |
| approach and flare  | 117     | 750      | 114             | 642               | 1,286      | 108           | 916          |
| low fast pass       | 30      | 643      | 30              | 277               | 1,286      | 102           | 462          |
| slowdown            | 60      | 643      | 3               | 241               | 1,286      | 101           | 401          |
| vertical descent    | 10      | 267      | 0               | 99                | 1,286      | 99            | 26           |
| hover and touchdown | 50      | 100      | 0               | 99                | 1,286      | 99            | 20           |
| after the script    | 1       | 99       | 0               | 99                | 1,286      | 99            | 19           |

### The height workers by segment

3 workers wrapped; the flight's 3,653 bakes of 3,653 posted, 731 never made resident, 3.1 a second.

| Segment             | Bakes / s | Baked | Never resident | Post to reply, p50 / p95 (ms) | Ground speed ÷ height, mean / max (s⁻¹) |
| ------------------- | --------- | ----- | -------------- | ----------------------------- | --------------------------------------- |
| orbit coast         | 21.5      | 1     | 0              | 43.5 / 43.5                   | 0.024 / 0.024                           |
| descent arc         | 2.2       | 1,800 | 13             | 438.9 / 530.9                 | 0.040 / 1.000                           |
| approach and flare  | 9.3       | 1,116 | 297            | 526.6 / 894.7                 | 0.963 / 4.708                           |
| low fast pass       | 7.1       | 212   | 212            | 849.9 / 894.8                 | 1.000 / 1.000                           |
| slowdown            | 7.3       | 439   | 206            | 828.3 / 882.3                 | 0.505 / 0.805                           |
| vertical descent    | 8.7       | 84    | 3              | 742.3 / 825.2                 | 0.000 / 0.000                           |
| hover and touchdown | 2,000.0   | 1     | 0              | 694.5 / 694.5                 | 0.000 / 0.000                           |

### Memory: itemized, the engine's tally and the driver

- The lit cache: 1,286 slots × 311,024 B = 381.4 MiB
- The session's coarse-field-gpu: 0.1 MiB at its peak (0.0 s)
- The session's other: 0.0 MiB at its peak (143.2 s)
- The session's shadows: 64.0 MiB at its peak (453.9 s), its own figure for the cascades, which the itemized total counts as the engine allocated them
- The cascades: 64.0 MiB as the engine allocated them (its shadows category at its peak), T9.a's spec 64.0 MiB as built
- Itemized GPU (the caches, the session's masks and maps, the cascades): 445.6 MiB; with the workers' field copies (3.3 MiB at their peak): 448.9 MiB, against 2–3 GB (discrete); the pool's own copy beside them 0.0 MiB; 3 workers started; 1,353 samples at 1 Hz
- The engine's tally, the app's own GPU figure: 472.3 MiB at its whole peak (977.2 s); each category at its own peak: other 1.7 MiB (977.2 s), coarse-field-gpu 0.1 MiB (before the flight), atmosphere-tables 0.0 MiB (0.6 s), render-targets 24.3 MiB (0.6 s), height-cache 382.6 MiB (0.6 s), shadows 64.0 MiB (453.5 s)
- The driver: nvidia-smi's peak less its pre-launch baseline 799.0 MiB; the GPU process's peak 745.0 MiB
- At the run's end: the device 927.0 MiB, the GPU process 737.0 MiB, the rest of the card 190.0 MiB (the pre-launch baseline 136.0 MiB, a drift of 54.0 MiB)
- Less the driver: the itemized GPU -353.4 MiB, the engine's tally -326.7 MiB

### The binding

- maxStorageBufferBindingSize: the adapter's 2,147,483,644 B, requested 1,073,741,824 B, the device's 1,073,741,824 B
- The lit cache's 1,286 slots: its ceiling 10,589 slots at the requested binding, 1,323 slots at WebGPU's default 128 MiB

### The readout and the load

- VIEW's terrain readout every 250 ms, not shown: 5,357 publishes, 5,355 holding an answer; 4,615 asked of the workers, 4,615 answered, 0 failed; 0 of its guards' calls to fail the session counted and withheld
- The 1-minute load: 3.91 at 0 s, 3.43 at 60 s, 3.95 at 120 s, 4.48 at 180 s, 4.72 at 240 s, 4.29 at 300 s, 4.16 at 360 s, 4.83 at 420 s, 6.64 at 480 s, 5.85 at 540 s, 5.14 at 600 s, 4.60 at 660 s, 4.55 at 720 s, 4.66 at 780 s, 5.39 at 840 s, 5.65 at 900 s, 6.41 at 960 s, 6.68 at 1,020 s, 5.81 at 1,080 s, 5.68 at 1,140 s, 5.28 at 1,200 s, 4.55 at 1,230 s
