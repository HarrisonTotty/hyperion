/**
 * The faults a view's plate shows (plan R07, R07.T19; plan R10, R10.T16.b): its graphics fault,
 * then the terrain's, each its own fault line in `--status-caution` (decision-r10-t16-contracts §7).
 */

import type { ViewId } from "../../view/camera/state";
import type { GraphicsAnnunciation, GraphicsStatus } from "../../view/engine/status";
import { TERRAIN_NOT_AVAILABLE, type TerrainAnnunciation } from "../../view/terrain/annunciation";

/**
 * A graphics fault to show on a view's plate: the standing fault, unless it is another view's
 * refusal, which only that view's plate shows (decided 2026-10-02).
 */
export function faultFor(
  graphics: GraphicsStatus,
  annunciation: GraphicsAnnunciation | null,
  viewName: string,
): string | null {
  const otherView = graphics.fault?.kind === "view-refused" && graphics.fault.viewName !== viewName;
  return annunciation?.standing === "fault" && !otherView ? annunciation.text : null;
}

/**
 * The faults to show on a view's plate, each its own fault line, in order: its graphics fault
 * ({@link faultFor}), then the terrain's while the view hands a body over with every height worker
 * lost (`TerrainSession.terrainFaults`), so that a device loss never hides it.
 *
 * @param viewName - The view's name, as a `view-refused` fault carries it.
 * @param view - The view's ID, as the terrain session knows it.
 */
export function faultsFor(
  graphics: GraphicsStatus,
  annunciation: GraphicsAnnunciation | null,
  viewName: string,
  view: ViewId,
  terrainFaults: ReadonlySet<ViewId>,
): ReadonlyArray<string> {
  const graphicsFault = faultFor(graphics, annunciation, viewName);
  const faults = graphicsFault === null ? [] : [graphicsFault];
  return terrainFaults.has(view) ? [...faults, TERRAIN_NOT_AVAILABLE] : faults;
}

/**
 * A view's terrain annunciation for its label block's statements (`labelStatements`), or `null`
 * while its faults hold the terrain's (`TERRAIN_NOT_AVAILABLE`): the fault reaches the block at
 * once, and the annunciation at the readout rate, so without this the two could stand together
 * for up to a readout interval, `TERRAIN: STREAMING` beside a fault that says nothing is coming
 * (decision-r10-t16-contracts §7; the batch review's C2).
 */
export function terrainStatementOf(
  terrain: TerrainAnnunciation | null,
  faults: ReadonlyArray<string>,
): TerrainAnnunciation | null {
  return faults.includes(TERRAIN_NOT_AVAILABLE) ? null : terrain;
}
