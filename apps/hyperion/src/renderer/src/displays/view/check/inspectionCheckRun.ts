/**
 * The views check's inspection run (`--views-check --inspection --universe <ID> --body <ID>`; the
 * owner's request, 2026-11-01): a served body opened from the `SYSTEM` display through its orbit
 * map's `VIEW BODY`, as an operator opens it, and `VIEW`'s photorealistic body inspection of it
 * captured for the looks by eye.
 *
 * @remarks
 * In order:
 *
 * 1. The universe's name, the body's system, as a range query's row describes it, and the body's
 *    record are asked of the server once the link is up (`list_universes`, `resolve_system` and
 *    `body_detail` at the epoch).
 * 2. The universe is opened on `GALAXY` (`F2`, its row's `OPEN UNIVERSE`), and the system on
 *    `SYSTEM` through `App`'s opening, as `OPEN SYSTEM` opens a charted system, at the epoch.
 * 3. On `SYSTEM` the body's row is selected in the body list, `FOCUS BODY` pressed, which focuses it
 *    or a moon's planet, and `VIEW BODY` pressed, which opens the body in `VIEW`'s inspection.
 * 4. In `VIEW`, once the primary draws photorealistically and no `ATMOSPHERE: PENDING` stands, and
 *    no `LIGHTING: PENDING` either, its sky's host discs having arrived (fix-retina-frame, after
 *    the owner's Mac profile, whose low orbit window measured an unlit, empty frame), the
 *    exposure must stand at `AUTO`, which the inspection opens on, and, for a body whose record
 *    (`body_detail`, asked in step 1) has a surface, the ground at the line of sight on the lower
 *    plate must read the body's surveyed ground (`ELEVATION`), the inspection having surveyed it as
 *    it opened, or the plate must say `TERRAIN: NOT YET MODELLED`; a giant states none. Where the
 *    room holds the readout back for the picture (the plates' cap, the owner's delegated call,
 *    2026-11-01; {@link readoutHeldBack}), the ground is read under `MAXIMIZE VIEW`, which gives
 *    the picture back with the readout standing, as its capture is taken. Once
 *    `AUTO` has settled and a few frames are drawn the page is captured as
 *    `inspection-<system>-<index>`; then with `MAXIMIZE VIEW` on, the panels hidden and the stage
 *    the display's whole body, as `inspection-<system>-<index>-maximized` (the owner's request,
 *    2026-11-01), and with it off again; then with the `STYLE` panel's `GRID` at `OFF`, the image
 *    bare, as `inspection-<system>-<index>-grid-off`.
 * 5. Over surveyed ground the camera then closes in along its line of sight, the flight key `W`
 *    held on the primary's canvas in short holds at a rate step (`PageUp`, `PageDown`) whose one
 *    step flies at most half of what is left, each read back from the `POSITION` line once its coast
 *    has all but run out,
 *    until it stands {@link CLOSE_ALTITUDE_SHARE} of the body's radius above it (at least
 *    {@link CLOSE_ALTITUDE_MIN_M}); once the terrain has streamed, its ground is read, `AUTO` has
 *    settled and a few frames are drawn, it is captured as `inspection-<system>-<index>-close`;
 *    where the room holds the readout back there, its ground is read under `MAXIMIZE VIEW` after.
 * 6. With the frame profile alone, the camera closes in on by the same holds to
 *    {@link LOW_HEIGHT_M} above the ground at its line of sight (`SLANT RANGE FROM CAMERA`), where
 *    the shadow cascades draw, and once the terrain has streamed and `AUTO` has settled its third
 *    window is measured; where the room holds the readout back, the slant range is read under
 *    `MAXIMIZE VIEW` on the way down, which is turned off again before the window.
 *
 * The run fails where a step does not take effect within its wait, where `VIEW BODY` is held back
 * (with its reason), where the primary's label block shows a fault line as it is captured, or as
 * it is captured maximized, where it states `LIGHTING: NOT RECEIVED` or `LIGHTING: PENDING` as a
 * capture is taken or a window begins, or where the primary drew no lit body larger than a point
 * in a capture's frames or a window's ({@link LitBodyDraws}).
 *
 * With the frame profile (`--timing`; the owner's report, 2026-11-01) a window of frames is
 * measured as the first capture is taken, from orbit, another as the close capture is, and a
 * third low over the ground (`inspectionProfile.ts`), each once `LIGHTING: PENDING` is waited out,
 * its record holding the label block's statements as it ended, and the main process writes them
 * once the run has passed.
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

import type {
  InspectionTimingWindowName,
  ServedWorldNames,
  ViewsCheckApi,
} from "../../../../../preload/api";
import { vec3 } from "../../../geometry/vec3";
import { ATMOSPHERE_STATEMENTS } from "../../../view/atmosphere/labels";
import { MAX_FREE_STEP_S } from "../../../view/camera/freeCamera";
import type { FrameSubmission } from "../../../view/engine/types";
import { LIGHTING_NOT_RECEIVED, LIGHTING_PENDING } from "../../../view/lighting/hostLights";
import { PHOTOREAL_PASS_LABELS } from "../../../view/photoreal/passes";
import { INSPECTION_SCENE_NAME } from "../../../view/scenes/inspectionScene";
import { TERRAIN_NOT_MODELLED } from "../../../view/terrain/readouts";
import { FOCUS_KEY, VIEW_BODY_KEY } from "../../system/OrbitMapPanel";
import type { SystemTarget } from "../../system/systemTarget";
import { MAXIMIZE_KEY } from "../displayKeys";
import { PRIMARY_NAME, PRIMARY_VIEW_NAME } from "../viewNames";
import {
  exposureEv100,
  METRES_PER_UNIT,
  positionRangeM,
  primaryCanvas,
  primaryPlate,
  type PrimaryPlate,
  rateStepOfCameraLine,
} from "./atmosphereCheckRun";
import type { InspectionProfiler, ProfiledPlate } from "./inspectionProfile";
import { viewDriver } from "./viewsCheckRun";
import { type SubmissionWatcher, viewOf, type ViewsProbe } from "./viewsProbe";

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
  /**
   * The longest wait for the ground at the line of sight to read surveyed, and for the terrain to
   * stream once the camera has closed in, ms: the survey recorded, the field computed and sent,
   * the patches baked.
   */
  readonly groundWaitMs: number;
  /** The most holds the camera closes in by before the run fails. */
  readonly closeHolds: number;
  /**
   * The primary's draws waited for after each hold, each a step's most of the camera's time, for
   * its coast to all but run out: it damps with that time constant, so two leave under a fifth of
   * it to come, short of the goal still.
   */
  readonly coastFrames: number;
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
  groundWaitMs: 300_000,
  closeHolds: 40,
  coastFrames: 2,
};

