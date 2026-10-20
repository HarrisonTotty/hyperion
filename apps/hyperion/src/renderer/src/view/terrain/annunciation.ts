/**
 * The two terrain annunciations of the view's label block (plan R05, T9, Design note 23).
 *
 * @remarks
 * `TERRAIN: STREAMING` holds while any patch the selection asks for is drawn by a coarser resident
 * ancestor, or not yet drawn at all; `TERRAIN: DETAIL LIMITED` while the drawn selection is coarser
 * than the reference selection, the same pure selection at τ = 1 px at the view's presented
 * resolution with no depth cap. Where both hold, `STREAMING` shows: it clears by itself, and the
 * operator answers it differently. Each is steady text with no status colour and no flashing; a
 * condition must hold for 250 ms before it shows and clear for 1 s before it goes, so that a patch
 * arriving mid-frame cannot make the line flicker. The wording is the guide's nomenclature row,
 * signed off by the owner (2026-10-02).
 */

import type { DrawSet } from "./cache";
import { parentKey, patchKeyString } from "./patchKey";
import type { Selection } from "./select";

/** A terrain annunciation's text, as the guide's nomenclature has it. */
export type TerrainAnnunciation = "TERRAIN: STREAMING" | "TERRAIN: DETAIL LIMITED";

/**
 * The terrain's fault for its first cause (plan R10, R10.T16.b; decision-r10-t16-contracts §7, the
 * string and its guide row ruled under the owner's delegation): every height worker is lost, so no
 * view draws terrain and each body is drawn by its figure, until `VIEW` is shown again and its
 * terrain made anew. Its second cause is {@link TERRAIN_COMPUTATION_FAILED}'s (R10.T16.e).
 *
 * @remarks
 * It is a fault, not a status: set on the label block's fault line in `--status-caution`, after any
 * graphics fault, on every view that hands a body over, and never among `labelStatements`' steady
 * statements, whose `TERRAIN: STREAMING` and `TERRAIN: DETAIL LIMITED` it stands in place of. It is
 * not debounced, since it cannot flicker: it stands until the session is made again.
 */
export const TERRAIN_NOT_AVAILABLE =
  "TERRAIN NOT AVAILABLE: height computation failed, reopen VIEW to retry";

/**
 * The terrain's fault for a computation that keeps failing (plan R10, R10.T16.e;
 * decision-r10-t16-leans §8, the string and its guide row ruled under the owner's delegation): the
 * terrain session's step, or a view's survey edges, failed for {@link ANNUNCIATION_ONSET_MS} on end,
 * so no view draws terrain and each body is drawn by its figure, until `VIEW` is shown again and
 * its terrain made anew.
 *
 * @remarks
 * The same fault as {@link TERRAIN_NOT_AVAILABLE}, for its second cause: in the same place and
 * colour, as sticky, and never debounced. A single failed frame raises nothing.
 */
export const TERRAIN_COMPUTATION_FAILED =
  "TERRAIN NOT AVAILABLE: terrain computation failed, reopen VIEW to retry";

/**
 * The terrain's fault, by its cause (plan R10, R10.T16.e; decision-r10-t16-leans §8): the height
 * workers lost ({@link TERRAIN_NOT_AVAILABLE}) or a computation that keeps failing
 * ({@link TERRAIN_COMPUTATION_FAILED}).
 */
export type TerrainFault = typeof TERRAIN_NOT_AVAILABLE | typeof TERRAIN_COMPUTATION_FAILED;

/**
 * Whether a fault line's `text` is the terrain's fault, of either cause (plan R10, R10.T16.e;
 * decision-r10-t16-leans §8).
 */
export function isTerrainFault(text: string): text is TerrainFault {
  return text === TERRAIN_NOT_AVAILABLE || text === TERRAIN_COMPUTATION_FAILED;
}

/** How long a condition must hold before its line shows, in milliseconds. */
export const ANNUNCIATION_ONSET_MS = 250;

