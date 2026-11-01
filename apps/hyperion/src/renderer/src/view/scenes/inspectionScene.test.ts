import { galacticPositionFromLy, type UniverseTime } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { dot, norm, normalise, scale, sub } from "../../geometry/vec3";
import { timeAfter } from "../../lib/scene/lightTime";
import { toSystemBodiesModel } from "../../lib/system/bodiesWire";
import { startRun } from "../../displays/view/viewRun";
import {
  FIXTURE_EARTH,
  FIXTURE_JUPITER,
  FIXTURE_SYSTEM,
  FIXTURE_UNIVERSE,
  populatedBodies,
  sliceBodies,
} from "../../test/planetaryFixture";
import { sceneModelOf, stateNearEarth } from "../../test/sceneFixture";
import { DEFAULT_FOV_DEG } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import { cameraSceneOf, type ViewBody, type ViewScene } from "../scene/model";
import {
  type BodyInspection,
  hillRadiusOf,
  INSPECTION_ASPECT,
  INSPECTION_DISC_SHARE,
  INSPECTION_SCENE_NAME,
  inspectionScene,
} from "./inspectionScene";

/** The fixture's system's designation of record. */
const DESIGNATION = "H7K 4C0RFZ D-7";

/** The display's time the inspections open at: an hour after the epoch. */
const OPENED: UniverseTime = { seconds: 3_600, nanos: 0 };

/** The populated answer's ringed planet, its moon and its ring. */
const RINGED_PLANET = `${FIXTURE_SYSTEM}.0100`;
const ITS_MOON = `${FIXTURE_SYSTEM}.0101`;

/** An inspection of `body` in the fixture's answer `answer`, as `SYSTEM` makes one. */
function inspectionOf(
  answer: ReturnType<typeof sliceBodies>,
  body: BodyInspection["body"],
): BodyInspection {
  const result = toSystemBodiesModel(answer, DESIGNATION);
  if (result.kind !== "ok") {
    throw new Error(`the fixture's answer is unusable: ${result.fault}`);
  }
  return {
    universe: FIXTURE_UNIVERSE,
    place: {
      kind: "charted",
      system: FIXTURE_SYSTEM,
      designation: DESIGNATION,
      barycentre: galacticPositionFromLy([26_000, 0, 12]),
    },
    model: result.model,
    bodies: result.bodies,
    body,
    time: OPENED,
  };
}

/** The scene's body `id`, which it must draw. */
function bodyOf(scene: ViewScene, id: string): ViewBody {
  const body = scene.bodies.find((each) => each.id === id);
  if (body === undefined) {
    throw new Error(`the scene draws no ${id}`);
  }
  return body;
}

