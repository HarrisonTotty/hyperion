# Descent demand record, 2026-10-05

Notes:

- Flown with the level orbit coast (af69a9e), Selection.limitExcess (F1), the morph bands at τ′ (F2), the cache keeping the baked patches selection hides (F3) and selection's perf (c). Supersedes 2026-10-03-demand-calibrated.
- Each cell ran as its own process, nice, at most four at once beside the hard record's high cells, under a selection-time cap of 2 h and a wall-time cap of 4 h. No cell was cut short; all four ran at 64 Hz.
- Selection times are provisional, taken under load from other lanes (each cell's load average and wall time are in the JSON). The figures of record for selection time are lane B's perf (c) A/B and the owner's quiet-machine runs.

Seed 7. Fixed-step runs of the scripted descent through `selectPatches` and a
simulated cache (R05.T13.a): patches selected, measured demand (first-time-selected keys a
second), the per-level prediction D, the share of frames `limited`, and the selection time.
Timings are provisional unless the machine was quiet (Design note 27).

The second table of each cell (decision-r05-high-bound.md, F4):

- τ′, the effective tolerance τ × max(1, `limitExcess` ÷ w) that every baked leaf meets, over
  the `limited` frames; the record's one view has w = 1. The record selects at the setting's τ;
  the terrain pass selects at τ ÷ 1.1.
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

## high, min(hard, 4σ_n), ridges off

78721 frames at 64 Hz over 0.0–1230.0 s; hash `097124d1f2220ee2`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 65, 73              | 1.7       | 0.8   | 2.07       | 0%      | 393295–393295          | 0.1 / 0.2 / 0.7             |
| descent arc         | 114, 205            | 2.7       | 3.5   | 0.75       | 0%      | 14076–392408           | 0.2 / 0.4 / 101.5           |
| approach and flare  | 174, 194            | 29.1      | 39.4  | 0.74       | 0%      | 508–19626              | 0.4 / 0.9 / 28.0            |
| low fast pass       | 166, 197            | 94.6      | 130.9 | 0.72       | 0%      | 300–300                | 0.6 / 1.0 / 18.9            |
| slowdown            | 130, 191            | 71.8      | 60.6  | 1.19       | 0%      | 193–461                | 0.5 / 0.9 / 3.0             |
| vertical descent    | 77, 126             | 0.1       | 70.7  | 0.00       | 0%      | 2–200                  | 0.3 / 0.6 / 1.2             |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | 1–2                    | 0.3 / 0.3 / 13.5            |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.8% (0.0%)                | 1.0 (—)                        | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 1.2% (0.0%)                | 1.6 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 3.5% (0.0%)                | 13.2 (—)                       | 1.4             |
| low fast pass       | —                       | —                  | —                  | —              | 3.8% (0.1%)                | 31.7 (6.2)                     | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.5% (0.0%)                | 113.7 (—)                      | 11.4            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## low, min(hard, 4σ_n), ridges off

78721 frames at 64 Hz over 0.0–1230.0 s; hash `0bea67f07336f615`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 20, 24              | 0.1       | 0.0  | 6.71       | 0%      | 393295–393295          | 0.1 / 0.1 / 13.1            |
| descent arc         | 32, 56              | 0.4       | 0.2  | 1.79       | 0%      | 14076–392408           | 0.1 / 0.1 / 35.8            |
| approach and flare  | 47, 57              | 7.9       | 4.4  | 1.77       | 0%      | 508–19626              | 0.2 / 0.3 / 50.6            |
| low fast pass       | 48, 64              | 20.1      | 11.7 | 1.71       | 0%      | 300–300                | 0.2 / 0.3 / 0.9             |
| slowdown            | 79, 144             | 49.5      | 7.0  | 7.04       | 0%      | 193–461                | 0.3 / 3.2 / 89.7            |
| vertical descent    | 78, 108             | 0.0       | 11.6 | 0.00       | 0%      | 2–200                  | 0.3 / 0.4 / 0.7             |
| hover and touchdown | 69, 69              | 0.0       | 0.0  | 0.00       | 0%      | 1–2                    | 0.3 / 0.3 / 47.4            |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.1% (0.0%)                | 1.8 (—)                        | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 0.1% (0.0%)                | 2.0 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 1.6% (0.0%)                | 10.5 (—)                       | 2.6             |
| low fast pass       | —                       | —                  | —                  | —              | 3.8% (0.0%)                | 22.4 (—)                       | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.5% (0.0%)                | 75.8 (—)                       | 11.7            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## high, min(hard, 4σ_n), ridges on

78721 frames at 64 Hz over 0.0–1230.0 s; hash `26ead1208525a116`. **statistical, not a bound on the ridged planet (bound.rs finding).**

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 43, 48              | 0.4       | 0.2   | 2.16       | 0%      | 392975–392975          | 0.1 / 0.2 / 1.4             |
| descent arc         | 70, 194             | 1.7       | 2.4   | 0.72       | 0%      | 13756–392087           | 0.1 / 0.3 / 79.8            |
| approach and flare  | 186, 247            | 98.7      | 40.9  | 2.41       | 0%      | 369–20079              | 0.5 / 1.4 / 73.9            |
| low fast pass       | 166, 181            | 92.4      | 130.8 | 0.71       | 0%      | 300–300                | 0.5 / 0.9 / 2.0             |
| slowdown            | 126, 167            | 96.5      | 162.8 | 0.59       | 0%      | 175–272                | 0.6 / 4.7 / 59.2            |
| vertical descent    | 77, 127             | 0.0       | 77.8  | 0.00       | 0%      | 2–200                  | 0.4 / 19.2 / 63.6           |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | 1–2                    | 0.3 / 3.1 / 43.2            |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.2% (0.0%)                | 1.0 (—)                        | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 0.7% (0.0%)                | 1.5 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 3.5% (0.0%)                | 25.3 (13.4)                    | 67.0            |
| low fast pass       | —                       | —                  | —                  | —              | 2.9% (0.2%)                | 85.6 (15.8)                    | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.7% (0.0%)                | 171.7 (—)                      | 11.4            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## low, min(hard, 4σ_n), ridges on

78721 frames at 64 Hz over 0.0–1230.0 s; hash `98ea4e20d8f79817`. **statistical, not a bound on the ridged planet (bound.rs finding).**

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 19, 21              | 0.0       | 0.0  | 0.00       | 0%      | 392975–392975          | 0.1 / 0.1 / 7.4             |
| descent arc         | 24, 48              | 0.3       | 0.1  | 1.99       | 0%      | 13756–392087           | 0.1 / 0.1 / 85.5            |
| approach and flare  | 61, 122             | 91.3      | 4.9  | 18.67      | 0%      | 369–20079              | 0.2 / 1.2 / 92.1            |
| low fast pass       | 47, 58              | 22.3      | 11.7 | 1.91       | 0%      | 300–300                | 0.2 / 3.1 / 29.2            |
| slowdown            | 85, 135             | 62.8      | 11.8 | 5.31       | 0%      | 175–272                | 0.3 / 1.0 / 81.9            |
| vertical descent    | 78, 107             | 0.0       | 12.8 | 0.00       | 0%      | 2–200                  | 0.3 / 0.6 / 4.1             |
| hover and touchdown | 69, 69              | 0.0       | 0.0  | 0.00       | 0%      | 1–2                    | 0.3 / 0.3 / 10.3            |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 0.1% (0.0%)                | 2.0 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 1.7% (0.0%)                | 17.2 (—)                       | 85.1            |
| low fast pass       | —                       | —                  | —                  | —              | 2.9% (0.0%)                | 59.4 (—)                       | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.7% (0.0%)                | 114.5 (—)                      | 11.6            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