/** How long a condition must be clear before its line goes, in milliseconds. */
export const ANNUNCIATION_CLEAR_MS = 1_000;

/** The two conditions of one frame, before the debounce. */
export interface TerrainConditions {
  /** A selected patch is drawn by a coarser ancestor, or not drawn at all. */
  readonly streaming: boolean;
  /** The selection is coarser somewhere than the reference selection. */
  readonly detailLimited: boolean;
}

/**
 * Whether `selection` is coarser than `reference` anywhere: some reference patch is covered in
 * `selection` by one of its ancestors.
 */
export function coarserThan(selection: Selection, reference: Selection): boolean {
  for (const [keyString, referenced] of reference.patches) {
    if (selection.patches.has(keyString)) {
      continue;
    }
    let ancestor = parentKey(referenced.key);
    while (ancestor !== null) {
      if (selection.patches.has(patchKeyString(ancestor))) {
        return true;
      }
      ancestor = parentKey(ancestor);
    }
  }
  return false;
}

/**
 * The two conditions of one frame, from the draw set, the selection and the reference: `DETAIL
 * LIMITED` where the selection is coarser than the reference, or the patch budget stopped it
 * (`Selection.limited`), so that a caller on the high setting may pass the selection as its own
 * reference rather than run an uncapped second selection.
 */
export function terrainConditions(
  draw: DrawSet,
  selection: Selection,
  reference: Selection,
): TerrainConditions {
  return {
    streaming: draw.standingIn > 0 || draw.missing > 0,
    // A selection the patch budget stopped is coarser than its own τ asks (the patch-demand ruling).
    detailLimited: selection.limited || coarserThan(selection, reference),
  };
}

/**
 * The line one frame's conditions call for, before the debounce: `STREAMING` over
 * `DETAIL LIMITED` where both hold (Design note 23).
 */
export function terrainAnnunciation(
  draw: DrawSet,
  selection: Selection,
  reference: Selection,
): TerrainAnnunciation | null {
  return lineFor(terrainConditions(draw, selection, reference));
}

function lineFor(shown: TerrainConditions): TerrainAnnunciation | null {
  if (shown.streaming) {
    return "TERRAIN: STREAMING";
  }
  return shown.detailLimited ? "TERRAIN: DETAIL LIMITED" : null;
}

interface Debounced {
  shown: boolean;
  /** When the condition last changed, in milliseconds. */
  sinceMs: number;
  /** The condition as last seen. */
  holds: boolean;
}

function step(d: Debounced, holds: boolean, nowMs: number): void {
  if (holds !== d.holds) {
    d.holds = holds;
    d.sinceMs = nowMs;
  }
  const heldMs = nowMs - d.sinceMs;
  if (!d.shown && d.holds && heldMs >= ANNUNCIATION_ONSET_MS) {
    d.shown = true;
  } else if (d.shown && !d.holds && heldMs >= ANNUNCIATION_CLEAR_MS) {
    d.shown = false;
  }
}

/**
 * The debounce of the terrain line: each condition shows after holding for
 * {@link ANNUNCIATION_ONSET_MS} and goes after being clear for {@link ANNUNCIATION_CLEAR_MS}.
 *
 * @remarks
 * One per view, fed every frame with the frame's conditions and a monotonic time.
 */
export class TerrainAnnunciationDebounce {
  private readonly streaming: Debounced = { shown: false, sinceMs: 0, holds: false };
  private readonly detailLimited: Debounced = { shown: false, sinceMs: 0, holds: false };

  /**
   * Takes one frame's conditions and returns the line to show.
   *
   * @param nowMs - A monotonic time in milliseconds, such as `performance.now()`.
   */
  update(conditions: TerrainConditions, nowMs: number): TerrainAnnunciation | null {
    step(this.streaming, conditions.streaming, nowMs);
    step(this.detailLimited, conditions.detailLimited, nowMs);
    return lineFor({ streaming: this.streaming.shown, detailLimited: this.detailLimited.shown });
  }
}
