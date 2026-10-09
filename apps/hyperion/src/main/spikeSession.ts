/**
 * One descent-spike run in the main process (plan R05, T13.c, T14.c, T14.d, T14.e, T14.i, T14.k):
 * the trace and the 1 Hz memory sampler, with the GPU's clocks beside the memory, between the
 * renderer's start and stop, the trace's windows at the renderer's cycles, the results file from
 * its report, the capture's files, and the app's exit with the run's status.
 *
 * @remarks
 * The trace is a list of windows (decision-r05-trace-windows.md). Each window is written to its own
 * file, `spike-trace-<k>.pftrace` under the run's profile, with what its stop told: the buffer's
 * last reported use and whether Chromium lost data. The trace's transport is closed after the last
 * stop. The files are reduced in order after the last stop, not during the run, whose CPU they
 * would load, and each is deleted once reduced.
 *
 * A smoke run hands its report to the same results call as a full run
 * (decision-r05-trace-windows-2.md, addendum A): its windows are merged with it, the frame-span
 * check included, each window's verdict is logged, and the call answers the first failed window's
 * reason, with which the renderer fails the smoke. A smoke builds and writes no results file.
 *
 * Electron is reached only through {@link SpikeSessionDeps}, so that `index.ts` wires it and the
 * logic is testable; the IPC handlers (`registerSpikeHandlers`) call into the session.
 */

