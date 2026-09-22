import {
  Fragment,
  type KeyboardEvent,
  type ReactNode,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { isTextEntry } from "../lib/textEntry";
import { useElementSize } from "../lib/useElementSize";
import { usePrefersReducedMotion } from "../lib/usePrefersReducedMotion";
import {
  type Camera,
  fitPxPerUnit,
  PRESETS,
  type PresetName,
  type Viewport,
  zoomLimits,
} from "./camera";
import { buildDrawList, type ScreenPoint } from "./drawList";
import type { SpatialScene } from "./marks";
import { type ColourTokens, paint, readTokens, sameTokens } from "./paint";
import { pick } from "./pick";
import { PRESET_CONTROLS, PresetButtons } from "./PresetButtons";
import { type CameraMove, useOrbitCamera } from "./useOrbitCamera";
import { usePointerOrbit } from "./usePointerOrbit";

/**
 * Space kept clear between the fitted sphere and the edge of the view, in `rem`, room for the
 * data edge's ticks and a curve label above the circle.
 */
const FIT_MARGIN_REM = 2;

/** Degrees an arrow key turns the camera, and with `Shift`. */
const ARROW_STEP_DEG = 5;
const FINE_ARROW_STEP_DEG = 1;

/** The factor one press of a zoom key zooms by. */
const ZOOM_STEP = 1.25;

/** The view's keys, shown above it and describing its canvas. */
const KEY_LEGEND: ReadonlyArray<string> = ["ARROWS ROTATE", "+/− ZOOM", "T S F O VIEWS", "Z FIT"];

/** How many steps an arrow key turns the camera by: right raises the azimuth, up the elevation. */
interface ArrowTurn {
  readonly azimuthSteps: number;
  readonly elevationSteps: number;
}

const ARROW_TURNS: Readonly<Record<string, ArrowTurn>> = {
  ArrowLeft: { azimuthSteps: -1, elevationSteps: 0 },
  ArrowRight: { azimuthSteps: 1, elevationSteps: 0 },
  ArrowUp: { azimuthSteps: 0, elevationSteps: 1 },
  ArrowDown: { azimuthSteps: 0, elevationSteps: -1 },
};

/** The preset a letter key chooses, `T`, `S`, `F` or `O`, or `null` for any other key. */
function presetForKey(key: string): PresetName | null {
  return PRESET_CONTROLS.find((control) => control.key === key.toUpperCase())?.name ?? null;
}

/**
 * The view's zoom keys, which act from anywhere on its display but a text field (plan 05, design
 * note D3): `+` and `=` zoom in, `-` and `−` zoom out, `Z` fits.
 */
function viewKeyMove(key: string): CameraMove | null {
  switch (key) {
    case "+":
    case "=":
      return { kind: "zoom", factor: ZOOM_STEP };
    case "-":
    case "−":
      return { kind: "zoom", factor: 1 / ZOOM_STEP };
    case "z":
    case "Z":
      return { kind: "fit" };
    default:
      return null;
  }
}

/** Props of {@link SpatialView}. */
export interface SpatialViewProps {
  /** What to draw, in scene units about the view centre. */
  readonly scene: SpatialScene;
  /** The radius, in scene units, that the default zoom fits into the view: the query radius. */
  readonly fitRadius: number;
  /** Writes a length in scene units with its unit, for the scale bar: `20 ly`, `500 AU`. */
  readonly formatLength: (length: number) => string;
  /** The reference frame's name, shown with the view: `GALACTIC`. */
  readonly frameName: string;
  /** The accessible name of the view's canvas. */
  readonly accessibleName: string;
  /** Called with the mark the operator picks on the canvas. */
  readonly onSelect: (id: string) => void;
  /**
   * Furniture drawn over the view, such as the display's own legends. It takes no pointer, so that
   * the canvas under it gets every drag and pick: it holds no controls.
   */
  readonly children?: ReactNode;
}

/**
 * The general 3D spatial view: an orthographic orbit camera over a scene of marks, a reference
 * plane and spheres, painted on a canvas (plan 05, T10).
 *
 * @remarks
 * The view owns its camera, which starts at the oblique preset at the zoom that fits a sphere of
 * `fitRadius` (plan 05, design note D16). With the canvas focused the arrow keys turn it 5° a
 * press, 1° with `Shift`, right raising the azimuth and up the elevation; from anywhere on the
 * display but a text field `+` and `=` zoom in and `-` and `−` out by 1.25, and `Z` fits again.
 * The preset buttons `TOP`, `SIDE`, `FRONT` and `OBLIQUE`, or their keys `T`, `S`, `F` and `O`,
 * turn the camera to their view over 120 ms, or at once under reduced motion, keeping the zoom.
 * Zoom stays within 0.5 to 100 times the scale that fits. A drag turns the camera, the wheel and a
 * pinch zoom it, and a click picks the nearest mark (see {@link usePointerOrbit}). All input is
 * gathered into one frame. Its container is measured through a `ResizeObserver`; the canvas's
 * backing store follows the device pixel ratio, and sizes in `rem` follow the root font size. The
 * draw list is built during render from the scene, the camera and the viewport, and a layout
 * effect paints it whenever one of them or the colour tokens change. Nothing runs on a loop: an
 * idle view paints nothing. The canvas takes focus and is described by the visible key
 * legend; labels and furniture are DOM over it, since the canvas carries no text (D15).
 */
export function SpatialView({
  scene,
  fitRadius,
  accessibleName,
  onSelect,
  children,
}: SpatialViewProps) {
  const legendId = useId();
  const { ref: stageRef, size } = useElementSize();

  // The viewport, the camera and the draw list are memoised for their identity, not their cost: an
  // equal re-render keeps the draw list, and the paint effect, which runs when it changes, is not
  // run again (the view redraws on demand only).
  const widthPx = size?.widthPx ?? 0;
  const heightPx = size?.heightPx ?? 0;
  const remPx = size?.remPx ?? 0;
  const pixelRatio = size?.devicePixelRatio ?? 1;
  const viewport = useMemo<Viewport | null>(
    () => (widthPx > 0 && heightPx > 0 ? { widthPx, heightPx, remPx } : null),
    [widthPx, heightPx, remPx],
  );

  const fittedPxPerUnit =
    viewport === null ? null : fitPxPerUnit(fitRadius, viewport, FIT_MARGIN_REM * viewport.remPx);
  const { camera: cameraState, move, turnTo } = useOrbitCamera(fittedPxPerUnit);
  const reducedMotion = usePrefersReducedMotion();
  const choosePreset = useCallback(
    (name: PresetName): void => {
      turnTo(PRESETS[name], reducedMotion);
    },
    [turnTo, reducedMotion],
  );
  const camera = useMemo<Camera | null>(() => {
    if (fittedPxPerUnit === null) {
      return null;
    }
    const limits = zoomLimits(fittedPxPerUnit);
    const chosen = cameraState.pxPerUnit ?? fittedPxPerUnit;
    return {
      ...cameraState.angles,
      pxPerUnit: Math.min(limits.maxPxPerUnit, Math.max(limits.minPxPerUnit, chosen)),
    };
  }, [cameraState, fittedPxPerUnit]);

  const drawList = useMemo(
    () => (camera === null || viewport === null ? null : buildDrawList(scene, camera, viewport)),
    [scene, camera, viewport],
  );

  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [tokens, setTokens] = useState<ColourTokens | null>(null);
  // The tokens are read when the view is mounted and each time its display is shown again, when
  // `Activity` runs its effects anew; an unchanged set keeps its identity, and paints nothing.
  useLayoutEffect(() => {
    if (canvasRef.current === null) {
      return;
    }
    const current = readTokens(canvasRef.current);
    setTokens((previous) =>
      previous !== null && sameTokens(previous, current) ? previous : current,
    );
  }, []);

  const backingWidthPx = Math.round(widthPx * pixelRatio);
  const backingHeightPx = Math.round(heightPx * pixelRatio);
  // A new backing size, which clears the canvas, comes with a new viewport and so a new draw list,
  // or with a new pixel ratio: either paints again.
  useLayoutEffect(() => {
    const context = canvasRef.current?.getContext("2d") ?? null;
    if (context === null || drawList === null || tokens === null) {
      return;
    }
    paint(context, drawList, tokens, pixelRatio);
  }, [drawList, tokens, pixelRatio]);

  // The view's single keys act from anywhere on the display but a text field (D3).
  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent): void => {
      if (event.ctrlKey || event.altKey || event.metaKey || isTextEntry(event.target)) {
        return;
      }
      // A letter with Shift, or held down, is left alone; `+` is typed with Shift, and repeats.
      const letter = !event.shiftKey && !event.repeat;
      const preset = letter ? presetForKey(event.key) : null;
      if (preset !== null) {
        event.preventDefault();
        choosePreset(preset);
        return;
      }
      const keyMove = viewKeyMove(event.key);
      if (keyMove === null || (!letter && keyMove.kind === "fit")) {
        return;
      }
      event.preventDefault();
      move(keyMove);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [move, choosePreset]);

  // The arrow keys turn the camera only while the canvas has focus, so that the page never scrolls
  // under it and the arrows stay free for the list beside it.
  const onCanvasKeyDown = (event: KeyboardEvent<HTMLCanvasElement>): void => {
    const turn = ARROW_TURNS[event.key];
    if (turn === undefined || event.ctrlKey || event.altKey || event.metaKey) {
      return;
    }
    event.preventDefault();
    const stepDeg = event.shiftKey ? FINE_ARROW_STEP_DEG : ARROW_STEP_DEG;
    move({
      kind: "turn",
      azimuthDeg: turn.azimuthSteps * stepDeg,
      elevationDeg: turn.elevationSteps * stepDeg,
    });
  };

  // A click picks the mark nearest the pointer within 1 rem, half the guide's 2 rem target; a click
  // on empty space selects nothing and keeps the selection.
  const onClick = (pointPx: ScreenPoint): void => {
    if (drawList === null) {
      return;
    }
    const picked = pick(drawList.anchors, pointPx, remPx);
    if (picked !== null) {
      onSelect(picked);
    }
  };
  const pointerHandlers = usePointerOrbit(canvasRef, remPx, move, onClick);

  return (
    <div className="spatial-view">
      <div className="spatial-view__controls">
        <PresetButtons angles={cameraState.angles} onChoose={choosePreset} />
        <p className="spatial-view__keys" id={legendId}>
          {KEY_LEGEND.map((entry, index) => (
            <Fragment key={entry}>
              {index > 0 ? " " : null}
              <span className="spatial-view__key">{entry}</span>
            </Fragment>
          ))}
        </p>
      </div>
      <div className="spatial-view__stage" ref={stageRef}>
        <canvas
          ref={canvasRef}
          className="spatial-view__canvas"
          width={backingWidthPx}
          height={backingHeightPx}
          // A view to rotate, zoom and pick from, which no native element is; its readings and
          // labels are text in the DOM around it. The rule counts a canvas as interactive, which
          // HTML does not; the plan names this role.
          // oxlint-disable-next-line jsx-a11y/no-interactive-element-to-noninteractive-role
          role="application"
          tabIndex={0}
          aria-label={accessibleName}
          aria-describedby={legendId}
          onKeyDown={onCanvasKeyDown}
          {...pointerHandlers}
        />
        <div className="spatial-view__overlay">{children}</div>
      </div>
    </div>
  );
}
