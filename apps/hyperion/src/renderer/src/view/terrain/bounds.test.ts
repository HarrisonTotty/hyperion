import { describe, expect, it } from "vitest";

import { dot, sub, vec3 } from "../../geometry/vec3";
import { seededRandom } from "../../test/seededRandom";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import { distanceToBoxM, patchBounds, relativeBounds, type PatchBounds } from "./bounds";
import { PATCH_QUADS, vertexDir } from "./cube";
import { FACES, MAX_LEVEL, type PatchKey, rootKey } from "./patchKey";
import { type BodyFigure, type PlanetGeometry, planetGeometry, surfacePoint } from "./planet";

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
