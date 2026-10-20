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
 * Bakes go through lanes (plan R10, R10.T16.b; decision-r10-live-path, D2): each of a body's
 * terrain passes takes its own ({@link HeightWorkerPool.lane}), with its own bake settings (its
 * height source, vertex path and normals), demand, generation and listeners, over the pool's one
 * queue, workers and fields. The queue takes forced requests first, then the larger weighted excess
 * w × ρ ÷ τ whatever the lane (Design note 24's priority, comparable across lanes), then the older
 * lane's, then by key. A pool made with bake settings (the descent spike's) is a pool of one lane,
 * whose calls are the pool's own; one made without them (the terrain session's) holds the fields
 * and answers the class maps, and bakes through its lanes alone.
 *
 * A lane's bakes are of one height source, its settings'. The test planet reads no field, so no
 * field ever makes its bakes stale. A body's bakes are held back until its field is started, and
 * its field alone makes them stale (per-body epochs): a bake in flight when the field starts again
 * or is released is baked again, and one in flight while blocks arrive is baked again by R10's
 * rebake rule, conservatively, since the blocks' changes may not be known yet: unless it is
 * complete, its horizon map complete, with its survey mask whole (`rebake.ts`' `staleInFlight`). A
 * bake the field cannot make yet, a survey whose margin is still to come, waits for the body's next
 * block. Each block's change of the survey is reported once ({@link HeightWorkerPool.onFieldChange}),
 * for the bakes already delivered (`rebake.ts`).
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

/**
 * The default number of height workers (R05 Design note 11): clamp(⌊threads ÷ 4⌋, 1, 3), the
 * machine's budget for the terrain whatever the views and bodies.
 *
 * @remarks
 * One pool runs them: the descent spike's (as `defaultSpikeWorkers`) and `VIEW`'s terrain session's
 * (plan R10, R10.T16.a). A thread count the browser does not report as a finite number takes the
 * least, one worker.
 *
 * @param hardwareConcurrency - The machine's logical processors, `navigator.hardwareConcurrency`.
 */
export function defaultHeightWorkers(hardwareConcurrency: number): number {
  if (!Number.isFinite(hardwareConcurrency)) {
    return 1;
  }
  return Math.min(3, Math.max(1, Math.floor(hardwareConcurrency / 4)));
}

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
   * The settings of the pool's own lane, the setting's vertex path and normal resolution and the
   * height source, sent with every bake of it: a pool of one lane, whose calls are the pool's own
   * (the descent spike's). A body's source holds its bakes back until its field is started, and
   * makes them stale as its field grows.
   *
   * @remarks
   * Absent for a pool that holds fields and answers class maps and bakes through its lanes alone
   * (the terrain session's, R10.T16.a and T16.b): its {@link HeightWorkerPool.request},
   * {@link HeightWorkerPool.reprioritise} and {@link HeightWorkerPool.cancelStale} throw.
   */
  readonly bake?: BakeSettings;
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

/** One lane's bakes: its settings, demand and listeners (R10.T16.b). */
interface Lane {
  /** Its place among the lanes made, the first 0: the queue's order between equal requests. */
  readonly index: number;
  /** The settings sent with every bake of it. */
  readonly settings: BakeSettings;
  /** The body whose field its bakes read, `null` for the test planet, which reads none. */
  readonly sourceBody: BodyIdHex | null;
  readonly queue: Map<string, Queued>;
  readonly wanted: Set<string>;
  readonly failedKeys: Set<string>;
  /** The requests the source body's field could not make yet, until its next block. */
  readonly waiting: Map<string, PatchRequest>;
  readonly bakedListeners: Set<(bake: BakedPatch) => void>;
  readonly failureListeners: Set<(failure: BakeFailure) => void>;
  generation: number;
  terminated: boolean;
}

interface InFlight {
  readonly id: number;
  /** The lane it was asked through; its answer reaches that lane alone. */
  readonly lane: Lane;
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

/** A queued request with its lane, as the queue orders them. */
interface Candidate {
  readonly lane: Lane;
  readonly queued: Queued;
}

/**
 * Orders requests: forced first, then higher priority whatever the lane, then the older lane's,
 * then by key string. With one lane it is R05's order.
 */
function before(a: Candidate, b: Candidate): boolean {
  const ra = a.queued.request;
  const rb = b.queued.request;
  if (ra.forced !== rb.forced) {
    return ra.forced;
  }
  if (ra.priority !== rb.priority) {
    return ra.priority > rb.priority;
  }
  if (a.lane.index !== b.lane.index) {
    return a.lane.index < b.lane.index;
  }
  return a.queued.keyString < b.queued.keyString;
}

function newLane(index: number, settings: BakeSettings): Lane {
  return {
    index,
    settings,
    sourceBody: settings.source.kind === "body" ? settings.source.body : null,
    queue: new Map(),
    wanted: new Set(),
    failedKeys: new Set(),
    waiting: new Map(),
    bakedListeners: new Set(),
    failureListeners: new Set(),
    generation: 0,
    terminated: false,
  };
}

/**
 * One lane of a {@link HeightWorkerPool} (plan R10, R10.T16.b): a terrain pass's bakes, of its own
 * settings, over the pool's queue, workers and fields.
 *
 * @remarks
 * Its demand replaces its own alone; its bakes and failures reach its own listeners alone; and
 * terminating it drops its queued, waiting and in-flight requests (an answer still to come is
 * dropped as it arrives) and leaves the fields, the class-map queries and the other lanes. A pass
 * rebuilt after a device loss terminates its lane and takes a new one, so that no field is posted
 * again (decision-r10-live-path, D2).
 */
export interface HeightWorkerLane {
  /** The settings every bake of the lane is sent with. */
  readonly settings: BakeSettings;
  /** The lane's current generation, which each new request is tagged with. */
  readonly currentGeneration: number;
  /** The lane's requests queued, not yet sent to a worker. */
  readonly queuedCount: number;
  /**
   * How many of the lane's requests wait for a block of its body's field, which their surveys'
   * margins are still to come in: patches waiting on a surveyed region's block, which Design note
   * 16 counts toward `TERRAIN: STREAMING`.
   */
  readonly waitingCount: number;
  /**
   * Whether the pool can bake nothing more: every worker given up (`lostWorkers` equal to the
   * pool's size), R10.T16.b's fault.
   */
  readonly unavailable: boolean;
  /** Whether the lane, or its pool, was terminated. */
  readonly terminated: boolean;
  /**
   * Queues one request, or re-scores it if queued already or waiting for a block; one in flight is
   * left alone.
   *
   * @throws Error once the lane or its pool is terminated.
   */
  request(r: PatchRequest): void;
  /**
   * Replaces the lane's demand, once a frame: queues new requests, re-scores queued ones and drops
   * those no longer in it; the other lanes' are untouched. Requests in flight are untouched, and
   * their results are kept if their keys are still in the demand when they arrive.
   *
   * @throws Error once the lane or its pool is terminated.
   */
  reprioritise(demand: ReadonlyArray<PatchRequest>): void;
  /**
   * Drops every queued or waiting request of the lane, and every result yet to arrive, whose key is
   * not in `keep`.
   *
   * @throws Error once the lane or its pool is terminated.
   */
  cancelStale(keep: ReadonlySet<string>): void;
  /** Listens for the lane's bakes; returns the function that stops listening. */
  onBaked(cb: (bake: BakedPatch) => void): () => void;
  /** Listens for the lane's requests given up because their bake failed; returns the unsubscribe. */
  onFailed(cb: (failure: BakeFailure) => void): () => void;
  /**
   * Ends the lane: drops its queued, waiting and in-flight requests and its listeners, and leaves
   * the pool's fields, class-map queries and other lanes; a second call does nothing.
   */
  terminate(): void;
}

/** A listener set's adder, which returns the unsubscribe. */
function listen<T>(listeners: Set<T>, listener: T): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Tells every listener of `listeners` `value`, each whatever the one before threw: a listener that
 * throws, a bug of its own, is reported, so that the others hear and the pool's message handler
 * goes on to pump the queue (plan R10, R10.T16; the batch review's S4).
 */
function tell<T>(listeners: Iterable<(value: T) => void>, value: T, what: string): void {
  for (const listener of listeners) {
    try {
      listener(value);
    } catch (error: unknown) {
      console.error(`a listener of the height workers' ${what} failed:`, error);
    }
  }
}

/**
 * The pool of height workers that bake patches for the cache.
 *
 * @remarks
 * Each frame a terrain pass calls its lane's `reprioritise` (or, in a pool of one lane, the pool's
 * {@link HeightWorkerPool.reprioritise}) with the selection's demand (the patches not yet
 * resident); the pool hands bakes to the lane's `onBaked` listeners, which insert them into the
 * cache. A key whose bake failed is reported through `onFailed` and not requested again by that
 * lane. {@link HeightWorkerPool.terminate} stops every worker and drops every listener.
 */
export class HeightWorkerPool {
  private readonly createWorker: () => WorkerLike;
  private readonly slots: Slot[] = [];
  /** The live lanes, oldest first. */
  private readonly lanes: Lane[] = [];
  /** The pool's own lane, of the settings it was made with; `null` for a pool of lanes alone. */
  private readonly own: Lane | null;
  private readonly refusalListeners = new Set<(refusal: FieldRefusal) => void>();
  private readonly changeListeners = new Set<(change: FieldChange) => void>();
  private readonly fields = new Map<BodyIdHex, KeptField>();
  private nextLane = 0;
  private nextId = 1;
  private terminated = false;

  constructor(opts: HeightWorkerPoolOptions) {
    if (!Number.isInteger(opts.workers) || opts.workers < 1) {
      throw new Error(`a height-worker pool needs at least one worker, not ${opts.workers}`);
    }
    this.createWorker = opts.createWorker;
    this.own = opts.bake === undefined ? null : this.addLane(opts.bake);
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

  /** The current generation of the pool's own lane, which every new request is tagged with. */
  get currentGeneration(): number {
    return this.own?.generation ?? 0;
  }

  /** How many requests are queued, not yet sent to a worker, over every lane. */
  get queuedCount(): number {
    let n = 0;
    for (const lane of this.lanes) {
      n += lane.queue.size;
    }
    return n;
  }

  /** How many requests are in flight, over every worker. */
  get inFlightCount(): number {
    let n = 0;
    for (const slot of this.slots) {
      n += slot.inFlight.size;
    }
    return n;
  }

  /** How many workers the pool runs, given up or not: its size. */
  get workerCount(): number {
    return this.slots.length;
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
   * The pool's own copy of every body's field together, bytes: each kept field's blocks, held on
   * the render thread for a worker that replaces a failed one (R10.T7's "The pool's own copy"; R10.T14
   * and R12 Design note 6 state it beside {@link HeightWorkerPool.fieldResidentBytes}).
   */
  get keptFieldBytes(): number {
    let bytes = 0;
    for (const kept of this.fields.values()) {
      for (const block of kept.blocks) {
        bytes += block.byteLength;
      }
    }
    return bytes;
  }

  /**
   * How many requests wait for a block of their source body's field, which their surveys' margins
   * are still to come in, over every lane: patches waiting on a surveyed region's chunk, which R10's
   * Design note 16 counts toward R05's `TERRAIN: STREAMING`.
   */
  get waitingCount(): number {
    let n = 0;
    for (const lane of this.lanes) {
      n += lane.waiting.size;
    }
    return n;
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
   * How many of this start of `body`'s field's blocks are posted, and how many of them have had
   * their changes reported ({@link HeightWorkerPool.onFieldChange}), in order from the first;
   * `null` with no field kept (R10.T16.b: the session holds a coverage back until the workers have
   * taken the blocks posted before it).
   */
  fieldBlocks(body: BodyIdHex): { readonly posted: number; readonly reported: number } | null {
    const kept = this.fields.get(body);
    return kept === undefined ? null : { posted: kept.blocks.length, reported: kept.reported };
  }

  /**
   * A lane of bakes of `bake`'s settings over the pool's queue, workers and fields (R10.T16.b): a
   * terrain pass's, which it terminates when it is rebuilt or disposed.
   *
   * @throws Error once the pool is terminated.
   */
  lane(bake: BakeSettings): HeightWorkerLane {
    this.assertLive();
    const lane = this.addLane(bake);
    // The object's getters bind `this` to the object, so the pool's state is read through arrows.
    const unavailable = (): boolean => this.lostWorkers === this.workerCount;
    return {
      settings: bake,
      get currentGeneration(): number {
        return lane.generation;
      },
      get queuedCount(): number {
        return lane.queue.size;
      },
      get waitingCount(): number {
        return lane.waiting.size;
      },
      get unavailable(): boolean {
        return unavailable();
      },
      get terminated(): boolean {
        return lane.terminated;
      },
      request: (r) => {
        this.laneRequest(lane, r);
      },
      reprioritise: (demand) => {
        this.laneReprioritise(lane, demand);
      },
      cancelStale: (keep) => {
        this.laneCancelStale(lane, keep);
      },
      onBaked: (cb) => listen(lane.bakedListeners, cb),
      onFailed: (cb) => listen(lane.failureListeners, cb),
      terminate: () => {
        this.dropLane(lane);
      },
    };
  }

  /**
   * Queues one request of the pool's own lane, or re-scores it if queued already or waiting for a
   * block; one in flight is left alone.
   *
   * @throws Error once the pool is terminated, or for a pool made without bake settings.
   */
  request(r: PatchRequest): void {
    this.laneRequest(this.ownLane(), r);
  }

  /**
   * Replaces the pool's own lane's demand, once a frame: queues new requests, re-scores queued ones
   * and drops those no longer in the demand. Requests in flight are untouched, and their results are
   * kept if their keys are still in the demand when they arrive.
   *
   * @throws Error once the pool is terminated, or for a pool made without bake settings.
   */
  reprioritise(demand: ReadonlyArray<PatchRequest>): void {
    this.laneReprioritise(this.ownLane(), demand);
  }

  /**
   * Drops every queued or waiting request of the pool's own lane, and every result yet to arrive,
   * whose key is not in `keep`.
   *
   * @throws Error once the pool is terminated, or for a pool made without bake settings.
   */
  cancelStale(keep: ReadonlySet<string>): void {
    this.laneCancelStale(this.ownLane(), keep);
  }

  /**
   * Listens for the bakes of the pool's own lane; returns the function that stops listening. A pool
   * made without bake settings has none to tell of.
   */
  onBaked(cb: (bake: BakedPatch) => void): () => void {
    return this.own === null ? () => undefined : listen(this.own.bakedListeners, cb);
  }

  /** Listens for the own lane's requests given up because their bake failed; returns the unsubscribe. */
  onFailed(cb: (failure: BakeFailure) => void): () => void {
    return this.own === null ? () => undefined : listen(this.own.failureListeners, cb);
  }

  /** Listens for a body's field refused by a worker; returns the unsubscribe. */
  onFieldRefused(cb: (refusal: FieldRefusal) => void): () => void {
    return listen(this.refusalListeners, cb);
  }

  /**
   * Listens for each block of a payload the workers have taken, once a block, with what it changed
   * of the survey; returns the unsubscribe. The bakes delivered before it that R10's rebake rule
   * makes stale (`rebake.ts`'s `needsRebake`) are the listener's to ask again.
   */
  onFieldChange(cb: (change: FieldChange) => void): () => void {
    return listen(this.changeListeners, cb);
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
    for (const lane of this.lanes) {
      if (lane.sourceBody === body) {
        // Nothing waits for a field that is gone: the next demand asks again, held until it starts.
        lane.waiting.clear();
      }
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

  /** Stops every worker and drops every lane and listener; the pool cannot be used afterwards. */
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
    for (const lane of this.lanes) {
      this.clearLane(lane);
    }
    this.lanes.length = 0;
    this.refusalListeners.clear();
    this.changeListeners.clear();
    this.fields.clear();
  }

  /** {@link HeightWorkerLane.request} of `lane`. */
  private laneRequest(lane: Lane, r: PatchRequest): void {
    this.assertLane(lane);
    const keyString = patchKeyString(r.key);
    if (lane.failedKeys.has(keyString)) {
      return;
    }
    lane.wanted.add(keyString);
    if (lane.waiting.has(keyString)) {
      lane.waiting.set(keyString, r);
    } else if (!this.isInFlight(lane, keyString)) {
      lane.queue.set(keyString, { request: r, keyString });
    }
    this.pump();
  }

  /** {@link HeightWorkerLane.reprioritise} of `lane`. */
  private laneReprioritise(lane: Lane, demand: ReadonlyArray<PatchRequest>): void {
    this.assertLane(lane);
    lane.generation += 1;
    lane.wanted.clear();
    lane.queue.clear();
    const waited = new Map(lane.waiting);
    lane.waiting.clear();
    for (const request of demand) {
      const keyString = patchKeyString(request.key);
      if (lane.failedKeys.has(keyString)) {
        continue;
      }
      lane.wanted.add(keyString);
      if (waited.has(keyString)) {
        // Still waiting for the field's next block: asking again before it would be refused again.
        lane.waiting.set(keyString, request);
      } else if (!this.isInFlight(lane, keyString)) {
        lane.queue.set(keyString, { request, keyString });
      }
    }
    this.pump();
  }

  /** {@link HeightWorkerLane.cancelStale} of `lane`. */
  private laneCancelStale(lane: Lane, keep: ReadonlySet<string>): void {
    this.assertLane(lane);
    for (const keyString of lane.queue.keys()) {
      if (!keep.has(keyString)) {
        lane.queue.delete(keyString);
      }
    }
    for (const keyString of lane.waiting.keys()) {
      if (!keep.has(keyString)) {
        lane.waiting.delete(keyString);
      }
    }
    for (const keyString of lane.wanted) {
      if (!keep.has(keyString)) {
        lane.wanted.delete(keyString);
      }
    }
  }

  private addLane(bake: BakeSettings): Lane {
    const lane = newLane(this.nextLane, bake);
    this.nextLane += 1;
    this.lanes.push(lane);
    return lane;
  }

  /** Ends `lane`: its requests and listeners dropped; its answers still to come are dropped too. */
  private dropLane(lane: Lane): void {
    if (lane.terminated) {
      return;
    }
    this.clearLane(lane);
    const at = this.lanes.indexOf(lane);
    if (at >= 0) {
      this.lanes.splice(at, 1);
    }
    this.pump();
  }

  private clearLane(lane: Lane): void {
    lane.terminated = true;
    lane.queue.clear();
    lane.waiting.clear();
    lane.wanted.clear();
    lane.bakedListeners.clear();
    lane.failureListeners.clear();
  }

  private assertLive(): void {
    if (this.terminated) {
      throw new Error("the height-worker pool was terminated");
    }
  }

  private assertLane(lane: Lane): void {
    this.assertLive();
    if (lane.terminated) {
      throw new Error("the height-worker lane was terminated");
    }
  }

  /** The pool's own lane, of a pool made with bake settings. */
  private ownLane(): Lane {
    this.assertLive();
    if (this.own === null) {
      throw new Error(
        "the height-worker pool was made without bake settings: it holds fields and answers class maps, and bakes through its lanes alone",
      );
    }
    return this.own;
  }

  private isInFlight(lane: Lane, keyString: string): boolean {
    for (const slot of this.slots) {
      for (const inFlight of slot.inFlight.values()) {
        if (inFlight.lane === lane && inFlight.keyString === keyString) {
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

  private send(slot: Slot, next: Candidate): void {
    const { lane, queued } = next;
    lane.queue.delete(queued.keyString);
    const field = this.sourceField(lane);
    const inFlight: InFlight = {
      id: this.takeId(),
      lane,
      request: queued.request,
      keyString: queued.keyString,
      generation: lane.generation,
      field,
      blocksAtSend: field?.blocks.length ?? 0,
    };
    slot.inFlight.set(inFlight.id, inFlight);
    slot.worker.postMessage(
      {
        kind: "bake",
        id: inFlight.id,
        key: queued.request.key,
        generation: inFlight.generation,
        settings: lane.settings,
      },
      [],
    );
  }

  /** The best queued request over every lane whose bakes can be made now. */
  private best(): Candidate | null {
    let best: Candidate | null = null;
    for (const lane of this.lanes) {
      // A body's patches are baked over its field, so none is sent before the field is started.
      if (lane.sourceBody !== null && !this.fields.has(lane.sourceBody)) {
        continue;
      }
      for (const queued of lane.queue.values()) {
        const candidate = { lane, queued };
        if (best === null || before(candidate, best)) {
          best = candidate;
        }
      }
    }
    return best;
  }

  /** Puts a request back in its lane's queue if its key is still wanted and not queued already. */
  private requeue(inFlight: InFlight): void {
    const { lane } = inFlight;
    if (
      !lane.terminated &&
      lane.wanted.has(inFlight.keyString) &&
      !lane.queue.has(inFlight.keyString)
    ) {
      lane.queue.set(inFlight.keyString, {
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
        if (inFlight === undefined || inFlight.lane.terminated) {
          break;
        }
        const { lane } = inFlight;
        if (this.madeStale(inFlight, reply.bake.layers)) {
          this.requeue(inFlight);
        } else if (lane.wanted.has(inFlight.keyString)) {
          lane.wanted.delete(inFlight.keyString);
          tell(lane.bakedListeners, reply.bake, "bakes");
        }
        break;
      }
      case "bake-unsurveyed": {
        const inFlight = slot.inFlight.get(reply.id);
        slot.inFlight.delete(reply.id);
        if (inFlight === undefined || inFlight.lane.terminated) {
          break;
        }
        const field = this.sourceField(inFlight.lane);
        if (
          field !== inFlight.field ||
          field === null ||
          field.blocks.length > inFlight.blocksAtSend
        ) {
          // A block came since, which may hold what it lacked, or the field started anew.
          this.requeue(inFlight);
        } else if (inFlight.lane.wanted.has(inFlight.keyString)) {
          inFlight.lane.waiting.set(inFlight.keyString, inFlight.request);
        }
        break;
      }
      case "bake-failed": {
        const inFlight = slot.inFlight.get(reply.id);
        slot.inFlight.delete(reply.id);
        if (inFlight !== undefined && !inFlight.lane.terminated) {
          const { lane } = inFlight;
          lane.wanted.delete(inFlight.keyString);
          lane.failedKeys.add(inFlight.keyString);
          const failure = { keyString: inFlight.keyString, message: reply.message };
          tell(lane.failureListeners, failure, "failed bakes");
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
          tell(this.refusalListeners, refusal, "refused fields");
        }
        break;
      }
    }
    this.pump();
  }

  /**
   * Replaces a failed worker, re-queueing its requests still wanted in their lanes and re-posting
   * every body's field, or gives its place up after {@link MAX_CONSECUTIVE_WORKER_FAILURES}
   * failures in a row.
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

  /** `lane`'s source body's field as kept now: `null` for the test planet, or none started. */
  private sourceField(lane: Lane): KeptField | null {
    return lane.sourceBody === null ? null : (this.fields.get(lane.sourceBody) ?? null);
  }

  /**
   * Whether a bake made after `inFlight` was sent is stale on arrival: its body's field started
   * anew or was released since, or blocks came since and R10's rebake rule may take it, being
   * incomplete, with a horizon map whose far field a block could change, or with a survey mask a
   * block could change (`rebake.ts`' `staleInFlight`). The blocks' own changes may not have been
   * answered yet, so the rule is taken conservatively here.
   */
  private madeStale(inFlight: InFlight, layers: TerrainLayers | null): boolean {
    if (inFlight.lane.sourceBody === null) {
      return false;
    }
    const field = this.sourceField(inFlight.lane);
    if (field !== inFlight.field) {
      return true;
    }
    if (field === null || field.blocks.length === inFlight.blocksAtSend) {
      return false;
    }
    return staleInFlight(layers);
  }

  /** After a block of `body`'s field: the requests of its lanes waiting for it are asked again. */
  private afterBlock(body: BodyIdHex): void {
    for (const lane of this.lanes) {
      if (lane.sourceBody !== body) {
        continue;
      }
      for (const [keyString, request] of lane.waiting) {
        if (!lane.queue.has(keyString) && !this.isInFlight(lane, keyString)) {
          lane.queue.set(keyString, { request, keyString });
        }
      }
      lane.waiting.clear();
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
    tell(this.changeListeners, change, "field changes");
  }

  /** The next message ID, shared by bakes and field messages. */
  private takeId(): number {
    const id = this.nextId;
    this.nextId += 1;
    return id;
  }
}
