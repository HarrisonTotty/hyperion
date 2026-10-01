import {
  type KeyboardEvent,
  memo,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import { StatusLine, type StatusStanding } from "../../components/StatusLine";
import type { BodyDistanceUnit } from "../../lib/format";
import { type ElementSize, useElementSize } from "../../lib/useElementSize";
import { usePrefersReducedMotion } from "../../lib/usePrefersReducedMotion";
import type { Anchor } from "../../spatial/drawList";
import { type ColourTokens, readTokens } from "../../spatial/paint";
import { pick } from "../../spatial/pick";
import { useThrottledValue } from "../../spatial/useThrottledValue";
import {
  flightKey,
  flightKeyAction,
  flightKeyReleased,
  type ViewKeyAction,
  viewKeyAction,
} from "../../view/camera/keys";
import { type CameraTarget, offeredPresets } from "../../view/camera/state";
import {
  type GraphicsAnnunciation,
  graphicsAnnunciation,
  useGraphicsStatus,
} from "../../view/engine/status";
import type { ViewSize } from "../../view/engine/types";
import {
  controlEv100,
  DEFAULT_EXPOSURE,
  type ExposureControl,
} from "../../view/photometry/exposure";
import { cameraSceneOf } from "../../view/scene/model";
import { buildWireframeDrawList, type DrawAnchor } from "../../view/wireframe/drawList";
import { WireframeRenderer } from "../../view/wireframe/submit";
import { CameraControls } from "./CameraControls";
import { ExposurePanel } from "./ExposurePanel";
import { DEFAULT_ENGINE_SOURCE, useViewEngine, type ViewEngineSource } from "./useViewEngine";
import { ViewCanvas } from "./ViewCanvas";
import { ViewLabelBlock } from "./ViewLabelBlock";
import { ViewMarkLabels } from "./ViewMarkLabels";
import { ViewMarkList } from "./ViewMarkList";
import {
  commandRun,
  labelLines,
  labelStatements,
  type MarkRow,
  markRows,
  PRESET_NAMES,
  runPose,
  SCENE_OPTIONS,
  type SceneOption,
  startRun,
  stepRun,
  targetKey,
  type ViewRun,
} from "./viewRun";

/** The shortest time between two changes of the view's readouts: 4 Hz (Design note 18). */
const READOUT_INTERVAL_MS = 250;

/** The keys of the canvas, shown beside it and describing it. */
const KEY_LEGEND = "W/S A/D R/F MOVE · ARROWS Q/E TURN · PAGE UP/DOWN RATE";

/** The pointer's reach to a mark, rem: half the guide's 2 rem target, as plan 05's pick has it. */
const PICK_REM = 1;

/** The engine's standing while it is not ready: being made, until an adapter answers. */
const ACQUIRING: { readonly text: string; readonly standing: StatusStanding } = {
  text: "GRAPHICS ACQUIRING ADAPTER",
  standing: "waiting",
};

/** Where the view could not be made and the graphics status gives no cause of its own. */
const NOT_MADE: GraphicsAnnunciation = {
  text: "GRAPHICS NOT AVAILABLE: views could not be made, relaunch to retry",
  standing: "fault",
};

/** Props of {@link ViewDisplay}. */
interface ViewDisplayProps {
  /** Where the engine comes from: R01's by default, a fake in a test. */
  readonly engineSource?: ViewEngineSource | undefined;
}

interface ViewStageProps {
  readonly option: SceneOption;
  readonly engineSource: ViewEngineSource;
  readonly exposure: ExposureControl;
  readonly onExposureChange: (exposure: ExposureControl) => void;
  readonly easedMoves: boolean;
  readonly onEasedMovesChange: (easedMoves: boolean) => void;
}

/** What the drawing loop reads of the display, kept current by an effect. */
interface LoopInputs {
  readonly exposure: ExposureControl;
  readonly selection: CameraTarget | null;
  readonly reducedMotion: boolean;
  readonly size: ElementSize | null;
  readonly tokens: ColourTokens | null;
}

/** What the loop publishes for the DOM, at most every {@link READOUT_INTERVAL_MS}. */
interface Published {
  readonly run: ViewRun;
  /** The marks where the frame drew them, device px. */
  readonly anchors: ReadonlyArray<DrawAnchor>;
}

/** A draw-list anchor as plan 05's `pick` reads it, its target's key its ID. */
function pickAnchor(anchor: DrawAnchor): Anchor {
  return {
    id: targetKey(anchor.target),
    xPx: anchor.xPx,
    yPx: anchor.yPx,
    depth: anchor.distanceM,
    radiusPx: 0,
  };
}

/** The rows of the list, with the units they were last shown in kept for their hysteresis. */
interface ShownRows {
  readonly run: ViewRun;
  readonly rows: ReadonlyArray<MarkRow>;
}

function unitsOf(rows: ReadonlyArray<MarkRow>): ReadonlyMap<string, BodyDistanceUnit> {
  return new Map(rows.map((row) => [row.key, row.unit]));
}

function ViewStage({
  option,
  engineSource,
  exposure,
  onExposureChange,
  easedMoves,
  onEasedMovesChange,
}: ViewStageProps) {
  const legendId = useId();
  const [initial] = useState(() => startRun(option.make()));
  const runRef = useRef<ViewRun>(initial);
  const [published, setPublished] = useState<Published>({ run: initial, anchors: [] });
  const shown = useThrottledValue(published, READOUT_INTERVAL_MS);
  const [selection, setSelection] = useState<CameraTarget | null>(null);
  const reducedMotion = usePrefersReducedMotion();
  const engineState = useViewEngine(engineSource);
  const annunciation = graphicsAnnunciation(useGraphicsStatus());
  const { ref: stageRef, size } = useElementSize();
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null);
  const heldRef = useRef(new Set<string>());
  const inputsRef = useRef<LoopInputs>({
    exposure,
    selection,
    reducedMotion,
    size,
    tokens: null,
  });

  // The list's ranges switch unit with hysteresis, from the units they were last shown in;
  // adjusted during render as each published run arrives.
  const [shownRows, setShownRows] = useState<ShownRows>(() => ({
    run: initial,
    rows: markRows(initial),
  }));
  let rows = shownRows.rows;
  if (shownRows.run !== shown.run) {
    rows = markRows(shown.run, unitsOf(shownRows.rows));
    setShownRows({ run: shown.run, rows });
  }

  // The loop reads the display through a ref, current after every commit; the colour tokens are
  // read from the canvas when it mounts and each time the display is shown again.
  useLayoutEffect(() => {
    inputsRef.current = {
      exposure,
      selection,
      reducedMotion,
      size,
      tokens: canvas === null ? null : readTokens(canvas),
    };
  }, [exposure, selection, reducedMotion, size, canvas]);

  // The redraw loop: every frame while the display is shown; `Activity` tears it down when hidden.
  useEffect(() => {
    if (engineState.kind !== "ready" || canvas === null) {
      return undefined;
    }
    const { engine } = engineState;
    const view = engine.createView(canvas, "view");
    const renderer = new WireframeRenderer(engine);
    let lastMs: number | null = null;
    let publishedMs = Number.NEGATIVE_INFINITY;
    let sized: ViewSize | null = null;
    let anchors: ReadonlyArray<DrawAnchor> = [];
    let frame = 0;
    const tick = (nowMs: number): void => {
      const dtS = lastMs === null ? 0 : (nowMs - lastMs) / 1000;
      lastMs = nowMs;
      const inputs = inputsRef.current;
      const run = stepRun(runRef.current, {
        dtS,
        held: heldRef.current,
        reducedMotion: inputs.reducedMotion,
      });
      runRef.current = run;
      if (inputs.size !== null && inputs.tokens !== null && inputs.size.widthPx > 0) {
        const ratio = inputs.size.devicePixelRatio;
        const viewport = {
          widthPx: Math.max(1, Math.round(inputs.size.widthPx * ratio)),
          heightPx: Math.max(1, Math.round(inputs.size.heightPx * ratio)),
        };
        if (sized?.widthPx !== viewport.widthPx || sized.heightPx !== viewport.heightPx) {
          view.resize(viewport);
          sized = viewport;
        }
        const camera = { pose: runPose(run), fovXRad: (run.camera.fovDeg * Math.PI) / 180 };
        const list = buildWireframeDrawList(run.scene, camera, viewport, inputs.tokens, {
          lowSetting: false,
          ev100: controlEv100(inputs.exposure),
          selection: inputs.selection,
          destination: null,
          remPx: inputs.size.remPx * ratio,
        });
        anchors = list.anchors;
        renderer.render(view, list, camera, viewport);
      }
      if (nowMs - publishedMs >= READOUT_INTERVAL_MS) {
        publishedMs = nowMs;
        setPublished({ run, anchors });
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      renderer.dispose();
      view.dispose();
    };
  }, [engineState, canvas]);

  const command = useCallback(
    (action: ViewKeyAction): void => {
      const result = commandRun(runRef.current, action, { easedMoves, reducedMotion });
      if (result.kind === "refused") {
        return;
      }
      runRef.current = result.run;
      setPublished((previous) => ({ ...previous, run: result.run }));
      if (action.kind === "target") {
        setSelection(result.run.camera.target);
      }
    },
    [easedMoves, reducedMotion],
  );

  // The view's single keys act from anywhere on the display but a text field (keys.ts).
  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent): void => {
      const action = viewKeyAction(event);
      if (action === null) {
        return;
      }
      event.preventDefault();
      command(action);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [command]);

  const onCanvasKeyDown = (event: KeyboardEvent<HTMLCanvasElement>): void => {
    const held = flightKey(event);
    if (held !== null) {
      event.preventDefault();
      heldRef.current.add(held);
      return;
    }
    const action = flightKeyAction(event);
    if (action !== null) {
      event.preventDefault();
      command(action);
    }
  };
  const onCanvasKeyUp = (event: KeyboardEvent<HTMLCanvasElement>): void => {
    const released = flightKeyReleased(event);
    if (released !== null) {
      heldRef.current.delete(released);
    }
  };
  const onCanvasBlur = (): void => {
    heldRef.current.clear();
  };

  const ratio = size?.devicePixelRatio ?? 1;
  const onPick = (xPx: number, yPx: number): void => {
    const remPx = size?.remPx ?? 16;
    const picked = pick(
      shown.anchors.map(pickAnchor),
      { xPx: xPx * ratio, yPx: yPx * ratio },
      PICK_REM * remPx * ratio,
    );
    const row = rows.find((each) => each.key === picked);
    if (row !== undefined) {
      setSelection(row.target);
    }
  };

  // The view stands in its place only once its engine is made; until then, or where it cannot
  // be, the graphics' own annunciation (R01's), or the adapter being acquired.
  const engineLine: { readonly text: string; readonly standing: StatusStanding } | null =
    engineState.kind === "ready"
      ? null
      : engineState.kind === "pending"
        ? ACQUIRING
        : (annunciation ?? NOT_MADE);
  const fault = annunciation?.standing === "fault" ? annunciation.text : null;

  return (
    <div className="view">
      <div className="view__main">
        {engineLine === null ? (
          <>
            <ViewCanvas
              canvasRef={setCanvas}
              stageRef={stageRef}
              accessibleName={`VIEW, WIREFRAME, ${PRESET_NAMES[shown.run.camera.preset]}`}
              describedBy={legendId}
              onKeyDown={onCanvasKeyDown}
              onKeyUp={onCanvasKeyUp}
              onBlur={onCanvasBlur}
              onPick={onPick}
            >
              <ViewMarkLabels anchors={shown.anchors} devicePixelRatio={ratio} rows={rows} />
              <ViewLabelBlock
                lines={labelLines(shown.run, exposure)}
                statements={labelStatements(shown.run)}
                fault={fault}
              />
            </ViewCanvas>
            <p className="view__keys" id={legendId}>
              {KEY_LEGEND}
            </p>
          </>
        ) : (
          <div className="view__unavailable">
            <StatusLine text={engineLine.text} standing={engineLine.standing} />
          </div>
        )}
      </div>
      <div className="view__side">
        <section className="panel view-targets" aria-labelledby={`${legendId}-targets`}>
          <h2 className="panel__title" id={`${legendId}-targets`}>
            Targets
          </h2>
          <ViewMarkList
            rows={rows}
            selectedKey={selection === null ? null : targetKey(selection)}
            onSelect={(row) => {
              setSelection(row.target);
            }}
          />
        </section>
        <CameraControls
          preset={shown.run.camera.preset}
          offered={offeredPresets(cameraSceneOf(shown.run.scene))}
          fovDeg={shown.run.camera.fovDeg}
          easedMoves={easedMoves}
          reducedMotion={reducedMotion}
          onAction={command}
          onEasedMovesChange={onEasedMovesChange}
        />
        <ExposurePanel exposure={exposure} meteredEv100={null} onChange={onExposureChange} />
      </div>
    </div>
  );
}

