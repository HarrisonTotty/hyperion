/**
 * The renderer's half of a perf run (plan R12, R12.T4.c; Design notes 4, 7, 13 and 14): the scene
 * from `config()`, its warm-up pose, the resets before each repetition, the repetitions with R05's
 * metrics, the pass times by `PASS_ROWS` keyed by view role and label, and the itemised tally, each
 * repetition's sent by `endRepetition`.
 *
 * @remarks
 * In order, over `window.hyperion.perf` ({@link PerfSceneRunDeps.api}): `config()`, the main
 * process having started the scene's server and placed the ship; the scene's views mounted by its
 * driver on that server and arrival; `describe()` once they are made; the warm-up, the script's
 * first pose held (script time 0) with the views drawing, its frame intervals handed to
 * `warmUpSample()` about once a second until the verdict is `steady` or `not-steady`; then for
 * each repetition the views dropped (Design note 13's resets: their terrain session, and with it
 * the patch cache, its height textures and the height workers' queues, go with them),
 * `beginRepetition(n)`, which restarts the server and arrives again, the views mounted anew on the
 * new server and arrival and the script flown from time 0 over its whole span, and
 * `endRepetition(n, result)`; and `finish()`. A repetition whose views find the patch cache not
 * empty at its first frame fails the run, since its reset did not happen.
 *
 * Any failure (a `GraphicsFault`, no adapter, the views not made, the served world not read, an
 * unmapped pass, a call the main process refuses) ends the run once with `finish` `failed` and its
 * reason in full, the screen told how it ended first (`onEnded`: the views' own fault line, or the
 * run's in the guide's voice). Only frames of a repetition's span are in its result: the warm-up's
 * never reach its percentiles.
 */

import {
  type AdapterRecord,
  type InternalScaleRecord,
  type JsonObject,
  type JsonValue,
  MEMORY_ITEMS_ORDER,
  type MemoryItem,
} from "@hyperion/perf";

import type {
  PerfApi,
  PerfArrival,
  PerfConfig,
  PerfDescribedView,
  PerfEnd,
  PerfRunDescription,
  SpikeSegmentSpan,
} from "../../../../preload/api";
import type { AllocationEvent } from "../engine/memory";
import type { PassTimes } from "../engine/types";
import { type QualitySetting, SETTINGS } from "../quality/qualitySetting";
import type { PatchEvent } from "../spike/metrics";
import type { WorkerMemory } from "../terrain/workers/pool";
import type { LedgerFigures } from "./ledger";
import { type PassRoles, passRolesOf, UnmappedPass } from "./passRows";
import { type PerfFrameSample, RELEASE_MIN_BYTES, RepetitionMetrics } from "./repetitionMetrics";
import { PERF_SCENES, type PerfScene } from "./scenes";

export type { PerfFrameSample } from "./repetitionMetrics";

/** How often the warm-up's frame intervals go to the main process, ms (Design note 14). */
export const WARM_UP_SAMPLE_MS = 1000;

/**
 * How long after a repetition's last frame its pass times are waited for, ms: R01's reads lag the
 * frames by a few, and a frame whose reads have not ended is left out of its rows.
 */
export const PASS_TIMES_SETTLE_MS = 250;

/** The page's clock, injected so that the controller runs on a test's. */
export interface PerfClock {
  /** `performance.now()`, ms. */
  nowMs(): number;
  /** `performance.timeOrigin`, ms since the epoch. */
  timeOriginMs(): number;
  /**
   * Resolves after `ms`.
   *
   * @throws Error, as a rejection, once `signal` aborts.
   */
  wait(ms: number, signal: AbortSignal): Promise<void>;
}

