/**
 * The views check's terrain run (plan R10, R10.T16.d; decision-r10-t16-contracts §6): it drives
 * `VIEW` on the kept `TERRAIN TEST` by its controls, as T20's script does on `PHASE TEST`, through
 * its setting's phases, forces a device loss mid-stream, tears the session down, and gathers the
 * record the main process judges and writes.
 *
 * @remarks
 * A launch runs one setting, which lasts until a relaunch (the guide's `QUALITY` row). Its phases,
 * in order ({@link TERRAIN_CHECK_PLAN}):
 *
 * 1. `terrain-alone`: the photorealistic primary alone, from the scene's start through the
 *    hand-over, the descent and the sunset to past the survey's western edge (a smoke's ends a few
 *    seconds after the primary first draws the terrain). Its looks are captured at moments of the
 *    script ({@link TerrainCheckTiming.captures}) for the by-hand checks, the exposure first put
 *    under `AUTO` on the `LIT` meter so that they show the sunlit ground through the sunset (no
 *    verdict reads it).
 * 2. With instruments: on high instrument 1 photorealistic and instrument 2 in the wireframe
 *    (`terrain-photoreal-wireframe`); on low one wireframe instrument (`terrain-wireframe`), since
 *    `QUALITY LOW` allows one photorealistic view. Through a whole loop of the script (a smoke's for
 *    a few seconds).
 * 3. `terrain-loss`: the primary alone on a fresh session (`VIEW` taken to `PHASE TEST` and back,
 *    the script from its start), played to the field's revision rise; once the delta's first block
 *    has been sent, with blocks still to come and bakes in flight, the coverage mask is read back
 *    and the harness destroys the device ({@link TerrainProbe.destroyDevice}). Then: the restore,
 *    the mask read back again, the stream's end and the terrain drawn again, and the mask read a
 *    last time against the store's coverage.
 *
 * Then `VIEW` is taken to `PHASE TEST`, which has no terrain, and the session's teardown is read:
 * no worker left, no GPU bytes of the terrain's.
 *
 * Each session's phases start once its materials have compiled, which it starts as it is made
 * (R10.T14.d): the run's first and the loss phase's fresh one. The compile's time and the longest
 * time between animation frames over it are recorded for each session made
 * (`TerrainCheckRecord.sessions`), and judged as the phases' gaps are. Before its first choice of
 * `TERRAIN TEST` the run waits for VIEW's opening to end, the materials' preparations its views made
 * as they opened settled (`TerrainCheckRecord.opening`, recorded and not judged). From then to the
 * teardown's choice of `PHASE TEST` is the judged span (decision-r10-t14d-stall §3): the page's
 * longest time between animation frames over it, outside the forced loss's restore, is recorded
 * and judged (`TerrainCheckRecord.span`), the scene choices and each phase's configuration between
 * the phases among it.
 */

import { norm } from "../../../geometry/vec3";
import type {
  QualitySettingName,
  TerrainCheckLossRecord,
  TerrainCheckPhaseName,
  TerrainCheckPhaseRecord,
  TerrainCheckRecord,
  TerrainCheckSpanRecord,
  ViewsCheckApi,
  ViewsCheckStyle,
  ViewsCheckTerrainFixture,
  ViewsCheckWindow,
} from "../../../../../preload/api";
import { DEFAULT_FOV_DEG } from "../../../view/camera/projection";
import { IDENTITY_QUATERNION } from "../../../view/camera/quaternion";
import { PHASE_SCENE_NAME } from "../../../view/scenes/phaseScene";
import {
  TERRAIN_SCENE_NAME,
  TERRAIN_TEST_SCRIPT,
  terrainShipBodyFixedM,
  terrainTestGolden,
} from "../../../view/scenes/terrainScene";
import {
  handoverRangeM,
  handoverTauPx,
  limbDistanceM,
  pixelAngleRad,
} from "../../../view/terrain/handover";
import { PRIMARY_NAME, PRIMARY_VIEW_NAME } from "../viewNames";
import {
  type QuietSpan,
  bakesInFlight,
  longestGapOutside,
  longestGapThrough,
  type MaskReading,
  type TerrainMark,
  type TerrainProbe,
  type TerrainProbeOptions,
} from "./terrainProbe";
import { viewDriver, type ViewsCheckPhasePlan } from "./viewsCheckRun";
import type { ViewsProbe } from "./viewsProbe";

