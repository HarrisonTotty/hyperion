/**
 * The several-views check's script (plan R07, T20): it drives `VIEW` through its controls, as an
 * operator would, into each phase's configuration, holds it while the probe measures, resizes the
 * primary alone once, and gathers the record the main process writes.
 *
 * @remarks
 * The scene is the kept `PHASE TEST`, which needs no server. The phases, in order:
 *
 * 1. `photoreal-alone`: the photorealistic primary, no instrument;
 * 2. `photoreal-two-wireframe`: with both instruments open in the wireframe, T20's cockpit. After
 *    it the primary's stage is narrowed to 80% and 65% of its width and given it back, a step at a
 *    time, with every allocation recorded;
 * 3. `wireframe-two-wireframe`: the primary in the wireframe too, T20's first case on the UHD 620's
 *    low setting;
 * 4. `wireframe-photoreal-wireframe`: instrument 1 photorealistic, T20's second;
 * 5. `wireframe-alone`: both instruments closed, the wireframe's own baseline.
 *
 * Phases 1 and 2, and 5 and 3, are the pairs from which the per-canvas overhead is read. Then the
 * cockpit is set up again, unmeasured, and the person at a shown run is asked whether every view is
 * the right way up: last, so that turning the cameras to judge it changes no measured phase (the
 * scene's bodies lie on the horizon line, where a still picture cannot show a view turned upside
 * down). Every
 * control is found by what the operator reads: the `SCENE` buttons, the `Instruments` panel's
 * `OPEN`, `CLOSE` and `CONTROLS` buttons (unfolded first in the compact layout), the style key `4`
 * and each canvas's accessible name, `VIEW, <style drawn>, <slot>, <preset>`.
 */

import type {
  ViewsCheckApi,
  ViewsCheckPhaseName,
  ViewsCheckPhaseRecord,
  ViewsCheckRecord,
  ViewsCheckResizeRecord,
  ViewsCheckStyle,
  ViewsCheckViewName,
  ViewsCheckWindow,
} from "../../../../../preload/api";
import { PER_CANVAS_OVERHEAD_MS } from "../../../view/budget/viewBudget";
import { STYLE_TOGGLE_KEY } from "../../../view/photoreal/style";
import { PHASE_SCENE_NAME } from "../../../view/scenes/phaseScene";
import {
  INSTRUMENT_SLOTS,
  type InstrumentSlot,
  instrumentName,
  PRIMARY_NAME,
  PRIMARY_VIEW_NAME,
} from "../viewNames";
import type { ShownView, ViewsProbe } from "./viewsProbe";

/** One phase: the primary's style and each instrument's, `null` for a closed one. */
export interface ViewsCheckPhasePlan {
  readonly name: ViewsCheckPhaseName;
  readonly primary: ViewsCheckStyle;
  readonly instruments: readonly [ViewsCheckStyle | null, ViewsCheckStyle | null];
}

/** The phases in their order (see the module's remarks). */
export const VIEWS_CHECK_PLAN: ReadonlyArray<ViewsCheckPhasePlan> = [
  { name: "photoreal-alone", primary: "photorealistic", instruments: [null, null] },
  {
    name: "photoreal-two-wireframe",
    primary: "photorealistic",
    instruments: ["wireframe", "wireframe"],
  },
  {
    name: "wireframe-two-wireframe",
    primary: "wireframe",
    instruments: ["wireframe", "wireframe"],
  },
  {
    name: "wireframe-photoreal-wireframe",
    primary: "wireframe",
    instruments: ["photorealistic", "wireframe"],
  },
  { name: "wireframe-alone", primary: "wireframe", instruments: [null, null] },
];

/** The phase after which the primary is resized, and set up again for the question: T20's cockpit. */
export const COCKPIT_PHASE: ViewsCheckPhaseName = "photoreal-two-wireframe";

/** The primary stage's widths through the resize, as fractions of its own; 1 gives it back. */
export const RESIZE_FRACTIONS: ReadonlyArray<number> = [0.8, 0.65, 1];

