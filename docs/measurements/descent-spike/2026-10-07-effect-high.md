# Descent spike: effect, high, 2026-10-07

- **Overall:** fail
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 0.17, 0.46, 2.74
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, window shown, canvas 1509 × 821 px
- **T:** 16.67 ms; warm-up 10 s
- **Trace:** 9 perfetto-proto windows, 1194.7 s traced after the warm-up, unprofiled; 8 boundaries left out 1517 frames (largest stall 16.80 ms); largest file 224 MiB, buffer use up to 24 %
- **Incomplete pass times:** none of 71485 frames after the warm-up
- **GPU clocks:** from nvidia-smi; graphics median 975 MHz (46 %), p5 225, p95 1185, after the warm-up, of 2115 MHz; memory median 810 MHz; performance states P3 to P8
- **Options:** `--setting high` `--seed 7` `--smoke false` `--workers 3` `--ridged off` `--dawnSafety on` `--traceProfile off` `--terrainVertexPath baked-offsets` `--terrainNormals double`

## The criterion (Design note 21)

| Row                                                                                      | Limit    | Value    | Verdict | Note                                                                    |
| ---------------------------------------------------------------------------------------- | -------- | -------- | ------- | ----------------------------------------------------------------------- |
| 50th percentile ≤ T + 0.5 ms                                                             | 17.17 ms | 16.87 ms | pass    |                                                                         |
| 95th percentile ≤ T + 1 ms                                                               | 17.67 ms | 18.29 ms | fail    |                                                                         |
| 99th percentile ≤ 2T                                                                     | 33.33 ms | 39.84 ms | fail    |                                                                         |
| ≤ 1% of intervals above 1.5 T                                                            | 1 %      | 3.06 %   | fail    |                                                                         |
| none above 3 T                                                                           | 0        | 83       | fail    |                                                                         |
| main thread ≤ 0.8 T at the 95th percentile                                               | 13.33 ms | 3.60 ms  | pass    |                                                                         |
| GPU pass sum ≤ 0.8 T at the 95th percentile                                              | 13.33 ms | 5.72 ms  | pass    | measured at a median 975 of 2115 MHz (the driver's choice at this load) |
| terrain and atmosphere GPU time, summed per frame, ≤ 6 ms (5 + 1) at the 95th percentile | 6.00 ms  | 5.59 ms  | pass    | measured at a median 975 of 2115 MHz (the driver's choice at this load) |
| GPU resident ≤ 3 GB                                                                      | 3.0 GB   | 0.636 GB | pass    |                                                                         |

## Terrain and atmosphere against their estimates

Each pass row's sum in each complete frame, beside the brainstorm's estimate: findings, which no verdict reads. The criterion judges the two together.

| Row        | Estimate | p50     | p95     | p99     | Finding                                      |
| ---------- | -------- | ------- | ------- | ------- | -------------------------------------------- |
| terrain    | 5.00 ms  | 1.16 ms | 2.15 ms | 2.38 ms |                                              |
| atmosphere | 1.00 ms  | 3.19 ms | 4.07 ms | 4.28 ms | over its estimate: a finding for T19 and R12 |

## Frames by segment

Intervals from presentation times. Whole descent: 71470 intervals, p50 16.87 ms, p95 18.29 ms, p99 39.84 ms, max 84.55 ms. Frames left out at the trace's window boundaries: 1517.

| Segment             | Intervals                                                               | Verdicts (p50, p95, p99, missed, hitches) |
| ------------------- | ----------------------------------------------------------------------- | ----------------------------------------- |
| orbit coast         | 2999 intervals, p50 16.87 ms, p95 18.23 ms, p99 38.92 ms, max 44.62 ms  | pass, fail, fail, fail, pass              |
| descent arc         | 52464 intervals, p50 16.87 ms, p95 18.05 ms, p99 37.92 ms, max 64.20 ms | pass, fail, fail, fail, fail              |
| approach and flare  | 7158 intervals, p50 16.80 ms, p95 18.81 ms, p99 43.58 ms, max 60.93 ms  | pass, fail, fail, fail, fail              |
| low fast pass       | 1773 intervals, p50 16.81 ms, p95 20.36 ms, p99 47.64 ms, max 66.56 ms  | pass, fail, fail, fail, fail              |
| slowdown            | 3522 intervals, p50 16.92 ms, p95 21.45 ms, p99 49.91 ms, max 84.55 ms  | pass, fail, fail, fail, fail              |
| vertical descent    | 595 intervals, p50 16.88 ms, p95 21.67 ms, p99 43.46 ms, max 54.32 ms   | pass, fail, fail, fail, fail              |
| hover and touchdown | 2959 intervals, p50 16.88 ms, p95 21.74 ms, p99 47.41 ms, max 75.55 ms  | pass, fail, fail, fail, fail              |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 16.3      | 16.0  | 16.0     | 14.5            | 0.9                      | 323           | 53                     | 13.8          |
| descent arc         | 11.3      | 11.2  | 11.2     | 32.3            | 4.4                      | 471           | 87                     | 167.8         |
| approach and flare  | 89.5      | 45.3  | 45.3     | 258.1           | 46.7                     | 568           | 137                    | 108.9         |
| low fast pass       | 135.3     | 42.3  | 42.3     | 341.3           | 144.0                    | 261           | 131                    | 30.0          |
| slowdown            | 83.1      | 40.5  | 40.5     | 213.5           | 81.4                     | 156           | 113                    | 56.8          |
| vertical descent    | 1.1       | 1.6   | 1.6      | 83.0            | 83.0                     | 101           | 101                    | 0.5           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 0.1             | 0.1                      | 99            | 99                     | 0.0           |

## Memory (peaks)

- GPU headline: 607 MiB from nvidia-smi
- App processes: 1131 MiB; tracing service (the measurement's own): 243 MiB
- Renderer private: 415 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 607 MiB; the GPU process alone: 531 MiB
- Adapter tally: 431 MiB

Uploads 7827 MiB; 0 pipelines created after warm-up; 0 untimed passes.
