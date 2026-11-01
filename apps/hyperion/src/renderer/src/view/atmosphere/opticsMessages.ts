/**
 * The messages between the render thread's optics pool (`opticsPool.ts`) and its workers
 * (`optics.worker.ts`), and the buffers each transfers (plan R08, R08.T10.a; Design notes 1 and
 * 14).
 *
 * @remarks
 * A light module: beside type imports it holds only {@link transferablesOf} and
 * {@link keptBakeLightsOf}, so that the render thread's pool takes nothing of the optics' tables. Each request carries the body's input
 * whole, so that any worker can serve it from its own k-tables and its own assembly
 * (`opticsJobs.ts`); a join's request carries the shares' sums, transferred from the render
 * thread, which received them transferred.
 */

import type { AbsorberCurve, AbsorberCurveSums, AbsorberSpec, CurveShare } from "./absorbers";
import type { AssembledMedium } from "./assemble";
import type { BodyAtmosphereInput } from "./atmosphereInput";
import type { DiscBandValues, DiscReflectanceTable } from "./discTable";
import type { AtmosphereMedium, RenderChannel } from "./medium";
import type { BakeSpectrum } from "./spectralColour";
import type { MsBinShare, MsSourceTable, ThickBakeBody } from "./thick/bake";
import type { DiscBakeAir } from "./thick/discBake";
import type { DeckBinShare, DeckSplit, DeckTables } from "./thick/deck";
import type { PolarisationChannelShare, PolarisationTable } from "./thick/polarisation";
import type { SpectralColumn } from "./thick/spectralColumn";

/**
 * What the pool asks of an optics worker.
 *
 * - `assemble`: the body's medium and labels (`assemble.ts`'s `assembleMedium`), with no curves.
 * - `curve-sums`: one share of the curves of the medium's absorbers under the lights in `slots`
 *   (`absorbers.ts`'s `absorberCurveSums`; `decision-r08-t4d-kdist-chain.md` §15, ruling 3).
 * - `curve-join`: the curves under the lights in `slots` from every share's sums
 *   (`absorberCurvesFromSums`), `sums[share][absorber]`.
 * - `thick-bake` (R08.T14.b): the thick multiple-scattering tables (`thick/bake.ts`) of `column`
 *   on `body`, one a latitude band in `bands` (indices of `thickBakeBands(body)`), each with a
 *   table a light in `slots`. The column's absorber terms take the input's absorber specs of their
 *   names, which the worker forms itself (`absorberSpecsOf`, as a curve share does), and the bin
 *   curves under each light.
 * - `thick-bake-bins` (the pool's bin shares, R08.T14.d): a `thick-bake` of one band in `bins`
 *   alone, ascending bake bins, its preparation of those bins and the reference bin, answered
 *   before the channels (`thick/bake.ts`'s `MsBinShare`).
 * - `thick-gather`: one band's thick table from its bin shares, which together hold every bin once
 *   (`thick/bake.ts`'s `gatherBins`), the shares transferred.
 * - `polarisation-bake` (R08.T14.e): the polarisation correction's tables
 *   (`thick/polarisation.ts`) of `column` on `body`, laid out as `thick-bake`'s and with its
 *   absorbers formed the same way, for any world with air, thin or thick.
 * - `polarisation-bake-channel` (the pool's channel shares, R08.T14.d): a `polarisation-bake` of
 *   one band in one render channel, `channel`, prepared at that channel's wavelength alone and
 *   answered before the table (`thick/polarisation.ts`'s `PolarisationChannelShare`).
 * - `polarisation-gather`: one band's correction table from its three channels' shares
 *   (`thick/polarisation.ts`'s `gatherChannels`), the shares transferred.
 * - `disc-bake` (R08.T16.b): the disc's reflectance table (`thick/discBake.ts`) of every band in
 *   `bands` and light in `slots`, `thick-bake`'s fields with the medium as the tables hold it and
 *   the body's bakes that have landed (`air`): J from its routed bake where given, else baked as
 *   `thick-bake` bakes it.
 * - `disc-band-bake` (the pool's bands, R08.T14.d): one band of a `disc-bake`'s table, `part`, an
 *   index of `bands`, with every field of the whole table's request, the sun's floor and the
 *   thermal source being every band's.
 * - `deck-bake-bins` (the pool's bin shares, R08.T14.d): a `deck-bake` of one band in `bins` alone,
 *   answered before the channels (`thick/deck.ts`'s `DeckBinShare`).
 * - `deck-polarisation-channel`: a `deck-bake`'s correction over the deck of one band in one render
 *   channel, `channel`, answered before the table, or as none where the column needs none.
 * - `deck-gather`: one band's deck tables from its bin shares and its correction from its channel
 *   shares (`thick/deck.ts`'s `gatherDeckBins`), the shares transferred.
 * - `deck-bake` (R08.T15.a.2): a cloud-deck split's tables (`thick/deck.ts`) of `column` on `body`
 *   at `split`, laid out and lit as `thick-bake`'s: the deck prepared once, each band's
 *   `bakeDeck`, and, where the column needs it (`needsPolarisationCorrection`), each band's
 *   polarisation correction over the deck.
 *
 * `slots` index the input's lights, which are its light slots in `lightsAt`'s order. A kept
 * body's bake (`input.kind === "kept"`, whose medium the scene built and no assembly gives specs or
 * spectra) carries its own in `kept` ({@link KeptBakeLights}), which `slots` then index.
 */
