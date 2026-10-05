# Descent demand record, 2026-10-05

**Superseded** by [`2026-10-05-demand-hard-2.md`](2026-10-05-demand-hard-2.md) (record version 3,
R05.T13.a's follow-up of decision-r05-record-tau.md): selected at the setting's τ, not the terrain
pass's τ ÷ 1.1. Where unlimited, its segments' patch counts run 4–22% low (30% in the ridged low
coast) and their demand 2–21% low (half in the ridged low coast), and high ridges-off's 0% `limited`
is not the pass's: 34% of the arc and 30% of the approach at τ ÷ 1.1 (decision-r05-record-tau.md).
This record is kept because decision-r05-high-bound.md, F4's handoff and the plan cite it.

Notes:

- Flown with the level orbit coast (af69a9e), Selection.limitExcess (F1), the morph bands at τ′ (F2), the cache keeping the baked patches selection hides (F3) and selection's perf (c). Supersedes 2026-10-04-demand-hard.
- Each cell ran as its own process, nice, at most four at once. Ridges off and on, low ran first beside the two high cells, under a selection-time cap of 1.6 h and a wall-time cap of 1.85 h. The high cells, stopped twice (once to free the heavy-test lock, once by a power-off), were rerun from the start beside the 4σ cells, under caps of 2 h and 4 h. No cell was cut short.
- Ridges on, high ran at 16 Hz, a sampling reduction: its demand a second compares with the others'; its patch counts, selection times and τ′ steps are per sampled frame (a 16 Hz step spans four 64 Hz ones).
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

## high, hard ε_n, ridges off

78721 frames at 64 Hz over 0.0–1230.0 s; hash `ec4300c0eecb583f`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 455, 482            | 8.7       | 13.2  | 0.66       | 0%      | 393295–393295          | 0.9 / 4.7 / 72.3            |
| descent arc         | 662, 915            | 16.1      | 26.4  | 0.61       | 0%      | 14076–392408           | 1.1 / 2.4 / 150.0           |
| approach and flare  | 783, 857            | 123.0     | 219.0 | 0.56       | 0%      | 508–19626              | 2.0 / 8.3 / 146.9           |
| low fast pass       | 547, 655            | 347.5     | 310.3 | 1.12       | 0%      | 300–300                | 2.0 / 8.1 / 959.7           |
| slowdown            | 298, 645            | 153.9     | 186.0 | 0.83       | 0%      | 193–461                | 1.0 / 6.5 / 83.9            |
| vertical descent    | 77, 126             | 0.1       | 70.7  | 0.00       | 0%      | 2–200                  | 0.3 / 10.5 / 76.3           |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | 1–2                    | 0.3 / 7.7 / 26.5            |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 3.7% (0.0%)                | 2.4 (—)                        | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 6.5% (0.0%)                | 3.7 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 5.5% (0.2%)                | 19.4 (16.4)                    | 0.8             |
| low fast pass       | —                       | —                  | —                  | —              | 3.9% (1.1%)                | 66.9 (29.5)                    | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.5% (0.0%)                | 287.0 (54.9)                   | 10.7            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## low, hard ε_n, ridges off

78721 frames at 64 Hz over 0.0–1230.0 s; hash `2fdf56b241ee324f`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 53, 59              | 0.5       | 0.7  | 0.68       | 0%      | 393295–393295          | 0.1 / 0.1 / 0.4             |
| descent arc         | 80, 153             | 1.7       | 2.1  | 0.81       | 0%      | 14076–392408           | 0.1 / 0.3 / 3.9             |
| approach and flare  | 145, 161            | 23.1      | 29.3 | 0.79       | 0%      | 508–19626              | 0.3 / 0.7 / 9.8             |
| low fast pass       | 129, 167            | 73.9      | 57.4 | 1.29       | 0%      | 300–300                | 0.3 / 0.7 / 1.2             |
| slowdown            | 111, 158            | 66.6      | 34.6 | 1.93       | 0%      | 193–461                | 0.4 / 0.9 / 9.2             |
| vertical descent    | 76, 104             | 0.0       | 21.0 | 0.00       | 0%      | 2–200                  | 0.4 / 0.8 / 1.3             |
| hover and touchdown | 67, 67              | 0.0       | 0.0  | 0.00       | 0%      | 1–2                    | 0.3 / 0.5 / 44.3            |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.2% (0.0%)                | 2.0 (—)                        | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 0.8% (0.0%)                | 2.8 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 3.4% (0.0%)                | 18.6 (—)                       | 1.7             |
| low fast pass       | —                       | —                  | —                  | —              | 4.0% (0.1%)                | 45.3 (19.7)                    | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.5% (0.0%)                | 192.8 (—)                      | 11.5            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## high, hard ε_n, ridges on

19681 frames at 16 Hz over 0.0–1230.0 s; hash `7c53b786241b137a`.

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 980, 981            | 29.5      | 226.3 | 0.13       | 100%    | 392975–392975          | 1.8 / 3.9 / 102.0           |
| descent arc         | 980, 981            | 27.8      | 255.3 | 0.11       | 100%    | 13772–392087           | 1.9 / 5.2 / 95.0            |
| approach and flare  | 949, 1051           | 239.7     | 401.0 | 0.60       | 74%     | 369–20079              | 3.2 / 29.2 / 197.0          |
| low fast pass       | 620, 699            | 360.4     | 385.2 | 0.94       | 0%      | 300–300                | 2.2 / 4.6 / 45.6            |
| slowdown            | 228, 551            | 144.3     | 290.6 | 0.50       | 0%      | 176–272                | 0.9 / 1.7 / 3.0             |
| vertical descent    | 77, 127             | 0.0       | 77.9  | 0.00       | 0%      | 2–200                  | 0.2 / 0.3 / 0.7             |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | 1–2                    | 0.2 / 0.2 / 0.5             |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | 2.07 / 2.11 / 2.11      | 0.00 / 0.01        | 0.002 / 0.006      | 0.0%           | 44.8% (8.5%)               | 2.9 (2.1)                      | 0.0             |
| descent arc         | 2.56 / 3.27 / 3.34      | 0.01 / 0.04        | 0.002 / 0.014      | 0.0%           | 41.7% (0.7%)               | 4.1 (3.1)                      | 0.0             |
| approach and flare  | 1.94 / 2.65 / 2.66      | 0.01 / 0.04        | 0.008 / 0.019      | 0.0%           | 20.1% (1.1%)               | 55.6 (45.1)                    | 84.8            |
| low fast pass       | —                       | —                  | —                  | —              | 13.1% (5.2%)               | 139.2 (67.6)                   | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 2.6% (0.3%)                | 1014.8 (166.5)                 | 15.1            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## low, hard ε_n, ridges on

78721 frames at 64 Hz over 0.0–1230.0 s; hash `4f7d481f140c06bb`.

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 271, 292            | 7.4       | 12.6 | 0.59       | 0%      | 392975–392975          | 0.4 / 0.8 / 10.6            |
| descent arc         | 513, 648            | 15.4      | 31.4 | 0.49       | 48%     | 13756–392087           | 0.7 / 1.5 / 21.4            |
| approach and flare  | 441, 648            | 125.1     | 77.3 | 1.62       | 25%     | 369–20079              | 0.8 / 1.7 / 7.5             |
| low fast pass       | 159, 173            | 88.9      | 78.6 | 1.13       | 0%      | 300–300                | 0.4 / 0.8 / 1.9             |
| slowdown            | 110, 160            | 78.7      | 66.4 | 1.18       | 0%      | 175–272                | 0.3 / 0.7 / 1.7             |
| vertical descent    | 76, 103             | 0.0       | 23.3 | 0.00       | 0%      | 2–200                  | 0.2 / 0.3 / 0.6             |
| hover and touchdown | 67, 67              | 0.0       | 0.0  | 0.00       | 0%      | 1–2                    | 0.2 / 0.2 / 31.2            |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 3.0% (0.0%)                | 2.0 (—)                        | 0.0             |
| descent arc         | 2.48 / 2.77 / 2.81      | 0.00 / 0.04        | 0.001 / 0.016      | 0.0%           | 6.2% (0.0%)                | 2.8 (2.5)                      | 0.0             |
| approach and flare  | 2.23 / 2.25 / 2.26      | 0.01 / 0.04        | 0.003 / 0.019      | 0.0%           | 5.6% (0.1%)                | 37.1 (30.0)                    | 81.0            |
| low fast pass       | —                       | —                  | —                  | —              | 3.3% (0.8%)                | 93.2 (45.0)                    | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.7% (0.0%)                | 667.1 (110.9)                  | 11.5            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
