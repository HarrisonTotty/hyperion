/**
 * The cloud-deck split's bakes (plan R08, R08.T15.a.1; Design notes 9 and 17): the deck as the
 * upper medium's reflecting boundary, its bidirectional reflectance from the discrete-ordinates
 * solver over the deck's own layers, the multiple-scattering table J_ms of the medium above it
 * (`bake.ts`), and R08.T14.e's polarisation correction over the same medium with the deck as a
 * depolarising ground (`polarisation.ts`), one latitude band at a time, spectral in, channels out.
 *
 * @remarks
 * **The split** (Design note 9, "The cloud-deck split"). Above the split's altitude (a deck's top,
 * or a gas envelope's floor; `regime.ts`'s `AtmosphereRegime`) the tables run with the deck as a
 * baked reflecting boundary, and a thinner layer above it stays a term of the upper medium. The
 * deck's layers are the medium below the split: down to the ground, its albedo beneath, for a deck
 * over a surface (Venus-class); for an envelope, no deeper than τ(550) {@link
 * DECK_FLOOR_OPTICAL_DEPTH} from the top, over a black floor (decision-deep-decks-split §3.3 item
 * 4, R08.T16.a's "reads the medium below it no deeper than τ(550) 1,000").
 *
 * **The layers.** The upper medium takes the thick bake's grid, its layers evenly in ξ between the
 * split and the top (`bake.ts`'s `bakeLayering` over a span), and the deck {@link DECK_LAYERS} of
 * its own between the floor and the split, by the same rule. Each light's exponential sums are
 * fitted over the whole column, the upper layers and then the deck's (`bake.ts`'s `BakeLayout`), so
 * that a term of a bin's gases is one g interval from the top to the floor, as correlated sums take
 * it (Design note 5): the deck's absorption in each solve is the same term's as the medium above.
 *
 * **The deck's reflectance** ({@link deckReflectance}) is the solver's own formal solution over the
 * deck's delta-M layers (`bake.ts`'s `deltaMLayer`, 16 streams): for a plane-parallel beam at μ′ of
 * flux 1 normal to itself, ρ^m(μ, μ′) = π I^m(0, +μ) ÷ μ′, I^m the upward intensity at the deck's
 * top at user cosine μ (K. Stamnes, S.-C. Tsay, W. Wiscombe and K. Jayaweera, Appl. Opt. 27 (1988)
 * 2502–2509, the user-angle integration; `discreteOrdinates.ts`'s `GroundReflectance` for the
 * boundary terms). It is read at every cosine the upper medium's solve asks: its quadrature nodes and
 * user cosines going out, its nodes and its beams' μ₀ coming in. Taken so, it is the deck's discrete
 * reflection itself: a slab's reflectance as the ground gives the whole column's plane-parallel
 * solve to rounding (`discreteOrdinates.test.ts`).
 *
 * **Stated approximations.**
 * - The deck reflects as a plane-parallel slab, lit at the upper medium's μ₀, the pseudo-spherical
 *   solve's one scattering geometry (A. Dahlback and K. Stamnes, Planet. Space Sci. 39 (1991)
 *   671–683): its own sphericity is left out.
 * - Its reflectance is delta-M's, with no truncated-single-scattering correction (Nakajima and
 *   Tanaka's TMS), which no table of m ≤ 1 could hold: the cloud top's single-scattering peaks
 *   (a glory, a backscatter lobe) are smoothed into the truncated phase function.
 * - In the polarisation correction the deck depolarises: it reflects I into I alone, and Q, U and
 *   V not at all (decision-r08-vector §2, "Lambertian ground and decks depolarise").
 *
 * **What is stored** ({@link DeckTables}), per band and light, in the render channels on storage
 * (`binConversion.ts`):
 * - `above`, the thick bake's J_ms table over the upper medium with the deck as its ground
 *   (`bake.ts`'s `MsSourceTable`, its altitudes from the split up);
 * - each slot's `reflectance`, ρ^m(μ_v, μ₀) at the grid's upward view cosines and μ₀, each bin's the
 *   weighted mean over its gases' terms;
 * - and its `diffuse`, the deck's upward radiance at its top less its reflection of the kernels' own
 *   beam, I^m(τ_L, +μ_v) − (μ₀ T_k ÷ π) ρ^m(μ_v, μ₀), T_k the kernels' beam at the deck (extinction
 *   less the forward delta), per unit of the star's irradiance normal to its beam at the top: the
 *   deck's reflection of the sky and of the aureole, which the kernels add beside their own beam's
 *   reflection, as they add their own single scattering beside E.
 *
 * Pure: no state, no clock, no platform API.
 */

