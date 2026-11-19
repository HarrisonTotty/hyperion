/**
 * A still image's job (plan R11, R11.T10.b, Design note 17): the still rendered progressively
 * beside the live view, one batch of whole passes an animation frame, read back, encoded and saved;
 * and the session's stills, suspended by a device loss in flight and disabled by a second.
 *
 * @remarks
 * The job runs against two interfaces, so that it is tested against fakes and the photorealistic
 * renderer supplies the real ones: the {@link StillDevice}, R01's queue and clock (its
 * `onSubmittedWorkDone()`, an empty submit, the animation frame, `onPassTimes` and its faults), and
 * the {@link StillRenderer}, the scene frozen at the request and drawn tile by tile into R01's
 * `createRenderTarget`. That draw is the hook in `photoreal/renderer.ts` (`PhotorealStills`), `VIEW`'s
 * {@link StillSource}.
 *
 * In order: every pipeline the still needs is made asynchronously, before the first batch
 * ({@link StillRenderer.prepare}), since a synchronous compile of the high setting's pipelines on
 * the GPU process's main thread is the realistic way to trip Chromium's 15 s watchdog; the job
 * waits for the frustum's terrain to stream, at most {@link STILL_STREAMING_TIMEOUT_S}, and labels
 * the still `TERRAIN: STREAMING` where it had not; without timestamps it measures the empty-submit
 * round trip five times; then each animation frame it submits the batch R11.T10.a's
 * {@link BatchController} proposes, and only once the last one's `onSubmittedWorkDone()` has
 * resolved, so that one batch is ever in flight. Last, each tile is read back and the tiles are
 * assembled, cropped to the still, encoded as a PNG and saved with the label.
 *
 * A device or GPU-process loss while the job's GPU work is in flight abandons it at once. The
 * session ({@link StillSession}) then suspends stills (`STILLS LIMITED`): only an explicit request
 * makes the next, starting from one unit, its growth capped at half the batch that was in flight; a
 * second loss in flight disables them for the session, since three losses disable WebGPU.
 *
 * Nothing here runs while no still is asked for: a still costs nothing until its request.
 */

import type {
  SavedStill,
  StillLabel,
  StillRefusal,
  StillSaveAnswer,
} from "../../../../preload/api";
import type { GpuTimer } from "../engine/status";
import type { ViewSize } from "../engine/types";
import type { QualitySetting } from "../quality/qualitySetting";
import {
  BatchController,
  type BatchMeasurement,
  EMPTY_SUBMIT_SAMPLES,
  type MeasuredPass,
} from "./batchController";
import {
  STILL_CONTROL_IDLE,
  type StillControlState,
  type StillEnd,
  type StillSizeChoice,
  type StillStanding,
  type StillStep,
} from "./StillControl";
import { stillSizeOptions } from "./stillSize";
import { type StillRequest, stillLabel } from "./stillLabel";
import type { StillTile, StillTiling } from "./stillPlan";

/**
 * How long a still waits for its frustum's terrain to stream to the level its pixels warrant, s:
 * 20 (Design note 17), to be measured in R11.T10.d's soak. Past it the still is drawn as it stands
 * and labelled `TERRAIN: STREAMING`.
 */
export const STILL_STREAMING_TIMEOUT_S = 20;

/** The shortest time between two progress readouts, ms: the guide's 4 Hz for live readouts. */
export const STILL_PROGRESS_INTERVAL_MS = 250;

/**
 * What the job needs of the device: R01's queue, clock and faults, behind an interface so that a
 * fake stands in for them.
 */
