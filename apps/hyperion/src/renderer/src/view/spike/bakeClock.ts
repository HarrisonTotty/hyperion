/**
 * The bake-time clock at the descent spike's height-worker boundary (plan R10, R10.T14.b;
 * decision-r10-t14b-measure §6(c) and §6(e)): each bake's post-to-reply time, an estimate of the
 * worker's own time on it, the bakes a second, and the replies never made resident, for the
 * streaming figures per segment and the size of T8.c's first lever.
 *
 * @remarks
 * It wraps each `WorkerLike` the spike's terrain session starts, as the views check's
 * `countedWorker` does (`displays/view/check/terrainProbe.ts`), and changes nothing the pool sends
 * or hears: every message is forwarded as it is, and the pool (`workers/pool.ts`) and the workers
 * are untouched. Its own listener is added to the worker as the worker is wrapped, before the pool
 * adds any, so that a reply's time is read before the pool answers it with the next post.
 *
 * A worker answers its messages one at a time, in the order they were posted (`height.worker.ts`
 * awaits its module's load, a settled promise after the first, then answers each synchronously),
 * and every message has a reply. So a bake starts no earlier than its post and no earlier than the
 * same worker's previous reply of any kind; reply − max(post, previous reply) estimates the
 * worker's time on it, the messages' passage and the render thread's lateness in hearing the reply
 * included.
 *
 * The flight's figures count the bakes posted since {@link BakeClock.markFlightStart}, so that the
 * measurement's own pre-flight bakes are left out, and every bake when it is never called.
 */

import type { HeightWorkerReply, HeightWorkerRequest } from "../terrain/workers/messages";
import type { WorkerLike } from "../terrain/workers/pool";

/** The 50th and 95th percentiles (nearest rank) and the largest of some times, ms; null with none. */
export interface MsSpread {
  readonly p50: number | null;
  readonly p95: number | null;
  readonly max: number | null;
}

/** The value at fraction `p` of `sorted` (ascending, not empty), by nearest rank. */
function nearestRank(sorted: ReadonlyArray<number>, p: number): number {
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(p * sorted.length) - 1))] ?? 0;
}

/**
 * The {@link MsSpread} of `values`, ms, by nearest rank (the spike's convention, `fixedStep.ts`'
 * spreads); nulls with no values.
 */
export function msSpread(values: ReadonlyArray<number>): MsSpread {
  if (values.length === 0) {
    return { p50: null, p95: null, max: null };
  }
  const sorted = values.toSorted((a, b) => a - b);
  return {
    p50: nearestRank(sorted, 0.5),
    p95: nearestRank(sorted, 0.95),
    max: sorted.at(-1) ?? null,
  };
}

/** What a {@link BakeClock} reads besides the messages. */
export interface BakeClockOptions {
  /** The clock, ms; `performance.now()` by default. */
  readonly nowMs?: () => number;
  /**
   * The script's segment now, `null` outside one, read at each post, reply and resident; absent,
   * the clock gives no per-segment figures.
   */
  readonly segment?: () => string | null;
}

/** The kinds of message the pool posts a height worker. */
export type BakeClockRequestKind = HeightWorkerRequest["kind"];

/** The kinds of reply a height worker posts the pool. */
export type BakeClockReplyKind = HeightWorkerReply["kind"];

/** How a bake was answered: baked, refused by the module, or not yet bakeable over the survey. */
type BakeOutcome = "baked" | "failed" | "unsurveyed";

/** The bakes of a window or a worker and their times. */
export interface BakeTimes {
  /** Bake requests posted. */
  readonly posted: number;
  /** Answered with a bake (`baked`). */
  readonly baked: number;
  /** Answered as failed (`bake-failed`). */
  readonly failed: number;
  /** Answered as not yet bakeable, a surveyed vertex's cell still to come (`bake-unsurveyed`). */
  readonly unsurveyed: number;
  /** The baked replies' times from post to reply, ms. */
  readonly postToReplyMs: MsSpread;
  /** The baked replies' estimated worker times, reply − max(post, the worker's previous reply), ms. */
  readonly computeMs: MsSpread;
}

/** One worker's bakes, the worker numbered from 0 in the order the clock wrapped them. */
export interface WorkerBakeTimes extends BakeTimes {
  readonly worker: number;
}

