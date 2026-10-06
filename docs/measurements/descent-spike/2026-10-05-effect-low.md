# Descent spike: effect, low, 2026-10-05

- **Overall:** not-measured (provisional: not a quiet machine)
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 21.27, 25.43, 16.07
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, seed 7, window hidden, canvas 806 × 431 px
- **T:** — (no window shown); warm-up 10 s
- **Trace:** — (the trace has no timed event)
- **Incomplete pass times:** — (not recorded before results version 5)
- **GPU clocks:** — (not recorded before results version 5)
- **Options:** `--setting low` `--seed 7` `--smoke false` `--ridged off` `--dawnSafety on` `--capture /home/quantum/gh/hyperion/.claude/worktrees/agent-a41f8f84c6e445909/target/laneD/capture` `--terrainVertexPath face-differences` `--terrainNormals mesh`

## The criterion (Design note 21)

| Row                                               | Limit    | Value    | Verdict      | Note            |
| ------------------------------------------------- | -------- | -------- | ------------ | --------------- |
| 50th percentile ≤ T + 0.5 ms                      | —        | 16.70 ms | not-measured | no window shown |
| 95th percentile ≤ 35 ms                           | —        | 16.80 ms | not-measured | no window shown |
| 99th percentile ≤ 2T                              | —        | 50.00 ms | not-measured | no window shown |
| ≤ 1% of intervals above 1.5 T                     | 1 %      | —        | not-measured | no window shown |
| none above 3 T                                    | 0        | —        | not-measured | no window shown |
| main thread ≤ 0.8 T at the 95th percentile        | —        | 5.20 ms  | not-measured | no window shown |
| GPU pass sum ≤ 0.8 T at the 95th percentile       | —        | 4.07 ms  | not-measured | no window shown |
| terrain GPU time ≤ 14 ms at the 95th percentile   | 14.00 ms | 0.71 ms  | pass         |                 |
| atmosphere GPU time ≤ 4 ms at the 95th percentile | 4.00 ms  | 3.57 ms  | pass         |                 |
| GPU resident ≤ 1 GB                               | 1.0 GB   | 0.254 GB | pass         |                 |

## Frames by segment

Intervals from requestAnimationFrame timestamps. Whole descent: 67928 intervals, p50 16.70 ms, p95 16.80 ms, p99 50.00 ms, max 5883.00 ms.

| Segment             | Intervals                                                                 | Verdicts (p50, p95, p99, missed, hitches)                            |
| ------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| orbit coast         | 2789 intervals, p50 16.70 ms, p95 33.30 ms, p99 50.00 ms, max 83.40 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| descent arc         | 49468 intervals, p50 16.70 ms, p95 33.30 ms, p99 50.00 ms, max 5883.00 ms | not-measured, not-measured, not-measured, not-measured, not-measured |
| approach and flare  | 6986 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 166.60 ms   | not-measured, not-measured, not-measured, not-measured, not-measured |
| low fast pass       | 1640 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.40 ms, max 1949.90 ms  | not-measured, not-measured, not-measured, not-measured, not-measured |
| slowdown            | 3522 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |
| vertical descent    | 588 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 33.40 ms     | not-measured, not-measured, not-measured, not-measured, not-measured |
| hover and touchdown | 2934 intervals, p50 16.70 ms, p95 16.80 ms, p99 33.30 ms, max 50.10 ms    | not-measured, not-measured, not-measured, not-measured, not-measured |

## Streaming (patches a second)

| Segment             | Requested | Baked | Resident | Predicted, hard | Predicted, min(hard, 4σ) | Patches, hard | Patches, min(hard, 4σ) | STREAMING (s) |
| ------------------- | --------- | ----- | -------- | --------------- | ------------------------ | ------------- | ---------------------- | ------------- |
| orbit coast         | 1.6       | 1.6   | 1.6      | 0.7             | 0.0                      | 39            | 19                     | 1.4           |
| descent arc         | 0.8       | 0.8   | 0.8      | 2.1             | 0.2                      | 50            | 25                     | 14.5          |
| approach and flare  | 14.3      | 14.0  | 14.0     | 29.0            | 4.4                      | 86            | 35                     | 23.9          |
| low fast pass       | 42.7      | 41.4  | 41.4     | 57.4            | 11.7                     | 82            | 36                     | 17.2          |
| slowdown            | 51.8      | 49.2  | 49.2     | 34.8            | 7.1                      | 88            | 74                     | 36.6          |
| vertical descent    | 0.0       | 0.0   | 0.0      | 21.4            | 11.8                     | 77            | 79                     | 0.0           |
| hover and touchdown | 0.0       | 0.0   | 0.0      | 0.0             | 0.0                      | 67            | 69                     | 0.0           |

## Memory (peaks)

- GPU headline: 242 MiB from nvidia-smi
- App processes: 5830 MiB; tracing service (the measurement's own): 1676 MiB
- Renderer private: 3177 MiB
- DRM fdinfo resident: — (no DRM client in the process's fdinfo (NVIDIA's driver writes none))
- nvidia-smi less its baseline: 242 MiB; the GPU process alone: 194 MiB
- Adapter tally: 75 MiB

Uploads 3091 MiB; 0 pipelines created after warm-up; 0 untimed passes.
