/**
 * The height-worker pool: one priority queue on the render thread feeding a few module workers,
 * each with its own WebAssembly instance of `hyperion-surface` (plan R05, T10.a, Design note 11).
 *
 * @remarks
 * Each worker has at most two requests in flight, so that it never idles between bakes while
 * cancelling stays cheap: a queued request is cancelled by removing it, and priorities are re-scored
 * every frame. Per-worker queues were rejected, because their cancellations race the bakes. A
 * result whose generation is stale is still delivered if its key is still wanted, and dropped
 * otherwise.
 *
 * A body's coarse field reaches the workers block by block (plan R10, T7, Design note 16):
 * {@link HeightWorkerPool.fieldInit} posts its first block, {@link HeightWorkerPool.fieldChunk}
 * each later one and {@link HeightWorkerPool.fieldRelease} drops it, each to every worker at once.
 * A worker takes its messages in the order sent, so a bake posted after a block is made with it,
 * and the transient copies are one block, at most 1 MiB, a worker, where R05's one field went to
 * one worker at a time. A bake made before a body's field was started is baked again.
 */

import type { BodyIdHex } from "@hyperion/protocol";

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

/** A block of a body's field that a worker refused, reported once for each start of the field. */
export interface FieldRefusal {
  readonly body: BodyIdHex;
  readonly message: string;
}

/** A body's field as the pool keeps it, for the workers that start later. */
interface KeptField {
  /** Its blocks in the order posted, its first block first. */
  readonly blocks: ArrayBuffer[];
  /** The first message ID of this start of the field: a refusal of an older one is not its. */
  readonly sinceId: number;
  /** Whether a refusal of this start has been reported. */
  refused: boolean;
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
  /** How many fields had been started when the request was sent. */
  readonly fieldEpoch: number;
}

interface Slot {
  worker: WorkerLike;
  readonly onMessage: (event: MessageEvent<HeightWorkerReply>) => void;
  readonly onError: () => void;
  readonly inFlight: Map<number, InFlight>;
  /** What the worker last said it holds of every body's field, bytes. */
  fieldBytes: number;
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
  private readonly refusalListeners = new Set<(refusal: FieldRefusal) => void>();
  private readonly fields = new Map<BodyIdHex, KeptField>();
  private fieldEpoch = 0;
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
        fieldBytes: 0,
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

  /**
   * The workers' copies of every body's field together, bytes, as each running worker last
   * reported its own (R10.T14; R12 Design note 6's line for the field in the workers).
   */
  get fieldResidentBytes(): number {
    let bytes = 0;
    for (const slot of this.slots) {
      bytes += slot.lost ? 0 : slot.fieldBytes;
    }
    return bytes;
  }

  /** The bodies whose fields the pool keeps, in the order they were started. */
  get fieldBodies(): ReadonlyArray<BodyIdHex> {
    return [...this.fields.keys()];
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

  /** Listens for a body's field refused by a worker; returns the unsubscribe. */
  onFieldRefused(cb: (refusal: FieldRefusal) => void): () => void {
    this.refusalListeners.add(cb);
    return () => {
      this.refusalListeners.delete(cb);
    };
  }

  /**
   * Starts `body`'s field in every worker from its first block, R09's block 0 of the first payload
   * (Design note 16), cloned to each.
   *
   * @remarks
   * The pool keeps the body's blocks on the render thread, about 12 MB for an Earth (R09 Design
   * note 17), so that a worker started later, to replace a failed one, gets them too. Bakes in
   * flight are baked again once they arrive, as R05's were after a new field.
   *
   * @throws Error if `body`'s field is already started: release it first.
   */
  fieldInit(body: BodyIdHex, block: ArrayBuffer): void {
    this.assertLive();
    if (this.fields.has(body)) {
      throw new Error(`the field of ${body} is already started; release it first`);
    }
    this.fields.set(body, { blocks: [block], sinceId: this.nextId, refused: false });
    this.fieldEpoch += 1;
    for (const slot of this.slots) {
      this.postBlock(slot, "field-init", body, block);
    }
  }

  /**
   * Adds one more block to `body`'s field in every worker: any block after the first, a later
   * payload's block 0 included.
   *
   * @throws Error if `body`'s field was not started with {@link HeightWorkerPool.fieldInit}.
   */
  fieldChunk(body: BodyIdHex, block: ArrayBuffer): void {
    this.assertLive();
    const kept = this.fields.get(body);
    if (kept === undefined) {
      throw new Error(`a block of ${body} was posted before its field was started`);
    }
    kept.blocks.push(block);
    for (const slot of this.slots) {
      this.postBlock(slot, "field-chunk", body, block);
    }
  }

  /** Drops `body`'s field from every worker and from the pool; a body not started is no error. */
  fieldRelease(body: BodyIdHex): void {
    this.assertLive();
    if (!this.fields.delete(body)) {
      return;
    }
    for (const slot of this.slots) {
      if (!slot.lost) {
        slot.worker.postMessage({ kind: "field-release", id: this.takeId(), body }, []);
      }
    }
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
    this.refusalListeners.clear();
    this.fields.clear();
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
        if (slot.inFlight.size !== load || slot.lost) {
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
      id: this.takeId(),
      request: next.request,
      keyString: next.keyString,
      generation: this.generation,
      fieldEpoch: this.fieldEpoch,
    };
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
      case "field-held":
        slot.fieldBytes = reply.heldBytes;
        break;
      case "field-refused": {
        slot.fieldBytes = reply.heldBytes;
        const kept = this.fields.get(reply.body);
        if (kept !== undefined && reply.id >= kept.sinceId && !kept.refused) {
          kept.refused = true;
          const refusal = { body: reply.body, message: reply.message };
          for (const listener of this.refusalListeners) {
            listener(refusal);
          }
        }
        break;
      }
    }
    this.pump();
  }

  /**
   * Replaces a failed worker, re-queueing its requests still wanted and re-posting every body's
   * field, or gives its place up after {@link MAX_CONSECUTIVE_WORKER_FAILURES} failures in a row.
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
    slot.fieldBytes = 0;
    slot.failures += 1;
    if (slot.failures >= MAX_CONSECUTIVE_WORKER_FAILURES) {
      slot.lost = true;
    } else {
      slot.worker = this.createWorker();
      this.attach(slot);
      for (const [body, kept] of this.fields) {
        kept.blocks.forEach((block, n) => {
          this.postBlock(slot, n === 0 ? "field-init" : "field-chunk", body, block);
        });
      }
    }
    this.pump();
  }

  /** Posts one block of `body`'s field to a running worker. */
  private postBlock(
    slot: Slot,
    kind: "field-init" | "field-chunk",
    body: BodyIdHex,
    block: ArrayBuffer,
  ): void {
    if (slot.lost) {
      return;
    }
    // Cloned, not transferred: the pool keeps the block for the next worker.
    slot.worker.postMessage({ kind, id: this.takeId(), body, block }, []);
  }

  /** The next message ID, shared by bakes and field messages. */
  private takeId(): number {
    const id = this.nextId;
    this.nextId += 1;
    return id;
  }
}