function ViewPanels({ engineSource = DEFAULT_ENGINE_SOURCE }: ViewDisplayProps) {
  const [sceneName, setSceneName] = useState(SCENE_OPTIONS[0]?.name ?? "");
  const [exposure, setExposure] = useState<ExposureControl>(DEFAULT_EXPOSURE);
  const [easedMoves, setEasedMoves] = useState(false);
  const option = SCENE_OPTIONS.find((each) => each.name === sceneName);
  if (option === undefined) {
    throw new Error(`no kept scene named ${sceneName}`);
  }
  return (
    <div className="view-display">
      <div className="view-display__bar">
        <fieldset className="preset-buttons" aria-label="Scene">
          <span className="field__label">SCENE</span>
          {SCENE_OPTIONS.map((each) => (
            <button
              key={each.name}
              type="button"
              className="control preset-buttons__button"
              aria-pressed={each.name === sceneName}
              onClick={() => {
                setSceneName(each.name);
              }}
            >
              {each.name}
            </button>
          ))}
        </fieldset>
      </div>
      <ViewStage
        key={option.name}
        option={option}
        engineSource={engineSource}
        exposure={exposure}
        onExposureChange={setExposure}
        easedMoves={easedMoves}
        onEasedMovesChange={setEasedMoves}
      />
    </div>
  );
}

