import { SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";
import { useState } from "react";

import type { SystemModel } from "../../lib/system/model";
import type { SystemModelResult } from "../../lib/system/wire";
import type { RequestState } from "../../lib/useServerRequest";
import { nextRequestTime } from "./requestTime";
import { type SummaryTarget, useSystemSummary } from "./useSystemSummary";

/** Seconds in a megayear of Julian years. */
const SECONDS_PER_MYR = 1e6 * SECONDS_PER_JULIAN_YEAR;

/** What the `SYSTEM` display is drawn from, and where its requests stand. */
export interface SystemData {
  /** Where the latest `system_summary` request stands. */
  readonly summary: RequestState<"system_summary">;
  /** The answer on show, kept through a newer request; `null` before the first. */
  readonly shown: SystemModelResult | null;
  /** The time the display last asked the server for. */
  readonly requestTime: UniverseTime;
}

/**
 * The instants at which what an answer describes changes, so that a display time moved past one of
 * them is asked about again (D18): each star's death inside the clock window and, for a system not
 * yet formed, its birth, the answer's time less its (negative) age, rounded down to the second so
 * that a step onto it asks.
 */
function changesOf(model: SystemModel | null): ReadonlyArray<UniverseTime> {
  if (model === null) {
    return [];
  }
  const deaths = model.hosts
    .map((host) => host.deathTime)
    .filter((time): time is UniverseTime => time !== null);
  if (model.formed) {
    return deaths;
  }
  const birthS = Math.floor(model.time.seconds - model.ageMyr * SECONDS_PER_MYR);
  return Number.isSafeInteger(birthS) ? [...deaths, { seconds: birthS, nanos: 0 }] : deaths;
}

/**
 * Owns the requests of one `SYSTEM` display: what it asks the server about its system, and when it
 * asks again as its time moves (plan 14, P14.T41.a and design note D18).
 *
 * @remarks
 * The display opens held at its target's time and asks for it. When the display time moves more
 * than a year from the time last asked for, or past a star's death or the system's birth, it asks
 * again at the display time; between, it draws the last answer at the display time by propagating
 * its orbits, and asks nothing, so that a step of an hour costs no request. The time asked for is
 * adjusted during render, as `useServerRequest` adjusts its body, so the new request goes out in
 * the render the step lands in.
 *
 * This is where the bodies join: `system_bodies` (P14.T35.b) is asked at the same time, by a
 * `useSystemBodies` beside `useSystemSummary`, and its bodies' `valid_until` join the instants
 * that make the display ask again.
 *
 * @param target - The system, or `null` when the display has none.
 * @param displayTime - The display's own time, which the operator steps.
 * @param generation - Changing it asks again, as `RETRY` does.
 */
export function useSystemData(
  target: SummaryTarget | null,
  displayTime: UniverseTime,
  generation: number,
): SystemData {
  const [requestTime, setRequestTime] = useState(displayTime);
  const summary = useSystemSummary(target, requestTime, generation);
  const model = summary.shown?.kind === "ok" ? summary.shown.model : null;
  const next = nextRequestTime(requestTime, displayTime, changesOf(model));
  if (next !== requestTime) {
    setRequestTime(next);
  }
  return { summary: summary.state, shown: summary.shown, requestTime: next };
}
