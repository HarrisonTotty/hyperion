import { expect, it } from "vitest";

import { BOTTOM_M, kernelOf } from "../../test/atmosphereGate";
import { describeMarchGate, raysOf } from "../../test/marchGate";
import { R05_EARTH_REFERENCE } from "./earth";
import { planetTwin, tableSources } from "./tablesCpu";

/**
 * R05.T12.e's quadrature gate for the widened march (R08.T8) on R05's Earth, and the count of the
 * gate's rays, which holds for every medium. The gate's machinery is `test/marchGate.ts`'s; the
 * measured Earth's gate is `marchGateMeasured.test.ts`, the slab Earth's `marchGateSlab.test.ts`.
 */

it("covers the march's 268 rays, the night limb's among them, the last light's 525, and the sky's others 278 from high's band's cameras and 358 from low's", () => {
  const counts = (["high", "low"] as const).map((setting) => {
    const rays = raysOf(setting);
    return [
      rays.filter((ray) => kernelOf(ray) === "rayMarch" && ray.family !== "last light").length,
      rays.filter((ray) => ray.family === "last light").length,
      rays.length,
    ];
  });
  expect(counts).toEqual([
    [268, 525, 1_071],
    [268, 525, 1_151],
  ]);
});

describeMarchGate({
  name: "R05's Earth",
  medium: R05_EARTH_REFERENCE,
  sources: () => tableSources(planetTwin(R05_EARTH_REFERENCE, BOTTOM_M)),
  // At one source on high the night limb sets the march's twilight worst (4.69%, from 400 km at
  // 0.1 H, sun 100°/180°; 4.79% in even steps alone) and the high limb from 99.999 km the sky's
  // (4.32%); the last light's is 0.79%, where even steps put it at 12.47%. Since R08.T8.e high's
  // count on the bare ground is scaled by its optical depth: the march's ordinary worst is the disc
  // at 20 steps, ground zenith 60°, sun 80°/0° (1.43%; 0.45% at 36 steps throughout), measured
  // 2026-10-22; the thin sky at 28 steps puts the sky's ordinary worst at six sources at 0.12%
  // (0.07%), measured 2026-10-23. On low the high limb
  // from 80 km at tangent 0.84 km, sun 90°/0°, sets the sky's (24.00%, against even steps' 46.37%
  // on the band from 50 km), and no ray stands over `LOW_TWIN_WORST`: the two band rays from 50 km
  // that stood over the sky view's bound in even steps (19.44% and 46.37%) fall under it.
  record: {
    highMisses: [[], []],
    high: [
      {
        kernel: {
          march: 0.01429,
          marchTwilight: 0.04687,
          sky: 0.00593,
          skyTwilight: 0.04317,
          lastLight: 0.00788,
        },
        even: {
          march: 0.05739,
          marchTwilight: 0.04786,
          sky: 0.01028,
          skyTwilight: 0.04317,
          lastLight: 0.12466,
        },
      },
      {
        kernel: {
          march: 0.01076,
          marchTwilight: 0.00597,
          sky: 0.00125,
          skyTwilight: 0.00634,
          lastLight: 0.00457,
        },
        even: {
          march: 0.03459,
          marchTwilight: 0.00766,
          sky: 0.00112,
          skyTwilight: 0.00977,
          lastLight: 0.00509,
        },
      },
    ],
    low: [
      {
        kernel: {
          march: 0.02589,
          marchTwilight: 0.23945,
          sky: 0.0331,
          skyTwilight: 0.24005,
          lastLight: 0.04502,
        },
        even: {
          march: 0.27464,
          marchTwilight: 0.23945,
          sky: 0.05183,
          skyTwilight: 0.46367,
          lastLight: 0.35943,
        },
      },
      {
        kernel: {
          march: 0.01689,
          marchTwilight: 0.02736,
          sky: 0.00374,
          skyTwilight: 0.03315,
          lastLight: 0.02344,
        },
        even: {
          march: 0.19377,
          marchTwilight: 0.03137,
          sky: 0.00565,
          skyTwilight: 0.04859,
          lastLight: 0.02716,
        },
      },
    ],
    overLow: [[], []],
  },
});
