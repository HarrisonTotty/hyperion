/**
 * The disc's reflectance table's bake (plan R08, R08.T16.b; Design notes 5, 13 and 17): the air's
 * light from outside the air at each node of `discTable.ts`'s grid, marched as the full passes'
 * march takes it, in the optics worker.
 *
 * @remarks
 * **What is marched.** At each node a view ray comes from far outside down to a point of the datum
 * of the band's sphere (Design note 17's band frame: the reference medium at h\* on the sphere of
 * radius R_b, every coefficient and absorber amount over s_b), entering at the top's shell, and is
 * marched as `rayMarch.wgsl` marches a drawn surface's ray: each step's source is its single
 * scattering of the star's light, shadowed by the band's ground, through the star's transmittance
 * there, plus its multiple scattering, the thick table's J times the kernels' own σ_s,k
 * (`thickRead.wgsl`'s read, on the table's `thick` side: J held at 0, its share
 * clamp(32 μ₀ + 7, 0, 1), the analytic term's rest 0 here), carried along the view's own path, its
 * absorbers by the path's column under the light slot's curves of growth, each step's in-scatter
 * taken without cancellation (`common.wgsl`'s `stepFactor`). The node's sun and view cosines and
 * azimuth are its point's; each step reads the medium and the table at its own h\* and its own
 * cosines on the sphere, so the limb's and the terminator's curvature are the sphere's. The forward
 * lobes' higher orders are not marched: on a `thick` table they take the analytic term's share,
 * which is 0 wherever the table stands.
 *
 * **The thick table.** The kernels' read of J (`read.ts`, `thickRead.wgsl`): the body's routed bake
 * with the polarisation correction summed in where its tables hold one (R08.T14.f), or this bake's
 * own of the same solver (`bake.ts`) for a body not routed, so that the table is the spectral bake's
 * either way (Design note 5); the single scattering and the transmittance are the march's, in the
 * render channels, as the full passes draw them, so that a body's colour does not move at the 10⁹ m
 * boundary (Design note 13).
 *
 * **The star's transmittance** at a step is a table of the band's sphere on Bruneton's (r, μ)
 * mapping ({@link DISC_SUN_TABLE}), each node's optical depth and absorber amounts by Simpson's rule
 * along its ray to the top, the light slot's curves added at each node; read bilinearly. Its ground
 * shadow is the sphere's, as `source.wgsl`'s.
 *
 * **The air's own light** (R08.T18.b; `thermalSource.ts`): a self-luminous medium's view rays also
 * gather what the kernels' thermal forms gather, each step's B̃ at its h\* times the depth it absorbs
 * through slot 0's path and step factor, and σ_s,k J_th of the bake's scattered thermal light where
 * it holds it, at the step's travel cosine; one value a view cosine, π L_th, held in a thermal layer
 * a band past the light slots' ({@link DiscReflectanceTable.thermalScale}).
 */

import type { Rgb } from "../../photometry/toneCurve";
import { type AbsorberCurve, absorberTransmittance } from "../absorbers";
import {
  DISC_AZIMUTHS,
  DISC_LAYER_VALUES,
  DISC_SUN_COSINES,
  DISC_VIEW_COSINES,
  discAzimuthAt,
  discSunCosineAt,
  type DiscReflectanceTable,
  discTableValuesOf,
  discTexelIndex,
  discViewCosineAt,
} from "../discTable";
import {
  ABSORBER_PRESSURE_FLOOR_PA,
  ABSORBER_PRESSURE_REFERENCE_PA,
  type AtmosphereMedium,
  densityLookup,
  extinction,
  phaseAt,
} from "../medium";
import {
  distanceToTopM,
  intersectsGround,
  localRadiusM,
  type Shell,
  transmittanceUvToRMu,
} from "../opticalDepth";
import type { MsSourceTable, ThickBakeBand } from "./bake";
import {
  bandsScattering,
  type MediumTemperature,
  type ThermalRgb,
  type ThermalSource,
  thermalPlanckInto,
  thermalScatteringInto,
  thermalSourceOf,
} from "../thermalSource";
import type { PolarisationTable } from "./polarisation";
import { fromHalfBits } from "../../sky/half";
import {
  type PackedThickTable,
  packThickTable,
  THICK_ALTITUDES,
  THICK_ROWS,
  THICK_SUN_COSINES,
  THICK_SUN_COSINES_BELOW,
  THICK_TRAVEL_COSINES,
  thickAnalyticShare,
  thickCurveChannel,
  thickJInto,
  type ThickRow,
  thickRowAt,
  type ThickSource,
  thickSourceScratch,
  thickSunPlace,
} from "./read";
import { cos, exp, expm1, log, sin } from "../../../lib/pinnedMath";

/** The star's transmittance table's nodes: along μ (Bruneton's u) and along r (his v). */
export const DISC_SUN_TABLE = { cosines: 128, radii: 64, steps: 64 } as const;

/** The view ray's steps from the top to the datum: quadratic towards the datum, as the march's. */
export const DISC_VIEW_STEPS = 48;

