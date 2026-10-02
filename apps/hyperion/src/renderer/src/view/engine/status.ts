/**
 * The graphics status every view and the `LINK` display read: the adapter, the launch mode, the
 * timer and any fault, in the console's own words.
 *
 * @remarks
 * A lost device and a crashed GPU process are faults while they last, in `StatusLine`'s `fault`
 * standing; a refused software adapter, no WebGPU or no adapter, the safe mode and the disabled
 * state, and the wait for the adapter, are the console stating its own condition, in the `refused`
 * standing (plain text). None is an alert: the guide says alerts are raised by the server and a
 * console never invents one (R01 Design note 10). A refused shader (`GRAPHICS SHADER REFUSED`) and a
 * view not re-created after a device loss (`GRAPHICS VIEW REFUSED`, decided 2026-10-02) are faults
 * too. The words were signed off on 2026-09-30 (R01.T5.c); each is one constant here.
 */

import { createContext, useContext, useSyncExternalStore } from "react";

import type { GraphicsApi, GraphicsLaunchMode } from "../../../../preload/api";
import type { StatusStanding } from "../../components/StatusLine";
import {
  type AdapterOutcome,
  type AdapterSummary,
  type GpuCapabilities,
  requestAdapterOutcome,
  type StyleAvailability,
} from "./platform";

/** A fault the console reports about its own graphics, current until it clears. */
export type GraphicsFault =
  | {
      readonly kind: "device-lost";
      readonly reason: GPUDeviceLostReason;
      readonly message: string;
    }
  | { readonly kind: "gpu-process-gone"; readonly count: number }
  /**
   * A shader did not compile on the GPU, and its draws are left out: a bug the console reports
   * (Design note 12). `effectName` is the effect's display name.
   */
  | { readonly kind: "shader-refused"; readonly effectName: string }
  /**
   * A view not re-created after a device loss draws nothing until it is released or the console
   * relaunches (decided 2026-10-02). `viewName` is the name the view was created with.
   */
  | { readonly kind: "view-refused"; readonly viewName: string };

/** The graphics' standing condition. */
export type GraphicsCondition =
  /** The adapter has been asked for and has not answered. */
  | { readonly kind: "acquiring" }
  | {
      readonly kind: "nominal";
      readonly summary: AdapterSummary;
      readonly styles: StyleAvailability;
    }
  /** A software adapter: the wireframe only (R01 Design note 8). */
  | { readonly kind: "software-adapter"; readonly summary: AdapterSummary }
  /** `navigator.gpu` is absent. */
  | { readonly kind: "no-webgpu" }
  /** `requestAdapter` gave null at start. */
  | { readonly kind: "no-adapter" }
  /** The declared safe mode, which has no WebGPU (R01 Design note 5). */
  | { readonly kind: "safe-mode" }
  /** WebGPU given up on for this session (R01 Design note 9). */
  | {
      readonly kind: "disabled";
      readonly cause: "device-losses" | "adapter-withdrawn";
      readonly losses: number;
    };

/**
 * Whether timestamps carry Dawn's 65,536 ns quantization, or the device has no `timestamp-query`
 * at all (R01 Design note 4).
 */
export type GpuTimer = "quantized" | "full" | "absent";

/** How the adapter rounds a colour-attachment write, per float format (R01 Design note 22). */
export type TargetRounding = "nearest" | "toward-zero" | "unknown";

/** The float colour formats whose rounding the adapter's probe classes. */
export type ProbedTargetFormat = "rgba16float" | "rg11b10ufloat";

