# Photorealistic frame timings with the thick read forced: QW32, RTX 3080, 2026-10-27 (R08.T14.c)

- **Runs:** main's quiet window QW32, two timed runs back to back. Each held the heavy-test lock
  and the GPU lock (192 s and 185 s) in a scope capped at 8 GB and `CPUQuota` 1200%.
  - Run 1: 17:52:06–17:55:18 local. The 1-minute load was 0.59 at the start and 1.46 at the end.
  - Run 2: 17:56:21–17:59:26 local, the load 0.58 at the start and 1.52 at the end. It was taken
    under QW31's stall rule (below), by main's word.
- **Machine:** AMD Ryzen 7 3700X, 16 threads, 32 GB; NVIDIA GeForce RTX 3080 10 GiB, Vulkan; Linux.
  The owner's desktop shares the GPU.
- **Clocks** (`nvidia-smi`, every 100 ms).
  - Run 1: P0 in 1,727 of 1,879 samples. The busy median (utilisation ≥ 90%) was 1,965 of
    2,115 MHz, with 5 busy samples under 1,800 MHz. The GPU reached at most 67 °C.
  - Run 2: P0 in 1,700 of 1,809, the busy median 1,965 MHz, none under 1,800, at most 68 °C.
- **Tree:** the lane `r08-t14c` on 77ef3787, with R08.T14.c's working tree, the client built from
  it.
