/**
 * The hand-over check (plan R10, R10.T10.g; decision-r10-t16-leans §1): the terrain run's
 * check-only variant (`--views-check --terrain --handover`), which flies `VIEW`'s primary through a
 * body's hand-over and back at three looks and reads back the frames either side of each switch,
 * for its main process to judge per surveyed cell.
 *
 * @remarks
 * T10.e's scratch harness, committed (its looks, its line and its analysis), with the survey's
 * findings on it mended: the session's compile waited for, the exposure's lock verified, a cell
 * dark on one side counted, not dropped, and every cell kept in the record.
 *
 * For each look of `deps.looks` (all of {@link HANDOVER_LOOKS}, or the one `--look` names), on the
 * launch's setting:
 *
 * 1. `VIEW` is put on the look's check-only kept scene (`handoverScenes.ts`), the primary alone and
 *    photorealistic, and the run waits for the look's fresh terrain session to compile its
 *    materials (`TerrainProbe.compiled`).
 * 2. The exposure is put under `AUTO` on the `LIT` meter until its EV100 stands, within a step of
 *    its reading, for {@link HandoverCheckTiming.settleMs}, then `INHIBIT`, verified: `EXPOSURE`
 *    must read `INHIBITED · OPERATOR` at the EV100 it locked from then to the look's end (the
 *    record's `HandoverExposureRecord`; the main process fails the look where it did not hold). The
 *    read-backs are of the scene target, before the exposure's scale, so the lock keeps the
 *    captures comparable and the meter out of the frames compared.
 * 3. Once the session's coverage holds the world's survey whole, the read-backs are armed. On the
 *    way in until the terrain's first draw has been read, and on the way out until the disc's first
 *    frame after the hand-back has been read (`readBackWanted`'s windows, `handoverPairing.ts`),
 *    every primary frame's scene target is read back over the region holding the body
 *    (`bodyRect`), at most {@link HandoverCheckTiming.readBacksInFlight} at once. Each is held
 *    until the next, and only the frames either side of a regime's change, as `SwitchPairing`
 *    pairs them, are read (`readFrame`) and compared (`compareReadings`): the regime's switch from
 *    the disc (`disc-to-figure`), the terrain's first draw against the disc's last frame
 *    (`disc-to-terrain`), and the hand-back (`terrain-to-disc`). A frame's regime is that of its
 *    scene target's draws' materials ({@link regimeOf}), and every primary frame's is kept, so that a
 *    comparison states how many frames apart its two are and whether each was its regime's last or
 *    first (`pairAdjacency`). Each frame's reading flags the cells where the disc's μ floor binds
 *    at the cell's centre (plan R10, R10.T15.b; `muFloorBinds`), on the normal map the session's
 *    disc shaded that frame with ({@link TerrainProbe.normalMap}), which the main process leaves
 *    out of its per-cell bound and counts.
 * 4. Three looks of the page are captured: the disc on the way in, the terrain at the hold and the
 *    disc after the hand-back.
 *
 * The world is the scenes' (`HandoverScenes.world`): `TERRAIN TEST`'s now, a served world's for
 * T15, the run unchanged.
 */