/** The bakes of the flight or of one segment, with their rate and the residents. */
export interface BakeWindow extends BakeTimes {
  /** The patches made resident ({@link BakeClock.noteResident}). */
  readonly residents: number;
  /**
   * Baked replies never made resident, baked − residents (§6(c)): the bakes the pass dropped, stale
   * or no longer wanted. Below 0 where bakes posted before the window became resident in it.
   */
  readonly bakedNeverResident: number;
  /** The window's span, ms: from the flight's start, or a segment's first event, to its last event. */
  readonly spanMs: number;
  /** Baked replies a second over {@link BakeWindow.spanMs}, or null over no span. */
  readonly bakesPerSecond: number | null;
}

/** One segment's bakes: posts by their segment, replies and residents by theirs. */
export interface SegmentBakes extends BakeWindow {
  readonly segment: string | null;
}

/** The clock's figures. */
export interface BakeClockSummary {
  /** The workers wrapped, restarts included. */
  readonly workersMade: number;
  /** When the flight started, ms on the clock, or null where it was never marked. */
  readonly flightStartMs: number | null;
  /** The flight's bakes: those posted since its start. */
  readonly total: BakeWindow;
  /** Each worker's flight bakes, in the order the workers were wrapped. */
  readonly byWorker: ReadonlyArray<WorkerBakeTimes>;
  /**
   * Each segment's flight bakes, in the order the segments were first met, those with no post,
   * reply or resident in the flight left out; none without a segment callback.
   */
  readonly bySegment: ReadonlyArray<SegmentBakes>;
  /** Every message posted since the flight's start, by kind, bakes among them. */
  readonly requests: Readonly<Record<BakeClockRequestKind, number>>;
  /** Every reply heard since the flight's start, by kind. */
  readonly replies: Readonly<Record<BakeClockReplyKind, number>>;
  /** Flight bakes posted and not answered, on workers still live. */
  readonly inFlight: number;
  /** Flight bakes in flight when their worker failed or was terminated, which the pool asks again. */
  readonly abandoned: number;
}

/** A bake posted and not yet answered. */
interface PostedBake {
  readonly postMs: number;
  readonly flight: number;
  readonly segment: string | null;
}

/** A bake answered. */
interface AnsweredBake {
  readonly worker: number;
  readonly flight: number;
  readonly outcome: BakeOutcome;
  readonly replyMs: number;
  readonly postToReplyMs: number;
  readonly computeMs: number;
  /** The segment of its reply. */
  readonly segment: string | null;
}

/** A post or a resident, by its flight and segment. */
interface Noted {
  readonly flight: number;
  readonly atMs: number;
  readonly segment: string | null;
}

/** A bake's post, by its worker too. */
interface Post extends Noted {
  readonly worker: number;
}

/** One wrapped worker's state. */
interface WorkerState {
  readonly index: number;
  /** Its bakes in flight, by request ID. */
  readonly inFlight: Map<number, PostedBake>;
  /** Its last reply of any kind, ms, or null before the first. */
  lastReplyMs: number | null;
  live: boolean;
}

function noRequests(): Record<BakeClockRequestKind, number> {
  return {
    bake: 0,
    "field-init": 0,
    "field-chunk": 0,
    "field-release": 0,
    "class-map": 0,
    "ground-readout": 0,
  };
}

function noReplies(): Record<BakeClockReplyKind, number> {
  return {
    baked: 0,
    "bake-failed": 0,
    "bake-unsurveyed": 0,
    "field-held": 0,
    "field-refused": 0,
    "class-map": 0,
    "class-map-failed": 0,
    "ground-readout": 0,
    "ground-readout-failed": 0,
  };
}

/**
 * The bake-time wrapper at the spike's worker boundary (decision-r10-t14b-measure §6(c), §6(e)).
 *
 * @example
 * ```ts
 * const clock = new BakeClock({ segment: () => currentSegment });
 * const height = (): WorkerLike => clock.wrap(browserHeightWorker());
 * // … once the pre-flight bakes are in:
 * clock.markFlightStart();
 * pass.observe({ onResident: () => clock.noteResident() });
 * ```
 */
export class BakeClock {
  readonly #now: () => number;
  readonly #segment: (() => string | null) | null;
  readonly #workers: WorkerState[] = [];
  readonly #answered: AnsweredBake[] = [];
  readonly #posts: Post[] = [];
  readonly #residents: Noted[] = [];
  /** The flight's number: 0 until the first mark, then one more at each. */
  #flight = 0;
  #flightStartMs: number | null = null;
  #requests = noRequests();
  #replies = noReplies();
  /** The flight of each bake abandoned in flight. */
  readonly #abandoned: number[] = [];
  /** The segments in the order first met. */
  readonly #segments: Array<string | null> = [];

  constructor(options: BakeClockOptions = {}) {
    this.#now = options.nowMs ?? (() => performance.now());
    this.#segment = options.segment ?? null;
  }

