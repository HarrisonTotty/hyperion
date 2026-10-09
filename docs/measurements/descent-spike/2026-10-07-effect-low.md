# Descent spike: effect, low, 2026-10-07

- **Overall:** fail (provisional: not a quiet machine)
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 2.69, 6.67, 12.53
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, window shown, canvas 1509 × 821 px
- **T:** 33.33 ms; warm-up 10 s
- **Trace:** 9 perfetto-proto windows, 1195.3 s traced after the warm-up, unprofiled; 8 boundaries left out 1462 frames (largest stall 33.50 ms); largest file 223 MiB, buffer use up to 24 %
- **Incomplete pass times:** none of 70732 frames after the warm-up
- **GPU clocks:** from nvidia-smi; graphics median 270 MHz (13 %), p5 210, p95 675, after the warm-up, of 2115 MHz; memory median 405 MHz; performance states P5 to P8
- **Options:** `--setting low` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile off` `--terrainVertexPath face-differences` `--terrainNormals mesh`

## The criterion (Design note 21)

| Row                                                                                        | Limit    | Value    | Verdict | Note                                                                    |
| ------------------------------------------------------------------------------------------ | -------- | -------- | ------- | ----------------------------------------------------------------------- |
| 50th percentile ≤ T + 0.5 ms                                                               | 33.83 ms | 16.87 ms | pass    |                                                                         |
| 95th percentile ≤ 35 ms                                                                    | 35.00 ms | 21.62 ms | pass    |                                                                         |
| 99th percentile ≤ 2T                                                                       | 66.67 ms | 47.83 ms | pass    |                                                                         |
| ≤ 1% of intervals above 1.5 T                                                              | 1 %      | 0.60 %   | pass    |                                                                         |
| none above 3 T                                                                             | 0        | 0        | pass    |                                                                         |
| main thread ≤ 0.8 T at the 95th percentile                                                 | 26.67 ms | 2.20 ms  | pass    |                                                                         |
| GPU pass sum ≤ 0.8 T at the 95th percentile                                                | 26.67 ms | 4.27 ms  | pass    | measured at a median 270 of 2115 MHz (the driver's choice at this load) |
| terrain and atmosphere GPU time, summed per frame, ≤ 18 ms (14 + 4) at the 95th percentile | 18.00 ms | 3.98 ms  | pass    | measured at a median 270 of 2115 MHz (the driver's choice at this load) |
| GPU resident ≤ 1 GB                                                                        | 1.0 GB   | 0.276 GB | pass    |                                                                         |

## Terrain and atmosphere against their estimates

Each pass row's sum in each complete frame, beside the brainstorm's estimate: findings, which no verdict reads. The criterion judges the two together.

| Row        | Estimate | p50     | p95     | p99     | Finding |
| ---------- | -------- | ------- | ------- | ------- | ------- |
| terrain    | 14.00 ms | 0.83 ms | 1.45 ms | 1.57 ms |         |
| atmosphere | 4.00 ms  | 1.92 ms | 3.16 ms | 3.47 ms |         |

## Frames by segment

Intervals from presentation times. Whole descent: 70715 intervals, p50 16.87 ms, p95 21.62 ms, p99 47.83 ms, max 83.14 ms. Frames left out at the trace's window boundaries: 1462.

| Segment             | Intervals                                                               | Verdicts (p50, p95, p99, missed, hitches) |
| ------------------- | ----------------------------------------------------------------------- | ----------------------------------------- |
| orbit coast         | 2948 intervals, p50 16.88 ms, p95 23.33 ms, p99 47.64 ms, max 83.14 ms  | pass, pass, pass, pass, pass              |
| descent arc         | 51810 intervals, p50 16.87 ms, p95 21.52 ms, p99 47.67 ms, max 69.52 ms | pass, pass, pass, pass, pass              |
| approach and flare  | 7096 intervals, p50 16.81 ms, p95 21.44 ms, p99 48.14 ms, max 63.09 ms  | pass, pass, pass, pass, pass              |
| low fast pass       | 1764 intervals, p50 16.81 ms, p95 21.42 ms, p99 51.13 ms, max 62.77 ms  | pass, pass, pass, fail, pass              |
| slowdown            | 3535 intervals, p50 16.79 ms, p95 20.36 ms, p99 49.10 ms, max 61.59 ms  | pass, pass, pass, pass, pass              |
| vertical descent    | 597 intervals, p50 16.90 ms, p95 19.74 ms, p99 46.05 ms, max 59.87 ms   | pass, pass, pass, pass, pass              |
| hover and touchdown | 2965 intervals, p50 16.88 ms, p95 22.08 ms, p99 46.92 ms, max 61.97 ms  | pass, pass, pass, pass, pass              |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 2.8       | 2.8   | 2.8      | 0.7             | 0.0                      | 59            | 21                     | 1.3           |
| descent arc         | 2.1       | 2.1   | 2.1      | 2.6             | 0.3                      | 96            | 35                     | 16.5          |
| approach and flare  | 27.2      | 26.6  | 26.6     | 34.8            | 5.4                      | 167           | 51                     | 36.7          |
| low fast pass       | 86.7      | 84.4  | 84.4     | 63.1            | 12.9                     | 146           | 53                     | 25.4          |
| slowdown            | 69.4      | 65.9  | 65.9     | 41.0            | 8.5                      | 119           | 80                     | 45.8          |
| vertical descent    | 0.0       | 0.0   | 0.0      | 25.3            | 13.6                     | 76            | 78                     | 0.0           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 0.0             | 0.0                      | 67            | 69                     | 0.0           |

## Memory (peaks)

- GPU headline: 263 MiB from nvidia-smi
- App processes: 1031 MiB; tracing service (the measurement's own): 244 MiB
- Renderer private: 303 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 263 MiB; the GPU process alone: 188 MiB
- Adapter tally: 91 MiB

Uploads 3746 MiB; 0 pipelines created after warm-up; 0 untimed passes.