/** Everything the console knows of its graphics. */
export interface GraphicsStatus {
  readonly condition: GraphicsCondition;
  /** The adapter's until its device reports its own, or `null` before one answers. */
  readonly capabilities: GpuCapabilities | null;
  readonly launchMode: GraphicsLaunchMode;
  /** Whether `--hyperion-gpu-timing` lifted timestamp quantization for this launch. */
  readonly gpuTiming: boolean;
  /** From {@link GraphicsStatus.gpuTiming} and `timestamp-query`: the adapter's, then the device's. */
  readonly timer: GpuTimer;
  /** `unknown` for each format until probed. */
  readonly targetRounding: Readonly<Record<ProbedTargetFormat, TargetRounding>>;
  /** The current fault, cleared on recovery. */
  readonly fault: GraphicsFault | null;
  /** Device losses this session. */
  readonly deviceLosses: number;
  /** GPU-process crashes this launch, as the main process counts them. */
  readonly gpuProcessCrashes: number;
}

/** Something that changes the graphics status. */
export type GraphicsEvent =
  /** The feed's first adapter request answered. */
  | { readonly kind: "adapter-outcome"; readonly outcome: AdapterOutcome }
  /** The feed's adapter request after a GPU-process crash answered (the "re-acquiring"). */
  | { readonly kind: "adapter-reacquired"; readonly outcome: AdapterOutcome }
  | {
      readonly kind: "device-lost";
      readonly reason: GPUDeviceLostReason;
      readonly message: string;
    }
  /** A fresh adapter vetted and its device made after a loss. */
  | {
      readonly kind: "device-restored";
      readonly outcome: AdapterOutcome & { readonly kind: "adapter" };
    }
  /** A rebuild after a loss asked for an adapter and was given none. */
  | { readonly kind: "adapter-withdrawn" }
  /** The main process reported a GPU-process crash. */
  | { readonly kind: "gpu-process-gone"; readonly count: number }
  /** A WGSL module failed `getCompilationInfo` (R01.T8.k); `effectName` is the effect's display name. */
  | { readonly kind: "shader-refused"; readonly effectName: string }
  /** A view's re-creation failed in a restore after a device loss. */
  | { readonly kind: "view-refused"; readonly viewName: string }
  /** A view whose re-creation failed was disposed. */
  | { readonly kind: "view-released"; readonly viewName: string }
  /**
   * The engine made its device and read its capabilities, in which a feature the harness withheld
   * reads as absent (R01 Design note 24). Sent at each creation, a rebuild's included.
   */
  | { readonly kind: "device-capabilities"; readonly capabilities: GpuCapabilities }
  /** The adapter's rounding probe answered (R01.T8.j). */
  | {
      readonly kind: "target-rounding";
      readonly rounding: Readonly<Record<ProbedTargetFormat, TargetRounding>>;
    };

/**
 * Device losses in a session after which the client stops re-creating the device, as the
 * brainstorm's "three such losses disable WebGPU for the session" and Chromium's own count.
 */
export const DEVICE_LOSS_LIMIT = 3;

/** The status of a launch before its adapter answers. */
export function initialGraphicsStatus(
  launchMode: GraphicsLaunchMode,
  gpuTiming: boolean,
): GraphicsStatus {
  return {
    condition: launchMode === "safe" ? { kind: "safe-mode" } : { kind: "acquiring" },
    capabilities: null,
    launchMode,
    gpuTiming,
    timer: "absent",
    targetRounding: { rgba16float: "unknown", rg11b10ufloat: "unknown" },
    fault: null,
    deviceLosses: 0,
    gpuProcessCrashes: 0,
  };
}

function timerOf(capabilities: GpuCapabilities | null, gpuTiming: boolean): GpuTimer {
  if (capabilities === null || !capabilities.timestampQuery) {
    return "absent";
  }
  return gpuTiming ? "full" : "quantized";
}

function conditionOf(outcome: AdapterOutcome): GraphicsCondition {
  let condition: GraphicsCondition;
  switch (outcome.kind) {
    case "no-webgpu":
      condition = { kind: "no-webgpu" };
      break;
    case "no-adapter":
      condition = { kind: "no-adapter" };
      break;
    case "adapter":
      condition = outcome.summary.fallback
        ? { kind: "software-adapter", summary: outcome.summary }
        : { kind: "nominal", summary: outcome.summary, styles: outcome.styles };
      break;
  }
  return condition;
}

