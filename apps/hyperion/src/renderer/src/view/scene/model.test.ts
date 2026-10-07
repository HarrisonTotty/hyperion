import { describe, expect, it } from "vitest";

import { add, cross, norm, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  aBody,
  aViewScene,
  FIXTURE_MOON,
  FIXTURE_PLANET,
  FIXTURE_SHIP,
} from "../../test/viewFixtures";
import { newCameraState } from "../camera/state";
import { rotation3FromRows } from "../coords/rotation";
import { hullOutline, TEST_HULL, TEST_PLATE_DISTANCE_M } from "./hull";
import { cameraSceneOf, sceneOrigins, staticRetarded } from "./model";

describe("hullOutline", () => {
  const base = {
    name: "BOX",
    vertices: [vec3(0, 0, 0), vec3(1, 0, 0), vec3(0, 1, 0)],
    edges: [[0, 1]] as const,
    faces: [[0, 1, 2]] as const,
    windows: [],
    eyePointM: vec3(0, 0, 0),
    lengthM: 1,
  };

  it("refuses an edge index out of range", () => {
    expect(() => hullOutline({ ...base, edges: [[0, 3]] })).toThrow(RangeError);
  });

  it("refuses a face index out of range", () => {
    expect(() => hullOutline({ ...base, faces: [[0, 1, -1]] })).toThrow(RangeError);
  });

  it("refuses an index that is not an integer", () => {
    expect(() => hullOutline({ ...base, edges: [[0, 0.5]] })).toThrow(RangeError);
  });

  it("refuses a vertex that is not finite", () => {
    expect(() =>
      hullOutline({ ...base, vertices: [vec3(Number.NaN, 0, 0), vec3(1, 0, 0), vec3(0, 1, 0)] }),
    ).toThrow(RangeError);
  });

  it.each([[1], [-1], [0.5]])(
    "refuses window %s, which is not one of its faces (R07.T16.e)",
    (face) => {
      expect(() => hullOutline({ ...base, windows: [face] })).toThrow(RangeError);
    },
  );

  it("refuses a window named twice (R07.T16.e)", () => {
    expect(() =>
      hullOutline({
        ...base,
        faces: [
          [0, 1, 2],
          [0, 2, 1],
        ],
        windows: [1, 1],
      }),
    ).toThrow(/as a window twice/);
  });

  it("accepts an outline whose indices are all its own", () => {
    expect(hullOutline(base)).toBe(base);
  });

  it("accepts a window that is one of its faces (R07.T16.e)", () => {
    const glazed = { ...base, windows: [0] };
    expect(hullOutline(glazed)).toBe(glazed);
  });
});

/** A side of a hull's face as its two vertex indices, in either order. */
function sideKey(i: number, j: number): string {
  return [Math.min(i, j), Math.max(i, j)].join("-");
}

/** One of `TEST_HULL`'s vertices, or a point that is not finite for an index it lacks. */
function hullVertex(index: number): Vec3 {
  return TEST_HULL.vertices[index] ?? vec3(Number.NaN, 0, 0);
}

/** The unit normal of one of `TEST_HULL`'s faces. */
function unitNormal([i, j, k]: readonly [number, number, number]): Vec3 {
  const a = hullVertex(i);
  const n = cross(sub(hullVertex(j), a), sub(hullVertex(k), a));
  return scale(n, 1 / norm(n));
}

describe("TEST_HULL", () => {
  it("is 20 m long", () => {
    const zs = TEST_HULL.vertices.map((v) => v.z);
    expect([Math.max(...zs) - Math.min(...zs), TEST_HULL.lengthM]).toEqual([20, 20]);
  });

  it("has its 1 m square plate 1 m forward of the seat's eye point, facing it", () => {
    const plate = TEST_HULL.vertices.slice(9, 13);
    const centre = scale(plate.reduce(add, vec3(0, 0, 0)), 1 / 4);
    const [a, b, c] = plate;
    if (a === undefined || b === undefined || c === undefined) {
      throw new Error("the test hull has no plate");
    }
    expect({
      centre: sub(centre, TEST_HULL.eyePointM),
      width: norm(sub(b, a)),
      height: norm(sub(c, b)),
    }).toEqual({ centre: vec3(0, 0, -TEST_PLATE_DISTANCE_M), width: 1, height: 1 });
  });

  it("has its plate, its last two faces, as its one window (R07.T16.e)", () => {
    const plate = new Set([9, 10, 11, 12]);
    expect({
      windows: TEST_HULL.windows,
      onThePlate: TEST_HULL.faces.flatMap((corners, index) =>
        corners.every((corner) => plate.has(corner)) ? [index] : [],
      ),
    }).toEqual({ windows: [14, 15], onThePlate: [14, 15] });
  });

  it("draws every side of its opaque faces as an edge, but the diagonal of a flat quad (R07.T16.e)", () => {
    // Over the image its silhouette's boundary then always lies under a cased outline, the mark
    // the silhouette is the ground of (decision-r07-t16a, item 3).
    const drawn = new Set(TEST_HULL.edges.map(([i, j]) => sideKey(i, j)));
    const opaque = TEST_HULL.faces.filter((_, index) => !TEST_HULL.windows.includes(index));
    const undrawn = opaque.flatMap((face) =>
      [0, 1, 2].flatMap((side) => {
        const i = face[side] ?? -1;
        const j = face[(side + 1) % 3] ?? -1;
        if (drawn.has(sideKey(i, j))) {
          return [];
        }
        // A side left undrawn must be shared with one coplanar face: a flat quad's diagonal.
        const sharing = opaque.filter(
          (other) => other !== face && other.includes(i) && other.includes(j),
        );
        const flat =
          sharing.length === 1 &&
          sharing.every((other) => norm(cross(unitNormal(face), unitNormal(other))) < 1e-12);
        return flat ? [] : [sideKey(i, j)];
      }),
    );
    expect(undrawn).toEqual([]);
  });
});

