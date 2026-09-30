import {
  galacticPositionFromLy,
  type RequestOf,
  type SystemIdHex,
  type UniverseIdHex,
  universeTimeFromYears,
} from "@hyperion/protocol";

import type { CentreLy } from "../../lib/galaxy/model";
import {
  REQUEST_TIMEOUT_MS,
  type RequestState,
  useServerRequest,
} from "../../lib/useServerRequest";

/** What lies between the chart's centre and the selected system (plan 07, P07.T11.c). */
export interface LineExtinction {
  /** Visual extinction A(V), in magnitudes. */
  readonly aVMag: number;
  /** Reddening E(B-V), in magnitudes. */
  readonly eBVMag: number;
  /** Extinction in the K band, A(K), in magnitudes. */
  readonly aKMag: number;
  /** Hydrogen column N(H) of every phase, in atoms per square centimetre. */
  readonly hydrogenColumnPerCm2: number;
}

/** What the chart asks about the extinction to its selection. */
export interface ExtinctionInput {
  /** The open universe, or `null` when none is. */
  readonly universe: UniverseIdHex | null;
  /** The centre of the chart on show, where the line starts, or `null` before an answer. */
  readonly centreLy: CentreLy | null;
  /** The selected system, where the line ends, or `null` with none selected. */
  readonly system: SystemIdHex | null;
  /** The time of the chart on show, in years from the epoch, which places the system. */
  readonly timeYr: number;
  /** Changing it asks the same line again, which is what `RETRY` does after a failure. */
  readonly generation: number;
}

/** Where the request for the line stands, and its figures once they are in. */
export interface Extinction {
  readonly state: RequestState<"extinction">;
  /**
   * The line's figures for the selection and time asked about, or `null` until they arrive, when
   * the server names no such system, and after a failure: never a figure of another selection.
   */
  readonly reading: LineExtinction | null;
}

/** The `extinction` request for one system from the chart's centre, or `null` for none. */
function extinctionRequest({
  universe,
  centreLy,
  system,
  timeYr,
}: ExtinctionInput): RequestOf<"extinction"> | null {
  if (universe === null || centreLy === null || system === null) {
    return null;
  }
  return {
    kind: "extinction",
    universe,
    origin: galacticPositionFromLy(centreLy),
    time: universeTimeFromYears(timeYr),
    targets: [{ type: "system", id: system }],
  };
}

/**
 * Asks the server for the extinction from the chart's centre to the selected system at the chart's
 * time (plan 07, P07.T11.c).
 *
 * @remarks
 * One `System` target through `useServerRequest`, whose body is compared by value: a newer
 * selection, centre or time supersedes the request in flight on the client and cancels it on the
 * server. The reading is only ever the answer to the request as it now stands, so a changed
 * selection shows the missing state until its own answer, never the last system's figures. The
 * line is the seed's own clumpy gas at the server's fixed budget of steps, the same from either end.
 */
export function useExtinction(input: ExtinctionInput): Extinction {
  const state = useServerRequest<"extinction">(
    extinctionRequest(input),
    REQUEST_TIMEOUT_MS,
    input.generation,
  );
  const [target] = state.kind === "ok" ? state.response.targets : [];
  const reading =
    target?.status === "ok"
      ? {
          aVMag: target.a_v_mag,
          eBVMag: target.e_b_v_mag,
          aKMag: target.a_k_mag,
          hydrogenColumnPerCm2: target.hydrogen_column_per_cm2,
        }
      : null;
  return { state, reading };
}
