/**
 * The views checks' timings in the check-only software mode (plan R01, R01.T16): each wait that
 * bounds how long a control, a style's pipelines, the media, `AUTO` or a restore may take is five
 * times as long, and the measured windows, settles, holds and script times are as they are.
 *
 * @remarks
 * SwiftShader made the photorealistic style's pipelines in 52 s in a pod, and its media were ready
 * at 64.7 s at the high setting, against the checks' 60 s waits; single frames took up to 22.7 s
 * (the research lane's prototype, AMD EPYC 7571, 4 vCPU, 2026-10-29). Every other launch mode keeps
 * its timing object as it is.
 */

import type { GraphicsLaunchMode } from "../../../../../preload/api";
import type { AtmosphereCheckTiming } from "./atmosphereCheckRun";
import type { HandoverCheckTiming } from "./handoverCheckRun";
import type { InspectionCheckTiming } from "./inspectionCheckRun";
import type { TerrainCheckTiming } from "./terrainCheckRun";
import type { ViewsCheckTiming } from "./viewsCheckRun";

/**
 * How many times longer a software launch's waits are: fixed, not a variable (main's ruling,
 * 2026-10-29). The main process's watchdog and `viewsCheck.sh`'s limit stretch by the same five.
 */
export const SOFTWARE_WAIT_FACTOR = 5;

/** A wait, ms, stretched for the software mode. */
function stretched(ms: number): number {
  return ms * SOFTWARE_WAIT_FACTOR;
}

/** T20's timing for a launch: its control wait stretched in the software mode. */
export function viewsCheckTimingOf(
  timing: ViewsCheckTiming,
  launchMode: GraphicsLaunchMode,
): ViewsCheckTiming {
  return launchMode === "software" ? { ...timing, waitMs: stretched(timing.waitMs) } : timing;
}

/**
 * The atmosphere run's timing for a launch: its wait for a control and the media, and `AUTO`'s
 * settle limit, stretched in the software mode.
 */
export function atmosphereCheckTimingOf(
  timing: AtmosphereCheckTiming,
  launchMode: GraphicsLaunchMode,
): AtmosphereCheckTiming {
  return launchMode === "software"
    ? {
        ...timing,
        waitMs: stretched(timing.waitMs),
        mediaWaitMs: stretched(timing.mediaWaitMs),
        settleMaxMs: stretched(timing.settleMaxMs),
      }
    : timing;
}

/**
 * The terrain run's timing for a launch: its waits for a control and for the restore stretched in
 * the software mode.
 */
export function terrainCheckTimingOf(
  timing: TerrainCheckTiming,
  launchMode: GraphicsLaunchMode,
): TerrainCheckTiming {
  return launchMode === "software"
    ? {
        ...timing,
        waitMs: stretched(timing.waitMs),
        restoreMs: stretched(timing.restoreMs),
      }
    : timing;
}

/**
 * The hand-over check's timing for a launch: its waits, the survey's among them, and a look's
 * overrun stretched in the software mode.
 */
export function handoverCheckTimingOf(
  timing: HandoverCheckTiming,
  launchMode: GraphicsLaunchMode,
): HandoverCheckTiming {
  return launchMode === "software"
    ? {
        ...timing,
        waitMs: stretched(timing.waitMs),
        surveyWaitMs: stretched(timing.surveyWaitMs),
        overrunMs: stretched(timing.overrunMs),
      }
    : timing;
}

/**
 * The inspection run's timing for a launch (the owner's request, 2026-11-01): its waits for a
 * control, for the first photorealistic frames and for its surveyed ground, and `AUTO`'s settle
 * limit, stretched in the software mode.
 */
export function inspectionCheckTimingOf(
  timing: InspectionCheckTiming,
  launchMode: GraphicsLaunchMode,
): InspectionCheckTiming {
  return launchMode === "software"
    ? {
        ...timing,
        waitMs: stretched(timing.waitMs),
        drawWaitMs: stretched(timing.drawWaitMs),
        settleMaxMs: stretched(timing.settleMaxMs),
        groundWaitMs: stretched(timing.groundWaitMs),
      }
    : timing;
}