import type {
  HandoverCheckRecord,
  HandoverComparisonRecord,
  HandoverExposureRecord,
  HandoverFrameRecord,
  HandoverLookName,
  HandoverLookRecord,
  HandoverRegime,
  HandoverSwitchName,
  QualitySettingName,
  ViewsCheckApi,
} from "../../../../../preload/api";
import { norm, type Vec3 } from "../../../geometry/vec3";
import type { NormalMapTexels } from "../../../view/bodies/discNormals";
import { DEFAULT_FOV_DEG } from "../../../view/camera/projection";
import { selectionViewport } from "../../../view/terrain/handover";
import { PHASE_SCENE_NAME } from "../../../view/scenes/phaseScene";
import { PRIMARY_NAME, PRIMARY_VIEW_NAME } from "../viewNames";
import { SERVER_SCENE_NAME } from "../viewRun";
import {
  approachDurationS,
  approachPart,
  closingTimeS,
  eyeOnLineM,
  HANDOVER_LOOKS,
  type HandoverLook,
  limbRatioAt,
  type LookGeometry,
  lookAngles,
  lookAttitude,
  texelMappingMismatches,
} from "./handoverGeometry";
import {
  bodyRect,
  compareReadings,
  frameCamera,
  type FrameReading,
  type PixelRect,
  readFrame,
} from "./handoverReading";
import {
  type FramePair,
  HANDOVER_IN_WINDOW,
  LockWatch,
  pairAdjacency,
  type PairedFrame,
  SettleWatch,
  SwitchPairing,
} from "./handoverPairing";
import { handoverRangeAtM, type HandoverScenes } from "./handoverScenes";
import { TERRAIN_LIT_PREFIX, type TerrainProbe } from "./terrainProbe";
import { viewDriver } from "./viewsCheckRun";
import type { ViewsProbe } from "./viewsProbe";

/** The primary's scene target, whose frames are read back. */
const SCENE_TARGET = `${PRIMARY_VIEW_NAME}:hdr`;

/** Pixels about the body's disc that each read-back's region takes in. */
const RECT_MARGIN_PX = 4;

/** `EXPOSURE`'s automation level once `INHIBIT` holds. */
const INHIBITED = "INHIBITED · OPERATOR";

/** The switches, in the order a look meets them. */
const SWITCHES: ReadonlyArray<HandoverSwitchName> = [
  "disc-to-figure",
  "disc-to-terrain",
  "terrain-to-disc",
];

/** How long the hand-over check waits, and how often it looks. */
export interface HandoverCheckTiming {
  /** The longest wait for a control to take effect or a session to compile, ms. */
  readonly waitMs: number;
  /** The longest wait for a look's session to hold the world's survey whole, ms. */
  readonly surveyWaitMs: number;
  /** How often a wait looks again, ms. */
  readonly pollMs: number;
  /** How long `AUTO`'s EV100 must stand unchanged before it is locked, ms. */
  readonly settleMs: number;
  /** The most read-backs in flight at once. */
  readonly readBacksInFlight: number;
  /**
   * How long a look may run past its script's length before it is given up, ms; or, where it
   * counts from the script's last step ({@link HandoverCheckTiming.overrunFrom}), how long its
   * script may stand still.
   */
  readonly overrunMs: number;
  /**
   * What the overrun counts from: `"start"`, the look's start plus its script's length, on the wall
   * clock; or `"stall"`, the script's last step, so that a look is given up only once its script has
   * stood still for the overrun (the software mode's, whose frames carry the script slowly).
   */
  readonly overrunFrom: "start" | "stall";
}

/**
 * The check's timing on a fixture's world, a full run's and a smoke's alike (the smoke's line is
 * the faster): `TERRAIN TEST`'s field is about 2 MB.
 */
export const HANDOVER_CHECK_TIMING: HandoverCheckTiming = {
  waitMs: 60_000,
  surveyWaitMs: 60_000,
  pollMs: 100,
  settleMs: 3_000,
  readBacksInFlight: 3,
  overrunMs: 60_000,
  overrunFrom: "start",
};

/**
 * The check's timing on a served world (plan R10, R10.T15.b): a look's survey waited for 5 minutes,
 * since each look's session asks the field again, and a served level-8 field is about 15 MB
 * (15,140,689 B in 58 chunks at 23, T15.b.1's record), about 7.5 times `TERRAIN TEST`'s, for the
 * workers to decode; three looks' waits then fill the 15 minutes the main process's watchdog gains
 * for a served world (`viewsCheckWatchdogMs`).
 */
export const SERVED_HANDOVER_CHECK_TIMING: HandoverCheckTiming = {
  ...HANDOVER_CHECK_TIMING,
  surveyWaitMs: 300_000,
};