function withOutcome(status: GraphicsStatus, outcome: AdapterOutcome): GraphicsStatus {
  const capabilities = outcome.kind === "adapter" ? outcome.capabilities : null;
  return {
    ...status,
    condition: conditionOf(outcome),
    capabilities,
    timer: timerOf(capabilities, status.gpuTiming),
  };
}

/** Whether the condition is final for the launch: nothing but a relaunch leaves it. */
function settled(condition: GraphicsCondition): boolean {
  return condition.kind === "safe-mode" || condition.kind === "disabled";
}

function afterDeviceLoss(
  status: GraphicsStatus,
  event: GraphicsEvent & { readonly kind: "device-lost" },
): GraphicsStatus {
  const deviceLosses = status.deviceLosses + 1;
  if (settled(status.condition)) {
    return { ...status, deviceLosses };
  }
  if (deviceLosses >= DEVICE_LOSS_LIMIT) {
    return {
      ...status,
      deviceLosses,
      fault: null,
      condition: { kind: "disabled", cause: "device-losses", losses: deviceLosses },
    };
  }
  const fault: GraphicsFault = {
    kind: "device-lost",
    reason: event.reason,
    message: event.message,
  };
  return { ...status, deviceLosses, fault };
}

function afterProcessGone(status: GraphicsStatus, count: number): GraphicsStatus {
  const gpuProcessCrashes = Math.max(status.gpuProcessCrashes, count);
  if (settled(status.condition)) {
    return { ...status, gpuProcessCrashes };
  }
  return {
    ...status,
    gpuProcessCrashes,
    fault: { kind: "gpu-process-gone", count: gpuProcessCrashes },
  };
}

/**
 * The status after a refused shader: its fault, unless a fault already stands.
 *
 * @remarks
 * The first refusal stands; a later one, of the same effect or another, does not replace it (each
 * is in the log). A device loss or a crashed GPU process outranks a refusal,
 * since the operator can act on those.
 */
function afterShaderRefused(status: GraphicsStatus, effectName: string): GraphicsStatus {
  if (settled(status.condition) || status.fault !== null) {
    return status;
  }
  return { ...status, fault: { kind: "shader-refused", effectName } };
}

/**
 * The status after a view's re-creation failed: its fault, unless a fault already stands.
 *
 * @remarks
 * As {@link afterShaderRefused}: the first refusal stands, and a device loss or a crashed GPU
 * process outranks it. A restore that follows (`device-restored`) clears it, and one that fails
 * again re-raises it.
 */
function afterViewRefused(status: GraphicsStatus, viewName: string): GraphicsStatus {
  if (settled(status.condition) || status.fault !== null) {
    return status;
  }
  return { ...status, fault: { kind: "view-refused", viewName } };
}

/** The status after a refused view is released: its own fault cleared, any other kept. */
function afterViewReleased(status: GraphicsStatus, viewName: string): GraphicsStatus {
  return status.fault?.kind === "view-refused" && status.fault.viewName === viewName
    ? { ...status, fault: null }
    : status;
}

function afterAdapterOutcome(status: GraphicsStatus, outcome: AdapterOutcome): GraphicsStatus {
  const next = withOutcome(status, outcome);
  return outcome.kind === "adapter" ? withoutProcessFault(next) : next;
}

/** Whether the condition is one an adapter answered with. */
function hasAdapter(condition: GraphicsCondition): boolean {
  return condition.kind === "nominal" || condition.kind === "software-adapter";
}

/** The status with a crashed GPU process's fault cleared, and any other fault kept. */
function withoutProcessFault(status: GraphicsStatus): GraphicsStatus {
  return status.fault?.kind === "gpu-process-gone" ? { ...status, fault: null } : status;
}

