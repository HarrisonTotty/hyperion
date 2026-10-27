/**
 * What an optics worker answers, as one worker's state over its requests (plan R08, R08.T10.a;
 * Design notes 1 and 14; `decision-r08-t4d-kdist-chain.md` §15, ruling 3).
 *
 * @remarks
 * The worker (`optics.worker.ts`) holds no logic of its own: it hands each message to
 * {@link OpticsJobs.answer}, so that a Node-environment test drives the same answers, as
 * `heightBake.ts` does for the height worker.
 *
 * - **The k-tables.** Each line species' tables are loaded once a worker, on the first request
 *   whose input lists it (`assemble.ts`'s `lineSpeciesOf`), and kept for the worker's life: in its
 *   heap, not the per-planet row (Design note 11).
 * - **The media.** The last {@link OPTICS_MEDIA_KEPT} bodies' assemblies are kept by their key
 *   (`atmosphereKey`), so that a worker that assembled a body serves its curve shares and join
 *   from the same specs. A worker asked for a share of a body it has not assembled forms the
 *   body's absorber specs alone (`absorberSpecsOf`, with no aerosol optics), equal to the
 *   assembly's, and keeps those.
 * - **The shares.** A share is `absorbers.ts`'s `absorberCurveSums` of each absorber under the
 *   lights asked, and the join `absorberCurvesFromSums` of every share's, of which only each
 *   light's channel curves are answered: the per-planet tables read those, and the bake bins'
 *   curves, about 1 MB an absorber and sun, are left to R08.T14's bakes.
 * - **The thick bake** (R08.T14.b; `thick/bake.ts`): the body's absorber specs as a share takes
 *   them, each of the column's absorber terms matched to the spec of its name, its bin curves under
 *   the lights asked formed here (`absorberCurvesBySun`, the lines' exponentials shared by the
 *   lights), the column's optics and the fits prepared once a request, and one table a band asked.
 * - **The polarisation correction** (R08.T14.e; `thick/polarisation.ts`): its lights formed as the
 *   thick bake's, its own preparation (the column's optics at the render channels and the reference
 *   bin, the thick bake's layers, and the fits in the channels' three bins) once a request, and one
 *   table a band asked. A thick world asks for both: they share no work but the layers' rule and
 *   the absorber specs a worker keeps.
 * - **The answer** is a structured copy of the reply, whose every buffer is transferred: the copy
 *   is made in the worker, off the render thread, and leaves the worker's own arrays (its kept
 *   assemblies, and any table a medium references, such as a cross-section's) attached.
 *
 * A throw in a job is a bug (`decision-r08-t10a-leans.md` item 6) and is answered `failed` with its
 * message, but for a bake's, which is answered `thick-bake-failed` or `polarisation-bake-failed`: a
 * failed bake is never the medium's fault (decision-r08-t10a-leans, UX review item 1). A k-table
 * that cannot be loaded rejects, so that the worker raises its error and the pool replaces it.
 */

import {
  absorberCurvesBySun,
  absorberCurvesFromSums,
  absorberCurveSums,
  type AbsorberSpec,
} from "./absorbers";
import type { KDistributionTable } from "./absorbers/kDistribution";
import {
  absorberSpecsOf,
  type AssembledMedium,
  assembleMedium,
  type AssembleOptions,
  curveSlotsOf,
  lineSpeciesOf,
} from "./assemble";
import { atmosphereKey } from "./atmosphereInput";
import {
  type OpticsAnswer,
  type OpticsReply,
  type OpticsRequest,
  transferablesOf,
} from "./opticsMessages";
import type { BakeSpectrum } from "./spectralColour";
import { bakeMultipleScattering, prepareBake, type ThickBakeLight } from "./thick/bake";
import { bakePolarisationCorrection, preparePolarisation } from "./thick/polarisation";

/** How many bodies' assemblies, or absorber specs, a worker keeps, by key, the latest asked. */
export const OPTICS_MEDIA_KEPT = 2;

/** What a worker's jobs take from outside: how its k-tables are read. */
export interface OpticsJobsDeps {
  /**
   * The committed k-distribution tables of `species` that the client holds, leaving out a species
   * it holds none for: `absorbers/kDistribution.ts`'s `loadKDistributions` over `fetchPart` in the
   * worker, a test's reader of inlined parts or its toy tables in a test.
   */
  readonly loadKDistributions: (
    species: ReadonlyArray<string>,
  ) => Promise<ReadonlyArray<KDistributionTable>>;
}

/** A body's absorber specs and its light slots' spectra, as a worker keeps them. */
interface HeldAbsorbers {
  readonly specs: ReadonlyArray<AbsorberSpec>;
  readonly spectra: ReadonlyArray<BakeSpectrum>;
}

