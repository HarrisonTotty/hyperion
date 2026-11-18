# Descent spike: thewired, low, 2026-11-18 (notes)

Notes on `2026-11-18-thewired-low` and `2026-11-18-thewired-low-2`. They are the UHD 620's two
hidden low runs for R10.T14.b's sizes ([README](README.md), "The owner's UHD 620 runs";
decision-r10-t14b-constants). The notes also cover the other by-hand runs of the same session, which
have no results file of their own. The results files are as the spike wrote them, formatted by
Prettier.

## The runs

- **Commands.** The README's two commands, run through `descentSpike.sh` directly, the recipe's
  second step. `just build` ran first, on its own, so that its load had cleared before each run
  started:

  ```sh
  bash apps/hyperion/scripts/descentSpike.sh --setting low --world terrain-test --style photorealistic --sizes on --seed 7 --hidden
  bash apps/hyperion/scripts/descentSpike.sh --setting low --world terrain-test --style wireframe --sizes on --seed 7 --hidden
  ```

- **Machine.** `thewired`, the UHD 620 laptop of the roadmap's
  [Target hardware](../../agent/plans/rendering-and-planets/README.md#target-hardware):
  - a ThinkPad X1 Yoga 4th generation with an i7-8665U;
  - Intel UHD Graphics 620 (`8086:3ea0`) on Mesa 26.2.7's ANV over i915;
  - Linux 6.18.64-1-lts, under Xorg;
  - Electron 45.1.0 (Chromium 156.0.8078.49).
- **Power state.** On AC, at the machine's default profile, as R12.T8 asks.
  - `platform_profile` was `performance`, which TLP 1.11 sets on AC by default
    (`TLP_PROFILE_AC=PRF`; `tlp-stat -s` showed `performance/PRF`). `/etc/tlp.conf` does not
    override it.
  - `intel_pstate` ran the `powersave` governor at EPP `balance_performance`.
  - RAPL's PL1 changed during the session. At 21:54 UTC the MMIO zone (`intel-rapl-mmio:0`) held
    PL1 15 W and PL2 51 W. By 23:08 UTC its PL1 read 51 W. What raised it, and when, was not
    traced, so the PL1 these runs ran under is unknown. The spike's results do not record it;
    R12's run records do.
- **Nothing else ran.** No lane, build, test or local LLM.
- **Build.** `2fc93d05a`, a clean tree.
- **Starts on a quiet machine.** A script waited for the 1-minute load average under 1 and the
  package under 60 °C before each run:

  | Run            | Started (UTC) | Load at start | Package at start | Length  |
  | -------------- | ------------- | ------------- | ---------------- | ------- |
  | Photorealistic | 22:01:01      | 0.89          | 43 °C            | 1,302 s |
  | Wireframe      | 22:23:42      | 0.98          | 52 °C            | 1,279 s |

- **The label "not a quiet machine".** Both files carry it, from load averages of 3.77 and 1.44.
  - The client reads those after it has started, when its own start-up has already raised the load.
    Exactly when it reads them was not traced.
  - The runs began under 1, as the table shows.
  - The RTX 3080's run in a quiet window (`2026-10-21-effect-high-2`) reads 3.91 the same way.
  - So here the label does not show other work on the machine.
- **Shader cache.** The photorealistic run started cold, since no spike had run on this machine
  before. The wireframe run took the cache that the photorealistic run left.
- **Hidden.** No window was shown, so the criterion's frame rows are `not-measured`. The intervals
  are `requestAnimationFrame`'s, paced by Chromium's timer: an indication, never a verdict. The
  canvas was 805 × 433 px, as on the RTX 3080's runs (806 × 431).

## Two GPU hangs in the photorealistic run

- **What the kernel did.** It reset the render engine twice, at 22:20:29 and 22:21:24 UTC. Each time
  it logged `Resetting rcs0 for preemption time out` and `electron[…] context reset due to GPU
hang`, then `GPU HANG: ecode 9:1:84d77afc` and `9:1:85d7dfff`.
  - i915's preemption timeout on `rcs0` is 640 ms here, and its heartbeat interval 2,500 ms.
  - Mesa reported `VK_ERROR_DEVICE_LOST` for each hang.
  - For a moment Chromium could not create the canvas's shared-image textures.
  - The kernel saved its error state to `/sys/class/drm/card1/error`. That file is root-only, so
    it was not kept.
- **The results file does not record the device losses.** The run recovered and finished, but
  nothing in its file marks either loss.
  - Its two largest intervals are the likeliest traces of them: 5,266.6 ms in the slowdown and
    4,149.8 ms in the hover and touchdown.
  - Those intervals were not matched to the resets by time.
- **The likely cause is the `bodies` pass, R07's lit bodies.** It is the only pass whose slowest
  frames approach the 640 ms timeout. The heaviest passes:

  | Pass                   | Frames | p50, ms | p95, ms | p99, ms |
  | ---------------------- | ------ | ------- | ------- | ------- |
  | `bodies`               | 3,874  | 171.1   | 226.0   | 470.1   |
  | `disc cells`           | 3,469  | 31.8    | 36.2    | 38.3    |
  | `view:wireframe`       | 18,453 | 1.6     | 28.4    | 33.1    |
  | `discs`                | 3,878  | 10.5    | 15.4    | 16.4    |
  | `symbology`            | 3,878  | 9.4     | 11.4    | 40.1    |
  | `terrain cluster cull` | 3,019  | 1.3     | 1.4     | 3.2     |
  - The GPU pass sum is 249.0 ms at p95.
  - The criterion's terrain-and-atmosphere row is 1.92 ms at p95, against 18 ms.
  - The `bodies` figures are at the low setting, on the 805 × 433 canvas.

- **The GPU was not throttled.** It ran at full clock throughout: a median of 1,150 MHz of 1,150,
  read from i915's sysfs. So the time is the work itself.
- **The wireframe run had no hang.** Its `view:wireframe` pass is 2.2, 66.9 and 181.3 ms at p50,
  p95 and p99. Its pass sum is 67.5 ms at p95.

## Frame intervals (hidden, not judged)

p50 / p95 / p99 / maximum, in ms, from `requestAnimationFrame` (the low setting's frame is 33.3 ms):

| Segment             | Photorealistic                      | Wireframe                         |
| ------------------- | ----------------------------------- | --------------------------------- |
| Whole descent       | 16.7 / 266.7 / 816.6 / 5,266.6      | 16.7 / 116.6 / 200.0 / 2,833.2    |
| orbit coast         | 16.7 / 33.4 / 50.0 / 183.3          | 16.7 / 16.7 / 16.8 / 16.8         |
| descent arc         | 16.7 / 283.3 / 850.0 / 2,149.8      | 16.7 / 116.6 / 183.2 / 283.4      |
| approach and flare  | 183.4 / 550.0 / 1,016.6 / 1,166.6   | 50.0 / 150.0 / 366.6 / 1,250.0    |
| low fast pass       | 100.0 / 1,383.3 / 2,099.8 / 2,099.8 | 216.7 / 849.9 / 2,099.9 / 2,116.6 |
| slowdown            | 16.7 / 33.4 / 200.1 / 5,266.6       | 216.7 / 983.2 / 1,116.6 / 2,833.2 |
| vertical descent    | 133.3 / 849.9 / 1,283.3 / 1,283.3   | 216.7 / 683.2 / 766.7 / 766.7     |
| hover and touchdown | 33.3 / 83.3 / 233.3 / 4,149.8       | 33.3 / 83.3 / 100.0 / 583.4       |

## The sizes (R10.T14.b)

The ruled form is Σ max(1.3 AR, 3 F) + forced. Its slots against the build's own:

| Cache                                   | Ruled, this run        | As built               | Ratio   | The RTX 3080's ruled slots, 2026-10-21 (provisional) |
| --------------------------------------- | ---------------------- | ---------------------- | ------- | ---------------------------------------------------- |
| Lit (photorealistic, weight 1)          | 1,488 slots, 266.2 MB  | 375 slots of 178,908 B | × 3.97  | 2,996, against 422 slots of 158,936 B, × 7.10        |
| Wireframe instrument (weight 0.25)      | 1,416.4 slots, 53.9 MB | 520 slots of 38,028 B  | × 2.72  | 1,369, against 520, × 2.63                           |
| Wireframe, both views (weights 1, 0.25) | 5,838 slots, 222.0 MB  | 520 slots of 38,028 B  | × 11.23 | 6,509, against 520, × 12.52                          |

- **Not a comparison between machines.** The build changed between the two dates: the lit slot grew
  from 158,936 B to 178,908 B, and the build's lit slots fell from 422 to 375.
- **Fewer samples here.** This machine took fewer samples over the descent: 773 for the lit cache,
  against 1,152.
- **What the sizes are for.** As the README says, the low setting's and the wireframe's slots are
  re-sized from these runs once R10.T4.b's statistical bound or R09's lever (ii) is in selection.
- **The diagnostic occluder's A/B costs nothing** in either run: 0.0 slots, 0.0 %.

## Memory and streaming

| Figure                                     | Photorealistic      | Wireframe           |
| ------------------------------------------ | ------------------- | ------------------- |
| GPU headline (drm-fdinfo), against 1 GB    | 312 MiB             | 217 MiB             |
| Itemised, with the workers' field copies   | 86.4 MiB            | 22.4 MiB            |
| Adapter tally                              | 109 MiB             | 35 MiB              |
| Height workers' bakes                      | 5,137, 4.1 a second | 6,440, 5.4 a second |
| Bakes never made resident                  | 615                 | 384                 |
| `TERRAIN: STREAMING` shown, of the 1,200 s | 154.2 s             | 137.1 s             |

`TERRAIN: STREAMING` shows in the approach, the low fast pass and the slowdown in both runs, and
in the descent arc in the photorealistic run (51.0 s).

## Other runs in the same session

### R08's deck bake

[R08](../../agent/plans/rendering-and-planets/08-atmospheres.md)'s "Pending: the owner's runs":
the UHD 620's deck bake.

- **How it ran.** Through `thick/deck.bench.ts`'s documented command: one thread at `nice 10`.
- **When.** Started at 22:45:21 UTC, at a load of 0.86 and a package of 58 °C.
- **Figures.** Each is a mean of 3.

| Fixture                                                    | Preparation | A band | The correction, a band |
| ---------------------------------------------------------- | ----------- | ------ | ---------------------- |
| Venus-class, split at the cloud's top, deck to the ground  | 1.79 s      | 5.97 s | —                      |
| Saturn-class NH₃-like deck, an envelope, one of four bands | 6.1 ms      | 1.93 s | 0.48 s                 |

- **Against the ceiling.** A Venus-class band alone is over `BAKE_CEILING_S`, 5 s a world.
- **Vitest's warning.** Vitest flagged the export getters (`bandIndex`, `solveLower`,
  `solveLowerTransposed`), as it did on the pod.

### R12.T3.b's fdinfo

[R12.T3.b](../../agent/plans/rendering-and-planets/12-measurements-and-settings-ladder/records/R12.T3.b.md)'s
by-hand evidence on the UHD 620. One capture of the GPU process and of Xorg was taken five minutes
into the photorealistic run.

- **The GPU process.** It held six i915 clients on nine descriptors: IDs 11, 15, 16, 17 (on four
  descriptors), 19 and 23. Two of them held resident memory:
  - client 15, 61,008 KiB resident, of which 14,696 KiB shared;
  - client 23, 186,168 KiB resident.
- **Xorg.** Xorg runs as the user, so its fdinfo is readable. Its client 4, on five descriptors,
  held 23,024 KiB resident.
- **The `stolen-system0` keys** are present, and zero.
- **Now the fixtures.** These captures replaced the hand-written fdinfo fixtures, which had two
  clients for the GPU process, not six.
- **R12's own sampler has not run here yet.** `sampleGpuMemory` and `sampleDisplayServer` run in a
  `perf-run`.

### R12.T2's `/sys`

[R12.T2](../../agent/plans/rendering-and-planets/12-measurements-and-settings-ladder/records/R12.T2.md)'s
`/sys` was captured against the `x1-yoga` fixture tree; the capture now replaces that tree. Its
differences from the hand-written fixtures:

- **The GPU is `card1`, not `card0`.** The samplers find the card by PCI vendor, so this is
  harmless.
- **Seven thermal zones, not six.** `SEN1` is zone 1. `iwlwifi_1` is zone 6, and it has a reading.
  `x86_pkg_temp` is zone 5 in both.
- **RAPL.** `intel-rapl:0`'s PL1 is 51 W, not 44 W. τ is 27,983,872 µs on both RAPL zones, not 28 s.
- **USB-C.** The second UCSI source reads `online` 1, and its `usb_type` is `C [PD] PD_PPS`.
- **`energy_uj` is root-only**, as the plan expects.