/**
 * The status after the request that follows a GPU-process crash answers.
 *
 * @remarks
 * A granted adapter clears the crash's fault. Where an adapter had been granted already, the
 * condition, capabilities and timer are left as they are: a view's rebuild after the same crash
 * reports the device's own (`device-restored`, `device-capabilities`), which hold (Design note 24),
 * and the two answers are not ordered. Where none had been, the new adapter's outcome is taken.
 * No adapter where one had been granted is the adapter withdrawn, as a rebuild handles it (Design
 * note 9). No adapter where none had been restates the condition and clears the fault: nothing is
 * being re-acquired any more, the condition's own line carries the remedy, and `Process Restarts`
 * keeps the count.
 */
function afterReacquired(status: GraphicsStatus, outcome: AdapterOutcome): GraphicsStatus {
  if (outcome.kind === "adapter") {
    return withoutProcessFault(
      hasAdapter(status.condition) ? status : withOutcome(status, outcome),
    );
  }
  if (hasAdapter(status.condition)) {
    return reduceGraphicsStatus(status, { kind: "adapter-withdrawn" });
  }
  return withoutProcessFault(withOutcome(status, outcome));
}

/**
 * The status after `event`.
 *
 * @remarks
 * The safe mode holds whatever the adapter, and `disabled` holds whatever follows: both end only
 * with a relaunch. A device loss is a fault and counts; the {@link DEVICE_LOSS_LIMIT}th disables
 * WebGPU for the session. A restore clears the fault and keeps the count. An adapter withdrawn on a
 * rebuild disables WebGPU whatever the count (R01 Design note 9). A granted adapter clears a
 * crashed GPU process's fault and keeps its count: it is the evidence that the graphics work again
 * (decided 2026-09-30).
 */
export function reduceGraphicsStatus(status: GraphicsStatus, event: GraphicsEvent): GraphicsStatus {
  let next: GraphicsStatus;
  switch (event.kind) {
    case "adapter-outcome":
      next = settled(status.condition) ? status : afterAdapterOutcome(status, event.outcome);
      break;
    case "adapter-reacquired":
      next = settled(status.condition) ? status : afterReacquired(status, event.outcome);
      break;
    case "device-lost":
      next = afterDeviceLoss(status, event);
      break;
    case "device-restored":
      next = settled(status.condition)
        ? status
        : { ...withOutcome(status, event.outcome), fault: null };
      break;
    case "adapter-withdrawn":
      next = settled(status.condition)
        ? status
        : {
            ...status,
            fault: null,
            condition: {
              kind: "disabled",
              cause: "adapter-withdrawn",
              losses: status.deviceLosses,
            },
          };
      break;
    case "gpu-process-gone":
      next = afterProcessGone(status, event.count);
      break;
    case "device-capabilities":
      next = settled(status.condition)
        ? status
        : {
            ...status,
            capabilities: event.capabilities,
            timer: timerOf(event.capabilities, status.gpuTiming),
          };
      break;
    case "shader-refused":
      next = afterShaderRefused(status, event.effectName);
      break;
    case "view-refused":
      next = afterViewRefused(status, event.viewName);
      break;
    case "view-released":
      next = afterViewReleased(status, event.viewName);
      break;
    case "target-rounding":
      next = { ...status, targetRounding: event.rounding };
      break;
  }
  return next;
}

/** One of the graphics annunciations, with the `StatusLine` standing it takes. */
export interface GraphicsAnnunciation {
  readonly text: string;
  readonly standing: Extract<StatusStanding, "refused" | "fault">;
}

/**
 * The graphics annunciations' words (R01 Design note 10, R01.T5.c, signed off 2026-09-30).
 *
 * @remarks
 * Upper case is the guide's nomenclature; the mixed-case clause after the colon is a sentence
 * giving the cause, the operator's remedy or what the console is doing about it, as
 * `MAP DATA INVALID: <cause>` has it. "Not offered" is always `not available`, never
 * `unavailable`.
 */
