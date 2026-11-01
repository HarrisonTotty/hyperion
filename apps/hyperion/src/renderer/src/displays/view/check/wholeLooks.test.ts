import { beforeAll, describe, expect, it } from "vitest";

import terrainGoldenText from "../../../../../../../../crates/hyperion-surface/tests/golden/worker/terrain_test.golden?raw";
import { norm, normalise, sub, vec3 } from "../../../geometry/vec3";
import { rotate } from "../../../view/camera/quaternion";
import { rotateToBody } from "../../../view/coords/rotation";
import {
  provideWholeTerrainField,
  TERRAIN_MOON_CENTRE_M,
  terrainTestGolden,
} from "../../../view/scenes/terrainScene";
import {
  readWholeDisc,
  WHOLE_LOOKS,
  type WholeDiscFrame,
  wholeLookDistanceM,
  wholeLookRotation,
  wholeLookScene,
  wholeLookSceneOptions,
} from "./wholeLooks";

beforeAll(() => {
  // The patch's golden stands in for the native tool's file: the same moon, body, seed and header.
  provideWholeTerrainField(terrainGoldenText);
});

describe("the whole-body run's looks (R10.T14.c)", () => {
  it.each(WHOLE_LOOKS.map((look) => [look.name, look] as const))(
    "turns the moon so that the %s look's direction faces the star",
    (_name, look) => {
      const faced = rotateToBody(wholeLookRotation(look.over), normalise(look.over));
      expect(norm(sub(faced, vec3(-1, 0, 0)))).toBeLessThan(1e-12);
    },
  );

  it("holds the ship over the look's direction, sunward of the moon, looking at its centre", () => {
    for (const look of WHOLE_LOOKS) {
      const scene = wholeLookScene(look).sceneAt(0);
      const moon = scene.bodies.find((body) => body.id === terrainTestGolden().body);
      const ship = scene.craft[0];
      if (moon?.rotation === null || moon === undefined || ship === undefined) {
        throw new Error("the look's scene has its moon and ship");
      }
      expect(moon.rotation).toEqual(wholeLookRotation(look.over));
      const position = ship.pose.position;
      if (position.kind !== "body_fixed") {
        throw new Error("the ship stands in the moon's fixed frame");
      }
      const fromCentre = rotateToBody(moon.rotation, position.m);
      expect(norm(fromCentre) / wholeLookDistanceM(look)).toBeCloseTo(1, 12);
      // Between the moon and the star (the moon 1 au out along +x), on the line to it.
      expect(normalise(fromCentre).x).toBeCloseTo(-1, 12);
      const forward = rotate(ship.pose.attitude, vec3(0, 0, -1));
      expect(forward.x).toBeCloseTo(1, 12);
      expect(moon.centreM).toBe(TERRAIN_MOON_CENTRE_M);
      expect(scene.provenance).toEqual({ kind: "kept", name: look.scene });
    }
  });

  it("offers each look with a fixture of its own, the whole body's", () => {
    const options = wholeLookSceneOptions();
    expect(options.map((option) => option.name)).toEqual(WHOLE_LOOKS.map((look) => look.scene));
    expect(new Set(options.map((option) => option.terrain)).size).toBe(WHOLE_LOOKS.length);
    expect(options.every((option) => option.terrain !== undefined)).toBe(true);
  });

  it("stands its looks at the old patch, a cube corner and the patch's antipode", () => {
    const [orbit, corner, antipode] = WHOLE_LOOKS;
    if (orbit === undefined || corner === undefined || antipode === undefined) {
      throw new Error("three looks");
    }
    expect(norm(sub(normalise(orbit.over), normalise(vec3(1, 1, 0.8))))).toBeCloseTo(0, 12);
    expect(norm(sub(normalise(corner.over), normalise(vec3(1, 1, 1))))).toBeCloseTo(0, 12);
    expect(norm(sub(normalise(antipode.over), normalise(vec3(-1, -1, -0.8))))).toBeCloseTo(0, 12);
  });
});

/** A frame of 64 × 48 px across 60°, the disc's radius some 14 px. */
const FRAME: WholeDiscFrame = {
  widthPx: 64,
  heightPx: 48,
  fovXRad: Math.PI / 3,
  distanceM: 4e6,
  radiusM: 1e6,
  reliefM: 1e4,
};

/** The frame's texels, each pixel's value `at` of its distance from the centre, px. */
function frameOf(at: (distancePx: number, x: number, y: number) => number): Float16Array {
  const halves = new Float16Array(4 * FRAME.widthPx * FRAME.heightPx);
  for (let y = 0; y < FRAME.heightPx; y += 1) {
    for (let x = 0; x < FRAME.widthPx; x += 1) {
      const value = at(Math.hypot(x + 0.5 - 32, y + 0.5 - 24), x, y);
      halves.set([value, value, value, 1], 4 * (y * FRAME.widthPx + x));
    }
  }
  return halves;
}

const WHOLE_RECT = { x: 0, y: 0, width: 64, height: 48 } as const;

describe("a whole-body look's frame read", () => {
  it("finds no sky in a lit disc, and nothing lit beyond its limb", () => {
    const reading = readWholeDisc(
      frameOf((d) => (d < 15 ? 2 : 0)),
      WHOLE_RECT,
      FRAME,
    );
    expect(reading.discPixels).toBeGreaterThan(400);
    expect(reading.outsidePixels).toBeGreaterThan(1_500);
    expect([reading.skyPixels, reading.litOutsidePixels, reading.finite]).toEqual([0, 0, true]);
  });

  it("counts a hole in the disc as the sky's, and ground beyond the limb as lit", () => {
    const halves = frameOf((d, x, y) =>
      (x === 33 && y === 24) || d < 15
        ? x === 33 && y === 24
          ? 0
          : 2
        : x === 2 && y === 2
          ? 2
          : 0,
    );
    const reading = readWholeDisc(halves, WHOLE_RECT, FRAME);
    expect([reading.skyPixels, reading.litOutsidePixels]).toEqual([1, 1]);
  });

  it("reads a texel that is not finite as such", () => {
    const halves = frameOf((d) => (d < 15 ? 2 : 0));
    halves[0] = Number.NaN;
    expect(readWholeDisc(halves, WHOLE_RECT, FRAME).finite).toBe(false);
  });
});
