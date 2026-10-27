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
import { bytesPerTexel } from "../engine/memory";
import type { BufferHandle, ComputeHandle, RenderEngine, TextureHandle } from "../engine/types";
import { MAX_BODY_LIGHTS } from "../lighting/hostLights";
import type { AbsorberAmount, AbsorberCurve } from "./absorbers";
import {
  FORWARD_LOBE_X_NODES,
  FORWARD_ORDERS_ENTRIES,
  forwardLobeOfTable,
  lobeIsDrawn,
} from "./forwardLobe";
import { forwardPeakEntry } from "./forwardPeak";
import type { SpheroidFigure } from "./hillaire";
import { MARCH_EDGES_MAX, marchEdgesM } from "./marchSteps";
import {
  type AtmosphereMedium,
  checkTabulatedTops,
  type DensityProfile,
  densityAt,
  layerEdgesM,
  localScaleHeightM,
  type MediumTerm,
  type PhaseFunction,
  type PhaseMatrixElement,
  type PhaseTable,
  phaseTable,
  type TabulatedDensity,
  tabulatedDensity,
  type TermAbsorber,
} from "./medium";
import {
  BAND_STEP,
  bandLatitudeRad,
  type BodyGravity,
  gravityRatio,
  type OblateSlicing,
  principalRadiiM,
  respacedSlicing,
  WIDE_BAND_STEP,
  WIDE_KAPPA_STEP,
} from "./oblate";
import preciseTrigWgsl from "../shaders/preciseTrig.wgsl?raw";
import commonWgsl from "./shaders/common.wgsl?raw";
import irradianceWgsl from "./shaders/irradiance.wgsl?raw";
import mediumWgsl from "./shaders/medium.wgsl?raw";
import multiScatteringWgsl from "./shaders/multiScattering.wgsl?raw";
import scatteringWgsl from "./shaders/scattering.wgsl?raw";
import slotDepthsWgsl from "./shaders/slotDepths.wgsl?raw";
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
 * The sky-irradiance table's size (plan R08, R08.T9.b; Design note 7): Bruneton 2017's irradiance
 * texture, 64 in the sun's μ by 16 in height (`IRRADIANCE_TEXTURE_WIDTH` and `_HEIGHT` in
 * precomputed_atmospheric_scattering's `constants.h`), the same on both settings. Its rows stand at
 * x_r = √(h ÷ H) where Bruneton's stand at h ÷ H (`irradiance.wgsl`): Earth's at 0, 0.44, 1.8, 4 km
 * and on, so that a haze's kilometre-scale falloff lies across rows rather than within the first.
 */
export const IRRADIANCE_SIZE: TableSize = { widthTexels: 64, heightTexels: 16 };

/**
 * The most κ slices a body's transmittance tables hold, one layer each (plan R08, Design note 17;
 * R08.T6.f): `oblate.wgsl`'s `MAX_SLICES`.
 *
 * @remarks
 * Plan 14 caps the flattening at 0.2, where Design note 17's ω_fig gives 9–10 slices; Saturn takes
 * 5, Jupiter 4, the ice giants 2, Earth 1.
 */
export const MAX_SLICES = 16;

/**
 * The most latitude bands a body's multiple-scattering table holds, one layer a band and light
 * slot (R08.T6.f): `oblate.wgsl`'s `MAX_BANDS`. Plan 14's cap gives 6–7; Saturn takes 4.
 */
export const MAX_BANDS = 16;

/**
 * The heights at which a body's table holds the medium's local scale height for the lit passes
 * (plan R08, R08.T9.b; {@link packOblateBody}): 64, h\*_i = H i ÷ 63 from the datum to the top,
 * `oblate.wgsl`'s `OBLATE_SCALE_HEIGHTS`.
 */
export const OBLATE_SCALE_HEIGHTS = 64;

/**
 * The texel of a body's table that opens what the lit passes read of the medium (R08.T9.b), past
 * its header, slices and bands: `oblate.wgsl`'s `OBLATE_SURFACE_TEXEL`.
 */
export const OBLATE_SURFACE_TEXEL = 4 + MAX_SLICES + MAX_BANDS;

/**
 * The texels of a body's table ({@link packOblateBody}): four of its header, then
 * {@link MAX_SLICES} slices and {@link MAX_BANDS} bands, `oblate.wgsl`'s layout (R08.T6.f); since
 * R08.T9.b one more of the medium's absorbers and the local scale height at
 * {@link OBLATE_SCALE_HEIGHTS} heights, four a texel.
 */
export const OBLATE_BODY_TEXELS = OBLATE_SURFACE_TEXEL + 1 + OBLATE_SCALE_HEIGHTS / 4;

/**
 * The body a planet's tables are sliced for (plan R08, Design note 17; R08.T6.f): its datum and
 * R08.T3.d's gravity, whose slicing gives the κ slices and latitude bands. `tablesCpu.ts`'s
 * `TwinBody` is this shape.
 */
export interface AtmosphereBody {
  /** The datum spheroid: R07's `BodyFigure`. */
  readonly figure: SpheroidFigure;
  /** R08.T3.d's `bodyGravity` of the body, its slicing included. */
  readonly gravity: BodyGravity;
}

/** One latitude band of a body's tables (Design note 17, _Latitude bands_; R08.T6.e's twinBandOf). */
export interface TablesBand {
  /** s_b. */
  readonly gravityRatio: number;
  /** The geodetic latitude whose s is s_b, rad (`oblate.ts`'s `bandLatitudeRad`). */
  readonly latitudeRad: number;
  /**
   * The ground of the band's sphere in its gravity-scaled frame, m: s_b √(MN) at its latitude, or
   * R_ref for a body of one band, built as R05's per-planet tables are.
   */
  readonly bottomRadiusM: number;
  /** The factor every coefficient takes in the band's frame: 1 ÷ s_b. */
  readonly coefficientFactor: number;
}

/**
 * How a planet's tables are sliced (plan R08, Design note 17; R08.T6.f): the ground of each κ
 * slice, κ_k R_ref, and each latitude band, as R08.T6.e's `oblatePlanetTwin` builds them.
 */
export interface TablesPlan {
  /** κ_k of the slices, ascending. */
  readonly kappa: ReadonlyArray<number>;
  /** κ_k R_ref, m. */
  readonly sliceRadiiM: ReadonlyArray<number>;
  readonly bands: ReadonlyArray<TablesBand>;
  /**
   * Whether the one band reads the one slice's table, a body of one slice at κ = 1 and one band:
   * Earth's, and every body with no spheroid, whose tables are R05's.
   */
  readonly shared: boolean;
}

/** One slice at κ = 1 and one band at s = 1: R05's per-planet tables on R_ref. */
const ONE_SLICE_AND_BAND: OblateSlicing = {
  kappa: Float64Array.of(1),
  bandGravityRatio: Float64Array.of(1),
};

/**
 * The slices and bands of a planet's tables on `referenceRadiusM`, R_ref, for `body`, or one of
 * each for none (R08.T6.f): each band at its own latitude's s and √(MN), its sphere s_b √(MN) there
 * (R_ref for a body of one band), every coefficient over s_b, as `tablesCpu.ts`'s `twinBandOf`.
 *
 * @throws RangeError for more slices than {@link MAX_SLICES} or bands than {@link MAX_BANDS}, which
 *   the tables cannot hold (Design note 11's widening is R08.T11's).
 */
export function tablesPlanOf(body: AtmosphereBody | null, referenceRadiusM: number): TablesPlan {
  const slicing = body?.gravity.slicing ?? ONE_SLICE_AND_BAND;
  const kappa = Array.from(slicing.kappa);
  if (kappa.length > MAX_SLICES || slicing.bandGravityRatio.length > MAX_BANDS) {
    throw new RangeError(
      `a body's tables hold at most ${MAX_SLICES} slices and ${MAX_BANDS} bands, not ${kappa.length} and ${slicing.bandGravityRatio.length}`,
    );
  }
  const oneBand = slicing.bandGravityRatio.length === 1;
  const bands = Array.from(slicing.bandGravityRatio, (s): TablesBand => {
    if (body === null || oneBand) {
      return {
        gravityRatio: s,
        latitudeRad: body === null ? 0 : bandLatitudeRad(body.gravity, s),
        bottomRadiusM: referenceRadiusM,
        coefficientFactor: 1 / s,
      };
    }
    const latitudeRad = bandLatitudeRad(body.gravity, s);
    const { meridionalM, primeVerticalM } = principalRadiiM(body.figure, latitudeRad);
    return {
      gravityRatio: s,
      latitudeRad,
      bottomRadiusM: s * Math.sqrt(meridionalM * primeVerticalM),
      coefficientFactor: 1 / s,
    };
  });
  return {
    kappa,
    sliceRadiiM: kappa.map((k) => k * referenceRadiusM),
    bands,
    shared: oneBand && kappa.length === 1 && kappa[0] === 1 && bands[0]?.gravityRatio === 1,
  };
}

