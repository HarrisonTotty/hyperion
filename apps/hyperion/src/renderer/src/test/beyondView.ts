/**
 * Spheres just off a view, for the tests of what is drawn for nothing wholly beyond one of the
 * view's side planes (`sphereOutsideView`, R07.T19 and T19.e): R07's lit bodies and their
 * footprints, R06's host discs and R02's occluder spheres.
 */
import { add, normalise, scale, type Vec3, vec3 } from "../geometry/vec3";
import type { Viewport } from "../view/camera/projection";

/**
 * The centres of spheres just beyond each of an unturned camera's four side planes, each widened
 * by `marginPx` as `sphereOutsideView` widens them: on each side, beside the view, level with its
 * corner, across the camera's plane (90° off the axis) and behind the camera.
 *
 * @param fovXRad - The camera's horizontal field of view, rad.
 * @param distanceM - How far along the plane from the camera each centre stands, m.
 * @param beyondM - How far beyond the plane each centre stands, m: a sphere's radius and a little.
 */
export function centresJustBeyond(
  fovXRad: number,
  viewport: Viewport,
  marginPx: number,
  distanceM: number,
  beyondM: number,
): Vec3[] {
  const tanHalf = Math.tan(fovXRad / 2);
  const marginTan = (marginPx * 2 * tanHalf) / viewport.widthPx;
  const tanX = tanHalf + marginTan;
  const tanY = (tanHalf * viewport.heightPx) / viewport.widthPx + marginTan;
  const sides = [
    { out: vec3(1, 0, 0), along: vec3(0, 1, 0), tan: tanX, other: tanY },
    { out: vec3(-1, 0, 0), along: vec3(0, 1, 0), tan: tanX, other: tanY },
    { out: vec3(0, 1, 0), along: vec3(1, 0, 0), tan: tanY, other: tanX },
    { out: vec3(0, -1, 0), along: vec3(1, 0, 0), tan: tanY, other: tanX },
  ];
  return sides.flatMap(({ out, along, tan, other }) => {
    const normal = normalise(add(out, vec3(0, 0, tan)));
    const inPlane = add(scale(out, tan), vec3(0, 0, -1));
    return [inPlane, add(inPlane, scale(along, other)), along, scale(inPlane, -1)].map((p) =>
      add(scale(normalise(p), distanceM), scale(normal, beyondM)),
    );
  });
}

/**
 * The centre of a sphere `distanceM` away whose limb stands `pastPx` beyond the right edge of an
 * unturned camera's view, on the horizontal plane through the view's centre.
 */
export function limbPastRightEdge(
  fovXRad: number,
  viewport: Viewport,
  distanceM: number,
  radiusM: number,
  pastPx: number,
): Vec3 {
  const tanHalf = Math.tan(fovXRad / 2);
  const tan = tanHalf + (pastPx * 2 * tanHalf) / viewport.widthPx;
  return add(
    scale(normalise(vec3(tan, 0, -1)), distanceM),
    scale(normalise(vec3(1, 0, tan)), radiusM),
  );
}
