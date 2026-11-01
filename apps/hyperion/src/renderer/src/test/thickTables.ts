/**
 * Synthetic thick bakes, polarisation corrections and deck tables for the read path's tests and the
 * smoke page's checks (plan R08, R08.T14.c, R08.T14.f, R08.T15.a.2 and R08.T15.b.2): `bake.ts`'s
 * `MsSourceTable`, `polarisation.ts`'s `PolarisationTable` and `deck.ts`'s `DeckTables` shapes, with
 * the table below a deck (`BelowDeckTable`), on the bake's grid, with values set so that the packed tables (`thick/read.ts`, `thick/deckRead.ts`)
 * follow a pattern in every axis, without running a solve. The values are a test's, not a world's.
 */

import type { AtmosphereMedium } from "../view/atmosphere/medium";
import type { TablesBand } from "../view/atmosphere/tables";
import type { MsSourceSlot, MsSourceTable } from "../view/atmosphere/thick/bake";
import type { BelowDeckTable, DeckSlot, DeckTables } from "../view/atmosphere/thick/deck";
import {
  BELOW_DECK_ALTITUDES,
  BELOW_DECK_MODES,
  BELOW_DECK_SUN_COSINES,
  BELOW_DECK_TRAVEL_COSINES,
  DECK_TABLE_MODES,
  DECK_TABLE_PHASE_ENTRIES,
  DECK_TABLE_VIEW_COSINES,
} from "../view/atmosphere/thick/deckRead";
import type { PolarisationSlot, PolarisationTable } from "../view/atmosphere/thick/polarisation";
import {
  THICK_ALTITUDES,
  THICK_CORRECTION_MODES,
  THICK_SOURCE_MODES,
  THICK_SUN_COSINES,
  THICK_TRAVEL_COSINES,
  thickSunCosineAt,
} from "../view/atmosphere/thick/read";

/** J⁰ and G at a point of the pattern, per channel, sr⁻¹ per unit of scattering. */
export interface ThickPatternPoint {
  readonly level: readonly [number, number, number];
  readonly slope: readonly [number, number, number];
}

/**
 * The pattern's J⁰ and G at altitude `a`, sun cosine `s`, travel cosine `v`, band `b` and light slot
 * `k`: smooth in each, J⁰ in [0.01, 0.07] sr⁻¹ (of Ψ_ms's order), G a fifth of it with either sign.
 */
export function thickPatternAt(
  a: number,
  s: number,
  v: number,
  b: number,
  k: number,
): ThickPatternPoint {
  const base = (c: number): number =>
    0.04 +
    0.012 * Math.cos(0.17 * a + 0.3 * c + 0.4 * b) +
    0.01 * Math.sin(0.21 * s + 0.5 * k) +
    0.008 * Math.cos(0.37 * v + 0.2 * c);
  const slope = (c: number): number => 0.2 * base(c) * Math.sin(0.29 * v + 0.13 * s - 0.4 * c + b);
  return { level: [base(0), base(1), base(2)], slope: [slope(0), slope(1), slope(2)] };
}

/**
 * The pattern's grid: altitudes evenly from the ground to the medium's top, and the bake's cosines,
 * its sun cosines below the horizontal among them (R08.T14.d.2).
 */
function patternGrid(medium: AtmosphereMedium): {
  readonly altitudesM: Float64Array;
  readonly sunCosines: Float64Array;
  readonly travelCosines: Float64Array;
} {
  const top = medium.topHeightM;
  return {
    altitudesM: Float64Array.from(
      { length: THICK_ALTITUDES },
      (_, a) => (top * a) / (THICK_ALTITUDES - 1),
    ),
    sunCosines: Float64Array.from({ length: THICK_SUN_COSINES }, (_, s) => thickSunCosineAt(s)),
    travelCosines: Float64Array.from(
      { length: THICK_TRAVEL_COSINES },
      (_, v) => (v + 0.5) / (THICK_TRAVEL_COSINES / 2) - 1,
    ),
  };
}