export type OpticsRequest =
  | { readonly kind: "assemble"; readonly id: number; readonly input: BodyAtmosphereInput }
  | {
      readonly kind: "curve-sums";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly share: CurveShare;
    }
  | {
      readonly kind: "curve-join";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly sums: ReadonlyArray<ReadonlyArray<AbsorberCurveSums>>;
    }
  | {
      readonly kind: "thick-bake";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
      /**
       * Whether the bake solves the channels its lights allow at their own wavelengths, a haze
       * route's (`thick/bake.ts`'s remarks, "Own wavelengths"; R08.T14.d.4): none where absent.
       */
      readonly ownWavelengths?: boolean;
    }
  | {
      readonly kind: "thick-bake-bins";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
      /** As `thick-bake`'s; the share with bin 0 bakes the own channels' points beside its bins. */
      readonly ownWavelengths?: boolean;
      /** The bake bins baked, ascending. */
      readonly bins: ReadonlyArray<number>;
    }
  | {
      readonly kind: "thick-gather";
      readonly id: number;
      readonly shares: ReadonlyArray<MsBinShare>;
    }
  | {
      readonly kind: "polarisation-bake";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
    }
  | {
      readonly kind: "polarisation-bake-channel";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
      /** The render channel baked. */
      readonly channel: RenderChannel;
    }
  | {
      readonly kind: "polarisation-gather";
      readonly id: number;
      readonly shares: ReadonlyArray<PolarisationChannelShare>;
    }
  | {
      readonly kind: "disc-bake";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
      readonly air: DiscBakeAir;
    }
  | {
      readonly kind: "disc-band-bake";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
      readonly air: DiscBakeAir;
      /** The band baked: its index in `bands`. */
      readonly part: number;
    }
  | {
      readonly kind: "deck-bake";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      /** Where the split lies and how deep its deck is read (`thick/deck.ts`'s `DeckSplit`). */
      readonly split: DeckSplit;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
    }
  | {
      readonly kind: "deck-bake-bins";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      readonly split: DeckSplit;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
      /** The bake bins baked, ascending. */
      readonly bins: ReadonlyArray<number>;
    }
  | {
      readonly kind: "deck-polarisation-channel";
      readonly id: number;
      readonly input: BodyAtmosphereInput;
      readonly slots: ReadonlyArray<number>;
      readonly bands: ReadonlyArray<number>;
      readonly column: SpectralColumn;
      readonly body: ThickBakeBody;
      readonly split: DeckSplit;
      /** A kept body's absorber specs and light spectra, in place of the input's. */
      readonly kept?: KeptBakeLights;
      /** The render channel baked. */
      readonly channel: RenderChannel;
    }
  | {
      readonly kind: "deck-gather";
      readonly id: number;
      readonly decks: ReadonlyArray<DeckBinShare>;
      /** The correction's channel shares, none where the column needs no correction. */
      readonly channels: ReadonlyArray<PolarisationChannelShare> | null;
    };

