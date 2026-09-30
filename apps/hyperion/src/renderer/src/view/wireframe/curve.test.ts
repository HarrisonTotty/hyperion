import { describe, expect, it } from "vitest";

import { vec3 } from "../../geometry/vec3";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION } from "../camera/quaternion";
import { cosineWindows, sampleCurve } from "./curve";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };
const CAMERA: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

/** A circle of radius `radiusM` about the camera's line of sight, `distanceM` ahead. */
function circleAhead(radiusM: number, distanceM: number): (u: number) => ReturnType<typeof vec3> {
  return (u) => vec3(radiusM * Math.cos(u), radiusM * Math.sin(u), -distanceM);
}

describe("cosineWindows", () => {
  it("gives the arc where α cos u + β sin u exceeds the threshold", () => {
    // cos u > 0.5: |u| < 60°.
    const [window] = cosineWindows(1, 0, 0.5, -Math.PI, Math.PI);
    expect(window?.map((u) => Number(((u * 180) / Math.PI).toFixed(9)))).toEqual([-60, 60]);
  });

  it("gives nothing where the threshold is out of reach, and the whole range below reach", () => {
    expect([cosineWindows(1, 0, 1.5, 0, 1), cosineWindows(1, 0, -1.5, 0, 1)]).toEqual([
      [],
      [[0, 1]],
    ]);
  });

  it("splits an arc across 2π into two pieces of the range", () => {
    // cos u > 0.9 about u = 0, in [0, 2π): two pieces, one at each end.
    const windows = cosineWindows(1, 0, 0.9, 0, 2 * Math.PI);
    expect(windows.length).toBe(2);
  });
});

describe("sampleCurve", () => {
  it("finds a drawn window however short against the first spans", () => {
    const runs = sampleCurve(
      { point: circleAhead(10, 100), windows: [[0.1, 0.11]], initialSpans: 1 },
      CAMERA,
      VIEWPORT,
    );
    expect(runs.length).toBe(1);
  });

  it("ends a run where the curve passes behind the near plane and starts another after", () => {
    // A circle about the camera in the x–z plane: half of it is behind the camera.
    const runs = sampleCurve(
      {
        point: (u) => vec3(100 * Math.sin(u), 0, -100 * Math.cos(u)),
        windows: [[-Math.PI * 0.4, Math.PI * 1.6]],
        initialSpans: 4,
      },
      CAMERA,
      VIEWPORT,
    );
    // Every drawn point is in front of the near plane, and the part behind is left out.
    const zs = runs.flat().map((p) => p.z);
    expect({ behind: zs.filter((z) => z > -0.1 + 1e-9).length, runs: runs.length }).toEqual({
      behind: 0,
      runs: 2,
    });
  });

  it("does not refine a span wholly off one side of the view", () => {
    const off = sampleCurve(
      { point: circleAhead(1e6, 10), windows: [[-0.1, 0.1]], initialSpans: 1 },
      CAMERA,
      VIEWPORT,
    );
    expect(off.flat().length).toBe(2);
  });

  it("refines a curve on the view until its chords leave under 0.25 px", () => {
    const runs = sampleCurve(
      { point: circleAhead(10, 20), windows: [[0, 2 * Math.PI]], initialSpans: 4 },
      CAMERA,
      VIEWPORT,
    );
    // A circle 747 px in radius on screen takes about π √(r ÷ 2s) ≈ 120 chords at s = 0.25 px.
    expect(runs.flat().length).toBeGreaterThan(100);
  });
});
