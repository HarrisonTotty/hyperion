/**
 * The height-worker pool: one priority queue on the render thread feeding a few module workers,
 * each with its own WebAssembly instance of `hyperion-surface` (plan R05, T10.a, Design note 11).
 *
 * @remarks
 * Each worker has at most two requests in flight, so that it never idles between bakes while
 * cancelling stays cheap: a queued request is cancelled by removing it, and priorities are re-scored
 * every frame. Per-worker queues were rejected, because their cancellations race the bakes. A
 * result whose generation is stale is still delivered if its key is still wanted, and dropped
 * otherwise. The coarse field is posted to one worker at a time, each copy acknowledged before the
 * next is made, so that at most one transient copy exists; once a field is posted, a worker bakes
 * only after it holds it, and a bake made from an older field is baked again.
 */

import { patchKeyString } from "../patchKey";
import type { PatchRequest } from "../select";
import type { BakedPatch, BakeSettings, HeightWorkerReply, HeightWorkerRequest } from "./messages";

/** The part of a `Worker` the pool uses, so that tests can stand in for one. */
export interface WorkerLike {
  addEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  /** An `ErrorEvent` when the script threw, a plain `Event` when a module failed to load. */
  addEventListener(type: "error", listener: (event: Event) => void): void;
  removeEventListener(
    type: "message",
    listener: (event: MessageEvent<HeightWorkerReply>) => void,
  ): void;
  removeEventListener(type: "error", listener: (event: Event) => void): void;
  postMessage(message: HeightWorkerRequest, transfer: Transferable[]): void;
  terminate(): void;
}

/** The most requests a worker has in flight at once. */
export const MAX_IN_FLIGHT_PER_WORKER = 2;

/**
 * How many times in a row a worker may fail, with no answer between, before its place in the pool
 * is given up rather than restarted: a module that never loads must not restart forever.
 */
export const MAX_CONSECUTIVE_WORKER_FAILURES = 3;

/** What makes a pool. */
export interface HeightWorkerPoolOptions {
  /** How many workers to run, at least one (Design note 11's defaults, or `--workers`). */
  readonly workers: number;
  /**
   * Starts one worker: `new Worker(new URL("./height.worker.ts", import.meta.url), { type:
   * "module" })` in the app, a fake in tests.
   */
  readonly createWorker: () => WorkerLike;
  /** The setting's vertex path and normal resolution, sent with every bake. */
  readonly bake: BakeSettings;
}

/** A request given up because its bake failed, for the metrics and the console. */
export interface BakeFailure {
  readonly keyString: string;
  readonly message: string;
}

interface Queued {
  readonly request: PatchRequest;
  readonly keyString: string;
}

interface InFlight {
  readonly id: number;
  readonly request: PatchRequest;
  readonly keyString: string;
  readonly generation: number;
  /** The field the worker held when the request was sent. */
  readonly fieldEpoch: number;
}

interface Slot {
  worker: WorkerLike;
  readonly onMessage: (event: MessageEvent<HeightWorkerReply>) => void;
  readonly onError: () => void;
  readonly inFlight: Map<number, InFlight>;
  /** Whether the worker holds the current field. */
  hasField: boolean;
  /** Failures in a row with no answer between. */
  failures: number;
  /** Given up after {@link MAX_CONSECUTIVE_WORKER_FAILURES}: no worker runs in this place. */
  lost: boolean;
}

/** Orders requests: forced first, then higher priority, then by key string. */
function before(a: Queued, b: Queued): boolean {
  if (a.request.forced !== b.request.forced) {
    return a.request.forced;
  }
  if (a.request.priority !== b.request.priority) {
    return a.request.priority > b.request.priority;
  }
  return a.keyString < b.keyString;
}

/**
 * The pool of height workers that bake patches for the cache.
 *
 * @remarks
 * Each frame the terrain pass calls {@link HeightWorkerPool.reprioritise} with the selection's
 * demand (the patches not yet resident); the pool hands bakes to {@link HeightWorkerPool.onBaked}'s
 * listeners, which insert them into the cache. A key whose bake failed is reported through
 * {@link HeightWorkerPool.onFailed} and not requested again. {@link HeightWorkerPool.terminate}
 * stops every worker and drops every listener.
 */
export class HeightWorkerPool {
  private readonly createWorker: () => WorkerLike;
  private readonly settings: BakeSettings;
  private readonly slots: Slot[] = [];
  private readonly queue = new Map<string, Queued>();
  private readonly wanted = new Set<string>();
  private readonly failedKeys = new Set<string>();
  private readonly bakedListeners = new Set<(bake: BakedPatch) => void>();
  private readonly failureListeners = new Set<(failure: BakeFailure) => void>();
  private field: ArrayBuffer | null = null;
  private fieldEpoch = 0;
  private fieldPending: { readonly slot: Slot; readonly id: number } | null = null;
  private generation = 0;
  private nextId = 1;
  private terminated = false;

