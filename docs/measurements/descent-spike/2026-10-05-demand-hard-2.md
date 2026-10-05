# Descent demand record, 2026-10-05

Notes:

- Selected at the terrain pass's τ ÷ 1.1 (τ_sel), with D at the same tolerance (decision-r05-record-tau.md, R05.T13.a's follow-up, 24cd2cf), and each selection timed on the thread's CPU clock as well as the wall clock (8c8910f). Otherwise flown as F4's record: the level orbit coast, F1–F3 and selection's perf (c), with T14.c's cache keeping no bake's arrays, which selection does not see. Supersedes 2026-10-05-demand-hard, which selected at τ.
- Each cell ran as its own process, nice, at most four at once, without the heavy-test lock, each in its own scope capped at 5 GiB with no swap, under a selection-time cap of 2 h, a wall-time cap of 4 h and a kill at 4 h 5 min. The cells ran twice, at 24cd2cf and then at 8c8910f with the CPU clock; this is the second run, and every figure but the times agrees with the first, hashes included. Wall times 584–2,914 s a cell at loads of 2–19; RSS 0.3–0.6 GiB. No cell was cut short.
- Ridges on, high ran at 16 Hz, a sampling reduction: its demand a second compares with the others'; its patch counts, selection times and τ′ steps are per sampled frame (a 16 Hz step spans four 64 Hz ones).
- Wall-clock selection times are upper bounds, taken under load from other lanes (each cell's load average and wall time are in the JSON): a nice process waiting for a core counts the wait. The CPU times are the selection's own work, from process.threadCpuUsage. On this machine's kernel (CONFIG_HZ=1000) that clock advances in steps of about 1 ms, so the CPU percentiles hold to about ±1 ms: enough to tell the selection's work from a starved process's waits of tens of ms, not the last millisecond of a 2 ms budget. The figures of record for selection time are lane B's perf (c) A/B and the owner's quiet-machine runs.

Seed 7. Fixed-step runs of the scripted descent through `selectPatches` and a
simulated cache (R05.T13.a): patches selected, measured demand (first-time-selected keys a
second), the per-level prediction D, the share of frames `limited`, and the selection time.
Timings are provisional unless the machine was quiet (Design note 27). The selection's wall-clock
times are upper bounds under load, since a nice process waiting for a core counts the wait; its
time on the thread's CPU clock (`process.threadCpuUsage`, user and system) is its own work.

Selected at τ ÷ 1.1, the terrain pass's τ_sel; D at the same tolerance; selected every
frame, without the pass's cadence (decision-r05-record-tau.md). The pass's selection at a frame
is the record's at a pose at most 0.1 × d_min earlier, which moves the demand in time, not in
size. Of the `limited` frames, the first table gives the share whose τ′ exceeds the setting's
τ: at τ_sel, a limited frame may still draw within τ.

The second table of each cell (decision-r05-high-bound.md, F4):

- τ′, the effective tolerance τ_sel × max(1, `limitExcess` ÷ w) that every baked leaf meets,
  over the `limited` frames; the record's one view has w = 1.
- Δτ′, its change from the selection before, over the steps where either is limited, and
  Δτ′ ÷ τ′, the larger τ′ over the smaller, less 1. A vertex inside its morph band steps by
  about 6 × Δτ′ ÷ τ′ in morph factor (R05.T11.c). A step belongs to its later frame's segment.
- The share of those steps beyond the morph bands' margin of 1.1.
- The share of frames drawing a stand-in of level 12 or coarser in place of selected patches,
  and in brackets one covering a return: a patch the cache held earlier and evicted, selected
  again. The rest are first bakes, which the ideal pool lands after the frame's draw. Then the
  largest ρ such a stand-in draws: its level's error at a covered patch's distance, pixels of
  bound.
- The forced region's patches baked a second.

## high, hard ε_n, ridges off

78721 frames at 64 Hz over 0.0–1230.0 s, selected at τ_sel 0.9091 px (τ 1 px); hash `564f08141558ac49`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Of them, τ′ > τ | Height above floor (m) | Select wall p50 / p95 / max (ms) | Select CPU p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | --------------- | ---------------------- | -------------------------------- | ------------------------------- |
| orbit coast         | 554, 585            | 10.4      | 14.5  | 0.72       | 0%      | —               | 393295–393295          | 0.6 / 1.0 / 3.1                  | 1.0 / 1.0 / 2.8                 |
| descent arc         | 792, 981            | 19.5      | 32.3  | 0.60       | 34%     | 0.0%            | 14076–392408           | 1.1 / 2.1 / 218.6                | 1.0 / 2.0 / 12.8                |
| approach and flare  | 917, 981            | 144.7     | 258.7 | 0.56       | 30%     | 0.0%            | 508–19626              | 2.1 / 3.8 / 528.3                | 2.0 / 3.9 / 26.3                |
| low fast pass       | 632, 755            | 376.6     | 341.3 | 1.10       | 0%      | —               | 300–300                | 1.9 / 3.6 / 163.8                | 2.0 / 3.0 / 6.9                 |
| slowdown            | 329, 752            | 159.2     | 211.6 | 0.75       | 0%      | —               | 193–461                | 1.0 / 2.6 / 251.0                | 1.0 / 3.0 / 6.8                 |
| vertical descent    | 77, 126             | 0.0       | 80.0  | 0.00       | 0%      | —               | 2–200                  | 0.2 / 0.3 / 0.7                  | 0.0 / 1.0 / 1.0                 |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | —               | 1–2                    | 0.2 / 0.2 / 22.7                 | 0.0 / 1.0 / 22.8                |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 5.5% (0.0%)                | 2.4 (—)                        | 0.0             |
| descent arc         | 0.94 / 0.96 / 0.96      | 0.00 / 0.01        | 0.001 / 0.009      | 0.0%           | 8.0% (0.0%)                | 3.7 (—)                        | 0.0             |
| approach and flare  | 0.92 / 0.92 / 0.92      | 0.00 / 0.01        | 0.002 / 0.013      | 0.0%           | 5.6% (0.2%)                | 19.4 (16.4)                    | 0.5             |
| low fast pass       | —                       | —                  | —                  | —              | 3.9% (1.3%)                | 66.9 (29.5)                    | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.5% (0.0%)                | 287.0 (54.9)                   | 10.7            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## low, hard ε_n, ridges off

78721 frames at 64 Hz over 0.0–1230.0 s, selected at τ_sel 1.8182 px (τ 2 px); hash `735ddf3d8cd1165f`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Of them, τ′ > τ | Height above floor (m) | Select wall p50 / p95 / max (ms) | Select CPU p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | --------------- | ---------------------- | -------------------------------- | ------------------------------- |
| orbit coast         | 57, 62              | 0.9       | 0.7  | 1.26       | 0%      | —               | 393295–393295          | 0.1 / 0.2 / 0.6                  | 0.0 / 1.0 / 1.1                 |
| descent arc         | 93, 177             | 2.1       | 2.6  | 0.79       | 0%      | —               | 14076–392408           | 0.1 / 0.3 / 152.7                | 0.0 / 1.0 / 8.1                 |
| approach and flare  | 165, 182            | 26.4      | 34.9 | 0.76       | 0%      | —               | 508–19626              | 0.5 / 2.2 / 158.0                | 1.0 / 1.3 / 3.1                 |
| low fast pass       | 145, 180            | 85.2      | 63.1 | 1.35       | 0%      | —               | 300–300                | 0.6 / 1.1 / 46.2                 | 1.0 / 1.0 / 2.0                 |
| slowdown            | 119, 171            | 68.8      | 41.0 | 1.68       | 0%      | —               | 193–461                | 0.4 / 0.9 / 1.9                  | 0.0 / 1.0 / 2.0                 |
| vertical descent    | 76, 104             | 0.0       | 23.9 | 0.00       | 0%      | —               | 2–200                  | 0.2 / 0.3 / 0.6                  | 0.0 / 1.0 / 1.0                 |
| hover and touchdown | 67, 67              | 0.0       | 0.0  | 0.00       | 0%      | —               | 1–2                    | 0.2 / 0.3 / 10.1                 | 0.0 / 1.0 / 9.9                 |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.6% (0.0%)                | 1.8 (—)                        | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 0.9% (0.0%)                | 2.6 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 3.5% (0.0%)                | 18.6 (—)                       | 1.7             |
| low fast pass       | —                       | —                  | —                  | —              | 4.0% (0.1%)                | 45.3 (19.7)                    | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.5% (0.0%)                | 192.8 (—)                      | 11.5            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## high, hard ε_n, ridges on

19681 frames at 16 Hz over 0.0–1230.0 s, selected at τ_sel 0.9091 px (τ 1 px); hash `47f284d65935223d`.

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Of them, τ′ > τ | Height above floor (m) | Select wall p50 / p95 / max (ms) | Select CPU p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | --------------- | ---------------------- | -------------------------------- | ------------------------------- |
| orbit coast         | 980, 981            | 29.5      | 248.9 | 0.12       | 100%    | 100.0%          | 392975–392975          | 1.6 / 2.2 / 5.2                  | 2.0 / 2.0 / 4.3                 |
| descent arc         | 980, 981            | 27.8      | 298.3 | 0.09       | 100%    | 100.0%          | 13772–392087           | 1.7 / 2.9 / 271.3                | 2.0 / 3.0 / 6.9                 |
| approach and flare  | 970, 1051           | 250.3     | 461.4 | 0.54       | 84%     | 87.8%           | 369–20079              | 2.9 / 5.3 / 208.5                | 3.0 / 5.0 / 26.3                |
| low fast pass       | 678, 760            | 382.0     | 423.7 | 0.90       | 0%      | —               | 300–300                | 2.2 / 2.8 / 5.3                  | 2.0 / 3.0 / 5.0                 |
| slowdown            | 239, 612            | 147.2     | 323.3 | 0.46       | 0%      | —               | 176–272                | 1.0 / 1.6 / 2.2                  | 1.0 / 2.0 / 2.4                 |
| vertical descent    | 77, 127             | 0.0       | 87.9  | 0.00       | 0%      | —               | 2–200                  | 0.2 / 0.3 / 0.6                  | 0.0 / 1.0 / 1.0                 |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | —               | 1–2                    | 0.2 / 0.2 / 0.5                  | 0.0 / 1.0 / 1.0                 |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | 2.07 / 2.11 / 2.11      | 0.00 / 0.01        | 0.002 / 0.006      | 0.0%           | 44.8% (8.5%)               | 2.9 (2.1)                      | 0.0             |
| descent arc         | 2.56 / 3.27 / 3.34      | 0.01 / 0.04        | 0.002 / 0.014      | 0.0%           | 41.7% (0.7%)               | 4.1 (3.1)                      | 0.0             |
| approach and flare  | 1.62 / 2.64 / 2.66      | 0.01 / 0.04        | 0.008 / 0.019      | 0.0%           | 20.1% (1.1%)               | 55.6 (45.1)                    | 85.5            |
| low fast pass       | —                       | —                  | —                  | —              | 13.1% (5.2%)               | 139.2 (67.6)                   | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 2.6% (0.3%)                | 1014.8 (166.5)                 | 14.2            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## low, hard ε_n, ridges on

78721 frames at 64 Hz over 0.0–1230.0 s, selected at τ_sel 1.8182 px (τ 2 px); hash `f38d18c072344abe`.

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Of them, τ′ > τ | Height above floor (m) | Select wall p50 / p95 / max (ms) | Select CPU p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | --------------- | ---------------------- | -------------------------------- | ------------------------------- |
| orbit coast         | 354, 379            | 15.9      | 13.8  | 1.15       | 0%      | —               | 392975–392975          | 0.8 / 1.7 / 396.3                | 1.0 / 2.0 / 14.6                |
| descent arc         | 573, 648            | 17.6      | 38.7  | 0.45       | 58%     | 82.3%           | 13756–392087           | 0.8 / 1.9 / 159.9                | 1.0 / 2.0 / 50.6                |
| approach and flare  | 471, 648            | 131.8     | 88.7  | 1.49       | 29%     | 87.6%           | 369–20079              | 1.3 / 2.8 / 221.5                | 1.0 / 2.2 / 5.1                 |
| low fast pass       | 180, 200            | 94.9      | 141.2 | 0.67       | 0%      | —               | 300–300                | 0.7 / 1.2 / 9.0                  | 1.0 / 1.2 / 2.2                 |
| slowdown            | 114, 169            | 85.7      | 73.3  | 1.17       | 0%      | —               | 175–272                | 0.4 / 0.8 / 1.6                  | 0.0 / 1.0 / 2.0                 |
| vertical descent    | 75, 102             | 0.0       | 26.4  | 0.00       | 0%      | —               | 2–200                  | 0.2 / 0.3 / 0.6                  | 0.0 / 1.0 / 1.0                 |
| hover and touchdown | 66, 66              | 0.0       | 0.0   | 0.00       | 0%      | —               | 1–2                    | 0.2 / 0.2 / 16.1                 | 0.0 / 1.0 / 16.1                |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 6.3% (0.0%)                | 2.0 (—)                        | 0.0             |
| descent arc         | 2.39 / 2.77 / 2.81      | 0.00 / 0.04        | 0.001 / 0.016      | 0.0%           | 7.0% (0.0%)                | 2.8 (2.5)                      | 0.0             |
| approach and flare  | 2.22 / 2.25 / 2.26      | 0.01 / 0.04        | 0.003 / 0.019      | 0.0%           | 5.9% (0.2%)                | 37.1 (30.0)                    | 82.3            |
| low fast pass       | —                       | —                  | —                  | —              | 3.3% (0.8%)                | 93.2 (45.0)                    | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.7% (0.0%)                | 667.1 (110.9)                  | 11.4            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
