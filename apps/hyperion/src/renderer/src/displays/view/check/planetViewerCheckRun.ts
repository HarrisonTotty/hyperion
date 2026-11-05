/**
 * The planet viewer run (plan R15, T6): the `PLANET VIEWER`'s planet captured from orbit, from low
 * orbit and on its surface, driven through the consoles as an operator would.
 *
 * @remarks
 * 1. The first universe the server lists is opened on `GALAXY`, and the `PLANET VIEWER` shown.
 * 2. `GENERATE` shows the panel's inputs as they stand at mount, an Earth about a Sun, in `VIEW`'s
 *    `ORBIT` camera. Once it is drawn photorealistic, lit, its atmosphere and its ground surveyed,
 *    and `AUTO` settled, the page is captured as `planet-viewer-orbit`.
 * 3. The wheel zooms the camera to {@link LOW_ORBIT_HEIGHT_M} above the ground at its line of sight,
 *    which it looks down along at the centre, read on the lower plate as `SLANT RANGE FROM CAMERA`
 *    beside the ground's `ELEVATION`, from which the steps above the datum are worked out (the
 *    camera zooms its altitude above the datum, `orbitZoomed`, and is held above it, so over ground
 *    below the datum it goes to the height above the datum); once the terrain has streamed and
 *    `AUTO` settled, the page is captured as `planet-viewer-low-orbit`.
 * 4. Zoomed again to {@link SURFACE_HEIGHT_M} above the ground, the look is tilted to the horizon
 *    under `R`, held on the view's canvas, and the page is captured as `planet-viewer-surface`.
 *
 * Each capture is taken twice: of the whole tab, and with `MAXIMIZE VIEW` on (`-maximized`), its
 * label block and panels hidden, so that the planet fills the view. Where the lower plate holds its
 * readout back for the room it is read under `MAXIMIZE VIEW` too. The run fails,
 * naming its step, where the server refuses what it asks, where a control does not take effect or
 * a reading does not come within its wait, where the tab overflows the window, where the primary's
 * label block shows a fault line or
 * `LIGHTING: NOT RECEIVED`, where a capture's frames drew no lit body larger than a point, where the
 * planet's ground is not modelled, or where it is stopped (`STOPPED`).
 */
import type { RequestClient, UniverseInfo } from "@hyperion/protocol";

import type { ViewsCheckApi } from "../../../../../preload/api";
import { ATMOSPHERE_STATEMENTS } from "../../../view/atmosphere/labels";
import { ORBIT_ZOOM_FACTOR } from "../../../view/camera/orbit";
import { GENERATE_KEY } from "../../planet/PlanetInputsPanel";
import { MAXIMIZE_KEY } from "../displayKeys";
import { PRIMARY_NAME, PRIMARY_VIEW_NAME } from "../viewNames";
import {
  exposureEv100,
  primaryCanvas,
  primaryPlate,
  type PrimaryPlate,
} from "./atmosphereCheckRun";
import {
  inspectionGround,
  inspectionLighting,
  LitBodyDraws,
  lowerPlateLines,
  readoutHeldBack,
  slantRangeM,
} from "./inspectionCheckRun";
import { viewDriver } from "./viewsCheckRun";
import type { FrameSubmission } from "../../../view/engine/types";
import { type SubmissionWatcher, viewOf, type ViewsProbe } from "./viewsProbe";

