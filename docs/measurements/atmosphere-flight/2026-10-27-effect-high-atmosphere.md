# Atmosphere flight: effect, high, 2026-10-27

- **Overall:** pass
- **Machine:** AMD Ryzen 7 3700X 8-Core Processor, 16 threads; GPU 4318:8710; load average 1.52, 3.30, 7.19 (provisional: load average 1.52 at the start; a shared machine; a hidden run: no presentation)
- **Versions:** app 0.1.0, Electron 44.4.3, Chromium 152.0.7977.130
- **Launch:** linux, vulkan mode, setting high, window hidden 1926 × 1086 DIP at a device-pixel ratio of 1; the primary 1126 × 906 px; pass timer full
- **Media ready:** 1.1 s after the scene's choice
- **Captures (not committed):** `target/views-check/atmosphere/captures-20261027T001430`
- **Faults:** fault lines 0 atmosphere, 0 graphics; recorded, not judged: 0 engine faults, 0 GPU-process exits, console errors 0

## Captures (provisional times)

| Capture                    | Designation        | Camera                  | Range    | Phase | Exposure        | AUTO settled (s) | Draws | Frame interval median / p95 (ms) | GPU median / p95 (ms) | Statements                                                                                             | Fault lines |
| -------------------------- | ------------------ | ----------------------- | -------- | ----- | --------------- | ---------------- | ----- | -------------------------------- | --------------------- | ------------------------------------------------------------------------------------------------------ | ----------- |
| moonlit-night              | —                  | SEAT                    | —        | —     | EV100 1.0 AUTO  | 1.0              | 100   | 16.7 / 83.3                      | 1.35 / 1.35           | ROTATION: NOT YET MODELLED · BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED                               | —           |
| fixture-test-earth-o3-slab | TEST EARTH O3 SLAB | FREE · RATE 31,600 km/s | 50.8 Mm  | 45.0° | EV100 14.9 AUTO | 1.0              | 5428  | 16.7 / 16.8                      | 4.98 / 7.52           | POSITIONS AS SEEN FROM SHIP · ROTATION: NOT YET MODELLED · BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED | —           |
| fixture-test-mars          | TEST MARS          | FREE · RATE 1000 km/s   | 27.1 Mm  | 57.9° | EV100 15.3 AUTO | 1.0              | 2489  | 16.7 / 16.7                      | 5.42 / 6.71           | POSITIONS AS SEEN FROM SHIP · ROTATION: NOT YET MODELLED · BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED | —           |
| fixture-test-saturn        | TEST SATURN        | FREE · RATE 31,600 km/s | 482.3 Mm | 58.9° | EV100 15.9 AUTO | 1.0              | 1284  | 16.7 / 16.8                      | 6.76 / 9.32           | POSITIONS AS SEEN FROM SHIP · ROTATION: NOT YET MODELLED · BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED | —           |
| fixture-test-earth         | TEST EARTH         | FREE · RATE 31,600 km/s | 51.0 Mm  | 45.2° | EV100 14.9 AUTO | 1.0              | 3469  | 16.7 / 16.7                      | 1.53 / 7.25           | POSITIONS AS SEEN FROM SHIP · ROTATION: NOT YET MODELLED · BODY AND CRAFT PHOTOMETRY: NOT YET MODELLED | —           |

## Stages (as reckoned)

| Target             | Stage | Rate step | Asked (m) | Hold planned / held (s) | Longest step (ms) | Flown (m) | Short of centre (m) |
| ------------------ | ----- | --------- | --------- | ----------------------- | ----------------- | --------- | ------------------- |
| TEST MOON          | 1     | 13        | 5.000e+7  | 15.811 / 15.816         | 17                | 5.001e+7  | 3.297e+8            |
| TEST EARTH O3 SLAB | 1     | 20        | 2.010e+11 | 20.102 / 20.099         | 50                | 2.010e+11 | 1.061e+10           |
| TEST EARTH O3 SLAB | 2     | 18        | 1.008e+10 | 10.081 / 9.716          | 950               | 9.716e+9  | 8.936e+8            |
| TEST EARTH O3 SLAB | 3     | 15        | 8.426e+8  | 26.645 / 26.649         | 17                | 8.427e+8  | 5.085e+7            |
| TEST MARS          | 1     | 16        | 9.681e+8  | 9.681 / 9.683           | 17                | 9.683e+8  | 5.078e+7            |
| TEST MARS          | 2     | 12        | 2.367e+7  | 23.667 / 23.666         | 17                | 2.367e+7  | 2.712e+7            |
| TEST SATURN        | 1     | 15        | 5.245e+8  | 16.586 / 16.583         | 17                | 5.244e+8  | 4.823e+8            |
| TEST EARTH         | 1     | 20        | 2.012e+11 | 20.119 / 20.116         | 33                | 2.012e+11 | 1.062e+10           |
| TEST EARTH         | 2     | 18        | 1.009e+10 | 10.092 / 10.100         | 17                | 1.010e+10 | 5.217e+8            |
| TEST EARTH         | 3     | 15        | 4.707e+8  | 14.884 / 14.883         | 17                | 4.706e+8  | 5.101e+7            |

## Reasons

None.
