/**
 * Which of a view's stars are sprites and which are baked (plan R06, Design note 20, T13.c).
 *
 * @remarks
 * Of the stars a view keeps (its cull), a star is a sprite if it is among the sprite budget's
 * brightest, or if its parallax across a 30 au crossing of the system exceeds a tenth of a pixel
 * (within about 9 ly at 1080p and 60°), so that it can be placed every frame from the camera; the
 * rest are baked into the cube. The payload lists stars brightest first, and the cull keeps that
 * order, so the budget's brightest are the first kept. Sprite and bake take the same light from
 * the same function, `starIlluminanceRgbLx`, so a star handed from one to the other keeps its flux.
 */

import type { SkyStars } from "@hyperion/protocol";

import { METRES_PER_LIGHT_YEAR } from "@hyperion/protocol";

import { bakedBeyondM, type SkyCamera } from "./model";

/** A view's stars split between sprites and the bake, by index in the sky's stars. */
export interface SkySelection {
  /** The sprites, brightest first. */
  readonly sprites: Uint32Array;
  /** The baked stars, brightest first. */
  readonly baked: Uint32Array;
}

/**
 * Splits the stars a view keeps between sprites and the bake.
 *
 * @param kept - The kept stars' indices, brightest first (the cull's `kept`).
 * @param spriteBudget - The most stars drawn as sprites for their brightness (the setting's).
 * @param camera - The view's field of view and width, for the parallax rule.
 * @throws Error for an index outside the sky's stars.
 * @remarks
 * A near star is a sprite whatever the budget: the budget bounds the bright ones, and only the
 * nuclear disc and clusters hold enough near stars to outrun it, where the bake is redone as the
 * camera moves (Design note 20).
 */
export function selectSkySprites(
  stars: SkyStars,
  kept: Uint32Array,
  spriteBudget: number,
  camera: Pick<SkyCamera, "fovDeg" | "widthPx">,
): SkySelection {
  const nearM = bakedBeyondM(camera);
  const sprites: number[] = [];
  const baked: number[] = [];
  for (const index of kept) {
    const distanceLy = stars.distanceLy[index];
    if (distanceLy === undefined) {
      throw new Error(`sky star ${index} is outside the sky's ${stars.count} stars`);
    }
    const distanceM = distanceLy * METRES_PER_LIGHT_YEAR;
    if (sprites.length < spriteBudget || distanceM < nearM) {
      sprites.push(index);
    } else {
      baked.push(index);
    }
  }
  return { sprites: Uint32Array.from(sprites), baked: Uint32Array.from(baked) };
}
