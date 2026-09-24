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

import { formatBearingDeg, formatSignedDeg } from "../lib/format";
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
import { StaleMark } from "../components/StaleMark";
import { AxisTriad } from "./AxisTriad";
import { AXIS_FALLBACK_MESSAGE, CoreArrow } from "./CoreArrow";
import { buildDrawList, type ScreenPoint } from "./drawList";
import {
  type BoxPx,
  coreArrowBoxes,
  coreArrowLayout,
  coreLabelText,
  placeCurveLabels,
  TRIAD_BOX_REM,
  triadBoxRem as triadBoxOf,
  triadFootprintPx,
  triadLayout,
} from "./furniture";
import { chooseLabels, placeLabels } from "./labels";
import type { LocalFrame } from "./frame";
import type { SpatialScene } from "./marks";
import { type ColourTokens, paint, readTokens, sameTokens, staleTokens } from "./paint";
import { pick } from "./pick";
import { PRESET_CONTROLS, PresetButtons } from "./PresetButtons";
import { Reading, type SpatialQuantity, type SpatialReading } from "./Reading";
import type { ScaleUnit } from "./scale";
import { ScaleBar } from "./ScaleBar";
import { type CameraMove, useOrbitCamera } from "./useOrbitCamera";
import { usePointerOrbit } from "./usePointerOrbit";
import { useThrottledValue } from "./useThrottledValue";

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

/**
 * The view's keys, shown above it and describing its canvas. The preset views' keys are shown on
 * their buttons, and are not repeated here.
 */
const KEY_LEGEND: ReadonlyArray<string> = ["ARROWS ROTATE", "+/− ZOOM", "Z FIT"];

/** The shortest time between two changes of the azimuth and elevation readouts: 4 Hz. */
const READOUT_INTERVAL_MS = 250;

/** The longest the scale bar may be, in `rem`. */
const SCALE_BAR_MAX_REM = 6;

/** The width of the azimuth and elevation values, in characters: `000°` and `+90°`. */
const ANGLE_WIDTH_CH = 4;

/**
 * How many steps an arrow key turns the camera by. The keys turn it as a drag does, as if the scene
 * were taken by its near side: right raises the azimuth, and down raises the camera.
 */
interface ArrowTurn {
  readonly azimuthSteps: number;
  readonly elevationSteps: number;
}

const ARROW_TURNS: Readonly<Record<string, ArrowTurn>> = {
  ArrowLeft: { azimuthSteps: -1, elevationSteps: 0 },
  ArrowRight: { azimuthSteps: 1, elevationSteps: 0 },
  ArrowUp: { azimuthSteps: 0, elevationSteps: -1 },
  ArrowDown: { azimuthSteps: 0, elevationSteps: 1 },
};

/** The first number in a label, with its sign, grouping and decimals. */
const FIRST_NUMBER = /[+\-−]?\d[\d,.]*/u;

interface FiguresProps {
  readonly text: string;
}

/** A label's text with its numbers in monospaced figures, as the console sets every number. */
function Figures({ text }: FiguresProps) {
  const found = FIRST_NUMBER.exec(text);
  if (found === null) {
    return text;
  }
  const end = found.index + found[0].length;
  return (
    <>
      {text.slice(0, found.index)}
      <span className="spatial-label__figures">{found[0]}</span>
      <Figures text={text.slice(end)} />
    </>
  );
}