- **Command:** as QW31's: the smoke page's part 1 with GPU timing (`--smoke-gpu-timing=1`), hidden
  and offscreen, on the client's Vulkan switches with the timestamp quantization lifted, and a
  fresh `--user-data-dir`. It adds `--smoke-thick-read=1`, R08.T14.c's smoke-only override; the
  route itself stays gated (`thick/routing.ts`'s `GATED_REGIMES` is empty).
- **What the override adds** (`smoke/frameTiming.ts`).
  - (K) is the measured Earth through the thick read. It has its own tables beside (A)'s, holding
    its thick bake: one band and two light slots, both under the reference Sun
    (`thick/keptBake.ts`, baked on the page in 2.6 s). It is drawn at (A)'s combinations on both
    settings.
  - (L) is the same tables under R10's smoke class map, at (I)'s rows.
  - The sky view, the aerial perspective and the march read the table (`hillaire.ts`'s
    `withThickRead`), and the analytic term reads below the horizon.
  - J_ms takes 2 × 270,336 B as `rgba16float` would hold it, 541,184 B allocated with the info
    rows. The measured Earth's per-planet row comes to 1,139,888 B in all.
- **The group:** `R08.T11 the photorealistic frame's timings`. Its parts took (D) 3.0 s, (K)'s bake
  2.6 s, (A) 67–68 s, (B) 17.4 s, (C) 1.9 s, (G) 4.7–4.8 s, (H) 1.0 s, (I) 13.1–13.2 s, (J)
  24.7–24.8 s, (K) 24.6–29.3 s, (L) 7.2–7.3 s, (E) 0.3 s and (F) 0.6 s. No timed frame was
  dropped. The part's one failure is R07.T16.f's spatial strokes, a hardware run's known failure.
- **Figures:** ms, p95 (median) of the frame's GPU pass sum on high at 1920 × 1080. The raw check
  lines of the measured Earth's and the judged class map's rows, thin and thick, from both runs,
  are in `2026-10-27-qw32-thick-read-frame-timings.txt` beside this file.

## The stalls, and how the two runs are judged

QW31's rule (main, 2026-10-27): a row is stalled in a run if at least 5 consecutive frames run over
1.5 times its median, and a stalled row is judged from the other run. Every other row is judged on
the worse p95 of the two.

- Stalled in run 1: the thin measured Earth's 10⁷ m x2 (24 frames; the same external plateau as
  QW31's), and the thick one's 400 km x1 (23 frames).
- Stalled in run 2: the thin measured Earth's 1.54 × 10⁷ m x4 (22 frames), and the thick one's
  10⁷ m x1 (6) and 10⁷ m x2 (13).

## The verdict (`decision-r08-t11-frame.md` §2: the target 15.67 ms p95, the blocker 16.67 ms)

Judged, thin against thick, p95 ms:

| Medium    | Camera       | Set | Thin  | Thick | Verdict (thick) | Δ atmosphere median |
| --------- | ------------ | --- | ----- | ----- | --------------- | ------------------- |
| measured  | 400 km       | x1  | 7.73  | 8.47  | meets           | +0.94               |
| measured  | 400 km       | x2  | 10.66 | 16.11 | over 15.67      | +1.58               |
| measured  | 400 km       | x2p | 9.30  | 10.12 | meets           | +0.82               |
| measured  | 400 km       | x4  | 11.43 | 13.06 | meets           | +1.56               |
| measured  | 10⁷ m        | x1  | 8.55  | 9.67  | meets           | +0.90               |
| measured  | 10⁷ m        | x2  | 12.06 | 13.85 | meets           | +1.82               |
| measured  | 10⁷ m        | x2p | 13.85 | 15.61 | meets           | +1.09               |
| measured  | 10⁷ m        | x4  | 15.97 | 14.85 | meets           | +1.82               |
| measured  | 1.54 × 10⁷ m | x1  | 7.75  | 10.46 | meets           | +0.83               |
| measured  | 1.54 × 10⁷ m | x2  | 12.89 | 15.22 | meets           | +1.49               |
| measured  | 1.54 × 10⁷ m | x2p | 11.98 | 11.05 | meets           | +1.06               |
| measured  | 1.54 × 10⁷ m | x4  | 11.67 | 13.13 | meets           | +1.35               |
| measured  | 2.55 × 10⁷ m | x4  | 8.49  | 9.53  | meets           | +0.96               |
| measured  | 10⁸ m        | x4  | 3.91  | 4.45  | meets           | +0.33               |
| class map | 400 km       | x2  | 12.66 | 17.51 | over 16.67      | +1.49               |
| class map | 400 km       | x2p | 10.94 | 11.70 | meets           | +0.76               |
| class map | 400 km       | x4  | 13.48 | 18.75 | over 16.67      | +1.48               |
| class map | 10⁷ m        | x2  | 13.34 | 18.79 | over 16.67      | +1.82               |
| class map | 10⁷ m        | x2p | 12.51 | 13.98 | meets           | +1.08               |
| class map | 10⁷ m        | x4  | 14.75 | 16.51 | over 15.67      | +1.81               |

The measured Earth's other rows, x1 to x2p at 2.55 × 10⁷ m and 10⁸ m, meet 15.67 ms thin and
thick.

- **The three rows over 16.67 ms are run 1's tails.** Their medians are steady, and their p95s come
  from a run of slow frames that is too short to be a stall.
  - In run 1, class map 400 km x2 had 14 of its 60 frames over 16.67 ms, 400 km x4 had 16 and
    10⁷ m x2 had 6.
  - In those frames the atmosphere pass's p95 rose to 12.0–13.8 ms, over a median of 8.1–9.9.
  - In run 2 the same rows are clean: 13.79 (13.66), 15.61 (14.90) and 15.14 (15.01), with
    atmosphere p95s of 8.3–10.1.
  - The same run-1 tail gives the measured Earth's 400 km x2 its 16.11; run 2 has 12.23.
- **The cost is the atmosphere pass's median.** The thick read adds +0.8 to +1.8 ms on high, and
  +1.5 to +1.8 at x2 and x4 at 400 km and 10⁷ m. The bodies, disc-cell and disc passes are
  unchanged, thin to thick, to 0.04 ms.
- **The row at the blocker on its median alone** is class map 10⁷ m x4. Its median is 16.22–16.32
  and its p95 16.45–16.51 in both runs. So R08.T14.d's routing does not meet the blocker as built,
  and T14.f's lever ("if T14.c's frame is over 15.67 ms") is triggered.
- **The levers, recorded and not built:**
  - The read finds its altitude row by five dependent loads for every source. Finding it once a
    sample would take the loads out of the source loop, but the thin text's loop leaves no place to
    do it without changing that text.
  - T14.f's summed table of m = 0..2, the bake's E and ΔJ_pol on one grid, is one read where a thin
    world makes Ψ_ms's and the correction's.
