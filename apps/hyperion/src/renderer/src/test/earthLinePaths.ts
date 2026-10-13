/**
 * Earth's slant paths for R08.T4.d's end-to-end test (`decision-r08-t4d-kdist.md`, its tests):
 * O₂ and H₂O from one air mass to the horizon's 35 from the ground and from 3 km, as layers of pressure,
 * temperature and each species' column on each path. A bless test commits them
 * (`earthLinePaths.json`), the reduction tool reads them to take the paths line by line
 * (`scripts/atmosphereData.mjs earth-reference`), and the test reads both.
 */

import type { AbsorberLayer } from "../view/atmosphere/absorbers";
import { type AtmosphereColumn, hydrostaticColumn } from "../view/atmosphere/column";
import { localRadiusM } from "../view/atmosphere/opticalDepth";

/**
 * The column: the US Standard Atmosphere 1976's sea level (101 325 Pa, 288.15 K) and its 6.5 K
 * km⁻¹ troposphere to its 216.65 K tropopause, as Design note 3's radiative-convective profile with
 * β = R_d Γ ÷ g₀ = 287.05 × 0.0065 ÷ 9.806 65 = 0.1903 (U.S. Standard Atmosphere, 1976, NOAA-S/T
 * 76-1562, Table 4), on Earth's mean radius and standard gravity.
 */
export function earthLineColumn(): AtmosphereColumn {
  return hydrostaticColumn({
    surfacePa: 101_325,
    temperature: {
      kind: "radiativeConvective",
      surfaceK: 288.15,
      surfacePa: 101_325,
      beta: (287.05 * 0.0065) / 9.806_65,
      skinK: 216.65,
    },
    meanMolarMassGPerMol: 28.9644,
    referenceGravityMS2: 9.806_65,
    referenceRadiusM: 6_371_008.8,
  });
}

/** O₂'s mole fraction in dry air, 0.209 476 (U.S. Standard Atmosphere, 1976, Table 3). */
export const EARTH_O2_FRACTION = 0.209_476;

/**
 * The water layer: AFGL's US Standard profile's precipitable water, 1.42 g cm⁻² (G. P. Anderson et
 * al., AFGL-TR-86-0110, 1986, Table 1), as a constant mixing ratio from the ground to 300 hPa: a
 * stated fixture, not Earth's profile.
 */
export const EARTH_H2O_COLUMN_PER_M2 = (14.2 / 0.018_015_28) * 6.022_140_76e23;

/** The top of the water layer, Pa. */
export const EARTH_H2O_TOP_PA = 30_000;

/** Earth's line absorbers' layers on the column: O₂ well mixed, and the water layer. */
export function earthLineLayers(column: AtmosphereColumn): ReadonlyArray<AbsorberLayer> {
  const airColumn = column.surfaceNumberDensityPerM3 * columnIntegralM(column);
  return [
    { species: "O2", columnPerM2: EARTH_O2_FRACTION * airColumn, basePa: 101_325, topPa: 0 },
    {
      species: "H2O",
      columnPerM2: EARTH_H2O_COLUMN_PER_M2,
      basePa: 101_325,
      topPa: EARTH_H2O_TOP_PA,
    },
  ];
}

/** ∫ρ dz over the whole column, m, by the trapezoid on its levels. */
function columnIntegralM(column: AtmosphereColumn): number {
  let sum = 0;
  const z = column.altitudesM;
  const rho = column.density.relative;
  for (let i = 1; i < z.length; i += 1) {
    sum += 0.5 * ((rho[i] ?? 0) + (rho[i - 1] ?? 0)) * ((z[i] ?? 0) - (z[i - 1] ?? 0));
  }
  return sum;
}

/** The paths' starting heights, m. */
export const PATH_HEIGHTS_M: ReadonlyArray<number> = [0, 3_000];

/**
 * The paths' zenith angles, degrees: those of plane-parallel air masses 1, 1.5, 2, 3, 5, 10 and 20,
 * and the horizon. On the fixture's spherical, refraction-free ray the horizon from the ground is
 * 35 air masses (Kasten and Young 1989, Appl. Opt. 28, 4735, give 38.09 with refraction).
 */
export const PATH_ZENITH_DEG: ReadonlyArray<number> = [1, 1.5, 2, 3, 5, 10, 20]
  .map((m) => (Math.acos(1 / m) * 180) / Math.PI)
  .concat([90]);

