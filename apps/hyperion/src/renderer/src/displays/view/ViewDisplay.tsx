import type { UniverseIdHex, UniverseTime } from "@hyperion/protocol";
import {
  type FocusEvent,
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

import { DisclosureGlyph } from "../../components/DisclosureGlyph";
import { StaleMark } from "../../components/StaleMark";
import { StatusLine, type StatusStanding } from "../../components/StatusLine";
import { StatusText } from "../../components/StatusText";
import type { BodyDistanceUnit } from "../../lib/format";
import type { SceneFrame } from "../../lib/scene/apparent";
import type { SceneKinematics, SceneModel, SystemPlace } from "../../lib/scene/model";
import { type ElementSize, useElementSize } from "../../lib/useElementSize";
import { useUniverse } from "../../lib/universe";
import { usePrefersReducedMotion } from "../../lib/usePrefersReducedMotion";
import type { Anchor } from "../../spatial/drawList";
import { type ColourTokens, readTokens } from "../../spatial/paint";
import { pick } from "../../spatial/pick";
import type { ScreenBoxPx } from "../../spatial/symbols";
import { useThrottledValue } from "../../spatial/useThrottledValue";
import { maxFreeRateStep } from "../../view/camera/freeCamera";
import { BudgetedScale, drawsInFrame, PrimaryFrameTimes } from "../../view/budget/framePacing";
import {
  INSTRUMENT_RATE_HZ,
  photorealisticAllowed,
  type ViewBudget,
  viewBudgets,
  type ViewSpec,
} from "../../view/budget/viewBudget";
import {
  flightKey,
  flightKeyAction,
  flightKeyReleased,
  heldAfterModifier,
  type ViewKeyAction,
  viewKeyAction,
} from "../../view/camera/keys";
import { addTurns, NO_TURN, type ViewTurn } from "../../view/camera/look";
import { type PrimaryModifier, primaryModifierOf } from "../../lib/platform";
import {
  type CameraTarget,
  offeredPresets,
  type RenderStyle,
  type ViewId,
} from "../../view/camera/state";
import {
  type GpuTimer,
  type GraphicsAnnunciation,
  graphicsAnnunciation,
  useGraphicsStatus,
} from "../../view/engine/status";
import type { RenderView } from "../../view/engine/types";
import {
  controlEv100,
  DEFAULT_EXPOSURE,
  type ExposureControl,
  exposureLevelReading,
  VIEW_CAMERA,
} from "../../view/photometry/exposure";
import {
  cameraKinematics,
  reportBondRatioFindings,
  serverSceneAtFrame,
  serverSceneAtPush,
} from "../../view/scene/fromServer";
import { cameraSceneOf, type ViewStar } from "../../view/scene/model";
import type { DrawAnchor, DrawnSurveyEdge, LineOfSightPoint } from "../../view/wireframe/drawList";
import type { StyleAvailability } from "../../view/engine/platform";
import { hostLights, lightingState, sceneHostDiscs } from "../../view/lighting/hostLights";
import type { PhotorealStatus } from "../../view/photoreal/renderer";
import { AutoExposure, type Metering, meteringFor } from "../../view/post/autoExposure";
import type { MeterMode } from "../../view/post/meter";
import { CameraControls, NO_OWN_SHIP } from "./CameraControls";
import { placeLines, useCameraPlace } from "./cameraPlace";
import { KeyLegend } from "./KeyLegend";
import { type DragKind, dragKindOf } from "./useViewDrag";
import { litLabelsOf } from "./photorealFrame";
import { StyleControl } from "./StyleControl";
import {
  availabilityOf,
  NO_VIEW_REFUSALS,
  type StyleRefusals,
  styleRefusals,
  withPermission,
} from "./styleRefusals";
import { InstrumentControls } from "./InstrumentControls";
import { InstrumentsPanel, type InstrumentsPanelSlot } from "./InstrumentsPanel";
import { InstrumentView } from "./InstrumentView";
import { type Instrument, type InstrumentsFrame, useInstruments } from "./useInstruments";
import {
  type InstrumentSlot,
  instrumentViewId,
  PRIMARY_NAME,
  PRIMARY_VIEW_ID,
  PRIMARY_VIEW_NAME,
} from "./viewNames";
import { autoNotAvailable, ExposurePanel, exposureNote } from "./ExposurePanel";
import { MeterControl, meterLabel } from "./MeterControl";
import {
  cameraAnnunciation,
  serverSceneStanding,
  systemPlace,
  viewProvenance,
} from "./serverScene";
import { type InterimStarsInput, useInterimStars } from "./useInterimStars";
import { type DrawnSky, primarySkyPlace, type SkyLineReading, useViewSky } from "./useViewSky";
import { QUALITY_NAMES, type QualitySetting } from "../../view/quality/qualitySetting";
import {
  DEFAULT_ENGINE_SOURCE,
  useViewEngine,
  type ViewEngineSource,
  type ViewEngineState,
} from "./useViewEngine";
import { ViewCanvas } from "./ViewCanvas";
import {
  makeViewFrameDrawer,
  terrainReportOf,
  type ViewFrameDrawer,
  type ViewFrameInputs,
} from "./viewFrameDrawer";
import { ViewLabelBlock } from "./ViewLabelBlock";
import { ViewLowerPlate } from "./ViewLowerPlate";
import { InstrumentStub } from "./InstrumentStub";
import { measureRoom, NOT_MEASURED, type RoomMeasures, sameRoomMeasures } from "./plateMeasures";
import { BESIDE_CLASS, ViewRoomSizers } from "./ViewRoomSizers";
import {
  INSTRUMENT_SLOT_REM,
  type RoomInput,
  roomToOpen,
  sameRoom,
  type SlotRoomInput,
  type SlotStanding,
  STUB_HEIGHT_REM,
  type ViewRoom,
  viewRoom,
  wholeRoom,
} from "./viewRoom";
import { PoseIntervals } from "../../view/terrain/poseInterval";
import {
  type MarkLabelState,
  markLabelPlaces,
  markLabelTransform,
  type PlateSizePx,
  ViewMarkLabels,
} from "./ViewMarkLabels";
import { styleName } from "../../view/photoreal/style";
import { isTerrainFault, type TerrainAnnunciation } from "../../view/terrain/annunciation";
import { TerrainFailureGuard } from "../../view/terrain/failureGuard";
import type { TerrainSession } from "../../view/terrain/session";
import type { WorkerLike } from "../../view/terrain/workers/pool";
import { useServerLink } from "../../lib/serverLink";
import { faultsFor } from "./viewFaults";
import {
  browserHeightWorker,
  stepTerrainSession,
  terrainSessionSource,
  useTerrainFaults,
  useTerrainSession,
} from "./useTerrainSession";
import { ViewMarkList } from "./ViewMarkList";
import {
  GroundReadouts,
  type GroundSnapshot,
  lineOfSightPointOf,
  readoutBodyOf,
} from "./groundReadout";
import { terrainLowerParts } from "./terrainPlate";
import { standingReadout, useTerrainLines } from "./useTerrainLines";
import { ViewSceneContext } from "./ViewSceneProvider";
import {
  DEFAULT_FOLD,
  FOLD_BUTTONS,
  type FoldPanel,
  type SideFolds,
  toggledFold,
  viewLayout,
} from "./viewLayout";
import {
  commandRun,
  EASED_MOVES_STATEMENT,
  type LabelLine,
  photorealStatements,
  WIREFRAME_ONLY,
  labelLines,
  labelStatements,
  type MarkRow,
  markRows,
  PRESET_NAMES,
  rangesFromCamera,
  runPose,
  SCENE_OPTIONS,
  type SceneOption,
  SERVER_SCENE_NAME,
  startRun,
  startServerRun,
  stepRun,
  type SurveyEdgeListNote,
  surveyEdgeNotes,
  targetKey,
  type ViewRun,
} from "./viewRun";

/** The shortest time between two changes of the view's readouts: 4 Hz (Design note 18). */
const READOUT_INTERVAL_MS = 250;

/**
 * The name the stage's primary view is created with: the engine's name for it, and the one a
 * `view-refused` fault carries, by which the plate shows that fault for this view alone.
 */
const VIEW_NAME = PRIMARY_VIEW_NAME;

/** The primary view's identity in the scene's camera reports and the budgets. */
const VIEW_ID: ViewId = PRIMARY_VIEW_ID;

/**
 * The label of the `PRIMARY` view's upper plate's first line that never scrolls: the lines before
 * it, `FRAME` to `METER`, scroll where the stage cannot hold the plates whole
 * (decision-r10-view-block-room §3).
 */
const STARS_LABEL = "STARS";

/**
 * The last of the upper plate's lines that stay in view at its scroll's first position, the guide's
 * "always shown" lines `FRAME` to `CAMERA`, with `QUALITY` among them.
 */
const PINNED_THROUGH = "CAMERA";

/** How many of `lines` before `STARS` stay in view at the scroll's first position: to `CAMERA`. */
function pinnedLinesOf(lines: ReadonlyArray<LabelLine>): number {
  const scrolling = lines.findIndex((line) => line.label === STARS_LABEL);
  const before = scrolling < 0 ? lines : lines.slice(0, scrolling);
  return before.findIndex((line) => line.label === PINNED_THROUGH) + 1;
}

/** How an open slot is held, as the `INSTRUMENTS` panel says it: as its stub, whole, or not. */
function heldOf(standing: SlotStanding): InstrumentsPanelSlot["held"] {
  let held: InstrumentsPanelSlot["held"];
  switch (standing) {
    case "stub":
      held = "stub";
      break;
    case "held":
      held = "whole";
      break;
    case "stands":
    case "closed":
      held = null;
      break;
  }
  return held;
}

/** What the room reads of a slot: whether it is open, and its height as it last stood whole. */
function slotRoomOf(slot: Instrument | undefined): SlotRoomInput {
  const panel = slot?.panelSize ?? null;
  return {
    open: slot?.open === true,
    heightRem:
      panel === null || !(panel.remPx > 0)
        ? INSTRUMENT_SLOT_REM.heightRem
        : panel.heightPx / panel.remPx,
  };
}

/** No stars, one array for every frame without an answer. */
const NO_STARS: ReadonlyArray<ViewStar> = [];

/** The pointer's reach to a mark, rem: half the guide's 2 rem target, as plan 05's pick has it. */
const PICK_REM = 1;

/** The camera presets, every one of which a scene with an own ship offers. */
const PRESET_COUNT = Object.keys(PRESET_NAMES).length;

/** The folding panels, in the side column's order. */
const FOLD_PANELS: ReadonlyArray<FoldPanel> = [
  "instruments",
  "camera",
  "style",
  "exposure",
  "meter",
];

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
  /**
   * The quality setting `VIEW` draws at (R07.T17), the launch's `--setting`: its budgets (R07.T19),
   * its sky and its frames in either style take their forms from it; `high` by default.
   */
  readonly setting?: QualitySetting | undefined;
  /**
   * The platform the client runs on, `window.hyperion.platform` (R07.T19.f): its primary modifier,
   * ⌘ on macOS and Ctrl elsewhere, steps the free camera's rate with the arrows, and the key
   * legend names it; Linux's by default.
   */
  readonly platform?: string | undefined;
  /**
   * Starts one of the terrain session's height workers (plan R10, R10.T16.a), stable in identity:
   * the renderer's module worker by default (`browserHeightWorker`), a fake in a test.
   */
  readonly heightWorkers?: (() => WorkerLike) | undefined;
  /**
   * The kept scenes the `SCENE` selector offers, stable in identity: {@link SCENE_OPTIONS} by
   * default. The views check's terrain run gives its own (plan R10, R10.T16.d), `TERRAIN TEST`'s
   * fixture paced and counted by its probe; no other launch gives any.
   */
  readonly sceneOptions?: ReadonlyArray<SceneOption> | undefined;
  /**
   * Watches the terrain session `VIEW` holds (the views check's terrain run, R10.T16.d): called
   * with each session made, its returned function when `VIEW` lets the session go.
   */
  readonly watchTerrain?: ((session: TerrainSession) => () => void) | undefined;
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
  /**
   * The view's engine, made once above the stage, so that a new scene (the server's arriving, or a
   * kept one standing in again) remounts the stage without asking for a new adapter and device.
   */
  readonly engineState: ViewEngineState;
  readonly exposure: ExposureControl;
  readonly onExposureChange: (exposure: ExposureControl) => void;
  /** The operator's meter, held above the stage as the exposure is, so a new scene keeps it. */
  readonly meter: MeterMode;
  readonly onMeterChange: (meter: MeterMode) => void;
  readonly easedMoves: boolean;
  readonly onEasedMovesChange: (easedMoves: boolean) => void;
  /** The interim stars (R02.T16) and their count line, or `null` before an answer. */
  readonly stars: ReadonlyArray<ViewStar>;
  readonly countLine: string | null;
  /** The open universe, about which the sky is asked (R06), or `null`. */
  readonly universe: UniverseIdHex | null;
  /** The quality setting the views are budgeted at, and drawn at. */
  readonly setting: QualitySetting;
  /** The platform's primary modifier, whose chord with the arrows steps the rate (R07.T19.f). */
  readonly modifier: PrimaryModifier;
  /**
   * The terrain session of the stage's source (plan R10, R10.T16.a), made above the stage over
   * `engineState`'s engine, or `null` with none: every view tells it what it drew each frame it
   * draws, the primary's frame steps it, and every view's lit bodies take its class maps
   * (R10.T10.d) and reliefs (R10.T10.e); without one every disc is its photometry's uniform law
   * and none hands over.
   */
  readonly terrain: TerrainSession | null;
}

