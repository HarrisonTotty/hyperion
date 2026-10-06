# Descent spike: effect, high, 2026-10-06

- **Overall:** not-measured
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 0.46, 1.33, 1.64
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, window hidden, canvas 1398 × 793 px
- **T:** — (no window shown); warm-up 10 s
- **Trace:** 9 perfetto-proto windows, 1198.7 s traced after the warm-up, unprofiled; 8 boundaries left out 743 frames (largest stall 183.40 ms); largest file 224 MiB, buffer use up to 24 %
- **Incomplete pass times:** — (not recorded before results version 5)
- **GPU clocks:** — (not recorded before results version 5)
- **Options:** `--setting high` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--traceProfile off` `--terrainVertexPath baked-offsets` `--terrainNormals double`

## The criterion (Design note 21)

| Row                                               | Limit   | Value    | Verdict      | Note                     |
| ------------------------------------------------- | ------- | -------- | ------------ | ------------------------ |
| 50th percentile ≤ T + 0.5 ms                      | —       | 16.70 ms | not-measured | no window shown          |
| 95th percentile ≤ T + 1 ms                        | —       | 83.30 ms | not-measured | no window shown          |
| 99th percentile ≤ 2T                              | —       | 83.40 ms | not-measured | no window shown          |
| ≤ 1% of intervals above 1.5 T                     | 1 %     | —        | not-measured | no window shown          |
| none above 3 T                                    | 0       | —        | not-measured | no window shown          |
| main thread ≤ 0.8 T at the 95th percentile        | —       | 4.60 ms  | not-measured | no window shown          |
| GPU pass sum ≤ 0.8 T at the 95th percentile       | —       | —        | not-measured | no window shown          |
| terrain GPU time ≤ 5 ms at the 95th percentile    | 5.00 ms | —        | not-measured | no timed terrain pass    |
| atmosphere GPU time ≤ 1 ms at the 95th percentile | 1.00 ms | —        | not-measured | no timed atmosphere pass |
| GPU resident ≤ 3 GB                               | 3.0 GB  | 0.719 GB | pass         |                          |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 53909 intervals, p50 16.70 ms, p95 83.30 ms, p99 83.40 ms, max 183.40 ms. Frames left out at the trace's window boundaries: 743.

| Segment             | Intervals                                                                 | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 2303 intervals, p50 16.70 ms, p95 66.70 ms, p99 83.40 ms, max 133.40 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 35829 intervals, p50 16.70 ms, p95 83.30 ms, p99 100.00 ms, max 183.40 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 6937 intervals, p50 16.70 ms, p95 16.80 ms, p99 49.90 ms, max 133.30 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 1763 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 3536 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.00 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 594 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.20 ms, max 33.40 ms     | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 2946 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 14.5      | 14.2  | 14.2     | 14.5            | 0.9                      | 270           | 51                     | 13.2          |
| descent arc         | 10.0      | 9.9   | 9.9      | 32.3            | 4.4                      | 410           | 78                     | 160.9         |
| approach and flare  | 83.4      | 43.3  | 43.3     | 258.1           | 46.7                     | 529           | 129                    | 106.7         |
| low fast pass       | 133.0     | 41.9  | 41.9     | 341.3           | 144.0                    | 269           | 125                    | 30.0          |
| slowdown            | 82.1      | 39.5  | 39.5     | 213.5           | 81.4                     | 160           | 111                    | 56.7          |
| vertical descent    | 1.5       | 1.5   | 1.5      | 83.0            | 83.0                     | 93            | 93                     | 0.4           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 0.1             | 0.1                      | 90            | 90                     | 0.0           |

## Memory (peaks)

- GPU headline: 686 MiB from nvidia-smi
- App processes: 2285 MiB; tracing service (the measurement's own): 246 MiB
- Renderer private: 408 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 686 MiB; the GPU process alone: 631 MiB
- Adapter tally: 427 MiB

Uploads 6831 MiB; 0 pipelines created after warm-up; 0 untimed passes.
