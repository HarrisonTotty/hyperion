/**
 * The atmosphere's per-planet tables (plan R05, R05.T12.b, Design note 16): transmittance and
 * multiple scattering, built by compute kernels through the engine when the medium changes and
 * never when the sun moves, since both are per unit illuminance and depend on the medium alone.
 *
 * @remarks
 * Hillaire 2020's first two tables, at his sizes (Design note 16): transmittance 256 × 64 and
 * multiple scattering 32 × 32, the same on both settings. Each is a `presentation-only` kernel
 * writing an `rgba16float` storage texture made under the `atmosphere-tables` memory category; only
 * the smoke harness reads them back. The tables are spherical, on a shell whose ground is
 * `bottomRadiusM` from the centre; T12.c's lookups take r = √(MN) + h on the spheroid. Every kernel
 * forms its heights from heights, never as a radius less the ground's, and takes no builtin
 * trigonometry (plan R08, Design note 18); every table maps its unit ranges to its texels by
 * Bruneton 2017's exact pair (Design note 19). Both since R08.T6.g.
 *
 * The transmittance table stores optical depth, not transmittance (plan R08, Design note 8,
 * R08.T6.c): rgb the optical depth of every term but the absorbers, alpha absorber slot 0's column
 * u ÷ U, and a second `rgba16float` table beside it, the absorber columns, slots 1 to 4's. Each
 * light's curves of growth (Design note 5), uploaded with the medium ({@link packAbsorberCurves}),
 * turn the columns into optical depth on read, and every read site exponentiates in `f32`. Stored
 * as T, a slant path through a Venus-class atmosphere fell below the half float's least value.
 * The multiple-scattering table has one layer a light slot under an absorber, each built with that
 * light's curves, and one layer otherwise ({@link multiScatteringLayersOf}, R08.T7).
 *
 * The medium reaches every kernel in three parts (plan R08, R08.T6.b): its shell, term count,
 * steps and ground albedo in the `Medium` uniform ({@link packMediumUniform}), its terms in a
 * storage buffer of {@link MAX_TERMS} terms ({@link packMedium}), and its tabulated densities and
 * phases in two 2D array textures, one layer a term ({@link packDensityTables},
 * {@link packPhaseTables}). A compute pass binds no sampler, so the kernels read the tables by
 * `textureLoad` and interpolate by hand; a table of one layer binds where a kernel declares
 * `texture_2d_array`, as a one-layer array (R08.T0). A medium with no tabulated term of a kind
 * binds a one-layer, one-texel placeholder, which the kernels never read. The tables hold the
 * medium resampled onto the kernels' own grids, the medium {@link kernelMedium} gives, which the
 * CPU twin (`tablesCpu.ts`) takes as the kernels' oracle.
 *
 * Since R08.T7 that medium also has each tabulated phase's forward delta taken out
 * ({@link withoutForwardDelta}, Design note 6): the delta's light is the direct beam's. A tabulated
 * phase whose forward part is a lobe (`forwardLobe.ts`'s `lobeIsDrawn`) carries its lobe's share P
 * per channel in its phase code, and its higher orders' effective phase Ẽ(x, θ) in a third 2D
 * array texture, one layer a term ({@link packLobeTables}). The per-frame marches add the orders
 * at each sample; the multiple-scattering kernel takes the lobes' scattering P σ_s out of the
 * medium it integrates, as delta-M does, and raises the beam by e^(x P), so the orders' light is
 * not counted again in Hillaire's isotropic term.
 */

import { BUFFER_USAGE, TEXTURE_USAGE } from "../engine/gpuFlags";
import type { KernelPair } from "../engine/kernels";
import type { BufferHandle, ComputeHandle, RenderEngine, TextureHandle } from "../engine/types";
import type { AbsorberAmount, AbsorberCurve } from "./absorbers";
import {
  FORWARD_LOBE_X_NODES,
  FORWARD_ORDERS_ENTRIES,
  forwardLobeOfTable,
  lobeIsDrawn,
} from "./forwardLobe";
import { forwardPeakEntry } from "./forwardPeak";
import { MARCH_EDGES_MAX, marchEdgesM } from "./marchSteps";
import {
  type AtmosphereMedium,
  checkTabulatedTops,
  type DensityProfile,
  densityAt,
  layerEdgesM,
  type MediumTerm,
  type PhaseFunction,
  type PhaseMatrixElement,
  type PhaseTable,
  phaseTable,
  type TabulatedDensity,
  tabulatedDensity,
  type TermAbsorber,
} from "./medium";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import commonWgsl from "./shaders/common.wgsl?raw";
import mediumWgsl from "./shaders/medium.wgsl?raw";
import multiScatteringWgsl from "./shaders/multiScattering.wgsl?raw";
import transmittanceWgsl from "./shaders/transmittance.wgsl?raw";

/**
 * The most terms a medium may have: `MAX_TERMS` in `common.wgsl`, the length of the kernels'
 * `terms` buffer. The CPU twin holds no limit; a medium of more terms is the kernels' refusal.
 */
export const MAX_TERMS = 8;

/** A table's size, texels. */
export interface TableSize {
  readonly widthTexels: number;
  readonly heightTexels: number;
}

/** The transmittance table's size: 256 in μ by 64 in r (Hillaire 2020's code, Design note 16). */
export const TRANSMITTANCE_SIZE: TableSize = { widthTexels: 256, heightTexels: 64 };

/** The multiple-scattering table's size: 32 in the sun's μ by 32 in r (Hillaire 2020). */
export const MULTI_SCATTERING_SIZE: TableSize = { widthTexels: 32, heightTexels: 32 };

/**
 * Midpoint steps along each transmittance ray: 256.
 *
 * @remarks
 * The midpoint rule's error on e^(−t/H) is about (Δh/H)² ÷ 24 of a term's depth, Δh the height a
 * step climbs, and the aerosol's 1.2 km scale height is the finest structure. At 128 steps, a
 * shallow ray from the ground (μ = 0.11, 5 km steps climbing 0.56 km each) left the blue
 * transmittance 1.2% above the oracle on SwiftShader; 256 quarters the error. Hillaire's 40, sampled
 * at 0.3 of each step, misses the plan's 1% by more. The table is built only when the medium
 * changes, so the cost is paid rarely.
 *
 * Since R08.T6.d each ray is split where it crosses a term's layer edges ({@link packLayerEdges}),
 * and the steps are spread over the pieces in proportion to their length, at least
 * {@link TRANSMITTANCE_STEPS_A_PIECE} a piece (`decision-r08-t6b-floor.md` §9.2).
 */
export const TRANSMITTANCE_SAMPLES = 256;

/**
 * The fewest steps a piece of a transmittance ray between two layer edges takes: 4
 * (`decision-r08-t6b-floor.md` §9.2), or the ray's steps over its pieces where they are fewer than
 * four a piece. `transmittance.wgsl` holds the same number.
 */
export const TRANSMITTANCE_STEPS_A_PIECE = 4;

/**
 * Steps along each of the 64 multiple-scattering rays: 32 (sebh's reference takes 20), placed
 * toward each ray's lowest point and, before it, toward the texel's point (`marchSplit` with
 * `fromCamera`, R05.T12.e's rule) since R08.T6.d, where R05 took 32 even midpoints (plan R08's
 * Risks, "Deviations in T6.a, as built": even steps erred by up to 10.5% in the table, placed ones
 * by 2.6%). Since R08.T6.i each side is cut into pieces where the ray crosses a layer edge, as the
 * per-frame marches' are (`marchSteps.ts`'s `marchPieces`).
 */
export const MULTI_SCATTERING_SAMPLES = 32;

/**
 * The absorber slots the per-planet tables hold: 5, the transmittance table's alpha and the
 * absorber-column table's four channels (plan R08, Design note 8, R08.T6.c), `common.wgsl`'s
 * `ABSORBER_SLOTS`. It is `absorbers.ts`'s `ABSORBER_COLUMNS`, which `absorberSpecs` fills, held
 * equal by a test rather than imported, so that the render thread's bundle does not take that
 * module's tables.
 */
export const ABSORBER_SLOTS = 5;

/**
 * The nodes of a curve of growth as the kernels read it: 241, u ÷ U = 10^(−3 + j ÷ 40) at node j,
 * `absorbers.ts`'s `ABSORBER_CURVE_DECADES` and `ABSORBER_CURVE_NODES_PER_DECADE` (held equal by
 * a test) and `common.wgsl`'s `curveDepth`, which finds a column's node from its logarithm.
 */
export const ABSORBER_CURVE_NODES = 241;

/** The decade of a curve's first node about the absorber's vertical column U: 10⁻³ U. */
const CURVE_FIRST_DECADE = -3;

/** A curve's nodes per decade of column. */
const CURVE_NODES_PER_DECADE = 40;

/**
 * The deepest optical depth a curve holds: −ln 10⁻⁶, `absorbers.ts`'s
 * `ABSORBER_TRANSMITTANCE_FLOOR` as an optical depth (held equal by a test) and `common.wgsl`'s
 * `CURVE_DEPTH_MAX`.
 */
export const ABSORBER_CURVE_DEPTH_MAX = -Math.log(1e-6);

/**
 * Each light's curves of growth for a medium's absorbers (plan R08, Design note 5): light slot
 * k's curves, one an absorber slot, in the order of the medium's absorber terms
 * ({@link absorberSlots}), each `absorbers.ts`'s `absorberCurve(term.absorber.spec, spectrum)`
 * for that light's spectrum.
 *
 * @remarks
 * Slot 0 is the body's brightest star, whose curves a planetshine source takes (Design note 7).
 * Since R08.T7 each source of the per-frame kernels takes its own slot's, and the per-planet
 * multiple-scattering table has one layer a slot, each built with that slot's curves (plan R08's
 * Risks, "Which star's absorber curve the shared tables are built with").
 */
export type AbsorberCurveSlots = ReadonlyArray<ReadonlyArray<AbsorberCurve>>;

/** No light's curves: what a medium with no absorber term takes. */
export const NO_ABSORBER_CURVES: AbsorberCurveSlots = [];

/**
 * One of a medium's absorber terms in its slot of the absorber columns (Design note 8): a column
 * absorber's molecules, or a pair absorber's molecule pairs (R08.T4.c), whose term's density is
 * already the square of its column's.
 */
