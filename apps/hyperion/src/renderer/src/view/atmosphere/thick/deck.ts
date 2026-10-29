/**
 * The cloud-deck split's bakes (plan R08, R08.T15.a.1 and T15.b.1; Design notes 9 and 17): the deck
 * as the upper medium's reflecting boundary, its bidirectional reflectance from the
 * discrete-ordinates solver over the deck's own layers, the multiple-scattering table J_ms of the
 * medium above it (`bake.ts`), R08.T14.e's polarisation correction over the same medium with the
 * deck as a depolarising ground (`polarisation.ts`), and the downwelling table below a deck over a
 * surface, one latitude band at a time, spectral in, channels out.
 *
 * @remarks
 * **The split** (Design note 9, "The cloud-deck split"). Above the split's altitude (a deck's top,
 * or a gas envelope's floor; `regime.ts`'s `AtmosphereRegime`) the tables run with the deck as a
 * baked reflecting boundary, and a thinner layer above it stays a term of the upper medium. The
 * deck's layers are the medium below the split: down to the ground, its albedo beneath, for a deck
 * over a surface (Venus-class); for an envelope, no deeper than τ(550) {@link
 * DECK_FLOOR_OPTICAL_DEPTH} from the top, over a black floor (decision-deep-decks-split §3.3 item
 * 4, R08.T16.a's "reads the medium below it no deeper than τ(550) 1,000"). The τ is read as the
 * split's is (`regime.ts`'s `envelopeFloorM`): in the green channel at 544 nm, at most 4.6% over
 * 550 nm's in Rayleigh's τ, and in the reference frame, s = 1, so that the floor lies at one height
 * in every band and a band of gravity ratio s reads the deck to τ 1,000 ÷ s.
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
 * - Its reflectance is held to the table's modes, m = 0 and 1, though a cloud top's single
 *   scattering carries strong higher ones: on a deck of Cornette–Shanks g 0.75, m ≤ 1 misses ρ by
 *   up to 87% where μ and μ₀ are at least 0.25, m ≤ 2 by 47% and m ≤ 7 by 1.9% (measured
 *   2026-10-29, plan R08's records); the `giant-deck` gate (R08.T15.a.2) judges it.
 * - An envelope's floor is black: a conservative deck of g 0.8 read to τ 1,000 ÷ s loses 0.9–1.3%
 *   of its incident flux through it (measured, the four bands of a Saturn-class figure).
 *
 * **What is stored** ({@link DeckTables}), per band and light, in the render channels on storage
 * (`binConversion.ts`):
 * - `above`, the thick bake's J_ms table over the upper medium with the deck as its ground
 *   (`bake.ts`'s `MsSourceTable`, its altitudes from the split up);
 * - each slot's `reflectance`, ρ^m(μ_v, μ₀) at the grid's upward view cosines and μ₀, each bin's the
 *   weighted mean over its gases' terms;
 * - its `diffuse`, the deck's upward radiance at its top less its reflection of the kernels' own
 *   beam, I^m(τ_L, +μ_v) − (μ₀ T_k ÷ π) ρ^m(μ_v, μ₀), T_k the kernels' beam at the deck (extinction
 *   less the forward delta), per unit of the star's irradiance normal to its beam at the top: the
 *   deck's reflection of the sky and of the aureole, which the kernels add beside their own beam's
 *   reflection, as they add their own single scattering beside E;
 * - and its `beamReflection`, T_k ρ^m term by term, so that the beam's reflection and `diffuse`
 *   add up to the deck's radiance exactly where a bin's gases take several terms.
 *
 * **Below the deck** (R08.T15.b.1; {@link BelowDeckTable}). Under a deck over a surface the sky and
 * the aerial perspective read a plane-parallel table of the diffuse radiance by altitude, travel
 * cosine and μ₀, m = 0 and 1, with the downward flux and the direct beam's transmission. It is the
 * whole column's own solve ({@link deckColumnSolve}: the upper medium's layers and the deck's, one
 * stack, the pseudo-spherical beam through all of them and the ground's albedo beneath), read at the
 * deck's altitudes: the field the deck's reflectance summarises from above, solved once a bin and
 * term beside the bake's two. An envelope has no view from below, and no table.
 *
 * Pure: no state, no clock, no platform API.
 */

