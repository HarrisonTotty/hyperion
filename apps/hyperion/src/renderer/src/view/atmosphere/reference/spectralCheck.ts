/**
 * R08.T17's spectral check (plan R08, Verification's "Spectral error"; Design notes 5 and 10): the
 * reference traced in the 15 bake bins (`spectralCases.ts`), taken to the render channels, against
 * the same reference in the three fitted channels and against the client's twin, at Earth and
 * Mars.
 *
 * @remarks
 * - **The spectral colour.** A bin's radiance L_b over the sun's irradiance in it E_b is X_b, the
 *   light per unit of the star's in that bin. The channel is L_c = E_c Σ_b (W_cb ÷ C_c) X_b, the
 *   spectral bakes' conversion on storage (`thick/binConversion.ts`), E_c the three-channel case's
 *   sun, so that a flat X gives the three-channel case's own colour exactly. The bins are traced
 *   independently, so σ_c = E_c (Σ_b (W_cb σ_b ÷ (C_c E_b))²)^½.
 * - **What is compared.** Each geometry of the spectral parts against the three-channel
 *   reference's vector I (the error of the channels alone, both by the tracer) and against the
 *   twin's routed radiance, the polarisation correction summed in (`vectorGate.json`'s
 *   `twinValues.routed`, the client as it draws the case). Recorded per image, never gated:
 *   Design note 10's tallies (`gate.ts`'s metric with the spectral reading as L_ref), the median
 *   and the largest Δu′v′ with its geometry, the largest ΔE*ab through `toneCurve` at the AVG
 *   meter's exposure of the image's mean, and IPRT's relative RMS per channel. Each Δu′v′ is
 *   recorded beside its noise, the radius of the spectral reading's 1σ in (u′, v′) by the bins'
 *   standard errors, linearised.
 * - **The beam.** At each ground view straight up, the sun's beam from each reference's
 *   `sunOpticalDepth`: the bins' E_c Σ_b (W_cb ÷ C_c) e^(−τ_b) against the channels' E_c e^(−τ_c),
 *   by Δu′v′ and the channels' luminance over the bins', and the twin's own transmittance towards
 *   the sun by Δu′v′. The bins sample τ at their centres, where at the horizon τ changes by more
 *   than 1 across a bin: on Bruneton 2017's model of the same air that leaves the bins' beam
 *   0.0002 (the sun 10° up) to 0.0006 (on the horizon) in u′v′ from its colour at 1 nm, 0.0017 on
 *   the horizon without ozone (R08.T17's cross-check), beside R08.T4.a's 1 nm figure for the
 *   channels (0.022 under the Sun).
 * - **The fluxes.** Each aggregate's downwelling flux at the ground and upwelling flux at the top,
 *   by Δu′v′ and the channels' luminance over the bins'.
 * - **The readings.** Each geometry's spectral reading in the channels, its σ and its Δu′v′ noise,
 *   so that another pipeline (Bruneton 2017's, R08.T17's cross-check) can be held to it.
 *
 * The twin's figures are those of R08.T9.c's surfaces per channel, the disc's and the aerial
 * perspective's among them, as `vectorGate.json` records them.
 */

import type { Rgb } from "../../photometry/toneCurve";
import {
  deltaEab,
  type GateReport,
  gate,
  luminanceOf,
  type ReferenceReading,
  uvPrimeDifference,
} from "../gate";
import { uvOfRgb } from "../spectralColour";
import { type BinConversion, binConversion } from "../thick/binConversion";
import { recorded, recordedRgb, twinValueKey } from "./caseGate";
import { type ReferenceFile, readingsOf } from "./references";
import type { SpectralReferenceCase } from "./spectralCases";
import { exp } from "../../../lib/pinnedMath";

/** The record's format. */
export const SPECTRAL_CHECK_FORMAT = 1;

/** A spectral part with its traced reference. */
export interface SpectralTrace {
  readonly spectral: SpectralReferenceCase;
  readonly reference: ReferenceFile;
}