/** The layers' count, equal in ln p from the start's pressure to {@link TOP_PRESSURE_PA}. */
export const PATH_LAYERS = 80;

/** The paths' top, Pa: 1, above which O₂'s and H₂O's lines hold under 10⁻⁵ of a slant τ. */
export const TOP_PRESSURE_PA = 1;

/** One layer of a start height's paths. */
export interface PathLayer {
  /** Its absorber-weighted (Curtis–Godson) pressure, Pa, and temperature, K, for O₂. */
  readonly pressurePa: number;
  readonly temperatureK: number;
  /** O₂'s and H₂O's mole fractions in it. */
  readonly o2Fraction: number;
  readonly h2oFraction: number;
  /** Each path's O₂ and H₂O columns through it, m⁻², in {@link PATH_ZENITH_DEG}'s order. */
  readonly o2PerM2: ReadonlyArray<number>;
  readonly h2oPerM2: ReadonlyArray<number>;
  /**
   * Each path's pair columns through it for H₂O's continuum, m⁻⁵: ∫ n_H₂O² ds (`H2O+H2O`) and
   * ∫ n_H₂O (n − n_H₂O) ds (`H2O+air`).
   */
  readonly h2oSelfPairPerM5: ReadonlyArray<number>;
  readonly h2oAirPairPerM5: ReadonlyArray<number>;
}

/** A start height's layers and paths. */
export interface EarthLinePaths {
  readonly startHeightM: number;
  readonly zenithDeg: ReadonlyArray<number>;
  /** Each path's relative air mass: its O₂ column over the vertical one from the start. */
  readonly airMasses: ReadonlyArray<number>;
  readonly layers: ReadonlyArray<PathLayer>;
}

/** p and T at a height on the column: linear in ln p and in T between its levels. */
function stateAt(
  column: AtmosphereColumn,
  heightM: number,
): { readonly p: number; readonly t: number } {
  const z = column.altitudesM;
  let i = 0;
  while (i < z.length - 2 && (z[i + 1] ?? Infinity) < heightM) {
    i += 1;
  }
  const z0 = z[i] ?? 0;
  const z1 = z[i + 1] ?? z0;
  const f = Math.min(Math.max((heightM - z0) / (z1 - z0), 0), 1);
  const p0 = column.pressuresPa[i] ?? Number.NaN;
  const p1 = column.pressuresPa[i + 1] ?? p0;
  const t0 = column.temperaturesK[i] ?? Number.NaN;
  const t1 = column.temperaturesK[i + 1] ?? t0;
  return { p: Math.exp(Math.log(p0) + (Math.log(p1) - Math.log(p0)) * f), t: t0 + (t1 - t0) * f };
}

/** The height at a pressure on the column, m: linear in ln p between its levels. */
function heightAt(column: AtmosphereColumn, pressurePa: number): number {
  const p = column.pressuresPa;
  let i = 0;
  while (i < p.length - 2 && (p[i + 1] ?? 0) > pressurePa) {
    i += 1;
  }
  const a = Math.log(p[i] ?? Number.NaN);
  const b = Math.log(p[i + 1] ?? Number.NaN);
  const f = Math.min(Math.max((Math.log(pressurePa) - a) / (b - a), 0), 1);
  return (
    (column.altitudesM[i] ?? 0) +
    ((column.altitudesM[i + 1] ?? 0) - (column.altitudesM[i] ?? 0)) * f
  );
}

/** Steps per layer along each path, for its columns. */
const STEPS_PER_LAYER = 400;

/**
 * Earth's paths from a start height: {@link PATH_LAYERS} layers equal in ln p, each with its
 * O₂-weighted pressure and temperature and each path's columns through it, by the midpoint rule
 * along the spherical ray at {@link STEPS_PER_LAYER} steps a layer.
 */
