import { useEffect } from "react";

import type { ViewsCheckApi } from "../../../../../preload/api";
import { recordTerrainCheck, runTerrainCheck, TERRAIN_CHECK_TIMING } from "./terrainCheckRun";
import type { TerrainProbe } from "./terrainProbe";
import { recordViewsCheck, runViewsCheck, STOPPED, VIEWS_CHECK_TIMING } from "./viewsCheckRun";
import type { ViewsProbe } from "./viewsProbe";

/** Props of {@link ViewsCheckRunner}. */
export interface ViewsCheckRunnerProps {
  /** The check's functions, `window.hyperion.viewsCheck`. */
  readonly api: ViewsCheckApi;
  /** The probe the `VIEW` display's engine source belongs to. */
  readonly probe: ViewsProbe;
  /**
   * The terrain run's probe on a `--terrain` launch (plan R10, R10.T16.d), whose engine source,
   * workers and kept scenes `VIEW` takes; `null` on T20's.
   */
  readonly terrain: TerrainProbe | null;
}

/** The name of each phase's measured window in the trace's user timing. */
const WINDOW_MEASURE = "hyperion views check window";

/** A wait on the page's own timers, ended early, as a rejection, by `signal`. */
function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    const stop = (): void => {
      clearTimeout(timer);
      reject(new Error(STOPPED));
    };
    const timer = setTimeout(() => {
      signal.removeEventListener("abort", stop);
      resolve();
    }, ms);
    signal.addEventListener("abort", stop, { once: true });
  });
}

/**
 * Runs the several-views check once the app mounts (plan R07, T20), or on a `--terrain` launch
 * the terrain run (plan R10, R10.T16.d), and ends the app with it: the probe times the page's
 * animation frames, the script drives `VIEW`, and the main process writes the record and exits
 * with 0, or with 1 and the reason when the script could not finish (or a terrain check failed).
 *
 * @remarks
 * Renders nothing. Mounted only on a `--views-check` launch, beside the consoles.
 */
export function ViewsCheckRunner({ api, probe, terrain }: ViewsCheckRunnerProps) {
  useEffect(() => {
    const controller = new AbortController();
    const restoreFrames = probe.installFrameClock(window);
    const stopWatching = terrain?.start() ?? null;
    const shared = {
      document,
      nowMs: () => performance.now(),
      markWindow: ({ startMs, endMs }: { readonly startMs: number; readonly endMs: number }) => {
        performance.measure(WINDOW_MEASURE, { start: startMs, end: endMs });
      },
      sleep: (ms: number) => sleep(ms, controller.signal),
      signal: controller.signal,
      devicePixelRatio: window.devicePixelRatio,
    };
    const ended =
      terrain === null
        ? recordViewsCheck(
            api,
            () =>
              runViewsCheck({
                ...shared,
                api,
                probe,
                timing: api.launch.smoke ? VIEWS_CHECK_TIMING.smoke : VIEWS_CHECK_TIMING.full,
              }),
            controller.signal,
          )
        : recordTerrainCheck(
            api,
            () =>
              runTerrainCheck({
                ...shared,
                api,
                views: probe,
                terrain,
                setting: api.launch.setting,
                timing: api.launch.smoke ? TERRAIN_CHECK_TIMING.smoke : TERRAIN_CHECK_TIMING.full,
              }),
            controller.signal,
          );
    ended.catch((error: unknown) => {
      console.error("views check: the run could not be ended:", error);
    });
    return () => {
      controller.abort();
      restoreFrames();
      stopWatching?.();
    };
  }, [api, probe, terrain]);
  return null;
}