/** The conversion's factors W_cb ÷ C_c times E_c ÷ E_b, one row a channel, one entry a bin. */
type ChannelFactors = readonly [
  ReadonlyArray<number>,
  ReadonlyArray<number>,
  ReadonlyArray<number>,
];

/** The three channels of a bin-indexed quantity, and its σ, as the module's remarks take them. */
interface Converted {
  readonly value: Rgb;
  readonly sigma: Rgb;
}

/** The conversion's factors W_cb ÷ C_c times E_c ÷ E_b, per channel and bin. */
function channelFactors(spectral: SpectralReferenceCase): ChannelFactors {
  const conversion: BinConversion = binConversion(spectral.spectrum);
  if (conversion.guarded.some((g) => g)) {
    throw new Error(`${spectral.name}'s sun is too far outside Rec. 709 for the conversion`);
  }
  const sun = spectral.base.sunIlluminanceLx;
  const row = (c: 0 | 1 | 2): number[] =>
    Array.from(
      conversion.factors[c],
      (f, b) => (f * sun[c]) / (spectral.sunIrradianceWPerM2Nm[b] ?? Number.NaN),
    );
  return [row(0), row(1), row(2)];
}

/** Bin values in the channels (the module's remarks). */
function convertedValue(factors: ChannelFactors, values: ReadonlyArray<number>): Rgb {
  const value = (c: 0 | 1 | 2): number =>
    factors[c].reduce((s, f, b) => s + f * (values[b] ?? Number.NaN), 0);
  return [value(0), value(1), value(2)];
}

/** Bin values and their σ in the channels (the module's remarks). */
function converted(
  factors: ChannelFactors,
  values: ReadonlyArray<number>,
  sigmas: ReadonlyArray<number>,
): Converted {
  const sigma = (c: 0 | 1 | 2): number =>
    Math.sqrt(factors[c].reduce((s, f, b) => s + (f * (sigmas[b] ?? Number.NaN)) ** 2, 0));
  return { value: convertedValue(factors, values), sigma: [sigma(0), sigma(1), sigma(2)] };
}

/** The radius of the 1σ of a colour in (u′, v′), from its bins' σ, linearised. */
function uvNoise(
  factors: ChannelFactors,
  values: ReadonlyArray<number>,
  sigmas: ReadonlyArray<number>,
): number {
  const base = convertedValue(factors, values);
  const [u0, v0] = uvOfRgb(base);
  let sum = 0;
  for (const [b, s] of sigmas.entries()) {
    const nudged: Rgb = [
      base[0] + (factors[0][b] ?? 0) * s,
      base[1] + (factors[1][b] ?? 0) * s,
      base[2] + (factors[2][b] ?? 0) * s,
    ];
    const [u, v] = uvOfRgb(nudged);
    sum += (u - u0) ** 2 + (v - v0) ** 2;
  }
  return Math.sqrt(sum);
}

/** One geometry of a spectral reference, in the channels. */
export interface SpectralReading extends ReferenceReading {
  /** Its Δu′v′ noise (the module's remarks). */
  readonly uvNoise: number;
}

/**
 * A spectral part's geometries as the gate's readings in the channels, each in its base case's
 * image, of the vector I.
 *
 * @throws Error for a reference of other wavelengths than its case's, or a geometry the case does
 *   not have.
 */
export function spectralReadingsOf(trace: SpectralTrace): SpectralReading[] {
  const { spectral, reference } = trace;
  if (
    reference.wavelengthsNm.length !== spectral.wavelengthsNm.length ||
    reference.wavelengthsNm.some((nm, i) => Math.abs(nm - (spectral.wavelengthsNm[i] ?? 0)) > 1e-9)
  ) {
    throw new Error(`${reference.case} was not traced at ${spectral.name}'s wavelengths`);
  }
  const factors = channelFactors(spectral);
  const images = new Map(spectral.geometries.map((g) => [g.name, g.image] as const));
  return reference.geometries.map((g) => {
    const image = images.get(g.name);
    if (image === undefined) {
      throw new Error(`${spectral.name} has no geometry ${g.name}`);
    }
    const whole = converted(factors, g.radiance, g.standardError);
    const quarter = converted(factors, g.radiance, g.quarterStandardError);
    return {
      name: g.name,
      image,
      radiance: whole.value,
      standardError: whole.sigma,
      samples: g.samples,
      quarterSamples: g.quarterSamples,
      quarterStandardError: quarter.sigma,
      uvNoise: uvNoise(factors, g.radiance, g.standardError),
    };
  });
}

