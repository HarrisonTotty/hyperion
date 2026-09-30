import { describe, expect, it } from "vitest";

import { dot, norm, normalise, sub, vec3, type Vec3 } from "../../geometry/vec3";
import { rotate } from "../camera/quaternion";
import { rotateToBody } from "../coords/rotation";
import {
  FRAME_CHANGE_DURATION_S,
  FRAME_CHANGE_LANDER,
  FRAME_CHANGE_LANDER_AT,
  FRAME_CHANGE_MOON_HILL_M,
  FRAME_CHANGE_MOON_M,
  FRAME_CHANGE_PLANET_HILL_M,
  FRAME_CHANGE_PLANET_M,
  frameChangeCameraAt,
  frameChangeRotationAt,
  frameChangeScene,
  frameChangeShipAt,
} from "./frameChange";
import { keptBody, keptTime } from "./kept";

/** Where along a path, in steps of a second, it is inside a sphere: "o" outside, "i" inside. */
function passes(path: (tS: number) => Vec3, centreM: Vec3, radiusM: number): string {
  let trace = "";
  for (let tS = 0; tS <= FRAME_CHANGE_DURATION_S; tS += 1) {
    const mark = norm(sub(path(tS), centreM)) < radiusM ? "i" : "o";
    if (!trace.endsWith(mark)) {
      trace += mark;
    }
  }
  return trace;
}

describe("the frame-change scene's paths", () => {
  it("take the own ship into the planet's Hill sphere and out", () => {
    expect(passes(frameChangeShipAt, FRAME_CHANGE_PLANET_M, FRAME_CHANGE_PLANET_HILL_M)).toBe(
      "oio",
    );
  });

  it("take the own ship into the moon's Hill sphere and out", () => {
    expect(passes(frameChangeShipAt, FRAME_CHANGE_MOON_M, FRAME_CHANGE_MOON_HILL_M)).toBe("oio");
  });

  it("take the free camera into both Hill spheres and out", () => {
    expect([
      passes(frameChangeCameraAt, FRAME_CHANGE_PLANET_M, FRAME_CHANGE_PLANET_HILL_M),
      passes(frameChangeCameraAt, FRAME_CHANGE_MOON_M, FRAME_CHANGE_MOON_HILL_M),
    ]).toEqual(["oio", "oio"]);
  });
});

describe("the frame-change scene's planet", () => {
  it("has a proper rotation at the script's start and end", () => {
    expect(() => [
      frameChangeRotationAt(0),
      frameChangeRotationAt(FRAME_CHANGE_DURATION_S),
    ]).not.toThrow();
  });

  it("stands the lander upright on its surface", () => {
    const tS = 50;
    const lander = frameChangeScene()
      .sceneAt(tS)
      .craft.find((c) => c.id === FRAME_CHANGE_LANDER);
    if (lander === undefined) {
      throw new Error("the scene has no lander");
    }
    const up = normalise(rotateToBody(frameChangeRotationAt(tS), FRAME_CHANGE_LANDER_AT.m));
    expect(dot(rotate(lander.pose.attitude, vec3(0, 1, 0)), up)).toBeCloseTo(1, 12);
  });
});

describe("the kept scenes' helpers", () => {
  it("names a body of the kept system by its index", () => {
    expect(keptBody(3)).toBe("0200080020000000.0003");
  });

  it("keeps a script time's nanoseconds below a second", () => {
    expect(keptTime(1.999_999_999_9)).toEqual({ seconds: 1, nanos: 999_999_999 });
  });
});
