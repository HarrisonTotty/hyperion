/**
 * Turning a view's camera by dragging on its canvas (plan R07, T19.f): a press that moves past
 * the click slop turns the camera, one that does not is a click, which picks.
 */

import { type PointerEvent as ReactPointerEvent, useRef } from "react";

import { CLICK_SLOP_REM, TOUCH_CLICK_SLOP_REM } from "../../spatial/usePointerOrbit";
import { type CanvasPointPx, type CanvasSizePx, dragTurn } from "../../view/camera/drag";
import type { ViewTurn } from "../../view/camera/look";

/** One press on a view's canvas, and what it has become. */
export interface ViewPress {
  /** The pointer that pressed; any other is ignored until it is released. */
  readonly pointerId: number;
  /** Where it went down, CSS px from the canvas's top left. */
  readonly start: CanvasPointPx;
  /** Where it was last seen. */
  readonly last: CanvasPointPx;
  /** Whether it has moved past the slop, and so turns the camera and will not pick. */
  readonly dragging: boolean;
  /** How far it may stray and still be a click, CSS px. */
  readonly slopPx: number;
}

/**
 * A press at `point`: a click until it strays the spatial displays' click slop (`usePointerOrbit`)
 * from there, a quarter of a `rem` for a mouse or a pen and half a `rem` for a finger.
 *
 * @param pointerType - `PointerEvent.pointerType`: `touch` takes a finger's slop.
 * @param remPx - CSS px in a `rem` at the interface scale.
 */
export function pressAt(
  pointerId: number,
  point: CanvasPointPx,
  pointerType: string,
  remPx: number,
): ViewPress {
  const slopRem = pointerType === "touch" ? TOUCH_CLICK_SLOP_REM : CLICK_SLOP_REM;
  return { pointerId, start: point, last: point, dragging: false, slopPx: slopRem * remPx };
}

/**
 * A press moved to `point`, and the turn it asks of the camera, or `null` while it is still a
 * click.
 *
 * @remarks
 * As the spatial displays' drag does, a press that comes as far as its slop from where it went down
 * becomes a drag, whose first turn runs from the press point, so that nothing of the drag is lost
 * and the direction under the press follows the pointer; each later move turns from the last.
 *
 * @param sizePx - The canvas's laid-out size, CSS px.
 * @param fovXDeg - The view's horizontal field of view, degrees.
 */
export function pressMoved(
  press: ViewPress,
  point: CanvasPointPx,
  sizePx: CanvasSizePx,
  fovXDeg: number,
): { readonly press: ViewPress; readonly turn: ViewTurn | null } {
  if (!press.dragging) {
    const strayPx = Math.hypot(point.xPx - press.start.xPx, point.yPx - press.start.yPx);
    if (strayPx < press.slopPx) {
      return { press, turn: null };
    }
  }
  const from = press.dragging ? press.last : press.start;
  return {
    press: { ...press, last: point, dragging: true },
    turn: dragTurn(from, point, sizePx, fovXDeg),
  };
}

/** What {@link useViewDrag} asks of its view. */
export interface ViewDragInput {
  /** The view's horizontal field of view as it is drawn now, degrees. */
  readonly fovDeg: () => number;
  /** CSS px in a `rem`, which sizes the click slop. */
  readonly remPx: number;
  /** Called at a press, before anything else: the view becomes the `CONTROLS` view. */
  readonly onPress: () => void;
  /** Gathers a drag's turn for the view's next frame. */
  readonly onTurn: (turn: ViewTurn) => void;
  /** Called with a click's press point, CSS px from the canvas's top left. */
  readonly onPick: (xPx: number, yPx: number) => void;
}

/** The pointer handlers a view's canvas takes, and the end of a drag for its `blur`. */
export interface ViewDragHandlers {
  readonly onPointerDown: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  readonly onPointerMove: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  readonly onPointerUp: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  readonly onPointerCancel: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  readonly onLostPointerCapture: (event: ReactPointerEvent<HTMLCanvasElement>) => void;
  /** Ends a press without a click, releasing its capture: the canvas lost the focus. */
  readonly end: (canvas: HTMLCanvasElement) => void;
}

/** Where a pointer event is, CSS px from the canvas's top left. */
function pointOn(event: ReactPointerEvent<HTMLCanvasElement>): CanvasPointPx {
  const box = event.currentTarget.getBoundingClientRect();
  return { xPx: event.clientX - box.left, yPx: event.clientY - box.top };
}

/**
 * Turns a view's camera by dragging on its canvas, and picks by clicking it (plan R07, T19.f).
 *
 * @remarks
 * A press of the primary button, a finger or a pen captures its pointer, so that a drag leaving
 * the canvas keeps turning until it is released, and makes the view the `CONTROLS` view. Once it
 * comes as far as the click slop from where it went down it is a drag ({@link pressMoved}): each
 * move gathers the turn that carries the direction under the pointer with it (`dragTurn`, at the
 * view's field of view and the canvas's laid-out size), which the view's next frame applies. A
 * press released inside the slop is a click, reported with its press point. `pointercancel`, a
 * capture lost without a `pointerup`, and the canvas's `blur` ({@link ViewDragHandlers.end}) end a
 * press without a click. A second pointer while one is down is ignored. Nothing depends on hover or
 * the secondary button, and nothing coasts after a release.
 */
export function useViewDrag(input: ViewDragInput): ViewDragHandlers {
  const pressRef = useRef<ViewPress | null>(null);

  const release = (canvas: HTMLCanvasElement): ViewPress | null => {
    const press = pressRef.current;
    pressRef.current = null;
    if (press !== null && canvas.hasPointerCapture(press.pointerId)) {
      canvas.releasePointerCapture(press.pointerId);
    }
    return press;
  };
  const ownPress = (event: ReactPointerEvent<HTMLCanvasElement>): ViewPress | null => {
    const press = pressRef.current;
    return press !== null && press.pointerId === event.pointerId ? press : null;
  };

  return {
    onPointerDown: (event) => {
      if (event.button !== 0 || pressRef.current !== null) {
        return;
      }
      input.onPress();
      pressRef.current = pressAt(
        event.pointerId,
        pointOn(event),
        event.pointerType,
        input.remPx > 0 ? input.remPx : 16,
      );
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    onPointerMove: (event) => {
      const press = ownPress(event);
      if (press === null) {
        return;
      }
      const box = event.currentTarget.getBoundingClientRect();
      const moved = pressMoved(
        press,
        pointOn(event),
        { widthPx: box.width, heightPx: box.height },
        input.fovDeg(),
      );
      pressRef.current = moved.press;
      if (moved.turn !== null) {
        input.onTurn(moved.turn);
      }
    },
    onPointerUp: (event) => {
      if (ownPress(event) === null) {
        return;
      }
      const press = release(event.currentTarget);
      if (press !== null && !press.dragging) {
        input.onPick(press.start.xPx, press.start.yPx);
      }
    },
    onPointerCancel: (event) => {
      if (ownPress(event) !== null) {
        release(event.currentTarget);
      }
    },
    // After a `pointerup` the press is already gone, and this does nothing.
    onLostPointerCapture: (event) => {
      if (ownPress(event) !== null) {
        release(event.currentTarget);
      }
    },
    end: (canvas) => {
      release(canvas);
    },
  };
}