describe("the body inspection's scene (the owner's request)", () => {
  it("is a kept scene of served bodies under its own name, with no own ship, photorealistic", () => {
    const kept = inspectionScene(inspectionOf(sliceBodies(), FIXTURE_EARTH));
    const scene = kept.sceneAt(0);

    expect(kept.name).toBe(INSPECTION_SCENE_NAME);
    expect(scene.provenance).toEqual({ kind: "kept", name: INSPECTION_SCENE_NAME });
    expect(scene.served?.universe).toBe(FIXTURE_UNIVERSE);
    expect(scene.served?.place.designation).toBe(DESIGNATION);
    expect(scene.served?.grants.get(FIXTURE_EARTH)).toBe("full");
    expect(scene.ownShip).toBeNull();
    expect(scene.craft).toEqual([]);
    const run = startRun(kept);
    expect(run.camera.style).toBe("photorealistic");
    expect(run.camera.preset).toBe("free");
  });

  it("draws the system's records as the server's scene would: its star and its planets", () => {
    const scene = inspectionScene(inspectionOf(sliceBodies(), FIXTURE_EARTH)).sceneAt(0);

    expect(scene.system).toBe(FIXTURE_SYSTEM);
    expect(scene.bodies.map((body) => [body.id, body.kind])).toEqual([
      [`${FIXTURE_SYSTEM}.0000`, "star"],
      [FIXTURE_EARTH, "planet"],
      [FIXTURE_JUPITER, "planet"],
    ]);
    const earth = bodyOf(scene, FIXTURE_EARTH);
    expect(earth.designation).toBe(`${DESIGNATION} /768`);
    expect(earth.appearance?.figure).not.toBeNull();
    expect(earth.rotation).not.toBeNull();
    // Seen from its own centre, the body is the local one, drawn where it is.
    expect(earth.retarded?.lightTimeS).toBe(0);
  });

  it("draws a ringed planet's rings and moon", () => {
    const scene = inspectionScene(inspectionOf(populatedBodies(), RINGED_PLANET)).sceneAt(0);

    expect(scene.rings.map((ring) => ring.body)).toEqual([RINGED_PLANET]);
    expect(bodyOf(scene, ITS_MOON).parent).toBe(RINGED_PLANET);
  });

  it("frames the body so that its disc fills two thirds of a 16:9 view's height", () => {
    const scene = inspectionScene(inspectionOf(sliceBodies(), FIXTURE_EARTH)).sceneAt(0);
    const earth = bodyOf(scene, FIXTURE_EARTH);
    const pose = scene.defaultPose;

    expect(pose.frame).toEqual({ kind: "body", body: FIXTURE_EARTH });
    const fovYRad = 2 * Math.atan(Math.tan((DEFAULT_FOV_DEG * Math.PI) / 360) / INSPECTION_ASPECT);
    const discRad = 2 * Math.asin(earth.radiusM / norm(pose.positionM));
    expect(discRad / fovYRad).toBeCloseTo(INSPECTION_DISC_SHARE, 12);
  });

  it("looks at the body's centre from its day side, its pole up", () => {
    const scene = inspectionScene(inspectionOf(sliceBodies(), FIXTURE_EARTH)).sceneAt(0);
    const earth = bodyOf(scene, FIXTURE_EARTH);
    const star = bodyOf(scene, `${FIXTURE_SYSTEM}.0000`);
    const pose = scene.defaultPose;
    // The camera looks down its own −z with +y up.
    const forward = rotate(pose.orientation, { x: 0, y: 0, z: -1 });
    const up = rotate(pose.orientation, { x: 0, y: 1, z: 0 });

    expect(dot(forward, normalise(scale(pose.positionM, -1)))).toBeCloseTo(1, 12);
    const towardStar = normalise(sub(star.centreM, earth.centreM));
    const phaseDeg = (Math.acos(dot(normalise(pose.positionM), towardStar)) * 180) / Math.PI;
    expect(phaseDeg).toBeGreaterThan(20);
    expect(phaseDeg).toBeLessThan(90);
    const pole = earth.appearance?.figure?.pole;
    if (pole === undefined || pole === null) {
      throw new Error("the slice's Earth has its pole");
    }
    expect(dot(up, pole)).toBeGreaterThan(0.9);
  });

  it("holds the camera in the body's frame, a moon's too", () => {
    const planet = inspectionScene(inspectionOf(sliceBodies(), FIXTURE_EARTH)).sceneAt(0);
    const moon = inspectionScene(inspectionOf(populatedBodies(), ITS_MOON)).sceneAt(0);

    expect(cameraSceneOf(planet).frameBodies.map((body) => body.id)).toContain(FIXTURE_EARTH);
    expect(moon.defaultPose.frame).toEqual({ kind: "body", body: ITS_MOON });
    expect(cameraSceneOf(moon).frameBodies.map((body) => body.id)).toContain(ITS_MOON);
  });

  it("runs its clock at one second a second from the display's time", () => {
    const kept = inspectionScene(inspectionOf(sliceBodies(), FIXTURE_EARTH));

    expect(kept.sceneAt(0).time).toEqual(OPENED);
    expect(kept.sceneAt(90.5).time).toEqual(timeAfter(OPENED, 90.5));
    expect(kept.sceneAt(90.5).timeRate).toBe(1);
  });
});

describe("a body's Hill radius from its record", () => {
  it("is the scene's from the wire's kilograms, to rounding", () => {
    const system = sceneModelOf(stateNearEarth()).system;
    if (system === null) {
      throw new Error("the slice's scene holds its system");
    }
    const agreements = system.bodies.bodies.map((body) => {
      const wire = system.hillRadiiM.get(body.id) ?? null;
      const own = hillRadiusOf(body);
      return [
        body.id,
        own === null || wire === null ? own === wire : Math.abs(own - wire) / wire < 1e-9,
      ];
    });

    expect(system.hillRadiiM.size).toBeGreaterThan(0);
    expect(agreements).toEqual(agreements.map(([id]) => [id, true]));
  });
});