/** One phase of the terrain run: its name, the primary's style and each instrument's. */
export interface TerrainCheckPhasePlan {
  readonly name: TerrainCheckPhaseName;
  readonly primary: ViewsCheckStyle;
  readonly instruments: ViewsCheckPhasePlan["instruments"];
}

/** Each setting's phases, in order (decision-r10-t16-contracts §6; see the module's remarks). */
export const TERRAIN_CHECK_PLAN: Readonly<
  Record<QualitySettingName, ReadonlyArray<TerrainCheckPhasePlan>>
> = {
  high: [
    { name: "terrain-alone", primary: "photorealistic", instruments: [null, null] },
    {
      name: "terrain-photoreal-wireframe",
      primary: "photorealistic",
      instruments: ["photorealistic", "wireframe"],
    },
    { name: "terrain-loss", primary: "photorealistic", instruments: [null, null] },
  ],
  low: [
    { name: "terrain-alone", primary: "photorealistic", instruments: [null, null] },
    { name: "terrain-wireframe", primary: "photorealistic", instruments: ["wireframe", null] },
    { name: "terrain-loss", primary: "photorealistic", instruments: [null, null] },
  ],
};

/** A look the first phase captures, by its label, at a time of the script. */
export interface TerrainCapture {
  readonly label: string;
  /** The script's time, s. */
  readonly scriptS: number;
}

/** How long each part of the terrain run takes. */
export interface TerrainCheckTiming {
  /** From a configuration reached to its window's start, ms. */
  readonly settleMs: number;
  /**
   * The script's time at which the first phase ends, s; `null` to end it
   * {@link TerrainCheckTiming.afterTerrainMs} after the primary first draws the terrain.
   */
  readonly aloneUntilS: number | null;
  /** How long the first phase goes on after the primary first draws the terrain, ms (a smoke's). */
  readonly afterTerrainMs: number;
  /** The script's time by which the primary must have drawn the terrain in the first phase, s. */
  readonly terrainByS: number;
  /** The instruments' phase: through a whole loop of the script (`null`), or for this long, ms. */
  readonly instrumentsMs: number | null;
  /** The script's time by which the delta must be streaming with bakes in flight, s. */
  readonly lossByS: number;
  /** The longest wait for the restore, and for the stream's end and the terrain after it, ms. */
  readonly restoreMs: number;
  /** How long the loss phase goes on once the stream has ended and the terrain is drawn again, ms. */
  readonly afterLossMs: number;
  /** The longest wait for a control to take effect, ms. */
  readonly waitMs: number;
  /** How often a wait looks again, ms: the loss's moment is looked for at this rate. */
  readonly pollMs: number;
  /** The first phase's looks. */
  readonly captures: ReadonlyArray<TerrainCapture>;
}

/** The script's time past the survey's western edge, s: the first phase's end, and a loop's. */
const PAST_THE_EDGE_S = 265;

/**
 * The full run's looks, at the script's times (`TERRAIN_TEST_SCRIPT`): the disc under the live
 * survey before the hand-over (T10.d), about the hand-over (T10.e by eye), the descent, the track
 * over the cube's faces' edge at 45° east (about 212 to 222 s), the sunset over the survey at 240 s
 * (T8.b), and the survey's western edge between 250 and 260 s (T10.c).
 */
const FULL_CAPTURES: ReadonlyArray<TerrainCapture> = [
  { label: "disc-20s", scriptS: 20 },
  { label: "disc-34s", scriptS: 34 },
  { label: "terrain-42s", scriptS: 42 },
  { label: "terrain-60s", scriptS: 60 },
  { label: "descent-100s", scriptS: 100 },
  { label: "descent-150s", scriptS: 150 },
  { label: "lowest-180s", scriptS: 180 },
  { label: "track-205s", scriptS: 205 },
  { label: "face-edge-212s", scriptS: 212 },
  { label: "face-edge-218s", scriptS: 218 },
  { label: "sunset-228s", scriptS: 228 },
  { label: "sunset-234s", scriptS: 234 },
  { label: "sunset-238s", scriptS: 238 },
  { label: "sunset-241s", scriptS: 241 },
  { label: "sunset-244s", scriptS: 244 },
  { label: "edge-248s", scriptS: 248 },
  { label: "edge-252s", scriptS: 252 },
  { label: "edge-256s", scriptS: 256 },
  { label: "edge-260s", scriptS: 260 },
];