import { BAKE_WAVELENGTHS_NM, CHANNEL_WAVELENGTHS_NM, columnLengthM } from "../medium";
import {
  type BakeGround,
  type BakeLayering,
  bakeLayering,
  bakeMultipleScattering,
  type BinSolve,
  type BinTerm,
  checkBakeInputs,
  deltaMLayer,
  type MsSourceTable,
  type PreparedBake,
  prepareBake,
  THICK_BAKE_GRID,
  THICK_BAKE_MODES,
  THICK_BAKE_REFERENCE_BIN,
  THICK_BAKE_STREAMS,
  type ThickBakeBand,
  thickBakeBands,
  type ThickBakeBody,
  type ThickBakeGrid,
  type ThickBakeLight,
} from "./bake";
import {
  type GroundCosines,
  type GroundReflectance,
  type OrdinateLayer,
  solveDiscreteOrdinates,
} from "./discreteOrdinates";
import { EXP_SUM_OPTIONS, type ExpSumOptions } from "./expSum";
import {
  bakePolarisationCorrection,
  type PolarisationGround,
  type PolarisationOptions,
  type PolarisationTable,
  preparePolarisation,
  type PreparedPolarisation,
} from "./polarisation";
import { ENVELOPE_FLOOR_CHANNEL } from "./regime";
import { type SpectralColumn, spectralOptics } from "./spectralColumn";

/**
 * How deep an envelope's deck is read, the vertical τ from the top at the green channel: 1,000, the
 * depth of R08.T14.a's discrete-ordinate check (f), so that the deep decks' 10⁵–10⁶ never enter a
 * solve (decision-deep-decks-split §3.3 item 4; R08.T16.a).
 */
export const DECK_FLOOR_OPTICAL_DEPTH = 1_000;

/**
 * The deck's layers below the split, each evenly in ξ between the floor and the split as the thick
 * bake's are above it (lean, 2026-10-29): half by optical depth, so that a τ 30 cloud takes layers
 * of τ about 2 and an envelope's τ 990 some of τ 60, and half by height for the air beneath.
 */
export const DECK_LAYERS = 32;

/**
 * Where the deck's layers end.
 *
 * - `ground`: at the medium's ground, its albedo beneath: a deck over a surface (Venus-class).
 * - `envelope`: at τ {@link DECK_FLOOR_OPTICAL_DEPTH} from the top, or the column's base if that is
 *   shallower, over a black floor: a gas envelope, which has no ground.
 */
export type DeckFloor = "ground" | "envelope";

/** A cloud-deck split: where it lies and how deep its deck is read. */
export interface DeckSplit {
  /**
   * The split's altitude, h\* in m above the medium's ground: the deck's top or the envelope's floor
   * (`regime.ts`'s `deckAltitudeM`), above the deck's floor and below the column's top.
   */
  readonly altitudeM: number;
  readonly floor: DeckFloor;
}

/** A deck's preparation options. */
export interface DeckOptions {
  /** The upper medium's grid; `bake.ts`'s `THICK_BAKE_GRID` by default. */
  readonly grid?: ThickBakeGrid;
  /** The exponential sums' options; `expSum.ts`'s by default. */
  readonly expSum?: ExpSumOptions;
  /** The deck's layers, a whole number of the grid's altitude strides; {@link DECK_LAYERS} by default. */
  readonly deckLayers?: number;
}

