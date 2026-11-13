/**
 * The ray march over stacked sheet decks (plan R08, R08.T16.a.2): the hand Jupiter's medium above
 * its envelope's floor, on the floor's datum of its figure (f = 0.065) and gravity, drawn by the
 * analytic term (its route closed), from 10⁹ m. Its decks are sheets: its NH₃ deck's τ(550) of 1.2
 * lies within 2 km of its 178 km base, at the lower end of an 18 km piece of a nadir ray, which
 * R08.T6.i's shares by length gave 3 of 36 steps. The march at the high setting's 36 samples is held
 * to 5% of its march at sixteen times them in every channel (the brief's lean, as R05.T12.e's 2% is
 * the thin march's), over the disc's centre, middle and edge, on the equator and at 45°, where an
 * edge's shell stands about 1 km off the edge's level, at phases 0° and 60°.
 *
 * @remarks
 * The tables take about 35 s (pod, 16 vCPU, 2026-11-02), the marches a tenth of a second. Before
 * the march shared a cut side's steps by the light each piece returns and took its crossings onto
 * their edges' levels (`marchSteps.ts`'s `depthSharedPieces` and `MARCH_EDGE_LEAST_RISE`), its red
 * read 0.36–0.61 of the march at sixteen times the steps over these rays. Since the kernels' tables
 * keep each deck's column (`tables.ts`'s `resampledDensity`; they drew its H₂O and NH₄SH sheets at
 * 1.7% of it) and the marches cut a projected sheet at both its ends, it reads within 2.6%.
 */
import { beforeAll, describe, expect, it } from "vitest";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { detailOf, recordInput } from "../../test/atmosphereRecords";
import { handJupiterDetail } from "../../test/giantFixtures";
import { NEAR_PLANE_M } from "../camera/projection";
import { quaternionFromRows } from "../camera/quaternion";
import { SUN_RADIUS_M } from "../lighting/hostDisc";
import { AU_M } from "../scenes/kept";
import { absorberCurveSlots, assembleMedium } from "./assemble";
import { TABLE_SIZES, type TableSizes } from "./hillaire";
import { kernelMedium } from "./tables";
import {
  type OblatePlanetTwin,
  oblatePlanetTwin,
  oblateRayMarchRayAlongTwin,
  oblateRayMarchTexelTwin,
  twinView,
} from "./tablesCpu";
import { deckDatumOf, upperMediumOf } from "./thick/deckRead";

/** The camera's distance from the centre, m: the disc's boundary (Design note 13). */
const DISTANCE_M = 1e9;

/** The march's tolerance against its converged march, per channel: the brief's lean. */
const TOLERANCE = 0.05;

/** The converged march's steps, as a multiple of high's. */
const CONVERGED = 16;

/** The rays' places on the disc, a fraction of its radius, and their angles from the equator, °. */
const RADII = [0, 0.5, 0.8, 0.9] as const;
const ANGLES_DEG = [0, 45] as const;

/** High's sizes with `k` times its ray march's steps and its counts by optical depth. */
function marchedAt(k: number): TableSizes {
  const high = TABLE_SIZES.high;
  const rule = high.marchDepthSteps;
  return {
    ...high,
    rayMarchSamples: high.rayMarchSamples * k,
    marchDepthSteps:
      rule === null
        ? null
        : {
            ...rule,
            groundFloor: rule.groundFloor * k,
            groundFloorPerEdge: rule.groundFloorPerEdge * k,
          },
  };
}

/** The body-fixed axes' coordinates of a vector of the view's: (z, x, y). */
function bodyFixed(v: Vec3): Vec3 {
  return vec3(v.z, v.x, v.y);
}

/** The hand Jupiter's upper medium over its envelope's floor, its tables on the floor's datum. */
function handJupiterAboveItsFloor(): OblatePlanetTwin {
  const { medium, drawn } = assembleMedium(recordInput(detailOf(handJupiterDetail())), {
    envelopes: true,
  });
  if (medium === undefined || drawn.envelope === undefined) {
    throw new Error("the hand Jupiter draws an envelope's medium");
  }
  const floorM = drawn.envelope.floorAltitudeM;
  return oblatePlanetTwin(
    kernelMedium(upperMediumOf(medium, floorM)),
    {
      figure: deckDatumOf(drawn.body.figure, drawn.body.gravity, floorM),
      gravity: drawn.body.gravity,
    },
    absorberCurveSlots(drawn),
  );
}

/**
 * The largest |L₃₆ ÷ L₅₇₆ − 1| over the rays and channels at the phase `phaseDeg`: the camera over
 * the equator looking at the centre, the pole up the image, the sun in the equatorial plane.
 */
function worstAt(planet: OblatePlanetTwin, phaseDeg: number): number {
  const { figure } = planet.body;
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const alpha = (phaseDeg * Math.PI) / 180;
  const sun = vec3(Math.sin(alpha), 0, Math.cos(alpha));
  const cameraM = vec3(DISTANCE_M, 0, 0);
  const view = twinView(
    {
      positionM: cameraM,
      orientation: quaternionFromRows([vec3(0, 0, 1), vec3(1, 0, 0), vec3(0, 1, 0)]),
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 1920, heightPx: 1080 },
    },
    {
      directionBodyFixed: bodyFixed(sun),
      illuminanceLx: [1, 1, 1],
      angularRadiusRad: Math.asin(SUN_RADIUS_M / AU_M),
    },
    figure,
    NEAR_PLANE_M,
  );
  let worst = 0;
  for (const r of RADII) {
    for (const angleDeg of ANGLES_DEG) {
      const phi = (angleDeg * Math.PI) / 180;
      const ray = normalise(
        vec3((a / DISTANCE_M) * r * Math.cos(phi), (c / DISTANCE_M) * r * Math.sin(phi), -1),
      );
      const along = oblateRayMarchRayAlongTwin(
        planet,
        view,
        cameraM,
        bodyFixed(ray),
        Number.POSITIVE_INFINITY,
      );
      const marched = oblateRayMarchTexelTwin(planet, view, TABLE_SIZES.high, along);
      const converged = oblateRayMarchTexelTwin(planet, view, marchedAt(CONVERGED), along);
      for (const ch of [0, 1, 2] as const) {
        worst = Math.max(worst, Math.abs(marched[ch] / converged[ch] - 1));
      }
    }
  }
  return worst;
}

describe("the ray march over stacked sheet decks (R08.T16.a.2)", () => {
  let planet: OblatePlanetTwin | undefined;
  beforeAll(() => {
    planet = handJupiterAboveItsFloor();
  }, 300_000);
  const built = (): OblatePlanetTwin => {
    if (planet === undefined) {
      throw new Error("the hand Jupiter's tables are built before the tests");
    }
    return planet;
  };

  for (const phaseDeg of [0, 60]) {
    it(`marches the hand Jupiter's decks at the high setting's 36 samples within 5% of sixteen times them at a phase of ${phaseDeg}°`, () => {
      expect(worstAt(built(), phaseDeg)).toBeLessThan(TOLERANCE);
    });
  }
});
