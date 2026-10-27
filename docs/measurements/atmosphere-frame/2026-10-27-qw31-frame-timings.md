# Photorealistic frame timings: QW31, RTX 3080, 2026-10-27 (R08.T11.d)

- **Runs:** main's quiet window QW31, two timed runs back to back, each holding the heavy-test lock
  and the GPU lock (152 s each) in a scope capped at 8 GB and `CPUQuota` 1200%.
  - Run 1: 16:55:11–16:57:43 local (21:55:11 UTC), the 1-minute load 1.22 at the start and 1.20 at
    the end.
  - Run 2: 16:59:01–17:01:33 local, the load 0.34 at the start and 1.30 at the end. It was taken to
    retake run 1's two stalled rows (below), by main's word.
- **Machine:** AMD Ryzen 7 3700X, 16 threads, 32 GB; NVIDIA GeForce RTX 3080 10 GiB, Vulkan; Linux.
  The owner's desktop shares the GPU.
- **Clocks** (`nvidia-smi`, every 100 ms).
  - Run 1: P0 in 1,412 of 1,490 samples. While busy (utilisation ≥ 90%) the graphics clock's
    median was 1,965 of 2,115 MHz, with 5 busy samples under 1,800 MHz, all at 16:55:24, before the
    timed parts. The memory clock was 9,501 MHz throughout P0, and the GPU at most 67 °C.
  - Run 2: P0 in 1,397 of 1,487; the busy median 1,950 MHz; at most 68 °C.
- **Tree:** `r08-t11d-fix` at abea9b3e, the client built from it. Its code is the landed ef712615
  (R08.T11.d's second series on R01.T15's c183b046), diff-equal.
- **Command:** as QW29's (`2026-10-27-qw29-frame-timings.md`): the smoke page's part 1 with GPU
  timing, hidden and offscreen (the window never shown), on the client's Vulkan switches with the
  timestamp quantization lifted, a fresh `--user-data-dir`.
- **The group:** `R08.T11 the photorealistic frame's timings`, part 1 152.9 and 152.6 s. The
  group's parts took (D) 3.1 s, (A) 68.4 s, (B) 17.5 s, (C) 1.9–2.0 s, (G) 4.7–4.8 s,
  (H) 1.0–1.1 s, (I) 13.1–13.2 s, (J) 24.7–24.9 s, (E) 0.2 s and (F) 0.5–0.6 s. Every check of
  the group passed in both runs and no timed frame was dropped. The part's one failure is
  R07.T16.f's spatial strokes, a hardware run's known failure.
