import { galacticPositionFromLy } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { norm, sub, vec3 } from "../../geometry/vec3";
import {
  aCameraPose,
  aCameraScene,
  FIXTURE_PLANET,
  FIXTURE_SHIP,
  FIXTURE_SYSTEM,
} from "../../test/viewFixtures";
import type { ViewPosition } from "../coords/position";
import { relativeToCamera } from "../coords/relative";
import type { CameraFrame } from "./pose";
import { rebase, sameCameraFrame } from "./rebase";

const LANDMARK: ViewPosition = { kind: "body", body: FIXTURE_PLANET, m: vec3(-6.371e6, 0, 0) };

describe("rebase", () => {
  it("returns the pose unchanged, with no change, into the frame it is already in", () => {
    const pose = aCameraPose();
    const { origins } = aCameraScene();
    const result = rebase(pose, { kind: "system", system: FIXTURE_SYSTEM }, origins);
    expect(result.pose).toBe(pose);
    expect(result.change).toBeNull();
  });

  it("keeps the vector to a landmark across a change, and round-trips", () => {
    const { origins } = aCameraScene();
    const pose = aCameraPose({ positionM: vec3(1.49e11, 2e6, -3e5) });
    const body: CameraFrame = { kind: "body", body: FIXTURE_PLANET };
    const inBody = rebase(pose, body, origins);
    expect(inBody.change).toEqual({ from: pose.frame, to: body });
    const before = relativeToCamera(LANDMARK, pose, origins);
    const after = relativeToCamera(LANDMARK, inBody.pose, origins);
    expect(norm(sub(after, before))).toBeLessThan(1e-3);
    const back = rebase(inBody.pose, pose.frame, origins).pose;
    expect(norm(sub(back.positionM, pose.positionM))).toBeLessThan(1e-3);
    expect(back.orientation).toBe(pose.orientation);
  });

  it("re-expresses a pose held about a craft in the craft's own frame", () => {
    const { origins } = aCameraScene();
    const seat = aCameraPose({
      frame: { kind: "craft", craft: FIXTURE_SHIP },
      positionM: vec3(0, 1.5, -6),
    });
    const inBody = rebase(seat, { kind: "body", body: FIXTURE_PLANET }, origins).pose;
    expect(inBody.positionM).toEqual(vec3(2e7, 1.5, -6));
  });
});

describe("sameCameraFrame", () => {
  it("tells galactic frames apart by their origins' offsets", () => {
    const origin = galacticPositionFromLy([8_000, 26_000, 20]);
    const moved = {
      cell_ly: origin.cell_ly,
      offset_m: [origin.offset_m[0] + 1, origin.offset_m[1], origin.offset_m[2]] satisfies [
        number,
        number,
        number,
      ],
    };
    expect(sameCameraFrame({ kind: "galactic", origin }, { kind: "galactic", origin })).toBe(true);
    expect(sameCameraFrame({ kind: "galactic", origin }, { kind: "galactic", origin: moved })).toBe(
      false,
    );
  });

  it("tells frames of different kinds apart", () => {
    expect(
      sameCameraFrame(
        { kind: "system", system: FIXTURE_SYSTEM },
        { kind: "body", body: FIXTURE_PLANET },
      ),
    ).toBe(false);
  });
});