/**
 * The check's timing for a launch, before the software mode's stretch: a served world's where the
 * launch names one (`ViewsCheckLaunch.served`), else a fixture's.
 */
export function handoverCheckTimingFor(served: boolean): HandoverCheckTiming {
  return served ? SERVED_HANDOVER_CHECK_TIMING : HANDOVER_CHECK_TIMING;
}

/**
 * The kept scene the check chooses before its first photorealistic press, or `null` to stay where
 * `VIEW` opened, `pressed` telling which scene is chosen.
 *
 * @remarks
 * A served launch links to the server, so that `VIEW` opens on `SERVER` with no universe open, whose
 * primary never turns photorealistic, and the check would wait there to its end (R10.T15.b.2's
 * runs); it goes to `PHASE TEST` first. A fixture's launch has no link and opens on a kept scene.
 */
export function sceneBeforeTheLooks(pressed: (scene: string) => boolean): string | null {
  return pressed(SERVER_SCENE_NAME) ? PHASE_SCENE_NAME : null;
}

/**
 * When a look is given up: once the wall clock passes its start plus its script's length and the
 * overrun, or, where the overrun counts from the script's last step
 * ({@link HandoverCheckTiming.overrunFrom}), once its script has stood still for the overrun.
 *
 * @remarks
 * A kept scene's script steps at most `MAX_FREE_STEP_S` (0.25 s) a frame, so on SwiftShader, whose
 * served frames at the high setting took about 2.4 s each in a pod, a look's script runs at about a
 * tenth of the wall clock's pace, and a look of some 270 s takes about 45 minutes (R10.T15.b.2's
 * runs). The software mode therefore counts from the last step, and the main process's watchdog
 * bounds the whole run; a GPU launch keeps the wall clock's bound.
 */
export class LookDeadline {
  readonly #timing: Pick<HandoverCheckTiming, "overrunMs" | "overrunFrom">;
  readonly #endMs: number;
  /** The script's time at its last step, s, or `null` before the first reading. */
  #lastS: number | null = null;
  /** When the script last stepped, ms: the look's start until it steps. */
  #lastStepMs: number;

  /**
   * @param startMs - When the look starts, ms on the page's clock.
   * @param durationS - Its script's length, s.
   */
  constructor(
    timing: Pick<HandoverCheckTiming, "overrunMs" | "overrunFrom">,
    startMs: number,
    durationS: number,
  ) {
    this.#timing = timing;
    this.#endMs = startMs + durationS * 1000 + timing.overrunMs;
    this.#lastStepMs = startMs;
  }

  /** Whether the look is given up at `nowMs`, ms, its script then at `scriptS`, s. */
  passed(nowMs: number, scriptS: number): boolean {
    if (this.#timing.overrunFrom === "start") {
      return nowMs > this.#endMs;
    }
    if (this.#lastS === null) {
      this.#lastS = scriptS;
    } else if (scriptS > this.#lastS) {
      this.#lastS = scriptS;
      this.#lastStepMs = nowMs;
    }
    return nowMs > this.#lastStepMs + this.#timing.overrunMs;
  }
}

/**
 * The looks a launch's run makes: all of {@link HANDOVER_LOOKS} in order, or the one `name` names
 * (`--look`; R10.T15.b.2's runs: a served look on SwiftShader at the high setting takes over an
 * hour, so that a launch a look keeps each within its watchdog and the runs resumable by look).
 */
export function handoverLooksFor(name: HandoverLookName | undefined): ReadonlyArray<HandoverLook> {
  return name === undefined ? HANDOVER_LOOKS : HANDOVER_LOOKS.filter((look) => look.name === name);
}