  /**
   * `inner`, its bake requests and their replies timed; every message forwarded unchanged.
   *
   * @remarks
   * Call it in the factory the pool calls (`HeightWorkerPoolOptions.createWorker`): a restarted
   * worker is a new one to the clock.
   */
  wrap(inner: WorkerLike): WorkerLike {
    const state: WorkerState = {
      index: this.#workers.length,
      inFlight: new Map(),
      lastReplyMs: null,
      live: true,
    };
    this.#workers.push(state);
    // Added before the pool's own, so that each reply is timed before the pool posts the next.
    inner.addEventListener("message", (event: MessageEvent<HeightWorkerReply>) => {
      this.#replied(state, event.data);
    });
    inner.addEventListener("error", () => {
      this.#abandon(state);
    });
    return {
      addEventListener: inner.addEventListener.bind(inner),
      removeEventListener: inner.removeEventListener.bind(inner),
      postMessage: (message: HeightWorkerRequest, transfer: Transferable[]): void => {
        this.#posted(state, message);
        inner.postMessage(message, transfer);
      },
      terminate: (): void => {
        this.#abandon(state);
        state.live = false;
        inner.terminate();
      },
    };
  }

  /**
   * Starts the flight now: from here on the figures count only the bakes posted after this, and the
   * messages, replies and residents heard after it.
   */
  markFlightStart(): void {
    this.#flight += 1;
    this.#flightStartMs = this.#now();
    this.#requests = noRequests();
    this.#replies = noReplies();
  }

  /**
   * A patch was made resident (a pass's `onResident`), for the replies never made resident; the
   * spike calls it for every pass the pool bakes for.
   */
  noteResident(): void {
    this.#residents.push({ flight: this.#flight, atMs: this.#now(), segment: this.#segmentNow() });
  }

  /**
   * The flight's baked replies in each window of `windowMs` from its start (or the first reply),
   * oldest first, through the window of the last reply.
   *
   * @throws RangeError for a window that is not finite and above 0.
   */
  bakedPerWindow(windowMs: number): number[] {
    if (!(Number.isFinite(windowMs) && windowMs > 0)) {
      throw new RangeError(`a bake window must be finite and above 0 ms, got ${windowMs}`);
    }
    const baked = this.#flightAnswers().filter((a) => a.outcome === "baked");
    const startMs = this.#flightStartMs ?? baked[0]?.replyMs ?? 0;
    const windows: number[] = [];
    for (const answer of baked) {
      const n = Math.max(0, Math.floor((answer.replyMs - startMs) / windowMs));
      while (windows.length <= n) {
        windows.push(0);
      }
      windows[n] = (windows[n] ?? 0) + 1;
    }
    return windows;
  }