  constructor(opts: HeightWorkerPoolOptions) {
    if (!Number.isInteger(opts.workers) || opts.workers < 1) {
      throw new Error(`a height-worker pool needs at least one worker, not ${opts.workers}`);
    }
    this.createWorker = opts.createWorker;
    this.settings = opts.bake;
    for (let n = 0; n < opts.workers; n += 1) {
      const slot: Slot = {
        worker: this.createWorker(),
        onMessage: (event) => {
          this.receive(slot, event.data);
        },
        onError: () => {
          this.replace(slot);
        },
        inFlight: new Map(),
        hasField: false,
        failures: 0,
        lost: false,
      };
      this.slots.push(slot);
      this.attach(slot);
    }
  }

  /** The current generation, which every new request is tagged with. */
  get currentGeneration(): number {
    return this.generation;
  }

  /** How many requests are queued, not yet sent to a worker. */
  get queuedCount(): number {
    return this.queue.size;
  }

  /** How many requests are in flight, over every worker. */
  get inFlightCount(): number {
    let n = 0;
    for (const slot of this.slots) {
      n += slot.inFlight.size;
    }
    return n;
  }

  /**
   * How many workers were given up after failing {@link MAX_CONSECUTIVE_WORKER_FAILURES} times in a
   * row; when it equals the pool's size, nothing more is baked.
   */
  get lostWorkers(): number {
    return this.slots.filter((s) => s.lost).length;
  }

  /** Queues one request, or re-scores it if queued already; in flight, it is left alone. */
  request(r: PatchRequest): void {
    this.assertLive();
    const keyString = patchKeyString(r.key);
    if (this.failedKeys.has(keyString)) {
      return;
    }
    this.wanted.add(keyString);
    if (!this.isInFlight(keyString)) {
      this.queue.set(keyString, { request: r, keyString });
    }
    this.pump();
  }