/** The browser's clock: `performance` and `setTimeout`. */
export const PAGE_CLOCK: PerfClock = {
  nowMs: () => performance.now(),
  timeOriginMs: () => performance.timeOrigin,
  wait: (ms, signal) =>
    new Promise<void>((resolve, reject) => {
      if (signal.aborted) {
        reject(new Error("the perf run was stopped", { cause: signal.reason }));
        return;
      }
      const onAbort = (): void => {
        clearTimeout(timer);
        reject(new Error("the perf run was stopped", { cause: signal.reason }));
      };
      const timer = setTimeout(() => {
        signal.removeEventListener("abort", onAbort);
        resolve();
      }, ms);
      signal.addEventListener("abort", onAbort, { once: true });
    }),
};

/**
 * A failure of the scene: its reason in the record's words, and what the screen shows of it once
 * the run has ended.
 */
export interface PerfFailure {
  /** The record's words in full: `finish`'s reason, and the record's notes. */
  readonly reason: string;
  /**
   * The cause as the run's fault line states it, in the guide's voice, with no scene ID, plan
   * reference or `GraphicsFault:` prefix ({@link perfFailedLine}); or `null` where the views stand
   * on their own fault line, which stays shown.
   */
  readonly shown: string | null;
}

/** How the run ended, as the screen shows it once it has (the guide's "Data states"). */
export type PerfRunOutcome =
  | { readonly kind: "complete" }
  | {
      readonly kind: "failed";
      /** The cause on the run's fault line, or `null` where the views' own line stands. */
      readonly shown: string | null;
    };

/** The status line of a run that ended complete. */
export const PERF_COMPLETE = "PERFORMANCE RUN COMPLETE";

/**
 * The cause on the run's fault line where the views stood on their own fault line and are gone, or
 * where an error the run did not word ended it: the main process's log gives its reason in full.
 */
export const PERF_FAILED_LOGGED = "the run's log states why";

/** The run's fault line: `PERFORMANCE RUN FAILED: <cause>`. */
export function perfFailedLine(cause: string): string {
  return `PERFORMANCE RUN FAILED: ${cause}`;
}

/** What a scene's views and engine tell the run as they draw. */
export interface PerfSceneSink {
  /** A frame drawn, with the run's resolve count by its end (`ResolveCounter.value`). */
  frame(sample: PerfFrameSample): void;
  /** A patch's progress (R05's `SpikeListeners.onPatch`). */
  patch(event: PatchEvent): void;
  /** One resolve's pass times, numbered on the run's count (`ResolveCounter.runFrame`). */
  passTimes(times: PassTimes): void;
  /** One of the engine's allocation events. */
  allocation(event: AllocationEvent): void;
  /** The scene cannot go on, with why, in the record's words and the screen's. */
  failed(failure: PerfFailure): void;
}

/** Where a mount of the scene's views draws: the warm-up's server and arrival, or a span's. */
export interface PerfStage {
  /** The warm-up holds the script's first pose; a repetition flies it from time 0. */
  readonly phase: "warm-up" | "repetition";
  /** 0 for the warm-up, else the repetition's number, from 1. */
  readonly n: number;
  readonly serverUrl: string;
  readonly arrival: PerfArrival;
}

/** A scene's views once made, and what its script is. */
export interface PerfDrawn {
  /** The views drawn, as the record's `views` (less their internal scale), in the scene's order. */
  readonly views: ReadonlyArray<PerfDescribedView>;
  readonly adapter: AdapterRecord | null;
  readonly capabilities: Readonly<Record<string, boolean>> | null;
  readonly kernelPaths: Readonly<Record<string, string>> | null;
  /** The height workers the views' terrain runs. */
  readonly heightWorkers: number;
  /** The script's span, s: a repetition ends at the first frame at or past it. */
  readonly spanS: number;
  /** The script's segments, s from its start. */
  readonly segments: ReadonlyArray<SpikeSegmentSpan>;
  /** R05's predicted demand of a segment, patches a second, or `null` where it is not had. */
  readonly demandPerS: (segment: string) => number | null;
  /** What the record's notes say of how the renderer drew the scene. */
  readonly notes: ReadonlyArray<string>;
}

