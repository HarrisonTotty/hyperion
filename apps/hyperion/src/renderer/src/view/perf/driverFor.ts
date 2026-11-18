/**
 * The perf run's driver of the configured scene (plan R12, R12.T4.c), or why the renderer has
 * none: the descents and `arrival-memory` on R05's spike (`descentDriver.ts`), and
 * `ringed-giant-approach` and `nuclear-disc-sky` on `VIEW`'s free scripted camera
 * (`viewDriver.ts`); the graphics safe mode and a renderer without WebGPU draw no view.
 */

import type { PerfConfig } from "../../../../preload/api";
import { DescentDriver, type DescentStages, type PerfEngine } from "./descentDriver";
import type { PerfDriverChoice } from "./runPerfScene";
import { PERF_SCENES } from "./scenes";
import { ViewDriver, type ViewStages } from "./viewDriver";

/** What {@link perfDriverFor} needs of the page. */
export interface PerfDriverPage {
  readonly engine: PerfEngine;
  /** The stage the spike's scenes are shown on. */
  readonly stages: DescentStages;
  /** The stage `VIEW`'s scenes are shown on. */
  readonly views: ViewStages;
  /** `window.hyperion.graphics.launchMode`. */
  readonly launchMode: PerfConfig["launchMode"];
  /** Whether the renderer has WebGPU (`navigator.gpu`). */
  readonly webGpu: boolean;
  /** The height workers a scene that names none runs: the machine's default. */
  readonly defaultWorkers: () => number;
}

/** The driver of the configured scene, or why the renderer has none. */
export function perfDriverFor(config: PerfConfig, page: PerfDriverPage): PerfDriverChoice {
  const scene = PERF_SCENES[config.launch.scene];
  if (page.launchMode === "safe") {
    return {
      kind: "refused",
      reason: "the graphics safe mode draws no WebGPU view",
      shown: "the graphics safe mode draws no view, relaunch to retry",
    };
  }
  if (!page.webGpu) {
    return { kind: "refused", reason: "this renderer has no WebGPU", shown: "no WebGPU" };
  }
  const setting = config.launch.setting;
  let choice: PerfDriverChoice;
  switch (scene.script.kind) {
    case "descent":
    case "arrival":
      choice = {
        kind: "driver",
        driver: new DescentDriver({
          scene,
          setting,
          workers: scene.harness.heightWorkers ?? page.defaultWorkers(),
          engine: page.engine,
          stages: page.stages,
        }),
      };
      break;
    case "approach":
    case "sweep":
      choice = {
        kind: "driver",
        driver: new ViewDriver({ scene, setting, engine: page.engine, stages: page.views }),
      };
      break;
  }
  return choice;
}