export const GRAPHICS_WORDS = {
  acquiring: "GRAPHICS ACQUIRING ADAPTER",
  softwareAdapter: "GRAPHICS SOFTWARE ADAPTER: photorealistic style not available",
  noWebGpu: "GRAPHICS NOT AVAILABLE: no WebGPU",
  noAdapter: "GRAPHICS NO ADAPTER: views not available, relaunch to retry",
  deviceLost: "GRAPHICS DEVICE LOST: re-creating",
  processRestarted: "GRAPHICS PROCESS RESTARTED: re-acquiring",
  shaderRefused: (effectName: string): string =>
    `GRAPHICS SHADER REFUSED: ${effectName} did not compile, not drawn`,
  viewRefused:
    "GRAPHICS VIEW REFUSED: not re-created after device loss, not drawn, relaunch to retry",
  safeMode: "GRAPHICS SAFE MODE: views not available, relaunch to retry",
  disabledByLosses: (losses: number): string =>
    `GRAPHICS DISABLED: ${losses} device losses, relaunch to retry`,
  disabledWithdrawn: "GRAPHICS DISABLED: adapter withdrawn, relaunch to retry",
} as const;

function faultAnnunciation(fault: GraphicsFault): GraphicsAnnunciation {
  let text: string;
  switch (fault.kind) {
    case "device-lost":
      text = GRAPHICS_WORDS.deviceLost;
      break;
    case "gpu-process-gone":
      text = GRAPHICS_WORDS.processRestarted;
      break;
    case "shader-refused":
      text = GRAPHICS_WORDS.shaderRefused(fault.effectName);
      break;
    case "view-refused":
      text = GRAPHICS_WORDS.viewRefused;
      break;
  }
  return { text, standing: "fault" };
}

/**
 * The statement of a condition that ends only with a relaunch: the safe mode or the disabled
 * state, or `null` for any other.
 *
 * @remarks
 * These are the two the header strip's banner shows (R01.T5.b).
 */
export function graphicsModeAnnunciation(status: GraphicsStatus): GraphicsAnnunciation | null {
  const { condition } = status;
  if (condition.kind === "safe-mode") {
    return { text: GRAPHICS_WORDS.safeMode, standing: "refused" };
  }
  if (condition.kind === "disabled") {
    const text =
      condition.cause === "device-losses"
        ? GRAPHICS_WORDS.disabledByLosses(condition.losses)
        : GRAPHICS_WORDS.disabledWithdrawn;
    return { text, standing: "refused" };
  }
  return null;
}

function conditionAnnunciation(condition: GraphicsCondition): GraphicsAnnunciation | null {
  let text: string | null;
  switch (condition.kind) {
    case "acquiring":
      text = GRAPHICS_WORDS.acquiring;
      break;
    case "software-adapter":
      text = GRAPHICS_WORDS.softwareAdapter;
      break;
    case "no-webgpu":
      text = GRAPHICS_WORDS.noWebGpu;
      break;
    case "no-adapter":
      text = GRAPHICS_WORDS.noAdapter;
      break;
    case "nominal":
    case "safe-mode":
    case "disabled":
      text = null;
      break;
  }
  return text === null ? null : { text, standing: "refused" };
}

/**
 * The current graphics annunciation, or `null` when there is nothing to say.
 *
 * @remarks
 * The safe mode and the disabled state come first, since they end only with a relaunch; then a
 * current fault; then the adapter's own condition.
 */
export function graphicsAnnunciation(status: GraphicsStatus): GraphicsAnnunciation | null {
  const mode = graphicsModeAnnunciation(status);
  if (mode !== null) {
    return mode;
  }
  if (status.fault !== null) {
    return faultAnnunciation(status.fault);
  }
  return conditionAnnunciation(status.condition);
}

/** Holds the graphics status and tells subscribers when it changes. */
export class GraphicsStatusStore {
  #status: GraphicsStatus;
  readonly #listeners = new Set<() => void>();

