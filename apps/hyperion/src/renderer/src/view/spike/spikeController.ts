/**
 * The descent spike's run control in the renderer (plan R05, T13.c): it answers the run's
 * listeners, starts and stops the main process's measuring, captures a span of frames when
 * `--capture` is given, and ends the run with its results or, for `--smoke`, its status.
 *
 * @remarks
 * Pure of React and of the browser's globals: `SpikeApp` gives it the preload's functions, the
 * measured GPU and a clock, so that a test drives a whole run with fakes.
 */

import type { DescentSpikeReport, SpikeApi, SpikeLaunch } from "../../../../preload/api";
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

/** How long `--smoke` runs, script seconds. */
export const SMOKE_S = 10;

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
  /** When the trace's start resolved, ms; `null` until it has. */
  #traceStartedMs: number | null = null;

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
    this.#recorder = new SpikeRecorder(descent, this.#launch.setting, this.#deps.gpu);
    this.#durationS = this.#launch.smoke ? SMOKE_S : descent.profile.durationS;
    if (this.#deps.capture !== null) {
      const pass = descent.profile.segmentSpans().find(({ name }) => name === "low fast pass");
      // The smoke's span starts at 5 s; a full run's in the low fast pass, where demand peaks.
      const fromS = this.#launch.smoke ? SMOKE_S / 2 : (pass?.startS ?? 0) + 5;
      this.#capture = { fromS, started: null, framesLeft: CAPTURE_FRAMES, written: null };
    }
    this.#measuring = this.#launch.smoke ? Promise.resolve() : this.#startTrace();
    this.#measuring.catch((error: unknown) => {
      this.fail("the trace did not start", error);
    });
  }

  /** Starts the trace: its one window (T14.d) runs from the start resolving to the stop's request. */
  async #startTrace(): Promise<void> {
    await this.#deps.spike.startTrace();
    this.#traceStartedMs = this.#deps.nowMs();
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
    const detail = error instanceof Error ? `: ${error.message}` : "";
    this.#deps.spike.end({ status: "fail", reason: `${reason}${detail}` }).catch((e: unknown) => {
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
      const span = this.#capture;
      const capture = this.#deps.capture;
      if (span !== null && capture !== null) {
        // A span the run outlasted is ended where the run ends.
        this.#endCapture(span, capture);
        await span.written;
      }
      if (this.#launch.smoke) {
        await spike.end(
          recorder.bakedPatches > 0
            ? { status: "pass" }
            : { status: "fail", reason: "no patch was baked in a worker" },
        );
        return;
      }
      const startedMs = this.#traceStartedMs;
      if (startedMs === null) {
        throw new Error("the trace's start resolved without its time recorded");
      }
      const stopRequestedMs = this.#deps.nowMs();
      await spike.stopTrace();
      const terrain = this.#terrain;
      await spike.writeResults({
        ...recorder.report(this.#deps.canvas()),
        scriptStartMs,
        traceWindows: [{ startedMs, stopRequestedMs }],
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