/** A full run's timing (about fourteen minutes) and a smoke's (about four). */
export const TERRAIN_CHECK_TIMING: {
  readonly full: TerrainCheckTiming;
  readonly smoke: TerrainCheckTiming;
} = {
  full: {
    settleMs: 2_000,
    aloneUntilS: PAST_THE_EDGE_S,
    afterTerrainMs: 3_000,
    terrainByS: 120,
    instrumentsMs: null,
    lossByS: 170,
    restoreMs: 30_000,
    afterLossMs: 10_000,
    waitMs: 60_000,
    pollMs: 20,
    captures: FULL_CAPTURES,
  },
  smoke: {
    settleMs: 1_000,
    aloneUntilS: null,
    afterTerrainMs: 3_000,
    terrainByS: 90,
    instrumentsMs: 8_000,
    lossByS: 170,
    restoreMs: 30_000,
    afterLossMs: 3_000,
    waitMs: 60_000,
    pollMs: 20,
    captures: [],
  },
};

/**
 * The fixture's wait before each block, ms: the delta's three blocks then arrive over about six
 * seconds, so that the run finds blocks still to come and bakes in flight to destroy the device
 * among (decision-r10-t16-contracts §6).
 */
export const TERRAIN_CHECK_PACE_MS = 1_500;

/** The terrain probe's options for a launch: the pace, and a read-back a second (two in a smoke). */
export function terrainProbeOptions(smoke: boolean): TerrainProbeOptions {
  return { paceMs: TERRAIN_CHECK_PACE_MS, readBackMs: smoke ? 500 : 1_000 };
}

/** What the terrain run drives and reads. */
export interface TerrainCheckDeps {
  readonly api: Pick<ViewsCheckApi, "startPhase" | "endPhase" | "capture">;
  readonly views: Pick<
    ViewsProbe,
    "canvasOf" | "timer" | "restoresAtMs" | "preparations" | "frameTimes"
  >;
  readonly terrain: Pick<
    TerrainProbe,
    | "session"
    | "moon"
    | "scriptS"
    | "workers"
    | "mark"
    | "resetPeaks"
    | "destroyDevice"
    | "firstTerrainSince"
    | "readMask"
    | "coverageCells"
    | "faults"
    | "terrainFaults"
    | "teardown"
    | "phase"
    | "compiled"
    | "sessions"
    | "scriptSAt"
  >;
  readonly setting: QualitySettingName;
  /**
   * The field the terrain probe serves: the launch's (`ViewsCheckLaunch.fixture`; R10.T1.c), whose
   * golden the hand-over's range and the loss phase's first payload are read from.
   */
  readonly fixture: ViewsCheckTerrainFixture;
  readonly document: Document;
  readonly timing: TerrainCheckTiming;
  /** The page's clock, `performance.now()`, on which the probes stamp frames. */
  readonly nowMs: () => number;
  /** Marks a phase's window in the page's user timing, as T20's run does. */
  readonly markWindow: (window: ViewsCheckWindow) => void;
  readonly sleep: (ms: number) => Promise<void>;
  /** Stops the run: before its next step, wait or call it throws `STOPPED` (`viewsCheckRun.ts`). */
  readonly signal: AbortSignal;
  readonly devicePixelRatio: number;
}

/**
 * `TEST MOON`'s hand-over range at a canvas `widthPx` wide at the default field of view on
 * `setting`, m (`handoverRangeM` of the field's relief, against the camera's distance to the limb),
 * and the script's time at which the descent crosses it, s, by bisection over the descent; on
 * `fixture`'s field, `TERRAIN TEST`'s own unless given.
 */