export interface StillDevice {
  /** From `GraphicsStatus.timer`, for the whole job. */
  readonly timer: GpuTimer;
  /** Resolves once all the work submitted so far is done: the queue's `onSubmittedWorkDone()`. */
  submittedWorkDone(): Promise<void>;
  /** Submits an empty command buffer, whose round trip is the baseline without timestamps. */
  submitEmpty(): void;
  /** Resolves at the next animation frame. */
  nextFrame(): Promise<void>;
  /** A monotonic clock, ns. */
  nowNs(): number;
  /**
   * Hands each frame's pass times to `listener` as they resolve (R01's `onPassTimes`), with
   * timestamps; never without.
   *
   * @returns The listener's removal.
   */
  onPassTimes(listener: (passes: ReadonlyArray<MeasuredPass>) => void): () => void;
  /**
   * Calls `listener` at a device loss or a GPU-process crash (R01's `onFault`'s `device-lost` and
   * `gpu-process-gone`).
   *
   * @returns The listener's removal.
   */
  onLoss(listener: () => void): () => void;
}

/**
 * What the job needs of the renderer: the scene frozen at the request, drawn tile by tile, behind an
 * interface so that a fake stands in for it.
 */
export interface StillRenderer {
  /** The still's tiles (R11.T10.a's `planTiles`). */
  readonly tiling: StillTiling;
  /**
   * The job's units in order, each one whole pass of one tile, by the label its cost is learnt
   * under (R11.T10.a's `BatchControllerOptions.units`).
   */
  readonly units: ReadonlyArray<string>;
  /**
   * Makes every pipeline the still needs, asynchronously (`createMaterialAsync`,
   * `createComputeAsync`), resolving once all are made: the job submits nothing before.
   */
  prepare(): Promise<void>;
  /** Whether the frustum's terrain has streamed to the level the still's pixels warrant. */
  streamed(): boolean;
  /** Encodes and submits the units [first, first + count) as one submission. */
  submit(first: number, count: number): void;
  /**
   * `tile`'s 8-bit sRGB texels, tone-mapped, once every unit is done (R01's `readTexture`): its
   * texture's rows, `tile.bytesPerRow` bytes each.
   */
  readTile(tile: StillTile): Promise<Uint8Array>;
  /** Releases the targets and pipelines it made. */
  dispose(): void;
}

/** Makes a PNG of `rgba`, the still's pixels row by row from the top, 4 bytes each. */
export type StillEncoder = (
  rgba: Uint8ClampedArray<ArrayBuffer>,
  size: ViewSize,
) => Promise<Uint8Array>;

/**
 * Encodes a still through an `OffscreenCanvas`'s `convertToBlob` (Design note 17): an 8-bit sRGB
 * PNG of the pixels as given.
 *
 * @throws Error where the canvas gives no 2D context.
 */
export async function encodeStillPng(
  rgba: Uint8ClampedArray<ArrayBuffer>,
  size: ViewSize,
): Promise<Uint8Array> {
  const canvas = new OffscreenCanvas(size.widthPx, size.heightPx);
  const context = canvas.getContext("2d", { colorSpace: "srgb" });
  if (context === null) {
    throw new Error("an OffscreenCanvas gave no 2D context");
  }
  context.putImageData(new ImageData(rgba, size.widthPx, size.heightPx), 0, 0);
  return new Uint8Array(await (await canvas.convertToBlob({ type: "image/png" })).arrayBuffer());
}

/** Saves a still's PNG and its label (`StillStore.save`). */
export type StillSaver = (png: Uint8Array, label: StillLabel) => Promise<StillSaveAnswer>;

/** What a still's job is asked to make. */
export interface StillJobOptions {
  readonly device: StillDevice;
  readonly renderer: StillRenderer;
  /** What the view held at the request, from which the label is made. */
  readonly request: StillRequest;
  readonly setting: QualitySetting;
  /** T, the display's measured vsync period, ns (R11.T10.a's `batchCapNs`). */
  readonly frameIntervalNs: number;
  /** The most units a batch may hold, after a suspension; none otherwise. */
  readonly maxUnits?: number | undefined;
  readonly encode: StillEncoder;
  readonly save: StillSaver;
  /** Each step, as the job takes it. */
  readonly onStep: (step: StillStep) => void;
  /** The streaming wait's bound, s: {@link STILL_STREAMING_TIMEOUT_S} unless given. */
  readonly streamingTimeoutS?: number | undefined;
}

