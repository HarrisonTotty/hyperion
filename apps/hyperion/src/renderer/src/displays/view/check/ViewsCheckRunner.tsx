import { useEffect } from "react";

import type { ViewsCheckApi } from "../../../../../preload/api";
import { recordViewsCheck, runViewsCheck, STOPPED, VIEWS_CHECK_TIMING } from "./viewsCheckRun";
import type { ViewsProbe } from "./viewsProbe";

/** Props of {@link ViewsCheckRunner}. */
export interface ViewsCheckRunnerProps {
  /** The check's functions, `window.hyperion.viewsCheck`. */
  readonly api: ViewsCheckApi;
  /** The probe the `VIEW` display's engine source belongs to. */
  readonly probe: ViewsProbe;
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
 * Runs the several-views check once the app mounts (plan R07, T20), and ends the app with it: the
 * probe times the page's animation frames, the script drives `VIEW`, and the main process writes
 * the record and exits with 0, or with 1 and the reason when the script could not finish.
 *
 * @remarks
 * Renders nothing. Mounted only on a `--views-check` launch, beside the consoles.
 */
export function ViewsCheckRunner({ api, probe }: ViewsCheckRunnerProps) {
  useEffect(() => {
    const controller = new AbortController();
    const restoreFrames = probe.installFrameClock(window);
    void recordViewsCheck(
      api,
      () =>
        runViewsCheck({
          api,
          probe,
          document,
          timing: api.launch.smoke ? VIEWS_CHECK_TIMING.smoke : VIEWS_CHECK_TIMING.full,
          nowMs: () => performance.now(),
          markWindow: ({ startMs, endMs }) => {
            performance.measure(WINDOW_MEASURE, { start: startMs, end: endMs });
          },
          sleep: (ms) => sleep(ms, controller.signal),
          signal: controller.signal,
          devicePixelRatio: window.devicePixelRatio,
        }),
      controller.signal,
    ).catch((error: unknown) => {
      console.error("views check: the run could not be ended:", error);
    });
    return () => {
      controller.abort();
      restoreFrames();
    };
  }, [api, probe]);
  return null;
}