/** How long each part of the run takes, ms. */
export interface ViewsCheckTiming {
  /** From a configuration reached to its trace's start: the scale controller and caches settle. */
  readonly settleMs: number;
  /**
   * From the trace's start to the measured window's: the start's own pause is left out, as R05's
   * trace windows leave out a second at each boundary (`TRACE_BOUNDARY_GUARD_S`).
   */
  readonly guardMs: number;
  /** A phase's measured window. */
  readonly measureMs: number;
  /** After the window, for the pass timer's last reads to arrive. */
  readonly graceMs: number;
  /** Each step of the resize. */
  readonly resizeStepMs: number;
  /** The longest wait for a control to take effect (a style's pipelines compile first). */
  readonly waitMs: number;
  /** How often a wait looks again. */
  readonly pollMs: number;
}

/** A full run's timing (about four minutes) and the smoke's (well under one). */
export const VIEWS_CHECK_TIMING: {
  readonly full: ViewsCheckTiming;
  readonly smoke: ViewsCheckTiming;
} = {
  full: {
    settleMs: 5_000,
    guardMs: 1_000,
    measureMs: 20_000,
    graceMs: 1_000,
    resizeStepMs: 2_000,
    waitMs: 60_000,
    pollMs: 100,
  },
  smoke: {
    settleMs: 1_000,
    guardMs: 1_000,
    measureMs: 2_000,
    graceMs: 500,
    resizeStepMs: 1_000,
    waitMs: 60_000,
    pollMs: 100,
  },
};

/** What the script drives and reads. */
export interface ViewsCheckDeps {
  readonly api: Pick<ViewsCheckApi, "startPhase" | "endPhase" | "askRightWayUp">;
  readonly probe: Pick<
    ViewsProbe,
    | "phase"
    | "canvasOf"
    | "allocationMark"
    | "allocationsSince"
    | "longestFrameMs"
    | "faults"
    | "timer"
  >;
  /** The page, whose `VIEW` is shown. */
  readonly document: Document;
  readonly timing: ViewsCheckTiming;
  /** The page's clock, `performance.now()`, on which the probe stamps frames. */
  readonly nowMs: () => number;
  /**
   * Marks a phase's measured window in the trace (`performance.measure`), whose span sets the
   * trace's clock against the page's for the main process.
   */
  readonly markWindow: (window: ViewsCheckWindow) => void;
  readonly sleep: (ms: number) => Promise<void>;
  /**
   * Stops the run: before its next step, wait or call to the main process it throws
   * {@link STOPPED}, having touched nothing more.
   */
  readonly signal: AbortSignal;
  readonly devicePixelRatio: number;
}

/** A canvas's slot (as the operator names it), its view's engine name and its style. */
interface CanvasReading {
  readonly slot: string;
  readonly view: ViewsCheckViewName;
  readonly style: ViewsCheckStyle;
}

/**
 * `VIEW, <style>, <slot>, <preset>` read back; `null` for a name of another shape, another style
 * or a slot `VIEW` does not have.
 */
function readCanvasName(name: string): CanvasReading | null {
  const [label, style, slot] = name.split(", ");
  const view = slot === undefined ? null : engineName(slot);
  if (label !== "VIEW" || slot === undefined || view === null) {
    return null;
  }
  if (style === "WIREFRAME") {
    return { slot, view, style: "wireframe" };
  }
  return style === "PHOTOREALISTIC" ? { slot, view, style: "photorealistic" } : null;
}

/** The style's name as a canvas's name and the label block give it. */
function styleWord(style: ViewsCheckStyle): string {
  return style === "wireframe" ? "WIREFRAME" : "PHOTOREALISTIC";
}

/**
 * The engine's name for a slot's view, as `viewNames.ts` makes it (`PRIMARY_VIEW_NAME`,
 * `instrumentViewId`; the test holds them equal).
 */
export function engineName(slot: string): ViewsCheckViewName | null {
  if (slot === PRIMARY_NAME) {
    return "view";
  }
  if (slot === instrumentName(1)) {
    return "instrument-1";
  }
  return slot === instrumentName(2) ? "instrument-2" : null;
}

/** `VIEW`'s controls and canvases in the page, as the operator finds them. */
class ViewControls {
  readonly #document: Document;