/** How a still's job ended. */
export type StillJobOutcome =
  | { readonly kind: "saved"; readonly still: SavedStill; readonly streamed: boolean }
  /** Not saved: why, and what to do, as a sentence. */
  | { readonly kind: "not-saved"; readonly reason: string }
  /**
   * Abandoned at a device or GPU-process loss in flight, with the units of the batch then in
   * flight, or of the last one submitted, 1 before any.
   */
  | { readonly kind: "lost"; readonly batchUnits: number };

/** Thrown inside the job at a loss, so that every wait in flight ends with it. */
class StillLost extends Error {
  override readonly name = "StillLost";
}

/**
 * Each refusal's words, as `STILL NOT SAVED:` states it: a refusal of the record or the image is
 * the console's own bug, with nothing for the operator to do; one still at a time is not.
 */
const REFUSAL_WORDS: Readonly<Record<StillRefusal, string>> = {
  oversized: "the image is over the size the console saves",
  "malformed-label": "the console refused its record",
  busy: "another still is being saved, TAKE STILL to retry",
  "not-png": "the console refused its image",
  "size-mismatch": "the console refused its image",
  "label-mismatch": "the console refused its record",
};

/** A failed write's words, by the system's code: what went wrong, and what to do. */
function writeFailureWords(code: string): string {
  switch (code) {
    case "ENOSPC":
    case "EDQUOT":
      return "the disk is full, free space and TAKE STILL to retry";
    case "EACCES":
    case "EPERM":
    case "EROFS":
      return "the stills folder cannot be written";
    default:
      return "the still could not be written, TAKE STILL to retry";
  }
}

/** A save's answer as the job's outcome, `streamed` whether its terrain had streamed. */
function outcomeOf(answer: StillSaveAnswer, streamed: boolean): StillJobOutcome {
  let outcome: StillJobOutcome;
  switch (answer.kind) {
    case "saved":
      outcome = { kind: "saved", still: answer.still, streamed };
      break;
    case "refused":
      outcome = { kind: "not-saved", reason: REFUSAL_WORDS[answer.reason] };
      break;
    case "failed":
      outcome = { kind: "not-saved", reason: writeFailureWords(answer.reason) };
      break;
  }
  return outcome;
}

/**
 * The tiles' texels assembled into the still's pixels: each tile's rect taken from its texture's
 * rows, the columns past the still's right edge cropped (R11.T10.a's padded last column), every
 * pixel opaque.
 *
 * @remarks
 * The alpha is set to 255: R07 writes `METER_CLASS` in the HDR target's alpha, which no picture's
 * coverage is, and a still is the image alone (Design note 17), so its PNG is opaque whatever a
 * tile's read-back carries there.
 *
 * @throws RangeError where a tile's bytes are not its texture's rows.
 */
export function assembleTiles(
  tiling: StillTiling,
  texels: ReadonlyArray<Uint8Array>,
): Uint8ClampedArray<ArrayBuffer> {
  const { widthPx, heightPx } = tiling.size;
  const out = new Uint8ClampedArray(widthPx * heightPx * 4);
  for (const [index, tile] of tiling.tiles.entries()) {
    const bytes = texels[index];
    if (bytes?.byteLength !== tile.bytesPerRow * tile.texture.heightPx) {
      throw new RangeError(`tile ${String(index)}'s texels are not its texture's rows`);
    }
    const rowBytes = (tile.rect.x1Px - tile.rect.x0Px) * 4;
    for (let v = 0; v < tile.rect.y1Px - tile.rect.y0Px; v += 1) {
      const from = v * tile.bytesPerRow;
      out.set(
        bytes.subarray(from, from + rowBytes),
        ((tile.rect.y0Px + v) * widthPx + tile.rect.x0Px) * 4,
      );
    }
  }
  for (let alpha = 3; alpha < out.length; alpha += 4) {
    out[alpha] = 255;
  }
  return out;
}