  constructor(initial: GraphicsStatus) {
    this.#status = initial;
  }

  /**
   * Registers `listener`, called after every change.
   *
   * @returns Its removal.
   */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** Subscribers registered now. */
  get listenerCount(): number {
    return this.#listeners.size;
  }

  /** The current status; the same object until it changes. */
  readonly getSnapshot = (): GraphicsStatus => this.#status;

  /** Applies `event` and tells the subscribers if the status changed. */
  dispatch(event: GraphicsEvent): void {
    const next = reduceGraphicsStatus(this.#status, event);
    if (next === this.#status) {
      return;
    }
    this.#status = next;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}

/** Whether `value` is a WebGPU entry point, by its `requestAdapter` method. */
function isGpu(value: unknown): value is GPU {
  return (
    typeof value === "object" &&
    value !== null &&
    "requestAdapter" in value &&
    typeof value.requestAdapter === "function"
  );
}

/**
 * `navigator.gpu`, or `undefined` where WebGPU is absent.
 *
 * @remarks
 * Chromium leaves it undefined where WebGPU is unavailable, although `lib.dom` types it as always
 * present, so it is read as `unknown` and narrowed.
 */
export function navigatorGpu(): GPU | undefined {
  const value: unknown = Reflect.get(navigator, "gpu");
  return isGpu(value) ? value : undefined;
}

/**
 * Feeds the store from the preload and the feed's own adapter requests: one at start, and one
 * after each GPU-process crash.
 *
 * @param graphics - `window.hyperion.graphics`.
 * @param gpu - `navigator.gpu`, or `undefined` where WebGPU is absent.
 * @returns The feed's end: it stops listening for crash reports and drops a pending answer.
 * @remarks
 * The safe mode asks for no adapter: it has no WebGPU (R01 Design note 5). The request after a
 * crash is the "re-acquiring" its fault states, answered as `adapter-reacquired`; only the latest
 * request's answer is dispatched, so an answer from before a crash cannot clear its fault.
 */
export function feedGraphicsStatus(
  store: GraphicsStatusStore,
  graphics: GraphicsApi,
  gpu: GPU | undefined,
): () => void {
  let ended = false;
  /** The latest request's number: an answer to an earlier one is dropped. */
  let latest = 0;
  const request = (afterCrash: boolean): void => {
    latest += 1;
    const id = latest;
    const answer = (outcome: AdapterOutcome): void => {
      if (ended || id !== latest) {
        return;
      }
      store.dispatch(
        afterCrash ? { kind: "adapter-reacquired", outcome } : { kind: "adapter-outcome", outcome },
      );
    };
    void requestAdapterOutcome(gpu)
      .then((outcome): void => {
        answer(outcome);
        return undefined;
      })
      .catch((error: unknown) => {
        console.error("the adapter request failed:", error);
        answer({ kind: "no-adapter" });
      });
  };
  const safe = graphics.launchMode === "safe";
  const unsubscribe = graphics.onGpuProcessGone(({ count }) => {
    store.dispatch({ kind: "gpu-process-gone", count });
    if (!safe) {
      request(true);
    }
  });
  if (!safe) {
    request(false);
  }
  return () => {
    ended = true;
    unsubscribe();
  };
}

/**
 * Hands the graphics status store to every display.
 *
 * @remarks
 * `null` outside a provider, which {@link useGraphicsStatus} reports.
 */
export const GraphicsStatusContext = createContext<GraphicsStatusStore | null>(null);

/**
 * The current graphics status, re-rendering the caller on every change.
 *
 * @throws Error when no {@link GraphicsStatusContext} provider is above the caller, a wiring bug.
 */
export function useGraphicsStatus(): GraphicsStatus {
  const store = useContext(GraphicsStatusContext);
  if (store === null) {
    throw new Error("useGraphicsStatus needs a GraphicsStatusContext provider");
  }
  return useSyncExternalStore(store.subscribe, store.getSnapshot);
}
