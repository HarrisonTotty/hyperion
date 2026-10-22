# Hand-over check: effect, high, 2026-10-22

- **Overall:** pass
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; load average 5.33, 5.64, 5.37 (provisional: load average 5.33 at the start; a shared machine; a hidden run: no presentation)
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, setting high, window hidden 1926 × 1086 DIP at a device-pixel ratio of 1
- **Field:** `terrain-test-weathered`, TEST MOON weathered (`terrain_test_weathered.golden`), served in TERRAIN TEST's place
- **World:** TERRAIN TEST WEATHERED; the primary 1126 × 906 px presented, its scene target 1126 × 906 px; the hand-over range 15419 km (relief 15812 m at τ 1 px, 1.0255e-3 rad a pixel); cells of level 6
- **The line:** 1% of the limb distance a second, from 3 to 0.85 of the range, a 12 s hold, out to 1.25
- **Bounds:** 0.333 stop at every switch, cells of 2 shared pixels or more judged, no floor; at most 5% of a frame's body pixels off the datum
- **Captures (not committed):** `target/views-check/terrain/captures-20261022T142027`
- **Faults:** 0 engine faults, 0 GPU-process exits; console errors 0

## Looks

| Look                | Verdict | Judged       | i, e, α asked          | i, e, α at the range   | EV100 locked (held) | Survey cells | Switches (script s: figure, terrain, disc) |
| ------------------- | ------- | ------------ | ---------------------- | ---------------------- | ------------------- | ------------ | ------------------------------------------ |
| handover-30         | pass    | yes          | 15.00°, 15.00°, 30.00° | 15.00°, 15.00°, 30.00° | 14.5 (held)         | 48           | 119.40, 119.53, 164.91                     |
| handover-90         | pass    | yes          | 45.00°, 45.00°, 90.00° | 45.00°, 45.00°, 90.00° | 12.9 (held)         | 48           | 119.40, 119.48, 164.91                     |
| handover-terminator | pass    | no, recorded | 78.00°, 12.00°, 90.00° | 78.00°, 12.00°, 90.00° | 12.8 (held)         | 48           | 119.40, 119.50, 164.91                     |

## Switches (provisional)

Changes are log₂(after ÷ before), stops, over each frame's own pixels / over the pixels both share.

| Look                | Switch          | Frames (script s, regime, limb distance ÷ range)     | Frames apart | Body            | Survey          | Cells judged | Over 1/3 stop | Dark on one side | Worst cell               | Verdict  |
| ------------------- | --------------- | ---------------------------------------------------- | ------------ | --------------- | --------------- | ------------ | ------------- | ---------------- | ------------------------ | -------- |
| handover-30         | disc-to-figure  | 119.38 s disc at 0.9092 → 119.40 s figure at 0.9091  | 1            | +0.000 / +0.000 | -0.000 / -0.000 | 48 of 48     | 0             | 0                | +0.000 (cell 3903, 6 px) | pass     |
| handover-30         | disc-to-terrain | 119.38 s disc at 0.9092 → 119.53 s terrain at 0.9079 | 13           | -0.000 / +0.001 | -0.000 / -0.000 | 48 of 48     | 0             | 0                | -0.003 (cell 3389, 8 px) | pass     |
| handover-30         | terrain-to-disc | 164.89 s terrain at 1.1110 → 164.91 s disc at 1.1112 | 1            | -0.000 / -0.000 | +0.004 / +0.004 | 48 of 48     | 0             | 0                | +0.016 (cell 3646, 4 px) | pass     |
| handover-90         | disc-to-figure  | 119.38 s disc at 0.9092 → 119.40 s figure at 0.9091  | 1            | -0.000 / -0.000 | -0.000 / -0.000 | 48 of 48     | 0             | 0                | -0.001 (cell 3710, 6 px) | pass     |
| handover-90         | disc-to-terrain | 119.38 s disc at 0.9092 → 119.48 s terrain at 0.9083 | 11           | -0.001 / -0.002 | -0.002 / -0.002 | 48 of 48     | 0             | 0                | -0.013 (cell 3709, 5 px) | pass     |
| handover-90         | terrain-to-disc | 164.89 s terrain at 1.1110 → 164.91 s disc at 1.1112 | 1            | +0.000 / +0.000 | +0.006 / +0.006 | 48 of 48     | 0             | 0                | -0.043 (cell 3711, 2 px) | pass     |
| handover-terminator | disc-to-figure  | 119.38 s disc at 0.9092 → 119.40 s figure at 0.9091  | 1            | +0.000 / -0.000 | -0.000 / -0.000 | 48 of 48     | 0             | 0                | -0.000 (cell 7808, 9 px) | recorded |
| handover-terminator | disc-to-terrain | 119.38 s disc at 0.9092 → 119.50 s terrain at 0.9082 | 13           | -0.000 / -0.002 | -0.004 / -0.004 | 48 of 48     | 0             | 0                | -0.040 (cell 3389, 9 px) | recorded |
| handover-terminator | terrain-to-disc | 164.89 s terrain at 1.1110 → 164.91 s disc at 1.1112 | 1            | +0.000 / +0.000 | -0.001 / -0.000 | 48 of 48     | 0             | 0                | +0.210 (cell 3646, 4 px) | recorded |

## Cells over 1/3 stop

None.

## Reasons

None.
