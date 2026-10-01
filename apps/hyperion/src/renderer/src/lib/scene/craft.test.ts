import { METRES_PER_LIGHT_YEAR } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import { predictedPath } from "./craft";
import type { SceneCraft, SceneKinematics } from "./model";

const AT = { seconds: 1_000, nanos: 250_000_000 };

function craft(state: SceneKinematics, plannedPath: SceneCraft["plannedPath"] = null): SceneCraft {
  return {
    craft: "ISV-1",
    hull: "corvette",
    state,
    attitude: { w: 1, x: 0, y: 0, z: 0 },
    angularVelocityRadPerS: { x: 0, y: 0, z: 0 },
    plannedPath,
  };
}

const IN_SYSTEM: SceneKinematics = {
  position: { kind: "system", system: FIXTURE_SYSTEM, offsetM: { x: 1e11, y: 0, z: 0 } },
  velocityMPerS: { x: 0, y: 7_000, z: -10 },
  time: AT,
};

describe("predictedPath", () => {
  it("returns a planned path unchanged", () => {
    const planned: SceneKinematics[] = [
      IN_SYSTEM,
      { ...IN_SYSTEM, time: { seconds: 1_060, nanos: 0 } },
    ];

    expect(predictedPath(craft(IN_SYSTEM, planned), 600)).toBe(planned);
  });

  it("otherwise extrapolates the pose in a straight line at its velocity", () => {
    expect(predictedPath(craft(IN_SYSTEM), 60.5)).toEqual([
      IN_SYSTEM,
      {
        position: {
          kind: "system",
          system: FIXTURE_SYSTEM,
          offsetM: { x: 1e11, y: 423_500, z: -605 },
        },
        velocityMPerS: IN_SYSTEM.velocityMPerS,
        time: { seconds: 1_060, nanos: 750_000_000 },
      },
    ]);
  });

  it("carries a galactic pose across a light-year cell", () => {
    const start: SceneKinematics = {
      position: {
        kind: "galactic",
        position: { cell_ly: [10, -3, 0], offset_m: [METRES_PER_LIGHT_YEAR - 1_000, 0, 5] },
      },
      velocityMPerS: { x: 100, y: -1, z: 0 },
      time: AT,
    };

    const [, end] = predictedPath(craft(start), 30);

    expect(end?.position).toEqual({
      kind: "galactic",
      position: { cell_ly: [11, -4, 0], offset_m: [2_000, METRES_PER_LIGHT_YEAR - 30, 5] },
    });
  });

  it("refuses a negative horizon", () => {
    expect(() => predictedPath(craft(IN_SYSTEM), -1)).toThrow(RangeError);
  });
});
