/**
 * The client's sky: a `sky` response with its decoded stars and band, and the rules for asking
 * again and for holding a sky through a request asked again (plan R06, Design notes 5, 13 and 20,
 * T12 and T12.b).
 *
 * @remarks
 * A sky is asked on arrival in a system, past its `valid_until`, on a jump of the display time
 * back before the time it was computed for, and when a camera has moved so far from the observer
 * that the nearest baked star would shift by a tenth of a pixel (its parallax, Design note 13).
 * Stars near enough to shift by more as the camera crosses the system are sprites, placed every
 * frame (Design note 20), and never baked, so they do not count here. A view that asks for more
 * than the held sky's request (a deeper camera limit, a larger N_max, the eye, a cone) asks again,
 * and so does one holding a reply that is not final once no request is in flight: its request
 * ended before its last reply, as when the link dropped (R06.T11.d).
 *
 * A sky asked again within one arrival arrives nearest first, as the first did, so its first
 * replies hold less than the sky held. Through a request asked for the held sky's own time, the
 * held sky stays until a reply is as complete as it, or until its parallax reaches a pixel
 * ({@link keepsHeldSky}, R06.T12.b).
 */

import {
  type GalacticPosition,
  MAX_SKY_STARS,
  METRES_PER_LIGHT_YEAR,
  type SkyBand,
  type SkyRequest,
  type SkyResponse,
  type SkyStars,
  type UniverseTime,
} from "@hyperion/protocol";

import { norm } from "../../geometry/vec3";
import { galacticDeltaM } from "../coords/position";

/** One au, m (IAU 2012 Resolution B2). */
const AU_M = 149_597_870_700;

/**
 * The system crossing a parallax sprite is judged over, m: 30 au, the brainstorm's journey across a
 * system (Design note 20).
 */
export const PARALLAX_BASELINE_M = 30 * AU_M;

/** The shift, px, at which a star is a parallax sprite or the bake is redone: a tenth of a pixel. */
export const PARALLAX_THRESHOLD_PX = 0.1;

/**
 * The shift, px, at which a sky held through a request asked again gives way to that request's
 * newest reply, however incomplete: one pixel, ten times {@link PARALLAX_THRESHOLD_PX}, so that no
 * camera carries a sky past a pixel of its nearest baked star's parallax (R06.T12.b; decided
 * 2026-10-13, `decision-r06-t17-findings.md` §2).
 */
export const PARALLAX_HOLD_PX = 1;

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

/** A camera as the parallax rule reads it. */
export interface SkyCamera {
  /** Where the camera is; the rule measures its offset from the held sky's observer. */
  readonly position: GalacticPosition;
  /** Its horizontal field of view, degrees. */
  readonly fovDeg: number;
  /** Its width, px. */
  readonly widthPx: number;
}

/**
 * Why a sky is asked again, or `null` while the one held stands: `arrival`, no sky of this arrival
 * held; `expired` and `jump`, the held sky stands for another time; `partial`, its request ended
 * before its final reply; `limits`, a view asks for more than its request did; `parallax`, a camera
 * has moved from its observer.
 */
export type SkyRequestReason = "arrival" | "partial" | "expired" | "jump" | "limits" | "parallax";

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

/** The margin by which a camera's limit must deepen to ask again, mag: rounding is not a reason. */
const LIMIT_MARGIN_MAG = 0.05;

/**
 * Whether `now` asks for stars the held sky's request did not: a deeper camera limit, a larger
 * N_max, the eye where it was not asked or with other parameters, or another cone. A shallower
 * limit or a smaller N_max is the views' cull, never a new census.
 */
function asksMore(now: SkyRequest, held: SkyRequest): boolean {
  const deeper =
    (now.camera_limit_v ?? Number.NEGATIVE_INFINITY) >
    (held.camera_limit_v ?? Number.NEGATIVE_INFINITY) + LIMIT_MARGIN_MAG;
  const more = (now.n_max ?? MAX_SKY_STARS) > (held.n_max ?? MAX_SKY_STARS);
  const eye =
    now.eye !== null &&
    (held.eye === null ||
      now.eye.field_factor !== held.eye.field_factor ||
      now.eye.age_years !== held.eye.age_years ||
      now.eye.pigmentation !== held.eye.pigmentation);
  const cone =
    (now.cone === null) !== (held.cone === null) ||
    (now.cone !== null &&
      held.cone !== null &&
      (now.cone.half_angle_deg !== held.cone.half_angle_deg ||
        now.cone.axis.some((component, axis) => component !== held.cone?.axis[axis])));
  return deeper || more || eye || cone;
}