/**
 * A bake whose packed J⁰ and G are {@link thickPatternAt}'s and whose m = 2 is 0, one table a band
 * of `bands` (a tables plan's, in its order), with `slots` light slots, on the bake's grid:
 * altitudes evenly from the ground to `medium`'s top, the table's 46 μ₀ (`thick/read.ts`'s
 * `thickSunCosineAt`, the rows below the horizontal with the same pattern as those above) and
 * μ_v = (v + ½) ÷ 8 − 1.
 * Its J is in sr⁻¹ per unit of the kernels' scattering, as the bake's (`bake.ts`'s `MsSourceSlot`),
 * so that the packed table is the pattern over any medium.
 */
export function patternedThickBake(
  bands: ReadonlyArray<TablesBand>,
  medium: AtmosphereMedium,
  slots: number,
): MsSourceTable[] {
  const { altitudesM, sunCosines, travelCosines } = patternGrid(medium);
  return bands.map((band, b): MsSourceTable => {
    const slotTables = Array.from({ length: slots }, (_, k): MsSourceSlot => {
      const values = new Float32Array(
        THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * THICK_SOURCE_MODES * 3,
      );
      for (let a = 0; a < THICK_ALTITUDES; a += 1) {
        for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
          for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
            const point = thickPatternAt(a, s, v, b, k);
            const mu0 = sunCosines[s] ?? Number.NaN;
            const muV = travelCosines[v] ?? Number.NaN;
            const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
            const at =
              ((a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * THICK_SOURCE_MODES * 3;
            for (let c = 0; c < 3; c += 1) {
              values[at + c] = point.level[c] ?? Number.NaN;
              values[at + 3 + c] = (point.slope[c] ?? Number.NaN) * sines;
            }
          }
        }
      }
      return {
        values,
        bins: [],
        solves: 0,
        doubleForwardShare: 0,
        flooredSlantFactors: 0,
        largestBeamDeficit: 0,
      };
    });
    return {
      band: {
        gravityRatio: band.gravityRatio,
        latitudeRad: band.latitudeRad,
        bottomRadiusM: band.bottomRadiusM,
        coefficientFactor: band.coefficientFactor,
      },
      altitudesM,
      sunCosines,
      travelCosines,
      modes: THICK_SOURCE_MODES,
      verticalOpticalDepth: new Float64Array(15),
      slots: slotTables,
    };
  });
}

/** ΔJ_0, ΔJ_1 ÷ (sin θ_v sin θ₀) and ΔJ_2 ÷ (sin θ_v sin θ₀)² at a point, per channel, sr⁻¹. */
export interface ThickCorrectionPoint {
  readonly level: readonly [number, number, number];
  readonly slope: readonly [number, number, number];
  readonly curve: readonly [number, number, number];
}

/**
 * The correction's pattern at altitude `a`, sun cosine `s`, travel cosine `v`, band `b` and light slot
 * `k` (R08.T14.f): smooth in each and of either sign, ΔJ_0 at most a tenth of
 * {@link thickPatternAt}'s J⁰, ΔJ_1's quotient 8% of it and ΔJ_2's 5%.
 */
export function thickCorrectionPatternAt(
  a: number,
  s: number,
  v: number,
  b: number,
  k: number,
): ThickCorrectionPoint {
  const { level: base } = thickPatternAt(a, s, v, b, k);
  const at = (c: number): number => base[c] ?? Number.NaN;
  const level = (c: number): number =>
    0.1 * at(c) * Math.sin(0.23 * a - 0.31 * s + 0.17 * v + 0.5 * c + 0.7 * b + 0.3 * k);
  const slope = (c: number): number =>
    0.08 * at(c) * Math.cos(0.19 * a + 0.27 * v - 0.4 * c + b - 0.2 * k);
  const curve = (c: number): number =>
    0.05 * at(c) * Math.sin(0.33 * s + 0.41 * v + 0.6 * c - 0.5 * b - k);
  return {
    level: [level(0), level(1), level(2)],
    slope: [slope(0), slope(1), slope(2)],
    curve: [curve(0), curve(1), curve(2)],
  };
}

