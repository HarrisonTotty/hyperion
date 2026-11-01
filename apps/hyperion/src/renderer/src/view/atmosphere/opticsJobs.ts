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
 * - **The assembly** is made under {@link OPTICS_ASSEMBLY}: a gas envelope sent `ok` takes its
 *   medium (R08.T16.a.2).
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
 * - **A kept body's bake** (R08.T14.c): its request carries the medium's absorber specs and its
 *   lights' spectra (`opticsMessages.ts`'s `KeptBakeLights`), which a kept input's assembly cannot
 *   give, and its lights are formed from those as an assembled body's are.
 * - **The polarisation correction** (R08.T14.e; `thick/polarisation.ts`): its lights formed as the
 *   thick bake's, its own preparation (the column's optics at the render channels and the reference
 *   bin, the thick bake's layers, and the fits in the channels' three bins) once a request, and one
 *   table a band asked. A thick world asks for both: they share no work but the layers' rule and
 *   the absorber specs a worker keeps.
 * - **The disc's table** (R08.T16.b; `thick/discBake.ts`): its lights formed as the thick bake's,
 *   J the routed bake's where the request carries it and otherwise baked here as a `thick-bake`
 *   bakes it, the correction summed in where carried, then every band's and light's table marched;
 *   or, for a `disc-band-bake`, one band's alone, which the pool gathers (R08.T14.d).
 * - **The deck** (R08.T15.a.2; `thick/deck.ts`): its lights formed as the thick bake's, the deck
 *   prepared once a request (`prepareDeck`, the upper medium's thick-bake preparation among it),
 *   each band asked baked (`bakeDeck`), and where the column needs it the correction over the deck
 *   prepared once and baked a band (`bakeDeckPolarisation`), so that one job carries everything the
 *   deck's route reads.
 * - **Bin and channel shares** (R08.T14.d's bake time; the pool's remarks, "Bins"): a
 *   `thick-bake-bins` or `deck-bake-bins` prepares and bakes its bins alone (`thick/bake.ts`'s
 *   `prepareBake` and `bakeBins`, `thick/deck.ts`'s `bakeDeckBins`), a `polarisation-bake-channel`
 *   or `deck-polarisation-channel` its render channel alone, and a gather converts the shares to
 *   the band's tables. A body's bake lights are kept with its specs, by their slots and the
 *   column's absorber terms, so that its other shares on a worker form none again.
 * - **The answer** is a structured copy of the reply, whose every buffer is transferred: the copy
 *   is made in the worker, off the render thread, and leaves the worker's own arrays (its kept
 *   assemblies, and any table a medium references, such as a cross-section's) attached.
 *
 * A throw in a job is a bug (`decision-r08-t10a-leans.md` item 6) and is answered `failed` with its
 * message, but for a bake's, which is answered `thick-bake-failed`, `polarisation-bake-failed`,
 * `disc-bake-failed` (a disc's band's too) or `deck-bake-failed`: a failed bake is never the
 * medium's fault (decision-r08-t10a-leans, UX review item 1). A k-table that cannot be loaded
 * rejects, so that the worker raises its error and the pool replaces it.
 */

import { absorberCurvesFromSums, absorberCurveSums, type AbsorberSpec } from "./absorbers";
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
import type { PolarisationTable } from "./thick/polarisation";
import {
  type BakeBandOptions,
  bakeBins,
  bakeMultipleScattering,
  bakePointsOf,
  gatherBins,
  type MsSourceTable,
  NO_OWN_CHANNELS,
  type OwnChannels,
  ownWavelengthChannels,
  prepareBake,
  THICK_BAKE_REFERENCE_BIN,
  type ThickBakeBand,
  thickBakeBands,
  type ThickBakeLight,
  thickBakeLights,
} from "./thick/bake";
import {
  bakeDeck,
  bakeDeckBins,
  bakeDeckPolarisation,
  deckPolarisationChannel,
  gatherDeckBins,
  prepareDeck,
  prepareDeckPolarisation,
} from "./thick/deck";
import { discBandFor, discReflectanceFor } from "./thick/discBake";
import {
  bakePolarisationCorrection,
  gatherChannels,
  needsPolarisationCorrection,
  type PolarisationChannelShare,
  polarisationChannel,
  preparePolarisation,
} from "./thick/polarisation";
import { BAKE_BIN_COUNT, CHANNEL_WAVELENGTHS_NM } from "./medium";

/** How many bodies' assemblies, or absorber specs, a worker keeps, by key, the latest asked. */
export const OPTICS_MEDIA_KEPT = 2;

/**
 * What every assembly a worker makes takes beside its k-tables (R08.T16.a.2): a gas envelope sent
 * `ok` drawn ({@link AssembleOptions.envelopes}), split at its floor (`thick/regime.ts`'s
 * `classifyRegime`), its deck baked where its gate opens the route (`thick/routing.ts`'s
 * `deckRouted`) and drawn by the analytic term under `ATMOSPHERE: APPROXIMATE` until then.
 */
export const OPTICS_ASSEMBLY: AssembleOptions = { envelopes: true };

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

/**
 * What a worker keeps of a body: its assembly where it made one, its absorbers, and the bake lights
 * formed of them, by their slots and the column's absorber terms.
 */
interface HeldBody {
  readonly assembled: AssembledMedium | null;
  readonly absorbers: HeldAbsorbers;
  readonly lights: Map<string, ReadonlyArray<ThickBakeLight>>;
}

/** One optics worker's state and its answers (see the module's remarks). */
export class OpticsJobs {
  readonly #load: OpticsJobsDeps["loadKDistributions"];
  /** Each line species' tables as loaded: none, one, or a temperate one and the hot one continuing it. */
  readonly #tables = new Map<string, Promise<ReadonlyArray<KDistributionTable>>>();
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
      species =
        request.kind === "thick-gather" ||
        request.kind === "polarisation-gather" ||
        request.kind === "deck-gather"
          ? []
          : lineSpeciesOf(request.input, OPTICS_ASSEMBLY);
    } catch (error: unknown) {
      return failed(request, error);
    }
    const kDistributions = await this.#tablesOf(species);
    let reply: OpticsReply;
    try {
      reply = structuredClone(this.#reply(request, { ...OPTICS_ASSEMBLY, kDistributions }));
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
        const own = ownChannelsOf(request, lights);
        const prepared = prepareBake(
          request.column,
          lights,
          undefined,
          undefined,
          undefined,
          bakePointsOf(own),
          own,
        );
        reply = {
          kind: "thick-baked",
          id: request.id,
          tables: request.bands.map((band) =>
            bakeMultipleScattering(prepared, request.body, band, bandOptionsOf(request)),
          ),
        };
        break;
      }
      case "thick-bake-bins": {
        const band = oneBandOf(request.bands);
        const lights = this.#bakeLights(request, options);
        const own = ownChannelsOf(request, lights);
        // The share with bin 0 bakes the own channels' points beside its bins.
        const points = request.bins.includes(0)
          ? bakePointsOf(own).filter((b) => b >= BAKE_BIN_COUNT)
          : [];
        const prepared = prepareBake(
          request.column,
          lights,
          undefined,
          undefined,
          undefined,
          [...request.bins, ...points],
          own,
        );
        reply = {
          kind: "thick-bins-baked",
          id: request.id,
          share: bakeBins(prepared, request.body, band, bandOptionsOf(request)),
        };
        break;
      }
      case "thick-gather":
        reply = { kind: "thick-baked", id: request.id, tables: [gatherBins(request.shares)] };
        break;
      case "disc-bake": {
        const table = discReflectanceFor(request.air, discBandsOf(request), () =>
          this.#discJ(request, options),
        );
        reply = { kind: "disc-baked", id: request.id, table };
        break;
      }
      case "disc-band-bake": {
        const values = discBandFor(request.air, discBandsOf(request), request.part, () =>
          this.#discJ(request, options),
        );
        reply = { kind: "disc-band-baked", id: request.id, values };
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
      case "polarisation-bake-channel": {
        const band = oneBandOf(request.bands);
        const wavelengthNm = CHANNEL_WAVELENGTHS_NM[request.channel];
        const lights = this.#bakeLights(request, options);
        const prepared = preparePolarisation(request.column, lights, {
          wavelengthsNm: [wavelengthNm],
        });
        reply = {
          kind: "polarisation-channel-baked",
          id: request.id,
          share: polarisationChannel(prepared, request.body, band, 0),
        };
        break;
      }
      case "polarisation-gather":
        reply = {
          kind: "polarisation-baked",
          id: request.id,
          tables: [gatherChannels(request.shares)],
        };
        break;
      case "deck-bake": {
        const lights = this.#bakeLights(request, options);
        const prepared = prepareDeck(request.column, lights, request.split);
        const tables = request.bands.map((band) => bakeDeck(prepared, request.body, band));
        let correction: ReadonlyArray<PolarisationTable> | null = null;
        if (needsPolarisationCorrection(request.column)) {
          const polarisation = prepareDeckPolarisation(prepared, lights);
          correction = request.bands.map((band) =>
            bakeDeckPolarisation(prepared, polarisation, request.body, band),
          );
        }
        reply = { kind: "deck-baked", id: request.id, tables, correction };
        break;
      }
      case "deck-bake-bins": {
        const band = oneBandOf(request.bands);
        const lights = this.#bakeLights(request, options);
        const prepared = prepareDeck(request.column, lights, request.split, {
          bins: request.bins,
        });
        reply = {
          kind: "deck-bins-baked",
          id: request.id,
          share: bakeDeckBins(prepared, request.body, band),
        };
        break;
      }
      case "deck-polarisation-channel": {
        const band = oneBandOf(request.bands);
        const wavelengthNm = CHANNEL_WAVELENGTHS_NM[request.channel];
        let share: PolarisationChannelShare | null = null;
        if (needsPolarisationCorrection(request.column)) {
          const lights = this.#bakeLights(request, options);
          // The correction reads the deck's layers alone: one bin of the upper medium is prepared,
          // the reference bin, whose optics its layers are placed by.
          const prepared = prepareDeck(request.column, lights, request.split, {
            bins: [THICK_BAKE_REFERENCE_BIN],
          });
          const polarisation = prepareDeckPolarisation(prepared, lights, {
            wavelengthsNm: [wavelengthNm],
          });
          share = deckPolarisationChannel(prepared, polarisation, request.body, band, 0);
        }
        reply = { kind: "deck-channel-baked", id: request.id, share };
        break;
      }
      case "deck-gather":
        reply = {
          kind: "deck-baked",
          id: request.id,
          tables: [gatherDeckBins(request.decks)],
          correction: request.channels === null ? null : [gatherChannels(request.channels)],
        };
        break;
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
    this.#keep(key, { assembled, absorbers, lights: new Map() });
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
      {
        kind:
          | "curve-sums"
          | "curve-join"
          | "thick-bake"
          | "thick-bake-bins"
          | "polarisation-bake"
          | "polarisation-bake-channel"
          | "disc-bake"
          | "disc-band-bake"
          | "deck-bake"
          | "deck-bake-bins"
          | "deck-polarisation-channel";
      }
    >,
    options: AssembleOptions,
  ): HeldAbsorbers {
    return this.#held(request, options).absorbers;
  }

  /** The body a request asks of, its absorber specs formed and kept where none was. */
  #held(request: Extract<OpticsRequest, { input: unknown }>, options: AssembleOptions): HeldBody {
    const key = atmosphereKey(request.input);
    let held = this.#take(key);
    if (held === undefined) {
      held = {
        assembled: null,
        absorbers: absorberSpecsOf(request.input, options),
        lights: new Map(),
      };
      this.#keep(key, held);
    }
    return held;
  }

  /** A disc's J where its request carries none: its bands' thick bake, as a `thick-bake` bakes. */
  #discJ(
    request: Extract<OpticsRequest, { kind: "disc-bake" | "disc-band-bake" }>,
    options: AssembleOptions,
  ): ReadonlyArray<MsSourceTable> {
    if (request.air.deck !== undefined) {
      // A deck's J is its own bake's, above the deck as its boundary: no solve here makes it.
      throw new RangeError("a disc over a deck reads its deck's J, which its request carries");
    }
    // The lights only where J is solved here: a routed bake's carries its own.
    const prepared = prepareBake(request.column, this.#bakeLights(request, options));
    return request.bands.map((band) => bakeMultipleScattering(prepared, request.body, band));
  }

  /**
   * A bake's lights (the module's remarks; `thick/bake.ts`'s `thickBakeLights`): each light asked,
   * with the bin curves of the column's absorber terms, each the body's absorber spec of its name,
   * the body's as assembled or, for a kept body, the request's `kept` ones; throws for a column
   * absorber the body does not have.
   */
  #bakeLights(
    request: Extract<
      OpticsRequest,
      {
        kind:
          | "thick-bake"
          | "thick-bake-bins"
          | "polarisation-bake"
          | "polarisation-bake-channel"
          | "disc-bake"
          | "disc-band-bake"
          | "deck-bake"
          | "deck-bake-bins"
          | "deck-polarisation-channel";
      }
    >,
    options: AssembleOptions,
  ): ReadonlyArray<ThickBakeLight> {
    if (request.kept !== undefined) {
      const { absorbers, spectra } = request.kept;
      return thickBakeLights(request.column, absorbers, spectraOf(spectra, request.slots));
    }
    const held = this.#held(request, options);
    // The lights depend on the slots and the column's absorber terms alone, beside the body's specs
    // and spectra that `held` keeps: a band's other bins' jobs on this worker take them as formed.
    const key = [
      request.slots.join(","),
      ...request.column.terms.flatMap((term) =>
        term.source.kind === "absorber" ? [term.name] : [],
      ),
    ].join("|");
    let lights = held.lights.get(key);
    if (lights === undefined) {
      const { specs, spectra } = held.absorbers;
      lights = thickBakeLights(request.column, specs, spectraOf(spectra, request.slots));
      held.lights.set(key, lights);
    }
    return lights;
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

  /**
   * The tables of `species`, each species' loaded once a worker (two where a hot table continues a
   * temperate one); rejects where a load fails.
   */
  async #tablesOf(species: ReadonlyArray<string>): Promise<ReadonlyArray<KDistributionTable>> {
    const tables = await Promise.all(
      species.map((key) => {
        let held = this.#tables.get(key);
        if (held === undefined) {
          const loading = this.#load([key]).then((loaded) =>
            loaded.filter((each) => each.species === key),
          );
          held = loading;
          this.#tables.set(key, loading);
          // A load that fails is forgotten, so that a later request loads it afresh; the failure
          // itself reaches the caller through the request that awaits it.
          void loading.catch(() => {
            if (this.#tables.get(key) === loading) {
              this.#tables.delete(key);
            }
          });
        }
        return held;
      }),
    );
    return tables.flat();
  }
}

