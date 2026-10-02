/**
 * Where a scene's system is at a time (rendering plan R03, R03.T16): its {@link SystemPlace}
 * carried along the straight line systems move on.
 */
import type { GalacticPosition, UniverseTime } from "@hyperion/protocol";

import { scale } from "../../geometry/vec3";
import { galacticTranslated } from "../../view/coords/position";
import { secondsBetween } from "./lightTime";
import type { SystemPlace } from "./model";

/**
 * The system's barycentre at `time`: a stated place's barycentre plus its velocity times the time
 * since the place's own, the straight line the simulation's `position_at` follows (plan 08,
 * P08.T7.a), each offset kept in `[0, 1 ly)` with its cell carried.
 *
 * @remarks
 * A charted place, which has no velocity or time, gives its barycentre unchanged, and an unknown
 * place `null`. The scene adapter refuses a stated place whose speed is not below c or whose cells
 * leave the galactic frame's, so that over any time the wire can state its drift stays inside the
 * frame.
 *
 * @throws RangeError if the drift leaves the galactic frame's cells, as `galacticTranslated` does:
 *   a bug for a place the adapter accepted.
 */
export function barycentreAt(place: SystemPlace, time: UniverseTime): GalacticPosition | null {
  let at: GalacticPosition | null;
  switch (place.kind) {
    case "stated":
      at = galacticTranslated(
        place.barycentre,
        scale(place.velocityMPerS, secondsBetween(time, place.time)),
      );
      break;
    case "charted":
      at = place.barycentre;
      break;
    case "unknown":
      at = null;
      break;
  }
  return at;
}
