# Descent spike: thewired, low, 2026-11-18

- **Overall:** not-measured (provisional: not a quiet machine)
- **Machine:** Intel(R) Core(TM) i7-8665U CPU @ 1.90GHz, 8 threads; GPU 32902:16032; governor powersave; load average 3.77, 3.18, 1.86
- **Versions:** app 0.1.0, Electron 45.1.0, Chromium 156.0.8078.49
- **Launch:** linux, vulkan mode, timer full, seed 7, world terrain-test, style photorealistic, sizes on, window hidden, canvas 805 × 433 px
- **T:** — (no window shown); warm-up 10 s; pacing display, measured vsync 16.60 ms
- **Trace:** 9 perfetto-proto windows, 1198.0 s traced after the warm-up, unprofiled; 8 boundaries left out 436 frames (largest stall 916.70 ms); largest file 74 MiB, buffer use up to 8 %
- **Incomplete pass times:** 6 dropped and 0 partial of 18478 frames after the warm-up, left out of the GPU rows' sums, whose verdicts hold whatever their times
- **GPU clocks:** from i915-sysfs; graphics median 1150 MHz (100 %), p5 300, p95 1150, after the warm-up, of 1150 MHz
- **Options:** `--setting low` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile off` `--world terrain-test` `--style photorealistic` `--sizes on` `--terrainVertexPath face-differences` `--terrainNormals mesh`

## The criterion (Design note 21)

| Row                                                                                        | Limit    | Value     | Verdict      | Note                                                                                 |
| ------------------------------------------------------------------------------------------ | -------- | --------- | ------------ | ------------------------------------------------------------------------------------ |
| 50th percentile ≤ T + 0.5 ms                                                               | —        | 16.70 ms  | not-measured | no window shown                                                                      |
| 95th percentile ≤ 35 ms                                                                    | —        | 266.70 ms | not-measured | no window shown                                                                      |
| 99th percentile ≤ 2T                                                                       | —        | 816.60 ms | not-measured | no window shown                                                                      |
| ≤ 1% of intervals above 1.5 T                                                              | 1 %      | —         | not-measured | no window shown                                                                      |
| none above 3 T                                                                             | 0        | —         | not-measured | no window shown                                                                      |
| main thread ≤ 0.8 T at the 95th percentile                                                 | —        | 24.00 ms  | not-measured | no window shown                                                                      |
| GPU pass sum ≤ 0.8 T at the 95th percentile                                                | —        | 249.02 ms | not-measured | no window shown                                                                      |
| terrain and atmosphere GPU time, summed per frame, ≤ 18 ms (14 + 4) at the 95th percentile | 18.00 ms | 1.92 ms   | pass         | 6 frames with incomplete pass times left out; the verdict holds whatever their times |
| GPU resident ≤ 1 GB                                                                        | 1.0 GB   | 0.327 GB  | pass         |                                                                                      |

## Terrain and atmosphere against their estimates

Each pass row's sum in each complete frame, beside the brainstorm's estimate: findings, which no verdict reads. The criterion judges the two together.

| Row        | Estimate | p50     | p95     | p99     | Finding                                                                       |
| ---------- | -------- | ------- | ------- | ------- | ----------------------------------------------------------------------------- |
| terrain    | 14.00 ms | 1.74 ms | 1.92 ms | 3.98 ms |                                                                               |
| atmosphere | 4.00 ms  | —       | —       | —       | 6 frames' pass times were incomplete (the GPU was more than 27 frames behind) |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 18478 intervals, p50 16.70 ms, p95 266.70 ms, p99 816.60 ms, max 5266.60 ms. Frames left out at the trace's window boundaries: 436.

| Segment             | Intervals                                                                   | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | --------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 2468 intervals, p50 16.70 ms, p95 33.40 ms, p99 50.00 ms, max 183.30 ms     | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 12360 intervals, p50 16.70 ms, p95 283.30 ms, p99 850.00 ms, max 2149.80 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 574 intervals, p50 183.40 ms, p95 550.00 ms, p99 1016.60 ms, max 1166.60 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 81 intervals, p50 100.00 ms, p95 1383.30 ms, p99 2099.80 ms, max 2099.80 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 1894 intervals, p50 16.70 ms, p95 33.40 ms, p99 200.10 ms, max 5266.60 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 45 intervals, p50 133.30 ms, p95 849.90 ms, p99 1283.30 ms, max 1283.30 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 1055 intervals, p50 33.30 ms, p95 83.30 ms, p99 233.30 ms, max 4149.80 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 0.0       | 0.0   | 0.0      | 3204.5          | 3204.5                   | 0             | 0                      | 0.0           |
| descent arc         | 0.5       | 0.7   | 0.7      | 3790.0          | 3790.0                   | 31            | 31                     | 51.0          |
| approach and flare  | 82.2      | 6.6   | 6.6      | 12821.3         | 12821.3                  | 205           | 205                    | 41.2          |
| low fast pass       | 126.6     | 18.9  | 18.9     | 9220.7          | 9220.7                   | 376           | 376                    | 30.0          |
| slowdown            | 27.3      | 4.3   | 4.3      | 4835.6          | 4835.6                   | 10            | 10                     | 21.4          |
| vertical descent    | 10.0      | 10.3  | 10.3     | 1073.1          | 1073.1                   | 128           | 128                    | 4.1           |
| hover and touchdown | 0.2       | 0.1   | 0.1      | 1.1             | 1.1                      | 5             | 5                      | 6.5           |

## Memory (peaks)

- GPU headline: 312 MiB from drm-fdinfo
- App processes: 2549 MiB; tracing service (the measurement's own): 124 MiB
- Renderer private: 1094 MiB
- DRM fdinfo resident: 312 MiB
- nvidia-smi less its baseline: — (no nvidia-smi baseline (not NVIDIA)); the GPU process alone: — (nvidia-smi does not list the GPU process)
- Adapter tally: 109 MiB

Uploads 2335 MiB; 233 pipelines created after warm-up; untimed passes — (no pass counted over a generated world).

## Sizes (R10.T14.b)

Counted once a second of script time, by unbudgeted re-selections of each pass's own inputs: F at the pass's margins, F₀ at margin 0, AR all round, and F′ and AR′ with the diagnostic occluder; each view's counts leave the forced region out, which is counted once. The views are found in each selection's input by their streaming weight, then their viewport.

- **Second view:** instrument-1 on spike-craft, wireframe, weight 0.25; viewport 274 × 140 px from 0.0 s, 80 × 146 px from 0.3 s.
- **The header's lowest sampled height:** -9,538.1 m.
- **The lit and wireframe passes' forced sets** (T14.c's finding 4): compared at 3,347 frames, different at 3,208, first at 1,050.0 s, by at most 105 patches.

### The lit cache: spike (photorealistic, weight 1)

773 samples; attached to 1 pass; 375 slots of 178,908 B.

| Count                     | spike                                            | union                                                  |
| ------------------------- | ------------------------------------------------ | ------------------------------------------------------ |
| F                         | 460 (1,052.1 s, approach and flare, 15,091 m up) | 460 (1,052.1 s, approach and flare, 15,091 m up)       |
| F₀ (margin 0)             | 455 (1,047.6 s, approach and flare, 17,763 m up) | 455 (1,047.6 s, approach and flare, 17,763 m up)       |
| AR (all round)            | 454 (1,090.4 s, low fast pass, 300 m up)         | 454 (1,090.4 s, low fast pass, 300 m up)               |
| F′ (diagnostic occluder)  | 460 (1,052.1 s, approach and flare, 15,091 m up) | 460 (1,052.1 s, approach and flare, 15,091 m up)       |
| AR′ (diagnostic occluder) | 454 (1,090.4 s, low fast pass, 300 m up)         | 454 (1,090.4 s, low fast pass, 300 m up)               |
| Forced, once              |                                                  | 108, 0 seen (991.0 s, approach and flare, 76,466 m up) |

| Form                                 | Slots   | Bytes         | Slots, diagnostic | Bytes, diagnostic | Against the as-built 375 slots |
| ------------------------------------ | ------- | ------------- | ----------------- | ----------------- | ------------------------------ |
| Ruled: Σ max(1.3 AR, 3 F) + forced   | 1,488.0 | 266,215,104 B | 1,488.0           | 266,215,104 B     | × 3.97                         |
| (a) its largest same-sample value    | 1,485.0 | 265,678,380 B | 1,485.0           | 265,678,380 B     | × 3.96                         |
| (b) max(1.3 AR∪, 3 F∪) + forced      | 1,488.0 | 266,215,104 B | 1,488.0           | 266,215,104 B     | × 3.97                         |
| (c) the frustum rule, Σ 3 F + forced | 1,488.0 | 266,215,104 B | 1,488.0           | 266,215,104 B     | × 3.97                         |

- **A/B:** the diagnostic occluder at its deepest -9,538.1 m, against the as-built -66,604.0 m; the price in slots, each a share of the sized all-round peak Σ 1.3 AR (590.2 slots): the rule's own, rule − rule′, 0.0 slots, 0.0 %, the reading the 10 % test takes; the union's all-round term 1.3 (AR − AR′) 0.0 (0.0 %), its frustum term 3 (F − F′) 0.0 (0.0 %).
- **The all-round peak's residency:** the resident share's peak 100.0 % (426.5 s, descent arc, 191,571 m up), the swept share's 100.0 % (238.1 s, descent arc, 259,109 m up), the cap at AR∪'s peak × 0.83; resident at 100 % at some sample. The descent never turns, its views look along the track and VIEW's LOOK turns are not flown: a "never" is the descent's.
- **The forced region:** first selected at 991.0 s; every forced patch resident first at 1,182.0 s, and at every sample after; touchdown at 1,199.9 s (0 forced, 0 of them resident); a lead of 17.9 s (touchdown − every forced patch resident); the contact by the session's rule at 991.0 s (108 forced, 0 of them resident).
- **Box heights by level** (the frustum count's seen leaves): L0 max 867,735.0 m, median at F∪'s peak —; L1 max 404,228.6 m, median at F∪'s peak —; L2 max 243,412.6 m, median at F∪'s peak —; L3 max 187,027.6 m, median at F∪'s peak —; L4 max 168,335.4 m, median at F∪'s peak —; L5 max 156,737.7 m, median at F∪'s peak —; L6 max 147,052.2 m, median at F∪'s peak 66,281.3 m; L7 max 139,255.1 m, median at F∪'s peak 44,273.6 m; L8 max 135,673.8 m, median at F∪'s peak 25,550.9 m; L9 max 133,625.4 m, median at F∪'s peak 14,750.4 m; L10 max 132,126.0 m, median at F∪'s peak 7,199.9 m; L11 max 131,297.5 m, median at F∪'s peak —; L12 max 130,895.6 m, median at F∪'s peak —; L13 max 130,741.0 m, median at F∪'s peak —; L14 max 130,680.0 m, median at F∪'s peak —; L15 max 130,653.8 m, median at F∪'s peak —; L16 max 130,640.7 m, median at F∪'s peak —; L17 max 130,634.1 m, median at F∪'s peak —.
- **k_n = ε_n P₀ ÷ e_n**, 1 level table met; the last (from 238.1 s): L0 49.42, L1 80.96, L2 138.01, L3 200.74, L4 282.75, L5 359.13, L6 382.05, L7 451.28, L8 539.52, L9 530.64, L10 464.68, L11 365.54, L12 304.93, L13 280.19, L14 279.03, L15 276.57, L16 272.83, L17 0.00.
- **The counting's cost:** p50 4.60 ms, p95 15.80 ms, max 32.00 ms a sample.

| Segment             | Frames | Drawn, mean / max | Standing in | Missing | Stand-in share   | Limited frames | Made resident / s |
| ------------------- | ------ | ----------------- | ----------- | ------- | ---------------- | -------------- | ----------------- |
| descent arc         | 3,233  | 121.8 / 213       | 0.3 / 16    | 0.0 / 2 | 2.2 % / 100.0 %  | 2,120          | 0.8               |
| approach and flare  | 574    | 175.8 / 374       | 15.9 / 375  | 0.0 / 0 | 5.4 % / 87.6 %   | 574            | 6.7               |
| low fast pass       | 81     | 255.0 / 375       | 329.9 / 382 | 0.0 / 0 | 87.8 % / 91.3 %  | 81             | 19.4              |
| slowdown            | 1,894  | 6.3 / 375         | 8.9 / 362   | 0.0 / 2 | 87.9 % / 90.7 %  | 50             | 4.4               |
| vertical descent    | 45     | 0.6 / 2           | 115.3 / 443 | 0.0 / 2 | 83.8 % / 100.0 % | 0              | 11.2              |
| hover and touchdown | 1,055  | 0.0 / 4           | 4.1 / 341   | 0.0 / 2 | 72.1 % / 78.0 %  | 0              | 0.1               |
| after the script    | 148    | 16.0 / 272        | 85.6 / 437  | 0.0 / 2 | 36.0 % / 100.0 % | 10             | —                 |

The pass's own counts at the samples, the largest in each segment:

| Segment             | Samples | Selected | Limited samples | Selected resident | Slots used | Pinned forced | Pinned drawn |
| ------------------- | ------- | -------- | --------------- | ----------------- | ---------- | ------------- | ------------ |
| descent arc         | 647     | 187      | 440             | 187               | 375        | 0             | 316          |
| approach and flare  | 78      | 400      | 78              | 180               | 375        | 108           | 274          |
| low fast pass       | 27      | 415      | 27              | 55                | 375        | 104           | 112          |
| slowdown            | 15      | 400      | 15              | 41                | 375        | 100           | 88           |
| vertical descent    | 3       | 443      | 0               | 77                | 79         | 98            | 2            |
| hover and touchdown | 2       | 437      | 0               | 99                | 105        | 99            | 6            |
| after the script    | 1       | 2        | 0               | 0                 | 0          | 0             | 0            |

### The wireframe cache: instrument-1 (wireframe, weight 0.25)

164 samples; attached to 1 pass; 520 slots of 38,028 B.

| Count                     | instrument-1                                    | union                                                      |
| ------------------------- | ----------------------------------------------- | ---------------------------------------------------------- |
| F                         | 433 (1,075.3 s, approach and flare, 2,463 m up) | 433 (1,075.3 s, approach and flare, 2,463 m up)            |
| F₀ (margin 0)             | 430 (1,075.3 s, approach and flare, 2,463 m up) | 430 (1,075.3 s, approach and flare, 2,463 m up)            |
| AR (all round)            | 1,008 (1,186.2 s, hover and touchdown, 2 m up)  | 1,008 (1,186.2 s, hover and touchdown, 2 m up)             |
| F′ (diagnostic occluder)  | 433 (1,075.3 s, approach and flare, 2,463 m up) | 433 (1,075.3 s, approach and flare, 2,463 m up)            |
| AR′ (diagnostic occluder) | 1,008 (1,186.2 s, hover and touchdown, 2 m up)  | 1,008 (1,186.2 s, hover and touchdown, 2 m up)             |
| Forced, once              |                                                 | 106, 106 seen (1,056.1 s, approach and flare, 12,841 m up) |

| Form                                 | Slots   | Bytes        | Slots, diagnostic | Bytes, diagnostic | Against the as-built 520 slots |
| ------------------------------------ | ------- | ------------ | ----------------- | ----------------- | ------------------------------ |
| Ruled: Σ max(1.3 AR, 3 F) + forced   | 1,416.4 | 53,885,676 B | 1,416.4           | 53,885,676 B      | × 2.72                         |
| (a) its largest same-sample value    | 1,409.4 | 53,619,480 B | 1,409.4           | 53,619,480 B      | × 2.71                         |
| (b) max(1.3 AR∪, 3 F∪) + forced      | 1,416.4 | 53,885,676 B | 1,416.4           | 53,885,676 B      | × 2.72                         |
| (c) the frustum rule, Σ 3 F + forced | 1,405.0 | 53,429,340 B | 1,405.0           | 53,429,340 B      | × 2.70                         |

- **A/B:** the diagnostic occluder at its deepest -9,538.1 m, against the as-built -66,604.0 m; the price in slots, each a share of the sized all-round peak Σ 1.3 AR (1,310.4 slots): the rule's own, rule − rule′, 0.0 slots, 0.0 %, the reading the 10 % test takes; the union's all-round term 1.3 (AR − AR′) 0.0 (0.0 %), its frustum term 3 (F − F′) 0.0 (0.0 %).
- **The all-round peak's residency:** the resident share's peak 9.6 % (1,173.4 s, vertical descent, 129 m up), the swept share's 100.0 % (1,050.2 s, approach and flare, 16,137 m up), the cap at AR∪'s peak × 0.52; resident at 100 % never. The descent never turns, its views look along the track and VIEW's LOOK turns are not flown: a "never" is the descent's.
- **The forced region:** first selected at 1,052.6 s; every forced patch resident first at 1,168.0 s, not at every sample after; touchdown at 1,199.9 s (99 forced, 99 of them resident); a lead of 31.9 s (touchdown − every forced patch resident); the contact by the session's rule at — (none).
- **Box heights by level** (the frustum count's seen leaves): L0 max 867,735.0 m, median at F∪'s peak —; L4 max 168,335.4 m, median at F∪'s peak 168,223.8 m; L5 max 156,741.9 m, median at F∪'s peak 156,698.4 m; L6 max 147,053.3 m, median at F∪'s peak 147,043.4 m; L7 max 139,255.1 m, median at F∪'s peak 139,253.4 m; L8 max 135,673.8 m, median at F∪'s peak 135,673.5 m; L9 max 133,625.4 m, median at F∪'s peak 133,625.4 m; L10 max 132,126.0 m, median at F∪'s peak 132,126.0 m; L11 max 131,297.5 m, median at F∪'s peak 131,297.4 m; L12 max 130,895.6 m, median at F∪'s peak 130,895.6 m; L13 max 130,741.0 m, median at F∪'s peak 130,741.0 m; L14 max 130,680.0 m, median at F∪'s peak 130,680.0 m; L15 max 130,653.8 m, median at F∪'s peak 130,653.8 m; L16 max 130,640.7 m, median at F∪'s peak 130,640.7 m; L17 max 130,634.1 m, median at F∪'s peak 130,634.1 m.
- **k_n = ε_n P₀ ÷ e_n**, 1 level table met; the last (from 1,050.2 s): L0 49.42, L1 80.96, L2 138.01, L3 200.74, L4 282.75, L5 359.13, L6 382.05, L7 451.28, L8 539.52, L9 530.64, L10 464.68, L11 365.54, L12 304.93, L13 280.19, L14 279.03, L15 276.57, L16 272.83, L17 0.00.
- **The counting's cost:** p50 6.50 ms, p95 17.10 ms, max 45.20 ms a sample.

| Segment             | Frames | Drawn, mean / max | Standing in | Missing     | Stand-in share   | Limited frames | Made resident / s |
| ------------------- | ------ | ----------------- | ----------- | ----------- | ---------------- | -------------- | ----------------- |
| approach and flare  | 124    | 0.0 / 0           | 0.0 / 0     | 407.2 / 535 | 0.0 % / 0.0 %    | 0              | 14.6              |
| low fast pass       | 81     | 0.0 / 0           | 0.0 / 0     | 2.0 / 2     | 0.0 % / 0.0 %    | 0              | 0.0               |
| slowdown            | 1,894  | 27.2 / 475        | 451.6 / 546 | 0.1 / 2     | 96.4 % / 100.0 % | 29             | 18.4              |
| vertical descent    | 45     | 370.4 / 408       | 16.1 / 74   | 0.0 / 0     | 6.9 % / 37.6 %   | 40             | 13.7              |
| hover and touchdown | 1,055  | 78.8 / 404        | 77.4 / 449  | 0.0 / 2     | 17.9 % / 100.0 % | 34             | 7.6               |
| after the script    | 148    | 78.0 / 78         | 0.0 / 0     | 0.0 / 0     | 0.0 % / 0.0 %    | 0              | —                 |

The pass's own counts at the samples, the largest in each segment:

| Segment             | Samples | Selected | Limited samples | Selected resident | Slots used | Pinned forced | Pinned drawn |
| ------------------- | ------- | -------- | --------------- | ----------------- | ---------- | ------------- | ------------ |
| approach and flare  | 30      | 534      | 0               | 9                 | 354        | 106           | 9            |
| low fast pass       | 27      | 2        | 0               | 0                 | 423        | 99            | 6            |
| slowdown            | 53      | 516      | 4               | 170               | 520        | 102           | 154          |
| vertical descent    | 8       | 297      | 7               | 288               | 520        | 99            | 297          |
| hover and touchdown | 45      | 449      | 1               | 134               | 520        | 99            | 183          |
| after the script    | 1       | 114      | 0               | 114               | 339        | 99            | 53           |

### The height workers by segment

3 workers wrapped; the flight's 5,137 bakes of 5,137 posted, 615 never made resident, 4.1 a second.

| Segment             | Bakes / s | Baked | Never resident | Post to reply, p50 / p95 (ms) | Ground speed ÷ height, mean / max (s⁻¹) |
| ------------------- | --------- | ----- | -------------- | ----------------------------- | --------------------------------------- |
| after the script    | 13.8      | 202   | 0              | 171.3 / 271.1                 | 0.000 / 0.000                           |
| descent arc         | 0.8       | 586   | 0              | 82.8 / 108.7                  | 0.029 / 0.998                           |
| approach and flare  | 11.7      | 1,391 | 171            | 139.3 / 199.6                 | 0.806 / 4.707                           |
| low fast pass       | 22.2      | 671   | 104            | 176.8 / 212.6                 | 1.000 / 1.000                           |
| slowdown            | 28.1      | 1,682 | 333            | 114.8 / 220.4                 | 0.408 / 0.794                           |
| vertical descent    | 20.3      | 229   | 0              | 148.4 / 203.6                 | 0.000 / 0.000                           |
| hover and touchdown | 21.3      | 376   | 7              | 73.7 / 502.1                  | 0.000 / 0.000                           |

### Memory: itemized, the engine's tally and the driver

- The lit cache: 375 slots × 178,908 B = 64.0 MiB
- The wireframe cache: 520 slots × 38,028 B = 18.9 MiB
- The session's coarse-field-gpu: 0.2 MiB at its peak (2.0 s)
- The session's other: 0.0 MiB at its peak (238.8 s)
- The cascades: none allocated (the low setting has no shadow cascades: the engine allocated nothing under shadows); T9.a's spec 64.0 MiB
- Itemized GPU (the caches, the session's masks and maps, the cascades): 83.1 MiB; with the workers' field copies (3.3 MiB at their peak): 86.4 MiB, against 1 GB (low); the pool's own copy beside them 0.0 MiB; 3 workers started; 1,082 samples at 1 Hz
- The engine's tally, the app's own GPU figure: 108.5 MiB at its whole peak (1,175.9 s); each category at its own peak: other 2.2 MiB (1,050.0 s), coarse-field-gpu 0.2 MiB (1.0 s), geometry-clusters 14.1 MiB (1,175.9 s), atmosphere-tables 0.0 MiB (238.0 s), render-targets 7.2 MiB (237.8 s), height-cache 85.2 MiB (1,050.0 s); 3 destructions of allocations made before the run's control listened, left out
- The driver: nvidia-smi's peak less its pre-launch baseline — (no nvidia-smi baseline (not NVIDIA)); the GPU process's peak — (nvidia-smi does not list the GPU process)
- At the run's end: the device — (no nvidia-smi reading at the run's last memory sample), the GPU process — (nvidia-smi does not list the GPU process at the run's last memory sample), the rest of the card — (no nvidia-smi reading at the run's last memory sample) (the pre-launch baseline — (no nvidia-smi baseline (not NVIDIA)), a drift of — (no nvidia-smi reading at the run's last memory sample))
- Less the driver: the itemized GPU — (no nvidia-smi baseline (not NVIDIA)), the engine's tally — (no nvidia-smi baseline (not NVIDIA))

### The binding

- maxStorageBufferBindingSize: the adapter's 4,294,967,292 B, requested 1,073,741,824 B, the device's 1,073,741,824 B
- The lit cache's 375 slots of 375 requested, on its own path, no fallback: its ceiling 15,584 slots at the requested binding, 1,948 slots at WebGPU's default 128 MiB
- The wireframe cache's 520 slots of 520 requested, on its own path, no fallback: its ceiling 31,767 slots at the requested binding, 3,970 slots at WebGPU's default 128 MiB

### The readout and the load

- VIEW's terrain readout every 250 ms, not shown: 3,072 publishes, 3,071 holding an answer; 3,025 asked of the workers, 3,025 answered, 0 failed; 0 of its guards' calls to fail the session counted and withheld
- The 1-minute load: 3.77 at 0 s, 8.82 at 60 s, 11.81 at 120 s, 11.01 at 180 s, 6.85 at 240 s, 4.45 at 300 s, 1.68 at 360 s, 1.28 at 420 s, 1.00 at 480 s, 0.51 at 540 s, 0.30 at 600 s, 0.35 at 660 s, 0.13 at 720 s, 0.23 at 780 s, 0.27 at 840 s, 0.19 at 900 s, 0.47 at 960 s, 0.78 at 1,020 s, 1.25 at 1,080 s, 3.20 at 1,140 s, 4.15 at 1,200 s, 3.09 at 1,230 s

## Geometry (plan R14)

The main view's lit pass a frame by segment, median (largest): cluster path on, casters culled by their receivers' depths on, caster stride cap 8, overlay depth cluster, the lit pass's interior patches by clusters, far cascades every 4 frames, the mesh bodies' figures by clusters, the hulls' faces by clusters, the wireframe's interior patches by clusters, the mesh page pool's bytes 32 MiB, meshes cut at 2 px in the camera and 0.5 px as the wireframe's occluders, the lit pass's edge patches by clusters, the camera's strides by measured error, occlusion off, the lit pass's interior patches shaded forward, the lit selection's budget its patches and lit triangles, the stand-ins on their own grids at the camera's stride cap, their masks at stride 1, the overlay's marks the ground hides culled, the edge bakes' stand-ins on the cluster path where exact, the disc cells packed, the mesh bodies' figures shaded forward, the terrain lit by every planetshine source, the detail sets generated, the lit terrain's τ of a CSS pixel.

| Segment             | Interior patches | Edge patches | Edge patches by clusters | Drawn triangles       | Drawn by clusters     | Caster triangles (4 cascades) | Overlay triangles     |
| ------------------- | ---------------- | ------------ | ------------------------ | --------------------- | --------------------- | ----------------------------- | --------------------- |
| orbit coast         | —                | —            | —                        | —                     | —                     | —                             | —                     |
| descent arc         | 81 (110)         | 104 (124)    | 103 (114)                | 1,829,376 (2,107,904) | 55,406 (313,050)      | —                             | 1,121,824 (1,335,808) |
| approach and flare  | 138 (358)        | 36 (55)      | 36 (53)                  | 1,608,704 (3,305,984) | 50,275 (1,911,640)    | —                             | 902,896 (1,929,856)   |
| low fast pass       | 336 (361)        | 14 (15)      | 14 (15)                  | 3,075,072 (3,294,720) | 2,634,344 (2,850,272) | —                             | 2,689,328 (2,913,696) |
| slowdown            | 340 (362)        | 14 (14)      | 14 (14)                  | 3,109,888 (3,290,624) | 2,713,889 (2,944,924) | —                             | 2,800,864 (2,959,536) |
| vertical descent    | 0 (0)            | 2 (2)        | 0 (0)                    | 21,504 (21,504)       | 1,344 (1,344)         | —                             | 1,600 (1,600)         |
| hover and touchdown | 0 (0)            | 2 (4)        | 0 (0)                    | 21,504 (43,008)       | 1,344 (1,344)         | —                             | 21,504 (43,008)       |
