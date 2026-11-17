/**
 * The main process's half of a perf run (plan R12, R12.T4.b; Design notes 7, 13 and 14).
 *
 * @remarks
 * In order: {@link PerfRunner.prepare} checks the preconditions (`checkQuiet`), refusing unless
 * `--force`, which the record then carries; starts the scene's local server in a fresh data
 * directory (or names the remote one `--address` gave), and the 10 Hz thermal, throttle and clock
 * samplers and the 1 Hz memory samplers, every sample stamped on the runner's clock. The renderer
 * then asks: `config()`, `describe()` once its views are made, `warmUpSample()` until the warm-up's
 * verdict (Design note 7: steady, or `NOT STEADY` at ten minutes), then for each repetition
 * `beginRepetition(n)`, which restarts a local server with a fresh data directory before the span
 * opens (Design note 13) and starts its trace, and `endRepetition(n, result)`, which closes the span
 * and joins the samples within it alone; `finish()` ends the run. The record is assembled from the
 * repetitions done, with the trend screen's flag in its notes, validated by `@hyperion/perf`, and
 * appended to `--perf-out` under the first free number of its date, machine and scene.
 *
 * Main sends nothing the renderer did not ask for, so a renderer that stops asking is caught by a
 * watchdog: no call within the phase's limit ends the run as failed, as a `GraphicsFault` the
 * renderer reports, a refused adapter, or the GPU or renderer process gone do, with a record whose
 * notes say so. A record holds at least one repetition, so a run that fails before its first ends
 * with none, saying why.
 */

import {
  appendRecords,
  validatePerfRecord,
  type MachineRecord,
  type PerfSceneHarness,
  type PerfRunRecord,
  type ServerRecord,
  type ThrottleFigures,
} from "@hyperion/perf";

import type {
  GraphicsLaunchMode,
  PerfArrival,
  PerfConfig,
  PerfEnd,
  PerfLaunch,
  PerfRepetitionResult,
  PerfRepetitionStart,
  PerfRunDescription,
  PerfWarmUpSample,
  PerfWarmUpVerdict,
} from "../../preload/api";
import { assemblePerfRecord, type RepetitionInputs, trendNote } from "./assemble";
import type { FrameTraceResult } from "./frameTrace";
import type { MemorySpan } from "./memory";
import type { PreconditionReport } from "./preconditions";
import type { PerfSample, TallyRelease } from "./samplers";
import type { ServerExit, Wait } from "./server";
import { judgeWarmUp, type Timed, trendOf, type WarmUpVerdict } from "./steadiness";

/** The run's server, as the runner starts and stops it: a {@link LocalServer}. */
export interface RunServer {
  start(dataDir: string): Promise<string>;
  stop(): Promise<ServerExit | null>;
}

/** The 10 Hz sampler, as the runner reads it: R12.T2's `PerfSampler`. */
export interface RunSampler {
  start(): void;
  stop(): void;
  readonly samples: ReadonlyArray<PerfSample>;
  summarise(fromMs: number, toMs: number): ThrottleFigures;
  discardBefore(ms: number): void;
}

/** The 1 Hz memory sampler, as the runner reads it: `memory.ts`'s `PerfMemorySampler`. */
export interface RunMemory {
  start(): void;
  stop(): void;
  span(fromMs: number, toMs: number, releases: ReadonlyArray<TallyRelease>): MemorySpan;
  discardBefore(ms: number): void;
}

/** A repetition's trace, as the runner drives it: `frameTrace.ts`'s `WindowedFrameTrace`. */
export interface RunTrace {
  start(repetition: number): Promise<void>;
  stop(): Promise<FrameTraceResult>;
}

/** The build the run measures, which the recipe states (`just perf-run`). */
export interface RunBuild {
  /** `git rev-parse HEAD`, 40 hexadecimal digits. */
  readonly commit: string;
  /** Whether the tree had changes beyond `commit`. */
  readonly dirty: boolean;
  /** `hyperion-base`'s `GENERATOR_VERSION`. */
  readonly generatorVersion: number;
}