/** What the drawing loop reads of the several views' budgets, kept current by an effect. */
interface BudgetInputs {
  /** Each view's budget (R07.T18's `viewBudgets`), the primary's and the open instruments'. */
  readonly budgets: ReadonlyMap<ViewId, ViewBudget>;
  /** The GPU timer's state: `absent` feeds the resolution controller the frame interval. */
  readonly timer: GpuTimer;
  /** The quality setting every view draws at (R07.T17). */
  readonly setting: QualitySetting;
  /** Draws the instruments in a frame the primary draws. */
  readonly instruments: (frame: InstrumentsFrame) => void;
}

/** An instrument view with its standing in the stage's budgets and graphics (R07.T19). */
interface InstrumentViewState {
  readonly slot: Instrument;
  /** Where it stands on the stage (R10.T12.b): whole, as its stub, held whole, or closed. */
  readonly standing: SlotStanding;
  /** Why each style is held back for it: its adapter's, then (while open) the budget's. */
  readonly refusals: StyleRefusals;
  /** The styles it may switch to. */
  readonly availability: StyleAvailability;
  /** The style its budget draws. */
  readonly budgetStyle: RenderStyle;
  /** The faults for its plate, the graphics' then the terrain's ({@link faultsFor}). */
  readonly faults: ReadonlyArray<string>;
}

/** What the view keys' listener reads of the stage, kept current by an effect. */
interface KeyTargets {
  /** Commands a view: the primary, or an open instrument. */
  readonly commandView: (view: ViewId, action: ViewKeyAction) => void;
  readonly primaryCanvas: HTMLCanvasElement | null;
  /** Each instrument's canvas while mounted. */
  readonly canvases: ReadonlyArray<{
    readonly id: ViewId;
    readonly canvas: HTMLCanvasElement | null;
  }>;
  /** The `CONTROLS` view, which a key pressed off a canvas acts on. */
  readonly controlsView: ViewId;
}

/** A view's budget, which `viewBudgets` gives every view listed. */
function budgetOf(budgets: ReadonlyMap<ViewId, ViewBudget>, id: ViewId): ViewBudget {
  const budget = budgets.get(id);
  if (budget === undefined) {
    throw new Error(`the view ${id} has no budget`);
  }
  return budget;
}

/** What the drawing loop reads of the display, kept current by an effect. */
interface LoopInputs {
  readonly exposure: ExposureControl;
  readonly selection: CameraTarget | null;
  readonly reducedMotion: boolean;
  readonly size: ElementSize | null;
  readonly tokens: ColourTokens | null;
  /** The interim stars (R02.T16), drawn into every frame's scene until the sky arrives. */
  readonly stars: ReadonlyArray<ViewStar>;
  /** The sky (R06), drawn in the interim stars' place once it has arrived, or `null`. */
  readonly sky: DrawnSky | null;
  /** The styles the adapter offers (R01's `styleAvailability`). */
  readonly availability: StyleAvailability;
  /** The operator's meter (R07.T13.b's control). */
  readonly meter: MeterMode;
  /** Takes the exposure the view's `AutoExposure` moved to, into the display's state. */
  readonly onExposureChange: (exposure: ExposureControl) => void;
  /** The stage's terrain session (R10.T16.a), or `null`. */
  readonly terrain: TerrainSession | null;
  /**
   * A kept scene's terrain fixture's follower of its script (`TerrainFixture.notePlayed`,
   * R10.T16.d), told the script's time each primary frame; `null` for any other stage.
   */
  readonly notePlayed: ((scriptS: number) => void) | null;
  /**
   * What the primary's terrain readout, as last rendered, reads of its line of sight, which its
   * tripod marks (`ViewFrameInputs.lineOfSightPoint`, plan R10, R10.T12): a point over ground,
   * surveyed or not, with its slant range where known, none at `NO GROUND`, and absent while the
   * plate states no readout, given way or not standing, so that the mark and the readout never
   * disagree.
   */
  readonly lineOfSightPoint: LineOfSightPoint | undefined;
}

/** What the loop publishes for the DOM, at most every {@link READOUT_INTERVAL_MS}. */
interface Published {
  readonly run: ViewRun;
  /** The marks where the frame drew them, device px. */
  readonly anchors: ReadonlyArray<DrawAnchor>;
  /** The style the frame was drawn in, which the label block states (R07.T8.a). */
  readonly drawnStyle: RenderStyle;
  /** The photorealistic view's standing (`PhotorealRenderer.status`). */
  readonly photoreal: PhotorealStatus;
  /**
   * The terrain's annunciation of the bodies its last frame handed over (plan R10, R10.T16.c), for
   * its label block's `labelStatements`; `null` with none.
   */
  readonly terrain: TerrainAnnunciation | null;
  /**
   * The survey edges its last frame marked, for its list's notes (plan R10, R10.T16.c;
   * decision-r10-t16-review §1).
   */
  readonly surveyEdges: ReadonlyArray<DrawnSurveyEdge>;
  /**
   * The contour interval its last frame drew its wireframe terrain at, m, for its label block's
   * `CONTOUR INTERVAL` (plan R10, R10.T11); `null` while it draws none.
   */
  readonly contourIntervalM: number | null;
  /**
   * What its label block's terrain lines are made from, its `FRAME` body's survey and the terrain
   * readout's latest answer (plan R10, R10.T12; `GroundReadouts`); `null` where it states none.
   */
  readonly ground: GroundSnapshot | null;
}

/** The survey edges before the loop's first frame: none. */
const NO_SURVEY_EDGES: ReadonlyArray<DrawnSurveyEdge> = [];

/** What the meter holds before the loop's first readout: no image is drawn yet. */
const NOT_YET_DRAWN: Metering = { kind: "no-image" };

/**
 * Whether the exposure's readout would change from `shown` to `next`: its automation level with who
 * inhibited it or why, or its EV100 at the readout's one decimal (R02's `exposureReading`); the
 * control's identity changes at every step under `AUTO`.
 */
function exposureShownChanged(shown: ExposureControl, next: ExposureControl): boolean {
  return (
    exposureLevelReading(shown) !== exposureLevelReading(next) ||
    (shown.kind === "manual" && next.kind === "manual" && shown.triple !== next.triple) ||
    Math.round(controlEv100(shown) * 10) !== Math.round(controlEv100(next) * 10)
  );
}

