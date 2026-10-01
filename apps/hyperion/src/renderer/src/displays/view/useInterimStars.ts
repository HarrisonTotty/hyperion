import type {
  GalacticPosition,
  SystemIdHex,
  SystemsInRange,
  UniverseIdHex,
  UniverseTime,
} from "@hyperion/protocol";
import { useState } from "react";

import { type RequestState, useServerRequest } from "../../lib/useServerRequest";
import {
  INTERIM_QUERIES,
  type InterimField,
  interimCountLine,
  interimField,
  interimRequest,
  retryRadiusLy,
} from "../../view/stars/interim";

/** Where the interim stars are asked about: the open universe and the scene's system. */
export interface InterimStarsInput {
  /** The open universe, or `null` when none is: then nothing is asked. */
  readonly universe: UniverseIdHex | null;
  readonly system: SystemIdHex;
  /** The system's barycentre, the queries' centre. */
  readonly centre: GalacticPosition;
  /** The scene's time on arrival. */
  readonly time: UniverseTime;
}

/** The interim field and its count line, once any query has answered. */
export interface InterimStars {
  readonly field: InterimField | null;
  /** `STARS n DRAWN · m WITHOUT V · RADII e/d/c/a ly`, or `null` before an answer. */
  readonly countLine: string | null;
}

/** The queries of one arrival: where and when they were asked, their radii and their retries. */
interface Arrival {
  readonly universe: UniverseIdHex | null;
  readonly system: SystemIdHex;
  readonly centre: GalacticPosition;
  readonly time: UniverseTime;
  readonly radiiLy: ReadonlyArray<number>;
  readonly retried: ReadonlyArray<boolean>;
}

function arrivalAt(input: InterimStarsInput): Arrival {
  return {
    ...input,
    radiiLy: INTERIM_QUERIES.map((query) => query.radiusLy),
    retried: INTERIM_QUERIES.map(() => false),
  };
}

/**
 * The interim star field (plan R02, R02.T16.a): the four `systems_in_range` queries of Design note
 * 19 about the scene's system, with the briefs, at the server's limit of 20,000, merged by ID.
 *
 * @remarks
 * Asked again only on arrival in another system (or another universe), never as the camera moves:
 * the parallax across a system is under a tenth of a pixel beyond about 9 ly. A query whose answer
 * left its own floor out over the limit is asked once more at the shrunk radius
 * `r × (0.9 × 20,000 ÷ Σ expected)^⅓`, and never a second time; the count line states the radii
 * used.
 */
export function useInterimStars(input: InterimStarsInput): InterimStars {
  const [arrival, setArrival] = useState<Arrival>(() => arrivalAt(input));
  let current = arrival;
  if (input.system !== arrival.system || input.universe !== arrival.universe) {
    current = arrivalAt(input);
    setArrival(current);
  }
  const bodies = INTERIM_QUERIES.map((query, index) =>
    current.universe === null
      ? null
      : interimRequest(current.universe, current.centre, current.time, {
          layer: query.layer,
          radiusLy: current.radiiLy[index] ?? query.radiusLy,
        }),
  );
  // Four queries, always four, so that the hooks are called in the same order every render.
  const states: ReadonlyArray<RequestState<"systems_in_range">> = [
    useServerRequest<"systems_in_range">(bodies[0] ?? null),
    useServerRequest<"systems_in_range">(bodies[1] ?? null),
    useServerRequest<"systems_in_range">(bodies[2] ?? null),
    useServerRequest<"systems_in_range">(bodies[3] ?? null),
  ];

  // A floor left out over the limit asks once more, adjusted during render as an answer arrives.
  const retryAt = states.map((state, index) => {
    const query = INTERIM_QUERIES[index];
    if (state.kind !== "ok" || query === undefined || current.retried[index] === true) {
      return null;
    }
    return retryRadiusLy(state.response, {
      layer: query.layer,
      radiusLy: current.radiiLy[index] ?? query.radiusLy,
    });
  });
  if (retryAt.some((radius) => radius !== null)) {
    setArrival({
      ...current,
      radiiLy: current.radiiLy.map((radius, index) => retryAt[index] ?? radius),
      retried: current.retried.map((retried, index) => retried || retryAt[index] !== null),
    });
  }

  const answers: SystemsInRange[] = states.flatMap((state) =>
    state.kind === "ok" ? [state.response] : [],
  );
  if (answers.length === 0) {
    return { field: null, countLine: null };
  }
  const field = interimField(answers, current.centre, current.system);
  return { field, countLine: interimCountLine(field, current.radiiLy) };
}