import { BAKE_WAVELENGTHS_NM, CHANNEL_WAVELENGTHS_NM, columnLengthM } from "../medium";
import {
  type BakeGround,
  type BakeLayering,
  bakeLayering,
  bakeMultipleScattering,
  binGroundAlbedo,
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
import type { BinConversion } from "./binConversion";
import {
  type DiscreteOrdinatesSolution,
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
import { type SphericalBeam, sphericalBeamPath } from "./sphericalBeam";
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
  let floorAlbedo: Float64Array;
  switch (split.floor) {
    case "ground":
      floorAltitudeM = 0;
      // The bake's own ground, bin by bin (`prepareBake`'s `groundAlbedo`).
      floorAlbedo = Float64Array.from(BAKE_WAVELENGTHS_NM, (nm) =>
        binGroundAlbedo(column.groundAlbedo, nm),
      );
      break;
    case "envelope":
      floorAltitudeM = envelopeDeckFloorM(column);
      floorAlbedo = new Float64Array(BAKE_WAVELENGTHS_NM.length);
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
    floorAlbedo,
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
  /**
   * Σ_t w_t T_k,t ρ^m_t, the deck's reflectance weighted term by term by the kernels' beam at its
   * top (extinction less the forward delta, pseudo-spherical), in the layout of `reflectance`:
   * (μ₀ ÷ π) times it, with `diffuse`, is the deck's upward radiance at its top exactly. The
   * kernels' own T_k times `reflectance` misses the covariance of a bin's terms' beams and
   * reflectances, which a strong gas term makes large (the science check, 2026-10-29).
   */
  readonly beamReflection: Float32Array;
  /** The deck's solves, one a solve of the table above it. */
  readonly solves: number;
}

/**
 * One latitude band of a cloud-deck split (Provides' `DeckTables`): the table above the deck and
 * the deck's reflectance and radiance, one slot a light.
 *
 * @remarks
 * A band of the default grids holds `above`'s 405,504 B a light as `f32` (`bake.ts`'s
 * `MsSourceTable`) and the deck's 3 × 32 × 8 × 2 × 3 values, 18,432 B a light.
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
  /**
   * The downwelling table below a deck over a surface (R08.T15.b.1), or `null` for an envelope,
   * which has no view from below.
   */
  readonly below: BelowDeckTable | null;
}

/** A solve's deck record: ρ^m and the diffuse radiance, S × V × M, in its bin. */
interface DeckRecord {
  readonly reflectance: Float64Array;
  readonly diffuse: Float64Array;
  /** T_k ρ^m, the reflectance weighted by the kernels' beam at the deck. */
  readonly beamReflection: Float64Array;
}

/** The deck record's tables, in their order. */
const DECK_RECORD_KEYS = ["reflectance", "diffuse", "beamReflection"] as const;

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
  const recordOf = (solve: BinSolve, rho: GroundReflectance): DeckRecord => {
    const size = suns * views * modes;
    const out = {
      reflectance: new Float64Array(size),
      diffuse: new Float64Array(size),
      beamReflection: new Float64Array(size),
    };
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
          out.beamReflection[i] = kernel * r;
        }
      }
    }
    return out;
  };
  const above = bakeMultipleScattering(upper, body, band, {
    ground,
    onSolve: (bin, term, solve) => {
      const { groundReflectance } = solve;
      if (groundReflectance === undefined) {
        throw new Error(`bin ${String(bin)}'s solve took no deck`);
      }
      let byTerm = records.get(bin);
      if (byTerm === undefined) {
        byTerm = new Map();
        records.set(bin, byTerm);
      }
      byTerm.set(term, recordOf(solve, groundReflectance));
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
    const perBin = light.bins.map((bin, b): DeckRecord => {
      const sum = {
        reflectance: new Float64Array(size),
        diffuse: new Float64Array(size),
        beamReflection: new Float64Array(size),
      };
      for (const term of bin.terms) {
        const record = records.get(b)?.get(term);
        if (record === undefined) {
          throw new Error(`light ${slot}'s bin ${b} has a term with no deck`);
        }
        for (const key of DECK_RECORD_KEYS) {
          const target = sum[key];
          const source = record[key];
          for (let i = 0; i < size; i += 1) {
            target[i] = (target[i] ?? 0) + term.weight * (source[i] ?? Number.NaN);
          }
        }
      }
      return sum;
    });
    const { factors } = light.conversion;
    const converted = (key: (typeof DECK_RECORD_KEYS)[number]): Float32Array =>
      inChannels(
        factors,
        perBin.map((bin) => bin[key]),
        size,
      );
    return {
      reflectance: converted("reflectance"),
      diffuse: converted("diffuse"),
      beamReflection: converted("beamReflection"),
      solves: above.slots[slot]?.solves ?? 0,
    };
  });
  let below: BelowDeckTable | null;
  switch (prepared.split.floor) {
    case "ground":
      below = belowDeck(prepared, geometry);
      break;
    case "envelope":
      below = null;
      break;
  }
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
    below,
  };
}