/**
 * The chrome over a view's stage, CSS px from its top left (R07.T16.i): every box over the canvas
 * but the marks' labels, which is the label block's two plates (R10.T12.b), and each instrument
 * slot standing, whole or as its stub, but not the slots' column, which is empty between them, nor
 * the room's hidden sizers.
 */
function chromeBoxesPx(overlay: Element): ScreenBoxPx[] {
  const origin = overlay.getBoundingClientRect();
  return [
    ...overlay.querySelectorAll(
      ":scope > :not(.view-marks):not(.view-instruments):not(.view-room-sizers), :scope > .view-instruments > .view-instrument",
    ),
  ].map((element) => {
    const box = element.getBoundingClientRect();
    return {
      leftPx: box.left - origin.left,
      topPx: box.top - origin.top,
      widthPx: box.width,
      heightPx: box.height,
    };
  });
}

/**
 * Moves each mark's label with its mark, on every animation frame, at the marks the last drawn
 * frame placed; its text changes at 4 Hz (RM1 m10). Each takes its place by `markLabelPlaces`
 * (R07.T16.i;
 * decision-r07-quality-and-destination, Q6 (a) and addenda B to D): while its mark's centre lies
 * inside the stage, a place where its plate stands 0.25 rem inside the stage's edges, clear of the
 * chrome and 0.5 rem clear of the plates placed before it, the destination's label first, at its
 * `labelRisePx` above or below its whole chevron set, then the selection's, then the rest by range;
 * or it is hidden whole. A label whose mark this frame did not draw is hidden
 * until the next readout removes it. Each change of place, or of whether it is shown, cuts, never
 * eased as the guide's state transitions are: an eased move would carry its opaque plate over a
 * reticle for up to 150 ms, below the 6:1 a mark's meaning needs (decision-r07-t16d-followups, (d);
 * R07.T16.g).
 *
 * @remarks
 * The plates' sizes and the chrome's boxes are measured in the DOM, unrounded, before any label
 * moves, so that the frame forces at most one layout, before its writes. Each label's state is kept
 * with its element between frames, for the placing's hysteresis; a label mounted again starts
 * afresh.
 *
 * @param labels - The marks' labels by their target's key.
 * @param lineOfSightPx - The line-of-sight mark's box as the last drawn frame drew it, device px
 *   (`DrawnLineOfSight.boxPx`, plan R10, R10.T12), held clear as the chrome is; `null` where it was
 *   not drawn.
 * @param stage - The stage's size, CSS px, its device-pixel ratio and its rem.
 * @param states - Each label's state after the frame before, written for the next.
 * @param nowMs - The animation frame's time, by which each label's changes are counted.
 */
