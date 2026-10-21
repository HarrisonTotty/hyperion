import type { BodyIdHex, SurfaceHeaderDto } from "@hyperion/protocol";
import { useState } from "react";

import {
  NO_READOUT_UNITS,
  type ReadoutUnits,
  type TerrainLabelModel,
  terrainLabelModel,
} from "../../view/terrain/readouts";
import type { SurfaceGroundReadout } from "../../view/terrain/workers/messages";
import { GROUND_READOUT_OPTIONS, type GroundSnapshot } from "./groundReadout";

/** A view's terrain lines as its label block states them, with what they were made from. */
export interface TerrainLines {
  /** The lines (`terrainLabelModel`). */
  readonly model: TerrainLabelModel;
  /** The `FRAME` body's field header, from which the tallest forms are made; `null` with none. */
  readonly header: SurfaceHeaderDto | null;
  /** The readout's answer the lines read while the readout stands, else `null`. */
  readonly readout: SurfaceGroundReadout | null;
}

/**
 * The readout's answer the primary's plate states, by which its tripod and its list's note stand
 * (decision-r10-view-block-room §3): the lines' while the plate holds the readout, `null` where it
 * gives way for room or does not stand.
 *
 * @param readout - Whether the plate's readout stands or gives way (`ViewRoom.readout`).
 */
export function standingReadout(
  lines: TerrainLines | null,
  readout: "stands" | "yields",
): SurfaceGroundReadout | null {
  return lines === null || readout === "yields" ? null : lines.readout;
}

/** The units the lines were last shown in, for their hysteresis, by the snapshot they came from. */
interface HeldUnits {
  readonly snapshot: GroundSnapshot | null;
  readonly body: BodyIdHex | null;
  /** The units the snapshot's lines were made from. */
  readonly before: ReadoutUnits;
  /** The units its lines kept, from which the next snapshot's are made. */
  readonly after: ReadoutUnits;
}

/** No lines shown yet. */
const NOTHING_HELD: HeldUnits = {
  snapshot: null,
  body: null,
  before: NO_READOUT_UNITS,
  after: NO_READOUT_UNITS,
};

/**
 * The `PRIMARY` view's terrain lines from its loop's latest publish (plan R10, R10.T12;
 * `GroundReadouts`), or `null` where it states none: the survey line, and while the readout may
 * stand, `DATUM` and the readout (`terrainLabelModel`).
 *
 * @remarks
 * The units of `FINEST`, `RESOLUTION`, `SLANT RANGE` and the capacity keep their hysteresis from
 * one publish to the next, adjusted during render as each publish arrives, and start afresh when
 * the `FRAME` body changes. The readout stands only while no terrain fault stands, the view's own
 * (`faults`) or the session's (`GroundSnapshot.sessionFault`; decision-r10-t12-leans §5).
 *
 * @param faults - The view's fault lines, the terrain's among them.
 * @param remedy - What the survey line's two failed states end with (`TerrainLabelInput.remedy`):
 *   `VIEW`'s `reopen VIEW to retry` unless given, the descent spike's `relaunch to retry` on the
 *   spike (plan R10, R10.T14.c).
 */
export function useTerrainLines(
  snapshot: GroundSnapshot | null,
  faults: ReadonlyArray<string>,
  remedy?: string,
): TerrainLines | null {
  const [held, setHeld] = useState<HeldUnits>(NOTHING_HELD);
  if (snapshot === null) {
    if (held.snapshot !== null) {
      setHeld(NOTHING_HELD);
    }
    return null;
  }
  const fresh = held.snapshot !== snapshot;
  let before = held.before;
  if (fresh) {
    before = snapshot.body === held.body ? held.after : NO_READOUT_UNITS;
  }
  const model = terrainLabelModel(
    {
      survey: snapshot.survey,
      readout: snapshot.readout,
      faults: snapshot.sessionFault === null ? faults : [...faults, snapshot.sessionFault],
      slantFromCamera: snapshot.slantFromCamera,
      padWidthM: GROUND_READOUT_OPTIONS.padWidthM,
      surfaceUnset: snapshot.surfaceUnset,
      remedy,
    },
    before,
  );
  if (fresh) {
    setHeld({ snapshot, body: snapshot.body, before, after: model.units });
  }
  return {
    model,
    header: snapshot.survey?.header ?? null,
    readout: model.readout.length > 0 ? snapshot.readout : null,
  };
}