/**
 * The channels a thick bake's request solves at their own wavelengths: those its lights allow where
 * it asks them (`thick/bake.ts`'s `ownWavelengthChannels`, R08.T14.d.4), none where it does not.
 */
function ownChannelsOf(
  request: { readonly ownWavelengths?: boolean },
  lights: ReadonlyArray<ThickBakeLight>,
): OwnChannels {
  return request.ownWavelengths === true ? ownWavelengthChannels(lights) : NO_OWN_CHANNELS;
}

/** A thick bake's request's band options: its streams' curved paths where it asks them (R08.T14.d.4). */
function bandOptionsOf(request: { readonly curvedStreams?: boolean }): BakeBandOptions {
  return request.curvedStreams === true ? { curvedStreams: true } : {};
}

/** A share's one band, or a refusal: a share is of one band. */
function oneBandOf(bands: ReadonlyArray<number>): number {
  const [band, ...others] = bands;
  if (band === undefined || others.length > 0) {
    throw new RangeError(`a share is of one band, not ${bands.length}`);
  }
  return band;
}

/** A disc's request's bands, `bake.ts`'s `thickBakeBands` of its body at the indices it asks. */
function discBandsOf(
  request: Extract<OpticsRequest, { kind: "disc-bake" | "disc-band-bake" }>,
): ReadonlyArray<ThickBakeBand> {
  const bands = thickBakeBands(request.body);
  return request.bands.map((band) => {
    const geometry = bands[band];
    if (geometry === undefined) {
      throw new RangeError(`the body has ${bands.length} latitude bands, not a band ${band}`);
    }
    return geometry;
  });
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
 * failure, `thick-bake-failed`, `polarisation-bake-failed` or `deck-bake-failed`, with the bands it
 * asked.
 */
function failed(request: OpticsRequest, error: unknown): OpticsAnswer {
  const message = error instanceof Error ? error.message : String(error);
  let reply: OpticsReply;
  switch (request.kind) {
    case "thick-bake":
    case "thick-bake-bins":
      reply = { kind: "thick-bake-failed", id: request.id, bands: request.bands, message };
      break;
    case "thick-gather":
      reply = { kind: "thick-bake-failed", id: request.id, bands: [], message };
      break;
    case "polarisation-bake":
    case "polarisation-bake-channel":
      reply = { kind: "polarisation-bake-failed", id: request.id, bands: request.bands, message };
      break;
    case "polarisation-gather":
      reply = { kind: "polarisation-bake-failed", id: request.id, bands: [], message };
      break;
    case "disc-bake":
    case "disc-band-bake":
      reply = { kind: "disc-bake-failed", id: request.id, message };
      break;
    case "deck-bake":
    case "deck-bake-bins":
    case "deck-polarisation-channel":
      reply = { kind: "deck-bake-failed", id: request.id, bands: request.bands, message };
      break;
    case "deck-gather":
      reply = { kind: "deck-bake-failed", id: request.id, bands: [], message };
      break;
    case "assemble":
    case "curve-sums":
    case "curve-join":
      reply = { kind: "failed", id: request.id, message };
      break;
  }
  return { reply, transfer: [] };
}