/**
 * Runs one still's job (Design note 17): prepares, waits for the terrain, measures the baseline
 * without timestamps, submits its batches one in flight, then reads back, encodes and saves.
 *
 * @remarks
 * The renderer is disposed whatever the outcome. A loss in flight ends every wait at once and the
 * job answers `lost`; once the tiles are read back, a loss no longer reaches the still.
 *
 * @throws Error, as a rejection, where the renderer, the encoder or the save fails for a reason of
 *   its own: the caller's to report.
 */
export async function runStillJob(options: StillJobOptions): Promise<StillJobOutcome> {
  const { device, renderer } = options;
  const total = renderer.units.length;
  // Aborted at a loss in flight: its signal is what every wait looks for.
  const loss = new AbortController();
  const lostSignal = new Promise<never>((_resolve, reject) => {
    loss.signal.addEventListener(
      "abort",
      () => {
        reject(new StillLost("the device was lost while a still was in flight"));
      },
      { once: true },
    );
  });
  // Never left unhandled: every wait races it, and a job that ends first never sees it.
  lostSignal.catch(() => undefined);
  const removeLoss = device.onLoss(() => {
    loss.abort();
  });
  /**
   * `work`, unless a loss comes first or came while it was in flight: a race settles with the
   * first of its promises already settled, so a loss is also looked for once it has.
   */
  const inFlight = async <T>(work: Promise<T>): Promise<T> => {
    const result = await Promise.race([work, lostSignal]);
    if (loss.signal.aborted) {
      throw new StillLost("the device was lost while a still was in flight");
    }
    return result;
  };
  const passes: MeasuredPass[] = [];
  const removePasses =
    device.timer === "absent"
      ? () => undefined
      : device.onPassTimes((resolved) => {
          passes.push(...resolved);
        });
  let batchUnits = 1;
  try {
    options.onStep({ kind: "preparing", donePasses: 0, totalPasses: total });
    await inFlight(renderer.prepare());

    options.onStep({ kind: "streaming", donePasses: 0, totalPasses: total });
    const timeoutNs = (options.streamingTimeoutS ?? STILL_STREAMING_TIMEOUT_S) * 1e9;
    const waitStartNs = device.nowNs();
    while (!renderer.streamed() && device.nowNs() - waitStartNs < timeoutNs) {
      // One look an animation frame, by design.
      // oxlint-disable-next-line no-await-in-loop
      await inFlight(device.nextFrame());
    }
    const streamed = renderer.streamed();

    const emptyRoundTripsNs: number[] = [];
    if (device.timer === "absent") {
      for (let k = 0; k < EMPTY_SUBMIT_SAMPLES; k += 1) {
        const startNs = device.nowNs();
        device.submitEmpty();
        // Each round trip alone, by design: two at once would measure each other.
        // oxlint-disable-next-line no-await-in-loop
        await inFlight(device.submittedWorkDone());
        emptyRoundTripsNs.push(device.nowNs() - startNs);
      }
    }
    const controller = new BatchController({
      setting: options.setting,
      frameIntervalNs: options.frameIntervalNs,
      timing:
        device.timer === "absent"
          ? { timer: "absent", emptyRoundTripsNs }
          : { timer: device.timer },
      units: renderer.units,
      ...(options.maxUnits === undefined ? {} : { maxUnits: options.maxUnits }),
    });

    let done = 0;
    let measured: BatchMeasurement = { kind: "none" };
    options.onStep({ kind: "rendering", donePasses: 0, totalPasses: total });
    for (;;) {
      // One batch an animation frame, by design.
      // oxlint-disable-next-line no-await-in-loop
      await inFlight(device.nextFrame());
      const batch = controller.nextBatch(measured);
      if (batch === null) {
        break;
      }
      batchUnits = batch.count;
      const startNs = device.nowNs();
      renderer.submit(batch.first, batch.count);
      // The next batch waits for this one's work to be done: one batch in flight, by design.
      // oxlint-disable-next-line no-await-in-loop
      await inFlight(device.submittedWorkDone());
      const roundTripNs = device.nowNs() - startNs;
      done += batch.count;
      options.onStep({ kind: "rendering", donePasses: done, totalPasses: total });
      measured =
        device.timer === "absent"
          ? { kind: "round-trip", roundTripNs }
          : { kind: "pass-times", passes: passes.splice(0) };
    }

    options.onStep({ kind: "saving", donePasses: total, totalPasses: total });
    const texels = await inFlight(
      Promise.all(renderer.tiling.tiles.map((tile) => renderer.readTile(tile))),
    );
    removeLoss();
    const pixels = assembleTiles(renderer.tiling, texels);
    const png = await options.encode(pixels, renderer.tiling.size);
    const label = stillLabel(options.request, streamed);
    return outcomeOf(await options.save(png, label), streamed);
  } catch (error: unknown) {
    if (!loss.signal.aborted && !(error instanceof StillLost)) {
      // A lost device fails a read-back or a compile before its loss is reported, so the job
      // waits an animation frame for the report before it takes the failure as the job's own.
      await Promise.race([device.nextFrame(), lostSignal]).catch(() => undefined);
    }
    if (loss.signal.aborted || error instanceof StillLost) {
      return { kind: "lost", batchUnits };
    }
    throw error;
  } finally {
    removeLoss();
    removePasses();
    renderer.dispose();
  }
}