/** What the disc's bake takes (the module's remarks). */
export interface DiscBakeInput {
  /**
   * The kernels' medium (`tables.ts`'s `kernelMedium` of the drawn medium): the forward deltas out,
   * its tabulated profiles and phases on the kernels' grids.
   */
  readonly medium: AtmosphereMedium;
  /**
   * Each light slot's curves of growth, one an absorber term in the medium's order of them
   * (`tables.ts`'s `AbsorberCurveSlots`); none for a medium with no absorber.
   */
  readonly curves: ReadonlyArray<ReadonlyArray<AbsorberCurve>>;
  /** The body's latitude bands, in the tables' order (`bake.ts`'s `thickBakeBands`). */
  readonly bands: ReadonlyArray<ThickBakeBand>;
  /** The thick table J is read from, `thick` kind, one layer a band and light slot. */
  readonly source: PackedThickTable;
  /** A self-luminous medium's thermal source (R08.T18.b), for the table's thermal layers. */
  readonly thermal?: ThermalSource;
}

/** A medium term as the bake reads it, in its band's frame. */
interface BandTerm {
  readonly densityOf: (heightM: number) => number;
  readonly scattering: Rgb;
  readonly extinction: Rgb;
  readonly phase: AtmosphereMedium["terms"][number]["phase"];
  /** Its absorber's index among the medium's absorbers, or −1. */
  readonly absorber: number;
  /** Its amount at relative density 1, of its curve's kind, per metre of path; 0 for none. */
  readonly amountPerM: number;
}

/** One band as the bake marches it. */
interface BandFrame {
  readonly shell: Shell;
  readonly terms: ReadonlyArray<BandTerm>;
  /** The absorbers' amounts' kinds, in their order. */
  readonly absorberKinds: ReadonlyArray<AbsorberCurve["kind"]>;
  /** The medium's pressure over 1 bar at a height, or `null` with no pressure profile. */
  readonly pressureBarOf: ((heightM: number) => number) | null;
}

/** A band's terms in its frame: every coefficient and amount per metre times 1 ÷ s_b. */
function bandFrameOf(medium: AtmosphereMedium, band: ThickBakeBand): BandFrame {
  const f = band.coefficientFactor;
  const kinds: Array<AbsorberCurve["kind"]> = [];
  const terms = medium.terms.map((term): BandTerm => {
    const sigmaT = extinction(term);
    let absorber = -1;
    let amountPerM = 0;
    if (term.absorber !== undefined) {
      absorber = kinds.length;
      kinds.push(term.absorber.kind);
      amountPerM =
        f *
        (term.absorber.kind === "column"
          ? term.absorber.numberDensityPerM3
          : term.absorber.pairDensityPerM6);
    }
    return {
      densityOf: densityLookup(term.density),
      scattering: [term.scattering[0] * f, term.scattering[1] * f, term.scattering[2] * f],
      extinction: [sigmaT[0] * f, sigmaT[1] * f, sigmaT[2] * f],
      phase: term.phase,
      absorber,
      amountPerM,
    };
  });
  return {
    shell: {
      bottomRadiusM: band.bottomRadiusM,
      topRadiusM: band.bottomRadiusM + medium.topHeightM,
    },
    terms,
    absorberKinds: kinds,
    pressureBarOf: medium.pressureBar === undefined ? null : densityLookup(medium.pressureBar),
  };
}

/** A path's absorber amounts and their pressure-weighted sums, one an absorber. */
interface PathAmounts {
  readonly amounts: Float64Array;
  readonly weighted: Float64Array;
}

/** The optical depth per channel a light slot's curves give a path's amounts. */
function curveDepthOf(
  frame: BandFrame,
  curves: ReadonlyArray<AbsorberCurve>,
  path: PathAmounts,
): Rgb {
  const depth: [number, number, number] = [0, 0, 0];
  for (const [k, kind] of frame.absorberKinds.entries()) {
    const curve = curves[k];
    const u = path.amounts[k] ?? 0;
    if (curve === undefined || !(u > 0)) {
      continue;
    }
    // The path's Curtis–Godson pressure, held at the curves' floor as `common.wgsl`'s
    // `pathLogPressure` holds it; none for a medium with no pressure profile, whose curves have no
    // pressure rows.
    const pressurePa =
      frame.pressureBarOf === null
        ? undefined
        : Math.max(
            (ABSORBER_PRESSURE_REFERENCE_PA * (path.weighted[k] ?? 0)) / u,
            ABSORBER_PRESSURE_FLOOR_PA,
          );
    const t = absorberTransmittance(
      curve,
      kind === "column" ? { kind, columnPerM2: u } : { kind, pairColumnPerM5: u },
      pressurePa,
    );
    depth[0] -= log(t[0]);
    depth[1] -= log(t[1]);
    depth[2] -= log(t[2]);
  }
  return depth;
}

/**
 * One band's star transmittance, one light slot's: {@link DISC_SUN_TABLE}'s nodes on Bruneton's
 * mapping of the band's shell, each its ray's optical depth to the top with the slot's curves added.
 */
interface SunTable {
  readonly shell: Shell;
  /** The top's radius squared, m², and √(top² − bottom²), m: the mapping's terms. */
  readonly top2: number;
  readonly bigH: number;
  /** Optical depth per channel, `[(v × cosines + u) × 3 + c]`. */
  readonly depth: Float64Array;
}

/** A band's ray integrals to the top at each node of the star's table, before any curve. */
interface SunRays {
  readonly depth: Float64Array;
  readonly paths: ReadonlyArray<PathAmounts>;
}