export interface AbsorberSlot {
  /** Its slot, 0 to {@link ABSORBER_SLOTS} − 1: its rank among the medium's absorber terms. */
  readonly slot: number;
  /** Its index among the medium's terms. */
  readonly termIndex: number;
  readonly term: MediumTerm;
  readonly absorber: TermAbsorber;
  /**
   * U, the spec's vertical amount its curves are spaced about, of the absorber's kind
   * (`absorbers.ts`'s `absorberVerticalAmount`).
   */
  readonly verticalAmount: AbsorberAmount;
  /**
   * The term's density at relative density 1 over U, m⁻¹: n ÷ U for a column absorber,
   * n_A n_B ÷ U for a pair absorber, so that the term's relative density integrated along a path,
   * times it, is the path's amount u over U, the quantity the tables store, whatever the kind.
   */
  readonly columnPerM: number;
}

/**
 * A term absorber's density at relative density 1, m⁻³ or molecule pairs m⁻⁶, its spec's vertical
 * amount, and that amount's value in its kind's unit, molecules m⁻² or molecule pairs m⁻⁵.
 */
function densityAndAmount(absorber: TermAbsorber): {
  readonly density: number;
  readonly amount: AbsorberAmount;
  readonly value: number;
} {
  let of: { readonly density: number; readonly amount: AbsorberAmount; readonly value: number };
  switch (absorber.kind) {
    case "column": {
      const value = absorber.spec.columnPerM2;
      of = {
        density: absorber.numberDensityPerM3,
        amount: { kind: "column", columnPerM2: value },
        value,
      };
      break;
    }
    case "pair": {
      const value = absorber.spec.pairColumnPerM5;
      of = {
        density: absorber.pairDensityPerM6,
        amount: { kind: "pair", pairColumnPerM5: value },
        value,
      };
      break;
    }
  }
  return of;
}

/**
 * A medium's absorber terms in their slots: each term that carries an `absorber`, column or pair,
 * in the order of the medium's terms.
 *
 * @throws RangeError for more than {@link ABSORBER_SLOTS} absorber terms, which the tables cannot
 *   hold (`absorbers.ts`'s `absorberSpecs` keeps at most that many profiles), or for one whose
 *   density or vertical amount is not finite and above 0.
 */
export function absorberSlots(medium: AtmosphereMedium): ReadonlyArray<AbsorberSlot> {
  const slots: AbsorberSlot[] = [];
  for (const [termIndex, term] of medium.terms.entries()) {
    const absorber = term.absorber;
    if (absorber === undefined) {
      continue;
    }
    const { density: n, amount, value: u } = densityAndAmount(absorber);
    if (!(Number.isFinite(n) && n > 0 && Number.isFinite(u) && u > 0)) {
      throw new RangeError(
        `${absorber.kind} absorber ${term.name} of medium ${medium.name} needs a density and a vertical amount above 0, not ${n} and ${u}`,
      );
    }
    slots.push({
      slot: slots.length,
      termIndex,
      term,
      absorber,
      verticalAmount: amount,
      columnPerM: n / u,
    });
  }
  if (slots.length > ABSORBER_SLOTS) {
    throw new RangeError(
      `atmosphere medium ${medium.name} has ${slots.length} absorber terms; at most ${ABSORBER_SLOTS} fit the tables`,
    );
  }
  return slots;
}

/**
 * An amount's value in its kind's unit, molecules m⁻² or molecule pairs m⁻⁵, as `absorbers.ts`
 * reads it, here so that the render thread's bundle takes nothing from `absorbers.ts` but types.
 */
function amountValue(amount: AbsorberAmount): number {
  let value: number;
  switch (amount.kind) {
    case "column":
      value = amount.columnPerM2;
      break;
    case "pair":
      value = amount.pairColumnPerM5;
      break;
  }
  return value;
}

/**
 * The medium's line slots (R08.T4.d; `decision-r08-t4d-kdist-chain.md` §15, ruling 2): a bitmask
 * of its absorber slots whose curves have pressure rows, bit k for slot k, the column absorbers one
 * of whose species has lines (`absorbers.ts`'s `absorberCurvePressures`). The kernels read it from
 * `Medium.absorbers.w` and the composite from `Draw.shell.z` (`common.wgsl`'s `pressureChannelOf`).
 *
 * @remarks
 * `absorbers.ts`'s `absorberSpecs` puts every species mixed through one layer in one profile, so
 * the well-mixed line species, the bulk gas's, are one slot, and its channel holds the bulk's
 * pressure-weighted column. Each other line slot, H₂O's among them, takes a channel of its own.
 */
export function lineSlotMask(medium: AtmosphereMedium): number {
  let mask = 0;
  for (const { slot, absorber } of absorberSlots(medium)) {
    if (absorber.kind === "column" && absorber.spec.species.some((s) => s.lines !== undefined)) {
      mask |= 1 << slot;
    }
  }
  return mask;
}

/**
 * Absorber slot `slot`'s pressure channel among the line slots `mask` ({@link lineSlotMask}): its
 * rank among them in slot order, or −1 for a slot without lines, which reads no pressure. Channels
 * 0 to 3 stand in `absorberPressureColumns`' rgba and channel 4 in `lastAbsorberPressureColumn`'s r.
 */
export function pressureChannelOf(mask: number, slot: number): number {
  if ((mask & (1 << slot)) === 0) {
    return -1;
  }
  let below = 0;
  for (let k = 0; k < slot; k += 1) {
    below += (mask >> k) & 1;
  }
  return below;
}

/** How many pressure channels the line slots `mask` take: one a line slot. */
export function pressureChannelCount(mask: number): number {
  let count = 0;
  for (let k = 0; k < ABSORBER_SLOTS; k += 1) {
    count += (mask >> k) & 1;
  }
  return count;
}

/** The pressure channels one table holds, in its rgba: four. */
export const PRESSURE_CHANNELS_A_TABLE = 4;

/**
 * The size of `lastAbsorberPressureColumn`, the fifth pressure channel's table: the
 * transmittance's for a medium of five line slots, one texel, never read, for any other
 * (`decision-r08-t4d-kdist-chain.md` §15, ruling 2: a second table only past four line slots).
 */
export function lastPressureColumnSize(medium: AtmosphereMedium): TableSize {
  return pressureChannelCount(lineSlotMask(medium)) > PRESSURE_CHANNELS_A_TABLE
    ? TRANSMITTANCE_SIZE
    : { widthTexels: 1, heightTexels: 1 };
}

/** The relative tolerance a curve's node columns are held to on the kernels' grid. */
const CURVE_GRID_TOLERANCE = 1e-9;

/**
 * Checks that `curves` give every light slot one curve an absorber slot of `medium`, each of its
 * absorber's kind and on the kernels' grid about its absorber's vertical amount.
 *
 * @throws RangeError for a medium with absorbers and no light slot, a slot of another number of
 *   curves, or a curve of the other kind (a pair curve on a column absorber or the reverse), whose
 *   nodes are not {@link ABSORBER_CURVE_NODES}, at u ÷ U = 10^(−3 + j ÷ 40) about its own
 *   absorber's U to 10⁻⁹ of ln u, or whose optical depths are not finite and in [0, −ln 10⁻⁶]
 *   (`absorbers.ts`'s floor); or, for a curve's pressure rows (R08.T4.d), rows whose ln p is
 *   not finite and ascending, rows or a read's top that differ from light slot 0's, several rows
 *   on a slot without lines or one row on a slot with them ({@link lineSlotMask}), or several
 *   rows on a medium with no pressure profile ({@link AtmosphereMedium.pressureBar}) to read
 *   them by.
 */
export function checkAbsorberCurves(medium: AtmosphereMedium, curves: AbsorberCurveSlots): void {
  const slots = absorberSlots(medium);
  if (slots.length > 0 && curves.length === 0) {
    throw new RangeError(
      `atmosphere medium ${medium.name} has ${slots.length} absorber terms and no light's curves of growth`,
    );
  }
  const deepest = ABSORBER_CURVE_DEPTH_MAX;
  const lines = lineSlotMask(medium);
  for (const [light, perAbsorber] of curves.entries()) {
    if (perAbsorber.length !== slots.length) {
      throw new RangeError(
        `light slot ${light} gives ${perAbsorber.length} curves for medium ${medium.name}'s ${slots.length} absorbers`,
      );
    }
    for (const [k, curve] of perAbsorber.entries()) {
      const slot = slots[k];
      if (curve.kind !== slot?.absorber.kind) {
        throw new RangeError(
          `light slot ${light}'s curve for absorber ${slot?.term.name ?? k} is a ${curve.kind} absorber's, not a ${slot?.absorber.kind ?? "missing"} absorber's`,
        );
      }
      const vertical = amountValue(slot.verticalAmount);
      const nodes = curve.logColumns;
      const onGrid =
        nodes.length === ABSORBER_CURVE_NODES &&
        nodes.every((x, j) => {
          const expected =
            Math.log(vertical) + (CURVE_FIRST_DECADE + j / CURVE_NODES_PER_DECADE) * Math.LN10;
          return Math.abs(x - expected) <= CURVE_GRID_TOLERANCE * Math.max(Math.abs(expected), 1);
        });
      const rows = curve.logPressures.length;
      const depthsHeld = curve.opticalDepth.every(
        (depths) =>
          depths.length === ABSORBER_CURVE_NODES * rows &&
          depths.every((d) => Number.isFinite(d) && d >= 0 && d <= deepest * (1 + 1e-12)),
      );
      if (!(onGrid && depthsHeld)) {
        throw new RangeError(
          `light slot ${light}'s curve for absorber ${slot.term.name} is not on the tables' grid of ${ABSORBER_CURVE_NODES} nodes about its vertical amount, or its optical depths leave [0, ${deepest}]`,
        );
      }
      const refusal = rowsRefusal(medium, curve, curves[0]?.[k], pressureChannelOf(lines, k) >= 0);
      if (refusal !== undefined) {
        throw new RangeError(
          `light slot ${light}'s curve for absorber ${slot.term.name} has pressure rows ${refusal}`,
        );
      }
    }
  }
}

