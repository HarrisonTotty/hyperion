/**
 * The smoke page's twin workers (`twinWorker.ts`): the atmosphere's CPU twins of `tablesCpu.ts`,
 * computed off the page's thread and beside the GPU's work, each promise answering with the twin
 * the page would have computed itself.
 *
 * @remarks
 * Jobs start in the order asked, each on the first worker free, so that a group's twins finish
 * roughly in the order its checks read them. A twin that throws rejects its promise with an `Error`
 * of the same name and message, which the harness's group records as it would the twin's own
 * throw. Every promise the pool returns is marked handled: a group that fails before it reads a
 * twin it started leaves no unhandled rejection, and a group that reads it still sees the failure.
 */

import type { VolumeSize } from "../view/atmosphere/hillaire";
import type { AtmosphereMedium } from "../view/atmosphere/medium";
import type { AbsorberCurveSlots, TableSize } from "../view/atmosphere/tables";
import type {
  PlanetTwin,
  RayMarchSettings,
  TwinTable,
  TwinTransmittance,
  TwinView,
  TwinVolume,
} from "../view/atmosphere/tablesCpu";
import { handled } from "./ordered";
import type { TwinAnswer, TwinJob, TwinReply, TwinRequest } from "./twinJobs";

/** The part of a `Worker` the pool uses, so that a test can stand in for one. */
export interface TwinWorker {
  addEventListener(type: "message", listener: (event: MessageEvent<TwinReply>) => void): void;
  addEventListener(type: "error" | "messageerror", listener: (event: Event) => void): void;
  postMessage(message: TwinRequest, transfer: Transferable[]): void;
  terminate(): void;
}

/** Starts one twin worker, a same-origin module file. */
function startTwinWorker(): TwinWorker {
  return new Worker(new URL("./twinWorker.ts", import.meta.url), {
    type: "module",
    name: "smoke twin",
  });
}

/** The most twin workers a page starts, whatever it is asked (perf-smoke, 2026-10-13). */
export const MAX_TWIN_WORKERS = 4;

/**
 * How many twin workers a page takes on a machine of `threads` hardware threads: `asked`, the run's
 * `--smoke-twin-workers`, if it is a whole number from 1, or else a quarter of the threads, from 1;
 * never more than {@link MAX_TWIN_WORKERS}. A quarter, since the two variants run at once and
 * SwiftShader's GPU process takes some 2 cores of its own while it draws.
 */
export function twinWorkerCount(threads: number, asked: number | null): number {
  const count =
    asked !== null && Number.isInteger(asked) && asked >= 1
      ? asked
      : Math.max(1, Math.floor(threads / 4));
  return Math.min(MAX_TWIN_WORKERS, count);
}

/** A job waiting for its worker, and how its promise settles. */
interface Pending {
  readonly request: TwinRequest;
  readonly resolve: (answer: TwinAnswer) => void;
  readonly reject: (error: Error) => void;
}

/** An `Error` with a twin's name and message, as the page would have caught it. */
function twinError(name: string, message: string): Error {
  const error = new Error(message);
  error.name = name;
  return error;
}

/** Atmosphere twins computed on a pool of workers. */
export class TwinPool {
  readonly #idle: TwinWorker[] = [];
  readonly #workers: TwinWorker[] = [];
  readonly #queue: Pending[] = [];
  /** The job each busy worker runs. */
  readonly #running = new Map<TwinWorker, Pending>();
  #next = 0;
  /** Why the pool takes no more jobs: disposed, or a worker that failed. */
  #closed: Error | null = null;

  readonly #count: number;
  readonly #start: () => TwinWorker;

  /**
   * A pool of `count` workers, started with its first job, so that a run that asks for no twin
   * starts none.
   *
   * @param start - Starts one worker; the module worker's by default.
   */
  constructor(count: number, start: () => TwinWorker = startTwinWorker) {
    if (!(Number.isInteger(count) && count >= 1)) {
      throw new RangeError(`a twin pool takes at least one worker, not ${String(count)}`);
    }
    this.#count = count;
    this.#start = start;
  }