describe("the view fixtures", () => {
  it("build a valid scene with no argument", () => {
    const scene = aViewScene();
    const origins = sceneOrigins(scene);
    expect({
      planet: origins.bodyCentreM(FIXTURE_PLANET),
      ship: origins.craftPosition(FIXTURE_SHIP),
      camera: newCameraState(cameraSceneOf(scene), "camera").preset,
    }).toEqual({
      planet: aBody().centreM,
      ship: { kind: "body", body: FIXTURE_PLANET, m: vec3(2e7, 0, 0) },
      camera: "seat",
    });
  });
});

describe("staticRetarded", () => {
  it("puts a kept scene's body at rest where it is drawn, with no light time", () => {
    const centreM = vec3(1.5e11, 0, -2);
    expect(staticRetarded(centreM)).toEqual({
      centreM,
      velocityMPerS: vec3(0, 0, 0),
      lightTimeS: 0,
    });
    expect(staticRetarded(centreM).centreM).toBe(centreM);
  });

  it("is every fixture body's, at its own drawn centre", () => {
    const moved = aBody({ centreM: vec3(1, 2, 3) });
    expect(moved.retarded).toEqual(staticRetarded(vec3(1, 2, 3)));
    expect(aBody({ retarded: null }).retarded).toBeNull();
  });
});

describe("cameraSceneOf", () => {
  it("takes planets and moons with a Hill radius as frame bodies, and no star", () => {
    const scene = aViewScene({
      bodies: [
        ...aViewScene().bodies,
        aBody({ id: "0200080020000000.0000", kind: "star", hillRadiusM: null }),
      ],
    });
    expect(cameraSceneOf(scene).frameBodies.map((body) => body.id)).toEqual([
      FIXTURE_PLANET,
      FIXTURE_MOON,
    ]);
  });

  it("offers every body and every craft but the own ship as a target", () => {
    expect(cameraSceneOf(aViewScene()).targets).toEqual([
      { kind: "body", body: FIXTURE_PLANET },
      { kind: "body", body: FIXTURE_MOON },
    ]);
  });

  it("gives the own ship its hull's eye point and length", () => {
    const ship = cameraSceneOf(aViewScene()).ownShip;
    expect([ship?.eyePointM, ship?.lengthM]).toEqual([TEST_HULL.eyePointM, 20]);
  });

  it("has no own ship where the scene has none", () => {
    expect(cameraSceneOf(aViewScene({ ownShip: null })).ownShip).toBeNull();
  });

  it("refuses a scene whose own ship is not among its craft", () => {
    expect(() => cameraSceneOf(aViewScene({ craft: [] }))).toThrow(Error);
  });
});

describe("sceneOrigins", () => {
  it("gives the scene's barycentre for its own system", () => {
    const scene = aViewScene();
    expect(sceneOrigins(scene).systemBarycentre(scene.system)).toBe(scene.barycentre);
  });

  it("refuses another system", () => {
    expect(() => sceneOrigins(aViewScene()).systemBarycentre("0200080020000009")).toThrow(Error);
  });

  it("gives a body's rotation", () => {
    const rotation = rotation3FromRows([vec3(0, -1, 0), vec3(1, 0, 0), vec3(0, 0, 1)]);
    const scene = aViewScene({ bodies: [aBody({ rotation })] });
    expect(sceneOrigins(scene).bodyFixedRotation(FIXTURE_PLANET)).toBe(rotation);
  });

  it("refuses a body the scene does not have", () => {
    expect(() => sceneOrigins(aViewScene()).bodyCentreM("0200080020000000.0009")).toThrow(Error);
  });

  it("refuses a craft the scene does not have", () => {
    expect(() => sceneOrigins(aViewScene()).craftPosition("nobody")).toThrow(Error);
  });
});