/** A `transform` that moves an element from the top left to a point given in `rem`. */
function translateRem(leftRem: number, topRem: number): string {
  return `translate(${leftRem}rem, ${topRem}rem)`;
}

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
  /**
   * Changing it returns the view to the zoom that fits `fitRadius`, as `Z` does: a zoom preset
   * changes it with each press, so that the preset fits even after the operator has zoomed. Absent,
   * a new radius keeps a zoom the operator chose, as the star chart's does.
   */
  readonly fitRequest?: number | undefined;
  /**
   * Called as the operator leaves the fitted zoom by hand, with `false` for a zoom key, the wheel
   * or a pinch, and returns to it, with `true` for `Z`, so that a display's zoom preset shows
   * whether the view still matches it (the orchestrator's ruling 59.6). Absent, as on the star
   * chart, nothing is told.
   */
  readonly onFitChange?: ((fitting: boolean) => void) | undefined;
  /** Writes a length in scene units with its unit, for the scale bar: `20 ly`, `500 AU`. */
  readonly formatLength: (length: number) => string;
  /**
   * The units the scale bar may be read in, largest first, as {@link formatLength} writes them;
   * the scene's own unit alone when absent.
   */
  readonly scaleUnits?: ReadonlyArray<ScaleUnit> | undefined;
  /** The reference frame's name, shown with the view: `GALACTIC`. */
  readonly frameName: string;
  /**
   * The view centre's coordinates in the frame, shown with the view: `RADIUS 26,000.0 ly`,
   * `ANGLE 045.0°`, `HEIGHT +12.0 ly`.
   */
  readonly centre: ReadonlyArray<SpatialReading>;
  /** The time the scene shows, labelled with its time system: `UT +0.00 yr`. */
  readonly time: SpatialReading;
  /**
   * The distance from the view centre to the galactic axis, for the core arrow: the same quantity
   * as the centre's `RADIUS`, written the same way (`26,000.0 ly`).
   */
  readonly coreDistance: SpatialQuantity;
  /**
   * The galactic directions at the view centre, for the axis triad and the core arrow, where the
   * scene's frame is tilted to them: the orbit map's, whose reference plane is a system's own
   * (plan 14, D21, and the orchestrator's ruling 33). The scene's frame's own when absent, as on
   * the star chart, whose frame is the galaxy's.
   */
  readonly axes?: LocalFrame | undefined;
  /** The accessible name of the view's canvas; `, stale` is added to it while `stale` holds. */
  readonly accessibleName: string;
  /**
   * Whether the scene is a snapshot its source no longer backs, as after the link is lost: it is
   * then drawn in `--text-muted` with a trailing `S`, the guide's stale state.
   */
  readonly stale: boolean;
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
 * plane and spheres, painted on a canvas, with its orientation, scale, frame, centre and time
 * around it (plan 05, T10).
 *
 * @remarks
 * The view owns its camera, which starts at the oblique preset at the zoom that fits a sphere of
 * `fitRadius` (plan 05, design note D16). With the canvas focused the arrow keys turn it 5° a
 * press, 1° with `Shift`, as a drag does: right raises the azimuth and down raises the camera. From
 * anywhere on the display but a text field `+` and `=` zoom in and `-` and `−` out by 1.25, and `Z`
 * fits again. The preset buttons `TOP`, `SIDE`, `FRONT` and `OBLIQUE`, or their keys `T`, `S`, `F`
 * and `O`, turn the camera to their view over 120 ms, or at once under reduced motion, keeping the
 * zoom. Zoom stays within 0.5 to 100 times the scale that fits. A drag turns the camera, the wheel
 * and a pinch zoom it, and a click picks the nearest mark (see {@link usePointerOrbit}). All input
 * is gathered into one frame. Its container is measured through a `ResizeObserver`; the canvas's
 * backing store follows the device pixel ratio, and sizes in `rem` follow the root font size. The
 * draw list is built during render from the scene, the camera and the viewport, and a layout
 * effect paints it whenever one of them or the colour tokens change. Nothing runs on a loop: an
 * idle view paints nothing.
 *
 * Around the canvas, as the guide's 3D conventions require: the azimuth and elevation (`AZM 030°`,
 * `ELV +30°`), changing at most four times a second and not announced as they change; over the
 * canvas, the axis triad, the core arrow, a 1-2-5 scale bar, and the labels of the chosen marks
 * and of the spheres and rings, all DOM text, since the canvas carries none (D15); and below it,
 * in the same place on every spatial view, the frame, the centre and the time. The labels are
 * hidden from assistive technology, since the list and the readout beside the view carry the same
 * text. The canvas takes focus and is described by the visible key legend.
 *
 * The camera, the plane, the grid, the stalks, the fill rule and the preset views all follow
 * `scene.frame`. The triad and the core arrow follow `axes` where it is given, and so does the
 * note that the galactic directions are undefined on the axis, which is about them.
 */
