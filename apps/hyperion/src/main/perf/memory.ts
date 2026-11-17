/**
 * A perf run's 1 Hz memory sampling (plan R12, R12.T4.b; Design notes 6 and 14): the GPU process's
 * DRM fdinfo, NVIDIA's `nvidia-smi`, the display server's fdinfo and every process's working set,
 * each sample stamped on the runner's clock so that a repetition takes those of its span alone.
 *
 * @remarks
 * R12.T3.b's samplers read each source; this module only runs them at
 * {@link MEMORY_SAMPLE_INTERVAL_MS} and summarises a span. The GPU process's ID is the one
 * `app.getAppMetrics()` names at each sample (`appMemoryOf`), since a restarted GPU process takes
 * a new one; a span's DRM summary leaves out the samples within 2 s of a large release the tally saw
 * (`summariseDrm`).
 */

import type { Measured } from "../measured";
import {
  appMemoryOf,
  type AppMemorySample,
  type DrmMemorySample,
  type DrmSpanSummary,
  type Every,
  failureOf,
  type NvidiaMemorySample,
  type NvidiaSpanSummary,
  summariseDrm,
  summariseNvidia,
  type TallyRelease,
} from "./samplers";

/** The 1 Hz memory samplers' interval, ms (Design note 14). */
export const MEMORY_SAMPLE_INTERVAL_MS = 1000;

/** What the memory sampler reads, each a sampler of R12.T3.b's over the run's own sources. */
export interface PerfMemorySources {
  /** `app.getAppMetrics()`. */
  appMetrics(): unknown;
  /** The GPU process's DRM sample (`sampleGpuMemory` with the GPU's drivers). */
  gpu(pid: number, atMs: number): Promise<Measured<DrmMemorySample>>;
  /** NVIDIA's sample for the GPU process (`sampleNvidia` with the baseline), or `null` off NVIDIA. */
  readonly nvidia:
    ((gpuPid: number | null, atMs: number) => Promise<Measured<NvidiaMemorySample>>) | null;
  /** Xorg's DRM sample (`sampleDisplayServer`), or `null` where there is none to read. */
  displayServer(atMs: number): Promise<DrmMemorySample | null>;
}

/** What a {@link PerfMemorySampler} is given. */
export interface PerfMemorySamplerOptions {
  readonly sources: PerfMemorySources;
  /** {@link MEMORY_SAMPLE_INTERVAL_MS} outside a test. */
  readonly intervalMs: number;
  /** The runner's clock, ms. */
  readonly now: () => number;
  readonly every: Every;
}

/** A span's memory, as the record's repetition takes it. */
export interface MemorySpan {
  readonly drm: DrmSpanSummary | null;
  readonly nvidia: NvidiaSpanSummary | null;
  readonly displayServer: DrmSpanSummary | null;
  readonly appPeakBytes: number | null;
}

/** The 1 Hz memory samplers of a run, each sample timestamped. */
export class PerfMemorySampler {
  readonly #options: PerfMemorySamplerOptions;
  readonly #app: AppMemorySample[] = [];
  readonly #drm: DrmMemorySample[] = [];
  readonly #nvidia: NvidiaMemorySample[] = [];
  readonly #displayServer: DrmMemorySample[] = [];
  #cancel: (() => void) | null = null;
  #reading = false;
  #failure: string | null = null;

  constructor(options: PerfMemorySamplerOptions) {
    this.#options = options;
  }

  /** Starts sampling every {@link PerfMemorySamplerOptions.intervalMs}, once. */
  start(): void {
    if (this.#cancel !== null) {
      return;
    }
    this.#cancel = this.#options.every(() => {
      if (this.#reading) {
        return;
      }
      this.#reading = true;
      void this.sampleOnce()
        .catch((error: unknown) => {
          this.#failure = failureOf(error);
        })
        .finally(() => {
          this.#reading = false;
        });
    }, this.#options.intervalMs);
  }

  /** Stops sampling; the samples taken are kept. */
  stop(): void {
    this.#cancel?.();
    this.#cancel = null;
  }

  /** Why a tick failed, or `null` when none did. */
  get failure(): string | null {
    return this.#failure;
  }

  /** The GPU process's ID at the last sample, or `null` before one names it. */
  get gpuPid(): number | null {
    return this.#app.at(-1)?.gpuPid ?? null;
  }

  /** Takes one sample of every source now. */
  async sampleOnce(): Promise<void> {
    const { sources, now } = this.#options;
    const atMs = now();
    const app = appMemoryOf(sources.appMetrics(), atMs);
    this.#app.push(app);
    const [drm, nvidia, displayServer] = await Promise.all([
      app.gpuPid === null ? Promise.resolve(null) : sources.gpu(app.gpuPid, atMs),
      sources.nvidia === null ? Promise.resolve(null) : sources.nvidia(app.gpuPid, atMs),
      sources.displayServer(atMs),
    ]);
    if (drm?.value !== null && drm?.value !== undefined) {
      this.#drm.push(drm.value);
    }
    if (nvidia?.value !== null && nvidia?.value !== undefined) {
      this.#nvidia.push(nvidia.value);
    }
    if (displayServer !== null) {
      this.#displayServer.push(displayServer);
    }
  }

  /** The samples in `[fromMs, toMs]` summarised, the DRM peaks settled after `releases`. */
  span(fromMs: number, toMs: number, releases: ReadonlyArray<TallyRelease>): MemorySpan {
    const within = <T extends { readonly atMs: number }>(samples: ReadonlyArray<T>): T[] =>
      samples.filter(({ atMs }) => atMs >= fromMs && atMs <= toMs);
    const app = within(this.#app);
    const nvidia = within(this.#nvidia);
    return {
      drm: summariseDrm(within(this.#drm), releases),
      nvidia: nvidia.length === 0 ? null : summariseNvidia(nvidia),
      displayServer: summariseDrm(within(this.#displayServer), []),
      appPeakBytes: app.length === 0 ? null : Math.max(...app.map(({ totalBytes }) => totalBytes)),
    };
  }

  /** Drops the samples before `ms`, once their repetition is summarised. */
  discardBefore(ms: number): void {
    for (const samples of [this.#app, this.#drm, this.#nvidia, this.#displayServer]) {
      const keep = samples.findIndex(({ atMs }) => atMs >= ms);
      samples.splice(0, keep < 0 ? samples.length : keep);
    }
  }
}