export function terrainHandover(
  widthPx: number,
  setting: QualitySettingName,
  fixture: ViewsCheckTerrainFixture = "terrain-test",
): { readonly rangeM: number; readonly scriptS: number } | null {
  if (!(widthPx > 0)) {
    return null;
  }
  const { header } = terrainTestGolden(fixture);
  const radPerPx = pixelAngleRad(
    { fovXRad: (DEFAULT_FOV_DEG * Math.PI) / 180, orientation: IDENTITY_QUATERNION },
    { widthPx, heightPx: widthPx },
  );
  const rangeM = handoverRangeM(
    header.realised_relief_m,
    handoverTauPx("photorealistic", setting),
    radPerPx,
  );
  const limbAt = (tS: number): number =>
    limbDistanceM(norm(terrainShipBodyFixedM(tS, fixture)), header.equatorial_radius_m);
  let low = 0;
  let high: number = TERRAIN_TEST_SCRIPT.descentEndS;
  if (!(limbAt(low) > rangeM && limbAt(high) < rangeM)) {
    return null;
  }
  for (let i = 0; i < 60; i += 1) {
    const mid = (low + high) / 2;
    if (limbAt(mid) > rangeM) {
      low = mid;
    } else {
      high = mid;
    }
  }
  return { rangeM, scriptS: (low + high) / 2 };
}

/** The cells of `before` all in `after`. */
function kept(before: MaskReading, after: MaskReading): boolean {
  return [...before.cells].every((cell) => after.cells.has(cell));
}

/** Whether two sets hold the same members. */
function sameCells(a: ReadonlySet<number>, b: ReadonlySet<number>): boolean {
  return a.size === b.size && [...a].every((cell) => b.has(cell));
}

/**
 * Runs the terrain run in `deps.document`'s `VIEW` and returns its record.
 *
 * @throws Error naming the step when a control does not take effect within the timing's wait, or
 * when the run is stopped through `deps.signal` (`STOPPED`).
 */
