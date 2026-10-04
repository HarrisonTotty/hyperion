import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { seededRandom } from "../../test/seededRandom";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { distanceToBoxM, patchBounds, relativeBounds, type PatchBounds } from "./bounds";
import { PATCH_QUADS, vertexDir, type Xyz } from "./cube";
import { FACES, MAX_LEVEL, type PatchKey, rootKey } from "./patchKey";
import {
  type BodyFigure,
  levelHeightRangeM,
  type PlanetGeometry,
  planetGeometry,
  spheroidNormal,
  surfacePoint,
} from "./planet";

const SPHERE: BodyFigure = { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000, pole: null };

/** 100 patches at levels 0 to 24 on every face, from a fixed seed. */
function samplePatches(): PatchKey[] {
  const random = seededRandom(0x626f756e);
  return Array.from({ length: 100 }, (_, n) => {
    const level = n % (MAX_LEVEL + 1);
    const side = 2 ** level;
    const face = FACES[n % 6] ?? 0;
    return {
      face,
      level,
      i: Math.floor(random() * side),
      j: Math.floor(random() * side),
    };
  });
}

/** Whether `p` lies inside the bounds' sphere and box, metres. */
function encloses(b: PatchBounds, p: { x: number; y: number; z: number }): boolean {
  const r = sub(p, b.centre);
  if (Math.sqrt(dot(r, r)) > b.radiusM) {
    return false;
  }
  const fromBox = sub(p, b.box.centre);
  return b.box.axes.every(
    (axis, k) => Math.abs(dot(fromBox, axis)) <= (b.box.halfExtentsM[k] ?? 0),
  );
}

function everyVertexEnclosed(planet: PlanetGeometry, key: PatchKey): boolean {
  const b = patchBounds(planet, key);
  for (let y = 0; y <= PATCH_QUADS; y += 1) {
    for (let x = 0; x <= PATCH_QUADS; x += 1) {
      const dir = vertexDir(key, x, y);
      for (const h of [b.minHeightM, b.maxHeightM, (b.minHeightM + b.maxHeightM) / 2]) {
        const [px, py, pz] = surfacePoint(planet.figure, dir, h);
        if (!encloses(b, vec3(px, py, pz))) {
          return false;
        }
      }
    }
  }
  return true;
}

describe("a patch's bounds", () => {
  it("has no height range and bounds the spheroid alone without a level table", () => {
    const planet = planetGeometry(WGS84_FIGURE, null);
    const b = patchBounds(planet, { face: 2, level: 10, i: 300, j: 700 });
    expect([b.minHeightM, b.maxHeightM]).toEqual([0, 0]);
    expect(everyVertexEnclosed(planet, { face: 2, level: 10, i: 300, j: 700 })).toBe(true);
  });

  it.each([
    ["a sphere", SPHERE],
    ["WGS 84's figure", WGS84_FIGURE],
  ])("encloses every vertex of 100 sampled patches on %s", (_, figure) => {
    const planet = planetGeometry(figure, goldenLevelTable("off"));
    const misses = samplePatches().filter((k) => !everyVertexEnclosed(planet, k));
    expect(misses).toEqual([]);
  });

  it("takes the level's height range from the table", () => {
    const table = goldenLevelTable("off");
    const b = patchBounds(planetGeometry(WGS84_FIGURE, table), { face: 0, level: 5, i: 3, j: 4 });
    expect([b.minHeightM, b.maxHeightM]).toEqual([table[21], table[22]]);
  });

  it("bounds a whole face", () => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    for (const face of FACES) {
      expect(everyVertexEnclosed(planet, rootKey(face))).toBe(true);
    }
  });

  it("puts a camera inside the box at no distance from it, and one above it at its height", () => {
    const planet = planetGeometry(SPHERE, null);
    const key: PatchKey = { face: 0, level: 12, i: 2048, j: 2048 };
    const b = patchBounds(planet, key);
    expect(distanceToBoxM(relativeBounds(b, b.centre))).toBe(0);
    const [cx, cy, cz] = surfacePoint(SPHERE, vertexDir(key, 32, 32), 10_000);
    const d = distanceToBoxM(relativeBounds(b, vec3(cx, cy, cz)));
    // The box is padded by the largest chord between its boundary samples, 150 m at level 12.
    expect(d).toBeGreaterThan(9_800);
    expect(d).toBeLessThanOrEqual(10_000);
  });
});

function toVec3(p: Xyz): Vec3 {
  return vec3(p[0], p[1], p[2]);
}

/** The boundary's vertices, every fourth, in order round the patch. */
const REFERENCE_RING: readonly (readonly [number, number])[] = [
  ...Array.from({ length: 16 }, (_, n) => [4 * n, 0] as const),
  ...Array.from({ length: 16 }, (_, n) => [PATCH_QUADS, 4 * n] as const),
  ...Array.from({ length: 16 }, (_, n) => [PATCH_QUADS - 4 * n, PATCH_QUADS] as const),
  ...Array.from({ length: 16 }, (_, n) => [0, PATCH_QUADS - 4 * n] as const),
];

/**
 * `patchBounds` in the vector form it had before R05.T7 perf (c) built it in scalars: the oracle the
 * scalar form must match bit for bit.
 */