  constructor(document: Document) {
    this.#document = document;
  }

  /** Every canvas shown, by slot. */
  canvases(): ReadonlyMap<string, CanvasReading> {
    const readings = new Map<string, CanvasReading>();
    for (const canvas of this.#document.querySelectorAll('canvas[role="application"]')) {
      const reading = readCanvasName(canvas.getAttribute("aria-label") ?? "");
      if (reading !== null) {
        readings.set(reading.slot, reading);
      }
    }
    return readings;
  }

  /** The style a slot's canvas drew, or `null` while it has none. */
  styleOf(slot: string): ViewsCheckStyle | null {
    return this.canvases().get(slot)?.style ?? null;
  }

  /** The primary's stage, the element whose size its canvas takes. */
  primaryStage(): HTMLElement | null {
    for (const canvas of this.#document.querySelectorAll('canvas[role="application"]')) {
      if (readCanvasName(canvas.getAttribute("aria-label") ?? "")?.slot === PRIMARY_NAME) {
        return canvas.closest<HTMLElement>(".view__stage");
      }
    }
    return null;
  }

  /** The `SCENE` button of `name`, or `null`. */
  sceneButton(name: string): HTMLButtonElement | null {
    const scene = this.#document.querySelector('fieldset[aria-label="Scene"]');
    return scene === null ? null : buttonIn(scene, name);
  }

  /** A button of the `Instruments` panel's row `legend` (`INSTRUMENT 1`, `CONTROLS`). */
  panelButton(legend: string, label: string): HTMLButtonElement | null {
    for (const set of this.#document.querySelectorAll("fieldset")) {
      if (set.querySelector("legend")?.textContent === legend) {
        return buttonIn(set, label);
      }
    }
    return null;
  }

