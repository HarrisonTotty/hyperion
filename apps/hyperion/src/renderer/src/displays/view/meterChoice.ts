/**
 * The meter `VIEW` meters with: the operator's choice, or the opening meter of a kept scene that
 * brings one (ux-calls-view, item 3b, the owner's delegated call, 2026-11-01).
 */
import type { MeterMode } from "../../view/post/meter";

/**
 * The operator's choices of meter: the one made in a scene that brings no meter of its own, and the
 * one made in the opening of a scene that does (the stage's key: the body inspection's name with
 * its sequence), or `null` before any such choice.
 *
 * @remarks
 * Kept apart, so that a meter chosen in the body inspection stands while it is open whatever is
 * chosen in a scene visited meanwhile, and the display returns to its own once it is left.
 */
export interface MeterChoice {
  readonly meter: MeterMode;
  readonly opened: { readonly opening: string; readonly meter: MeterMode } | null;
}

/** The display's choice as it mounts: `AVG`, and none in an opening. */
export const DEFAULT_METER_CHOICE: MeterChoice = { meter: "average", opened: null };

/**
 * The meter `VIEW` shows and meters with in `opening`: where the scene brings its own,
 * `startMeter`, the body inspection's `INCIDENT`, until the operator chooses one in that opening,
 * which then stands in it; else the operator's choice in the scenes that bring none.
 *
 * @param startMeter - The kept scene's opening meter (`KeptScene.startMeter`); absent for the
 *   server's scene and any other kept scene.
 */
export function shownMeter(
  choice: MeterChoice,
  opening: string,
  startMeter: MeterMode | undefined,
): MeterMode {
  if (startMeter === undefined) {
    return choice.meter;
  }
  return choice.opened?.opening === opening ? choice.opened.meter : startMeter;
}

/** `choice` with `meter` chosen in `opening`, whose scene brings `startMeter` or none. */
export function chooseMeter(
  choice: MeterChoice,
  meter: MeterMode,
  opening: string,
  startMeter: MeterMode | undefined,
): MeterChoice {
  return startMeter === undefined
    ? { ...choice, meter }
    : { ...choice, opened: { opening, meter } };
}