/** How long each part of the planet viewer run may take. */
export interface PlanetViewerCheckTiming {
  /** The longest wait for the link, and for a control to take effect, ms. */
  readonly waitMs: number;
  /** The longest wait for the planet's first photorealistic frames, lit, its air not pending, ms. */
  readonly drawWaitMs: number;
  /** How often a wait looks again, ms. */
  readonly pollMs: number;
  /** How far apart the `EXPOSURE` line's EV is read while `AUTO` settles, ms. */
  readonly settleStepMs: number;
  /** How near two readings in a row must be for `AUTO` to count as settled, EV. */
  readonly settleToleranceEv: number;
  /** The longest wait for `AUTO` to settle before a capture, ms: past it the run fails. */
  readonly settleMaxMs: number;
  /** The primary's draws waited for before a capture, and after each zoom. */
  readonly framesBeforeCapture: number;
  /** The longest wait for the ground to read surveyed, and for the terrain to stream, ms. */
  readonly groundWaitMs: number;
  /** The most zooms the camera is brought to a height by before the run fails. */
  readonly zoomTries: number;
  /**
   * The primary's draws `R` is held for, at least, to tilt the look to the horizon: each draw
   * turns it by at most `MAX_FREE_STEP_S` of its π/4 rad/s, so 25 turn it past π/2.
   */
  readonly tiltFrames: number;
  /** The least time `R` is held for, ms: the camera's two seconds to the horizon, with room. */
  readonly tiltMs: number;
}

/** The run's timing: a minute for a control, three for the first photorealistic frames. */
export const PLANET_VIEWER_CHECK_TIMING: PlanetViewerCheckTiming = {
  waitMs: 60_000,
  drawWaitMs: 180_000,
  pollMs: 100,
  settleStepMs: 1_000,
  settleToleranceEv: 0.1,
  settleMaxMs: 60_000,
  framesBeforeCapture: 5,
  groundWaitMs: 300_000,
  zoomTries: 6,
  tiltFrames: 25,
  tiltMs: 3_000,
};

/** The low orbit's height above the ground at the line of sight, m: 400 km, a station's. */
export const LOW_ORBIT_HEIGHT_M = 400_000;

/** The surface look's height above the ground at the line of sight, m: 300 m, a low flight's. */
export const SURFACE_HEIGHT_M = 300;

/** How near its goal a zoom must bring the camera, as a share of the goal's height: 10%. */
export const HEIGHT_TOLERANCE = 0.1;

/** The run's captures, in the order it takes them, each of the whole tab and `-maximized`. */
export const PLANET_VIEWER_CAPTURES = [
  "planet-viewer-orbit",
  "planet-viewer-low-orbit",
  "planet-viewer-surface",
] as const;

/** One of the run's captures. */
export type PlanetViewerCaptureLabel = (typeof PLANET_VIEWER_CAPTURES)[number];

/** A wheel's pixels in a zoom step (`ViewCanvas`'s `wheelSteps`, in pixels). */
const WHEEL_PX_PER_STEP = 100;

/** What the planet viewer run drives and reads. */
export interface PlanetViewerCheckDeps {
  readonly api: Pick<ViewsCheckApi, "capture">;
  /** The views probe: the primary's draws, and its submissions, by which a body is seen drawn. */
  readonly views: Pick<ViewsProbe, "drawTimes" | "watch">;
  /** The link's request client, through which the run lists the server's universes. */
  readonly requests: Pick<RequestClient, "request">;
  readonly document: Document;
  readonly nowMs: () => number;
  readonly sleep: (ms: number) => Promise<void>;
  /** Stops the run: before its next step, wait or call it throws `STOPPED` (`viewsCheckRun.ts`). */
  readonly signal: AbortSignal;
  readonly timing: PlanetViewerCheckTiming;
}

/**
 * The ground's elevation above the datum at the line of sight as the lower plate's `ELEVATION` line
 * reads it (`ELEVATION ~2140 m ± 180 m`, `ELEVATION -1.25 km ± 0.02 km`), m, or `null` where it reads
 * none.
 */
export function elevationM(lines: ReadonlyArray<string>): number | null {
  for (const line of lines) {
    const match = /^ELEVATION ~?([+-]?[\d,]+(?:\.\d+)?) (m|km)\b/u.exec(line);
    const digits = match?.[1];
    if (digits !== undefined) {
      return Number(digits.replaceAll(",", "")) * (match?.[2] === "km" ? 1000 : 1);
    }
  }
  return null;
}