- **What changed since QW29** (R08.T11.d and R07.T8.g):
  - A view's march runs over the air's area alone (`marchArea.ts`; the `area` cover), the tile
    cull being off.
  - The march is specialised for a body of one slice and one band under a medium with no layer
    edge (R05's and the measured Earth on WGS 84, both class maps). The slab and `TEST SATURN` keep
    the general march.
  - R07.T8.g's near-limb band landed too (its hoisting did not).
- **QW30's rows:** (J) `TEST SATURN` under the setting's four sets, x2p and x4 added. Its giants
  stand at 20 of their radii from it (`SATURN_GIANT_DISTANCE_M`), out of every camera's view. (I)
  adds x2p on both class maps, and the generated palette is judged.
- **Figures:** ms, p95 (median) of the frame's GPU pass sum on high at 1920 × 1080. "atmosphere"
  is the `atmosphere view` passes and the composite. The raw check lines of both runs, with every
  timed frame's sum in timed order, are in `2026-10-27-qw31-frame-timings.txt` beside this file,
  each run under its own `#` line.

## The external stalls, and how the two runs are judged

Each run had a stall that is not the frame: a plateau of 18 to 27 consecutive frames of one or two
rows, stepping down at constant multiples of the row's median before it returned. The clocks show
no drop at the time (P0, about 1,960 MHz busy).

- In run 1, the measured Earth's 10⁷ m x2 ran 49 → 30.5 → 21 ms, then its median 12.0. R05's
  Earth's 1.54 × 10⁷ m x4 ran 39.8 → 24.7 → 17.7 ms, then 9.7.
- In run 2, R05's Earth's 1.54 × 10⁷ m x2p ran 39 → 23.8 ms; its x2, 16.5 ms for 6 frames; and the
  measured Earth's 1.54 × 10⁷ m x2, 34 → 22 ms.
- Each stalled row is clean in the other run. That makes them an external GPU stall, which is
  recorded and not judged.

The rule (main, 2026-10-27):

- A row is stalled in a run if at least 5 consecutive frames run over 1.5 times its median. A row
  stalled in one run is judged from the other.
- Every other row is judged on the worse p95 of the two runs.
- That is stricter than judging run 2 alone. Single-frame spikes are tails, judged as they fall:
  for example, the slab's 1.54 × 10⁷ m x2 in run 2, with six spikes of 15.4–18.0 ms over 120
  frames.

## The verdict (`decision-r08-t11-frame.md` §2: the target 15.67 ms p95, the blocker 16.67 ms)

Of the 92 judged high rows, 85 meet 15.67 ms, 5 are over it and at most 16.67 ms, and 2 are over
16.67 ms, the blocker:

| Medium             | Camera       | Set | Run 1         | Run 2         | Judged | QW29's p95 | Verdict    |
| ------------------ | ------------ | --- | ------------- | ------------- | ------ | ---------- | ---------- |
| the slab Earth     | 10⁷ m        | x4  | 16.73 (16.64) | 16.76 (16.52) | 16.76  | 17.66      | over 16.67 |
| `TEST SATURN`      | 9.45 × 10⁷ m | x4  | 16.69 (16.35) | 16.58 (16.29) | 16.69  | -          | over 16.67 |
| the slab Earth     | 400 km       | x4  | 16.01 (15.61) | 15.66 (15.56) | 16.01  | 16.47      | over 15.67 |
| the slab Earth     | 10⁷ m        | x2  | 15.73 (15.63) | 15.72 (15.60) | 15.73  | 18.35      | over 15.67 |
| the slab Earth     | 1.54 × 10⁷ m | x2  | 13.91 (13.77) | 16.28 (13.70) | 16.28  | 14.63      | over 15.67 |
| the measured Earth | 10⁷ m        | x4  | 15.82 (12.92) | 15.93 (12.83) | 15.93  | 19.58      | over 15.67 |
| `TEST SATURN`      | 9.45 × 10⁷ m | x2  | 15.81 (15.52) | 15.59 (15.49) | 15.81  | 16.41      | over 15.67 |

- Both rows over 16.67 ms are x4. Both media keep the general march: the slab has layer edges, and
  `TEST SATURN`'s figure has 5 slices and 4 bands. Their medians are 16.29–16.64 ms. No x2 or x2p
  row is over 16.67 ms. So under §7 this is R08.T11.f's case.
- The brief's binding rows:
  - the slab's 10⁷ m x4: 16.76, over 16.67;
  - R10's smoke class map's x4 rows: 13.44 and 14.82, which meet, against QW29's 17.33 and 18.33;
  - the measured Earth's 10⁷ m x4: 15.93 by its tails, its median 12.83–12.92, against QW29's
    19.58;
  - the slab's 10⁷ m x2: 15.73, its median 15.60–15.63.
- Both class maps meet 15.67 ms at every row in both runs. The generated palette's worst row is
  10⁷ m x4, 15.30 (15.14). Its x2 and x2p rows are at most 14.23, so nothing goes to main from
  item 8.
- `TEST SATURN`'s x2p rows meet everywhere, at most 15.60 at 9.45 × 10⁷ m. Its x4 rows meet
  everywhere but 9.45 × 10⁷ m, and are otherwise at most 15.29.
- R05's Earth meets everywhere, at most 13.53 ms. The measured Earth's rows other than 10⁷ m x4
  are at most 14.98.

## The rows, judged and recorded

Each run's p95 (median), the p95 judged and its run, QW29's p95, and the judged run's medians of
the split (ms). "stall" marks a run's stalled row.

### R05's Earth

| camera   | set | run 1              | run 2              | judged        | QW29 p95 | verdict | atmosphere | disc cells | bodies | discs |
| -------- | --- | ------------------ | ------------------ | ------------- | -------- | ------- | ---------- | ---------- | ------ | ----- |
| 400 km   | x1  | 7.01 (6.61)        | 6.95 (6.58)        | 7.01 (run 1)  | 9.10     | meets   | 3.40       | 0.77       | 2.03   | 0.06  |
| 400 km   | x2  | 8.94 (8.86)        | 8.82 (8.75)        | 8.94 (run 1)  | 11.58    | meets   | 4.98       | 0.96       | 2.51   | 0.06  |
| 400 km   | x2p | 8.28 (8.21)        | 8.25 (8.17)        | 8.28 (run 1)  | 10.93    | meets   | 5.01       | 0.77       | 2.03   | 0.06  |
| 400 km   | x4  | 9.69 (9.63)        | 9.57 (9.50)        | 9.69 (run 1)  | 12.91    | meets   | 5.05       | 1.14       | 3.06   | 0.06  |
| 1e7 m    | x1  | 7.66 (7.39)        | 8.93 (7.34)        | 8.93 (run 2)  | 9.69     | meets   | 4.27       | 0.93       | 1.77   | 0.02  |
| 1e7 m    | x2  | 12.50 (9.98)       | 12.47 (9.87)       | 12.50 (run 1) | 12.99    | meets   | 6.28       | 1.15       | 2.20   | 0.02  |
| 1e7 m    | x2p | 12.32 (9.78)       | 9.79 (9.65)        | 12.32 (run 1) | 12.81    | meets   | 6.30       | 0.98       | 2.14   | 0.02  |
| 1e7 m    | x4  | 10.98 (10.87)      | 13.53 (10.79)      | 13.53 (run 2) | 14.13    | meets   | 6.27       | 1.33       | 2.84   | 0.03  |
| 1.54e7 m | x1  | 6.67 (6.56)        | 6.66 (6.55)        | 6.67 (run 1)  | 8.86     | meets   | 3.80       | 0.86       | 1.54   | 0.03  |
| 1.54e7 m | x2  | 10.99 (8.79)       | 16.49 (8.84) stall | 10.99 (run 1) | 13.74    | meets   | 5.60       | 1.03       | 1.79   | 0.03  |
| 1.54e7 m | x2p | 9.03 (8.81)        | 39.09 (8.82) stall | 9.03 (run 1)  | 11.71    | meets   | 5.67       | 0.94       | 1.87   | 0.03  |
| 1.54e7 m | x4  | 39.81 (9.75) stall | 9.83 (9.62)        | 9.83 (run 2)  | 13.54    | meets   | 5.65       | 1.23       | 2.38   | 0.03  |
| 2.55e7 m | x1  | 4.89 (4.75)        | 4.85 (4.73)        | 4.89 (run 1)  | 6.61     | meets   | 2.79       | 0.66       | 0.95   | 0.02  |
| 2.55e7 m | x2  | 6.38 (6.23)        | 6.38 (6.22)        | 6.38 (run 1)  | 8.49     | meets   | 4.06       | 0.79       | 1.04   | 0.02  |
| 2.55e7 m | x2p | 6.59 (6.36)        | 6.45 (6.31)        | 6.59 (run 1)  | 8.79     | meets   | 4.09       | 0.75       | 1.17   | 0.02  |
| 2.55e7 m | x4  | 7.08 (6.89)        | 7.02 (6.86)        | 7.08 (run 1)  | 9.55     | meets   | 4.07       | 0.99       | 1.48   | 0.02  |
| 1e8 m    | x1  | 2.27 (2.23)        | 2.27 (2.21)        | 2.27 (run 2)  | 3.24     | meets   | 1.44       | 0.27       | 0.17   | 0.01  |
| 1e8 m    | x2  | 2.92 (2.86)        | 2.89 (2.84)        | 2.92 (run 1)  | 4.16     | meets   | 2.03       | 0.31       | 0.19   | 0.01  |
| 1e8 m    | x2p | 3.01 (2.95)        | 2.98 (2.92)        | 3.01 (run 1)  | 4.19     | meets   | 2.04       | 0.35       | 0.23   | 0.01  |
| 1e8 m    | x4  | 3.17 (3.12)        | 3.20 (3.11)        | 3.20 (run 2)  | 5.57     | meets   | 2.06       | 0.44       | 0.27   | 0.01  |

### the measured Earth

| camera   | set | run 1               | run 2               | judged        | QW29 p95 | verdict    | atmosphere | disc cells | bodies | discs |
| -------- | --- | ------------------- | ------------------- | ------------- | -------- | ---------- | ---------- | ---------- | ------ | ----- |
| 400 km   | x1  | 7.68 (7.64)         | 7.88 (7.59)         | 7.88 (run 2)  | 11.43    | meets      | 4.40       | 0.77       | 2.02   | 0.06  |
| 400 km   | x2  | 10.66 (10.50)       | 10.51 (10.42)       | 10.66 (run 1) | 14.56    | meets      | 6.55       | 0.97       | 2.57   | 0.06  |
| 400 km   | x2p | 9.30 (9.20)         | 9.28 (9.19)         | 9.30 (run 1)  | 12.41    | meets      | 6.00       | 0.78       | 2.05   | 0.06  |
| 400 km   | x4  | 11.48 (11.34)       | 11.37 (11.26)       | 11.48 (run 1) | 14.67    | meets      | 6.66       | 1.16       | 3.15   | 0.06  |
| 1e7 m    | x1  | 8.57 (8.48)         | 10.70 (8.49)        | 10.70 (run 2) | 12.13    | meets      | 5.41       | 0.93       | 1.77   | 0.02  |
| 1e7 m    | x2  | 48.90 (12.01) stall | 14.98 (11.81)       | 14.98 (run 2) | 14.99    | meets      | 8.13       | 1.16       | 2.17   | 0.02  |
| 1e7 m    | x2p | 11.00 (10.84)       | 10.86 (10.77)       | 11.00 (run 1) | 14.90    | meets      | 7.32       | 0.99       | 2.20   | 0.02  |
| 1e7 m    | x4  | 15.82 (12.92)       | 15.93 (12.83)       | 15.93 (run 2) | 19.58    | over 15.67 | 8.19       | 1.35       | 2.93   | 0.02  |
| 1.54e7 m | x1  | 7.67 (7.54)         | 7.65 (7.58)         | 7.67 (run 1)  | 9.84     | meets      | 4.78       | 0.86       | 1.55   | 0.03  |
| 1.54e7 m | x2  | 12.80 (10.46)       | 34.29 (10.86) stall | 12.80 (run 1) | 15.43    | meets      | 7.24       | 1.04       | 1.82   | 0.03  |
| 1.54e7 m | x2p | 11.95 (9.76)        | 9.90 (9.72)         | 11.95 (run 1) | 12.65    | meets      | 6.56       | 0.96       | 1.90   | 0.03  |
| 1.54e7 m | x4  | 11.60 (11.40)       | 11.43 (11.31)       | 11.60 (run 1) | 15.20    | meets      | 7.27       | 1.26       | 2.52   | 0.03  |
| 2.55e7 m | x1  | 5.68 (5.54)         | 5.65 (5.53)         | 5.68 (run 1)  | 7.27     | meets      | 3.59       | 0.66       | 0.95   | 0.02  |
| 2.55e7 m | x2  | 7.66 (7.52)         | 7.70 (7.48)         | 7.70 (run 2)  | 9.93     | meets      | 5.32       | 0.80       | 1.03   | 0.02  |
| 2.55e7 m | x2p | 7.31 (7.15)         | 7.29 (7.11)         | 7.31 (run 1)  | 9.59     | meets      | 4.84       | 0.76       | 1.20   | 0.02  |
| 2.55e7 m | x4  | 8.56 (8.28)         | 8.39 (8.25)         | 8.56 (run 1)  | 10.99    | meets      | 5.39       | 1.03       | 1.52   | 0.02  |
| 1e8 m    | x1  | 2.69 (2.61)         | 2.65 (2.60)         | 2.69 (run 1)  | 3.38     | meets      | 1.82       | 0.28       | 0.18   | 0.01  |
| 1e8 m    | x2  | 3.58 (3.50)         | 3.56 (3.46)         | 3.58 (run 1)  | 4.48     | meets      | 2.66       | 0.32       | 0.19   | 0.01  |
| 1e8 m    | x2p | 3.43 (3.33)         | 3.42 (3.33)         | 3.43 (run 1)  | 4.41     | meets      | 2.40       | 0.36       | 0.24   | 0.01  |
| 1e8 m    | x4  | 3.88 (3.81)         | 3.89 (3.79)         | 3.89 (run 2)  | 5.20     | meets      | 2.69       | 0.47       | 0.29   | 0.01  |

### the slab Earth

| camera   | set | run 1         | run 2         | judged        | QW29 p95 | verdict    | atmosphere | disc cells | bodies | discs |
| -------- | --- | ------------- | ------------- | ------------- | -------- | ---------- | ---------- | ---------- | ------ | ----- |
| 400 km   | x1  | 10.77 (10.69) | 10.78 (10.70) | 10.78 (run 2) | 11.37    | meets      | 7.52       | 0.77       | 2.02   | 0.06  |
| 400 km   | x2  | 14.86 (14.80) | 14.88 (14.78) | 14.88 (run 2) | 15.35    | meets      | 10.97      | 0.97       | 2.45   | 0.06  |
| 400 km   | x2p | 13.52 (13.43) | 13.68 (13.47) | 13.68 (run 2) | 14.08    | meets      | 10.30      | 0.77       | 2.02   | 0.06  |
| 400 km   | x4  | 16.01 (15.61) | 15.66 (15.56) | 16.01 (run 1) | 16.47    | over 15.67 | 10.97      | 1.16       | 3.11   | 0.06  |
| 1e7 m    | x1  | 11.57 (11.28) | 11.34 (11.28) | 11.57 (run 1) | 11.92    | meets      | 8.19       | 0.93       | 1.80   | 0.02  |
| 1e7 m    | x2  | 15.73 (15.63) | 15.72 (15.60) | 15.73 (run 1) | 18.35    | over 15.67 | 11.88      | 1.17       | 2.24   | 0.02  |
| 1e7 m    | x2p | 14.80 (14.71) | 14.84 (14.70) | 14.84 (run 2) | 15.57    | meets      | 11.22      | 0.99       | 2.14   | 0.02  |
| 1e7 m    | x4  | 16.73 (16.64) | 16.76 (16.52) | 16.76 (run 2) | 17.66    | OVER 16.67 | 11.89      | 1.35       | 2.92   | 0.02  |
| 1.54e7 m | x1  | 10.25 (10.09) | 10.14 (10.05) | 10.25 (run 1) | 10.81    | meets      | 7.32       | 0.86       | 1.56   | 0.03  |
| 1.54e7 m | x2  | 13.91 (13.77) | 16.28 (13.70) | 16.28 (run 2) | 14.63    | over 15.67 | 10.54      | 1.05       | 1.77   | 0.03  |
| 1.54e7 m | x2p | 14.52 (13.11) | 13.26 (13.05) | 14.52 (run 1) | 13.94    | meets      | 9.91       | 0.95       | 1.89   | 0.03  |
| 1.54e7 m | x4  | 14.92 (14.74) | 14.86 (14.64) | 14.92 (run 1) | 16.32    | meets      | 10.60      | 1.26       | 2.51   | 0.03  |
| 2.55e7 m | x1  | 7.34 (7.21)   | 7.26 (7.18)   | 7.34 (run 1)  | 7.92     | meets      | 5.26       | 0.66       | 0.96   | 0.02  |
| 2.55e7 m | x2  | 9.92 (9.76)   | 9.88 (9.71)   | 9.92 (run 1)  | 10.70    | meets      | 7.57       | 0.80       | 1.05   | 0.02  |
| 2.55e7 m | x2p | 9.67 (9.43)   | 9.61 (9.37)   | 9.67 (run 1)  | 10.47    | meets      | 7.12       | 0.76       | 1.20   | 0.02  |
| 2.55e7 m | x4  | 10.65 (10.50) | 10.70 (10.48) | 10.70 (run 2) | 11.83    | meets      | 7.60       | 1.04       | 1.49   | 0.02  |
| 1e8 m    | x1  | 3.03 (2.97)   | 3.02 (2.94)   | 3.03 (run 1)  | 3.54     | meets      | 2.18       | 0.28       | 0.18   | 0.01  |
| 1e8 m    | x2  | 3.97 (3.91)   | 3.95 (3.88)   | 3.97 (run 1)  | 4.56     | meets      | 3.07       | 0.32       | 0.20   | 0.01  |
| 1e8 m    | x2p | 3.86 (3.81)   | 3.86 (3.80)   | 3.86 (run 1)  | 4.58     | meets      | 2.87       | 0.36       | 0.24   | 0.01  |
| 1e8 m    | x4  | 4.28 (4.19)   | 4.28 (4.20)   | 4.28 (run 1)  | 5.30     | meets      | 3.09       | 0.48       | 0.30   | 0.01  |

### the measured Earth under R10's smoke class map

| camera | set | run 1         | run 2         | judged        | QW29 p95 | verdict | atmosphere | disc cells | bodies | discs |
| ------ | --- | ------------- | ------------- | ------------- | -------- | ------- | ---------- | ---------- | ------ | ----- |
| 400 km | x2  | 12.24 (12.15) | 11.98 (11.90) | 12.24 (run 1) | 15.08    | meets   | 6.68       | 1.38       | 3.70   | 0.06  |
| 400 km | x2p | 10.90 (10.81) | 10.77 (10.69) | 10.90 (run 1) | -        | meets   | 6.04       | 1.17       | 3.21   | 0.06  |
| 400 km | x4  | 13.44 (13.36) | 13.40 (13.30) | 13.44 (run 1) | 17.33    | meets   | 6.71       | 1.59       | 4.68   | 0.06  |
| 1e7 m  | x2  | 13.47 (13.24) | 13.21 (13.03) | 13.47 (run 1) | 16.45    | meets   | 8.23       | 1.52       | 3.14   | 0.03  |
| 1e7 m  | x2p | 12.49 (12.38) | 12.40 (12.28) | 12.49 (run 1) | -        | meets   | 7.37       | 1.34       | 3.32   | 0.02  |
| 1e7 m  | x4  | 14.82 (14.49) | 14.60 (14.44) | 14.82 (run 1) | 18.33    | meets   | 8.23       | 1.71       | 4.19   | 0.02  |

### the measured Earth under a generated palette's class map

| camera | set | run 1         | run 2         | judged        | QW29 p95 | verdict | atmosphere | disc cells | bodies | discs |
| ------ | --- | ------------- | ------------- | ------------- | -------- | ------- | ---------- | ---------- | ------ | ----- |
| 400 km | x2  | 12.61 (12.52) | 12.49 (12.36) | 12.61 (run 1) | 16.49    | meets   | 6.68       | 1.61       | 3.84   | 0.06  |
| 400 km | x2p | 11.32 (11.22) | 11.10 (11.02) | 11.32 (run 1) | -        | meets   | 6.04       | 1.40       | 3.38   | 0.06  |
| 400 km | x4  | 13.57 (13.45) | 13.32 (13.21) | 13.57 (run 1) | 17.87    | meets   | 6.72       | 1.83       | 4.51   | 0.06  |
| 1e7 m  | x2  | 14.23 (13.94) | 13.87 (13.76) | 14.23 (run 1) | 18.27    | meets   | 8.23       | 1.98       | 3.36   | 0.03  |
| 1e7 m  | x2p | 12.99 (12.89) | 12.85 (12.71) | 12.99 (run 1) | -        | meets   | 7.37       | 1.81       | 3.35   | 0.02  |
| 1e7 m  | x4  | 15.30 (15.14) | 15.09 (14.95) | 15.30 (run 1) | 20.56    | meets   | 8.26       | 2.27       | 4.25   | 0.02  |

### TEST SATURN

| camera   | set | run 1         | run 2         | judged        | QW29 p95 | verdict    | atmosphere | disc cells | bodies | discs |
| -------- | --- | ------------- | ------------- | ------------- | -------- | ---------- | ---------- | ---------- | ------ | ----- |
| 6.40e7 m | x1  | 8.28 (8.21)   | 8.27 (8.20)   | 8.28 (run 1)  | 8.90     | meets      | 4.99       | 0.80       | 2.03   | 0.06  |
| 6.40e7 m | x2  | 12.90 (11.99) | 11.97 (11.89) | 12.90 (run 1) | 12.62    | meets      | 8.03       | 1.01       | 2.56   | 0.06  |
| 6.40e7 m | x2p | 11.71 (11.63) | 11.72 (11.63) | 11.72 (run 2) | -        | meets      | 8.43       | 0.80       | 2.00   | 0.06  |
| 6.40e7 m | x4  | 12.62 (12.54) | 12.58 (12.50) | 12.62 (run 1) | -        | meets      | 8.06       | 1.11       | 2.97   | 0.06  |
| 9.45e7 m | x1  | 10.57 (10.47) | 10.57 (10.49) | 10.57 (run 2) | 11.36    | meets      | 7.15       | 1.01       | 1.97   | 0.03  |
| 9.45e7 m | x2  | 15.81 (15.52) | 15.59 (15.49) | 15.81 (run 1) | 16.41    | over 15.67 | 11.40      | 1.26       | 2.51   | 0.03  |
| 9.45e7 m | x2p | 15.60 (15.45) | 15.59 (15.48) | 15.60 (run 1) | -        | meets      | 11.73      | 1.05       | 2.31   | 0.03  |
| 9.45e7 m | x4  | 16.69 (16.35) | 16.58 (16.29) | 16.69 (run 1) | -        | OVER 16.67 | 11.40      | 1.43       | 3.19   | 0.03  |
| 1.46e8 m | x1  | 8.50 (8.37)   | 8.49 (8.35)   | 8.50 (run 1)  | 9.32     | meets      | 5.77       | 0.82       | 1.42   | 0.03  |
| 1.46e8 m | x2  | 12.45 (12.26) | 12.40 (12.23) | 12.45 (run 1) | 13.37    | meets      | 9.24       | 0.98       | 1.68   | 0.03  |
| 1.46e8 m | x2p | 12.80 (12.35) | 12.50 (12.33) | 12.80 (run 1) | -        | meets      | 9.36       | 0.91       | 1.72   | 0.03  |
| 1.46e8 m | x4  | 13.30 (13.09) | 15.29 (13.08) | 15.29 (run 2) | -        | meets      | 9.32       | 1.21       | 2.18   | 0.03  |
| 2.41e8 m | x1  | 6.41 (6.21)   | 6.37 (6.22)   | 6.41 (run 1)  | 7.18     | meets      | 4.36       | 0.63       | 0.88   | 0.02  |
| 2.41e8 m | x2  | 9.28 (9.05)   | 9.24 (9.07)   | 9.28 (run 1)  | 10.25    | meets      | 6.97       | 0.77       | 0.97   | 0.02  |
| 2.41e8 m | x2p | 9.46 (9.24)   | 9.38 (9.25)   | 9.46 (run 1)  | -        | meets      | 7.06       | 0.74       | 1.11   | 0.02  |
| 2.41e8 m | x4  | 9.86 (9.72)   | 9.98 (9.71)   | 9.98 (run 2)  | -        | meets      | 6.99       | 1.01       | 1.36   | 0.02  |
| 9.45e8 m | x1  | 3.06 (2.93)   | 3.04 (2.93)   | 3.06 (run 1)  | 3.79     | meets      | 2.17       | 0.26       | 0.17   | 0.01  |
| 9.45e8 m | x2  | 4.34 (4.11)   | 4.32 (4.10)   | 4.34 (run 1)  | 5.14     | meets      | 3.29       | 0.30       | 0.19   | 0.01  |
| 9.45e8 m | x2p | 4.36 (4.24)   | 4.39 (4.26)   | 4.39 (run 2)  | -        | meets      | 3.37       | 0.34       | 0.23   | 0.01  |
| 9.45e8 m | x4  | 4.73 (4.40)   | 4.59 (4.39)   | 4.73 (run 1)  | -        | meets      | 3.33       | 0.46       | 0.27   | 0.01  |

## Against QW29 (medians, ms)

- _The atmosphere at x2_ from 400 km, 10⁷ m and 1.54 × 10⁷ m. The specialised march's media and
  the general march's differ:
  - the measured Earth: 6.55, 8.13 and 7.24, against QW29's 9.51, 10.88 and 9.70;
  - R05's Earth: 4.98, 6.28 and 5.60, against 7.46, 8.94 and 8.03;
  - the slab Earth (general): 10.97, 11.88 and 10.54, against 11.22, 12.24 and 10.91;
  - `TEST SATURN` at 9.45 × 10⁷ m, the Earth's 10⁷ m (general): 11.40, against 11.93. Its
    atmosphere is now 82% over R05's Earth's at that frame, against QW29's 33%, since R05's takes
    the specialised march.
- _The face and the cells_ are R07.T8.g's: `bodies` 1.5–3.2 ms and `disc cells` 0.8–1.4 ms at the
  plain Earths' near rows. The class maps add 1.0–1.5 ms (R10's fixture) and 1.2–1.4 ms (the
  generated palette) on `bodies` over the plain measured Earth.