/**
 * The `VIEW` display (plan R02, R02.T15): the surroundings drawn as a wireframe in perspective at
 * real scale, from a seat, chase or free camera, with its list of targets, its label block, its
 * camera controls and its exposure instrument.
 *
 * @remarks
 * Until R02.T17 brings the server's scene, it draws a kept test scene chosen under `SCENE`,
 * `PRECISION TEST` or `FRAME CHANGE TEST`, under the header strip's `TRAINING` banner, which `App`
 * shows while this display is; a new choice starts its script and camera afresh, keeping the
 * exposure and the `EASED CAMERA MOVES` setting. The engine is loaded lazily through R01's
 * `loadRenderEngine` when the display is first shown; until it is ready, or where it cannot be
 * had, the graphics' own annunciation stands in the view's place. The view redraws every frame
 * while the display is shown and stops when it is hidden (`Activity` tears its loop down); its
 * readouts change at most four times a second. A preset change and a slew to a target are cuts,
 * eased over 0.4 s only under the `EASED CAMERA MOVES` setting and never under
 * `prefers-reduced-motion`, which also removes the free camera's ramp and damping (Design note 18).
 *
 * Keys, from anywhere on the display but a text field: `1` `2` `3` the presets, `]` and `[` the
 * next and previous target, `+` and `-` the field of view; on the focused canvas the flight keys
 * (W/S, A/D, R/F, the arrows, Q/E, PageUp/PageDown). A click on the canvas, or the list, selects a
 * mark, whose bracket reticle the view then draws; another craft's mark carries its range and
 * closure rate, and a body drawn as its symbol its designation, as DOM labels over the canvas.
 */
export const ViewDisplay = memo(ViewPanels);
