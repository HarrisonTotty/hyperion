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
 * **A thick bake** ({@link OpticsWorkerPool.bakeThick}; R08.T14.c) is one `thick-bake` job a band
 * asked, queued as a build's jobs are (see "Bands"). Its failures are the bake's own, never a
 * build's: a `thick-bake-failed` answer, a job that throws, and every worker given up each settle
 * it `failed` with their message, which the atmosphere cache reads as `ATMOSPHERE: APPROXIMATE`,
 * never as `ATMOSPHERE NOT AVAILABLE` (decision-r08-t10a-leans, UX review item 1). Its requests
 * transfer nothing, so a worker that fails with one is replaced and the job asked again of the
 * next free worker, as an assembly is.
 *
 * **A polarisation correction** ({@link OpticsWorkerPool.bakePolarisation}; R08.T14.f) is one
 * `polarisation-bake` job a band, asked as a thick bake is, with the same request's fields, and
 * failing as one does: its own failure, never a build's.
 *
 * **A disc's table** ({@link OpticsWorkerPool.bakeDisc}; R08.T16.b) is one `disc-band-bake` job a
 * band, each with the whole table's fields and the medium and landed bakes it marches with, their
 * bands gathered by `discTable.ts`'s `discTableOfBands`; a body whose routed bake has not landed
 * has its J baked first as a thick bake is, then given to its bands. It fails as a thick bake does:
 * its own failure, never a build's. A body of one band asks one `disc-bake`.
 * **A deck** ({@link OpticsWorkerPool.bakeDeck}; R08.T15.a.2) is one `deck-bake` job a band, a
 * thick bake's fields and the split, answered with the band's deck tables and the correction over
 * it together, and failing as a thick bake does.
 *
 * **Bands** (R08.T14.d's bake time): a bake of several bands is asked one job a band, in the bands'
 * order, so that the workers bake them side by side, each band's job preparing the column alone
 * from the same request (on the bodies of several bands measured, a giant's air and its deck, the
 * preparation takes 0.01 s against a band's 1–4 s; pod, 2026-10-31). A band's tables are a pure
 * function of its request, so the bake gathers the same bits in the bands' order whichever worker
 * bakes each and in whatever order they land. The first band to fail fails the bake, its other
 * bands' jobs cancelled; another body's bakes go on.
 *
 * **Bins** (R08.T14.d's bake time): where a bake's bands do not fill the workers
 * ({@link binSharesFor}), each band is split further. A thick bake's band is asked one
 * `thick-bake-bins` a share of its 15 bake bins (two shares a worker, bin b in share b mod their
 * count), each job preparing and solving its bins alone, then one `thick-gather` of the shares,
 * asked ahead of the jobs waiting and transferring them, which converts every bin to the channels
 * in the bins' order. A correction's band is asked one `polarisation-bake-channel` a render
 * channel and a `polarisation-gather`; a deck's band its bins' `deck-bake-bins`, its correction's
 * channels' `deck-polarisation-channel` and a `deck-gather`; and a disc of one band whose routed
 * bake has not landed has its J so baked before its one `disc-bake`. Each bin's and channel's
 * values are its own request's pure function and are gathered by the code the whole band's job
 * runs, so the tables are the bits of one job's whatever the split. A share or gather that fails
 * fails its band, as a band fails its bake.
 *
 * The workers start at the first build or bake, so a session that draws no disc loads none of the
 * optics. This module takes only types of `assemble.ts` and of the thick bake's modules, whose
 * tables stay in the workers.
 */

import type { AbsorberCurve, AbsorberCurveSums } from "./absorbers";
import type { AssembledMedium } from "./assemble";
import type { BodyAtmosphereInput } from "./atmosphereInput";
import {
  type KeptBakeLights,
  type OpticsReply,
  type OpticsRequest,
  transferablesOf,
} from "./opticsMessages";
import { type DiscBandValues, discTableOfBands, type DiscReflectanceTable } from "./discTable";
import { keepForwardLobes, type MadeLobe } from "./forwardLobe";
import type { MsBinShare, MsSourceTable, ThickBakeBody } from "./thick/bake";
import type { DiscBakeAir } from "./thick/discBake";
import type { DeckBinShare, DeckSplit, DeckTables } from "./thick/deck";
import type { PolarisationChannelShare, PolarisationTable } from "./thick/polarisation";
import type { SpectralColumn } from "./thick/spectralColumn";

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
 * and eight threads, about three to four times faster; accepted as the lane's L1); or, where the OS
 * tells performance cores from efficiency cores (Apple silicon's, `HyperionApi.performanceThreads`),
 * clamp(⌊(P + ⌊E ÷ 3⌋) ÷ 2⌋, 1, 4) of its P performance and E efficiency threads, as the height
 * workers count them (a lean, 2026-11-03).
 *
 * @remarks
 * The workers are idle but at a body's arrival, so they may take half the machine's threads for
 * those seconds; the height workers (`defaultHeightWorkers`) stay the steady budget. An Apple chip's
 * efficiency cores, which `hardwareConcurrency` counts with the rest, run an optics share far
 * slower, so they count at a third there too: an M5's four and six give three where its ten threads
 * gave four, beside its four height workers at the same arrival; an M1's four and four two; an M1
 * Pro's eight and two four. A thread count the browser does not report as a finite number takes
 * the least, one.
 *
 * @param hardwareConcurrency - The machine's logical processors, `navigator.hardwareConcurrency`.
 * @param performanceThreads - Its performance cores' threads with a third of its efficiency
 *   cores', `HyperionApi.performanceThreads`, or `null` where its OS tells none apart.
 */
export function defaultOpticsWorkers(
  hardwareConcurrency: number,
  performanceThreads: number | null = null,
): number {
  const threads =
    performanceThreads !== null && Number.isInteger(performanceThreads) && performanceThreads >= 1
      ? performanceThreads
      : hardwareConcurrency;
  if (!Number.isFinite(threads)) {
    return 1;
  }
  return Math.min(MOST_OPTICS_WORKERS, Math.max(1, Math.floor(threads / 2)));
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

/**
 * What a thick bake asks (plan R08, R08.T14.c): a `thick-bake` request's fields
 * (`opticsMessages.ts`'s {@link OpticsRequest}), every band in one ask, which the pool asks one
 * job a band.
 */
export interface ThickBakeAsk {
  /**
   * The body's input: a server body's as its build took it, whose absorber specs and light spectra
   * the worker forms; a kept body's stands for no medium (its `kept` lights carry them).
   */
  readonly input: BodyAtmosphereInput;
  /** The light slots to bake, indices of the input's lights or of `kept.spectra`. */
  readonly slots: ReadonlyArray<number>;
  /** The latitude bands to bake, indices of `thick/bake.ts`'s `thickBakeBands(body)`. */
  readonly bands: ReadonlyArray<number>;
  readonly column: SpectralColumn;
  readonly body: ThickBakeBody;
  /** A kept body's absorber specs and light spectra, in place of the input's. */
  readonly kept?: KeptBakeLights;
  /**
   * Whether the bake solves the channels its lights allow at their own wavelengths, a haze route's
   * (`thick/bake.ts`'s remarks, "Own wavelengths"; R08.T14.d.4), the share with bin 0 baking their
   * points: none where absent.
   */
  readonly ownWavelengths?: boolean;
  /**
   * Whether its diffuse streams take their curved paths' shares, a haze route's (`thick/bake.ts`'s
   * remarks, "Curved streams"; R08.T14.d.4): none where absent.
   */
  readonly curvedStreams?: boolean;
  /**
   * Whether its rows of a low sun take the horizon's first-order spherical terms, both, a haze
   * route's (`thick/bake.ts`'s remarks, "The horizon's first-order terms"; R08.T14.d.4): none where
   * absent.
   */
  readonly horizonTerms?: boolean;
}

/** How a thick bake ended. */
export type ThickBakeOutcome =
  /** One table a band asked, in their order, each with a slot a light asked, in theirs. */
  | { readonly kind: "baked"; readonly tables: ReadonlyArray<MsSourceTable> }
  /**
   * The bake failed: its job's own failure, a job that threw, its worker lost or every worker given
   * up, with the message for the console's log. The bake's failure alone, never the medium's.
   */
  | { readonly kind: "failed"; readonly message: string }
  | { readonly kind: "cancelled" };

/** A thick bake under way. */
export interface ThickBake {
  /** Settles once with how the bake ended; never rejects. */
  readonly outcome: Promise<ThickBakeOutcome>;
  /** Drops its job if not yet sent and ignores its answer if sent; settles it `cancelled`. */
  cancel(): void;
}

/** What a deck's bake asks (R08.T15.a.2): a thick bake's fields and the split. */
export interface DeckBakeAsk extends ThickBakeAsk {
  /** Where the split lies and how deep its deck is read (`thick/deck.ts`'s `DeckSplit`). */
  readonly split: DeckSplit;
}

/** How a deck's bake ended (R08.T15.a.2), as a thick bake's does. */
export type DeckBakeOutcome =
  /**
   * One deck table a band asked, in their order, each with a slot a light asked, in theirs, and the
   * correction over the deck the same way, or `null` where the column needs none.
   */
  | {
      readonly kind: "baked";
      readonly tables: ReadonlyArray<DeckTables>;
      readonly correction: ReadonlyArray<PolarisationTable> | null;
    }
  /** The bake failed, as a thick bake fails, with the message for the console's log. */
  | { readonly kind: "failed"; readonly message: string }
  | { readonly kind: "cancelled" };

/** A deck's bake under way (R08.T15.a.2). */
export interface DeckBake {
  /** Settles once with how the bake ended; never rejects. */
  readonly outcome: Promise<DeckBakeOutcome>;
  /** Drops its job if not yet sent and ignores its answer if sent; settles it `cancelled`. */
  cancel(): void;
}

/** How a polarisation correction's bake ended (R08.T14.f), as a thick bake's does. */
export type PolarisationBakeOutcome =
  /** One table a band asked, in their order, each with a slot a light asked, in theirs. */
  | { readonly kind: "baked"; readonly tables: ReadonlyArray<PolarisationTable> }
  /** The bake failed, as a thick bake fails, with the message for the console's log. */
  | { readonly kind: "failed"; readonly message: string }
  | { readonly kind: "cancelled" };

/** A polarisation correction's bake under way (R08.T14.f). */
export interface PolarisationBake {
  /** Settles once with how the bake ended; never rejects. */
  readonly outcome: Promise<PolarisationBakeOutcome>;
  /** Drops its job if not yet sent and ignores its answer if sent; settles it `cancelled`. */
  cancel(): void;
}

/**
 * What a disc's table asks (plan R08, R08.T16.b): a `disc-bake` request's fields, a thick bake's
 * with every band of the tables and what the march takes of them.
 */
export interface DiscBakeAsk extends ThickBakeAsk {
  readonly air: DiscBakeAir;
}

/** How a disc's table's bake ended (R08.T16.b), as a thick bake's does. */
export type DiscBakeOutcome =
  | { readonly kind: "baked"; readonly table: DiscReflectanceTable }
  /** The bake failed, as a thick bake fails, with the message for the console's log. */
  | { readonly kind: "failed"; readonly message: string }
  | { readonly kind: "cancelled" };

/** A disc's table's bake under way (R08.T16.b). */
export interface DiscBake {
  /** Settles once with how the bake ended; never rejects. */
  readonly outcome: Promise<DiscBakeOutcome>;
  /** Drops its job if not yet sent and ignores its answer if sent; settles it `cancelled`. */
  cancel(): void;
}

/** What builds a body's medium and its curves for the atmosphere cache: the pool, or a test's. */
export interface AtmosphereBuilder {
  /** Builds a body's assembly and its light slots' curves. */
  build(input: BodyAtmosphereInput): OpticsBuild;
  /** Bakes a thick body's multiple-scattering tables (R08.T14.c), every band in one ask. */
  bakeThick(ask: ThickBakeAsk): ThickBake;
  /**
   * Bakes a body's polarisation correction (R08.T14.f), every band in one ask: a thick bake's
   * request's fields, for a thin or a thick body.
   */
  bakePolarisation(ask: ThickBakeAsk): PolarisationBake;
  /** Bakes a body's disc reflectance table (R08.T16.b), every band and light in one ask. */
  bakeDisc(ask: DiscBakeAsk): DiscBake;
  /**
   * Bakes a cloud-deck split's tables and the correction over it (R08.T15.a.2), every band in one
   * ask.
   */
  bakeDeck(ask: DeckBakeAsk): DeckBake;
  /** Stops every worker; builds and bakes under way settle `cancelled`. */
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

/** How a bake, or one band of it, ended other than baked. */
type Unbaked = { readonly kind: "failed"; readonly message: string } | typeof CANCELLED;

/** How a bake, or one band of it, ended. */
type Settled = { readonly kind: "baked" } | Unbaked;

/** A bake, or one band of it, under way: what each of the pool's bakes answers. */
interface Settling<Outcome> {
  readonly outcome: Promise<Outcome>;
  cancel(): void;
}

/** How a disc's band's job ended (the module's remarks, "Bands"). */
type DiscBandOutcome = { readonly kind: "baked"; readonly values: DiscBandValues } | Unbaked;

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
      // The worker's lobes kept before the medium is drawn, whose first draw then makes none.
      keepForwardLobes(taken.lobes);
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

  /**
   * Bakes a thick body's multiple-scattering tables (see the module's remarks): one `thick-bake`
   * job a band of `ask`, each asking every slot.
   *
   * @throws Error once the pool is terminated.
   */
  bakeThick(ask: ThickBakeAsk): ThickBake {
    this.#startBake();
    const shares = binSharesFor(ask.bands.length, this.runningWorkers);
    return this.#byBand(
      ask,
      (band) =>
        shares > 1
          ? this.#thickByBins(band, shares)
          : this.#bakeJob(band, (id) => ({ kind: "thick-bake", id, ...band }), bakeOutcomeOf),
      (parts) => ({ kind: "baked", tables: parts.flatMap((part) => part.tables) }),
    );
  }

  /**
   * One band's thick bake split by its bins (the module's remarks, "Bins"): one `thick-bake-bins`
   * job a share of `shares`, then a `thick-gather` of their shares, asked ahead of the queue.
   */
  #thickByBins(ask: ThickBakeAsk, shares: number): ThickBake {
    const parts = binShares(shares).map((bins) =>
      this.#bakeJob(
        ask,
        (id) => ({ kind: "thick-bake-bins", id, ...ask, bins }),
        binShareOutcomeOf,
      ),
    );
    return then(
      gathered(parts, (landed) => ({
        kind: "baked" as const,
        shares: landed.map((part) => part.share),
      })),
      ({ shares: landed }) =>
        this.#bakeJob(ask, (id) => ({ kind: "thick-gather", id, shares: landed }), bakeOutcomeOf, {
          transfer: transferablesOf(landed),
          ahead: true,
        }),
    );
  }

  /**
   * Bakes a body's polarisation correction (see the module's remarks): one `polarisation-bake` job
   * a band of `ask`, each asking every slot.
   *
   * @throws Error once the pool is terminated.
   */
  bakePolarisation(ask: ThickBakeAsk): PolarisationBake {
    this.#startBake();
    const split = binSharesFor(ask.bands.length, this.runningWorkers) > 1;
    return this.#byBand(
      ask,
      (band) =>
        split
          ? this.#correctionByChannels(band)
          : this.#bakeJob(
              band,
              (id) => ({ kind: "polarisation-bake", id, ...band }),
              polarisationOutcomeOf,
            ),
      (parts) => ({ kind: "baked", tables: parts.flatMap((part) => part.tables) }),
    );
  }

  /**
   * One band's correction split by its render channels (the module's remarks, "Bins"): one
   * `polarisation-bake-channel` job a channel, then a `polarisation-gather` asked ahead of the queue.
   */
  #correctionByChannels(ask: ThickBakeAsk): PolarisationBake {
    const parts = RENDER_CHANNELS.map((channel) =>
      this.#bakeJob(
        ask,
        (id) => ({ kind: "polarisation-bake-channel", id, ...ask, channel }),
        channelShareOutcomeOf,
      ),
    );
    return then(
      gathered(parts, (landed) => ({
        kind: "baked" as const,
        shares: landed.map((part) => part.share),
      })),
      ({ shares }) =>
        this.#bakeJob(
          ask,
          (id) => ({ kind: "polarisation-gather", id, shares }),
          polarisationOutcomeOf,
          { transfer: transferablesOf(shares), ahead: true },
        ),
    );
  }

  /**
   * Bakes a body's disc reflectance table (see the module's remarks): one `disc-band-bake` job a
   * band of `ask`, its J baked first where `ask.air` holds none, or one `disc-bake` for a body of
   * one band.
   *
   * @throws Error once the pool is terminated.
   */
  bakeDisc(ask: DiscBakeAsk): DiscBake {
    this.#startBake();
    const spread = ask.bands.length > 1 || binSharesFor(ask.bands.length, this.runningWorkers) > 1;
    if (ask.air.bake === undefined && spread) {
      // A body whose routed bake has not landed: its J spread over the workers, then its table.
      const { air, ...thick } = ask;
      return then(this.bakeThick(thick), (solved) =>
        this.#discOf({ ...ask, air: { ...air, bake: solved.tables } }),
      );
    }
    return this.#discOf(ask);
  }

  /** A disc's table as {@link bakeDisc} asks it once its J is given or left to the job. */
  #discOf(ask: DiscBakeAsk): DiscBake {
    if (ask.bands.length <= 1) {
      return this.#bakeJob(ask, (id) => ({ kind: "disc-bake", id, ...ask }), discOutcomeOf);
    }
    return this.#discBands(ask, ask.air);
  }

  /**
   * Bakes a cloud-deck split's tables (see the module's remarks): one `deck-bake` job a band of
   * `ask`, each asking every slot.
   *
   * @throws Error once the pool is terminated.
   */
  bakeDeck(ask: DeckBakeAsk): DeckBake {
    this.#startBake();
    const shares = binSharesFor(ask.bands.length, this.runningWorkers);
    return this.#byBand(
      ask,
      (band) =>
        shares > 1
          ? this.#deckByBins(band, shares)
          : this.#bakeJob(band, (id) => ({ kind: "deck-bake", id, ...band }), deckOutcomeOf),
      deckOfBands,
    );
  }

  /**
   * One band's deck split by its bins and its correction by its render channels (the module's
   * remarks, "Bins"): one `deck-bake-bins` job a share of `shares` and one
   * `deck-polarisation-channel` a channel, then a `deck-gather` asked ahead of the queue.
   */
  #deckByBins(ask: DeckBakeAsk, shares: number): DeckBake {
    const decks = gathered(
      binShares(shares).map((bins) =>
        this.#bakeJob(
          ask,
          (id) => ({ kind: "deck-bake-bins", id, ...ask, bins }),
          deckShareOutcomeOf,
        ),
      ),
      (landed) => ({ kind: "baked" as const, decks: landed.map((part) => part.share) }),
    );
    const channels = gathered(
      RENDER_CHANNELS.map((channel) =>
        this.#bakeJob(
          ask,
          (id) => ({ kind: "deck-polarisation-channel", id, ...ask, channel }),
          deckChannelOutcomeOf,
        ),
      ),
      (landed) => ({ kind: "baked" as const, channels: landed.map((part) => part.share) }),
    );
    return then(both(decks, channels), ({ first, second }) => {
      const corrections = second.channels;
      const corrected = corrections.filter((share) => share !== null);
      const landed = {
        decks: first.decks,
        channels: corrected.length === corrections.length ? corrected : null,
      };
      return this.#bakeJob(ask, (id) => ({ kind: "deck-gather", id, ...landed }), deckOutcomeOf, {
        transfer: transferablesOf(landed),
        ahead: true,
      });
    });
  }

  /** Refuses a bake once the pool is terminated, and starts the workers at the first. */
  #startBake(): void {
    if (this.#terminated) {
      throw new Error("the optics-worker pool was terminated");
    }
    this.#start();
  }

  /**
   * A bake of `ask`'s bands, one ask a band (the module's remarks, "Bands"), each `one` of `ask`
   * with that band alone, gathered in the bands' order; an ask of one band, or none, is `one` of
   * itself.
   */
  #byBand<Ask extends ThickBakeAsk, Baked extends { readonly kind: "baked" }>(
    ask: Ask,
    one: (band: Ask) => Settling<Baked | Unbaked>,
    gather: (parts: ReadonlyArray<Baked>) => Baked,
  ): Settling<Baked | Unbaked> {
    if (ask.bands.length <= 1) {
      return one(ask);
    }
    return gathered(
      ask.bands.map((band) => one({ ...ask, bands: [band] })),
      gather,
    );
  }

  /** A disc's table from one `disc-band-bake` a band, each with `air`, gathered in their order. */
  #discBands(ask: DiscBakeAsk, air: DiscBakeAir): DiscBake {
    return gathered(
      ask.bands.map((_, part) =>
        this.#bakeJob(
          ask,
          (id) => ({ kind: "disc-band-bake", id, ...ask, air, part }),
          discBandOutcomeOf,
        ),
      ),
      (parts): Extract<DiscBakeOutcome, { readonly kind: "baked" }> => ({
        kind: "baked",
        table: discTableOfBands(parts.map((part) => part.values)),
      }),
    );
  }

  /**
   * One bake's job, `make`'s request, its outcome read by `outcomeOf` against `ask`: posted with
   * `transfer` (none by default), and queued `ahead` of every job waiting where asked, as a gather
   * that a bake waits on.
   */
  #bakeJob<Ask, Outcome extends Settled>(
    ask: Ask,
    make: (id: number) => OpticsRequest,
    outcomeOf: (outcome: JobOutcome, ask: Ask) => Outcome,
    posting: { readonly transfer: ArrayBuffer[]; readonly ahead: boolean } = NOT_AHEAD,
  ): Settling<Outcome | Unbaked> {
    const job = this.#submit(make, posting.transfer, posting.ahead);
    let cancelled = false;
    const settled = job.outcome.then(
      (outcome): Outcome | Unbaked => (cancelled ? CANCELLED : outcomeOf(outcome, ask)),
      (error: unknown): Unbaked => failedWith(error),
    );
    return {
      outcome: settled,
      cancel: () => {
        cancelled = true;
        job.cancel();
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
  #submit(make: (id: number) => OpticsRequest, transfer: ArrayBuffer[], ahead = false): JobHandle {
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
      if (ahead) {
        this.#queue.unshift(job);
      } else {
        this.#queue.push(job);
      }
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
function assemblyOf(outcome: JobOutcome):
  | {
      readonly kind: "assembled";
      readonly assembled: AssembledMedium;
      readonly lobes: ReadonlyArray<MadeLobe>;
    }
  | Exclude<OpticsBuildOutcome, { readonly kind: "built" }> {
  if (outcome.kind !== "answered") {
    return failureOf(outcome);
  }
  if (outcome.reply.kind !== "assembled") {
    return { kind: "failed", message: `an assembly was answered ${outcome.reply.kind}` };
  }
  return { kind: "assembled", assembled: outcome.reply.assembled, lobes: outcome.reply.lobes };
}

/** A thick bake's job's outcome as the bake's (see the module's remarks). */
function bakeOutcomeOf(outcome: JobOutcome, ask: ThickBakeAsk): ThickBakeOutcome {
  let baked: ThickBakeOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "thick-baked") {
        baked =
          reply.tables.length === ask.bands.length
            ? { kind: "baked", tables: reply.tables }
            : {
                kind: "failed",
                message: `a thick bake of ${ask.bands.length} bands was answered with ${reply.tables.length} tables`,
              };
      } else if (reply.kind === "thick-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = { kind: "failed", message: `a thick bake was answered ${reply.kind}` };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      // A bake's request transfers nothing, so the pool asks it again rather than losing it: held
      // here for a request that would.
      baked = { kind: "failed", message: "an optics worker failed with the thick bake's request" };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/** A polarisation correction's job's outcome as the bake's, as {@link bakeOutcomeOf} reads a thick one. */
function polarisationOutcomeOf(outcome: JobOutcome, ask: ThickBakeAsk): PolarisationBakeOutcome {
  let baked: PolarisationBakeOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "polarisation-baked") {
        baked =
          reply.tables.length === ask.bands.length
            ? { kind: "baked", tables: reply.tables }
            : {
                kind: "failed",
                message: `a polarisation correction of ${ask.bands.length} bands was answered with ${reply.tables.length} tables`,
              };
      } else if (reply.kind === "polarisation-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = { kind: "failed", message: `a polarisation correction was answered ${reply.kind}` };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      baked = {
        kind: "failed",
        message: "an optics worker failed with the polarisation correction's request",
      };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/** A `disc-bake` job's outcome as its bake's. */
function discOutcomeOf(outcome: JobOutcome): DiscBakeOutcome {
  let baked: DiscBakeOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "disc-baked") {
        baked = { kind: "baked", table: reply.table };
      } else if (reply.kind === "disc-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = { kind: "failed", message: `a disc's bake was answered ${reply.kind}` };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      baked = { kind: "failed", message: "an optics worker failed with the disc's bake's request" };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/** A deck's job's outcome as the bake's, as {@link bakeOutcomeOf} reads a thick one. */
function deckOutcomeOf(outcome: JobOutcome, ask: DeckBakeAsk): DeckBakeOutcome {
  let baked: DeckBakeOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "deck-baked") {
        const bands = ask.bands.length;
        const corrected = reply.correction?.length ?? bands;
        if (reply.tables.length !== bands) {
          baked = {
            kind: "failed",
            message: `a deck of ${bands} bands was answered with ${reply.tables.length} tables`,
          };
        } else if (corrected !== bands) {
          baked = {
            kind: "failed",
            message: `a deck of ${bands} bands was answered with a correction of ${corrected}`,
          };
        } else {
          baked = { kind: "baked", tables: reply.tables, correction: reply.correction };
        }
      } else if (reply.kind === "deck-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = { kind: "failed", message: `a deck's bake was answered ${reply.kind}` };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      baked = { kind: "failed", message: "an optics worker failed with the deck's request" };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/** A `disc-band-bake` job's outcome as its band's, as {@link discOutcomeOf} reads a table's. */
function discBandOutcomeOf(outcome: JobOutcome): DiscBandOutcome {
  let baked: DiscBandOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "disc-band-baked") {
        baked = { kind: "baked", values: reply.values };
      } else if (reply.kind === "disc-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = { kind: "failed", message: `a disc's band was answered ${reply.kind}` };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      baked = { kind: "failed", message: "an optics worker failed with the disc's band's request" };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/** A deck's bands' outcomes as its bake's, each band's tables and correction in their order. */
function deckOfBands(
  parts: ReadonlyArray<Extract<DeckBakeOutcome, { readonly kind: "baked" }>>,
): Extract<DeckBakeOutcome, { readonly kind: "baked" }> {
  const corrections = parts.map((part) => part.correction);
  const corrected = corrections.filter((correction) => correction !== null);
  if (corrected.length !== 0 && corrected.length !== corrections.length) {
    throw new RangeError("a deck's bands were answered with and without a correction");
  }
  return {
    kind: "baked",
    tables: parts.flatMap((part) => part.tables),
    correction: corrected.length === 0 ? null : corrected.flat(),
  };
}

/**
 * A bake of its bands' parts (the module's remarks, "Bands"): baked once every part has, `gather`
 * of their outcomes in the parts' order; failed with the first part to fail, the others cancelled;
 * cancelled with them, or by its own `cancel`.
 */
function gathered<
  Part extends { readonly kind: "baked" },
  Whole extends { readonly kind: "baked" },
>(
  parts: ReadonlyArray<Settling<Part | Unbaked>>,
  gather: (baked: ReadonlyArray<Part>) => Whole,
): Settling<Whole | Unbaked> {
  let resolve: (outcome: Whole | Unbaked) => void = ignore;
  const outcome = new Promise<Whole | Unbaked>((settleWith) => {
    resolve = settleWith;
  });
  let open = true;
  const finish = (ended: Whole | Unbaked): void => {
    if (open) {
      open = false;
      resolve(ended);
    }
  };
  const cancelParts = (): void => {
    for (const part of parts) {
      part.cancel();
    }
  };
  const baked: Part[] = [];
  let landed = 0;
  const take = async (index: number, part: Settling<Part | Unbaked>): Promise<void> => {
    const ended = await part.outcome;
    if (ended.kind !== "baked") {
      cancelParts();
      finish(ended);
      return;
    }
    baked[index] = ended;
    landed += 1;
    if (landed === parts.length && open) {
      finish(gather(baked));
    }
  };
  for (const [index, part] of parts.entries()) {
    void take(index, part).catch((error: unknown) => {
      cancelParts();
      finish(failedWith(error));
    });
  }
  return {
    outcome,
    cancel: () => {
      finish(CANCELLED);
      cancelParts();
    },
  };
}

/**
 * Two bakes as one (the module's remarks, "Bins"): baked once both have, failed with the first to
 * fail, the other cancelled, as {@link gathered} gathers parts of one kind.
 */
function both<First extends { readonly kind: "baked" }, Second extends { readonly kind: "baked" }>(
  first: Settling<First | Unbaked>,
  second: Settling<Second | Unbaked>,
): Settling<{ readonly kind: "baked"; readonly first: First; readonly second: Second } | Unbaked> {
  /** One of the two, tagged with which. */
  type Either =
    | { readonly kind: "baked"; readonly first: First }
    | { readonly kind: "baked"; readonly second: Second };
  const tagged = <Part extends { readonly kind: "baked" }>(
    part: Settling<Part | Unbaked>,
    tag: (baked: Part) => Either,
  ): Settling<Either | Unbaked> => ({
    outcome: part.outcome.then((ended): Either | Unbaked =>
      ended.kind === "baked" ? tag(ended) : ended,
    ),
    cancel: () => {
      part.cancel();
    },
  });
  return gathered(
    [
      tagged(first, (baked) => ({ kind: "baked", first: baked })),
      tagged(second, (baked) => ({ kind: "baked", second: baked })),
    ],
    (parts) => {
      const one = parts.find((part) => "first" in part);
      const other = parts.find((part) => "second" in part);
      if (one === undefined || !("first" in one) || other === undefined || !("second" in other)) {
        throw new Error("two bakes as one landed without both");
      }
      return { kind: "baked", first: one.first, second: other.second };
    },
  );
}

/**
 * A bake that waits on another (the module's remarks, "Bins"): `after` of `first`'s outcome once it
 * has baked, `first`'s failure or cancellation otherwise; cancelling it cancels both.
 */
function then<First extends { readonly kind: "baked" }, Next extends Settled>(
  first: Settling<First | Unbaked>,
  after: (baked: First) => Settling<Next | Unbaked>,
): Settling<Next | Unbaked> {
  let second: Settling<Next | Unbaked> | null = null;
  let cancelled = false;
  const run = async (): Promise<Next | Unbaked> => {
    const landed = await first.outcome;
    if (landed.kind !== "baked") {
      return landed;
    }
    if (cancelled) {
      return CANCELLED;
    }
    second = after(landed);
    return second.outcome;
  };
  const settled = run().catch((error: unknown): Unbaked => failedWith(error));
  return {
    outcome: settled.then((outcome): Next | Unbaked => (cancelled ? CANCELLED : outcome)),
    cancel: () => {
      cancelled = true;
      first.cancel();
      second?.cancel();
    },
  };
}

/** How a `polarisation-bake-channel` job ended: its band's share of one channel, or why not. */
type ChannelShareOutcome =
  { readonly kind: "baked"; readonly share: PolarisationChannelShare } | Unbaked;

/** A `polarisation-bake-channel` job's outcome as its share's. */
function channelShareOutcomeOf(outcome: JobOutcome): ChannelShareOutcome {
  let baked: ChannelShareOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "polarisation-channel-baked") {
        baked = { kind: "baked", share: reply.share };
      } else if (reply.kind === "polarisation-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = { kind: "failed", message: `a correction's channel was answered ${reply.kind}` };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      baked = { kind: "failed", message: "an optics worker failed with a correction's channel" };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/** The render channels a correction is split into, red, green and blue. */
const RENDER_CHANNELS = [0, 1, 2] as const;

/** How a `deck-bake-bins` job ended: its band's share of its bins, or why not. */
type DeckShareOutcome = { readonly kind: "baked"; readonly share: DeckBinShare } | Unbaked;

/** A `deck-bake-bins` job's outcome as its share's. */
function deckShareOutcomeOf(outcome: JobOutcome): DeckShareOutcome {
  let baked: DeckShareOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "deck-bins-baked") {
        baked = { kind: "baked", share: reply.share };
      } else if (reply.kind === "deck-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = { kind: "failed", message: `a deck's bins were answered ${reply.kind}` };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      baked = { kind: "failed", message: "an optics worker failed with a deck's bins" };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/** How a `deck-polarisation-channel` job ended: its channel's share, none, or why not. */
type DeckChannelOutcome =
  { readonly kind: "baked"; readonly share: PolarisationChannelShare | null } | Unbaked;

/** A `deck-polarisation-channel` job's outcome as its share's. */
function deckChannelOutcomeOf(outcome: JobOutcome): DeckChannelOutcome {
  let baked: DeckChannelOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "deck-channel-baked") {
        baked = { kind: "baked", share: reply.share };
      } else if (reply.kind === "deck-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = {
          kind: "failed",
          message: `a deck's correction's channel was answered ${reply.kind}`,
        };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      baked = {
        kind: "failed",
        message: "an optics worker failed with a deck's correction's channel",
      };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/** How a `thick-bake-bins` job ended: its band's share of its bins, or why not. */
type BinShareOutcome = { readonly kind: "baked"; readonly share: MsBinShare } | Unbaked;

/** A `thick-bake-bins` job's outcome as its share's, as {@link bakeOutcomeOf} reads a band's. */
function binShareOutcomeOf(outcome: JobOutcome): BinShareOutcome {
  let baked: BinShareOutcome;
  switch (outcome.kind) {
    case "answered": {
      const { reply } = outcome;
      if (reply.kind === "thick-bins-baked") {
        baked = { kind: "baked", share: reply.share };
      } else if (reply.kind === "thick-bake-failed") {
        baked = { kind: "failed", message: reply.message };
      } else {
        baked = { kind: "failed", message: `a thick bake's bins were answered ${reply.kind}` };
      }
      break;
    }
    case "failed":
      baked = outcome;
      break;
    case "lost":
      baked = { kind: "failed", message: "an optics worker failed with a thick bake's bins" };
      break;
    case "cancelled":
      baked = CANCELLED;
      break;
  }
  return baked;
}

/**
 * The bin shares a band of a bake is split into (the module's remarks, "Bins"): none, 1, where the
 * bake's bands fill the workers, and otherwise {@link SHARES_A_WORKER} a worker over its bands, no
 * more than a share a bin.
 */
export function binSharesFor(bands: number, workers: number): number {
  if (bands === 0 || bands >= workers) {
    return 1;
  }
  return Math.min(BAKE_BINS, Math.ceil((SHARES_A_WORKER * workers) / bands));
}

/** The bake bins of each of `count` shares: bin b in share b mod `count`, ascending in each. */
export function binShares(count: number): ReadonlyArray<ReadonlyArray<number>> {
  const bins = Array.from({ length: BAKE_BINS }, (_, b) => b);
  return Array.from({ length: count }, (_, share) => bins.filter((b) => b % count === share));
}

/** The thick bake's bins: `medium.ts`'s `BAKE_BIN_COUNT`, held equal to it by `opticsBins.test.ts`. */
export const BAKE_BINS = 15;

/**
 * The bin shares a worker takes of a band (the module's remarks, "Bins"): two, so that a band's
 * uneven bins (the generated Earth's 2 to 21 terms) balance over the workers as the queue hands
 * them out.
 */
const SHARES_A_WORKER = 2;

/** A bake job posted as any is: nothing transferred, at the queue's end. */
const NOT_AHEAD: { readonly transfer: ArrayBuffer[]; readonly ahead: boolean } = {
  transfer: [],
  ahead: false,
};

/** A bake's failure from what was thrown: a bug in what it was asked, with its message. */
function failedWith(error: unknown): { readonly kind: "failed"; readonly message: string } {
  return { kind: "failed", message: error instanceof Error ? error.message : String(error) };
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
