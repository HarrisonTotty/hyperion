/**
 * The descent spike's run control in the renderer (plan R05, T13.c, T14.e): it answers the run's
 * listeners, starts and stops the main process's measuring, cycles the trace at its windows'
 * boundaries, captures a span of frames when `--capture` is given, and ends the run with its
 * results or, for `--smoke`, its status.
 *
 * @remarks
 * Pure of React and of the browser's globals: `SpikeApp` gives it the preload's functions, the
 * measured GPU and a clock, so that a test drives a whole run with fakes.
 *
 * The trace is cycled when the script first passes each boundary (`traceBoundaries`), without the
 * frame awaiting it. A cycle that fails, or that is still pending at the next boundary, ends the
 * trace but not the run: one last window, failed with the reason, stands for the rest of the run,
 * so that no boundary the trace never reached leaves frames out.
 */

import type {
  DescentSpikeReport,
  SpikeApi,
  SpikeLaunch,
  SpikeTraceWindow,
} from "../../../../preload/api";
import type { AllocationEvent } from "../engine/memory";
import type { PassTimes, RenderEngine } from "../engine/types";
import {
  type TerrainSettings,
  terrainSettingsFor,
  type TerrainVariant,
} from "../quality/qualitySetting";
import type { SelectionInput } from "../terrain/select";
import type { GpuCapture } from "./capture";
import type { PipelineTally } from "./pipelineShim";
import { type RecordedDescent, type ResolveCounter, SpikeRecorder } from "./spikeHarness";
import { TRACE_BOUNDARY_GUARD_S, traceBoundaries } from "./traceWindows";

/** How long `--smoke` runs, script seconds. */
export const SMOKE_S = 10;

/**
 * Where a smoke run cycles its trace, script seconds: three windows in its 10 s, so that the smoke
 * proves the cycle, the window files and their reduction end to end (T14.e), where the descent's
 * first boundary, at 120 s, is beyond it.
 */
export const SMOKE_TRACE_BOUNDARIES_S: ReadonlyArray<number> = [3, 6];

/** How many frames a capture spans (T15.a's fixed span). */
export const CAPTURE_FRAMES = 120;

/** How often the renderer's memory is handed to the main process's sampler, ms. */
const MEMORY_INTERVAL_MS = 1000;

/** What one frame tells the controller (lane C's `SpikeFrameSample`). */
export interface ControllerFrame {
  readonly scriptTimeS: number;
  readonly rafTimestampMs: number;
  /** Script time 0's `requestAnimationFrame` timestamp, ms. */
  readonly scriptStartMs: number;
  /** The callback's start, `performance.now()` ms: its `spike.frame` span's start. */
  readonly callbackStartMs: number;
  readonly callbackMs: number;
  readonly passesSubmitted: number;
  readonly patchesHard: number;
  readonly streaming: boolean;
}

/** The capture's span controls and its result (`GpuCapture`'s). */
export type SpanCapture = Pick<
  GpuCapture,
  "startSpan" | "frame" | "endSpan" | "dispose" | "result"
>;

/** What the controller is given. */
export interface SpikeControllerDeps {
  readonly spike: SpikeApi;
  readonly gpu: {
    readonly resolves: Pick<ResolveCounter, "value" | "runFrame">;
    readonly tally: PipelineTally;
  };
  /** T15.a's capture when `--capture` is given (the parts the controller drives). */
  readonly capture: SpanCapture | null;
  /** The main view's canvas size, device pixels, for the report. */
  readonly canvas: () => DescentSpikeReport["canvas"];
  /** `performance.now()`, ms: the clock the trace's window times are read on. */
  readonly nowMs: () => number;
  readonly log: (message: string, error?: unknown) => void;
}

