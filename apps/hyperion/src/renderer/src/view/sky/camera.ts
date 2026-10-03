/**
 * Where a view's camera is in the galaxy, and its offset from the sky's observer (plan R06,
 * T13.c; R02 Design note 1).
 */

import type { GalacticPosition } from "@hyperion/protocol";

import { add, type Vec3 } from "../../geometry/vec3";
import type { CameraPose } from "../camera/pose";
import { expressIn, galacticDeltaM, galacticTranslated } from "../coords/position";
import { frameOrigin } from "../coords/relative";
import { sceneOrigins, type ViewScene } from "../scene/model";

/**
 * The camera's galactic position: its frame's origin plus its offset, composed exactly as the
 * scene's camera report does; `null` where the scene's system has no known position.
 */
export function cameraGalacticPosition(
  pose: CameraPose,
  scene: ViewScene,
): GalacticPosition | null {
  if (scene.barycentre === null) {
    return null;
  }
  const origins = sceneOrigins(scene);
  const origin = frameOrigin(pose.frame, origins);
  if (origin.kind === "galactic") {
    return galacticTranslated(origin.position, pose.positionM);
  }
  const galactic = expressIn(
    { ...origin, m: add(origin.m, pose.positionM) },
    { kind: "galactic" },
    origins,
  );
  if (galactic.kind !== "galactic") {
    throw new Error("a position expressed in the galactic frame came back in another frame");
  }
  return galactic.position;
}

/**
 * The camera's offset from the sky's observer, m along the galactic axes; `null` where the
 * camera's galactic position is not known.
 */
export function cameraFromObserverM(
  pose: CameraPose,
  scene: ViewScene,
  observer: GalacticPosition,
): Vec3 | null {
  const camera = cameraGalacticPosition(pose, scene);
  return camera === null ? null : galacticDeltaM(observer, camera);
}