/** Composite Simpson's rule along each node's ray to the top (the module's remarks). */
function sunRaysOf(frame: BandFrame): SunRays {
  const { cosines, radii, steps } = DISC_SUN_TABLE;
  const nodes = cosines * radii;
  const depth = new Float64Array(nodes * 3);
  const paths: PathAmounts[] = [];
  const absorbers = frame.absorberKinds.length;
  for (let v = 0; v < radii; v += 1) {
    for (let u = 0; u < cosines; u += 1) {
      const { rM, mu } = transmittanceUvToRMu(frame.shell, u / (cosines - 1), v / (radii - 1));
      const tMax = distanceToTopM(frame.shell, rM, mu);
      const dt = tMax / steps;
      const sum = [0, 0, 0];
      const amounts = new Float64Array(absorbers);
      const weighted = new Float64Array(absorbers);
      for (let i = 0; i <= steps; i += 1) {
        const w = (i === 0 || i === steps ? 1 : i % 2 === 1 ? 4 : 2) * (dt / 3);
        const h = localRadiusM(rM, mu, i * dt) - frame.shell.bottomRadiusM;
        const pressure = frame.pressureBarOf?.(Math.max(h, 0)) ?? 0;
        for (const term of frame.terms) {
          const rho = term.densityOf(Math.max(h, 0)) * w;
          sum[0] = (sum[0] ?? 0) + term.extinction[0] * rho;
          sum[1] = (sum[1] ?? 0) + term.extinction[1] * rho;
          sum[2] = (sum[2] ?? 0) + term.extinction[2] * rho;
          if (term.absorber >= 0) {
            amounts[term.absorber] = (amounts[term.absorber] ?? 0) + term.amountPerM * rho;
            weighted[term.absorber] =
              (weighted[term.absorber] ?? 0) + term.amountPerM * rho * pressure;
          }
        }
      }
      const at = (v * cosines + u) * 3;
      depth[at] = sum[0] ?? 0;
      depth[at + 1] = sum[1] ?? 0;
      depth[at + 2] = sum[2] ?? 0;
      paths.push({ amounts, weighted });
    }
  }
  return { depth, paths };
}

/** A light slot's star table from its band's rays: each node's depth with the slot's curves. */
function sunTableOf(
  frame: BandFrame,
  rays: SunRays,
  curves: ReadonlyArray<AbsorberCurve>,
): SunTable {
  const depth = Float64Array.from(rays.depth);
  if (frame.absorberKinds.length > 0) {
    rays.paths.forEach((path, node) => {
      const added = curveDepthOf(frame, curves, path);
      depth[node * 3] = (depth[node * 3] ?? 0) + added[0];
      depth[node * 3 + 1] = (depth[node * 3 + 1] ?? 0) + added[1];
      depth[node * 3 + 2] = (depth[node * 3 + 2] ?? 0) + added[2];
    });
  }
  const { bottomRadiusM: bottom, topRadiusM: top } = frame.shell;
  return {
    shell: frame.shell,
    top2: top * top,
    bigH: Math.sqrt(top * top - bottom * bottom),
    depth,
  };
}

/**
 * The star's transmittance from radius `rM` along zenith cosine `mu` into `out`, 0 where the band's
 * ground hides it.
 */
function sunTransmittanceInto(
  table: SunTable,
  rM: number,
  mu: number,
  out: [number, number, number],
): void {
  const shell = table.shell;
  const r = Math.min(Math.max(rM, shell.bottomRadiusM), shell.topRadiusM);
  if (intersectsGround(shell, r, mu)) {
    out[0] = 0;
    out[1] = 0;
    out[2] = 0;
    return;
  }
  const { cosines, radii } = DISC_SUN_TABLE;
  // `opticalDepth.ts`'s `transmittanceRMuToUv`, its shell's terms taken once a table.
  const rho = Math.sqrt(Math.max(r * r - shell.bottomRadiusM ** 2, 0));
  const d = Math.max(-r * mu + Math.sqrt(Math.max(r * r * (mu * mu - 1) + table.top2, 0)), 0);
  const dMin = shell.topRadiusM - r;
  const u = (d - dMin) / (rho + table.bigH - dMin);
  const x = Math.min(Math.max(u, 0), 1) * (cosines - 1);
  const y = Math.min(Math.max(rho / table.bigH, 0), 1) * (radii - 1);
  const x0 = Math.min(Math.floor(x), cosines - 2);
  const y0 = Math.min(Math.floor(y), radii - 2);
  const fx = x - x0;
  const fy = y - y0;
  const depth = table.depth;
  const i00 = (y0 * cosines + x0) * 3;
  const i10 = i00 + 3;
  const i01 = i00 + cosines * 3;
  const i11 = i01 + 3;
  for (let c = 0; c < 3; c += 1) {
    const tau =
      (1 - fy) * ((1 - fx) * (depth[i00 + c] ?? Number.NaN) + fx * (depth[i10 + c] ?? Number.NaN)) +
      fy * ((1 - fx) * (depth[i01 + c] ?? Number.NaN) + fx * (depth[i11 + c] ?? Number.NaN));
    out[c] = exp(-tau);
  }
}

/** One step of a view ray, from the camera's side, with what its sun-independent march holds. */
interface ViewStep {
  /** The step's midpoint, in the node's frame (z the point's normal), and its radius, m. */
  readonly x: number;
  readonly z: number;
  readonly rM: number;
  readonly heightM: number;
  /** The thick table's rows about its height. */
  readonly row: ThickRow;
  /** Each term's scattering at the step per channel, the band's frame: `[term × 3 + c]`. */
  readonly scattering: Float64Array;
  /** The kernels' σ_s,k per channel there. */
  readonly kernelScattering: Rgb;
  /** The view's throughput before the step per channel, slot by slot: `[slot × 3 + c]`. */
  readonly throughput: Float64Array;
  /** Its length times stepFactor of its optical depth per channel, slot by slot. */
  readonly weight: Float64Array;
  /**
   * Slot 0's depth the step absorbs, its terms' (σ_ext − σ_s) and its absorbers' curves', times
   * stepFactor of its optical depth, per channel: the emission's weight (R08.T18.b).
   */
  readonly absorbedWeight: Float64Array;
}

