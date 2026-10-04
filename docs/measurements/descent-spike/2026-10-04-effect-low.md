# Descent spike: effect, low, 2026-10-04

- **Overall:** not-measured (provisional: not a quiet machine)
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 10.40, 10.43, 10.63
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, window hidden, canvas 806 × 431 px
- **T:** — (no window shown); warm-up 10 s
- **Trace:** — (the trace has no timed event)
- **Options:** `--setting low` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--terrainVertexPath face-differences` `--terrainNormals mesh`

## The criterion (Design note 21)

| Row                                               | Limit    | Value    | Verdict      | Note            |
| ------------------------------------------------- | -------- | -------- | ------------ | --------------- |
| 50th percentile ≤ T + 0.5 ms                      | —        | 16.70 ms | not-measured | no window shown |
| 95th percentile ≤ 35 ms                           | —        | 33.30 ms | not-measured | no window shown |
| 99th percentile ≤ 2T                              | —        | 50.10 ms | not-measured | no window shown |
| ≤ 1% of intervals above 1.5 T                     | 1 %      | —        | not-measured | no window shown |
| none above 3 T                                    | 0        | —        | not-measured | no window shown |
| main thread ≤ 0.8 T at the 95th percentile        | —        | 7.10 ms  | not-measured | no window shown |
| GPU pass sum ≤ 0.8 T at the 95th percentile       | —        | 4.26 ms  | not-measured | no window shown |
| terrain GPU time ≤ 14 ms at the 95th percentile   | 14.00 ms | 0.57 ms  | pass         |                 |
| atmosphere GPU time ≤ 4 ms at the 95th percentile | 4.00 ms  | 3.73 ms  | pass         |                 |
| GPU resident ≤ 1 GB                               | 1.0 GB   | 0.252 GB | pass         |                 |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 65518 intervals, p50 16.70 ms, p95 33.30 ms, p99 50.10 ms, max 133.30 ms.

| Segment             | Intervals                                                                | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | ------------------------------------------------------------------------ | -------------------------------------------------------------------- |
| orbit coast         | 2869 intervals, p50 16.70 ms, p95 16.80 ms, p99 49.90 ms, max 100.00 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 47609 intervals, p50 16.70 ms, p95 33.30 ms, p99 66.60 ms, max 133.30 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 6539 intervals, p50 16.70 ms, p95 33.30 ms, p99 50.00 ms, max 100.00 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 1682 intervals, p50 16.70 ms, p95 16.80 ms, p99 50.00 ms, max 66.70 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 3352 intervals, p50 16.70 ms, p95 33.20 ms, p99 50.00 ms, max 100.00 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 579 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 50.10 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 2887 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 66.60 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 1.6       | 1.6   | 1.6      | 0.7             | 0.0                      | 39            | 19                     | 1.1           |
| descent arc         | 0.8       | 0.8   | 0.8      | 2.1             | 0.2                      | 50            | 25                     | 14.8          |
| approach and flare  | 14.4      | 13.2  | 13.2     | 29.0            | 4.4                      | 87            | 35                     | 35.7          |
| low fast pass       | 43.0      | 41.0  | 41.0     | 57.4            | 11.7                     | 82            | 36                     | 22.7          |
| slowdown            | 48.5      | 40.6  | 40.6     | 34.8            | 7.1                      | 88            | 76                     | 45.6          |
| vertical descent    | 0.0       | 0.0   | 0.0      | 21.4            | 11.8                     | 77            | 79                     | 0.0           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 0.0             | 0.0                      | 67            | 69                     | 0.0           |

## Memory (peaks)

- GPU headline: 240 MiB from nvidia-smi
- App processes: 3800 MiB; tracing service (the measurement's own): 1632 MiB
- Renderer private: 1494 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 240 MiB; the GPU process alone: 192 MiB
- Adapter tally: 75 MiB

Uploads 2998 MiB; 0 pipelines created after warm-up; 0 untimed passes.
