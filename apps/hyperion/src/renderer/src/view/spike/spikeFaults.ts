/**
 * The descent spike's own remedy (plan R10, R10.T14.c; decision-r10-t14c-guide Q2; the guide's
 * `DESCENT SPIKE` row): the spike has no `VIEW` to reopen, and its terrain is made once a run, so a
 * remedy that `VIEW` gives as `reopen VIEW to retry` reads `relaunch to retry` on it.
 *
 * @remarks
 * The terrain's faults keep `VIEW`'s texts in the session (`TerrainSession.terrainFaults`), so that
 * every test of them (`isTerrainFault`: `terrainStatementOf`'s suppression of `TERRAIN: STREAMING`
 * and `TERRAIN: DETAIL LIMITED`, `terrainReadoutStands`) holds on the spike as on `VIEW`; the spike
 * swaps the remedy only where it shows a fault ({@link spikeFaultLine}) or ends its run on one.
 */

import {
  isTerrainFault,
  TERRAIN_COMPUTATION_FAILED,
  TERRAIN_NOT_AVAILABLE,
  type TerrainFault,
} from "../terrain/annunciation";

/**
 * What every remedy of the spike's own faults reads: its terrain's faults, its survey line's two
 * failed states and `TERRAIN NOT MEASURED`, so that a later ruling on them is this one edit.
 */
export const SPIKE_RETRY = "relaunch to retry";

/**
 * The terrain's fault on the spike, by its cause: `VIEW`'s `TERRAIN NOT AVAILABLE` with the spike's
 * remedy, `TERRAIN NOT AVAILABLE: height computation failed, relaunch to retry` and
 * `TERRAIN NOT AVAILABLE: terrain computation failed, relaunch to retry`.
 */
export const SPIKE_TERRAIN_FAULTS: Readonly<Record<TerrainFault, string>> = {
  [TERRAIN_NOT_AVAILABLE]: `TERRAIN NOT AVAILABLE: height computation failed, ${SPIKE_RETRY}`,
  [TERRAIN_COMPUTATION_FAILED]: `TERRAIN NOT AVAILABLE: terrain computation failed, ${SPIKE_RETRY}`,
};

/**
 * A fault line as the spike shows it: the terrain's fault, of either cause, with the spike's remedy
 * ({@link SPIKE_TERRAIN_FAULTS}), and any other fault as it is.
 */
export function spikeFaultLine(fault: string): string {
  return isTerrainFault(fault) ? SPIKE_TERRAIN_FAULTS[fault] : fault;
}