  /** The clock's figures so far. */
  summary(): BakeClockSummary {
    const answers = this.#flightAnswers();
    const posts = this.#posts.filter((p) => p.flight === this.#flight);
    const residents = this.#residents.filter((r) => r.flight === this.#flight);
    const [firstMs, lastMs] = spanOf(answers, posts, residents);
    const total = windowOf(
      answers,
      posts.length,
      residents.length,
      this.#flightStartMs ?? firstMs,
      lastMs,
    );
    const bySegment: SegmentBakes[] = [];
    for (const segment of this.#segment === null ? [] : this.#segments) {
      const segmentAnswers = answers.filter((a) => a.segment === segment);
      const segmentPosts = posts.filter((p) => p.segment === segment);
      const segmentResidents = residents.filter((r) => r.segment === segment);
      // A segment met only before the flight's start has none of its figures.
      if (segmentAnswers.length + segmentPosts.length + segmentResidents.length > 0) {
        bySegment.push({
          segment,
          ...windowOf(
            segmentAnswers,
            segmentPosts.length,
            segmentResidents.length,
            ...spanOf(segmentAnswers, segmentPosts, segmentResidents),
          ),
        });
      }
    }
    let inFlight = 0;
    for (const worker of this.#workers) {
      if (worker.live) {
        for (const posted of worker.inFlight.values()) {
          inFlight += posted.flight === this.#flight ? 1 : 0;
        }
      }
    }
    return {
      workersMade: this.#workers.length,
      flightStartMs: this.#flightStartMs,
      total,
      byWorker: this.#workers.map((worker) => ({
        worker: worker.index,
        ...timesOf(
          answers.filter((a) => a.worker === worker.index),
          posts.filter((p) => p.worker === worker.index).length,
        ),
      })),
      bySegment,
      requests: { ...this.#requests },
      replies: { ...this.#replies },
      inFlight,
      abandoned: this.#abandoned.filter((flight) => flight === this.#flight).length,
    };
  }

  /** The answers to the flight's bakes. */
  #flightAnswers(): AnsweredBake[] {
    return this.#answered.filter((a) => a.flight === this.#flight);
  }

  #segmentNow(): string | null {
    if (this.#segment === null) {
      return null;
    }
    const segment = this.#segment();
    if (!this.#segments.includes(segment)) {
      this.#segments.push(segment);
    }
    return segment;
  }

  #posted(state: WorkerState, message: HeightWorkerRequest): void {
    this.#requests[message.kind] += 1;
    if (message.kind !== "bake") {
      return;
    }
    const postMs = this.#now();
    const segment = this.#segmentNow();
    state.inFlight.set(message.id, { postMs, flight: this.#flight, segment });
    this.#posts.push({ flight: this.#flight, atMs: postMs, segment, worker: state.index });
  }

  #replied(state: WorkerState, reply: HeightWorkerReply): void {
    const replyMs = this.#now();
    this.#replies[reply.kind] += 1;
    switch (reply.kind) {
      case "baked":
        this.#answer(state, reply.id, "baked", replyMs);
        break;
      case "bake-failed":
        this.#answer(state, reply.id, "failed", replyMs);
        break;
      case "bake-unsurveyed":
        this.#answer(state, reply.id, "unsurveyed", replyMs);
        break;
      case "field-held":
      case "field-refused":
      case "class-map":
      case "class-map-failed":
      case "ground-readout":
      case "ground-readout-failed":
        break;
    }
    state.lastReplyMs = replyMs;
  }

  /** Bake `id`'s answer on `state`'s worker at `replyMs`, before its last reply moves on. */
  #answer(state: WorkerState, id: number, outcome: BakeOutcome, replyMs: number): void {
    const posted = state.inFlight.get(id);
    if (posted === undefined) {
      return;
    }
    state.inFlight.delete(id);
    const startMs = Math.max(posted.postMs, state.lastReplyMs ?? posted.postMs);
    this.#answered.push({
      worker: state.index,
      flight: posted.flight,
      outcome,
      replyMs,
      postToReplyMs: replyMs - posted.postMs,
      computeMs: replyMs - startMs,
      segment: this.#segmentNow(),
    });
  }

  /** The worker failed or is terminated: its bakes in flight will not be answered. */
  #abandon(state: WorkerState): void {
    for (const posted of state.inFlight.values()) {
      this.#abandoned.push(posted.flight);
    }
    state.inFlight.clear();
  }
}

/** The first and last time, ms, of the answers, posts and residents; ±∞ reversed with none. */
function spanOf(
  answers: ReadonlyArray<AnsweredBake>,
  posts: ReadonlyArray<Noted>,
  residents: ReadonlyArray<Noted>,
): readonly [number, number] {
  let first = Number.POSITIVE_INFINITY;
  let last = Number.NEGATIVE_INFINITY;
  for (const atMs of [
    ...answers.map((a) => a.replyMs),
    ...posts.map((p) => p.atMs),
    ...residents.map((r) => r.atMs),
  ]) {
    first = Math.min(first, atMs);
    last = Math.max(last, atMs);
  }
  return [first, last];
}

/** The {@link BakeTimes} of `answers` and `posted` bake posts. */
function timesOf(answers: ReadonlyArray<AnsweredBake>, posted: number): BakeTimes {
  const baked = answers.filter((a) => a.outcome === "baked");
  return {
    posted,
    baked: baked.length,
    failed: answers.filter((a) => a.outcome === "failed").length,
    unsurveyed: answers.filter((a) => a.outcome === "unsurveyed").length,
    postToReplyMs: msSpread(baked.map((a) => a.postToReplyMs)),
    computeMs: msSpread(baked.map((a) => a.computeMs)),
  };
}

/** The {@link BakeWindow} of `answers`, `posted` posts and `residents` from `fromMs` to `toMs`. */
function windowOf(
  answers: ReadonlyArray<AnsweredBake>,
  posted: number,
  residents: number,
  fromMs: number,
  toMs: number,
): BakeWindow {
  const times = timesOf(answers, posted);
  const spanMs = Number.isFinite(fromMs) && Number.isFinite(toMs) ? Math.max(0, toMs - fromMs) : 0;
  return {
    ...times,
    residents,
    bakedNeverResident: times.baked - residents,
    spanMs,
    bakesPerSecond: spanMs > 0 ? times.baked / (spanMs / 1000) : null,
  };
}
