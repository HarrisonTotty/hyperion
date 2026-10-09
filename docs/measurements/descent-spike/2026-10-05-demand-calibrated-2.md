# Descent demand record, 2026-10-05

**Corrected 2026-10-06** (R05.T13.a's follow-up, record version 4). The record stands and its
JSON is unchanged. Two of its statements are corrected, and two caveats are added:

- Its CPU column read `process.threadCpuUsage` alone, which on this kernel steps by the scheduler's
  tick, about 1 ms, as the notes say. Version 4 reads the thread's clock just after
  `process.cpuUsage`, which makes it exact. No other figure depends on the clock: the hashes agree.
- "At most 0.1 × d_min earlier" described the terrain pass before 13b011b (R05.T11.c, after this
  record). The pass now re-selects on a move of m ÷ (1 + m) = 0.0909 × d_min. The record selects
  every frame, so none of its figures moves.
- Its times were taken under Vite's module runner, whose getters for imported bindings the bundled
  game does not call, so they need not be the game's (R05's Risks, "The record's module runner").
- Its times predate R05.T7 perf (d) (39e7534), which made selection about half as costly with its
  output unchanged bit for bit. Every figure but the times is still current; R05.T13.a's follow-up
  in the plan gives three hard-bound cells re-timed at seed 7 with the exact clock.

Notes:

- Selected at the terrain pass's τ ÷ 1.1 (τ_sel), with D at the same tolerance (decision-r05-record-tau.md, R05.T13.a's follow-up, 24cd2cf), and each selection timed on the thread's CPU clock as well as the wall clock (8c8910f). Otherwise flown as F4's record: the level orbit coast, F1–F3 and selection's perf (c), with T14.c's cache keeping no bake's arrays, which selection does not see. Supersedes 2026-10-05-demand-calibrated, which selected at τ.
- Each cell ran as its own process, nice, at most four at once, without the heavy-test lock, each in its own scope capped at 5 GiB with no swap, under a selection-time cap of 2 h, a wall-time cap of 4 h and a kill at 4 h 5 min. The cells ran twice, at 24cd2cf and then at 8c8910f with the CPU clock; this is the second run, and every figure but the times agrees with the first, hashes included. Wall times 179–853 s a cell; no cell was cut short. All four ran at 64 Hz.
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

## high, min(hard, 4σ_n), ridges off

78721 frames at 64 Hz over 0.0–1230.0 s, selected at τ_sel 0.9091 px (τ 1 px); hash `2b47dd70744a84bd`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Of them, τ′ > τ | Height above floor (m) | Select wall p50 / p95 / max (ms) | Select CPU p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | --------------- | ---------------------- | -------------------------------- | ------------------------------- |
| orbit coast         | 80, 89              | 1.7       | 0.9   | 1.95       | 0%      | —               | 393295–393295          | 0.2 / 0.3 / 134.9                | 0.0 / 1.0 / 1.6                 |
| descent arc         | 136, 237            | 3.2       | 4.4   | 0.73       | 0%      | —               | 14076–392408           | 0.2 / 0.5 / 133.2                | 0.0 / 1.0 / 5.5                 |
| approach and flare  | 200, 222            | 34.0      | 46.7  | 0.73       | 0%      | —               | 508–19626              | 0.4 / 0.9 / 56.9                 | 0.0 / 1.0 / 15.8                |
| low fast pass       | 189, 221            | 105.3     | 144.0 | 0.73       | 0%      | —               | 300–300                | 0.7 / 1.1 / 7.9                  | 1.0 / 1.0 / 2.0                 |
| slowdown            | 140, 214            | 73.7      | 82.1  | 0.90       | 0%      | —               | 193–461                | 0.5 / 1.2 / 142.4                | 1.0 / 1.0 / 2.6                 |
| vertical descent    | 77, 126             | 0.0       | 80.0  | 0.00       | 0%      | —               | 2–200                  | 0.4 / 1.0 / 106.2                | 0.4 / 1.0 / 1.9                 |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | —               | 1–2                    | 0.4 / 0.8 / 81.8                 | 0.0 / 1.0 / 36.7                |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.8% (0.0%)                | 1.0 (—)                        | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 1.4% (0.0%)                | 1.6 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 3.8% (0.0%)                | 13.2 (—)                       | 0.7             |
| low fast pass       | —                       | —                  | —                  | —              | 3.7% (0.1%)                | 31.0 (7.3)                     | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.5% (0.0%)                | 113.7 (—)                      | 11.4            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## low, min(hard, 4σ_n), ridges off

