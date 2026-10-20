import { describe, expect, it } from "vitest";

import { add, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { NEAR_PLANE_M, type ProjectionCamera, project, type Viewport } from "../camera/projection";
import { IDENTITY_QUATERNION, quaternionFromAxisAngle, rotate } from "../camera/quaternion";
import { type HullOutline, hullOutline, TEST_HULL } from "../scene/hull";
import { hullBoundingRadiusM, hullEdges, hullFaces, hullOnView } from "./hulls";
import type { ScreenPx, ScreenSegment } from "./symbology";

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

describe("hullBoundingRadiusM", () => {
  it("is the farthest vertex's distance from the reference point: TEST_HULL's stern corners, 11.06 m", () => {
    expect(hullBoundingRadiusM(TEST_HULL)).toBeCloseTo(Math.hypot(4, 2.5, 10), 12);
  });
});

const VIEWPORT: Viewport = { widthPx: 1920, heightPx: 1080 };

/** A camera looking down its frame's −z with +y up, 60° across. */
const STRAIGHT: ProjectionCamera = { orientation: IDENTITY_QUATERNION, fovXRad: Math.PI / 3 };

/** An outline of bare edges between `vertices`, with no faces. */
function wires(
  vertices: ReadonlyArray<Vec3>,
  edges: ReadonlyArray<readonly [number, number]>,
): HullOutline {
  return hullOutline({
    name: "WIRES",
    vertices,
    edges,
    faces: [],
    windows: [],
    eyePointM: vec3(0, 0, 0),
    lengthM: 10,
  });
}

/** An opaque square plate `halfM` each way about its reference point, square to z. */
function opaqueSquare(halfM: number): HullOutline {
  return hullOutline({
    name: "SQUARE",
    vertices: [
      vec3(-halfM, -halfM, 0),
      vec3(halfM, -halfM, 0),
      vec3(halfM, halfM, 0),
      vec3(-halfM, halfM, 0),
    ],
    edges: [
      [0, 1],
      [1, 2],
      [2, 3],
      [3, 0],
    ],
    faces: [
      [0, 1, 2],
      [0, 2, 3],
    ],
    windows: [],
    eyePointM: vec3(0, 0, 0),
    lengthM: 2 * halfM,
  });
}

/** Where {@link STRAIGHT} puts a point `v` from the camera on the view, px. */
function onView(v: Vec3): ScreenPx {
  const p = project(v, STRAIGHT, VIEWPORT);
  return { xPx: p.xPx, yPx: p.yPx };
}

/** How far apart two points on the view stand, px, to a millionth of a pixel. */
function apartPx(a: ScreenPx | undefined, b: ScreenPx): number {
  return a === undefined
    ? Number.POSITIVE_INFINITY
    : Math.round(Math.hypot(a.xPx - b.xPx, a.yPx - b.yPx) * 1e6) / 1e6;
}

/** How far each end of `edge` stands from the ends it should have, px. */
function endsOffPx(edge: ScreenSegment | undefined, ends: readonly [ScreenPx, ScreenPx]): number[] {
  return [apartPx(edge?.[0], ends[0]), apartPx(edge?.[1], ends[1])];
}

describe("hullOnView, a hull as the tripod gives way to it (R10.T12; decision-r10-t12-commit3 §1)", () => {
  it("clips an edge with one end behind the near plane where it meets the plane, whichever end that is", () => {
    // Two canopy bows from 5 m ahead of the eye to 5 m behind it, 1 m below and to either side,
    // one drawn from its front end and one from its back.
    const hull = wires(
      [vec3(0.5, -1, -5), vec3(0.5, -1, 5), vec3(-0.5, -1, 5), vec3(-0.5, -1, -5)],
      [
        [0, 1],
        [2, 3],
      ],
    );
    const { edges } = hullOnView(hull, IDENTITY_QUATERNION, vec3(0, 0, 0), STRAIGHT, VIEWPORT);
    expect({
      count: edges.length,
      frontFirst: endsOffPx(edges[0], [
        onView(vec3(0.5, -1, -5)),
        onView(vec3(0.5, -1, -NEAR_PLANE_M)),
      ]),
      backFirst: endsOffPx(edges[1], [
        onView(vec3(-0.5, -1, -NEAR_PLANE_M)),
        onView(vec3(-0.5, -1, -5)),
      ]),
    }).toEqual({ count: 2, frontFirst: [0, 0], backFirst: [0, 0] });
  });

  it("drops an edge wholly behind the near plane, and keeps one wholly in front as projected", () => {
    const hull = wires(
      [vec3(0, -1, -5), vec3(1, -1, -5), vec3(0, -1, -NEAR_PLANE_M / 2), vec3(0, -1, 5)],
      [
        [0, 1],
        [2, 3],
      ],
    );
    const { edges } = hullOnView(hull, IDENTITY_QUATERNION, vec3(0, 0, 0), STRAIGHT, VIEWPORT);
    expect({
      count: edges.length,
      kept: endsOffPx(edges[0], [onView(vec3(0, -1, -5)), onView(vec3(1, -1, -5))]),
    }).toEqual({ count: 1, kept: [0, 0] });
  });

  it("projects the edges about the craft's place, turned by its attitude", () => {
    const attitude = quaternionFromAxisAngle(vec3(0, 1, 0), 0.7);
    const originM = vec3(3, -2, -40);
    const { edges } = hullOnView(TEST_HULL, attitude, originM, STRAIGHT, VIEWPORT);
    const expected = hullEdges(TEST_HULL, attitude).map(([a, b]): readonly [ScreenPx, ScreenPx] => [
      onView(add(originM, a)),
      onView(add(originM, b)),
    ]);
    expect({
      count: edges.length,
      off: expected.map((ends, i) => endsOffPx(edges[i], ends)),
    }).toEqual({ count: expected.length, off: expected.map(() => [0, 0]) });
  });

  it("covers the point where the line of sight meets an opaque face: TEST_HULL's stern, 40 m ahead", () => {
    expect(
      hullOnView(TEST_HULL, IDENTITY_QUATERNION, vec3(0, 0, -50), STRAIGHT, VIEWPORT).coversPoint,
    ).toBe(true);
  });

  it("does not cover the point where the line of sight passes through a window", () => {
    // TEST_HULL's seat looking forward: the line of sight passes through its plate, a window, and
    // over its nose; the same outline with the plate opaque covers it.
    const originM = scale(TEST_HULL.eyePointM, -1);
    const covers = (hull: HullOutline): boolean =>
      hullOnView(hull, IDENTITY_QUATERNION, originM, STRAIGHT, VIEWPORT).coversPoint;
    expect([covers(TEST_HULL), covers(hullOutline({ ...TEST_HULL, windows: [] }))]).toEqual([
      false,
      true,
    ]);
  });

  it.each([
    ["1 m ahead", vec3(0, 0, -1), true],
    ["behind the camera", vec3(0, 0, 50), false],
    ["nearer than the near plane", vec3(0, 0, -NEAR_PLANE_M / 2), false],
  ])("tells whether an opaque face %s covers the point", (_, originM, covers) => {
    expect(
      hullOnView(opaqueSquare(1), IDENTITY_QUATERNION, originM, STRAIGHT, VIEWPORT).coversPoint,
    ).toBe(covers);
  });
});