- _The atmosphere alone_ (B) is drawn with its own kernels, and so on the reference path: the
  whole frame, by the general march. The measured Earth at x2 takes 9.98, 11.26 and 10.09 ms from
  400 km, 10⁷ m and 1.54 × 10⁷ m, against QW29's 9.67, 11.01 and 9.84, so 0.25–0.31 ms more for
  the same march. The cause is not separated here: the reference path also runs the composite's
  area tests and the footprint entry's texel offset. (B) is not judged.
- _Small air bodies_ (C), the measured Earth with its air: 1.76, 1.84 and 1.96 ms at 4, 10 and
  32 px in run 1, and 1.89, 1.98 and 2.10 in run 2, against QW29's 2.53–2.73. Without its air it
  takes 0.37 ms.
- _Three in one view_ (H): 6.98 ms (run 1) and 7.69 ms (run 2), with the atmosphere at 6.40 and
  7.11, against QW29's 9.98 and 9.35.
- (C) and (H) differ by 7–10% between the runs, where the judged rows differ by 0–2%. Each run's
  60 frames are steady (p95 within 1% of the median), so this is a run-to-run difference, not a
  tail.

## Low, p95 ms, and re-projected to the UHD 620 (R07's Gen9 cycles a shade)

The factor is (3,000 to 7,000 Gen9 cycles a shade ÷ the RTX 3080's 3,500,
`decision-r07-small-disc-cost.md`) × (the run's busy clock ÷ the UHD 620's): 1.53–3.57 at its
1.1 GHz and 5.61–13.10 at its 300 MHz floor (run 1, 1,965 MHz). It scales a dependent chain's
latency (`decision-r08-t11-frame.md` §5). These are estimates; the owner's pass settles them.

