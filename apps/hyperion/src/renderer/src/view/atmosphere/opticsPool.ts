/**
 * The optics-worker pool: one queue on the render thread feeding a few module workers, which
 * assemble each body's medium and split its curves of growth by cell between them (plan R08,
 * R08.T10.a; Design notes 1 and 14; `decision-r08-t4d-kdist-chain.md` §15, ruling 3).
 *
 * @remarks
 * **A build** ({@link OpticsWorkerPool.build}) asks for the body's assembly and, at once, for its
 * brightest light's curves alone, split into one share a running worker (`absorbers.ts`'s
 * `absorberCurveSums`, cell index modulo the share count), then for its other lights' curves
 * together, split the same way. Each set of shares is joined in a worker
 * (`absorberCurvesFromSums`), since a join costs 0.13–0.23 s on Earth (R08.T4.d's record), too
 * long for the render thread. The build lands once the assembly and every light slot's curves have;
 * one whose assembly draws no medium, or no absorber, lands with no curves, its shares dropped.
 *
 * **The queue** is taken in the order asked, one job a worker at a time: a job takes from a tenth
 * of a second to seconds, so a second in flight would gain nothing. Bodies are built in the order
 * they were wanted, and within a body the brightest light's shares come before the others'.
 *
 * **Transfers.** A worker's reply is transferred to the render thread, and a join's sums are
 * transferred on from there to the joining worker, so the render thread copies no table.
 *
 * **Failures,** as the height pool's (`terrain/workers/pool.ts`): a worker that raises its error is
 * replaced and its job asked again of the next free worker, and its place is given up after
 * {@link MAX_CONSECUTIVE_OPTICS_FAILURES} failures with no answer between. A join whose worker
 * failed took its sums with it, so its build asks for that light set's shares again, as many times.
 * With every worker given up, every build fails. A job that throws is answered `failed` and fails
 * its build: a bug (`decision-r08-t10a-leans.md` item 6).
 *
 * The workers start at the first build, so a session that draws no disc loads none of the optics.
 * This module takes only types of `assemble.ts`, whose tables stay in the workers.
 */

import type { AbsorberCurve, AbsorberCurveSums } from "./absorbers";
import type { AssembledMedium } from "./assemble";
import type { BodyAtmosphereInput } from "./atmosphereInput";
import { type OpticsReply, type OpticsRequest, transferablesOf } from "./opticsMessages";

/** The part of a `Worker` the pool uses, so that tests can stand in for one. */
export interface OpticsWorkerLike {
  addEventListener(type: "message", listener: (event: MessageEvent<OpticsReply>) => void): void;
  /** An `ErrorEvent` when the script threw, a plain `Event` when the module failed to load. */
  addEventListener(type: "error", listener: (event: Event) => void): void;
  removeEventListener(type: "message", listener: (event: MessageEvent<OpticsReply>) => void): void;
  removeEventListener(type: "error", listener: (event: Event) => void): void;
  postMessage(message: OpticsRequest, transfer: Transferable[]): void;
  terminate(): void;
}

/**
 * How many times in a row a worker may fail, with no answer between, before its place in the pool
 * is given up rather than restarted: the height pool's three (`MAX_CONSECUTIVE_WORKER_FAILURES`).
 */
export const MAX_CONSECUTIVE_OPTICS_FAILURES = 3;

/** The most optics workers a pool runs by default. */
const MOST_OPTICS_WORKERS = 4;

/**
 * The default number of optics workers: clamp(⌊threads ÷ 2⌋, 1, 4)
 * (`decision-r08-t4d-kdist-chain.md` §15, ruling 3: four shares on the UHD 620 host's four cores
 * and eight threads, about three to four times faster; accepted as the lane's L1).
 *
 * @remarks
 * The workers are idle but at a body's arrival, so they may take half the machine's threads for
 * those seconds; the height workers' ⌊threads ÷ 4⌋ (`defaultHeightWorkers`) stay the steady
 * budget. A thread count the browser does not report as a finite number takes the least, one.
 *
 * @param hardwareConcurrency - The machine's logical processors, `navigator.hardwareConcurrency`.
 */
export function defaultOpticsWorkers(hardwareConcurrency: number): number {
  if (!Number.isFinite(hardwareConcurrency)) {
    return 1;
  }
  return Math.min(MOST_OPTICS_WORKERS, Math.max(1, Math.floor(hardwareConcurrency / 2)));
}