/** The terrain variant `launch`'s `--vertex-path` and `--normals` ask for (nulls left out). */
export function variantOf(launch: SpikeLaunch): TerrainVariant {
  return {
    ...(launch.vertexPath === null ? {} : { vertexPath: launch.vertexPath }),
    ...(launch.normals === null ? {} : { normals: launch.normals }),
  };
}

/** A trace window's times as the controller records them, `null` until known. */
interface WindowTimes {
  readonly startedMs: number;
  stopRequestedMs: number | null;
  readonly failure: string | null;
}

/** The trace operation in flight: its start, or its cycle at a boundary, script seconds. */
interface PendingTrace {
  readonly atBoundaryS: number | null;
}

/** An error's message for a reason, with a leading colon, or nothing. */
function detailOf(error: unknown): string {
  return error instanceof Error ? `: ${error.message}` : "";
}

/** The capture's span: from where it starts, script seconds, and how many frames are left. */
interface CaptureSpan {
  readonly fromS: number;
  started: Promise<void> | null;
  framesLeft: number;
  written: Promise<void> | null;
}

/** One run's control. */
export class SpikeController {
  readonly #deps: SpikeControllerDeps;
  readonly #launch: SpikeLaunch;
  #recorder: SpikeRecorder | null = null;
  #durationS = Infinity;
  #ended = false;
  #lastMemoryMs = Number.NEGATIVE_INFINITY;
  #measuring: Promise<void> | null = null;
  #capture: CaptureSpan | null = null;
  #terrain: TerrainSettings | null = null;
  /** Where the trace is cycled, script seconds, ascending. */
  #boundariesS: ReadonlyArray<number> = [];
  /** The next boundary in {@link SpikeController.#boundariesS} the script has not passed. */
  #nextBoundary = 0;
  /** The trace's windows so far, each from its start resolving. */
  readonly #windows: WindowTimes[] = [];
  /** The start or cycle in flight, or `null`. */
  #pending: PendingTrace | null = null;
  /** The last cycle, settled (its failure handled), for the finish to await. */
  #cycled: Promise<void> = Promise.resolve();
  /** Why the trace ended before the run, or `null`. */
  #traceEnded: string | null = null;

  constructor(deps: SpikeControllerDeps) {
    this.#deps = deps;
    this.#launch = deps.spike.launch;
  }

  /** Whether the run has ended (its end sent or being sent). */
  get ended(): boolean {
    return this.#ended;
  }

  /** The engine the run draws on: its pass times and allocations go to the recorder. */
  engine(engine: Pick<RenderEngine, "onPassTimes" | "onAllocation">): () => void {
    const offTimes = engine.onPassTimes((times: PassTimes) => {
      this.#recorder?.passTimes(times);
    });
    const offAllocation = engine.onAllocation((event: AllocationEvent) => {
      this.#recorder?.allocation(event);
    });
    return () => {
      offTimes();
      offAllocation();
    };
  }