/** Everything of a deck's bake that does not depend on the band. */
export interface PreparedDeck {
  readonly split: DeckSplit;
  /** The deck's floor, h\* in m: the ground's 0, or an envelope's τ {@link DECK_FLOOR_OPTICAL_DEPTH}. */
  readonly floorAltitudeM: number;
  /**
   * The upper medium's thick bake: its table's layers between the split and the top, its lights'
   * terms laid over {@link PreparedDeck.whole}.
   */
  readonly upper: PreparedBake;
  /** The upper medium's layers (`upper`'s own, as a layering). */
  readonly upperLayering: BakeLayering;
  /** The deck's layers, from the split down to the floor. */
  readonly deck: BakeLayering;
  /** The whole column's: the upper medium's layers from the top down, then the deck's. */
  readonly whole: BakeLayering;
  /** Per bin, the floor's albedo: the ground's (`upper.groundAlbedo`), or 0 under an envelope. */
  readonly floorAlbedo: Float64Array;
}

/**
 * An envelope's deck floor: the highest height, m above the medium's ground, at which the vertical
 * optical depth from the top, of every term but the absorbers at its whole extinction in the green
 * channel (`regime.ts`'s `envelopeFloorM`'s reading), reaches {@link DECK_FLOOR_OPTICAL_DEPTH}; the
 * ground, 0, where the whole column is shallower.
 */
