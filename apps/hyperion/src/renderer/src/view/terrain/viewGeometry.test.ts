import { describe, expect, it } from "vitest";

import { normalise, vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { seededRandom } from "../../test/seededRandom";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { distanceToBoxM, type PatchBounds, patchBounds, relativeBounds } from "./bounds";
import { aboveHorizon, frustumOf, horizonCone, inFrustum } from "./cull";
import { FACES, MAX_LEVEL, type PatchKey } from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import { distanceToBoxFromM, type ViewGeometry, viewExcess, viewGeometry } from "./viewGeometry";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

/**
 * `viewExcess` as R05.T7 perf (c) left it, the oracle of perf (d)'s form: the frustum's planes in
 * order with every box reach taken, then the horizon's corners from the lowest, then the distance.
 */
function formerViewExcess(v: ViewGeometry, b: PatchBounds, errorM: number): number {
  const box = b.box;
  const [a0, a1, a2] = box.axes;
  const [e0, e1, e2] = box.halfExtentsM;
  const cx = box.centre.x - v.cameraX;
  const cy = box.centre.y - v.cameraY;
  const cz = box.centre.z - v.cameraZ;
  const r = b.radiusM;
  const planes = v.planes;
  if (cx * cx + cy * cy + cz * cz > r * r) {
    for (let p = 0; p < planes.length; p += 4) {
      const nx = planes[p] ?? 0;
      const ny = planes[p + 1] ?? 0;
      const nz = planes[p + 2] ?? 0;
      const centre = nx * cx + ny * cy + nz * cz + (planes[p + 3] ?? 0);
      if (centre < -r) {
        return -1;
      }
      const reach =
        centre +
        Math.abs(nx * a0.x + ny * a0.y + nz * a0.z) * e0 +
        Math.abs(nx * a1.x + ny * a1.y + nz * a1.z) * e1 +
        Math.abs(nx * a2.x + ny * a2.y + nz * a2.z) * e2;
      if (reach < 0) {
        return -1;
      }
    }
  }
  if (v.horizonSq > 0) {
    const ro = v.occluderRadiusM;
    let visible = false;
    for (let corner = 0; corner < 8 && !visible; corner += 1) {
      const s0 = (corner & 1) === 0 ? -e0 : e0;
      const s1 = (corner & 2) === 0 ? -e1 : e1;
      const s2 = (corner & 4) === 0 ? -e2 : e2;
      const x = (cx + a0.x * s0 + a1.x * s1 + a2.x * s2) / ro;
      const y = (cy + a0.y * s0 + a1.y * s1 + a2.y * s2) / ro;
      const z = (cz + a0.z * s0 + a1.z * s1 + a2.z * s2) / ro;
      const vtDotVc = -(x * v.scaledX + y * v.scaledY + z * v.scaledZ);
      visible = !(
        vtDotVc > v.horizonSq && (vtDotVc * vtDotVc) / (x * x + y * y + z * z) > v.horizonSq
      );
    }
    if (!visible) {
      return -1;
    }
  }
  const o0 = Math.max(0, Math.abs(cx * a0.x + cy * a0.y + cz * a0.z) - e0);
  const o1 = Math.max(0, Math.abs(cx * a1.x + cy * a1.y + cz * a1.z) - e1);
  const o2 = Math.max(0, Math.abs(cx * a2.x + cy * a2.y + cz * a2.z) - e2);
  const d = Math.max(Math.sqrt(o0 * o0 + o1 * o1 + o2 * o2), NEAR_PLANE_M);
  return (errorM * v.excessPerMetre) / d;
}

describe("a view's prepared geometry", () => {
  it("culls and measures as cull.ts and bounds.ts do, over random cameras and patches", () => {
    const random = seededRandom(0x76696577);
    let seen = 0;
    let culled = 0;
    const mismatches: string[] = [];
    for (let n = 0; n < 300; n += 1) {
      const dir = normalise(vec3(random() - 0.5, random() - 0.5, random() - 0.5));
      const altitudeM = 10 ** (random() * 7);
      const [cx, cy, cz] = surfacePoint(WGS84_FIGURE, [dir.x, dir.y, dir.z], altitudeM);
      const camera = vec3(cx, cy, cz);
      const look = normalise(vec3(random() - 0.5, random() - 0.5, random() - 0.5));
      const orientation = lookAlong(look, Math.abs(look.z) > 0.9 ? vec3(1, 0, 0) : vec3(0, 0, 1));
      const fovXRad = 0.2 + random() * 2;
      const viewport = { widthPx: 1920, heightPx: 1080 };
      const frustum = frustumOf({ orientation, fovXRad, viewport });
      const horizon = horizonCone(PLANET, camera);
      const excessPerMetre = 1234.5;
      const geometry = viewGeometry(frustum, camera, horizon.occluderRadiusM, excessPerMetre);
      for (let m = 0; m < 20; m += 1) {
        const level = Math.floor(random() * (MAX_LEVEL - 4));
        const side = 2 ** level;
        const key: PatchKey = {
          face: FACES[Math.floor(random() * 6)] ?? 0,
          level,
          i: Math.floor(random() * side),
          j: Math.floor(random() * side),
        };
        const b = patchBounds(PLANET, key);
        const rel = relativeBounds(b, camera);
        const visible = inFrustum(rel, frustum) && aboveHorizon(rel, horizon);
        const errorM = 1 + random() * 100;
        const expected = visible
          ? (errorM * excessPerMetre) / Math.max(distanceToBoxM(rel), NEAR_PLANE_M)
          : -1;
        const got = viewExcess(geometry, b, errorM);
        if (visible) {
          seen += 1;
        } else {
          culled += 1;
        }
        if (Math.abs(got - expected) > 1e-9 * Math.max(1, Math.abs(expected))) {
          mismatches.push(`${n}/${m}: ${got} ≠ ${expected}`);
        }
      }
    }
    expect(mismatches).toEqual([]);
    // Both outcomes are exercised.
    expect(seen).toBeGreaterThan(200);
    expect(culled).toBeGreaterThan(200);
  });
});

describe("a view's excess test", () => {
  it("gives the former form's excess to the bit, near the ground, in orbit and inside patches", () => {
    const random = seededRandom(0x65786373);
    const mismatches: string[] = [];
    let seen = 0;
    let culled = 0;
    for (let n = 0; n < 400; n += 1) {
      const level = Math.floor(random() * (MAX_LEVEL - 2));
      const side = 2 ** level;
      const key: PatchKey = {
        face: FACES[Math.floor(random() * 6)] ?? 0,
        level,
        i: Math.floor(random() * side),
        j: Math.floor(random() * side),
      };
      const b = patchBounds(PLANET, key);
      // About the patch: inside its box, just above it, a few radii off, or in orbit.
      const reach = b.radiusM * ([0.1, 1.5, 6][n % 3] ?? 1);
      const up = normalise(b.centre);
      const lift = n % 4 === 0 ? 4e5 : (random() - 0.2) * reach;
      const camera = vec3(
        b.centre.x + (random() - 0.5) * reach + up.x * lift,
        b.centre.y + (random() - 0.5) * reach + up.y * lift,
        b.centre.z + (random() - 0.5) * reach + up.z * lift,
      );
      const look = normalise(vec3(random() - 0.5, random() - 0.5, random() - 0.5));
      const orientation = lookAlong(look, Math.abs(look.z) > 0.9 ? vec3(1, 0, 0) : vec3(0, 0, 1));
      const frustum = frustumOf({
        orientation,
        fovXRad: 0.3 + random() * 2,
        viewport: { widthPx: 1920, heightPx: 1080 },
      });
      const geometry = viewGeometry(
        frustum,
        camera,
        horizonCone(PLANET, camera).occluderRadiusM,
        987.6,
      );
      for (let m = 0; m < 25; m += 1) {
        // The patch itself, then its neighbourhood's.
        const other =
          m === 0
            ? b
            : patchBounds(PLANET, {
                ...key,
                i: Math.min(side - 1, Math.max(0, key.i + Math.floor(random() * 7) - 3)),
                j: Math.min(side - 1, Math.max(0, key.j + Math.floor(random() * 7) - 3)),
              });
        const errorM = random() * 50;
        const want = formerViewExcess(geometry, other, errorM);
        const got = viewExcess(geometry, other, errorM);
        if (want < 0) {
          culled += 1;
        } else {
          seen += 1;
        }
        if (!Object.is(got, want)) {
          mismatches.push(`${n}/${m}: ${got} ≠ ${want}`);
        }
      }
    }
    expect(mismatches).toEqual([]);
    expect(seen).toBeGreaterThan(1_000);
    expect(culled).toBeGreaterThan(1_000);
  });
});

describe("the distance to a patch's box from a point", () => {
  it("is bounds.ts's to the number, over random points and patches", () => {
    const random = seededRandom(0x64697374);
    const mismatches: string[] = [];
    let inside = 0;
    for (let n = 0; n < 2_000; n += 1) {
      const level = Math.floor(random() * (MAX_LEVEL - 4));
      const side = 2 ** level;
      const key: PatchKey = {
        face: FACES[Math.floor(random() * 6)] ?? 0,
        level,
        i: Math.floor(random() * side),
        j: Math.floor(random() * side),
      };
      const b = patchBounds(PLANET, key);
      // Points about the patch, some inside its box, as a contact on the ground would be.
      const spread = b.radiusM * (random() < 0.3 ? 0.2 : 3);
      const point = vec3(
        b.centre.x + (random() - 0.5) * spread,
        b.centre.y + (random() - 0.5) * spread,
        b.centre.z + (random() - 0.5) * spread,
      );
      const want = distanceToBoxM(relativeBounds(b, point));
      const got = distanceToBoxFromM(b, point);
      if (want === 0) {
        inside += 1;
      }
      if (!Object.is(got, want)) {
        mismatches.push(`${n}: ${got} ≠ ${want}`);
      }
    }
    expect(mismatches).toEqual([]);
    expect(inside).toBeGreaterThan(100);
  });
});