/** A view's scene frozen at a still's request. */
export interface StillScene {
  readonly request: StillRequest;
}

/** What a still is asked of its view at the request (plan R11, R11.T10.e.1). */
export interface StillAsk {
  /** The still's size as a multiple of the view's output size, a whole number from 1. */
  readonly scale: number;
}

/**
 * What a view gives at a still's request ({@link StillSource.snapshot}): its scene, frozen, or why
 * it gives none now, its cause in the words `NOT AVAILABLE:` states it after its colon (`the view
 * draws no photorealistic image`).
 */
export type StillSnapshot =
  | { readonly kind: "scene"; readonly scene: StillScene }
  | { readonly kind: "unavailable"; readonly cause: string };

/**
 * Where a still's scene comes from: the view, frozen at the request, and its device; supplied by
 * the photorealistic renderer's hook (`photoreal/renderer.ts`' `PhotorealStills`).
 */
export interface StillSource {
  readonly device: StillDevice;
  readonly setting: QualitySetting;
  /** T, the display's measured vsync period, ns. */
  frameIntervalNs(): number;
  /**
   * What the view holds now, frozen: the label's inputs and whatever the renderer needs, the cells
   * surveyed now among them, so that cells surveyed while the job runs are not used; or why it
   * gives no still now.
   *
   * @param ask - The still's size (R11.T10.e.1); absent, the view's output size.
   */
  snapshot(ask?: StillAsk): StillSnapshot;
  /**
   * The view's output size at scale 1, device px, which a still's size is a multiple of (plan R11,
   * R11.T10.e.1), or `null` while the view gives no still; absent, a source that offers one size,
   * its scene's.
   */
  outputSize?(): ViewSize | null;
  /**
   * The longest side a still of the view may be drawn at now, px: the tile limit where its frame
   * draws air, which a still draws untiled (plan R11, R11.T10.e; main's ruling, 2026-11-19), or
   * `null` where it is tiled; absent, none. {@link StillSource.snapshot} holds the still's size
   * within it, so that the size offered is the size drawn and the label states.
   */
  sizeLimitPx?(): number | null;
  /**
   * Why the view gives no still now, a {@link StillSnapshot}'s cause, or `null` where it gives
   * one: what `TAKE STILL` is held back for, read as the session is made and at each
   * {@link StillSession.refresh}. Absent, the view always gives one.
   */
  unavailable?(): string | null;
  /** A renderer of `scene`, which the job disposes. */
  renderer(scene: StillScene): StillRenderer;
}

