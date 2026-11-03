/**
 * The discs a photorealistic view warms the terrain of before it hands them over (lane
 * fix-coast-class-map; the hand-over's `HANDOVER_AT_CELLS`): each disc, or mesh, of its plan in its
 * hand-over range at the photorealistic τ (`warmingDiscs`), pure. The session's warmed passes and
 * the reliefs it withholds are tested in `sessionPasses.test.ts` and `session.test.ts`.
 */

import type { BodyIdHex, DetailSeedHex } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import {
  moonRangeM,
  SESSION_MOON,
  SESSION_MOON_RADIUS_M,
  SESSION_SCENE,
  viewAt,
} from "../../test/terrainSessionFixtures";
import type { LitRegime } from "../bodies/regime";
import { IDENTITY_ROTATION } from "../coords/rotation";
import { SETTINGS } from "../quality/qualitySetting";
import type { ViewBody, ViewScene } from "../scene/model";
import { type TerrainViewReport, warmingDiscs } from "./session";

const MOON: BodyIdHex = SESSION_MOON;
const SEED: DetailSeedHex = "0123456789abcdef";

/** `moonLikeHeader`'s realised relief, m. */
const MOON_RELIEF_M = 18_000;

/** The moon's photorealistic hand-over range on the fixtures' high setting, m. */
const RANGE_M = moonRangeM(SETTINGS.high.terrain.tauPx);

/** R07's `REGIME_HYSTERESIS`, h: a body not handed over goes in at range ÷ (1 + h). */
const H = 0.1;

/** The scene's moon. */
function sceneMoon(scene: ViewScene): ViewBody {
  const moon = scene.bodies.find((body) => body.id === MOON);
  if (moon === undefined) {
    throw new Error("the scene holds its moon");
  }
  return moon;
}

/** `scene` with its moon `moon`. */
function withMoon(scene: ViewScene, moon: ViewBody): ViewScene {
  return { ...scene, bodies: scene.bodies.map((body) => (body.id === MOON ? moon : body)) };
}

/** The scene with the moon turning (its rotation the identity) and carrying its detail seed. */
const SCENE: ViewScene = withMoon(SESSION_SCENE, {
  ...sceneMoon(SESSION_SCENE),
  rotation: IDENTITY_ROTATION,
  detailSeed: SEED,
});

/** A photorealistic view `limbM` from the moon's limb, its last plan drawing the moon as `regime`. */
function atLimb(limbM: number, regime: LitRegime): TerrainViewReport {
  return {
    ...viewAt(Math.hypot(limbM, SESSION_MOON_RADIUS_M), "photorealistic", { scene: SCENE }),
    regimes: new Map([[MOON, regime]]),
  };
}

/** The moon's relief alone. */
function moonRelief(body: BodyIdHex): number | null {
  return body === MOON ? MOON_RELIEF_M : null;
}

describe("the discs a photorealistic view warms the terrain of (HANDOVER_AT_CELLS)", () => {
  it("warms the moon drawn as a disc inside its range ÷ 1.1, where it would hand over", () => {
    expect([...warmingDiscs(atLimb((0.99 * RANGE_M) / (1 + H), "disc"), moonRelief)]).toEqual([
      MOON,
    ]);
  });

  it("warms the moon drawn as the mesh its drawn air, or a mesh it overlaps, makes of its disc", () => {
    expect([...warmingDiscs(atLimb((0.99 * RANGE_M) / (1 + H), "mesh"), moonRelief)]).toEqual([
      MOON,
    ]);
  });

  it("warms none outside it", () => {
    expect([...warmingDiscs(atLimb((1.01 * RANGE_M) / (1 + H), "disc"), moonRelief)]).toEqual([]);
  });

  it("warms no body its plan already draws as terrain, nor one it draws as a point", () => {
    const limbM = (0.5 * RANGE_M) / (1 + H);
    expect([
      [...warmingDiscs(atLimb(limbM, "terrain"), moonRelief)],
      [...warmingDiscs(atLimb(limbM, "point"), moonRelief)],
    ]).toEqual([[], []]);
  });

  it("warms none of a body with no relief, nor in a view drawn in the wireframe", () => {
    const report = atLimb((0.5 * RANGE_M) / (1 + H), "disc");
    expect([
      [...warmingDiscs(report, () => null)],
      [...warmingDiscs({ ...report, drawnStyle: "wireframe" }, moonRelief)],
    ]).toEqual([[], []]);
  });

  it("warms none whose rotation is not known", () => {
    const still = withMoon(SCENE, { ...sceneMoon(SCENE), rotation: null });
    const report = atLimb((0.5 * RANGE_M) / (1 + H), "disc");
    expect([...warmingDiscs({ ...report, scene: still }, moonRelief)]).toEqual([]);
  });
});