/**
 * The altitude above the datum a camera is brought to for `goalM` above ground standing `groundM`
 * above the datum, m: `goalM` above the ground, or above the datum where the ground lies below it,
 * a sea's floor perhaps, since `ORBIT` holds the camera above the datum.
 */
export function goalAltitudeM(groundM: number, goalM: number): number {
  return goalM + Math.max(groundM, 0);
}

/**
 * The wheel's zoom steps, nearer for a positive one, that bring a camera at `altitudeM` above the
 * datum to `goalAltitude` above it: `ORBIT` scales the altitude above the datum by
 * {@link ORBIT_ZOOM_FACTOR} a step, so the steps are the log of the altitudes' ratio.
 */
export function zoomStepsTo(altitudeM: number, goalAltitudeMetres: number): number {
  return Math.log(altitudeM / goalAltitudeMetres) / Math.log(ORBIT_ZOOM_FACTOR);
}

/**
 * How far the console's frame overflows the window, CSS px, 0 where it fits or is not shown: the
 * guide's layout holds every display within the window, with no page scroll.
 */
export function consoleOverflowPx(
  frame: Pick<Element, "scrollHeight" | "clientHeight"> | null,
): number {
  return frame === null ? 0 : Math.max(frame.scrollHeight - frame.clientHeight, 0);
}

/** The most of the primary's passes and materials a failure names. */
const NAMED_DRAWS = 24;

/**
 * The passes and materials the primary drew in its last frame, as the views probe tells its
 * submissions, which a capture's failure names: `bodies late: terrain-lit-cluster-…`.
 */
export class PrimaryDraws implements SubmissionWatcher {
  #frameMs: number | null = null;
  #draws = new Set<string>();

  submitted(
    owner: string,
    frame: Pick<FrameSubmission, "label" | "draws">,
    frameMs: number | null,
  ): void {
    if (frameMs === null || viewOf(owner) !== PRIMARY_VIEW_NAME) {
      return;
    }
    if (frameMs !== this.#frameMs) {
      this.#frameMs = frameMs;
      this.#draws = new Set();
    }
    for (const draw of frame.draws) {
      this.#draws.add(`${frame.label}: ${draw.material.name}`);
    }
  }

  madeTarget(): void {
    // Its targets say nothing of what it draws.
  }

