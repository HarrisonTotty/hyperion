import { useEffect } from "react";

import type { ViewsCheckApi } from "../../../../../preload/api";
import { pageAtmosphereFixtures } from "../../../view/scenes/atmosphereScene";
import {
  ATMOSPHERE_CHECK_TIMING,
  recordAtmosphereCheck,
  runAtmosphereCheck,
} from "./atmosphereCheckRun";
import { HANDOVER_CHECK_TIMING, recordHandoverCheck, runHandoverCheck } from "./handoverCheckRun";
import type { HandoverScenes } from "./handoverScenes";
import { recordTerrainCheck, runTerrainCheck, TERRAIN_CHECK_TIMING } from "./terrainCheckRun";
import type { TerrainProbe } from "./terrainProbe";
import { recordViewsCheck, runViewsCheck, STOPPED, VIEWS_CHECK_TIMING } from "./viewsCheckRun";
import type { ViewsProbe } from "./viewsProbe";

/**
 * A views-check launch's run (`ViewsCheckLaunch.run`) with what it runs on: T20's needs the views
 * probe alone; the terrain run (plan R10, R10.T16.d) its terrain probe, whose engine source,
 * workers and kept scenes `VIEW` takes; the hand-over check (plan R10, R10.T10.g) that probe and
 * its looks, which `VIEW` offers through the terrain probe's kept scenes; the atmosphere run (plan
 * R08, R08.T10.b) the views probe alone, as T20's.
 */
export type CheckedRun =
  | { readonly kind: "several-views" }
  | { readonly kind: "atmosphere" }
  | { readonly kind: "terrain"; readonly terrain: TerrainProbe }
  | {
      readonly kind: "handover";
      readonly terrain: TerrainProbe;
      readonly scenes: HandoverScenes;
    };

/** Props of {@link ViewsCheckRunner}. */
export interface ViewsCheckRunnerProps {
  /** The check's functions, `window.hyperion.viewsCheck`. */
  readonly api: ViewsCheckApi;
  /** The probe the `VIEW` display's engine source belongs to. */
  readonly probe: ViewsProbe;
  /** The launch's run, with its own probes. */
  readonly run: CheckedRun;
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
 * Runs the launch's run once the app mounts: the several-views check (plan R07, T20), the terrain
 * run (plan R10, R10.T16.d), the hand-over check (plan R10, R10.T10.g) or the atmosphere run (plan
 * R08, R08.T10.b), and ends the app with it: the probe times the page's animation frames, the script drives `VIEW`, and the main process
 * writes the record and exits with 0, or with 1 and the reason when the script could not finish
 * (or a terrain check, the hand-over's verdict or an atmosphere capture failed).
 *
 * @remarks
 * Renders nothing. Mounted only on a `--views-check` launch, beside the consoles.
 */
export function ViewsCheckRunner({ api, probe, run }: ViewsCheckRunnerProps) {
  useEffect(() => {
    const controller = new AbortController();
    const restoreFrames = probe.installFrameClock(window);
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
    let stopWatching: (() => void) | null = null;
    let ended: Promise<void>;
    switch (run.kind) {
      case "several-views":
        ended = recordViewsCheck(
          api,
          () =>
            runViewsCheck({
              ...shared,
              api,
              probe,
              timing: api.launch.smoke ? VIEWS_CHECK_TIMING.smoke : VIEWS_CHECK_TIMING.full,
            }),
          controller.signal,
        );
        break;
      case "atmosphere":
        ended = recordAtmosphereCheck(
          api,
          () =>
            runAtmosphereCheck({
              ...shared,
              api,
              views: probe,
              fixtures: pageAtmosphereFixtures(),
              // Through the probe's frame clock, which stamps VIEW's own callbacks the same.
              frame: () =>
                new Promise<number>((resolve) => {
                  window.requestAnimationFrame(resolve);
                }),
              timing: api.launch.smoke
                ? ATMOSPHERE_CHECK_TIMING.smoke
                : ATMOSPHERE_CHECK_TIMING.full,
            }),
          controller.signal,
        );
        break;
      case "terrain": {
        const { terrain } = run;
        stopWatching = terrain.start();
        ended = recordTerrainCheck(
          api,
          () =>
            runTerrainCheck({
              ...shared,
              api,
              views: probe,
              terrain,
              setting: api.launch.setting,
              fixture: api.launch.fixture,
              timing: api.launch.smoke ? TERRAIN_CHECK_TIMING.smoke : TERRAIN_CHECK_TIMING.full,
            }),
          controller.signal,
        );
        break;
      }
      case "handover": {
        const { terrain, scenes } = run;
        stopWatching = terrain.start();
        ended = recordHandoverCheck(
          api,
          () =>
            runHandoverCheck({
              ...shared,
              api,
              views: probe,
              terrain,
              scenes,
              setting: api.launch.setting,
              timing: HANDOVER_CHECK_TIMING,
            }),
          controller.signal,
        );
        break;
      }
    }
    ended.catch((error: unknown) => {
      console.error("views check: the run could not be ended:", error);
    });
    return () => {
      controller.abort();
      restoreFrames();
      stopWatching?.();
    };
  }, [api, probe, run]);
  return null;
}