/**
 * A polarisation correction whose ΔJ is {@link thickCorrectionPatternAt}'s (R08.T14.f), one table a
 * band of `bands`, with `slots` light slots, on {@link patternedThickBake}'s grid for `medium`. ΔJ is
 * per unit of the whole σ_s, so that over a medium with no forward delta the packed correction is
 * the pattern itself.
 */
export function patternedCorrection(
  bands: ReadonlyArray<TablesBand>,
  medium: AtmosphereMedium,
  slots: number,
): PolarisationTable[] {
  const { altitudesM, sunCosines, travelCosines } = patternGrid(medium);
  return bands.map((band, b): PolarisationTable => {
    const slotTables = Array.from({ length: slots }, (_, k): PolarisationSlot => {
      const values = new Float32Array(
        THICK_ALTITUDES * THICK_SUN_COSINES * THICK_TRAVEL_COSINES * THICK_CORRECTION_MODES * 3,
      );
      for (let a = 0; a < THICK_ALTITUDES; a += 1) {
        for (let s = 0; s < THICK_SUN_COSINES; s += 1) {
          for (let v = 0; v < THICK_TRAVEL_COSINES; v += 1) {
            const point = thickCorrectionPatternAt(a, s, v, b, k);
            const mu0 = sunCosines[s] ?? Number.NaN;
            const muV = travelCosines[v] ?? Number.NaN;
            const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
            const at =
              ((a * THICK_SUN_COSINES + s) * THICK_TRAVEL_COSINES + v) * THICK_CORRECTION_MODES * 3;
            for (let c = 0; c < 3; c += 1) {
              values[at + c] = point.level[c] ?? Number.NaN;
              values[at + 3 + c] = (point.slope[c] ?? Number.NaN) * sines;
              values[at + 6 + c] = (point.curve[c] ?? Number.NaN) * sines * sines;
            }
          }
        }
      }
      return {
        values,
        solves: 0,
        complexPairs: 0,
        imaginaryShares: [Number.POSITIVE_INFINITY, 0],
        nonsymmetricLayers: 0,
      };
    });
    return {
      band: {
        gravityRatio: band.gravityRatio,
        latitudeRad: band.latitudeRad,
        bottomRadiusM: band.bottomRadiusM,
        coefficientFactor: band.coefficientFactor,
      },
      altitudesM,
      sunCosines,
      travelCosines,
      modes: THICK_CORRECTION_MODES,
      slots: slotTables,
    };
  });
}

/**
 * The deck's pattern at sun cosine row `s`, view cosine `v`, band `b` and light slot `k` (R08.T15.a.2),
 * per channel: each mode's quotient over sinᵐ θ_v sinᵐ θ₀ (`thick/deckRead.ts`'s stored value) of
 * either sign, falling with m, the beam's ρ − ρ_ss about 0.1 at m = 0, its βρ 0.8 of it, the sky-lit
 * D about 0.02 sr⁻¹ at m = 0, and G and G_β about 0.2 sr.
 */
export function deckPatternAt(
  s: number,
  v: number,
  b: number,
  k: number,
  m: number,
  c: number,
): { readonly reflectance: number; readonly diffuse: number; readonly single: number } {
  const wave = Math.cos(0.23 * s + 0.41 * v + 0.37 * m + 0.3 * c + 0.5 * b + 0.2 * k);
  const fall = 0.5 ** m;
  return {
    reflectance: (m === 0 ? 0.1 + 0.03 * wave : 0.04 * wave) * fall,
    diffuse: (m === 0 ? 0.02 + 0.005 * wave : 0.006 * wave) * fall,
    single: 0.2 + 0.05 * Math.sin(0.19 * s + 0.31 * v + 0.4 * c + b - 0.3 * k),
  };
}

