import { describe, expect, it } from "vitest";

import { normalise, vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M } from "../camera/projection";
import { lookAlong } from "../camera/quaternion";
import { seededRandom } from "../../test/seededRandom";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { distanceToBoxM, patchBounds, relativeBounds } from "./bounds";
import { aboveHorizon, frustumOf, horizonCone, inFrustum } from "./cull";
import { FACES, MAX_LEVEL, type PatchKey } from "./patchKey";
import { planetGeometry, surfacePoint } from "./planet";
import { distanceToBoxFromM, viewExcess, viewGeometry } from "./viewGeometry";

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

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
