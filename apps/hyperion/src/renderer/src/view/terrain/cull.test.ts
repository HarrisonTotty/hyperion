import { describe, expect, it } from "vitest";

import { dot, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { lookAlong } from "../camera/quaternion";
import { seededRandom } from "../../test/seededRandom";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { type CameraRelativeBounds, type OrientedBox, patchBounds, relativeBounds } from "./bounds";
import { faceUvToDir, faceUvToXyz, stToUv, unitDir, uvToSt, vertexDir, xyzToFaceUv } from "./cube";
import { aboveHorizon, frustumOf, horizonCone, inFrustum } from "./cull";
import { FACES, MAX_LEVEL, type PatchKey, patchKeyString, rootKey } from "./patchKey";
import {
  type BodyFigure,
  levelHeightRangeM,
  lowestHeightM,
  type PlanetGeometry,
  planetGeometry,
  surfacePoint,
} from "./planet";

const R = 6_371_000;
const SPHERE: BodyFigure = { equatorialRadiusM: R, polarRadiusM: R, pole: null };
const VIEWPORT = { widthPx: 1920, heightPx: 1080 };

function v(p: readonly [number, number, number]): Vec3 {
  return vec3(p[0], p[1], p[2]);
}

/** A point-sized box at `pointFromCameraM`, for tests of the horizon alone. */
function pointBounds(pointFromCameraM: Vec3): CameraRelativeBounds {
  const box: OrientedBox = {
    centre: pointFromCameraM,
    axes: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)],
    halfExtentsM: [0, 0, 0],
  };
  return { centreM: pointFromCameraM, radiusM: 0, box };
}

/** The point at `radiusM` from the centre, `angleRad` round from +x towards +y. */
function atAngle(radiusM: number, angleRad: number): Vec3 {
  return vec3(radiusM * Math.cos(angleRad), radiusM * Math.sin(angleRad), 0);
}

describe("the horizon test", () => {
  const planet = planetGeometry(SPHERE, goldenLevelTable("off"));
  const occluder = R + lowestHeightM(planet);

  it("from 400 km, sees a patch just beyond the horizon at its highest and not at its lowest", () => {
    const camera = vec3(R + 400_000, 0, 0);
    const h = horizonCone(planet, camera);
    const high = occluder + 9_000;
    const low = occluder + 1;
    // Beyond the occluder's horizon by half the high point's own dip.
    const angle = Math.acos(occluder / camera.x) + 0.5 * Math.acos(occluder / high);
    expect(aboveHorizon(pointBounds(sub(atAngle(high, angle), camera)), h)).toBe(true);
    expect(aboveHorizon(pointBounds(sub(atAngle(low, angle), camera)), h)).toBe(false);
  });

  it("from 300 km, sees a 9 km peak beyond the datum's horizon", () => {
    const camera = vec3(R + 300_000, 0, 0);
    const peak = R + 9_000;
    const angle = Math.acos(R / camera.x) + 0.9 * Math.acos(R / peak);
    expect(
      aboveHorizon(pointBounds(sub(atAngle(peak, angle), camera)), horizonCone(planet, camera)),
    ).toBe(true);
  });

  it("is disabled below the occluder's radius", () => {
    const camera = vec3(occluder - 1, 0, 0);
    const behind = sub(vec3(-R, 0, 0), camera);
    expect(aboveHorizon(pointBounds(behind), horizonCone(planet, camera))).toBe(true);
  });

  it("calls exact tangency visible", () => {
    // On the unit occluder from (2, 0, 0) the tangent point is (½, √3 ÷ 2, 0), exactly on the cone.
    const h = { cameraM: vec3(2, 0, 0), occluderRadiusM: 1 };
    expect(aboveHorizon(pointBounds(vec3(-1.5, Math.sqrt(3) / 2, 0)), h)).toBe(true);
    expect(aboveHorizon(pointBounds(vec3(-2.5, 0.1, 0)), h)).toBe(false);
  });

  it("culls the far side's faces from orbit and keeps the near one", () => {
    const camera = vec3(R + 400_000, 0, 0);
    const h = horizonCone(planet, camera);
    const visible = FACES.map((f) =>
      aboveHorizon(relativeBounds(patchBounds(planet, rootKey(f)), camera), h),
    );
    // Face 0 (+x) faces the camera, face 3 (−x) is behind the planet; the four others reach past
    // the horizon at their edges.
    expect(visible[0]).toBe(true);
    expect(visible[3]).toBe(false);
  });
});