import { mkdir, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { DescentSpikeReport, SpikeLaunch, SpikeResultsAnswer } from "../preload/api";
import { type Measured, measured, missing } from "./measured";
import type { TraceFigures } from "./reduceTrace";
import {
  buildResults,
  describeClocks,
  describeClockSamples,
  gpuClocksOf,
  type MemorySample,
  type ResultsFiles,
  type RunDescription,
  writeResults,
} from "./results";
import type { SpikeHandlerDeps, SpikeTraceState, TraceWindowStop } from "./spike";
import type { SpikeCaptureFiles } from "./spikeReport";
import {
  mergeTraceWindows,
  type TraceRecording,
  type TraceSettings,
  type TraceWindowFile,
  windowFileFailure,
} from "./traceWindows";

/** What a session needs of the main process. */
export interface SpikeSessionDeps {
  readonly launch: SpikeLaunch;
  /** The run's description, gathered at its start (`describeMachine` and the launch). */
  readonly describe: () => Promise<RunDescription>;
  /** The trace (`SpikeTrace`). */
  readonly trace: {
    /** How every window is recorded, as the run's file records it. */
    readonly settings: TraceSettings;
    /** Whether it is recording, stopped, or in the middle of a start, stop or cycle. */
    readonly state: SpikeTraceState;
    start(): Promise<void>;
    /** Stops it and writes its window to `path`. */
    stop(path: string): Promise<TraceWindowStop>;
    /**
     * Stops it, writes its window to `path`, tells `stopped` what the stop told, and starts it
     * again.
     */
    cycle(path: string, stopped: (stop: TraceWindowStop) => void): Promise<void>;
    /** Ends its transport after the last stop. */
    close(): void;
  };
  /**
   * Where the trace's windows are written: the run's own profile (`userData`), which the recipe
   * keeps on disk and removes.
   */
  readonly traceDir: string;
  /**
   * Reduces a window's file (`reduceTraceFile`), given the categories every window recorded
   * (`trace.settings.categories`), which say which GPU-process slices it can hold.
   */
  readonly reduce: (path: string, categories: ReadonlyArray<string>) => Promise<TraceFigures>;
  readonly memory: { start(): void; stop(): Promise<ReadonlyArray<MemorySample>> };
  /** The results file's directory (`--out`, or `docs/measurements/descent-spike/`). */
  readonly outDir: string;
  readonly exit: (code: number) => void;
  readonly log: (line: string) => void;
  /** The file operations, Node's by default. */
  readonly files?: SpikeFiles;
}

/** The file operations a session makes. */
export interface SpikeFiles {
  mkdir(path: string): Promise<unknown>;
  writeFile(path: string, data: string | Uint8Array): Promise<void>;
  rm(path: string): Promise<void>;
  /** A file's size, bytes. */
  size(path: string): Promise<number>;
  /** For the results file (`writeResults`'s). */
  readonly results?: ResultsFiles;
}

function messageOf(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

const NODE_FILES: SpikeFiles = {
  mkdir: (path) => mkdir(path, { recursive: true }),
  writeFile: (path, data) => writeFile(path, data),
  rm: (path) => rm(path, { force: true }),
  size: (path) => stat(path).then(({ size }) => size),
};

/** What became of a window's stop: still running, whole with what it told, or failed and why. */
type WindowStop =
  | { readonly kind: "pending" }
  | { readonly kind: "stopped"; readonly stop: TraceWindowStop }
  | { readonly kind: "failed"; readonly reason: string };

/** A window's file as its stop left it, before it is reduced. */
interface WrittenWindow {
  readonly path: string;
  outcome: WindowStop;
}

/** The file name of the run's `k`-th trace window, counted from 0 as `run.trace.windows` is. */
export function traceWindowFileName(k: number): string {
  return `spike-trace-${k}.pftrace`;
}

/** A window that gave no trace, for `reason`. */
function unreduced(reason: string): TraceWindowFile {
  return { trace: missing(reason), bytes: null, bufferPercent: null, lostData: false };
}

/** A window's reduction in a line of the run's log. */
function describeWindow(k: number, n: number, file: TraceWindowFile): string {
  const failure = windowFileFailure(file);
  if (failure !== null) {
    return `descent spike: trace window ${k + 1} of ${n} failed: ${failure}`;
  }
  const buffer =
    file.bufferPercent === null
      ? "no buffer reading"
      : `${file.bufferPercent.toFixed(2)} % of its buffer`;
  return `descent spike: trace window ${k + 1} of ${n}: ${String(file.bytes)} B, ${buffer}`;
}

/** A run of the spike: the state between the renderer's calls. */
export class SpikeSession {
  readonly #deps: SpikeSessionDeps;
  #run: Promise<RunDescription> | null = null;
  #measuring = false;
  /** The windows stopped so far, in order. */
  #windows: WrittenWindow[] = [];
  #trace: Measured<TraceRecording> = missing("the trace was not recorded");
  #memory: ReadonlyArray<MemorySample> = [];
  /** The renderer's last private-memory reading, bytes. */
  #rendererBytes: number | null = null;
  #ended = false;

  /** Whether the renderer has ended the run (`end`). */
  get ended(): boolean {
    return this.#ended;
  }

  readonly #files: SpikeFiles;

  constructor(deps: SpikeSessionDeps) {
    this.#deps = deps;
    this.#files = deps.files ?? NODE_FILES;
  }

  /** The renderer's private bytes, for the sampler's `rendererPrivateBytes`. */
  rendererPrivateBytes(): number | null {
    return this.#rendererBytes;
  }

  /** The handlers' operations, for `registerSpikeHandlers`. */
  operations(): Omit<SpikeHandlerDeps<never>, "handle" | "isSender"> {
    return {
      startMeasuring: () => this.#start(),
      cycleTrace: () => this.#cycle(),
      stopMeasuring: () => this.#stop(),
      rendererMemory: (bytes) => {
        this.#rendererBytes = bytes;
      },
      writeResults: (report) => this.#writeResults(report),
      writeCapture: (capture) => this.#writeCapture(capture),
      end: (code, reason) => {
        this.#ended = true;
        this.#deps.log(
          code === 0 ? "descent spike: pass" : `descent spike: FAIL ${reason ?? "(no reason)"}`,
        );
        this.#deps.exit(code);
      },
    };
  }

  async #start(): Promise<void> {
    if (this.#measuring) {
      throw new Error("the spike is already measuring");
    }
    this.#run = this.#deps.describe();
    await this.#run;
    this.#windows = [];
    await this.#deps.trace.start();
    this.#deps.memory.start();
    this.#measuring = true;
  }

  /** The next window's file, before its stop. */
  #nextWindow(): WrittenWindow {
    const window: WrittenWindow = {
      path: join(this.#deps.traceDir, traceWindowFileName(this.#windows.length)),
      outcome: { kind: "pending" },
    };
    this.#windows.push(window);
    return window;
  }

  /** Refuses a call that would overlap a start, stop or cycle still in flight. */
  #refuseWhileBusy(): void {
    if (this.#deps.trace.state === "busy") {
      throw new Error("the spike's trace is busy with another start, stop or cycle");
    }
  }

  /**
   * Ends the trace's window and begins the next, at the renderer's boundary.
   *
   * @throws Error if the spike is not measuring or the trace is not recording, before any window
   * is written; or if the stop or the start fails, the window then written or failed.
   */
  async #cycle(): Promise<void> {
    if (!this.#measuring) {
      throw new Error("the spike is not measuring");
    }
    this.#refuseWhileBusy();
    if (this.#deps.trace.state !== "recording") {
      throw new Error("the spike's trace is not recording");
    }
    const window = this.#nextWindow();
    try {
      await this.#deps.trace.cycle(window.path, (stop) => {
        window.outcome = { kind: "stopped", stop };
      });
    } catch (error: unknown) {
      // A start that failed after a whole stop leaves the window whole.
      if (window.outcome.kind === "pending") {
        window.outcome = { kind: "failed", reason: messageOf(error) };
      }
      throw error;
    }
  }

  async #stop(): Promise<void> {
    if (!this.#measuring) {
      throw new Error("the spike is not measuring");
    }
    this.#refuseWhileBusy();
    this.#measuring = false;
    this.#memory = await this.#deps.memory.stop();
    try {
      // A trace a failed cycle ended has no last window to stop.
      if (this.#deps.trace.state === "recording") {
        const window = this.#nextWindow();
        try {
          window.outcome = { kind: "stopped", stop: await this.#deps.trace.stop(window.path) };
        } catch (error: unknown) {
          // The window fails with the reason; the run goes on.
          const reason = messageOf(error);
          window.outcome = { kind: "failed", reason };
          this.#deps.log(`descent spike: the trace's last stop failed: ${reason}`);
        }
      }
    } finally {
      // After the last stop, whatever became of it, so that no frame sees the detach.
      this.#deps.trace.close();
    }
    const files: TraceWindowFile[] = [];
    const n = this.#windows.length;
    for (const [k, window] of this.#windows.entries()) {
      // One window at a time, in order: each file can reach hundreds of MB.
      // oxlint-disable-next-line no-await-in-loop
      const file = await this.#reduceWindow(window);
      this.#deps.log(describeWindow(k, n, file));
      files.push(file);
    }
    this.#trace = measured({ settings: this.#deps.trace.settings, windows: files });
  }

  /** One window's file reduced, its size read first, and then removed. */
  async #reduceWindow({ path, outcome }: WrittenWindow): Promise<TraceWindowFile> {
    try {
      let file: TraceWindowFile;
      switch (outcome.kind) {
        case "pending":
          // Every stop has settled by the last one: a window still pending is a broken invariant.
          file = unreduced("its stop never finished");
          break;
        case "failed":
          // A stop that failed may have left part of a file: none of it is read.
          file = unreduced(outcome.reason);
          break;
        case "stopped":
          file = await this.#reduceStopped(path, outcome.stop);
          break;
      }
      return file;
    } finally {
      // A window's trace can reach hundreds of MB; its figures are what the results keep.
      await this.#files.rm(path);
    }
  }

  /** A whole window's file reduced, with what its stop told. */
  async #reduceStopped(
    path: string,
    { bufferPercent, lostData }: TraceWindowStop,
  ): Promise<TraceWindowFile> {
    let bytes: number | null = null;
    try {
      bytes = await this.#files.size(path);
      const trace: Measured<TraceFigures> = measured(
        await this.#deps.reduce(path, this.#deps.trace.settings.categories),
      );
      return { trace, bytes, bufferPercent, lostData };
    } catch (error: unknown) {
      const reason =
        bytes === null
          ? `its file could not be read: ${messageOf(error)}`
          : `the trace could not be reduced: ${messageOf(error)}`;
      return { trace: missing(reason), bytes, bufferPercent, lostData };
    }
  }

  /**
   * A smoke's windows checked against its report: merged, the frame-span check included, each
   * window's verdict and each boundary's gap logged, since a smoke writes no results file.
   *
   * @returns The first failed window's reason ("trace window k of n: …"), or why the run has no
   * trace; `null` when every window passed.
   */
  #checkSmokeTrace(report: DescentSpikeReport): string | null {
    const merged = mergeTraceWindows(this.#trace, report);
    const windows = merged.run.value?.windows ?? [];
    for (const [k, window] of windows.entries()) {
      const where = `trace window ${k + 1} of ${windows.length}`;
      const checked = merged.checkedFrames[k] ?? null;
      this.#deps.log(
        window.figures.value === null
          ? `descent spike: ${where} failed: ${window.figures.reason}`
          : `descent spike: ${where}: the spans of ${String(checked ?? 0)} frames match the renderer's`,
      );
    }
    const boundaries = merged.run.value?.boundaries ?? [];
    for (const [k, { stopRequestedS, resumedS }] of boundaries.entries()) {
      this.#deps.log(
        `descent spike: trace boundary ${k + 1} of ${boundaries.length}: ` +
          `${((resumedS - stopRequestedS) * 1000).toFixed(0)} ms from the stop to the next start`,
      );
    }
    return merged.figures.reason;
  }

  async #writeResults(report: DescentSpikeReport): Promise<SpikeResultsAnswer> {
    if (this.#run === null) {
      throw new Error("the spike wrote results before it started measuring");
    }
    if (this.#deps.launch.smoke) {
      // A smoke's 10 s are all warm-up and it writes no file: its log holds every clock sample.
      this.#deps.log(
        `descent spike: GPU clocks sampled ${describeClockSamples(gpuClocksOf(this.#memory))}`,
      );
      return { kind: "smoke checked", failure: this.#checkSmokeTrace(report) };
    }
    const described = await this.#run;
    // The terrain variant the renderer drew with, beside the flags that asked for it.
    const run =
      report.terrain === undefined
        ? described
        : {
            ...described,
            options: {
              ...described.options,
              terrainVertexPath: report.terrain.vertexPath,
              terrainNormals: report.terrain.normals,
            },
          };
    const results = buildResults({ run, report, trace: this.#trace, memory: this.#memory });
    const { clocks } = results.gpu;
    this.#deps.log(
      `descent spike: GPU clocks ${clocks.value === null ? `— (${clocks.reason})` : describeClocks(clocks.value, results.memory.series.tMs, report.warmupS)}`,
    );
    const paths = await writeResults(this.#deps.outDir, results, this.#files.results);
    this.#deps.log(`descent spike: results in ${paths.json}`);
    return { kind: "written", paths };
  }

  async #writeCapture(capture: SpikeCaptureFiles): Promise<string> {
    const dir = this.#deps.launch.capture;
    if (dir === null) {
      throw new Error("the spike was launched without --capture");
    }
    await this.#files.mkdir(dir);
    await this.#files.writeFile(join(dir, "capture.json"), capture.json);
    await this.#files.writeFile(join(dir, "capture.bin"), capture.bin);
    this.#deps.log(`descent spike: capture in ${dir}`);
    return dir;
  }
}
