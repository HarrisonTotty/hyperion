/**
 * The views check's inspection run (`--views-check --inspection --universe <ID> --body <ID>`; the
 * owner's request, 2026-11-01): a served body opened from the `SYSTEM` display through its orbit
 * map's `VIEW BODY`, as an operator opens it, and `VIEW`'s photorealistic body inspection of it
 * captured for the looks by eye.
 *
 * @remarks
 * In order:
 *
 * 1. The universe's name and the body's system, as a range query's row describes it, are asked of
 *    the server once the link is up (`list_universes`, `resolve_system` at the epoch).
 * 2. The universe is opened on `GALAXY` (`F2`, its row's `OPEN UNIVERSE`), and the system on
 *    `SYSTEM` through `App`'s opening, as `OPEN SYSTEM` opens a charted system, at the epoch.
 * 3. On `SYSTEM` the body's row is selected in the body list, `FOCUS BODY` pressed, which focuses it
 *    or a moon's planet, and `VIEW BODY` pressed, which opens the body in `VIEW`'s inspection.
 * 4. In `VIEW`, once the primary draws photorealistically and no `ATMOSPHERE: PENDING` stands, the
 *    exposure is put under `AUTO` (`ENABLE`), and once `AUTO` has settled and a few frames are drawn
 *    the page is captured as `inspection-<system>-<index>`.
 *
 * The run fails where a step does not take effect within its wait, where `VIEW BODY` is held back
 * (with its reason), or where the primary's label block shows a fault line as it is captured.
 */

import {
  type BodyIdHex,
  galacticDeltaLy,
  galacticPositionFromLy,
  parseBodyId,
  type RequestClient,
  type RequestKind,
  type RequestOf,
  type ResponseFor,
  type UniverseTime,
} from "@hyperion/protocol";

import type { ServedWorldNames, ViewsCheckApi } from "../../../../../preload/api";
import { vec3 } from "../../../geometry/vec3";
import { ATMOSPHERE_STATEMENTS } from "../../../view/atmosphere/labels";
import { INSPECTION_SCENE_NAME } from "../../../view/scenes/inspectionScene";
import type { SystemTarget } from "../../system/systemTarget";
import { PRIMARY_NAME, PRIMARY_VIEW_NAME } from "../viewNames";
import { exposureEv100, primaryPlate, type PrimaryPlate } from "./atmosphereCheckRun";
import { viewDriver } from "./viewsCheckRun";
import type { ViewsProbe } from "./viewsProbe";

/** How long each part of the inspection run may take. */
export interface InspectionCheckTiming {
  /** The longest wait for the link, and for a control to take effect, ms. */
  readonly waitMs: number;
  /**
   * The longest wait for the primary to draw photorealistically with nothing of its atmosphere
   * pending, ms: its pipelines compile, and the body's record and its optics are made.
   */
  readonly drawWaitMs: number;
  /** How often a wait looks again, ms. */
  readonly pollMs: number;
  /** How far apart the `EXPOSURE` line's EV is read while `AUTO` settles, ms. */
  readonly settleStepMs: number;
  /** How near two readings in a row must be for `AUTO` to count as settled, EV. */
  readonly settleToleranceEv: number;
  /** The longest wait for `AUTO` to settle before the capture, ms: past it the run fails. */
  readonly settleMaxMs: number;
  /** The primary's draws waited for before the capture. */
  readonly framesBeforeCapture: number;
}

/** The run's timing: a minute for a control, three for the first photorealistic frames. */
export const INSPECTION_CHECK_TIMING: InspectionCheckTiming = {
  waitMs: 60_000,
  drawWaitMs: 180_000,
  pollMs: 100,
  settleStepMs: 1_000,
  settleToleranceEv: 0.1,
  settleMaxMs: 60_000,
  framesBeforeCapture: 5,
};

/** The instant the system is opened at: the epoch, within every universe's clock window. */
const EPOCH: UniverseTime = { seconds: 0, nanos: 0 };

/** The galactic frame's origin, from which a system's position is read in light-years. */
const GALACTIC_ORIGIN = galacticPositionFromLy([0, 0, 0]);

/** What the inspection run drives and reads. */
export interface InspectionCheckDeps {
  readonly api: Pick<ViewsCheckApi, "capture">;
  readonly views: Pick<ViewsProbe, "drawTimes">;
  /** The link's request client, through which the run asks the server what it opens. */
  readonly requests: Pick<RequestClient, "request">;
  /** The universe and the body the launch names. */
  readonly served: ServedWorldNames;
  /** Opens the `SYSTEM` display on a system (`App`'s, as `OPEN SYSTEM` calls it). */
  readonly openSystem: (target: SystemTarget) => void;
  readonly document: Document;
  readonly nowMs: () => number;
  readonly sleep: (ms: number) => Promise<void>;
  /** Stops the run: before its next step, wait or call it throws `STOPPED` (`viewsCheckRun.ts`). */
  readonly signal: AbortSignal;
  readonly timing: InspectionCheckTiming;
}

