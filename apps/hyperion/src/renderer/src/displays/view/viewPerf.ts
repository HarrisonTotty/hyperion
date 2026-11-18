/**
 * A perf run's hooks on `VIEW`'s drawing loop (plan R12, R12.T4.c; Design notes 8 and 13): the
 * free scripted camera that `ringed-giant-approach` and `nuclear-disc-sky` fly, set on the primary
 * at each frame of the server's scene, and what each of its frames drew, told to the run.
 *
 * @remarks
 * Only a `--perf-run` launch gives them (`view/perf/viewDriver.ts`); without them `VIEW` is as it
 * always is. The camera, which its `CAMERA` line and its canvas's name call `SCRIPTED`, is the
 * primary's free camera at rest at the script's pose and field of view
 * (`scriptedFreeCamera`), set after the run's step and before the frame is drawn, so that the
 * frame, its terrain session's report, the camera reported to the scene and the published run (from
 * which the sky is asked, and asked again as the camera moves) all stand at the script's pose.
 */

import type { CameraPose } from "../../view/camera/pose";
import { scriptedFreeCamera } from "../../view/camera/freeCamera";
import type { RenderStyle } from "../../view/camera/state";
import type { ViewSize } from "../../view/engine/types";
import type { PhotorealStatus } from "../../view/photoreal/renderer";
import type { ViewRun } from "./viewRun";

/**
 * A scripted camera at one frame: the pose the script gives, the style the scene draws in and the
 * field of view it is drawn at, degrees.
 */
export interface ScriptedCamera {
  readonly pose: CameraPose;
  readonly style: RenderStyle;
  readonly fovDeg: number;
}

/** What one primary frame drawn at a scripted camera drew, as a perf run's metrics read it. */
export interface ViewPerfFrame {
  /** The frame's `requestAnimationFrame` timestamp, ms. */
  readonly rafTimestampMs: number;
  /** The frame callback's start, `performance.now()` ms. */
  readonly callbackStartMs: number;
  /** The main thread's time in the frame callback, its instruments and readouts included, ms. */
  readonly callbackMs: number;
  /** The primary's presented size, device px. */
  readonly size: ViewSize;
  /** The style it drew in: the wireframe stand-in while the photorealistic pipelines compile. */
  readonly drawnStyle: RenderStyle;
  /** Where its photorealistic renderer stands: `failed` where its pipelines could not be made. */
  readonly photoreal: PhotorealStatus;
  /** Its scene target's scale this frame: its budget's, or the resolution controller's below it. */
  readonly renderScale: number;
  /** Whether its terrain's annunciation is `TERRAIN: STREAMING`. */
  readonly streaming: boolean;
}

/** A perf run's hooks on `VIEW`'s loop, read at each frame. */
export interface ViewPerfHooks {
  /**
   * The primary's camera at a frame of the server's scene, at the frame's
   * `requestAnimationFrame` time: the script's, or `null` to leave the run's own (before the
   * script can stand in the scene).
   */
  readonly cameraAt: (run: ViewRun, nowMs: number) => ScriptedCamera | null;
  /** Told after each primary frame drawn at a scripted camera, with what it drew. */
  readonly onDrawn: (frame: ViewPerfFrame) => void;
}

/**
 * `run` with its camera the free camera at rest at `camera`'s pose, in its style and at its field
 * of view, marked as flown by its script (`ViewRun.scripted`), so that its `CAMERA` reads
 * `SCRIPTED`.
 */
export function scriptedRun(run: ViewRun, camera: ScriptedCamera): ViewRun {
  return {
    ...run,
    camera: { ...scriptedFreeCamera(run.camera, camera.pose, camera.style), fovDeg: camera.fovDeg },
    scripted: true,
  };
}
