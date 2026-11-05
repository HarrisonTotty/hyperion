import { describe, expect, it } from "vitest";

import { norm, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { type ProjectionCamera, projector, type Viewport } from "../camera/projection";
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

/** Whether a chord's point stands within 0.01 m of the curve's. */
function withinACentimetre(curveM: Vec3, chordM: Vec3): boolean {
  return norm(sub(curveM, chordM)) <= 0.01;
}

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

  it("halves a span the view reaches until its ends' depths are within a factor of 2, a run cut at the near plane among them (R08.T17)", () => {
    // A moon's orbit about a camera near its focus: a circle of 10⁹ m in the x–z plane, half of it
    // behind the camera, cut where it meets the near plane, 0.1 m ahead and 10⁹ m to the side.
    const project = projector(CAMERA, VIEWPORT);
    const runs = sampleCurve(
      {
        point: (u) => vec3(1e9 * Math.sin(u), 0, -1e9 * Math.cos(u)),
        windows: [[-Math.PI, Math.PI]],
        initialSpans: 2,
      },
      CAMERA,
      VIEWPORT,
    );
    const reached = runs.flatMap((run) =>
      run.slice(1).flatMap((b, i) => {
        const a = project(run[i] ?? b);
        const z = project(b);
        const onView = Math.max(a.xPx, z.xPx) >= 0 && Math.min(a.xPx, z.xPx) <= VIEWPORT.widthPx;
        return onView ? [Math.max(a.depth, z.depth) / Math.min(a.depth, z.depth)] : [];
      }),
    );
    expect({ reached: reached.length > 0, widest: Math.max(...reached) <= 2 }).toEqual({
      reached: true,
      widest: true,
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

  /**
   * The near half of a circle 10 m in radius whose centre stands 100 m ahead, in a plane through
   * the view's axis: it projects to a straight line, so its chords leave no lateral sagitta, while
   * they sag behind it along the line of sight.
   */
  const EDGE_ON = {
    point: (u: number) => vec3(0, 10 * Math.sin(u), -100 + 10 * Math.cos(u)),
    windows: [[-1, 1]] as const,
    initialSpans: 2,
  };

  it("halves a span that projects straight until its chords' depth sag is within what its spec allows (R02.T19.b)", () => {
    // A chord over Δ sags 10 (1 − cos(Δ ÷ 2)) m, under 0.01 m from Δ = 0.0894: 1 rad in 16.
    expect([
      sampleCurve(EDGE_ON, CAMERA, VIEWPORT).flat().length,
      sampleCurve({ ...EDGE_ON, depthSagWithin: withinACentimetre }, CAMERA, VIEWPORT).flat()
        .length,
    ]).toEqual([3, 33]);
  });

  it("stops halving a span for its depth sag alone after 8 halvings (R02.T19.b)", () => {
    const runs = sampleCurve({ ...EDGE_ON, depthSagWithin: () => false }, CAMERA, VIEWPORT);
    expect(runs.flat().length).toBe(2 * 2 ** 8 + 1);
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
