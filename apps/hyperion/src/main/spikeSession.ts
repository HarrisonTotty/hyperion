/**
 * One descent-spike run in the main process (plan R05, T13.c, T14.c, T14.d, T14.e): the trace and
 * the 1 Hz memory sampler between the renderer's start and stop, the trace's windows at the
 * renderer's cycles, the results file from its report, the capture's files, and the app's exit with
 * the run's status.
 *
 * @remarks
 * The trace is a list of windows (decision-r05-trace-windows.md). Each window is written to its own
 * file, `spike-trace-<k>.json` under the run's profile, with the buffer's use read just before its
 * stop. The files are reduced in order after the last stop, not during the run, whose CPU they
 * would load, and each is deleted once reduced.
 *
 * Electron is reached only through {@link SpikeSessionDeps}, so that `index.ts` wires it and the
 * logic is testable; the IPC handlers (`registerSpikeHandlers`) call into the session.
 */

import { mkdir, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { DescentSpikeReport, SpikeLaunch, SpikeResultsPaths } from "../preload/api";
import { type Measured, measured, missing } from "./measured";
import type { TraceFigures } from "./reduceTrace";
import {
  buildResults,
  type MemorySample,
  type ResultsFiles,
  type RunDescription,
  writeResults,
} from "./results";
import type { SpikeHandlerDeps, SpikeTraceState } from "./spike";
import type { SpikeCaptureFiles } from "./spikeReport";
import {
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
    stop(path: string): Promise<string>;
    /** Stops it, writes its window to `path`, and starts it again. */
    cycle(path: string): Promise<string>;
    /** The buffer's use, %, or `null` when nothing is reported. */
    bufferUsage(): Promise<number | null>;
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

/** A window's file as its stop left it, before it is reduced. */
interface WrittenWindow {
  readonly path: string;
  /** The buffer's use just before its stop, %, or `null`. */
  readonly bufferPercent: number | null;
}

/** The file name of the run's `k`-th trace window, counted from 0 as `run.trace.windows` is. */
export function traceWindowFileName(k: number): string {
  return `spike-trace-${k}.json`;
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

  /** The next window's file, with the buffer's use read before its stop. */
  async #nextWindow(): Promise<WrittenWindow> {
    const bufferPercent = await this.#deps.trace.bufferUsage();
    const window = {
      path: join(this.#deps.traceDir, traceWindowFileName(this.#windows.length)),
      bufferPercent,
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
    const { path } = await this.#nextWindow();
    await this.#deps.trace.cycle(path);
  }

  async #stop(): Promise<void> {
    if (!this.#measuring) {
      throw new Error("the spike is not measuring");
    }
    this.#refuseWhileBusy();
    this.#measuring = false;
    this.#memory = await this.#deps.memory.stop();
    // A trace a failed cycle ended has no last window to stop.
    if (this.#deps.trace.state === "recording") {
      const { path } = await this.#nextWindow();
      try {
        await this.#deps.trace.stop(path);
      } catch (error: unknown) {
        // Its file is missing, which fails the window; the run goes on.
        this.#deps.log(`descent spike: the trace's last stop failed: ${messageOf(error)}`);
      }
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
    if (this.#deps.launch.smoke) {
      // A smoke run writes no results, so its windows are checked here, failing its stop.
      for (const [k, file] of files.entries()) {
        const failure = windowFileFailure(file);
        if (failure !== null) {
          throw new Error(`trace window ${k + 1} of ${n}: ${failure}`);
        }
      }
    }
  }

  /** One window's file reduced, its size read first, and then removed. */
  async #reduceWindow({ path, bufferPercent }: WrittenWindow): Promise<TraceWindowFile> {
    let bytes: number | null = null;
    try {
      bytes = await this.#files.size(path);
      const trace: Measured<TraceFigures> = measured(
        await this.#deps.reduce(path, this.#deps.trace.settings.categories),
      );
      return { trace, bytes, bufferPercent };
    } catch (error: unknown) {
      const reason =
        bytes === null
          ? `its file could not be read: ${messageOf(error)}`
          : `the trace could not be reduced: ${messageOf(error)}`;
      return { trace: missing(reason), bytes, bufferPercent };
    } finally {
      // A window's trace can reach hundreds of MB; its figures are what the results keep.
      await this.#files.rm(path);
    }
  }

  async #writeResults(report: DescentSpikeReport): Promise<SpikeResultsPaths> {
    if (this.#run === null) {
      throw new Error("the spike wrote results before it started measuring");
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
    const paths = await writeResults(this.#deps.outDir, results, this.#files.results);
    this.#deps.log(`descent spike: results in ${paths.json}`);
    return paths;
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
