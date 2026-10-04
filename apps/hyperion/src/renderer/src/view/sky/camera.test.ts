import { galacticPositionFromLy } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import {
  aCameraPose,
  aViewScene,
  FIXTURE_PLANET,
  FIXTURE_PLANET_CENTRE_M,
  FIXTURE_SYSTEM,
} from "../../test/viewFixtures";
import { galacticDeltaM } from "../coords/position";
import { cameraFromObserverM } from "./camera";

/** `aViewScene`'s barycentre. */
const BARYCENTRE = galacticPositionFromLy([8_000, 26_000, 20]);

describe("cameraFromObserverM", () => {
  it("is a system-frame camera's offset from a sky observed at the barycentre", () => {
    const pose = aCameraPose({
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: vec3(1e8, -2e8, 3e7),
    });
    const offset = cameraFromObserverM(pose, aViewScene(), BARYCENTRE);
    expect(offset?.x).toBeCloseTo(1e8, 0);
    expect(offset?.y).toBeCloseTo(-2e8, 0);
    expect(offset?.z).toBeCloseTo(3e7, 0);
  });

  it("adds a body frame's origin to the camera's offset from the body", () => {
    const pose = aCameraPose({
      frame: { kind: "body", body: FIXTURE_PLANET },
      positionM: vec3(0, 7e6, 0),
    });
    const offset = cameraFromObserverM(pose, aViewScene(), BARYCENTRE);
    expect(offset?.x).toBeCloseTo(FIXTURE_PLANET_CENTRE_M.x, -1);
    expect(offset?.y).toBeCloseTo(7e6, 0);
  });

  it("measures from another observer by the difference of the two", () => {
    const observer = galacticPositionFromLy([8_000, 26_000, 21]);
    const pose = aCameraPose({
      frame: { kind: "system", system: FIXTURE_SYSTEM },
      positionM: vec3(0, 0, 0),
    });
    const offset = cameraFromObserverM(pose, aViewScene(), observer);
    expect(offset?.z).toBeCloseTo(galacticDeltaM(observer, BARYCENTRE).z, -1);
  });

  it("is null where the scene's system has no known position", () => {
    expect(
      cameraFromObserverM(aCameraPose(), aViewScene({ barycentre: null }), BARYCENTRE),
    ).toBeNull();
  });
});
