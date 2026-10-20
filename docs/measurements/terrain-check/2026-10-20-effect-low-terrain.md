# Terrain run: effect, low, 2026-10-20

- **Overall:** pass
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; governor schedutil; load average 1.45, 4.28, 9.99 (provisional: load average 1.45 at the start; a shared machine; a hidden run: no presentation)
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, timer full, setting low, window hidden 1286 × 724 DIP at a device-pixel ratio of 1
- **Captures (not committed):** `target/views-check/terrain/captures-20261020T022520`

## The hidden run's checks

| Check                                                                   | Verdict | Reading                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ----------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The setting's phases ran and held their views                           | pass    | terrain-alone, terrain-wireframe, terrain-loss                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Every frame drawn (no frame a view was due missed, outside the restore) | pass    | missed frames, and the longest time between draws (provisional): terrain-alone PRIMARY 0, 1317 ms; terrain-wireframe PRIMARY 0, 117 ms, INSTRUMENT 1 0, 117 ms; terrain-loss PRIMARY 0, 67 ms                                                                                                                                                                                                                                                                   |
| Every frame finite (the photorealistic scene targets read back)         | pass    | 686 read-backs                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| The terrain drawn once handed over                                      | pass    | terrain-alone PRIMARY 5533 of 5537 handed-over draws with a patch in view (327 with none), first 133 ms after the hand-over; terrain-wireframe PRIMARY 5574 of 5574 handed-over draws with a patch in view (1291 with none), first 0 ms after the hand-over, INSTRUMENT 1 0 of 0 handed-over draws with a patch in view (0 with none); terrain-loss PRIMARY 1963 of 1964 handed-over draws with a patch in view (36 with none), first 33 ms after the hand-over |
| One forced loss, then one restore, and no other fault                   | pass    | excluded: device-lost (destroyed): Device was destroyed.; 1 restores; 0 other faults; 0 GPU-process exits                                                                                                                                                                                                                                                                                                                                                       |
| The loss mid-stream, and after it no field asked again                  | pass    | destroyed at 121.6 s of the script, 2 blocks to come, 6 bakes in flight; after: 0 asks, 0 inits, 6 chunks for 2 blocks to 3 workers; the mask 24 cells before, 29 after the restore, 48 at the end of 48                                                                                                                                                                                                                                                        |
| The session torn down                                                   | pass    | disposed; 0 of 6 workers left; the session's textures 0 B; the terrain's GPU bytes left: none                                                                                                                                                                                                                                                                                                                                                                   |
| No pixel drawn by both surfaces on the datum                            | —       | held by T16.c's part-6 smoke group, `R10.T16 the frame`, on SwiftShader; not probed here                                                                                                                                                                                                                                                                                                                                                                        |

The hand-over: range 5311 km, crossed at 66.2 s of the script; the primary handed over at 68.1 s. Console errors: 0.

## Phases (provisional)

Durations as p50 / p95 / max ms.

| Phase             | Held | Window (s) | Script (s)    | Animation frames     | Main thread         | GPU frame          | Patches drawn (mean, max) | Bakes (a second) | Field posts (init, chunk) | Asks, blocks |
| ----------------- | ---- | ---------- | ------------- | -------------------- | ------------------- | ------------------ | ------------------------- | ---------------- | ------------------------- | ------------ |
| terrain-alone     | yes  | 264.8      | 2.7 → 265.1   | 16.7 / 16.8 / 1300.0 | 1.20 / 7.50 / 193.9 | 9.00 / 12.7 / 19.9 | 144.0, 211                | 3963 (15.0)      | 0, 21                     | 1, 7         |
| terrain-wireframe | yes  | 297.6      | 267.4 → 265.0 | 16.7 / 16.8 / 100.0  | 2.60 / 15.1 / 91.3  | 9.19 / 12.1 / 19.8 | 95.7, 211                 | 3178 (10.7)      | 0, 0                      | 0, 0         |
| terrain-loss      | yes  | 134.1      | 0.7 → 134.7   | 16.7 / 16.7 / 50.1   | 1.50 / 8.00 / 57.1  | 5.17 / 13.4 / 19.9 | 148.9, 211                | 1029 (7.7)       | 3, 21                     | 1, 8         |

## Views (provisional)

| Phase             | View         | Style          | Canvas (px) | Draws | GPU (ms)           | Submit (ms)         | Hand-over (script s) | Terrain draws | Read-backs |
| ----------------- | ------------ | -------------- | ----------- | ----- | ------------------ | ------------------- | -------------------- | ------------- | ---------- |
| terrain-alone     | PRIMARY      | photorealistic | 822 × 544   | 7811  | 9.00 / 12.7 / 19.9 | 0.80 / 1.40 / 187.3 | 68.1                 | 5533 of 5537  | 260        |
| terrain-wireframe | PRIMARY      | photorealistic | 822 × 544   | 8899  | 9.17 / 12.1 / 19.8 | 0.80 / 3.50 / 16.0  | 67.9                 | 5574 of 5574  | 294        |
| terrain-wireframe | INSTRUMENT 1 | wireframe      | 240 × 180   | 8899  | 0.02 / 0.05 / 1.05 | 0.20 / 0.50 / 9.00  | —                    | 0 of 0        | 0          |
| terrain-loss      | PRIMARY      | photorealistic | 822 × 544   | 4016  | 5.17 / 13.4 / 19.9 | 0.70 / 3.60 / 13.9  | 67.9                 | 1963 of 1964  | 132        |

## The session's memory at each phase's end (provisional)

| Phase             | Workers' fields | Kept blocks | Session's textures        | Terrain's GPU bytes alive                                         | Their peak                                                        |
| ----------------- | --------------- | ----------- | ------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------- |
| terrain-alone     | 3.34 MiB        | 0.03 MiB    | coarse-field-gpu 0.12 MiB | coarse-field-gpu 0.12 MiB, height-cache 65.14 MiB, other 0.29 MiB | coarse-field-gpu 0.12 MiB, height-cache 65.14 MiB, other 0.29 MiB |
| terrain-wireframe | 3.34 MiB        | 0.03 MiB    | coarse-field-gpu 0.12 MiB | coarse-field-gpu 0.12 MiB, height-cache 65.14 MiB, other 0.29 MiB | coarse-field-gpu 0.12 MiB, height-cache 65.14 MiB, other 0.29 MiB |
| terrain-loss      | 3.34 MiB        | 0.03 MiB    | coarse-field-gpu 0.12 MiB | coarse-field-gpu 0.12 MiB, height-cache 65.14 MiB, other 0.29 MiB | coarse-field-gpu 0.12 MiB, height-cache 65.14 MiB, other 0.29 MiB |