/** What a worker keeps of a body: its assembly where it made one, and its absorbers. */
interface HeldBody {
  readonly assembled: AssembledMedium | null;
  readonly absorbers: HeldAbsorbers;
}

/** One optics worker's state and its answers (see the module's remarks). */
export class OpticsJobs {
  readonly #load: OpticsJobsDeps["loadKDistributions"];
  /** Each line species' table as loaded, or `null` where the client holds none. */
  readonly #tables = new Map<string, Promise<KDistributionTable | null>>();
  /** The bodies kept, by key, least recently asked first. */
  readonly #bodies = new Map<string, HeldBody>();

  constructor(deps: OpticsJobsDeps) {
    this.#load = deps.loadKDistributions;
  }

  /** The line species whose tables the worker has loaded or is loading, in the order first asked. */
  get loadedSpecies(): ReadonlyArray<string> {
    return [...this.#tables.keys()];
  }

  /**
   * Answers one request with its reply and the buffers to transfer with it.
   *
   * @throws Error, rejecting, when a k-table the input needs cannot be loaded: the worker's error.
   */
  async answer(request: OpticsRequest): Promise<OpticsAnswer> {
    let species: ReadonlyArray<string>;
    try {
      species = lineSpeciesOf(request.input);
    } catch (error: unknown) {
      return failed(request, error);
    }
    const kDistributions = await this.#tablesOf(species);
    let reply: OpticsReply;
    try {
      reply = structuredClone(this.#reply(request, { kDistributions }));
    } catch (error: unknown) {
      return failed(request, error);
    }
    return { reply, transfer: transferablesOf(reply) };
  }

  /** The reply to `request`, made with `options`' tables; throws for a bug. */
  #reply(request: OpticsRequest, options: AssembleOptions): OpticsReply {
    let reply: OpticsReply;
    switch (request.kind) {
      case "assemble":
        reply = { kind: "assembled", id: request.id, assembled: this.#assembled(request, options) };
        break;
      case "curve-sums": {
        const { specs, suns } = this.#absorbersUnder(request, options);
        const sums = specs.map((spec) => absorberCurveSums(spec, suns, request.share));
        reply = { kind: "curve-sums", id: request.id, sums };
        break;
      }
      case "thick-bake": {
        const lights = this.#bakeLights(request, options);
        const prepared = prepareBake(request.column, lights);
        reply = {
          kind: "thick-baked",
          id: request.id,
          tables: request.bands.map((band) => bakeMultipleScattering(prepared, request.body, band)),
        };
        break;
      }
      case "polarisation-bake": {
        const lights = this.#bakeLights(request, options);
        const prepared = preparePolarisation(request.column, lights);
        reply = {
          kind: "polarisation-baked",
          id: request.id,
          tables: request.bands.map((band) =>
            bakePolarisationCorrection(prepared, request.body, band),
          ),
        };
        break;
      }
      case "curve-join": {
        const { specs, suns } = this.#absorbersUnder(request, options);
        const perAbsorber = specs.map((spec, k) =>
          absorberCurvesFromSums(
            spec,
            suns,
            request.sums.map((share, s) => {
              const sums = share[k];
              if (sums === undefined) {
                throw new RangeError(`share ${s} holds no sums for absorber ${k}`);
              }
              return sums;
            }),
          ),
        );
        reply = { kind: "curves", id: request.id, curves: curveSlotsOf(perAbsorber, suns.length) };
        break;
      }
    }
    return reply;
  }

  /** The body's assembly, made once and kept. */
  #assembled(
    request: Extract<OpticsRequest, { kind: "assemble" }>,
    options: AssembleOptions,
  ): AssembledMedium {
    const key = atmosphereKey(request.input);
    const held = this.#take(key);
    if (held !== undefined && held.assembled !== null) {
      return held.assembled;
    }
    const assembled = assembleMedium(request.input, options);
    const absorbers: HeldAbsorbers =
      assembled.medium === undefined
        ? { specs: [], spectra: [] }
        : { specs: assembled.drawn.absorbers, spectra: assembled.drawn.spectra };
    this.#keep(key, { assembled, absorbers });
    return assembled;
  }

  /** The body's absorber specs and the spectra of the lights asked, kept where formed. */
  #absorbersUnder(
    request: Extract<OpticsRequest, { kind: "curve-sums" | "curve-join" }>,
    options: AssembleOptions,
  ): { readonly specs: ReadonlyArray<AbsorberSpec>; readonly suns: ReadonlyArray<BakeSpectrum> } {
    const { specs, spectra } = this.#heldAbsorbers(request, options);
    if (specs.length === 0) {
      return { specs, suns: [] };
    }
    return { specs, suns: spectraOf(spectra, request.slots) };
  }

