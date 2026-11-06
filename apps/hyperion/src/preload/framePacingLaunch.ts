/**
 * How the main process hands the launch's frame pacing (`--frame-pacing`, plan R07, T19; the
 * owner's frame rate) to the preload.
 *
 * @remarks
 * As `qualityLaunch.ts` does for the quality setting: the pacing rides in the renderer's own `argv`
 * as an extra switch, `--hyperion-frame-pacing=<rate|display>`, on every launch, and the preload
 * gives it to `HyperionApi.framePacing`. Both sides of the hand-off use this module; it imports
 * types only, so it is safe in either bundle.
 */

import type { FramePacingName } from "./api";

/**
 * The pacing a launch without `--frame-pacing` takes: `display`, today's frame counting, until the
 * owner rules (research-frame-pacing-120hz), since `rate` halves the low setting's rate on a 120 Hz
 * panel (60 to 30 Hz). The one constant to flip.
 */
export const DEFAULT_FRAME_PACING: FramePacingName = "display";

/** The switch carrying the pacing. */
const FRAME_PACING_SWITCH = "--hyperion-frame-pacing=";

/** The extra renderer argument that carries `pacing`, for `webPreferences.additionalArguments`. */
export function framePacingSwitch(pacing: FramePacingName): string {
  return `${FRAME_PACING_SWITCH}${pacing}`;
}

/**
 * The frame pacing carried by `argv`, the renderer's arguments.
 *
 * @throws Error if no argument carries a pacing, which means the window was created without
 *   {@link framePacingSwitch}.
 */
export function framePacingFromArgv(argv: readonly string[]): FramePacingName {
  const carrier = argv.find((argument) => argument.startsWith(FRAME_PACING_SWITCH));
  const value = carrier?.slice(FRAME_PACING_SWITCH.length);
  if (value !== "rate" && value !== "display") {
    throw new Error(`the renderer was given no ${FRAME_PACING_SWITCH}<rate|display>`);
  }
  return value;
}
