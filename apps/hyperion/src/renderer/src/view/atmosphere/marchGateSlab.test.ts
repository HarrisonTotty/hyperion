import { BOTTOM_M } from "../../test/atmosphereGate";
import { describeMarchGate } from "../../test/marchGate";
import { EARTH_SLAB_REFERENCE, EARTH_SLAB_REFERENCE_CURVES } from "./earth";
import { planetTwin, tableSources } from "./tablesCpu";

/**
 * R05.T12.e's quadrature gate for the widened march (R08.T8) on the slab Earth, a file of its own
 * so that Vitest runs it beside the other media's. The gate's machinery is `test/marchGate.ts`'s;
 * R05's Earth's gate and the rays' count are `marchGate.test.ts`.
 */

describeMarchGate({
  name: "the slab Earth",
  medium: EARTH_SLAB_REFERENCE,
  sources: () =>
    tableSources(planetTwin(EARTH_SLAB_REFERENCE, BOTTOM_M, EARTH_SLAB_REFERENCE_CURVES)),
  // At one source high misses the limb's and night limb's rays `tablesCpu.test.ts`'s
  // `SLAB_HIGH_MISS` names, and two of the sky's from 99.999 km that the ray march draws there:
  // the high band at view zenith 99.879°, sun 90°/180° (6.44% under the warp, a = 1; 6.74% in
  // §4's even steps), and the high limb at tangent 0.84 km, sun 90°/0° (5.25%, a = 0, as in §4).
  // Neither is the warp's: both stood over 5% under §4's even steps, which this gate did not run
  // on the slab Earth before (measured 2026-10-21; `decision-r08-t8-leans.md` §4.3, named misses
  // reported to main). No tolerance or count moves. `LOW_TWIN_WORST` is not re-pinned on a test
  // medium (`decision-r08-t6d-findings.md` §10.4.3): the rays over it are recorded. Since R08.T8.e
  // high's count on the bare ground is scaled by its optical depth, the named misses' sky chords at
  // 36: at one source the march's ordinary worst is the disc at 28 steps, 20 and 4 for each of the
  // slab's two edges (1.59%; 1.12% at 36 steps throughout), measured 2026-10-22; the thin sky at 28
  // steps puts the sky's ordinary worst at six sources at 0.12% (0.07%), measured 2026-10-23.
  record: {
    highMisses: [
      [
        "limb, 0.1 H, sun 90°/0°: 5.15%",
        "night limb, 400 km, 0.3 H, sun 100°/180°: 6.27%",
        "night limb, 101 km, 0.1 H, sun 90°/0°: 5.15%",
        "night limb, 101 km, 0.3 H, sun 100°/180°: 6.27%",
        "high band, 99999 m, view zenith 99.879°, sun 90°/180°: 6.44%",
        "high limb, 99.999 km, tangent 0.84 km, sun 90°/0°: 5.25%",
      ],
      [],
    ],
    high: [
      {
        kernel: {
          march: 0.01589,
          marchTwilight: 0.06275,
          sky: 0.01188,
          skyTwilight: 0.06439,
          lastLight: 0.00553,
        },
        even: {
          march: 0.16263,
          marchTwilight: 0.20829,
          sky: 0.16369,
          skyTwilight: 0.20916,
          lastLight: 0.16461,
        },
      },
      {
        kernel: {
          march: 0.01109,
          marchTwilight: 0.00963,
          sky: 0.0012,
          skyTwilight: 0.01124,
          lastLight: 0.00913,
        },
        even: {
          march: 0.02186,
          marchTwilight: 0.16481,
          sky: 0.00128,
          skyTwilight: 0.16305,
          lastLight: 0.17249,
        },
      },
    ],
    low: [
      {
        kernel: {
          march: 0.0597,
          marchTwilight: 0.28556,
          sky: 0.05435,
          skyTwilight: 0.28562,
          lastLight: 0.07273,
        },
        even: {
          march: 0.30338,
          marchTwilight: 0.44193,
          sky: 0.30292,
          skyTwilight: 0.49632,
          lastLight: 0.35863,
        },
      },
      {
        kernel: {
          march: 0.02603,
          marchTwilight: 0.06909,
          sky: 0.00368,
          skyTwilight: 0.04765,
          lastLight: 0.03925,
        },
        even: {
          march: 0.23908,
          marchTwilight: 0.30509,
          sky: 0.00648,
          skyTwilight: 0.30465,
          lastLight: 0.37626,
        },
      },
    ],
    overLow: [
      [
        "disc, ground zenith 60°, sun 80°/0°: 5.97%",
        "disc, ground zenith 80°, sun 80°/0°: 3.78%",
        "limb, 0.1 H, sun 90°/0°: 28.56%",
        "limb, 0.3 H, sun 30°/180°: 4.11%",
        "limb, 1 H, sun 30°/180°: 4.10%",
        "night limb, 400 km, 0.1 H, sun 95°/180°: 27.51%",
        "night limb, 400 km, 0.3 H, sun 95°/180°: 24.12%",
        "night limb, 101 km, 0.1 H, sun 90°/0°: 28.56%",
        "night limb, 101 km, 0.1 H, sun 95°/180°: 27.51%",
        "night limb, 101 km, 0.3 H, sun 95°/180°: 24.12%",
        "band, 50000 m, view zenith 97.008°, sun 90°/180°: 18.23%",
      ],
      ["limb, 0.3 H, sun 30°/180°: 3.95%", "limb, 1 H, sun 30°/180°: 4.12%"],
    ],
  },
});