- R05's Earth: p95 1.44–6.07 ms in run 1 and 1.05–5.27 in run 2.
- the measured Earth: 1.20–3.83 and 1.20–6.34.
- the slab Earth: 1.33–4.08 and 1.33–4.02.
- `TEST SATURN`: 1.44–4.16 and 1.44–4.30.
- Re-projected, the worst of run 1 comes to 21.7 ms at 1.1 GHz and 79.6 ms at 300 MHz (R05's
  Earth's 6.07); without it, to at most 14.9 and 54.5 ms. The low maxima are single rows' tails:
  R05's 6.07 and the measured Earth's 6.34 are each one run's, with the other run at 5.27 and 3.83.

## Tails (frames over 1.2 times their combination's median), run 1 and run 2

- the frame on high (A): 182 and 181 of 3,600 (5.06% and 5.03%), against QW29's 1.86%. The
  stalls' plateaus account for 52 and 42 of them.
- the frame on low (A): 130 and 167 of 3,600, against 4.
- the atmosphere alone (B): 25 and 25 of 1,800; the class maps (I): 6 and 7 of 720; `TEST SATURN`
  (J): 16 and 15 of 2,400. Small bodies (C), three in one view (H) and the bare scene (G): none.
- Beyond the stalls, the high tails stand at about 3.6–3.8% of frames, against QW29's 1.86%. Not
  separated here.