/** What draws a scene's views for a run (`PerfApp`'s descent driver, or a test's fake). */
export interface PerfSceneDriver {
  /**
   * Mounts the scene's views on `stage`'s server and arrival, telling `sink` of every frame from
   * the first; resolves once the views have drawn a frame, with what they are.
   *
   * @throws Error, as a rejection, where they cannot be made; `sink.failed` is told too.
   */
  mount(stage: PerfStage, sink: PerfSceneSink): Promise<PerfDrawn>;
  /** Drops the views and their terrain session, with its patch cache (Design note 13's resets). */
  unmount(): void;
  /** The patches the views' cache holds now. */
  cachedPatches(): number;
  /** Each running height worker's memory report. */
  workerMemory(): ReadonlyArray<WorkerMemory>;
  /** The itemised tally of the run's engine, or `null` before the engine is made. */
  memory(): { figures(): LedgerFigures; resetPeaks(): void } | null;
  /** Pipelines created after the warm-up so far (R05's shim). */
  latePipelines(): number;
  /** Ends the warm-up: pipeline creations from now on are late. */
  endWarmUp(): void;
  /** The run's resolve count now (`ResolveCounter.value`). */
  resolveCount(): number;
  /** Each view's internal scale over the span just flown, in the views' order. */
  internalScale(views: ReadonlyArray<PerfDescribedView>): ReadonlyArray<InternalScaleRecord>;
}

/**
 * The run's driver for its scene, or why the renderer has none, in the record's words and the
 * screen's.
 */
export type PerfDriverChoice =
  | { readonly kind: "driver"; readonly driver: PerfSceneDriver }
  | { readonly kind: "refused"; readonly reason: string; readonly shown: string };

/** What a {@link PerfSceneRun} is given. */
export interface PerfSceneRunDeps {
  /** `window.hyperion.perf`. */
  readonly api: PerfApi;
  readonly clock: PerfClock;
  /** The driver for the configured scene. */
  readonly driverFor: (config: PerfConfig) => PerfDriverChoice;
  /** Stops the run without finishing it: the page is going. */
  readonly signal: AbortSignal;
  /**
   * Told once as the run ends, before the main process is told (`finish` ends the launch, and the
   * page with it), so that the screen shows how it ended; never told of a run its signal stopped.
   */
  readonly onEnded: (outcome: PerfRunOutcome) => void;
}

/** A failure of the scene, its reason the record's. */
class SceneFailed extends Error {
  override readonly name = "SceneFailed";
}

/** The run stopped by its signal. */
class RunStopped extends Error {
  override readonly name = "RunStopped";
}

/** The flattened JSON form of a setting's value, or `undefined` for one JSON cannot hold. */
function jsonOf(value: unknown): JsonValue | undefined {
  if (value === null || typeof value === "string" || typeof value === "boolean") {
    return value;
  }
  if (typeof value === "number") {
    return Number.isFinite(value) ? value : String(value);
  }
  if (typeof value === "bigint") {
    return value.toString();
  }
  if (Array.isArray(value)) {
    return value.map((item: unknown) => jsonOf(item) ?? null);
  }
  if (typeof value === "object") {
    const out: Record<string, JsonValue> = {};
    for (const [key, item] of Object.entries(value)) {
      const json = jsonOf(item);
      if (json !== undefined) {
        out[key] = json;
      }
    }
    return out;
  }
  return undefined;
}

/** Each leaf of `value` under `prefix`'s keys, `<prefix>.<field path>`, into `out`. */
function flattenInto(value: unknown, prefix: string, out: Record<string, JsonValue>): void {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    for (const [key, item] of Object.entries(value)) {
      flattenInto(item, `${prefix}.${key}`, out);
    }
    return;
  }
  const json = jsonOf(value);
  if (json !== undefined) {
    out[prefix] = json;
  }
}

/**
 * The setting's ladder values for the record (Design note 3): R05's `SETTINGS[setting]` flattened
 * to `quality.<field path>` keys, a non-finite number as its text, an array as one value.
 */