  /** Starts the workers, once. */
  #startWorkers(): void {
    if (this.#workers.length > 0) {
      return;
    }
    for (let i = 0; i < this.#count; i += 1) {
      const worker = this.#start();
      worker.addEventListener("message", (event: MessageEvent<TwinReply>) => {
        this.#answered(worker, event.data);
      });
      const fail = (event: Event): void => {
        // An error inside the worker is an `ErrorEvent` with its message; a module that does not
        // load, or a reply that cannot be read, a plain `Event`.
        const message: unknown = Reflect.get(event, "message");
        const why =
          typeof message === "string" && message.length > 0
            ? message
            : "its module did not load or a reply was lost";
        this.#fail(new Error(`a twin worker failed: ${why}`));
      };
      worker.addEventListener("error", fail);
      worker.addEventListener("messageerror", fail);
      this.#workers.push(worker);
      this.#idle.push(worker);
    }
  }

  /** `planetTwin`: both per-planet tables of a medium. */
  planet(
    medium: AtmosphereMedium,
    bottomRadiusM: number,
    curves: AbsorberCurveSlots,
  ): Promise<PlanetTwin> {
    return this.#run({ kind: "planet", medium, bottomRadiusM, curves }, (answer) =>
      answer.kind === "planet" ? answer.twin : null,
    );
  }

  /** `transmittanceTablesTwin`: a medium's transmittance and absorber-column tables. */
  transmittanceTables(
    medium: AtmosphereMedium,
    bottomRadiusM: number,
    curves: AbsorberCurveSlots,
  ): Promise<TwinTransmittance> {
    return this.#run({ kind: "transmittanceTables", medium, bottomRadiusM, curves }, (answer) =>
      answer.kind === "transmittanceTables" ? answer.twin : null,
    );
  }

  /** `transmittanceTwin`: a medium's transmittance table. */
  transmittance(medium: AtmosphereMedium, bottomRadiusM: number): Promise<TwinTable> {
    return this.#run({ kind: "transmittance", medium, bottomRadiusM }, (answer) =>
      answer.kind === "transmittance" ? answer.twin : null,
    );
  }

  /**
   * `multiScatteringTwin` over the transmittance tables given.
   *
   * @param curveSlot - The light slot whose layer the twin is (R08.T7): 0, the shared table's,
   *   unless given.
   */
  multiScattering(
    medium: AtmosphereMedium,
    bottomRadiusM: number,
    transmittance: TwinTransmittance,
    curveSlot = 0,
  ): Promise<TwinTable> {
    return this.#run(
      { kind: "multiScattering", medium, bottomRadiusM, transmittance, curveSlot },
      (answer) => (answer.kind === "multiScattering" ? answer.twin : null),
    );
  }

  /** `skyViewTwin`. */
  skyView(
    planet: PlanetTwin,
    view: TwinView,
    size: TableSize,
    samples: number,
  ): Promise<TwinTable> {
    return this.#run({ kind: "skyView", planet, view, size, samples }, (answer) =>
      answer.kind === "skyView" ? answer.twin : null,
    );
  }

  /** `aerialPerspectiveTwin`. */
  aerialPerspective(
    planet: PlanetTwin,
    view: TwinView,
    size: VolumeSize,
    samplesPerSlice: number,
    reachM: number,
  ): Promise<TwinVolume> {
    return this.#run(
      { kind: "aerialPerspective", planet, view, size, samplesPerSlice, reachM },
      (answer) => (answer.kind === "aerialPerspective" ? answer.twin : null),
    );
  }

  /**
   * `rayMarchTwin` over a scene of one depth at every pixel.
   *
   * @param depth - The scene's reversed-Z depth, `NEAR_M` over the surface's distance, or 0 where
   *   nothing is drawn.
   */
  rayMarch(
    planet: PlanetTwin,
    view: TwinView,
    settings: RayMarchSettings,
    depth: number,
  ): Promise<TwinTable> {
    return this.#run({ kind: "rayMarch", planet, view, settings, depth }, (answer) =>
      answer.kind === "rayMarch" ? answer.twin : null,
    );
  }

  /** Stops every worker; each job not yet answered rejects. */
  dispose(): void {
    this.#fail(new Error("the twin pool was disposed"));
  }

  /** Queues a job and narrows its answer to the job's kind. */
  #run<T>(job: TwinJob, narrow: (answer: TwinAnswer) => T | null): Promise<T> {
    const answer = new Promise<TwinAnswer>((resolve, reject) => {
      if (this.#closed !== null) {
        reject(this.#closed);
        return;
      }
      this.#startWorkers();
      this.#queue.push({ request: { id: this.#next, job }, resolve, reject });
      this.#next += 1;
      this.#dispatch();
    });
    return handled(
      answer.then((reply) => {
        const narrowed = narrow(reply);
        if (narrowed === null) {
          throw new Error(`a twin worker answered a ${job.kind} job with a ${reply.kind} twin`);
        }
        return narrowed;
      }),
    );
  }

  /** Hands queued jobs to idle workers, in order. */
  #dispatch(): void {
    while (this.#closed === null && this.#queue.length > 0 && this.#idle.length > 0) {
      const pending = this.#queue.shift();
      const worker = this.#idle.shift();
      if (pending === undefined || worker === undefined) {
        return;
      }
      this.#running.set(worker, pending);
      try {
        // Nothing is transferred: the job is copied, and the page keeps its tables.
        worker.postMessage(pending.request, []);
      } catch (error: unknown) {
        // A job that cannot be copied (`DataCloneError`) would never be answered.
        this.#fail(
          new Error(`a ${pending.request.job.kind} job could not be sent`, { cause: error }),
        );
      }
    }
  }

  /** Settles a worker's job with its reply and gives the worker the next one. */
  #answered(worker: TwinWorker, reply: TwinReply): void {
    const pending = this.#running.get(worker);
    if (pending === undefined || pending.request.id !== reply.id) {
      this.#fail(new Error(`a twin worker answered job ${reply.id}, which it was not running`));
      return;
    }
    this.#running.delete(worker);
    this.#idle.push(worker);
    switch (reply.kind) {
      case "answer":
        pending.resolve(reply.answer);
        break;
      case "error":
        pending.reject(twinError(reply.name, reply.message));
        break;
    }
    this.#dispatch();
  }

  /** Closes the pool: every worker stops, and every job waiting or running rejects with `error`. */
  #fail(error: Error): void {
    if (this.#closed !== null) {
      return;
    }
    this.#closed = error;
    for (const worker of this.#workers) {
      worker.terminate();
    }
    const unanswered = [...this.#running.values(), ...this.#queue];
    this.#running.clear();
    this.#queue.length = 0;
    this.#idle.length = 0;
    for (const pending of unanswered) {
      pending.reject(error);
    }
  }
}
