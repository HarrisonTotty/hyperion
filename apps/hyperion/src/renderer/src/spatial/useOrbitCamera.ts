import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

import {
  type Camera,
  type CameraAngles,
  PRESETS,
  rotateCamera,
  zoomCamera,
  zoomLimits,
} from "./camera";
import { createRedrawScheduler, type RedrawScheduler } from "./redraw";
import { easeOut, TRANSITION_MS, tweenCamera } from "./transition";

/**
 * The camera as the operator left it: the angles, and the zoom they chose.
 *
 * @remarks
 * `pxPerUnit` is `null` while the view fits its fit radius, as it does at first and after `Z`, so
 * that it keeps fitting as the view is resized or given a new radius. Once the operator zooms it is
 * a scale of its own, and only kept within the zoom limits of the current fit.
 */
export interface OrbitCameraState {
  readonly angles: CameraAngles;
  readonly pxPerUnit: number | null;
}

/** One thing the operator does to the camera. */
export type CameraMove =
  | {
      /** Turns the camera: the azimuth wraps, the elevation stops at ±90°. */
      readonly kind: "turn";
      readonly azimuthDeg: number;
      readonly elevationDeg: number;
    }
  | {
      /** Zooms by a factor, above 1 in, within the zoom limits. */
      readonly kind: "zoom";
      readonly factor: number;
    }
  | {
      /** Returns to the zoom that fits the fit radius. */
      readonly kind: "fit";
    };

const INITIAL_CAMERA: OrbitCameraState = { angles: PRESETS.oblique, pxPerUnit: null };

/**
 * The camera after one move, zoomed about the fitted scale while it is still fitting.
 *
 * @remarks
 * A chosen scale is first brought within the limits of the current fit, as the view shows it, so
 * that a zoom after a change of radius or size starts from what is on screen.
 */
function moved(
  camera: OrbitCameraState,
  move: CameraMove,
  fittedPxPerUnit: number,
): OrbitCameraState {
  const limits = zoomLimits(fittedPxPerUnit);
  const shown: Camera = {
    ...camera.angles,
    pxPerUnit: zoomCamera(
      { ...camera.angles, pxPerUnit: camera.pxPerUnit ?? fittedPxPerUnit },
      1,
      limits,
    ).pxPerUnit,
  };
  let result: OrbitCameraState;
  switch (move.kind) {
    case "turn": {
      const { azimuthDeg, elevationDeg } = rotateCamera(shown, move.azimuthDeg, move.elevationDeg);
      result = { angles: { azimuthDeg, elevationDeg }, pxPerUnit: camera.pxPerUnit };
      break;
    }
    case "zoom":
      result = {
        angles: camera.angles,
        pxPerUnit: zoomCamera(shown, move.factor, limits).pxPerUnit,
      };
      break;
    case "fit":
      result = { angles: camera.angles, pxPerUnit: null };
      break;
  }
  return result;
}

/**
 * The camera after the moves gathered for one frame, each applied in turn, so that the result is
 * the same however the moves fall into frames.
 *
 * @remarks
 * A camera the moves leave as it was, such as one held against a limit, is returned itself, so that
 * it paints nothing.
 */
function afterMoves(
  camera: OrbitCameraState,
  moves: ReadonlyArray<CameraMove>,
  fittedPxPerUnit: number,
): OrbitCameraState {
  const result = moves.reduce((current, move) => moved(current, move, fittedPxPerUnit), camera);
  const unchanged =
    result.angles.azimuthDeg === camera.angles.azimuthDeg &&
    result.angles.elevationDeg === camera.angles.elevationDeg &&
    result.pxPerUnit === camera.pxPerUnit;
  return unchanged ? camera : result;
}

/** The operator's orbit camera and the way to move it. */
export interface OrbitCamera {
  readonly camera: OrbitCameraState;
  /**
   * Gathers a move, applied on the next frame together with any others that arrive before it: key
   * repeats and pointer moves cost one render a frame. A move ends a transition under way.
   */
  readonly move: (move: CameraMove) => void;
  /**
   * Turns the camera to `angles`, keeping its zoom: over {@link TRANSITION_MS} with the guide's
   * ease-out, a frame at a time, landing exactly on them, or at once when `instantly`, as under
   * reduced motion. Moves gathered and not yet applied are dropped.
   */
  readonly turnTo: (angles: CameraAngles, instantly: boolean) => void;
}

/** A turn under way to a preset: where it started, where it ends, and the time of its first frame. */
interface Transition {
  readonly from: CameraAngles;
  readonly to: CameraAngles;
  readonly startMs: number | null;
}

function sameAngles(a: CameraAngles, b: CameraAngles): boolean {
  return a.azimuthDeg === b.azimuthDeg && a.elevationDeg === b.elevationDeg;
}