  /** Unfolds the `Instruments` panel where the compact layout has folded it. */
  unfoldInstruments(): void {
    const toggle = this.#document.querySelector<HTMLButtonElement>(
      '.view-instruments-panel__title button[aria-expanded="false"]',
    );
    toggle?.click();
  }

  /** Why a button is held back, from the text it is described by, or "". */
  reasonOf(button: HTMLButtonElement | null): string {
    const ids = button?.getAttribute("aria-describedby") ?? "";
    return ids
      .split(" ")
      .map((id) => (id.length === 0 ? "" : (this.#document.getElementById(id)?.textContent ?? "")))
      .join(" ")
      .trim();
  }

  /** Presses the style key with the focus off every canvas, as `VIEW`'s single keys take it. */
  pressStyleKey(): void {
    for (const type of ["keydown", "keyup"]) {
      this.#document.body.dispatchEvent(
        new KeyboardEvent(type, { key: STYLE_TOGGLE_KEY, bubbles: true, cancelable: true }),
      );
    }
  }
}

function buttonIn(container: Element, label: string): HTMLButtonElement | null {
  for (const button of container.querySelectorAll("button")) {
    if (button.textContent.trim() === label) {
      return button;
    }
  }
  return null;
}

/** The error a stopped run ends with. */
export const STOPPED = "views check: stopped";

/**
 * Runs `act` on each of `items` in turn, each after the one before has settled: every step of the
 * script acts on the page as the step before left it.
 */
async function inOrder<T>(items: ReadonlyArray<T>, act: (item: T) => Promise<void>): Promise<void> {
  await items.reduce<Promise<void>>(
    (previous, item) => previous.then(() => act(item)),
    Promise.resolve(),
  );
}

/** The views shown, primary first, by their engine names, with the styles their canvases name. */
function shownViews(controls: ViewControls): ShownView[] {
  return [...controls.canvases().values()]
    .map((reading) => ({ name: reading.view, style: reading.style }))
    .toSorted((a, b) =>
      a.name === PRIMARY_VIEW_NAME
        ? -1
        : b.name === PRIMARY_VIEW_NAME
          ? 1
          : a.name < b.name
            ? -1
            : 1,
    );
}

/** The instruments a phase opens, by slot, with their styles; `null` for one it closes. */
function instrumentStyles(
  plan: ViewsCheckPhasePlan,
): ReadonlyArray<readonly [InstrumentSlot, ViewsCheckStyle | null]> {
  return INSTRUMENT_SLOTS.map((slot, index) => [slot, plan.instruments[index] ?? null] as const);
}

/**
 * Runs the check in `deps.document`'s `VIEW` and returns its record.
 *
 * @throws Error naming the step when a control does not take effect within the timing's wait, or
 * when the run is stopped through `deps.signal`.
 */
export async function runViewsCheck(deps: ViewsCheckDeps): Promise<ViewsCheckRecord> {
  const { probe, timing } = deps;
  const controls = new ViewControls(deps.document);
  // Every step, wait and call to the main process first looks whether the run was stopped.
  const stopped = (): void => {
    if (deps.signal.aborted) {
      throw new Error(STOPPED);
    }
  };
  const wait = async (ms: number): Promise<void> => {
    stopped();
    await deps.sleep(ms);
    stopped();
  };
  const step = <T>(items: ReadonlyArray<T>, act: (item: T) => Promise<void>): Promise<void> =>
    inOrder(items, async (item) => {
      stopped();
      await act(item);
    });
  const api: ViewsCheckDeps["api"] = {
    startPhase: async (name) => {
      stopped();
      await deps.api.startPhase(name);
    },
    endPhase: async (name, window) => {
      stopped();
      await deps.api.endPhase(name, window);
    },
    askRightWayUp: async () => {
      stopped();
      await deps.api.askRightWayUp();
    },
  };
  const until = async (
    what: string,
    test: () => boolean,
    why = (): string => "",
  ): Promise<void> => {
    const deadline = deps.nowMs() + timing.waitMs;
    const look = async (): Promise<void> => {
      stopped();
      if (test()) {
        return;
      }
      if (deps.nowMs() > deadline) {
        const reason = why();
        throw new Error(
          `views check: ${what} within ${String(timing.waitMs / 1000)} s${reason.length > 0 ? ` (${reason})` : ""}`,
        );
      }
      await wait(timing.pollMs);
      await look();
    };
    await look();
  };
  const press = async (
    find: () => HTMLButtonElement | null,
    what: string,
  ): Promise<HTMLButtonElement> => {
    await until(`${what} was not found`, () => find() !== null);
    const button = find();
    if (button === null) {
      throw new Error(`views check: ${what} went away`);
    }
    stopped();
    button.click();
    return button;
  };
  const operate = async (slot: string): Promise<void> => {
    controls.unfoldInstruments();
    await press(() => controls.panelButton("CONTROLS", slot), `CONTROLS ${slot}`);
  };
  const setStyle = async (slot: string, style: ViewsCheckStyle): Promise<void> => {
    if (controls.styleOf(slot) === style) {
      return;
    }
    await operate(slot);
    stopped();
    controls.pressStyleKey();
    await until(`${slot} did not draw ${styleWord(style)}`, () => controls.styleOf(slot) === style);
  };
  const setOpen = async (slot: InstrumentSlot, open: boolean): Promise<void> => {
    const name = instrumentName(slot);
    if (controls.canvases().has(name) === open) {
      return;
    }
    controls.unfoldInstruments();
    const label = open ? "OPEN" : "CLOSE";
    const button = await press(() => controls.panelButton(name, label), `${name}'s ${label}`);
    await until(
      `${name} did not ${open ? "open" : "close"}`,
      () => controls.canvases().has(name) === open,
      () => controls.reasonOf(button),
    );
  };
  const configure = async (plan: ViewsCheckPhasePlan): Promise<void> => {
    const instruments = instrumentStyles(plan);
    await step(instruments, ([slot, style]) => setOpen(slot, style !== null));
    const wanted: ReadonlyArray<readonly [string, ViewsCheckStyle]> = [
      [PRIMARY_NAME, plan.primary],
      ...instruments.flatMap(([slot, style]) =>
        style === null ? [] : [[instrumentName(slot), style] as const],
      ),
    ];
    // Every view off the photorealistic style first, so that the low setting's one
    // photorealistic view is free before another takes it.
    await step(
      [
        ...wanted.filter(([, style]) => style === "wireframe"),
        ...wanted.filter(([, style]) => style === "photorealistic"),
      ],
      ([slot, style]) => setStyle(slot, style),
    );
    await operate(PRIMARY_NAME);
  };
  const resizePrimary = async (): Promise<ViewsCheckResizeRecord> => {
    const stage = controls.primaryStage();
    if (stage === null) {
      throw new Error("views check: the primary's stage was not found");
    }
    const names = shownViews(controls).map((view) => view.name);
    const canvases = (): ViewsCheckResizeRecord["before"] =>
      names.map((name) => ({ name, canvas: probe.canvasOf(name) }));
    const before = canvases();
    const mark = probe.allocationMark();
    const steps: Array<ViewsCheckResizeRecord["steps"][number]> = [];
    await step(RESIZE_FRACTIONS, async (fraction) => {
      const startMs = deps.nowMs();
      stage.style.width = fraction === 1 ? "" : `${String(fraction * 100)}%`;
      await wait(timing.resizeStepMs);
      steps.push({
        widthFraction: fraction,
        longestFrameMs: probe.longestFrameMs(startMs, deps.nowMs()),
        views: canvases(),
      });
    });
    const allocations = probe.allocationsSince(mark).map(({ kind, name }) => ({ kind, name }));
    return { before, steps, allocations };
  };

  await until("VIEW did not show its primary view", () => controls.styleOf(PRIMARY_NAME) !== null);
  await press(() => controls.sceneButton(PHASE_SCENE_NAME), `the SCENE button ${PHASE_SCENE_NAME}`);
  await until(
    `${PHASE_SCENE_NAME} was not chosen`,
    () =>
      controls.sceneButton(PHASE_SCENE_NAME)?.getAttribute("aria-pressed") === "true" &&
      controls.styleOf(PRIMARY_NAME) !== null,
  );
  const phases: ViewsCheckPhaseRecord[] = [];
  const resized: ViewsCheckResizeRecord[] = [];
  await step(VIEWS_CHECK_PLAN, async (plan) => {
    await configure(plan);
    await wait(timing.settleMs);
    await api.startPhase(plan.name);
    await wait(timing.guardMs);
    const startMs = deps.nowMs();
    await wait(timing.measureMs);
    const window = { startMs, endMs: deps.nowMs() };
    deps.markWindow(window);
    // The trace stops at the window's end; the pass timer's last reads arrive after it.
    await api.endPhase(plan.name, window);
    await wait(timing.graceMs);
    phases.push(probe.phase(plan.name, window.startMs, window.endMs, shownViews(controls)));
    if (plan.name === COCKPIT_PHASE) {
      resized.push(await resizePrimary());
    }
  });
  const resize = resized[0];
  const cockpit = VIEWS_CHECK_PLAN.find((plan) => plan.name === COCKPIT_PHASE);
  if (resize === undefined || cockpit === undefined) {
    throw new Error(`views check: the plan has no ${COCKPIT_PHASE} phase`);
  }
  await configure(cockpit);
  await api.askRightWayUp();
  return {
    timer: probe.timer(),
    devicePixelRatio: deps.devicePixelRatio,
    phases,
    resize,
    faults: probe.faults(),
    perCanvasOverheadMs: PER_CANVAS_OVERHEAD_MS,
  };
}

/** What {@link recordViewsCheck} asks of the main process. */
export type ViewsCheckEnding = Pick<ViewsCheckApi, "writeResults" | "end">;

/**
 * Runs `run` and ends the check with its outcome: the record written and the run passed, or the
 * run failed with the reason it threw; nothing at all once `signal` has stopped it.
 */
export async function recordViewsCheck(
  api: ViewsCheckEnding,
  run: () => Promise<ViewsCheckRecord>,
  signal: AbortSignal,
): Promise<void> {
  try {
    const record = await run();
    if (signal.aborted) {
      return;
    }
    await api.writeResults(record);
    await api.end({ status: "pass" });
  } catch (error: unknown) {
    if (signal.aborted) {
      return;
    }
    const reason = error instanceof Error ? error.message : String(error);
    console.error("views check: the run failed:", error);
    await api.end({ status: "fail", reason });
  }
}
