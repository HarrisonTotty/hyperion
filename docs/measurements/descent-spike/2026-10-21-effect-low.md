# Descent spike: effect, low, 2026-10-21

- **Overall:** not-measured (provisional: not a quiet machine)
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 6.48, 5.07, 6.50
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, world terrain-test, style photorealistic, sizes on, window hidden, canvas 806 × 431 px
- **T:** — (no window shown); warm-up 10 s
- **Trace:** 9 perfetto-proto windows, 1174.5 s traced after the warm-up, unprofiled; 8 boundaries left out 2726 frames (largest stall 33.40 ms); largest file 345 MiB, buffer use up to 36 %
- **Incomplete pass times:** none of 69967 frames after the warm-up
- **GPU clocks:** from nvidia-smi; graphics median 1110 MHz (52 %), p5 435, p95 1500, after the warm-up, of 2115 MHz; memory median 5001 MHz; performance states P0 to P8
- **Options:** `--setting low` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile off` `--world terrain-test` `--style photorealistic` `--sizes on` `--terrainVertexPath face-differences` `--terrainNormals mesh`

## The criterion (Design note 21)

| Row                                                                                        | Limit    | Value    | Verdict      | Note                                                                                      |
| ------------------------------------------------------------------------------------------ | -------- | -------- | ------------ | ----------------------------------------------------------------------------------------- |
| 50th percentile ≤ T + 0.5 ms                                                               | —        | 16.70 ms | not-measured | no window shown                                                                           |
| 95th percentile ≤ 35 ms                                                                    | —        | 16.80 ms | not-measured | no window shown                                                                           |
| 99th percentile ≤ 2T                                                                       | —        | 16.80 ms | not-measured | no window shown                                                                           |
| ≤ 1% of intervals above 1.5 T                                                              | 1 %      | —        | not-measured | no window shown                                                                           |
| none above 3 T                                                                             | 0        | —        | not-measured | no window shown                                                                           |
| main thread ≤ 0.8 T at the 95th percentile                                                 | —        | 9.60 ms  | not-measured | no window shown                                                                           |
| GPU pass sum ≤ 0.8 T at the 95th percentile                                                | —        | 6.41 ms  | not-measured | no window shown; measured at a median 1110 of 2115 MHz (the driver's choice at this load) |
| terrain and atmosphere GPU time, summed per frame, ≤ 18 ms (14 + 4) at the 95th percentile | 18.00 ms | —        | not-measured | no timed terrain or atmosphere pass                                                       |
| GPU resident ≤ 1 GB                                                                        | 1.0 GB   | 0.349 GB | pass         |                                                                                           |

## Terrain and atmosphere against their estimates

Each pass row's sum in each complete frame, beside the brainstorm's estimate: findings, which no verdict reads. The criterion judges the two together.

| Row        | Estimate | p50 | p95 | p99 | Finding                  |
| ---------- | -------- | --- | --- | --- | ------------------------ |
| terrain    | 14.00 ms | —   | —   | —   | no timed terrain pass    |
| atmosphere | 4.00 ms  | —   | —   | —   | no timed atmosphere pass |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 69967 intervals, p50 16.70 ms, p95 16.80 ms, p99 16.80 ms, max 1150.00 ms. Frames left out at the trace's window boundaries: 2726.

| Segment             | Intervals                                                                 | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 2998 intervals, p50 16.70 ms, p95 16.80 ms, p99 16.80 ms, max 33.30 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 51102 intervals, p50 16.70 ms, p95 16.70 ms, p99 16.80 ms, max 1150.00 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 7104 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 66.80 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 1744 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 50.10 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 3494 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 66.70 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 582 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 66.60 ms     | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 2942 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.00 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 0.0       | 0.0   | 0.0      | 3204.5          | 3204.5                   | 0             | 0                      | 0.0           |
| descent arc         | 0.7       | 0.9   | 0.9      | 3790.0          | 3790.0                   | 121           | 121                    | 96.7          |
| approach and flare  | 119.8     | 4.0   | 4.0      | 12821.3         | 12821.3                  | 245           | 245                    | 75.9          |
| low fast pass       | 204.1     | 9.7   | 9.7      | 9220.7          | 9220.7                   | 360           | 360                    | 30.0          |
| slowdown            | 101.2     | 15.0  | 15.0     | 4835.6          | 4835.6                   | 365           | 365                    | 60.0          |
| vertical descent    | 4.9       | 5.7   | 5.7      | 1073.1          | 1073.1                   | 173           | 173                    | 7.5           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 1.1             | 1.1                      | 99            | 99                     | 0.0           |

## Memory (peaks)

- GPU headline: 333 MiB from nvidia-smi
- App processes: 2361 MiB; tracing service (the measurement's own): 323 MiB
- Renderer private: 844 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 333 MiB; the GPU process alone: 285 MiB
- Adapter tally: 93 MiB

Uploads 5998 MiB; 3 pipelines created after warm-up; untimed passes — (no pass counted over a generated world).

## Sizes (R10.T14.b)

Counted once a second of script time, by unbudgeted re-selections of each pass's own inputs: F at the pass's margins, F₀ at margin 0, AR all round, and F′ and AR′ with the diagnostic occluder; each view's counts leave the forced region out, which is counted once. The views are found in each selection's input by their streaming weight, then their viewport.

- **Second view:** instrument-1 on spike-craft, wireframe, weight 0.25; viewport 275 × 140 px from 0.0 s, 80 × 140 px from 0.1 s.
- **The header's lowest sampled height:** -9,538.1 m.
- **The lit and wireframe passes' forced sets** (T14.c's finding 4): compared at 14,896 frames, different at 6,517, first at 1,050.0 s, by at most 105 patches.

### The lit cache: spike (photorealistic, weight 1)

1,152 samples; attached to 1 pass; 422 slots of 158,936 B.

| Count                     | spike                                             | union                                                  |
| ------------------------- | ------------------------------------------------- | ------------------------------------------------------ |
| F                         | 963 (527.5 s, descent arc, 155,359 m up)          | 963 (527.5 s, descent arc, 155,359 m up)               |
| F₀ (margin 0)             | 963 (527.5 s, descent arc, 155,359 m up)          | 963 (527.5 s, descent arc, 155,359 m up)               |
| AR (all round)            | 1,483 (1,066.0 s, approach and flare, 7,458 m up) | 1,483 (1,066.0 s, approach and flare, 7,458 m up)      |
| F′ (diagnostic occluder)  | 963 (621.4 s, descent arc, 121,709 m up)          | 963 (621.4 s, descent arc, 121,709 m up)               |
| AR′ (diagnostic occluder) | 1,483 (1,066.0 s, approach and flare, 7,458 m up) | 1,483 (1,066.0 s, approach and flare, 7,458 m up)      |
| Forced, once              |                                                   | 107, 0 seen (993.0 s, approach and flare, 69,875 m up) |

| Form                                 | Slots   | Bytes         | Slots, diagnostic | Bytes, diagnostic | Against the as-built 422 slots |
| ------------------------------------ | ------- | ------------- | ----------------- | ----------------- | ------------------------------ |
| Ruled: Σ max(1.3 AR, 3 F) + forced   | 2,996.0 | 476,172,256 B | 2,996.0           | 476,172,256 B     | × 7.10                         |
| (a) its largest same-sample value    | 2,889.0 | 459,166,104 B | 2,889.0           | 459,166,104 B     | × 6.85                         |
| (b) max(1.3 AR∪, 3 F∪) + forced      | 2,996.0 | 476,172,256 B | 2,996.0           | 476,172,256 B     | × 7.10                         |
| (c) the frustum rule, Σ 3 F + forced | 2,996.0 | 476,172,256 B | 2,996.0           | 476,172,256 B     | × 7.10                         |

- **A/B:** the diagnostic occluder at its deepest -9,538.1 m, against the as-built -66,604.0 m; the price in slots, each a share of the sized all-round peak Σ 1.3 AR (1,927.9 slots): the rule's own, rule − rule′, 0.0 slots, 0.0 %, the reading the 10 % test takes; the union's all-round term 1.3 (AR − AR′) 0.0 (0.0 %), its frustum term 3 (F − F′) 0.0 (0.0 %).
- **The all-round peak's residency:** the resident share's peak 100.0 % (426.4 s, descent arc, 191,595 m up), the swept share's 100.0 % (232.3 s, descent arc, 261,206 m up), the cap at AR∪'s peak × 0.28; resident at 100 % at some sample. The descent never turns, its views look along the track and VIEW's LOOK turns are not flown: a "never" is the descent's.
- **The forced region:** first selected at 990.8 s; every forced patch resident first at 1,169.0 s, not at every sample after; touchdown at 1,199.9 s (99 forced, 99 of them resident); a lead of 30.9 s (touchdown − every forced patch resident); the contact by the session's rule at 990.8 s (105 forced, 0 of them resident).
- **Box heights by level** (the frustum count's seen leaves): L0 max 867,735.0 m, median at F∪'s peak —; L1 max 404,228.6 m, median at F∪'s peak —; L4 max 168,296.7 m, median at F∪'s peak —; L5 max 135,148.2 m, median at F∪'s peak —; L6 max 85,242.2 m, median at F∪'s peak —; L7 max 48,280.1 m, median at F∪'s peak 44,280.9 m; L8 max 29,606.8 m, median at F∪'s peak 25,944.7 m; L9 max 17,421.6 m, median at F∪'s peak 14,821.6 m; L10 max 10,601.8 m, median at F∪'s peak —; L11 max 8,403.8 m, median at F∪'s peak —; L12 max 7,405.0 m, median at F∪'s peak —; L13 max 7,026.2 m, median at F∪'s peak —; L14 max 6,772.3 m, median at F∪'s peak —; L15 max 6,706.8 m, median at F∪'s peak —; L16 max 6,674.1 m, median at F∪'s peak —; L17 max 6,654.7 m, median at F∪'s peak —.
- **k_n = ε_n P₀ ÷ e_n**, 1 level table met; the last (from 0.5 s): L0 49.42, L1 80.96, L2 138.01, L3 200.74, L4 282.75, L5 359.13, L6 382.05, L7 451.28, L8 539.52, L9 530.64, L10 464.68, L11 365.54, L12 304.93, L13 280.19, L14 279.03, L15 276.57, L16 272.83, L17 0.00.
- **The counting's cost:** p50 4.20 ms, p95 11.00 ms, max 26.10 ms a sample.

| Segment             | Frames | Drawn, mean / max | Standing in | Missing | Stand-in share  | Limited frames | Made resident / s |
| ------------------- | ------ | ----------------- | ----------- | ------- | --------------- | -------------- | ----------------- |
| orbit coast         | 3,570  | 0.0 / 0           | 0.0 / 0     | 0.0 / 1 | 0.0 % / 0.0 %   | 0              | 0.0               |
| descent arc         | 53,828 | 120.4 / 211       | 0.3 / 23    | 0.0 / 1 | 1.3 % / 100.0 % | 30,275         | 0.9               |
| approach and flare  | 7,104  | 193.0 / 211       | 25.6 / 349  | 0.0 / 0 | 8.3 % / 84.7 %  | 7,104          | 4.0               |
| low fast pass       | 1,744  | 57.9 / 67         | 309.2 / 382 | 0.0 / 0 | 85.8 % / 90.7 % | 1,744          | 9.7               |
| slowdown            | 3,494  | 32.8 / 48         | 333.5 / 397 | 0.0 / 0 | 91.3 % / 96.1 % | 3,494          | 15.0              |
| vertical descent    | 582    | 21.4 / 57         | 104.3 / 296 | 0.0 / 0 | 41.7 % / 90.8 % | 170            | 5.7               |
| hover and touchdown | 2,942  | 3.0 / 3           | 0.0 / 0     | 0.0 / 0 | 0.0 % / 0.0 %   | 0              | 0.0               |
| after the script    | 4,391  | 3.0 / 3           | 0.0 / 0     | 0.0 / 0 | 0.0 % / 0.0 %   | 0              | —                 |

The pass's own counts at the samples, the largest in each segment:

| Segment             | Samples | Selected | Limited samples | Selected resident | Slots used | Pinned forced | Pinned drawn |
| ------------------- | ------- | -------- | --------------- | ----------------- | ---------- | ------------- | ------------ |
| orbit coast         | 60      | 0        | 0               | 0                 | 1          | 0             | 1            |
| descent arc         | 857     | 211      | 464             | 211               | 422        | 0             | 331          |
| approach and flare  | 84      | 381      | 84              | 211               | 422        | 108           | 316          |
| low fast pass       | 30      | 397      | 30              | 65                | 422        | 102           | 115          |
| slowdown            | 60      | 412      | 60              | 118               | 422        | 101           | 83           |
| vertical descent    | 10      | 326      | 3               | 122               | 422        | 99            | 54           |
| hover and touchdown | 50      | 99       | 0               | 99                | 422        | 99            | 23           |
| after the script    | 1       | 99       | 0               | 99                | 422        | 99            | 23           |

### The wireframe cache: instrument-1 (wireframe, weight 0.25)

182 samples; attached to 1 pass; 520 slots of 38,028 B.

| Count                     | instrument-1                                    | union                                                     |
| ------------------------- | ----------------------------------------------- | --------------------------------------------------------- |
| F                         | 421 (1,066.0 s, approach and flare, 7,458 m up) | 421 (1,066.0 s, approach and flare, 7,458 m up)           |
| F₀ (margin 0)             | 418 (1,066.0 s, approach and flare, 7,458 m up) | 418 (1,066.0 s, approach and flare, 7,458 m up)           |
| AR (all round)            | 709 (1,180.0 s, hover and touchdown, 2 m up)    | 709 (1,180.0 s, hover and touchdown, 2 m up)              |
| F′ (diagnostic occluder)  | 421 (1,066.0 s, approach and flare, 7,458 m up) | 421 (1,066.0 s, approach and flare, 7,458 m up)           |
| AR′ (diagnostic occluder) | 709 (1,180.0 s, hover and touchdown, 2 m up)    | 709 (1,180.0 s, hover and touchdown, 2 m up)              |
| Forced, once              |                                                 | 106, 106 seen (1,070.0 s, approach and flare, 5,293 m up) |

| Form                                 | Slots   | Bytes        | Slots, diagnostic | Bytes, diagnostic | Against the as-built 520 slots |
| ------------------------------------ | ------- | ------------ | ----------------- | ----------------- | ------------------------------ |
| Ruled: Σ max(1.3 AR, 3 F) + forced   | 1,369.0 | 52,060,332 B | 1,369.0           | 52,060,332 B      | × 2.63                         |
| (a) its largest same-sample value    | 1,362.0 | 51,794,136 B | 1,362.0           | 51,794,136 B      | × 2.62                         |
| (b) max(1.3 AR∪, 3 F∪) + forced      | 1,369.0 | 52,060,332 B | 1,369.0           | 52,060,332 B      | × 2.63                         |
| (c) the frustum rule, Σ 3 F + forced | 1,369.0 | 52,060,332 B | 1,369.0           | 52,060,332 B      | × 2.63                         |

- **A/B:** the diagnostic occluder at its deepest -9,538.1 m, against the as-built -66,604.0 m; the price in slots, each a share of the sized all-round peak Σ 1.3 AR (921.7 slots): the rule's own, rule − rule′, 0.0 slots, 0.0 %, the reading the 10 % test takes; the union's all-round term 1.3 (AR − AR′) 0.0 (0.0 %), its frustum term 3 (F − F′) 0.0 (0.0 %).
- **The all-round peak's residency:** the resident share's peak 2.7 % (1,181.0 s, hover and touchdown, 2 m up), the swept share's 100.0 % (1,050.0 s, approach and flare, 16,376 m up), the cap at AR∪'s peak × 0.73; resident at 100 % never. The descent never turns, its views look along the track and VIEW's LOOK turns are not flown: a "never" is the descent's.
- **The forced region:** first selected at 1,052.5 s; every forced patch resident first at 1,173.0 s, and at every sample after; touchdown at 1,199.9 s (99 forced, 99 of them resident); a lead of 26.9 s (touchdown − every forced patch resident); the contact by the session's rule at — (none).
- **Box heights by level** (the frustum count's seen leaves): L0 max 867,735.0 m, median at F∪'s peak —; L4 max 168,335.4 m, median at F∪'s peak 168,223.8 m; L5 max 156,741.9 m, median at F∪'s peak 156,698.4 m; L6 max 147,053.3 m, median at F∪'s peak 147,043.4 m; L7 max 139,255.1 m, median at F∪'s peak 139,253.4 m; L8 max 135,673.8 m, median at F∪'s peak 135,673.5 m; L9 max 133,625.4 m, median at F∪'s peak 133,625.4 m; L10 max 132,126.0 m, median at F∪'s peak 132,125.9 m; L11 max 131,297.5 m, median at F∪'s peak 131,297.4 m; L12 max 130,895.6 m, median at F∪'s peak 130,895.6 m; L13 max 130,741.0 m, median at F∪'s peak 130,741.0 m; L14 max 130,680.0 m, median at F∪'s peak 130,680.0 m; L15 max 130,653.8 m, median at F∪'s peak 130,653.8 m; L16 max 130,640.7 m, median at F∪'s peak 130,640.7 m; L17 max 130,634.1 m, median at F∪'s peak 130,634.1 m.
- **k_n = ε_n P₀ ÷ e_n**, 1 level table met; the last (from 1,050.0 s): L0 49.42, L1 80.96, L2 138.01, L3 200.74, L4 282.75, L5 359.13, L6 382.05, L7 451.28, L8 539.52, L9 530.64, L10 464.68, L11 365.54, L12 304.93, L13 280.19, L14 279.03, L15 276.57, L16 272.83, L17 0.00.
- **The counting's cost:** p50 0.30 ms, p95 5.50 ms, max 8.60 ms a sample.

| Segment             | Frames | Drawn, mean / max | Standing in | Missing     | Stand-in share   | Limited frames | Made resident / s |
| ------------------- | ------ | ----------------- | ----------- | ----------- | ---------------- | -------------- | ----------------- |
| approach and flare  | 1,743  | 0.0 / 0           | 0.0 / 0     | 432.5 / 549 | 0.0 % / 0.0 %    | 0              | 0.0               |
| low fast pass       | 1,744  | 0.0 / 0           | 0.0 / 0     | 2.0 / 2     | 0.0 % / 0.0 %    | 0              | 0.0               |
| slowdown            | 3,494  | 0.1 / 2           | 12.7 / 470  | 1.9 / 19    | 2.8 % / 100.0 %  | 0              | 0.6               |
| vertical descent    | 582    | 35.2 / 102        | 298.5 / 451 | 0.0 / 0     | 85.0 % / 100.0 % | 209            | 30.1              |
| hover and touchdown | 2,942  | 77.3 / 78         | 0.1 / 14    | 0.0 / 0     | 0.1 % / 11.9 %   | 0              | 0.3               |
| after the script    | 4,391  | 77.0 / 77         | 0.0 / 0     | 0.0 / 0     | 0.0 % / 0.0 %    | 0              | —                 |

The pass's own counts at the samples, the largest in each segment:

| Segment             | Samples | Selected | Limited samples | Selected resident | Slots used | Pinned forced | Pinned drawn |
| ------------------- | ------- | -------- | --------------- | ----------------- | ---------- | ------------- | ------------ |
| approach and flare  | 31      | 520      | 0               | 0                 | 0          | 105           | 0            |
| low fast pass       | 30      | 2        | 0               | 0                 | 0          | 0             | 0            |
| slowdown            | 60      | 449      | 0               | 7                 | 9          | 100           | 2            |
| vertical descent    | 10      | 449      | 4               | 128               | 295        | 99            | 101          |
| hover and touchdown | 50      | 118      | 0               | 114               | 347        | 99            | 57           |
| after the script    | 1       | 113      | 0               | 113               | 347        | 99            | 56           |

### The height workers by segment

3 workers wrapped; the flight's 3,576 bakes of 3,576 posted, 726 never made resident, 3.0 a second.

| Segment             | Bakes / s | Baked | Never resident | Post to reply, p50 / p95 (ms) | Ground speed ÷ height, mean / max (s⁻¹) |
| ------------------- | --------- | ----- | -------------- | ----------------------------- | --------------------------------------- |
| orbit coast         | 29.2      | 1     | 0              | 31.8 / 31.8                   | 0.024 / 0.024                           |
| descent arc         | 0.9       | 776   | 2              | 149.2 / 283.5                 | 0.040 / 1.000                           |
| approach and flare  | 8.6       | 1,014 | 533            | 398.8 / 491.4                 | 0.953 / 4.708                           |
| low fast pass       | 14.5      | 434   | 143            | 409.7 / 462.9                 | 1.000 / 1.000                           |
| slowdown            | 16.3      | 976   | 43             | 377.8 / 420.9                 | 0.507 / 0.805                           |
| vertical descent    | 36.3      | 362   | 5              | 165.0 / 264.3                 | 0.000 / 0.000                           |
| hover and touchdown | 29.9      | 13    | 0              | 199.7 / 218.2                 | 0.000 / 0.000                           |

### Memory: itemized, the engine's tally and the driver

- The lit cache: 422 slots × 158,936 B = 64.0 MiB
- The wireframe cache: 520 slots × 38,028 B = 18.9 MiB
- The session's coarse-field-gpu: 0.1 MiB at its peak (0.0 s)
- The session's other: 0.0 MiB at its peak (143.4 s)
- The cascades: none allocated (the low setting has no shadow cascades: the engine allocated nothing under shadows); T9.a's spec 64.0 MiB
- Itemized GPU (the caches, the session's masks and maps, the cascades): 82.9 MiB; with the workers' field copies (3.3 MiB at their peak): 86.3 MiB, against 1 GB (low); the pool's own copy beside them 0.0 MiB; 3 workers started; 1,291 samples at 1 Hz
- The engine's tally, the app's own GPU figure: 92.7 MiB at its whole peak (1,050.0 s); each category at its own peak: other 1.3 MiB (1,050.0 s), coarse-field-gpu 0.1 MiB (before the flight), atmosphere-tables 0.0 MiB (0.4 s), render-targets 7.2 MiB (0.2 s), height-cache 84.1 MiB (1,050.0 s)
- The driver: nvidia-smi's peak less its pre-launch baseline 333.0 MiB; the GPU process's peak 285.0 MiB
- At the run's end: the device 468.0 MiB, the GPU process 284.0 MiB, the rest of the card 184.0 MiB (the pre-launch baseline 136.0 MiB, a drift of 48.0 MiB)
- Less the driver: the itemized GPU -250.1 MiB, the engine's tally -240.3 MiB

### The binding

- maxStorageBufferBindingSize: the adapter's 2,147,483,644 B, requested 1,073,741,824 B, the device's 1,073,741,824 B
- The lit cache's 422 slots: its ceiling 15,584 slots at the requested binding, 1,948 slots at WebGPU's default 128 MiB
- The wireframe cache's 520 slots: its ceiling 31,767 slots at the requested binding, 3,970 slots at WebGPU's default 128 MiB

### The readout and the load

- VIEW's terrain readout every 250 ms, not shown: 5,110 publishes, 5,109 holding an answer; 4,860 asked of the workers, 4,860 answered, 0 failed; 0 of its guards' calls to fail the session counted and withheld
- The 1-minute load: 6.48 at 0 s, 5.33 at 60 s, 3.37 at 120 s, 2.78 at 180 s, 4.60 at 240 s, 4.06 at 300 s, 3.16 at 360 s, 3.12 at 420 s, 3.14 at 480 s, 3.41 at 540 s, 3.23 at 600 s, 2.42 at 660 s, 3.10 at 720 s, 3.11 at 780 s, 6.06 at 840 s, 5.49 at 900 s, 5.90 at 960 s, 6.51 at 1,020 s, 7.88 at 1,080 s, 7.26 at 1,140 s, 5.47 at 1,200 s, 3.63 at 1,230 s
