import { type PointerEvent as ReactPointerEvent, type RefObject, useEffect, useRef } from "react";

import type { ScreenPoint } from "./drawList";
import type { CameraMove } from "./useOrbitCamera";

/** Degrees the camera turns for each `rem` a pointer is dragged (plan 05, T10.d). */
export const DRAG_DEG_PER_REM = 8;

/**
 * How far, in `rem`, a mouse or a pen may move between down and up and still be a click, not a
 * drag.
 */
export const CLICK_SLOP_REM = 0.25;

/**
 * How far, in `rem`, a finger may move between down and up and still be a tap, not a drag: 8 px at
 * 100%, as the platforms allow a touch (Android's touch slop is 8 dp).
 *
 * @remarks
 * A finger wobbles further on a tap than a mouse does on a click, and the guide has consoles run on
 * touch screens with targets of 2 rem, so a finger is given twice a mouse's slop. A drag still turns
 * the camera from where the finger went down, so nothing of it is lost.
 */
export const TOUCH_CLICK_SLOP_REM = 0.5;

/** Wheel travel, in CSS pixels, that zooms by a factor of 2. */
const WHEEL_PX_PER_DOUBLING = 400;

/** CSS pixels in a line of wheel travel, for a wheel that reports lines. */
const WHEEL_LINE_PX = 16;

/** One pointer that is down on the canvas. */
interface ActivePointer {
  /** Where it went down, in CSS pixels from the canvas's top left. */
  readonly start: ScreenPoint;
  /** Where it was last seen. */
  last: ScreenPoint;
}

/** The pointers down on the canvas, and what their gesture has become. */
interface Gesture {
  readonly pointers: Map<number, ActivePointer>;
  /** Whether the gesture has turned the camera; a gesture that never did may still be a click. */
  dragging: boolean;
  /** Whether a second pointer joined, making it a pinch, never a click. */
  pinched: boolean;
  /** How far, in `rem`, the first pointer may stray and still click: a finger's slop or a mouse's. */
  slopRem: number;
}

function newGesture(): Gesture {
  return { pointers: new Map(), dragging: false, pinched: false, slopRem: CLICK_SLOP_REM };
}

function distancePx(a: ScreenPoint, b: ScreenPoint): number {
  return Math.hypot(a.xPx - b.xPx, a.yPx - b.yPx);
}

/** The separation of the first two pointers, or `null` with fewer. */
function separationPx(pointers: ReadonlyMap<number, ActivePointer>): number | null {
  const [first, second] = pointers.values();
  return first === undefined || second === undefined ? null : distancePx(first.last, second.last);
}

/** Where a pointer event is, in CSS pixels from the element's top left. */
function pointOn(element: Element, clientX: number, clientY: number): ScreenPoint {
  const box = element.getBoundingClientRect();
  return { xPx: clientX - box.left, yPx: clientY - box.top };
}

/** Wheel travel in CSS pixels, whatever unit the wheel reports it in. */
function wheelTravelPx(event: WheelEvent, pageHeightPx: number): number {
  switch (event.deltaMode) {
    case WheelEvent.DOM_DELTA_LINE:
      return event.deltaY * WHEEL_LINE_PX;
    case WheelEvent.DOM_DELTA_PAGE:
      return event.deltaY * pageHeightPx;
    default:
      return event.deltaY;
  }
}

/** The pointer handlers a spatial view's canvas takes. */
export interface PointerOrbitHandlers {
  readonly onPointerDown: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  readonly onPointerMove: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  readonly onPointerUp: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  readonly onPointerCancel: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  readonly onLostPointerCapture: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
}

