import { beforeAll, describe, expect, it } from "vitest";

import { sub, type Vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { R05_EARTH_REFERENCE } from "./earth";
import { type AtmosphereCamera, type SunState, TABLE_SIZES } from "./hillaire";
import marchAreaWgsl from "./shaders/marchArea.wgsl?raw";
import {
  farLimbFootprintTwin,
  farLimbShellsOf,
  MARCH_SAME_SURFACE_SHARE,
  type PlanetTwin,
  planetTwin,
  rayMarchRayTwin,
  rayMarchTexelTwin,
  rayMarchTwin,
  type TwinView,
  twinView,
} from "./tablesCpu";

/** R05's Earth's sphere, m: WGS 84's equatorial radius a (NIMA TR8350.2). */
const RADIUS_M = 6_378_137;

/** The camera's distance from the centre, m: three radii, the disc some 40° across. */
const DISTANCE_M = 3 * RADIUS_M;

/** A small half-resolution frame: low's march over a 96 × 54 output, the disc some 64 px across. */
const VIEWPORT = { widthPx: 96, heightPx: 54 };

let planet: PlanetTwin;

beforeAll(() => {
  planet = planetTwin(R05_EARTH_REFERENCE, RADIUS_M);
});

/**
 * A view of the body's disc, lit at 45° of phase: the camera on +x, offset towards +z so that the
 * disc stands off the frame's centre, the sun out of the plane of the camera and the pole.
 */
function discView(): TwinView {
  const positionM: Vec3 = { x: DISTANCE_M, y: 0, z: 0.1 * RADIUS_M };
  const camera: AtmosphereCamera = {
    positionM,
    orientation: lookAlong(sub({ x: 0, y: 0.2 * RADIUS_M, z: 0 }, positionM), {
      x: 0,
      y: 0,
      z: 1,
    }),
    fovXRad: Math.PI / 3,
    viewport: VIEWPORT,
  };
  const sun: SunState = {
    directionBodyFixed: { x: 0.5, y: 0.5, z: Math.SQRT1_2 },
    illuminanceLx: [1, 1, 1],
    angularRadiusRad: 0.004_65,
  };
  return twinView(
    camera,
    sun,
    { equatorialRadiusM: RADIUS_M, polarRadiusM: RADIUS_M },
    NEAR_PLANE_M,
  );
}

/**
 * The distance along `view`'s unnormalised ray through (`u`, `v`) (forward + x right + y up, its
 * forward part 1, so the distance is the view-space depth) to the sphere's near side, or `null`
 * where it misses.
 */
function sphereDepthAlong(view: TwinView, u: number, v: number): number | null {
  const nx = 2 * u - 1;
  const ny = 1 - 2 * v;
  const w = {
    x: view.forward.x + nx * view.right.x + ny * view.upRay.x,
    y: view.forward.y + nx * view.right.y + ny * view.upRay.y,
    z: view.forward.z + nx * view.right.z + ny * view.upRay.z,
  };
  const o = view.cameraM;
  const a = w.x * w.x + w.y * w.y + w.z * w.z;
  const b = o.x * w.x + o.y * w.y + o.z * w.z;
  const c = o.x * o.x + o.y * o.y + o.z * o.z - RADIUS_M * RADIUS_M;
  const disc = b * b - a * c;
  if (!(disc >= 0)) {
    return null;
  }
  return (-b - Math.sqrt(disc)) / a;
}

describe("the half-resolution march over a drawn body (fix-disc-air-rows)", () => {
  // The body drawn as the frame draws it, its depth at each pixel's centre (reversed-Z, near ÷
  // the view-space depth), sky elsewhere. Each texel's centre ray passes through the corner of
  // four pixels, so that no one pixel's depth is the ray's: one nearer than the ray's own ground
  // ended it in the air, above the body (the dark rows and columns of pod-looks' captures).
  it("ends each texel's ray over the body on its ground, as a ray past the body does", () => {
    const view = discView();
    const sizes = TABLE_SIZES.low;
    const { widthPx: fullW, heightPx: fullH } = VIEWPORT;
    const depthAt = (x: number, y: number): number => {
      const z = sphereDepthAlong(view, (x + 0.5) / fullW, (y + 0.5) / fullH);
      return z === null ? 0 : NEAR_PLANE_M / z;
    };
    const target = rayMarchTwin(planet, view, sizes, depthAt);
    const shells = farLimbShellsOf(planet, view);
    // A depth past the body, at which the ray ends on its ground.
    const past = NEAR_PLANE_M / (10 * DISTANCE_M);
    const short: string[] = [];
    let over = 0;
    for (let y = 0; y < target.heightTexels; y += 1) {
      for (let x = 0; x < target.widthTexels; x += 1) {
        const u = (x + 0.5) / target.widthTexels;
        const v = (y + 0.5) / target.heightTexels;
        const texel = { x, y, widthTexels: target.widthTexels, heightTexels: target.heightTexels };
        // Over the body whole: its own pixel's, its ray's and its footprint's, none blending.
        const own = depthAt(
          Math.min(Math.floor(u * fullW), fullW - 1),
          Math.min(Math.floor(v * fullH), fullH - 1),
        );
        const footprint = shells === null ? null : farLimbFootprintTwin(shells, view, texel);
        if (
          !(own > 0) ||
          sphereDepthAlong(view, u, v) === null ||
          (footprint !== null && footprint.blend > 0)
        ) {
          continue;
        }
        over += 1;
        const expected = rayMarchTexelTwin(
          planet,
          view,
          sizes,
          rayMarchRayTwin(planet.medium, view, u, v, past, sizes),
        );
        const at = (y * target.widthTexels + x) * 4;
        const drawn = Array.from(target.texels.subarray(at, at + 4));
        if (drawn.some((value, c) => Math.abs(value - (expected[c] ?? Number.NaN)) > 1e-12)) {
          short.push(`(${String(x)}, ${String(y)})`);
        }
      }
    }
    expect([over > 300, short]).toEqual([true, []]);
  });

  it("holds the kernels' share of the same surface to the twin's", () => {
    expect(marchAreaWgsl).toContain(
      `const MARCH_SAME_SURFACE_SHARE : f32 = ${String(MARCH_SAME_SURFACE_SHARE)};`,
    );
  });
});
