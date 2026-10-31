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
 * boundary terms). As the upper medium's ground it is read at every cosine the upper medium's solve
 * asks, its quadrature nodes and user cosines going out, its nodes and its beams' μ₀ coming in, at
 * the bake's modes: taken so, it is the deck's discrete reflection itself, a slab's reflectance as
 * the ground giving the whole column's plane-parallel solve to rounding (`discreteOrdinates.test.ts`).
 *
 * **The deck's own tables** (R08.T15.a.2; main's ruling of 2026-10-29) hold its reflectance at
 * {@link DECK_MODES} modes, m = 0 … 7, with its single scattering out of the table and computed
 * exactly where it is read. The modes are solved again at the kernels' cosines alone (the grid's
 * upward cosines out, its sun cosines above the horizontal in), m = 0 … 7 in one solve beside the
 * coupling's; the upper medium's solve still reads only the bake's modes of the deck, since the
 * deck's light reaches J only through the modes J stores.
 * - _The multiple-scattering part_, ρ^m − ρ_ss^m, ρ_ss^m the single scattering of the same delta-M
 *   layers ({@link deckSingleScattering}): ((2 − δ_m0) ÷ 4) Σ_p ω′_p P′_p^m(μ, −μ₀) E_p ÷ (μ₀ + μ),
 *   P′^m(μ, μ′) = Σ_l (2l + 1) χ′_l Λ_l^m(μ) Λ_l^m(μ′) over the layer's truncated moments, and
 *   E_p = e^(−τ′_above x) (1 − e^(−τ′_p x)), x = 1 ÷ μ₀ + 1 ÷ μ, the plane-parallel slab's own.
 *   Taken out, a cloud top's single-scattering peaks no longer ask the table for its modes: on a
 *   Cornette–Shanks g 0.75 deck, m ≤ 7 of the rest misses ρ by 0.45% where μ and μ₀ are at least
 *   0.25 (main's research advice of 2026-10-29; pod, 4 vCPU), where the whole reflectance at m ≤ 7
 *   misses by 1.9%, and a Mie deck's by 15–17%.
 * - _The exact single scattering_ (T. Nakajima and M. Tanaka, JQSRT 40 (1988) 51–69, their TMS, the
 *   truncated single scattering replaced by the full phase function's on the delta-M layers, as the
 *   DISORT report restates it: K. Stamnes, S.-C. Tsay, W. Wiscombe and I. Laszlo, DISORT v1.1
 *   (2000), §3.6.2, eq. 68): ρ_TMS =
 *   π Σ_p (ω′_p ÷ (1 − f_p)) p_p(Θ) E_p ÷ (μ₀ + μ), p_p the layer's whole phase function per
 *   steradian, its terms' mixed by their scattering, so that ω′ ÷ (1 − f) per unit of τ′ is the
 *   true scattering per unit of τ. It is stored separable, G(μ, μ₀) p(Θ) ({@link deckSingleGeometry}):
 *   G = π Σ_p (ω′_p ÷ (1 − f_p)) E_p ÷ (μ₀ + μ), and p the layers' phases mixed by G's weights at
 *   the nadir pair, μ = μ₀ = 1, a stated approximation for a deck of several modes at different
 *   depths (the research advice measured ≤ 0.012% of ρ on three decks of one mode; R08.T15.a.2
 *   ≤ 0.024% after the bins' conversion, on the NH₃-like envelope, `giant-deck` and a deck of CS
 *   g 0.85 over g 0.4, 2026-10-30), on the u = √(θ ÷ π) grid of the kernels' phase tables
 *   ({@link DECK_PHASE_U}).
 *
 * **Stated approximations.**
 * - The deck reflects as a plane-parallel slab, lit at the upper medium's μ₀, the pseudo-spherical
 *   solve's one scattering geometry (A. Dahlback and K. Stamnes, Planet. Space Sci. 39 (1991)
 *   671–683): its own sphericity is left out.
 * - In the polarisation correction the deck depolarises: it reflects I into I alone, and Q, U and
 *   V not at all (decision-r08-vector §2, "Lambertian ground and decks depolarise").
 * - The multiple-scattering part is held to {@link DECK_MODES}: at grazing cosines (μ 0.06, μ₀
 *   0.016) no count up to 8 converges (research advice, 2026-10-29), where the upper medium's path
 *   dilutes the deck's share.
 * - The deck's sky-lit radiance (`diffuse`) is held to the bake's modes, which reflect a
 *   Rayleigh-like sky exactly and a forward-scattering haze above the deck to a few per cent
 *   (`diffuseModes`, a test's option, takes it to the deck's).
 * - The exact single scattering is separable, G(μ, μ₀) p(Θ), each bin's phase mixed by G's weights
 *   at the nadir pair, and the bins' into a channel's by their converted weights there.
 * - The deck's reflection of the aureole, in `diffuse`, keeps delta-M's truncated reflectance at
 *   the bake's modes, not the exact single scattering and {@link DECK_MODES}: light of about
 *   τ_s (f − f_c) ÷ μ₀ of the beam over the deck (the science check's estimate, under 1% for a
 *   thin haze above it, more at low suns; not measured).
 * - An envelope's floor is black: a conservative deck of g 0.8 read to τ 1,000 ÷ s loses 0.9–1.3%
 *   of its incident flux through it (measured, the four bands of a Saturn-class figure).
 *
 * **What is stored** ({@link DeckTables}), per band and light, in the render channels on storage
 * (`binConversion.ts`), at the grid's sun cosines and upward view cosines:
 * - `above`, the thick bake's J_ms table over the upper medium with the deck as its ground
 *   (`bake.ts`'s `MsSourceTable`, its altitudes from the split up, on every row of the grid, the 14
 *   below the horizontal among them);
 * - each slot's `reflectance`, ρ^m − ρ_ss^m at {@link DECK_MODES}, each bin's the weighted mean over
 *   its gases' terms;
 * - its `diffuse`, the deck's upward radiance at its top less its reflection of the kernels' own
 *   beam, I^m(τ_L, +μ_v) − (μ₀ T_k ÷ π) ρ^m(μ_v, μ₀), T_k the kernels' beam at the deck (extinction
 *   less the forward delta), per unit of the star's irradiance normal to its beam at the top: the
 *   deck's reflection of the sky and of the aureole, which the kernels add beside their own beam's
 *   reflection, as they add their own single scattering beside E; below the horizontal, where no
 *   beam reaches the deck, the sky's reflection alone;
 * - its `beamReflection`, T_k (ρ^m − ρ_ss^m) term by term, so that the beam's reflection and
 *   `diffuse` add up to the deck's radiance exactly where a bin's gases take several terms;
 * - and the exact single scattering's G, its beam's G_β = T_k G term by term, and its phase per
 *   channel, so that the deck's upward radiance per unit of the star's irradiance is
 *   (μ₀ ÷ π) (Σ_m βρ^m cos mΔφ + G_β p(Θ)) + Σ_m D^m cos mΔφ.
 * - Below the horizontal the beam's tables are 0, since no beam reaches the deck there.
 *
 * **Below the deck** (R08.T15.b.1; {@link BelowDeckTable}). Under a deck over a surface the sky and
 * the aerial perspective read a plane-parallel table of the diffuse radiance by altitude, travel
 * cosine and μ₀ above the horizontal, at the bake's modes, with the downward flux and the direct
 * beam's transmission. It is the
 * whole column's own solve ({@link deckColumnSolve}: the upper medium's layers and the deck's, one
 * stack, the pseudo-spherical beam through all of them and the ground's albedo beneath), read at the
 * deck's altitudes: the field the deck's reflectance summarises from above, solved once a bin and
 * term beside the bake's two. An envelope has no view from below, and no table.
 *
 * **Bin and channel shares** (R08.T14.d's bake time). Every table here is summed in the bake bins
 * and converted to the channels last, as the thick bake's is, so a preparation of some bins alone
 * ({@link DeckOptions.bins}) and its {@link bakeDeckBins} give those bins as the whole band's bake
 * does, and {@link gatherDeckBins} of shares holding every bin once is {@link bakeDeck}'s tables to
 * the bit. The correction over the deck splits by its channels ({@link deckPolarisationChannel}).
 *
 * Pure: no state, no clock, no platform API.
 */

import { FORWARD_PEAK_U } from "../forwardPeak";
import {
  BAKE_BIN_COUNT,
  BAKE_WAVELENGTHS_NM,
  CHANNEL_WAVELENGTHS_NM,
  columnLengthM,
  phaseAt,
} from "../medium";
import {
  type BakeGround,
  type BakeLayering,
  bakeBins,
  bakeLayering,
  binGroundAlbedo,
  type BinSolve,
  type BinTerm,
  checkBakeInputs,
  deltaMLayer,
  gatherBins,
  type MsBinShare,
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
  normalizedAssociatedLegendre,
  type OrdinateLayer,
  solveDiscreteOrdinates,
} from "./discreteOrdinates";
import { EXP_SUM_OPTIONS, type ExpSumOptions } from "./expSum";
import {
  bakePolarisationCorrection,
  type PolarisationChannelShare,
  polarisationChannel,
  type PolarisationGround,
  type PolarisationOptions,
  type PolarisationTable,
  preparePolarisation,
  type PreparedPolarisation,
} from "./polarisation";
import { ENVELOPE_FLOOR_CHANNEL } from "./regime";
import { type SphericalBeam, sphericalBeamPath } from "./sphericalBeam";
import { type SpectralColumn, spectralOptics, type SpectralPhase } from "./spectralColumn";
import { cos, exp } from "../../../lib/pinnedMath";

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
 * The deck's own tables' azimuthal modes, m = 0 … 7 (R08.T15.a.2; main's ruling 2 of 2026-10-29): of
 * its reflectance less its single scattering, which the kernels compute exactly (the module's
 * remarks). The research advice measured m ≤ 7 of it to miss ρ by 0.45–1.0% at μ, μ₀ ≥ 0.25 and
 * 3.8–7.9% at ≥ 0.1 on three decks, m ≤ 5 by 0.9–2.7% and 11–23%; the `giant-deck` gate is recorded
 * at 3, 6 and 8. The upper medium's J and its coupling to the deck keep `bake.ts`'s
 * `THICK_BAKE_MODES`.
 */
export const DECK_MODES = 8;

/**
 * The deck's phase table's grid: u = √(θ ÷ π), 256 entries, i ÷ 255 at entry i, the kernels' phase
 * tables' (`forwardPeak.ts`'s `FORWARD_PEAK_U`, `tables.ts`'s `PHASE_TABLE_ENTRIES`).
 */
export const DECK_PHASE_U: Float64Array = FORWARD_PEAK_U;

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
  /**
   * The upper medium's grid; `bake.ts`'s `THICK_BAKE_GRID` by default, its 46 rows, the 14 below
   * the horizontal among them (R08.T15.a.2): the deck's own tables and the table below it take its
   * rows above the horizontal.
   */
  readonly grid?: ThickBakeGrid;
  /** The exponential sums' options; `expSum.ts`'s by default. */
  readonly expSum?: ExpSumOptions;
  /** The deck's layers, a whole number of the grid's altitude strides; {@link DECK_LAYERS} by default. */
  readonly deckLayers?: number;
  /**
   * The floor's albedo in every bin, in [0, 1], a test's option (R08.T15.a.2's record of an
   * envelope's floor at 0 and 1): the ground's under `ground` and 0 under `envelope` by default.
   */
  readonly floorAlbedo?: number;
  /**
   * The bake bins prepared, ascending: every bin by default, or a share of them for a job that
   * bakes those alone ({@link bakeDeckBins}; `bake.ts`'s `prepareBake`).
   */
  readonly bins?: ReadonlyArray<number>;
}

/** A deck band's options. */
export interface DeckBandOptions {
  /**
   * The modes of the deck's sky-lit radiance, `diffuse`, a test's option (main's ruling 3 of
   * 2026-10-29): the upper medium solved to them, J still stored to `THICK_BAKE_MODES`; a whole
   * number in [`THICK_BAKE_MODES`, {@link DECK_MODES}], `THICK_BAKE_MODES` by default.
   */
  readonly diffuseModes?: number;
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
  /**
   * Per bin, the floor's albedo: the ground's (`upper.groundAlbedo`), or 0 under an envelope, or
   * the test's {@link DeckOptions.floorAlbedo}.
   */
  readonly floorAlbedo: Float64Array;
  /** {@link DeckOptions.floorAlbedo}, the polarisation correction's floor too, or `null`. */
  readonly floorAlbedoOverride: number | null;
  /**
   * Per bin, per term of the column, its whole phase function per steradian at the bin's wavelength
   * on {@link DECK_PHASE_U}, the exact single scattering's (the module's remarks); 0 for a term
   * that scatters nothing.
   */
  readonly termPhases: ReadonlyArray<ReadonlyArray<Float64Array>>;
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
 * A phase function per steradian at each of {@link DECK_PHASE_U}'s angles, θ = π u²: a table's
 * channel 0 and Rayleigh's closed form as `medium.ts`'s `phaseAt` reads them, and a `moments` phase
 * as its Legendre series, (1 ÷ 4π) Σ_l (2l + 1)(χ_l − f_c) P_l(cos θ), its forward delta's share
 * f_c taken out, which no angle of a reflection reaches.
 */
function phaseOnGrid(phase: SpectralPhase): Float64Array {
  const out = new Float64Array(DECK_PHASE_U.length);
  for (const [i, u] of DECK_PHASE_U.entries()) {
    const mu = cos(Math.PI * u * u);
    let value: number;
    switch (phase.kind) {
      case "none":
        value = 0;
        break;
      case "rayleigh": {
        const rho = phase.depolarisation;
        value = phaseAt({ kind: "rayleigh", depolarisation: [rho, rho, rho] }, mu)[0];
        break;
      }
      case "tabulated":
        value = phaseAt({ kind: "tabulated", table: phase.table }, mu)[0];
        break;
      case "moments": {
        const { moments, forwardDelta } = phase;
        // P_l by its recurrence, l P_l = (2l − 1) μ P_{l−1} − (l − 1) P_{l−2}.
        let previous = 1;
        let current = mu;
        let sum = (moments[0] ?? Number.NaN) - forwardDelta;
        for (let l = 1; l < moments.length; l += 1) {
          sum += (2 * l + 1) * ((moments[l] ?? Number.NaN) - forwardDelta) * current;
          const next = ((2 * l + 1) * mu * current - l * previous) / (l + 1);
          previous = current;
          current = next;
        }
        value = sum / (4 * Math.PI);
        break;
      }
    }
    out[i] = value;
  }
  return out;
}

/**
 * Everything of a deck's bake but the band (the module's remarks): the floor, the upper medium's and
 * the deck's layers, the upper medium's thick-bake preparation (the column's optics in the 15
 * bins, its table's point optics, and each light's exponential sums fitted over the whole column),
 * and each term's whole phase function in each bin for the deck's exact single scattering.
 *
 * @throws RangeError as `bake.ts`'s `prepareBake` refuses the grid or lights, for deck layers that
 *   are not a whole number of the grid's strides, a floor albedo outside [0, 1], a split that does
 *   not lie above the floor and below the column's top, or as the column's optics refuse.
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
  const override = options.floorAlbedo ?? null;
  if (override !== null && !(override >= 0 && override <= 1)) {
    throw new RangeError(`a deck's floor albedo lies in [0, 1], not ${override}`);
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
  if (override !== null) {
    floorAlbedo.fill(override);
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
  const upper = prepareBake(
    column,
    lights,
    grid,
    options.expSum ?? EXP_SUM_OPTIONS,
    { layering: upperLayering, fitLayering: whole },
    options.bins,
  );
  return {
    split,
    floorAltitudeM,
    upper,
    upperLayering,
    deck,
    whole,
    floorAlbedo,
    floorAlbedoOverride: override,
    termPhases: upper.binPhases.map((terms, b) =>
      // A share's other bins hold no optics, and take no phase.
      terms.map((phase, t) =>
        (upper.binOptics[b]?.[t]?.scatteringPerM ?? 0) > 0
          ? phaseOnGrid(phase)
          : new Float64Array(DECK_PHASE_U.length),
      ),
    ),
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
export interface DeckOrdinateLayers {
  /** The delta-M layers, from the top down. */
  readonly layers: ReadonlyArray<OrdinateLayer>;
  /** Each layer's vertical extinction optical depth without the gases, in the band's frame. */
  readonly extinction: Float64Array;
  /** Each layer's depth as the kernels march it: the gases' and the extinction less the forward delta. */
  readonly kernelDepths: Float64Array;
  /** Each layer's truncated fraction f, delta-M's (`bake.ts`'s `DeltaMLayer.truncated`). */
  readonly truncated: Float64Array;
  /** Per layer, each term's share of its scattering, `[p][t]`, all 0 where it scatters nothing. */
  readonly scatteringShares: ReadonlyArray<Float64Array>;
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
  const truncated = new Float64Array(layerCount);
  const scatteringShares: Float64Array[] = [];
  for (let p = 0; p < layerCount; p += 1) {
    let tau = 0;
    let tauS = 0;
    let delta = 0;
    const moments = new Float64Array(streams + 1);
    const shares = new Float64Array(terms.length);
    for (const [t, o] of terms.entries()) {
      const c = (layerColumnsM[t]?.[p] ?? Number.NaN) * factor;
      tau += (o.scatteringPerM + o.absorptionPerM) * c;
      const s = o.scatteringPerM * c;
      if (s > 0) {
        tauS += s;
        delta += s * o.forwardDelta;
        shares[t] = s;
        for (let l = 0; l <= streams; l += 1) {
          moments[l] = (moments[l] ?? 0) + s * (o.moments[l] ?? Number.NaN);
        }
      }
    }
    const forwardDelta = tauS > 0 ? delta / tauS : 0;
    for (let l = 0; l <= streams; l += 1) {
      moments[l] = tauS > 0 ? (moments[l] ?? 0) / tauS : l === 0 ? 1 : 0;
    }
    if (tauS > 0) {
      for (let t = 0; t < shares.length; t += 1) {
        shares[t] = (shares[t] ?? 0) / tauS;
      }
    }
    const total = tau + factor * (absorptionDepth[offset + p] ?? Number.NaN);
    const scaled = deltaMLayer(total, tauS, moments, forwardDelta, streams);
    layers.push(scaled.layer);
    extinction[p] = tau;
    kernelDepths[p] = Math.max(total - tauS * forwardDelta, 0);
    truncated[p] = scaled.truncated;
    scatteringShares.push(shares);
  }
  return { layers, extinction, kernelDepths, truncated, scatteringShares };
}

/**
 * Each layer's share of a slab's single scattering from a beam at μ₀ to an outgoing cosine μ, plane
 * parallel: E_p = e^(−τ_above x) (1 − e^(−τ_p x)), x = 1 ÷ μ₀ + 1 ÷ μ, τ the layers' own (the
 * module's remarks).
 */
function layerEscapes(layers: ReadonlyArray<OrdinateLayer>, mu: number, mu0: number): Float64Array {
  const x = 1 / mu0 + 1 / mu;
  const out = new Float64Array(layers.length);
  let above = 0;
  for (const [p, layer] of layers.entries()) {
    const top = exp(-above * x);
    above += layer.opticalDepth;
    out[p] = top - exp(-above * x);
  }
  return out;
}

/**
 * The single scattering of a slab's layers as their own discrete-ordinates solve holds it
 * (`deckReflectance`'s, the module's remarks): per mode m = 0 … `maxMode`, ρ_ss^m(μ, μ′) =
 * ((2 − δ_m0) ÷ 4) Σ_p ω′_p P′_p^m(μ, −μ′) E_p ÷ (μ′ + μ), P′_p^m = Σ_l (2l + 1) χ′_l Λ_l^m(μ)
 * Λ_l^m(−μ′) over the layer's moments the solve takes (χ′₀ … χ′_{NSTR−1}; the primes the deck's
 * delta-M scaling, its layers' as they are given), the slab's beam plane-parallel at μ′
 * of flux 1 normal to itself, at the outgoing and incident cosines of `cosines`.
 *
 * @param streams - NSTR of the slab's solve: its moments χ₀ … χ_{NSTR−1} are read.
 */
export function deckSingleScattering(
  layers: ReadonlyArray<OrdinateLayer>,
  cosines: GroundCosines,
  maxMode: number,
  streams: number = THICK_BAKE_STREAMS,
): GroundReflectance {
  const { outgoingCosines, incidentCosines } = cosines;
  const columns = incidentCosines.length;
  const lmax = streams - 1;
  const modes = Array.from(
    { length: maxMode + 1 },
    () => new Float64Array(outgoingCosines.length * columns),
  );
  for (const [i, mu] of outgoingCosines.entries()) {
    for (const [j, mu0] of incidentCosines.entries()) {
      const escapes = layerEscapes(layers, mu, mu0);
      for (let m = 0; m <= maxMode; m += 1) {
        const out = normalizedAssociatedLegendre(m, lmax, mu);
        const into = normalizedAssociatedLegendre(m, lmax, -mu0);
        let sum = 0;
        for (const [p, layer] of layers.entries()) {
          const omega = layer.singleScatteringAlbedo;
          if (!(omega > 0)) {
            continue;
          }
          let phase = 0;
          for (let l = m; l <= lmax; l += 1) {
            phase +=
              (2 * l + 1) *
              (layer.moments[l] ?? 0) *
              (out[l] ?? Number.NaN) *
              (into[l] ?? Number.NaN);
          }
          sum += omega * phase * (escapes[p] ?? Number.NaN);
        }
        const row = modes[m];
        if (row !== undefined) {
          row[i * columns + j] = (((m === 0 ? 1 : 2) / 4) * sum) / (mu0 + mu);
        }
      }
    }
  }
  return { outgoingCosines, incidentCosines, modes };
}

/**
 * The exact single scattering's geometry factor of a slab's delta-M layers (Nakajima and Tanaka's
 * TMS, the module's remarks): G(μ, μ′) = π Σ_p (ω′_p ÷ (1 − f_p)) E_p ÷ (μ′ + μ), sr, at the outgoing
 * and incident cosines of `cosines`, entry (i, j) at `i × incidentCosines.length + j`, so that
 * G p(Θ) is the reflectance of the slab's single scattering by a phase function p per steradian.
 *
 * @param truncated - Each layer's truncated fraction f.
 */
export function deckSingleGeometry(
  layers: ReadonlyArray<OrdinateLayer>,
  truncated: ArrayLike<number>,
  cosines: GroundCosines,
): Float64Array {
  const { outgoingCosines, incidentCosines } = cosines;
  const columns = incidentCosines.length;
  const out = new Float64Array(outgoingCosines.length * columns);
  for (const [i, mu] of outgoingCosines.entries()) {
    for (const [j, mu0] of incidentCosines.entries()) {
      out[i * columns + j] = (Math.PI * trueScatteringOf(layers, truncated, mu, mu0)) / (mu0 + mu);
    }
  }
  return out;
}

/** Σ_p (ω′_p ÷ (1 − f_p)) E_p at (μ, μ₀): the slab's true single scattering, each layer's escaping. */
function trueScatteringOf(
  layers: ReadonlyArray<OrdinateLayer>,
  truncated: ArrayLike<number>,
  mu: number,
  mu0: number,
): number {
  const escapes = layerEscapes(layers, mu, mu0);
  let sum = 0;
  for (const [p, layer] of layers.entries()) {
    sum += (layer.singleScatteringAlbedo / (1 - (truncated[p] ?? 0))) * (escapes[p] ?? Number.NaN);
  }
  return sum;
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
  return (bin, term, cosines, maxMode) =>
    deckReflectance(
      deckLayersOf(prepared, band, bin, term).layers,
      prepared.floorAlbedo[bin] ?? Number.NaN,
      cosines,
      maxMode,
    );
}

// ---------------------------------------------------------------------------------------------
// One band.

/** One light slot's deck table. */
export interface DeckSlot {
  /**
   * ρ^m(μ_v, μ₀) − ρ_ss^m(μ_v, μ₀) per channel, the multiple-scattering part of the deck's
   * reflectance, its single scattering out (the module's remarks), dimensionless, S × V × M × 3
   * row-major, entry `((s V + v) M + m) 3 + c`: s the sun's cosine, v the upward view cosine (the
   * reflected light's direction of travel; an observer looking down at elevation −e reads
   * μ_v = sin e), m the mode of {@link DeckTables.modes}, c red, green, blue; 0 below the horizontal.
   * Each bin's is the mean over its gases' terms by their weights. The kernels read it, times
   * their own transmittance, in place of {@link DeckSlot.beamReflection} for a beam the bake did
   * not carry.
   */
  readonly reflectance: Float32Array;
  /**
   * The deck's upward radiance at its top less its reflection of the kernels' own beam,
   * I^m(τ_L, +μ_v) − (μ₀ T_k ÷ π) ρ^m(μ_v, μ₀) (the module's remarks), sr⁻¹ per unit of the star's
   * irradiance normal to its beam at the top, S × V × M_D × 3, M_D {@link DeckTables.diffuseModes},
   * entry `((s V + v) M_D + m) 3 + c`.
   */
  readonly diffuse: Float32Array;
  /**
   * Σ_t w_t T_k,t (ρ^m_t − ρ_ss,t^m), the multiple-scattering part weighted term by term by the
   * kernels' beam at the deck's top (extinction less the forward delta, pseudo-spherical), in the
   * layout of `reflectance`: (μ₀ ÷ π) times it and {@link DeckSlot.singleBeamGeometry}'s G_β p(Θ),
   * with `diffuse`, is the deck's upward radiance at its top. The kernels' own T_k times
   * `reflectance` misses the covariance of a bin's terms' beams and reflectances, which a strong gas
   * term makes large (the science check, 2026-10-29).
   */
  readonly beamReflection: Float32Array;
  /**
   * G(μ_v, μ₀), the exact single scattering's geometry factor ({@link deckSingleGeometry}), sr,
   * S × V × 3, entry `(s V + v) 3 + c`: for a beam the bake did not carry, as `reflectance` is.
   */
  readonly singleGeometry: Float32Array;
  /** Σ_t w_t T_k,t G_t, G weighted term by term by the kernels' beam, as `beamReflection` is. */
  readonly singleBeamGeometry: Float32Array;
  /**
   * p(Θ) per channel, sr⁻¹, at {@link DECK_PHASE_U}'s angles, entry `i 3 + c`: the deck's layers'
   * whole phase functions mixed by G's weights at the nadir pair in each bin, and the bins' by their
   * converted weights (the module's remarks).
   */
  readonly phase: Float32Array;
  /** The deck's solves, one a solve of the table above it, and its own at the kernels' cosines. */
  readonly solves: number;
}

/**
 * One latitude band of a cloud-deck split (Provides' `DeckTables`): the table above the deck and
 * the deck's reflectance and radiance, one slot a light.
 *
 * @remarks
 * A band of the default grids holds `above`'s 874,368 B a light as `f32` (`bake.ts`'s
 * `MsSourceTable`) and the deck's 46 × 8 × (8 + 8 + 3 + 2) × 3 values and its phase's 256 × 3,
 * 95,808 B a light.
 */
export interface DeckTables {
  readonly band: ThickBakeBand;
  /** The split's altitude, h\* in m: the upper medium's ground, the same in every band. */
  readonly deckAltitudeM: number;
  /** The deck's floor, h\* in m. */
  readonly floorAltitudeM: number;
  /** J_ms over the upper medium, the deck its ground (`bake.ts`'s table, altitudes from the split). */
  readonly above: MsSourceTable;
  /** μ₀, ascending in [−1, 1]: the grid's, the rows below the horizontal among them. */
  readonly sunCosines: Float64Array;
  /** μ_v, ascending in (0, 1]: the grid's upward cosines. */
  readonly viewCosines: Float64Array;
  /** M, the modes of `reflectance` and `beamReflection`, m = 0 … M − 1: {@link DECK_MODES}. */
  readonly modes: number;
  /** M_D, the modes of `diffuse`: `bake.ts`'s `THICK_BAKE_MODES`, or the test's option. */
  readonly diffuseModes: number;
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

/** A solve's deck record in its bin: per point of S × V × M (or × M_D, or alone), and its phase's weights. */
interface DeckRecord {
  /** ρ^m − ρ_ss^m. */
  readonly reflectance: Float64Array;
  readonly diffuse: Float64Array;
  /** T_k (ρ^m − ρ_ss^m), weighted by the kernels' beam at the deck. */
  readonly beamReflection: Float64Array;
  /** G, S × V. */
  readonly singleGeometry: Float64Array;
  /** T_k G. */
  readonly singleBeamGeometry: Float64Array;
  /** Per term of the column, Σ_p W_p s_p,t, W_p the layer's share of G at the nadir pair. */
  readonly phaseWeights: Float64Array;
  /** Σ_p W_p. */
  readonly nadirWeight: number;
}

/** The deck record's tables summed over terms and converted, in their order. */
const DECK_RECORD_KEYS = [
  "reflectance",
  "diffuse",
  "beamReflection",
  "singleGeometry",
  "singleBeamGeometry",
] as const;

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
 * The deck's delta-M layers in one band, bin and term of its gases, from its top down, as its solves
 * and its single scattering take them.
 */
export function deckLayersOf(
  prepared: PreparedDeck,
  band: ThickBakeBand,
  bin: number,
  term: BinTerm,
): DeckOrdinateLayers {
  const { upper, deck } = prepared;
  return deckOrdinateLayers(
    upper.binOptics[bin] ?? [],
    deck.layerColumnsM,
    deck.interfaceHeightsM.length - 1,
    band.coefficientFactor,
    term.absorptionDepth,
    upper.grid.layers,
    THICK_BAKE_STREAMS,
  );
}

/**
 * A cloud-deck split's bake of one latitude band (Provides' `bakeDeck`, in T14.b's shape; the
 * module's remarks): the thick bake of the upper medium with the deck as its ground, the deck's
 * reflectance solved once a bin and term of its gases at the bake's modes for that ground and once
 * more at {@link DECK_MODES} at the kernels' cosines, its single scattering, and the deck's tables
 * from those solves, summed by the terms' weights and converted to the channels: the band's
 * {@link bakeDeckBins} of every bin, gathered by {@link gatherDeckBins}.
 *
 * @param band - The band's index in `bake.ts`'s `thickBakeBands` order.
 * @throws RangeError for a band the body does not have, a `diffuseModes` outside
 *   [`THICK_BAKE_MODES`, {@link DECK_MODES}], a preparation of some bins alone, or as the solver
 *   refuses a layer.
 */
export function bakeDeck(
  prepared: PreparedDeck,
  body: ThickBakeBody,
  band: number,
  options: DeckBandOptions = {},
): DeckTables {
  return gatherDeckBins([bakeDeckBins(prepared, body, band, options)]);
}

/** One light's deck in one bin, summed over the bin's terms, before the channels. */
export interface DeckBinLight {
  readonly reflectance: Float64Array;
  readonly diffuse: Float64Array;
  readonly beamReflection: Float64Array;
  readonly singleGeometry: Float64Array;
  readonly singleBeamGeometry: Float64Array;
  /** The bin's phase times its nadir weight, Σ_t (Σ_p W_p s_p,t) p_t(Θ), on {@link DECK_PHASE_U}. */
  readonly weightedPhase: Float64Array;
  /** Σ_p W_p over the bin's terms by their weights. */
  readonly nadirWeight: number;
}

/** One bin of a deck's band, before the channels (the bins' share, {@link bakeDeckBins}). */
export interface DeckBinPart {
  readonly bin: number;
  /** The deck's vertical extinction optical depth in the bin, in the band's frame. */
  readonly deckOpticalDepth: number;
  /** One a light of the preparation, in its order. */
  readonly lights: ReadonlyArray<DeckBinLight>;
  /** Below a deck over a surface, one a light; `null` for an envelope. */
  readonly below: ReadonlyArray<BelowBinLight> | null;
}

/** One light's table below the deck in one bin, summed over its terms, before the channels. */
export interface BelowBinLight {
  readonly radiance: Float64Array;
  readonly irradiance: Float64Array;
  readonly direct: Float64Array;
}

/** The grid of the table below a deck over a surface, the same in every bin. */
export interface BelowDeckGrid {
  readonly altitudesM: Float64Array;
  readonly sunCosines: Float64Array;
  readonly travelCosines: Float64Array;
  readonly modes: number;
}

/**
 * A deck's band over some of its bins (plan R08, R08.T14.d's bake time): what one optics job of a
 * deck's band split by bins answers, for {@link gatherDeckBins} to take with the band's other bins.
 */
export interface DeckBinShare {
  /** The table above the deck, the same bins' share (`bake.ts`'s `MsBinShare`). */
  readonly above: MsBinShare;
  readonly band: ThickBakeBand;
  readonly deckAltitudeM: number;
  readonly floorAltitudeM: number;
  readonly sunCosines: Float64Array;
  readonly viewCosines: Float64Array;
  readonly modes: number;
  readonly diffuseModes: number;
  /** The table below a deck over a surface's grid; `null` for an envelope. */
  readonly below: BelowDeckGrid | null;
  /** The bins baked, ascending. */
  readonly bins: ReadonlyArray<DeckBinPart>;
}

/**
 * One deck's band in the bins `prepared` holds ({@link DeckOptions.bins}), before the channels:
 * {@link bakeDeck}'s solves, a bin share of them.
 *
 * @throws RangeError as {@link bakeDeck}.
 */
export function bakeDeckBins(
  prepared: PreparedDeck,
  body: ThickBakeBody,
  band: number,
  options: DeckBandOptions = {},
): DeckBinShare {
  const geometry = bandOf(body, band);
  const diffuseModes = options.diffuseModes ?? THICK_BAKE_MODES;
  if (!(
    Number.isInteger(diffuseModes) &&
    diffuseModes >= THICK_BAKE_MODES &&
    diffuseModes <= DECK_MODES
  )) {
    throw new RangeError(
      `the deck's sky-lit radiance takes ${THICK_BAKE_MODES} to ${DECK_MODES} modes, not ${diffuseModes}`,
    );
  }
  const { upper, floorAlbedo } = prepared;
  const { grid } = upper;
  const layers = grid.layers;
  const suns = grid.sunCosines.length;
  const views = grid.upwardCosines.length;
  const modes = DECK_MODES;
  const n = THICK_BAKE_STREAMS / 2;
  // The suns above the horizontal, the grid's last: the deck's own solve's beams.
  const daylit = grid.sunCosines.filter((mu) => mu > 0);
  const firstDaylit = suns - daylit.length;
  const kernelCosines: GroundCosines = {
    outgoingCosines: grid.upwardCosines,
    incidentCosines: daylit,
  };
  const termCount = upper.column.terms.length;
  // The deck's layers once a bin and term, for the coupling's solve and the deck's own: by bin
  // first, since a column without gases shares one term among its bins.
  const layered = new Map<number, Map<BinTerm, DeckOrdinateLayers>>();
  const layersOf = (bin: number, term: BinTerm): DeckOrdinateLayers => {
    let byTerm = layered.get(bin);
    if (byTerm === undefined) {
      byTerm = new Map();
      layered.set(bin, byTerm);
    }
    let found = byTerm.get(term);
    if (found === undefined) {
      found = deckLayersOf(prepared, geometry, bin, term);
      byTerm.set(term, found);
    }
    return found;
  };
  const ground: BakeGround = (bin, term, cosines, maxMode) =>
    deckReflectance(layersOf(bin, term).layers, floorAlbedo[bin] ?? Number.NaN, cosines, maxMode);
  const records = new Map<number, Map<BinTerm, DeckRecord>>();
  const recordOf = (bin: number, term: BinTerm, solve: BinSolve): DeckRecord => {
    const coupling = solve.groundReflectance;
    if (coupling === undefined) {
      throw new Error(`bin ${String(bin)}'s solve took no deck`);
    }
    const deck = layersOf(bin, term);
    const own = deckReflectance(
      deck.layers,
      floorAlbedo[bin] ?? Number.NaN,
      kernelCosines,
      modes - 1,
    );
    const single = deckSingleScattering(deck.layers, kernelCosines, modes - 1);
    const geometryFactor = deckSingleGeometry(deck.layers, deck.truncated, kernelCosines);
    const size = suns * views;
    const out = {
      reflectance: new Float64Array(size * modes),
      diffuse: new Float64Array(size * diffuseModes),
      beamReflection: new Float64Array(size * modes),
      singleGeometry: new Float64Array(size),
      singleBeamGeometry: new Float64Array(size),
    };
    const columns = coupling.incidentCosines.length;
    const daylitColumns = daylit.length;
    for (let s = 0; s < suns; s += 1) {
      const beam = solve.solution.beams[s];
      const mu0 = grid.sunCosines[s] ?? Number.NaN;
      const lit = s >= firstDaylit;
      const j = s - firstDaylit;
      const kernel = lit ? (solve.kernelBeams[s]?.interfaceTransmission[layers] ?? Number.NaN) : 0;
      for (let v = 0; v < views; v += 1) {
        const point = s * views + v;
        if (lit) {
          const g = geometryFactor[v * daylitColumns + j] ?? Number.NaN;
          out.singleGeometry[point] = g;
          out.singleBeamGeometry[point] = kernel * g;
          for (let m = 0; m < modes; m += 1) {
            const at = v * daylitColumns + j;
            const multiple =
              (own.modes[m]?.[at] ?? Number.NaN) - (single.modes[m]?.[at] ?? Number.NaN);
            out.reflectance[point * modes + m] = multiple;
            out.beamReflection[point * modes + m] = kernel * multiple;
          }
        }
        for (let m = 0; m < diffuseModes; m += 1) {
          // The coupling's cosines are `groundCosinesOf`'s: the nodes first, then the views and the
          // suns above the horizontal.
          const r = lit ? (coupling.modes[m]?.[(n + v) * columns + n + j] ?? Number.NaN) : 0;
          const leaving = beam?.modes[m]?.userUp[layers * views + v] ?? Number.NaN;
          out.diffuse[point * diffuseModes + m] = leaving - ((mu0 * kernel) / Math.PI) * r;
        }
      }
    }
    // The phase's weights: each layer's share of G at the nadir pair, W_p = ω′_p E_p ÷ (1 − f_p),
    // spread over its terms by their scattering.
    const escapes = layerEscapes(deck.layers, 1, 1);
    const phaseWeights = new Float64Array(termCount);
    let nadirWeight = 0;
    for (const [p, layer] of deck.layers.entries()) {
      const w =
        (layer.singleScatteringAlbedo / (1 - (deck.truncated[p] ?? 0))) *
        (escapes[p] ?? Number.NaN);
      nadirWeight += w;
      const shares = deck.scatteringShares[p];
      for (let t = 0; t < termCount; t += 1) {
        phaseWeights[t] = (phaseWeights[t] ?? 0) + w * (shares?.[t] ?? 0);
      }
    }
    return { ...out, phaseWeights, nadirWeight };
  };
  const above = bakeBins(upper, body, band, {
    ground,
    onSolve: (bin, term, solve) => {
      let byTerm = records.get(bin);
      if (byTerm === undefined) {
        byTerm = new Map();
        records.set(bin, byTerm);
      }
      byTerm.set(term, recordOf(bin, term, solve));
    },
    solveModes: diffuseModes,
  });
  const deckLayerCount = prepared.deck.interfaceHeightsM.length - 1;
  const none = new Float64Array(prepared.whole.interfaceHeightsM.length - 1);
  const points = suns * views;
  const sizes = {
    reflectance: points * modes,
    diffuse: points * diffuseModes,
    beamReflection: points * modes,
    singleGeometry: points,
    singleBeamGeometry: points,
  } as const;
  const entries = DECK_PHASE_U.length;
  let belowParts: ReturnType<typeof belowDeckBins> | null;
  switch (prepared.split.floor) {
    case "ground":
      belowParts = belowDeckBins(prepared, geometry);
      break;
    case "envelope":
      belowParts = null;
      break;
  }
  const parts = upper.bins.map((b): DeckBinPart => {
    const lights = upper.lights.map((light, slot): DeckBinLight => {
      const bin = light.bins[b];
      if (bin === undefined) {
        throw new Error(`light ${slot} has no bin ${b}`);
      }
      const sum = {
        reflectance: new Float64Array(sizes.reflectance),
        diffuse: new Float64Array(sizes.diffuse),
        beamReflection: new Float64Array(sizes.beamReflection),
        singleGeometry: new Float64Array(sizes.singleGeometry),
        singleBeamGeometry: new Float64Array(sizes.singleBeamGeometry),
      };
      const phaseWeights = new Float64Array(termCount);
      let nadirWeight = 0;
      for (const term of bin.terms) {
        const record = records.get(b)?.get(term);
        if (record === undefined) {
          throw new Error(`light ${slot}'s bin ${b} has a term with no deck`);
        }
        const w = term.weight;
        for (const key of DECK_RECORD_KEYS) {
          const target = sum[key];
          const source = record[key];
          for (let i = 0; i < target.length; i += 1) {
            target[i] = (target[i] ?? 0) + w * (source[i] ?? Number.NaN);
          }
        }
        for (let t = 0; t < termCount; t += 1) {
          phaseWeights[t] = (phaseWeights[t] ?? 0) + w * (record.phaseWeights[t] ?? Number.NaN);
        }
        nadirWeight += w * record.nadirWeight;
      }
      // The bin's phase times its nadir weight, Σ_t (Σ_p W_p s_p,t) p_t(Θ): converted below, and
      // divided by the converted nadir weight, so that G p is the bins' converted G p at the nadir.
      const weightedPhase = new Float64Array(entries);
      const phases = prepared.termPhases[b] ?? [];
      for (let t = 0; t < termCount; t += 1) {
        const share = phaseWeights[t] ?? 0;
        const phase = phases[t];
        if (share === 0 || phase === undefined) {
          continue;
        }
        for (let i = 0; i < entries; i += 1) {
          weightedPhase[i] = (weightedPhase[i] ?? 0) + share * (phase[i] ?? Number.NaN);
        }
      }
      return { ...sum, weightedPhase, nadirWeight };
    });
    return {
      bin: b,
      deckOpticalDepth: deckOrdinateLayers(
        upper.binOptics[b] ?? [],
        prepared.deck.layerColumnsM,
        deckLayerCount,
        geometry.coefficientFactor,
        none,
        layers,
        THICK_BAKE_STREAMS,
      ).extinction.reduce((sum, tau) => sum + tau, 0),
      lights,
      below: belowParts?.parts.get(b) ?? null,
    };
  });
  return {
    above,
    band: geometry,
    deckAltitudeM: prepared.split.altitudeM,
    floorAltitudeM: prepared.floorAltitudeM,
    sunCosines: grid.sunCosines.slice(),
    viewCosines: grid.upwardCosines.slice(),
    modes,
    diffuseModes,
    below: belowParts?.grid ?? null,
    bins: parts,
  };
}

/**
 * A deck's band from its bin shares ({@link bakeDeckBins}), which together hold every bin once:
 * the table above gathered as `bake.ts`'s `gatherBins` gathers a band, and the deck's and the
 * table below's bins converted to the channels in the bins' order, so that a deck baked a share of
 * its bins a job is the bits of one baked whole.
 *
 * @throws RangeError for shares that do not hold every bin once, or as `bake.ts`'s `gatherBins`.
 */
export function gatherDeckBins(shares: ReadonlyArray<DeckBinShare>): DeckTables {
  const first = shares[0];
  if (first === undefined) {
    throw new RangeError("a deck's band gathers at least one share of its bins");
  }
  const byBin: DeckBinPart[] = [];
  for (const share of shares) {
    if ((share.below === null) !== (first.below === null)) {
      throw new RangeError("a deck's bin shares are over a surface and an envelope");
    }
    for (const part of share.bins) {
      if (byBin[part.bin] !== undefined) {
        throw new RangeError(`a deck's bin shares hold bin ${part.bin} twice`);
      }
      byBin[part.bin] = part;
    }
  }
  const parts = Array.from({ length: BAKE_BIN_COUNT }, (_, b): DeckBinPart => {
    const part = byBin[b];
    if (part === undefined) {
      throw new RangeError(`a deck's bin shares hold no bin ${b}`);
    }
    return part;
  });
  const above = gatherBins(shares.map((share) => share.above));
  const { conversions } = first.above;
  const entries = DECK_PHASE_U.length;
  const slots = conversions.map((factors, slot): DeckSlot => {
    const perBin = parts.map((part) => {
      const light = part.lights[slot];
      if (light === undefined) {
        throw new RangeError(`bin ${part.bin} of the deck holds no light ${slot}`);
      }
      return light;
    });
    const converted = (key: (typeof DECK_RECORD_KEYS)[number]): Float32Array =>
      inChannels(
        factors,
        perBin.map((bin) => bin[key]),
        perBin[0]?.[key].length ?? 0,
      );
    const weightedPhase = inChannels(
      factors,
      perBin.map((bin) => bin.weightedPhase),
      entries,
    );
    const nadir = inChannels(
      factors,
      perBin.map((bin) => Float64Array.of(bin.nadirWeight)),
      1,
    );
    const phase = weightedPhase.map((value, i) => {
      const weight = nadir[i % 3] ?? Number.NaN;
      return weight > 0 ? value / weight : 0;
    });
    return {
      reflectance: converted("reflectance"),
      diffuse: converted("diffuse"),
      beamReflection: converted("beamReflection"),
      singleGeometry: converted("singleGeometry"),
      singleBeamGeometry: converted("singleBeamGeometry"),
      phase,
      solves: 2 * (above.slots[slot]?.solves ?? 0),
    };
  });
  return {
    band: first.band,
    deckAltitudeM: first.deckAltitudeM,
    floorAltitudeM: first.floorAltitudeM,
    above,
    sunCosines: first.sunCosines.slice(),
    viewCosines: first.viewCosines.slice(),
    modes: first.modes,
    diffuseModes: first.diffuseModes,
    deckOpticalDepth: Float64Array.from(parts, (part) => part.deckOpticalDepth),
    slots,
    below: first.below === null ? null : belowDeckOf(first.below, parts, conversions),
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

/** A grid's sun cosines above the horizontal: the table below the deck's (R08.T15.b.1). */
function daylitCosines(grid: ThickBakeGrid): Float64Array {
  return grid.sunCosines.filter((mu) => mu > 0);
}

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
 * pseudo-spherical beams over all of them, and the ground's albedo beneath, at the grid's μ₀ above
 * the horizontal (the table below takes those alone, as R08.T15.b.1 built it) and its view cosines,
 * at the bake's modes.
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
  const suns = daylitCosines(grid);
  const scaledBeams = Array.from(suns, (mu0) =>
    sphericalBeamPath(radius, heights, scaledDepths, mu0),
  );
  const kernelBeams = Array.from(suns, (mu0) =>
    sphericalBeamPath(radius, heights, kernelDepths, mu0),
  );
  const solution = solveDiscreteOrdinates({
    layers,
    groundAlbedo: floorAlbedo[bin] ?? Number.NaN,
    beamCosines: suns,
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
 * The table below the deck of one band in the bins `prepared` holds (the module's remarks): the
 * whole column solved once a bin and term, its fields read at the deck's altitudes and summed by
 * the terms' weights, before the channels ({@link belowDeckOf}).
 */
function belowDeckBins(
  prepared: PreparedDeck,
  band: ThickBakeBand,
): { readonly grid: BelowDeckGrid; readonly parts: Map<number, ReadonlyArray<BelowBinLight>> } {
  const { upper, deck } = prepared;
  const { grid } = upper;
  const deckLayers = deck.interfaceHeightsM.length - 1;
  const ground = grid.layers + deckLayers;
  const altitudes = deck.altitudesM.length;
  const sunCosines = daylitCosines(grid);
  const suns = sunCosines.length;
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
        const aureole = (sunCosines[s] ?? Number.NaN) * Math.max(scaled - kernel, 0);
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
  const records = new Map<number, Map<BinTerm, BelowRecord>>();
  const sums = upper.lights.map((light, slot) =>
    upper.bins.map((b): BelowBinLight => {
      const bin = light.bins[b];
      if (bin === undefined) {
        throw new Error(`light ${slot} has no bin ${b}`);
      }
      const sum = {
        radiance: new Float64Array(size),
        irradiance: new Float64Array(altitudes * suns),
        direct: new Float64Array(altitudes * suns),
      };
      let byTerm = records.get(b);
      if (byTerm === undefined) {
        byTerm = new Map();
        records.set(b, byTerm);
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
    }),
  );
  const upward = grid.upwardCosines;
  return {
    grid: {
      altitudesM: deck.altitudesM.slice(),
      sunCosines,
      travelCosines: Float64Array.from({ length: v2 }, (_, v) =>
        v < views ? -(upward[views - 1 - v] ?? Number.NaN) : (upward[v - views] ?? Number.NaN),
      ),
      modes,
    },
    parts: new Map(
      upper.bins.map((b, i) => [
        b,
        sums.map((perBin, slot) => {
          const part = perBin[i];
          if (part === undefined) {
            throw new Error(`light ${slot} has no bin ${b} below the deck`);
          }
          return part;
        }),
      ]),
    ),
  };
}

/** The table below the deck from every bin's sums, each light's converted to the channels. */
function belowDeckOf(
  grid: BelowDeckGrid,
  parts: ReadonlyArray<DeckBinPart>,
  conversions: ReadonlyArray<BinConversion["factors"]>,
): BelowDeckTable {
  const slots = conversions.map((factors, slot): BelowDeckSlot => {
    const sums = parts.map((part) => {
      const light = part.below?.[slot];
      if (light === undefined) {
        throw new RangeError(`bin ${part.bin} below the deck holds no light ${slot}`);
      }
      return light;
    });
    const size = (key: keyof BelowBinLight): number => sums[0]?.[key].length ?? 0;
    return {
      radiance: inChannels(
        factors,
        sums.map((sum) => sum.radiance),
        size("radiance"),
      ),
      irradiance: inChannels(
        factors,
        sums.map((sum) => sum.irradiance),
        size("irradiance"),
      ),
      direct: inChannels(
        factors,
        sums.map((sum) => sum.direct),
        size("direct"),
      ),
    };
  });
  return {
    altitudesM: grid.altitudesM,
    sunCosines: grid.sunCosines,
    travelCosines: grid.travelCosines,
    modes: grid.modes,
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
    floorAlbedo = prepared.floorAlbedoOverride ?? floorAlbedo;
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
 * One band's correction over the deck in one render channel (the optics pool's channel shares,
 * R08.T14.d's bake time): {@link bakeDeckPolarisation}'s solves, a channel's share of them,
 * `polarisation` prepared at that channel's wavelength alone (`polarisation.ts`'s
 * `polarisationChannel`).
 *
 * @param point - The channel's index in `polarisation`'s points.
 * @throws RangeError as `polarisation.ts`'s `polarisationChannel`.
 */
export function deckPolarisationChannel(
  prepared: PreparedDeck,
  polarisation: PreparedPolarisation,
  body: ThickBakeBody,
  band: number,
  point: number,
): PolarisationChannelShare {
  return polarisationChannel(
    polarisation,
    body,
    band,
    point,
    deckPolarisationGround(prepared, bandOf(body, band)),
  );
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
