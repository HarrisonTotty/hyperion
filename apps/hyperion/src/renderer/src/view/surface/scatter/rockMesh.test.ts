import { describe, expect, it } from "vitest";

import { dot, vec3 } from "../../../geometry/vec3";
import { randomRocks } from "../../../test/rockFixtures";
import type { Rock } from "./rockLayer";
import {
  chordalBound,
  MAX_ROCK_LEVEL,
  MIN_ROCK_LEVEL,
  radialDepthM,
  rockLevel,
  rockToleranceM,
  tessellateRock,
} from "./rockMesh";

/** A vertex of `mesh` turned back into the rock's frame, from its body-fixed offset. */
function inFrame(rock: Rock, x: number, y: number, z: number): [number, number, number] {
  const p = vec3(x, y, z);
  const [a, b, c] = rock.axes;
  return [dot(p, a), dot(p, b), dot(p, c)];
}

/** The `f32` spacing at `x`: 2⁻²³ of its binade's least value. */
function f32Step(x: number): number {
  return 2 ** (Math.floor(Math.log2(Math.max(Math.abs(x), 2 ** -126))) - 23);
}

/**
 * The greatest depth below the rock's surface, m, over a barycentric grid of `steps` subdivisions
 * of each triangle of its tessellation at `level`, its corners' frame points recomputed from the
 * body-fixed mesh.
 */
function sampledDepthM(rock: Rock, level: number, steps: number): number {
  const mesh = tessellateRock(rock, level);
  const vertices = mesh.positionsM.length / 3;
  const frame = new Float64Array(3 * vertices);
  for (let v = 0; v < vertices; v += 1) {
    frame.set(
      inFrame(
        rock,
        mesh.positionsM[3 * v] ?? 0,
        mesh.positionsM[3 * v + 1] ?? 0,
        mesh.positionsM[3 * v + 2] ?? 0,
      ),
      3 * v,
    );
  }
  let most = 0;
  for (let t = 0; t < mesh.triangles.length; t += 3) {
    const [p = 0, q = 0, r = 0] = [0, 1, 2].map((k) => 3 * (mesh.triangles[t + k] ?? 0));
    for (let i = 0; i <= steps; i += 1) {
      for (let j = 0; i + j <= steps; j += 1) {
        const [u, v] = [i / steps, j / steps];
        const w = 1 - u - v;
        const at = (k: number): number =>
          u * (frame[p + k] ?? 0) + v * (frame[q + k] ?? 0) + w * (frame[r + k] ?? 0);
        most = Math.max(most, radialDepthM(rock, at(0), at(1), at(2)));
      }
    }
  }
  return most;
}