/**
 * A body's table as `oblate.wgsl` reads it (`oblateBody`, `rgba32float`, {@link OBLATE_BODY_TEXELS}
 * texels of one row; R08.T6.f), formed in `f64` and narrowed once: texel 0 (a, c, R_ref, e²);
 * texel 1 the gravity ratio's coefficients (A, B, K, O), s(φ) = (A cos²φ + B sin²φ) ÷
 * √(cos²φ + K sin²φ) + O, here from `oblate.ts`'s `gravityRatio` at the equator and the pole, which
 * give A = s_e − O and B = (s_p − O) c ÷ a, O the added gravity over g_ref, and K = (c ÷ a)², and
 * A = B = 0, O = 1 for a body with no spheroid; texel 2 (slices, bands, layers a band, the medium's
 * top); texel 3 (k = a ÷ c, k², 1 ÷ s_e, 1 ÷ s_p); then each slice's (κ_k R_ref, √κ_k, ln κ_k, 0)
 * and each band's (s_b, ln s_b, 0, 0). Since R08.T9.b, for the lit passes, which declare no medium
 * (`surfaceLighting.wgsl`): texel {@link OBLATE_SURFACE_TEXEL} the medium's absorber slots and its
 * line slots' bitmask ({@link absorberCount}, {@link lineSlotMask}), then the medium's local scale
 * height (`medium.ts`'s `localScaleHeightM`) at {@link OBLATE_SCALE_HEIGHTS} heights h\* evenly
 * from the datum to the top, four a texel, which the fitted lookup of a body of several bands
 * places its second point by.
 *
 * @param figure - The datum: a body's, or a sphere of R_ref for tables with no body.
 * @param medium - The medium the tables are built for: its top, its absorbers and its column.
 * @param layersPerBand - The multiple-scattering layers a band takes: its light slots.
 */
export function packOblateBody(
  figure: SpheroidFigure,
  gravity: BodyGravity | null,
  plan: TablesPlan,
  referenceRadiusM: number,
  medium: AtmosphereMedium,
  layersPerBand: number,
): Float32Array {
  const topHeightM = medium.topHeightM;
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const texels = new Float32Array(OBLATE_BODY_TEXELS * 4);
  const ratio = c / a;
  let coefficients: readonly [number, number, number, number] = [0, 0, 1, 1];
  let inverseEquator = 1;
  let inversePole = 1;
  if (gravity !== null && gravity.spheroid !== null) {
    const offset = gravity.gravityOffsetMS2 / gravity.referenceGravityMS2;
    const atEquator = gravityRatio(gravity, 0);
    const atPole = gravityRatio(gravity, Math.PI / 2);
    coefficients = [atEquator - offset, (atPole - offset) * ratio, ratio * ratio, offset];
    inverseEquator = 1 / atEquator;
    inversePole = 1 / atPole;
  }
  texels.set([a, c, referenceRadiusM, ((a - c) * (a + c)) / (a * a)], 0);
  texels.set(coefficients, 4);
  texels.set([plan.kappa.length, plan.bands.length, layersPerBand, topHeightM], 8);
  texels.set([a / c, (a / c) ** 2, inverseEquator, inversePole], 12);
  for (const [k, kappa] of plan.kappa.entries()) {
    texels.set([kappa * referenceRadiusM, Math.sqrt(kappa), Math.log(kappa), 0], (4 + k) * 4);
  }
  for (const [b, band] of plan.bands.entries()) {
    texels.set([band.gravityRatio, Math.log(band.gravityRatio), 0, 0], (4 + MAX_SLICES + b) * 4);
  }
  texels.set([absorberCount(medium), lineSlotMask(medium), 0, 0], OBLATE_SURFACE_TEXEL * 4);
  texels.set(packedScaleHeightsM(medium), (OBLATE_SURFACE_TEXEL + 1) * 4);
  return texels;
}

/**
 * The medium's local scale height in h\* as a body's table holds it for the lit passes (plan R08,
 * R08.T9.b; {@link packOblateBody}): `medium.ts`'s `localScaleHeightM` at
 * {@link OBLATE_SCALE_HEIGHTS} heights evenly from the datum to the top, m, in `f32`, which
 * `oblate.wgsl`'s `oblateScaleHeightAt` interpolates linearly and the twin reads the same way.
 */
export function packedScaleHeightsM(medium: AtmosphereMedium): Float32Array {
  return Float32Array.from({ length: OBLATE_SCALE_HEIGHTS }, (_, i) =>
    localScaleHeightM(medium, (medium.topHeightM * i) / (OBLATE_SCALE_HEIGHTS - 1)),
  );
}

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
 * Steps along each of the sky-irradiance table's rays (plan R08, R08.T9.b): placed toward the
 * texel's point and cut at the layer edges, as the multiple-scattering table's are; 32, by the
 * convergence measurement in plan R08's Risks ("Deviations in T9.b, as built").
 */
export const IRRADIANCE_SAMPLES = 32;

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
    : ONE_TEXEL;
}

/** A table of one texel a layer: one the kernels never read. */
const ONE_TEXEL: TableSize = { widthTexels: 1, heightTexels: 1 };

/**
 * Which of the transmittance kernel's column tables a body's {@link AtmosphereTables} make at full
 * size (plan R08, R08.T11.e): those the medium's kernels read (`read`, the tables' own), or the
 * absorber-column and pressure-column tables whatever the medium reads (`every`), as they were made
 * before R08.T11.e, which the smoke draws beside them to show that no pixel moves.
 */
export type ColumnTables = "read" | "every";

/** The sizes of the transmittance kernel's three column tables, one layer a slice each. */
export interface ColumnTableSizes {
  /** Absorber slots 1 to 4's columns. */
  readonly absorberColumns: TableSize;
  /** Pressure channels 0 to 3's pressure-weighted columns. */
  readonly absorberPressureColumns: TableSize;
  /** Pressure channel 4's ({@link lastPressureColumnSize}). */
  readonly lastAbsorberPressureColumn: TableSize;
}

/**
 * The sizes of a medium's column tables (plan R08, R08.T11.e; `decision-r08-t11-frame.md` §5):
 * each at {@link TRANSMITTANCE_SIZE} where the kernels read it and one texel a layer where they
 * read none, as the fifth pressure channel's already is (R08.T6.f's candidate). The kernels read the
 * absorber-column table only with two absorbers or more and the pressure-column table only with one
 * or more (`sliceStoredAt` in `oblate.wgsl`, `transmittanceThroughLayer` in `common.wgsl`,
 * `slotDepths.wgsl`), and the transmittance kernel writes each only then, so a one-texel table moves
 * no pixel. Under `every` the first two keep {@link TRANSMITTANCE_SIZE} whatever the medium reads.
 */