/**
 * Starts one optics worker on the renderer's own module: the browser's, which the pool's factory
 * uses unless a test gives its own.
 */
export function startOpticsWorker(): OpticsWorkerLike {
  return new Worker(new URL("./optics.worker.ts", import.meta.url), {
    type: "module",
    name: "optics",
  });
}

/** How a build ended. */
export type OpticsBuildOutcome =
  /** The assembly, with every light slot's curves (`DrawnMedium.curves`) where it draws a medium. */
  | { readonly kind: "built"; readonly assembled: AssembledMedium }
  /** A job threw, or every worker was given up: the message, for the console's log. */
  | { readonly kind: "failed"; readonly message: string }
  | { readonly kind: "cancelled" };

/** A build under way. */
export interface OpticsBuild {
  /** Settles once with how the build ended; never rejects. */
  readonly outcome: Promise<OpticsBuildOutcome>;
  /** Drops its jobs not yet sent and ignores the answers of those sent; settles it `cancelled`. */
  cancel(): void;
}

/** What builds a body's medium and its curves for the atmosphere cache: the pool, or a test's. */
export interface AtmosphereBuilder {
  /** Builds a body's assembly and its light slots' curves. */
  build(input: BodyAtmosphereInput): OpticsBuild;
  /** Stops every worker; builds under way settle `cancelled`. */
  terminate(): void;
}

/** What makes a pool. */
export interface OpticsWorkerPoolOptions {
  /** How many workers to run, at least one ({@link defaultOpticsWorkers}). */
  readonly workers: number;
  /** Starts one worker: {@link startOpticsWorker} in the app, a fake in tests. */
  readonly createWorker: () => OpticsWorkerLike;
}

/** A reply with an answer, as a job's outcome carries it. */
type Answer = Exclude<OpticsReply, { readonly kind: "failed" }>;

/** How a job ended. */
type JobOutcome =
  | { readonly kind: "answered"; readonly reply: Answer }
  | { readonly kind: "failed"; readonly message: string }
  /** Its worker failed after it was sent, taking the buffers it transferred with it. */
  | { readonly kind: "lost" }
  | { readonly kind: "cancelled" };

/** A job queued or in flight. */
interface Job {
  readonly request: OpticsRequest;
  readonly transfer: ArrayBuffer[];
  readonly resolve: (outcome: JobOutcome) => void;
  settled: boolean;
}

/** A job as a build holds it. */
interface JobHandle {
  readonly outcome: Promise<JobOutcome>;
  cancel(): void;
}

/** One place in the pool and the worker running in it. */
interface Slot {
  worker: OpticsWorkerLike;
  readonly onMessage: (event: MessageEvent<OpticsReply>) => void;
  readonly onError: () => void;
  job: Job | null;
  failures: number;
  /** Given up after {@link MAX_CONSECUTIVE_OPTICS_FAILURES}: no worker runs in this place. */
  lost: boolean;
}

/** A light set's curves, or why there are none. */
type PhaseOutcome =
  | { readonly kind: "curves"; readonly curves: ReadonlyArray<ReadonlyArray<AbsorberCurve>> }
  | Exclude<OpticsBuildOutcome, { readonly kind: "built" }>;

/** A job's, a light set's or a build's outcome once cancelled. */
const CANCELLED = { kind: "cancelled" } as const;

/**
 * The light sets a body's curves are asked in: the brightest light alone, then the others
 * together (`decision-r08-t4d-kdist-chain.md` §15, ruling 3), by slot.
 */
function lightSets(lights: number): ReadonlyArray<ReadonlyArray<number>> {
  if (lights === 0) {
    return [];
  }
  const others = Array.from({ length: lights - 1 }, (_, k) => k + 1);
  return others.length === 0 ? [[0]] : [[0], others];
}

/** The optics workers of the render thread (see the module's remarks). */
export class OpticsWorkerPool implements AtmosphereBuilder {
  readonly #workers: number;
  readonly #createWorker: () => OpticsWorkerLike;
  readonly #slots: Slot[] = [];
  /** The jobs not yet sent, in the order asked. */
  readonly #queue: Job[] = [];
  #nextId = 1;
  #terminated = false;

