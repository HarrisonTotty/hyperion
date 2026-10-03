/**
 * The descent spike's measurement harness in the renderer (plan R05, T13.c and T14.a's pending
 * wiring): the measured engine source, the pass rows, and the recorder that turns the run's
 * listeners into T14.a's `SpikeMetrics`.
 *
 * @remarks
 * The engine is reached only through the spike's `ViewEngineSource` (R01 leaves no other way):
 * its `GPU` is wrapped (`wrapGpu`) so that the device it gives carries T14.a's pipeline tally, a
 * count of the timer's resolves, and T15.a's capture when `--capture` is given, and the same
 * wrapped `GPU` is the engine's `LoadEngineOptions.gpu`, so a rebuild after a device loss is
 * measured too.
 */

import type { DescentSpikeReport, SpikeLatePipeline, SpikePassRow } from "../../../../preload/api";
import type { ViewEngineSource } from "../../displays/view/useViewEngine";
import { loadRenderEngine } from "../engine/loadEngine";
import type { AllocationEvent } from "../engine/memory";
import { requestAdapterOutcome } from "../engine/platform";
import type { PassTimes } from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import { levelBoundM, type PlanetGeometry } from "../terrain/planet";
import { type SelectionInput, selectPatches } from "../terrain/select";
import type { GpuCapture } from "./capture";
import { SETTING_VIEWS } from "./demandRecord";
import { boundedPlanet, type DemandView, levelRatio, perLevelDemand } from "./demand";
import type { DescentProfile } from "./descentProfile";
import { type FrameSample, type PatchEvent, SpikeMetrics } from "./metrics";
import { PipelineTally, shimPipelines, wrapGpu } from "./pipelineShim";

/** The warm-up the criterion leaves out, s (Design note 21), and the pipeline tally's too. */
export const SPIKE_WARMUP_S = 10;

/**
 * Which of Design note 21's GPU rows a pass counts towards, by its `FrameSubmission.label`: the
 * terrain's, the atmosphere's (`atmosphere composite`, the compute `atmosphere view`, and its
 * tables), and the rest (`spike display`, `view:wireframe`).
 */
export function rowOf(label: string): SpikePassRow {
  if (label.startsWith("terrain")) {
    return "terrain";
  }
  return label.startsWith("atmosphere") ? "atmosphere" : "other";
}

/** The spike's own pass labels and their rows, for the capture's `meta.passRows`. */
export const SPIKE_PASS_ROWS: Readonly<Record<string, SpikePassRow>> = {
  terrain: "terrain",
  "atmosphere composite": "atmosphere",
  "atmosphere view": "atmosphere",
  "spike display": "other",
  "view:wireframe": "other",
};

/**
 * Counts R01's timer resolves on a device: each `resolveQuerySet` an encoder makes is one, so the
 * count is the engine's timing frame number (`PassTimes.frame`) as it is made, which its
 * asynchronous times are matched against.
 */
export class ResolveCounter {
  #count = 0;

  /** The resolves so far: the last timing frame number given out. */
  get value(): number {
    return this.#count;
  }

  /** `device`, its `createCommandEncoder` replaced on the instance to count each encoder's resolves. */
  wrap(device: GPUDevice): GPUDevice {
    const create = device.createCommandEncoder.bind(device);
    device.createCommandEncoder = (descriptor?: GPUCommandEncoderDescriptor) => {
      const encoder = create(descriptor);
      const resolve = encoder.resolveQuerySet.bind(encoder);
      encoder.resolveQuerySet = (...args: Parameters<GPUCommandEncoder["resolveQuerySet"]>) => {
        this.#count += 1;
        resolve(...args);
      };
      return encoder;
    };
    return device;
  }
}

/** The spike's measured GPU: its engine source and what the device wrapper feeds. */
export interface SpikeGpu {
  readonly source: ViewEngineSource;
  readonly tally: PipelineTally;
  readonly resolves: ResolveCounter;
}

/**
 * The spike's engine source over `gpu` (the browser's `navigator.gpu`): every device it gives is
 * shimmed for the pipeline tally and the resolve count, then the capture where one is given.
 *
 * @param scriptTimeS - The descent's script time now, s, which each pipeline creation is stamped
 *   with.
 */
export function spikeGpu(
  gpu: GPU | undefined,
  capture: GpuCapture | null,
  scriptTimeS: () => number,
): SpikeGpu {
  const tally = new PipelineTally(scriptTimeS);
  const resolves = new ResolveCounter();
  const wrapped =
    gpu === undefined
      ? undefined
      : wrapGpu(gpu, (device) => {
          const shimmed = resolves.wrap(shimPipelines(device, tally));
          return capture === null ? shimmed : capture.wrapDevice(shimmed);
        });
  return {
    tally,
    resolves,
    source: {
      requestAdapter: () => requestAdapterOutcome(wrapped),
      load: (outcome, status) =>
        loadRenderEngine(outcome, status, wrapped === undefined ? {} : { gpu: wrapped }),
    },
  };
}