export async function runTerrainCheck(deps: TerrainCheckDeps): Promise<TerrainCheckRecord> {
  const { terrain, timing } = deps;
  const driver = viewDriver(deps);
  const { stopped, wait, until } = driver;
  const plans = TERRAIN_CHECK_PLAN[deps.setting];
  const phases: TerrainCheckPhaseRecord[] = [];
  /** The forced loss's restore, which the judged span leaves out as the loss phase does. */
  let restore: QuietSpan | null = null;
  /**
   * The run's own computations on the page's main thread, each phase's record made at its end:
   * the judged span leaves out the gaps they make (R10.T14.d).
   */
  const harness: QuietSpan[] = [];
  /** Waits until `test` holds or `limit` does, looking every poll; returns whether `test` held. */
  const watch = async (test: () => boolean, limit: () => boolean): Promise<boolean> => {
    stopped();
    if (test()) {
      return true;
    }
    if (limit()) {
      return false;
    }
    await wait(timing.pollMs);
    return watch(test, limit);
  };
  const capture = async (label: string): Promise<void> => {
    stopped();
    await deps.api.capture(label);
  };
  const script = (): number => terrain.scriptS ?? 0;
  /** Starts a phase: its trace call, its tally's peaks and its mark. */
  const begin = async (name: TerrainCheckPhaseName): Promise<TerrainMark> => {
    stopped();
    await deps.api.startPhase(name);
    terrain.resetPeaks();
    return terrain.mark();
  };
  /** Ends a phase begun at `start`: its window marked and handed on, and its record kept. */
  const finish = async (
    name: TerrainCheckPhaseName,
    start: TerrainMark,
    quiet: QuietSpan | null,
  ): Promise<void> => {
    const end = terrain.mark();
    const window = { startMs: start.atMs, endMs: end.atMs };
    deps.markWindow(window);
    stopped();
    await deps.api.endPhase(name, window);
    const fromMs = deps.nowMs();
    phases.push(terrain.phase(name, start, end, driver.shownViews(), quiet));
    harness.push({ fromMs, toMs: deps.nowMs() });
  };
  /**
   * Waits for `VIEW`'s session on `TERRAIN TEST` and for its materials' compile, which it starts as
   * it is made (R10.T14.d): a phase starts after it, so that a cold compile's stall of the page is
   * the session's making's (`TerrainCheckRecord.sessions`), not the phase's.
   */
  const sessionCompiled = async (): Promise<void> => {
    await until("no terrain session was made", () => terrain.session !== null);
    await until("the terrain session's materials did not compile", () => terrain.compiled);
  };
  /**
   * Takes `VIEW` off `TERRAIN TEST` and back: a fresh session, the script from its start, compiled;
   * returns the counters as `TERRAIN TEST` is chosen again, before the session asks for anything.
   */
  const freshSession = async (): Promise<TerrainMark> => {
    await driver.chooseScene(PHASE_SCENE_NAME);
    await until("the terrain session was not let go", () => terrain.session === null);
    // Before the choice, so that no block the new session asks for is counted before it.
    const chosen = terrain.mark();
    await driver.chooseScene(TERRAIN_SCENE_NAME);
    await sessionCompiled();
    return chosen;
  };

  /**
   * Puts the operated view's exposure under `AUTO` (`ENABLE`) on the `LIT` meter, so that the looks
   * are exposed for the sunlit ground, the disc's included, through the sunset; `VIEW` opens under
   * `MAN` on `AVG`, and no verdict reads the exposure.
   */
  const autoExposure = async (): Promise<void> => {
    const offered = (): boolean => {
      const button = driver.controls.exposureButton("ENABLE");
      return button !== null && button.getAttribute("aria-disabled") !== "true";
    };
    driver.controls.unfoldPanel("view-exposure");
    await until("the EXPOSURE panel's ENABLE was not offered", offered);
    stopped();
    driver.controls.exposureButton("ENABLE")?.click();
    await until("AUTO did not resume on ENABLE", () => !offered());
    driver.controls.unfoldPanel("view-meter");
    const lit = (): HTMLButtonElement | null => driver.controls.meterButton("LIT");
    await until("the EXPOSURE METER's LIT was not found", () => lit() !== null);
    stopped();
    lit()?.click();
    await until(
      "the LIT meter was not chosen",
      () => lit()?.getAttribute("aria-pressed") === "true",
    );
  };

  /** The first phase: the primary alone from the script's start. */
  const alone = async (plan: TerrainCheckPhasePlan): Promise<void> => {
    await driver.configure(plan);
    await autoExposure();
    await wait(timing.settleMs);
    const start = await begin(plan.name);
    const looks = [...timing.captures];
    // The primary's first terrain draw of the phase, once seen, held in an object since the closures
    // below set it.
    const first: { terrainMs: number | null } = { terrainMs: null };
    const ended = (): boolean => {
      first.terrainMs ??= terrain.firstTerrainSince(PRIMARY_VIEW_NAME, start.atMs);
      if (timing.aloneUntilS !== null) {
        return script() >= timing.aloneUntilS;
      }
      return first.terrainMs !== null && deps.nowMs() >= first.terrainMs + timing.afterTerrainMs;
    };
    const look = async (): Promise<void> => {
      stopped();
      if (ended() || (first.terrainMs === null && script() > timing.terrainByS)) {
        return;
      }
      const due = looks.findIndex((each) => script() >= each.scriptS);
      if (due >= 0) {
        const [taken] = looks.splice(due, 1);
        if (taken !== undefined) {
          await capture(taken.label);
        }
      } else {
        await wait(timing.pollMs);
      }
      await look();
    };
    await look();
    if (first.terrainMs !== null) {
      await capture("first-terrain");
    }
    await finish(plan.name, start, null);
  };

  /** The second phase: the instruments opened, through a loop of the script or for a while. */
  const withInstruments = async (plan: TerrainCheckPhasePlan): Promise<void> => {
    await driver.configure(plan);
    await wait(timing.settleMs);
    const start = await begin(plan.name);
    if (timing.instrumentsMs === null) {
      // Through the script's end, its loop, and on to the first phase's end again; a script that
      // stops playing ends the phase at twice its length, and the record shows where it stood.
      let wrapped = false;
      let last = script();
      const limitMs = start.atMs + 2_000 * TERRAIN_TEST_SCRIPT.durationS;
      await watch(
        () => {
          const now = script();
          wrapped ||= now < last;
          last = now;
          return wrapped && now >= PAST_THE_EDGE_S;
        },
        () => deps.nowMs() > limitMs,
      );
    } else {
      await wait(timing.instrumentsMs);
    }
    await finish(plan.name, start, null);
  };

  /** The last phase: a fresh session, its device destroyed mid-stream, and its restore. */
  const lossPhase = async (plan: TerrainCheckPhasePlan): Promise<TerrainCheckLossRecord> => {
    await driver.configure(plan);
    const chosen = await freshSession();
    await driver.configure(plan);
    const start = await begin(plan.name);
    const firstBlocks = terrainTestGolden(deps.fixture).payloads[0]?.blocksHex.length ?? 0;
    // The fresh session's blocks, from the choice, since some may come during its compile.
    const midStream = (): boolean => {
      const mark = terrain.mark();
      return (
        script() >= TERRAIN_TEST_SCRIPT.deltaAtS &&
        mark.fixture.blocksSent - chosen.fixture.blocksSent > firstBlocks &&
        mark.fixture.blocksToCome > 0 &&
        bakesInFlight(mark.workers) > 0
      );
    };
    const streaming = await watch(midStream, () => script() > timing.lossByS);
    const before = streaming ? await terrain.readMask() : null;
    const atDestroy = terrain.mark();
    const restoresBefore = deps.views.restoresAtMs().length;
    const destroyed = streaming && terrain.destroyDevice();
    const destroyedAtMs = destroyed ? deps.nowMs() : null;
    const notDestroyed = destroyed
      ? null
      : streaming
        ? "no device was held to destroy"
        : `the delta did not stream with bakes in flight by ${String(timing.lossByS)} s of the script`;
    let afterRestore: MaskReading | null = null;
    let terrainAgainAtMs: number | null = null;
    let quiet: QuietSpan | null = null;
    if (destroyedAtMs !== null) {
      const deadline = deps.nowMs() + timing.restoreMs;
      const timedOut = (): boolean => deps.nowMs() > deadline;
      await watch(() => deps.views.restoresAtMs().length > restoresBefore, timedOut);
      const restoredAtMs = deps.views.restoresAtMs()[restoresBefore] ?? null;
      if (restoredAtMs !== null) {
        afterRestore = await readAfterRestore(before);
        await watch(
          () =>
            terrain.mark().fixture.blocksToCome === 0 &&
            terrain.firstTerrainSince(PRIMARY_VIEW_NAME, restoredAtMs) !== null,
          timedOut,
        );
        terrainAgainAtMs = terrain.firstTerrainSince(PRIMARY_VIEW_NAME, restoredAtMs);
        quiet = { fromMs: destroyedAtMs, toMs: terrainAgainAtMs ?? deps.nowMs() };
        await capture("restored");
      }
      await wait(timing.afterLossMs);
    }
    const end = destroyedAtMs === null ? null : await terrain.readMask();
    const expected = terrain.coverageCells();
    const after = terrain.mark();
    restore =
      quiet ?? (destroyedAtMs === null ? null : { fromMs: destroyedAtMs, toMs: after.atMs });
    await finish(plan.name, start, restore);
    return {
      destroyedAtMs,
      destroyedAtScriptS: destroyed ? (atDestroy.scriptS ?? null) : null,
      notDestroyed,
      blocksToCome: atDestroy.fixture.blocksToCome,
      bakesInFlight: bakesInFlight(atDestroy.workers),
      after: {
        fieldAsks: after.fixture.fieldAsks - atDestroy.fixture.fieldAsks,
        fieldInits: after.workers.inits - atDestroy.workers.inits,
        fieldChunks: after.workers.chunks - atDestroy.workers.chunks,
        blocksSent: after.fixture.blocksSent - atDestroy.fixture.blocksSent,
      },
      workers: terrain.session?.workerCount ?? 0,
      terrainAgainAtMs,
      masks: {
        before: before?.cells.size ?? null,
        afterRestore: afterRestore?.cells.size ?? null,
        end: end?.cells.size ?? null,
        expected: expected.size,
        beforeKept: before !== null && afterRestore !== null && kept(before, afterRestore),
        endAsExpected: end !== null && sameCells(end.cells, expected),
      },
    };
  };

  /**
   * The judged span from `fromMs` to `toMs` (R10.T14.d; decision-r10-t14d-stall §3): the page's
   * longest time between animation frames over it, outside the restore and the run's own
   * computations, and where it ended.
   */
  const spanRecord = (fromMs: number, toMs: number): TerrainCheckSpanRecord => {
    const { gapMs, endMs } = longestGapOutside(
      deps.views.frameTimes(fromMs, toMs),
      fromMs,
      toMs,
      restore === null ? harness : [restore, ...harness],
    );
    return {
      fromMs,
      toMs,
      longestFrameGapMs: gapMs,
      longestGapEndMs: endMs,
      longestGapEndScriptS: terrain.scriptSAt(endMs),
      harnessMs: harness.reduce((sum, span) => sum + span.toMs - span.fromMs, 0),
    };
  };

  /** The mask as soon as it reads back on the restored device: a texture other than `before`'s. */
  const readAfterRestore = async (before: MaskReading | null): Promise<MaskReading | null> => {
    const deadline = deps.nowMs() + timing.restoreMs;
    const look = async (): Promise<MaskReading | null> => {
      stopped();
      const read = await terrain.readMask();
      if (read !== null && read.texture !== before?.texture) {
        return read;
      }
      if (deps.nowMs() > deadline) {
        return null;
      }
      await wait(timing.pollMs);
      return look();
    };
    return look();
  };

  /** Runs `plans[index]` and each after it, each acting on the page the one before left. */
  const runFrom = async (index: number): Promise<TerrainCheckLossRecord | null> => {
    stopped();
    const plan = plans[index];
    if (plan === undefined) {
      return null;
    }
    let loss: TerrainCheckLossRecord | null = null;
    switch (plan.name) {
      case "terrain-alone":
        await alone(plan);
        break;
      case "terrain-photoreal-wireframe":
      case "terrain-wireframe":
        await withInstruments(plan);
        break;
      case "terrain-loss":
        loss = await lossPhase(plan);
        break;
    }
    return (await runFrom(index + 1)) ?? loss;
  };

  // VIEW's opening: until the preparations its views made as they opened have settled (R10.T14.d),
  // so that no first draw of a prepared material races its preparation in what is judged.
  const openedFromMs = deps.nowMs();
  // VIEW drawn at all first, as a scene's choice waits for it, so that a launch that cannot draw
  // VIEW fails for that reason.
  await until(
    "VIEW did not show its primary view",
    () => driver.controls.styleOf(PRIMARY_NAME) !== null,
  );
  await until("VIEW's views made no preparation", () => deps.views.preparations().seen > 0);
  await until("VIEW's preparations did not settle", () => deps.views.preparations().inFlight === 0);
  const openedAtMs = deps.nowMs();
  const preparations = deps.views.preparations().seen;
  await driver.chooseScene(TERRAIN_SCENE_NAME);
  await sessionCompiled();
  const loss = await runFrom(0);
  if (loss === null) {
    throw new Error(`views check: the ${deps.setting} plan has no loss phase`);
  }
  // The judged span ends at the teardown's choice: VIEW on a kept scene with no terrain, the
  // session let go.
  const judgedToMs = deps.nowMs();
  await driver.chooseScene(PHASE_SCENE_NAME);
  await until("the terrain session was not let go", () => terrain.session === null);
  await wait(timing.settleMs);
  return {
    timer: deps.views.timer(),
    devicePixelRatio: deps.devicePixelRatio,
    phases,
    faults: terrain.faults(),
    terrainFaults: terrain.terrainFaults(),
    sessions: terrain.sessions(),
    span: spanRecord(openedAtMs, judgedToMs),
    opening: {
      preparedMs: openedAtMs - openedFromMs,
      preparations,
      longestFrameGapMs: longestGapThrough(
        deps.views.frameTimes(openedFromMs, Infinity),
        openedFromMs,
        openedAtMs,
      ),
    },
    restoresAtMs: deps.views.restoresAtMs(),
    loss,
    teardown: terrain.teardown(),
    handover: terrainHandover(
      deps.views.canvasOf(PRIMARY_VIEW_NAME).widthPx,
      deps.setting,
      deps.fixture,
    ),
  };
}

/** What {@link recordTerrainCheck} asks of the main process. */
export type TerrainCheckEnding = Pick<ViewsCheckApi, "writeTerrainResults" | "end">;

/**
 * Runs `run` and ends the check with its outcome: the record written (the main process judges
 * it) and the run ended, or the run failed with the reason it threw; nothing at all once `signal`
 * has stopped it.
 */
export async function recordTerrainCheck(
  api: TerrainCheckEnding,
  run: () => Promise<TerrainCheckRecord>,
  signal: AbortSignal,
): Promise<void> {
  try {
    const record = await run();
    if (signal.aborted) {
      return;
    }
    await api.writeTerrainResults(record);
    await api.end({ status: "pass" });
  } catch (error: unknown) {
    if (signal.aborted) {
      return;
    }
    const reason = error instanceof Error ? error.message : String(error);
    console.error("views check: the terrain run failed:", error);
    await api.end({ status: "fail", reason });
  }
}