  /** @throws RangeError for a worker count that is not a whole number of at least one. */
  constructor(opts: OpticsWorkerPoolOptions) {
    if (!Number.isInteger(opts.workers) || opts.workers < 1) {
      throw new RangeError(`an optics-worker pool needs at least one worker, not ${opts.workers}`);
    }
    this.#workers = opts.workers;
    this.#createWorker = opts.createWorker;
  }

  /** How many workers the pool runs once started, given up or not: its size. */
  get workerCount(): number {
    return this.#workers;
  }

  /** How many workers have been started and not given up. */
  get runningWorkers(): number {
    return this.#slots.filter((slot) => !slot.lost).length;
  }

  /** How many workers were given up after failing {@link MAX_CONSECUTIVE_OPTICS_FAILURES} times. */
  get lostWorkers(): number {
    return this.#slots.filter((slot) => slot.lost).length;
  }

  /** How many jobs are queued, not yet sent to a worker. */
  get queuedCount(): number {
    return this.#queue.length;
  }

  /** How many jobs are in flight, over every worker. */
  get inFlightCount(): number {
    return this.#slots.filter((slot) => slot.job !== null).length;
  }

  /**
   * Builds a body's assembly and its light slots' curves (see the module's remarks).
   *
   * @throws Error once the pool is terminated.
   */
  build(input: BodyAtmosphereInput): OpticsBuild {
    if (this.#terminated) {
      throw new Error("the optics-worker pool was terminated");
    }
    this.#start();
    const handles = new Set<JobHandle>();
    let cancelled = false;
    const submit = (make: (id: number) => OpticsRequest, transfer: ArrayBuffer[]): JobHandle => {
      const handle = this.#submit(make, transfer);
      handles.add(handle);
      return handle;
    };
    const cancelAll = (): void => {
      for (const handle of handles) {
        handle.cancel();
      }
    };
    const count = Math.max(1, this.runningWorkers);
    const sharesOf = (slots: ReadonlyArray<number>): JobHandle[] =>
      Array.from({ length: count }, (_, index) =>
        submit((id) => ({ kind: "curve-sums", id, input, slots, share: { index, count } }), []),
      );
    const assembly = submit((id) => ({ kind: "assemble", id, input }), []);
    const lights = input.kind === "record" ? input.lights.length : 0;
    // Asked at once, behind the assembly, the brightest light's shares first and alone.
    const sets = lightSets(lights).map((slots) => ({ slots, shares: sharesOf(slots) }));

    // Read through a call, since a narrowing of the flag would outlive the awaits that change it.
    const isCancelled = (): boolean => cancelled;

    // A light set's shares joined once they have all landed, asked again where the join's worker
    // failed with their sums.
    const phase = async (
      slots: ReadonlyArray<number>,
      shares: ReadonlyArray<JobHandle>,
      attempt: number,
    ): Promise<PhaseOutcome> => {
      const answered = await Promise.all(shares.map((share) => share.outcome));
      const sums: Array<ReadonlyArray<AbsorberCurveSums>> = [];
      for (const outcome of answered) {
        if (outcome.kind !== "answered") {
          return failureOf(outcome);
        }
        if (outcome.reply.kind !== "curve-sums") {
          return { kind: "failed", message: `a curve share was answered ${outcome.reply.kind}` };
        }
        sums.push(outcome.reply.sums);
      }
      if (isCancelled()) {
        return CANCELLED;
      }
      const join = submit(
        (id) => ({ kind: "curve-join", id, input, slots, sums }),
        transferablesOf(sums),
      );
      const joined = await join.outcome;
      if (joined.kind === "lost" && attempt < MAX_CONSECUTIVE_OPTICS_FAILURES && !isCancelled()) {
        return phase(slots, sharesOf(slots), attempt + 1);
      }
      if (joined.kind !== "answered") {
        return failureOf(joined);
      }
      if (joined.reply.kind !== "curves") {
        return { kind: "failed", message: `a curve join was answered ${joined.reply.kind}` };
      }
      return { kind: "curves", curves: joined.reply.curves };
    };
    // Each light set is joined as soon as its own shares have landed, the assembly's or not.
    const phases = Promise.all(sets.map(({ slots, shares }) => phase(slots, shares, 1))).catch(
      (error: unknown): ReadonlyArray<PhaseOutcome> => [
        { kind: "failed", message: error instanceof Error ? error.message : String(error) },
      ],
    );

    // Without curves to join: the shares dropped, and their light sets let settle.
    const withoutCurves = async (outcome: OpticsBuildOutcome): Promise<OpticsBuildOutcome> => {
      cancelAll();
      await phases;
      return outcome;
    };

    const run = async (): Promise<OpticsBuildOutcome> => {
      const taken = assemblyOf(await assembly.outcome);
      if (taken.kind !== "assembled") {
        return withoutCurves(taken);
      }
      const { assembled } = taken;
      if (assembled.medium === undefined) {
        return withoutCurves({ kind: "built", assembled });
      }
      const { drawn } = assembled;
      if (drawn.absorbers.length === 0 || drawn.spectra.length === 0) {
        const curves = drawn.spectra.map(() => []);
        return withoutCurves({
          kind: "built",
          assembled: { ...assembled, drawn: { ...drawn, curves } },
        });
      }
      const landed = await phases;
      for (const each of landed) {
        if (each.kind === "failed") {
          cancelAll();
          return each;
        }
      }
      const curves: Array<ReadonlyArray<AbsorberCurve>> = [];
      for (const each of landed) {
        if (each.kind !== "curves") {
          return CANCELLED;
        }
        curves.push(...each.curves);
      }
      if (isCancelled()) {
        return CANCELLED;
      }
      const fits =
        curves.length === drawn.spectra.length &&
        curves.every((slot) => slot.length === drawn.absorbers.length);
      if (!fits) {
        return {
          kind: "failed",
          message: `the curves joined do not fit the medium's ${drawn.spectra.length} light slots and ${drawn.absorbers.length} absorbers`,
        };
      }
      return { kind: "built", assembled: { ...assembled, drawn: { ...drawn, curves } } };
    };

    const settled = run().then(
      (outcome): OpticsBuildOutcome => (cancelled ? CANCELLED : outcome),
      (error: unknown): OpticsBuildOutcome => ({
        kind: "failed",
        message: error instanceof Error ? error.message : String(error),
      }),
    );
    return {
      outcome: settled,
      cancel: () => {
        cancelled = true;
        cancelAll();
      },
    };
  }