/**
 * Bins' values converted to the channels (`binConversion.ts`'s rule, X_c = Σ_b f_cb X_b), point by
 * point: `size` points a bin, three channels a point.
 */
function inChannels(
  factors: BinConversion["factors"],
  perBin: ReadonlyArray<Float64Array>,
  size: number,
): Float32Array {
  const values = new Float32Array(size * 3);
  factors.forEach((channel, c) => {
    for (let i = 0; i < size; i += 1) {
      let x = 0;
      for (const [b, bin] of perBin.entries()) {
        x += (channel[b] ?? Number.NaN) * (bin[i] ?? Number.NaN);
      }
      values[i * 3 + c] = x;
    }
  });
  return values;
}

// ---------------------------------------------------------------------------------------------
// Below the deck (R08.T15.b.1).

/** The whole column's solve in one bin and term of a band: the upper medium's layers and the deck's. */
export interface DeckColumnSolve {
  readonly solution: DiscreteOrdinatesSolution;
  /** The delta-M beams over the whole column, one a μ₀. */
  readonly scaledBeams: ReadonlyArray<SphericalBeam>;
  /** The kernels' beams over it, extinction less the forward delta, one a μ₀. */
  readonly kernelBeams: ReadonlyArray<SphericalBeam>;
}

/**
 * The whole column's solve in bin `bin` for one term of its gases (the module's remarks, "Below the
 * deck"): the upper medium's delta-M layers and the deck's, each by `bake.ts`'s arithmetic, the
 * pseudo-spherical beams over all of them, and the ground's albedo beneath, at the grid's μ₀ and
 * view cosines, m = 0 and 1.
 *
 * @throws RangeError as the solver refuses a layer.
 */
export function deckColumnSolve(
  prepared: PreparedDeck,
  band: ThickBakeBand,
  bin: number,
  term: BinTerm,
): DeckColumnSolve {
  const { upper, deck, whole, floorAlbedo } = prepared;
  const { grid } = upper;
  const optics = upper.binOptics[bin] ?? [];
  const factor = band.coefficientFactor;
  const above = deckOrdinateLayers(
    optics,
    upper.layerColumnsM,
    grid.layers,
    factor,
    term.absorptionDepth,
    0,
    THICK_BAKE_STREAMS,
  );
  const beneath = deckOrdinateLayers(
    optics,
    deck.layerColumnsM,
    deck.interfaceHeightsM.length - 1,
    factor,
    term.absorptionDepth,
    grid.layers,
    THICK_BAKE_STREAMS,
  );
  const layers = [...above.layers, ...beneath.layers];
  const scaledDepths = Float64Array.from(layers, (layer) => layer.opticalDepth);
  const kernelDepths = Float64Array.from([...above.kernelDepths, ...beneath.kernelDepths]);
  const radius = band.bottomRadiusM;
  const heights = whole.interfaceHeightsM;
  const scaledBeams = Array.from(grid.sunCosines, (mu0) =>
    sphericalBeamPath(radius, heights, scaledDepths, mu0),
  );
  const kernelBeams = Array.from(grid.sunCosines, (mu0) =>
    sphericalBeamPath(radius, heights, kernelDepths, mu0),
  );
  const solution = solveDiscreteOrdinates({
    layers,
    groundAlbedo: floorAlbedo[bin] ?? Number.NaN,
    beamCosines: grid.sunCosines,
    userCosines: grid.upwardCosines,
    maxMode: THICK_BAKE_MODES - 1,
    streams: THICK_BAKE_STREAMS,
    beamPaths: scaledBeams,
  });
  return { solution, scaledBeams, kernelBeams };
}

