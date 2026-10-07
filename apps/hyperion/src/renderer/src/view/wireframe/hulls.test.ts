import { describe, expect, it } from "vitest";

import { norm, sub, vec3 } from "../../geometry/vec3";
import { IDENTITY_QUATERNION, quaternionFromAxisAngle, rotate } from "../camera/quaternion";
import { TEST_HULL } from "../scene/hull";
import { hullEdges, hullFaces } from "./hulls";

describe("hullEdges", () => {
  it("returns every edge of TEST_HULL once", () => {
    const edges = hullEdges(TEST_HULL, IDENTITY_QUATERNION);
    const keys = edges.map(([a, b]) => {
      const i = TEST_HULL.vertices.findIndex((v) => norm(sub(v, a)) === 0);
      const j = TEST_HULL.vertices.findIndex((v) => norm(sub(v, b)) === 0);
      return [Math.min(i, j), Math.max(i, j)].join("-");
    });
    const expected = TEST_HULL.edges.map(([i, j]) => [Math.min(i, j), Math.max(i, j)].join("-"));
    expect({ count: edges.length, unique: new Set(keys).size, keys: keys.toSorted() }).toEqual({
      count: TEST_HULL.edges.length,
      unique: TEST_HULL.edges.length,
      keys: expected.toSorted(),
    });
  });

  it("turns the edges by the craft's attitude", () => {
    const attitude = quaternionFromAxisAngle(vec3(0, 1, 0), 0.7);
    const [first] = hullEdges(TEST_HULL, attitude);
    const nose = TEST_HULL.vertices[0] ?? vec3(0, 0, 0);
    expect(norm(sub(first?.[0] ?? vec3(0, 0, 0), rotate(attitude, nose)))).toBe(0);
  });
});

describe("hullFaces", () => {
  it("returns TEST_HULL's 14 opaque faces as triangles, and none of its window's (R07.T16.e)", () => {
    const faces = hullFaces(TEST_HULL, IDENTITY_QUATERNION);
    const plate = TEST_HULL.vertices.slice(9, 13);
    const onPlate = faces.filter((corners) =>
      corners.every((corner) => plate.some((p) => norm(sub(p, corner)) === 0)),
    );
    expect([faces.length, onPlate.length]).toEqual([14, 0]);
  });
});