/**
 * A kept body's bake lights (plan R08, R08.T14.c): what an assembled body's input gives a bake and a
 * kept body's cannot (`assemble.ts`'s `absorberSpecsOf` gives a kept input none), plain data that a
 * structured clone carries.
 */
export interface KeptBakeLights {
  /**
   * The medium's absorber terms' specs (`medium.ts`'s `TermAbsorber.spec`), each matched to the
   * column's absorber term of its name, as an assembled body's are.
   */
  readonly absorbers: ReadonlyArray<AbsorberSpec>;
  /** The light slots' spectra in the bake bins, in `lightsAt`'s order, which `slots` index. */
  readonly spectra: ReadonlyArray<BakeSpectrum>;
}

/**
 * A kept medium's bake lights: its absorber terms' specs in their order, and the spectra of the
 * lights it is drawn under (a kept scene's own suns; `reference/cases.ts`'s
 * `referenceCaseBakeLights` gives a case's).
 */
export function keptBakeLightsOf(
  medium: AtmosphereMedium,
  spectra: ReadonlyArray<BakeSpectrum>,
): KeptBakeLights {
  return {
    absorbers: medium.terms.flatMap((term) =>
      term.absorber === undefined ? [] : [term.absorber.spec],
    ),
    spectra,
  };
}

/**
 * What an optics worker answers, by its request's `id`.
 *
 * - `assembled`: the medium and its labels, its curves empty.
 * - `curve-sums`: the share's sums, one an absorber in the medium's absorber terms' order; none for
 *   a body that draws no medium or has no absorber.
 * - `curves`: the light slots' channel curves, `curves[slot asked][absorber]`.
 * - `thick-baked`: a `thick-bake`'s tables, one a band asked in its order, their slots the lights
 *   asked in theirs.
 * - `thick-bins-baked`: a `thick-bake-bins`'s share of its band's bins.
 * - `thick-bake-failed`: a `thick-bake`, `thick-bake-bins` or `thick-gather` that threw or was
 *   refused, with its message and the bands asked (none for a gather). Never `failed`, which the pool reads as the medium's build failing: a failed bake is
 *   `ATMOSPHERE: APPROXIMATE`'s, never `ATMOSPHERE NOT AVAILABLE`'s (decision-r08-t10a-leans, UX
 *   review item 1).
 * - `polarisation-baked`: a `polarisation-bake`'s tables, one a band asked in its order, their
 *   slots the lights asked in theirs.
 * - `polarisation-channel-baked`: a `polarisation-bake-channel`'s share of its band.
 * - `polarisation-bake-failed`: a `polarisation-bake`, `polarisation-bake-channel` or
 *   `polarisation-gather` that threw or was refused, as `thick-bake-failed` is, never `failed`.
 * - `disc-baked`: a `disc-bake`'s table.
 * - `disc-band-baked`: a `disc-band-bake`'s band (`discTable.ts`'s `DiscBandValues`).
 * - `disc-bake-failed`: a `disc-bake` or `disc-band-bake` that threw or was refused, as
 *   `thick-bake-failed` is, never `failed`.
 * - `deck-baked`: a `deck-bake`'s tables, one a band asked in its order, their slots the lights
 *   asked in theirs, and the correction over the deck the same way, or `null` where the column
 *   needs none.
 * - `deck-bins-baked`: a `deck-bake-bins`'s share of its band's bins.
 * - `deck-channel-baked`: a `deck-polarisation-channel`'s share, or `null` where the column needs
 *   no correction.
 * - `deck-bake-failed`: a `deck-bake`, `deck-bake-bins`, `deck-polarisation-channel` or
 *   `deck-gather` that threw or was refused, as `thick-bake-failed` is, never `failed`.
 * - `failed`: the job threw, a bug (`decision-r08-t10a-leans.md` item 6), with its message.
 */