export function settingValuesOf(setting: QualitySetting): JsonObject {
  const out: Record<string, JsonValue> = {};
  flattenInto(SETTINGS[setting], "quality", out);
  return out;
}

/** What the record says of the run, from its scene and the views as drawn. */
export function runDescriptionOf(
  scene: PerfScene,
  setting: QualitySetting,
  drawn: PerfDrawn,
): PerfRunDescription {
  return {
    scene: { version: scene.version, seed: scene.universe.seed, world: scene.world },
    settingValues: settingValuesOf(setting),
    views: drawn.views,
    pacing: scene.pacing[setting],
    repetitionSeconds: scene.span.kind === "script" ? scene.span.seconds : null,
    heightWorkers: drawn.heightWorkers,
    adapter: drawn.adapter,
    capabilities: drawn.capabilities,
    kernelPaths: drawn.kernelPaths,
  };
}

/** Where the run stands, as its frames and events are routed. */
type Phase =
  | { readonly kind: "idle" }
  | { readonly kind: "warm-up"; readonly intervalsMs: number[] }
  | {
      readonly kind: "repetition";
      readonly n: number;
      readonly metrics: RepetitionMetrics;
      readonly spanS: number;
      /** Whether its first frame has been seen, when the patch cache is read. */
      started: boolean;
      /** Whether its span's last frame has been seen: later frames hold the last pose. */
      flown: boolean;
      readonly end: () => void;
    };

/** One run of the configured scene, and the sink its driver tells. */
export class PerfSceneRun implements PerfSceneSink {
  readonly #deps: PerfSceneRunDeps;
  #phase: Phase = { kind: "idle" };
  #driver: PerfSceneDriver | null = null;
  #lastRafMs: number | null = null;
  #failure: PerfFailure | null = null;
  readonly #failed: Promise<never>;
  #rejectFailed: (error: Error) => void = () => undefined;
  #finished = false;

