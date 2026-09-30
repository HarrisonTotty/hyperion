import { describe, expect, it } from "vitest";

import { add, norm, sub, vec3 } from "../../geometry/vec3";
import { type KeplerOrbit, orbitPolyline } from "../../lib/orbit";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { ellipseOf, orbitPath } from "./orbits";

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };

/** The Moon's orbit, roughly: 3.844 × 10⁸ m, e 0.0549, inclined 5.1°. */
const ORBIT: KeplerOrbit = {
  semiMajorAxisM: 3.844e8,
  eccentricity: 0.0549,
  inclinationRad: (5.1 * Math.PI) / 180,
  ascendingNodeRad: 1.1,
  argumentOfPeriapsisRad: 0.4,
  meanAnomalyAtEpochRad: 0,
  periodS: 2.36e6,
};

/** A camera `distanceM` from the orbit's focus, looking at it from above and to the side. */
function cameraAt(distanceM: number): {
  camera: ProjectionCamera;
  parentM: ReturnType<typeof vec3>;
} {
  const parentM = vec3(0, 0.6 * distanceM, -0.8 * distanceM);
  return {
    camera: { orientation: lookAlong(parentM, vec3(0, 0, 1)), fovXRad: Math.PI / 3 },
    parentM,
  };
}

describe("ellipseOf", () => {
  it("is the propagated ellipse of lib/orbit.ts", () => {
    const ellipse = ellipseOf(ORBIT);
    const points = orbitPolyline(ORBIT, 360);
    const gaps = points.map((p, i) => norm(sub(p, ellipse((2 * Math.PI * i) / 360))));
    expect(Math.max(...gaps) / ORBIT.semiMajorAxisM).toBeLessThan(1e-12);
  });
});

describe("orbitPath", () => {
  it("stays within 0.25 px of the propagated curve between its points", () => {
    const { camera, parentM } = cameraAt(1.2e9);
    const ellipse = ellipseOf(ORBIT);
    const runs = orbitPath(ORBIT, parentM, camera, VIEWPORT);
    // Every true point of the curve lies within 0.25 px of the drawn polyline.
    const drawn = runs.flatMap((run) =>
      run
        .slice(1)
        .map(
          (b, i) => [project(run[i] ?? b, camera, VIEWPORT), project(b, camera, VIEWPORT)] as const,
        ),
    );
    let worstPx = 0;
    for (let k = 0; k < 4000; k += 1) {
      const p = project(add(parentM, ellipse((2 * Math.PI * k) / 4000)), camera, VIEWPORT);
      let nearest = Number.POSITIVE_INFINITY;
      for (const [a, b] of drawn) {
        const dx = b.xPx - a.xPx;
        const dy = b.yPx - a.yPx;
        const l2 = dx * dx + dy * dy;
        const t =
          l2 > 0 ? Math.min(1, Math.max(0, ((p.xPx - a.xPx) * dx + (p.yPx - a.yPx) * dy) / l2)) : 0;
        nearest = Math.min(nearest, Math.hypot(p.xPx - a.xPx - t * dx, p.yPx - a.yPx - t * dy));
      }
      worstPx = Math.max(worstPx, nearest);
    }
    expect(worstPx).toBeLessThan(0.25);
  });

  it("takes fewer points far away than near", () => {
    const count = (distanceM: number): number => {
      const { camera, parentM } = cameraAt(distanceM);
      return orbitPath(ORBIT, parentM, camera, VIEWPORT).reduce((n, run) => n + run.length, 0);
    };
    expect(count(1e11)).toBeLessThan(count(1.2e9));
  });
});