describe("a rock's tessellation (Design note 6)", () => {
  const rocks = randomRocks(0x726f636b, 1000);

  it(
    "lies inside its shape within the chordal bound, and the level's bound within the tolerance, for 10³ random rocks",
    { timeout: 120_000 },
    () => {
      let worst = 0;
      for (const rock of rocks) {
        const level = rockLevel(rock);
        const bound = chordalBound(rock, level);
        // Within the tolerance below the finest level, and the level below it not, above the least.
        const within = level === MAX_ROCK_LEVEL || bound <= rockToleranceM(rock);
        const least =
          level === MIN_ROCK_LEVEL || chordalBound(rock, level - 1) > rockToleranceM(rock);
        expect([within, least]).toEqual([true, true]);
        const depth = sampledDepthM(rock, level, 3);
        expect(depth).toBeLessThanOrEqual(bound);
        worst = Math.max(worst, depth / bound);
      }
      expect(worst).toBeGreaterThan(0.3);
    },
  );

  it(
    "is held by its chordal bound under a denser sampling at several levels",
    { timeout: 120_000 },
    () => {
      for (const rock of rocks.slice(0, 24)) {
        for (const level of [MIN_ROCK_LEVEL, 5, rockLevel(rock)]) {
          expect(sampledDepthM(rock, level, 12)).toBeLessThanOrEqual(chordalBound(rock, level));
        }
      }
    },
  );

  it("puts every vertex on the analytic surface to the f32 step", { timeout: 60_000 }, () => {
    for (const rock of rocks.slice(0, 100)) {
      const mesh = tessellateRock(rock, rockLevel(rock));
      for (let v = 0; v < mesh.positionsM.length / 3; v += 1) {
        const body = [0, 1, 2].map((k) => Math.fround(mesh.positionsM[3 * v + k] ?? 0));
        const [x = 0, y = 0, z = 0] = body;
        const step = f32Step(Math.max(...body.map(Math.abs)));
        const [fx, fy, fz] = inFrame(rock, x, y, z);
        expect(Math.abs(radialDepthM(rock, fx, fy, fz))).toBeLessThanOrEqual(2 * step);
      }
    }
  });

  it(
    "is closed, its triangles counter-clockwise from outside and its normals outward and unit",
    { timeout: 60_000 },
    () => {
      for (const rock of rocks.slice(0, 12)) {
        const mesh = tessellateRock(rock, Math.min(rockLevel(rock), 16));
        const key = (v: number): string =>
          [0, 1, 2].map((k) => String(mesh.positionsM[3 * v + k])).join(",");
        const edges = new Map<string, number>();
        for (let t = 0; t < mesh.triangles.length; t += 3) {
          const corners = [0, 1, 2].map((k) => mesh.triangles[t + k] ?? 0);
          const at = corners.map((v) =>
            vec3(
              mesh.positionsM[3 * v] ?? 0,
              mesh.positionsM[3 * v + 1] ?? 0,
              mesh.positionsM[3 * v + 2] ?? 0,
            ),
          );
          const [p, q, r] = at;
          if (p === undefined || q === undefined || r === undefined) {
            throw new Error("a triangle has three corners");
          }
          const e1 = vec3(q.x - p.x, q.y - p.y, q.z - p.z);
          const e2 = vec3(r.x - p.x, r.y - p.y, r.z - p.z);
          const n = vec3(
            e1.y * e2.z - e1.z * e2.y,
            e1.z * e2.x - e1.x * e2.z,
            e1.x * e2.y - e1.y * e2.x,
          );
          const centroid = vec3(
            (p.x + q.x + r.x) / 3,
            (p.y + q.y + r.y) / 3,
            (p.z + q.z + r.z) / 3,
          );
          expect(dot(n, centroid)).toBeGreaterThan(0);
          for (let k = 0; k < 3; k += 1) {
            const [from, to] = [key(corners[k] ?? 0), key(corners[(k + 1) % 3] ?? 0)];
            const directed = `${from}>${to}`;
            edges.set(directed, (edges.get(directed) ?? 0) + 1);
          }
        }
        for (const [directed, count] of edges) {
          const [from, to] = directed.split(">");
          expect(count).toBe(1);
          expect(edges.get(`${to ?? ""}>${from ?? ""}`)).toBe(1);
        }
        for (let v = 0; v < mesh.normals.length / 3; v += 1) {
          const n = vec3(
            mesh.normals[3 * v] ?? 0,
            mesh.normals[3 * v + 1] ?? 0,
            mesh.normals[3 * v + 2] ?? 0,
          );
          const p = vec3(
            mesh.positionsM[3 * v] ?? 0,
            mesh.positionsM[3 * v + 1] ?? 0,
            mesh.positionsM[3 * v + 2] ?? 0,
          );
          expect(Math.abs(dot(n, n) - 1)).toBeLessThan(1e-12);
          expect(dot(n, p)).toBeGreaterThan(0);
        }
      }
    },
  );

  it("refuses a level that is not a whole number of quads", () => {
    const [rock] = rocks;
    if (rock === undefined) {
      throw new Error("the fixture holds rocks");
    }
    expect(() => tessellateRock(rock, 0)).toThrow(RangeError);
    expect(() => chordalBound(rock, 2.5)).toThrow(RangeError);
  });
});