/** What the recorder needs of the measured descent (T13.b's `PreparedDescent`). */
export interface RecordedDescent {
  readonly planet: PlanetGeometry;
  readonly profile: DescentProfile;
  /** σ_n for levels 0 to 24 (`omittedSigmaM`), for the calibrated count. */
  readonly omittedSigmaM: ReadonlyArray<number>;
}

/** A segment's mean per-level D over its 1 Hz poses, at the height above the floor (T13.a). */
function meanDemand(
  planet: PlanetGeometry,
  profile: DescentProfile,
  span: { readonly startS: number; readonly endS: number },
  view: DemandView,
): number {
  let sum = 0;
  let count = 0;
  for (let t = span.startS; t < span.endS; t += 1) {
    const pose = profile.poseAt(t);
    sum += perLevelDemand(planet, { ...pose, altitudeM: pose.heightAboveFloorM }, view).perS;
    count += 1;
  }
  return count === 0 ? 0 : sum / count;
}

/**
 * Turns the spike run's listeners into T14.a's metrics: each frame with its engine frames and its
 * patch counts under both bounds, each patch event at its script time, the pass times and the
 * allocations, and the report at the end.
 */
export class SpikeRecorder {
  readonly #metrics: SpikeMetrics;
  readonly #calibrated: PlanetGeometry;
  readonly #resolves: { readonly value: number };
  readonly #tally: PipelineTally;
  #lastScriptS = 0;
  #patchesCalibrated = 0;
  #liveBytes = 0;
  #peakBytes = 0;
  #warmupEnded = false;
  /** Patches baked so far, which the smoke run checks. */
  bakedPatches = 0;

  constructor(
    descent: RecordedDescent,
    setting: QualitySetting,
    gpu: { readonly resolves: { readonly value: number }; readonly tally: PipelineTally },
  ) {
    const { planet, profile, omittedSigmaM } = descent;
    this.#calibrated = boundedPlanet(planet, "calibrated", omittedSigmaM);
    this.#resolves = gpu.resolves;
    this.#tally = gpu.tally;
    const view = SETTING_VIEWS.find((v) => v.setting === setting)?.view;
    if (view === undefined) {
      throw new Error(`no view for the ${setting} setting`);
    }
    const spans = profile.segmentSpans();
    const predicted = new Map(
      spans.map((span) => [
        span.name,
        {
          hardPerS: meanDemand(planet, profile, span, view),
          calibratedPerS: meanDemand(this.#calibrated, profile, span, view),
        },
      ]),
    );
    this.#metrics = new SpikeMetrics({
      warmupS: SPIKE_WARMUP_S,
      segments: spans,
      levels: Array.from({ length: planet.finestLevel + 1 }, (_, level) => ({
        level,
        epsilonM: levelBoundM(planet, level),
        k: levelRatio(planet, level, view),
      })),
      rowOf,
      predicted: (segment) => predicted.get(segment) ?? { hardPerS: 0, calibratedPerS: 0 },
      firstEngineFrame: gpu.resolves.value,
    });
  }

  /** A frame of the run (`SpikeListeners.onFrame`). */
  frame(sample: Omit<FrameSample, "engineFrame" | "patchesCalibrated">): void {
    this.#lastScriptS = sample.scriptTimeS;
    if (!this.#warmupEnded && sample.scriptTimeS >= SPIKE_WARMUP_S) {
      this.#warmupEnded = true;
      this.#tally.endWarmup();
    }
    this.#metrics.frame({
      ...sample,
      engineFrame: this.#resolves.value,
      patchesCalibrated: this.#patchesCalibrated,
    });
  }

  /** A selection's inputs (`SpikeListeners.onSelect`): selected again under min(hard, 4σ_n). */
  select(input: SelectionInput): void {
    this.#patchesCalibrated = selectPatches({ ...input, planet: this.#calibrated }).patches.size;
  }

  /** A patch's progress (`SpikeListeners.onPatch`), at the last frame's script time. */
  patch(event: PatchEvent): void {
    if (event === "baked") {
      this.bakedPatches += 1;
    }
    this.#metrics.patches(event, 1, this.#lastScriptS);
  }

  /** The engine's pass times (`RenderEngine.onPassTimes`). */
  passTimes(times: PassTimes): void {
    this.#metrics.passTimes(times);
  }

  /** The engine's allocations (`RenderEngine.onAllocation`): uploads, and the live peak. */
  allocation(event: AllocationEvent): void {
    this.#metrics.allocation(event);
    if (event.kind === "created") {
      this.#liveBytes += event.bytes;
      this.#peakBytes = Math.max(this.#peakBytes, this.#liveBytes);
    } else if (event.kind === "destroyed") {
      this.#liveBytes = Math.max(0, this.#liveBytes - event.bytes);
    }
  }

  /** The run's report for the results file. */
  report(canvas: DescentSpikeReport["canvas"]): DescentSpikeReport {
    const latePipelines: SpikeLatePipeline[] = this.#tally.late().map((creation) => ({
      label: creation.label.length > 0 ? creation.label : "(unlabelled)",
      kind: creation.kind,
      async: creation.async,
      scriptTimeS: creation.scriptTimeS,
    }));
    return this.#metrics.report({ latePipelines, adapterPeakBytes: this.#peakBytes, canvas });
  }
}
