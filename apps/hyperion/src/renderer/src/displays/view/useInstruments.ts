/**
 * `VIEW`'s instrument views (plan R07, T19; Design note 15): two fixed slots over the primary
 * view's right edge, each opened and closed by the operator, each with its own camera, style,
 * selection and canvas, drawn by the primary's loop in the frames its budget allows.
 *
 * @remarks
 * Each instrument follows the scene its primary drew in the same frame (`followRun`), at its own
 * rate: its budget's 30 Hz, in frames the primary draws (`drawsInFrame`), so that its passes are
 * counted in the primary's frame, and it draws through the primary's own frame path
 * (`ViewFrameDrawer`) but for the histogram, which it does not take. It draws at the primary's
 * exposure, which its label block says (`SOURCE PRIMARY`, Design note 11), and culls the sky for
 * its own camera. Its camera is reported to the server's scene while it is open, as the primary's
 * is. Its readouts change at most four times a second.
 */
import {
  type RefObject,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { type ElementSize, useElementSize } from "../../lib/useElementSize";
import { type ColourTokens, readTokens } from "../../spatial/paint";
import type { SceneKinematics } from "../../lib/scene/model";
import type { ViewBudget, ViewSpec } from "../../view/budget/viewBudget";
import { drawsInFrame } from "../../view/budget/framePacing";
import {
  flightKey,
  flightKeyAction,
  flightKeyReleased,
  heldAfterModifier,
  type KeyPress,
  type ViewKeyAction,
} from "../../view/camera/keys";
import { addTurns, NO_TURN, type ViewTurn } from "../../view/camera/look";
import { DEFAULT_FOV_DEG } from "../../view/camera/projection";
import type { PrimaryModifier } from "../../lib/platform";
import type { CameraTarget, RenderStyle, ViewId } from "../../view/camera/state";
import type { StyleAvailability } from "../../view/engine/platform";
import { useGraphicsStatus } from "../../view/engine/status";
import type { RenderView } from "../../view/engine/types";
import type { ExposureControl } from "../../view/photometry/exposure";
import type { PhotorealStatus } from "../../view/photoreal/renderer";
import type { QualitySetting } from "../../view/quality/qualitySetting";
import { cameraKinematics } from "../../view/scene/fromServer";
import type { ViewStar } from "../../view/scene/model";
import type { DrawAnchor } from "../../view/wireframe/drawList";
import { availabilityOf, styleRefusals } from "./styleRefusals";
import type { SkyModel } from "../../view/sky/model";
import { type CulledViewSky, cullViewSky, type DrawnSky, viewSkyLabel } from "./useViewSky";
import type { ViewEngineState } from "./useViewEngine";
import { makeViewFrameDrawer, type ViewFrameDrawer } from "./viewFrameDrawer";
import {
  INSTRUMENT_SLOTS,
  type InstrumentSlot,
  instrumentName,
  instrumentViewId,
} from "./viewNames";
import {
  commandRun,
  followRun,
  runPose,
  startInstrumentRun,
  type ViewRun,
  WIREFRAME_ONLY,
} from "./viewRun";

/** What an instrument's loop last published for the DOM. */
export interface InstrumentShown {
  readonly run: ViewRun;
  /** The marks where its frame drew them, device px. */
  readonly anchors: ReadonlyArray<DrawAnchor>;
  /** The style it was drawn in, which its label block states. */
  readonly drawnStyle: RenderStyle;
  /** Its photorealistic renderer's standing. */
  readonly photoreal: PhotorealStatus;
}

/** One slot's standing, as the display renders it. */
export interface Instrument {
  readonly slot: InstrumentSlot;
  readonly id: ViewId;
  readonly name: string;
  readonly open: boolean;
  /** The style its camera asks for. */
  readonly style: RenderStyle;
  readonly selection: CameraTarget | null;
  /** What it last drew, or `null` while it is closed. */
  readonly shown: InstrumentShown | null;
  /** Whether the engine refused its canvas, so that it draws nothing until it is opened again. */
  readonly refused: boolean;
  /** Its stage as laid out, or `null` before it is. */
  readonly size: ElementSize | null;
  /** Its `STARS` reading of its own cull of the sky, or `null` while the interim field stands. */
  readonly skyLabel: string | null;
  /** Its canvas while mounted, by which a key pressed on it is told to act on it. */
  readonly canvas: HTMLCanvasElement | null;
  /** Receive its canvas and its stage, which the display measures. */
  readonly canvasRef: (canvas: HTMLCanvasElement | null) => void;
  readonly stageRef: (stage: HTMLElement | null) => void;
  /**
   * Its slot's panel as laid out while it is open, or `null` (closed, or before it is measured),
   * by which the room for one more slot is reckoned (R07.T19.b).
   */
  readonly panelSize: ElementSize | null;
  /** Receives its slot's panel, which the display measures. */
  readonly panelRef: (panel: HTMLElement | null) => void;
}

/** What the primary's loop gives the instruments each animation frame. */
export interface InstrumentsFrame {
  /** The animation frame's timestamp, ms. */
  readonly nowMs: number;
  /** The animation frame's index since the loop started, by which each rate is paced. */
  readonly frame: number;
  /** Whether the readouts change in this frame (4 Hz), the primary's with them. */
  readonly publish: boolean;
  /** The primary's run as it drew this frame. */
  readonly primary: ViewRun;
  readonly budgets: ReadonlyMap<ViewId, ViewBudget>;
  /** The quality setting `VIEW` is given, at which each instrument draws (R07.T17). */
  readonly setting: QualitySetting;
  /** The primary view's exposure, which each instrument draws at. */
  readonly exposure: ExposureControl;
  readonly stars: ReadonlyArray<ViewStar>;
  /** Reports each instrument's camera to the server's scene, or `null` for a kept scene. */
  readonly reportCamera: ((view: ViewId, pose: SceneKinematics) => void) | null;
}

/** What `useInstruments` reads. */
export interface InstrumentsInput {
  readonly engineState: ViewEngineState;
  /** The primary's run, as its loop last stepped it, from which an instrument opens. */
  readonly primaryRun: () => ViewRun;
  /** Removes a view's camera from the server's scene, or `null` for a kept scene. */
  readonly removeCamera: ((view: ViewId) => void) | null;
  readonly easedMoves: boolean;
  readonly reducedMotion: boolean;
  /**
   * The sky the primary draws (R06), or `null` while the interim field stands: each instrument
   * culls it for its own camera (R06 Design note 20).
   */
  readonly sky: SkyModel | null;
  /**
   * The primary's exposure as its readout shows it, at which each instrument's `STARS` states its
   * camera's limit (R07.T13.e).
   */
  readonly exposure: ExposureControl;
  /** The quality setting `VIEW` is given, whose sprite budget each slot's cull keeps (R07.T17). */
  readonly setting: QualitySetting;
  /** The platform's primary modifier, whose chord with the arrows steps the rate (R07.T19.f). */
  readonly modifier: PrimaryModifier;
}

/** The instruments, their commands and their loop. */
export interface Instruments {
  /** The slots in order, open or not. */
  readonly slots: ReadonlyArray<Instrument>;
  /** The open instruments as the budget reads them, in slot order. */
  readonly specs: ReadonlyArray<ViewSpec>;
  readonly open: (slot: InstrumentSlot) => void;
  readonly close: (slot: InstrumentSlot) => void;
  /**
   * A camera command for a slot, as a key or its controls ask (R02's `commandRun`); a style its
   * `availability` refuses leaves the camera as it was.
   */
  readonly command: (
    slot: InstrumentSlot,
    action: ViewKeyAction,
    availability: StyleAvailability,
  ) => void;
  /** Selects a mark of a slot, from its list or a pick on its canvas. */
  readonly select: (slot: InstrumentSlot, target: CameraTarget) => void;
  /**
   * A key pressed on a slot's canvas: a flight key held, or its rate stepped.
   *
   * @returns Whether it was one of the view's keys, acted on.
   */
  readonly keyDown: (slot: InstrumentSlot, press: KeyPress & { preventDefault(): void }) => boolean;
  readonly keyUp: (slot: InstrumentSlot, press: KeyPress) => void;
  /** A slot's canvas lost focus: no flight key stays held. */
  readonly releaseKeys: (slot: InstrumentSlot) => void;
  /** Gathers a drag's turn of a slot's camera for the slot's next frame (R07.T19.f). */
  readonly turn: (slot: InstrumentSlot, turn: ViewTurn) => void;
  /** A slot's horizontal field of view as it is drawn now, degrees, by which a drag is turned. */
  readonly fovDeg: (slot: InstrumentSlot) => number;
  /** Draws each open instrument whose budget's rate falls in this frame; the primary calls it. */
  readonly frame: (input: InstrumentsFrame) => void;
}

interface SlotState {
  readonly open: boolean;
  readonly style: RenderStyle;
  readonly selection: CameraTarget | null;
  readonly shown: InstrumentShown | null;
  readonly refused: boolean;
}

const CLOSED: SlotState = {
  open: false,
  style: "wireframe",
  selection: null,
  shown: null,
  refused: false,
};

type Slots = readonly [SlotState, SlotState];

function stateOf(slots: Slots, slot: InstrumentSlot): SlotState {
  return slots[slot - 1] ?? CLOSED;
}

function withSlot(
  slots: Slots,
  slot: InstrumentSlot,
  change: (state: SlotState) => SlotState,
): Slots {
  return slot === 1 ? [change(slots[0]), slots[1]] : [slots[0], change(slots[1])];
}

/** What the loop reads of a slot, kept current after every commit. */
interface SlotInputs {
  readonly open: boolean;
  readonly selection: CameraTarget | null;
  readonly size: ElementSize | null;
  readonly tokens: ColourTokens | null;
  readonly availability: StyleAvailability;
  /** Its own cull of the sky, or `null`. */
  readonly sky: DrawnSky | null;
}

/** A slot's drawer and its loop's clock, while its canvas is mounted. */
interface SlotLoop {
  readonly drawer: ViewFrameDrawer;
  lastMs: number | null;
  anchors: ReadonlyArray<DrawAnchor>;
  drawnStyle: RenderStyle;
}

/**
 * Makes a slot's view and drawer while its canvas is mounted and the engine is ready, and records
 * a refusal of its canvas.
 */
function useSlotDrawer(
  slot: InstrumentSlot,
  engineState: ViewEngineState,
  canvas: HTMLCanvasElement | null,
  loopsRef: RefObject<Map<InstrumentSlot, SlotLoop>>,
  onRefused: (slot: InstrumentSlot) => void,
): void {
  useEffect(() => {
    if (engineState.kind !== "ready" || canvas === null) {
      return undefined;
    }
    const { engine } = engineState;
    const loops = loopsRef.current;
    const name = instrumentViewId(slot);
    let view: RenderView;
    try {
      view = engine.createView(canvas, name);
    } catch (error: unknown) {
      // Not a loss: the canvas gave no context. The slot stays unmade until it is opened again.
      console.error(`view ${name} could not be made:`, error);
      onRefused(slot);
      return undefined;
    }
    // Opened during a device loss, the view waits and its drawer is made at the restore.
    const release = makeViewFrameDrawer(engine, view, name, (drawer) => {
      loops.set(slot, { drawer, lastMs: null, anchors: [], drawnStyle: "wireframe" });
    });
    return () => {
      loops.delete(slot);
      release();
    };
  }, [slot, engineState, canvas, loopsRef, onRefused]);
}

/**
 * A slot's own cull of the primary's sky, at its camera's role, field of view and canvas width and
 * the setting's sprite budget, remade only when one of them or the sky changes, and its `STARS` reading at the primary's
 * exposure (R07.T13.e).
 */
function useSlotSky(
  sky: SkyModel | null,
  exposure: ExposureControl,
  state: SlotState,
  size: ElementSize | null,
  setting: QualitySetting,
): CulledViewSky | null {
  const camera = state.open ? (state.shown?.run.camera ?? null) : null;
  const role = camera?.role ?? null;
  const fovDeg = camera?.fovDeg ?? null;
  const widthPx = size === null ? null : Math.round(size.widthPx * size.devicePixelRatio);
  const drawn = useMemo(
    () =>
      sky === null || role === null || fovDeg === null || widthPx === null
        ? null
        : cullViewSky(sky, role, fovDeg, widthPx, setting),
    [sky, role, fovDeg, widthPx, setting],
  );
  return drawn === null || role === null || fovDeg === null
    ? null
    : { drawn, labelValue: viewSkyLabel(drawn.model, role, exposure, fovDeg) };
}

/** The instruments of a `VIEW` stage. */
export function useInstruments(input: InstrumentsInput): Instruments {
  const {
    engineState,
    primaryRun,
    removeCamera,
    easedMoves,
    reducedMotion,
    sky,
    exposure,
    setting,
    modifier,
  } = input;
  const graphics = useGraphicsStatus();
  const [slots, setSlots] = useState<Slots>([CLOSED, CLOSED]);
  const [canvasOne, setCanvasOne] = useState<HTMLCanvasElement | null>(null);
  const [canvasTwo, setCanvasTwo] = useState<HTMLCanvasElement | null>(null);
  const stageOne = useElementSize();
  const stageTwo = useElementSize();
  const panelOne = useElementSize();
  const panelTwo = useElementSize();
  const skyOne = useSlotSky(sky, exposure, stateOf(slots, 1), stageOne.size, setting);
  const skyTwo = useSlotSky(sky, exposure, stateOf(slots, 2), stageTwo.size, setting);
  // The loops read the drawn skies, which keep their identity while only the label moves.
  const drawnOne = skyOne?.drawn ?? null;
  const drawnTwo = skyTwo?.drawn ?? null;
  const runs = useRef(new Map<InstrumentSlot, ViewRun>());
  const held = useRef(new Map<InstrumentSlot, Set<string>>());
  // Each slot's drags' turn since it last drew (R07.T19.f).
  const turns = useRef(new Map<InstrumentSlot, ViewTurn>());
  const loopsRef = useRef(new Map<InstrumentSlot, SlotLoop>());
  const inputsRef = useRef<ReadonlyMap<InstrumentSlot, SlotInputs>>(new Map());
  const reducedMotionRef = useRef(reducedMotion);

  const availability = useCallback(
    (slot: InstrumentSlot): StyleAvailability =>
      availabilityOf(styleRefusals(graphics, stateOf(slots, slot).shown?.photoreal ?? "idle")),
    [graphics, slots],
  );

  useLayoutEffect(() => {
    const canvases = [canvasOne, canvasTwo];
    const sizes = [stageOne.size, stageTwo.size];
    const skies = [drawnOne, drawnTwo];
    inputsRef.current = new Map(
      INSTRUMENT_SLOTS.map((slot) => {
        const state = stateOf(slots, slot);
        const canvas = canvases[slot - 1] ?? null;
        return [
          slot,
          {
            open: state.open,
            selection: state.selection,
            size: sizes[slot - 1] ?? null,
            // A canvas just unmounted is still held until its ref's update lands.
            tokens: canvas === null || !canvas.isConnected ? null : readTokens(canvas),
            availability: availability(slot),
            sky: skies[slot - 1] ?? null,
          },
        ];
      }),
    );
    reducedMotionRef.current = reducedMotion;
  }, [
    slots,
    canvasOne,
    canvasTwo,
    stageOne.size,
    stageTwo.size,
    drawnOne,
    drawnTwo,
    availability,
    reducedMotion,
  ]);

  const onRefused = useCallback((slot: InstrumentSlot): void => {
    setSlots((previous) => withSlot(previous, slot, (state) => ({ ...state, refused: true })));
  }, []);
  useSlotDrawer(1, engineState, canvasOne, loopsRef, onRefused);
  useSlotDrawer(2, engineState, canvasTwo, loopsRef, onRefused);

  // An open instrument's camera leaves the server's scene with the stage, as the primary's does.
  useEffect(() => {
    if (removeCamera === null) {
      return undefined;
    }
    // One map for the hook's life, so the cleanup reads the runs open at the time.
    const open = runs.current;
    return () => {
      for (const slot of open.keys()) {
        removeCamera(instrumentViewId(slot));
      }
    };
  }, [removeCamera]);

  const open = useCallback(
    (slot: InstrumentSlot): void => {
      if (runs.current.has(slot)) {
        return;
      }
      const run = startInstrumentRun(primaryRun());
      runs.current.set(slot, run);
      held.current.set(slot, new Set());
      setSlots((previous) =>
        withSlot(previous, slot, () => ({
          open: true,
          style: run.camera.style,
          selection: null,
          shown: { run, anchors: [], drawnStyle: "wireframe", photoreal: "idle" },
          refused: false,
        })),
      );
    },
    [primaryRun],
  );

  const close = useCallback(
    (slot: InstrumentSlot): void => {
      runs.current.delete(slot);
      held.current.delete(slot);
      turns.current.delete(slot);
      removeCamera?.(instrumentViewId(slot));
      setSlots((previous) => withSlot(previous, slot, () => CLOSED));
    },
    [removeCamera],
  );

  const command = useCallback(
    (slot: InstrumentSlot, action: ViewKeyAction, styles: StyleAvailability): void => {
      const run = runs.current.get(slot);
      if (run === undefined) {
        return;
      }
      const result = commandRun(run, action, { easedMoves, reducedMotion }, styles);
      if (result.kind === "refused") {
        return;
      }
      runs.current.set(slot, result.run);
      setSlots((previous) =>
        withSlot(previous, slot, (state) => ({
          ...state,
          style: result.run.camera.style,
          selection: action.kind === "target" ? result.run.camera.target : state.selection,
          shown: state.shown === null ? null : { ...state.shown, run: result.run },
        })),
      );
    },
    [easedMoves, reducedMotion],
  );

  const select = useCallback((slot: InstrumentSlot, target: CameraTarget): void => {
    setSlots((previous) => withSlot(previous, slot, (state) => ({ ...state, selection: target })));
  }, []);

  const keyDown = useCallback(
    (slot: InstrumentSlot, press: KeyPress & { preventDefault(): void }): boolean => {
      // The modifier's own press releases the held arrows (all held keys on macOS), so that they
      // never turn while the rate's chord is held (R07.T19.f).
      const released = heldAfterModifier(held.current.get(slot) ?? new Set(), press, modifier);
      if (released !== null) {
        held.current.set(slot, new Set(released));
        return false;
      }
      const key = flightKey(press);
      if (key !== null) {
        press.preventDefault();
        held.current.get(slot)?.add(key);
        return true;
      }
      const action = flightKeyAction(press, modifier);
      if (action === null) {
        return false;
      }
      press.preventDefault();
      // A flight action steps the rate, which no style refusal touches.
      command(slot, action, WIREFRAME_ONLY);
      return true;
    },
    [command, modifier],
  );

  const keyUp = useCallback((slot: InstrumentSlot, press: KeyPress): void => {
    const released = flightKeyReleased(press);
    if (released !== null) {
      held.current.get(slot)?.delete(released);
    }
  }, []);

  const releaseKeys = useCallback((slot: InstrumentSlot): void => {
    held.current.get(slot)?.clear();
  }, []);

  const turn = useCallback((slot: InstrumentSlot, each: ViewTurn): void => {
    if (runs.current.has(slot)) {
      turns.current.set(slot, addTurns(turns.current.get(slot) ?? NO_TURN, each));
    }
  }, []);

  const fovDeg = useCallback(
    (slot: InstrumentSlot): number => runs.current.get(slot)?.camera.fovDeg ?? DEFAULT_FOV_DEG,
    [],
  );

  const frame = useCallback((each: InstrumentsFrame): void => {
    for (const slot of INSTRUMENT_SLOTS) {
      const loop = loopsRef.current.get(slot);
      const run = runs.current.get(slot);
      const inputs = inputsRef.current.get(slot);
      const budget = each.budgets.get(instrumentViewId(slot));
      if (
        loop === undefined ||
        run === undefined ||
        inputs?.open !== true ||
        budget === undefined ||
        !drawsInFrame(each.frame, budget.rateHz)
      ) {
        continue;
      }
      const dtS = loop.lastMs === null ? 0 : (each.nowMs - loop.lastMs) / 1000;
      loop.lastMs = each.nowMs;
      let next = followRun(run, each.primary, {
        dtS,
        held: held.current.get(slot) ?? new Set(),
        reducedMotion: reducedMotionRef.current,
        turn: turns.current.get(slot) ?? NO_TURN,
      });
      turns.current.delete(slot);
      // Where its pipelines could not be made, the view returns to the wireframe and its control
      // holds the style back with the reason.
      const failed =
        next.camera.style === "photorealistic" && loop.drawer.photorealStatus === "failed";
      if (failed) {
        next = { ...next, camera: { ...next.camera, style: "wireframe" } };
      }
      runs.current.set(slot, next);
      if (next.source.kind === "server") {
        each.reportCamera?.(instrumentViewId(slot), cameraKinematics(runPose(next), next.scene));
      }
      const drawn = loop.drawer.draw({
        run: next,
        size: inputs.size,
        tokens: inputs.tokens,
        exposure: each.exposure,
        stars: each.stars,
        sky: inputs.sky,
        selection: inputs.selection,
        style: budget.style,
        setting: each.setting,
        renderScale: budget.renderScale,
        availability: inputs.availability,
        // The primary is the exposure's source: an instrument meters nothing (Design note 11).
        meter: null,
      });
      if (drawn !== null) {
        loop.anchors = drawn.anchors;
        loop.drawnStyle = drawn.drawnStyle;
      }
      if (failed || each.publish) {
        const shown: InstrumentShown = {
          run: next,
          anchors: loop.anchors,
          drawnStyle: loop.drawnStyle,
          photoreal: loop.drawer.photorealStatus,
        };
        setSlots((previous) =>
          withSlot(previous, slot, (state) =>
            state.open ? { ...state, style: next.camera.style, shown } : state,
          ),
        );
      }
    }
  }, []);

  return {
    slots: INSTRUMENT_SLOTS.map((slot) => {
      const state = stateOf(slots, slot);
      return {
        slot,
        id: instrumentViewId(slot),
        name: instrumentName(slot),
        open: state.open,
        style: state.style,
        selection: state.selection,
        shown: state.shown,
        refused: state.refused,
        size: slot === 1 ? stageOne.size : stageTwo.size,
        skyLabel: (slot === 1 ? skyOne : skyTwo)?.labelValue ?? null,
        canvas: slot === 1 ? canvasOne : canvasTwo,
        canvasRef: slot === 1 ? setCanvasOne : setCanvasTwo,
        stageRef: slot === 1 ? stageOne.ref : stageTwo.ref,
        // A closed slot's panel keeps the size it last had: it takes no room.
        panelSize: state.open ? (slot === 1 ? panelOne.size : panelTwo.size) : null,
        panelRef: slot === 1 ? panelOne.ref : panelTwo.ref,
      };
    }),
    specs: INSTRUMENT_SLOTS.filter((slot) => stateOf(slots, slot).open).map((slot) => ({
      id: instrumentViewId(slot),
      slot: "instrument",
      style: stateOf(slots, slot).style,
    })),
    open,
    close,
    command,
    select,
    keyDown,
    keyUp,
    releaseKeys,
    turn,
    fovDeg,
    frame,
  };
}
