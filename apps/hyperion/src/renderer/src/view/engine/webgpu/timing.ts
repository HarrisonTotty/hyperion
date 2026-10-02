/**
 * Per-pass GPU time, from one `GPUQuerySet` the adapter owns (R01 Design note 19).
 *
 * @remarks
 * Where the device has `timestamp-query`, every pass the adapter encodes (a frame's draws, each
 * post-process, dispatches, splats, mip passes) carries `timestampWrites`; since every pass is the
 * adapter's own (Design note 24), none is `bracketed`. A frame's queries resolve into a buffer
 * that is mapped a frame or more later, and the listeners get each pass by its label. The query
 * set and a small ring of resolve and staging buffers are made through the engine's one creation
 * path, so the allocation tally sees them, and the buffers are reused, not made each frame.
 * Without the feature there is no query set and nothing is ever reported.
 */

import { BUFFER_USAGE, MAP_MODE } from "../gpuFlags";
import type { BufferSpec } from "../memory";
import type { GpuTimer } from "../status";
import type { BufferHandle, PassTimes } from "../types";

/** A pass whose two timestamps the frame holds. */
export interface TimedPass {
  readonly label: string;
  readonly bracketed: boolean;
  readonly begin: number;
  readonly end: number;
}

/** What the timer needs of the engine: its one creation path. */
export interface TimerHost {
  /** Makes a query set, counted as an allocation; the engine destroys it at disposal. */
  createQuerySet(descriptor: GPUQuerySetDescriptor): GPUQuerySet;
  /** Makes a buffer, counted as an allocation; the engine destroys it at disposal. */
  createBuffer(spec: BufferSpec): BufferHandle;
  gpuBufferOf(handle: BufferHandle): GPUBuffer;
}

/** Passes one frame can time: two queries each. */
export const PASSES_PER_FRAME = 64;

/** Resolve and staging pairs at most: frames whose times may be in flight at once. */
export const TIMING_FRAMES_IN_FLIGHT = 3;

/** Bytes of one pair's buffers: every query of a frame, 8 bytes each. */
const RESOLVE_BYTES = PASSES_PER_FRAME * 2 * 8;

/** The timestamp writes of the beginning and end of one pass. */
export interface PassWrites {
  readonly querySet: GPUQuerySet;
  readonly beginningOfPassWriteIndex: number;
  readonly endOfPassWriteIndex: number;
}

/** A frame's resolve buffer and the staging buffer it is copied into to be mapped. */
interface ResolvePair {
  readonly resolved: GPUBuffer;
  readonly staging: GPUBuffer;
}

/** Allocates a frame's timestamps and reports them once resolved. */
export class PassTimer {
  readonly timer: GpuTimer;
  readonly #host: TimerHost | null;
  #querySet: GPUQuerySet | null = null;
  readonly #listeners = new Set<(times: PassTimes) => void>();
  /** Pairs not in flight, ready for the next resolve. */
  readonly #free: ResolvePair[] = [];
  #pairs = 0;
  #pending: TimedPass[] = [];
  #frame = 0;
  #warned = false;
  #warnedInFlight = false;
  #disposed = false;

  /**
   * Makes the timer; its query set is made with the first pass it times, after the engine's
   * caller has had the chance to listen for allocations.
   *
   * @param timer - The device's timer: `absent` without `timestamp-query`, when no query set is
   * made and nothing is ever reported.
   */
  constructor(host: TimerHost, timer: GpuTimer) {
    this.timer = timer;
    this.#host = timer === "absent" ? null : host;
  }

  /** The passes timed since the last resolve, in the order they were encoded. */
  get pending(): ReadonlyArray<TimedPass> {
    return this.#pending;
  }

  /** Stops reporting and drops the listeners; the engine destroys the query set and buffers. */
  dispose(): void {
    this.#disposed = true;
    this.#listeners.clear();
    this.#pending = [];
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

  /** A mark of the passes timed so far, for {@link PassTimer.rollBack}. */
  mark(): number {
    return this.#pending.length;
  }

  /**
   * Forgets the passes timed since `mark`: their encoding threw, so they were never submitted and
   * their timestamps never written.
   */
  rollBack(mark: number): void {
    this.#pending.length = Math.min(this.#pending.length, mark);
  }

  /**
   * Encodes the resolve of the frame's timestamps, and returns what reads them once the encoder is
   * submitted, or `null` when nothing was timed.
   *
   * @remarks
   * When every pair is still in flight, the frame's times are dropped, with one warning, rather
   * than a buffer being made.
   */
  resolve(
    encoder: Pick<GPUCommandEncoder, "resolveQuerySet" | "copyBufferToBuffer">,
  ): (() => void) | null {
    const passes = this.#pending;
    if (passes.length === 0 || this.#querySet === null) {
      return null;
    }
    this.#pending = [];
    const pair = this.#free.pop() ?? this.#makePair();
    if (pair === null) {
      if (!this.#warnedInFlight) {
        this.#warnedInFlight = true;
        console.warn(`${TIMING_FRAMES_IN_FLIGHT} frames' pass times are in flight; dropping some`);
      }
      return null;
    }
    this.#frame += 1;
    const frame = this.#frame;
    const bytes = passes.length * 2 * 8;
    encoder.resolveQuerySet(this.#querySet, 0, passes.length * 2, pair.resolved, 0);
    encoder.copyBufferToBuffer(pair.resolved, 0, pair.staging, 0, bytes);
    return () => {
      void pair.staging
        .mapAsync(MAP_MODE.READ, 0, bytes)
        .then((): void => {
          const stamps = new BigUint64Array(pair.staging.getMappedRange(0, bytes).slice(0, bytes));
          pair.staging.unmap();
          this.#free.push(pair);
          this.#report(frame, passes, stamps);
          return undefined;
        })
        .catch((error: unknown) => {
          // A read cut short by the engine's own disposal is not a failure.
          if (!this.#disposed) {
            console.error("reading the pass times failed:", error);
          }
        });
    };
  }

  /** A new pair, while fewer than {@link TIMING_FRAMES_IN_FLIGHT} exist, or `null`. */
  #makePair(): ResolvePair | null {
    const host = this.#host;
    if (host === null || this.#pairs >= TIMING_FRAMES_IN_FLIGHT) {
      return null;
    }
    this.#pairs += 1;
    const make = (name: string, usage: GPUBufferUsageFlags): GPUBuffer =>
      host.gpuBufferOf(
        host.createBuffer({
          name: `pass times ${name} ${this.#pairs}`,
          bytes: RESOLVE_BYTES,
          usage,
          category: "other",
        }),
      );
    return {
      resolved: make("resolved", BUFFER_USAGE.QUERY_RESOLVE | BUFFER_USAGE.COPY_SRC),
      staging: make("readback", BUFFER_USAGE.COPY_DST | BUFFER_USAGE.MAP_READ),
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
    if (this.#host === null || this.#disposed) {
      return undefined;
    }
    this.#querySet ??= this.#host.createQuerySet({
      label: "pass times",
      type: "timestamp",
      count: PASSES_PER_FRAME * 2,
    });
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