/** How far above the body the camera closes in to, as a share of its equatorial radius: 5%. */
export const CLOSE_ALTITUDE_SHARE = 0.05;

/** The least height above the body the camera closes in to, m: 30 km, above a small body's peaks. */
export const CLOSE_ALTITUDE_MIN_M = 30_000;

/** How near its goal a hold leaves the camera before the run stops, as a share of its height. */
export const CLOSE_TOLERANCE = 0.2;

/**
 * How high above the ground at its line of sight the camera closes in to for the frame profile's
 * low window, m (main's ask, 2026-11-01; R14.T6.b): 4 km, `TERRAIN TEST`'s lowest
 * (`TERRAIN_LOWEST_ALTITUDE_M`), where R10.T9.b's cascades shade `TEST MOON`'s craters. From orbit
 * and at {@link CLOSE_ALTITUDE_SHARE} of the radius their weight is 0 (`cascadeWeight`: cascade 1's
 * texel no finer than the horizon map's), so this window is the one that times `terrain shadows`.
 */
export const LOW_HEIGHT_M = 4_000;

/** The instant the system is opened at: the epoch, within every universe's clock window. */
const EPOCH: UniverseTime = { seconds: 0, nanos: 0 };

/** The galactic frame's origin, from which a system's position is read in light-years. */
const GALACTIC_ORIGIN = galacticPositionFromLy([0, 0, 0]);