  /** The body's absorber specs and its lights' spectra, kept where formed. */
  #heldAbsorbers(
    request: Extract<
      OpticsRequest,
      { kind: "curve-sums" | "curve-join" | "thick-bake" | "polarisation-bake" }
    >,
    options: AssembleOptions,
  ): HeldAbsorbers {
    const key = atmosphereKey(request.input);
    let held = this.#take(key);
    if (held === undefined) {
      held = { assembled: null, absorbers: absorberSpecsOf(request.input, options) };
      this.#keep(key, held);
    }
    return held.absorbers;
  }

  /**
   * A bake's lights (the module's remarks): each light asked, with the bin curves of the column's
   * absorber terms, each the body's absorber spec of its name; throws for a column absorber the
   * body does not have.
   */
  #bakeLights(
    request: Extract<OpticsRequest, { kind: "thick-bake" | "polarisation-bake" }>,
    options: AssembleOptions,
  ): ReadonlyArray<ThickBakeLight> {
    const { specs, spectra } = this.#heldAbsorbers(request, options);
    const suns = spectraOf(spectra, request.slots);
    const absorbers = request.column.terms.flatMap((term) => {
      if (term.source.kind !== "absorber") {
        return [];
      }
      const spec = specs.find((each) => each.name === term.name);
      if (spec === undefined) {
        throw new RangeError(`the body has no absorber ${term.name} for the bake's column`);
      }
      return [spec];
    });
    const curves = absorbers.map((spec) => absorberCurvesBySun(spec, suns));
    return suns.map((spectrum, j): ThickBakeLight => ({
      spectrum,
      absorbers: curves.map((bySun) => {
        const bins = bySun[j]?.bins;
        if (bins === undefined) {
          throw new RangeError(`an absorber has no curves for light ${j}`);
        }
        return bins;
      }),
    }));
  }

  /** The body kept under `key`, made the most recently asked. */
  #take(key: string): HeldBody | undefined {
    const held = this.#bodies.get(key);
    if (held !== undefined) {
      this.#bodies.delete(key);
      this.#bodies.set(key, held);
    }
    return held;
  }

  /** Keeps a body under `key`, letting the least recently asked go past {@link OPTICS_MEDIA_KEPT}. */
  #keep(key: string, held: HeldBody): void {
    this.#bodies.delete(key);
    this.#bodies.set(key, held);
    for (const oldest of this.#bodies.keys()) {
      if (this.#bodies.size <= OPTICS_MEDIA_KEPT) {
        break;
      }
      this.#bodies.delete(oldest);
    }
  }

  /** The tables of `species`, each loaded once a worker; rejects where a load fails. */
  async #tablesOf(species: ReadonlyArray<string>): Promise<ReadonlyArray<KDistributionTable>> {
    const tables = await Promise.all(
      species.map((key) => {
        let table = this.#tables.get(key);
        if (table === undefined) {
          const loading = this.#load([key]).then(
            (loaded) => loaded.find((each) => each.species === key) ?? null,
          );
          table = loading;
          this.#tables.set(key, loading);
          // A load that fails is forgotten, so that a later request loads it afresh; the failure
          // itself reaches the caller through the request that awaits it.
          void loading.catch(() => {
            if (this.#tables.get(key) === loading) {
              this.#tables.delete(key);
            }
          });
        }
        return table;
      }),
    );
    return tables.flatMap((table) => (table === null ? [] : [table]));
  }
}

/** The spectra of the light slots asked. */
function spectraOf(
  spectra: ReadonlyArray<BakeSpectrum>,
  slots: ReadonlyArray<number>,
): ReadonlyArray<BakeSpectrum> {
  return slots.map((slot) => {
    const spectrum = spectra[slot];
    if (spectrum === undefined) {
      throw new RangeError(`light slot ${slot} is not one of the body's ${spectra.length}`);
    }
    return spectrum;
  });
}

/**
 * A job's failure as its answer, nothing to transfer: a bug's message, `failed`, or a bake's own
 * failure, `thick-bake-failed` or `polarisation-bake-failed`, with the bands it asked.
 */
function failed(request: OpticsRequest, error: unknown): OpticsAnswer {
  const message = error instanceof Error ? error.message : String(error);
  let reply: OpticsReply;
  switch (request.kind) {
    case "thick-bake":
      reply = { kind: "thick-bake-failed", id: request.id, bands: request.bands, message };
      break;
    case "polarisation-bake":
      reply = { kind: "polarisation-bake-failed", id: request.id, bands: request.bands, message };
      break;
    case "assemble":
    case "curve-sums":
    case "curve-join":
      reply = { kind: "failed", id: request.id, message };
      break;
  }
  return { reply, transfer: [] };
}
