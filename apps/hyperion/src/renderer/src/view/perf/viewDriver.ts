/**
 * The perf run's driver of the scenes `VIEW` draws on its free scripted camera (plan R12, R12.T4.c
 * and R12.T5.a; Design notes 8 and 13): `ringed-giant-approach` and `nuclear-disc-sky`, each
 * `VIEW`'s display over the served scene the arrival placed the ship in, its primary set at each
 * frame at the scene's `scripts.ts` pose (`displays/view/viewPerf.ts`).
 *
 * @remarks
 * Each mount is a stage `PerfApp` renders under its own `key` (`PerfViewStage`): a new link to the
 * stage's server, the arrival's universe opened, and a new `VIEW` with its terrain session, its
 * atmosphere cache and its sky, so that a repetition's patch cache, height textures, height
 * workers' queues, tables and sky are new and asked of the restarted server again (Design note 13's
 * resets). The engine is the run's measured one (`PerfEngine`), kept across the mounts. The primary
 * draws photorealistic at 60°, at the window's size less the header strip, `VIEW`'s scene bar and
 * key legend, with `MAXIMIZE VIEW` on, its label block whole, and no instrument open, at its own
 * budget's rate under R07's pacing, which counts the display's frames.
 *
 * The script stands in the scene once the server's scene of the pinned system is drawn: the sweep
 * about its barycentre, the approach about the giant in its widest ring's frame
 * (`scriptedCamera.ts`, `ringedGiantIn`), placed once a mount. The warm-up holds the script's first
 * pose; a repetition holds it too until its first frame drawn in the scene's style, then flies it on
 * the frames' `requestAnimationFrame` clock. The wireframe stand-in's frames, drawn while the
 * photorealistic pipelines compile, are left out of the run; pipelines that cannot be made end it.
 * Each frame drawn at the script is told to the run, with the run's resolve count by then.
 */

import type { InternalScaleRecord } from "@hyperion/perf";
import type { BodyIdHex } from "@hyperion/protocol";

import type { PerfDescribedView } from "../../../../preload/api";
import type { ScriptedCamera, ViewPerfFrame, ViewPerfHooks } from "../../displays/view/viewPerf";
import { DEFAULT_FOV_DEG } from "../camera/projection";
import type { QualitySetting } from "../quality/qualitySetting";
import type { TerrainSession } from "../terrain/session";
import type { TerrainPass } from "../terrain/terrainPass";
import type { WorkerMemory } from "../terrain/workers/pool";
import { capabilityFlags, type PerfEngine } from "./descentDriver";
import type { MemoryLedger } from "./ledger";
import type {
  PerfDrawn,
  PerfFailure,
  PerfSceneDriver,
  PerfSceneSink,
  PerfStage,
} from "./runPerfScene";
import type { PerfScene } from "./scenes";
import {
  barycentrePlacement,
  placedPose,
  ringedGiantIn,
  type ScriptPlacement,
} from "./scriptedCamera";
import { nuclearDiscSweep, type PerfScript, ringedGiantApproach } from "./scripts";
import { PerfStages } from "./stages";

/**
 * What the driver reads of the terrain session `VIEW` makes (`TerrainSession`'s): its passes'
 * patch caches and its height workers.
 */
export interface ViewDrawnSession extends Pick<
  TerrainSession,
  "workerMemory" | "workerCount" | "passBodies" | "wirePassBodies"
> {
  pass(body: BodyIdHex): Pick<TerrainPass, "usedSlots"> | null;
  wirePass(body: BodyIdHex): Pick<TerrainPass, "usedSlots"> | null;
}

/** What `PerfApp` renders for a mount: `PerfViewStage`'s props, under its own `key`. */
export interface ViewStage {
  readonly key: string;
  readonly serverUrl: string;
  /** The arrival's universe, which the stage opens once its link is up. */
  readonly universe: string;
  readonly setting: QualitySetting;
  /** The hooks on `VIEW`'s loop: the scripted camera and its frames. */
  readonly perf: ViewPerfHooks;
  /** Watches the terrain session `VIEW` makes (`ViewDisplay`'s `watchTerrain`). */
  readonly watchTerrain: (session: ViewDrawnSession) => () => void;
  /** The stage cannot go on (its link or its universe refused), with why. */
  readonly onFailed: (failure: PerfFailure) => void;
}

/** The title of the frame over `VIEW`'s scenes, mounted or between mounts. */
export const VIEW_TITLE = "VIEW";

/** The scenes `VIEW` draws, as `PerfApp` renders them. */
export class ViewStages extends PerfStages<ViewStage> {}

