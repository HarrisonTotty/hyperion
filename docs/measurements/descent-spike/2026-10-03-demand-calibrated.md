# Descent demand record, 2026-10-03

**Superseded** by [`2026-10-05-demand-calibrated.md`](2026-10-05-demand-calibrated.md) (record
version 2, R05.T13.a's F4 of decision-r05-high-bound.md). This record was flown before the level
orbit coast (af69a9e) and before the cache stopped evicting the baked patches selection hides (F3,
cc96486):

- every orbit-coast row is stale;
- every descent-arc row is stale too, since the arc was flown up to 1.05 km lower near its start.

Seed 7. Fixed-step runs of the scripted descent through `selectPatches` and a
simulated cache (R05.T13.a): patches selected, measured demand (first-time-selected keys a
second), the per-level prediction D, the share of frames `limited`, and the selection time.
Timings are provisional unless the machine was quiet (Design note 27).

## high, min(hard, 4σ_n), ridges off

78721 frames at 64 Hz over 0.0–1230.0 s; hash `c6b4f6aac173984b`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 65, 73              | 1.7       | 0.8   | 2.02       | 0%      | 393302–394307          | 0.4 / 0.9 / 100.9           |
| descent arc         | 114, 205            | 2.7       | 3.5   | 0.75       | 0%      | 14076–392408           | 0.4 / 1.0 / 80.0            |
| approach and flare  | 174, 194            | 29.4      | 39.4  | 0.75       | 0%      | 508–19626              | 0.9 / 1.9 / 63.9            |
| low fast pass       | 166, 197            | 95.2      | 130.9 | 0.73       | 0%      | 300–300                | 0.9 / 2.4 / 34.3            |
| slowdown            | 130, 191            | 72.2      | 60.6  | 1.19       | 0%      | 193–461                | 0.9 / 2.6 / 61.3            |
| vertical descent    | 77, 126             | 0.1       | 70.7  | 0.00       | 0%      | 2–200                  | 0.6 / 27.0 / 37.1           |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | 1–2                    | 0.5 / 22.9 / 60.0           |

## low, min(hard, 4σ_n), ridges off

78721 frames at 64 Hz over 0.0–1230.0 s; hash `2288439603e0b5c4`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 20, 24              | 0.1       | 0.0  | 6.63       | 0%      | 393302–394307          | 0.1 / 0.2 / 94.4            |
| descent arc         | 32, 56              | 0.4       | 0.2  | 1.78       | 0%      | 14076–392408           | 0.1 / 0.2 / 26.4            |
| approach and flare  | 47, 57              | 7.9       | 4.4  | 1.77       | 0%      | 508–19626              | 0.3 / 0.5 / 1.6             |
| low fast pass       | 48, 64              | 20.4      | 11.7 | 1.74       | 0%      | 300–300                | 0.3 / 0.5 / 1.2             |
| slowdown            | 79, 144             | 49.7      | 7.0  | 7.07       | 0%      | 193–461                | 0.5 / 1.4 / 3.4             |
| vertical descent    | 78, 108             | 0.0       | 11.6 | 0.00       | 0%      | 2–200                  | 0.4 / 0.7 / 1.0             |
| hover and touchdown | 69, 69              | 0.0       | 0.0  | 0.00       | 0%      | 1–2                    | 0.4 / 0.7 / 1.2             |

## high, min(hard, 4σ_n), ridges on

78721 frames at 64 Hz over 0.0–1230.0 s; hash `8368900b66cac22f`. **statistical, not a bound on the ridged planet (bound.rs finding).**

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 43, 48              | 0.4       | 0.2   | 2.14       | 0%      | 392982–393987          | 0.2 / 0.3 / 3.1             |
| descent arc         | 70, 194             | 1.7       | 2.4   | 0.72       | 0%      | 13756–392087           | 0.3 / 1.1 / 81.1            |
| approach and flare  | 186, 247            | 102.0     | 40.9  | 2.50       | 0%      | 369–20079              | 0.9 / 2.7 / 59.1            |
| low fast pass       | 166, 181            | 93.1      | 130.8 | 0.71       | 0%      | 300–300                | 1.0 / 2.3 / 22.8            |
| slowdown            | 126, 167            | 97.0      | 162.8 | 0.60       | 0%      | 175–272                | 0.8 / 1.7 / 17.0            |
| vertical descent    | 77, 127             | 0.0       | 77.8  | 0.00       | 0%      | 2–200                  | 0.4 / 0.6 / 9.4             |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | 1–2                    | 0.4 / 0.4 / 0.8             |

## low, min(hard, 4σ_n), ridges on

78721 frames at 64 Hz over 0.0–1230.0 s; hash `3f3311eadcb946f4`. **statistical, not a bound on the ridged planet (bound.rs finding).**

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 19, 21              | 0.0       | 0.0  | 0.00       | 0%      | 392982–393987          | 0.3 / 0.5 / 170.3           |
| descent arc         | 24, 48              | 0.3       | 0.1  | 1.99       | 0%      | 13756–392087           | 0.2 / 0.3 / 60.0            |
| approach and flare  | 61, 122             | 93.3      | 4.9  | 19.07      | 0%      | 369–20079              | 0.5 / 2.1 / 50.8            |
| low fast pass       | 47, 58              | 22.5      | 11.7 | 1.92       | 0%      | 300–300                | 0.4 / 1.3 / 45.0            |
| slowdown            | 85, 135             | 63.1      | 11.8 | 5.33       | 0%      | 175–272                | 0.7 / 1.7 / 66.4            |
| vertical descent    | 78, 107             | 0.0       | 12.8 | 0.00       | 0%      | 2–200                  | 0.5 / 0.7 / 1.1             |
| hover and touchdown | 69, 69              | 0.0       | 0.0  | 0.00       | 0%      | 1–2                    | 0.4 / 0.4 / 0.9             |