78721 frames at 64 Hz over 0.0–1230.0 s, selected at τ_sel 1.8182 px (τ 2 px); hash `225e5e1c307e3017`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Of them, τ′ > τ | Height above floor (m) | Select wall p50 / p95 / max (ms) | Select CPU p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | --------------- | ---------------------- | -------------------------------- | ------------------------------- |
| orbit coast         | 21, 26              | 0.0       | 0.0  | 0.00       | 0%      | —               | 393295–393295          | 0.0 / 0.1 / 0.7                  | 0.0 / 0.9 / 1.0                 |
| descent arc         | 34, 63              | 0.5       | 0.3  | 1.58       | 0%      | —               | 14076–392408           | 0.1 / 0.1 / 6.4                  | 0.0 / 1.0 / 6.1                 |
| approach and flare  | 51, 60              | 8.7       | 5.6  | 1.57       | 0%      | —               | 508–19626              | 0.1 / 0.4 / 7.3                  | 0.0 / 1.0 / 7.0                 |
| low fast pass       | 52, 67              | 23.0      | 12.9 | 1.78       | 0%      | —               | 300–300                | 0.2 / 0.4 / 0.8                  | 0.0 / 1.0 / 1.4                 |
| slowdown            | 80, 143             | 50.0      | 8.5  | 5.91       | 0%      | —               | 193–461                | 0.2 / 0.7 / 4.1                  | 0.0 / 1.0 / 2.0                 |
| vertical descent    | 77, 107             | 0.0       | 13.6 | 0.00       | 0%      | —               | 2–200                  | 0.2 / 0.3 / 12.2                 | 0.0 / 1.0 / 10.4                |
| hover and touchdown | 68, 68              | 0.0       | 0.0  | 0.00       | 0%      | —               | 1–2                    | 0.2 / 0.3 / 0.6                  | 0.0 / 1.0 / 1.0                 |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 0.2% (0.0%)                | 1.8 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 1.8% (0.0%)                | 10.5 (—)                       | 2.6             |
| low fast pass       | —                       | —                  | —                  | —              | 3.8% (0.0%)                | 22.4 (—)                       | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.6% (0.0%)                | 75.8 (—)                       | 11.7            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## high, min(hard, 4σ_n), ridges on

78721 frames at 64 Hz over 0.0–1230.0 s, selected at τ_sel 0.9091 px (τ 1 px); hash `96c68c9a3dc41393`. **statistical, not a bound on the ridged planet (bound.rs finding).**

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Of them, τ′ > τ | Height above floor (m) | Select wall p50 / p95 / max (ms) | Select CPU p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | --------------- | ---------------------- | -------------------------------- | ------------------------------- |
| orbit coast         | 46, 53              | 0.5       | 0.2   | 2.35       | 0%      | —               | 392975–392975          | 0.1 / 0.1 / 0.6                  | 0.0 / 1.0 / 1.0                 |
| descent arc         | 81, 220             | 2.1       | 3.0   | 0.69       | 0%      | —               | 13756–392087           | 0.1 / 0.3 / 6.6                  | 0.0 / 1.0 / 6.8                 |
| approach and flare  | 212, 273            | 103.1     | 50.4  | 2.04       | 0%      | —               | 369–20079              | 0.4 / 1.1 / 137.6                | 1.0 / 1.0 / 8.7                 |
| low fast pass       | 189, 205            | 98.3      | 143.9 | 0.68       | 0%      | —               | 300–300                | 0.5 / 1.0 / 4.2                  | 1.0 / 1.0 / 2.0                 |
| slowdown            | 142, 188            | 108.2     | 183.2 | 0.59       | 0%      | —               | 175–272                | 0.6 / 1.0 / 84.5                 | 1.0 / 1.0 / 3.0                 |
| vertical descent    | 77, 127             | 0.0       | 87.8  | 0.00       | 0%      | —               | 2–200                  | 0.2 / 0.5 / 0.8                  | 0.0 / 1.0 / 1.0                 |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | —               | 1–2                    | 0.2 / 0.3 / 22.2                 | 0.0 / 1.0 / 4.7                 |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.2% (0.0%)                | 1.0 (—)                        | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 0.9% (0.0%)                | 1.5 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 3.7% (0.0%)                | 25.3 (13.4)                    | 66.9            |
| low fast pass       | —                       | —                  | —                  | —              | 2.9% (0.2%)                | 85.6 (15.8)                    | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.7% (0.0%)                | 171.7 (—)                      | 11.4            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |

