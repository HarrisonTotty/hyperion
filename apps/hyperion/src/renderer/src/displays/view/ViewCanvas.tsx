import type { KeyboardEvent, MouseEvent, ReactNode } from "react";

/** Props of {@link ViewCanvas}. */
export interface ViewCanvasProps {
  /** Receives the canvas, which the display's engine draws into. */
  readonly canvasRef: (canvas: HTMLCanvasElement | null) => void;
  /** Receives the stage, which the display measures. */
  readonly stageRef: (stage: HTMLElement | null) => void;
  /** The canvas's accessible name: `VIEW, WIREFRAME, SEAT` (Design note 17). */
  readonly accessibleName: string;
  /** The ID of the text describing the canvas's keys. */
  readonly describedBy: string;
  readonly onKeyDown: (event: KeyboardEvent<HTMLCanvasElement>) => void;
  readonly onKeyUp: (event: KeyboardEvent<HTMLCanvasElement>) => void;
  /** Called when the canvas loses focus, so that no flight key stays held. */
  readonly onBlur: () => void;
  /** Called with a click's position, CSS px from the canvas's top left. */
  readonly onPick: (xPx: number, yPx: number) => void;
  /** The DOM drawn over the canvas: the label block on its plate. */
  readonly children: ReactNode;
}

/**
 * The view's canvas (plan R02, R02.T15.b): focusable, named for its style and camera, flown by the
 * flight keys while it has focus and picked from by a click; the text over it is DOM, never drawn
 * into it (the guide's "Outlines for symbology").
 */
export function ViewCanvas({
  canvasRef,
  stageRef,
  accessibleName,
  describedBy,
  onKeyDown,
  onKeyUp,
  onBlur,
  onPick,
  children,
}: ViewCanvasProps) {
  const onClick = (event: MouseEvent<HTMLCanvasElement>): void => {
    const box = event.currentTarget.getBoundingClientRect();
    onPick(event.clientX - box.left, event.clientY - box.top);
  };
  return (
    <div className="view__stage" ref={stageRef}>
      <canvas
        ref={canvasRef}
        className="view__canvas"
        // A view to fly and pick from, which no native element is; what it shows is text in the
        // DOM over and beside it. The rule counts a canvas as interactive, which HTML does not.
        // oxlint-disable-next-line jsx-a11y/no-interactive-element-to-noninteractive-role
        role="application"
        tabIndex={0}
        aria-label={accessibleName}
        aria-describedby={describedBy}
        onKeyDown={onKeyDown}
        onKeyUp={onKeyUp}
        onBlur={onBlur}
        onClick={onClick}
      />
      <div className="view__overlay">{children}</div>
    </div>
  );
}