/** What the run captured, and what the primary's label block said as it was, for its tests. */
export interface InspectionCapture {
  readonly label: string;
  readonly frame: string | null;
  readonly exposure: string | null;
  readonly statements: ReadonlyArray<string>;
}

/** The capture's label for `body`: `inspection-<system>-<index in hexadecimal>`. */
export function inspectionCaptureLabel(body: BodyIdHex): string {
  const { system, bodyIndex } = parseBodyId(body);
  return `inspection-${system}-${bodyIndex.toString(16).padStart(4, "0")}`;
}

/** A key's press and release, dispatched on `target` as a keyboard would. */
function pressKey(target: EventTarget, key: string): void {
  target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
  target.dispatchEvent(new KeyboardEvent("keyup", { key, bubbles: true, cancelable: true }));
}

/** The orbit map's button whose key is `key`, or `null`. */
function mapButton(document: Document, key: string): HTMLButtonElement | null {
  return document.querySelector<HTMLButtonElement>(
    `.orbit-map__head button[aria-keyshortcuts="${key}"]`,
  );
}

/**
 * Runs the inspection run in `deps.document`'s consoles and returns what it captured.
 *
 * @throws Error naming the step when the server refuses what is asked, when a control does not take
 *   effect within its wait, when `VIEW BODY` is held back, when `AUTO` does not settle, when a fault
 *   line stands on the primary's label block, or when the run is stopped through `deps.signal`
 *   (`STOPPED`).
 */
