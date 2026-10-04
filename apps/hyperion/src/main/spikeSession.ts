/**
 * One descent-spike run in the main process (plan R05, T13.c, T14.c): the trace and the 1 Hz
 * memory sampler between the renderer's start and stop, the results file from its report, the
 * capture's files, and the app's exit with the run's status.
 *
 * @remarks
 * Electron is reached only through {@link SpikeSessionDeps}, so that `index.ts` wires it and the
 * logic is testable; the IPC handlers (`registerSpikeHandlers`) call into the session.
 */

import { mkdir, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { DescentSpikeReport, SpikeLaunch, SpikeResultsPaths } from "../preload/api";
import type { TraceFigures } from "./reduceTrace";
import {
  buildResults,
  type MemorySample,
  type Measured,
  measured,
  missing,
  type ResultsFiles,
  type RunDescription,
  writeResults,
} from "./results";
import type { SpikeHandlerDeps } from "./spike";
import type { SpikeCaptureFiles } from "./spikeReport";

/** What a session needs of the main process. */
export interface SpikeSessionDeps {
  readonly launch: SpikeLaunch;
  /** The run's description, gathered at its start (`describeMachine` and the launch). */
  readonly describe: () => Promise<RunDescription>;
  readonly trace: {
    start(): Promise<void>;
    /** Stops it and writes it to `path`. */
    stop(path: string): Promise<string>;
  };
  /** Where the trace is written: in the run's own profile, which the recipe removes. */
  readonly tracePath: string;
  readonly reduce: (path: string) => Promise<TraceFigures>;
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
  /** For the results file (`writeResults`'s). */
  readonly results?: ResultsFiles;
}

const NODE_FILES: SpikeFiles = {
  mkdir: (path) => mkdir(path, { recursive: true }),
  writeFile: (path, data) => writeFile(path, data),
  rm: (path) => rm(path, { force: true }),
};

/** A run of the spike: the state between the renderer's calls. */
export class SpikeSession {
  readonly #deps: SpikeSessionDeps;
  #run: Promise<RunDescription> | null = null;
  #measuring = false;
  #trace: Measured<TraceFigures> = missing("the trace was not recorded");
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
    await this.#deps.trace.start();
    this.#deps.memory.start();
    this.#measuring = true;
  }

  async #stop(): Promise<void> {
    if (!this.#measuring) {
      throw new Error("the spike is not measuring");
    }
    this.#measuring = false;
    this.#memory = await this.#deps.memory.stop();
    const path = await this.#deps.trace.stop(this.#deps.tracePath);
    try {
      this.#trace = measured(await this.#deps.reduce(path));
    } catch (error: unknown) {
      this.#trace = missing(
        `the trace could not be reduced: ${error instanceof Error ? error.message : String(error)}`,
      );
    } finally {
      // The trace can reach 1.4 GB; its figures are what the results keep.
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