/**
 * The size choice over `source`'s offers at the operator's multiple `scale`: the largest offered at
 * most it, the output size where none is, each held within the source's size limit where it has
 * one; `null` with no output size.
 */
function sizeChoiceOf(source: StillSource, scale: number): StillSizeChoice | null {
  const output = source.outputSize?.() ?? null;
  if (output === null) {
    return null;
  }
  const options = stillSizeOptions(output, source.sizeLimitPx?.() ?? null);
  const offered = options.findLast((option) => option.scale <= scale);
  return { scale: offered?.scale ?? 1, options };
}

/** Whether two size choices offer the same sizes with the same one chosen. */
function sameSizeChoice(a: StillSizeChoice | null, b: StillSizeChoice | null): boolean {
  if (a === null || b === null) {
    return a === b;
  }
  return (
    a.scale === b.scale &&
    a.options.length === b.options.length &&
    a.options.every((option, n) => {
      const other = b.options[n];
      return (
        other !== undefined &&
        option.scale === other.scale &&
        option.size.widthPx === other.size.widthPx &&
        option.size.heightPx === other.size.heightPx
      );
    })
  );
}

/** `TAKE STILL`'s held-back reason for a view's cause, or `null` where it gives a still. */
function heldBackOf(cause: string | null | undefined): string | null {
  return cause === null || cause === undefined ? null : `NOT AVAILABLE: ${cause}`;
}

/** What a {@link StillSession} needs. */
export interface StillSessionOptions {
  readonly source: StillSource;
  readonly encode: StillEncoder;
  readonly save: StillSaver;
  /** A clock, ms, for the progress readouts' 4 Hz. */
  readonly nowMs: () => number;
  /** Reports a job that failed for a reason not its own, a bug. */
  readonly reportError: (error: unknown) => void;
}

/**
 * A client's stills for the session (Design note 17): at most one job at a time, its progress for
 * the control at about 4 Hz, and the loss rule.
 *
 * @remarks
 * An external store for `useSyncExternalStore`, whose snapshot is the control's
 * {@link StillControlState}. A loss in flight while stills are `available` suspends them; the next
 * still, asked for explicitly, takes at most half the batch that was in flight; a loss in flight
 * while `suspended` disables them. A loss while no still runs changes nothing. While the view gives
 * no still ({@link StillSource.unavailable}, read at {@link StillSession.refresh}, or a refused
 * request), `TAKE STILL` is held back with the cause.
 */
export class StillSession {
  readonly #options: StillSessionOptions;
  readonly #listeners = new Set<() => void>();
  #state: StillControlState = STILL_CONTROL_IDLE;
  /** The batch cap the next job takes after a suspension, or `undefined`. */
  #maxUnits: number | undefined = undefined;
  #lastStepMs = Number.NEGATIVE_INFINITY;
  /** The operator's multiple of the view's output size for the next still (R11.T10.e.1): 1 at first. */
  #scale = 1;

  constructor(options: StillSessionOptions) {
    this.#options = options;
    this.#state = {
      ...STILL_CONTROL_IDLE,
      heldBack: heldBackOf(options.source.unavailable?.()),
      size: sizeChoiceOf(options.source, this.#scale),
    };
  }

  /** Subscribes to the control's state; returns the unsubscription. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  /** The control's state as it stands. */
  readonly getSnapshot = (): StillControlState => this.#state;

  /**
   * Holds `TAKE STILL` back while the view gives no still, for the cause its source states now
   * ({@link StillSource.unavailable}), or offers it again where it gives one; a still in progress
   * runs on. The view calls it as it starts and stops drawing and at its readouts' rate.
   */
  refresh(): void {
    this.#hold(heldBackOf(this.#options.source.unavailable?.()));
    this.#offerSizes();
  }