/** What a {@link ViewDriver} is given. */
export interface ViewDriverOptions {
  readonly scene: PerfScene;
  readonly setting: QualitySetting;
  readonly engine: PerfEngine;
  readonly stages: ViewStages;
}

/** The value at fraction `p` of ascending `sorted`, not empty, by nearest rank. */
function nearestRank(sorted: ReadonlyArray<number>, p: number): number {
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))] ?? 1;
}

/** A scene's script once it stands in the served scene, or why it cannot. */
type Placed =
  | { readonly kind: "placed"; readonly script: PerfScript; readonly placement: ScriptPlacement }
  | { readonly kind: "gap"; readonly reason: string; readonly shown: string };

/** A scene the catalogue does not fly on `VIEW`'s camera, as the run's fault line states it. */
const NOT_VIEW_SCENE = "this scene is not flown on VIEW's camera";

/**
 * `scene`'s script placed in the served scene of `run`: the sweep about its system's barycentre, the
 * approach about its giant's widest ring.
 */
export function placeScript(
  scene: PerfScene,
  run: Parameters<ViewPerfHooks["cameraAt"]>[0],
): Placed {
  let placed: Placed;
  switch (scene.script.kind) {
    case "sweep":
      placed = {
        kind: "placed",
        script: nuclearDiscSweep(),
        placement: barycentrePlacement(run.scene.system),
      };
      break;
    case "approach": {
      const found =
        scene.body === null
          ? { kind: "gap" as const, reason: `${scene.id} names no giant`, shown: NOT_VIEW_SCENE }
          : ringedGiantIn(run.scene, scene.body);
      placed =
        found.kind === "gap"
          ? found
          : {
              kind: "placed",
              script: ringedGiantApproach(found.giant),
              placement: found.placement,
            };
      break;
    }
    case "descent":
    case "arrival":
      placed = {
        kind: "gap",
        reason: `${scene.id} flies the descent spike, not VIEW's camera`,
        shown: NOT_VIEW_SCENE,
      };
      break;
  }
  return placed;
}

/** The driver of a scene `VIEW` draws on its free scripted camera, remounted per repetition. */
export class ViewDriver implements PerfSceneDriver {
  readonly #options: ViewDriverOptions;
  #stage: ViewStage | null = null;
  #session: ViewDrawnSession | null = null;
  /** The render scales of the repetition's frames within its span, for its internal scale. */
  #scales: number[] = [];

  constructor(options: ViewDriverOptions) {
    this.#options = options;
  }