export async function runInspectionCheck(deps: InspectionCheckDeps): Promise<InspectionCapture> {
  const { document, timing, served } = deps;
  const driver = viewDriver(deps);
  const { stopped, wait, until, press, controls } = driver;
  const drawn = viewDriver({ ...deps, timing: { ...timing, waitMs: timing.drawWaitMs } });

  /** Asks `body` of the server until the link answers it, within the wait. */
  const asked = async <K extends RequestKind>(body: RequestOf<K>): Promise<ResponseFor<K>> => {
    const deadline = deps.nowMs() + timing.waitMs;
    const ask = async (): Promise<ResponseFor<K>> => {
      stopped();
      const outcome = await deps.requests.request(body).outcome;
      if (outcome.ok) {
        return outcome.response;
      }
      if (outcome.error.code !== "link_lost" || deps.nowMs() > deadline) {
        throw new Error(
          `views check: ${body.kind} was refused: ${outcome.error.code}: ${outcome.error.message}`,
        );
      }
      await wait(timing.pollMs * 10);
      return ask();
    };
    return ask();
  };
  const plate = (): PrimaryPlate => {
    const found = primaryPlate(document);
    if (found === null) {
      throw new Error("views check: the primary's label block is not shown");
    }
    return found;
  };

  // 1. What the launch names, as the server describes it.
  const { system } = parseBodyId(served.body);
  const listed = await asked({ kind: "list_universes" });
  const universe = listed.universes.find((each) => each.id === served.universe);
  if (universe === undefined) {
    throw new Error(`views check: the server lists no universe ${served.universe}`);
  }
  const resolved = await asked({
    kind: "resolve_system",
    universe: universe.id,
    system,
    time: EPOCH,
  });
  const record = resolved.record;

  // 2. The universe opened on GALAXY, and the system on SYSTEM.
  pressKey(document.body, "F2");
  const openButton = (): HTMLButtonElement | null =>
    document.querySelector<HTMLButtonElement>(
      `button[aria-label="Open universe ${CSS.escape(universe.name)}"]`,
    );
  await press(openButton, `GALAXY's OPEN UNIVERSE for ${universe.name}`);
  await until(`${universe.name} was not opened`, () => openButton() === null);
  const [x, y, z] = galacticDeltaLy(GALACTIC_ORIGIN, record.position);
  stopped();
  deps.openSystem({
    universe: universe.id,
    system,
    designation: record.designation,
    positionLy: vec3(x, y, z),
    time: EPOCH,
    // No chart's census stands behind the opening: the orbit map sizes its stars from the lightest
    // band, which `VIEW` does not read.
    bands: [],
  });

  // 3. The body selected, focused and opened in VIEW.
  const row = (): HTMLElement | null =>
    document.querySelector<HTMLElement>(
      `[role="tree"] [role="treeitem"][data-id="${served.body}"]`,
    );
  await until(`SYSTEM did not list ${served.body}`, () => row() !== null);
  stopped();
  row()?.click();
  await until(
    `SYSTEM did not select ${served.body}`,
    () => row()?.getAttribute("aria-selected") === "true",
  );
  const focus = (): HTMLButtonElement | null => mapButton(document, "C");
  if (focus()?.getAttribute("aria-pressed") !== "true") {
    await press(focus, "FOCUS BODY");
  }
  await until(
    "FOCUS BODY did not focus the body's planet",
    () => focus()?.getAttribute("aria-pressed") === "true",
    () => controls.reasonOf(focus()),
  );
  const viewBody = (): HTMLButtonElement | null => mapButton(document, "V");
  await until(
    "VIEW BODY was held back",
    () => viewBody() !== null && viewBody()?.getAttribute("aria-disabled") !== "true",
    () => controls.reasonOf(viewBody()),
  );
  await press(viewBody, "VIEW BODY");
  await until(
    `${INSPECTION_SCENE_NAME} was not chosen`,
    () => controls.sceneButton(INSPECTION_SCENE_NAME)?.getAttribute("aria-pressed") === "true",
  );

  // 4. The inspection drawn, its exposure under AUTO and settled, and captured.
  await drawn.until(
    "the inspection was not drawn photorealistic",
    () => controls.styleOf(PRIMARY_NAME) === "photorealistic",
  );
  await drawn.until(
    "the inspection's atmosphere stayed pending",
    () => !plate().statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
  );
  const enableOffered = (): boolean => {
    const button = controls.exposureButton("ENABLE");
    return button !== null && button.getAttribute("aria-disabled") !== "true";
  };
  controls.unfoldPanel("view-exposure");
  await until("the EXPOSURE panel's ENABLE was not offered", enableOffered);
  stopped();
  controls.exposureButton("ENABLE")?.click();
  await until("AUTO did not resume on ENABLE", () => !enableOffered());
  const read = (): number => {
    const line = plate().line("EXPOSURE");
    const ev = exposureEv100(line);
    if (ev === null) {
      throw new Error(`views check: the EXPOSURE line read no EV (${line ?? "none"})`);
    }
    return ev;
  };
  const settleFromMs = deps.nowMs();
  const settled = async (beforeEv: number): Promise<void> => {
    await wait(timing.settleStepMs);
    const ev = read();
    // The readings are to a tenth: the allowance keeps a tenth's step from failing on rounding.
    if (Math.abs(ev - beforeEv) <= timing.settleToleranceEv + 1e-9) {
      return;
    }
    if (deps.nowMs() - settleFromMs >= timing.settleMaxMs) {
      throw new Error(
        `views check: AUTO did not settle within ${String(timing.settleMaxMs / 1000)} s (EV100 ${beforeEv.toFixed(1)}, then ${ev.toFixed(1)})`,
      );
    }
    await settled(ev);
  };
  await settled(read());
  const fromMs = deps.nowMs();
  await until(
    `the primary did not draw ${String(timing.framesBeforeCapture)} frames`,
    () =>
      deps.views.drawTimes(PRIMARY_VIEW_NAME, fromMs, Number.POSITIVE_INFINITY).length >=
      timing.framesBeforeCapture,
  );
  const said = plate();
  if (said.faults.length > 0) {
    throw new Error(`views check: the inspection shows faults: ${said.faults.join("; ")}`);
  }
  const label = inspectionCaptureLabel(served.body);
  stopped();
  await deps.api.capture(label);
  return {
    label,
    frame: said.line("FRAME"),
    exposure: said.line("EXPOSURE"),
    statements: said.statements,
  };
}

/** What {@link recordInspectionCheck} asks of the main process. */
export type InspectionCheckEnding = Pick<ViewsCheckApi, "end">;

/**
 * Runs `run` and ends the check with its outcome: passed once its capture is taken, or failed with
 * the reason it threw; nothing at all once `signal` has stopped it.
 */
export async function recordInspectionCheck(
  api: InspectionCheckEnding,
  run: () => Promise<InspectionCapture>,
  signal: AbortSignal,
): Promise<void> {
  try {
    await run();
    if (signal.aborted) {
      return;
    }
    await api.end({ status: "pass" });
  } catch (error: unknown) {
    if (signal.aborted) {
      return;
    }
    const reason = error instanceof Error ? error.message : String(error);
    console.error("views check: the inspection run failed:", error);
    await api.end({ status: "fail", reason });
  }
}