/** What the inspection run drives and reads. */
export interface InspectionCheckDeps {
  readonly api: Pick<ViewsCheckApi, "capture">;
  /** The views probe: the primary's draws, and its submissions, by which a body is seen drawn. */
  readonly views: Pick<ViewsProbe, "drawTimes" | "watch">;
  /** The link's request client, through which the run asks the server what it opens. */
  readonly requests: Pick<RequestClient, "request">;
  /** The universe and the body the launch names; `null` fails the run, which then asks nothing. */
  readonly served: ServedWorldNames | null;
  /** Opens the `SYSTEM` display on a system (`App`'s, as `OPEN SYSTEM` calls it). */
  readonly openSystem: (target: SystemTarget) => void;
  readonly document: Document;
  readonly nowMs: () => number;
  readonly sleep: (ms: number) => Promise<void>;
  /** Stops the run: before its next step, wait or call it throws `STOPPED` (`viewsCheckRun.ts`). */
  readonly signal: AbortSignal;
  readonly timing: InspectionCheckTiming;
  /**
   * The frame profile's windows (`--timing`; the owner's report), measured from orbit and closed
   * in; `null` on a launch without it.
   */
  readonly profile: Pick<InspectionProfiler, "measure"> | null;
}

/** What the run captured, and what the primary's label block said as it was, for its tests. */
export interface InspectionCapture {
  /**
   * The capture's label, with `GRID` at `ON`; the maximized one's adds `-maximized`, and the one
   * with `GRID` at `OFF` `-grid-off`.
   */
  readonly label: string;
  readonly frame: string | null;
  readonly exposure: string | null;
  readonly statements: ReadonlyArray<string>;
  /** The lower plate's lines as it was captured: the survey line, `DATUM` and the ground. */
  readonly ground: ReadonlyArray<string>;
  /** The lower plate's lines as the close capture was taken, or `null` where none was. */
  readonly closeGround: ReadonlyArray<string> | null;
}

/** What the ground at the line of sight reads: surveyed, not modelled, or neither yet. */
export type InspectionGround = "surveyed" | "not modelled" | "waiting";

/**
 * What the lower plate's lines say of the ground at the line of sight: `surveyed` with an
 * `ELEVATION` reading, `not modelled` where the plate says `TERRAIN: NOT YET MODELLED`, and
 * `waiting` otherwise, `NOT SURVEYED` among them.
 */
export function inspectionGround(lines: ReadonlyArray<string>): InspectionGround {
  if (lines.includes(TERRAIN_NOT_MODELLED)) {
    return "not modelled";
  }
  return lines.some((line) => line.startsWith("ELEVATION ")) ? "surveyed" : "waiting";
}

/**
 * What the label block's statements say of the view's lighting (`lightingStatement`): `pending`
 * while `LIGHTING: PENDING` stands, `not received` while `LIGHTING: NOT RECEIVED` does, and `lit`
 * with neither.
 */
export function inspectionLighting(
  statements: ReadonlyArray<string>,
): "lit" | "pending" | "not received" {
  if (statements.includes(LIGHTING_PENDING)) {
    return "pending";
  }
  return statements.includes(LIGHTING_NOT_RECEIVED) ? "not received" : "lit";
}

/**
 * The beginnings of the names of the materials a lit body larger than a point is drawn with: its
 * disc's and limb's (`BODY_DISC_MATERIALS` and their emissive and air forms), its figure's mesh
 * (`SMOOTH_MESH_MATERIAL` and its forms) and its terrain's lit patches (`terrain-lit-…`).
 */
const LIT_BODY_MATERIALS = ["bodies:disc", "bodies:smoothMesh", "terrain-lit-"] as const;

/**
 * Whether `frame`, one of a view's submissions, draws a lit body larger than a point: a draw in
 * its photorealistic `bodies` or `discs` pass (`PHOTOREAL_PASS_LABELS`) of one of
 * {@link LIT_BODY_MATERIALS}. A body drawn as a point, or beyond the frame, draws none, nor do the
 * host discs, whose material is the sky's.
 */
export function drawsLitBody(frame: Pick<FrameSubmission, "label" | "draws">): boolean {
  return (
    (frame.label === PHOTOREAL_PASS_LABELS.bodies || frame.label === PHOTOREAL_PASS_LABELS.discs) &&
    frame.draws.some((draw) =>
      LIT_BODY_MATERIALS.some((prefix) => draw.material.name.startsWith(prefix)),
    )
  );
}

/**
 * The animation frames in which the primary drew a lit body larger than a point
 * ({@link drawsLitBody}), as the views probe tells its submissions (`ViewsProbe.watch`), so that a
 * capture or a window of an empty frame fails the run (fix-retina-frame).
 */