/** What a {@link PerfRunner} is given. */
export interface PerfRunnerDeps {
  readonly launch: PerfLaunch;
  /** The scene's server and height workers (`@hyperion/perf`'s `PERF_SCENE_HARNESS`). */
  readonly harness: PerfSceneHarness;
  readonly graphics: {
    readonly launchMode: GraphicsLaunchMode;
    readonly gpuTiming: boolean;
    /** Every switch in force, as Chromium's command line writes them. */
    readonly switches: ReadonlyArray<string>;
  };
  readonly build: RunBuild;
  /** Whether the window is hidden and drawn offscreen. */
  readonly hidden: boolean;
  /**
   * A remote server's URL where `--address` names another machine (its operator's script restarts
   * it), or `null` for the runner's own local server.
   */
  readonly remoteUrl: string | null;
  /** The local server, or `null` with a remote one. */
  readonly server: RunServer | null;
  /** Whether a co-resident server is pinned to one core here (`taskset`, Linux's alone). */
  readonly pinned: boolean;
  /**
   * Arrives at the scene on the server at a URL: the universe made with the pins' seed and the ship
   * placed (`just place-ship`); rejects where the server refuses it.
   */
  readonly place: (serverUrl: string) => Promise<PerfArrival>;
  /** A fresh, empty data directory for the server's `n`th start, 0 the warm-up's. */
  readonly freshDataDir: (n: number) => Promise<string>;
  readonly sampler: RunSampler;
  readonly memory: RunMemory;
  /** The repetitions' trace, or `null` to take the renderer's frame intervals alone. */
  readonly trace: RunTrace | null;
  readonly checkQuiet: () => Promise<PreconditionReport>;
  readonly describeMachine: (description: PerfRunDescription) => Promise<MachineRecord>;
  /** The run window's display's vsync period, ms, or `null` where unknown (a hidden window). */
  readonly vsyncPeriodMs: () => number | null;
  /** The runner's clock, ms since the epoch, which every sample and span shares. */
  readonly now: () => number;
  readonly wait: Wait;
  /** How many times longer the watchdog's limits are: 5 in the software mode, else 1. */
  readonly patience: number;
  /** The `--perf-out` file's text, empty where it does not exist. */
  readonly readOut: () => Promise<string>;
  readonly writeOut: (text: string) => Promise<void>;
  /** Where the record is appended, for the log. */
  readonly outPath: string;
  readonly log: (line: string) => void;
  readonly exit: (code: number) => void;
}

/** How long the renderer may be silent before its views are described, ms (SwiftShader's 52 s compiles, and margin). */
export const DESCRIBE_SILENCE_MS = 600_000;

/** How long the renderer may be silent between warm-up samples, ms. */
export const WARM_UP_SILENCE_MS = 60_000;

/** The room past a repetition's span before its silence is a hang, ms. */
export const SPAN_SILENCE_MARGIN_MS = 300_000;

/** The span assumed for a scene that measures its own (`arrival-memory`), s. */
export const MEASURED_SPAN_ALLOWANCE_S = 1200;

/** Where the run stands. */
type Phase =
  | { readonly kind: "describing" }
  | { readonly kind: "warming"; readonly startMs: number | null }
  | { readonly kind: "between"; readonly done: number }
  | { readonly kind: "spanning"; readonly n: number; readonly fromMs: number }
  | { readonly kind: "finished" };

/** A renderer's call out of turn: the runner refuses it, and the renderer's control fails. */
export class PerfRunRefused extends Error {
  override readonly name = "PerfRunRefused";
}