  mount(stage: PerfStage, sink: PerfSceneSink): Promise<PerfDrawn> {
    const { engine, scene, setting, stages } = this.#options;
    engine.sink = sink;
    const main = scene.views[setting][0];
    if (main === undefined) {
      const reason = `${scene.id}: the scene has no view at ${setting}`;
      sink.failed({ reason, shown: "the scene has no view at this quality setting" });
      return Promise.reject(new Error(reason));
    }
    this.#scales = [];
    return new Promise<PerfDrawn>((resolve, reject) => {
      const current = (): boolean => this.#stage === made;
      const fail = (failure: PerfFailure): void => {
        if (current()) {
          sink.failed(failure);
          reject(new Error(failure.reason));
        }
      };
      let placed: Placed | null = null;
      let firstMs: number | null = null;
      let tS = 0;
      let drawn = false;
      const cameraAt = (
        run: Parameters<ViewPerfHooks["cameraAt"]>[0],
        nowMs: number,
      ): ScriptedCamera | null => {
        if (!current()) {
          return null;
        }
        if (run.scene.system !== scene.system) {
          fail({
            reason: `${scene.id}: the served scene is of ${run.scene.system}, not the pinned ${scene.system}`,
            shown: "the served scene is of another system than the scene's pin, re-pin the scene",
          });
          return null;
        }
        placed ??= placeScript(scene, run);
        if (placed.kind === "gap") {
          fail({ reason: `${scene.id}: ${placed.reason}`, shown: placed.shown });
          return null;
        }
        // The warm-up holds the first pose, and so does a repetition until its first frame drawn in
        // the scene's style.
        tS = stage.phase === "warm-up" || firstMs === null ? 0 : (nowMs - firstMs) / 1000;
        return {
          pose: placedPose(placed.script.poseAt(tS), placed.placement),
          style: main.style,
          fovDeg: DEFAULT_FOV_DEG,
        };
      };
      const onDrawn = (frame: ViewPerfFrame): void => {
        if (!current() || placed?.kind !== "placed") {
          return;
        }
        if (frame.photoreal === "failed" && main.style === "photorealistic") {
          fail({
            reason: `${scene.id}: the photorealistic style's pipelines could not be made`,
            shown: "the photorealistic style's pipelines could not be made on this device",
          });
          return;
        }
        // A frame of the wireframe stand-in, drawn while the pipelines compile, is not the scene's:
        // it is left out, and the span's clock starts at the first frame in the scene's style.
        if (frame.drawnStyle !== main.style) {
          return;
        }
        if (stage.phase === "repetition") {
          firstMs ??= frame.rafTimestampMs;
        }
        engine.scriptTimeS = tS;
        if (tS <= placed.script.durationS) {
          this.#scales.push(frame.renderScale);
        }
        sink.frame({
          engineFrame: engine.gpu.resolves.value,
          scriptTimeS: tS,
          rafTimestampMs: frame.rafTimestampMs,
          callbackStartMs: frame.callbackStartMs,
          callbackMs: frame.callbackMs,
          streaming: frame.streaming,
        });
        if (!drawn) {
          drawn = true;
          resolve(this.#drawnOf(frame, placed.script, main));
        }
      };
      const made: ViewStage = {
        key: stage.phase === "warm-up" ? "warm-up" : `repetition-${String(stage.n)}`,
        serverUrl: stage.serverUrl,
        universe: stage.arrival.universe,
        setting,
        perf: { cameraAt, onDrawn },
        watchTerrain: (session) => {
          if (current()) {
            this.#session = session;
          }
          return () => {
            if (this.#session === session) {
              this.#session = null;
            }
          };
        },
        onFailed: fail,
      };
      this.#stage = made;
      this.#session = null;
      stages.set(made);
    });
  }

  unmount(): void {
    this.#stage = null;
    this.#session = null;
    this.#options.stages.set(null);
  }

  cachedPatches(): number {
    const session = this.#session;
    if (session === null) {
      return 0;
    }
    let patches = 0;
    for (const body of session.passBodies) {
      patches += session.pass(body)?.usedSlots ?? 0;
    }
    for (const body of session.wirePassBodies) {
      patches += session.wirePass(body)?.usedSlots ?? 0;
    }
    return patches;
  }

  workerMemory(): ReadonlyArray<WorkerMemory> {
    return this.#session?.workerMemory ?? [];
  }

  memory(): MemoryLedger | null {
    return this.#options.engine.ledger;
  }

  latePipelines(): number {
    return this.#options.engine.gpu.tally.late().length;
  }

  endWarmUp(): void {
    this.#options.engine.gpu.tally.endWarmup();
  }

  resolveCount(): number {
    return this.#options.engine.gpu.resolves.value;
  }

  /** The primary's render scale over the span's frames: its least and its median. */
  internalScale(views: ReadonlyArray<PerfDescribedView>): ReadonlyArray<InternalScaleRecord> {
    const sorted = this.#scales.toSorted((a, b) => a - b);
    const scale: InternalScaleRecord =
      sorted.length === 0
        ? { min: 1, median: 1 }
        : { min: sorted[0] ?? 1, median: nearestRank(sorted, 0.5) };
    return views.map(() => scale);
  }

  /** The views as drawn, and the script the camera flies. */
  #drawnOf(
    frame: ViewPerfFrame,
    script: PerfScript,
    main: PerfScene["views"][QualitySetting][number],
  ): PerfDrawn {
    const { scene, engine } = this.#options;
    const capabilities = engine.capabilities;
    return {
      views: [
        {
          role: main.role,
          style: main.style,
          widthPx: frame.size.widthPx,
          heightPx: frame.size.heightPx,
          rateHz: main.rateHz,
          lineAntialiasing: main.lineAntialiasing,
          exposure: null,
        },
      ],
      adapter: engine.adapter,
      capabilities: capabilities === null ? null : capabilityFlags(capabilities),
      kernelPaths: null,
      heightWorkers: this.#session?.workerCount ?? 0,
      span: {
        kind: "script",
        spanS: script.durationS,
        segments: script.segments.map(({ name, startS, endS }) => ({ name, startS, endS })),
        demandPerS: () => null,
      },
      notes: [
        `The renderer drew ${scene.id} with VIEW's display over the served scene, its primary (${main.style}) on its free scripted camera at ${String(DEFAULT_FOV_DEG)} degrees with MAXIMIZE VIEW on, its label block whole, and no instrument open; its wireframe stand-in's frames are left out; kernel paths and the exposure are not read and no patch demand is predicted (R12.T4.c).`,
        ...(scene.script.kind === "approach"
          ? [
              "The approach's script stands in the giant's widest ring's frame, z its normal and x toward the host star in its plane (lean, R12.T4.c). The photorealistic style draws no ring pass yet (R11's rings slot is not built).",
            ]
          : []),
      ],
    };
  }
}