  /**
   * Replaces the demand, once a frame: queues new requests, re-scores queued ones and drops those
   * no longer in the demand. Requests in flight are untouched, and their results are kept if their
   * keys are still in the demand when they arrive.
   */
  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.assertLive();
    this.generation += 1;
    this.wanted.clear();
    this.queue.clear();
    for (const request of demand) {
      const keyString = patchKeyString(request.key);
      if (this.failedKeys.has(keyString)) {
        continue;
      }
      this.wanted.add(keyString);
      if (!this.isInFlight(keyString)) {
        this.queue.set(keyString, { request, keyString });
      }
    }
    this.pump();
  }

  /** Drops every queued request, and every result yet to arrive, whose key is not in `keep`. */
  cancelStale(keep: ReadonlySet<string>): void {
    this.assertLive();
    for (const keyString of this.queue.keys()) {
      if (!keep.has(keyString)) {
        this.queue.delete(keyString);
      }
    }
    for (const keyString of this.wanted) {
      if (!keep.has(keyString)) {
        this.wanted.delete(keyString);
      }
    }
  }

  /** Listens for bakes; returns the function that stops listening. */
  onBaked(cb: (bake: BakedPatch) => void): () => void {
    this.bakedListeners.add(cb);
    return () => {
      this.bakedListeners.delete(cb);
    };
  }

  /** Listens for requests given up because their bake failed; returns the unsubscribe. */
  onFailed(cb: (failure: BakeFailure) => void): () => void {
    this.failureListeners.add(cb);
    return () => {
      this.failureListeners.delete(cb);
    };
  }

  /**
   * Posts the coarse field to every worker in turn, each copy acknowledged before the next.
   *
   * @remarks
   * The pool keeps `bytes`, about 15 MB on the render thread, so that a worker started later, to
   * replace a failed one, gets it too. A second call replaces the field in every worker, and bakes
   * in flight from the old field are baked again.
   */
  postField(bytes: ArrayBuffer): void {
    this.assertLive();
    this.field = bytes;
    this.fieldEpoch += 1;
    for (const slot of this.slots) {
      slot.hasField = false;
    }
    this.fieldPending = null;
    this.postNextField();
  }

  /** Stops every worker and drops every listener; the pool cannot be used afterwards. */
  terminate(): void {
    if (this.terminated) {
      return;
    }
    this.terminated = true;
    for (const slot of this.slots) {
      this.detach(slot);
      slot.worker.terminate();
      slot.inFlight.clear();
    }
    this.queue.clear();
    this.wanted.clear();
    this.bakedListeners.clear();
    this.failureListeners.clear();
    this.field = null;
    this.fieldPending = null;
  }

  private assertLive(): void {
    if (this.terminated) {
      throw new Error("the height-worker pool was terminated");
    }
  }

  private isInFlight(keyString: string): boolean {
    for (const slot of this.slots) {
      for (const inFlight of slot.inFlight.values()) {
        if (inFlight.keyString === keyString) {
          return true;
        }
      }
    }
    return false;
  }

  private attach(slot: Slot): void {
    slot.worker.addEventListener("message", slot.onMessage);
    slot.worker.addEventListener("error", slot.onError);
  }

  private detach(slot: Slot): void {
    slot.worker.removeEventListener("message", slot.onMessage);
    slot.worker.removeEventListener("error", slot.onError);
  }

  /** Whether a worker may be sent a bake: running, and holding the field if one was posted. */
  private canBake(slot: Slot): boolean {
    return !slot.lost && (this.field === null || slot.hasField);
  }

  /**
   * Hands queued requests to workers with room, the best request first, one a worker a round and
   * the least loaded workers first, so that no worker idles while another holds two.
   */
  private pump(): void {
    if (this.terminated) {
      return;
    }
    for (let load = 0; load < MAX_IN_FLIGHT_PER_WORKER; load += 1) {
      for (const slot of this.slots) {
        if (slot.inFlight.size !== load || !this.canBake(slot)) {
          continue;
        }
        const next = this.best();
        if (next === null) {
          return;
        }
        this.send(slot, next);
      }
    }
  }

  private send(slot: Slot, next: Queued): void {
    this.queue.delete(next.keyString);
    const inFlight: InFlight = {
      id: this.nextId,
      request: next.request,
      keyString: next.keyString,
      generation: this.generation,
      fieldEpoch: this.fieldEpoch,
    };
    this.nextId += 1;
    slot.inFlight.set(inFlight.id, inFlight);
    slot.worker.postMessage(
      {
        kind: "bake",
        id: inFlight.id,
        key: next.request.key,
        generation: inFlight.generation,
        settings: this.settings,
      },
      [],
    );
  }

  private best(): Queued | null {
    let best: Queued | null = null;
    for (const queued of this.queue.values()) {
      if (best === null || before(queued, best)) {
        best = queued;
      }
    }
    return best;
  }

  /** Puts a request back in the queue if its key is still wanted and not queued already. */
  private requeue(inFlight: InFlight): void {
    if (this.wanted.has(inFlight.keyString) && !this.queue.has(inFlight.keyString)) {
      this.queue.set(inFlight.keyString, {
        request: inFlight.request,
        keyString: inFlight.keyString,
      });
    }
  }

  private receive(slot: Slot, reply: HeightWorkerReply): void {
    slot.failures = 0;
    switch (reply.kind) {
      case "baked": {
        const inFlight = slot.inFlight.get(reply.id);
        slot.inFlight.delete(reply.id);
        if (inFlight === undefined) {
          break;
        }
        if (inFlight.fieldEpoch !== this.fieldEpoch) {
          this.requeue(inFlight);
        } else if (this.wanted.has(inFlight.keyString)) {
          this.wanted.delete(inFlight.keyString);
          for (const listener of this.bakedListeners) {
            listener(reply.bake);
          }
        }
        break;
      }
      case "bake-failed": {
        const inFlight = slot.inFlight.get(reply.id);
        slot.inFlight.delete(reply.id);
        if (inFlight !== undefined) {
          this.wanted.delete(inFlight.keyString);
          this.failedKeys.add(inFlight.keyString);
          const failure = { keyString: inFlight.keyString, message: reply.message };
          for (const listener of this.failureListeners) {
            listener(failure);
          }
        }
        break;
      }
      case "field-loaded":
        if (this.fieldPending !== null && this.fieldPending.id === reply.id) {
          this.fieldPending.slot.hasField = true;
          this.fieldPending = null;
          this.postNextField();
        }
        break;
    }
    this.pump();
  }

  /**
   * Replaces a failed worker, re-queueing its requests still wanted and re-posting the field, or
   * gives its place up after {@link MAX_CONSECUTIVE_WORKER_FAILURES} failures in a row.
   */
  private replace(slot: Slot): void {
    if (this.terminated || slot.lost) {
      return;
    }
    this.detach(slot);
    slot.worker.terminate();
    for (const inFlight of slot.inFlight.values()) {
      this.requeue(inFlight);
    }
    slot.inFlight.clear();
    slot.hasField = false;
    if (this.fieldPending !== null && this.fieldPending.slot === slot) {
      this.fieldPending = null;
    }
    slot.failures += 1;
    if (slot.failures >= MAX_CONSECUTIVE_WORKER_FAILURES) {
      slot.lost = true;
    } else {
      slot.worker = this.createWorker();
      this.attach(slot);
    }
    this.postNextField();
    this.pump();
  }

  /** Posts the field to the next running worker without it, unless one is loading it now. */
  private postNextField(): void {
    if (this.field === null || this.fieldPending !== null) {
      return;
    }
    const slot = this.slots.find((s) => !s.lost && !s.hasField);
    if (slot === undefined) {
      return;
    }
    const id = this.nextId;
    this.nextId += 1;
    this.fieldPending = { slot, id };
    // Cloned, not transferred: the pool keeps the field for the next worker.
    slot.worker.postMessage({ kind: "field", id, bytes: this.field }, []);
  }
}