  /** Stops every worker; every job settles `cancelled`, and the pool cannot be used afterwards. */
  terminate(): void {
    if (this.#terminated) {
      return;
    }
    this.#terminated = true;
    for (const slot of this.#slots) {
      this.#detach(slot);
      slot.worker.terminate();
      if (slot.job !== null) {
        settle(slot.job, CANCELLED);
        slot.job = null;
      }
    }
    for (const job of this.#queue.splice(0)) {
      settle(job, CANCELLED);
    }
  }

  /** Starts the workers, once. */
  #start(): void {
    if (this.#slots.length > 0) {
      return;
    }
    for (let n = 0; n < this.#workers; n += 1) {
      const slot: Slot = {
        worker: this.#createWorker(),
        onMessage: (event) => {
          this.#receive(slot, event.data);
        },
        onError: () => {
          this.#replace(slot);
        },
        job: null,
        failures: 0,
        lost: false,
      };
      this.#slots.push(slot);
      this.#attach(slot);
    }
  }

  /** Queues a job, failing it at once where every worker was given up. */
  #submit(make: (id: number) => OpticsRequest, transfer: ArrayBuffer[]): JobHandle {
    let resolve: (outcome: JobOutcome) => void = ignore;
    const outcome = new Promise<JobOutcome>((settleWith) => {
      resolve = settleWith;
    });
    const id = this.#nextId;
    this.#nextId += 1;
    const job: Job = { request: make(id), transfer, resolve, settled: false };
    if (this.#terminated) {
      settle(job, CANCELLED);
    } else if (this.runningWorkers === 0) {
      settle(job, EVERY_WORKER_LOST);
    } else {
      this.#queue.push(job);
      this.#pump();
    }
    return {
      outcome,
      cancel: () => {
        if (job.settled) {
          return;
        }
        settle(job, CANCELLED);
        const queued = this.#queue.indexOf(job);
        if (queued >= 0) {
          this.#queue.splice(queued, 1);
        }
      },
    };
  }

  /** Hands queued jobs to the free workers, in the order asked. */
  #pump(): void {
    for (const slot of this.#slots) {
      if (this.#terminated) {
        return;
      }
      if (slot.lost || slot.job !== null) {
        continue;
      }
      const job = this.#queue.shift();
      if (job === undefined) {
        return;
      }
      this.#send(slot, job);
    }
  }

  /** Sends a job to a free worker; a request that cannot be posted fails its job. */
  #send(slot: Slot, job: Job): void {
    slot.job = job;
    try {
      slot.worker.postMessage(job.request, job.transfer);
    } catch (error: unknown) {
      // A request the structured clone refuses: a bug in what was asked, not the worker's.
      slot.job = null;
      settle(job, {
        kind: "failed",
        message: error instanceof Error ? error.message : String(error),
      });
      this.#pump();
    }
  }

  /** Takes a worker's reply to its job. */
  #receive(slot: Slot, reply: OpticsReply): void {
    const job = slot.job;
    if (job === null || job.request.id !== reply.id) {
      return;
    }
    slot.job = null;
    slot.failures = 0;
    settle(
      job,
      reply.kind === "failed"
        ? { kind: "failed", message: reply.message }
        : { kind: "answered", reply },
    );
    this.#pump();
  }

  /**
   * Replaces a failed worker, asking its job again of the next free worker unless the job
   * transferred buffers away, or gives its place up after
   * {@link MAX_CONSECUTIVE_OPTICS_FAILURES} failures in a row.
   */
  #replace(slot: Slot): void {
    if (this.#terminated || slot.lost) {
      return;
    }
    this.#detach(slot);
    slot.worker.terminate();
    const job = slot.job;
    slot.job = null;
    if (job !== null && !job.settled) {
      if (job.transfer.length > 0) {
        settle(job, { kind: "lost" });
      } else {
        this.#queue.unshift(job);
      }
    }
    slot.failures += 1;
    if (slot.failures >= MAX_CONSECUTIVE_OPTICS_FAILURES) {
      slot.lost = true;
    } else {
      slot.worker = this.#createWorker();
      this.#attach(slot);
    }
    if (this.runningWorkers === 0) {
      for (const queued of this.#queue.splice(0)) {
        settle(queued, EVERY_WORKER_LOST);
      }
      return;
    }
    this.#pump();
  }

  #attach(slot: Slot): void {
    slot.worker.addEventListener("message", slot.onMessage);
    slot.worker.addEventListener("error", slot.onError);
  }

  #detach(slot: Slot): void {
    slot.worker.removeEventListener("message", slot.onMessage);
    slot.worker.removeEventListener("error", slot.onError);
  }
}

