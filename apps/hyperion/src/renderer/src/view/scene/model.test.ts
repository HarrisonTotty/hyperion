import { describe, expect, it } from "vitest";

import { add, norm, scale, sub, vec3 } from "../../geometry/vec3";
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
import { cameraSceneOf, sceneOrigins } from "./model";

describe("hullOutline", () => {
  const base = {
    name: "BOX",
    vertices: [vec3(0, 0, 0), vec3(1, 0, 0), vec3(0, 1, 0)],
    edges: [[0, 1]] as const,
    faces: [[0, 1, 2]] as const,
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

  it("accepts an outline whose indices are all its own", () => {
    expect(hullOutline(base)).toBe(base);
  });
});

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
