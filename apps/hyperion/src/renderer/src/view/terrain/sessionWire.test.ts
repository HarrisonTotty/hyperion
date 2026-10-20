/**
 * The bodies a wireframe view hands over to terrain (plan R10, R10.T11; Design note 13): the
 * wireframe's 4 px on both settings, with R07's hysteresis (`wireframeHandovers`), pure. The
 * session's wireframe passes, which take these, are tested with its other passes in
 * `sessionPasses.test.ts`.
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
import { IDENTITY_ROTATION } from "../coords/rotation";
import { WIREFRAME_TERRAIN_TAU_PX } from "../quality/qualitySetting";
import type { ViewBody, ViewScene } from "../scene/model";
import { type TerrainViewReport, wireframeHandovers } from "./session";

const MOON: BodyIdHex = SESSION_MOON;
const SEED: DetailSeedHex = "0123456789abcdef";

/** `moonLikeHeader`'s realised relief, m, by which the moon hands over. */
const MOON_RELIEF_M = 18_000;

/** The moon's hand-over range in the wireframe, at its 4 px, m: 7.48 × 10⁶ m at 1080p across 60°. */
const WIRE_RANGE_M = moonRangeM(WIREFRAME_TERRAIN_TAU_PX);

/** R07's `REGIME_HYSTERESIS`, h: in at range ÷ (1 + h), out past range ÷ (1 − h). */
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

/** The scene with the moon turning (its rotation the identity) and carrying its own detail seed. */
const SCENE: ViewScene = withMoon(SESSION_SCENE, {
  ...sceneMoon(SESSION_SCENE),
  rotation: IDENTITY_ROTATION,
  detailSeed: SEED,
});

/** The camera's distance from the moon's centre at which its limb lies `limbM` away. */
function distanceForLimbM(limbM: number): number {
  return Math.hypot(limbM, SESSION_MOON_RADIUS_M);
}

/** A wireframe view whose camera is `limbM` from the moon's limb, over the turning moon. */
function wireAtLimb(
  limbM: number,
  overrides: Partial<TerrainViewReport> = {},
  scene: ViewScene = SCENE,
): TerrainViewReport {
  return { ...viewAt(distanceForLimbM(limbM), "wireframe", { scene }), ...overrides };
}

/** The moon's relief alone: every other body has none. */
function moonRelief(body: BodyIdHex): number | null {
  return body === MOON ? MOON_RELIEF_M : null;
}

/** No body handed over at the last step. */
const NONE: ReadonlySet<BodyIdHex> = new Set();

/** The moon handed over at the last step. */
const THE_MOON: ReadonlySet<BodyIdHex> = new Set([MOON]);

describe("the bodies a wireframe view hands over (R10.T11; Design note 13)", () => {
  it("hands the moon over inside its wireframe range ÷ 1.1, where its relief subtends 4 px at its limb", () => {
    const report = wireAtLimb((0.99 * WIRE_RANGE_M) / (1 + H));
    expect([...wireframeHandovers(report, moonRelief, NONE)]).toEqual([MOON]);
  });

  it("does not hand it over afresh between range ÷ 1.1 and range ÷ 0.9", () => {
    expect([...wireframeHandovers(wireAtLimb(WIRE_RANGE_M), moonRelief, NONE)]).toEqual([]);
  });

  it("keeps it handed over out to range ÷ 0.9 from the bodies it handed over last", () => {
    const report = wireAtLimb((0.99 * WIRE_RANGE_M) / (1 - H));
    expect([...wireframeHandovers(report, moonRelief, THE_MOON)]).toEqual([MOON]);
  });

  it("hands it back beyond range ÷ 0.9", () => {
    const report = wireAtLimb((1.01 * WIRE_RANGE_M) / (1 - H));
    expect([...wireframeHandovers(report, moonRelief, THE_MOON)]).toEqual([]);
  });

  describe("hands nothing over", () => {
    const inside = (0.5 * WIRE_RANGE_M) / (1 + H);
    const unturned = withMoon(SCENE, { ...sceneMoon(SCENE), rotation: null });
    it.each<[string, TerrainViewReport, (body: BodyIdHex) => number | null]>([
      [
        "in a photorealistic view",
        { ...wireAtLimb(inside), style: "photorealistic", drawnStyle: "photorealistic" },
        moonRelief,
      ],
      [
        "in a photorealistic view standing in with the wireframe",
        { ...wireAtLimb(inside), style: "photorealistic", drawnStyle: "wireframe" },
        moonRelief,
      ],
      [
        "in a wireframe view whose last frame was photorealistic",
        { ...wireAtLimb(inside), style: "wireframe", drawnStyle: "photorealistic" },
        moonRelief,
      ],
      ["of a body whose rotation is not known", wireAtLimb(inside, {}, unturned), moonRelief],
      ["of a body with no relief", wireAtLimb(inside), () => null],
    ])("%s", (_case, report, reliefM) => {
      expect([...wireframeHandovers(report, reliefM, THE_MOON)]).toEqual([]);
    });
  });

  it("takes the same 4 px on both settings", () => {
    const limbs = [0.99, 1.01].map((k) => (k * WIRE_RANGE_M) / (1 + H));
    const handed = (["high", "low"] as const).map((setting) =>
      limbs.map((limbM) =>
        Array.from(wireframeHandovers(wireAtLimb(limbM, { setting }), moonRelief, NONE)),
      ),
    );
    expect(handed).toEqual([
      [[MOON], []],
      [[MOON], []],
    ]);
  });
});
