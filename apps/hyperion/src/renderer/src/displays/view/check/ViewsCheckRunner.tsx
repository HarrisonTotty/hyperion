import type { RequestClient } from "@hyperion/protocol";
import { useEffect, useLayoutEffect, useRef } from "react";

import type {
  GraphicsLaunchMode,
  ServedWorldNames,
  ViewsCheckApi,
} from "../../../../../preload/api";
import type { SystemTarget } from "../../system/systemTarget";
import { pageAtmosphereFixtures } from "../../../view/scenes/atmosphereScene";
import {
  ATMOSPHERE_CHECK_TIMING,
  primaryPlate,
  recordAtmosphereCheck,
  runAtmosphereCheck,
} from "./atmosphereCheckRun";
import type { PlacedAtmosphere } from "./atmospherePlaces";
import {
  handoverCheckTimingFor,
  handoverLooksFor,
  recordHandoverCheck,
  runHandoverCheck,
} from "./handoverCheckRun";
import type { HandoverScenes } from "./handoverScenes";
import {
  INSPECTION_CHECK_TIMING,
  recordInspectionCheck,
  runInspectionCheck,
} from "./inspectionCheckRun";
import { INSPECTION_PROFILE_TIMING, InspectionProfiler } from "./inspectionProfile";
import {
  atmosphereCheckTimingOf,
  handoverCheckTimingOf,
  inspectionCheckTimingOf,
  inspectionProfileTimingOf,
  terrainCheckTimingOf,
  viewsCheckTimingOf,
} from "./softwareTiming";
import { recordTerrainCheck, runTerrainCheck, TERRAIN_CHECK_TIMING } from "./terrainCheckRun";
import type { TerrainProbe } from "./terrainProbe";
import { recordViewsCheck, runViewsCheck, STOPPED, VIEWS_CHECK_TIMING } from "./viewsCheckRun";
import type { ViewsProbe } from "./viewsProbe";

/**
 * A views-check launch's run (`ViewsCheckLaunch.run`) with what it runs on: T20's needs the views
 * probe alone; the terrain run (plan R10, R10.T16.d) its terrain probe, whose engine source,
 * workers and kept scenes `VIEW` takes; the hand-over check (plan R10, R10.T10.g) that probe and
 * its looks, which `VIEW` offers through the terrain probe's kept scenes; the atmosphere run (plan
 * R08, R08.T10.b) the views probe alone, as T20's, and in the software mode its places, whose kept
 * scenes `VIEW` offers (`atmospherePlaces.ts`); the inspection run (the owner's request,
 * 2026-11-01) the views probe and the link's request client, through which it asks what it opens.
 */
export type CheckedRun =
  | { readonly kind: "several-views" }
  | { readonly kind: "atmosphere"; readonly placed: PlacedAtmosphere | null }
  | {
      readonly kind: "inspection";
      readonly requests: RequestClient;
      /** The body the launch names, which the preload holds an inspection launch to. */
      readonly served: ServedWorldNames | null;
    }
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
  /**
   * The launch's graphics mode, `window.hyperion.graphics.launchMode`: the check-only software
   * mode's waits are five times as long (plan R01, R01.T16).
   */
  readonly launchMode: GraphicsLaunchMode;
  /**
   * Opens the `SYSTEM` display on a system, `App`'s, as `OPEN SYSTEM` calls it: the inspection run
   * opens its body's system through it (the owner's request).
   */
  readonly openSystem: (target: SystemTarget) => void;
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
 * run (plan R10, R10.T16.d), the hand-over check (plan R10, R10.T10.g), the atmosphere run (plan
 * R08, R08.T10.b) or the inspection run (the owner's request), and ends the app with it: the probe times the page's animation frames, the script drives `VIEW`, and the main process
 * writes the record and exits with 0, or with 1 and the reason when the script could not finish
 * (or a terrain check, the hand-over's verdict or an atmosphere capture failed).
 *
 * @remarks
 * Renders nothing. Mounted only on a `--views-check` launch, beside the consoles. In the check-only
 * software mode each run's waits are stretched (`softwareTiming.ts`), its measured windows not.
 */
export function ViewsCheckRunner({
  api,
  probe,
  run,
  launchMode,
  openSystem,
}: ViewsCheckRunnerProps) {
  // The run reads the latest opener through a ref, so that a new one does not start it again.
  const openSystemRef = useRef(openSystem);
  useLayoutEffect(() => {
    openSystemRef.current = openSystem;
  }, [openSystem]);
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
              primaryStatements: () => primaryPlate(document)?.statements ?? [],
              timing: viewsCheckTimingOf(
                api.launch.smoke ? VIEWS_CHECK_TIMING.smoke : VIEWS_CHECK_TIMING.full,
                launchMode,
              ),
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
              setting: api.launch.setting,
              placed: run.placed,
              // Through the probe's frame clock, which stamps VIEW's own callbacks the same.
              frame: () =>
                new Promise<number>((resolve) => {
                  window.requestAnimationFrame(resolve);
                }),
              timing: atmosphereCheckTimingOf(
                api.launch.smoke ? ATMOSPHERE_CHECK_TIMING.smoke : ATMOSPHERE_CHECK_TIMING.full,
                launchMode,
              ),
            }),
          controller.signal,
        );
        break;
      case "inspection": {
        const { served } = run;
        // The frame profile, on an `--inspection --timing` launch alone (the owner's report).
        const profile =
          api.launch.timing === true
            ? new InspectionProfiler({
                api,
                views: probe,
                markWindow: shared.markWindow,
                nowMs: shared.nowMs,
                sleep: shared.sleep,
                devicePixelRatio: shared.devicePixelRatio,
                setting: api.launch.setting,
                timing: inspectionProfileTimingOf(INSPECTION_PROFILE_TIMING, launchMode),
              })
            : null;
        ended = recordInspectionCheck(
          api,
          () =>
            runInspectionCheck({
              ...shared,
              api,
              views: probe,
              requests: run.requests,
              served,
              openSystem: (target) => {
                openSystemRef.current(target);
              },
              timing: inspectionCheckTimingOf(INSPECTION_CHECK_TIMING, launchMode),
              profile,
            }),
          controller.signal,
          profile,
        );
        break;
      }
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
              passTimes: api.launch.passTimes === true,
              timing: terrainCheckTimingOf(
                api.launch.smoke ? TERRAIN_CHECK_TIMING.smoke : TERRAIN_CHECK_TIMING.full,
                launchMode,
              ),
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
              looks: handoverLooksFor(api.launch.look),
              setting: api.launch.setting,
              timing: handoverCheckTimingOf(
                handoverCheckTimingFor(api.launch.served !== undefined),
                launchMode,
              ),
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
  }, [api, probe, run, launchMode]);
  return null;
}
