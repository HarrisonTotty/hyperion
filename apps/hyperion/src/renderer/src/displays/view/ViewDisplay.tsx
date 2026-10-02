import type { UniverseIdHex, UniverseTime } from "@hyperion/protocol";
import {
  type KeyboardEvent,
  memo,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  use,
  useState,
} from "react";

import { StaleMark } from "../../components/StaleMark";
import { StatusLine, type StatusStanding } from "../../components/StatusLine";
import type { BodyDistanceUnit } from "../../lib/format";
import type { SceneFrame } from "../../lib/scene/apparent";
import type { SceneKinematics, SceneModel, SystemPlace } from "../../lib/scene/model";
import { type ElementSize, useElementSize } from "../../lib/useElementSize";
import { useUniverse } from "../../lib/universe";
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
import { type CameraTarget, offeredPresets, type ViewId, viewId } from "../../view/camera/state";
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
import {
  cameraKinematics,
  serverSceneAtFrame,
  serverSceneAtPush,
} from "../../view/scene/fromServer";
import { cameraSceneOf, type ViewStar } from "../../view/scene/model";
import { buildWireframeDrawList, type DrawAnchor } from "../../view/wireframe/drawList";
import { WireframeRenderer } from "../../view/wireframe/submit";
import { CameraControls } from "./CameraControls";
import { ExposurePanel } from "./ExposurePanel";
import {
  cameraAnnunciation,
  serverSceneStanding,
  systemPlace,
  viewProvenance,
} from "./serverScene";
import { type InterimStarsInput, useInterimStars } from "./useInterimStars";
import { DEFAULT_ENGINE_SOURCE, useViewEngine, type ViewEngineSource } from "./useViewEngine";
import { ViewCanvas } from "./ViewCanvas";
import { ViewLabelBlock } from "./ViewLabelBlock";
import { ViewMarkLabels } from "./ViewMarkLabels";
import { ViewMarkList } from "./ViewMarkList";
import { ViewSceneContext } from "./ViewSceneProvider";
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
  SERVER_SCENE_NAME,
  startRun,
  startServerRun,
  stepRun,
  targetKey,
  type ViewRun,
} from "./viewRun";

/** The shortest time between two changes of the view's readouts: 4 Hz (Design note 18). */
const READOUT_INTERVAL_MS = 250;

/** The keys of the canvas, shown beside it and describing it. */
const KEY_LEGEND = "W/S A/D R/F MOVE · ARROWS Q/E TURN · PAGE UP/DOWN RATE";

/** The view's identity in the scene's camera reports: one local view, the display's. */
const VIEW_ID: ViewId = viewId("view");

/** No stars, one array for every frame without an answer. */
const NO_STARS: ReadonlyArray<ViewStar> = [];

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

/** What a view of the server's scene reads of `useScene`, kept current by its stage. */
interface ServerInput {
  readonly model: SceneModel;
  readonly place: SystemPlace;
  /** Whether the scene is stale: its time is held, and its readings are muted with their `S`. */
  readonly stale: boolean;
  readonly frameAt: (nowMs: number) => SceneFrame | null;
  readonly reportCamera: (view: ViewId, pose: SceneKinematics) => void;
  readonly removeCamera: (view: ViewId) => void;
}

/** What a stage draws: a kept scene, or the server's. */
type StageSource =
  | { readonly kind: "kept"; readonly option: SceneOption }
  | { readonly kind: "server"; readonly server: ServerInput };

interface ViewStageProps {
  readonly source: StageSource;
  readonly engineSource: ViewEngineSource;
  readonly exposure: ExposureControl;
  readonly onExposureChange: (exposure: ExposureControl) => void;
  readonly easedMoves: boolean;
  readonly onEasedMovesChange: (easedMoves: boolean) => void;
  /** The interim stars (R02.T16) and their count line, or `null` before an answer. */
  readonly stars: ReadonlyArray<ViewStar>;
  readonly countLine: string | null;
}