/** The main process's half of one perf run. */
export class PerfRunner {
  readonly #deps: PerfRunnerDeps;
  #phase: Phase = { kind: "describing" };
  #startedAtMs = 0;
  #state: PreconditionReport | null = null;
  #forcedReasons: ReadonlyArray<string> = [];
  #serverUrl = "";
  #arrival: PerfArrival | null = null;
  #description: PerfRunDescription | null = null;
  #intervals: Timed[] = [];
  #warmUp: { readonly seconds: number; readonly steady: boolean } | null = null;
  #verdict: PerfWarmUpVerdict | null = null;
  readonly #repetitions: RepetitionInputs[] = [];
  #watchdog: AbortController | null = null;
  #finishing: Promise<void> | null = null;

  constructor(deps: PerfRunnerDeps) {
    this.#deps = deps;
  }

  /** Whether the run has ended. */
  get ended(): boolean {
    return this.#phase.kind === "finished" || this.#finishing !== null;
  }

  /** The server record the run's placement gives. */
  get #serverRecord(): ServerRecord {
    const { harness, remoteUrl } = this.#deps;
    return remoteUrl === null
      ? {
          placement: harness.server.placement,
          numWorkers: harness.server.numWorkers,
          restartedPerRepetition: true,
        }
      : { placement: "remote", numWorkers: null, restartedPerRepetition: false };
  }

  /**
   * Checks the preconditions and starts the warm-up's server and the samplers, before the window
   * is made.
   *
   * @returns Whether the run goes on: `false` when a precondition fails without `--force` or the
   *   server cannot be started, each said in the log, and nothing left running.
   */
  async prepare(): Promise<boolean> {
    const { checkQuiet, launch, log, server, remoteUrl, freshDataDir, now } = this.#deps;
    const report = await checkQuiet();
    this.#state = report;
    if (!report.quiet) {
      if (!launch.force) {
        for (const refusal of report.refusals) {
          log(`perf run: REFUSED ${refusal}`);
        }
        log("perf run: the machine is not quiet (Design note 7); --force runs anyway, marked");
        return false;
      }
      this.#forcedReasons = report.refusals;
      for (const refusal of report.refusals) {
        log(`perf run: FORCED past ${refusal}`);
      }
    }
    try {
      this.#serverUrl =
        server === null ? (remoteUrl ?? "") : await server.start(await freshDataDir(0));
      this.#arrival = await this.#deps.place(this.#serverUrl);
    } catch (error: unknown) {
      log(`perf run: the server could not be started or arrived at: ${String(error)}`);
      await server?.stop().catch(() => null);
      return false;
    }
    this.#startedAtMs = now();
    this.#deps.sampler.start();
    this.#deps.memory.start();
    this.#phase = { kind: "describing" };
    this.#arm(DESCRIBE_SILENCE_MS, "describe its views");
    log(
      `perf run: ${launch.scene} at ${launch.setting}, ${String(launch.repetitions)} repetitions, server ${this.#serverUrl}`,
    );
    return true;
  }

  /** The operations `window.hyperion.perf`'s handlers call, each checked against the phase. */
  operations(): {
    config(): Promise<PerfConfig>;
    describe(description: PerfRunDescription): Promise<void>;
    warmUpSample(sample: PerfWarmUpSample): Promise<PerfWarmUpVerdict>;
    beginRepetition(n: number): Promise<PerfRepetitionStart>;
    endRepetition(n: number, result: PerfRepetitionResult): Promise<void>;
    finish(end: PerfEnd): Promise<void>;
  } {
    return {
      config: () => Promise.resolve().then(() => this.#config()),
      describe: (description) => this.#describe(description),
      warmUpSample: (sample) => Promise.resolve(this.#warmUpSample(sample)),
      beginRepetition: (n) => this.#beginRepetition(n),
      endRepetition: (n, result) => this.#endRepetition(n, result),
      finish: (end) => this.#finish(end),
    };
  }

  /** Ends the run as failed for a cause the main process sees: a process gone, the window closed. */
  fault(reason: string): Promise<void> {
    return this.#finish({ status: "failed", reason });
  }

  #config(): PerfConfig {
    const { launch, graphics, hidden } = this.#deps;
    const arrival = this.#arrival;
    if (arrival === null) {
      throw new PerfRunRefused("config refused: the run has not arrived; prepare it first");
    }
    return {
      arrival,
      launch,
      switches: graphics.switches,
      gpuTiming: graphics.gpuTiming,
      launchMode: graphics.launchMode,
      hidden,
      serverUrl: this.#serverUrl,
    };
  }

  #refuseUnless(kind: Phase["kind"], call: string): void {
    if (this.#phase.kind !== kind) {
      throw new PerfRunRefused(`${call} refused: the run is ${this.#phase.kind}, not ${kind}`);
    }
  }

  async #describe(description: PerfRunDescription): Promise<void> {
    this.#refuseUnless("describing", "describe");
    this.#description = description;
    this.#phase = { kind: "warming", startMs: null };
    this.#arm(WARM_UP_SILENCE_MS, "send a warm-up sample");
    await Promise.resolve();
  }

  /** The warm-up's series: the 10 Hz sampler's GPU clock and package, and the renderer's frames. */
  #series(): { gpuMHz: Timed[]; packageDegC: Timed[]; intervalsMs: ReadonlyArray<Timed> } {
    const gpuMHz: Timed[] = [];
    const packageDegC: Timed[] = [];
    for (const { thermal, throttle } of this.#deps.sampler.samples) {
      const mhz = throttle.gpu.source === "none" ? null : throttle.gpu.mhz.value;
      if (mhz !== null) {
        gpuMHz.push({ atMs: thermal.atMs, value: mhz });
      }
      if (thermal.packageDegC.value !== null) {
        packageDegC.push({ atMs: thermal.atMs, value: thermal.packageDegC.value });
      }
    }
    return { gpuMHz, packageDegC, intervalsMs: this.#intervals };
  }

  #warmUpSample(sample: PerfWarmUpSample): PerfWarmUpVerdict {
    if (this.#verdict !== null) {
      return this.#verdict;
    }
    this.#refuseUnless("warming", "warmUpSample");
    const phase = this.#phase;
    const nowMs = this.#deps.now();
    const startMs = phase.kind === "warming" && phase.startMs !== null ? phase.startMs : nowMs;
    this.#phase = { kind: "warming", startMs };
    for (const intervalMs of sample.intervalsMs) {
      this.#intervals.push({ atMs: nowMs, value: intervalMs });
    }
    const verdict: WarmUpVerdict = judgeWarmUp(this.#series(), startMs, nowMs);
    const answer: PerfWarmUpVerdict = { kind: verdict.kind, elapsedS: verdict.elapsedS };
    if (verdict.kind === "warming") {
      this.#arm(WARM_UP_SILENCE_MS, "send a warm-up sample");
      return answer;
    }
    this.#verdict = answer;
    this.#warmUp = { seconds: verdict.elapsedS, steady: verdict.kind === "steady" };
    this.#intervals = [];
    this.#deps.log(
      verdict.kind === "steady"
        ? `perf run: warm-up steady after ${verdict.elapsedS.toFixed(0)} s`
        : `perf run: warm-up NOT STEADY after ${verdict.elapsedS.toFixed(0)} s (${verdict.why})`,
    );
    this.#phase = { kind: "between", done: 0 };
    this.#arm(DESCRIBE_SILENCE_MS, "begin repetition 1");
    return answer;
  }

  async #beginRepetition(n: number): Promise<PerfRepetitionStart> {
    this.#refuseUnless("between", "beginRepetition");
    const phase = this.#phase;
    const done = phase.kind === "between" ? phase.done : 0;
    const { launch, server, freshDataDir, trace, now, log } = this.#deps;
    if (n !== done + 1 || n > launch.repetitions) {
      throw new PerfRunRefused(
        `beginRepetition refused: repetition ${String(n)} follows ${String(done)} of ${String(launch.repetitions)}`,
      );
    }
    // Outside the span: the restart takes a few seconds, the thermal state carrying across it.
    this.#disarm();
    if (server !== null) {
      await server.stop();
      this.#serverUrl = await server.start(await freshDataDir(n));
    }
    await trace?.start(n);
    const fromMs = now();
    this.#phase = { kind: "spanning", n, fromMs };
    this.#arm(this.#spanLimitMs(), `end repetition ${String(n)}`);
    log(`perf run: repetition ${String(n)} of ${String(launch.repetitions)} begins`);
    // The arrival at script time 0, inside the span, so that its work lands in every repetition.
    const arrival = await this.#deps.place(this.#serverUrl);
    this.#arrival = arrival;
    return { serverUrl: this.#serverUrl, arrival };
  }

  /** How long a repetition may run before its silence is a hang, ms, before {@link PerfRunnerDeps.patience}. */
  #spanLimitMs(): number {
    const spanS = this.#description?.repetitionSeconds ?? MEASURED_SPAN_ALLOWANCE_S;
    return 2 * spanS * 1000 + SPAN_SILENCE_MARGIN_MS;
  }

  async #endRepetition(n: number, result: PerfRepetitionResult): Promise<void> {
    this.#refuseUnless("spanning", "endRepetition");
    const phase = this.#phase;
    if (phase.kind !== "spanning" || phase.n !== n) {
      throw new PerfRunRefused(`endRepetition refused: repetition ${String(n)} is not open`);
    }
    const { sampler, memory, trace, now, log, launch } = this.#deps;
    const toMs = now();
    this.#disarm();
    const traced = trace === null ? null : await trace.stop();
    if (traced?.kind === "none") {
      log(
        `perf run: repetition ${String(n)}'s frames are requestAnimationFrame's: ${traced.reason}`,
      );
    }
    const releases: TallyRelease[] = result.releases.map(({ atEpochMs, bytes }) => ({
      atMs: atEpochMs,
      bytes,
    }));
    const memorySpan = memory.span(phase.fromMs, toMs, releases);
    this.#repetitions.push({
      result,
      span: {
        fromMs: phase.fromMs,
        toMs,
        presentedWindowsMs:
          traced?.kind === "presented"
            ? traced.windowsMs.map((window) => window.map((ms) => ms))
            : null,
        throttle: sampler.summarise(phase.fromMs, toMs),
        ...memorySpan,
      },
    });
    sampler.discardBefore(toMs);
    memory.discardBefore(toMs);
    this.#phase = { kind: "between", done: n };
    this.#arm(
      DESCRIBE_SILENCE_MS,
      n === launch.repetitions ? "finish" : `begin repetition ${String(n + 1)}`,
    );
    log(
      `perf run: repetition ${String(n)} ends after ${((toMs - phase.fromMs) / 1000).toFixed(0)} s, ${String(result.frames.intervalsMs.length)} frames`,
    );
  }

  /** Ends the run once: the record assembled and appended where it holds a repetition. */
  #finish(end: PerfEnd): Promise<void> {
    this.#finishing ??= this.#conclude(end);
    return this.#finishing;
  }

  async #conclude(end: PerfEnd): Promise<void> {
    const { log, exit } = this.#deps;
    this.#disarm();
    const failed = end.status === "failed";
    let code = failed ? 1 : 0;
    try {
      await this.#stopAll();
      const record = await this.#record(end);
      if (record === null) {
        code = 1;
      } else {
        const appended = appendRecords(await this.#deps.readOut(), [record]);
        if (appended.kind === "duplicate") {
          log(`perf run: the record is already in ${this.#deps.outPath}, as ${appended.existing}`);
          code = 1;
        } else {
          await this.#deps.writeOut(appended.text);
          log(`perf run: record ${appended.ids.join(", ")} appended to ${this.#deps.outPath}`);
        }
      }
    } catch (error: unknown) {
      log(`perf run: FAIL the record could not be written: ${String(error)}`);
      code = 1;
    } finally {
      this.#phase = { kind: "finished" };
    }
    log(end.status === "failed" ? `perf run: FAIL ${end.reason}` : "perf run: done");
    exit(code);
  }

  /** Stops the samplers, an open trace and the server. */
  async #stopAll(): Promise<void> {
    const { sampler, memory, trace, server, log } = this.#deps;
    sampler.stop();
    memory.stop();
    if (this.#phase.kind === "spanning" && trace !== null) {
      await trace.stop().catch((error: unknown) => {
        log(`perf run: the open trace could not be stopped: ${String(error)}`);
      });
    }
    await server?.stop().catch((error: unknown) => {
      log(`perf run: the server could not be stopped: ${String(error)}`);
    });
  }

  /** The run's record, or `null` where it can hold none (no repetition ended), said in the log. */
  async #record(end: PerfEnd): Promise<PerfRunRecord | null> {
    const { launch, build, graphics, hidden, pinned, harness, log } = this.#deps;
    const description = this.#description;
    const state = this.#state;
    if (description === null || state === null || this.#repetitions.length === 0) {
      log(
        `perf run: no record: the run ended before its first repetition did${end.status === "failed" ? ` (${end.reason})` : ""}`,
      );
      return null;
    }
    const notes: string[] = [end.status === "failed" ? `FAILED: ${end.reason}.` : end.notes];
    if (this.#repetitions.length < launch.repetitions) {
      notes.push(
        `Ended after ${String(this.#repetitions.length)} of ${String(launch.repetitions)} repetitions.`,
      );
    }
    if (harness.server.placement === "co-resident" && !pinned && this.#deps.remoteUrl === null) {
      notes.push("The co-resident server ran unpinned: its pin to one core is taskset's, Linux's.");
    }
    const vsyncPeriodMs = hidden ? null : this.#deps.vsyncPeriodMs();
    const record = assemblePerfRecord({
      launch,
      startedAtMs: this.#startedAtMs,
      commit: build.commit,
      dirty: build.dirty,
      generatorVersion: build.generatorVersion,
      description,
      machine: await this.#deps.describeMachine(description),
      state: state.state,
      forcedReasons: this.#forcedReasons,
      harness: {
        gpuTiming: graphics.gpuTiming,
        switches: graphics.switches,
        launchMode: graphics.launchMode,
        warmUp: this.#warmUp ?? { seconds: 0, steady: false },
        server: this.#serverRecord,
        vsyncPeriodMs,
        display: hidden ? "virtual" : "physical",
      },
      repetitions: this.#repetitions,
      notes,
    });
    // The trend screen reads the record's own per-repetition means, presentations' where traced.
    const trend = trendNote(trendOf(record.runs.map((run) => run.frames.meanMs ?? 0)));
    return trend === null
      ? record
      : validatePerfRecord({ ...record, notes: [record.notes, `${trend}.`].join(" ").trim() });
  }

  /** Starts the watchdog: no call within `ms` times the patience ends the run as a hang. */
  #arm(ms: number, awaited: string): void {
    this.#disarm();
    const { wait, patience } = this.#deps;
    const abort = new AbortController();
    this.#watchdog = abort;
    const limitMs = ms * patience;
    void wait(limitMs, abort.signal)
      .then(() =>
        this.fault(
          `the renderer stopped answering: it did not ${awaited} within ${String(Math.round(limitMs / 1000))} s`,
        ),
      )
      .catch((error: unknown) => {
        this.#deps.log(`perf run: the watchdog's end failed: ${String(error)}`);
      });
  }

  #disarm(): void {
    this.#watchdog?.abort();
    this.#watchdog = null;
  }
}