/** The deck's phase's pattern, sr⁻¹, at entry `i` of its 256 and channel `c`: smooth, forward-peaked. */
export function deckPhasePatternAt(i: number, c: number, b: number, k: number): number {
  const u = i / (DECK_TABLE_PHASE_ENTRIES - 1);
  return 0.03 + 0.6 * (1 - u) ** 3 + 0.02 * Math.cos(9 * u + 0.4 * c + 0.3 * b + 0.2 * k);
}

/**
 * {@link patternedCorrection}'s correction over a deck (R08.T15.a.2), its altitudes from the split
 * at `deckAltitudeM` up, on the J's altitudes to the bit, as the deck's bake lays it: one table a
 * band of `bands`, with `slots` light slots, over `medium`, the upper medium as the kernels read it.
 */
export function patternedDeckCorrection(
  bands: ReadonlyArray<TablesBand>,
  medium: AtmosphereMedium,
  slots: number,
  deckAltitudeM: number,
): PolarisationTable[] {
  return patternedCorrection(bands, medium, slots).map((band): PolarisationTable => ({
    band: band.band,
    altitudesM: band.altitudesM.map((h) => h + deckAltitudeM),
    sunCosines: band.sunCosines,
    travelCosines: band.travelCosines,
    modes: band.modes,
    slots: band.slots,
  }));
}

/**
 * The vertical optical depth from the top of {@link patternedBelowDeck}'s beam, per channel, at h\*
 * `heightM` beneath a split at `deckAltitudeM`, in band `b`: linear in h\*, 0.2 at the split and
 * 3.2–3.6 at the ground by channel, so that its beam is exact for the read's geometric step.
 */
export function belowPatternDepth(
  heightM: number,
  deckAltitudeM: number,
  b: number,
  c: number,
): number {
  return 0.2 + (3 + 0.2 * c + 0.1 * b) * (1 - heightM / deckAltitudeM);
}

/**
 * The table below's pattern at altitude row `a`, sun row `s`, travel cosine `v`, band `b`, light
 * slot `k`, mode `m` and channel `c` (R08.T15.b.2): each mode's quotient over sinᵐ θ_v sinᵐ θ₀
 * (`thick/deckRead.ts`'s stored value) of either sign past m = 0, falling with m, I⁰ about
 * 0.02 sr⁻¹, and the irradiance about 0.3 μ₀.
 */
export function belowPatternAt(
  a: number,
  s: number,
  v: number,
  b: number,
  k: number,
  m: number,
  c: number,
): { readonly radiance: number; readonly irradiance: number } {
  const wave = Math.cos(0.29 * a + 0.23 * s + 0.41 * v + 0.37 * m + 0.3 * c + 0.5 * b + 0.2 * k);
  const mu0 = (s + 0.5) / BELOW_DECK_SUN_COSINES;
  return {
    radiance: (m === 0 ? 0.02 + 0.005 * wave : 0.006 * wave) * 0.5 ** m,
    irradiance: mu0 * (0.3 + 0.05 * Math.sin(0.31 * a + 0.4 * c + b - 0.3 * k)),
  };
}

/**
 * A table below a deck split at `deckAltitudeM` over a surface whose packed values follow
 * {@link belowPatternAt}, in band `b` with `slots` light slots (R08.T15.b.2): 17 altitudes evenly
 * from the ground to the split, the daylit 32 sun cosines and the 16 travel cosines, and the beam
 * e^(−τ ÷ μ₀) of {@link belowPatternDepth}, plane-parallel.
 */
