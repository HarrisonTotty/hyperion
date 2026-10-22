/**
 * A body's atmosphere camera, where the view draws the body (plan R08, R08.T8.b; Design note 13).
 *
 * @remarks
 * The march draws a body's atmosphere about the body's drawn centre, `ViewBody.centreM`, which
 * follows R03's rule (`view/scene/fromServer.ts`): the ship's local body at its `geometricM`, the
 * state collision uses, and every other body at its `apparentM`, where the ship sees it, a free
 * camera's own local body included, so that a moon's sky shows its planet where the camera sees
 * it. The camera's offset from that centre is formed in `f64` through the view's one difference
 * against the camera (`relativeToCamera`), then turned into the body-fixed axes by the body's
 * rotation at the time it is drawn at, or, where the rotation is not known, by the axes about the
 * pole the figure is drawn about (R07's `figurePole`, +z without one), as R07's disc and smooth
 * mesh place it: the atmosphere's figure is a spheroid of revolution, so only its pole orients it.
 * A contact, which has no position the view may draw an atmosphere at, has none.
 *
 * Nothing calls it yet: R08.T10.b draws each body's atmosphere in the photorealistic view with it.
 */

import { scale } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { conjugate, multiply, quaternionFromRows } from "../camera/quaternion";
import { type CameraOrigins, relativeToCamera } from "../coords/relative";
import { type Rotation3, rotateToBodyFixed } from "../coords/rotation";
import { figurePole } from "../bodies/oblate";
import { poleAxes } from "../bodies/smoothMesh";
import type { ViewSize } from "../engine/types";
import type { ViewBody, ViewScene } from "../scene/model";
import type { AtmosphereCamera } from "./hillaire";

/** The view's projection the atmosphere camera keeps: its horizontal field and presented size. */
export interface AtmosphereProjection {
  /** The horizontal field of view, rad. */
  readonly fovXRad: number;
  /** The presented size, device pixels. */
  readonly viewport: ViewSize;
}

/**
 * The rotation from a body's body-fixed axes to the galactic ones that its atmosphere is drawn in:
 * its own where it is known, and otherwise the axes about its figure's pole (+z without one).
 */
export function atmosphereAxesOf(body: ViewBody): Rotation3 {
  if (body.rotation !== null) {
    return body.rotation;
  }
  const figure = body.appearance?.figure ?? null;
  return poleAxes(figure === null ? { x: 0, y: 0, z: 1 } : figurePole(figure));
}

/**
 * The camera of `body`'s atmosphere for the view at `pose` (Design note 13): its position from the
 * body's drawn centre, `f64`, and its orientation, each in the body-fixed axes; `null` for a
 * contact (`ViewBody.retarded` is `null`).
 *
 * @param origins - The scene's origins (`sceneOrigins`), which resolve the camera's frame.
 */
export function bodyAtmosphereCamera(
  scene: ViewScene,
  body: ViewBody,
  pose: CameraPose,
  origins: CameraOrigins,
  projection: AtmosphereProjection,
): AtmosphereCamera | null {
  if (body.retarded === null) {
    return null;
  }
  const centreM = relativeToCamera(
    { kind: "system", system: scene.system, m: body.centreM },
    pose,
    origins,
  );
  const axes = atmosphereAxesOf(body);
  return {
    positionM: rotateToBodyFixed(axes, scale(centreM, -1)),
    orientation: multiply(conjugate(quaternionFromRows(axes.rows)), pose.orientation),
    fovXRad: projection.fovXRad,
    viewport: projection.viewport,
  };
}
