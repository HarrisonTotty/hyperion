import { describe, expect, it } from "vitest";

import { add, norm, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { FIXTURE_EARTH, FIXTURE_JUPITER, FIXTURE_SYSTEM } from "../../test/planetaryFixture";
import {
  sceneModelOf,
  shipFrameOf,
  stateAfterHeartbeat,
  stateNearEarth,
  viewBodyOf,
  viewSceneOf,
} from "../../test/sceneFixture";
import type { CameraPose } from "../camera/pose";
import { lookAlong, rotate } from "../camera/quaternion";
import { rebase } from "../camera/rebase";
import { sceneFrameFor } from "../camera/state";
import { rotateToBody, rotateToBodyFixed } from "../coords/rotation";
import { cameraSceneOf, sceneOrigins, type ViewBody } from "../scene/model";
import { atmosphereAxesOf, bodyAtmosphereCamera } from "./bodyCamera";

/**
 * A body's atmosphere camera where the view draws the body (plan R08, R08.T8.b; Design note 13):
 * about its drawn centre, the ship's local body's geometric and every other body's apparent, in its
 * body-fixed axes, and none for a contact. The slice's scene with the ship 10,000 km from its
 * Earth, whose local body the Earth is.
 */

const PROJECTION = { fovXRad: 1, viewport: { widthPx: 64, heightPx: 32 } } as const;

const model = sceneModelOf(stateNearEarth());
const frame = shipFrameOf(model);
const scene = viewSceneOf(model, frame);
const origins = sceneOrigins(scene);

/** Where the frame places a body: its geometric and apparent centres, m from the barycentre. */
function placed(id: string): { readonly geometricM: Vec3; readonly apparentM: Vec3 } {
  const seen = frame.bodies.find((each) => each.id === id);
  if (seen?.kind !== "placed") {
    throw new Error(`the fixture's frame places ${id}`);
  }
  return seen;
}

/** A free camera in the system frame `positionM` from the barycentre, looking along `forward`. */
function freeCamera(positionM: Vec3, forward: Vec3): CameraPose {
  return {
    frame: { kind: "system", system: FIXTURE_SYSTEM },
    positionM,
    orientation: lookAlong(forward, vec3(0, 0, 1)),
  };
}

/** The camera from `centreM`, m from the barycentre, in `body`'s atmosphere axes. */
function bodyFixedFrom(body: ViewBody, pose: CameraPose, centreM: Vec3): Vec3 {
  return rotateToBodyFixed(atmosphereAxesOf(body), sub(pose.positionM, centreM));
}

describe("a body's atmosphere camera (R08.T8.b)", () => {
  const jupiter = viewBodyOf(scene, FIXTURE_JUPITER);
  const earth = viewBodyOf(scene, FIXTURE_EARTH);
  // 2 × 10⁸ m from Jupiter as drawn: inside its Hill sphere, so the free camera's own local body.
  const pose = freeCamera(add(jupiter.centreM, vec3(2e8, 0, 0)), vec3(-1, 0.1, 0));

  it("sets the free camera inside another body's Hill sphere, whose frame it takes", () => {
    expect(sceneFrameFor(pose, cameraSceneOf(scene))).toEqual({
      kind: "body",
      body: FIXTURE_JUPITER,
    });
  });

  it("forms that body's camera about where it is seen, not where it is", () => {
    const camera = bodyAtmosphereCamera(scene, jupiter, pose, origins, PROJECTION);
    const { geometricM, apparentM } = placed(FIXTURE_JUPITER);
    const position = camera?.positionM ?? vec3(Number.NaN, Number.NaN, Number.NaN);
    expect([
      norm(sub(position, bodyFixedFrom(jupiter, pose, apparentM))) < 1e-3,
      norm(sub(position, bodyFixedFrom(jupiter, pose, geometricM))) > 1e3,
    ]).toEqual([true, true]);
  });

  it("forms the camera about where it is for the ship's local body alone", () => {
    const camera = bodyAtmosphereCamera(scene, earth, pose, origins, PROJECTION);
    const { geometricM, apparentM } = placed(FIXTURE_EARTH);
    const position = camera?.positionM ?? vec3(Number.NaN, Number.NaN, Number.NaN);
    const fromGeometric = norm(sub(position, bodyFixedFrom(earth, pose, geometricM)));
    // The Earth seen from the ship 10,000 km off stands a light time's motion from where it is.
    const seenApart = norm(sub(apparentM, geometricM));
    expect([fromGeometric < 1e-3, seenApart > 1]).toEqual([true, true]);
  });

  it("forms the same camera from the pose held in that body's own frame", () => {
    const inBody = rebase(pose, sceneFrameFor(pose, cameraSceneOf(scene)), origins).pose;
    const camera = bodyAtmosphereCamera(scene, jupiter, pose, origins, PROJECTION);
    const held = bodyAtmosphereCamera(scene, jupiter, inBody, origins, PROJECTION);
    const missing = vec3(Number.NaN, Number.NaN, Number.NaN);
    // The position to the rounding of the frames' origins, Jupiter 8.2 × 10¹¹ m from the
    // barycentre (measured 2026-10-22: 0 m apart, the pose rebased to (2 × 10⁸, 0, 0) m exactly);
    // the orientation, which the rebase does not touch, to the bit.
    expect([
      inBody.frame,
      norm(sub(held?.positionM ?? missing, camera?.positionM ?? missing)) < 1e-3,
      held?.orientation,
    ]).toEqual([{ kind: "body", body: FIXTURE_JUPITER }, true, camera?.orientation]);
  });

  it("turns the camera's view into the body's axes with its position", () => {
    const camera = bodyAtmosphereCamera(scene, jupiter, pose, origins, PROJECTION);
    const forward = rotate(camera?.orientation ?? pose.orientation, vec3(0, 0, -1));
    const expected = rotateToBodyFixed(
      atmosphereAxesOf(jupiter),
      rotate(pose.orientation, vec3(0, 0, -1)),
    );
    expect(norm(sub(forward, expected))).toBeLessThan(1e-12);
    expect([camera?.fovXRad, camera?.viewport]).toEqual([PROJECTION.fovXRad, PROJECTION.viewport]);
  });

  it("orients a body whose rotation is not known by its figure's pole", () => {
    const figure = jupiter.appearance?.figure;
    if (jupiter.appearance === null || figure === undefined || figure === null) {
      throw new Error("the fixture's Jupiter has a figure");
    }
    const pole = vec3(0, 0.6, 0.8);
    const unturned: ViewBody = {
      ...jupiter,
      rotation: null,
      appearance: { ...jupiter.appearance, figure: { ...figure, pole } },
    };
    const z = rotateToBody(atmosphereAxesOf(unturned), vec3(0, 0, 1));
    expect(norm(sub(z, pole))).toBeLessThan(1e-12);
  });

  it("forms no camera for a contact, which has no position to draw its air at", () => {
    const heard = sceneModelOf(stateAfterHeartbeat());
    const contactScene = viewSceneOf(heard, shipFrameOf(heard));
    const contact = viewBodyOf(contactScene, FIXTURE_EARTH);
    expect([
      contact.retarded,
      bodyAtmosphereCamera(contactScene, contact, pose, sceneOrigins(contactScene), PROJECTION),
    ]).toEqual([null, null]);
  });
});