export function columnTableSizes(
  medium: AtmosphereMedium,
  columnTables: ColumnTables = "read",
): ColumnTableSizes {
  const count = absorberCount(medium);
  const every = columnTables === "every";
  return {
    absorberColumns: every || count > 1 ? TRANSMITTANCE_SIZE : ONE_TEXEL,
    absorberPressureColumns: every || count > 0 ? TRANSMITTANCE_SIZE : ONE_TEXEL,
    lastAbsorberPressureColumn: lastPressureColumnSize(medium),
  };
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

/**
 * The sky-irradiance kernel (plan R08, R08.T9.b): `view/shaders/preciseTrig.wgsl`, `common.wgsl`,
 * `medium.wgsl`, `scattering.wgsl`, then `irradiance.wgsl`.
 */
export const IRRADIANCE_KERNEL: KernelPair = {
  name: "atmosphere irradiance",
  reference: `${preciseTrigWgsl}\n${commonWgsl}\n${mediumWgsl}\n${scatteringWgsl}\n${irradianceWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/** The format the smoke's kernel arm writes its kernels' outputs in (R08.T6.g). */
export const KERNEL_ARM_FORMAT = "rgba32float";

/**
 * A planet's tables' textures: the stored `rgba16float` ones, or the kernel arm's `rgba32float`
 * ones ({@link KERNEL_ARM_FORMAT}). The transmittance kernel writes the transmittance table and the
 * absorber-column table beside it (R08.T6.c), and the pressure-weighted column tables (R08.T4.d),
 * each a 2D array of one layer a κ slice (R08.T6.f); the multiple-scattering kernel one layer of its
 * 2D array a latitude band and light slot ({@link multiScatteringLayersOf}, R08.T7, R08.T6.f); and,
 * where the body takes them, the slot-depth kernel one layer a κ slice and light slot (R08.T8.e,
 * {@link AtmosphereTables.slotDepths}), `null` where it does not.
 */
export interface PlanetTableTextures extends TransmittanceTextures {
  readonly multiScattering: TextureHandle;
  readonly slotDepths: TextureHandle | null;
}

/**
 * The multiple-scattering table's layers for a medium and its lights' curves (plan R08, R08.T7;
 * `decision-r08-t6b-floor.md` §8): under an absorber, one a light slot, each built with that
 * light's curves through the kernel's `curveSlot`, so that each sun's source reads its own; with
 * none, one, since without an absorber the table is the same for every star (the smooth terms
 * stand at the fitted wavelengths and the star's colour multiplies after). These are a band's: the
 * table has one layer a band and light slot since R08.T6.f ({@link AtmosphereTables.multiScattering}).
 * A layer is 8 KB.
 */
export function multiScatteringLayersOf(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
): number {
  return absorberCount(medium) > 0 ? Math.max(1, curves.length) : 1;
}

/**
 * The slot-depth kernel (plan R08, R08.T8.e, lever 5; `decision-r08-t8-leans.md` §10 and §13):
 * `view/shaders/preciseTrig.wgsl`, `common.wgsl`, then `slotDepths.wgsl`, which writes each light
 * slot's optical depth to the top, its curves applied at each texel of the stored transmittance
 * tables ({@link AtmosphereTables.slotDepths}).
 */
export const SLOT_DEPTHS_KERNEL: KernelPair = {
  name: "atmosphere slot depths",
  reference: `${preciseTrigWgsl}\n${commonWgsl}\n${slotDepthsWgsl}`,
  subgroup: null,
  readback: "presentation-only",
};

/**
 * The per-planet row, B, decimal: 2 MB (10⁶ B), Design note 11's "under 2 MB a planet" as
 * `decision-r08-t8-leans.md` §13, point 2 reads it.
 */
export const PER_PLANET_ROW_BYTES = 2_000_000;

/**
 * Every per-planet allocation a body's {@link AtmosphereTables} makes, as built, at its allocated
 * size, B (plan R08, Design note 11; `decision-r08-t8-leans.md` §13, point 2): what the 2 MB row
 * counts. The per-view tables have a line of their own and the smoke's kernel arm is not drawn, so
 * neither counts; nor do a band's own transmittance tables, which a build makes for its multiple
 * scattering and releases after it.
 */
export interface PerPlanetRow {
  /**
   * The transmittance kernel's four tables in every κ slice: the optical depth at
   * {@link TRANSMITTANCE_SIZE}, and the three column tables at their sizes for the medium
   * ({@link columnTableSizes}; since R08.T11.e one texel a slice each where the kernels read none).
   */
  readonly transmittanceBytes: number;
  /** The lights' curves of growth ({@link packAbsorberCurves}), Design note 11's row's since R08.T4.d. */
  readonly curvesBytes: number;
  /** The multiple-scattering and sky-irradiance layers, a band and light slot each. */
  readonly bandLayerBytes: number;
  /** The body's table ({@link packOblateBody}) and the terms buffer ({@link TERMS_BUFFER_BYTES}). */
  readonly bodyAndTermsBytes: number;
  /** The density, phase and forward-lobe tables, or their one-texel placeholders. */
  readonly mediumTableBytes: number;
  /** The row before the slot layers: every part above. */
  readonly baseBytes: number;
  /**
   * What the slot layers ({@link AtmosphereTables.slotDepths}) take: 131 KB a κ slice and light slot
   * under an absorber, built or not; 0 for a medium with none.
   */
  readonly slotDepthBytes: number;
}

/** A texture's bytes at `format`: its texels times the format's (`engine/memory.ts`'s rule). */
function bytesOf(format: GPUTextureFormat, ...extent: readonly number[]): number {
  return extent.reduce((product, n) => product * n, bytesPerTexel(format));
}

/**
 * The per-planet row of a medium, its lights' curves and a plan, as {@link AtmosphereTables} builds
 * them (`decision-r08-t8-leans.md` §13, point 2): every allocation at its allocated size, B, from the
 * shapes the packers give, which `tables.test.ts` holds to the engine's own allocations.
 *
 * @remarks
 * Design note 11's estimates count the tables a medium reads. Until R08.T11.e the absorber-column
 * and pressure-column tables were made at full size in every slice even where the medium left them
 * unused, 393,224 B a slice with the depth's, so that a thin giant came to more
 * (`decision-r08-t8-leans.md` §13, text F; R08.T11.a's rows); since R08.T11.e each one a medium does
 * not read is one texel a slice ({@link columnTableSizes}), unless `columnTables` is `every`.
 */
export function perPlanetRowOf(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  plan: TablesPlan,
  columnTables: ColumnTables = "read",
): PerPlanetRow {
  const slices = plan.kappa.length;
  const { widthTexels: w, heightTexels: h } = TRANSMITTANCE_SIZE;
  const sizes = columnTableSizes(medium, columnTables);
  const tableBytes = (size: TableSize): number =>
    bytesOf("rgba16float", size.widthTexels, size.heightTexels);
  const transmittanceBytes =
    slices *
    (bytesOf("rgba16float", w, h) +
      tableBytes(sizes.absorberColumns) +
      tableBytes(sizes.absorberPressureColumns) +
      tableBytes(sizes.lastAbsorberPressureColumn));
  const slots = absorberSlots(medium);
  const lead = curves[0] ?? [];
  const block = slots.reduce((sum, _, k) => sum + (lead[k]?.logPressures.length ?? 1), 0);
  const curvesBytes =
    slots.length === 0
      ? bytesOf(CURVES_FORMAT, 1, 1)
      : bytesOf(CURVES_FORMAT, ABSORBER_CURVE_NODES, 1 + curves.length * block);
  const layersPerBand = multiScatteringLayersOf(medium, curves);
  const bandLayers = plan.bands.length * layersPerBand;
  const bandLayerBytes =
    bytesOf("rgba16float", MULTI_SCATTERING_SIZE.widthTexels, MULTI_SCATTERING_SIZE.heightTexels) *
      bandLayers +
    bytesOf(IRRADIANCE_FORMAT, IRRADIANCE_SIZE.widthTexels, IRRADIANCE_SIZE.heightTexels) *
      bandLayers;
  const bodyAndTermsBytes = bytesOf(OBLATE_BODY_FORMAT, OBLATE_BODY_TEXELS) + TERMS_BUFFER_BYTES;
  const terms = medium.terms.length;
  const tabulatedDensities =
    medium.terms.some((term) => term.density.kind === "tabulated") ||
    medium.pressureBar !== undefined;
  const tabulatedPhases = medium.terms.some((term) => term.phase.kind === "tabulated");
  const densityLayers = terms + (medium.pressureBar === undefined ? 0 : 1);
  const mediumTableBytes =
    (tabulatedDensities
      ? bytesOf(DENSITY_FORMAT, DENSITY_TABLE_LEVELS, densityLayers)
      : bytesOf(DENSITY_FORMAT, 1)) +
    (tabulatedPhases
      ? bytesOf(PHASE_FORMAT, PHASE_TABLE_ENTRIES, terms) +
        bytesOf(LOBE_FORMAT, FORWARD_ORDERS_ENTRIES, FORWARD_LOBE_X_NODES, terms)
      : bytesOf(PHASE_FORMAT, 1) + bytesOf(LOBE_FORMAT, 1));
  const baseBytes =
    transmittanceBytes + curvesBytes + bandLayerBytes + bodyAndTermsBytes + mediumTableBytes;
  const slotDepthBytes =
    slots.length === 0 ? 0 : slices * layersPerBand * bytesOf(SLOT_DEPTHS_FORMAT, w, h);
  return {
    transmittanceBytes,
    curvesBytes,
    bandLayerBytes,
    bodyAndTermsBytes,
    mediumTableBytes,
    baseBytes,
    slotDepthBytes,
  };
}

/**
 * Why a body's tables hold no slot layers, so that its ray march keeps the shared table's read with
 * the curves applied on read (`decision-r08-t8-leans.md` §10, amendments 2 and 4, and §13, points 1–3),
 * each with what decided it, for R08.T11 to record:
 *
 * - `noAbsorber`: the medium has none, and the shared table's rgb is already the whole depth;
 * - `severalBands`: the body has two or more latitude bands, ln(s_max ÷ s_min) over
 *   `oblate.ts`'s `ONE_BAND_BELOW`, where a layer's depth divided by s takes s^(n − 1) of the
 *   absorber depth today's read takes, about 1.2% on the ice giants and 7% on Saturn (§13, point 1);
 * - `lightSlots`: the curves take more light slots than R07's `MAX_BODY_LIGHTS`, which no body's stars
 *   do (`settings.ts`'s `SKY_SUN_CAP`), only a test's or the smoke's;
 * - `row`: the per-planet row, as built, with the layers would pass {@link PER_PLANET_ROW_BYTES}.
 */
export type SlotDepthsRefusal =
  | { readonly kind: "noAbsorber" }
  | { readonly kind: "severalBands"; readonly bands: number }
  | { readonly kind: "lightSlots"; readonly slots: number; readonly most: number }
  | { readonly kind: "row"; readonly rowBytes: number; readonly mostBytes: number };

/**
 * Whether a body's tables build the slot layers ({@link AtmosphereTables.slotDepths}): `null` where
 * they do, or why not ({@link SlotDepthsRefusal}), decided from what `setMedium` knows, the medium,
 * the lights' curves and the plan, so that the body's ray march takes its read when its pipeline is
 * made (`decision-r08-t8-leans.md` §13, point 2). The layers never widen `KAPPA_STEP` or
 * `BAND_STEP`: the row is the plan's as it stands, with them.
 */
export function slotDepthsRefusal(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  plan: TablesPlan,
): SlotDepthsRefusal | null {
  if (absorberCount(medium) === 0) {
    return { kind: "noAbsorber" };
  }
  if (plan.bands.length > 1) {
    return { kind: "severalBands", bands: plan.bands.length };
  }
  if (curves.length > MAX_BODY_LIGHTS) {
    return { kind: "lightSlots", slots: curves.length, most: MAX_BODY_LIGHTS };
  }
  const row = perPlanetRowOf(medium, curves, plan);
  const rowBytes = row.baseBytes + row.slotDepthBytes;
  return rowBytes > PER_PLANET_ROW_BYTES
    ? { kind: "row", rowBytes, mostBytes: PER_PLANET_ROW_BYTES }
    : null;
}

/**
 * Design note 11's widening of a body's slicing over the 2 MB row (plan R08, R08.T11.e): none
 * (`none`); κ's step widened to `oblate.ts`'s `WIDE_KAPPA_STEP` (`kappa`); or that and the bands'
 * to `WIDE_BAND_STEP` (`kappaAndBands`).
 */
export type RowWidening = "none" | "kappa" | "kappaAndBands";

/** A body as its tables are sliced under the 2 MB row ({@link rowBodyOf}). */
export interface RowBody {
  /** The body with the slicing the tables are built in: the given one, or it widened. */
  readonly body: AtmosphereBody | null;
  readonly widening: RowWidening;
  /** Whether the row, the slot layers aside, still passes {@link PER_PLANET_ROW_BYTES} at it. */
  readonly over: boolean;
}

/**
 * The slicing a body's tables are built in under the 2 MB row (plan R08, Design note 11; R08.T11.e,
 * `decision-r08-t11-frame.md` §5): the body's own where its per-planet row, the slot layers aside
 * (they never widen a step, `decision-r08-t8-leans.md` §13), is at most
 * {@link PER_PLANET_ROW_BYTES}; past it, κ's step widened to `WIDE_KAPPA_STEP` (`oblate.ts`'s
 * `respacedSlicing`), and still past it the bands' to `WIDE_BAND_STEP` as well, in Design note 11's
 * order. The row is the tables' own, with the column tables the medium reads
 * ({@link columnTableSizes}), so that tables made with every one at full size (`every`, the smoke's)
 * are sliced as the client's.
 *
 * @remarks
 * A row still past 2 MB after both is left over it, and `over` says so. The ruling's next step for
 * the cap at f = 0.2, fewer bands and never a further κ widening (κ's slices carry the limb's colour
 * on every grazing ray), is not built: no fixture's row takes it. The excess it was ruled for is the
 * thick J_ms bands, which no table here holds yet, and the capped giants over 2 MB after both
 * widenings are over by their slices (two absorbers or more, 393 KB a slice), which fewer bands
 * cannot take, so they are left over rather than their picture coarsened. A row over by its curves
 * or its band layers could take fewer bands, its count chosen by Design note 10's metric on the
 * twin; none exists today (R08.T11.e's record). Memory only: R12.T3 judges it.
 *
 * @throws RangeError as {@link tablesPlanOf}.
 */
export function rowBodyOf(
  medium: AtmosphereMedium,
  curves: AbsorberCurveSlots,
  body: AtmosphereBody | null,
  referenceRadiusM: number,
): RowBody {
  const over = (sliced: AtmosphereBody): boolean =>
    perPlanetRowOf(medium, curves, tablesPlanOf(sliced, referenceRadiusM)).baseBytes >
    PER_PLANET_ROW_BYTES;
  if (body === null || !over(body)) {
    return { body, widening: "none", over: false };
  }
  const widenedAt = (kappaStep: number, bandStep: number): AtmosphereBody => ({
    ...body,
    gravity: {
      ...body.gravity,
      slicing: respacedSlicing(body.gravity.slicing, kappaStep, bandStep),
    },
  });
  const kappa = widenedAt(WIDE_KAPPA_STEP, BAND_STEP);
  if (!over(kappa)) {
    return { body: kappa, widening: "kappa", over: false };
  }
  const both = widenedAt(WIDE_KAPPA_STEP, WIDE_BAND_STEP);
  return { body: both, widening: "kappaAndBands", over: over(both) };
}

/**
 * The slot layers' format: `rgba16float`, the shared table's, whose stored optical depth keeps
 * Design note 8's 10⁻³; the kernel arm's {@link KERNEL_ARM_FORMAT}.
 */
const SLOT_DEPTHS_FORMAT: GPUTextureFormat = "rgba16float";

/** The slot layers' texture name. */
const SLOT_DEPTHS_NAME = "atmosphere slot depths";

/** A row of nothing, the tables' before their first build. */
const EMPTY_ROW: PerPlanetRow = {
  transmittanceBytes: 0,
  curvesBytes: 0,
  bandLayerBytes: 0,
  bodyAndTermsBytes: 0,
  mediumTableBytes: 0,
  baseBytes: 0,
  slotDepthBytes: 0,
};

/** The per-planet kernels, as built or as the kernel arm assembles them. */
interface PlanetTableKernels {
  readonly transmittance: ComputeHandle;
  readonly multiScattering: ComputeHandle;
  /** The slot layers' kernel, made at the first build that takes it. */
  readonly slotDepths: () => ComputeHandle;
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
 * Floats in the `Medium` uniform: its shell, term count and steps, the ground's albedo, its
 * absorber slots' count in a vec4f of its own (R08.T6.c), and a per-planet build's own (R08.T6.f).
 */
const MEDIUM_UNIFORM_FLOATS = 16;

/**
 * A per-planet build's own part of the `Medium` uniform (plan R08, R08.T6.f; `common.wgsl`'s
 * `Medium.build`): what one dispatch of the transmittance or multiple-scattering kernel builds.
 */
export interface MediumBuild {
  /** The factor every coefficient and absorber column per metre takes: 1 ÷ s_b in a band's frame. */
  readonly coefficientFactor: number;
  /** The layer of the transmittance tables the transmittance kernel writes or the other reads. */
  readonly transmittanceLayer: number;
  /** The layer of the multiple-scattering table the kernel writes: band × light slots + slot. */
  readonly multiScatteringLayer: number;
}

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
 *   `medium.wgsl`'s `curveSlot()`: one dispatch a slot since R08.T7;
 *   {@link SHARED_TABLE_CURVE_SLOT} by default.
 * @param build - The build's factor and layers (R08.T6.f): by default R05's, the factor 1, the
 *   transmittance tables' layer 0 and the multiple-scattering layer `curveSlot`.
 */
export function packMediumUniform(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  samples: number,
  curveSlot = SHARED_TABLE_CURVE_SLOT,
  build: MediumBuild = {
    coefficientFactor: 1,
    transmittanceLayer: 0,
    multiScatteringLayer: curveSlot,
  },
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
  packed.set(
    [build.coefficientFactor, build.transmittanceLayer, build.multiScatteringLayer, 0],
    12,
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

/** What fixes the tables' contents: the medium, the ground's radius, the lights' curves, the body. */
function keyOf(
  medium: AtmosphereMedium,
  bottomRadiusM: number,
  curves: AbsorberCurveSlots,
  body: AtmosphereBody | null,
): string {
  return JSON.stringify({ medium, bottomRadiusM, curves, body });
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
 * The transmittance kernel's four tables, each a 2D array of one layer a κ slice (R08.T6.f), or a
 * band's own one-layer tables: the optical depth with slot 0's column, slots 1 to 4's columns, and
 * the pressure-weighted columns of channels 0 to 3 and of channel 4 (R08.T4.d).
 */
export interface TransmittanceTextures {
  readonly transmittance: TextureHandle;
  readonly absorberColumns: TextureHandle;
  /** The pressure-weighted columns of pressure channels 0 to 3, in rgba (R08.T4.d). */
  readonly absorberPressureColumns: TextureHandle;
  /** The pressure-weighted column of pressure channel 4, in r ({@link lastPressureColumnSize}). */
  readonly lastAbsorberPressureColumn: TextureHandle;
}

/**
 * The stored transmittance tables of one device, with the layers and the column tables' sizes they
 * were made at.
 */
interface SliceTextures extends TransmittanceTextures {
  readonly layers: number;
  readonly sizes: ColumnTableSizes;
}

/**
 * The GPU objects of one device: the slices' transmittance tables and the bands' multiple
 * scattering, the body's table, the medium's buffer and tables, the lights' curves, the kernels.
 */
interface TableResources {
  /** One layer a κ slice ({@link TablesPlan}, R08.T6.f). */
  slices: SliceTextures;
  /** One layer a latitude band and light slot ({@link multiScatteringLayersOf}, R08.T7, R08.T6.f). */
  multiScattering: LayeredTexture;
  /** The sky's irradiance, the multiple scattering's layers, built on demand (R08.T9.b). */
  irradiance: LayeredTexture;
  /** The body's table ({@link packOblateBody}, R08.T6.f). */
  readonly oblateBody: TextureHandle;
  readonly terms: BufferHandle;
  densityTables: LayeredTexture;
  phaseTables: LayeredTexture;
  /** The terms' forward-lobe tables ({@link packLobeTables}), with their shape. */
  lobeTables: PlanesTexture;
  absorberCurves: UploadedTexture;
  /**
   * Each light slot's optical depth to the top, one layer a κ slice and light slot (R08.T8.e,
   * {@link AtmosphereTables.slotDepths}), or `null` for a body that keeps the shared read.
   */
  slotDepths: LayeredTexture | null;
  readonly transmittanceKernel: ComputeHandle;
  readonly multiScatteringKernel: ComputeHandle;
}

/**
 * The pass every per-planet table build dispatches under (the transmittance, the multiple
 * scattering, the slot layers and the sky's irradiance): the photorealistic style's `atmosphere`
 * slot names it among its passes' labels (plan R08, R08.T10.b; `photoreal/passes.ts`).
 */
export const ATMOSPHERE_TABLES_PASS = "atmosphere tables";

/** The density tables' texture format: one float a level. */
const DENSITY_FORMAT: GPUTextureFormat = "r32float";
/** The phase tables' texture format: the three channels and an unused alpha. */
const PHASE_FORMAT: GPUTextureFormat = "rgba32float";
/** The forward-lobe tables' texture format: the three channels and an unused alpha (R08.T7). */
const LOBE_FORMAT: GPUTextureFormat = "rgba32float";
/** The body's table's format: its figure's and gravity's constants in `f32` (R08.T6.f). */
const OBLATE_BODY_FORMAT: GPUTextureFormat = "rgba32float";
/**
 * The sky-irradiance table's format, `f32` (R08.T9.b), so that the twilight ground keeps its light:
 * per unit of the Sun's illuminance Earth's sky gives some 6 × 10⁻⁸ at −12°, the half float's 2⁻²⁴.
 * A layer is 16 KB.
 */
const IRRADIANCE_FORMAT: GPUTextureFormat = "rgba32float";

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
 * Makes a band table's 2D array texture of `layers` layers at `size`, one a band and light slot
 * ({@link multiScatteringLayersOf}): the multiple-scattering table, written by its kernel and read
 * by the per-frame kernels and the sky-irradiance kernel, or the sky-irradiance table (R08.T9.b),
 * read by the lit passes.
 */
function createBandLayers(
  engine: RenderEngine,
  name: string,
  size: TableSize,
  layers: number,
  format: GPUTextureFormat,
): LayeredTexture {
  const texture = engine.createTexture({
    name,
    size: [size.widthTexels, size.heightTexels, layers],
    dimension: "2d",
    format,
    mips: 1,
    usage: TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC,
    category: "atmosphere-tables",
  });
  return { texture, widthTexels: size.widthTexels, layers };
}

/** Releases the transmittance kernel's four tables. */
function releaseTransmittance(engine: RenderEngine, tables: TransmittanceTextures): void {
  for (const texture of [
    tables.transmittance,
    tables.absorberColumns,
    tables.absorberPressureColumns,
    tables.lastAbsorberPressureColumn,
  ]) {
    engine.releaseTexture(texture);
  }
}

/** Releases the buffer and textures of `resources`; the kernels have no release. */
function releaseResources(engine: RenderEngine, resources: TableResources): void {
  engine.releaseBuffer(resources.terms);
  releaseTransmittance(engine, resources.slices);
  for (const texture of [
    resources.multiScattering.texture,
    resources.irradiance.texture,
    resources.oblateBody,
    resources.densityTables.texture,
    resources.phaseTables.texture,
    resources.lobeTables.texture,
    resources.absorberCurves.texture,
  ]) {
    engine.releaseTexture(texture);
  }
  if (resources.slotDepths !== null) {
    engine.releaseTexture(resources.slotDepths.texture);
  }
}

/**
 * A table the transmittance kernel writes and the others read, a 2D array of `layers` layers at
 * `format` (`rgba16float` stored, or the kernel arm's).
 */
function storedTable(
  engine: RenderEngine,
  name: string,
  size: TableSize,
  layers: number,
  format: GPUTextureFormat = "rgba16float",
): TextureHandle {
  return engine.createTexture({
    name,
    size: [size.widthTexels, size.heightTexels, layers],
    dimension: "2d",
    format,
    mips: 1,
    usage: TEXTURE_USAGE.STORAGE_BINDING | TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_SRC,
    category: "atmosphere-tables",
  });
}

/** The transmittance tables' names, and the suffix a set of them takes. */
function transmittanceNames(suffix: string): Readonly<Record<keyof TransmittanceTextures, string>> {
  return {
    transmittance: `atmosphere transmittance${suffix}`,
    absorberColumns: `atmosphere absorber columns${suffix}`,
    absorberPressureColumns: `atmosphere absorber pressure columns${suffix}`,
    lastAbsorberPressureColumn: `atmosphere last absorber pressure column${suffix}`,
  };
}

/**
 * The transmittance kernel's four tables of `layers` layers, the column tables at `sizes`
 * ({@link columnTableSizes}), at `format`, named with `suffix`.
 */
function createTransmittance(
  engine: RenderEngine,
  suffix: string,
  layers: number,
  sizes: ColumnTableSizes,
  format: GPUTextureFormat = "rgba16float",
): TransmittanceTextures {
  const names = transmittanceNames(suffix);
  return {
    transmittance: storedTable(engine, names.transmittance, TRANSMITTANCE_SIZE, layers, format),
    absorberColumns: storedTable(
      engine,
      names.absorberColumns,
      sizes.absorberColumns,
      layers,
      format,
    ),
    absorberPressureColumns: storedTable(
      engine,
      names.absorberPressureColumns,
      sizes.absorberPressureColumns,
      layers,
      format,
    ),
    lastAbsorberPressureColumn: storedTable(
      engine,
      names.lastAbsorberPressureColumn,
      sizes.lastAbsorberPressureColumn,
      layers,
      format,
    ),
  };
}

/** Whether two sets of column-table sizes are the same. */
function sameSizes(a: ColumnTableSizes, b: ColumnTableSizes): boolean {
  const keys = [
    "absorberColumns",
    "absorberPressureColumns",
    "lastAbsorberPressureColumn",
  ] as const;
  return keys.every(
    (k) => a[k].widthTexels === b[k].widthTexels && a[k].heightTexels === b[k].heightTexels,
  );
}

/** A device's resources, the slices' column tables made at `sizes`, the first build's. */
function createResources(engine: RenderEngine, sizes: ColumnTableSizes): TableResources {
  const one = { widthTexels: 1, layers: 1 };
  return {
    slices: { ...createTransmittance(engine, "", 1, sizes), layers: 1, sizes },
    multiScattering: createBandLayers(
      engine,
      "atmosphere multiple scattering",
      MULTI_SCATTERING_SIZE,
      1,
      "rgba16float",
    ),
    irradiance: createBandLayers(
      engine,
      "atmosphere irradiance",
      IRRADIANCE_SIZE,
      1,
      IRRADIANCE_FORMAT,
    ),
    oblateBody: engine.createTexture({
      name: "atmosphere oblate body",
      size: [OBLATE_BODY_TEXELS, 1],
      dimension: "2d",
      format: OBLATE_BODY_FORMAT,
      mips: 1,
      usage: TEXTURE_USAGE.TEXTURE_BINDING | TEXTURE_USAGE.COPY_DST,
      category: "atmosphere-tables",
    }),
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
    slotDepths: null,
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
 *
 * On an oblate body (plan R08, Design note 17; R08.T6.f) the transmittance tables are 2D arrays of
 * one layer a κ slice, the reference medium on the sphere κ_k R_ref, and the multiple-scattering
 * table one layer a latitude band and light slot, each band an ordinary build in its own
 * gravity-scaled frame at its g(φ) and √(MN) over its own transmittance tables, which are built
 * into textures made for the build and released after it (a body of one slice at κ = 1 and one
 * band reads the slice's, as R05's). Earth and every body under `ONE_SLICE_BELOW` and
 * `ONE_BAND_BELOW` keep one of each, R05's tables (Design note 11). The body's table
 * ({@link AtmosphereTables.oblateBody}) carries its figure, gravity and slicing to the kernels.
 *
 * Since R08.T9.b the tables also hold the sky's diffuse irradiance on a horizontal surface
 * ({@link AtmosphereTables.irradiance}, Bruneton 2017's 64 × 16), one layer a band and light slot
 * as the multiple scattering, built in the same frame from the same tables at its first read after
 * each build, which the lit passes of R07, R10 and R11 read with the transmittance tables and the
 * body's table through `surfaceLighting.wgsl`.
 */
export class AtmosphereTables {
  readonly #engine: RenderEngine;
  readonly #offRestored: () => void;
  #resources: TableResources;
  #medium: AtmosphereMedium;
  #bottomRadiusM: number;
  #curves: AbsorberCurveSlots;
  /** The body the caller gave, which the key holds and a restore builds for again. */
  #body: AtmosphereBody | null;
  /** The body with the slicing the tables are built in ({@link rowBodyOf}, R08.T11.e). */
  #builtBody: AtmosphereBody | null;
  /** Design note 11's widening the last build took (R08.T11.e). */
  #widening: RowWidening = "none";
  /** Which column tables the tables make at full size (R08.T11.e, {@link ColumnTables}). */
  readonly #columnTables: ColumnTables;
  #plan: TablesPlan;
  /** The medium's layer edges, {@link packLayerEdges}'s, packed once a medium. */
  #layerEdges: Float32Array = new Float32Array(LAYER_EDGES_FLOATS);
  #key: string | null = null;
  #builds = 0;
  /** Whether {@link AtmosphereTables.dispose} has released the resources. */
  #disposed = false;
  /** The kernel arm's kernels ({@link AtmosphereTables.buildKernelArm}), made at its first call. */
  #armKernels: PlanetTableKernels | null = null;
  /** Whether the sky's irradiance holds the last build's ({@link AtmosphereTables.irradiance}). */
  #irradianceBuilt = false;
  /** The sky-irradiance kernel, made at the irradiance's first build on a device. */
  #irradianceKernel: ComputeHandle | null = null;
  /** The slot-depth kernel, made at the first build on a device that takes the layers (R08.T8.e). */
  #slotDepthsKernel: ComputeHandle | null = null;
  /** The kernel arm's slot-depth kernel, made at the first arm that takes the layers. */
  #armSlotDepthsKernel: ComputeHandle | null = null;
  /** Why the last build made no slot layers, or `null` where it made them (R08.T8.e). */
  #slotDepthsRefusal: SlotDepthsRefusal | null = { kind: "noAbsorber" };
  /** The last build's per-planet row (R08.T8.e, `decision-r08-t8-leans.md` §13, point 2). */
  #row: PerPlanetRow = EMPTY_ROW;

  /**
   * Makes the tables and builds them for `medium`.
   *
   * @param bottomRadiusM - The ground's radius on the tables' sphere, m: R_ref, the slices' scale.
   * @param curves - Each light's curves of growth for the medium's absorbers; none for a medium
   *   with no absorber.
   * @param body - The body whose slicing the tables are built in (R08.T6.f), widened by Design note
   *   11's rule over the 2 MB row ({@link rowBodyOf}, R08.T11.e); none for one slice and one band,
   *   R05's tables.
   * @param columnTables - Which column tables are made at full size: those the medium reads, or,
   *   for the smoke's check that one texel moves no pixel, every one (R08.T11.e).
   * @throws Error or RangeError as {@link packMedium}, {@link packAbsorberCurves} and
   *   {@link tablesPlanOf}, and `EngineUnavailable` while the engine has no device (a loss), having
   *   released the buffer and textures it made before.
   */
  constructor(
    engine: RenderEngine,
    medium: AtmosphereMedium,
    bottomRadiusM: number,
    curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
    body: AtmosphereBody | null = null,
    columnTables: ColumnTables = "read",
  ) {
    this.#engine = engine;
    this.#medium = medium;
    this.#bottomRadiusM = bottomRadiusM;
    this.#curves = curves;
    this.#body = body;
    this.#builtBody = body;
    this.#columnTables = columnTables;
    this.#plan = tablesPlanOf(body, bottomRadiusM);
    this.#resources = createResources(engine, columnTableSizes(medium, columnTables));
    try {
      this.setMedium(medium, bottomRadiusM, curves, body);
    } catch (error: unknown) {
      releaseResources(engine, this.#resources);
      throw error;
    }
    this.#offRestored = engine.onRestored(() => {
      this.#resources = createResources(engine, columnTableSizes(this.#medium, this.#columnTables));
      this.#armKernels = null;
      this.#irradianceKernel = null;
      this.#slotDepthsKernel = null;
      this.#armSlotDepthsKernel = null;
      this.#key = null;
      this.setMedium(this.#medium, this.#bottomRadiusM, this.#curves, this.#body);
    });
  }

  /**
   * What light loses per (r, μ) to the top, `rgba16float`, a 2D array of one layer a κ slice
   * (R08.T6.f): the optical depth of every term but the absorbers in rgb and absorber slot 0's
   * column u ÷ U in alpha (R08.T6.c); a new handle after a restore.
   */
  get transmittance(): TextureHandle {
    return this.#resources.slices.transmittance;
  }

  /** Absorber slots 1 to 4's columns u ÷ U per (r, μ) to the top, rgba, one layer a slice. */
  get absorberColumns(): TextureHandle {
    return this.#resources.slices.absorberColumns;
  }

  /**
   * Pressure channels 0 to 3's pressure-weighted columns per (r, μ) to the top, rgba,
   * `rgba16float`, one layer a slice: one channel a line slot ({@link pressureChannelOf}; R08.T4.d).
   */
  get absorberPressureColumns(): TextureHandle {
    return this.#resources.slices.absorberPressureColumns;
  }

  /**
   * Pressure channel 4's pressure-weighted column per (r, μ) to the top, in r, `rgba16float`, one
   * layer a slice: the transmittance's size for a medium of five line slots, one texel otherwise
   * ({@link lastPressureColumnSize}); a new handle after a medium or a restore that resizes it.
   */
  get lastAbsorberPressureColumn(): TextureHandle {
    return this.#resources.slices.lastAbsorberPressureColumn;
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
   * a latitude band and light slot, band b's slot k at layer b × {@link AtmosphereTables.layersPerBand}
   * + k (R08.T6.f), each built with that light's curves (R08.T7).
   */
  get multiScattering(): TextureHandle {
    return this.#resources.multiScattering.texture;
  }

  /** The multiple-scattering table's layers: bands × light slots. */
  get multiScatteringLayers(): number {
    return this.#resources.multiScattering.layers;
  }

  /** The multiple-scattering layers a band takes, one a light slot ({@link multiScatteringLayersOf}). */
  get layersPerBand(): number {
    return multiScatteringLayersOf(this.#medium, this.#curves);
  }

  /**
   * The sky's diffuse irradiance on a horizontal surface per (h, μ_sun), per unit illuminance,
   * `rgba32float`, {@link IRRADIANCE_SIZE} (plan R08, R08.T9.b; Bruneton 2017's layout): a 2D array
   * of the multiple-scattering table's layers, band b's light slot k at layer
   * b × {@link AtmosphereTables.layersPerBand} + k, which the lit passes read through
   * `surfaceLighting.wgsl`.
   *
   * @remarks
   * Built at the first call after each build of the tables, from them, so that tables whose air
   * lights no surface (the smoke's, a body seen only as a point) never pay for it; a new handle
   * after a restore or a medium of another layer count.
   *
   * @throws `EngineUnavailable` when it has to build while the engine has no device (a loss); the
   *   first call after the restore builds it.
   */
  irradiance(): TextureHandle {
    if (!this.#irradianceBuilt) {
      this.#dispatchIrradiance();
      this.#irradianceBuilt = true;
    }
    return this.#resources.irradiance.texture;
  }

  /** The body's table ({@link packOblateBody}), `rgba32float`, which the per-frame kernels read. */
  get oblateBody(): TextureHandle {
    return this.#resources.oblateBody;
  }

  /**
   * Each light slot's optical depth to the top, `rgba16float` at {@link TRANSMITTANCE_SIZE}, a 2D
   * array of one layer a κ slice and light slot, slice k's slot p at layer k ×
   * {@link AtmosphereTables.layersPerBand} + p (plan R08, R08.T8.e, lever 5;
   * `decision-r08-t8-leans.md` §10 and §13): each texel the shared table's stored optical depth plus
   * that slot's curves of growth at the texel's own stored columns, so that the ray march reads a
   * source's light, and the bare ground's, in one read. `null` for a body that keeps the shared
   * read ({@link AtmosphereTables.slotDepthsRefusal}); a new handle after a restore or a build of
   * another layer count.
   */
  get slotDepths(): TextureHandle | null {
    return this.#resources.slotDepths?.texture ?? null;
  }

  /**
   * Why the tables hold no slot layers ({@link slotDepthsRefusal}), for R08.T11 to record each body
   * kept off, or `null` where they hold them.
   */
  get slotDepthsRefusal(): SlotDepthsRefusal | null {
    return this.#slotDepthsRefusal;
  }

  /**
   * The per-planet row of the last build ({@link perPlanetRowOf}): every allocation the tables
   * make, the slot layers' bytes whether or not they are made.
   */
  get row(): PerPlanetRow {
    return this.#row;
  }

  /** The bytes the tables allocate, B: the row's base, and the slot layers where they are made. */
  get rowBytes(): number {
    return this.#row.baseBytes + (this.#slotDepthsRefusal === null ? this.#row.slotDepthBytes : 0);
  }

  /** The slices and bands the tables are built in ({@link tablesPlanOf}). */
  get plan(): TablesPlan {
    return this.#plan;
  }

  /**
   * The body the tables are sliced for, with the slicing they are built in: the one the last build
   * was given, its slicing widened over the 2 MB row where Design note 11's rule widens it
   * ({@link rowBodyOf}, R08.T11.e), or `null` for one slice and one band.
   */
  get body(): AtmosphereBody | null {
    return this.#builtBody;
  }

  /** Design note 11's widening the last build took over the 2 MB row ({@link rowBodyOf}, R08.T11.e). */
  get widening(): RowWidening {
    return this.#widening;
  }

  /** The ground's radius on the tables' sphere, m: R_ref, the slices' scale, as last built. */
  get bottomRadiusM(): number {
    return this.#bottomRadiusM;
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
   * Rebuilds the tables if `medium`, `bottomRadiusM`, `curves` or `body` differs from what they were
   * built for.
   *
   * @param curves - Each light's curves of growth for the medium's absorbers; none for a medium
   *   with no absorber. The multiple-scattering table has one layer a band and light slot under an
   *   absorber, each built with that slot's curves ({@link multiScatteringLayersOf}, R08.T7).
   * @param body - The body whose slicing the tables are built in (R08.T6.f); none for one slice and
   *   one band.
   * @returns Whether they were rebuilt.
   * @throws Error or RangeError as {@link packMedium}, {@link packAbsorberCurves} and
   *   {@link tablesPlanOf}, and the tables keep what they held and follow; or `EngineUnavailable`
   *   while the engine has no device (a loss), and the tables follow `medium` from then on, so that
   *   the engine's restore builds it; or Error once they are disposed.
   */
  setMedium(
    medium: AtmosphereMedium,
    bottomRadiusM: number,
    curves: AbsorberCurveSlots = NO_ABSORBER_CURVES,
    body: AtmosphereBody | null = null,
  ): boolean {
    if (this.#disposed) {
      throw new Error(
        "the atmosphere's tables were disposed: a caller holds them past their release",
      );
    }
    const key = keyOf(medium, bottomRadiusM, curves, body);
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
    // Design note 11's widening over the 2 MB row (R08.T11.e): the slicing the plan is built in.
    const sliced = rowBodyOf(medium, curves, body, bottomRadiusM);
    const plan = tablesPlanOf(sliced.body, bottomRadiusM);
    const layersPerBand = multiScatteringLayersOf(medium, curves);
    const oblateBody = packOblateBody(
      sliced.body?.figure ?? { equatorialRadiusM: bottomRadiusM, polarRadiusM: bottomRadiusM },
      sliced.body?.gravity ?? null,
      plan,
      bottomRadiusM,
      medium,
      layersPerBand,
    );
    // Whether the body takes the slot layers (R08.T8.e, `decision-r08-t8-leans.md` §13): decided
    // here, from what the build knows, so that its ray march takes its read when its pipeline is
    // made; the key above already holds all of it, so the choice sets off no rebuild of its own.
    const refusal = slotDepthsRefusal(medium, curves, plan);
    const row = perPlanetRowOf(medium, curves, plan, this.#columnTables);
    this.#medium = medium;
    this.#bottomRadiusM = bottomRadiusM;
    this.#curves = curves;
    this.#body = body;
    this.#builtBody = sliced.body;
    this.#widening = sliced.widening;
    this.#plan = plan;
    this.#layerEdges = layerEdges;
    this.#slotDepthsRefusal = refusal;
    this.#row = row;
    this.#key = null;
    this.#irradianceBuilt = false;
    const engine = this.#engine;
    const r = this.#resources;
    engine.writeBuffer(r.terms, 0, terms);
    r.densityTables = this.#upload(r.densityTables, densityTables, DENSITY_FORMAT);
    r.phaseTables = this.#upload(r.phaseTables, phaseTables, PHASE_FORMAT);
    r.lobeTables = this.#uploadPlanes(r.lobeTables, lobeTables);
    r.absorberCurves = this.#uploadCurves(r.absorberCurves, absorberCurves);
    engine.writeTexture(r.oblateBody, [0, 0, 0], [OBLATE_BODY_TEXELS, 1, 1], oblateBody);
    const sizes = columnTableSizes(medium, this.#columnTables);
    const sliceCount = plan.kappa.length;
    const held = r.slices;
    if (held.layers !== sliceCount || !sameSizes(sizes, held.sizes)) {
      // Made before the old ones go, so that a loss while making them leaves no released handle.
      const made = createTransmittance(engine, "", sliceCount, sizes);
      releaseTransmittance(engine, held);
      r.slices = { ...made, layers: sliceCount, sizes };
    }
    const layers = plan.bands.length * layersPerBand;
    if (r.multiScattering.layers !== layers) {
      const made = createBandLayers(
        engine,
        r.multiScattering.texture.name,
        MULTI_SCATTERING_SIZE,
        layers,
        "rgba16float",
      );
      engine.releaseTexture(r.multiScattering.texture);
      r.multiScattering = made;
    }
    if (r.irradiance.layers !== layers) {
      const made = createBandLayers(
        engine,
        r.irradiance.texture.name,
        IRRADIANCE_SIZE,
        layers,
        IRRADIANCE_FORMAT,
      );
      engine.releaseTexture(r.irradiance.texture);
      r.irradiance = made;
    }
    const slotLayers = refusal === null ? sliceCount * layersPerBand : 0;
    const heldSlots = r.slotDepths;
    if (slotLayers === 0) {
      if (heldSlots !== null) {
        engine.releaseTexture(heldSlots.texture);
        r.slotDepths = null;
      }
    } else if (heldSlots?.layers !== slotLayers) {
      const made = storedTable(
        engine,
        SLOT_DEPTHS_NAME,
        TRANSMITTANCE_SIZE,
        slotLayers,
        SLOT_DEPTHS_FORMAT,
      );
      if (heldSlots !== null) {
        engine.releaseTexture(heldSlots.texture);
      }
      r.slotDepths = {
        texture: made,
        widthTexels: TRANSMITTANCE_SIZE.widthTexels,
        layers: slotLayers,
      };
    }
    this.#dispatchTables(
      {
        transmittance: r.transmittanceKernel,
        multiScattering: r.multiScatteringKernel,
        slotDepths: () => (this.#slotDepthsKernel ??= engine.createCompute(SLOT_DEPTHS_KERNEL)),
      },
      {
        ...r.slices,
        multiScattering: r.multiScattering.texture,
        slotDepths: r.slotDepths?.texture ?? null,
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
   * pressure-weighted column tables are built from the medium, one layer a slice, and the multiple
   * scattering over the stored `rgba16float` ones, a band's own built anew where it has them
   * (R08.T6.f), as the stored table's twin takes them, so that each holds its kernel's own `f32`
   * output. The sky's irradiance needs no arm: its stored table is its kernel's `f32` (R08.T9.b).
   * Where the body takes the slot layers (R08.T8.e, {@link AtmosphereTables.slotDepths}) a sixth
   * holds them in `rgba32float`, built as the stored layers are, over the stored tables
   * (`decision-r08-t8-leans.md` §13, point 9).
   *
   * @returns The five textures, and the sixth or `null`, which the caller releases.
   * @throws `EngineUnavailable` while the engine has no device (a loss).
   */
  buildKernelArm(): PlanetTableTextures {
    const engine = this.#engine;
    this.#armKernels ??= {
      transmittance: engine.createCompute(withKernelArmFormat(TRANSMITTANCE_KERNEL)),
      multiScattering: engine.createCompute(withKernelArmFormat(MULTI_SCATTERING_KERNEL)),
      slotDepths: () =>
        (this.#armSlotDepthsKernel ??= engine.createCompute(
          withKernelArmFormat(SLOT_DEPTHS_KERNEL),
        )),
    };
    const r = this.#resources;
    const slots = r.slotDepths;
    const tables: PlanetTableTextures = {
      ...createTransmittance(
        engine,
        " (kernel arm)",
        r.slices.layers,
        r.slices.sizes,
        KERNEL_ARM_FORMAT,
      ),
      multiScattering: storedTable(
        engine,
        "atmosphere multiple scattering (kernel arm)",
        MULTI_SCATTERING_SIZE,
        r.multiScattering.layers,
        KERNEL_ARM_FORMAT,
      ),
      slotDepths:
        slots === null
          ? null
          : storedTable(
              engine,
              `${SLOT_DEPTHS_NAME} (kernel arm)`,
              TRANSMITTANCE_SIZE,
              slots.layers,
              KERNEL_ARM_FORMAT,
            ),
    };
    this.#dispatchTables(this.#armKernels, tables);
    return tables;
  }

  /**
   * Latitude band `band`'s own transmittance tables, stored, built anew into textures made for the
   * call, as {@link AtmosphereTables.setMedium} builds them for its multiple scattering and
   * releases them (R08.T6.f): the smoke's twins hold the band's table over them. A body of one
   * slice at κ = 1 and one band builds the slice's own.
   *
   * @returns The four one-layer tables, which the caller releases.
   * @throws RangeError for a band the tables do not have; `EngineUnavailable` as above.
   */
  bandTransmittance(band: number): TransmittanceTextures {
    const spec = this.#plan.bands[band];
    if (spec === undefined) {
      throw new RangeError(`the tables have ${this.#plan.bands.length} bands, not band ${band}`);
    }
    const tables = createTransmittance(
      this.#engine,
      ` (band ${band})`,
      1,
      this.#resources.slices.sizes,
    );
    this.#dispatchTransmittance(
      this.#resources.transmittanceKernel,
      tables,
      spec.bottomRadiusM,
      spec.coefficientFactor,
      0,
    );
    return tables;
  }

  /**
   * One transmittance build: the kernel into layer `layer` of `outputs`, on the ground
   * `bottomRadiusM` with every coefficient times `coefficientFactor`.
   */
  #dispatchTransmittance(
    kernel: ComputeHandle,
    outputs: TransmittanceTextures,
    bottomRadiusM: number,
    coefficientFactor: number,
    layer: number,
  ): void {
    const r = this.#resources;
    this.#engine.dispatch(
      kernel,
      {
        uniforms: {
          medium: packMediumUniform(
            this.#medium,
            bottomRadiusM,
            TRANSMITTANCE_SAMPLES,
            SHARED_TABLE_CURVE_SLOT,
            { coefficientFactor, transmittanceLayer: layer, multiScatteringLayer: 0 },
          ),
          layerEdges: this.#layerEdges,
        },
        buffers: { terms: r.terms },
        sampled: { densityTables: r.densityTables.texture },
        storage: {
          transmittanceOut: { texture: outputs.transmittance, level: 0 },
          absorberColumnsOut: { texture: outputs.absorberColumns, level: 0 },
          absorberPressureColumnsOut: { texture: outputs.absorberPressureColumns, level: 0 },
          lastAbsorberPressureColumnOut: { texture: outputs.lastAbsorberPressureColumn, level: 0 },
        },
      },
      groupsOf(TRANSMITTANCE_SIZE),
      ATMOSPHERE_TABLES_PASS,
    );
  }

  /**
   * Builds the tables for the medium, ground and body the tables follow: the transmittance kernel
   * into `outputs`' four tables, one layer a κ slice, then, where `outputs` has them, the slot-depth
   * kernel into the slot layers over the stored tables, one dispatch (R08.T8.e), then, band by band,
   * the multiple-scattering
   * kernel into `outputs.multiScattering`, layer band × light slots + k with light slot k's curves
   * (R08.T7), reading the stored tables: the slice's where the one band shares it, and otherwise
   * the band's own, built first into textures made for the build and released after it (R08.T6.f).
   */
  #dispatchTables(kernels: PlanetTableKernels, outputs: PlanetTableTextures): void {
    const engine = this.#engine;
    const r = this.#resources;
    const medium = this.#medium;
    const plan = this.#plan;
    for (const [k, radiusM] of plan.sliceRadiiM.entries()) {
      this.#dispatchTransmittance(kernels.transmittance, outputs, radiusM, 1, k);
    }
    const slots = multiScatteringLayersOf(medium, this.#curves);
    // The slot layers, after the slices and before the multiple scattering, from the stored tables
    // (R08.T8.e): one dispatch, a workgroup layer a κ slice and light slot.
    if (outputs.slotDepths !== null) {
      engine.dispatch(
        kernels.slotDepths(),
        {
          uniforms: { medium: packMediumUniform(medium, this.#bottomRadiusM, 1) },
          buffers: {},
          sampled: {
            transmittance: r.slices.transmittance,
            absorberColumns: r.slices.absorberColumns,
            absorberPressureColumns: r.slices.absorberPressureColumns,
            lastAbsorberPressureColumn: r.slices.lastAbsorberPressureColumn,
            absorberCurves: r.absorberCurves.texture,
          },
          storage: { slotDepthsOut: { texture: outputs.slotDepths, level: 0 } },
        },
        [
          Math.ceil(TRANSMITTANCE_SIZE.widthTexels / 8),
          Math.ceil(TRANSMITTANCE_SIZE.heightTexels / 8),
          plan.kappa.length * slots,
        ],
        ATMOSPHERE_TABLES_PASS,
      );
    }
    let own: TransmittanceTextures | null = null;
    try {
      for (const [b, band] of plan.bands.entries()) {
        let read: TransmittanceTextures = r.slices;
        if (!plan.shared) {
          own ??= createTransmittance(engine, " (band)", 1, r.slices.sizes);
          this.#dispatchTransmittance(
            r.transmittanceKernel,
            own,
            band.bottomRadiusM,
            band.coefficientFactor,
            0,
          );
          read = own;
        }
        for (let slot = 0; slot < slots; slot += 1) {
          engine.dispatch(
            kernels.multiScattering,
            {
              uniforms: {
                medium: packMediumUniform(
                  medium,
                  band.bottomRadiusM,
                  MULTI_SCATTERING_SAMPLES,
                  slot,
                  {
                    coefficientFactor: band.coefficientFactor,
                    transmittanceLayer: 0,
                    multiScatteringLayer: b * slots + slot,
                  },
                ),
                layerEdges: this.#layerEdges,
              },
              buffers: { terms: r.terms },
              sampled: {
                densityTables: r.densityTables.texture,
                transmittance: read.transmittance,
                absorberColumns: read.absorberColumns,
                absorberPressureColumns: read.absorberPressureColumns,
                lastAbsorberPressureColumn: read.lastAbsorberPressureColumn,
                absorberCurves: r.absorberCurves.texture,
              },
              storage: { multiScatteringOut: { texture: outputs.multiScattering, level: 0 } },
            },
            [MULTI_SCATTERING_SIZE.widthTexels, MULTI_SCATTERING_SIZE.heightTexels, 1],
            ATMOSPHERE_TABLES_PASS,
          );
        }
      }
    } finally {
      if (own !== null) {
        releaseTransmittance(engine, own);
      }
    }
  }

  /**
   * Builds the sky's irradiance for the medium, ground and body the tables follow (R08.T9.b): band by
   * band, the sky-irradiance kernel into layer band × light slots + k of the stored table, over the
   * band's transmittance tables (the slice's where the one band shares it, or its own, built into
   * textures made for the build and released after it, as {@link AtmosphereTables.setMedium} builds
   * them) and that layer of the stored multiple scattering, with light slot k's curves.
   */
  #dispatchIrradiance(): void {
    const engine = this.#engine;
    const r = this.#resources;
    const medium = this.#medium;
    const plan = this.#plan;
    const slots = multiScatteringLayersOf(medium, this.#curves);
    const kernel = (this.#irradianceKernel ??= engine.createCompute(IRRADIANCE_KERNEL));
    let own: TransmittanceTextures | null = null;
    try {
      for (const [b, band] of plan.bands.entries()) {
        let read: TransmittanceTextures = r.slices;
        if (!plan.shared) {
          own ??= createTransmittance(engine, " (band)", 1, r.slices.sizes);
          this.#dispatchTransmittance(
            r.transmittanceKernel,
            own,
            band.bottomRadiusM,
            band.coefficientFactor,
            0,
          );
          read = own;
        }
        for (let slot = 0; slot < slots; slot += 1) {
          // One workgroup a texel (`irradiance.wgsl`).
          engine.dispatch(
            kernel,
            {
              uniforms: {
                medium: packMediumUniform(medium, band.bottomRadiusM, IRRADIANCE_SAMPLES, slot, {
                  coefficientFactor: band.coefficientFactor,
                  transmittanceLayer: 0,
                  multiScatteringLayer: b * slots + slot,
                }),
                layerEdges: this.#layerEdges,
              },
              buffers: { terms: r.terms },
              sampled: {
                densityTables: r.densityTables.texture,
                transmittance: read.transmittance,
                absorberColumns: read.absorberColumns,
                absorberPressureColumns: read.absorberPressureColumns,
                lastAbsorberPressureColumn: read.lastAbsorberPressureColumn,
                absorberCurves: r.absorberCurves.texture,
                multiScattering: r.multiScattering.texture,
                phaseTables: r.phaseTables.texture,
                lobeTables: r.lobeTables.texture,
              },
              storage: { irradianceOut: { texture: r.irradiance.texture, level: 0 } },
            },
            [IRRADIANCE_SIZE.widthTexels, IRRADIANCE_SIZE.heightTexels, 1],
            ATMOSPHERE_TABLES_PASS,
          );
        }
      }
    } finally {
      if (own !== null) {
        releaseTransmittance(engine, own);
      }
    }
  }

  /**
   * Stops following the engine's restores and releases the buffer and the textures it holds, so
   * that the atmosphere cache's eviction frees a body's per-planet memory (plan R08, R08.T10.b;
   * `decision-r08-t10a-leans.md` item 10). Called again, it does nothing. The kernels have no
   * release, and the engine drops the release of a lost device's handles. Nothing may read the
   * tables after it: a reader takes the cache's tables afresh each frame.
   */
  dispose(): void {
    if (this.#disposed) {
      return;
    }
    this.#disposed = true;
    this.#offRestored();
    releaseResources(this.#engine, this.#resources);
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