export class LitBodyDraws implements SubmissionWatcher {
  #lastFrameMs: number | null = null;

  submitted(owner: string, frame: FrameSubmission, frameMs: number | null): void {
    if (frameMs !== null && viewOf(owner) === PRIMARY_VIEW_NAME && drawsLitBody(frame)) {
      this.#lastFrameMs = Math.max(this.#lastFrameMs ?? frameMs, frameMs);
    }
  }

  madeTarget(): void {
    // Its targets say nothing of what it draws.
  }

  /** Whether the primary drew a lit body in an animation frame stamped `fromMs` or later. */
  drewSince(fromMs: number): boolean {
    return this.#lastFrameMs !== null && this.#lastFrameMs >= fromMs;
  }
}

/** The name a lower plate's yield line gives the readout it holds back (`viewRoom.ts`' `lowerYieldLine`). */
const READOUT_NAME = "GROUND AT LINE OF SIGHT";

/**
 * Whether the lower plate's lines hold its readout back for the room: a yield line naming
 * `GROUND AT LINE OF SIGHT` in its place (`viewRoom.ts`' `lowerYieldLine`), as the plates' cap
 * gives it way at 1280 × 720 (the owner's delegated call, 2026-11-01).
 */
export function readoutHeldBack(lines: ReadonlyArray<string>): boolean {
  return lines.some(
    (line) => line.startsWith("NOT AVAILABLE: no room for ") && line.includes(READOUT_NAME),
  );
}

/** Metres in each of the slant range's units (`formatSurfaceRange`'s). */
const SLANT_METRES_PER_UNIT: Readonly<Record<string, number>> = { m: 1, ...METRES_PER_UNIT };

/**
 * The camera's height above the ground at its line of sight as the lower plate's `SLANT RANGE FROM
 * CAMERA` line reads it (`4.12 km ± 0.03 km`, `~4.0 km` grazing), m, or `null` where it reads
 * none: the inspection's camera looks at the body's centre, so its line of sight is its nadir.
 */
export function slantRangeM(lines: ReadonlyArray<string>): number | null {
  for (const line of lines) {
    const match = /^SLANT RANGE(?: FROM CAMERA)? ~?([\d,]+(?:\.\d+)?) (m|km|Mm|Gm|AU)\b/u.exec(
      line,
    );
    const digits = match?.[1];
    const perUnit = match?.[2] === undefined ? undefined : SLANT_METRES_PER_UNIT[match[2]];
    if (digits !== undefined && perUnit !== undefined) {
      return Number(digits.replaceAll(",", "")) * perUnit;
    }
  }
  return null;
}

/**
 * The free camera's rate step for a hold that closes `remainingM`: the fastest whose one camera
 * step, at most {@link MAX_FREE_STEP_S} of its rate, flies no more than half of it, so that a hold
 * of a draw or two leaves the camera short of its goal; between 0 and 31.
 */
export function closeRateStep(remainingM: number): number {
  const step = Math.floor(2 * Math.log10(remainingM / 2 / MAX_FREE_STEP_S));
  return Math.min(Math.max(step, 0), 31);
}

/** The lower plate's lines (`.view-label--lower`, by the primary canvas's description), as read. */
export function lowerPlateLines(document: Document): ReadonlyArray<string> {
  const ids = primaryCanvas(document)?.getAttribute("aria-describedby")?.split(" ") ?? [];
  const id = ids.find((each) => each.endsWith("-lower"));
  const plate = id === undefined ? null : document.getElementById(id);
  if (plate === null) {
    return [];
  }
  return [
    ...plate.querySelectorAll(
      ".view-label__line, .view-label__heading, .view-label__statement, .request-status__text",
    ),
  ].map((line) => {
    const label = line.querySelector("dt")?.textContent.trim();
    const value = line.querySelector("dd")?.textContent.replaceAll(/\s+/gu, " ").trim();
    return label === undefined || value === undefined
      ? line.textContent.replaceAll(/\s+/gu, " ").trim()
      : `${label} ${value}`;
  });
}

/**
 * The free camera's rate step as the display reads it: its `CAMERA` line's, or where no such line
 * stands, as under `MAXIMIZE VIEW`, which hides it with the upper block, the camera panel's rate
 * reading, which the hidden side column still holds (lane perf-render-thread-ground: the 4 km
 * window's descent goes on under `MAXIMIZE VIEW` where the room holds the readout back).
 */