/** A view ray of one view cosine, slot by slot, and its throughput at the datum. */
interface ViewRay {
  readonly muView: number;
  readonly steps: ReadonlyArray<ViewStep>;
  /** Each slot's throughput at the datum per channel. */
  readonly through: ReadonlyArray<Rgb>;
}

/** `stepFactor`'s g(x) = (1 − e^(−x)) ÷ x, exactly. */
function stepFactor(x: number): number {
  return x > 0 ? -expm1(-x) / x : 1;
}

/**
 * The view ray of view cosine `muView` down to the band's datum point (0, 0, R_b), its steps
 * quadratic towards the datum from the top's shell, as the march places a drawn surface's.
 */
function viewRayOf(
  frame: BandFrame,
  source: PackedThickTable,
  curves: ReadonlyArray<ReadonlyArray<AbsorberCurve>>,
  slots: number,
  muView: number,
  steps: number,
): ViewRay {
  const R = frame.shell.bottomRadiusM;
  const sinView = Math.sqrt(Math.max(1 - muView * muView, 0));
  const length = distanceToTopM(frame.shell, R, muView);
  const terms = frame.terms;
  const absorbers = frame.absorberKinds.length;
  const throughput = Array.from({ length: slots }, (): [number, number, number] => [1, 1, 1]);
  const pathDepth = Array.from({ length: slots }, (): [number, number, number] => [0, 0, 0]);
  const path: PathAmounts = {
    amounts: new Float64Array(absorbers),
    weighted: new Float64Array(absorbers),
  };
  const out: ViewStep[] = [];
  for (let i = steps - 1; i >= 0; i -= 1) {
    const near = length * (i / steps) ** 2;
    const far = length * ((i + 1) / steps) ** 2;
    const dt = far - near;
    const t = 0.5 * (near + far);
    const x = t * sinView;
    const z = R + t * muView;
    const rM = Math.hypot(x, z);
    const heightM = Math.max(rM - R, 0);
    const scattering = new Float64Array(terms.length * 3);
    const kernelScattering: [number, number, number] = [0, 0, 0];
    const sigma = [0, 0, 0];
    const pressure = frame.pressureBarOf?.(heightM) ?? 0;
    terms.forEach((term, k) => {
      const rho = term.densityOf(heightM);
      for (const c of CHANNELS) {
        const s = term.scattering[c] * rho;
        scattering[k * 3 + c] = s;
        kernelScattering[c] += s;
        sigma[c] = (sigma[c] ?? 0) + term.extinction[c] * rho;
      }
      if (term.absorber >= 0) {
        path.amounts[term.absorber] =
          (path.amounts[term.absorber] ?? 0) + term.amountPerM * rho * dt;
        path.weighted[term.absorber] =
          (path.weighted[term.absorber] ?? 0) + term.amountPerM * rho * dt * pressure;
      }
    });
    const before = new Float64Array(slots * 3);
    const weight = new Float64Array(slots * 3);
    const absorbedWeight = new Float64Array(3);
    for (let slot = 0; slot < slots; slot += 1) {
      const slotThroughput = throughput[slot] ?? [1, 1, 1];
      const slotDepth = pathDepth[slot] ?? [0, 0, 0];
      const curveDepth =
        absorbers > 0 ? curveDepthOf(frame, curves[slot] ?? curves[0] ?? [], path) : null;
      for (let c = 0; c < 3; c += 1) {
        const absorbed = curveDepth === null ? 0 : (curveDepth[c] ?? 0) - (slotDepth[c] ?? 0);
        if (curveDepth !== null) {
          slotDepth[c] = curveDepth[c] ?? 0;
        }
        const stepDepth = (sigma[c] ?? 0) * dt + absorbed;
        before[slot * 3 + c] = slotThroughput[c] ?? 1;
        weight[slot * 3 + c] = dt * stepFactor(stepDepth);
        if (slot === 0) {
          absorbedWeight[c] =
            (((sigma[c] ?? 0) - (kernelScattering[c] ?? 0)) * dt + absorbed) *
            stepFactor(stepDepth);
        }
        slotThroughput[c] = (slotThroughput[c] ?? 1) * exp(-stepDepth);
      }
    }
    out.push({
      x,
      z,
      rM,
      heightM,
      row: thickRowAt(source, heightM),
      scattering,
      kernelScattering,
      throughput: before,
      weight,
      absorbedWeight,
    });
  }
  return { muView, steps: out, through: throughput };
}

/**
 * A view ray's own light (the module's remarks, "The air's own light"), π L_th per channel, cd m⁻²
 * times π: each step's B̃ through slot 0's emission weight, and σ_s,k J_th of the band's scattered
 * thermal light at the step's travel cosine.
 *
 * @param lnBands - ln s_b of the body's bands, the scattered light's read.
 * @param s - The band's own s_b, at which the read takes its own layer.
 */