function envelopeDeckFloorM(column: SpectralColumn): number {
  const green = CHANNEL_WAVELENGTHS_NM[ENVELOPE_FLOOR_CHANNEL];
  const [optics] = spectralOptics(column, [green]);
  const top = column.topHeightM;
  const read = column.terms.flatMap((term, t) => {
    const o = optics?.[t];
    return term.source.kind === "absorber" || o === undefined
      ? []
      : [{ term, extinction: o.scatteringPerM + o.absorptionPerM }];
  });
  const toTop = read.map(({ term }) => columnLengthM(term.density, top));
  const depthAbove = (h: number): number =>
    read.reduce(
      (sum, { term, extinction }, i) =>
        sum + extinction * ((toTop[i] ?? Number.NaN) - columnLengthM(term.density, h)),
      0,
    );
  if (depthAbove(0) < DECK_FLOOR_OPTICAL_DEPTH) {
    return 0;
  }
  let low = 0;
  let high = top;
  for (let step = 0; step < 64; step += 1) {
    const middle = 0.5 * (low + high);
    if (depthAbove(middle) >= DECK_FLOOR_OPTICAL_DEPTH) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return low;
}

/** Two layerings, one above the other, as one: `above`'s layers from the top down, then `below`'s. */
function stackedLayering(above: BakeLayering, below: BakeLayering): BakeLayering {
  return {
    interfaceHeightsM: Float64Array.from([
      ...above.interfaceHeightsM,
      ...below.interfaceHeightsM.subarray(1),
    ]),
    layerColumnsM: above.layerColumnsM.map((columns, t) =>
      Float64Array.from([...columns, ...(below.layerColumnsM[t] ?? [])]),
    ),
    altitudesM: Float64Array.from([...below.altitudesM, ...above.altitudesM.subarray(1)]),
  };
}

/**
 * Everything of a deck's bake but the band (the module's remarks): the floor, the upper medium's and
 * the deck's layers, and the upper medium's thick-bake preparation (the column's optics in the 15
 * bins, its table's point optics, and each light's exponential sums fitted over the whole column).
 *
 * @throws RangeError as `bake.ts`'s `prepareBake` refuses the grid or lights, for deck layers that
 *   are not a whole number of the grid's strides, for a split that does not lie above the floor and
 *   below the column's top, or as the column's optics refuse.
 */
export function prepareDeck(
  column: SpectralColumn,
  lights: ReadonlyArray<ThickBakeLight>,
  split: DeckSplit,
  options: DeckOptions = {},
): PreparedDeck {
  const grid = options.grid ?? THICK_BAKE_GRID;
  checkBakeInputs(column, lights, grid);
  const deckLayers = options.deckLayers ?? DECK_LAYERS;
  if (!(
    Number.isInteger(deckLayers) &&
    deckLayers >= 1 &&
    deckLayers % grid.altitudeStride === 0
  )) {
    throw new RangeError(
      `a deck takes whole strides of ${grid.altitudeStride} layers, not ${deckLayers}`,
    );
  }
  let floorAltitudeM: number;
  switch (split.floor) {
    case "ground":
      floorAltitudeM = 0;
      break;
    case "envelope":
      floorAltitudeM = envelopeDeckFloorM(column);
      break;
  }
  if (!(split.altitudeM > floorAltitudeM && split.altitudeM < column.topHeightM)) {
    throw new RangeError(
      `a split lies above its deck's floor at ${floorAltitudeM} m and below the top at ` +
        `${column.topHeightM} m, not at ${split.altitudeM} m`,
    );
  }
  const [reference] = spectralOptics(column, [
    BAKE_WAVELENGTHS_NM[THICK_BAKE_REFERENCE_BIN] ?? Number.NaN,
  ]);
  const upperLayering = bakeLayering(column, reference ?? [], grid, {
    bottomM: split.altitudeM,
    topM: column.topHeightM,
  });
  const deck = bakeLayering(
    column,
    reference ?? [],
    { ...grid, layers: deckLayers },
    { bottomM: floorAltitudeM, topM: split.altitudeM },
  );
  const whole = stackedLayering(upperLayering, deck);
  const upper = prepareBake(column, lights, grid, options.expSum ?? EXP_SUM_OPTIONS, {
    layering: upperLayering,
    fitLayering: whole,
  });
  return {
    split,
    floorAltitudeM,
    upper,
    upperLayering,
    deck,
    whole,
    floorAlbedo:
      split.floor === "ground"
        ? upper.groundAlbedo.slice()
        : new Float64Array(BAKE_WAVELENGTHS_NM.length),
  };
}

// ---------------------------------------------------------------------------------------------
// The deck's layers and reflectance.

/** A term's optics at relative density 1 at one wavelength, as the bake and the correction hold them. */
interface LayerTermOptics {
  readonly scatteringPerM: number;
  readonly absorptionPerM: number;
  readonly forwardDelta: number;
  /** χ₀ … χ_NSTR at least. */
  readonly moments: ArrayLike<number>;
}

/** A run of layers as a solve takes them, and their extinction before the gases. */
interface DeckOrdinateLayers {
  /** The delta-M layers, from the top down. */
  readonly layers: ReadonlyArray<OrdinateLayer>;
  /** Each layer's vertical extinction optical depth without the gases, in the band's frame. */
  readonly extinction: Float64Array;
  /** Each layer's depth as the kernels march it: the gases' and the extinction less the forward delta. */
  readonly kernelDepths: Float64Array;
}

/**
 * The delta-M layers of a run of a column's layers in one band (`bake.ts`'s arithmetic, term by term
 * in the column's order): each term's column times the band's factor, mixed by its scattering, the
 * gases' depth of one term of the bin's sums added, then `deltaMLayer` at `streams`.
 *
 * @param layerColumnsM - Per term, per layer from the top, ∫ρ dh over it, m.
 * @param absorptionDepth - One term of a bin's gases, per layer of the whole column, in the reference
 *   frame; the run's layers are its entries from `offset` on.
 */
function deckOrdinateLayers(
  terms: ReadonlyArray<LayerTermOptics>,
  layerColumnsM: ReadonlyArray<Float64Array>,
  layerCount: number,
  factor: number,
  absorptionDepth: Float64Array,
  offset: number,
  streams: number,
): DeckOrdinateLayers {
  const layers: OrdinateLayer[] = [];
  const extinction = new Float64Array(layerCount);
  const kernelDepths = new Float64Array(layerCount);
  for (let p = 0; p < layerCount; p += 1) {
    let tau = 0;
    let tauS = 0;
    let delta = 0;
    const moments = new Float64Array(streams + 1);
    for (const [t, o] of terms.entries()) {
      const c = (layerColumnsM[t]?.[p] ?? Number.NaN) * factor;
      tau += (o.scatteringPerM + o.absorptionPerM) * c;
      const s = o.scatteringPerM * c;
      if (s > 0) {
        tauS += s;
        delta += s * o.forwardDelta;
        for (let l = 0; l <= streams; l += 1) {
          moments[l] = (moments[l] ?? 0) + s * (o.moments[l] ?? Number.NaN);
        }
      }
    }
    const forwardDelta = tauS > 0 ? delta / tauS : 0;
    for (let l = 0; l <= streams; l += 1) {
      moments[l] = tauS > 0 ? (moments[l] ?? 0) / tauS : l === 0 ? 1 : 0;
    }
    const total = tau + factor * (absorptionDepth[offset + p] ?? Number.NaN);
    layers.push(deltaMLayer(total, tauS, moments, forwardDelta, streams).layer);
    extinction[p] = tau;
    kernelDepths[p] = Math.max(total - tauS * forwardDelta, 0);
  }
  return { layers, extinction, kernelDepths };
}

/**
 * A deck's bidirectional reflectance at `cosines` (the module's remarks): the solver over its
 * layers, a plane-parallel beam at each incident cosine μ′, the upward intensity at its top at each
 * outgoing cosine μ, ρ^m(μ, μ′) = π I^m(0, +μ) ÷ μ′ for m = 0 … `maxMode`.
 *
 * @param layers - The deck's layers from its top down.
 * @param floorAlbedo - The Lambertian albedo beneath them, in [0, 1].
 * @param streams - NSTR of the deck's solve; the thick bake's 16 by default, whatever the streams of
 *   the solve that reads it.
 * @throws RangeError as `discreteOrdinates.ts`'s `solveDiscreteOrdinates` refuses the deck's solve.
 */
export function deckReflectance(
  layers: ReadonlyArray<OrdinateLayer>,
  floorAlbedo: number,
  cosines: GroundCosines,
  maxMode: number,
  streams: number = THICK_BAKE_STREAMS,
): GroundReflectance {
  const { outgoingCosines, incidentCosines } = cosines;
  const solution = solveDiscreteOrdinates({
    layers,
    groundAlbedo: floorAlbedo,
    beamCosines: incidentCosines,
    userCosines: outgoingCosines,
    maxMode,
    streams,
  });
  const columns = incidentCosines.length;
  const modes = Array.from({ length: maxMode + 1 }, (_, m) => {
    const rho = new Float64Array(outgoingCosines.length * columns);
    solution.beams.forEach((beam, j) => {
      const up = beam.modes[m]?.userUp;
      const mu = incidentCosines[j] ?? Number.NaN;
      for (let i = 0; i < outgoingCosines.length; i += 1) {
        rho[i * columns + j] = (Math.PI * (up?.[i] ?? Number.NaN)) / mu;
      }
    });
    return rho;
  });
  return { outgoingCosines, incidentCosines, modes };
}

/**
 * The deck as the thick bake's ground in one band (`bake.ts`'s `BakeGround`): in bin `bin`, for one
 * term of its gases, the deck's layers in that bin and term and their {@link deckReflectance}.
 */
export function deckGround(prepared: PreparedDeck, band: ThickBakeBand): BakeGround {
  const { upper, deck, floorAlbedo } = prepared;
  const layerCount = deck.interfaceHeightsM.length - 1;
  return (bin, term, cosines, maxMode) =>
    deckReflectance(
      deckOrdinateLayers(
        upper.binOptics[bin] ?? [],
        deck.layerColumnsM,
        layerCount,
        band.coefficientFactor,
        term.absorptionDepth,
        upper.grid.layers,
        THICK_BAKE_STREAMS,
      ).layers,
      floorAlbedo[bin] ?? Number.NaN,
      cosines,
      maxMode,
    );
}

// ---------------------------------------------------------------------------------------------
// One band.

/** One light slot's deck table. */
export interface DeckSlot {
  /**
   * ρ^m(μ_v, μ₀) per channel, dimensionless, S × V × M × 3 row-major, entry
   * `((s V + v) M + m) 3 + c`: s the sun's cosine, v the upward view cosine (the reflected light's
   * direction of travel; an observer looking down at elevation −e reads μ_v = sin e), m the mode,
   * c red, green, blue. Each bin's is the mean over its gases' terms by their weights.
   */
  readonly reflectance: Float32Array;
  /**
   * The deck's upward radiance at its top less its reflection of the kernels' own beam,
   * I^m(τ_L, +μ_v) − (μ₀ T_k ÷ π) ρ^m(μ_v, μ₀) (the module's remarks), sr⁻¹ per unit of the star's
   * irradiance normal to its beam at the top, in the layout of `reflectance`.
   */
  readonly diffuse: Float32Array;
  /** The deck's solves, one a solve of the table above it. */
  readonly solves: number;
}

/**
 * One latitude band of a cloud-deck split (Provides' `DeckTables`): the table above the deck and
 * the deck's reflectance and radiance, one slot a light.
 *
 * @remarks
 * A band of the default grids holds `above`'s 405,504 B a light as `f32` (`bake.ts`'s
 * `MsSourceTable`) and the deck's 2 × 32 × 8 × 2 × 3 values, 12,288 B a light.
 */
export interface DeckTables {
  readonly band: ThickBakeBand;
  /** The split's altitude, h\* in m: the upper medium's ground, the same in every band. */
  readonly deckAltitudeM: number;
  /** The deck's floor, h\* in m. */
  readonly floorAltitudeM: number;
  /** J_ms over the upper medium, the deck its ground (`bake.ts`'s table, altitudes from the split). */
  readonly above: MsSourceTable;
  /** μ₀, ascending in (0, 1]: the grid's. */
  readonly sunCosines: Float64Array;
  /** μ_v, ascending in (0, 1]: the grid's upward cosines. */
  readonly viewCosines: Float64Array;
  /** M, the modes, m = 0 … M − 1: `bake.ts`'s `THICK_BAKE_MODES`. */
  readonly modes: number;
  /** The deck's vertical extinction optical depth in each bin in the band's frame, without the gases. */
  readonly deckOpticalDepth: Float64Array;
  /** One a light of the preparation, in its order. */
  readonly slots: ReadonlyArray<DeckSlot>;
}

/** A solve's deck record: ρ^m and the diffuse radiance, S × V × M, in its bin. */
interface DeckRecord {
  readonly reflectance: Float64Array;
  readonly diffuse: Float64Array;
}

/** A band of a body, or a refusal. */
function bandOf(body: ThickBakeBody, band: number): ThickBakeBand {
  const bands = thickBakeBands(body);
  const geometry = bands[band];
  if (geometry === undefined || !Number.isInteger(band)) {
    throw new RangeError(`the body has ${bands.length} latitude bands, not a band ${band}`);
  }
  return geometry;
}

/**
 * A cloud-deck split's bake of one latitude band (Provides' `bakeDeck`, in T14.b's shape; the
 * module's remarks): the thick bake of the upper medium with the deck as its ground, the deck's
 * reflectance solved once a bin and term of its gases, and the deck's tables from those same solves,
 * summed by the terms' weights and converted to the channels.
 *
 * @param band - The band's index in `bake.ts`'s `thickBakeBands` order.
 * @throws RangeError for a band the body does not have, or as the solver refuses a layer.
 */
export function bakeDeck(prepared: PreparedDeck, body: ThickBakeBody, band: number): DeckTables {
  const geometry = bandOf(body, band);
  const { upper } = prepared;
  const { grid } = upper;
  const layers = grid.layers;
  const suns = grid.sunCosines.length;
  const views = grid.upwardCosines.length;
  const modes = THICK_BAKE_MODES;
  const n = THICK_BAKE_STREAMS / 2;
  const ground = deckGround(prepared, geometry);
  const records = new Map<number, Map<BinTerm, DeckRecord>>();
  let reflectance: GroundReflectance | undefined;
  const recordOf = (solve: BinSolve, rho: GroundReflectance): DeckRecord => {
    const size = suns * views * modes;
    const out = { reflectance: new Float64Array(size), diffuse: new Float64Array(size) };
    const columns = rho.incidentCosines.length;
    for (let s = 0; s < suns; s += 1) {
      const beam = solve.solution.beams[s];
      const mu0 = grid.sunCosines[s] ?? Number.NaN;
      const kernel = solve.kernelBeams[s]?.interfaceTransmission[layers] ?? Number.NaN;
      for (let v = 0; v < views; v += 1) {
        for (let m = 0; m < modes; m += 1) {
          // The cosines are `groundCosinesOf`'s: the nodes first, then the views and the suns.
          const r = rho.modes[m]?.[(n + v) * columns + n + s] ?? Number.NaN;
          const leaving = beam?.modes[m]?.userUp[layers * views + v] ?? Number.NaN;
          const i = (s * views + v) * modes + m;
          out.reflectance[i] = r;
          out.diffuse[i] = leaving - ((mu0 * kernel) / Math.PI) * r;
        }
      }
    }
    return out;
  };
  const above = bakeMultipleScattering(upper, body, band, {
    ground: (bin, term, cosines, maxMode) => {
      reflectance = ground(bin, term, cosines, maxMode);
      return reflectance;
    },
    onSolve: (bin, term, solve) => {
      if (reflectance === undefined) {
        throw new Error(`bin ${String(bin)}'s solve took no deck`);
      }
      let byTerm = records.get(bin);
      if (byTerm === undefined) {
        byTerm = new Map();
        records.set(bin, byTerm);
      }
      byTerm.set(term, recordOf(solve, reflectance));
      reflectance = undefined;
    },
  });
  const deckLayerCount = prepared.deck.interfaceHeightsM.length - 1;
  const none = new Float64Array(prepared.whole.interfaceHeightsM.length - 1);
  const deckOpticalDepth = Float64Array.from(upper.binOptics, (optics) =>
    deckOrdinateLayers(
      optics,
      prepared.deck.layerColumnsM,
      deckLayerCount,
      geometry.coefficientFactor,
      none,
      layers,
      THICK_BAKE_STREAMS,
    ).extinction.reduce((sum, tau) => sum + tau, 0),
  );
  const size = suns * views * modes;
  const slots = upper.lights.map((light, slot): DeckSlot => {
    const perBin = light.bins.map((bin, b) => {
      const sum = { reflectance: new Float64Array(size), diffuse: new Float64Array(size) };
      for (const term of bin.terms) {
        const record = records.get(b)?.get(term);
        if (record === undefined) {
          throw new Error(`light ${slot}'s bin ${b} has a term with no deck`);
        }
        for (let i = 0; i < size; i += 1) {
          sum.reflectance[i] =
            (sum.reflectance[i] ?? 0) + term.weight * (record.reflectance[i] ?? Number.NaN);
          sum.diffuse[i] = (sum.diffuse[i] ?? 0) + term.weight * (record.diffuse[i] ?? Number.NaN);
        }
      }
      return sum;
    });
    const converted = (pick: (bin: DeckRecord) => Float64Array): Float32Array => {
      const values = new Float32Array(size * 3);
      light.conversion.factors.forEach((factors, c) => {
        for (let i = 0; i < size; i += 1) {
          let x = 0;
          for (const [b, bin] of perBin.entries()) {
            x += (factors[b] ?? Number.NaN) * (pick(bin)[i] ?? Number.NaN);
          }
          values[i * 3 + c] = x;
        }
      });
      return values;
    };
    return {
      reflectance: converted((bin) => bin.reflectance),
      diffuse: converted((bin) => bin.diffuse),
      solves: above.slots[slot]?.solves ?? 0,
    };
  });
  return {
    band: geometry,
    deckAltitudeM: prepared.split.altitudeM,
    floorAltitudeM: prepared.floorAltitudeM,
    above,
    sunCosines: grid.sunCosines.slice(),
    viewCosines: grid.upwardCosines.slice(),
    modes,
    deckOpticalDepth,
    slots,
  };
}

// ---------------------------------------------------------------------------------------------
// The polarisation correction over the deck.

/**
 * R08.T14.e's correction prepared over a deck's upper medium: `polarisation.ts`'s
 * `preparePolarisation` on the deck's grid and layers (the upper medium's table and the whole
 * column's fit), so that its table's altitudes are `above`'s to the bit.
 *
 * @throws RangeError as `preparePolarisation` refuses.
 */
export function prepareDeckPolarisation(
  prepared: PreparedDeck,
  lights: ReadonlyArray<ThickBakeLight>,
  options: Omit<PolarisationOptions, "grid" | "layout"> = {},
): PreparedPolarisation {
  return preparePolarisation(prepared.upper.column, lights, {
    ...options,
    grid: prepared.upper.grid,
    layout: { layering: prepared.upperLayering, fitLayering: prepared.whole },
  });
}

/**
 * The deck as the correction's depolarising ground in one band (`polarisation.ts`'s
 * `PolarisationGround`): at a wavelength of the correction, for one term of its bin's gases, the
 * deck's layers by the correction's own optics there (each term's a₁, the scalar phase) and their
 * {@link deckReflectance} at 16 streams, the floor the point's albedo or black.
 */
export function deckPolarisationGround(
  prepared: PreparedDeck,
  band: ThickBakeBand,
): PolarisationGround {
  const { deck, upper } = prepared;
  const layerCount = deck.interfaceHeightsM.length - 1;
  return (point, term, cosines, maxMode) => {
    const terms = point.terms.map((o): LayerTermOptics => ({
      scatteringPerM: o.scatteringPerM,
      absorptionPerM: o.absorptionPerM,
      forwardDelta: o.forwardDelta,
      moments: o.moments.a1,
    }));
    let floorAlbedo: number;
    switch (prepared.split.floor) {
      case "ground":
        floorAlbedo = point.groundAlbedo;
        break;
      case "envelope":
        floorAlbedo = 0;
        break;
    }
    return deckReflectance(
      deckOrdinateLayers(
        terms,
        deck.layerColumnsM,
        layerCount,
        band.coefficientFactor,
        term.absorptionDepth,
        upper.grid.layers,
        THICK_BAKE_STREAMS,
      ).layers,
      floorAlbedo,
      cosines,
      maxMode,
    );
  };
}

/**
 * The polarisation correction of one latitude band over a deck's upper medium, the deck a
 * depolarising ground (`polarisation.ts`'s `bakePolarisationCorrection` with
 * {@link deckPolarisationGround}; decision-r08-vector §2).
 *
 * @param polarisation - {@link prepareDeckPolarisation} of the same deck, at the render channels.
 * @throws RangeError as `bakePolarisationCorrection` refuses.
 */
export function bakeDeckPolarisation(
  prepared: PreparedDeck,
  polarisation: PreparedPolarisation,
  body: ThickBakeBody,
  band: number,
): PolarisationTable {
  return bakePolarisationCorrection(
    polarisation,
    body,
    band,
    deckPolarisationGround(prepared, bandOf(body, band)),
  );
}