export function cameraRateStep(document: Document, cameraLine: string | null): number | null {
  if (cameraLine !== null) {
    return rateStepOfCameraLine(cameraLine);
  }
  const reading = document
    .querySelector('output[aria-label="Free camera rate"]')
    ?.textContent.replaceAll(/\s+/gu, " ")
    .trim();
  return reading === undefined ? null : rateStepOfCameraLine(`FREE · ${reading}`);
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
 * Runs the inspection run in `deps.document`'s consoles and returns what it captured, watching the
 * views probe's submissions for the lit bodies the primary draws while it runs.
 *
 * @throws Error naming the step when the server refuses what is asked, when a control does not take
 *   effect within its wait, when `VIEW BODY` is held back, when `AUTO` does not settle, when a fault
 *   line stands on the primary's label block, when its lighting is pending past the draw's wait or
 *   not received, when a capture's or a window's frames drew no lit body, or when the run is stopped
 *   through `deps.signal` (`STOPPED`).
 */
export async function runInspectionCheck(deps: InspectionCheckDeps): Promise<InspectionCapture> {
  const bodies = new LitBodyDraws();
  const unwatch = deps.views.watch(bodies);
  try {
    return await inspect(deps, bodies);
  } finally {
    unwatch();
  }
}

/** {@link runInspectionCheck}'s steps, `bodies` the lit bodies the primary draws. */
async function inspect(
  deps: InspectionCheckDeps,
  bodies: LitBodyDraws,
): Promise<InspectionCapture> {
  const { document, timing } = deps;
  const served = deps.served;
  if (served === null) {
    throw new Error("views check: the inspection launch names no universe and body");
  }
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
  /**
   * Waits out `LIGHTING: PENDING` within the draw's wait, the sky's first reply with its host discs
   * (its census may take its cores first), and fails on `LIGHTING: NOT RECEIVED`.
   */
  const lit = async (what: string): Promise<void> => {
    await drawn.until(
      `${what}'s lighting stayed pending`,
      () => inspectionLighting(plate().statements) !== "pending",
    );
    if (inspectionLighting(plate().statements) === "not received") {
      throw new Error(`views check: ${what} states ${LIGHTING_NOT_RECEIVED}`);
    }
  };
  /** Fails unless `said` states the lighting held and the primary drew a lit body since `fromMs`. */
  const shown = (what: string, said: PrimaryPlate, fromMs: number): void => {
    const lighting = inspectionLighting(said.statements);
    if (lighting !== "lit") {
      throw new Error(
        `views check: ${what} states ${lighting === "pending" ? LIGHTING_PENDING : LIGHTING_NOT_RECEIVED}`,
      );
    }
    if (!bodies.drewSince(fromMs)) {
      throw new Error(`views check: ${what} drew no lit body larger than a point`);
    }
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
  // The body's record, whose surface section says whether the view states its ground at all, and
  // whose figure gives the radius the camera closes in to.
  const { record: detail } = await asked({
    kind: "body_detail",
    universe: universe.id,
    body: served.body,
    time: EPOCH,
    detail: "full",
  });

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
  const focus = (): HTMLButtonElement | null => mapButton(document, FOCUS_KEY);
  if (focus()?.getAttribute("aria-pressed") !== "true") {
    await press(focus, "FOCUS BODY");
  }
  await until(
    "FOCUS BODY did not focus the body's planet",
    () => focus()?.getAttribute("aria-pressed") === "true",
    () => controls.reasonOf(focus()),
  );
  const viewBody = (): HTMLButtonElement | null => mapButton(document, VIEW_BODY_KEY);
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

  // 4. The inspection drawn, on AUTO from its opening, its ground surveyed, and captured.
  await drawn.until(
    "the inspection was not drawn photorealistic",
    () => controls.styleOf(PRIMARY_NAME) === "photorealistic",
  );
  await drawn.until(
    "the inspection's atmosphere stayed pending",
    () => !plate().statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
  );
  await lit("the inspection");
  await drawn.until(
    "the inspection did not open on AUTO",
    () => plate().line("EXPOSURE")?.endsWith(" AUTO") === true,
    () => plate().line("EXPOSURE") ?? "no EXPOSURE line",
  );
  const grounded = viewDriver({ ...deps, timing: { ...timing, waitMs: timing.groundWaitMs } });
  const ground = (): ReadonlyArray<string> => lowerPlateLines(document);
  // A body with no surface, a giant, states no ground: no lower plate stands for it.
  const hasSurface = detail.surface.state === "ok";
  if (hasSurface) {
    await grounded.until(
      "the ground at the line of sight did not read surveyed",
      () => inspectionGround(ground()) !== "waiting" || readoutHeldBack(ground()),
      () => ground().join("; "),
    );
  }
  const read = (): number => {
    const line = plate().line("EXPOSURE");
    const ev = exposureEv100(line);
    if (ev === null) {
      throw new Error(`views check: the EXPOSURE line read no EV (${line ?? "none"})`);
    }
    return ev;
  };
  /** Waits for `AUTO` to settle, from now, within {@link InspectionCheckTiming.settleMaxMs}. */
  const settle = async (): Promise<void> => {
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
  };
  await settle();
  /**
   * Waits for `count` of the primary's draws from now, and for at least `leastMs` to pass; returns
   * when it began, ms.
   */
  const framesDrawn = async (count = timing.framesBeforeCapture, leastMs = 0): Promise<number> => {
    const fromMs = deps.nowMs();
    await until(
      `the primary did not draw ${String(count)} frames`,
      () =>
        deps.views.drawTimes(PRIMARY_VIEW_NAME, fromMs, Number.POSITIVE_INFINITY).length >= count &&
        deps.nowMs() - fromMs >= leastMs,
    );
    return fromMs;
  };
  /** Measures the profile's window `name`, lit, and fails where its frames drew no lit body. */
  const measured = async (name: InspectionTimingWindowName, what: string): Promise<void> => {
    const profile = deps.profile;
    if (profile === null) {
      return;
    }
    await lit(what);
    const fromMs = deps.nowMs();
    await profile.measure(name, profiledPlate);
    shown(what, plate(), fromMs);
  };
  const profiledPlate = (): ProfiledPlate => {
    const now = plate();
    return { position: now.line("POSITION"), statements: now.statements };
  };
  const capturedFromMs = await framesDrawn();
  const said = plate();
  if (said.faults.length > 0) {
    throw new Error(`views check: the inspection shows faults: ${said.faults.join("; ")}`);
  }
  shown("the inspection", said, capturedFromMs);
  const groundSaid = ground();
  const label = inspectionCaptureLabel(served.body);
  stopped();
  await deps.api.capture(label);
  await measured("orbit", "the orbit window");
  // MAXIMIZE VIEW by its key, as an operator turns it on, captured, and off again.
  const maximized = (): boolean =>
    document.querySelector(".view__side")?.hasAttribute("hidden") === true;
  /** The ground at the line of sight, under `MAXIMIZE VIEW`, once it reads anything but waiting. */
  const groundMaximized = async (what: string): Promise<ReadonlyArray<string>> => {
    await grounded.until(
      `the ground at the line of sight did not read under MAXIMIZE VIEW ${what}`,
      () => inspectionGround(ground()) !== "waiting",
      () => ground().join("; "),
    );
    return ground();
  };
  /** Turns `MAXIMIZE VIEW` on or off by its key, as an operator does, and waits for it. */
  const setMaximized = async (on: boolean, what: string): Promise<void> => {
    stopped();
    pressKey(document.body, MAXIMIZE_KEY);
    await until(
      `MAXIMIZE VIEW did not ${on ? "hide" : "show"} the panels ${what}`,
      () => maximized() === on,
    );
  };
  pressKey(document.body, MAXIMIZE_KEY);
  await until("MAXIMIZE VIEW did not hide the panels", maximized);
  const maximizedFromMs = await framesDrawn();
  // Where the room held the readout back for the picture, MAXIMIZE VIEW gives it back standing.
  const groundRead = readoutHeldBack(groundSaid) ? await groundMaximized("from orbit") : groundSaid;
  const saidMaximized = plate();
  if (saidMaximized.faults.length > 0) {
    throw new Error(
      `views check: the maximized inspection shows faults: ${saidMaximized.faults.join("; ")}`,
    );
  }
  shown("the maximized inspection", saidMaximized, maximizedFromMs);
  stopped();
  await deps.api.capture(`${label}-maximized`);
  pressKey(document.body, MAXIMIZE_KEY);
  await until("MAXIMIZE VIEW did not show the panels again", () => !maximized());
  const gridOff = (): HTMLButtonElement | null =>
    [...document.querySelectorAll<HTMLButtonElement>(".view-style__grid button")].find(
      (button) => button.textContent.trim() === "OFF",
    ) ?? null;
  await press(gridOff, "the STYLE panel's GRID OFF");
  await until("GRID did not go OFF", () => gridOff()?.getAttribute("aria-pressed") === "true");
  const gridOffFromMs = await framesDrawn();
  shown("the inspection with GRID OFF", plate(), gridOffFromMs);
  stopped();
  await deps.api.capture(`${label}-grid-off`);
  const orbit = {
    label,
    frame: said.line("FRAME"),
    exposure: said.line("EXPOSURE"),
    statements: said.statements,
    ground: groundRead,
  };
  if (inspectionGround(groundRead) !== "surveyed") {
    return { ...orbit, closeGround: null };
  }

  // 5. Closed in over the surveyed ground, and captured.
  const radiusM =
    detail.figure?.state === "ok"
      ? detail.figure.value.equatorial_radius_m
      : detail.bulk.state === "ok"
        ? detail.bulk.value.radius_m
        : null;
  if (radiusM === null) {
    throw new Error(`views check: ${served.body}'s record gives no radius to close in to`);
  }
  const altitudeM = Math.max(CLOSE_ALTITUDE_SHARE * radiusM, CLOSE_ALTITUDE_MIN_M);
  const goalM = radiusM + altitudeM;
  const canvas = (): HTMLCanvasElement => {
    const found = primaryCanvas(document);
    if (found === null) {
      throw new Error("views check: the primary's canvas is not shown");
    }
    return found;
  };
  const range = (): number => {
    const line = plate().line("POSITION");
    const rangeM = positionRangeM(line);
    if (rangeM === null) {
      throw new Error(`views check: the POSITION line reads no range (${line ?? "none"})`);
    }
    return rangeM;
  };
  /** Sets the free camera's rate to `wanted` through `PageUp` and `PageDown` on its canvas. */
  const setRate = async (wanted: number): Promise<void> => {
    const now = cameraRateStep(document, plate().line("CAMERA"));
    if (now === null) {
      throw new Error("views check: the CAMERA line reads no FREE rate");
    }
    if (now === wanted) {
      return;
    }
    stopped();
    pressKey(canvas(), now < wanted ? "PageUp" : "PageDown");
    await until(
      `the rate did not step from ${String(now)}`,
      () => cameraRateStep(document, plate().line("CAMERA")) !== now,
    );
    await setRate(wanted);
  };
  /**
   * Closes in by holds of `W`, or of `S` past the goal, until what `remaining` reads of the goal is
   * within `toleranceM`, at most `left` holds; `missed` says where it stood when it gave up.
   */
  const closeIn = async (
    remaining: () => number,
    toleranceM: number,
    missed: () => string,
    left: number,
  ): Promise<void> => {
    const remainingM = remaining();
    if (Math.abs(remainingM) <= toleranceM) {
      return;
    }
    if (left === 0) {
      throw new Error(`views check: ${missed()} in ${String(timing.closeHolds)} holds`);
    }
    await setRate(closeRateStep(Math.abs(remainingM)));
    const key = remainingM > 0 ? "w" : "s";
    const target = canvas();
    stopped();
    target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    try {
      // A step's most of the camera's time, a draw or more, on a slow renderer or a fast one.
      await framesDrawn(1, 1000 * MAX_FREE_STEP_S);
    } finally {
      target.dispatchEvent(new KeyboardEvent("keyup", { key, bubbles: true, cancelable: true }));
    }
    await framesDrawn(timing.coastFrames, 1000 * timing.coastFrames * MAX_FREE_STEP_S);
    await closeIn(remaining, toleranceM, missed, left - 1);
  };
  await closeIn(
    () => range() - goalM,
    CLOSE_TOLERANCE * altitudeM,
    () =>
      `the camera did not close to ${String(Math.round(goalM / 1000))} km from the centre (at ${String(Math.round(range() / 1000))} km)`,
    timing.closeHolds,
  );
  await grounded.until(
    "the terrain did not stream once the camera closed in",
    () =>
      !plate().statements.includes("TERRAIN: STREAMING") &&
      (inspectionGround(ground()) === "surveyed" || readoutHeldBack(ground())),
    () => [...plate().statements, ...ground()].join("; "),
  );
  await lit("the close inspection");
  await settle();
  const closeFromMs = await framesDrawn();
  const close = plate();
  if (close.faults.length > 0) {
    throw new Error(`views check: the close inspection shows faults: ${close.faults.join("; ")}`);
  }
  shown("the close inspection", close, closeFromMs);
  const closeShown = ground();
  stopped();
  await deps.api.capture(`${label}-close`);
  await measured("close", "the close window");
  let closeGround = closeShown;
  if (readoutHeldBack(closeShown)) {
    await setMaximized(true, "closed in");
    closeGround = await groundMaximized("closed in");
    await setMaximized(false, "closed in");
    if (inspectionGround(closeGround) !== "surveyed") {
      throw new Error(
        `views check: the ground closed in did not read surveyed under MAXIMIZE VIEW (${closeGround.join("; ")})`,
      );
    }
  }
  // 6. With the frame profile, low over the same ground, where the cascades draw (main's ask).
  if (deps.profile !== null) {
    const aboveGround = (): number => {
      const lines = ground();
      const heightM = slantRangeM(lines);
      if (heightM === null) {
        throw new Error(`views check: the lower plate reads no slant range (${lines.join("; ")})`);
      }
      return heightM;
    };
    // The slant range is the readout's: read under MAXIMIZE VIEW where the room holds it back.
    const heldLow = readoutHeldBack(ground());
    if (heldLow) {
      await setMaximized(true, "on the way down");
      await groundMaximized("on the way down");
    }
    await closeIn(
      () => aboveGround() - LOW_HEIGHT_M,
      CLOSE_TOLERANCE * LOW_HEIGHT_M,
      () =>
        `the camera did not close to ${String(LOW_HEIGHT_M / 1000)} km above the ground (at ${String(Math.round(aboveGround()))} m)`,
      timing.closeHolds,
    );
    if (heldLow) {
      await setMaximized(false, "low over the ground");
    }
    await grounded.until(
      "the terrain did not stream once the camera stood low over the ground",
      () => !plate().statements.includes("TERRAIN: STREAMING"),
      () => plate().statements.join("; "),
    );
    await settle();
    await measured("low", "the low window");
  }
  return { ...orbit, closeGround };
}

/** What {@link recordInspectionCheck} asks of the main process. */
export type InspectionCheckEnding = Pick<ViewsCheckApi, "end">;

/** A failure's reason, logged with what failed. */
function failureOf(error: unknown, what: string): string {
  console.error(`views check: ${what}:`, error);
  return error instanceof Error ? error.message : String(error);
}

/**
 * Runs `run` and ends the check with its outcome: passed once its capture is taken, and with the
 * frame profile once its timing file is written, or failed with the reason it threw; nothing at all
 * once `signal` has stopped it.
 *
 * @remarks
 * The frame profile's windows are written whether the run passed or not, once any was measured: a
 * run that fails at a later step, as a slow machine's may wait out a step's frames, keeps the
 * profile it took (the owner's report, 2026-11-01).
 *
 * @param profile - The frame profile whose windows the run measured, written before the end;
 *   `null` on a launch without it.
 */
export async function recordInspectionCheck(
  api: InspectionCheckEnding,
  run: () => Promise<InspectionCapture>,
  signal: AbortSignal,
  profile: Pick<InspectionProfiler, "windows" | "write"> | null,
): Promise<void> {
  let failure: string | null = null;
  try {
    await run();
  } catch (error: unknown) {
    if (signal.aborted) {
      return;
    }
    failure = failureOf(error, "the inspection run failed");
  }
  if (signal.aborted) {
    return;
  }
  if (profile !== null && profile.windows.length > 0) {
    try {
      await profile.write();
    } catch (error: unknown) {
      failure ??= failureOf(error, "the frame profile was not written");
    }
  }
  await api.end(failure === null ? { status: "pass" } : { status: "fail", reason: failure });
}