/** What the hand-over check drives and reads. */
export interface HandoverCheckDeps {
  readonly api: Pick<ViewsCheckApi, "capture">;
  readonly views: Pick<ViewsProbe, "watch" | "canvasOf">;
  readonly terrain: Pick<
    TerrainProbe,
    | "session"
    | "compiled"
    | "sessions"
    | "coverageCells"
    | "normalMap"
    | "readSceneTarget"
    | "faults"
  >;
  /** The looks' scenes, which `VIEW` offers, and the world they are made on. */
  readonly scenes: HandoverScenes;
  /** The looks the run makes, in order: all of {@link HANDOVER_LOOKS}, or the one `--look` names. */
  readonly looks: ReadonlyArray<HandoverLook>;
  readonly setting: QualitySettingName;
  readonly document: Document;
  readonly timing: HandoverCheckTiming;
  /** The page's clock, `performance.now()`, on which the probes stamp frames. */
  readonly nowMs: () => number;
  readonly sleep: (ms: number) => Promise<void>;
  /** Stops the run: before its next step, wait or call it throws `STOPPED` (`viewsCheckRun.ts`). */
  readonly signal: AbortSignal;
  readonly devicePixelRatio: number;
}

/**
 * A frame's regime by the materials of its scene target's draws: the terrain's lit material, the
 * smooth figure's alone, or a disc's (`HandoverRegime`).
 */
export function regimeOf(materials: Iterable<string>): HandoverRegime {
  let figure = false;
  let disc = false;
  for (const name of materials) {
    if (name.startsWith(TERRAIN_LIT_PREFIX)) {
      return "terrain";
    }
    // The smooth figure's plain form, its emissive one (R08.T18.a) or its air's (R08.T16.b).
    figure ||=
      name === "bodies:smoothMesh" ||
      name === "bodies:smoothMeshEmissive" ||
      name === "bodies:smoothMeshAir";
    disc ||= name.startsWith("bodies:disc");
  }
  return figure ? "figure" : disc ? "disc" : "none";
}

/** `EXPOSURE`'s reading (`EV100 15.1 AUTO`) taken apart: its EV100 and its automation level. */
export function parseExposureReading(
  text: string | null,
): { readonly ev100: number; readonly level: string } | null {
  const match = text === null ? null : /^EV100 (-?\d+(?:\.\d+)?) (.+)$/u.exec(text);
  const [, value, level] = match ?? [];
  return value === undefined || level === undefined ? null : { ev100: Number(value), level };
}

/** The message of a caught value. */
function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** A primary frame of a look, as its submissions drew it. */
interface DrawnFrame {
  readonly atMs: number;
  readonly scriptS: number;
  readonly regime: HandoverRegime;
}

/** A frame read back and held until the next, its reading made once it is compared. */
interface HeldFrame extends PairedFrame {
  readonly atMs: number;
  readonly widthPx: number;
  readonly heightPx: number;
  readonly rect: PixelRect;
  readonly halfBits: Uint16Array;
  /** The normal map the frame's disc was shaded with, for its μ floor; `null` for none. */
  readonly normals: NormalMapTexels | null;
  reading: FrameReading | null;
}

/** A look's state while it runs, which the frames' watcher reads and writes. */
interface LookState {
  readonly look: HandoverLook;
  readonly geometry: LookGeometry;
  readonly surveyed: ReadonlySet<number>;
  readonly frames: DrawnFrame[];
  readonly pairing: SwitchPairing<HeldFrame>;
  readonly comparisons: Map<HandoverSwitchName, HandoverComparisonRecord>;
  readonly failures: string[];
  inFlight: number;
  /** The read-backs' processing, in the order they were asked. */
  chain: Promise<void>;
}

/**
 * Runs the hand-over check in `deps.document`'s `VIEW` and returns its record.
 *
 * @throws Error naming the step when a control does not take effect within the timing's wait, a
 *   session or a survey does not come, or the run is stopped through `deps.signal` (`STOPPED`).
 */