function placeMarkLabels(
  labels: ReadonlyMap<string, HTMLElement>,
  anchors: ReadonlyArray<DrawAnchor>,
  lineOfSightPx: ScreenBoxPx | null,
  stage: ElementSize,
  selection: CameraTarget | null,
  states: WeakMap<HTMLElement, MarkLabelState>,
  nowMs: number,
): void {
  const plates = new Map<string, PlateSizePx>();
  const previous = new Map<string, MarkLabelState>();
  let overlay: Element | null = null;
  for (const anchor of anchors) {
    const key = targetKey(anchor.target);
    const node = anchor.label === null ? undefined : labels.get(key);
    if (node !== undefined) {
      // Its laid-out size, unrounded; a translate leaves it as it is.
      const box = node.getBoundingClientRect();
      plates.set(key, { widthPx: box.width, heightPx: box.height });
      const state = states.get(node);
      if (state !== undefined) {
        previous.set(key, state);
      }
      overlay ??= node.closest(".view__overlay");
    }
  }
  const places = markLabelPlaces(anchors, plates, stage, {
    chrome: overlay === null ? [] : chromeBoxesPx(overlay),
    lineOfSightPx,
    selection: selection === null ? null : targetKey(selection),
    previous,
    nowMs,
  });
  const shown = new Set<string>();
  for (const anchor of anchors) {
    const key = targetKey(anchor.target);
    const node = anchor.label === null ? undefined : labels.get(key);
    const state = places.get(key);
    const transform =
      state === undefined ? null : markLabelTransform(anchor, stage.devicePixelRatio, state);
    if (node !== undefined && state !== undefined) {
      states.set(node, state);
    }
    if (node !== undefined && transform !== null) {
      node.style.transform = transform;
      node.style.visibility = "";
      shown.add(key);
    }
  }
  for (const [key, node] of labels) {
    if (!shown.has(key)) {
      node.style.visibility = "hidden";
    }
  }
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

/**
 * The rows and notes of the list, with the units they were last shown in kept for their
 * hysteresis.
 */
interface ShownRows {
  readonly run: ViewRun;
  readonly rows: ReadonlyArray<MarkRow>;
  /** The survey edges its notes were made from, and the notes (R10.T16.c). */
  readonly edges: ReadonlyArray<DrawnSurveyEdge>;
  readonly notes: ReadonlyArray<SurveyEdgeListNote>;
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
  engineState,
  exposure,
  onExposureChange,
  meter,
  onMeterChange,
  easedMoves,
  onEasedMovesChange,
  stars,
  countLine,
  universe,
  setting,
  modifier,
  terrain,
}: ViewStageProps) {
  const legendId = useId();
  const server = source.kind === "server" ? source.server : null;
  // A kept scene's terrain fixture follows the script the stage plays (R10.T16.d).
  const notePlayed = source.kind === "kept" ? (source.option.terrain?.notePlayed ?? null) : null;
  const serverRef = useRef<ServerInput | null>(null);
  const [initial] = useState(() => initialRun(source));
  const runRef = useRef<ViewRun>(initial);
  const [published, setPublished] = useState<Published>({
    run: initial,
    anchors: [],
    drawnStyle: initial.camera.style,
    photoreal: "idle",
    terrain: null,
    surveyEdges: NO_SURVEY_EDGES,
    contourIntervalM: null,
    ground: null,
  });
  const shown = useThrottledValue(published, READOUT_INTERVAL_MS);
  // The meter as the loop last read it out (R07.T16.b): given in the same frame as the exposure the
  // controller moved to, and not throttled again (the loop reads out at the readout rate), so that
  // the exposure's reading and the meter's status stand as one snapshot.
  const [metering, setMetering] = useState<Metering>(NOT_YET_DRAWN);
  const [selection, setSelection] = useState<CameraTarget | null>(null);
  const reducedMotion = usePrefersReducedMotion();
  const graphics = useGraphicsStatus();
  const annunciation = graphicsAnnunciation(graphics);
  // The views that show the terrain's fault, every height worker lost (R10.T16.b) or a guarded
  // computation failing on (R10.T16.e), each with its text.
  const terrainFaults = useTerrainFaults(terrain);
  const { ref: stageRef, size } = useElementSize();
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null);
  // Whether the engine refused the stage's view at its first creation (a canvas with no context).
  const [viewRefused, setViewRefused] = useState(false);
  const heldRef = useRef(new Set<string>());
  // The turn the drags on the canvas have asked for since the loop's last frame (R07.T19.f).
  const turnRef = useRef<ViewTurn>(NO_TURN);
  // The marks' labels by their target's key, which the loop moves with their marks every frame.
  const labelsRef = useRef(new Map<string, HTMLElement>());
  const inputsRef = useRef<LoopInputs>({
    exposure,
    selection,
    reducedMotion,
    size,
    tokens: null,
    stars: [],
    sky: null,
    availability: WIREFRAME_ONLY,
    meter: "average",
    onExposureChange,
    terrain,
    notePlayed,
    lineOfSightPoint: undefined,
  });

  // The sky replaces the interim field once it arrives, asked on the published run (R06.T13.c).
  const viewSky = useViewSky({
    universe,
    place: server?.place ?? null,
    run: shown.run,
    exposure,
    widthPx: size === null ? null : Math.round(size.widthPx * size.devicePixelRatio),
    setting,
  });
  const skyDrawn = viewSky.drawn;

  // The instruments, and every view's budget on the setting (R07.T19).
  const [operated, setOperated] = useState<ViewId>(VIEW_ID);
  const primaryRun = useCallback((): ViewRun => runRef.current, []);
  const instruments = useInstruments({
    engineState,
    primaryRun,
    removeCamera: server?.removeCamera ?? null,
    easedMoves,
    reducedMotion,
    sky: viewSky.drawn?.model ?? null,
    exposure,
    setting,
    modifier,
  });
  // The stage's room (plan R10, R10.T12.b; decision-r10-view-block-room §3): the slots by the
  // stage's size and their own heights, then the plates' parts in the ruled order beside those that
  // stand, from the plates as last laid out and the room decided before, its hysteresis's; adjusted
  // during render as the measures change.
  const { ref: stubRef, size: stubSize } = useElementSize();
  const [measures, setMeasures] = useState<RoomMeasures>(NOT_MEASURED);
  const roomInput: RoomInput = {
    stage: size,
    slots: [slotRoomOf(instruments.slots[0]), slotRoomOf(instruments.slots[1])],
    stubHeightRem:
      stubSize === null || !(stubSize.remPx > 0)
        ? STUB_HEIGHT_REM
        : stubSize.heightPx / stubSize.remPx,
    full: measures.full,
    beside: measures.beside,
    measuredStage: measures.stage,
  };
  const [shownRoom, setShownRoom] = useState<ViewRoom>(() => wholeRoom(roomInput.slots));
  const decided = viewRoom(roomInput, shownRoom);
  const room = sameRoom(decided, shownRoom) ? shownRoom : decided;
  if (room !== shownRoom) {
    setShownRoom(room);
  }
  const standingOf = (slot: InstrumentSlot): SlotStanding => room.slots[slot - 1] ?? "closed";
  // The plates take the width left of the slots that stand, whole or as a stub.
  const beside = room.slots.some((each) => each === "stands" || each === "stub");
  const heldOne = standingOf(1) === "stub" || standingOf(1) === "held";
  const heldTwo = standingOf(2) === "stub" || standingOf(2) === "held";
  // A held slot draws nothing, so it is budgeted for nothing either.
  const specs: ReadonlyArray<ViewSpec> = [
    { id: VIEW_ID, slot: "primary", style: published.run.camera.style },
    ...instruments.specs.filter((spec) =>
      instruments.slots.some((slot) => slot.id === spec.id && standingOf(slot.slot) === "stands"),
    ),
  ];
  const budgets = viewBudgets(specs, setting);
  // While no view can be drawn both styles are held back, by the style panel and the key 4 alike,
  // whatever the adapter offers (decision-r07-t19b-exposure-fit, item 2).
  const drawing = engineState.kind === "ready" && !viewRefused;
  const refusals = drawing
    ? withPermission(
        styleRefusals(graphics, published.photoreal),
        photorealisticAllowed(specs, setting, VIEW_ID),
      )
    : NO_VIEW_REFUSALS;
  const availability = availabilityOf(refusals);
  // Each instrument with its style refusals (its adapter's first, then the budget's), drawn style
  // and graphics fault, worked out here once from the specs, the setting and the graphics. While no
  // view can be drawn the instruments are held, as their canvases are not mounted.
  const instrumentViews: ReadonlyArray<InstrumentViewState> = instruments.slots.map((slot) => {
    const adapter = styleRefusals(graphics, slot.shown?.photoreal ?? "idle");
    const standing = standingOf(slot.slot);
    // A closed or held instrument has no budget to ask, and nothing to switch.
    const slotRefusals =
      standing === "stands"
        ? withPermission(adapter, photorealisticAllowed(specs, setting, slot.id))
        : adapter;
    return {
      slot,
      standing,
      refusals: slotRefusals,
      availability: availabilityOf(slotRefusals),
      budgetStyle: budgets.get(slot.id)?.style ?? "wireframe",
      faults: faultsFor(graphics, annunciation, slot.id, slot.id, terrainFaults),
    };
  });
  // The CONTROLS view: the side column's list and controls, and the single keys pressed off a
  // canvas, act on it; an instrument closed meanwhile, or held for the stage's room (R10.T12.b),
  // or no view drawn, leaves PRIMARY there, and a held one takes it back as it returns.
  const operatedView = drawing
    ? (instrumentViews.find(
        ({ slot, standing }) =>
          standing === "stands" && slot.id === operated && slot.shown !== null,
      ) ?? null)
    : null;
  const controlsView = operatedView === null ? VIEW_ID : operatedView.slot.id;
  const controlledShown = operatedView?.slot.shown ?? null;
  const faults = faultsFor(graphics, annunciation, VIEW_NAME, VIEW_ID, terrainFaults);
  // The primary's terrain lines (R10.T12): its survey line, DATUM and readout, from the loop's
  // latest publish, on the lower plate; the tripod and the list's note only while the plate states
  // the readout (decision-r10-view-block-room §3).
  const terrainLines = useTerrainLines(shown.ground, faults);
  const lowerParts = terrainLowerParts(terrainLines);
  const readoutAnswer = standingReadout(terrainLines, room.readout);
  const sightPoint = readoutAnswer === null ? undefined : lineOfSightPointOf(readoutAnswer);
  const hasPoint = sightPoint?.hasPoint;
  const pointRangeM = sightPoint?.rangeM ?? null;
  // The primary's own terrain annunciation (G10; R10.T16.c), none while its fault stands, of either
  // cause (`terrainStatementOf`'s rule, C2).
  const primaryTerrain = faults.some((fault) => isTerrainFault(fault)) ? null : shown.terrain;

  // The list's ranges switch unit with hysteresis, from the units they were last shown in;
  // adjusted during render as each published run arrives.
  const [shownRows, setShownRows] = useState<ShownRows>(() => ({
    run: initial,
    rows: markRows(initial),
    edges: NO_SURVEY_EDGES,
    notes: [],
  }));
  let { rows, notes } = shownRows;
  if (shownRows.run !== shown.run || shownRows.edges !== shown.surveyEdges) {
    rows = shownRows.run === shown.run ? rows : markRows(shown.run, unitsOf(shownRows.rows));
    notes = surveyEdgeNotes(shown.run.scene, shown.surveyEdges, shownRows.notes);
    setShownRows({ run: shown.run, rows, edges: shown.surveyEdges, notes });
  }

  // The loop reads the display through a ref, current after every commit; the colour tokens are
  // read from the canvas when it mounts and each time the display is shown again.
  useLayoutEffect(() => {
    inputsRef.current = {
      exposure,
      selection,
      reducedMotion,
      size,
      // A canvas just unmounted (its view refused) is still held until its ref's update lands,
      // and a detached element has no computed tokens.
      tokens: canvas === null || !canvas.isConnected ? null : readTokens(canvas),
      stars,
      sky: skyDrawn,
      availability,
      meter,
      onExposureChange,
      terrain,
      notePlayed,
      lineOfSightPoint: hasPoint === undefined ? undefined : { hasPoint, rangeM: pointRangeM },
    };
  }, [
    exposure,
    selection,
    reducedMotion,
    size,
    canvas,
    stars,
    skyDrawn,
    availability,
    meter,
    onExposureChange,
    terrain,
    notePlayed,
    hasPoint,
    pointRangeM,
  ]);

  // The drawing loop reads the server's scene, as the latest render holds it, through a ref.
  useLayoutEffect(() => {
    serverRef.current = server;
  }, [server]);

  // The loop reads the budgets through a ref, current after every commit (R07.T19).
  const budgetRef = useRef<BudgetInputs>({
    budgets,
    timer: graphics.timer,
    setting,
    instruments: instruments.frame,
  });
  useLayoutEffect(() => {
    budgetRef.current = { budgets, timer: graphics.timer, setting, instruments: instruments.frame };
  }, [budgets, graphics.timer, setting, instruments.frame]);

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

  // A held instrument is not drawn (R10.T12.b): its camera leaves the server's scene until it
  // returns and reports again, and no flight key stays held for it, its canvas gone with its focus.
  const releaseKeys = instruments.releaseKeys;
  useEffect(() => {
    const held: ReadonlyArray<readonly [InstrumentSlot, boolean]> = [
      [1, heldOne],
      [2, heldTwo],
    ];
    for (const [slot, isHeld] of held) {
      if (isHeld) {
        removeCamera?.(instrumentViewId(slot));
        releaseKeys(slot);
      }
    }
  }, [heldOne, heldTwo, removeCamera, releaseKeys]);

  // The redraw loop: every frame while the display is shown; `Activity` tears it down when hidden.
  useEffect(() => {
    if (engineState.kind !== "ready" || canvas === null) {
      return undefined;
    }
    const { engine } = engineState;
    let view: RenderView;
    try {
      view = engine.createView(canvas, VIEW_NAME);
    } catch (error: unknown) {
      // Not a loss: the canvas gave no context, or the engine could not configure it. The view
      // stays unmade until the stage remounts (a new scene); nothing here retries, so this cannot
      // loop.
      console.error(`view ${VIEW_NAME} could not be made:`, error);
      // The engine's refusal is the external system's answer, known only once the canvas exists.
      // oxlint-disable-next-line react/set-state-in-effect
      setViewRefused(true);
      return undefined;
    }
    // The view's frames, in either style, through the frame path every view draws by (R07.T19.c),
    // made at the restore where the engine has no device as the stage mounts.
    let drawer: ViewFrameDrawer | null = null;
    const releaseDrawer = makeViewFrameDrawer(engine, view, VIEW_NAME, (made) => {
      drawer = made;
    });
    // Each of the primary's frames with every view's GPU time in it, and its internal scale from
    // its budget and resolution controller (R07.T19; decision-r07-t18, item 1).
    const primaryTimes = new PrimaryFrameTimes(engine.passTimesFrame);
    const primaryScale = new BudgetedScale();
    const unsubscribeTimes = engine.onPassTimes((times) => {
      primaryTimes.passTimes(times);
    });
    const unsubscribeTimesRestored = engine.onRestored(() => {
      primaryTimes.reset(engine.passTimesFrame);
    });
    // Animation frames since the loop started, by which each view's rate is paced.
    let animationFrame = -1;
    let primaryMs: number | null = null;
    let drawnStyle: RenderStyle = "wireframe";
    // The view's exposure controller (R07.T13.a): it meters the photorealistic image's histogram,
    // smooths `AUTO` toward it and holds the display's control; a command the operator gives is
    // applied to it when the display's control changes to one the controller did not publish.
    const auto = new AutoExposure({
      source: VIEW_ID,
      program: VIEW_CAMERA,
      control: inputsRef.current.exposure,
      meter: inputsRef.current.meter,
    });
    // What the loop last read of the display's control, and what it last gave the display: an
    // operator's command is a control it reads that it did not give.
    let seenExposure = inputsRef.current.exposure;
    let givenExposure = seenExposure;
    let lastMs: number | null = null;
    let publishedMs = Number.NEGATIVE_INFINITY;
    let anchors: ReadonlyArray<DrawAnchor> = [];
    // The line-of-sight mark's box as the last frame drew it, which the labels stand clear of
    // (R10.T12).
    let lineOfSightPx: ScreenBoxPx | null = null;
    // Where each mark's label stood after the last frame, kept with its element (R07.T16.i).
    const labelStates = new WeakMap<HTMLElement, MarkLabelState>();
    // The terrain session the view last told what it drew, which forgets it when it goes.
    let reportedTo: TerrainSession | null = null;
    // The terrain's annunciation of the last frame drawn (R10.T16.c), for the label block.
    let terrainAnnunciation: TerrainAnnunciation | null = null;
    // The survey edges the last frame drawn marked, for the list's notes (R10.T16.c).
    let surveyEdges = NO_SURVEY_EDGES;
    // The guard of the terrain session's step (the batch review's S5; R10.T16.e): a failure said
    // once until a step succeeds, and the session's terrain faulted once its steps fail on for
    // 250 ms. One a session, so that a run of failures is its own.
    let stepGuard = new TerrainFailureGuard();
    // The primary's terrain readout, asked at each publish (R10.T12; decision-r10-t12-leans §3).
    const grounds = new GroundReadouts(VIEW_ID);
    // The contour interval of the last frame drawn (R10.T11), for the label block.
    let contourIntervalM: number | null = null;
    // One interval for a pose (R10.T12.b; decision-r10-view-block-room §10): the stage's views
    // state the contour interval held at the point their cameras share, through this one holder,
    // which the primary's frames give the instruments' with their own.
    const poseIntervals = new PoseIntervals();
    let frame = 0;
    const tick = (nowMs: number): void => {
      animationFrame += 1;
      const paced = budgetRef.current;
      const primaryBudget = budgetOf(paced.budgets, VIEW_ID);
      // A 30 Hz primary draws on every second vsync, and its instruments only in its frames. Its
      // labels are placed on every vsync, at the last drawn frame's marks, so that a readout's
      // commit between two drawn frames, which can widen a plate or the chrome, is never painted
      // with a label in part (R07.T16.i).
      if (!drawsInFrame(animationFrame, primaryBudget.rateHz)) {
        const between = inputsRef.current;
        if (between.size !== null) {
          placeMarkLabels(
            labelsRef.current,
            anchors,
            lineOfSightPx,
            between.size,
            between.selection,
            labelStates,
            nowMs,
          );
        }
        frame = requestAnimationFrame(tick);
        return;
      }
      // The previous primary frame's resolves end here, every view's counted in; the frames whose
      // passes have all resolved since the last are the controller's.
      primaryTimes.startFrame(engine.passTimesFrame);
      const gpuFramesMs = primaryTimes.take();
      const renderScale = primaryScale.update(
        primaryBudget,
        paced.timer === "absent" ? undefined : gpuFramesMs,
        primaryMs === null ? 0 : nowMs - primaryMs,
      );
      primaryMs = nowMs;
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
        turn: turnRef.current,
      });
      turnRef.current = NO_TURN;
      runRef.current = run;
      // A kept scene's terrain fixture is told how far its script has played: its field's revision
      // rises the first time the script passes the delta's time (R10.T16.d).
      if (run.source.kind === "kept") {
        inputs.notePlayed?.(run.tS);
      }
      if (inputs.exposure !== seenExposure) {
        seenExposure = inputs.exposure;
        if (inputs.exposure !== givenExposure) {
          auto.apply({ kind: "accepted", control: inputs.exposure });
        }
      }
      if (inputs.meter !== auto.meter) {
        auto.setMeter(inputs.meter);
      }
      // Last frame's histogram, if one arrived, under the exposure it was taken with; the
      // controller meters none while the view draws its wireframe, which then times the meter out.
      const reading = auto.step(drawer?.takeHistogram(), dtS);
      if (run.source.kind === "server" && current !== null) {
        current.reportCamera(VIEW_ID, cameraKinematics(runPose(run), run.scene));
        // Here in the loop, not in the render that made the first scene: it logs.
        reportBondRatioFindings(run.scene);
      }
      // The terrain session steps once a primary frame, before any view draws, on what each
      // instrument drew last and on what the primary draws now, noted first, so that its selection
      // is this frame's (R10.T16.a; the batch review's S2 and S6); a session let go is forgotten
      // by this view first.
      const session = inputs.terrain;
      if (reportedTo !== session) {
        reportedTo?.forgetView(VIEW_ID);
        reportedTo = session;
        stepGuard = new TerrainFailureGuard();
      }
      terrainAnnunciation = null;
      let frameInputs: ViewFrameInputs | null = null;
      if (drawer !== null) {
        if (run.camera.style === "photorealistic" && drawer.photorealStatus === "failed") {
          // Its pipelines could not be made: the view returns to the wireframe, and the control
          // holds the style back with the reason.
          runRef.current = { ...run, camera: { ...run.camera, style: "wireframe" } };
        }
        frameInputs = {
          run,
          size: inputs.size,
          tokens: inputs.tokens,
          exposure: reading.control,
          stars: inputs.stars,
          sky: inputs.sky,
          selection: inputs.selection,
          style: primaryBudget.style,
          setting: paced.setting,
          // The scene target at the render resolution times the budget's or controller's scale.
          renderScale,
          availability: inputs.availability,
          // The exposure's source: its frames take the histogram its meter reads (Design note 11).
          meter: auto.meter,
          // Every view's lit bodies shade and hand over by the session's (R10.T16.a).
          classMaps: session?.classMaps,
          reliefs: session?.reliefs,
          // The view's own terrain of the session's passes, and its survey edges (R10.T16.c),
          // its contour interval through the stage's holder (R10.T12.b).
          terrain:
            session === null
              ? undefined
              : { source: session, view: VIEW_ID, nowMs, intervals: poseIntervals },
          // The tripod, while the plate states the readout with its point (R10.T12).
          lineOfSightPoint: inputs.lineOfSightPoint,
        };
        // What it draws now, in the style it last drew in (S8), for the step below.
        const report =
          session === null
            ? null
            : terrainReportOf(
                frameInputs,
                drawer.regimes,
                primaryBudget.streamPriority,
                drawnStyle,
              );
        if (session !== null && report !== null) {
          // Its FRAME body, whose terrain readout it states, is wanted wherever the camera is
          // (R10.T12).
          session.noteView(VIEW_ID, { ...report, readoutBody: readoutBodyOf(run) });
        }
      }
      // Guarded: no throw of the step stops this loop or the instruments' (S5), and one that lasts
      // faults the terrain (R10.T16.e).
      stepTerrainSession(session, stepGuard, nowMs);
      if (drawer !== null && frameInputs !== null) {
        const drawn = drawer.draw(frameInputs);
        if (drawn !== null && inputs.size !== null) {
          anchors = drawn.anchors;
          lineOfSightPx = drawn.lineOfSight?.boxPx ?? null;
          terrainAnnunciation = drawn.terrain;
          surveyEdges = drawn.surveyEdges;
          contourIntervalM = drawn.contourIntervalM;
          // The wireframe, and the photorealistic view's stand-in while its pipelines compile.
          drawnStyle = drawn.drawnStyle;
          placeMarkLabels(
            labelsRef.current,
            anchors,
            lineOfSightPx,
            inputs.size,
            inputs.selection,
            labelStates,
            nowMs,
          );
        }
      }
      // What the meter says is true of the frame just drawn: an image coming to be drawn opens
      // the window before its first histogram, and one gone has nothing to meter (R07.T16.b).
      auto.noteImage(drawnStyle === "photorealistic");
      // The readouts change at 4 Hz, every view's in the same frame, so React renders them once:
      // a frame the instruments draw in too, at their 30 Hz, so that every view's block states
      // what it drew in that frame, one interval among them where they stand at one pose
      // (R10.T12.b).
      const publish =
        nowMs - publishedMs >= READOUT_INTERVAL_MS &&
        drawsInFrame(animationFrame, INSTRUMENT_RATE_HZ);
      // The instruments in the primary's frame, so that their passes count in it.
      paced.instruments({
        nowMs,
        frame: animationFrame,
        publish,
        primary: runRef.current,
        budgets: paced.budgets,
        setting: paced.setting,
        exposure: inputs.exposure,
        stars: inputs.stars,
        reportCamera:
          run.source.kind === "server" && current !== null ? current.reportCamera : null,
        terrain: session,
        intervals: poseIntervals,
      });
      if (publish) {
        publishedMs = nowMs;
        // The control the controller moved to reaches the display's state at the readout rate,
        // when its readout would change: its automation level with who inhibited it or why, or
        // its EV100 to the readout's 0.1.
        if (exposureShownChanged(givenExposure, auto.control)) {
          givenExposure = auto.control;
          inputs.onExposureChange(auto.control);
        }
        setMetering(auto.metering);
        setPublished({
          run,
          anchors,
          drawnStyle,
          photoreal: drawer?.photorealStatus ?? "idle",
          terrain: terrainAnnunciation,
          surveyEdges,
          contourIntervalM,
          ground: grounds.publish(session, run, nowMs),
        });
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      unsubscribeTimes();
      unsubscribeTimesRestored();
      reportedTo?.forgetView(VIEW_ID);
      // The drawer disposes of the view with itself.
      releaseDrawer();
    };
  }, [engineState, canvas]);

  const command = useCallback(
    (action: ViewKeyAction): void => {
      const result = commandRun(
        runRef.current,
        action,
        { easedMoves, reducedMotion },
        availability,
      );
      if (result.kind === "refused") {
        return;
      }
      runRef.current = result.run;
      setPublished((previous) => ({ ...previous, run: result.run }));
      if (action.kind === "target") {
        setSelection(result.run.camera.target);
      }
    },
    [easedMoves, reducedMotion, availability],
  );

  // Each view's command, through the styles its adapter and the budget allow it (R07.T19).
  const commandInstrument = instruments.command;
  const commandView = (view: ViewId, action: ViewKeyAction): void => {
    const each = instrumentViews.find(({ slot }) => slot.id === view && slot.open);
    if (each === undefined) {
      command(action);
      return;
    }
    commandInstrument(each.slot.slot, action, each.availability);
  };
  // The keys' listener reads the latest views and commands through a ref, current after every
  // commit, so that it is added once.
  const keysRef = useRef<KeyTargets>({
    commandView,
    primaryCanvas: canvas,
    canvases: [],
    controlsView,
  });
  useLayoutEffect(() => {
    keysRef.current = {
      commandView,
      primaryCanvas: canvas,
      canvases: instruments.slots.map((slot) => ({ id: slot.id, canvas: slot.canvas })),
      controlsView,
    };
  });

  // The view's single keys act from anywhere on the display but a text field (keys.ts): on the
  // focused canvas's view, which becomes the CONTROLS view first, and else on the CONTROLS view.
  useEffect(() => {
    const onKeyDown = (event: globalThis.KeyboardEvent): void => {
      const action = viewKeyAction(event);
      if (action === null) {
        return;
      }
      event.preventDefault();
      const keys = keysRef.current;
      const onCanvas =
        event.target !== null && event.target === keys.primaryCanvas
          ? VIEW_ID
          : (keys.canvases.find((each) => each.canvas !== null && each.canvas === event.target)
              ?.id ?? null);
      if (onCanvas !== null) {
        setOperated(onCanvas);
      }
      keys.commandView(onCanvas ?? keys.controlsView, action);
    };
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
    };
  }, []);

  const onCanvasKeyDown = (event: KeyboardEvent<HTMLCanvasElement>): void => {
    // The modifier's own press releases the held arrows, so that they never turn while the rate's
    // chord is held (R07.T19.f); it passes through.
    const released = heldAfterModifier(heldRef.current, event, modifier);
    if (released !== null) {
      heldRef.current = new Set(released);
      return;
    }
    const held = flightKey(event);
    if (held !== null) {
      event.preventDefault();
      setOperated(VIEW_ID);
      heldRef.current.add(held);
      return;
    }
    const action = flightKeyAction(event, modifier);
    if (action !== null) {
      event.preventDefault();
      setOperated(VIEW_ID);
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

  const placeLabel = useCallback((key: string, node: HTMLElement | null): void => {
    if (node === null) {
      labelsRef.current.delete(key);
    } else {
      labelsRef.current.set(key, node);
    }
  }, []);

  const ratio = size?.devicePixelRatio ?? 1;
  const remPx = size?.remPx ?? 16;
  const onPick = (xPx: number, yPx: number): void => {
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
  // A press on a canvas makes its view the CONTROLS view (decision-r07-t19, item 2d), before it
  // picks or turns; a drag's turns are gathered for the loop's next frame (R07.T19.f).
  const onPress = (): void => {
    setOperated(VIEW_ID);
  };
  const onTurn = (turn: ViewTurn): void => {
    turnRef.current = addTurns(turnRef.current, turn);
  };
  const fovDeg = (): number => runRef.current.camera.fovDeg;
  const dragKind = (): DragKind => dragKindOf(runRef.current.camera.preset);
  // Where the primary's camera is, at the readouts' rate, for its label block and camera panel.
  const place = useCameraPlace(shown.run);

  // The view stands in its place only once its engine is made; until then, or where it cannot
  // be, the graphics' own annunciation (R01's), or the adapter being acquired.
  const engineLine: { readonly text: string; readonly standing: StatusStanding } | null =
    engineState.kind === "ready"
      ? viewRefused
        ? NOT_MADE
        : null
      : engineState.kind === "pending"
        ? ACQUIRING
        : // Still acquiring by the status means the view's own request found nothing.
          annunciation === null || graphics.condition.kind === "acquiring"
          ? NOT_MADE
          : annunciation;
  // Room to open one more slot beside those that stand (a slot that is open stays open, held where
  // the stage cannot hold it).
  const openRoom = roomToOpen(roomInput, room);
  // Closing the CONTROLS instrument returns CONTROLS to PRIMARY. Its canvas never holds the focus
  // then, since CLOSE takes it.
  const closeInstrument = (id: ViewId): void => {
    const slot = instruments.slots.find((each) => each.id === id);
    if (slot === undefined) {
      return;
    }
    if (operated === id) {
      setOperated(VIEW_ID);
    }
    instruments.close(slot.slot);
  };

  // The scene's lighting and lit bodies' labels, which every view's photorealistic statements read.
  const lighting = lightingState(
    hostLights(
      shown.run.scene,
      sceneHostDiscs(shown.run.scene, viewSky.drawn?.model.response.hosts ?? null),
    ),
    viewSky.pending,
  );
  const litLabels = litLabelsOf(shown.run.scene);
  // The primary's STARS line, whose notes depend on the instruments open beside it
  // (decision-r06-t11f-stars-line): the sky's annunciations stand on it alone.
  const skyLine = viewSky.label(primarySkyPlace(instruments.slots));
  const stale = server?.stale === true;
  // The meter's control stands beside a drawn photorealistic image only, and the primary's block
  // states the meter while it does (decision-r07-t19-layout, item 1b).
  const meterStands = shown.drawnStyle === "photorealistic";
  // The meter as the panels show it: a meter's own status goes at once with the meter that found
  // nothing, in the render of the operator's choice (R07.T16.b).
  const meterShown = meteringFor(metering, meter);
  const primaryPhotoreal = photorealStatements(shown.run, lighting, shown.drawnStyle, litLabels);
  const primaryStatements = [
    ...labelStatements(shown.run, primaryTerrain),
    ...(easedMoves && !reducedMotion ? [EASED_MOVES_STATEMENT] : []),
    ...primaryPhotoreal,
  ];
  const primaryLines = withPlaceLines(
    withMeterLine(
      withQualityLine(
        withDrawnStyle(
          withSkyLine(labelLines(shown.run, exposure, stale, shown.contourIntervalM), skyLine),
          shown.drawnStyle,
        ),
        setting,
      ),
      meterStands ? meter : null,
    ),
    placeLines(place, stale),
  );
  const primaryCount = viewSky.drawn === null ? countLine : null;

  // The plates as laid out after each render, for the room's next decision (R10.T12.b): read from
  // the DOM, an external system, and kept only where they changed.
  const upperRef = useRef<HTMLDivElement | null>(null);
  const sizersRef = useRef<HTMLDivElement | null>(null);
  const pinnedLines = pinnedLinesOf(primaryLines);
  useLayoutEffect(() => {
    const measure = (): void => {
      const measured = measureRoom({
        upper: upperRef.current,
        sizers: sizersRef.current,
        pinnedLines,
        liveBeside: beside,
      });
      setMeasures((previous) => (sameRoomMeasures(previous, measured) ? previous : measured));
    };
    measure();
  });

  // The layout, from the `.view` box's size alone (R07.T19.b; decision-r07-t19-layout, item 1).
  const { ref: viewBoxRef, size: viewBox } = useElementSize();
  const layout = viewLayout(viewBox);
  const compact = layout === "compact";
  // In the compact layout exactly one of the folding panels is open, CAMERA by default.
  const [fold, setFold] = useState<FoldPanel>(DEFAULT_FOLD);
  // The folding panel that holds the focus, as the side column's focus events tell it.
  const [focusedFold, setFocusedFold] = useState<FoldPanel | null>(null);
  const [foldLayout, setFoldLayout] = useState(layout);
  // The panels that stand: every one but the meter's, which stands beside a drawn photorealistic
  // image only; the style's stands in every engine state (decision-r07-t19b-exposure-fit, item 2).
  // One that goes gives its place to CAMERA; on a switch to the compact layout, the panel holding
  // the focus is the one open.
  const stands = (panel: FoldPanel): boolean => panel !== "meter" || meterStands;
  const focusOpens =
    foldLayout !== layout && layout === "compact" && focusedFold !== null && stands(focusedFold)
      ? focusedFold
      : null;
  const opened: FoldPanel = focusOpens ?? (stands(fold) ? fold : DEFAULT_FOLD);
  if (opened !== fold) {
    setFold(opened);
  }
  if (foldLayout !== layout) {
    setFoldLayout(layout);
  }
  const folded = (panel: FoldPanel): boolean => compact && opened !== panel;
  const foldIds: Readonly<Record<FoldPanel, string>> = {
    instruments: `${legendId}-instruments`,
    camera: `${legendId}-camera`,
    style: `${legendId}-style`,
    exposure: `${legendId}-exposure`,
    meter: `${legendId}-meter`,
  };
  const foldOf = (target: EventTarget | null): FoldPanel | null => {
    const section = target instanceof Element ? target.closest("section") : null;
    return FOLD_PANELS.find((panel) => section !== null && section.id === foldIds[panel]) ?? null;
  };
  // The element over the stage that last took the focus (R10.T12.b). Where a render takes it away,
  // a slot held or returning with its canvas, or a scroll ended with its tab stop, the focus goes to
  // the PRIMARY view's canvas, as the stub's CLOSE gives it, never to the page, where no ring would
  // show it; looked at after every render, since any change of the room can do it.
  const stageFocusRef = useRef<HTMLElement | null>(null);
  const onMainFocus = (event: FocusEvent<HTMLDivElement>): void => {
    stageFocusRef.current = event.target instanceof HTMLElement ? event.target : null;
  };
  useLayoutEffect(() => {
    const last = stageFocusRef.current;
    const active = document.activeElement;
    if (last === null || canvas === null) {
      return;
    }
    const gone = !last.isConnected || last.tabIndex < 0;
    const lost = active === null || active === document.body || active === last;
    if (gone && lost) {
      stageFocusRef.current = null;
      canvas.focus();
    }
  });
  const onSideFocus = (event: FocusEvent<HTMLDivElement>): void => {
    setFocusedFold(foldOf(event.target));
  };
  const onSideBlur = (event: FocusEvent<HTMLDivElement>): void => {
    // The focus left the column for another control, or for nothing by the operator's own hand (a
    // press off any control, another window), not because its control was folded or went away.
    const { relatedTarget, target, currentTarget } = event;
    const leftForControl = relatedTarget instanceof Node && !currentTarget.contains(relatedTarget);
    const leftForNothing =
      relatedTarget === null && target.isConnected && target.closest("[hidden]") === null;
    if (leftForControl || leftForNothing) {
      setFocusedFold(null);
    }
  };
  // The disclosure buttons, by their panel, to which the focus goes when its panel folds.
  const foldButtonsRef = useRef(new Map<FoldPanel, HTMLButtonElement>());
  const foldButtonRef =
    (panel: FoldPanel) =>
    (button: HTMLButtonElement | null): void => {
      if (button === null) {
        foldButtonsRef.current.delete(panel);
      } else {
        foldButtonsRef.current.set(panel, button);
      }
    };
  // The Exposure panel's INHIBIT button, which takes the focus when the meter's panel goes in the
  // full layout.
  const inhibitButtonRef = useRef<HTMLButtonElement | null>(null);
  // The focus inside a panel that folds goes to that panel's disclosure button, and a panel that
  // goes away (the meter, on a style switch, a fault or the stand-in while the pipelines compile)
  // gives its focus to CAMERA's in the compact layout, and in the full layout to INHIBIT, the
  // control before it in the second column (the orchestrator's ruling on R07.T16.b).
  useLayoutEffect(() => {
    if (focusedFold === null) {
      return;
    }
    const folding = compact && focusedFold !== opened;
    const meterGone = !compact && focusedFold === "meter" && !meterStands;
    if (!folding && !meterGone) {
      return;
    }
    const active = document.activeElement;
    const lost =
      active === null ||
      active === document.body ||
      !active.isConnected ||
      active.closest("[hidden]") !== null;
    if (!lost) {
      return;
    }
    if (folding) {
      const buttons = foldButtonsRef.current;
      (buttons.get(focusedFold) ?? buttons.get(DEFAULT_FOLD))?.focus();
    } else {
      inhibitButtonRef.current?.focus();
    }
  }, [compact, focusedFold, opened, meterStands, foldButtonsRef]);

  // The CONTROLS view's camera and style, which a folded panel's standing lines describe. Its style
  // panel heads the second column and stands in every engine state, so that nothing under it moves
  // when the adapter answers or a fault takes the views: while no view can be drawn it shows the
  // camera's style with both buttons held back, the stage stating the cause
  // (decision-r07-t19b-exposure-fit, item 2).
  const controlledRun = controlledShown?.run ?? shown.run;
  const controlledRefusals = operatedView?.refusals ?? refusals;
  const controlledFaulted =
    controlledShown === null
      ? drawing && published.photoreal === "failed"
      : controlledShown.photoreal === "failed";
  // The first style's refusal, as the style control shows it.
  const styleReason = controlledRefusals.wireframe ?? controlledRefusals.photorealistic;
  const noOwnShip = offeredPresets(cameraSceneOf(controlledRun.scene)).length < PRESET_COUNT;
  // The exposure's status, `AUTO NOT AVAILABLE: …`, which stands under the row while it is folded.
  const exposureStanding = autoNotAvailable(meterShown);
  // The compact layout's row of disclosure buttons, and under it the faults and statuses of the
  // panels folded (decision-r07-t19-layout, item 1b); the limit reasons fold with their controls.
  const foldRow = compact ? (
    <>
      <div className="view-folds">
        {FOLD_BUTTONS.filter(({ panel }) => stands(panel)).map(({ panel, label }) => (
          <button
            key={panel}
            ref={foldButtonRef(panel)}
            type="button"
            className="control disclosure view-folds__button"
            aria-expanded={opened === panel}
            aria-controls={foldIds[panel]}
            onClick={() => {
              setFold(toggledFold(opened, panel));
            }}
          >
            <DisclosureGlyph expanded={opened === panel} />
            {label}
          </button>
        ))}
      </div>
      {folded("camera") && noOwnShip ? (
        <p className="view-camera__reason view-folds__standing">
          <StatusText text={NO_OWN_SHIP} />
        </p>
      ) : null}
      {folded("style") && controlledFaulted && styleReason !== null ? (
        <p className="view-style__reason view-style__reason--fault view-folds__standing">
          <StatusText text={styleReason} />
        </p>
      ) : null}
      {folded("exposure") && exposureStanding !== null ? (
        <p className="view-exposure__reason view-folds__standing">
          <output>
            <StatusText text={exposureStanding} renderPart={exposureNote} />
          </output>
        </p>
      ) : null}
    </>
  ) : null;
  const sideFolds: SideFolds = {
    cameraId: foldIds.camera,
    cameraHidden: folded("camera"),
    row: foldRow,
  };

  return (
    <div className={`view view--${layout}`} ref={viewBoxRef}>
      <div className="view__main" onFocus={onMainFocus}>
        {engineLine === null ? (
          <>
            <ViewCanvas
              canvasRef={setCanvas}
              stageRef={stageRef}
              accessibleName={[
                "VIEW",
                styleName(shown.drawnStyle),
                PRIMARY_NAME,
                PRESET_NAMES[shown.run.camera.preset],
              ].join(", ")}
              describedBy={
                lowerParts.length === 0
                  ? `${legendId}-label ${legendId}`
                  : `${legendId}-label ${legendId}-lower ${legendId}`
              }
              onKeyDown={onCanvasKeyDown}
              onKeyUp={onCanvasKeyUp}
              onBlur={onCanvasBlur}
              fovDeg={fovDeg}
              dragKind={dragKind}
              remPx={remPx}
              onPress={onPress}
              onTurn={onTurn}
              onPick={onPick}
            >
              <ViewMarkLabels
                anchors={shown.anchors}
                devicePixelRatio={ratio}
                rows={rows}
                stale={stale}
                labelRef={placeLabel}
              />
              <ViewLabelBlock
                ref={upperRef}
                id={`${legendId}-label`}
                lines={primaryLines}
                statements={primaryStatements}
                countLine={primaryCount}
                faults={faults}
                scroll={{ from: STARS_LABEL, room: room.upper, view: PRIMARY_NAME }}
                className={beside ? BESIDE_CLASS : undefined}
              />
              {lowerParts.length === 0 ? null : (
                <ViewLowerPlate
                  id={`${legendId}-lower`}
                  parts={lowerParts}
                  readout={room.readout}
                  survey={room.survey}
                  className={beside ? BESIDE_CLASS : undefined}
                />
              )}
              <div className="view-instruments">
                {instrumentViews.map(({ slot, standing, budgetStyle, faults: slotFaults }) =>
                  standing === "stub" ? (
                    <InstrumentStub
                      key={slot.id}
                      ref={stubRef}
                      name={slot.name}
                      onClose={() => {
                        closeInstrument(slot.id);
                        // Its CLOSE goes with it: the focus goes to the stage.
                        canvas?.focus();
                      }}
                    />
                  ) : standing === "stands" ? (
                    <InstrumentView
                      key={slot.id}
                      instrument={slot}
                      exposure={exposure}
                      budgetStyle={budgetStyle}
                      stale={stale}
                      lighting={lighting}
                      litLabels={litLabels}
                      primaryPhotorealStatements={primaryPhotoreal}
                      skyAwaiting={viewSky.awaiting}
                      faults={slotFaults}
                      legendId={legendId}
                      onKeyDown={(event) => {
                        // A view key on its canvas makes it the CONTROLS view before it acts.
                        if (instruments.keyDown(slot.slot, event)) {
                          setOperated(slot.id);
                        }
                      }}
                      onKeyUp={(event) => {
                        instruments.keyUp(slot.slot, event);
                      }}
                      onBlur={() => {
                        instruments.releaseKeys(slot.slot);
                      }}
                      fovDeg={() => instruments.fovDeg(slot.slot)}
                      dragKind={() => instruments.dragKind(slot.slot)}
                      onPress={() => {
                        setOperated(slot.id);
                      }}
                      onTurn={(turn) => {
                        instruments.turn(slot.slot, turn);
                      }}
                      onPick={(target) => {
                        if (target !== null) {
                          instruments.select(slot.slot, target);
                        }
                      }}
                    />
                  ) : null,
                )}
              </div>
              {size === null ? null : (
                <ViewRoomSizers
                  ref={sizersRef}
                  upper={{
                    lines: primaryLines,
                    statements: primaryStatements,
                    countLine: primaryCount,
                    faults,
                  }}
                  lower={lowerParts}
                  liveBeside={beside}
                  beside={room.heldForPlates}
                  scrollsFrom={STARS_LABEL}
                />
              )}
            </ViewCanvas>
            <KeyLegend id={legendId} modifier={modifier} />
          </>
        ) : (
          <div className="view__unavailable">
            <StatusLine text={engineLine.text} standing={engineLine.standing} />
          </div>
        )}
      </div>
      <div className="view__side" onFocus={onSideFocus} onBlur={onSideBlur}>
        <div className="view__column view__column--a">
          <InstrumentsPanel
            id={foldIds.instruments}
            fold={
              compact
                ? {
                    folded: folded("instruments"),
                    onToggle: () => {
                      setFold(toggledFold(opened, "instruments"));
                    },
                  }
                : undefined
            }
            toggleRef={compact ? foldButtonRef("instruments") : undefined}
            primary={{ id: VIEW_ID, name: PRIMARY_NAME }}
            slots={instruments.slots.map((slot) => ({
              id: slot.id,
              name: slot.name,
              open: slot.open,
              // Room for one more slot beside those open (a slot that is open stays open).
              room: openRoom,
              held: heldOf(standingOf(slot.slot)),
            }))}
            operated={controlsView}
            unavailable={engineLine !== null}
            onOpen={(id) => {
              const slot = instruments.slots.find((each) => each.id === id);
              if (slot !== undefined) {
                instruments.open(slot.slot);
              }
            }}
            onClose={closeInstrument}
            onOperate={setOperated}
          />
          {operatedView === null || controlledShown === null ? (
            <>
              <section className="panel view-targets" aria-labelledby={`${legendId}-targets`}>
                <h2 className="panel__title" id={`${legendId}-targets`}>
                  Targets{stale ? <StaleMark /> : null}{" "}
                  <span className="panel__designator">{PRIMARY_NAME}</span>
                </h2>
                <ViewMarkList
                  rows={rows}
                  notes={notes}
                  groundNote={hasPoint === true}
                  fromCamera={rangesFromCamera(shown.run.scene)}
                  stale={stale}
                  selectedKey={selection === null ? null : targetKey(selection)}
                  onSelect={(row) => {
                    setSelection(row.target);
                  }}
                />
              </section>
              {foldRow}
              <CameraControls
                preset={shown.run.camera.preset}
                offered={offeredPresets(cameraSceneOf(shown.run.scene))}
                fovDeg={shown.run.camera.fovDeg}
                rateStep={shown.run.camera.free.rateStep}
                maxRateStep={maxFreeRateStep(cameraSceneOf(shown.run.scene))}
                place={place}
                sceneStale={stale}
                modifier={modifier}
                easedMoves={easedMoves}
                reducedMotion={reducedMotion}
                onAction={command}
                onEasedMovesChange={onEasedMovesChange}
                designator={PRIMARY_NAME}
                id={sideFolds.cameraId}
                hidden={sideFolds.cameraHidden}
              />
            </>
          ) : (
            <InstrumentControls
              key={operatedView.slot.id}
              designator={operatedView.slot.name}
              shown={controlledShown}
              selection={operatedView.slot.selection}
              stale={stale}
              easedMoves={easedMoves}
              reducedMotion={reducedMotion}
              onAction={(action) => {
                commandView(operatedView.slot.id, action);
              }}
              onEasedMovesChange={onEasedMovesChange}
              onSelect={(target) => {
                instruments.select(operatedView.slot.slot, target);
              }}
              folds={sideFolds}
              modifier={modifier}
            />
          )}
        </div>
        <div className="view__column view__column--b">
          <StyleControl
            renderStyle={controlledRun.camera.style}
            refusals={controlledRefusals}
            faulted={controlledFaulted}
            onStyle={(style) => {
              commandView(controlsView, { kind: "style", style });
            }}
            designator={operatedView === null ? PRIMARY_NAME : operatedView.slot.name}
            id={foldIds.style}
            hidden={folded("style")}
          />
          <ExposurePanel
            exposure={exposure}
            metering={meterShown}
            onChange={onExposureChange}
            designator={PRIMARY_NAME}
            id={foldIds.exposure}
            hidden={folded("exposure")}
            inhibitRef={inhibitButtonRef}
          />
          {meterStands ? (
            <MeterControl
              meter={meter}
              exposure={exposure}
              source={VIEW_ID}
              metering={meterShown}
              onMeter={onMeterChange}
              designator={PRIMARY_NAME}
              id={foldIds.meter}
              hidden={folded("meter")}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}

/**
 * The `PRIMARY` view's label block's lines with where its camera is and where it looks after
 * `CAMERA`, `POSITION` and `POINTING` (R07.T19.f; `placeLines`). An instrument's block leaves them
 * to the camera panel: at 1280 × 720 its slot has no room for another line.
 */
function withPlaceLines(
  lines: ReadonlyArray<LabelLine>,
  place: ReadonlyArray<LabelLine>,
): ReadonlyArray<LabelLine> {
  return lines.flatMap((line) => (line.label === "CAMERA" ? [line, ...place] : [line]));
}

/**
 * The label block's lines with the operator's meter after the exposure, `METER AVG`, while the
 * meter's control stands (`meter` not `null`; decision-r07-t19-layout, item 1b).
 */
function withMeterLine(
  lines: ReadonlyArray<LabelLine>,
  meter: MeterMode | null,
): ReadonlyArray<LabelLine> {
  return meter === null
    ? lines
    : lines.flatMap((line) =>
        line.label === "EXPOSURE" ? [line, { label: "METER", value: meterLabel(meter) }] : [line],
      );
}

/**
 * The label block's lines with the `STYLE` the view drew, which a chosen style not yet drawn (its
 * pipelines compiling) does not replace (R07.T8.a).
 */
function withDrawnStyle(
  lines: ReadonlyArray<LabelLine>,
  drawn: RenderStyle,
): ReadonlyArray<LabelLine> {
  return lines.map((line) =>
    line.label === "STYLE" ? { ...line, value: styleName(drawn) } : line,
  );
}

/**
 * The `PRIMARY` view's label block's lines with `QUALITY` after `STYLE`: the setting every view of
 * the display draws at, the launch's `--setting`, always shown (R07.T17), on the primary's block
 * alone since it holds for the whole display.
 */
function withQualityLine(
  lines: ReadonlyArray<LabelLine>,
  setting: QualitySetting,
): ReadonlyArray<LabelLine> {
  return lines.flatMap((line) =>
    line.label === "STYLE" ? [line, { label: "QUALITY", value: QUALITY_NAMES[setting] }] : [line],
  );
}

/**
 * The label block's lines with the sky's `STARS` reading in place of R02's, once the sky is asked:
 * `PENDING`, then the held reply's, with the limit's field and the edge's while its note stands.
 */
function withSkyLine(
  lines: ReadonlyArray<LabelLine>,
  sky: SkyLineReading | null,
): ReadonlyArray<LabelLine> {
  return sky === null
    ? lines
    : lines.map((line) => (line.label === "STARS" ? { ...line, ...sky } : line));
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

function ViewPanels({
  engineSource = DEFAULT_ENGINE_SOURCE,
  setting = "high",
  platform = "linux",
  heightWorkers = browserHeightWorker,
  sceneOptions = SCENE_OPTIONS,
  watchTerrain,
}: ViewDisplayProps) {
  const statusId = useId();
  const host = use(ViewSceneContext);
  if (host === null) {
    throw new Error("the VIEW display is rendered outside a ViewSceneProvider");
  }
  const { scene, sceneName, keptName, knownSystem, choose } = host;
  const engineState = useViewEngine(engineSource);
  const [exposure, setExposure] = useState<ExposureControl>(DEFAULT_EXPOSURE);
  const [meter, setMeter] = useState<MeterMode>("average");
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
  const option = sceneOptions.find((each) => each.name === (serverChosen ? keptName : sceneName));
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
  // The terrain session of the stage's source, above the stage so that a new scene of the same
  // source keeps its fields (R10.T16.a): the server's scene's over the link, a kept scene's over
  // its terrain fixture, and none for any other kept scene.
  const link = useServerLink();
  const terrain = useTerrainSession({
    engineState,
    source: terrainSessionSource(
      server === null
        ? { kind: "kept", option }
        : { kind: "server", universe, link, onSurface: scene.onSurface },
    ),
    createWorker: heightWorkers,
    setting,
  });
  // The views check's terrain run watches the session (R10.T16.d); no other launch gives a watch.
  useEffect(
    () => (terrain === null || watchTerrain === undefined ? undefined : watchTerrain(terrain)),
    [terrain, watchTerrain],
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
          {[SERVER_SCENE_NAME, ...sceneOptions.map((each) => each.name)].map((name) => (
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
        engineState={engineState}
        exposure={exposure}
        onExposureChange={setExposure}
        meter={meter}
        onMeterChange={setMeter}
        easedMoves={easedMoves}
        onEasedMovesChange={setEasedMoves}
        stars={interim.field?.stars ?? NO_STARS}
        countLine={interim.countLine}
        universe={universe}
        setting={setting}
        modifier={primaryModifierOf(platform)}
        terrain={terrain}
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
 * (W/S, A/D, R/F, the arrows, Q/E, PageUp/PageDown, and the platform's primary modifier with the
 * up and down arrows, R07.T19.f). A drag on a canvas turns its camera, the free camera itself, or
 * a seat's or chase camera's look offset, which the arrows also turn there (R07.T19.f). A click
 * on the canvas, or the list, selects a mark, whose bracket reticle the view then draws. Its stars are the interim field of the open
 * universe's range queries about the scene's system (R02.T16), with their count line, asked only
 * where the system's position is known; another craft's mark carries its range and closure rate,
 * and a body drawn as its symbol its designation, as DOM labels over the canvas.
 *
 * The `PRIMARY` view fills the stage, and the instrument views `INSTRUMENT 1` and `INSTRUMENT 2`
 * open in fixed slots over its right edge (plan R07, T19; decision-r07-t19), each with its own
 * camera, style and selection at the primary's exposure. The primary's label block stands as two
 * plates, the upper at the stage's top left and, while the view states a body's terrain, the lower
 * at its foot, both left of the slots that stand; where the stage cannot hold them whole the
 * lower plate's readout, then its survey line and `DATUM`, give way to lines saying so, then the
 * upper plate's lines before `STARS` scroll, and then the slots are held; a slot the stage cannot
 * hold whole, `INSTRUMENT 2` first, is held, as its stub in its corner or off the stage, drawing
 * nothing until it would stand whole with a line to spare, its camera, style and target kept, so
 * that nothing over the stage is ever cut (plan R10, R10.T12.b; decision-r10-view-block-room §3). The `Instruments` panel opens and closes
 * them and holds `CONTROLS`, the view the `Targets`, `Camera` and `Style` panels and the single
 * keys pressed off a canvas act on; a press or a view key on a canvas makes its view the one. Every
 * view is budgeted on the quality setting (R07.T18's `viewBudgets`): paced at its rate, an
 * instrument only in the primary's frames, a photorealistic view's style held back where the
 * setting allows no more, and a photorealistic primary's scene target sized by the resolution
 * controller while instruments are open, from every view's GPU time in the primary's frame. The
 * setting is the launch's `--setting` (R07.T17): every view's sky, wireframe and photorealistic
 * frame take its forms, a photorealistic scene target at most its `terrain.renderHeightPx` rows.
 *
 * The server's scene of the open universe, and a kept scene that carries a terrain fixture, have a
 * terrain session (plan R10, R10.T16.a; `useTerrainSession`), made above the stage on the view's
 * engine with the height workers `heightWorkers` starts: every view tells it what it drew each
 * frame it draws, the primary's frame steps it, which wants the bodies' coarse fields the cameras
 * can resolve, and every view's lit bodies take its class maps and reliefs. Each view's
 * photorealistic frame draws the terrain of the bodies it hands over through the session, from its
 * own camera at its own τ and weight, and its label block states that terrain's annunciation; every
 * view, in either style, marks the surveys' edges (R10.T16.c). It goes with the engine when the
 * display is hidden.
 */
export const ViewDisplay = memo(ViewPanels);