function thermalAirInto(
  ray: ViewRay,
  thermal: ThermalSource,
  lnBands: ArrayLike<number>,
  s: number,
  out: ThermalRgb,
): void {
  const muView = ray.muView;
  const sinView = Math.sqrt(Math.max(1 - muView * muView, 0));
  const planck: ThermalRgb = [0, 0, 0];
  const scattered: ThermalRgb = [0, 0, 0];
  out.fill(0);
  for (const step of ray.steps) {
    const travel = (sinView * step.x + muView * step.z) / step.rM;
    thermalPlanckInto(thermal, step.heightM, planck);
    thermalScatteringInto(thermal, lnBands, s, step.heightM, travel, scattered);
    for (const c of CHANNELS) {
      out[c] +=
        (step.throughput[c] ?? 0) *
        (planck[c] * (step.absorbedWeight[c] ?? 0) +
          (step.weight[c] ?? 0) * step.kernelScattering[c] * scattered[c]);
    }
  }
  for (const c of CHANNELS) {
    out[c] *= Math.PI;
  }
}

/** The colour channels, red, green and blue. */
const CHANNELS = [0, 1, 2] as const;

/** The values a decoded texel holds: J⁰, G and G₂, each per channel, then ln J⁰ per channel. */
const DECODED_TEXEL = 12;

/**
 * One layer of a packed thick table decoded once (`read.ts`'s texel): J⁰, G and G₂ per channel in
 * the table's order, then ln J⁰ per channel (−∞ where J⁰ is not above 0), so that the bake's many
 * reads of one layer take no decoding (`read.ts`'s `addThickLayer` decodes each texel at each read)
 * and its geometric step one exponential a channel in place of two powers.
 */
export function decodedThickLayer(table: PackedThickTable, layer: number): Float64Array {
  const texels = THICK_ROWS * THICK_TRAVEL_COSINES;
  const out = new Float64Array(
    THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * DECODED_TEXEL,
  );
  for (let i = 0; i < THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES; i += 1) {
    const at = (layer * texels + i) * 4;
    const x = table.texels[at] ?? 0;
    const y = table.texels[at + 1] ?? 0;
    const z = table.texels[at + 2] ?? 0;
    const curve = table.texels[at + 3] ?? 0;
    const o = i * DECODED_TEXEL;
    out[o] = fromHalfBits(x & 0xffff);
    out[o + 1] = fromHalfBits(x >>> 16);
    out[o + 2] = fromHalfBits(y & 0xffff);
    out[o + 3] = fromHalfBits(y >>> 16);
    out[o + 4] = fromHalfBits(z & 0xffff);
    out[o + 5] = fromHalfBits(z >>> 16);
    if (curve !== 0) {
      out[o + 6] = thickCurveChannel(curve, 0);
      out[o + 7] = thickCurveChannel(curve, 1);
      out[o + 8] = thickCurveChannel(curve, 2);
    }
    for (const c of CHANNELS) {
      const level = out[o + c] ?? 0;
      out[o + 9 + c] = level > 0 ? log(level) : Number.NEGATIVE_INFINITY;
    }
  }
  return out;
}