/** What the request rule reads of now. */
export interface SkyNow {
  /** The request that would be sent now. */
  readonly request: SkyRequest;
  /** Every open view's camera. */
  readonly cameras: ReadonlyArray<SkyCamera>;
}

/**
 * How far a camera's offset from a sky's observer shifts the sky's nearest baked star, px: the
 * parallax rule's measure (Design note 13), the parallax sprites left out (Design note 20).
 */
export function parallaxShiftPx(
  sky: Pick<SkyModel, "request" | "stars">,
  camera: SkyCamera,
): number {
  const nearestM = nearestStarBeyondM(sky.stars, bakedBeyondM(camera));
  const offsetM = norm(galacticDeltaM(sky.request.observer, camera.position));
  return offsetM / nearestM / pixelAngleRad(camera);
}

/**
 * Why the sky should be asked again, or `null` while the one held stands (Design note 13).
 *
 * @param held - The sky held, or `null` with none.
 * @remarks
 * A held reply that is not final and stands for another time is asked again as `expired` or
 * `jump`, not `partial`, so that the hold reads the time it stands for ({@link keepsHeldSky}).
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
  // A sky arriving nearest first whose request ended before its final reply (R06.T11.d).
  if (!held.response.final) {
    return "partial";
  }
  if (asksMore(now.request, held.request)) {
    return "limits";
  }
  if (now.cameras.some((camera) => parallaxShiftPx(held, camera) >= PARALLAX_THRESHOLD_PX)) {
    return "parallax";
  }
  return null;
}

/**
 * Whether a reply is at least as complete as a held sky in every layer (R06.T12.b): final, or,
 * layer by layer, final, or complete at least as far (`complete_to_ly`) where the held sky's layer
 * is not final.
 *
 * @remarks
 * A layer the held sky states and the reply does not is not as complete. A layer of the reply's
 * that the held sky does not state holds nothing back.
 */
export function asCompleteAs(reply: SkyResponse, held: SkyResponse): boolean {
  if (reply.final) {
    return true;
  }
  return held.census.every((was) => {
    const now = reply.census.find((layer) => layer.layer === was.layer);
    return (
      now !== undefined && (now.final || (!was.final && now.complete_to_ly >= was.complete_to_ly))
    );
  });
}

/**
 * Whether a re-ask is for the held sky's own time, through which it is held (R06.T12.b): not on
 * `arrival`, with no sky of the arrival held, nor on `expired` or `jump`, whose held sky stands for
 * another time.
 */
function holdsThrough(reason: SkyRequestReason): boolean {
  let holds: boolean;
  switch (reason) {
    case "partial":
    case "limits":
    case "parallax":
      holds = true;
      break;
    case "arrival":
    case "expired":
    case "jump":
      holds = false;
      break;
  }
  return holds;
}

/**
 * Whether the held sky stays in place of a reply of a request asked again within its arrival
 * (R06.T12.b; decided 2026-10-13, `decision-r06-t17-findings.md` §2), so that a re-asked sky never
 * shows less than it held.
 *
 * @param reason - Why the request was asked ({@link skyRequestReason}).
 * @param held - The sky held: one of an earlier request of the same arrival.
 * @param reply - The newest reply of the request asked again.
 * @param cameras - Every open view's camera, now.
 * @remarks
 * Through a re-ask for `partial`, `limits` or `parallax`, the held sky stays until a reply is
 * final or {@link asCompleteAs} it, or until its parallax shift for any open camera
 * ({@link parallaxShiftPx}) reaches {@link PARALLAX_HOLD_PX}. Through `expired` and `jump`, whose
 * held sky stands for another time, and on `arrival`, each reply is held as it comes. Once a reply
 * of the request is held, so is each later one, which this rule is not asked about.
 */
export function keepsHeldSky(
  reason: SkyRequestReason,
  held: Pick<SkyModel, "request" | "response" | "stars">,
  reply: SkyResponse,
  cameras: ReadonlyArray<SkyCamera>,
): boolean {
  return (
    holdsThrough(reason) &&
    !asCompleteAs(reply, held.response) &&
    cameras.every((camera) => parallaxShiftPx(held, camera) < PARALLAX_HOLD_PX)
  );
}