/**
 * Why a curve's pressure rows cannot be packed (R08.T4.d), or undefined where they can: rows whose
 * ln p is not finite and ascending, rows or a read's top that differ from light slot 0's curve for
 * the same absorber, several rows on a slot without lines or one on a slot with them (which reads
 * a pressure channel, {@link lineSlotMask}), or several rows on a medium with no pressure profile
 * to read them by.
 */
function rowsRefusal(
  medium: AtmosphereMedium,
  curve: AbsorberCurve,
  firstLight: AbsorberCurve | undefined,
  lineSlot: boolean,
): string | undefined {
  const rows = curve.logPressures.length;
  const ascending =
    rows >= 1 &&
    Number.isFinite(curve.readTopLogPressure) &&
    curve.logPressures.every(
      (y, i) => Number.isFinite(y) && y > (curve.logPressures[i - 1] ?? -Infinity),
    );
  if (!ascending) {
    return "that are not finite and ascending";
  }
  const asFirst =
    firstLight !== undefined &&
    firstLight.logPressures.length === rows &&
    firstLight.logPressures.every((y, i) => y === curve.logPressures[i]) &&
    firstLight.readTopLogPressure === curve.readTopLogPressure;
  if (!asFirst) {
    return "or a read's top that differ from light slot 0's";
  }
  if (rows > 1 !== lineSlot) {
    return lineSlot ? "of one row on a slot with lines" : "on a slot without lines";
  }
  return rows > 1 && medium.pressureBar === undefined
    ? `and medium ${medium.name} has no pressure profile to read them by`
    : undefined;
}

/** A 2D table's contents: RGBA per texel, row by row from texel (0, 0). */
export interface PackedTable {
  readonly widthTexels: number;
  readonly heightTexels: number;
  readonly texels: Float32Array;
}

/**
 * The lights' curves of growth as `common.wgsl`'s `curveDepth` reads them, `rgba32float`, each a
 * block of its pressure rows (R08.T4.d), one row for an absorber without lines:
 *
 * - row 0, a header: texel k for absorber slot k, its first row within a light's block, its rows
 *   and its read's top ln(p ÷ 1 Pa) in rgb; texel {@link ABSORBER_SLOTS} the rows a light's block
 *   takes in r;
 * - light slot s's curve for absorber slot k at pressure row q in row 1 + s × block + first + q,
 *   texel j its optical depth −ln T_c per channel at node j in rgb and the row's ln(p ÷ 1 Pa) in
 *   alpha.
 *
 * Every light's curve for a slot has the same rows, its absorber's ({@link checkAbsorberCurves}).
 * A one-texel placeholder for a medium with no absorber, whose kernels read none.
 *
 * @throws RangeError as {@link checkAbsorberCurves}.
 */
export function packAbsorberCurves(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
): PackedTable {
  checkAbsorberCurves(medium, curves);
  const slots = absorberSlots(medium);
  if (slots.length === 0) {
    return { widthTexels: 1, heightTexels: 1, texels: new Float32Array(4) };
  }
  const width = ABSORBER_CURVE_NODES;
  const lead = curves[0] ?? [];
  const rowsOf = slots.map((_, k) => lead[k]?.logPressures.length ?? 1);
  const firsts = rowsOf.map((_, k) => rowsOf.slice(0, k).reduce((sum, n) => sum + n, 0));
  const block = rowsOf.reduce((sum, n) => sum + n, 0);
  const height = 1 + curves.length * block;
  const texels = new Float32Array(width * height * 4);
  for (const [k, rows] of rowsOf.entries()) {
    texels.set([firsts[k] ?? 0, rows, lead[k]?.readTopLogPressure ?? 0, 0], k * 4);
  }
  texels.set([block, 0, 0, 0], ABSORBER_SLOTS * 4);
  for (const [light, perAbsorber] of curves.entries()) {
    for (const [k, curve] of perAbsorber.entries()) {
      const [r, g, b] = curve.opticalDepth;
      for (const [q, lnP] of curve.logPressures.entries()) {
        const row = 1 + light * block + (firsts[k] ?? 0) + q;
        for (let j = 0; j < width; j += 1) {
          const at = q * width + j;
          texels.set(
            [r[at] ?? Number.NaN, g[at] ?? Number.NaN, b[at] ?? Number.NaN, lnP],
            (row * width + j) * 4,
          );
        }
      }
    }
  }
  return { widthTexels: width, heightTexels: height, texels };
}

/**
 * The transmittance kernel: `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `medium.wgsl`, then
 * `transmittance.wgsl`.
 */
