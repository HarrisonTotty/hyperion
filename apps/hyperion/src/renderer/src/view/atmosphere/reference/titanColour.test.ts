/**
 * `titan-class`'s disc colour as it is drawn (plan R08, R08.T17; research-leans-36 §1): the CPU
 * twin's disc centre at phase 0 and 30°, read through the path the body is drawn by, is
 * orange-brown: red over green at least 0.95 and blue over green at most 0.6 in I/F (the brief's
 * bounds), which Titan's geometric albedo, 1.40 and 0.55 at 645, 544 and 434 nm (0.284, 0.202 and
 * 0.111: E. Karkoschka, Icarus 133 (1998) 134, as A. Payne et al. 2025, arXiv:2508.13368, compile it), meets. The haze mixed with the gas from the ground read
 * 0.82 and 0.85 at phase 0 through the route, its N₂'s Rayleigh scattering lifting the blue inside
 * it.
 *
 * @remarks
 * The path is the route's where `thick/routing.ts`'s `thickBakeRouted` routes the case (its whole
 * column baked at m ≤ 2 with its correction, its own wavelengths and curved streams, as
 * `vectorGate.test.ts`'s `titanClass` bakes it), and the analytic term's otherwise, with no
 * correction, which an unrouted thick world takes none of (`correctionRouted`'s remarks). The
 * albedo is the whole disc's, a guide to its centre rather than its like: Titan's limb is the bluer.
 * The route's bake runs for some 15 s on a pod's 16 vCPU.
 */

import { describe, expect, it } from "vitest";

import type { Rgb } from "../../photometry/toneCurve";
import { bakeKeptCase, bakeKeptCorrection } from "../thick/keptBake";
import { CLEAR_COVER, classifyRegime } from "../thick/regime";
import { thickBakeRouted } from "../thick/routing";
import { caseTwin, twinValueOf } from "./caseGate";
import { referenceCaseBakeLights, type ReferenceCase } from "./cases";
import { titanClassReferenceCase } from "./thickCases";
import { type ReferenceTwin, twinBakeBody } from "./twin";

/** The Mie and the bake, ms: well over ten times their run. */
const SLOW_MS = 300_000;

/** The least red over green and the most blue over green in I/F that read as Titan's. */
const TITAN_COLOUR = { leastRedOverGreen: 0.95, mostBlueOverGreen: 0.6 } as const;

/** The disc's geometries judged: its centre at phase 0 and 30°. */
const DISCS = ["disc-p0", "disc-p30"] as const;

/** The case's twin on the path it is drawn by. */
function drawnTwin(referenceCase: ReferenceCase): ReferenceTwin {
  const { medium } = referenceCase;
  if (!thickBakeRouted(classifyRegime(medium, CLEAR_COVER), medium)) {
    return caseTwin(referenceCase);
  }
  const lights = referenceCaseBakeLights(referenceCase);
  const body = twinBakeBody(referenceCase.figure);
  return caseTwin(referenceCase, {
    thick: bakeKeptCase(referenceCase, lights, body, { ownWavelengths: true, curvedStreams: true }),
    polarisation: bakeKeptCorrection(referenceCase, lights, body),
  });
}

/** Each disc's I/F ratios as drawn, red over green and blue over green. */
function discRatios(): ReadonlyArray<readonly [string, number, number]> {
  const referenceCase = titanClassReferenceCase();
  const twin = drawnTwin(referenceCase);
  const [sunR, sunG, sunB] = referenceCase.sunIlluminanceLx;
  return DISCS.map((name) => {
    const [r, g, b]: Rgb = twinValueOf(twin, referenceCase, {
      quantity: "radiance",
      setting: "case",
      caseName: referenceCase.name,
      name,
    });
    return [name, r / sunR / (g / sunG), b / sunB / (g / sunG)] as const;
  });
}

describe("titan-class's disc as drawn", { timeout: SLOW_MS }, () => {
  it("reads Titan's orange-brown at phase 0 and 30°", () => {
    const ratios = discRatios();
    expect(
      ratios.map(([name, redOverGreen, blueOverGreen]) => [
        name,
        redOverGreen >= TITAN_COLOUR.leastRedOverGreen,
        blueOverGreen <= TITAN_COLOUR.mostBlueOverGreen,
      ]),
      `R/G and B/G: ${JSON.stringify(ratios)}`,
    ).toEqual(DISCS.map((name) => [name, true, true]));
  });
});
