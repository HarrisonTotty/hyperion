import { BOTTOM_M } from "../../test/atmosphereGate";
import { describeMarchGate } from "../../test/marchGate";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES } from "./earth";
import { planetTwin } from "./tablesCpu";

/**
 * R05.T12.e's quadrature gate for the widened march (R08.T8) on the measured Earth, a file of its
 * own so that Vitest runs it beside the other media's. The gate's machinery is
 * `test/marchGate.ts`'s; R05's Earth's gate and the rays' count are `marchGate.test.ts`.
 */

describeMarchGate({
  name: "the measured Earth",
  medium: EARTH_REFERENCE,
  planet: () => planetTwin(EARTH_REFERENCE, BOTTOM_M, EARTH_REFERENCE_CURVES),
  // Since R08.T8.e high's count on the bare ground is scaled by its optical depth: at one source the
  // march's ordinary worst is the disc at 20 steps (1.35%; 0.53% at 36 steps throughout), measured
  // 2026-10-22; the thin sky at 28 steps puts the sky's ordinary worst at six sources at 0.12%
  // (0.07%), measured 2026-10-23.
  record: {
    highMisses: [[], []],
    high: [
      {
        kernel: {
          march: 0.0135,
          marchTwilight: 0.03239,
          sky: 0.00529,
          skyTwilight: 0.03295,
          lastLight: 0.00554,
        },
        even: {
          march: 0.05093,
          marchTwilight: 0.03239,
          sky: 0.00883,
          skyTwilight: 0.03295,
          lastLight: 0.16369,
        },
      },
      {
        kernel: {
          march: 0.00983,
          marchTwilight: 0.00396,
          sky: 0.0012,
          skyTwilight: 0.00434,
          lastLight: 0.0022,
        },
        even: {
          march: 0.0282,
          marchTwilight: 0.00634,
          sky: 0.00128,
          skyTwilight: 0.00828,
          lastLight: 0.0034,
        },
      },
    ],
    low: [
      {
        kernel: {
          march: 0.0359,
          marchTwilight: 0.22354,
          sky: 0.02914,
          skyTwilight: 0.18242,
          lastLight: 0.03705,
        },
        even: {
          march: 0.25225,
          marchTwilight: 0.24471,
          sky: 0.04348,
          skyTwilight: 0.28025,
          lastLight: 0.35647,
        },
      },
      {
        kernel: {
          march: 0.01461,
          marchTwilight: 0.02526,
          sky: 0.00368,
          skyTwilight: 0.03032,
          lastLight: 0.01852,
        },
        even: {
          march: 0.16568,
          marchTwilight: 0.0248,
          sky: 0.00648,
          skyTwilight: 0.04029,
          lastLight: 0.01955,
        },
      },
    ],
    overLow: [[], []],
  },
});
