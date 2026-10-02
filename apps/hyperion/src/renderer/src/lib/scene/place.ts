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
 * The system's barycentre at `time`: the place's barycentre plus its velocity times the time since
 * the place's own, the straight line the simulation's `position_at` follows (plan 08, P08.T7.a),
 * each offset kept in `[0, 1 ly)` with its cell carried.
 *
 * @remarks
 * A place with no velocity or time, known only from the chart, gives its barycentre unchanged; a
 * place with no barycentre gives `null`. Over the clock window's ±1,000 years a system drifts at
 * most some 10 ly, so the result never leaves the galactic frame for a place the scene states.
 *
 * @throws RangeError if the drift leaves the galactic frame's cells, as `galacticTranslated` does.
 */
export function barycentreAt(place: SystemPlace, time: UniverseTime): GalacticPosition | null {
  const { barycentre, velocityMPerS, time: placed } = place;
  if (barycentre === null || velocityMPerS === null || placed === null) {
    return barycentre;
  }
  return galacticTranslated(barycentre, scale(velocityMPerS, secondsBetween(time, placed)));
}
