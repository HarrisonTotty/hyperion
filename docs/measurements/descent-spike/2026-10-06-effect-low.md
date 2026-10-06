# Descent spike: effect, low, 2026-10-06

- **Overall:** not-measured
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 0.53, 1.12, 1.80
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, window hidden, canvas 806 × 431 px
- **T:** — (no window shown); warm-up 10 s
- **Trace:** 9 perfetto-proto windows, 1193.7 s traced after the warm-up, unprofiled; 8 boundaries left out 1563 frames (largest stall 33.40 ms); largest file 216 MiB, buffer use up to 23 %
- **Incomplete pass times:** — (not recorded before results version 5)
- **GPU clocks:** — (not recorded before results version 5)
- **Options:** `--setting low` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile off` `--terrainVertexPath face-differences` `--terrainNormals mesh`

## The criterion (Design note 21)

| Row                                               | Limit    | Value    | Verdict      | Note            |
| ------------------------------------------------- | -------- | -------- | ------------ | --------------- |
| 50th percentile ≤ T + 0.5 ms                      | —        | 16.70 ms | not-measured | no window shown |
| 95th percentile ≤ 35 ms                           | —        | 16.80 ms | not-measured | no window shown |
| 99th percentile ≤ 2T                              | —        | 33.30 ms | not-measured | no window shown |
| ≤ 1% of intervals above 1.5 T                     | 1 %      | —        | not-measured | no window shown |
| none above 3 T                                    | 0        | —        | not-measured | no window shown |
| main thread ≤ 0.8 T at the 95th percentile        | —        | 2.10 ms  | not-measured | no window shown |
| GPU pass sum ≤ 0.8 T at the 95th percentile       | —        | 4.11 ms  | not-measured | no window shown |
| terrain GPU time ≤ 14 ms at the 95th percentile   | 14.00 ms | 0.68 ms  | pass         |                 |
| atmosphere GPU time ≤ 4 ms at the 95th percentile | 4.00 ms  | 3.58 ms  | pass         |                 |
| GPU resident ≤ 1 GB                               | 1.0 GB   | 0.256 GB | pass         |                 |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 70819 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms. Frames left out at the trace's window boundaries: 1563.

| Segment             | Intervals                                                               | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 2963 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.40 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 51922 intervals, p50 16.70 ms, p95 16.80 ms, p99 16.80 ms, max 50.10 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 7086 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.50 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 1770 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.40 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 3537 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.50 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 591 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.40 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 2949 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.40 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 1.6       | 1.6   | 1.6      | 0.7             | 0.0                      | 39            | 19                     | 0.7           |
| descent arc         | 0.8       | 0.8   | 0.8      | 2.6             | 0.3                      | 49            | 25                     | 6.9           |
| approach and flare  | 14.3      | 14.1  | 14.1     | 34.8            | 5.4                      | 86            | 35                     | 21.8          |
| low fast pass       | 42.8      | 42.0  | 42.0     | 63.1            | 12.9                     | 82            | 36                     | 15.0          |
| slowdown            | 51.8      | 49.3  | 49.3     | 41.0            | 8.5                      | 88            | 75                     | 35.6          |
| vertical descent    | 0.0       | 0.0   | 0.0      | 25.3            | 13.6                     | 77            | 79                     | 0.0           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 0.0             | 0.0                      | 68            | 70                     | 0.0           |

## Memory (peaks)

- GPU headline: 244 MiB from nvidia-smi
- App processes: 1899 MiB; tracing service (the measurement's own): 238 MiB
- Renderer private: 282 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 244 MiB; the GPU process alone: 196 MiB
- Adapter tally: 75 MiB

Uploads 3283 MiB; 0 pipelines created after warm-up; 0 untimed passes.
