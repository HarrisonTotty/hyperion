/**
 * The sky's sprites for one frame: each selected star's direction from the camera and its light
 * per channel, which R02's sprite path draws through its pixel-integrated point-spread function
 * (plan R06, Design note 20, T13.c).
 *
 * @remarks
 * A star's position relative to the sky's observer is its unit direction times its distance; its
 * direction from the camera is that less the camera's offset from the observer, differenced in
 * `f64` as R02 prescribes (Design note 1), then made a unit vector. For a star beyond the parallax
 * rule's distance the offset moves it by under a tenth of a pixel, but the same arithmetic serves
 * both, so a star crossing the rule does not jump.
 */

import { METRES_PER_LIGHT_YEAR, type SkyStars } from "@hyperion/protocol";

import { normalise, type Vec3, vec3 } from "../../geometry/vec3";
import type { SpriteStar } from "../wireframe/drawList";
import { starIlluminanceRgbLx } from "./photometry";

/**
 * The selected stars as sprites from a camera.
 *
 * @param indices - The sprites' indices in `stars` (the selection's `sprites`).
 * @param cameraFromObserverM - The camera's offset from the sky's observer, m, galactic axes.
 * @throws Error for an index outside the sky's stars.
 */
export function skySpriteStars(
  stars: SkyStars,
  indices: Uint32Array,
  cameraFromObserverM: Vec3,
): SpriteStar[] {
  const sprites: SpriteStar[] = [];
  for (const index of indices) {
    const at = index * 3;
    const distanceLy = stars.distanceLy[index];
    const [x, y, z] = [stars.directions[at], stars.directions[at + 1], stars.directions[at + 2]];
    const v = stars.vMag[index];
    const [chromaR, chromaG] = [stars.chroma[index * 2], stars.chroma[index * 2 + 1]];
    if (
      distanceLy === undefined ||
      x === undefined ||
      y === undefined ||
      z === undefined ||
      v === undefined ||
      chromaR === undefined ||
      chromaG === undefined
    ) {
      throw new Error(`sky star ${index} is outside the sky's ${stars.count} stars`);
    }
    const distanceM = distanceLy * METRES_PER_LIGHT_YEAR;
    const fromCamera = vec3(
      x * distanceM - cameraFromObserverM.x,
      y * distanceM - cameraFromObserverM.y,
      z * distanceM - cameraFromObserverM.z,
    );
    sprites.push({
      id: String(index),
      direction: normalise(fromCamera),
      illuminanceRgbLx: starIlluminanceRgbLx(v, chromaR, chromaG),
    });
  }
  return sprites;
}
