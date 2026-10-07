# Descent spike: effect, low, 2026-10-06

**PROFILED: diagnostic, not judged; renderer and app memory include the CPU profiler's samples.**

- **Overall:** not-measured
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 0.32, 1.41, 2.14
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, window hidden, canvas 806 × 431 px
- **T:** — (no window shown); warm-up 10 s
- **Trace:** 9 perfetto-proto windows, 1168.7 s traced after the warm-up, profiled; 8 boundaries left out 3022 frames (largest stall 66.80 ms); largest file 480 MiB, buffer use up to 26 %
- **Incomplete pass times:** — (not recorded before results version 5)
- **GPU clocks:** — (not recorded before results version 5)
- **Options:** `--setting low` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile on` `--terrainVertexPath face-differences` `--terrainNormals mesh`

## The criterion (Design note 21)

| Row                                                                                        | Limit    | Value    | Verdict      | Note                                                                                                              |
| ------------------------------------------------------------------------------------------ | -------- | -------- | ------------ | ----------------------------------------------------------------------------------------------------------------- |
| 50th percentile ≤ T + 0.5 ms                                                               | —        | 16.70 ms | not-measured | no window shown                                                                                                   |
| 95th percentile ≤ 35 ms                                                                    | —        | 16.80 ms | not-measured | no window shown                                                                                                   |
| 99th percentile ≤ 2T                                                                       | —        | 33.30 ms | not-measured | no window shown                                                                                                   |
| ≤ 1% of intervals above 1.5 T                                                              | 1 %      | —        | not-measured | no window shown                                                                                                   |
| none above 3 T                                                                             | 0        | —        | not-measured | no window shown                                                                                                   |
| main thread ≤ 0.8 T at the 95th percentile                                                 | —        | 2.50 ms  | not-measured | no window shown                                                                                                   |
| GPU pass sum ≤ 0.8 T at the 95th percentile                                                | —        | 4.08 ms  | not-measured | no window shown                                                                                                   |
| terrain and atmosphere GPU time, summed per frame, ≤ 18 ms (14 + 4) at the 95th percentile | 18.00 ms | —        | not-measured | not recorded before results version 6: the file keeps each row's 95th percentile, not each frame's sum of the two |
| GPU resident ≤ 1 GB                                                                        | 1.0 GB   | 0.254 GB | pass         |                                                                                                                   |

## Terrain and atmosphere against their estimates

Each pass row's sum in each complete frame, beside the brainstorm's estimate: findings, which no verdict reads. The criterion judges the two together.

| Row        | Estimate | p50     | p95     | p99     | Finding                                                                                           |
| ---------- | -------- | ------- | ------- | ------- | ------------------------------------------------------------------------------------------------- |
| terrain    | 14.00 ms | 0.33 ms | 0.67 ms | 0.80 ms |                                                                                                   |
| atmosphere | 4.00 ms  | —       | 3.55 ms | —       | p50, p99: not recorded before results version 6, whose file kept this row's 95th percentile alone |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 69183 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms. Frames left out at the trace's window boundaries: 3022.

| Segment             | Intervals                                                               | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 2954 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.00 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 50370 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 7054 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.00 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 1759 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 3518 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 586 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.40 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 2941 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.50 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 1.6       | 1.6   | 1.6      | 0.7             | 0.0                      | 39            | 19                     | 0.7           |
| descent arc         | 0.8       | 0.8   | 0.8      | 2.6             | 0.3                      | 49            | 25                     | 7.1           |
| approach and flare  | 14.3      | 14.0  | 14.0     | 34.8            | 5.4                      | 86            | 35                     | 23.1          |
| low fast pass       | 42.7      | 41.9  | 41.9     | 63.1            | 12.9                     | 82            | 36                     | 15.9          |
| slowdown            | 51.8      | 49.0  | 49.0     | 41.0            | 8.5                      | 88            | 75                     | 36.7          |
| vertical descent    | 0.0       | 0.0   | 0.0      | 25.3            | 13.6                     | 77            | 79                     | 0.0           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 0.0             | 0.0                      | 68            | 70                     | 0.0           |

## Memory (peaks)

- GPU headline: 242 MiB from nvidia-smi
- App processes: 2466 MiB; tracing service (the measurement's own): 469 MiB
- Renderer private: 576 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 242 MiB; the GPU process alone: 194 MiB
- Adapter tally: 75 MiB

Uploads 3324 MiB; 0 pipelines created after warm-up; 0 untimed passes.
