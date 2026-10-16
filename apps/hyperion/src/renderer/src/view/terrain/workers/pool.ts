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
 * one worker at a time.
 *
 * The pool's bakes are of one height source, its settings' (R10.T6.b). The test planet reads no
 * field, so no field ever makes its bakes stale. A body's bakes are held back until its field is
 * started, and its field alone makes them stale (per-body epochs): a bake in flight when the field
 * starts again or is released is baked again, and one in flight while blocks arrive is baked again
 * by R10's rebake rule, conservatively, since the blocks' changes may not be known yet: unless it
 * is complete, its horizon map complete, with its survey mask whole (`rebake.ts`' `staleInFlight`).
 * A bake the field cannot make yet, a survey whose margin
 * is still to come, waits for the body's next block. Each block's change of the survey is reported
 * once ({@link HeightWorkerPool.onFieldChange}), for the bakes already delivered (`rebake.ts`).
 *
 * A body's coarse class map for R07's disc (plan R10, T10.d) is asked of one worker
 * ({@link HeightWorkerPool.classMap}), after the blocks posted before it, which it reads.
 */

import type { BodyIdHex, DetailSeedHex } from "@hyperion/protocol";

import { patchKeyString } from "../patchKey";
import type { PatchRequest } from "../select";
import type {
  BakedPatch,
  BakeSettings,
  ClassMapAnswer,
  FieldBlockAnswer,
  FieldForm,
  HeightWorkerReply,
  HeightWorkerRequest,
  TerrainLayers,
} from "./messages";
import { staleInFlight } from "./rebake";

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
  /**
   * The setting's vertex path and normal resolution and the height source, sent with every bake.
   * A body's source holds its bakes back until its field is started, and makes them stale as its
   * field grows.
   */
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

/**
 * What one block of a body's payload changed in the workers' fields, reported once, from the first
 * worker to answer it: the cells it surveyed anew and the cells it brought that the fields did not
 * hold, which R10's rebake rule reads (`rebake.ts`'s `needsRebake`), and the body's level table
 * after it.
 */
export interface FieldChange extends FieldBlockAnswer {
  readonly body: BodyIdHex;
  /** The block's place among this start of the field's blocks, the first 0. */
  readonly sequence: number;
}

/** A body's field as the pool keeps it, for the workers that start later. */
interface KeptField {
  /** Its blocks in the order posted, its first block first. */
  readonly blocks: ArrayBuffer[];
  /** A payload's, which the workers decode, or a stand-in's, which they hold undecoded. */
  readonly form: FieldForm;
  /** The first message ID of this start of the field: a refusal of an older one is not its. */
  readonly sinceId: number;
  /** Whether a refusal of this start has been reported. */
  refused: boolean;
  /** How many of its blocks, from the first, have had their changes reported. */
  reported: number;
  /** The body's level table as a worker last reported it, `null` before any. */
  levels: Float64Array | null;
}

/** A block posted to a worker, by the message's ID. */
interface PostedBlock {
  readonly body: BodyIdHex;
  /** The start of the field it is of. */
  readonly field: KeptField;
  /** Its place among the field's blocks. */
  readonly sequence: number;
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
  /** The source body's field when the request was sent; `null` for the test planet, or none. */
  readonly field: KeptField | null;
  /** How many of that field's blocks had been posted when the request was sent. */
  readonly blocksAtSend: number;
}

/** A class-map query posted to a worker and not yet answered. */
interface PendingClassMap {
  readonly body: BodyIdHex;
  readonly detailSeed: DetailSeedHex;
  readonly resolve: (map: ClassMapAnswer | null) => void;
  readonly reject: (error: Error) => void;
}

interface Slot {
  worker: WorkerLike;
  readonly onMessage: (event: MessageEvent<HeightWorkerReply>) => void;
  readonly onError: () => void;
  readonly inFlight: Map<number, InFlight>;
  /** The class-map queries posted to this worker and not yet answered, by message ID. */
  readonly queries: Map<number, PendingClassMap>;
  /** The blocks posted to this worker and not yet answered, by message ID. */
  readonly posted: Map<number, PostedBlock>;
  /** What the worker last said it holds of every body's field, bytes. */
  fieldBytes: number;
  /** Failures in a row with no answer between. */
  failures: number;
  /** Given up after {@link MAX_CONSECUTIVE_WORKER_FAILURES}: no worker runs in this place. */
  lost: boolean;
}

/** A worker's requests out: its bakes and class-map queries in flight. */
function loadOf(slot: Slot): number {
  return slot.inFlight.size + slot.queries.size;
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
  private readonly changeListeners = new Set<(change: FieldChange) => void>();
  private readonly fields = new Map<BodyIdHex, KeptField>();
  /** The requests the source body's field could not make yet, until its next block. */
  private readonly waiting = new Map<string, PatchRequest>();
  /** The body whose field the bakes read, `null` for the test planet, which reads none. */
  private readonly sourceBody: BodyIdHex | null;
  private generation = 0;
  private nextId = 1;
  private terminated = false;

  constructor(opts: HeightWorkerPoolOptions) {
    if (!Number.isInteger(opts.workers) || opts.workers < 1) {
      throw new Error(`a height-worker pool needs at least one worker, not ${opts.workers}`);
    }
    this.createWorker = opts.createWorker;
    this.settings = opts.bake;
    this.sourceBody = opts.bake.source.kind === "body" ? opts.bake.source.body : null;
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
        queries: new Map(),
        posted: new Map(),
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

  /**
   * How many requests wait for a block of the source body's field, which their surveys' margins
   * are still to come in: patches waiting on a surveyed region's chunk, which R10's Design note 16
   * counts toward R05's `TERRAIN: STREAMING` (they are among the patches it finds missing; the
   * pass's first consumer reads this to tell them).
   */
  get waitingCount(): number {
    return this.waiting.size;
  }

  /**
   * The blocks posted to running workers that they have still to answer: what of the fields the
   * workers have yet to take.
   */
  get blocksInFlight(): number {
    let n = 0;
    for (const slot of this.slots) {
      n += slot.posted.size;
    }
    return n;
  }

  /**
   * `body`'s level table, 25 × 4 as the test planet's `levelTable`, as its field's latest block
   * reported it (R10.T6.b; T4.b's selection reads it); `null` before any, or with no field kept.
   */
  fieldLevels(body: BodyIdHex): Float64Array | null {
    return this.fields.get(body)?.levels ?? null;
  }

  /**
   * Queues one request, or re-scores it if queued already or waiting for a block; one in flight is
   * left alone.
   */
  request(r: PatchRequest): void {
    this.assertLive();
    const keyString = patchKeyString(r.key);
    if (this.failedKeys.has(keyString)) {
      return;
    }
    this.wanted.add(keyString);
    if (this.waiting.has(keyString)) {
      this.waiting.set(keyString, r);
    } else if (!this.isInFlight(keyString)) {
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
    const waited = new Map(this.waiting);
    this.waiting.clear();
    for (const request of demand) {
      const keyString = patchKeyString(request.key);
      if (this.failedKeys.has(keyString)) {
        continue;
      }
      this.wanted.add(keyString);
      if (waited.has(keyString)) {
        // Still waiting for the field's next block: asking again before it would be refused again.
        this.waiting.set(keyString, request);
      } else if (!this.isInFlight(keyString)) {
        this.queue.set(keyString, { request, keyString });
      }
    }
    this.pump();
  }

  /**
   * Drops every queued or waiting request, and every result yet to arrive, whose key is not in
   * `keep`.
   */
  cancelStale(keep: ReadonlySet<string>): void {
    this.assertLive();
    for (const keyString of this.queue.keys()) {
      if (!keep.has(keyString)) {
        this.queue.delete(keyString);
      }
    }
    for (const keyString of this.waiting.keys()) {
      if (!keep.has(keyString)) {
        this.waiting.delete(keyString);
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
   * Listens for each block of a payload the workers have taken, once a block, with what it changed
   * of the survey; returns the unsubscribe. The bakes delivered before it that R10's rebake rule
   * makes stale (`rebake.ts`'s `needsRebake`) are the listener's to ask again.
   */
  onFieldChange(cb: (change: FieldChange) => void): () => void {
    this.changeListeners.add(cb);
    return () => {
      this.changeListeners.delete(cb);
    };
  }

  /**
   * Starts `body`'s field in every worker from its first block, R09's block 0 of the first payload
   * (Design note 16), cloned to each; or, as a `stand-in`, a block of no payload that the workers
   * hold undecoded (the descent spike's synthetic field, R10.T7).
   *
   * @remarks
   * The pool keeps the body's blocks on the render thread, about 12 MB for an Earth (R09 Design
   * note 17), so that a worker started later, to replace a failed one, gets them too. Bakes of the
   * body in flight are baked again once they arrive; requests waiting for its field are sent.
   *
   * @throws Error if `body`'s field is already started: release it first.
   */
  fieldInit(body: BodyIdHex, block: ArrayBuffer, form: FieldForm = "payload"): void {
    this.assertLive();
    if (this.fields.has(body)) {
      throw new Error(`the field of ${body} is already started; release it first`);
    }
    const kept: KeptField = {
      blocks: [block],
      form,
      sinceId: this.nextId,
      refused: false,
      reported: 0,
      levels: null,
    };
    this.fields.set(body, kept);
    for (const slot of this.slots) {
      this.postBlock(slot, body, kept, 0);
    }
    this.afterBlock(body);
  }

  /**
   * Adds one more block to `body`'s field in every worker: any block after the first, a later
   * payload's block 0 included. Requests waiting for the body's field are sent again after it.
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
    const sequence = kept.blocks.length - 1;
    for (const slot of this.slots) {
      this.postBlock(slot, body, kept, sequence);
    }
    this.afterBlock(body);
  }

  /** Drops `body`'s field from every worker and from the pool; a body not started is no error. */
  fieldRelease(body: BodyIdHex): void {
    this.assertLive();
    if (!this.fields.delete(body)) {
      return;
    }
    if (body === this.sourceBody) {
      // Nothing waits for a field that is gone: the next demand asks again, held until it starts.
      this.waiting.clear();
    }
    for (const slot of this.slots) {
      if (!slot.lost) {
        slot.worker.postMessage({ kind: "field-release", id: this.takeId(), body }, []);
      }
    }
  }

  /**
   * `body`'s coarse class map for R07's disc (plan R10, T10.d), answered by one worker over the field
   * it holds of the body with the body's detail seed `detailSeed`: the least loaded running worker,
   * posted after every block posted so far, which it reads.
   *
   * @remarks
   * A worker that fails before it answers is replaced and the query asked again, of its replacement
   * once the fields are posted to it, or of another worker where its place is given up. A pool
   * terminated before the answer answers `null`.
   *
   * @returns The map, or `null` where the pool keeps no field of `body`, no worker runs, or the
   *   worker holds no payload field of it; rejected with the module's message where the module
   *   refused the query.
   */
  classMap(body: BodyIdHex, detailSeed: DetailSeedHex): Promise<ClassMapAnswer | null> {
    this.assertLive();
    if (!this.fields.has(body)) {
      return Promise.resolve(null);
    }
    return new Promise((resolve, reject) => {
      this.postQuery({ body, detailSeed, resolve, reject });
    });
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
      slot.posted.clear();
      for (const query of slot.queries.values()) {
        query.resolve(null);
      }
      slot.queries.clear();
    }
    this.queue.clear();
    this.waiting.clear();
    this.wanted.clear();
    this.bakedListeners.clear();
    this.failureListeners.clear();
    this.refusalListeners.clear();
    this.changeListeners.clear();
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
    // A body's patches are baked over its field, so none is sent before the field is started.
    if (this.sourceBody !== null && !this.fields.has(this.sourceBody)) {
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
    const field = this.sourceField();
    const inFlight: InFlight = {
      id: this.takeId(),
      request: next.request,
      keyString: next.keyString,
      generation: this.generation,
      field,
      blocksAtSend: field?.blocks.length ?? 0,
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
        if (this.madeStale(inFlight, reply.bake.layers)) {
          this.requeue(inFlight);
        } else if (this.wanted.has(inFlight.keyString)) {
          this.wanted.delete(inFlight.keyString);
          for (const listener of this.bakedListeners) {
            listener(reply.bake);
          }
        }
        break;
      }
      case "bake-unsurveyed": {
        const inFlight = slot.inFlight.get(reply.id);
        slot.inFlight.delete(reply.id);
        if (inFlight === undefined) {
          break;
        }
        const field = this.sourceField();
        if (
          field !== inFlight.field ||
          field === null ||
          field.blocks.length > inFlight.blocksAtSend
        ) {
          // A block came since, which may hold what it lacked, or the field started anew.
          this.requeue(inFlight);
        } else if (this.wanted.has(inFlight.keyString)) {
          this.waiting.set(inFlight.keyString, inFlight.request);
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
      case "field-held": {
        slot.fieldBytes = reply.heldBytes;
        const posted = slot.posted.get(reply.id);
        slot.posted.delete(reply.id);
        if (posted !== undefined && reply.block !== null) {
          this.report(posted, reply.block);
        }
        break;
      }
      case "class-map": {
        const query = slot.queries.get(reply.id);
        slot.queries.delete(reply.id);
        query?.resolve(reply.map);
        break;
      }
      case "class-map-failed": {
        const query = slot.queries.get(reply.id);
        slot.queries.delete(reply.id);
        query?.reject(new Error(`the class map of ${reply.body} was refused: ${reply.message}`));
        break;
      }
      case "field-refused": {
        slot.fieldBytes = reply.heldBytes;
        slot.posted.delete(reply.id);
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
    // The failed worker will never answer its blocks; a replacement is posted them afresh.
    slot.posted.clear();
    const queries = [...slot.queries.values()];
    slot.queries.clear();
    slot.fieldBytes = 0;
    slot.failures += 1;
    if (slot.failures >= MAX_CONSECUTIVE_WORKER_FAILURES) {
      slot.lost = true;
    } else {
      slot.worker = this.createWorker();
      this.attach(slot);
      for (const [body, kept] of this.fields) {
        kept.blocks.forEach((_, n) => {
          this.postBlock(slot, body, kept, n);
        });
      }
    }
    // Asked again after the fields, of the replacement or of another worker.
    for (const query of queries) {
      this.postQuery(query);
    }
    this.pump();
  }

  /** Posts a class-map query to the least loaded running worker, or answers `null` with none. */
  private postQuery(query: PendingClassMap): void {
    let slot: Slot | null = null;
    for (const each of this.slots) {
      if (!each.lost && (slot === null || loadOf(each) < loadOf(slot))) {
        slot = each;
      }
    }
    if (slot === null) {
      query.resolve(null);
      return;
    }
    const id = this.takeId();
    slot.queries.set(id, query);
    slot.worker.postMessage(
      { kind: "class-map", id, body: query.body, detailSeed: query.detailSeed },
      [],
    );
  }

  /** Posts block `sequence` of `body`'s field `kept` to a running worker. */
  private postBlock(slot: Slot, body: BodyIdHex, kept: KeptField, sequence: number): void {
    const block = kept.blocks[sequence];
    if (slot.lost || block === undefined) {
      return;
    }
    const id = this.takeId();
    slot.posted.set(id, { body, field: kept, sequence });
    // Cloned, not transferred: the pool keeps the block for the next worker.
    slot.worker.postMessage(
      sequence === 0
        ? { kind: "field-init", id, body, block, form: kept.form }
        : { kind: "field-chunk", id, body, block },
      [],
    );
  }

  /** The source body's field as kept now: `null` for the test planet, or none started. */
  private sourceField(): KeptField | null {
    return this.sourceBody === null ? null : (this.fields.get(this.sourceBody) ?? null);
  }

  /**
   * Whether a bake made after `inFlight` was sent is stale on arrival: its body's field started
   * anew or was released since, or blocks came since and R10's rebake rule may take it, being
   * incomplete, with a horizon map whose far field a block could change, or with a survey mask a
   * block could change (`rebake.ts`' `staleInFlight`). The blocks' own changes may not have been
   * answered yet, so the rule is taken conservatively here.
   */
  private madeStale(inFlight: InFlight, layers: TerrainLayers | null): boolean {
    if (this.sourceBody === null) {
      return false;
    }
    const field = this.sourceField();
    if (field !== inFlight.field) {
      return true;
    }
    if (field === null || field.blocks.length === inFlight.blocksAtSend) {
      return false;
    }
    return staleInFlight(layers);
  }

  /** After a block of `body`'s field: the requests waiting for it are asked again. */
  private afterBlock(body: BodyIdHex): void {
    if (body === this.sourceBody) {
      for (const [keyString, request] of this.waiting) {
        if (!this.queue.has(keyString) && !this.isInFlight(keyString)) {
          this.queue.set(keyString, { request, keyString });
        }
      }
      this.waiting.clear();
    }
    this.pump();
  }

  /** Reports the change of block `posted`, once, from the first worker to answer it. */
  private report(posted: PostedBlock, block: FieldBlockAnswer): void {
    const { body, field, sequence } = posted;
    if (this.fields.get(body) !== field || sequence < field.reported) {
      return;
    }
    field.reported = sequence + 1;
    field.levels = block.levels;
    const change: FieldChange = { ...block, body, sequence };
    for (const listener of this.changeListeners) {
      listener(change);
    }
  }

  /** The next message ID, shared by bakes and field messages. */
  private takeId(): number {
    const id = this.nextId;
    this.nextId += 1;
    return id;
  }
}
