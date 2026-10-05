/**
 * The `VIEW` display's views by name (plan R07, T19; decision-r07-t19 item 2c): the `PRIMARY`
 * view, which fills the stage, and the instrument views `INSTRUMENT 1` and `INSTRUMENT 2` in their
 * slots over its right edge.
 *
 * @remarks
 * The engine's names for the views stay internal and distinct (`view`, `instrument-1`,
 * `instrument-2`), so that a `view-refused` fault reaches only its own view; each is also the
 * view's identity in the scene's camera reports and the budgets. {@link viewDisplayName} is the one
 * map from an identity to what the operator reads.
 */
import { type ViewId, viewId } from "../../view/camera/state";

/** The primary view's engine name: R02's single view's, kept. */
export const PRIMARY_VIEW_NAME = "view";

/** The primary view's identity in the camera reports and the budgets. */
export const PRIMARY_VIEW_ID: ViewId = viewId(PRIMARY_VIEW_NAME);

/** The primary view's name, as its canvas, the `CONTROLS` selector and `SOURCE` read it. */
export const PRIMARY_NAME = "PRIMARY";

/** An instrument slot: the first at the top of the primary's right edge, the second below it. */
export type InstrumentSlot = 1 | 2;

/** The slots, in their order. */
export const INSTRUMENT_SLOTS: ReadonlyArray<InstrumentSlot> = [1, 2];

/** A slot's name, as its panel, its canvas and the `CONTROLS` selector give it: `INSTRUMENT 1`. */
export function instrumentName(slot: InstrumentSlot): string {
  return `INSTRUMENT ${String(slot)}`;
}

/** A slot's view identity, and its engine view's name: `instrument-1`. */
export function instrumentViewId(slot: InstrumentSlot): ViewId {
  return viewId(`instrument-${String(slot)}`);
}

/** A view's name for the operator: `PRIMARY`, `INSTRUMENT 1` or `INSTRUMENT 2`. */
export function viewDisplayName(id: ViewId): string {
  if (id === PRIMARY_VIEW_ID) {
    return PRIMARY_NAME;
  }
  const slot = INSTRUMENT_SLOTS.find((each) => instrumentViewId(each) === id);
  // Another view (a test's) by its identity, upper-cased as the meter showed it before T19.
  return slot === undefined ? id.toUpperCase() : instrumentName(slot);
}