export function SpatialView({
  scene,
  fitRadius,
  fitRequest,
  onFitChange,
  formatLength,
  scaleUnits,
  frameName,
  centre,
  time,
  coreDistance,
  axes,
  accessibleName,
  stale,
  onSelect,
  children,
}: SpatialViewProps) {
  const legendId = useId();
  const anglesId = useId();
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
  const {
    camera: cameraState,
    move: moveCamera,
    turnTo,
  } = useOrbitCamera(fittedPxPerUnit, fitRequest);
  // Every move the operator makes goes through here, from the keys, the wheel and a pinch alike, so
  // that a zoom made by hand is reported from the event that made it, never from an effect.
  const move = useCallback(
    (next: CameraMove): void => {
      moveCamera(next);
      if (onFitChange === undefined || fittedPxPerUnit === null) {
        return;
      }
      if (next.kind === "zoom") {
        onFitChange(false);
      } else if (next.kind === "fit") {
        onFitChange(true);
      }
    },
    [moveCamera, onFitChange, fittedPxPerUnit],
  );
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
  const shownAngles = useThrottledValue(cameraState.angles, READOUT_INTERVAL_MS);

  // The marks to label depend on the scene alone, and choosing them sorts every mark: a few
  // thousand on a chart, which a drag would otherwise sort again at every frame.
  const chosenMarks = useMemo(
    () => chooseLabels(scene.points, scene.selectedId, scene.destinationId),
    [scene],
  );
  const pinnedIds = [scene.selectedId, scene.destinationId].filter(
    (id): id is string => id !== null,
  );
  const availableIds = new Set(
    chosenMarks.filter((mark) => mark.status === "available").map((mark) => mark.id),
  );
  // The overlay's furniture: the triad in its corner, the core arrow short of it, and the curve
  // labels round both.
  // The triad is drawn in the view where the view is smaller than its usual box, since the overlay
  // clips what leaves it and the guide has a 3D view always show its triad.
  const shownAxes = axes ?? scene.frame;
  const triadBoxRem = viewport === null ? TRIAD_BOX_REM : triadBoxOf(viewport);
  const triad = triadLayout(scene.frame, cameraState.angles, triadBoxRem, shownAxes);
  const triadBox: BoxPx | null =
    viewport === null ? null : triadFootprintPx(triad, viewport, triadBoxRem);
  const coreArrow =
    viewport === null || triadBox === null
      ? null
      : coreArrowLayout(
          scene.frame,
          cameraState.angles,
          viewport,
          coreLabelText(coreDistance),
          [triadBox],
          shownAxes,
        );
  const curveLabels =
    drawList === null || viewport === null || triadBox === null || coreArrow === null
      ? []
      : placeCurveLabels(drawList.curveLabels, viewport, [
          triadBox,
          ...coreArrowBoxes(coreArrow, viewport.remPx),
        ]);
  // Marks' labels, placed last, keep off the furniture and the curve labels.
  const markLabels =
    drawList === null || viewport === null || triadBox === null || coreArrow === null
      ? []
      : placeLabels(chosenMarks, drawList.anchors, viewport, pinnedIds, [
          triadBox,
          ...coreArrowBoxes(coreArrow, viewport.remPx),
          ...curveLabels,
        ]);

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

  // A stale scene is painted in `--text-muted`, as a stale map's picture is ramped to it (T8.d).
  const paintTokens = useMemo(
    () => (tokens === null || !stale ? tokens : staleTokens(tokens)),
    [tokens, stale],
  );

  const backingWidthPx = Math.round(widthPx * pixelRatio);
  const backingHeightPx = Math.round(heightPx * pixelRatio);
  // A new backing size, which clears the canvas, comes with a new viewport and so a new draw list,
  // or with a new pixel ratio: either paints again.
  useLayoutEffect(() => {
    const context = canvasRef.current?.getContext("2d") ?? null;
    if (context === null || drawList === null || paintTokens === null) {
      return;
    }
    paint(context, drawList, paintTokens, pixelRatio);
  }, [drawList, paintTokens, pixelRatio]);

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
    <div className={stale ? "spatial-view spatial-view--stale" : "spatial-view"}>
      <div className="spatial-view__controls">
        <PresetButtons angles={cameraState.angles} onChoose={choosePreset} />
        <dl className="spatial-view__angles" id={anglesId}>
          <Reading
            reading={{
              label: "AZM",
              value: formatBearingDeg(shownAngles.azimuthDeg),
              unit: "",
              widthCh: ANGLE_WIDTH_CH,
            }}
          />
          <Reading
            reading={{
              label: "ELV",
              value: formatSignedDeg(shownAngles.elevationDeg),
              unit: "",
              widthCh: ANGLE_WIDTH_CH,
            }}
          />
        </dl>
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
          aria-label={stale ? `${accessibleName}, stale` : accessibleName}
          // The camera's angles and the keys, read when the canvas takes focus, not as they change.
          aria-describedby={`${anglesId} ${legendId}`}
          onKeyDown={onCanvasKeyDown}
          {...pointerHandlers}
        />
        <div className="spatial-view__overlay">
          <div className="spatial-view__labels" aria-hidden="true">
            {viewport === null
              ? null
              : curveLabels.map((label) => (
                  <span
                    key={label.key}
                    className="spatial-label spatial-label--curve"
                    style={{
                      transform: translateRem(
                        label.leftPx / viewport.remPx,
                        label.topPx / viewport.remPx,
                      ),
                    }}
                  >
                    <Figures text={label.text} />
                  </span>
                ))}
            {viewport === null
              ? null
              : markLabels.map((label) => (
                  <span
                    key={label.id}
                    className={
                      availableIds.has(label.id)
                        ? "spatial-label spatial-label--available"
                        : "spatial-label"
                    }
                    style={{
                      transform: translateRem(
                        label.leftPx / viewport.remPx,
                        label.topPx / viewport.remPx,
                      ),
                    }}
                  >
                    {label.text}
                  </span>
                ))}
          </div>
          <AxisTriad
            frame={scene.frame}
            angles={cameraState.angles}
            boxRem={triadBoxRem}
            axes={shownAxes}
          />
          {viewport === null || coreArrow === null ? null : (
            <CoreArrow layout={coreArrow} viewport={viewport} distance={coreDistance} />
          )}
          {children}
        </div>
      </div>
      <div className="spatial-view__furniture">
        <dl className="spatial-view__readings">
          <div className="field spatial-reading">
            <dt className="field__label">FRAME</dt> <dd>{frameName}</dd>
          </div>
          {centre.map((reading) => (
            <Reading key={reading.label} reading={reading} />
          ))}
          <Reading reading={time} />
        </dl>
        {camera === null || viewport === null ? null : (
          <ScaleBar
            pxPerUnit={camera.pxPerUnit}
            maxBarPx={SCALE_BAR_MAX_REM * viewport.remPx}
            formatLength={formatLength}
            units={scaleUnits}
          />
        )}
        {stale ? <StaleMark /> : null}
        {shownAxes.onAxis ? (
          <p className="spatial-view__axis-note">{AXIS_FALLBACK_MESSAGE}</p>
        ) : null}
      </div>
    </div>
  );
}
