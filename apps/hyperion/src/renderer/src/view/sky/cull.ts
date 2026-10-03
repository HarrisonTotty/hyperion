/**
 * Each view's cull of the sky's listed stars, and the hand-off of the culled stars' light to the
 * band layer (plan R06, Design note 20, T13.a).
 *
 * @remarks
 * The server lists every star brighter than the request's cut, the deepest of the views open, so
 * each view drops those fainter than its own limit: the eye by the limit map's limit in the star's
 * direction, moved by the star's eye colour offset (a hot star is seen fainter, Design note 3); a
 * camera by its one limit against the star's V plus its camera band term (the sensor's response
 * to the star's spectrum, Design note 6). A culled star's light is not lost: it is added to the
 * band layer's texel in its direction, so the view's total light is kept and the band is
 * brightened by exactly what was dropped.
 */

import type { SkyStars } from "@hyperion/protocol";

import { cubeTexelOf } from "./cube";
import { starIlluminanceRgbLx } from "./photometry";

/** A view's star limit: the eye's per direction, or a camera's. */
export type ViewStarLimit =
  | {
      readonly kind: "eye";
      /**
       * The eye's limit, V, in a direction on the galactic axes, at the view's field factor; NaN
       * where the sky carries no eye limit, which keeps every star.
       */
      readonly limitAt: (x: number, y: number, z: number) => number;
    }
  | {
      readonly kind: "camera";
      /** The camera's limit, V (`cameraLimitV`). */
      readonly limitV: number;
    };

/** What a view keeps of the sky, and the light of what it dropped. */
export interface CulledSky {
  /** The indices of the stars kept, in the payload's order (brightest first). */
  readonly kept: Uint32Array;
  /** The number of stars dropped. */
  readonly culledCount: number;
  /**
   * The dropped stars' illuminance per band texel, lx, three channels a texel, face after face
   * (+X, −X, +Y, −Y, +Z, −Z), rows from the top: the band layer adds it to the band's light,
   * divided by each texel's solid angle (`texelSolidAnglesSr` in `cube.ts`) to be a luminance.
   */
  readonly bandIlluminanceLx: Float64Array;
}

/** Whether a view sees one star of the sky, by its index. */
export function starIsSeen(stars: SkyStars, index: number, limit: ViewStarLimit): boolean {
  const v = stars.vMag[index] ?? Number.POSITIVE_INFINITY;
  let seen: boolean;
  switch (limit.kind) {
    case "eye": {
      const at = index * 3;
      const limitV = limit.limitAt(
        stars.directions[at] ?? 0,
        stars.directions[at + 1] ?? 0,
        stars.directions[at + 2] ?? 0,
      );
      seen = Number.isNaN(limitV) || v < limitV + (stars.eyeOffsetMag[index] ?? 0);
      break;
    }
    case "camera":
      seen = v + (stars.cameraBandMag[index] ?? 0) < limit.limitV;
      break;
  }
  return seen;
}

/**
 * Culls the sky's stars for one view and hands the dropped stars' light to the band.
 *
 * @param bandFaceTexels - The band map's face side, texels (the response's `face_texels`).
 */
export function cullSky(stars: SkyStars, limit: ViewStarLimit, bandFaceTexels: number): CulledSky {
  const kept = new Uint32Array(stars.count);
  let keptCount = 0;
  const band = new Float64Array(6 * bandFaceTexels * bandFaceTexels * 3);
  for (let index = 0; index < stars.count; index += 1) {
    if (starIsSeen(stars, index, limit)) {
      kept[keptCount] = index;
      keptCount += 1;
      continue;
    }
    const at = index * 3;
    const { face, column, row } = cubeTexelOf(
      stars.directions[at] ?? 0,
      stars.directions[at + 1] ?? 0,
      stars.directions[at + 2] ?? 0,
      bandFaceTexels,
    );
    const light = starIlluminanceRgbLx(
      stars.vMag[index] ?? Number.POSITIVE_INFINITY,
      stars.chroma[index * 2] ?? 0,
      stars.chroma[index * 2 + 1] ?? 0,
    );
    const texel = ((face * bandFaceTexels + row) * bandFaceTexels + column) * 3;
    for (let channel = 0; channel < 3; channel += 1) {
      band[texel + channel] = (band[texel + channel] ?? 0) + (light[channel] ?? 0);
    }
  }
  return {
    kept: kept.slice(0, keptCount),
    culledCount: stars.count - keptCount,
    bandIlluminanceLx: band,
  };
}
