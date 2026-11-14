import type { GpuTimer } from "../view/engine/status";
import type { MeasuredPass } from "../view/stills/batchController";
import type { StillDevice, StillRenderer, StillScene, StillSource } from "../view/stills/stillJob";
import type { StillRequest } from "../view/stills/stillLabel";
import { planTiles, type StillTile, type StillTiling } from "../view/stills/stillPlan";

/** The display's vsync period the fakes draw at, ns: 59.94 Hz. */
export const FAKE_FRAME_NS = 16_683_350;

/** The submit round trip's own cost, ns, beside the batch's GPU time: 3 ms. */
export const FAKE_SUBMIT_OVERHEAD_NS = 3_000_000;

/**
 * A device for a still's job (plan R11, R11.T10.b): a clock that the GPU's work and the animation
 * frames advance, the work submitted done only once its `submittedWorkDone()` resolves, its pass
 * times then handed over with timestamps, and a loss the test raises, at a batch or while a frame
 * it holds is awaited.
 *
 * @remarks
 * It counts what a job must never do: a batch submitted while another is in flight, or two in one
 * animation frame.
 */
export class FakeStillDevice implements StillDevice {
  readonly timer: GpuTimer;
  /** Each unit's GPU time, ns. */
  readonly costNs: number;
  #nowNs = 0;
  /** Units submitted and not yet done, by label. */
  #pending: string[] = [];
  #frame = 0;
  #lastSubmitFrame = -1;
  readonly #lossListeners = new Set<() => void>();
  readonly #passListeners = new Set<(passes: ReadonlyArray<MeasuredPass>) => void>();
  /** Every batch submitted, its units' count, in order. */
  readonly batches: number[] = [];
  /** Batches submitted while another was in flight, or in a frame that had one. */
  violations = 0;
  emptySubmits = 0;
  /** Raises a loss at the batch of this index, once, where set. */
  loseAtBatch: number | null = null;
  /** Holds the animation frame of this count open, never resolving it, where set. */
  holdFrameAt: number | null = null;
  /** Whether a frame is held open, awaited. */
  holding = false;

  constructor(timer: GpuTimer, costNs = 400_000) {
    this.timer = timer;
    this.costNs = costNs;
  }

  /** The renderer's submission of `labels`, one unit each. */
  submitUnits(labels: ReadonlyArray<string>): void {
    if (this.#pending.length > 0 || this.#lastSubmitFrame === this.#frame) {
      this.violations += 1;
    }
    this.#lastSubmitFrame = this.#frame;
    this.#pending = [...this.#pending, ...labels];
    this.batches.push(labels.length);
    if (this.loseAtBatch === this.batches.length - 1) {
      this.loseAtBatch = null;
      this.lose();
    }
  }

  /** Raises a device loss. */
  lose(): void {
    for (const listener of this.#lossListeners) {
      listener();
    }
  }

  async submittedWorkDone(): Promise<void> {
    // The work submitted so far, done as the promise resolves, not as it is asked for.
    const done = this.#pending.length;
    await Promise.resolve();
    const labels = this.#pending.slice(0, done);
    this.#pending = this.#pending.slice(done);
    this.#nowNs += FAKE_SUBMIT_OVERHEAD_NS + labels.length * this.costNs;
    if (this.timer !== "absent" && labels.length > 0) {
      const passes = labels.map((label) => ({ label, ns: this.costNs }));
      for (const listener of this.#passListeners) {
        listener(passes);
      }
    }
  }

  submitEmpty(): void {
    this.emptySubmits += 1;
  }

  nextFrame(): Promise<void> {
    this.#frame += 1;
    if (this.holdFrameAt === this.#frame) {
      this.holding = true;
      return new Promise(() => undefined);
    }
    this.#nowNs += FAKE_FRAME_NS;
    return Promise.resolve();
  }

  nowNs(): number {
    return this.#nowNs;
  }

  onPassTimes(listener: (passes: ReadonlyArray<MeasuredPass>) => void): () => void {
    this.#passListeners.add(listener);
    return () => {
      this.#passListeners.delete(listener);
    };
  }

  onLoss(listener: () => void): () => void {
    this.#lossListeners.add(listener);
    return () => {
      this.#lossListeners.delete(listener);
    };
  }

  /** The loss listeners held: none once a job has ended. */
  get lossListeners(): number {
    return this.#lossListeners.size;
  }
}

/** The passes each tile of the fake renderer takes, in order. */
export const FAKE_PASSES = ["terrain", "clouds", "tonemap"] as const;

/**
 * The jittered samples the fake renderer accumulates (Design note 17's sub-pixel jitter at full
 * resolution): each repeats every tile's passes, under the same labels, so their costs are known
 * from the first sample on.
 */
export const FAKE_SAMPLES = 4;

/**
 * A renderer for a still's job (plan R11, R11.T10.b): {@link FAKE_SAMPLES} samples of
 * {@link FAKE_PASSES} a tile at a 64 px tile limit, each unit submitted to the device, every tile
 * read back as texels holding its index plus one; it counts its pipelines made each way.
 */
export class FakeStillRenderer implements StillRenderer {
  readonly tiling: StillTiling;
  readonly units: ReadonlyArray<string>;
  readonly #device: FakeStillDevice;
  /** Whether the frustum's terrain has streamed: the test sets it. */
  terrainStreamed = true;
  prepared = false;
  disposed = false;
  /** Pipelines made, asynchronously, in `prepare`. */
  asyncPipelines = 0;
  /** Units submitted before `prepare` had resolved. */
  unprepared = 0;

  constructor(device: FakeStillDevice, request: StillRequest) {
    this.#device = device;
    this.tiling = planTiles(request.size, 64, "low");
    const sample = this.tiling.tiles.flatMap((tile) =>
      FAKE_PASSES.map((pass) => `${pass} ${String(tile.column)},${String(tile.row)}`),
    );
    this.units = Array.from({ length: FAKE_SAMPLES }, () => sample).flat();
  }

  async prepare(): Promise<void> {
    await Promise.resolve();
    this.asyncPipelines += FAKE_PASSES.length;
    this.prepared = true;
  }

  streamed(): boolean {
    return this.terrainStreamed;
  }

  submit(first: number, count: number): void {
    if (!this.prepared) {
      this.unprepared += count;
    }
    this.#device.submitUnits(this.units.slice(first, first + count));
  }

  readTile(tile: StillTile): Promise<Uint8Array> {
    const index = this.tiling.tiles.indexOf(tile);
    return Promise.resolve(
      new Uint8Array(tile.bytesPerRow * tile.texture.heightPx).fill(index + 1),
    );
  }

  dispose(): void {
    this.disposed = true;
  }
}

/**
 * A view's still source over the fakes: its request as the view holds it now, which a test may
 * change after the request, and each renderer it made.
 */
export class FakeStillSource implements StillSource {
  readonly device: FakeStillDevice;
  readonly setting = "low" as const;
  /** What the view holds now. */
  current: StillRequest;
  readonly renderers: FakeStillRenderer[] = [];
  snapshots = 0;

  constructor(device: FakeStillDevice, request: StillRequest) {
    this.device = device;
    this.current = request;
  }

  frameIntervalNs(): number {
    return FAKE_FRAME_NS;
  }

  snapshot(): StillScene {
    this.snapshots += 1;
    return { request: this.current };
  }

  renderer(scene: StillScene): FakeStillRenderer {
    const renderer = new FakeStillRenderer(this.device, scene.request);
    this.renderers.push(renderer);
    return renderer;
  }
}