function referenceBounds(
  planet: PlanetGeometry,
  key: PatchKey,
  heightRangeM: readonly [number, number],
): PatchBounds {
  const [minHeightM, maxHeightM] = heightRangeM;
  const half = PATCH_QUADS / 2;
  const centreDir = vertexDir(key, half, half);
  const origin = toVec3(surfacePoint(planet.figure, centreDir, (minHeightM + maxHeightM) / 2));
  const n = toVec3(spheroidNormal(planet.figure, centreDir));
  const across = sub(toVec3(vertexDir(key, PATCH_QUADS, half)), toVec3(vertexDir(key, 0, half)));
  const t1 = normalise(sub(across, scale(n, dot(across, n))));
  const t2 = cross(n, t1);
  const axes: readonly [Vec3, Vec3, Vec3] = [n, t1, t2];
  const points: Vec3[] = [];
  let ringTop: Vec3 | null = null;
  let marginM = 0;
  for (const [x, y] of [...REFERENCE_RING, REFERENCE_RING[0] ?? [0, 0]]) {
    const dir = vertexDir(key, x, y);
    const top = toVec3(surfacePoint(planet.figure, dir, maxHeightM));
    if (ringTop !== null) {
      marginM = Math.max(marginM, norm(sub(top, ringTop)));
    }
    ringTop = top;
    points.push(top, toVec3(surfacePoint(planet.figure, dir, minHeightM)));
  }
  points.push(
    toVec3(surfacePoint(planet.figure, centreDir, maxHeightM)),
    toVec3(surfacePoint(planet.figure, centreDir, minHeightM)),
  );
  const lows = [Infinity, Infinity, Infinity];
  const highs = [-Infinity, -Infinity, -Infinity];
  for (const p of points) {
    const r = sub(p, origin);
    axes.forEach((axis, k) => {
      const s = dot(r, axis);
      lows[k] = Math.min(lows[k] ?? Infinity, s);
      highs[k] = Math.max(highs[k] ?? -Infinity, s);
    });
  }
  let boxCentre = origin;
  const extents = axes.map((axis, k) => {
    const lo = lows[k] ?? 0;
    const hi = highs[k] ?? 0;
    boxCentre = add(boxCentre, scale(axis, (lo + hi) / 2));
    return (hi - lo) / 2 + marginM;
  });
  let radiusM = 0;
  for (const p of points) {
    radiusM = Math.max(radiusM, norm(sub(p, boxCentre)));
  }
  return {
    centre: boxCentre,
    radiusM: radiusM + marginM,
    minHeightM,
    maxHeightM,
    box: {
      centre: boxCentre,
      axes,
      halfExtentsM: [extents[0] ?? 0, extents[1] ?? 0, extents[2] ?? 0],
    },
  };
}

/** Every number of a patch's bounds, in a fixed order. */
function boundsNumbers(b: PatchBounds): number[] {
  return [
    b.centre.x,
    b.centre.y,
    b.centre.z,
    b.radiusM,
    b.minHeightM,
    b.maxHeightM,
    b.box.centre.x,
    b.box.centre.y,
    b.box.centre.z,
    ...b.box.axes.flatMap((a) => [a.x, a.y, a.z]),
    ...b.box.halfExtentsM,
  ];
}

describe("a patch's bounds, built in scalars", () => {
  it("equal the vector form's bit for bit, at face edges, cube corners and inside faces", () => {
    const random = seededRandom(0x62697473);
    const planets = [
      planetGeometry(WGS84_FIGURE, goldenLevelTable("off")),
      planetGeometry(WGS84_FIGURE, goldenLevelTable("on")),
      planetGeometry(SPHERE, null),
    ];
    const keys: PatchKey[] = [];
    for (const face of FACES) {
      for (let level = 0; level <= MAX_LEVEL; level += 1) {
        const last = 2 ** level - 1;
        // The four corners, an edge cell on each side and a random cell.
        for (const [i, j] of [
          [0, 0],
          [last, 0],
          [0, last],
          [last, last],
          [Math.floor(random() * (last + 1)), 0],
          [last, Math.floor(random() * (last + 1))],
          [Math.floor(random() * (last + 1)), last],
          [0, Math.floor(random() * (last + 1))],
          [Math.floor(random() * (last + 1)), Math.floor(random() * (last + 1))],
        ] as const) {
          keys.push({ face, level, i, j });
        }
      }
    }
    const mismatches: string[] = [];
    for (const planet of planets) {
      for (const key of keys) {
        const [low, high] = levelHeightRangeM(planet, key.level);
        // The level's range, a baked one inside it, and one of a single height.
        for (const range of [
          [low, high],
          [-12.25 - random() * 100, 40.5 + random() * 100],
          [-0, -0],
        ] as const) {
          const got = boundsNumbers(patchBounds(planet, key, range));
          const want = boundsNumbers(referenceBounds(planet, key, range));
          if (!got.every((x, n) => Object.is(x, want[n]))) {
            mismatches.push(`${key.face}/${key.level}/${key.i}/${key.j} [${range.join(", ")}]`);
          }
        }
      }
    }
    expect(keys.length).toBe(6 * 25 * 9);
    expect(mismatches).toEqual([]);
  });

  it("takes the level's range by default", () => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const key: PatchKey = { face: 4, level: 13, i: 8191, j: 77 };
    expect(boundsNumbers(patchBounds(planet, key))).toEqual(
      boundsNumbers(referenceBounds(planet, key, levelHeightRangeM(planet, key.level))),
    );
  });
});
