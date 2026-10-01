/**
 * Per-pass GPU time, from one `GPUQuerySet` the adapter owns (R01 Design note 19).
 *
 * @remarks
 Where the device has `timestamp-query`, every pass the adapter encodes (a frame's draws, each
 * post-process, dispatches, splats, mip passes) carries `timestampWrites`; since every pass is the
 * adapter's own (Design note 24), none is `bracketed`. A frame's queries resolve into a
 * buffer that is mapped a frame or more later, and the listeners get each pass by its label.
 * Without the feature there is no query set and nothing is ever reported.
 */

import { BUFFER_USAGE, MAP_MODE } from "../gpuFlags";
import type { GpuTimer } from "../status";
import type { PassTimes } from "../types";

/** A pass whose two timestamps the frame holds. */
export interface TimedPass {
  readonly label: string;
  readonly bracketed: boolean;
  readonly begin: number;
  readonly end: number;
}

/** What of a device the timer uses. */
export type TimerDevice = Pick<GPUDevice, "createQuerySet" | "createBuffer">;

/** Passes one frame can time: two queries each. */
export const PASSES_PER_FRAME = 64;

/** The timestamp writes of the beginning and end of one pass. */
export interface PassWrites {
  readonly querySet: GPUQuerySet;
  readonly beginningOfPassWriteIndex: number;
  readonly endOfPassWriteIndex: number;
}

/** Allocates a frame's timestamps and reports them once resolved. */
export class PassTimer {
  readonly timer: GpuTimer;
  readonly #device: TimerDevice | null;
  readonly #querySet: GPUQuerySet | null;
  readonly #listeners = new Set<(times: PassTimes) => void>();
  #pending: TimedPass[] = [];
  #frame = 0;
  #warned = false;
  #disposed = false;

  /**
   * Makes the timer, and its query set when the device has timestamps.
   *
   * @param timer - The device's timer: `absent` without `timestamp-query`, when no query set is
   * made and nothing is ever reported.
   */
  constructor(device: TimerDevice, timer: GpuTimer) {
    this.timer = timer;
    if (timer === "absent") {
      this.#device = null;
      this.#querySet = null;
      return;
    }
    this.#device = device;
    this.#querySet = device.createQuerySet({
      label: "pass times",
      type: "timestamp",
      count: PASSES_PER_FRAME * 2,
    });
  }

  /** The passes timed since the last resolve, in the order they were encoded. */
  get pending(): ReadonlyArray<TimedPass> {
    return this.#pending;
  }

  /** Stops reporting, drops the listeners and destroys the query set. */
  dispose(): void {
    this.#disposed = true;
    this.#listeners.clear();
    this.#pending = [];
    this.#querySet?.destroy();
  }

  /** Registers a listener for each frame's times; returns its removal. */
  listen(listener: (times: PassTimes) => void): () => void {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  }

  /** The timestamp writes of a pass the adapter encodes, or `undefined` when untimed. */
  writesFor(label: string): PassWrites | undefined {
    const pass = this.#allocate(label, false);
    return pass === undefined || this.#querySet === null
      ? undefined
      : {
          querySet: this.#querySet,
          beginningOfPassWriteIndex: pass.begin,
          endOfPassWriteIndex: pass.end,
        };
  }

  /**
   * Encodes the resolve of the frame's timestamps, and returns what reads them once the encoder is
   * submitted, or `null` when nothing was timed.
   */
  resolve(
    encoder: Pick<GPUCommandEncoder, "resolveQuerySet" | "copyBufferToBuffer">,
  ): (() => void) | null {
    const passes = this.#pending;
    if (passes.length === 0 || this.#device === null || this.#querySet === null) {
      return null;
    }
    this.#pending = [];
    this.#frame += 1;
    const frame = this.#frame;
    const count = passes.length * 2;
    const resolved = this.#device.createBuffer({
      label: "pass times resolved",
      size: count * 8,
      usage: BUFFER_USAGE.QUERY_RESOLVE | BUFFER_USAGE.COPY_SRC,
    });
    const staging = this.#device.createBuffer({
      label: "pass times readback",
      size: count * 8,
      usage: BUFFER_USAGE.COPY_DST | BUFFER_USAGE.MAP_READ,
    });
    encoder.resolveQuerySet(this.#querySet, 0, count, resolved, 0);
    encoder.copyBufferToBuffer(resolved, 0, staging, 0, count * 8);
    return () => {
      void staging
        .mapAsync(MAP_MODE.READ)
        .then((): void => {
          const stamps = new BigUint64Array(staging.getMappedRange().slice(0));
          this.#report(frame, passes, stamps);
          return undefined;
        })
        .catch((error: unknown) => {
          // A read cut short by the engine's own disposal is not a failure.
          if (!this.#disposed) {
            console.error("reading the pass times failed:", error);
          }
        })
        .finally(() => {
          staging.destroy();
          resolved.destroy();
        });
    };
  }

  /** Reports one frame's passes from their resolved timestamps, in nanoseconds. */
  #report(frame: number, passes: ReadonlyArray<TimedPass>, stamps: BigUint64Array): void {
    const times: PassTimes = {
      frame,
      timer: this.timer,
      passes: passes.map(({ label, bracketed, begin, end }) => ({
        label,
        bracketed,
        ns: Number((stamps[end] ?? 0n) - (stamps[begin] ?? 0n)),
      })),
    };
    for (const listener of this.#listeners) {
      listener(times);
    }
  }

  #allocate(label: string, bracketed: boolean): TimedPass | undefined {
    if (this.#querySet === null || this.#disposed) {
      return undefined;
    }
    const index = this.#pending.length;
    if (index >= PASSES_PER_FRAME) {
      if (!this.#warned) {
        this.#warned = true;
        console.warn(`more than ${PASSES_PER_FRAME} passes in a frame; the rest are not timed`);
      }
      return undefined;
    }
    const pass: TimedPass = { label, bracketed, begin: index * 2, end: index * 2 + 1 };
    this.#pending.push(pass);
    return pass;
  }
}