export const TRANSMITTANCE_KERNEL: KernelPair = {
  name: "atmosphere transmittance",
  reference: `${preciseTrigWgsl}\n${commonWgsl}\n${mediumWgsl}\n${transmittanceWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/**
 * The multiple-scattering kernel: `view/shaders/preciseTrig.wgsl`, `common.wgsl`, `medium.wgsl`,
 * then `multiScattering.wgsl`.
 */
export const MULTI_SCATTERING_KERNEL: KernelPair = {
  name: "atmosphere multiple scattering",
  reference: `${preciseTrigWgsl}\n${commonWgsl}\n${mediumWgsl}\n${multiScatteringWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/** The format the smoke's kernel arm writes its kernels' outputs in (R08.T6.g). */
export const KERNEL_ARM_FORMAT = "rgba32float";

/**
 * A planet's tables' textures: the stored `rgba16float` ones, or the kernel arm's `rgba32float`
 * ones ({@link KERNEL_ARM_FORMAT}). The transmittance kernel writes the transmittance table and the
 * absorber-column table beside it (R08.T6.c), and the pressure-weighted column tables (R08.T4.d);
 * the multiple-scattering kernel one layer of its 2D array a light slot
 * ({@link multiScatteringLayersOf}, R08.T7).
 */
export interface PlanetTableTextures {
  readonly transmittance: TextureHandle;
  readonly absorberColumns: TextureHandle;
  /** The pressure-weighted columns of pressure channels 0 to 3, in rgba (R08.T4.d). */
  readonly absorberPressureColumns: TextureHandle;
  /** The pressure-weighted column of pressure channel 4, in r ({@link lastPressureColumnSize}). */
  readonly lastAbsorberPressureColumn: TextureHandle;
  readonly multiScattering: TextureHandle;
}

/**
 * The multiple-scattering table's layers for a medium and its lights' curves (plan R08, R08.T7;
 * `decision-r08-t6b-floor.md` §8): under an absorber, one a light slot, each built with that
 * light's curves through the kernel's `curveSlot`, so that each sun's source reads its own; with
 * none, one, since without an absorber the table is the same for every star (the smooth terms
 * stand at the fitted wavelengths and the star's colour multiplies after). With R08.T6.f's bands
 * they become band × slot. A layer is 8 KB.
 */
export function multiScatteringLayersOf(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
): number {
  return absorberCount(medium) > 0 ? Math.max(1, curves.length) : 1;
}

/** The two per-planet kernels, as built or as the kernel arm assembles them. */
interface PlanetTableKernels {
  readonly transmittance: ComputeHandle;
  readonly multiScattering: ComputeHandle;
}

/** A kernel's `rgba16float` storage-texture declaration, its format between the two groups. */
const STORED_DECLARATION = /(texture_storage_(?:2d_array|[23]d)<)rgba16float(, write>)/gu;

/**
 * `pair` assembled a second time with its storage textures' format `rgba32float` in place of
 * `rgba16float`, under its own name: the smoke's kernel arm (plan R08, R08.T6.g;
 * `decision-r08-t6b-floor.md` §4), which holds a kernel's own `f32` output to its twin with no
 * half-float floor. It reads what the shipped kernel reads.
 *
 * @throws Error when `pair` declares no `rgba16float` storage texture.
 */
export function withKernelArmFormat(pair: KernelPair): KernelPair {
  const rewrite = (wgsl: string): string =>
    wgsl.replaceAll(STORED_DECLARATION, `$1${KERNEL_ARM_FORMAT}$2`);
  const reference = rewrite(pair.reference);
  if (reference === pair.reference) {
    throw new Error(`kernel ${pair.name} declares no rgba16float storage texture`);
  }
  return {
    ...pair,
    name: `${pair.name} (${KERNEL_ARM_FORMAT})`,
    reference,
    subgroup: pair.subgroup === null ? null : rewrite(pair.subgroup),
  };
}

/** Floats a term takes in the `terms` buffer: four `vec4f` (`common.wgsl`'s `Term`). */
const TERM_FLOATS = 16;

/**
 * Floats in the `Medium` uniform: its shell, term count and steps, the ground's albedo, and its
 * absorber slots' count in a vec4f of its own (R08.T6.c).
 */
const MEDIUM_UNIFORM_FLOATS = 12;

/** The bytes of the kernels' `terms` buffer: {@link MAX_TERMS} terms of 64 B, 512 B. */
export const TERMS_BUFFER_BYTES = MAX_TERMS * TERM_FLOATS * 4;

/**
 * The levels of a tabulated density's table: 1,024, at h_k = top × (k ÷ 1,023)², k = 0–1,023,
 * from the ground to the medium's top (Design note 2's square-root spacing).
 *
 * @remarks
 * The spacing is 2√(h × top) ÷ 1,023, finest at the ground where layers are thin. Read linear in
 * h between levels, an e^(−h ÷ H) errs by at most (Δh ÷ H)² ÷ 8 between them: at a 100 km top,
 * 6 × 10⁻⁶ at h = H = 8.4 km (R*T ÷ (M g₀) at the US Standard Atmosphere 1976's sea level, 8,435
 * m) and 4 × 10⁻⁵ at h = H = 1.2 km (Bruneton and Neyret 2008, §2's aerosol scale height
 * H_M ≃ 1.2 km). `tables.test.ts` holds R08.T3.a's
 * column, resampled, to its own optical depth. One layer is 4 KB in `r32float`.
 */
export const DENSITY_TABLE_LEVELS = 1_024;

/**
 * The entries of a tabulated phase's table: 256, even in u = √(θ ÷ π) from 0 to 1 (Design note 6's
 * grid). One layer is 4 KB in `rgba32float`, the three channels in rgb.
 */
export const PHASE_TABLE_ENTRIES = 256;

/** `Term.profile.x`'s code for each density profile (`medium.wgsl`'s `densityOf`). */
function densityCodeOf(
  profile: DensityProfile,
  topHeightM: number,
): readonly [number, number, number, number] {
  let packed: readonly [number, number, number, number];
  switch (profile.kind) {
    case "exponential":
      packed = [0, profile.scaleHeightM, 0, 0];
      break;
    case "tent":
      packed = [1, profile.bottomM, profile.peakM, profile.topM];
      break;
    case "tabulated":
      packed = [2, topHeightM, 0, 0];
      break;
  }
  return packed;
}

/**
 * `Term.phase` for each phase function (`source.wgsl`'s `phaseOf`): its parameters in xyz and its
 * code in w, 0 none, 1 Rayleigh with its depolarisation ratio ρ per channel, 2 Cornette–Shanks
 * with its g in x, 3 tabulated with its forward lobe's share P per channel in xyz, the lobe's
 * higher orders' and the multiple-scattering table's truncation's (`forwardLobe.ts`, R08.T7).
 */
function phaseCodeOf(phase: PhaseFunction): readonly [number, number, number, number] {
  let packed: readonly [number, number, number, number];
  switch (phase.kind) {
    case "none":
      packed = [0, 0, 0, 0];
      break;
    case "rayleigh":
      packed = [...phase.depolarisation, 1];
      break;
    case "cornette-shanks":
      packed = [phase.asymmetry, 0, 0, 2];
      break;
    case "tabulated": {
      // A lobe that is not drawn takes no share: the kernels then add no orders and truncate none.
      const lobe = forwardLobeOfTable(resampledPhase(phase.table));
      const share = lobeIsDrawn(lobe) ? lobe.share : ([0, 0, 0] as const);
      packed = [share[0], share[1], share[2], 3];
      break;
    }
  }
  return packed;
}

/**
 * The medium's terms as the kernels' `terms` storage buffer reads them (`common.wgsl`'s `Term`):
 * per term its scattering, its absorption, its profile and its phase function, four `vec4f`, for
 * {@link MAX_TERMS} terms whatever the medium's count. A tabulated term's tables are its layer of
 * {@link packDensityTables} and {@link packPhaseTables}, its index here. A Rayleigh phase carries
 * its ρ per channel (R08.T3.c), which the kernels draw. An absorber term (R08.T6.c), whose rgb
 * coefficients are 0, carries its slot + 1 in absorption.w ({@link absorberSlots}) and its
 * n ÷ U, m⁻¹, in scattering.w; every other term 0 in both.
 *
 * @throws Error when the medium has more than {@link MAX_TERMS} terms; RangeError, as
 *   {@link checkTabulatedTops}, for a tabulated density held up to the top from a last level below
 *   it, and, as {@link absorberSlots}, for more absorber terms than {@link ABSORBER_SLOTS} or one
 *   with no column.
 */
export function packMedium(medium: AtmosphereMedium): Float32Array {
  if (medium.terms.length > MAX_TERMS) {
    throw new Error(
      `atmosphere medium ${medium.name} has ${medium.terms.length} terms; at most ${MAX_TERMS} fit`,
    );
  }
  checkTabulatedTops(medium);
  const slots = new Map(absorberSlots(medium).map((slot) => [slot.termIndex, slot]));
  const packed = new Float32Array(MAX_TERMS * TERM_FLOATS);
  for (const [i, term] of medium.terms.entries()) {
    const at = i * TERM_FLOATS;
    const slot = slots.get(i);
    packed.set([...term.scattering, slot?.columnPerM ?? 0], at);
    packed.set([...term.absorption, slot === undefined ? 0 : slot.slot + 1], at + 4);
    packed.set(densityCodeOf(term.density, medium.topHeightM), at + 8);
    packed.set(phaseCodeOf(term.phase), at + 12);
  }
  return packed;
}

/**
 * The medium's shell, term count and a kernel's steps as `common.wgsl`'s `Medium` uniform, with the
 * ground's albedo and the count of its absorber slots (R08.T6.c), and in `absorbers.z` 1 where the
 * density tables hold the pressure profile past the terms' layers and in `absorbers.w` the line
 * slots' bitmask, {@link lineSlotMask} (R08.T4.d). The shell is the ground's radius and the
 * atmosphere's height above it, H itself, so that no kernel forms H as the top's radius less the
 * ground's, which in `f32` would carry both radii's rounding (plan R08, Design note 18, R08.T6.g).
 *
 * @param bottomRadiusM - The ground's radius, m.
 * @param samples - The kernel's steps along each ray.
 * @param curveSlot - The light slot whose curves of growth a per-planet table is built with,
 *   `medium.wgsl`'s `curveSlot()`, and the multiple-scattering layer it writes: one dispatch a
 *   slot since R08.T7; {@link SHARED_TABLE_CURVE_SLOT} by default.
 */
export function packMediumUniform(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  samples: number,
  curveSlot = SHARED_TABLE_CURVE_SLOT,
): Float32Array {
  const packed = new Float32Array(MEDIUM_UNIFORM_FLOATS);
  packed.set([bottomRadiusM, medium.topHeightM, medium.terms.length, samples], 0);
  packed.set([...medium.groundAlbedo, 0], 4);
  packed.set(
    [
      absorberCount(medium),
      curveSlot,
      medium.pressureBar === undefined ? 0 : 1,
      lineSlotMask(medium),
    ],
    8,
  );
  return packed;
}

/** A term's layer edges in {@link packLayerEdges}: at most four, `medium.ts`'s `layerEdgesM`. */
export const EDGES_PER_TERM = 4;

/** What stands in an unused edge's place: a height the kernel never crosses. */
const NO_EDGE_M = -1;

/** Where the per-frame marches' edges stand in {@link packLayerEdges}: after the terms' edges. */
const MARCH_EDGES_AT = MAX_TERMS * EDGES_PER_TERM;

/** Where their count stands in {@link packLayerEdges}: the `vec4f` after them, its x. */
const MARCH_EDGE_COUNT_AT = MARCH_EDGES_AT + MARCH_EDGES_MAX;

/** The floats of {@link packLayerEdges}: `common.wgsl`'s `LayerEdges`, 52. */
export const LAYER_EDGES_FLOATS = MARCH_EDGE_COUNT_AT + 4;

/**
 * The medium's layer edges as the kernels' `layerEdges` uniform reads them (`common.wgsl`'s
 * `LayerEdges`; plan R08, R08.T6.d and R08.T6.i; `decision-r08-t6b-floor.md` §9.2 and
 * `decision-r08-t6d-findings.md` §1), of its density tables as the kernels read them
 * ({@link kernelMedium}):
 * - for the transmittance kernel, term i's in `vec4f` i, the heights where its density table leaves
 *   0 and where each ramp ends, and −1 in an unused place, for {@link MAX_TERMS} terms;
 * - for the per-frame marches, each ramp as one edge at its middle height (`marchSteps.ts`'s
 *   `marchEdgesM`), the terms' in order in the next {@link MARCH_EDGES_MAX} places, −1 past them,
 *   formed in `f64` and narrowed once, and their count in the `vec4f` after them, (n, 0, 0, 0).
 *
 * @throws Error when the medium has more than {@link MAX_TERMS} terms, as {@link packMedium}.
 */
export function packLayerEdges(medium: AtmosphereMedium): Float32Array {
  if (medium.terms.length > MAX_TERMS) {
    throw new Error(
      `atmosphere medium ${medium.name} has ${medium.terms.length} terms; at most ${MAX_TERMS} fit`,
    );
  }
  const packed = new Float32Array(LAYER_EDGES_FLOATS).fill(NO_EDGE_M);
  const middles: number[] = [];
  for (const [i, term] of kernelMedium(medium).terms.entries()) {
    const edges = layerEdgesM(term.density);
    packed.set(edges.slice(0, EDGES_PER_TERM), i * EDGES_PER_TERM);
    middles.push(...marchEdgesM(edges.slice(0, EDGES_PER_TERM), medium.topHeightM));
  }
  packed.set(middles, MARCH_EDGES_AT);
  packed.set([middles.length, 0, 0, 0], MARCH_EDGE_COUNT_AT);
  return packed;
}

/**
 * The light slot of the brightest star, 0: the slot planetshine takes (Design note 7), and the
 * default of the multiple-scattering table's `curveSlot` (ruled 2026-10-10,
 * `decision-r08-t6b-floor.md` §8). Since R08.T7 the table has one layer a light slot, each built
 * through the kernel's and the twin's `curveSlot` ({@link multiScatteringLayersOf}).
 */
export const SHARED_TABLE_CURVE_SLOT = 0;

/** How many of a medium's terms are absorbers, each a slot of the absorber columns. */
export function absorberCount(medium: AtmosphereMedium): number {
  return medium.terms.filter((term) => term.absorber !== undefined).length;
}

/** The heights of a density table's {@link DENSITY_TABLE_LEVELS} levels under a top, m. */
export function densityTableLevelsM(topHeightM: number): Float64Array {
  const last = DENSITY_TABLE_LEVELS - 1;
  return Float64Array.from(
    { length: DENSITY_TABLE_LEVELS },
    (_, k) => topHeightM * (k / last) ** 2,
  );
}

/**
 * A tabulated density resampled onto its table's levels ({@link densityTableLevelsM}) under the
 * medium's top: its density at each level by the `tabulated` rule, as `medium.wgsl` reads the
 * table, linear in h between levels and constant beyond the last; with its layer's edges, where it
 * has any (`medium.ts`'s `layerEdgesM`), widened outward to the table's levels about them.
 *
 * @remarks
 * A ramp of up to one column interval, about 115 m on a 250 K column, spreads over one to three of
 * the new levels about it, 62–124 m apart between 10 and 40 km under a 100 km top. Each ramp's two edges are taken to the level at or below its
 * lower edge and the level at or above its upper one (plan R08, R08.T6.d), so that the whole
 * resampled ramp lies in the transmittance ray's piece between them and every other piece is
 * smooth. The levels alone could not give them: the first level above 0 may stand inside the old
 * ramp.
 */
export function resampledDensity(profile: TabulatedDensity, topHeightM: number): TabulatedDensity {
  const levels = densityTableLevelsM(topHeightM);
  const resampled = tabulatedDensity(
    levels,
    levels.map((h) => densityAt(profile, h)),
  );
  const edges = layerEdgesM(profile);
  if (edges.length === 0) {
    // A profile that says it has none keeps saying so, whatever its new levels would show.
    return profile.edgesM === undefined ? resampled : { ...resampled, edgesM: [] };
  }
  const last = levels.length - 1;
  const below = (h: number): number => {
    let k = last;
    while (k > 0 && (levels[k] ?? Number.NaN) > h) {
      k -= 1;
    }
    return levels[k] ?? Number.NaN;
  };
  const above = (h: number): number => {
    let k = 0;
    while (k < last && (levels[k] ?? Number.NaN) < h) {
      k += 1;
    }
    return levels[k] ?? Number.NaN;
  };
  // The edges come in pairs, each a ramp's lower and upper edge (`layerEdgesM`).
  return { ...resampled, edgesM: edges.map((h, i) => (i % 2 === 0 ? below(h) : above(h))) };
}

/**
 * Simpson panels a phase table's interval is integrated over, per 1 ÷ 255 of u it spans, at least
 * one such share: 8 an interval of the kernels' grid, and more for a coarser source grid's.
 *
 * @remarks
 * Composite Simpson's error is (b − a) h⁴ max|f⁗| ÷ 180; with h ≤ 1 ÷ 2,040 it is under 2 × 10⁻⁹
 * for a Henyey–Greenstein table up to g = 0.99, where a fixed 8 panels an interval left 1.6 × 10⁻⁴
 * on a source grid of 5° in θ (R08.T6.b's science check).
 */
const PHASE_SIMPSON_PANELS_PER_ENTRY = 8;

/** The integrand of {@link phaseIntegral} at u = x, where the phase is p. */
function phaseIntegrand(x: number, p: number): number {
  return p * x * Math.sin(Math.PI * x * x);
}

/**
 * A phase channel's integral over the sphere, linear in u between its entries:
 * ∫ p dΩ = 4π² ∫₀¹ p(u) u sin(πu²) du, since θ = πu² and dθ = 2πu du, by Simpson's rule.
 */
function phaseIntegral(u: Float64Array, values: Float64Array): number {
  let sum = 0;
  for (let i = 0; i + 1 < u.length; i += 1) {
    const u0 = u[i] ?? Number.NaN;
    const u1 = u[i + 1] ?? Number.NaN;
    const p0 = values[i] ?? Number.NaN;
    const p1 = values[i + 1] ?? Number.NaN;
    const panels =
      PHASE_SIMPSON_PANELS_PER_ENTRY *
      Math.max(1, Math.ceil((u1 - u0) * (PHASE_TABLE_ENTRIES - 1)));
    const h = (u1 - u0) / panels;
    let interval = 0;
    for (let k = 0; k <= panels; k += 1) {
      const f = k / panels;
      let weight = k % 2 === 1 ? 4 : 2;
      if (k === 0 || k === panels) {
        weight = 1;
      }
      interval += weight * phaseIntegrand(u0 + f * (u1 - u0), p0 + f * (p1 - p0));
    }
    sum += (interval * h) / 3;
  }
  return 4 * Math.PI * Math.PI * sum;
}

/** A channel linear in u between `u`'s entries, at each of `at`'s ascending points. */
function resampledChannel(u: Float64Array, values: Float64Array, at: Float64Array): Float64Array {
  const out = new Float64Array(at.length);
  let i = 0;
  for (const [k, x] of at.entries()) {
    while (i + 2 < u.length && (u[i + 1] ?? Number.NaN) <= x) {
      i += 1;
    }
    const u0 = u[i] ?? Number.NaN;
    const f = Math.min(Math.max((x - u0) / ((u[i + 1] ?? Number.NaN) - u0), 0), 1);
    const v0 = values[i] ?? Number.NaN;
    out[k] = v0 + ((values[i + 1] ?? Number.NaN) - v0) * f;
  }
  return out;
}

/** Whether a table's entries are already the kernels' grid: {@link PHASE_TABLE_ENTRIES}, even in u. */
function onTableGrid(u: Float64Array): boolean {
  const last = PHASE_TABLE_ENTRIES - 1;
  return u.length === PHASE_TABLE_ENTRIES && u.every((x, i) => x === i / last);
}

/**
 * A phase table resampled onto the kernels' grid, {@link PHASE_TABLE_ENTRIES} entries even in u:
 * the table itself when it is on that grid already, otherwise each channel linear in u between its
 * entries, as `medium.ts`'s `phaseAt` reads it, and scaled so that its integral over the sphere is
 * the source's, so that a normalised table stays normalised (R08.T6.a's hand-off).
 *
 * @remarks
 * Each matrix element is resampled alike and scaled by its channel's factor, so that it stays
 * normalised as a₁ is. The integrals are taken by Simpson's rule, linear in u between entries.
 */
export function resampledPhase(table: PhaseTable): PhaseTable {
  if (onTableGrid(table.u)) {
    return table;
  }
  const last = PHASE_TABLE_ENTRIES - 1;
  const grid = Float64Array.from({ length: PHASE_TABLE_ENTRIES }, (_, i) => i / last);
  const factors = table.values.map((channel) => {
    const source = phaseIntegral(table.u, channel);
    const resampled = phaseIntegral(grid, resampledChannel(table.u, channel, grid));
    return resampled > 0 ? source / resampled : 1;
  });
  const element = (channels: PhaseMatrixElement): PhaseMatrixElement => {
    const [r, g, b] = channels.map((channel, c) =>
      resampledChannel(table.u, channel, grid).map((v) => v * (factors[c] ?? Number.NaN)),
    );
    if (r === undefined || g === undefined || b === undefined) {
      throw new Error("a phase table has three channels");
    }
    return [r, g, b];
  };
  const matrix = table.matrix;
  return phaseTable(
    grid,
    element(table.values),
    matrix === undefined
      ? undefined
      : {
          a2: element(matrix.a2),
          a3: element(matrix.a3),
          a4: element(matrix.a4),
          b1: element(matrix.b1),
          b2: element(matrix.b2),
        },
    table.forwardPeak,
  );
}

/**
 * A term with its phase table's forward delta taken out (plan R08, Design note 6; R08.T7;
 * `decision-r08-aerosol-labels.md` item 3), or the term itself where its table holds none.
 *
 * @remarks
 * In each channel c with a delta of share f_c (`PhaseTable.forwardPeak`, laid on the table's first
 * entries by `aerosol.ts`'s `aerosolPhaseTable`, `forwardPeak.ts`'s shape), the delta's entries
 * are taken from a₁, a₂, a₃ and a₄, which carry the identity's matrix, every element of what remains
 * is divided by 1 − f_c, and the term's scattering is multiplied by 1 − f_c, so that its extinction
 * falls by f_c times its scattering and its table integrates to 1 again, with mean cosine
 * (g − f_c μ_δ) ÷ (1 − f_c), μ_δ the delta's own (`FORWARD_PEAK_COSINE`). That is exact for a
 * forward delta (Potter 1970, J. Atmos. Sci. 27, 943): the delta's light is the direct beam's, which
 * the shared transmittance then carries to the star's image, its glare, the light at every
 * scattering point and on the ground, and which no per-view table could sample.
 *
 * @throws RangeError for a delta on a table that is not on the kernels' grid, where its entries
 *   do not stand.
 */
export function withoutForwardDelta(term: MediumTerm): MediumTerm {
  if (term.phase.kind !== "tabulated") {
    return term;
  }
  const table = term.phase.table;
  const shares = table.forwardPeak;
  if (shares.every((share) => share === 0)) {
    return term;
  }
  if (!onTableGrid(table.u)) {
    throw new RangeError(
      `term ${term.name}'s forward delta stands on a table of ${table.u.length} entries, not the kernels' ${PHASE_TABLE_ENTRIES}`,
    );
  }
  // a₁ is a phase, never negative, and its rounding at the delta's entries is held at 0; the other
  // elements take their signs.
  const remaining = (
    channels: PhaseMatrixElement,
    delta: boolean,
    phase: boolean,
  ): PhaseMatrixElement => {
    const [r, g, b] = channels.map((channel, c) => {
      const share = shares[c] ?? 0;
      const keep = 1 - share;
      return channel.map((v, k) => {
        const left = delta ? v - forwardPeakEntry(share, k) : v;
        return (phase ? Math.max(0, left) : left) / keep;
      });
    });
    if (r === undefined || g === undefined || b === undefined) {
      throw new Error("a phase table has three channels");
    }
    return [r, g, b];
  };
  const matrix = table.matrix;
  const kept = phaseTable(
    table.u,
    remaining(table.values, true, true),
    matrix === undefined
      ? undefined
      : {
          a2: remaining(matrix.a2, true, false),
          a3: remaining(matrix.a3, true, false),
          a4: remaining(matrix.a4, true, false),
          b1: remaining(matrix.b1, false, false),
          b2: remaining(matrix.b2, false, false),
        },
  );
  const [r, g, b] = term.scattering;
  return {
    ...term,
    scattering: [r * (1 - shares[0]), g * (1 - shares[1]), b * (1 - shares[2])],
    phase: { kind: "tabulated", table: kept },
  };
}

/**
 * The medium as the kernels read it: each term's forward delta taken out
 * ({@link withoutForwardDelta}, R08.T7), each tabulated density resampled onto its table's levels
 * ({@link resampledDensity}) and each tabulated phase onto the kernels' grid
 * ({@link resampledPhase}); every other term as it is.
 *
 * @remarks
 * The CPU twin of this medium is the kernels' oracle (R08.T6.b's smoke); how far it lies from the
 * twin of `medium` itself is the resampling's error, which `tables.test.ts` bounds.
 *
 * @throws RangeError as {@link withoutForwardDelta}.
 */
export function kernelMedium(medium: AtmosphereMedium): AtmosphereMedium {
  const pressure = medium.pressureBar;
  return {
    ...medium,
    ...(pressure === undefined
      ? {}
      : { pressureBar: resampledDensity(pressure, medium.topHeightM) }),
    terms: medium.terms.map((given) => {
      const term = withoutForwardDelta(given);
      return {
        ...term,
        density:
          term.density.kind === "tabulated"
            ? resampledDensity(term.density, medium.topHeightM)
            : term.density,
        phase:
          term.phase.kind === "tabulated"
            ? { kind: "tabulated", table: resampledPhase(term.phase.table) }
            : term.phase,
      };
    }),
  };
}

/**
 * A 2D array texture's contents, one row a layer: `widthTexels` texels a layer, layer after layer,
 * each texel's channels in turn.
 */
export interface LayeredTable {
  readonly widthTexels: number;
  readonly layers: number;
  readonly texels: Float32Array;
}

/** The one-layer, one-texel placeholder a medium with no table of a kind binds, of `channels`. */
function placeholder(channels: number): LayeredTable {
  return { widthTexels: 1, layers: 1, texels: new Float32Array(channels) };
}

/**
 * The medium's density tables as `medium.wgsl` reads them, `r32float`: one layer a term, term i's
 * at layer i, its {@link resampledDensity} on {@link DENSITY_TABLE_LEVELS} levels where its
 * density is tabulated and zeros where it is not, and, for a medium with a pressure profile
 * (R08.T4.d), the profile resampled likewise at the layer past the terms', which `medium.wgsl`'s
 * `pressureBarAt` reads; the placeholder where no term's is tabulated and there is none.
 */
export function packDensityTables(medium: AtmosphereMedium): LayeredTable {
  const terms = medium.terms;
  const pressure = medium.pressureBar;
  if (!terms.some((term) => term.density.kind === "tabulated") && pressure === undefined) {
    return placeholder(1);
  }
  const layers = terms.length + (pressure === undefined ? 0 : 1);
  const texels = new Float32Array(DENSITY_TABLE_LEVELS * layers);
  for (const [i, term] of terms.entries()) {
    if (term.density.kind === "tabulated") {
      texels.set(
        resampledDensity(term.density, medium.topHeightM).relative,
        i * DENSITY_TABLE_LEVELS,
      );
    }
  }
  if (pressure !== undefined) {
    texels.set(
      resampledDensity(pressure, medium.topHeightM).relative,
      terms.length * DENSITY_TABLE_LEVELS,
    );
  }
  return { widthTexels: DENSITY_TABLE_LEVELS, layers, texels };
}

/**
 * The medium's phase tables as `source.wgsl` reads them, `rgba32float`, the channels in rgb and 0
 * in alpha: one layer a term, term i's at layer i, its {@link resampledPhase} on
 * {@link PHASE_TABLE_ENTRIES} entries where its phase is tabulated and zeros where it is not; the
 * placeholder where no term's is.
 */
export function packPhaseTables(medium: AtmosphereMedium): LayeredTable {
  const terms = medium.terms;
  if (!terms.some((term) => term.phase.kind === "tabulated")) {
    return placeholder(4);
  }
  const texels = new Float32Array(PHASE_TABLE_ENTRIES * 4 * terms.length);
  for (const [i, term] of terms.entries()) {
    if (term.phase.kind !== "tabulated") {
      continue;
    }
    const [r, g, b] = resampledPhase(term.phase.table).values;
    for (let k = 0; k < PHASE_TABLE_ENTRIES; k += 1) {
      texels.set(
        [r[k] ?? Number.NaN, g[k] ?? Number.NaN, b[k] ?? Number.NaN, 0],
        (i * PHASE_TABLE_ENTRIES + k) * 4,
      );
    }
  }
  return { widthTexels: PHASE_TABLE_ENTRIES, layers: terms.length, texels };
}

/**
 * A 2D array texture's contents of 2D layers: `widthTexels` × `heightTexels` texels a layer, layer
 * after layer, row by row, each texel's four channels in turn.
 */
export interface LayeredPlanes {
  readonly widthTexels: number;
  readonly heightTexels: number;
  readonly layers: number;
  readonly texels: Float32Array;
}

/**
 * The medium's forward-lobe tables as `source.wgsl`'s `lobeOrdersOf` reads them, `rgba32float`
 * (plan R08, R08.T7; `forwardLobe.ts`): one layer a term, term i's at layer i, Ẽ(x, θ) of its
 * tabulated phase per channel in rgb, the {@link FORWARD_ORDERS_ENTRIES} entries of the phase
 * table's u across and the {@link FORWARD_LOBE_X_NODES} nodes of x down, zeros for a term that is
 * not tabulated or whose forward part is no lobe (`forwardLobe.ts`'s `lobeIsDrawn`); a one-texel
 * placeholder where no term is tabulated.
 */
export function packLobeTables(medium: AtmosphereMedium): LayeredPlanes {
  const terms = medium.terms;
  if (!terms.some((term) => term.phase.kind === "tabulated")) {
    return { widthTexels: 1, heightTexels: 1, layers: 1, texels: new Float32Array(4) };
  }
  const w = FORWARD_ORDERS_ENTRIES;
  const h = FORWARD_LOBE_X_NODES;
  const texels = new Float32Array(w * h * terms.length * 4);
  for (const [i, term] of terms.entries()) {
    if (term.phase.kind !== "tabulated") {
      continue;
    }
    const lobe = forwardLobeOfTable(resampledPhase(term.phase.table));
    if (!lobeIsDrawn(lobe)) {
      continue;
    }
    for (let j = 0; j < h; j += 1) {
      const [r, g, b] = lobe.table[j] ?? [];
      for (let k = 0; k < w; k += 1) {
        texels.set([r?.[k] ?? 0, g?.[k] ?? 0, b?.[k] ?? 0, 0], ((i * h + j) * w + k) * 4);
      }
    }
  }
  return { widthTexels: w, heightTexels: h, layers: terms.length, texels };
}

/** Workgroups covering a table with 8 × 8 groups: the transmittance kernel's. */
function groupsOf(size: TableSize): readonly [number, number, number] {
  return [Math.ceil(size.widthTexels / 8), Math.ceil(size.heightTexels / 8), 1];
}

/** What fixes the tables' contents: the medium, the ground's radius and the lights' curves. */
function keyOf(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  curves: AbsorberCurveSlots,
): string {
  return JSON.stringify({ medium, bottomRadiusM, curves });
}

/** A 2D array texture the tables made, with the shape it was made at. */
interface LayeredTexture {
  readonly texture: TextureHandle;
  readonly widthTexels: number;
  readonly layers: number;
}

/** A 2D texture the tables made and upload, with the shape it was made at. */
interface UploadedTexture {
  readonly texture: TextureHandle;
  readonly widthTexels: number;
  readonly heightTexels: number;
}

/**
 * The GPU objects of one device: the transmittance, absorber-column and multiple-scattering
 * tables, the medium's buffer and tables, the lights' curves, the kernels.
 */
interface TableResources {
  readonly transmittance: TextureHandle;
  readonly absorberColumns: TextureHandle;
  readonly absorberPressureColumns: TextureHandle;
  /** The fifth pressure channel's table, resized as the medium's line slots need it. */
  lastAbsorberPressureColumn: TextureHandle;
  lastPressureColumnSize: TableSize;
  /** One layer a light slot under an absorber ({@link multiScatteringLayersOf}). */
  multiScattering: LayeredTexture;
  readonly terms: BufferHandle;
  densityTables: LayeredTexture;
  phaseTables: LayeredTexture;
  /** The terms' forward-lobe tables ({@link packLobeTables}), with their shape. */
  lobeTables: PlanesTexture;
  absorberCurves: UploadedTexture;
  readonly transmittanceKernel: ComputeHandle;
  readonly multiScatteringKernel: ComputeHandle;
}

/** The density tables' texture format: one float a level. */
const DENSITY_FORMAT: GPUTextureFormat = "r32float";
/** The phase tables' texture format: the three channels and an unused alpha. */
const PHASE_FORMAT: GPUTextureFormat = "rgba32float";
/** The forward-lobe tables' texture format: the three channels and an unused alpha (R08.T7). */
const LOBE_FORMAT: GPUTextureFormat = "rgba32float";

/** A 2D array texture of 2D layers the tables made and upload, with the shape it was made at. */
interface PlanesTexture {
  readonly texture: TextureHandle;
  readonly widthTexels: number;
  readonly heightTexels: number;
  readonly layers: number;
}

/** Makes a texture for {@link packLobeTables}'s contents, uploaded. */
function createPlanes(
  engine: RenderEngine,
  name: string,
  table: Pick<LayeredPlanes, "widthTexels" | "heightTexels" | "layers">,
): PlanesTexture {
  const texture = engine.createTexture({
    name,
    size: [table.widthTexels, table.heightTexels, table.layers],
    dimension: "2d",
    format: LOBE_FORMAT,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "atmosphere-tables",
  });
  return {
    texture,
    widthTexels: table.widthTexels,
    heightTexels: table.heightTexels,
    layers: table.layers,
  };
}
/**
 * The lights' curves' texture format: an optical depth a channel and an unused alpha, in `f32`,
 * since a curve's 10⁻³ U node in the linear regime holds a depth far under rgba16float's least
 * normal (R08.T6.c).
 */
const CURVES_FORMAT: GPUTextureFormat = "rgba32float";

/** Makes a 2D table's texture for {@link packAbsorberCurves}'s contents, uploaded. */
function createUploaded(
  engine: RenderEngine,
  name: string,
  format: GPUTextureFormat,
  table: Pick<PackedTable, "widthTexels" | "heightTexels">,
): UploadedTexture {
  const texture = engine.createTexture({
    name,
    size: [table.widthTexels, table.heightTexels],
    dimension: "2d",
    format,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "atmosphere-tables",
  });
  return { texture, widthTexels: table.widthTexels, heightTexels: table.heightTexels };
}

/** Makes a layered table's texture, uploaded with `TEXTURE_BINDING` and `COPY_DST`. */
function createLayered(
  engine: RenderEngine,
  name: string,
  format: GPUTextureFormat,
  table: Pick<LayeredTable, "widthTexels" | "layers">,
): LayeredTexture {
  const texture = engine.createTexture({
    name,
    size: [table.widthTexels, 1, table.layers],
    dimension: "2d",
    format,
    mips: 1,
    usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
    category: "atmosphere-tables",
  });
  return { texture, widthTexels: table.widthTexels, layers: table.layers };
}

/**
 * Makes the multiple-scattering table's 2D array texture of `layers` layers, one a light slot
 * ({@link multiScatteringLayersOf}), written by its kernel and read by the per-frame kernels.
 */
function createMultiScattering(
  engine: RenderEngine,
  name: string,
  layers: number,
  format: GPUTextureFormat,
): LayeredTexture {
  const texture = engine.createTexture({
    name,
    size: [MULTI_SCATTERING_SIZE.widthTexels, MULTI_SCATTERING_SIZE.heightTexels, layers],
    dimension: "2d",
    format,
    mips: 1,
    usage: TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC,
    category: "atmosphere-tables",
  });
  return { texture, widthTexels: MULTI_SCATTERING_SIZE.widthTexels, layers };
}

/** Releases the buffer and textures of `resources`; the kernels have no release. */
function releaseResources(engine: RenderEngine, resources: TableResources): void {
  engine.releaseBuffer(resources.terms);
  for (const texture of [
    resources.transmittance,
    resources.absorberColumns,
    resources.absorberPressureColumns,
    resources.lastAbsorberPressureColumn,
    resources.multiScattering.texture,
    resources.densityTables.texture,
    resources.phaseTables.texture,
    resources.lobeTables.texture,
    resources.absorberCurves.texture,
  ]) {
    engine.releaseTexture(texture);
  }
}

/** A stored table the kernels write and the per-frame kernels read, `rgba16float`. */
function storedTable(engine: RenderEngine, name: string, size: TableSize): TextureHandle {
  return engine.createTexture({
    name,
    size: [size.widthTexels, size.heightTexels],
    dimension: "2d",
    format: "rgba16float",
    mips: 1,
    usage: TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC,
    category: "atmosphere-tables",
  });
}

/** The name of the fifth pressure channel's table. */
const LAST_PRESSURE_COLUMN_NAME = "atmosphere last absorber pressure column";

function createResources(engine: RenderEngine): TableResources {
  const one = { widthTexels: 1, layers: 1 };
  const transmittanceTable = (name: string): TextureHandle =>
    storedTable(engine, name, TRANSMITTANCE_SIZE);
  const oneTexel = { widthTexels: 1, heightTexels: 1 };
  return {
    transmittance: transmittanceTable("atmosphere transmittance"),
    absorberColumns: transmittanceTable("atmosphere absorber columns"),
    absorberPressureColumns: transmittanceTable("atmosphere absorber pressure columns"),
    lastAbsorberPressureColumn: storedTable(engine, LAST_PRESSURE_COLUMN_NAME, oneTexel),
    lastPressureColumnSize: oneTexel,
    multiScattering: createMultiScattering(
      engine,
      "atmosphere multiple scattering",
      1,
      "rgba16float",
    ),
    terms: engine.createBuffer({
      name: "atmosphere terms",
      bytes: TERMS_BUFFER_BYTES,
      usage: BUFFER_USAGE.STORAGE | BUFFER_USAGE.COPY_DST,
      category: "atmosphere-tables",
    }),
    densityTables: createLayered(engine, "atmosphere density tables", DENSITY_FORMAT, one),
    phaseTables: createLayered(engine, "atmosphere phase tables", PHASE_FORMAT, one),
    lobeTables: createPlanes(engine, "atmosphere forward-lobe tables", {
      widthTexels: 1,
      heightTexels: 1,
      layers: 1,
    }),
    absorberCurves: createUploaded(engine, "atmosphere absorber curves", CURVES_FORMAT, {
      widthTexels: 1,
      heightTexels: 1,
    }),
    transmittanceKernel: engine.createCompute(TRANSMITTANCE_KERNEL),
    multiScatteringKernel: engine.createCompute(MULTI_SCATTERING_KERNEL),
  };
}

/**
 * A planet's transmittance and multiple-scattering tables, made once and rebuilt in place when
 * the medium changes, with the medium's terms and tables, which the per-frame kernels read too.
 *
 * @remarks
 * After a device loss the engine's restore makes every handle anew (`RenderEngine.onRestored`):
 * the tables then make their textures, buffer and kernels again and rebuild for the last medium,
 * so a caller reads {@link AtmosphereTables.transmittance} and the rest afresh each frame rather
 * than keeping the handles. A medium whose tables have another shape than the last's makes them
 * anew and releases the old.
 */
export class AtmosphereTables {
  readonly #engine: RenderEngine;
  readonly #offRestored: () => void;
  #resources: TableResources;
  #medium: AtmosphereMedium;
  #bottomRadiusM: number;
  #curves: AbsorberCurveSlots;
  /** The medium's layer edges, {@link packLayerEdges}'s, packed once a medium. */
  #layerEdges: Float32Array = new Float32Array(LAYER_EDGES_FLOATS);
  #key: string | null = null;
  #builds = 0;
  /** The kernel arm's kernels ({@link AtmosphereTables.buildKernelArm}), made at its first call. */
  #armKernels: PlanetTableKernels | null = null;

  /**
   * Makes the tables and builds them for `medium`.
   *
   * @param bottomRadiusM - The ground's radius on the tables' sphere, m.
   * @param curves - Each light's curves of growth for the medium's absorbers; none for a medium
   *   with no absorber.
   * @throws Error or RangeError as {@link packMedium} and {@link packAbsorberCurves}, and
   *   `EngineUnavailable` while the engine has no device (a loss), having released the buffer and
   *   textures it made before.
   */
  constructor(
    engine: RenderEngine,
    medium: AtmosphereMedium,
    bottomRadiusM: number,
    curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
  ) {
    this.#engine = engine;
    this.#medium = medium;
    this.#bottomRadiusM = bottomRadiusM;
    this.#curves = curves;
    this.#resources = createResources(engine);
    try {
      this.setMedium(medium, bottomRadiusM, curves);
    } catch (error: unknown) {
      releaseResources(engine, this.#resources);
      throw error;
    }
    this.#offRestored = engine.onRestored(() => {
      this.#resources = createResources(engine);
      this.#armKernels = null;
      this.#key = null;
      this.setMedium(this.#medium, this.#bottomRadiusM, this.#curves);
    });
  }

  /**
   * What light loses per (r, μ) to the top, `rgba16float`: the optical depth of every term but the
   * absorbers in rgb and absorber slot 0's column u ÷ U in alpha (R08.T6.c); a new handle after a
   * restore.
   */
  get transmittance(): TextureHandle {
    return this.#resources.transmittance;
  }

  /** Absorber slots 1 to 4's columns u ÷ U per (r, μ) to the top, rgba, `rgba16float`. */
  get absorberColumns(): TextureHandle {
    return this.#resources.absorberColumns;
  }

  /**
   * Pressure channels 0 to 3's pressure-weighted columns per (r, μ) to the top, rgba,
   * `rgba16float`: one channel a line slot ({@link pressureChannelOf}; R08.T4.d).
   */
  get absorberPressureColumns(): TextureHandle {
    return this.#resources.absorberPressureColumns;
  }

  /**
   * Pressure channel 4's pressure-weighted column per (r, μ) to the top, in r, `rgba16float`:
   * the transmittance's size for a medium of five line slots, one texel otherwise
   * ({@link lastPressureColumnSize}); a new handle after a medium or a restore that resizes it.
   */
  get lastAbsorberPressureColumn(): TextureHandle {
    return this.#resources.lastAbsorberPressureColumn;
  }

  /** The lights' curves of growth, {@link packAbsorberCurves}'s, `rgba32float`. */
  get absorberCurves(): TextureHandle {
    return this.#resources.absorberCurves.texture;
  }

  /** The lights' curves of growth the tables follow, with their medium. */
  get curves(): AbsorberCurveSlots {
    return this.#curves;
  }

  /**
   * Multiple scattering per (r, μ_sun), per unit illuminance, `rgba16float`: a 2D array of one layer
   * a light slot under an absorber, each built with that light's curves, and one layer otherwise
   * ({@link multiScatteringLayersOf}, R08.T7).
   */
  get multiScattering(): TextureHandle {
    return this.#resources.multiScattering.texture;
  }

  /** The multiple-scattering table's layers ({@link multiScatteringLayersOf}). */
  get multiScatteringLayers(): number {
    return this.#resources.multiScattering.layers;
  }

  /** The medium's terms, {@link packMedium}'s, for the kernels' `terms` binding. */
  get terms(): BufferHandle {
    return this.#resources.terms;
  }

  /** The medium's density tables, {@link packDensityTables}'s, for `densityTables`. */
  get densityTables(): TextureHandle {
    return this.#resources.densityTables.texture;
  }

  /** The medium's phase tables, {@link packPhaseTables}'s, for `phaseTables`. */
  get phaseTables(): TextureHandle {
    return this.#resources.phaseTables.texture;
  }

  /**
   * The medium's forward-lobe tables, {@link packLobeTables}'s, for the per-frame kernels'
   * `lobeTables` (R08.T7).
   */
  get lobeTables(): TextureHandle {
    return this.#resources.lobeTables.texture;
  }

  /**
   * The medium's layer edges, {@link packLayerEdges}'s, for the `layerEdges` uniform of the
   * transmittance and multiple-scattering kernels and of the per-frame sky view and ray march
   * (R08.T6.i). Read, never written.
   */
  get layerEdges(): Float32Array {
    return this.#layerEdges;
  }

  /**
   * The medium the tables follow: the last one {@link AtmosphereTables.setMedium} took, which a
   * restore builds and the per-frame kernels read beside them.
   */
  get medium(): AtmosphereMedium {
    return this.#medium;
  }

  /** How many times the tables have been built. */
  get builds(): number {
    return this.#builds;
  }

  /**
   * Rebuilds the tables if `medium`, `bottomRadiusM` or `curves` differs from what they were built
   * for.
   *
   * @param curves - Each light's curves of growth for the medium's absorbers; none for a medium
   *   with no absorber. The multiple-scattering table has one layer a light slot under an absorber,
   *   each built with that slot's curves ({@link multiScatteringLayersOf}, R08.T7).
   * @returns Whether they were rebuilt.
   * @throws Error or RangeError as {@link packMedium} and {@link packAbsorberCurves}, and the
   *   tables keep what they held and follow; or `EngineUnavailable` while the engine has no device
   *   (a loss), and the tables follow `medium` from then on, so that the engine's restore builds it.
   */
  setMedium(
    medium: AtmosphereMedium,
    bottomRadiusM: number,
    curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
  ): boolean {
    const key = keyOf(medium, bottomRadiusM, curves);
    if (key === this.#key) {
      return false;
    }
    // The kernels read the medium with its terms' forward deltas taken out, as their twin reads
    // `kernelMedium(medium)` (R08.T7).
    const direct = { ...medium, terms: medium.terms.map(withoutForwardDelta) };
    const terms = packMedium(direct);
    const densityTables = packDensityTables(direct);
    const phaseTables = packPhaseTables(direct);
    const lobeTables = packLobeTables(direct);
    const absorberCurves = packAbsorberCurves(medium, curves);
    const layerEdges = packLayerEdges(medium);
    this.#medium = medium;
    this.#bottomRadiusM = bottomRadiusM;
    this.#curves = curves;
    this.#layerEdges = layerEdges;
    this.#key = null;
    const engine = this.#engine;
    const r = this.#resources;
    engine.writeBuffer(r.terms, 0, terms);
    r.densityTables = this.#upload(r.densityTables, densityTables, DENSITY_FORMAT);
    r.phaseTables = this.#upload(r.phaseTables, phaseTables, PHASE_FORMAT);
    r.lobeTables = this.#uploadPlanes(r.lobeTables, lobeTables);
    r.absorberCurves = this.#uploadCurves(r.absorberCurves, absorberCurves);
    const lastSize = lastPressureColumnSize(medium);
    const held = r.lastPressureColumnSize;
    if (lastSize.widthTexels !== held.widthTexels || lastSize.heightTexels !== held.heightTexels) {
      // Made before the old one goes, so that a loss while making it leaves no released handle.
      const resized = storedTable(engine, LAST_PRESSURE_COLUMN_NAME, lastSize);
      engine.releaseTexture(r.lastAbsorberPressureColumn);
      r.lastAbsorberPressureColumn = resized;
      r.lastPressureColumnSize = lastSize;
    }
    const layers = multiScatteringLayersOf(medium, curves);
    if (r.multiScattering.layers !== layers) {
      const made = createMultiScattering(
        engine,
        r.multiScattering.texture.name,
        layers,
        "rgba16float",
      );
      engine.releaseTexture(r.multiScattering.texture);
      r.multiScattering = made;
    }
    this.#dispatchTables(
      { transmittance: r.transmittanceKernel, multiScattering: r.multiScatteringKernel },
      {
        transmittance: r.transmittance,
        absorberColumns: r.absorberColumns,
        absorberPressureColumns: r.absorberPressureColumns,
        lastAbsorberPressureColumn: r.lastAbsorberPressureColumn,
        multiScattering: r.multiScattering.texture,
      },
    );
    this.#key = key;
    this.#builds += 1;
    return true;
  }

  /**
   * The tables built once more by their kernels assembled to write `rgba32float`
   * ({@link withKernelArmFormat}), into five textures made for the call: the smoke's kernel arm
   * (plan R08, R08.T6.g; `decision-r08-t6b-floor.md` §4). The transmittance, absorber-column and
   * pressure-weighted column tables are built from the medium and the multiple scattering over the
   * stored `rgba16float` ones, as the stored table's twin takes them, so that each holds its
   * kernel's own `f32` output.
   *
   * @returns The five textures, which the caller releases.
   * @throws `EngineUnavailable` while the engine has no device (a loss).
   */
  buildKernelArm(): PlanetTableTextures {
    const engine = this.#engine;
    this.#armKernels ??= {
      transmittance: engine.createCompute(withKernelArmFormat(TRANSMITTANCE_KERNEL)),
      multiScattering: engine.createCompute(withKernelArmFormat(MULTI_SCATTERING_KERNEL)),
    };
    const usage = TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.COPY_SRC;
    const output = (name: string, size: TableSize): TextureHandle =>
      engine.createTexture({
        name,
        size: [size.widthTexels, size.heightTexels],
        dimension: "2d",
        format: KERNEL_ARM_FORMAT,
        mips: 1,
        usage,
        category: "atmosphere-tables",
      });
    const tables: PlanetTableTextures = {
      transmittance: output("atmosphere transmittance (kernel arm)", TRANSMITTANCE_SIZE),
      absorberColumns: output("atmosphere absorber columns (kernel arm)", TRANSMITTANCE_SIZE),
      absorberPressureColumns: output(
        "atmosphere absorber pressure columns (kernel arm)",
        TRANSMITTANCE_SIZE,
      ),
      lastAbsorberPressureColumn: output(
        "atmosphere last absorber pressure column (kernel arm)",
        lastPressureColumnSize(this.#medium),
      ),
      multiScattering: engine.createTexture({
        name: "atmosphere multiple scattering (kernel arm)",
        size: [
          MULTI_SCATTERING_SIZE.widthTexels,
          MULTI_SCATTERING_SIZE.heightTexels,
          this.#resources.multiScattering.layers,
        ],
        dimension: "2d",
        format: KERNEL_ARM_FORMAT,
        mips: 1,
        usage,
        category: "atmosphere-tables",
      }),
    };
    this.#dispatchTables(this.#armKernels, tables);
    return tables;
  }

  /**
   * Builds the tables for the medium and ground the tables follow: the transmittance kernel into
   * `outputs.transmittance`, `outputs.absorberColumns`, `outputs.absorberPressureColumns` and
   * `outputs.lastAbsorberPressureColumn`, then the multiple-scattering kernel, reading the stored
   * tables and the lights' curves, into `outputs.multiScattering`, once a layer, layer k with
   * light slot k's curves (R08.T7).
   */
  #dispatchTables(kernels: PlanetTableKernels, outputs: PlanetTableTextures): void {
    const engine = this.#engine;
    const r = this.#resources;
    const medium = this.#medium;
    const bottomRadiusM = this.#bottomRadiusM;
    const sampled = { densityTables: r.densityTables.texture };
    engine.dispatch(
      kernels.transmittance,
      {
        uniforms: {
          medium: packMediumUniform(medium, bottomRadiusM, TRANSMITTANCE_SAMPLES),
          layerEdges: this.#layerEdges,
        },
        buffers: { terms: r.terms },
        sampled,
        storage: {
          transmittanceOut: { texture: outputs.transmittance, level: 0 },
          absorberColumnsOut: { texture: outputs.absorberColumns, level: 0 },
          absorberPressureColumnsOut: { texture: outputs.absorberPressureColumns, level: 0 },
          lastAbsorberPressureColumnOut: { texture: outputs.lastAbsorberPressureColumn, level: 0 },
        },
      },
      groupsOf(TRANSMITTANCE_SIZE),
      "atmosphere tables",
    );
    for (let slot = 0; slot < r.multiScattering.layers; slot += 1) {
      engine.dispatch(
        kernels.multiScattering,
        {
          uniforms: {
            medium: packMediumUniform(medium, bottomRadiusM, MULTI_SCATTERING_SAMPLES, slot),
            layerEdges: this.#layerEdges,
          },
          buffers: { terms: r.terms },
          sampled: {
            ...sampled,
            transmittance: r.transmittance,
            absorberColumns: r.absorberColumns,
            absorberPressureColumns: r.absorberPressureColumns,
            lastAbsorberPressureColumn: r.lastAbsorberPressureColumn,
            absorberCurves: r.absorberCurves.texture,
          },
          storage: { multiScatteringOut: { texture: outputs.multiScattering, level: 0 } },
        },
        [MULTI_SCATTERING_SIZE.widthTexels, MULTI_SCATTERING_SIZE.heightTexels, 1],
        "atmosphere tables",
      );
    }
  }

  /**
   * Stops following the engine's restores. The engine owns the buffer and the textures it holds
   * then and frees them; the ones an earlier medium's shape left were released when it changed.
   */
  dispose(): void {
    this.#offRestored();
  }

  /**
   * Writes a layered table into `current`, or into a texture made at its shape, releasing
   * `current`, when its shape differs; a placeholder is left as it was made, zero.
   */
  #upload(current: LayeredTexture, table: LayeredTable, format: GPUTextureFormat): LayeredTexture {
    let target = current;
    if (current.widthTexels !== table.widthTexels || current.layers !== table.layers) {
      target = createLayered(this.#engine, current.texture.name, format, table);
      this.#engine.releaseTexture(current.texture);
    }
    if (table.widthTexels > 1) {
      this.#engine.writeTexture(
        target.texture,
        [0, 0, 0],
        [table.widthTexels, 1, table.layers],
        table.texels,
      );
    }
    return target;
  }

  /**
   * Writes the forward-lobe tables into `current`, or into a texture made at their shape,
   * releasing `current`, when their shape differs; a placeholder is left as it was made, zero.
   */
  #uploadPlanes(current: PlanesTexture, table: LayeredPlanes): PlanesTexture {
    let target = current;
    if (
      current.widthTexels !== table.widthTexels ||
      current.heightTexels !== table.heightTexels ||
      current.layers !== table.layers
    ) {
      target = createPlanes(this.#engine, current.texture.name, table);
      this.#engine.releaseTexture(current.texture);
    }
    if (table.widthTexels > 1) {
      this.#engine.writeTexture(
        target.texture,
        [0, 0, 0],
        [table.widthTexels, table.heightTexels, table.layers],
        table.texels,
      );
    }
    return target;
  }

  /**
   * Writes the lights' curves into `current`, or into a texture made at their shape, releasing
   * `current`, when their shape differs; a placeholder is left as it was made, zero.
   */
  #uploadCurves(current: UploadedTexture, table: PackedTable): UploadedTexture {
    let target = current;
    if (current.widthTexels !== table.widthTexels || current.heightTexels !== table.heightTexels) {
      target = createUploaded(this.#engine, current.texture.name, CURVES_FORMAT, table);
      this.#engine.releaseTexture(current.texture);
    }
    if (table.widthTexels > 1) {
      this.#engine.writeTexture(
        target.texture,
        [0, 0, 0],
        [table.widthTexels, table.heightTexels, 1],
        table.texels,
      );
    }
    return target;
  }
}
