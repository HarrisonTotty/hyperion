# Descent demand record, 2026-10-04

Notes:

- The four cells ran as four parallel processes (one core each, nice), each with its own selection-time cap (1.6 h) and wall-time cap (1.85 h, bakes included); a cell cut short says so.
- Ridges on, high ran at 16 Hz, a sampling reduction: its demand a second compares with the others', its patch counts and selection times are per sampled frame.
- The orbit coast still climbs about 18.37 m/s at seed 7 (the profile's re-fit before the descent arc's blend; an R05.T13.a fix is queued), so the coast's figures may shift slightly once it is fixed.

Seed 7. Fixed-step runs of the scripted descent through `selectPatches` and a
simulated cache (R05.T13.a): patches selected, measured demand (first-time-selected keys a
second), the per-level prediction D, the share of frames `limited`, and the selection time.
Timings are provisional unless the machine was quiet (Design note 27).

## high, hard ε_n, ridges off

78721 frames at 64 Hz over 0.0–1230.0 s; hash `120f2c93fcd00e8d`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 454, 482            | 8.7       | 13.3  | 0.65       | 0%      | 393302–394307          | 1.3 / 2.3 / 8.8             |
| descent arc         | 662, 915            | 16.4      | 26.5  | 0.62       | 0%      | 14076–392408           | 2.3 / 4.8 / 174.3           |
| approach and flare  | 783, 857            | 128.3     | 219.0 | 0.59       | 0%      | 508–19626              | 3.4 / 6.2 / 283.9           |
| low fast pass       | 547, 655            | 357.3     | 310.3 | 1.15       | 0%      | 300–300                | 3.2 / 4.3 / 234.8           |
| slowdown            | 298, 645            | 156.1     | 186.0 | 0.84       | 0%      | 193–461                | 1.7 / 3.6 / 32.4            |
| vertical descent    | 77, 126             | 0.1       | 70.7  | 0.00       | 0%      | 2–200                  | 0.5 / 0.7 / 1.3             |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | 1–2                    | 0.4 / 0.4 / 1.5             |

## low, hard ε_n, ridges off

78721 frames at 64 Hz over 0.0–1230.0 s; hash `2b41c82df664ba8c`.

Site -1845.8 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +418.6 m (120.0 s), low fast pass +418.6 → +418.6 m (30.0 s), slowdown +418.6 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 53, 59              | 0.5       | 0.7  | 0.67       | 0%      | 393302–394307          | 0.2 / 0.3 / 2.4             |
| descent arc         | 80, 153             | 1.7       | 2.1  | 0.82       | 0%      | 14076–392408           | 0.3 / 0.5 / 43.8            |
| approach and flare  | 145, 161            | 23.4      | 29.3 | 0.80       | 0%      | 508–19626              | 0.7 / 3.6 / 131.4           |
| low fast pass       | 129, 167            | 74.9      | 57.4 | 1.31       | 0%      | 300–300                | 0.8 / 1.5 / 10.1            |
| slowdown            | 111, 158            | 67.2      | 34.6 | 1.94       | 0%      | 193–461                | 0.8 / 1.7 / 13.3            |
| vertical descent    | 76, 104             | 0.0       | 21.0 | 0.00       | 0%      | 2–200                  | 0.5 / 0.9 / 2.0             |
| hover and touchdown | 67, 67              | 0.0       | 0.0  | 0.00       | 0%      | 1–2                    | 0.6 / 1.0 / 2.9             |

## high, hard ε_n, ridges on

19681 frames at 16 Hz over 0.0–1230.0 s; hash `88d3119a410a0e2a`.

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s  | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ----- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 980, 981            | 29.6      | 228.5 | 0.13       | 100%    | 392993–393987          | 3.7 / 6.2 / 29.1            |
| descent arc         | 980, 981            | 28.0      | 255.6 | 0.11       | 100%    | 13772–392087           | 3.9 / 8.4 / 384.3           |
| approach and flare  | 950, 1051           | 278.0     | 401.0 | 0.69       | 74%     | 369–20079              | 6.4 / 10.7 / 107.2          |
| low fast pass       | 621, 700            | 374.2     | 385.2 | 0.97       | 0%      | 300–300                | 5.7 / 13.1 / 167.7          |
| slowdown            | 228, 552            | 146.4     | 290.6 | 0.50       | 0%      | 176–272                | 2.4 / 4.6 / 58.3            |
| vertical descent    | 77, 128             | 0.1       | 77.9  | 0.00       | 0%      | 2–200                  | 0.5 / 0.7 / 2.6             |
| hover and touchdown | 66, 66              | 0.0       | 0.1   | 0.00       | 0%      | 1–2                    | 0.5 / 0.5 / 2.5             |

## low, hard ε_n, ridges on

78721 frames at 64 Hz over 0.0–1230.0 s; hash `b89ba9e6e89b6a79`.

Site -1644.6 m; least margin above the stretches' floors and clearances 0.0 m; lifts above the table: approach and flare +0.0 → +64.4 m (120.0 s), low fast pass +64.4 → +64.4 m (30.0 s), slowdown +64.4 → +0.0 m (60.0 s).

| Segment             | Patches (mean, max) | Demand /s | D /s | Demand ÷ D | Limited | Height above floor (m) | Select p50 / p95 / max (ms) |
| ------------------- | ------------------- | --------- | ---- | ---------- | ------- | ---------------------- | --------------------------- |
| orbit coast         | 271, 292            | 7.4       | 12.7 | 0.58       | 0%      | 392982–393987          | 0.8 / 1.5 / 8.3             |
| descent arc         | 514, 648            | 15.6      | 31.5 | 0.50       | 48%     | 13756–392087           | 1.7 / 3.7 / 132.6           |
| approach and flare  | 441, 648            | 138.5     | 77.3 | 1.79       | 25%     | 369–20079              | 2.0 / 3.7 / 199.0           |
| low fast pass       | 159, 173            | 90.9      | 78.6 | 1.16       | 0%      | 300–300                | 1.0 / 2.0 / 60.6            |
| slowdown            | 110, 160            | 79.3      | 66.4 | 1.19       | 0%      | 175–272                | 0.7 / 1.5 / 106.9           |
| vertical descent    | 76, 103             | 0.0       | 23.3 | 0.00       | 0%      | 2–200                  | 0.4 / 0.5 / 1.2             |
| hover and touchdown | 67, 67              | 0.0       | 0.0  | 0.00       | 0%      | 1–2                    | 0.4 / 0.4 / 1.1             |