  /**
   * Takes the next still at `scale` times the view's output size (R11.T10.e.1), or at the largest
   * size offered below it; a still in progress keeps its own.
   */
  setScale(scale: number): void {
    if (Number.isSafeInteger(scale) && scale >= 1) {
      this.#scale = scale;
      this.#offerSizes();
    }
  }

  /** Offers the sizes of the view's output size as it stands, the operator's multiple chosen. */
  #offerSizes(): void {
    const size = sizeChoiceOf(this.#options.source, this.#scale);
    if (!sameSizeChoice(size, this.#state.size)) {
      this.#set({ ...this.#state, size });
    }
  }

  #hold(heldBack: string | null): void {
    if (heldBack !== this.#state.heldBack) {
      this.#set({ ...this.#state, heldBack });
    }
  }

  /**
   * Asks for a still of the view as it stands now: refused, doing nothing, while one runs, once
   * stills are disabled, or while `TAKE STILL` is held back; and where the view gives no still at
   * the request, which then holds it back.
   *
   * @returns The job's end, or `null` where none was started.
   */
  async request(): Promise<StillJobOutcome | null> {
    if (
      this.#state.activity.kind === "running" ||
      this.#state.standing === "disabled" ||
      this.#state.heldBack !== null
    ) {
      return null;
    }
    const { source } = this.#options;
    // The sizes as the view offers them now, so that the size asked for is one drawn.
    this.#offerSizes();
    let outcome: StillJobOutcome;
    try {
      // Frozen now: what the view holds after this is not the still's.
      const snapshot = source.snapshot({ scale: this.#state.size?.scale ?? 1 });
      if (snapshot.kind === "unavailable") {
        this.#hold(heldBackOf(snapshot.cause));
        return null;
      }
      const { scene } = snapshot;
      const renderer = source.renderer(scene);
      this.#step({ kind: "preparing", donePasses: 0, totalPasses: renderer.units.length });
      outcome = await runStillJob({
        device: source.device,
        renderer,
        request: scene.request,
        setting: source.setting,
        frameIntervalNs: source.frameIntervalNs(),
        maxUnits: this.#maxUnits,
        encode: this.#options.encode,
        save: this.#options.save,
        onStep: (step) => {
          this.#step(step);
        },
      });
    } catch (error: unknown) {
      this.#options.reportError(error);
      outcome = { kind: "not-saved", reason: "the still could not be made, TAKE STILL to retry" };
    }
    this.#end(outcome);
    return outcome;
  }

  /**
   * Publishes a step: a new step at once, more progress of the same at most at 4 Hz and its last at
   * once, and nothing for the step already shown.
   */
  #step(step: StillStep): void {
    const nowMs = this.#options.nowMs();
    const { activity } = this.#state;
    const same = activity.kind === "running" && activity.step.kind === step.kind;
    if (
      same &&
      (activity.step.donePasses === step.donePasses ||
        (step.donePasses < step.totalPasses &&
          nowMs - this.#lastStepMs < STILL_PROGRESS_INTERVAL_MS))
    ) {
      return;
    }
    this.#lastStepMs = nowMs;
    this.#show(step);
  }

  #show(step: StillStep): void {
    this.#set({ ...this.#state, activity: { kind: "running", step } });
  }

  #end(outcome: StillJobOutcome): void {
    let standing: StillStanding = this.#state.standing;
    let last: StillEnd;
    switch (outcome.kind) {
      case "saved":
        last = { kind: "saved" };
        break;
      case "not-saved":
        last = { kind: "not-saved", reason: outcome.reason };
        break;
      case "lost":
        last = { kind: "abandoned" };
        standing = standing === "available" ? "suspended" : "disabled";
        // The next still starts from one unit and grows to half the batch that was in flight.
        this.#maxUnits = Math.max(1, Math.floor(outcome.batchUnits / 2));
        break;
    }
    this.#set({ ...this.#state, standing, activity: { kind: "idle", last } });
  }

  #set(state: StillControlState): void {
    this.#state = state;
    for (const listener of this.#listeners) {
      listener();
    }
  }
}