  /**
   * The descent is measured: recording and measuring begin, or a variant the setting cannot take
   * (low with baked offsets, which the command line refuses too) ends the run.
   */
  prepared(descent: RecordedDescent): void {
    try {
      this.#terrain = terrainSettingsFor(this.#launch.setting, variantOf(this.#launch));
    } catch (error: unknown) {
      this.fail("the terrain variant does not fit the setting", error);
      return;
    }
    try {
      this.#boundariesS = this.#launch.smoke
        ? SMOKE_TRACE_BOUNDARIES_S
        : traceBoundaries(descent.profile.segmentSpans());
    } catch (error: unknown) {
      this.fail("the trace's windows could not be placed", error);
      return;
    }
    this.#recorder = new SpikeRecorder(descent, this.#launch.setting, this.#deps.gpu);
    this.#durationS = this.#launch.smoke ? SMOKE_S : descent.profile.durationS;
    if (this.#deps.capture !== null) {
      const pass = descent.profile.segmentSpans().find(({ name }) => name === "low fast pass");
      // The smoke's span starts at 5 s; a full run's in the low fast pass, where demand peaks.
      const fromS = this.#launch.smoke ? SMOKE_S / 2 : (pass?.startS ?? 0) + 5;
      this.#capture = { fromS, started: null, framesLeft: CAPTURE_FRAMES, written: null };
    }
    this.#measuring = this.#startTrace();
    this.#measuring.catch((error: unknown) => {
      this.fail("the trace did not start", error);
    });
  }

  /** Starts the trace: its first window runs from the start resolving to the first boundary. */
  async #startTrace(): Promise<void> {
    this.#pending = { atBoundaryS: null };
    try {
      await this.#deps.spike.startTrace();
      this.#opened();
    } finally {
      this.#pending = null;
    }
  }

  /** A window has begun, its start resolved now, unless the trace has ended. */
  #opened(): void {
    if (this.#traceEnded === null) {
      this.#windows.push({ startedMs: this.#deps.nowMs(), stopRequestedMs: null, failure: null });
    }
  }

  /**
   * Ends the trace before the run, once: no further cycle, and one last window, failed with
   * `reason`, from now to the run's end.
   */
  #endTrace(reason: string): void {
    if (this.#traceEnded !== null) {
      return;
    }
    this.#traceEnded = reason;
    this.#deps.log(`the trace ended before the run: ${reason}`);
    this.#windows.push({ startedMs: this.#deps.nowMs(), stopRequestedMs: null, failure: reason });
  }

  /** Cycles the trace when the script first passes its next boundary. */
  #traceFrame(scriptTimeS: number): void {
    const boundaryS = this.#boundariesS[this.#nextBoundary];
    if (
      this.#traceEnded !== null ||
      boundaryS === undefined ||
      scriptTimeS < boundaryS ||
      scriptTimeS >= this.#durationS
    ) {
      return;
    }
    this.#nextBoundary += 1;
    const pending = this.#pending;
    if (pending !== null) {
      const what =
        pending.atBoundaryS === null ? "start" : `cycle at ${String(pending.atBoundaryS)} s`;
      this.#endTrace(
        `the trace's ${what} was still pending at the boundary at ${String(boundaryS)} s`,
      );
      return;
    }
    const current = this.#windows.at(-1);
    if (current === undefined) {
      // The start failed, which has ended the run.
      return;
    }
    current.stopRequestedMs = this.#deps.nowMs();
    this.#cycled = this.#cycle(boundaryS);
  }

  /** Cycles the trace at a boundary, opening the next window or ending the trace; never rejects. */
  async #cycle(boundaryS: number): Promise<void> {
    this.#pending = { atBoundaryS: boundaryS };
    try {
      await this.#deps.spike.cycleTrace();
      this.#opened();
    } catch (error: unknown) {
      this.#endTrace(`the trace's cycle at ${String(boundaryS)} s failed${detailOf(error)}`);
    } finally {
      this.#pending = null;
    }
  }

  /** A selection's inputs, counted again under min(hard, 4σ_n). */
  select(input: SelectionInput): void {
    this.#recorder?.select(input);
  }

  /** A patch's progress. */
  patch(event: "requested" | "baked" | "resident"): void {
    this.#recorder?.patch(event);
  }

  /** A frame of the run. */
  frame(sample: ControllerFrame): void {
    const recorder = this.#recorder;
    if (recorder === null || this.#ended) {
      return;
    }
    recorder.frame(sample);
    if (!this.#launch.smoke && sample.rafTimestampMs - this.#lastMemoryMs >= MEMORY_INTERVAL_MS) {
      this.#lastMemoryMs = sample.rafTimestampMs;
      this.#deps.spike.sampleMemory().catch((error: unknown) => {
        this.#deps.log("a memory sample failed", error);
      });
    }
    this.#captureFrame(sample.scriptTimeS);
    this.#traceFrame(sample.scriptTimeS);
    if (sample.scriptTimeS >= this.#durationS) {
      void this.#finish(recorder, sample.scriptStartMs);
    }
  }

  /** Ends the run as failed, once. */
  fail(reason: string, error?: unknown): void {
    if (this.#ended) {
      return;
    }
    this.#ended = true;
    this.#deps.log(`the descent spike failed: ${reason}`, error);
    this.#deps.capture?.dispose();
    this.#deps.spike
      .end({ status: "fail", reason: `${reason}${detailOf(error)}` })
      .catch((e: unknown) => {
        this.#deps.log("the run's end was refused", e);
      });
  }

  /** Starts, marks and ends the capture's span around the frames that follow. */
  #captureFrame(scriptTimeS: number): void {
    const span = this.#capture;
    const capture = this.#deps.capture;
    if (span === null || capture === null || span.written !== null) {
      return;
    }
    if (span.started === null) {
      if (scriptTimeS >= span.fromS) {
        // Between frames: this frame's work is submitted, the next one's not begun.
        span.started = capture.startSpan();
        span.started.catch((error: unknown) => {
          this.fail("the capture's span did not start", error);
        });
      }
      return;
    }
    capture.frame();
    span.framesLeft -= 1;
    if (span.framesLeft <= 0) {
      this.#endCapture(span, capture);
    }
  }

  /** Ends the capture's span and writes it, once its snapshot is read back. */
  #endCapture(span: CaptureSpan, capture: SpanCapture): void {
    const started = span.started;
    if (started === null || span.written !== null) {
      return;
    }
    capture.endSpan();
    const write = async (): Promise<void> => {
      await started;
      const { file, data } = capture.result();
      await this.#deps.spike.writeCapture({ json: JSON.stringify(file), bin: data });
    };
    span.written = write();
    span.written.catch((error: unknown) => {
      this.fail("the capture was not written", error);
    });
  }

  async #finish(recorder: SpikeRecorder, scriptStartMs: number): Promise<void> {
    if (this.#ended) {
      return;
    }
    this.#ended = true;
    const spike = this.#deps.spike;
    try {
      await this.#measuring;
      // A cycle still in flight settles first, so that the main process stops one recording.
      await this.#cycled;
      const span = this.#capture;
      const capture = this.#deps.capture;
      if (span !== null && capture !== null) {
        // A span the run outlasted is ended where the run ends.
        this.#endCapture(span, capture);
        await span.written;
      }
      const stopRequestedMs = this.#deps.nowMs();
      const last = this.#windows.at(-1);
      if (last !== undefined && last.stopRequestedMs === null) {
        last.stopRequestedMs = stopRequestedMs;
      }
      await spike.stopTrace();
      if (this.#launch.smoke) {
        const ended = this.#traceEnded;
        await spike.end(
          ended !== null
            ? { status: "fail", reason: ended }
            : recorder.bakedPatches > 0
              ? { status: "pass" }
              : { status: "fail", reason: "no patch was baked in a worker" },
        );
        return;
      }
      const traceWindows: SpikeTraceWindow[] = this.#windows.map((window) => ({
        startedMs: window.startedMs,
        stopRequestedMs: window.stopRequestedMs ?? stopRequestedMs,
        failure: window.failure,
      }));
      const terrain = this.#terrain;
      await spike.writeResults({
        ...recorder.report(this.#deps.canvas()),
        scriptStartMs,
        traceWindows,
        traceGuardS: TRACE_BOUNDARY_GUARD_S,
        ...(terrain === null
          ? {}
          : { terrain: { vertexPath: terrain.vertexPath, normals: terrain.normals } }),
      });
      await spike.end({ status: "pass" });
    } catch (error: unknown) {
      this.#deps.log("the descent spike could not finish", error);
      const reason = error instanceof Error ? error.message : String(error);
      await spike.end({ status: "fail", reason }).catch((e: unknown) => {
        this.#deps.log("the run's end was refused", e);
      });
    }
  }
}