/** One light slot's table below the deck. */
export interface BelowDeckSlot {
  /**
   * The diffuse radiance I^m(h, μ, μ₀) per channel, sr⁻¹ per unit of the star's irradiance normal
   * to its beam at the top, A × S × V × M × 3 row-major, entry `(((a S + s) V + v) M + m) 3 + c`:
   * a the altitude, s the sun's cosine, v the travel cosine of {@link BelowDeckTable}, m the mode, c
   * red, green, blue. I = I⁰ + I¹ cos Δφ, Δφ the azimuth of the light's direction of travel less
   * the sun's beam's (`discreteOrdinates.ts`'s convention). It is delta-M's diffuse field: the
   * aureole inside the truncated lobe and the exact single scattering (Nakajima and Tanaka's TMS)
   * are not in it, a stated approximation for R08.T15.b.2's read to judge.
   */
  readonly radiance: Float32Array;
  /**
   * The downward flux the kernels' own beam leaves out, per unit of the star's irradiance normal to
   * its beam at the top, A × S × 3, entry `(a S + s) 3 + c`: the diffuse field's
   * 2π ∫₀¹ I⁰(h, −μ) μ dμ on the quadrature and delta-M's aureole μ₀ (T′ − T_k), the light it
   * counts as beam beyond the kernels' T_k. With μ₀ times `direct` it is the solve's whole downward
   * flux, μ₀ T′ and the diffuse field's.
   */
  readonly irradiance: Float32Array;
  /**
   * The direct beam's transmission from the top to each altitude as the kernels march it, its
   * extinction less the forward delta (pseudo-spherical, `sphericalBeam.ts`), A × S × 3.
   */
  readonly direct: Float32Array;
}

/**
 * The plane-parallel downwelling table below a deck over a surface (R08.T15.b.1; Design note 9, "a
 * baked plane-parallel table of downwelling radiance by altitude, view angle and sun angle"), one
 * a band: the radiance field of the whole column's solve at the deck's altitudes, from the ground
 * to the split, for sky and aerial perspective beneath, with the irradiance that
 * `atmosphere_sky_irradiance` reads there and the deck's direct transmittance (both read by
 * R08.T15.b.2).
 *
 * @remarks
 * Of the default grids, 17 altitudes × 32 μ₀ × 16 travel cosines × 2 modes × 3 channels, 208,896 B
 * a band and light as `f32`, and the irradiance and transmittance 6,528 B each.
 */
export interface BelowDeckTable {
  /** h\* above the ground, m, ascending from 0 to the split: every stride-th of the deck's interfaces. */
  readonly altitudesM: Float64Array;
  /** μ₀, ascending in (0, 1]: the grid's. */
  readonly sunCosines: Float64Array;
  /**
   * The cosines of the light's direction of travel, positive upward, ascending: the downward half,
   * then the upward, as `bake.ts`'s `MsSourceTable`. An observer looking at elevation e reads
   * −sin e.
   */
  readonly travelCosines: Float64Array;
  /** M, the modes, m = 0 … M − 1: `bake.ts`'s `THICK_BAKE_MODES`. */
  readonly modes: number;
  /** One a light of the preparation, in its order. */
  readonly slots: ReadonlyArray<BelowDeckSlot>;
}

/** A solve's record below the deck, in its bin. */
interface BelowRecord {
  /** A × S × V × M. */
  readonly radiance: Float64Array;
  /** A × S. */
  readonly irradiance: Float64Array;
  readonly direct: Float64Array;
}

/**
 * The table below the deck of one band (the module's remarks): the whole column solved once a bin
 * and term, its fields read at the deck's altitudes, summed by the terms' weights and converted.
 */