/**
 * Turns and zooms a spatial view's camera with a pointer, the wheel and a pinch.
 *
 * @remarks
 * A drag with the primary button, a finger or a pen turns the camera 8° for each `rem` of travel:
 * across to the azimuth, and down to raise the camera. The pointer is captured, so a drag that
 * leaves the canvas keeps turning it. Movement under a quarter of a `rem` between down and up, or
 * half a `rem` for a finger, is a click, reported with the point, and does not turn the camera.
 * Two pointers zoom by the ratio of their separations. The wheel zooms by 2^(−travel ÷ 400 px)
 * through a listener that is not passive, so that the page never scrolls under the view.
 * `pointercancel`, or a capture lost without a `pointerup`, ends that pointer's part of the gesture
 * without a click, so that a missed up never leaves a pointer behind to turn the next drag into a
 * pinch. Nothing depends on hover or the secondary button.
 *
 * @param canvasRef - The canvas, which takes the wheel listener.
 * @param remPx - CSS pixels in a `rem`, which scales the drag and the click slop.
 * @param move - Gathers a camera move for the next frame.
 * @param onClick - Called with a click's point, in CSS pixels from the canvas's top left.
 */
export function usePointerOrbit(
  canvasRef: RefObject<HTMLCanvasElement | null>,
  remPx: number,
  move: (cameraMove: CameraMove) => void,
  onClick: (pointPx: ScreenPoint) => void,
): PointerOrbitHandlers {
  const gestureRef = useRef<Gesture>(newGesture());

  useEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null) {
      return undefined;
    }
    const onWheel = (event: WheelEvent): void => {
      event.preventDefault();
      const travelPx = wheelTravelPx(event, canvas.clientHeight);
      if (travelPx !== 0) {
        move({ kind: "zoom", factor: 2 ** (-travelPx / WHEEL_PX_PER_DOUBLING) });
      }
    };
    canvas.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      canvas.removeEventListener("wheel", onWheel);
    };
  }, [canvasRef, move]);

  const end = (event: ReactPointerEvent<HTMLCanvasElement>): ActivePointer | undefined => {
    const gesture = gestureRef.current;
    const pointer = gesture.pointers.get(event.pointerId);
    gesture.pointers.delete(event.pointerId);
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
    if (gesture.pointers.size === 0) {
      gestureRef.current = newGesture();
    }
    return pointer;
  };

  return {
    onPointerDown: (event) => {
      if (event.button !== 0 || !(remPx > 0)) {
        return;
      }
      const point = pointOn(event.currentTarget, event.clientX, event.clientY);
      const gesture = gestureRef.current;
      gesture.pointers.set(event.pointerId, { start: point, last: point });
      if (gesture.pointers.size > 1) {
        gesture.pinched = true;
      } else {
        gesture.slopRem = event.pointerType === "touch" ? TOUCH_CLICK_SLOP_REM : CLICK_SLOP_REM;
      }
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    onPointerMove: (event) => {
      const gesture = gestureRef.current;
      const pointer = gesture.pointers.get(event.pointerId);
      if (pointer === undefined) {
        return;
      }
      const point = pointOn(event.currentTarget, event.clientX, event.clientY);
      if (gesture.pointers.size > 1) {
        const beforePx = separationPx(gesture.pointers);
        pointer.last = point;
        const afterPx = separationPx(gesture.pointers);
        if (beforePx !== null && afterPx !== null && beforePx > 0 && afterPx > 0) {
          move({ kind: "zoom", factor: afterPx / beforePx });
        }
        return;
      }
      if (!gesture.dragging) {
        if (distancePx(pointer.start, point) < gesture.slopRem * remPx) {
          return;
        }
        // The drag turns the camera from where the pointer went down.
        gesture.dragging = true;
      }
      const degPerPx = DRAG_DEG_PER_REM / remPx;
      move({
        kind: "turn",
        azimuthDeg: (point.xPx - pointer.last.xPx) * degPerPx,
        elevationDeg: (point.yPx - pointer.last.yPx) * degPerPx,
      });
      pointer.last = point;
    },
    onPointerUp: (event) => {
      const gesture = gestureRef.current;
      const wasClick = !gesture.dragging && !gesture.pinched && gesture.pointers.size === 1;
      const pointer = end(event);
      if (wasClick && pointer !== undefined) {
        onClick(pointer.start);
      }
    },
    onPointerCancel: (event) => {
      end(event);
    },
    // After a `pointerup` the pointer is already gone, and this does nothing.
    onLostPointerCapture: (event) => {
      end(event);
    },
  };
}