/** One side of an image's comparison: Δu′v′, ΔE*ab and the relative RMS against the spectrum. */
export interface ImageColour {
  readonly medianUv: number;
  readonly largestUv: number;
  readonly largestUvAt: string;
  readonly largestDeltaE: number;
  readonly relativeRms: Rgb;
}

/** A median of numbers, the mean of the middle two of an even count. */
function median(values: ReadonlyArray<number>): number {
  const sorted = values.toSorted((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1
    ? (sorted[mid] ?? Number.NaN)
    : ((sorted[mid - 1] ?? Number.NaN) + (sorted[mid] ?? Number.NaN)) / 2;
}

/** An image's colour figures of `other` against the spectral readings. */
function imageColour(
  readings: ReadonlyArray<SpectralReading>,
  other: ReadonlyMap<string, Rgb>,
): ImageColour {
  const meanLuminance = readings.reduce((s, r) => s + luminanceOf(r.radiance), 0) / readings.length;
  const theirs = (r: SpectralReading): Rgb => {
    const value = other.get(r.name);
    if (value === undefined) {
      throw new Error(`no radiance of ${r.name} to compare`);
    }
    return value;
  };
  const uv = readings.map((r) => uvPrimeDifference(theirs(r), r.radiance));
  let at = 0;
  for (const [i, d] of uv.entries()) {
    if (d > (uv[at] ?? 0)) {
      at = i;
    }
  }
  const rms = (c: 0 | 1 | 2): number =>
    Math.sqrt(
      readings.reduce((s, r) => s + (theirs(r)[c] - r.radiance[c]) ** 2, 0) /
        readings.reduce((s, r) => s + r.radiance[c] ** 2, 0),
    );
  return {
    medianUv: median(uv),
    largestUv: Math.max(...uv),
    largestUvAt: readings[at]?.name ?? "",
    largestDeltaE: Math.max(...readings.map((r) => deltaEab(theirs(r), r.radiance, meanLuminance))),
    relativeRms: [rms(0), rms(1), rms(2)],
  };
}

/** Design note 10's tallies of a gate. */
export interface Tally {
  readonly comparisons: number;
  readonly passes: number;
  readonly misses: number;
  readonly refusals: number;
  /** The largest |ΔL| ÷ (T + 4σ) over the comparisons not refused, and where. */
  readonly worstRatio: number;
  readonly worstAt: string;
}

const CHANNEL_LETTERS = ["r", "g", "b"] as const;

/** A gate report's tallies. */
function tallyOf(report: GateReport): Tally {
  const held = report.comparisons.filter((c) => !c.refused);
  let worstRatio = 0;
  let worstAt = "";
  for (const c of held) {
    const ratio = Math.abs(c.client - c.reference) / c.bound;
    if (ratio > worstRatio) {
      worstRatio = ratio;
      worstAt = `${c.name}/${CHANNEL_LETTERS[c.channel]}`;
    }
  }
  return {
    comparisons: report.comparisons.length,
    passes: held.filter((c) => c.passes).length,
    misses: report.failures.length,
    refusals: report.comparisons.length - held.length,
    worstRatio,
    worstAt,
  };
}

/** One image of a body's check. */
export interface ImageCheck {
  readonly image: string;
  readonly geometries: number;
  /** The median of its spectral readings' Δu′v′ noise. */
  readonly medianUvNoise: number;
  /** The three-channel reference against the spectral one. */
  readonly channels: ImageColour;
  readonly channelsTally: Tally;
  /** The twin's routed radiance against the spectral reference. */
  readonly twin: ImageColour;
  readonly twinTally: Tally;
}

/** One beam of a body's check. */
export interface BeamCheck {
  readonly geometry: string;
  readonly sunZenithDeg: number;
  /** The bins' beam, E_c Σ_b (W_cb ÷ C_c) e^(−τ_b), lx. */
  readonly binsLx: Rgb;
  /** The channels' beam, E_c e^(−τ_c), lx. */
  readonly channelsLx: Rgb;
  readonly uv: number;
  /** The channels' luminance over the bins'. */
  readonly luminanceRatio: number;
  /**
   * The twin's own beam, E_c times its transmittance table's read at the observer towards the sun
   * (`caseGate.json`'s `case/beam` values), by Δu′v′ from the bins', where the record holds it.
   */
  readonly twinUv: number | null;
}

/** One flux of a body's check. */
export interface FluxCheck {
  readonly flux: string;
  /** The bins' flux in the channels, lx. */
  readonly binsLx: Rgb;
  /** The three-channel reference's, lx. */
  readonly channelsLx: Rgb;
  readonly uv: number;
  readonly uvNoise: number;
  readonly luminanceRatio: number;
}

/** A body's check (the module's remarks). */
export interface BodyCheck {
  readonly body: string;
  readonly parts: ReadonlyArray<string>;
  /** Every geometry's spectral reading in the channels, for the record and the cross-check. */
  readonly readings: ReadonlyArray<SpectralReading>;
  readonly images: ReadonlyArray<ImageCheck>;
  readonly beams: ReadonlyArray<BeamCheck>;
  readonly fluxes: ReadonlyArray<FluxCheck>;
  readonly channelsTally: Tally;
  readonly twinTally: Tally;
}

/** Two arrays added entry by entry. */
function added(x: ReadonlyArray<number>, y: ReadonlyArray<number>): number[] {
  return x.map((v, i) => v + (y[i] ?? Number.NaN));
}

/** The ground's views straight up, whose sun's beam the check reads. */
function isZenithView(name: string): boolean {
  return /^ground-s\d+-v0-a0$/u.test(name);
}

/** A beam from each reference's depths, or `undefined` where the ground hides the sun. */
function beamOf(
  factors: ChannelFactors,
  spectral: SpectralReferenceCase,
  binDepths: ReadonlyArray<number | null>,
  channelDepths: ReadonlyArray<number | null>,
): { readonly binsLx: Rgb; readonly channelsLx: Rgb } | undefined {
  if (binDepths.some((d) => d === null) || channelDepths.some((d) => d === null)) {
    return undefined;
  }
  const transmitted = binDepths.map(
    (d, b) => exp(-(d ?? 0)) * (spectral.sunIrradianceWPerM2Nm[b] ?? 0),
  );
  const binsLx = convertedValue(factors, transmitted);
  const sun = spectral.base.sunIlluminanceLx;
  const channel = (c: 0 | 1 | 2): number => sun[c] * exp(-(channelDepths[c] ?? 0));
  return { binsLx, channelsLx: [channel(0), channel(1), channel(2)] };
}

/**
 * A body's check from its spectral parts' traces, its three-channel reference and the twin's values.
 *
 * @param twin - The twin's routed radiance by geometry name ({@link routedTwinOf}).
 * @param caseGateTwin - The case gate's twin values by `caseGate.ts`'s `twinValueKey`
 *   (`twinValuesOfRecord` of `caseGate.json`), whose `beam` values are the twin's transmittance
 *   towards the sun, dimensionless, per channel.
 * @throws Error for parts of different bases, or a geometry the three-channel reference or the twin
 *   does not hold.
 */
export function bodyCheck(
  traces: ReadonlyArray<SpectralTrace>,
  channelReference: ReferenceFile,
  twin: ReadonlyMap<string, Rgb>,
  caseGateTwin: ReadonlyMap<string, Rgb>,
): BodyCheck {
  const [first] = traces;
  if (first === undefined || traces.some((t) => t.spectral.base !== first.spectral.base)) {
    throw new Error("a body's check takes its parts, of one base case");
  }
  const base = first.spectral.base;
  const factors = channelFactors(first.spectral);
  const readings = traces.flatMap(spectralReadingsOf);
  const channelReadings = new Map(
    readingsOf(base, channelReference).map((r) => [r.name, r] as const),
  );
  const channels = new Map(
    readings.map((r) => {
      const theirs = channelReadings.get(r.name);
      if (theirs === undefined) {
        throw new Error(`${channelReference.case} holds no geometry ${r.name}`);
      }
      return [r.name, theirs.radiance] as const;
    }),
  );
  const imageNames = [...new Set(readings.map((r) => r.image))];
  const images = imageNames.map((image): ImageCheck => {
    const members = readings.filter((r) => r.image === image);
    return {
      image,
      geometries: members.length,
      medianUvNoise: median(members.map((r) => r.uvNoise)),
      channels: imageColour(members, channels),
      channelsTally: tallyOf(gate(members, channels)),
      twin: imageColour(members, twin),
      twinTally: tallyOf(gate(members, twin)),
    };
  });
  const channelGeometries = new Map(channelReference.geometries.map((g) => [g.name, g] as const));
  const beams = traces.flatMap(({ spectral, reference }) =>
    reference.geometries.flatMap((g): BeamCheck[] => {
      const theirs = channelGeometries.get(g.name);
      const placed = spectral.geometries.find((each) => each.name === g.name);
      if (!isZenithView(g.name) || theirs === undefined || placed === undefined) {
        return [];
      }
      const beam = beamOf(
        factors,
        spectral,
        g.sunOpticalDepth[0] ?? [],
        theirs.sunOpticalDepth[0] ?? [],
      );
      if (beam === undefined) {
        return [];
      }
      const transmittance = caseGateTwin.get(
        twinValueKey({ quantity: "beam", setting: "case", caseName: base.name, name: g.name }),
      );
      const sun = spectral.base.sunIlluminanceLx;
      return [
        {
          geometry: g.name,
          sunZenithDeg: placed.sun.zenithDeg,
          binsLx: beam.binsLx,
          channelsLx: beam.channelsLx,
          uv: uvPrimeDifference(beam.channelsLx, beam.binsLx),
          luminanceRatio: luminanceOf(beam.channelsLx) / luminanceOf(beam.binsLx),
          twinUv:
            transmittance === undefined
              ? null
              : uvPrimeDifference(
                  [sun[0] * transmittance[0], sun[1] * transmittance[1], sun[2] * transmittance[2]],
                  beam.binsLx,
                ),
        },
      ];
    }),
  );
  const channelFluxes = new Map(channelReference.aggregates.map((a) => [a.name, a] as const));
  const fluxes = traces.flatMap(({ reference }) =>
    reference.aggregates.flatMap((a): FluxCheck[] => {
      const theirs = channelFluxes.get(a.name);
      if (theirs === undefined) {
        throw new Error(`${channelReference.case} holds no aggregate ${a.name}`);
      }
      const pair = (
        name: string,
        bins: ReadonlyArray<number>,
        binSigma: ReadonlyArray<number>,
        mine: ReadonlyArray<number>,
      ): FluxCheck => {
        const spectralFlux = convertedValue(factors, bins);
        const channelFlux: Rgb = [
          mine[0] ?? Number.NaN,
          mine[1] ?? Number.NaN,
          mine[2] ?? Number.NaN,
        ];
        return {
          flux: `${a.name}/${name}`,
          binsLx: spectralFlux,
          channelsLx: channelFlux,
          uv: uvPrimeDifference(channelFlux, spectralFlux),
          uvNoise: uvNoise(factors, bins, binSigma),
          luminanceRatio: luminanceOf(channelFlux) / luminanceOf(spectralFlux),
        };
      };
      return [
        pair(
          "downwelling",
          added(a.directGround, a.diffuseGround),
          a.diffuseGroundStandardError,
          added(theirs.directGround, theirs.diffuseGround),
        ),
        pair("upwelling", a.upwellingTop, a.upwellingTopStandardError, theirs.upwellingTop),
      ];
    }),
  );
  return {
    body: base.name,
    parts: traces.map((t) => t.spectral.name),
    readings,
    images,
    beams,
    fluxes,
    channelsTally: tallyOf(gate(readings, channels)),
    twinTally: tallyOf(gate(readings, twin)),
  };
}

/** The twin's routed radiance of a case by geometry, from `vectorGate.json`'s `twinValues`. */
export function routedTwinOf(vectorGate: unknown, caseName: string): Map<string, Rgb> {
  const values = new Map<string, Rgb>();
  const twinValues: unknown =
    typeof vectorGate === "object" && vectorGate !== null
      ? Reflect.get(vectorGate, "twinValues")
      : undefined;
  const routed: unknown =
    typeof twinValues === "object" && twinValues !== null
      ? Reflect.get(twinValues, "routed")
      : undefined;
  if (!Array.isArray(routed)) {
    return values;
  }
  const prefix = `case/radiance/${caseName}/`;
  for (const row of routed) {
    if (!Array.isArray(row)) {
      continue;
    }
    const [key, r, g, b]: unknown[] = row;
    if (
      typeof key === "string" &&
      key.startsWith(prefix) &&
      typeof r === "number" &&
      typeof g === "number" &&
      typeof b === "number"
    ) {
      values.set(key.slice(prefix.length), [r, g, b]);
    }
  }
  return values;
}

/** An image's colour figures as the record keeps them. */
function colourRecord(colour: ImageColour): unknown {
  return {
    medianUv: recorded(colour.medianUv),
    largestUv: recorded(colour.largestUv),
    largestUvAt: colour.largestUvAt,
    largestDeltaE: recorded(colour.largestDeltaE),
    relativeRms: recordedRgb(colour.relativeRms),
  };
}

/** A tally as the record keeps it. */
function tallyRecord(tally: Tally): unknown {
  return { ...tally, worstRatio: recorded(tally.worstRatio) };
}

/**
 * A body's check as the record keeps it, numbers to seven figures, every list of numbers short, so
 * that `bless.ts`'s text is Prettier's too and the record needs no line in `.prettierignore`.
 */
export function bodyCheckRecord(check: BodyCheck): unknown {
  return {
    parts: check.parts,
    channelsTally: tallyRecord(check.channelsTally),
    twinTally: tallyRecord(check.twinTally),
    images: check.images.map((i) => ({
      image: i.image,
      geometries: i.geometries,
      medianUvNoise: recorded(i.medianUvNoise),
      channels: colourRecord(i.channels),
      channelsTally: tallyRecord(i.channelsTally),
      twin: colourRecord(i.twin),
      twinTally: tallyRecord(i.twinTally),
    })),
    beams: check.beams.map((b) => ({
      geometry: b.geometry,
      sunZenithDeg: b.sunZenithDeg,
      binsLx: recordedRgb(b.binsLx),
      channelsLx: recordedRgb(b.channelsLx),
      uv: recorded(b.uv),
      luminanceRatio: recorded(b.luminanceRatio),
      twinUv: b.twinUv === null ? null : recorded(b.twinUv),
    })),
    fluxes: check.fluxes.map((f) => ({
      flux: f.flux,
      binsLx: recordedRgb(f.binsLx),
      channelsLx: recordedRgb(f.channelsLx),
      uv: recorded(f.uv),
      uvNoise: recorded(f.uvNoise),
      luminanceRatio: recorded(f.luminanceRatio),
    })),
    readings: check.readings.map((r) => ({
      name: r.name,
      radiance: recordedRgb(r.radiance),
      standardError: recordedRgb(r.standardError),
      uvNoise: recorded(r.uvNoise),
    })),
  };
}
