import { add, scale, type Vec3 } from "../../geometry/vec3";
import { type KeplerOrbit, orbitPolyline } from "../../lib/orbit";
import type { ProjectionCamera, Viewport } from "../camera/projection";
import { type Polyline, sampleCurve } from "./curve";

/**
 * An orbit's ellipse from its focus as a function of the eccentric anomaly E, m in the axes of the
 * parent's frame: a (cos E − e) P + b sin E Q, the form `lib/orbit.ts` propagates.
 *
 * @remarks
 * P and Q are taken from `orbitPolyline`'s own points at E = 0 (a (1 − e) P) and E = π/2
 * (−a e P + b Q), so that the curve is the propagated one and its orientation is computed once, by
 * `lib/orbit.ts`.
 */
export function ellipseOf(orbit: KeplerOrbit): (eccentricAnomalyRad: number) => Vec3 {
  const [periapsis, quarter] = orbitPolyline(orbit, 4);
  if (periapsis === undefined || quarter === undefined) {
    throw new Error("an orbit's polyline has fewer points than it was asked for");
  }
  const a = orbit.semiMajorAxisM;
  const e = orbit.eccentricity;
  const b = a * Math.sqrt(1 - e * e);
  const p = scale(periapsis, 1 / (a * (1 - e)));
  const q = scale(add(quarter, scale(p, a * e)), 1 / b);
  return (eccentricAnomalyRad) =>
    add(
      scale(p, a * (Math.cos(eccentricAnomalyRad) - e)),
      scale(q, b * Math.sin(eccentricAnomalyRad)),
    );
}

/**
 * An orbit's path as the wireframe draws it, sampled by screen-space error rather than a fixed count
 * (plan R02, R02.T12.b): each chord leaves a sagitta under 0.25 px, so a far orbit takes few points
 * and a near one many.
 *
 * @param parentM - The orbit's focus (its parent's centre) from the camera, m.
 */
export function orbitPath(
  orbit: KeplerOrbit,
  parentM: Vec3,
  camera: ProjectionCamera,
  viewport: Viewport,
): ReadonlyArray<Polyline> {
  const ellipse = ellipseOf(orbit);
  return sampleCurve(
    {
      point: (eccentricAnomalyRad) => add(parentM, ellipse(eccentricAnomalyRad)),
      windows: [[0, 2 * Math.PI]],
      initialSpans: 16,
    },
    camera,
    viewport,
  );
}