/** A job's outcome once every worker was given up. */
const EVERY_WORKER_LOST = { kind: "failed", message: "every optics worker was given up" } as const;

/** Takes nothing: a job's resolver until its promise has given it its own. */
function ignore(): void {}

/** Settles a job once. */
function settle(job: Job, outcome: JobOutcome): void {
  if (job.settled) {
    return;
  }
  job.settled = true;
  job.resolve(outcome);
}

/** An assembly job's outcome: the assembly, or why the build has none. */
function assemblyOf(
  outcome: JobOutcome,
):
  | { readonly kind: "assembled"; readonly assembled: AssembledMedium }
  | Exclude<OpticsBuildOutcome, { readonly kind: "built" }> {
  if (outcome.kind !== "answered") {
    return failureOf(outcome);
  }
  if (outcome.reply.kind !== "assembled") {
    return { kind: "failed", message: `an assembly was answered ${outcome.reply.kind}` };
  }
  return { kind: "assembled", assembled: outcome.reply.assembled };
}

/** A job's outcome other than an answer, as its build's. */
function failureOf(
  outcome: Exclude<JobOutcome, { readonly kind: "answered" }>,
): Exclude<OpticsBuildOutcome, { readonly kind: "built" }> {
  let failure: Exclude<OpticsBuildOutcome, { readonly kind: "built" }>;
  switch (outcome.kind) {
    case "failed":
      failure = outcome;
      break;
    case "lost":
      failure = {
        kind: "failed",
        message: `an optics worker failed with a join's sums ${MAX_CONSECUTIVE_OPTICS_FAILURES} times`,
      };
      break;
    case "cancelled":
      failure = CANCELLED;
      break;
  }
  return failure;
}