describe("the frustum test", () => {
  const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));

  it("keeps a patch larger than the frustum, the camera outside its sphere", () => {
    const key = rootKey(0);
    const b = patchBounds(planet, key);
    const camera = vec3(3 * WGS84_FIGURE.equatorialRadiusM, 0, 0);
    const rel = relativeBounds(b, camera);
    // Outside the sphere, so the planes decide, and a 10° view the face overfills.
    expect(Math.sqrt(dot(rel.centreM, rel.centreM))).toBeGreaterThan(rel.radiusM);
    const f = frustumOf({
      orientation: lookAlong(vec3(-1, 0, 0), vec3(0, 0, 1)),
      fovXRad: (10 * Math.PI) / 180,
      viewport: VIEWPORT,
    });
    expect(inFrustum(rel, f)).toBe(true);
  });

  it("culls a box wholly outside a side plane that its bounding sphere crosses", () => {
    // Looking down −z with a 60° square view: the +x side plane's inward normal is (cos 30°, 0,
    // −sin 30°). The box's centre is 2.7 m outside it and its half-extents 0.1 m; the sphere of
    // 3 m crosses the plane.
    const f = frustumOf({
      orientation: { w: 1, x: 0, y: 0, z: 0 },
      fovXRad: Math.PI / 3,
      viewport: { widthPx: 1000, heightPx: 1000 },
    });
    const centre = vec3(-6, 0, -5);
    const bounds = {
      centreM: centre,
      radiusM: 3,
      box: {
        centre,
        axes: [vec3(1, 0, 0), vec3(0, 1, 0), vec3(0, 0, 1)] as const,
        halfExtentsM: [0.1, 0.1, 0.1] as const,
      },
    };
    expect(inFrustum(bounds, f)).toBe(false);
    expect(inFrustum({ ...bounds, box: { ...bounds.box, centre: vec3(-2, 0, -5) } }, f)).toBe(true);
  });

  it("keeps a patch the camera is inside, whichever way it looks", () => {
    const key: PatchKey = { face: 1, level: 14, i: 9000, j: 9000 };
    const b = patchBounds(planet, key);
    const f = frustumOf({
      orientation: lookAlong(vec3(1, 1, 1), vec3(0, 0, 1)),
      fovXRad: Math.PI / 6,
      viewport: VIEWPORT,
    });
    expect(inFrustum(relativeBounds(b, b.centre), f)).toBe(true);
  });

  it("culls a patch behind the camera and keeps one ahead", () => {
    const key: PatchKey = { face: 0, level: 12, i: 2048, j: 2048 };
    const ground = v(surfacePoint(WGS84_FIGURE, vertexDir(key, 32, 32), 0));
    const up = normalise(ground);
    const camera = sub(ground, scale(up, -100_000));
    const b = relativeBounds(patchBounds(planet, key), camera);
    const lookingDown = frustumOf({
      orientation: lookAlong(scale(up, -1), vec3(0, 0, 1)),
      fovXRad: Math.PI / 3,
      viewport: VIEWPORT,
    });
    const lookingUp = frustumOf({
      orientation: lookAlong(up, vec3(0, 0, 1)),
      fovXRad: Math.PI / 3,
      viewport: VIEWPORT,
    });
    expect([inFrustum(b, lookingDown), inFrustum(b, lookingUp)]).toEqual([true, false]);
  });
});

/** Whether the segment from `camera` to `point` misses the occluder sphere (radius `r`, centre 0). */
function seenPastOccluder(camera: Vec3, point: Vec3, r: number): boolean {
  const d = sub(point, camera);
  const t = Math.min(1, Math.max(0, -dot(camera, d) / dot(d, d)));
  const nearest = vec3(camera.x + t * d.x, camera.y + t * d.y, camera.z + t * d.z);
  return dot(nearest, nearest) > r * r;
}

