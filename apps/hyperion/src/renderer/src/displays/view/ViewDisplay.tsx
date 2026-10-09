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
  type GraphicsStatus,
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
import type { DrawAnchor } from "../../view/wireframe/drawList";
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
import { InstrumentsPanel } from "./InstrumentsPanel";
import { InstrumentView } from "./InstrumentView";
import { type Instrument, type InstrumentsFrame, useInstruments } from "./useInstruments";
import { PRIMARY_NAME, PRIMARY_VIEW_ID, PRIMARY_VIEW_NAME } from "./viewNames";
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
import { makeViewFrameDrawer, type ViewFrameDrawer } from "./viewFrameDrawer";
import { ViewLabelBlock } from "./ViewLabelBlock";
import {
  type MarkLabelState,
  markLabelPlaces,
  markLabelTransform,
  type PlateSizePx,
  ViewMarkLabels,
} from "./ViewMarkLabels";
import { styleName } from "../../view/photoreal/style";
import { ViewMarkList } from "./ViewMarkList";
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
 * An instrument slot's height and width on the stage before one is open, rem (decision-r07-t19,
 * item 2a): its title and padding, and its 15 × 11.25 rem canvas beside a label block up to 18 rem
 * wide, whose dozen lines (the stars' reading over three) set the height, 15.9 rem as measured.
 */
const INSTRUMENT_SLOT_REM = { heightRem: 16, widthRem: 35 } as const;

/** The insets about and between the slots on the stage, rem. */
const INSTRUMENT_INSET_REM = 0.5;

/**
 * The least width left to the primary's label block beside the slots, rem: the 230 px it had at
 * 1280 × 720 with both open, which the ruling accepted.
 */
const PRIMARY_LABEL_MIN_REM = 14;

/**
 * Whether the stage has room for one more instrument slot beside the `open` ones, beside the
 * primary's label block at its least width; `true` before the stage is laid out.
 *
 * @remarks
 * The slots stand at the stage's top right and its foot (decision-r07-t19-layout, item 3), so the
 * room is the stage's height less the open slots' and the insets about them; a new slot is taken
 * to be as tall as those open (a slot's label block, not its canvas, sets its height).
 *
 * @param open - The open slots' panels as laid out, each `null` before it is measured.
 */
function roomForSlot(stage: ElementSize | null, open: ReadonlyArray<ElementSize | null>): boolean {
  if (stage === null || stage.remPx <= 0) {
    return true;
  }
  const inset = INSTRUMENT_INSET_REM * stage.remPx;
  const defaultPx = INSTRUMENT_SLOT_REM.heightRem * stage.remPx;
  const heightsPx = open.map((panel) => panel?.heightPx ?? defaultPx);
  const usedPx = heightsPx.reduce((sum, heightPx) => sum + heightPx + inset, 0);
  const slotPx =
    heightsPx.length === 0 ? defaultPx : (usedPx - inset * heightsPx.length) / heightsPx.length;
  const widthRem = stage.widthPx / stage.remPx;
  return (
    inset + usedPx + slotPx + inset <= stage.heightPx &&
    INSTRUMENT_SLOT_REM.widthRem + 3 * INSTRUMENT_INSET_REM + PRIMARY_LABEL_MIN_REM <= widthRem
  );
}

/**
 * A graphics fault to show on a view's plate: the standing fault, unless it is another view's
 * refusal, which only that view's plate shows (decided 2026-10-02).
 */
function faultFor(
  graphics: GraphicsStatus,
  annunciation: GraphicsAnnunciation | null,
  viewName: string,
): string | null {
  const otherView = graphics.fault?.kind === "view-refused" && graphics.fault.viewName !== viewName;
  return annunciation?.standing === "fault" && !otherView ? annunciation.text : null;
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
  /** Why each style is held back for it: its adapter's, then (while open) the budget's. */
  readonly refusals: StyleRefusals;
  /** The styles it may switch to. */
  readonly availability: StyleAvailability;
  /** The style its budget draws. */
  readonly budgetStyle: RenderStyle;
  /** A graphics fault for its plate, or `null`. */
  readonly fault: string | null;
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
}

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
 * but the marks' labels, which is the label block with its statements, and each open instrument
 * slot, but not the slots' column, which is empty between them.
 */
function chromeBoxesPx(overlay: Element): ScreenBoxPx[] {
  const origin = overlay.getBoundingClientRect();
  return [
    ...overlay.querySelectorAll(
      ":scope > :not(.view-marks):not(.view-instruments), :scope > .view-instruments > .view-instrument",
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
 * @param stage - The stage's size, CSS px, its device-pixel ratio and its rem.
 * @param states - Each label's state after the frame before, written for the next.
 * @param nowMs - The animation frame's time, by which each label's changes are counted.
 */
function placeMarkLabels(
  labels: ReadonlyMap<string, HTMLElement>,
  anchors: ReadonlyArray<DrawAnchor>,
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
}: ViewStageProps) {
  const legendId = useId();
  const server = source.kind === "server" ? source.server : null;
  const serverRef = useRef<ServerInput | null>(null);
  const [initial] = useState(() => initialRun(source));
  const runRef = useRef<ViewRun>(initial);
  const [published, setPublished] = useState<Published>({
    run: initial,
    anchors: [],
    drawnStyle: initial.camera.style,
    photoreal: "idle",
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
  const specs: ReadonlyArray<ViewSpec> = [
    { id: VIEW_ID, slot: "primary", style: published.run.camera.style },
    ...instruments.specs,
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
    // A closed instrument has no budget to ask, and nothing to switch.
    const slotRefusals = slot.open
      ? withPermission(adapter, photorealisticAllowed(specs, setting, slot.id))
      : adapter;
    return {
      slot,
      refusals: slotRefusals,
      availability: availabilityOf(slotRefusals),
      budgetStyle: budgets.get(slot.id)?.style ?? "wireframe",
      fault: faultFor(graphics, annunciation, slot.id),
    };
  });
  // The CONTROLS view: the side column's list and controls, and the single keys pressed off a
  // canvas, act on it; an instrument closed meanwhile, or no view drawn, leaves PRIMARY there.
  const operatedView = drawing
    ? (instrumentViews.find(
        ({ slot }) => slot.open && slot.id === operated && slot.shown !== null,
      ) ?? null)
    : null;
  const controlsView = operatedView === null ? VIEW_ID : operatedView.slot.id;
  const controlledShown = operatedView?.slot.shown ?? null;
  const fault = faultFor(graphics, annunciation, VIEW_NAME);

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
      // A canvas just unmounted (its view refused) is still held until its ref's update lands,
      // and a detached element has no computed tokens.
      tokens: canvas === null || !canvas.isConnected ? null : readTokens(canvas),
      stars,
      sky: skyDrawn,
      availability,
      meter,
      onExposureChange,
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
    // Where each mark's label stood after the last frame, kept with its element (R07.T16.i).
    const labelStates = new WeakMap<HTMLElement, MarkLabelState>();
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
      if (drawer !== null) {
        if (run.camera.style === "photorealistic" && drawer.photorealStatus === "failed") {
          // Its pipelines could not be made: the view returns to the wireframe, and the control
          // holds the style back with the reason.
          runRef.current = { ...run, camera: { ...run.camera, style: "wireframe" } };
        }
        const drawn = drawer.draw({
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
        });
        if (drawn !== null && inputs.size !== null) {
          anchors = drawn.anchors;
          // The wireframe, and the photorealistic view's stand-in while its pipelines compile.
          drawnStyle = drawn.drawnStyle;
          placeMarkLabels(
            labelsRef.current,
            anchors,
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
      // The readouts change at 4 Hz, every view's in the same frame, so React renders them once.
      const publish = nowMs - publishedMs >= READOUT_INTERVAL_MS;
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
        });
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      unsubscribeTimes();
      unsubscribeTimesRestored();
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
  const openPanels = instruments.slots.flatMap((slot) => (slot.open ? [slot.panelSize] : []));
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
    ...labelStatements(shown.run),
    ...(easedMoves && !reducedMotion ? [EASED_MOVES_STATEMENT] : []),
    ...primaryPhotoreal,
  ];

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
        <p className="view-camera__reason view-folds__standing">{NO_OWN_SHIP}</p>
      ) : null}
      {folded("style") && controlledFaulted && styleReason !== null ? (
        <p className="view-style__reason view-style__reason--fault view-folds__standing">
          {styleReason}
        </p>
      ) : null}
      {folded("exposure") && exposureStanding !== null ? (
        <p className="view-exposure__reason view-folds__standing">
          <output>{exposureNote(exposureStanding)}</output>
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
      <div className="view__main">
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
              describedBy={`${legendId}-label ${legendId}`}
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
                id={`${legendId}-label`}
                lines={withPlaceLines(
                  withMeterLine(
                    withQualityLine(
                      withDrawnStyle(
                        withSkyLine(labelLines(shown.run, exposure, stale), skyLine),
                        shown.drawnStyle,
                      ),
                      setting,
                    ),
                    meterStands ? meter : null,
                  ),
                  placeLines(place, stale),
                )}
                statements={primaryStatements}
                countLine={viewSky.drawn === null ? countLine : null}
                fault={fault}
              />
              <div className="view-instruments">
                {instrumentViews.map(({ slot, budgetStyle, fault: slotFault }) =>
                  slot.open ? (
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
                      fault={slotFault}
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
              room: roomForSlot(size, openPanels),
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
 * `PENDING`, then the held reply's, with the edge's field while its note stands.
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
 * camera, style and selection at the primary's exposure. The `Instruments` panel opens and closes
 * them and holds `CONTROLS`, the view the `Targets`, `Camera` and `Style` panels and the single
 * keys pressed off a canvas act on; a press or a view key on a canvas makes its view the one. Every
 * view is budgeted on the quality setting (R07.T18's `viewBudgets`): paced at its rate, an
 * instrument only in the primary's frames, a photorealistic view's style held back where the
 * setting allows no more, and a photorealistic primary's scene target sized by the resolution
 * controller while instruments are open, from every view's GPU time in the primary's frame. The
 * setting is the launch's `--setting` (R07.T17): every view's sky, wireframe and photorealistic
 * frame take its forms, a photorealistic scene target at most its `terrain.renderHeightPx` rows.
 */
export const ViewDisplay = memo(ViewPanels);