function belowDeck(prepared: PreparedDeck, band: ThickBakeBand): BelowDeckTable {
  const { upper, deck } = prepared;
  const { grid } = upper;
  const deckLayers = deck.interfaceHeightsM.length - 1;
  const ground = grid.layers + deckLayers;
  const altitudes = deck.altitudesM.length;
  const suns = grid.sunCosines.length;
  const views = grid.upwardCosines.length;
  const v2 = 2 * views;
  const modes = THICK_BAKE_MODES;
  const size = altitudes * suns * v2 * modes;
  const recordOf = (solve: DeckColumnSolve): BelowRecord => {
    const out = {
      radiance: new Float64Array(size),
      irradiance: new Float64Array(altitudes * suns),
      direct: new Float64Array(altitudes * suns),
    };
    for (let a = 0; a < altitudes; a += 1) {
      const k = ground - a * grid.altitudeStride;
      for (let s = 0; s < suns; s += 1) {
        const beam = solve.solution.beams[s];
        const kernel = solve.kernelBeams[s]?.interfaceTransmission[k] ?? Number.NaN;
        const scaled = solve.scaledBeams[s]?.interfaceTransmission[k] ?? Number.NaN;
        // The aureole, the light delta-M counts as beam that the kernels' own beam leaves out.
        const aureole = (grid.sunCosines[s] ?? Number.NaN) * Math.max(scaled - kernel, 0);
        out.irradiance[a * suns + s] = (beam?.fluxes.diffuseDown[k] ?? Number.NaN) + aureole;
        out.direct[a * suns + s] = kernel;
        const cell = (a * suns + s) * v2;
        for (let m = 0; m < modes; m += 1) {
          const mode = beam?.modes[m];
          for (let u = 0; u < views; u += 1) {
            out.radiance[(cell + views + u) * modes + m] =
              mode?.userUp[k * views + u] ?? Number.NaN;
            out.radiance[(cell + views - 1 - u) * modes + m] =
              mode?.userDown[k * views + u] ?? Number.NaN;
          }
        }
      }
    }
    return out;
  };
  const records = upper.binOptics.map(() => new Map<BinTerm, BelowRecord>());
  const slots = upper.lights.map((light, slot): BelowDeckSlot => {
    const sums = light.bins.map((bin, b) => {
      const sum = {
        radiance: new Float64Array(size),
        irradiance: new Float64Array(altitudes * suns),
        direct: new Float64Array(altitudes * suns),
      };
      const byTerm = records[b];
      if (byTerm === undefined) {
        throw new Error(`light ${slot} has no bin ${b}`);
      }
      for (const term of bin.terms) {
        let record = byTerm.get(term);
        if (record === undefined) {
          record = recordOf(deckColumnSolve(prepared, band, b, term));
          byTerm.set(term, record);
        }
        const w = term.weight;
        for (const key of ["radiance", "irradiance", "direct"] as const) {
          const target = sum[key];
          const source = record[key];
          for (let i = 0; i < target.length; i += 1) {
            target[i] = (target[i] ?? 0) + w * (source[i] ?? Number.NaN);
          }
        }
      }
      return sum;
    });
    const { factors } = light.conversion;
    return {
      radiance: inChannels(
        factors,
        sums.map((sum) => sum.radiance),
        size,
      ),
      irradiance: inChannels(
        factors,
        sums.map((sum) => sum.irradiance),
        altitudes * suns,
      ),
      direct: inChannels(
        factors,
        sums.map((sum) => sum.direct),
        altitudes * suns,
      ),
    };
  });
  const upward = grid.upwardCosines;
  return {
    altitudesM: deck.altitudesM.slice(),
    sunCosines: grid.sunCosines.slice(),
    travelCosines: Float64Array.from({ length: v2 }, (_, v) =>
      v < views ? -(upward[views - 1 - v] ?? Number.NaN) : (upward[v - views] ?? Number.NaN),
    ),
    modes,
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
 * @throws RangeError as `preparePolarisation` refuses, or for a moment order below the deck's 16
 *   streams, whose delta-M reads χ₀ … χ₁₆ of the correction's own optics.
 */
export function prepareDeckPolarisation(
  prepared: PreparedDeck,
  lights: ReadonlyArray<ThickBakeLight>,
  options: Omit<PolarisationOptions, "grid" | "layout"> = {},
): PreparedPolarisation {
  const order = options.momentOrder;
  if (order !== undefined && !(order >= THICK_BAKE_STREAMS)) {
    throw new RangeError(
      `the deck's ${THICK_BAKE_STREAMS}-stream reflectance reads moments to ${THICK_BAKE_STREAMS}, past ${order}`,
    );
  }
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