export function patternedBelowDeck(
  b: number,
  slots: number,
  deckAltitudeM: number,
): BelowDeckTable {
  const altitudesM = Float64Array.from(
    { length: BELOW_DECK_ALTITUDES },
    (_, a) => (a * deckAltitudeM) / (BELOW_DECK_ALTITUDES - 1),
  );
  const sunCosines = Float64Array.from(
    { length: BELOW_DECK_SUN_COSINES },
    (_, s) => (s + 0.5) / BELOW_DECK_SUN_COSINES,
  );
  const travelCosines = Float64Array.from(
    { length: BELOW_DECK_TRAVEL_COSINES },
    (_, v) => (v + 0.5) / (BELOW_DECK_TRAVEL_COSINES / 2) - 1,
  );
  const suns = BELOW_DECK_SUN_COSINES;
  const views = BELOW_DECK_TRAVEL_COSINES;
  const modes = BELOW_DECK_MODES;
  const cells = BELOW_DECK_ALTITUDES * suns;
  return {
    altitudesM,
    sunCosines,
    travelCosines,
    modes,
    slots: Array.from({ length: slots }, (_, k) => {
      const radiance = new Float32Array(cells * views * modes * 3);
      const irradiance = new Float32Array(cells * 3);
      const direct = new Float32Array(cells * 3);
      for (let a = 0; a < BELOW_DECK_ALTITUDES; a += 1) {
        for (let s = 0; s < suns; s += 1) {
          const mu0 = sunCosines[s] ?? Number.NaN;
          for (let c = 0; c < 3; c += 1) {
            const at = (a * suns + s) * 3 + c;
            irradiance[at] = belowPatternAt(a, s, 0, b, k, 0, c).irradiance;
            direct[at] = Math.exp(
              -belowPatternDepth(altitudesM[a] ?? Number.NaN, deckAltitudeM, b, c) / mu0,
            );
            for (let v = 0; v < views; v += 1) {
              const muV = travelCosines[v] ?? Number.NaN;
              const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
              for (let m = 0; m < modes; m += 1) {
                radiance[(((a * suns + s) * views + v) * modes + m) * 3 + c] =
                  belowPatternAt(a, s, v, b, k, m, c).radiance * sines ** m;
              }
            }
          }
        }
      }
      return { radiance, irradiance, direct };
    }),
  };
}

/**
 * Deck tables whose packed values follow {@link deckPatternAt} and {@link deckPhasePatternAt}
 * (R08.T15.a.2), one a band of `bands` (a tables plan's, in its order), with `slots` light slots, on
 * the deck's grid, the split at `deckAltitudeM` over the body's datum: the J above it
 * {@link patternedThickBake}'s over `medium`, the upper medium as the kernels read it, its altitudes
 * from the split up. Its beam's rows below the horizontal hold 0, as a bake's do. With `below`, a
 * deck over a surface's, each band holds {@link patternedBelowDeck}'s table (R08.T15.b.2); else
 * none, as an envelope's.
 */