  constructor(deps: PerfSceneRunDeps) {
    this.#deps = deps;
    this.#failed = new Promise<never>((_resolve, reject) => {
      this.#rejectFailed = reject;
    });
    // Raced against every step; a failure with no step waiting is not an unhandled one.
    this.#failed.catch(() => undefined);
    deps.signal.addEventListener(
      "abort",
      () => {
        this.#rejectFailed(new RunStopped("the perf run was stopped"));
      },
      { once: true },
    );
  }

  /**
   * Runs the scene to its end: `finish` with `complete` after the last repetition, or `failed` at
   * the first failure, `onEnded` told first. Stopped by the signal, it ends without finishing.
   *
   * @remarks
   * A failed run leaves its views mounted where they stand on their own fault line, so that the
   * line stays; otherwise they go, and the run's fault line stands in their place.
   */
  async run(): Promise<void> {
    const { api, signal } = this.#deps;
    try {
      const config = await this.#step(api.config());
      const choice = this.#deps.driverFor(config);
      if (choice.kind === "refused") {
        await this.#finish(
          { status: "failed", reason: choice.reason },
          { kind: "failed", shown: choice.shown },
        );
        return;
      }
      const driver = choice.driver;
      this.#driver = driver;
      const scene = PERF_SCENES[config.launch.scene];
      const setting = config.launch.setting;
      this.#phase = { kind: "warm-up", intervalsMs: [] };
      const warm = await this.#step(
        driver.mount(
          { phase: "warm-up", n: 0, serverUrl: config.serverUrl, arrival: config.arrival },
          this,
        ),
      );
      await this.#step(api.describe(runDescriptionOf(scene, setting, warm)));
      await this.#warmUp();
      driver.endWarmUp();
      const roles = passRolesOf(warm.views);
      for (let n = 1; n <= config.launch.repetitions; n += 1) {
        // Each repetition follows the last, from the same state (Design note 13): never at once.
        // oxlint-disable-next-line no-await-in-loop
        await this.#repetition(n, driver, warm, roles);
      }
      driver.unmount();
      await this.#finish({ status: "complete", notes: warm.notes.join(" ") }, { kind: "complete" });
    } catch (error: unknown) {
      this.#phase = { kind: "idle" };
      // An error the run did not word, a call the main process refused among them, is in its log.
      const failure = this.#failure ?? { reason: reasonOf(error), shown: PERF_FAILED_LOGGED };
      if (signal.aborted || failure.shown !== null) {
        this.#driver?.unmount();
      }
      if (signal.aborted) {
        return;
      }
      await this.#finish(
        { status: "failed", reason: failure.reason },
        { kind: "failed", shown: failure.shown },
      );
    }
  }

  /**
   * The warm-up after its views are described: a sample of its frame intervals about once a
   * second, until the main process's verdict is `steady` or `not-steady` (Design note 7).
   */
  async #warmUp(): Promise<void> {
    const { api, clock, signal } = this.#deps;
    await this.#step(clock.wait(WARM_UP_SAMPLE_MS, signal));
    const verdict = await this.#step(api.warmUpSample({ intervalsMs: this.#takeIntervals() }));
    if (verdict.kind === "warming") {
      await this.#warmUp();
    }
  }

  /**
   * Repetition `n`: the views dropped (Design note 13's resets: the views, their session and its
   * patch cache), the server restarted and arrived at by `beginRepetition`, the views mounted anew
   * on it and the script flown over its span, then its result handed over.
   */
  async #repetition(
    n: number,
    driver: PerfSceneDriver,
    warm: PerfDrawn,
    roles: PassRoles,
  ): Promise<void> {
    const { api, clock, signal } = this.#deps;
    this.#phase = { kind: "idle" };
    driver.unmount();
    const start = await this.#step(api.beginRepetition(n));
    driver.memory()?.resetPeaks();
    const lateBefore = driver.latePipelines();
    const metrics = new RepetitionMetrics({
      roles,
      segments: warm.segments,
      demandPerS: warm.demandPerS,
      firstEngineFrame: driver.resolveCount(),
      previousRafMs: this.#lastRafMs,
    });
    const flown = new Promise<void>((resolve) => {
      this.#phase = {
        kind: "repetition",
        n,
        metrics,
        spanS: warm.spanS,
        started: false,
        flown: false,
        end: resolve,
      };
    });
    await this.#step(
      driver.mount(
        { phase: "repetition", n, serverUrl: start.serverUrl, arrival: start.arrival },
        this,
      ),
    );
    await this.#step(flown);
    await this.#step(clock.wait(PASS_TIMES_SETTLE_MS, signal));
    const ledger = driver.memory()?.figures() ?? null;
    const workers = driver.workerMemory();
    const result = metrics.result({
      spanS: warm.spanS,
      timeOriginMs: clock.timeOriginMs(),
      memory: {
        itemPeakBytes: ledger === null ? {} : heldItems(ledger.peakBytes),
        tallyPeakBytes: ledger?.tallyPeakBytes ?? null,
        workerFieldBytes: workers.map(({ fieldBytes }) => fieldBytes),
        workerWasmBytes: workers.map(({ wasmBytes }) => wasmBytes),
      },
      latePipelines: Math.max(0, driver.latePipelines() - lateBefore),
      internalScale: driver.internalScale(warm.views),
    });
    this.#phase = { kind: "idle" };
    await this.#step(api.endRepetition(n, result));
  }

  /**
   * `promise`, unless the scene fails or the run is stopped first: a failure already told wins over
   * a step already settled, which is why it is raced first and checked again after it.
   */
  async #step<T>(promise: Promise<T>): Promise<T> {
    const value = await Promise.race([this.#failed, promise]);
    if (this.#deps.signal.aborted) {
      throw new RunStopped("the perf run was stopped");
    }
    if (this.#failure !== null) {
      throw new SceneFailed(this.#failure.reason);
    }
    return value;
  }

  /** Ends the run once, its end shown and then told to the main process. */
  async #finish(end: PerfEnd, outcome: PerfRunOutcome): Promise<void> {
    // A run stopped by its page tells nothing: another run of the page, or none, ends it.
    if (this.#finished || this.#deps.signal.aborted) {
      return;
    }
    this.#finished = true;
    this.#deps.onEnded(outcome);
    try {
      await this.#deps.api.finish(end);
    } catch (error: unknown) {
      console.error("perf run: the run's end could not be told to the main process:", error);
    }
  }

  /** The warm-up's frame intervals since the last sample, ms, oldest first. */
  #takeIntervals(): ReadonlyArray<number> {
    const phase = this.#phase;
    if (phase.kind !== "warm-up") {
      return [];
    }
    return phase.intervalsMs.splice(0);
  }

  /** A frame drawn: a warm-up's interval, or a repetition's frame until its span is flown. */
  frame(sample: PerfFrameSample): void {
    const phase = this.#phase;
    const lastRafMs = this.#lastRafMs;
    this.#lastRafMs = sample.rafTimestampMs;
    switch (phase.kind) {
      case "idle":
        return;
      case "warm-up":
        if (lastRafMs !== null) {
          phase.intervalsMs.push(Math.max(0, sample.rafTimestampMs - lastRafMs));
        }
        return;
      case "repetition":
        break;
    }
    if (phase.flown) {
      return;
    }
    if (!phase.started) {
      phase.started = true;
      const cached = this.#driver?.cachedPatches() ?? 0;
      if (cached > 0) {
        this.failed({
          reason: `the patch cache held ${String(cached)} patches at repetition ${String(phase.n)}'s start: Design note 13's reset did not happen`,
          shown: "the patch cache was not empty at a repetition's start",
        });
        return;
      }
    }
    phase.metrics.frame(sample);
    if (sample.scriptTimeS >= phase.spanS) {
      phase.flown = true;
      phase.end();
    }
  }

  /** A patch's progress, counted within a repetition's span. */
  patch(event: PatchEvent): void {
    const phase = this.#phase;
    if (phase.kind === "repetition" && !phase.flown) {
      phase.metrics.patch(event);
    }
  }

  /** One resolve's pass times, within a repetition; an unmapped pass fails the run. */
  passTimes(times: PassTimes): void {
    const phase = this.#phase;
    if (phase.kind !== "repetition") {
      return;
    }
    try {
      phase.metrics.passTimes(times);
    } catch (error: unknown) {
      this.failed({
        reason: reasonOf(error),
        shown:
          error instanceof UnmappedPass
            ? "a timed pass has no row in the record"
            : PERF_FAILED_LOGGED,
      });
    }
  }

  /** An allocation event: a release of {@link RELEASE_MIN_BYTES} or more within a repetition. */
  allocation(event: AllocationEvent): void {
    const phase = this.#phase;
    if (
      phase.kind === "repetition" &&
      event.kind === "destroyed" &&
      event.bytes >= RELEASE_MIN_BYTES
    ) {
      const { clock } = this.#deps;
      phase.metrics.release({
        atEpochMs: clock.timeOriginMs() + clock.nowMs(),
        bytes: event.bytes,
      });
    }
  }

  /** The scene cannot go on: the run fails with the first failure told. */
  failed(failure: PerfFailure): void {
    if (this.#failure !== null) {
      return;
    }
    this.#failure = failure;
    this.#rejectFailed(new SceneFailed(failure.reason));
  }
}

/** The memory items that held any bytes, as the record's `itemPeakBytes` lists them. */
function heldItems(
  bytes: Readonly<Record<MemoryItem, number>>,
): Readonly<Partial<Record<MemoryItem, number>>> {
  const out: Partial<Record<MemoryItem, number>> = {};
  for (const item of MEMORY_ITEMS_ORDER) {
    if (bytes[item] > 0) {
      out[item] = bytes[item];
    }
  }
  return out;
}

/** A failure's reason in words. */
function reasonOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