/** What the drawing loop reads of the display, kept current by an effect. */
interface LoopInputs {
  readonly exposure: ExposureControl;
  readonly selection: CameraTarget | null;
  readonly reducedMotion: boolean;
  readonly size: ElementSize | null;
  readonly tokens: ColourTokens | null;
  /** The interim stars (R02.T16), drawn into every frame's scene. */
  readonly stars: ReadonlyArray<ViewStar>;
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

/**
 * The run a stage starts with: its kept scene's, or the server's scene at its latest push, which
 * the drawing loop then reads at each frame.
 *
 * @throws Error when a stage is given a server scene that cannot be drawn, which `ViewPanels`
 *   never does.
 */
function initialRun(source: StageSource): ViewRun {
  if (source.kind === "kept") {
    return startRun(source.option.make());
  }
  const first = serverSceneAtPush(source.server.model, source.server.place);
  if (first === null) {
    throw new Error("a view was started on a server scene it cannot draw");
  }
  return startServerRun(first);
}

function ViewStage({
  source,
  engineSource,
  exposure,
  onExposureChange,
  easedMoves,
  onEasedMovesChange,
  stars,
  countLine,
}: ViewStageProps) {
  const legendId = useId();
  const server = source.kind === "server" ? source.server : null;
  const serverRef = useRef<ServerInput | null>(null);
  const [initial] = useState(() => initialRun(source));
  const runRef = useRef<ViewRun>(initial);
  const [published, setPublished] = useState<Published>({ run: initial, anchors: [] });
  const shown = useThrottledValue(published, READOUT_INTERVAL_MS);
  const [selection, setSelection] = useState<CameraTarget | null>(null);
  const reducedMotion = usePrefersReducedMotion();
  const engineState = useViewEngine(engineSource);
  const graphics = useGraphicsStatus();
  const annunciation = graphicsAnnunciation(graphics);
  const { ref: stageRef, size } = useElementSize();
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null);
  const heldRef = useRef(new Set<string>());
  const inputsRef = useRef<LoopInputs>({
    exposure,
    selection,
    reducedMotion,
    size,
    tokens: null,
    stars: [],
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
      stars,
    };
  }, [exposure, selection, reducedMotion, size, canvas, stars]);

  // The drawing loop reads the server's scene, as the latest render holds it, through a ref.
  useLayoutEffect(() => {
    serverRef.current = server;
  }, [server]);

  // The view's camera is reported to the server's scene while the stage draws it (R03.T14).
  const removeCamera = server?.removeCamera ?? null;
  useEffect(() => {
    if (removeCamera === null) {
      return undefined;
    }
    return () => {
      removeCamera(VIEW_ID);
    };
  }, [removeCamera]);

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
      const current = serverRef.current;
      const run = stepRun(runRef.current, {
        // The server's scene where the ship sees it at this frame's time (R03's `frameAt`).
        serverScene: current === null ? null : serverSceneAtFrame(current, nowMs),
        dtS,
        held: heldRef.current,
        reducedMotion: inputs.reducedMotion,
      });
      runRef.current = run;
      if (run.source.kind === "server" && current !== null) {
        current.reportCamera(VIEW_ID, cameraKinematics(runPose(run), run.scene));
      }
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
        const scene = { ...run.scene, stars: inputs.stars };
        const list = buildWireframeDrawList(scene, camera, viewport, inputs.tokens, {
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
        : // Still acquiring by the status means the view's own request found nothing.
          annunciation === null || graphics.condition.kind === "acquiring"
          ? NOT_MADE
          : annunciation;
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
              <ViewMarkLabels
                anchors={shown.anchors}
                devicePixelRatio={ratio}
                rows={rows}
                stale={server?.stale === true}
              />
              <ViewLabelBlock
                lines={labelLines(shown.run, exposure, server?.stale === true)}
                statements={
                  countLine === null
                    ? labelStatements(shown.run)
                    : [...labelStatements(shown.run), countLine]
                }
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
            Targets{server?.stale === true ? <StaleMark /> : null}
          </h2>
          <ViewMarkList
            rows={rows}
            stale={server?.stale === true}
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

/**
 * Where the interim stars of a server scene are asked: a stated place's own barycentre and time,
 * as the scene stated them, asked once per arrival and never carried per frame (R03.T16); a charted
 * place's barycentre at the scene's time; nothing for an unknown place.
 */
function interimAt(
  universe: UniverseIdHex | null,
  place: SystemPlace,
  sceneTime: UniverseTime,
): InterimStarsInput {
  let input: InterimStarsInput;
  switch (place.kind) {
    case "stated":
      input = { universe, system: place.system, centre: place.barycentre, time: place.time };
      break;
    case "charted":
      input = { universe, system: place.system, centre: place.barycentre, time: sceneTime };
      break;
    case "unknown":
      input = { universe, system: place.system, centre: null, time: sceneTime };
      break;
  }
  return input;
}

function ViewPanels({ engineSource = DEFAULT_ENGINE_SOURCE }: ViewDisplayProps) {
  const statusId = useId();
  const host = use(ViewSceneContext);
  if (host === null) {
    throw new Error("the VIEW display is rendered outside a ViewSceneProvider");
  }
  const { scene, sceneName, keptName, knownSystem, choose } = host;
  const [exposure, setExposure] = useState<ExposureControl>(DEFAULT_EXPOSURE);
  const [easedMoves, setEasedMoves] = useState(false);
  const universe = useUniverse().open?.id ?? null;
  const standing = serverSceneStanding(scene);
  // The server's scene is chosen by default; a kept scene stands in while it cannot be drawn.
  const serverChosen = sceneName === SERVER_SCENE_NAME;
  const model = scene.model;
  const sceneSystem = model?.system ?? null;
  const server: ServerInput | null =
    viewProvenance(sceneName, scene) === "server" && model !== null && sceneSystem !== null
      ? {
          model,
          place: systemPlace(sceneSystem.model.system, sceneSystem.place, knownSystem),
          stale: standing.stale,
          frameAt: scene.frameAt,
          reportCamera: scene.reportCamera,
          removeCamera: scene.removeCamera,
        }
      : null;
  const option = SCENE_OPTIONS.find((each) => each.name === (serverChosen ? keptName : sceneName));
  if (option === undefined) {
    throw new Error(`no kept scene named ${serverChosen ? keptName : sceneName}`);
  }

  // Where the scene starts, kept for its identity: the interim stars are asked about its system
  // from here, above the stage that a new scene remounts, so that a scene in the same system asks
  // nothing again (Design note 19).
  const keptStart = useMemo(() => option.make().sceneAt(0), [option]);
  const interim = useInterimStars(
    server === null
      ? {
          universe,
          system: keptStart.system,
          centre: keptStart.barycentre,
          time: keptStart.time,
        }
      : interimAt(universe, server.place, server.model.clock.time),
  );
  const lines = [
    ...(serverChosen && standing.annunciation !== null ? [standing.annunciation] : []),
    ...(server !== null && scene.cameraFault !== null
      ? [cameraAnnunciation(scene.cameraFault)]
      : []),
  ];
  return (
    <div className="view-display">
      <div className="view-display__bar">
        <fieldset className="preset-buttons" aria-label="Scene">
          <span className="field__label">SCENE</span>
          {[SERVER_SCENE_NAME, ...SCENE_OPTIONS.map((each) => each.name)].map((name) => (
            <button
              key={name}
              type="button"
              className="control preset-buttons__button"
              aria-pressed={name === sceneName}
              aria-describedby={
                name === SERVER_SCENE_NAME && lines.length > 0 ? statusId : undefined
              }
              onClick={() => {
                choose(name);
              }}
            >
              {name}
            </button>
          ))}
        </fieldset>
        {lines.length === 0 ? null : (
          <div className="view-display__status" id={statusId}>
            {lines.map((line) => (
              <StatusLine
                key={line.text}
                text={line.text}
                standing={line.standing}
                action={line.retry === true ? { label: "RETRY", onAction: scene.retry } : undefined}
              />
            ))}
          </div>
        )}
      </div>
      <ViewStage
        key={server === null ? option.name : SERVER_SCENE_NAME}
        source={server === null ? { kind: "kept", option } : { kind: "server", server }}
        engineSource={engineSource}
        exposure={exposure}
        onExposureChange={setExposure}
        easedMoves={easedMoves}
        onEasedMovesChange={setEasedMoves}
        stars={interim.field?.stars ?? NO_STARS}
        countLine={interim.countLine}
      />
    </div>
  );
}

/**
 * The `VIEW` display (plan R02, R02.T15 and T17): the surroundings drawn as a wireframe in
 * perspective at real scale, from a seat, chase or free camera, with its list of targets, its label
 * block, its camera controls and its exposure instrument.
 *
 * @remarks
 * It draws the scene chosen under `SCENE`: `SERVER`, the server's scene of the open universe from
 * R03's subscription (`useScene`), by default, or a kept test scene, `PRECISION TEST` or
 * `FRAME CHANGE TEST`; the subscription and the choice are held above the console frame by
 * `ViewSceneProvider`, which it must be rendered in, so that `App`'s header strip shows `TRAINING`
 * over a kept scene only. While the server's scene cannot be drawn (no universe open, the
 * subscription pending, refused or unanswered, the link down before a scene arrived, or the ship in
 * no system) the kept scene last chosen is drawn in its place, and a status line beside the
 * selector says why, offering `RETRY` after a refusal or a timeout. A server scene held through a
 * stale period (the link down, the scene reopened after the server ended it, or nothing received
 * for twice the heartbeat) stays drawn with its time held, its time and every target's range and
 * closure muted with their `S`, and the status line says why. The view's camera is reported to the
 * scene at each frame (the reporter sends at 4 Hz and at once on a change of frame). A new choice
 * starts its scene and camera afresh, keeping the exposure and the `EASED CAMERA MOVES` setting;
 * a server scene arriving in another system moves a free camera as a jump does.
 *
 * The engine is loaded lazily through R01's `loadRenderEngine` when the display is first shown;
 * until it is ready, or where it cannot be had, the graphics' own annunciation stands in the view's
 * place. The view redraws every frame while the display is shown and stops when it is hidden
 * (`Activity` tears its loop and its subscription down); its readouts change at most four times a
 * second. A preset change and a slew to a target are cuts, eased over 0.4 s only under the
 * `EASED CAMERA MOVES` setting and never under `prefers-reduced-motion`, which also removes the
 * free camera's ramp and damping (Design note 18).
 *
 * Keys, from anywhere on the display but a text field: `1` `2` `3` the presets, `]` and `[` the
 * next and previous target, `+` and `-` the field of view; on the focused canvas the flight keys
 * (W/S, A/D, R/F, the arrows, Q/E, PageUp/PageDown). A click on the canvas, or the list, selects a
 * mark, whose bracket reticle the view then draws. Its stars are the interim field of the open
 * universe's range queries about the scene's system (R02.T16), with their count line, asked only
 * where the system's position is known; another craft's mark carries its range and closure rate,
 * and a body drawn as its symbol its designation, as DOM labels over the canvas.
 */
export const ViewDisplay = memo(ViewPanels);