export async function runHandoverCheck(deps: HandoverCheckDeps): Promise<HandoverCheckRecord> {
  const { scenes, terrain, timing } = deps;
  const { world, approach } = scenes;
  const driver = viewDriver(deps);
  const { until, wait, stopped } = driver;
  // The survey's own wait, a served world's field being the larger (R10.T15.b).
  const surveyDriver = viewDriver({ ...deps, timing: { ...timing, waitMs: timing.surveyWaitMs } });
  const fovXRad = (DEFAULT_FOV_DEG * Math.PI) / 180;
  let current: LookState | null = null;

  /** Whether the frame at `scriptS` is one `look` reads back. */
  const wanted = (look: LookState, scriptS: number): boolean =>
    look.pairing.wants(approachPart(approach, scriptS), limbRatioAt(approach, scriptS));

  /** The camera's eye at the script's time `scriptS`, galactic, m from the body's centre. */
  const eyeOf = (look: LookState, scriptS: number): Vec3 =>
    eyeOnLineM(look.geometry, limbRatioAt(approach, scriptS), world.eyePointM);

  /** The reading of a frame held, made once. */
  const readingOf = (look: LookState, held: HeldFrame): FrameReading => {
    if (held.reading === null) {
      held.reading = readFrame({
        halfBits: held.halfBits,
        widthPx: held.widthPx,
        heightPx: held.heightPx,
        rect: held.rect,
        camera: frameCamera(
          look.geometry,
          eyeOf(look, held.scriptS),
          lookAttitude(look.geometry),
          fovXRad,
        ),
        figure: world.figure,
        level: world.cellLevel,
        surveyed: look.surveyed,
        normals: held.normals,
      });
    }
    return held.reading;
  };

  /** A held frame as the record states it. */
  const frameRecord = (look: LookState, held: HeldFrame): HandoverFrameRecord => {
    const reading = readingOf(look, held);
    return {
      atMs: held.atMs,
      scriptS: held.scriptS,
      regime: held.regime,
      rangeRatio: limbRatioAt(approach, held.scriptS),
      target: { widthPx: held.widthPx, heightPx: held.heightPx },
      bodyPixels: reading.bodyPixels,
      bodyMisses: reading.bodyMisses,
      surveyPixels: reading.surveyPixels,
      angles: lookAngles(look.geometry, eyeOf(look, held.scriptS)),
    };
  };

  /** Compares a pair of held frames across its switch, and keeps it. */
  const compare = (look: LookState, { name, before, after }: FramePair<HeldFrame>): void => {
    try {
      const figures = compareReadings(readingOf(look, before), readingOf(look, after));
      look.comparisons.set(name, {
        name,
        before: frameRecord(look, before),
        after: frameRecord(look, after),
        ...pairAdjacency(look.frames, before, after),
        ...figures,
      });
    } catch (error: unknown) {
      look.failures.push(`${name} could not be compared: ${messageOf(error)}`);
    }
  };

  /** A frame read back, against the one read before it: a comparison at a change of regime. */
  const take = (look: LookState, held: HeldFrame): void => {
    for (const pair of look.pairing.take(held)) {
      compare(look, pair);
    }
  };

  /** A primary frame drawn while `look` runs: kept, and read back where it is wanted. */
  const onFrame = (look: LookState, atMs: number, regime: HandoverRegime): void => {
    const scriptS = scenes.scriptS(look.look);
    if (scriptS === null) {
      return;
    }
    const frame: DrawnFrame = { atMs, scriptS, regime };
    const index = look.frames.length;
    look.frames.push(frame);
    if (!wanted(look, scriptS) || look.inFlight >= timing.readBacksInFlight) {
      return;
    }
    const size = deps.views.canvasOf(SCENE_TARGET);
    const rect = bodyRect(
      size.widthPx,
      size.heightPx,
      fovXRad,
      norm(eyeOf(look, scriptS)),
      world.figure.equatorialRadiusM,
      RECT_MARGIN_PX,
    );
    look.inFlight += 1;
    // The map the disc drew this frame with: a later answer replaces it, never this one's texels.
    const normals = terrain.normalMap(world.body);
    // Asked now, in the frame's own submission, so that the copy follows its draws; settled into
    // an outcome at once, so that no refusal goes unheard while the chain is behind.
    const read = terrain.readSceneTarget(SCENE_TARGET, rect).then(
      (bytes) => ({ bytes, error: null }),
      (error: unknown) => ({ bytes: null, error }),
    );
    /** Takes the read-back once those before it are taken, a failure kept as the look's. */
    const settle = async (): Promise<void> => {
      try {
        const outcome = await read;
        if (outcome.bytes === null) {
          look.failures.push(
            `a read-back at ${scriptS.toFixed(2)} s of the script was refused: ${messageOf(outcome.error)}`,
          );
        } else {
          take(look, {
            index,
            regime,
            scriptS,
            atMs,
            widthPx: size.widthPx,
            heightPx: size.heightPx,
            rect,
            halfBits: new Uint16Array(outcome.bytes),
            normals,
            reading: null,
          });
        }
      } catch (error: unknown) {
        look.failures.push(
          `the frame at ${scriptS.toFixed(2)} s of the script could not be read: ${messageOf(error)}`,
        );
      } finally {
        look.inFlight -= 1;
      }
    };
    look.chain = look.chain.then(settle);
  };

  /** `EXPOSURE`'s reading, its panel unfolded first where the compact layout folds it. */
  const exposureReading = (): string | null => {
    driver.controls.unfoldPanel("view-exposure");
    return driver.controls.exposureReading();
  };

  /** Puts the exposure under `AUTO` on `LIT`, lets it settle and locks it: its record so far. */
  const lockExposure = async (): Promise<HandoverExposureRecord> => {
    const level = (): string | null => parseExposureReading(exposureReading())?.level ?? null;
    if (level() !== "AUTO") {
      await driver.press(() => {
        driver.controls.unfoldPanel("view-exposure");
        const button = driver.controls.exposureButton("ENABLE");
        return button?.getAttribute("aria-disabled") === "true" ? null : button;
      }, "the EXPOSURE panel's ENABLE");
      await until("AUTO did not resume on ENABLE", () => level() === "AUTO");
    }
    await driver.press(() => {
      driver.controls.unfoldPanel("view-meter");
      return driver.controls.meterButton("LIT");
    }, "the EXPOSURE METER's LIT");
    await until(
      "the LIT meter was not chosen",
      () => driver.controls.meterButton("LIT")?.getAttribute("aria-pressed") === "true",
    );
    const settling = new SettleWatch(timing.settleMs);
    await until("the exposure did not settle under AUTO", () =>
      settling.note(parseExposureReading(exposureReading()), deps.nowMs()),
    );
    await driver.press(() => {
      driver.controls.unfoldPanel("view-exposure");
      return driver.controls.exposureButton("INHIBIT");
    }, "the EXPOSURE panel's INHIBIT");
    await until(
      "INHIBIT did not lock the exposure",
      () => parseExposureReading(exposureReading())?.level === INHIBITED,
    );
    const lockedReading = exposureReading();
    return {
      lockedEv100: parseExposureReading(lockedReading)?.ev100 ?? null,
      lockedReading,
      endReading: lockedReading,
      held: true,
    };
  };

  /** Runs one look and returns its record. */
  const runLook = async (look: HandoverLook): Promise<HandoverLookRecord> => {
    if (driver.controls.sceneButton(look.scene)?.getAttribute("aria-pressed") === "true") {
      // Chosen already (by hand, before the run measured its range): left first, so that the look
      // starts afresh, its script from 0 at the range measured.
      await driver.chooseScene(PHASE_SCENE_NAME);
    }
    const sessionsBefore = terrain.sessions().length;
    await driver.chooseScene(look.scene);
    await driver.configure({ primary: "photorealistic", instruments: [null, null] });
    await until(
      `${look.scene}'s terrain session did not compile`,
      () =>
        terrain.sessions().length > sessionsBefore && terrain.session !== null && terrain.compiled,
    );
    const failures: string[] = [];
    let exposure: HandoverExposureRecord;
    try {
      exposure = await lockExposure();
    } catch (error: unknown) {
      stopped();
      failures.push(`the exposure was not locked: ${messageOf(error)}`);
      exposure = { lockedEv100: null, lockedReading: null, endReading: null, held: false };
    }
    await surveyDriver.until(
      `${look.scene}'s survey did not arrive whole`,
      () => terrain.coverageCells(world.body).size >= world.surveyedCells,
    );
    const surveyed = terrain.coverageCells(world.body);
    const geometry = scenes.geometry(look);
    const durationS = approachDurationS(approach);
    const readyS = scenes.scriptS(look) ?? 0;
    const windowS = closingTimeS(approach, HANDOVER_IN_WINDOW);
    if (readyS >= windowS) {
      failures.push(
        `the look was ready at ${readyS.toFixed(1)} s of its script, after its first window opened at ${windowS.toFixed(1)} s`,
      );
    }
    const running: LookState = {
      look,
      geometry,
      surveyed,
      frames: [],
      pairing: new SwitchPairing<HeldFrame>(),
      comparisons: new Map(),
      failures,
      inFlight: 0,
      chain: Promise.resolve(),
    };
    const captures: ReadonlyArray<{ readonly label: string; readonly atS: number }> = [
      { label: `${look.name}-disc`, atS: closingTimeS(approach, 1.3) },
      {
        label: `${look.name}-terrain`,
        atS: closingTimeS(approach, approach.closestRatio) + approach.holdS / 2,
      },
      { label: `${look.name}-back`, atS: durationS - 1.5 },
    ];
    const taken = new Set<string>();
    const deadline = new LookDeadline(timing, deps.nowMs(), durationS);
    const lock = new LockWatch(exposure.lockedReading);
    const play = async (): Promise<void> => {
      stopped();
      const tS = scenes.scriptS(look) ?? 0;
      lock.note(exposureReading());
      const due = captures.find((each) => tS >= each.atS && !taken.has(each.label));
      if (due !== undefined) {
        taken.add(due.label);
        await deps.api.capture(due.label);
      }
      if (tS >= durationS - 0.5) {
        return;
      }
      if (deadline.passed(deps.nowMs(), tS)) {
        failures.push(`the look's script stood at ${tS.toFixed(1)} s at its deadline`);
        return;
      }
      await wait(timing.pollMs);
      await play();
    };
    current = running;
    try {
      await play();
      await running.chain;
    } finally {
      current = null;
    }
    const terrainIndex = running.frames.findIndex((frame) => frame.regime === "terrain");
    const deg = 180 / Math.PI;
    return {
      name: look.name,
      scene: look.scene,
      judged: look.judged,
      nominal: {
        incidenceDeg: look.incidenceRad * deg,
        emissionDeg: look.emissionRad * deg,
        phaseDeg: (look.incidenceRad + look.emissionRad) * deg,
      },
      atRange: lookAngles(geometry, eyeOnLineM(geometry, 1, world.eyePointM)),
      exposure: { ...exposure, endReading: lock.endReading, held: lock.held },
      surveyedCells: surveyed.size,
      mappingMismatches: texelMappingMismatches(surveyed, world.cellLevel),
      switchesS: {
        figure:
          running.frames.find((frame) => frame.regime === "figure" || frame.regime === "terrain")
            ?.scriptS ?? null,
        terrain: running.frames[terrainIndex]?.scriptS ?? null,
        disc:
          terrainIndex < 0
            ? null
            : (running.frames.find((frame, i) => i > terrainIndex && frame.regime === "disc")
                ?.scriptS ?? null),
      },
      comparisons: SWITCHES.flatMap((name) => {
        const comparison = running.comparisons.get(name);
        return comparison === undefined ? [] : [comparison];
      }),
      failures: [...failures, ...running.pairing.failures],
    };
  };

  /** Runs `deps.looks[index]` and each after it, each on the page the one before left. */
  const runFrom = async (index: number): Promise<HandoverLookRecord[]> => {
    stopped();
    const look = deps.looks[index];
    if (look === undefined) {
      return [];
    }
    const record = await runLook(look);
    return [record, ...(await runFrom(index + 1))];
  };

  let frameAtMs: number | null = null;
  const materials = new Set<string>();
  const unwatch = deps.views.watch({
    madeTarget: () => undefined,
    submitted: (owner, frame, frameMs) => {
      if (frameMs === null) {
        return;
      }
      if (frameMs !== frameAtMs) {
        frameAtMs = frameMs;
        materials.clear();
      }
      if (owner === SCENE_TARGET) {
        for (const draw of frame.draws) {
          materials.add(draw.material.name);
        }
      } else if (owner === PRIMARY_VIEW_NAME && current !== null) {
        // The canvas's pass is the frame's last: its scene target is drawn whole.
        onFrame(current, frameMs, regimeOf(materials));
      }
    },
  });
  try {
    await until(
      "VIEW did not show its primary view",
      () => driver.controls.styleOf(PRIMARY_NAME) !== null,
    );
    const opening = sceneBeforeTheLooks(
      (name) => driver.controls.sceneButton(name)?.getAttribute("aria-pressed") === "true",
    );
    if (opening !== null) {
      await driver.chooseScene(opening);
    }
    await driver.configure({ primary: "photorealistic", instruments: [null, null] });
    await until(
      "VIEW did not size its primary and its scene target",
      () =>
        deps.views.canvasOf(PRIMARY_VIEW_NAME).widthPx > 0 &&
        deps.views.canvasOf(SCENE_TARGET).widthPx > 0,
    );
    const canvas = deps.views.canvasOf(PRIMARY_VIEW_NAME);
    const target = deps.views.canvasOf(SCENE_TARGET);
    // The hand-over's own pixel and τ at the primary's canvas, the presented size
    // (decision-r10-t16-leans §13), at its CSS px, the selection's (fix-retina-frame), as the
    // terrain run's `terrainHandover` takes them.
    const { rangeM, tauPx, radPerPx } = handoverRangeAtM(
      world,
      deps.setting,
      selectionViewport(canvas, deps.devicePixelRatio).widthPx,
    );
    scenes.setRangeM(rangeM);
    const looks = await runFrom(0);
    return {
      world: world.name,
      devicePixelRatio: deps.devicePixelRatio,
      canvas,
      target,
      reliefM: world.reliefM,
      tauPx,
      radPerPx,
      rangeM,
      cellLevel: world.cellLevel,
      approach: { ...approach },
      looks,
      faults: terrain.faults(),
    };
  } finally {
    unwatch();
  }
}

/** What {@link recordHandoverCheck} asks of the main process. */
export type HandoverCheckEnding = Pick<ViewsCheckApi, "writeHandoverResults" | "end">;

/**
 * Runs `run` and ends the check with its outcome: the record written (the main process judges it)
 * and the run ended, or the run failed with the reason it threw; nothing at all once `signal` has
 * stopped it.
 */
export async function recordHandoverCheck(
  api: HandoverCheckEnding,
  run: () => Promise<HandoverCheckRecord>,
  signal: AbortSignal,
): Promise<void> {
  try {
    const record = await run();
    if (signal.aborted) {
      return;
    }
    await api.writeHandoverResults(record);
    await api.end({ status: "pass" });
  } catch (error: unknown) {
    if (signal.aborted) {
      return;
    }
    console.error("views check: the hand-over check failed:", error);
    await api.end({ status: "fail", reason: messageOf(error) });
  }
}
