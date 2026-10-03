/**
 * The client's sky: a `sky` response with its decoded stars and band, and the rules for asking
 * again (plan R06, Design notes 5, 13 and 20, T12).
 *
 * @remarks
 * A sky is asked on arrival in a system, past its `valid_until`, on a jump of the display time
 * back before the time it was computed for, and when a camera has moved so far from the observer
 * that the nearest baked star would shift by a tenth of a pixel (its parallax, Design note 13).
 * Stars near enough to shift by more as the camera crosses the system are sprites, placed every
 * frame (Design note 20), and never baked, so they do not count here.
 */

import {
  METRES_PER_LIGHT_YEAR,
  type SkyBand,
  type SkyRequest,
  type SkyResponse,
  type SkyStars,
  type UniverseTime,
} from "@hyperion/protocol";

/** One au, m (IAU 2012 Resolution B2). */
const AU_M = 149_597_870_700;

/**
 * The system crossing a parallax sprite is judged over, m: 30 au, the brainstorm's journey across a
 * system (Design note 20).
 */
export const PARALLAX_BASELINE_M = 30 * AU_M;

/** The shift, px, at which a star is a parallax sprite or the bake is redone: a tenth of a pixel. */
export const PARALLAX_THRESHOLD_PX = 0.1;

/** The sky one view draws from. */
export interface SkyModel {
  /** The request it answers. */
  readonly request: SkyRequest;
  readonly response: SkyResponse;
  readonly stars: SkyStars;
  readonly band: SkyBand;
  /** Whether the link was lost since it arrived: it is kept, and shown as stale. */
  readonly stale: boolean;
}

/** A camera's geometry, as the parallax rule reads it. */
export interface SkyCamera {
  /** How far the camera is from the sky's observer, m. */
  readonly offsetFromObserverM: number;
  /** Its horizontal field of view, degrees. */
  readonly fovDeg: number;
  /** Its width, px. */
  readonly widthPx: number;
}

/** Why a sky is asked again, or `null` while the one held stands. */
export type SkyRequestReason = "arrival" | "expired" | "jump" | "parallax";

/** One pixel's angle across a camera, rad: its field of view over its width. */
export function pixelAngleRad(camera: Pick<SkyCamera, "fovDeg" | "widthPx">): number {
  return (camera.fovDeg * Math.PI) / 180 / camera.widthPx;
}

/**
 * The distance beyond which a star is baked for a camera, m: inside it, a 30 au crossing shifts a
 * star by more than a tenth of a pixel and it is a parallax sprite (Design note 20).
 */
export function bakedBeyondM(camera: Pick<SkyCamera, "fovDeg" | "widthPx">): number {
  return PARALLAX_BASELINE_M / (PARALLAX_THRESHOLD_PX * pixelAngleRad(camera));
}

/**
 * The nearest star of the sky beyond a distance, m, or `Infinity` with none.
 *
 * @param beyondM - The distance inside which stars are skipped (the parallax sprites).
 */
export function nearestStarBeyondM(stars: SkyStars, beyondM: number): number {
  let nearest = Number.POSITIVE_INFINITY;
  for (let i = 0; i < stars.count; i += 1) {
    const distanceM = (stars.distanceLy[i] ?? Number.POSITIVE_INFINITY) * METRES_PER_LIGHT_YEAR;
    if (distanceM >= beyondM && distanceM < nearest) {
      nearest = distanceM;
    }
  }
  return nearest;
}

/** Whether `a` is earlier than `b`. */
function earlier(a: UniverseTime, b: UniverseTime): boolean {
  return a.seconds < b.seconds || (a.seconds === b.seconds && a.nanos < b.nanos);
}

/** What the request rule reads of now. */
export interface SkyNow {
  /** The request that would be sent now. */
  readonly request: SkyRequest;
  /** Every open view's camera. */
  readonly cameras: ReadonlyArray<SkyCamera>;
}

/**
 * Why the sky should be asked again, or `null` while the one held stands (Design note 13).
 *
 * @param held - The sky held, or `null` with none.
 */
export function skyRequestReason(held: SkyModel | null, now: SkyNow): SkyRequestReason | null {
  if (
    held === null ||
    held.request.universe !== now.request.universe ||
    held.request.exclude_system !== now.request.exclude_system
  ) {
    return "arrival";
  }
  const time = now.request.time;
  if (earlier(held.response.valid_until, time)) {
    return "expired";
  }
  if (earlier(time, held.request.time)) {
    return "jump";
  }
  for (const camera of now.cameras) {
    const nearestM = nearestStarBeyondM(held.stars, bakedBeyondM(camera));
    const shiftPx = camera.offsetFromObserverM / nearestM / pixelAngleRad(camera);
    if (shiftPx >= PARALLAX_THRESHOLD_PX) {
      return "parallax";
    }
  }
  return null;
}