  /** What the primary's last frame drew, at most {@link NAMED_DRAWS} of it, or `none`. */
  named(): string {
    const draws = [...this.#draws].slice(0, NAMED_DRAWS);
    return draws.length === 0 ? "none" : draws.join(", ");
  }
}

/** A key's press or release, dispatched on `target` as a keyboard would. */
function keyOn(target: EventTarget, type: "keydown" | "keyup", key: string): void {
  target.dispatchEvent(new KeyboardEvent(type, { key, bubbles: true, cancelable: true }));
}

/** A key's press and release on `target`. */
function pressKey(target: EventTarget, key: string): void {
  keyOn(target, "keydown", key);
  keyOn(target, "keyup", key);
}

/**
 * Runs the planet viewer run in `deps.document`'s consoles and returns its captures' labels,
 * watching the views probe's submissions for the lit bodies the primary draws.
 *
 * @throws Error naming the step where it cannot go on (the module's remarks).
 */
export async function runPlanetViewerCheck(
  deps: PlanetViewerCheckDeps,
): Promise<ReadonlyArray<PlanetViewerCaptureLabel>> {
  const bodies = new LitBodyDraws();
  const draws = new PrimaryDraws();
  const unwatch = deps.views.watch(bodies);
  const unwatchDraws = deps.views.watch(draws);
  try {
    return await viewPlanet(deps, bodies, draws);
  } finally {
    unwatchDraws();
    unwatch();
  }
}

/**
 * {@link runPlanetViewerCheck}'s steps, `bodies` the lit bodies the primary draws and `draws` what
 * its last frame drew.
 */
async function viewPlanet(
  deps: PlanetViewerCheckDeps,
  bodies: LitBodyDraws,
  draws: PrimaryDraws,
): Promise<ReadonlyArray<PlanetViewerCaptureLabel>> {
  const { document, timing } = deps;
  const { stopped, wait, until, press, controls } = viewDriver(deps);
  const drawn = viewDriver({ ...deps, timing: { ...timing, waitMs: timing.drawWaitMs } });
  const grounded = viewDriver({ ...deps, timing: { ...timing, waitMs: timing.groundWaitMs } });
  const plate = (): PrimaryPlate => {
    const found = primaryPlate(document);
    if (found === null) {
      throw new Error("views check: the primary's label block is not shown");
    }
    return found;
  };
  const ground = (): ReadonlyArray<string> => lowerPlateLines(document);
  const maximized = (): boolean =>
    document.querySelector(".planet-viewer .view__side")?.hasAttribute("hidden") === true;
  /** Turns `MAXIMIZE VIEW` on or off by its key, as an operator does, and waits for it. */
  const setMaximized = async (on: boolean): Promise<void> => {
    if (maximized() === on) {
      return;
    }
    stopped();
    pressKey(document.body, MAXIMIZE_KEY);
    await until(
      `MAXIMIZE VIEW did not ${on ? "hide" : "show"} the panels`,
      () => maximized() === on,
    );
  };
  /** Waits for `count` of the primary's draws from now; returns when it began, ms. */
  const framesDrawn = async (count = timing.framesBeforeCapture, leastMs = 0): Promise<number> => {
    const fromMs = deps.nowMs();
    await drawn.until(
      `the primary did not draw ${String(count)} frames`,
      () =>
        deps.views.drawTimes(PRIMARY_VIEW_NAME, fromMs, Number.POSITIVE_INFINITY).length >= count &&
        deps.nowMs() - fromMs >= leastMs,
    );
    return fromMs;
  };
  /** Waits for `AUTO` to settle, from now, within the timing's limit. */
  const settle = async (): Promise<void> => {
    const fromMs = deps.nowMs();
    const read = (): number => {
      const line = plate().line("EXPOSURE");
      const ev = exposureEv100(line);
      if (ev === null) {
        throw new Error(`views check: the EXPOSURE line read no EV (${line ?? "none"})`);
      }
      return ev;
    };
    const settled = async (beforeEv: number): Promise<void> => {
      await wait(timing.settleStepMs);
      const ev = read();
      if (Math.abs(ev - beforeEv) <= timing.settleToleranceEv + 1e-9) {
        return;
      }
      if (deps.nowMs() - fromMs >= timing.settleMaxMs) {
        throw new Error(
          `views check: AUTO did not settle within ${String(timing.settleMaxMs / 1000)} s (EV100 ${beforeEv.toFixed(1)}, then ${ev.toFixed(1)})`,
        );
      }
      await settled(ev);
    };
    await settled(read());
  };
  /** The ground at the line of sight once it reads, under `MAXIMIZE VIEW` where it is held back. */
  const groundRead = async (what: string): Promise<ReadonlyArray<string>> => {
    if (readoutHeldBack(ground())) {
      await setMaximized(true);
    }
    await grounded.until(
      `the ground at the line of sight did not read ${what}`,
      () => inspectionGround(ground()) !== "waiting",
      () => ground().join("; "),
    );
    const lines = ground();
    if (inspectionGround(lines) === "not modelled") {
      throw new Error("views check: the planet's ground is not modelled, so it has no surface");
    }
    return lines;
  };
  /** Zooms the camera to `goalM` above the ground at its line of sight, by the wheel. */
  const zoomTo = async (goalM: number, what: string): Promise<void> => {
    const tryZoom = async (tries: number): Promise<void> => {
      const lines = await groundRead(what);
      const heightM = slantRangeM(lines);
      const groundM = elevationM(lines);
      if (heightM === null || groundM === null) {
        throw new Error(`views check: the lower plate reads no height (${lines.join("; ")})`);
      }
      const altitudeM = heightM + groundM;
      const goalAltitude = goalAltitudeM(groundM, goalM);
      if (!(altitudeM > 0)) {
        throw new Error(`views check: the camera stands at no altitude (${lines.join("; ")})`);
      }
      if (Math.abs(altitudeM - goalAltitude) <= HEIGHT_TOLERANCE * goalM) {
        return;
      }
      if (tries >= timing.zoomTries) {
        throw new Error(
          `views check: the camera did not come to ${String(goalM)} m above the ground ${what} (at ${String(Math.round(heightM))} m)`,
        );
      }
      const canvas = primaryCanvas(document);
      if (canvas === null) {
        throw new Error("views check: the primary's canvas is not shown");
      }
      stopped();
      const steps = zoomStepsTo(altitudeM, goalAltitude);
      canvas.dispatchEvent(
        new WheelEvent("wheel", {
          deltaY: -steps * WHEEL_PX_PER_STEP,
          deltaMode: 0,
          bubbles: true,
          cancelable: true,
        }),
      );
      await framesDrawn();
      await tryZoom(tries + 1);
    };
    await tryZoom(0);
  };
  /**
   * Fails unless the label block holds no fault and the lighting held, and, where `litBody` asks,
   * a lit body was drawn; else the lack of one is logged with what was drawn.
   */
  const shown = (what: string, fromMs: number, litBody: "required" | "logged"): void => {
    const said = plate();
    if (said.faults.length > 0) {
      throw new Error(`views check: ${what} shows faults: ${said.faults.join("; ")}`);
    }
    if (inspectionLighting(said.statements) !== "lit") {
      throw new Error(`views check: ${what} is not lit (${said.statements.join("; ")})`);
    }
    if (!bodies.drewSince(fromMs)) {
      const why = `${what} drew no lit body larger than a point (it drew ${draws.named()})`;
      if (litBody === "required") {
        throw new Error(`views check: ${why}`);
      }
      console.warn(`views check: ${why}`);
    }
  };
  /**
   * Shows the whole tab again, lets the terrain stream, `AUTO` settle and the frames come, checks
   * them and captures; then the same with `MAXIMIZE VIEW` on, and off again.
   */
  const capture = async (
    label: PlanetViewerCaptureLabel,
    what: string,
    litBody: "required" | "logged",
  ): Promise<void> => {
    await setMaximized(false);
    await grounded.until(
      `the terrain did not stream ${what}`,
      () => !plate().statements.includes("TERRAIN: STREAMING"),
      () => plate().statements.join("; "),
    );
    await settle();
    const fromMs = await framesDrawn();
    // Captured before it is judged, so that a failed run leaves the picture of what failed.
    stopped();
    await deps.api.capture(label);
    shown(what, fromMs, litBody);
    const overflowPx = consoleOverflowPx(document.querySelector(".console"));
    if (overflowPx > 0) {
      throw new Error(
        `views check: the PLANET VIEWER does not fit the window: it overflows by ${String(overflowPx)} px`,
      );
    }
    await setMaximized(true);
    const maximizedFromMs = await framesDrawn();
    stopped();
    await deps.api.capture(`${label}-maximized`);
    shown(`${what} under MAXIMIZE VIEW`, maximizedFromMs, litBody);
    await setMaximized(false);
  };

  // 1. The first universe opened on GALAXY, and the PLANET VIEWER shown.
  const deadlineMs = deps.nowMs() + timing.waitMs;
  /** The server's universes, asked until the link answers, within the wait. */
  const listUniverses = async (): Promise<ReadonlyArray<UniverseInfo>> => {
    stopped();
    const listed = await deps.requests.request({ kind: "list_universes" }).outcome;
    if (listed.ok) {
      return listed.response.universes;
    }
    if (listed.error.code !== "link_lost" || deps.nowMs() > deadlineMs) {
      throw new Error(`views check: list_universes was refused: ${listed.error.message}`);
    }
    await wait(timing.pollMs * 10);
    return listUniverses();
  };
  const [universe] = await listUniverses();
  if (universe === undefined) {
    throw new Error("views check: the server lists no universe");
  }
  pressKey(document.body, "F2");
  const openButton = (): HTMLButtonElement | null =>
    document.querySelector<HTMLButtonElement>(
      `button[aria-label="Open universe ${CSS.escape(universe.name)}"]`,
    );
  await press(openButton, `GALAXY's OPEN UNIVERSE for ${universe.name}`);
  await until(`${universe.name} was not opened`, () => openButton() === null);
  pressKey(document.body, "F5");
  const generate = (): HTMLButtonElement | null =>
    document.querySelector<HTMLButtonElement>(
      `.planet-inputs button[aria-keyshortcuts="${GENERATE_KEY.toUpperCase()}"]`,
    );
  await until("the PLANET VIEWER was not shown", () => generate() !== null);

  // 2. The panel's inputs generated, drawn photorealistic from orbit, and captured.
  await until("GENERATE was held back", () => generate()?.getAttribute("aria-disabled") !== "true");
  await press(generate, "GENERATE");
  await drawn.until(
    "the planet was not drawn photorealistic",
    () => controls.styleOf(PRIMARY_NAME) === "photorealistic",
  );
  await drawn.until(
    "the planet's lighting and atmosphere stayed pending",
    () =>
      inspectionLighting(plate().statements) !== "pending" &&
      !plate().statements.includes(ATMOSPHERE_STATEMENTS.atmospherePending),
  );
  await groundRead("from orbit");
  await capture("planet-viewer-orbit", "the planet from orbit", "required");

  // 3. Down to low orbit, and captured.
  await zoomTo(LOW_ORBIT_HEIGHT_M, "in low orbit");
  await capture("planet-viewer-low-orbit", "the planet from low orbit", "required");

  // 4. Down to the surface, the look tilted to the horizon, and captured.
  await zoomTo(SURFACE_HEIGHT_M, "near the surface");
  await setMaximized(false);
  const canvas = primaryCanvas(document);
  if (canvas === null) {
    throw new Error("views check: the primary's canvas is not shown");
  }
  canvas.focus();
  stopped();
  keyOn(canvas, "keydown", "r");
  try {
    await framesDrawn(timing.tiltFrames, timing.tiltMs);
  } finally {
    keyOn(canvas, "keyup", "r");
  }
  // Close over the ground the guard's materials may not be drawn at all, over a sea above all:
  // there it is logged with what was drawn, and the capture judged by its faults and lighting.
  await capture("planet-viewer-surface", "the planet's surface", "logged");
  return PLANET_VIEWER_CAPTURES;
}

/** What {@link recordPlanetViewerCheck} asks of the main process. */
export type PlanetViewerCheckEnding = Pick<ViewsCheckApi, "end">;

/**
 * Runs `run` and ends the check with its outcome: passed once its captures are taken, or failed
 * with the reason it threw; nothing at all once `signal` has stopped it.
 */
export async function recordPlanetViewerCheck(
  api: PlanetViewerCheckEnding,
  run: () => Promise<ReadonlyArray<PlanetViewerCaptureLabel>>,
  signal: AbortSignal,
): Promise<void> {
  let failure: string | null = null;
  try {
    await run();
  } catch (error: unknown) {
    if (signal.aborted) {
      return;
    }
    console.error("views check: the planet viewer run failed:", error);
    failure = error instanceof Error ? error.message : String(error);
  }
  if (signal.aborted) {
    return;
  }
  await api.end(failure === null ? { status: "pass" } : { status: "fail", reason: failure });
}