/** The angles part of the way along a transition, on the guide's ease-out. */
function anglesAt(transition: Transition, elapsedMs: number): CameraAngles {
  const progress = easeOut(elapsedMs / TRANSITION_MS);
  const { azimuthDeg, elevationDeg } = tweenCamera(
    { ...transition.from, pxPerUnit: 1 },
    { ...transition.to, pxPerUnit: 1 },
    progress,
  );
  return { azimuthDeg, elevationDeg };
}

/**
 * Owns a spatial view's camera (plan 05, design note D16), moved by operator input at most once a
 * frame.
 *
 * @remarks
 * Moves are added to a ref and a frame is asked for through a redraw scheduler; the frame commits
 * one state update with everything gathered since the last. A turn to a preset asks for one frame
 * after another until it lands, and then for none. With no input there is no frame. The scheduler
 * is disposed when the view is unmounted or hidden, which cancels a frame still pending.
 *
 * @param fittedPxPerUnit - The zoom that fits the view's fit radius, or `null` before the view is
 *   laid out, when moves are ignored; a turn to a preset then sets the angles the view will open
 *   with.
 * @param fitRequest - Changing it returns the camera to the fitted zoom, as `Z` does, keeping its
 *   angles: a display's zoom preset changes it with each press, so that the preset fits whatever
 *   the operator zoomed to before. Absent, as on the star chart, nothing is asked.
 */
export function useOrbitCamera(fittedPxPerUnit: number | null, fitRequest?: number): OrbitCamera {
  const [camera, setCamera] = useState<OrbitCameraState>(INITIAL_CAMERA);
  const [fitRequestSeen, setFitRequestSeen] = useState(fitRequest);
  let shownCamera = camera;
  // Adjusted during render, as a derived reset, so that the preset's fit is drawn in the render it
  // is chosen in.
  if (fitRequest !== fitRequestSeen) {
    setFitRequestSeen(fitRequest);
    if (camera.pxPerUnit !== null) {
      shownCamera = { angles: camera.angles, pxPerUnit: null };
      setCamera(shownCamera);
    }
  }
  const pendingRef = useRef<CameraMove[]>([]);
  const transitionRef = useRef<Transition | null>(null);
  const schedulerRef = useRef<RedrawScheduler | null>(null);
  // The angles on screen, for a turn to start from; kept in a ref so that `turnTo`, and the key
  // listener that calls it, stay the same through a drag.
  const anglesRef = useRef(camera.angles);
  useLayoutEffect(() => {
    anglesRef.current = camera.angles;
  }, [camera.angles]);

  useEffect(() => {
    const scheduler = createRedrawScheduler(
      (callback) => window.requestAnimationFrame(callback),
      (handle) => {
        window.cancelAnimationFrame(handle);
      },
    );
    schedulerRef.current = scheduler;
    return () => {
      scheduler.dispose();
      schedulerRef.current = null;
      pendingRef.current = [];
      // A view hidden mid-turn is shown again where the turn left it.
      transitionRef.current = null;
    };
  }, []);

  // Stable while the fitted scale is, so that the document's key listener is not added again on
  // every frame of a drag.
  const move = useCallback(
    (next: CameraMove): void => {
      const scheduler = schedulerRef.current;
      if (scheduler === null || fittedPxPerUnit === null) {
        return;
      }
      transitionRef.current = null;
      pendingRef.current.push(next);
      scheduler.request(() => {
        const gathered = pendingRef.current;
        pendingRef.current = [];
        setCamera((previous) => afterMoves(previous, gathered, fittedPxPerUnit));
      });
    },
    [fittedPxPerUnit],
  );

  const turnTo = useCallback((angles: CameraAngles, instantly: boolean): void => {
    const scheduler = schedulerRef.current;
    if (scheduler === null) {
      return;
    }
    // A frame already asked for finds nothing gathered and no transition, and changes nothing.
    pendingRef.current = [];
    if (sameAngles(anglesRef.current, angles)) {
      // Already there: nothing to turn, and no frame to ask for.
      transitionRef.current = null;
      return;
    }
    if (instantly) {
      transitionRef.current = null;
      setCamera((previous) =>
        sameAngles(previous.angles, angles) ? previous : { angles, pxPerUnit: previous.pxPerUnit },
      );
      return;
    }
    transitionRef.current = { from: anglesRef.current, to: angles, startMs: null };
    const step = (timeMs: number): void => {
      const transition = transitionRef.current;
      if (transition === null) {
        return;
      }
      // The first frame starts the clock, so that the whole turn is seen.
      const startMs = transition.startMs ?? timeMs;
      const elapsedMs = timeMs - startMs;
      const shown = elapsedMs >= TRANSITION_MS ? transition.to : anglesAt(transition, elapsedMs);
      setCamera((previous) =>
        sameAngles(previous.angles, shown)
          ? previous
          : { angles: shown, pxPerUnit: previous.pxPerUnit },
      );
      if (elapsedMs >= TRANSITION_MS) {
        transitionRef.current = null;
      } else {
        transitionRef.current = { ...transition, startMs };
        scheduler.request(step);
      }
    };
    scheduler.request(step);
  }, []);

  return { camera: shownCamera, move, turnTo };
}