export type OpticsReply =
  | { readonly kind: "assembled"; readonly id: number; readonly assembled: AssembledMedium }
  | {
      readonly kind: "curve-sums";
      readonly id: number;
      readonly sums: ReadonlyArray<AbsorberCurveSums>;
    }
  | {
      readonly kind: "curves";
      readonly id: number;
      readonly curves: ReadonlyArray<ReadonlyArray<AbsorberCurve>>;
    }
  | {
      readonly kind: "thick-baked";
      readonly id: number;
      readonly tables: ReadonlyArray<MsSourceTable>;
    }
  | { readonly kind: "thick-bins-baked"; readonly id: number; readonly share: MsBinShare }
  | {
      readonly kind: "thick-bake-failed";
      readonly id: number;
      readonly bands: ReadonlyArray<number>;
      readonly message: string;
    }
  | {
      readonly kind: "polarisation-baked";
      readonly id: number;
      readonly tables: ReadonlyArray<PolarisationTable>;
    }
  | {
      readonly kind: "polarisation-channel-baked";
      readonly id: number;
      readonly share: PolarisationChannelShare;
    }
  | {
      readonly kind: "polarisation-bake-failed";
      readonly id: number;
      readonly bands: ReadonlyArray<number>;
      readonly message: string;
    }
  | { readonly kind: "disc-baked"; readonly id: number; readonly table: DiscReflectanceTable }
  | { readonly kind: "disc-band-baked"; readonly id: number; readonly values: DiscBandValues }
  | { readonly kind: "disc-bake-failed"; readonly id: number; readonly message: string }
  | {
      readonly kind: "deck-baked";
      readonly id: number;
      readonly tables: ReadonlyArray<DeckTables>;
      readonly correction: ReadonlyArray<PolarisationTable> | null;
    }
  | { readonly kind: "deck-bins-baked"; readonly id: number; readonly share: DeckBinShare }
  | {
      readonly kind: "deck-channel-baked";
      readonly id: number;
      readonly share: PolarisationChannelShare | null;
    }
  | {
      readonly kind: "deck-bake-failed";
      readonly id: number;
      readonly bands: ReadonlyArray<number>;
      readonly message: string;
    }
  | { readonly kind: "failed"; readonly id: number; readonly message: string };

/** A worker's reply and the buffers it transfers with it. */
export interface OpticsAnswer {
  readonly reply: OpticsReply;
  readonly transfer: ArrayBuffer[];
}

/**
 * Every distinct `ArrayBuffer` reachable from `value`, through objects, arrays, maps, sets and
 * typed arrays' buffers, each once, for a message's transfer list.
 *
 * @remarks
 * A `SharedArrayBuffer` is left out, since it is shared and not transferred. Listing a buffer
 * twice would make `postMessage` throw, so each is listed once however many arrays view it. The
 * walk is iterative, so a long array of objects cannot overflow the stack, and each object is
 * visited once, so a cycle ends it.
 */
export function transferablesOf(value: unknown): ArrayBuffer[] {
  const found = new Set<ArrayBuffer>();
  const seen = new WeakSet<object>();
  const stack: unknown[] = [value];
  while (stack.length > 0) {
    const item = stack.pop();
    if (typeof item !== "object" || item === null || seen.has(item)) {
      continue;
    }
    seen.add(item);
    if (item instanceof ArrayBuffer) {
      found.add(item);
    } else if (ArrayBuffer.isView(item)) {
      if (item.buffer instanceof ArrayBuffer) {
        found.add(item.buffer);
      }
    } else if (item instanceof Map) {
      for (const [key, member] of item) {
        stack.push(key, member);
      }
    } else if (item instanceof Set) {
      for (const member of item) {
        stack.push(member);
      }
    } else {
      stack.push(...Object.values(item));
    }
  }
  return [...found];
}