/** A decoded texel (a, s, v) added into `out` at weight `w`. */
function addDecoded(
  decoded: Float64Array,
  a: number,
  s: number,
  v: number,
  w: number,
  out: ThickSource,
): void {
  const o = ((a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * DECODED_TEXEL;
  for (const c of CHANNELS) {
    out.level[c] += w * (decoded[o + c] ?? 0);
    out.slope[c] += w * (decoded[o + 3 + c] ?? 0);
    out.curve[c] += w * (decoded[o + 6 + c] ?? 0);
  }
}

/** A {@link ThickSource} set to 0. */
function clearSource(out: ThickSource): void {
  out.level.fill(0);
  out.slope.fill(0);
  out.curve.fill(0);
}

/** The decoded read's scratch: the two rows of a geometric step. */
interface DecodedScratch {
  readonly lower: ThickSource;
  readonly upper: ThickSource;
}

/**
 * `read.ts`'s `addThickLayer` on a decoded layer, at weight 1 into a cleared `out`: linear in h,
 * μ₀ and μ_v between the cells' centres, but below μ₀ = 1/64 J⁰ geometric in μ₀ where both rows
 * hold it above 0, G and G₂ linear as their ratios to it (`read.ts`'s remarks).
 */
export function decodedThickRead(
  decoded: Float64Array,
  row: ThickRow,
  muSun: number,
  muTravel: number,
  scratch: DecodedScratch,
  out: ThickSource,
): void {
  clearSource(out);
  const x = thickSunPlace(muSun);
  const s0 = Math.min(Math.floor(x), THICK_SUN_COSINES - 2);
  const fs = x - s0;
  const y = Math.min(
    Math.max((muTravel + 1) * (THICK_TRAVEL_COSINES / 2) - 0.5, 0),
    THICK_TRAVEL_COSINES - 1,
  );
  const v0 = Math.min(Math.floor(y), THICK_TRAVEL_COSINES - 2);
  const fv = y - v0;
  const a0 = row.lower;
  const ta = row.t;
  for (let da = 0; da < 2; da += 1) {
    const wa = da === 0 ? 1 - ta : ta;
    for (let dv = 0; dv < 2; dv += 1) {
      const w = wa * (dv === 0 ? 1 - fv : fv);
      if (s0 >= THICK_SUN_COSINES_BELOW) {
        addDecoded(decoded, a0 + da, s0, v0 + dv, w * (1 - fs), out);
        addDecoded(decoded, a0 + da, s0 + 1, v0 + dv, w * fs, out);
        continue;
      }
      const { lower, upper } = scratch;
      const lowerAt = ((a0 + da) * THICK_SUN_COSINES + s0) * THICK_TRAVEL_COSINES + v0 + dv;
      const upperAt = lowerAt + THICK_TRAVEL_COSINES;
      clearSource(lower);
      clearSource(upper);
      addDecoded(decoded, a0 + da, s0, v0 + dv, 1, lower);
      addDecoded(decoded, a0 + da, s0 + 1, v0 + dv, 1, upper);
      for (const c of CHANNELS) {
        const l0 = lower.level[c];
        const l1 = upper.level[c];
        if (l0 > 0 && l1 > 0) {
          // J⁰_lower^(1 − f) J⁰_upper^f, as `read.ts`'s `addGeometric` forms it by two powers.
          const level = exp(
            (1 - fs) * (decoded[lowerAt * DECODED_TEXEL + 9 + c] ?? Number.NaN) +
              fs * (decoded[upperAt * DECODED_TEXEL + 9 + c] ?? Number.NaN),
          );
          out.level[c] += w * level;
          out.slope[c] +=
            w * level * ((1 - fs) * (lower.slope[c] / l0) + fs * (upper.slope[c] / l1));
          out.curve[c] +=
            w * level * ((1 - fs) * (lower.curve[c] / l0) + fs * (upper.curve[c] / l1));
        } else {
          out.level[c] += w * ((1 - fs) * l0 + fs * l1);
          out.slope[c] += w * ((1 - fs) * lower.slope[c] + fs * upper.slope[c]);
          out.curve[c] += w * ((1 - fs) * lower.curve[c] + fs * upper.curve[c]);
        }
      }
    }
  }
}

/** The scratch a node's march reads the thick table into. */
interface NodeScratch extends DecodedScratch {
  readonly source: ThickSource;
  readonly j: [number, number, number];
  readonly toSun: [number, number, number];
}

/** A fresh {@link NodeScratch}. */
function nodeScratch(): NodeScratch {
  return {
    source: thickSourceScratch(),
    lower: thickSourceScratch(),
    upper: thickSourceScratch(),
    j: [0, 0, 0],
    toSun: [0, 0, 0],
  };
}

/**
 * One node's air, π L ÷ E per channel, along a view ray: its sun at cosine `muSun` and azimuth
 * `azimuthRad` from the view's horizontal (the module's remarks).
 */
function nodeAirInto(
  frame: BandFrame,
  ray: ViewRay,
  sun: SunTable,
  decoded: Float64Array,
  slot: number,
  muSun: number,
  azimuthRad: number,
  scratch: NodeScratch,
  out: [number, number, number],
): void {
  const sinSun = Math.sqrt(Math.max(1 - muSun * muSun, 0));
  const s = { x: sinSun * cos(azimuthRad), y: sinSun * sin(azimuthRad), z: muSun };
  const muView = ray.muView;
  const sinView = Math.sqrt(Math.max(1 - muView * muView, 0));
  // The march's ray runs towards the datum: d = −v, v = (sin θ_v, 0, μ_v) towards the camera.
  const dDotS = -(sinView * s.x + muView * s.z);
  const phases = frame.terms.map((term) => phaseAt(term.phase, dDotS));
  let red = 0;
  let green = 0;
  let blue = 0;
  for (const step of ray.steps) {
    const nx = step.x / step.rM;
    const nz = step.z / step.rM;
    const muSunHere = nx * s.x + nz * s.z;
    // The view's travel cosine towards the camera, −(d · n), and h⊥ = d · s − (d · n)(s · n).
    const travel = sinView * nx + muView * nz;
    const horizontal = dDotS + travel * muSunHere;
    sunTransmittanceInto(sun, step.rM, muSunHere, scratch.toSun);
    let single0 = 0;
    let single1 = 0;
    let single2 = 0;
    for (let k = 0; k < phases.length; k += 1) {
      const p = phases[k] ?? [0, 0, 0];
      single0 += (step.scattering[k * 3] ?? 0) * p[0];
      single1 += (step.scattering[k * 3 + 1] ?? 0) * p[1];
      single2 += (step.scattering[k * 3 + 2] ?? 0) * p[2];
    }
    const tableShare = 1 - thickAnalyticShare(muSunHere);
    let multiple0 = 0;
    let multiple1 = 0;
    let multiple2 = 0;
    if (tableShare > 0) {
      decodedThickRead(decoded, step.row, muSunHere, travel, scratch, scratch.source);
      thickJInto(scratch.source, muSunHere, travel, horizontal, scratch.j);
      multiple0 = tableShare * Math.max(scratch.j[0], 0) * step.kernelScattering[0];
      multiple1 = tableShare * Math.max(scratch.j[1], 0) * step.kernelScattering[1];
      multiple2 = tableShare * Math.max(scratch.j[2], 0) * step.kernelScattering[2];
    }
    const at = slot * 3;
    red +=
      (step.throughput[at] ?? 0) *
      (step.weight[at] ?? 0) *
      (single0 * scratch.toSun[0] + multiple0);
    green +=
      (step.throughput[at + 1] ?? 0) *
      (step.weight[at + 1] ?? 0) *
      (single1 * scratch.toSun[1] + multiple1);
    blue +=
      (step.throughput[at + 2] ?? 0) *
      (step.weight[at + 2] ?? 0) *
      (single2 * scratch.toSun[2] + multiple2);
  }
  out[0] = Math.PI * red;
  out[1] = Math.PI * green;
  out[2] = Math.PI * blue;
}

/** The deepest sun under which the top of any band's air is lit: −√(1 − (R ÷ (R + H))²). */
export function discSunFloorOf(
  bands: ReadonlyArray<Pick<ThickBakeBand, "bottomRadiusM">>,
  topHeightM: number,
): number {
  let floor = 0;
  for (const band of bands) {
    const ratio = band.bottomRadiusM / (band.bottomRadiusM + topHeightM);
    floor = Math.min(floor, -Math.sqrt(1 - ratio * ratio));
  }
  return floor;
}

/**
 * One band's own light at any view cosine (R08.T18.b), the bake's own march, for the tests: π L_th
 * per channel, cd m⁻² times π.
 *
 * @throws RangeError for a band the input does not hold, or an input without a thermal source.
 */
export function discThermalMarcher(
  input: DiscBakeInput,
  band: number,
  options: DiscBakeOptions = {},
): (muView: number) => readonly [number, number, number] {
  const geometry = input.bands[band];
  const thermal = input.thermal;
  if (geometry === undefined || thermal === undefined) {
    throw new RangeError(`the disc's bake has no band ${band} with a thermal source`);
  }
  const frame = bandFrameOf(input.medium, geometry);
  const lnBands = input.bands.map((each) => log(each.gravityRatio));
  const steps = options.viewSteps ?? DISC_VIEW_STEPS;
  return (muView) => {
    const ray = viewRayOf(frame, input.source, input.curves, input.source.slots, muView, steps);
    const out: ThermalRgb = [0, 0, 0];
    thermalAirInto(ray, thermal, lnBands, geometry.gravityRatio, out);
    return out;
  };
}

/** A bake's options, for the tests' convergence. */
export interface DiscBakeOptions {
  /** The view ray's steps: {@link DISC_VIEW_STEPS}. */
  readonly viewSteps?: number;
}

/**
 * One band's air at any view cosine, sun cosine and azimuth, the bake's own march (the module's
 * remarks), for the tests that judge the table's grid against it.
 *
 * @returns `(muView, muSun, azimuthRad, slot) → [r, g, b, mean transmittance]`.
 * @throws RangeError for a band the input does not hold.
 */
export function discAirMarcher(
  input: DiscBakeInput,
  band: number,
  options: DiscBakeOptions = {},
): (
  muView: number,
  muSun: number,
  azimuthRad: number,
  slot: number,
) => readonly [number, number, number, number] {
  const geometry = input.bands[band];
  if (geometry === undefined) {
    throw new RangeError(`the disc's bake has ${input.bands.length} bands, not a band ${band}`);
  }
  const frame = bandFrameOf(input.medium, geometry);
  const slots = input.source.slots;
  const rays = sunRaysOf(frame);
  const suns = Array.from({ length: slots }, (_, slot) =>
    sunTableOf(frame, rays, input.curves[slot] ?? input.curves[0] ?? []),
  );
  const steps = options.viewSteps ?? DISC_VIEW_STEPS;
  const scratch = nodeScratch();
  const decoded = Array.from({ length: slots }, (_, slot) =>
    decodedThickLayer(input.source, band * slots + slot),
  );
  return (muView, muSun, azimuthRad, slot) => {
    const k = Math.min(slot, slots - 1);
    const ray = viewRayOf(frame, input.source, input.curves, slots, muView, steps);
    const out: [number, number, number] = [0, 0, 0];
    const sun = suns[k];
    if (sun === undefined) {
      throw new RangeError(`the disc's bake has no light slot ${k}`);
    }
    nodeAirInto(
      frame,
      ray,
      sun,
      decoded[k] ?? new Float64Array(),
      k,
      muSun,
      azimuthRad,
      scratch,
      out,
    );
    const through = ray.through[k] ?? [1, 1, 1];
    return [out[0], out[1], out[2], (through[0] + through[1] + through[2]) / 3];
  };
}

/**
 * A body's disc reflectance table (`discTable.ts`; the module's remarks): every band and light slot
 * of `input.source`, each node marched, its values rounded to the half floats the texture holds.
 *
 * @throws RangeError for a source of another band count than `input.bands`, or of another kind
 *   than `thick`.
 */
export function bakeDiscReflectance(
  input: DiscBakeInput,
  options: DiscBakeOptions = {},
): DiscReflectanceTable {
  const { source, bands } = input;
  if (source.kind !== "thick" || source.bands !== bands.length) {
    throw new RangeError(
      `the disc's bake reads a thick table of its ${bands.length} bands, not a ${source.kind} table of ${source.bands}`,
    );
  }
  const slots = source.slots;
  const sunFloor = discSunFloorOf(bands, input.medium.topHeightM);
  const lit = bands.length * slots * DISC_LAYER_VALUES;
  const { thermal } = input;
  const values = new Float32Array(
    lit + (thermal === undefined ? 0 : bands.length * DISC_LAYER_VALUES),
  );
  const steps = options.viewSteps ?? DISC_VIEW_STEPS;
  const scratch = nodeScratch();
  const air: [number, number, number] = [0, 0, 0];
  const lnBands = bands.map((each) => log(each.gravityRatio));
  // π L_th of each band at each view cosine, rgb, where the medium is self-luminous.
  const own = new Float64Array(bands.length * DISC_VIEW_COSINES * 3);
  bands.forEach((geometry, b) => {
    const frame = bandFrameOf(input.medium, geometry);
    const rays = sunRaysOf(frame);
    const views = Array.from({ length: DISC_VIEW_COSINES }, (_, v) =>
      viewRayOf(frame, source, input.curves, slots, discViewCosineAt(v), steps),
    );
    if (thermal !== undefined) {
      const glow: ThermalRgb = [0, 0, 0];
      views.forEach((ray, v) => {
        thermalAirInto(ray, thermal, lnBands, geometry.gravityRatio, glow);
        own.set(glow, (b * DISC_VIEW_COSINES + v) * 3);
      });
    }
    for (let slot = 0; slot < slots; slot += 1) {
      const sun = sunTableOf(frame, rays, input.curves[slot] ?? input.curves[0] ?? []);
      const layer = b * slots + slot;
      const decoded = decodedThickLayer(source, layer);
      for (let s = 0; s < DISC_SUN_COSINES; s += 1) {
        const muSun = discSunCosineAt(s, sunFloor);
        for (let k = 0; k < DISC_AZIMUTHS; k += 1) {
          const azimuth = discAzimuthAt(k);
          views.forEach((ray, v) => {
            nodeAirInto(frame, ray, sun, decoded, slot, muSun, azimuth, scratch, air);
            const through = ray.through[slot] ?? [1, 1, 1];
            values.set(
              [air[0], air[1], air[2], (through[0] + through[1] + through[2]) / 3],
              discTexelIndex(layer, s, v, k),
            );
          });
        }
      }
    }
  });
  // The thermal layers, a band's at every sun row and azimuth, over their largest magnitude, so that
  // the half floats hold them in [−1, 1] whatever the glow's luminance, signed: a cool glow's green
  // and blue below about 2,000 K are negative (Rec. 709's matching functions' negative lobes).
  const scale = Math.max(0, ...own.map((value) => Math.abs(value)));
  if (thermal === undefined || !(scale > 0)) {
    return {
      bands: bands.length,
      slots,
      sunFloor,
      values: discTableValuesOf(values.subarray(0, lit)),
    };
  }
  const base = bands.length * slots;
  for (let b = 0; b < bands.length; b += 1) {
    for (let s = 0; s < DISC_SUN_COSINES; s += 1) {
      for (let k = 0; k < DISC_AZIMUTHS; k += 1) {
        for (let v = 0; v < DISC_VIEW_COSINES; v += 1) {
          const at = (b * DISC_VIEW_COSINES + v) * 3;
          values.set(
            [(own[at] ?? 0) / scale, (own[at + 1] ?? 0) / scale, (own[at + 2] ?? 0) / scale, 0],
            discTexelIndex(base + b, s, v, k),
          );
        }
      }
    }
  }
  return {
    bands: bands.length,
    slots,
    sunFloor,
    values: discTableValuesOf(values),
    thermalScale: scale,
  };
}

/**
 * What a `disc-bake` job takes beside its thick bake's request (`opticsMessages.ts`): the medium
 * as the tables hold it, and what of the body's bakes has landed. Plain data, for a structured
 * clone.
 */
export interface DiscBakeAir {
  /** The drawn medium, every term's forward delta in: the correction's (`read.ts`'s `ThickCorrection`). */
  readonly medium: AtmosphereMedium;
  /** The kernels' medium of it (`tables.ts`'s `kernelMedium`), which the march reads. */
  readonly kernelMedium: AtmosphereMedium;
  /** Each light slot's curves of growth, the tables' (`AtmosphereTables.curves`). */
  readonly curves: ReadonlyArray<ReadonlyArray<AbsorberCurve>>;
  /**
   * The body's routed thick bake, one table a band in the tables' order, where it has landed: its J
   * is read as it is; absent, the job bakes its own of the same solver.
   */
  readonly bake?: ReadonlyArray<MsSourceTable>;
  /** The body's polarisation correction, one table a band, where it has landed: summed into J. */
  readonly correction?: ReadonlyArray<PolarisationTable>;
  /**
   * A self-luminous medium's column temperature (R08.T18.b), from which the table's thermal layers
   * are baked, the bake's scattered thermal light among them where it holds it.
   */
  readonly temperature?: MediumTemperature;
}

/**
 * A body's disc reflectance table as the optics worker answers a `disc-bake` (the module's
 * remarks): J from the routed bake given, or from `solve` (the thick bake of every band, run only
 * where none is given), with the correction summed in where given, packed as the kernels read it,
 * then {@link bakeDiscReflectance}.
 *
 * @param bands - The body's bands, `bake.ts`'s `thickBakeBands(body)`, in the tables' order.
 * @param solve - Bakes J for every band, in their order (`prepareBake` and `bakeMultipleScattering`).
 * @throws RangeError as `read.ts`'s `packThickTable` and {@link bakeDiscReflectance} refuse their
 *   inputs.
 */
export function discReflectanceFor(
  air: DiscBakeAir,
  bands: ReadonlyArray<ThickBakeBand>,
  solve: () => ReadonlyArray<MsSourceTable>,
): DiscReflectanceTable {
  const baked = air.bake ?? solve();
  const source = packThickTable(
    baked,
    air.kernelMedium,
    air.correction === undefined ? null : { bands: air.correction, medium: air.medium },
  );
  const scattering = bandsScattering(baked);
  const thermal =
    air.temperature === undefined
      ? undefined
      : thermalSourceOf(air.temperature, air.kernelMedium.topHeightM, scattering);
  return bakeDiscReflectance({
    medium: air.kernelMedium,
    curves: air.curves,
    bands,
    source,
    ...(thermal === undefined ? {} : { thermal }),
  });
}
