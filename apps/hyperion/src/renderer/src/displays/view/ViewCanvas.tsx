import type { FocusEvent, KeyboardEvent, ReactNode } from "react";

import type { ViewTurn } from "../../view/camera/look";
import { type DragKind, useViewDrag } from "./useViewDrag";

/** Props of {@link ViewCanvas}. */
export interface ViewCanvasProps {
  /** Receives the canvas, which the display's engine draws into. */
  readonly canvasRef: (canvas: HTMLCanvasElement | null) => void;
  /** Receives the stage, which the display measures. */
  readonly stageRef: (stage: HTMLElement | null) => void;
  /** The canvas's accessible name: `VIEW, WIREFRAME, PRIMARY, SEAT` (Design note 17; R07.T19). */
  readonly accessibleName: string;
  /** The ID of the text describing the canvas's keys. */
  readonly describedBy: string;
  readonly onKeyDown: (event: KeyboardEvent<HTMLCanvasElement>) => void;
  readonly onKeyUp: (event: KeyboardEvent<HTMLCanvasElement>) => void;
  /** Called when the canvas loses focus, so that no flight key stays held. */
  readonly onBlur: () => void;
  /**
   * The view's horizontal field of view as it is drawn now, degrees, by which a drag's travel is
   * turned into the camera's (R07.T19.f).
   */
  readonly fovDeg: () => number;
  /**
   * How a drag turns the view's camera now: an orbit about the own ship in `CHASE`, else a look
   * (decision-r07-t19f-position, item 4).
   */
  readonly dragKind: () => DragKind;
  /** CSS px in a `rem` at the interface scale, which sizes the click slop (R07.T19.f). */
  readonly remPx: number;
  /** Called at a press on the canvas, before it picks or turns: the view is the one acted on. */
  readonly onPress: () => void;
  /** Gathers a drag's turn of the view's camera for its next frame (R07.T19.f). */
  readonly onTurn: (turn: ViewTurn) => void;
  /** Called with a click's position, CSS px from the canvas's top left. */
  readonly onPick: (xPx: number, yPx: number) => void;
  /**
   * Called with a wheel's steps over the canvas, nearer for a positive step, one a notch (100 CSS
   * px of a pixel-mode wheel), which an `ORBIT` camera zooms by (plan R15, T4); none where absent.
   */
  readonly onZoom?: ((steps: number) => void) | undefined;
  /** The DOM drawn over the canvas: the label block on its plate. */
  readonly children: ReactNode;
}

/** CSS px of a pixel-mode wheel's scroll that make one step: a common mouse's notch. */
const WHEEL_PX_PER_STEP = 100;

/**
 * The steps a wheel's scroll of `deltaY` in `deltaMode` (`WheelEvent.deltaMode`: 0 px, 1 lines,
 * 2 pages) makes, nearer for a scroll up, a line or a page a step and pixels by
 * {@link WHEEL_PX_PER_STEP}.
 */
export function wheelSteps(deltaY: number, deltaMode: number): number {
  return deltaMode === 0 ? -deltaY / WHEEL_PX_PER_STEP : -Math.sign(deltaY);
}

/**
 * The view's canvas (plan R02, R02.T15.b): focusable, named for its style and camera, flown by the
 * flight keys while it has focus, turned by a drag and picked from by a click (R07.T19.f); the text
 * over it is DOM, never drawn into it (the guide's "Outlines for symbology").
 *
 * @remarks
 * A press captures its pointer and turns the camera once it strays past the click slop; a press
 * released inside it picks (`useViewDrag`). A press focuses the canvas, whose ring then shows that
 * its keys are live. Losing the focus ends a drag as it releases the held flight keys.
 */
export function ViewCanvas({
  canvasRef,
  stageRef,
  accessibleName,
  describedBy,
  onKeyDown,
  onKeyUp,
  onBlur,
  fovDeg,
  dragKind,
  remPx,
  onPress,
  onTurn,
  onPick,
  onZoom,
  children,
}: ViewCanvasProps) {
  const drag = useViewDrag({ fovDeg, dragKind, remPx, onPress, onTurn, onPick });
  const onCanvasBlur = (event: FocusEvent<HTMLCanvasElement>): void => {
    drag.end(event.currentTarget);
    onBlur();
  };
  return (
    <div className="view__stage" ref={stageRef}>
      <canvas
        ref={canvasRef}
        className="view__canvas"
        // A view to fly, turn and pick from, which no native element is; what it shows is text in
        // the DOM over and beside it. The rule counts a canvas as interactive, which HTML does not.
        // oxlint-disable-next-line jsx-a11y/no-interactive-element-to-noninteractive-role
        role="application"
        tabIndex={0}
        aria-label={accessibleName}
        aria-describedby={describedBy}
        onKeyDown={onKeyDown}
        onKeyUp={onKeyUp}
        onBlur={onCanvasBlur}
        onPointerDown={drag.onPointerDown}
        onPointerMove={drag.onPointerMove}
        onPointerUp={drag.onPointerUp}
        onPointerCancel={drag.onPointerCancel}
        onLostPointerCapture={drag.onLostPointerCapture}
        onWheel={(event) => {
          if (onZoom !== undefined && event.deltaY !== 0) {
            onZoom(wheelSteps(event.deltaY, event.deltaMode));
          }
        }}
      />
      <div className="view__overlay">{children}</div>
    </div>
  );
}