export function earthLinePaths(column: AtmosphereColumn, startHeightM: number): EarthLinePaths {
  const [o2, h2o] = earthLineLayers(column);
  const n0 = column.surfaceNumberDensityPerM3;
  const radius = column.referenceRadiusM;
  const h2oTopM = heightAt(column, EARTH_H2O_TOP_PA);
  // The water layer's number density over air's: its column over the air's below 300 hPa.
  let airBelow = 0;
  const z = column.altitudesM;
  for (let i = 1; i < z.length && (z[i - 1] ?? 0) < h2oTopM; i += 1) {
    const top = Math.min(z[i] ?? 0, h2oTopM);
    const rho0 = column.density.relative[i - 1] ?? 0;
    const rho1 = column.density.relative[i] ?? 0;
    const f = (top - (z[i - 1] ?? 0)) / ((z[i] ?? 0) - (z[i - 1] ?? 0));
    airBelow += 0.5 * (rho0 + (rho0 + (rho1 - rho0) * f)) * (top - (z[i - 1] ?? 0));
  }
  const h2oFraction = (h2o?.columnPerM2 ?? 0) / (n0 * airBelow);
  const o2Fraction = (o2?.columnPerM2 ?? 0) / (n0 * columnIntegralM(column));
  const startP = stateAt(column, startHeightM).p;
  const bounds = Array.from({ length: PATH_LAYERS + 1 }, (_, k) =>
    heightAt(column, startP * (TOP_PRESSURE_PA / startP) ** (k / PATH_LAYERS)),
  );
  const densityAt = (h: number): number =>
    stateAt(column, h).p / (1.380_649e-23 * stateAt(column, h).t);
  const layers: PathLayer[] = [];
  const vertical: number[] = [];
  for (let k = 0; k < PATH_LAYERS; k += 1) {
    const bottom = bounds[k] ?? 0;
    const top = bounds[k + 1] ?? bottom;
    // Weighted by number density, the Curtis–Godson mean the client's read takes for a gas mixed in the layer.
    let weight = 0;
    let pSum = 0;
    let tSum = 0;
    const dz = (top - bottom) / STEPS_PER_LAYER;
    for (let s = 0; s < STEPS_PER_LAYER; s += 1) {
      const h = bottom + (s + 0.5) * dz;
      const state = stateAt(column, h);
      const n = state.p / (1.380_649e-23 * state.t);
      weight += n * dz;
      pSum += n * state.p * dz;
      tSum += n * state.t * dz;
    }
    vertical.push(o2Fraction * weight);
    const o2PerM2: number[] = [];
    const h2oPerM2: number[] = [];
    const h2oSelfPairPerM5: number[] = [];
    const h2oAirPairPerM5: number[] = [];
    for (const zenith of PATH_ZENITH_DEG) {
      const mu = Math.cos((zenith * Math.PI) / 180);
      const r0 = radius + startHeightM;
      // Along the spherical ray, not the plane-parallel secant, which grows without bound at the horizon.
      const toRadius = (r: number): number =>
        -r0 * mu + Math.sqrt(Math.max(r0 * r0 * (mu * mu - 1) + r * r, 0));
      const t0 = toRadius(radius + bottom);
      const t1 = toRadius(radius + top);
      const dt = (t1 - t0) / STEPS_PER_LAYER;
      let n = 0;
      let wet = 0;
      let wetSquared = 0;
      for (let s = 0; s < STEPS_PER_LAYER; s += 1) {
        const t = t0 + (s + 0.5) * dt;
        const h = localRadiusM(r0, mu, t) - radius;
        const local = densityAt(h);
        n += local * dt;
        if (h < h2oTopM) {
          wet += local * dt;
          wetSquared += local * local * dt;
        }
      }
      o2PerM2.push(o2Fraction * n);
      h2oPerM2.push(h2oFraction * wet);
      h2oSelfPairPerM5.push(h2oFraction * h2oFraction * wetSquared);
      h2oAirPairPerM5.push(h2oFraction * (1 - h2oFraction) * wetSquared);
    }
    layers.push({
      pressurePa: pSum / weight,
      temperatureK: tSum / weight,
      o2Fraction,
      h2oFraction: bottom < h2oTopM ? h2oFraction : 0,
      o2PerM2,
      h2oPerM2,
      h2oSelfPairPerM5,
      h2oAirPairPerM5,
    });
  }
  const verticalColumn = vertical.reduce((a, b) => a + b, 0);
  const airMasses = PATH_ZENITH_DEG.map(
    (_, p) => layers.reduce((sum, layer) => sum + (layer.o2PerM2[p] ?? 0), 0) / verticalColumn,
  );
  return { startHeightM, zenithDeg: PATH_ZENITH_DEG, airMasses, layers };
}