## low, min(hard, 4σ_n), ridges on

78721 frames at 64 Hz over 0.0–1230.0 s, selected at τ_sel 1.8182 px (τ 2 px); hash `4ab601367a7d76b8`. **statistical, not a bound on the ridged planet (bound.rs finding).**

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Of them, τ′ > τ | Height above floor (m) | Select wall p50 / p95 / max (ms) | Select CPU p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | --------------- | ---------------------- | -------------------------------- | ------------------------------- |
| orbit coast         | 19, 21              | 0.0       | 0.0  | 0.00       | 0%      | —               | 392975–392975          | 0.1 / 0.1 / 0.5                  | 0.0 / 1.0 / 1.0                 |
| descent arc         | 26, 52              | 0.3       | 0.2  | 1.80       | 0%      | —               | 13756–392087           | 0.1 / 0.1 / 0.9                  | 0.0 / 0.9 / 1.1                 |
| approach and flare  | 65, 125             | 88.3      | 5.9  | 15.06      | 0%      | —               | 369–20079              | 0.1 / 0.7 / 89.8                 | 0.0 / 1.0 / 7.8                 |
| low fast pass       | 50, 60              | 23.3      | 12.9 | 1.81       | 0%      | —               | 300–300                | 0.2 / 0.4 / 1.6                  | 0.0 / 1.0 / 2.3                 |
| slowdown            | 85, 134             | 64.7      | 13.3 | 4.86       | 0%      | —               | 175–272                | 0.2 / 0.7 / 17.0                 | 0.0 / 1.0 / 2.0                 |
| vertical descent    | 77, 106             | 0.0       | 15.1 | 0.00       | 0%      | —               | 2–200                  | 0.2 / 0.2 / 0.5                  | 0.0 / 1.0 / 1.0                 |
| hover and touchdown | 68, 68              | 0.0       | 0.0  | 0.00       | 0%      | —               | 1–2                    | 0.2 / 0.2 / 0.6                  | 0.0 / 1.0 / 1.0                 |

| Segment             | τ′ p50 / p95 / max (px) | Δτ′ p95 / max (px) | Δτ′ ÷ τ′ p95 / max | Steps over 1.1 | Coarse stand-ins (returns) | Their largest ρ (returns) (px) | Forced bakes /s |
| ------------------- | ----------------------- | ------------------ | ------------------ | -------------- | -------------------------- | ------------------------------ | --------------- |
| orbit coast         | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| descent arc         | —                       | —                  | —                  | —              | 0.1% (0.0%)                | 1.8 (—)                        | 0.0             |
| approach and flare  | —                       | —                  | —                  | —              | 1.8% (0.0%)                | 17.2 (—)                       | 81.1            |
| low fast pass       | —                       | —                  | —                  | —              | 2.9% (0.0%)                | 58.2 (—)                       | 0.0             |
| slowdown            | —                       | —                  | —                  | —              | 0.7% (0.0%)                | 114.5 (—)                      | 11.6            |
| vertical descent    | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
| hover and touchdown | —                       | —                  | —                  | —              | 0.0% (0.0%)                | — (—)                          | 0.0             |