/** A random point of the patch's surface at a random height of its range, body-fixed metres. */
function surfaceSample(
  planet: PlanetGeometry,
  key: PatchKey,
  heightsM: readonly [number, number],
  random: () => number,
): Vec3 {
  const side = 2 ** key.level;
  const u = stToUv((key.i + random()) / side);
  const w = stToUv((key.j + random()) / side);
  const dir = unitDir(faceUvToXyz(key.face, u, w));
  const h = heightsM[0] + random() * (heightsM[1] - heightsM[0]);
  return v(surfacePoint(planet.figure, dir, h));
}

describe("the horizon test against a brute-force oracle", () => {
  it("culls no patch any ray from 1,000 random cameras reaches past the occluder", () => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const random = seededRandom(0x686f7269);
    const falseCulls: string[] = [];
    let culled = 0;
    for (let n = 0; n < 1000; n += 1) {
      const altitudeM = 10 ** (random() * 7.7);
      const dir = normalise(vec3(random() - 0.5, random() - 0.5, random() - 0.5));
      const camera = v(surfacePoint(WGS84_FIGURE, [dir.x, dir.y, dir.z], altitudeM));
      const h = horizonCone(planet, camera);
      // A patch whose centre lies about the camera's horizon distance away, where culls happen.
      const level = Math.floor(random() * 12);
      const target = normalise(
        vec3(dir.x + random() - 0.5, dir.y + random() - 0.5, dir.z + random() - 0.5),
      );
      const face =
        FACES.find((f) => {
          const centre = faceUvToDir(f, 0, 0);
          return (
            dot(centre, target) >= Math.max(...FACES.map((g) => dot(faceUvToDir(g, 0, 0), target)))
          );
        }) ?? 0;
      const side = 2 ** level;
      const key: PatchKey = {
        face,
        level,
        i: Math.floor(random() * side),
        j: Math.floor(random() * side),
      };
      if (aboveHorizon(relativeBounds(patchBounds(planet, key), camera), h)) {
        continue;
      }
      culled += 1;
      const heights = levelHeightRangeM(planet, level);
      for (let s = 0; s < 10_000; s += 1) {
        const p = surfaceSample(planet, key, heights, random);
        if (seenPastOccluder(camera, p, h.occluderRadiusM)) {
          falseCulls.push(`${n}: ${face}/${level}/${key.i}/${key.j}`);
          break;
        }
      }
    }
    expect(falseCulls).toEqual([]);
    // The oracle is tried on enough culled patches to mean something.
    expect(culled).toBeGreaterThan(100);
  }, 120_000);

  it.each([
    ["2 m above the ground", 2],
    ["in a valley 500 m below the datum", -500],
  ])("never culls what a camera %s can see", (_, altitudeM) => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const random = seededRandom(0x76616c6c);
    const dir = normalise(vec3(0.3, -0.2, 0.9));
    const camera = v(surfacePoint(WGS84_FIGURE, [dir.x, dir.y, dir.z], altitudeM));
    const h = horizonCone(planet, camera);
    const { face, u, v: w } = xyzToFaceUv([dir.x, dir.y, dir.z]);
    let seen = 0;
    const falseCulls: string[] = [];
    for (let level = MAX_LEVEL - 10; level <= MAX_LEVEL - 5; level += 1) {
      const side = 2 ** level;
      const i0 = Math.floor(uvToSt(u) * side);
      const j0 = Math.floor(uvToSt(w) * side);
      for (let di = -3; di <= 3; di += 1) {
        for (let dj = -3; dj <= 3; dj += 1) {
          const key: PatchKey = { face, level, i: i0 + di, j: j0 + dj };
          const heights = levelHeightRangeM(planet, level);
          let visible = false;
          for (let n = 0; n < 200 && !visible; n += 1) {
            visible = seenPastOccluder(
              camera,
              surfaceSample(planet, key, heights, random),
              h.occluderRadiusM,
            );
          }
          if (visible) {
            seen += 1;
            if (!aboveHorizon(relativeBounds(patchBounds(planet, key), camera), h)) {
              falseCulls.push(patchKeyString(key));
            }
          }
        }
      }
    }
    expect(falseCulls).toEqual([]);
    // The camera can see the ground about it, so the check is not vacuous.
    expect(seen).toBeGreaterThan(100);
  });
});
