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
import type { Rgb } from "../photometry/toneCurve";
import type { SpriteStar } from "../wireframe/drawList";
import { starIlluminanceRgbLx } from "./photometry";

/**
 * What a selection's sprites keep from frame to frame: each star's name, its illuminance per linear
 * Rec. 709 channel, lx, and its position from the sky's observer, m, three a star, of the sky's
 * stars it was made from.
 */
interface KeptSelection {
  readonly stars: SkyStars;
  readonly ids: ReadonlyArray<string>;
  readonly illuminancesRgbLx: ReadonlyArray<Rgb>;
  readonly positionsM: Float64Array;
}

/**
 * Each selection's kept parts, made at its first frame (lane perf-render-thread-2): a selection's
 * indices and its sky's stars are set as they are made and never after, so that only the camera's
 * offset, and with it each direction, changes from one frame to the next.
 */
const KEPT_SELECTIONS = new WeakMap<Uint32Array, KeptSelection>();

/** A selection's names, illuminances and positions, those kept where they are of `stars`. */
function keptSelection(stars: SkyStars, indices: Uint32Array): KeptSelection {
  const kept = KEPT_SELECTIONS.get(indices);
  if (kept !== undefined && kept.stars === stars) {
    return kept;
  }
  const ids: string[] = [];
  const illuminancesRgbLx: Rgb[] = [];
  const positionsM = new Float64Array(indices.length * 3);
  indices.forEach((index, i) => {
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
    ids.push(String(index));
    illuminancesRgbLx.push(starIlluminanceRgbLx(v, chromaR, chromaG));
    positionsM[i * 3] = x * distanceM;
    positionsM[i * 3 + 1] = y * distanceM;
    positionsM[i * 3 + 2] = z * distanceM;
  });
  const made = { stars, ids, illuminancesRgbLx, positionsM };
  KEPT_SELECTIONS.set(indices, made);
  return made;
}

/**
 * The selected stars as sprites from a camera.
 *
 * @remarks
 * Each star's name, illuminance and position are kept with its selection, made once (lane
 * perf-render-thread-2), its direction alone made each frame: the same values as made afresh.
 *
 * @param indices - The sprites' indices in `stars` (the selection's `sprites`), never changed once
 *   made.
 * @param cameraFromObserverM - The camera's offset from the sky's observer, m, galactic axes.
 * @throws Error for an index outside the sky's stars.
 */
export function skySpriteStars(
  stars: SkyStars,
  indices: Uint32Array,
  cameraFromObserverM: Vec3,
): SpriteStar[] {
  const { ids, illuminancesRgbLx, positionsM } = keptSelection(stars, indices);
  const sprites: SpriteStar[] = [];
  for (let i = 0; i < ids.length; i += 1) {
    const id = ids[i];
    const illuminanceRgbLx = illuminancesRgbLx[i];
    const [x, y, z] = [positionsM[i * 3], positionsM[i * 3 + 1], positionsM[i * 3 + 2]];
    if (
      id === undefined ||
      illuminanceRgbLx === undefined ||
      x === undefined ||
      y === undefined ||
      z === undefined
    ) {
      throw new Error(`the kept selection's sprite ${String(i)} is missing`);
    }
    const fromCamera = vec3(
      x - cameraFromObserverM.x,
      y - cameraFromObserverM.y,
      z - cameraFromObserverM.z,
    );
    sprites.push({ id, direction: normalise(fromCamera), illuminanceRgbLx });
  }
  return sprites;
}
