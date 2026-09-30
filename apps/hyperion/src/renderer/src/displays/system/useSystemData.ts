import { SECONDS_PER_JULIAN_YEAR, type UniverseTime } from "@hyperion/protocol";
import { useState } from "react";

import type { SystemBodiesResult } from "../../lib/system/bodiesWire";
import type { SystemBodies, SystemModel } from "../../lib/system/model";
import type { SystemModelResult } from "../../lib/system/wire";
import type { RequestState } from "../../lib/useServerRequest";
import { nextRequestTime } from "./requestTime";
import { useSystemBodies } from "./useSystemBodies";
import { type SummaryTarget, useSystemSummary } from "./useSystemSummary";

/** Seconds in a megayear of Julian years. */
const SECONDS_PER_MYR = 1e6 * SECONDS_PER_JULIAN_YEAR;

/** Nanoseconds in a second. */
const NANOS_PER_SECOND = 1_000_000_000;

/** What the `SYSTEM` display is drawn from, and where its requests stand. */
export interface SystemData {
  /** Where the latest `system_summary` request stands. */
  readonly summary: RequestState<"system_summary">;
  /** Where the latest `system_bodies` request stands. */
  readonly bodiesState: RequestState<"system_bodies">;
  /**
   * The hosts on show, kept through a newer request; `null` before the first answer. They are the
   * bodies' answer's own while one is on show, so that the bodies and the stars they orbit are of
   * one answer, and the summary's otherwise.
   */
  readonly shown: SystemModelResult | null;
  /** The bodies' answer on show, kept through a newer request; `null` before the first. */
  readonly bodies: SystemBodiesResult | null;
  /**
   * Whether the server does not serve `system_bodies` and has sent no bodies: the hosts show alone,
   * and the note says the planets are not yet modelled (the orchestrator's ruling 59.1).
   */
  readonly bodiesUnserved: boolean;
  /** The time the display last asked the server for. */
  readonly requestTime: UniverseTime;
}

/** The instant one nanosecond after `time`, the first at which elements held until `time` do not. */
function justAfter(time: UniverseTime): UniverseTime {
  return time.nanos + 1 < NANOS_PER_SECOND
    ? { seconds: time.seconds, nanos: time.nanos + 1 }
    : { seconds: time.seconds + 1, nanos: 0 };
}

/**
 * The instants at which what an answer describes changes, so that a display time moved past one of
 * them is asked about again (D18): each star's death inside the clock window and, for a system not
 * yet formed, its birth, the answer's time less its (negative) age, rounded down to the second so
 * that a step onto it asks; and the first instant past each body's orbit's `valid_until`, the last
 * at which its elements hold.
 */
function changesOf(
  model: SystemModel | null,
  bodies: SystemBodies | null,
): ReadonlyArray<UniverseTime> {
  if (model === null) {
    return [];
  }
  const deaths = model.hosts
    .map((host) => host.deathTime)
    .filter((time): time is UniverseTime => time !== null);
  const expiries = (bodies?.bodies ?? []).flatMap((body) =>
    body.orbit.state === "ok" && body.orbit.value.validUntil !== null
      ? [justAfter(body.orbit.value.validUntil)]
      : [],
  );
  if (model.formed) {
    return [...deaths, ...expiries];
  }
  const birthS = Math.floor(model.time.seconds - model.ageMyr * SECONDS_PER_MYR);
  return Number.isSafeInteger(birthS) ? [...deaths, { seconds: birthS, nanos: 0 }] : deaths;
}

/**
 * Owns the requests of one `SYSTEM` display: what it asks the server about its system, and when it
 * asks again as its time moves (plan 14, P14.T41.a and design note D18).
 *
 * @remarks
 * The display opens held at its target's time and asks for its stars (`system_summary`) and its
 * bodies (`system_bodies`) at that time, once each. When the display time moves more than a year
 * from the time last asked for, past a star's death or the system's birth, or past the earliest
 * `valid_until` of the bodies' orbits, it asks both again at the display time; between, it draws
 * the last answers at the display time by propagating their orbits, and asks nothing, so that a
 * step of an hour costs no request. The time asked for is adjusted during render, as
 * `useServerRequest` adjusts its body, so the new requests go out in the render the step lands in.
 *
 * The hosts are drawn from the bodies' answer while one is on show, since it carries them with the
 * bodies at one instant; before, and while the server answers `system_bodies` with `unsupported`,
 * from the summary's.
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
  const bodies = useSystemBodies(target, requestTime, generation);
  // The bodies' answer itself, which is a hosts' answer too, so that what is shown keeps its
  // identity from one render to the next and the map paints only when it changes.
  const shown: SystemModelResult | null =
    bodies.shown?.kind === "ok" ? bodies.shown : summary.shown;
  const model = shown?.kind === "ok" ? shown.model : null;
  const bodiesModel = bodies.shown?.kind === "ok" ? bodies.shown.bodies : null;
  const next = nextRequestTime(requestTime, displayTime, changesOf(model, bodiesModel));
  if (next !== requestTime) {
    setRequestTime(next);
  }
  return {
    summary: summary.state,
    bodiesState: bodies.state,
    shown,
    bodies: bodies.shown,
    bodiesUnserved: bodies.unserved,
    requestTime: next,
  };
}