export function patternedDeckTables(
  bands: ReadonlyArray<TablesBand>,
  medium: AtmosphereMedium,
  slots: number,
  deckAltitudeM: number,
  below = false,
): DeckTables[] {
  const above = patternedThickBake(bands, medium, slots);
  const { sunCosines } = patternGrid(medium);
  const viewCosines = Float64Array.from(
    { length: DECK_TABLE_VIEW_COSINES },
    (_, v) => (v + 0.5) / DECK_TABLE_VIEW_COSINES,
  );
  const suns = THICK_SUN_COSINES;
  const views = DECK_TABLE_VIEW_COSINES;
  const modes = DECK_TABLE_MODES;
  // Every mode of the sky-lit radiance, past a bake's 3, so that the read's every texel of D is held.
  const diffuseModes = DECK_TABLE_MODES;
  return bands.map((_, b): DeckTables => {
    const table = above[b];
    if (table === undefined) {
      throw new Error(`the pattern has a band ${b}`);
    }
    const deckSlots = Array.from({ length: slots }, (__, k): DeckSlot => {
      const reflectance = new Float32Array(suns * views * modes * 3);
      const beamReflection = new Float32Array(suns * views * modes * 3);
      const diffuse = new Float32Array(suns * views * diffuseModes * 3);
      const singleGeometry = new Float32Array(suns * views * 3);
      const singleBeamGeometry = new Float32Array(suns * views * 3);
      const phase = Float32Array.from({ length: DECK_TABLE_PHASE_ENTRIES * 3 }, (___, i) =>
        deckPhasePatternAt(Math.floor(i / 3), i % 3, b, k),
      );
      for (let s = 0; s < suns; s += 1) {
        const mu0 = sunCosines[s] ?? Number.NaN;
        const lit = mu0 > 0;
        for (let v = 0; v < views; v += 1) {
          const muV = viewCosines[v] ?? Number.NaN;
          const sines = Math.sqrt((1 - mu0 * mu0) * (1 - muV * muV));
          const point = s * views + v;
          for (let c = 0; c < 3; c += 1) {
            for (let m = 0; m < modes; m += 1) {
              const pattern = deckPatternAt(s, v, b, k, m, c);
              const at = (point * modes + m) * 3 + c;
              const power = sines ** m;
              reflectance[at] = lit ? pattern.reflectance * power : 0;
              beamReflection[at] = lit ? 0.8 * pattern.reflectance * power : 0;
              if (m < diffuseModes) {
                diffuse[(point * diffuseModes + m) * 3 + c] = pattern.diffuse * power;
              }
            }
            const single = deckPatternAt(s, v, b, k, 0, c).single;
            singleGeometry[point * 3 + c] = lit ? single : 0;
            singleBeamGeometry[point * 3 + c] = lit ? 0.7 * single : 0;
          }
        }
      }
      return {
        reflectance,
        diffuse,
        beamReflection,
        singleGeometry,
        singleBeamGeometry,
        phase,
        solves: 0,
      };
    });
    return {
      band: table.band,
      deckAltitudeM,
      floorAltitudeM: 0,
      above: { ...table, altitudesM: table.altitudesM.map((h) => h + deckAltitudeM) },
      sunCosines,
      viewCosines,
      modes,
      diffuseModes,
      deckOpticalDepth: new Float64Array(15),
      slots: deckSlots,
      below: below ? patternedBelowDeck(b, slots, deckAltitudeM) : null,
    };
  });
}

/** {@link leastZerothModes}'s least J⁰: above the horizontal, sr⁻¹, and below it, of the largest. */
export interface ZerothModesLeast {
  readonly above: number;
  readonly belowShare: number;
}

/**
 * A bake's least J⁰ (its m = 0, every channel; `bake.ts`'s `[altitude][μ₀][μ_v][m][RGB]`) over the
 * rows above the horizontal, and over those below it as a share of its largest J⁰ (R08.T14.d.2).
 * Each bin's J⁰ is at least 0, but a channel's is the bins' converted by signed matching functions
 * (`binConversion.ts`), so a near-dark twilight row's steep spectrum can take it a hair below 0:
 * −1.8 × 10⁻⁹ sr⁻¹ in TEST EARTH's blue at μ₀ −0.164, 3 × 10⁻⁸ of its largest, which the read holds
 * at 0.
 */
export function leastZerothModes(
  values: ArrayLike<number>,
  sunCosines: Float64Array,
): ZerothModesLeast {
  const perSun = THICK_TRAVEL_COSINES * THICK_SOURCE_MODES * 3;
  let above = Number.POSITIVE_INFINITY;
  let below = Number.POSITIVE_INFINITY;
  let largest = 0;
  for (let i = 0; i < values.length; i += 1) {
    if (Math.floor(i / 3) % THICK_SOURCE_MODES !== 0) {
      continue;
    }
    const value = values[i] ?? Number.NaN;
    largest = Math.max(largest, value);
    if ((sunCosines[Math.floor(i / perSun) % sunCosines.length] ?? Number.NaN) > 0) {
      above = Math.min(above, value);
    } else {
      below = Math.min(below, value);
    }
  }
  return { above, belowShare: below / largest };
}
