import { describe, expect, it } from "vitest";

import {
  CLIMATE_WIND_HEIGHT_M,
  COX_MUNK_HEIGHT_M,
  EARTH_SEA,
  equivalentWind,
  generationThreshold,
  minimumPhaseSpeedMps,
  type SeaAir,
  type SeaLiquid,
  seaState,
  type SeaStateInput,
  SLOPE_VARIANCE_FLOOR,
  spectralDensity,
  windAtHeight,
} from "./seaState";

/** An Earth sea at a wind at Cox and Munk's height, open or over a basin. */
function earthSea(windMps: number, heightM: number, fetchM: number | null = null): SeaStateInput {
  return {
    windMps,
    windHeightM: heightM,
    gravityMps2: EARTH_SEA.gravityMps2,
    liquid: EARTH_SEA.liquid,
    air: EARTH_SEA.air,
    fetchM,
  };
}

/**
 * A Titan-like sea: liquid methane at 93.65 K (`testing/liquids.rs`' 447.6 kg m⁻³; γ about
 * 0.0177 N/m and μ about 0.19 mPa s, Mulero et al. 2012 and Younglove and Ely 1987 as the ruling
 * cites them) under 1.47 bar of N₂ at 93.7 K (5.3 kg m⁻³), at 1.352 m s⁻².
 */
const TITAN_LIQUID: SeaLiquid = {
  densityKgM3: 447.6,
  surfaceTensionNPerM: 0.0177,
  viscosityPaS: 1.9e-4,
};
const TITAN_AIR: SeaAir = { densityKgM3: 5.3, kinematicViscosityM2s: 1.2e-6 };
const TITAN_GRAVITY_MPS2 = 1.352;

/** ∫ S(ω) dω over the sea's spectrum, by the midpoint rule in ln ω. */
function zerothMoment(state: ReturnType<typeof seaState>): number {
  const low = Math.log(state.peakAngularFrequencyRadS / 10);
  const high = Math.log(state.peakAngularFrequencyRadS * 50);
  const steps = 20_000;
  const step = (high - low) / steps;
  let sum = 0;
  for (let i = 0; i < steps; i++) {
    const omega = Math.exp(low + (i + 0.5) * step);
    sum += spectralDensity(state, omega) * omega * step;
  }
  return sum;
}

describe("the winds", () => {
  it("carries the 10 m wind to Cox and Munk's and Pierson and Moskowitz's heights", () => {
    expect(windAtHeight(1, 10, 12.5)).toBeCloseTo(1.02, 2);
    expect(windAtHeight(1, 10, 19.5)).toBeCloseTo(1.06, 2);
    expect(windAtHeight(7, 12.5, 12.5)).toBe(7);
  });

  it("is the identity at Earth's parameters", () => {
    for (const u of [0, 0.3, 3, 12, 30]) {
      expect(equivalentWind(u, EARTH_SEA.gravityMps2, EARTH_SEA.liquid, EARTH_SEA.air)).toBe(u);
    }
  });

  it("gives water its minimum phase speed of 0.231 m/s", () => {
    const earth = EARTH_SEA.liquid;
    expect(
      minimumPhaseSpeedMps(
        EARTH_SEA.gravityMps2,
        earth.surfaceTensionNPerM ?? 0,
        earth.densityKgM3,
      ),
    ).toBeCloseTo(0.231, 3);
  });

  it("takes Earth's minimum phase speed where the surface tension is not modelled", () => {
    const untensioned: SeaLiquid = { ...TITAN_LIQUID, surfaceTensionNPerM: null };
    const ratio = TITAN_AIR.densityKgM3 / untensioned.densityKgM3;
    const earthRatio = EARTH_SEA.air.densityKgM3 / EARTH_SEA.liquid.densityKgM3;
    expect(equivalentWind(1, TITAN_GRAVITY_MPS2, untensioned, TITAN_AIR)).toBeCloseTo(
      Math.sqrt(ratio / earthRatio),
      12,
    );
  });

  it("leaves an airless sea still", () => {
    const airless: SeaAir = { densityKgM3: 0, kinematicViscosityM2s: 1e-5 };
    expect(equivalentWind(10, 1.6, EARTH_SEA.liquid, airless)).toBe(0);
    const state = seaState({ ...earthSea(30, 10), air: airless });
    expect(state.glassy).toBe(true);
    expect(state.slopeVariance).toEqual({
      upwind: SLOPE_VARIANCE_FLOOR,
      crosswind: SLOPE_VARIANCE_FLOOR,
    });
  });
});

describe("Cox and Munk's slopes", () => {
  it("meets their clean-surface laws at three winds", () => {
    for (const u of [2, 8, 14]) {
      const state = seaState(earthSea(u, COX_MUNK_HEIGHT_M));
      expect(state.coxMunkWindMps).toBeCloseTo(u, 12);
      expect(state.slopeVariance.upwind).toBeCloseTo(3.16e-3 * u, 12);
      expect(state.slopeVariance.crosswind).toBeCloseTo(0.003 + 1.92e-3 * u, 12);
      // Their separate fits sum to 0.003 + 5.08 × 10⁻³ U, their fitted total 0.003 + 5.12 ×
      // 10⁻³ U, within the total's stated ±0.004.
      expect(
        Math.abs(
          state.slopeVariance.upwind + state.slopeVariance.crosswind - (0.003 + 5.12e-3 * u),
        ),
      ).toBeLessThan(0.004);
    }
  });

  it("holds the wind to 20 m/s and says where it extrapolates past 14", () => {
    const strong = seaState(earthSea(30, COX_MUNK_HEIGHT_M));
    expect(strong.coxMunkWindMps).toBe(20);
    expect(strong.extrapolated).toBe(true);
    expect(seaState(earthSea(13, COX_MUNK_HEIGHT_M)).extrapolated).toBe(false);
  });
});

describe("the waves' height and peak", () => {
  it("reaches Pierson and Moskowitz's H_s and ω_p over the open sea", () => {
    const u10 = 12;
    const state = seaState(earthSea(u10, CLIMATE_WIND_HEIGHT_M));
    const u19 = windAtHeight(u10, 10, 19.5);
    const g = EARTH_SEA.gravityMps2;
    expect(state.spectrum).toBe("pierson-moskowitz");
    expect(state.significantHeightM / ((u19 * u19) / g)).toBeCloseTo(0.21, 2);
    expect(state.peakAngularFrequencyRadS / (g / u19)).toBeCloseTo(0.877, 3);
  });

  it("caps a basin's height by its fetch and reaches Pierson and Moskowitz at full development", () => {
    const open = seaState(earthSea(10, 10));
    let last = 0;
    for (const fetchM of [1e3, 1e4, 1e5]) {
      const basin = seaState(earthSea(10, 10, fetchM));
      expect(basin.fetchLimited).toBe(true);
      expect(basin.spectrum).toBe("jonswap");
      expect(basin.significantHeightM).toBeLessThan(open.significantHeightM);
      expect(basin.significantHeightM).toBeGreaterThan(last);
      expect(basin.peakAngularFrequencyRadS).toBeGreaterThanOrEqual(open.peakAngularFrequencyRadS);
      last = basin.significantHeightM;
    }
    const g = EARTH_SEA.gravityMps2;
    const short = seaState(earthSea(10, 10, 1e4));
    expect((g * short.significantHeightM) / 100).toBeCloseTo(
      1.6e-3 * Math.sqrt((g * 1e4) / 100),
      12,
    );
    const developed = seaState(earthSea(10, 10, 1e8));
    expect(developed.fetchLimited).toBe(false);
    expect(developed.significantHeightM).toBe(open.significantHeightM);
  });

  it("normalises each spectrum to its significant height", () => {
    for (const state of [seaState(earthSea(10, 10)), seaState(earthSea(10, 10, 1e4))]) {
      expect(4 * Math.sqrt(zerothMoment(state))).toBeCloseTo(state.significantHeightM, 4);
    }
  });

  it("covers Monahan and O'Muircheartaigh's share of the sea in whitecaps", () => {
    const state = seaState(earthSea(10, 10));
    expect(state.whitecapFraction).toBeCloseTo(3.84e-6 * 10 ** 3.41, 12);
  });
});

describe("the generation threshold", () => {
  it("grows Earth's waves from a light air", () => {
    const threshold = generationThreshold(EARTH_SEA.gravityMps2, EARTH_SEA.liquid, EARTH_SEA.air);
    expect(threshold).toBeGreaterThan(1);
    expect(threshold).toBeLessThan(2);
  });

  it("puts a Titan methane sea's threshold near Hayes et al.'s 0.4–0.7 m/s", () => {
    const threshold = generationThreshold(TITAN_GRAVITY_MPS2, TITAN_LIQUID, TITAN_AIR);
    expect(threshold).toBeGreaterThan(0.3);
    expect(threshold).toBeLessThan(0.7);
  });

  it("leaves a Titan-like sea at 0.3 m/s glassy, a mirror at the floor", () => {
    const state = seaState({
      windMps: 0.3,
      windHeightM: 10,
      gravityMps2: TITAN_GRAVITY_MPS2,
      liquid: TITAN_LIQUID,
      air: TITAN_AIR,
      fetchM: null,
    });
    expect(state.glassy).toBe(true);
    expect(state.slopeVariance).toEqual({
      upwind: SLOPE_VARIANCE_FLOOR,
      crosswind: SLOPE_VARIANCE_FLOOR,
    });
    expect(state.significantHeightM).toBe(0);
    expect(state.whitecapFraction).toBe(0);
    // The facets' slopes spread under 0.6°: the sun's own disc, not a lobe about it.
    expect(Math.sqrt(state.slopeVariance.upwind)).toBeLessThan((0.6 * Math.PI) / 180);
  });

  it("raises a Titan-like sea's gravity waves by its own wind, their peak no faster than it", () => {
    const state = seaState({
      windMps: 1,
      windHeightM: 10,
      gravityMps2: TITAN_GRAVITY_MPS2,
      liquid: TITAN_LIQUID,
      air: TITAN_AIR,
      fetchM: null,
    });
    expect(state.glassy).toBe(false);
    // The slopes take the equivalent wind, several times the body's own.
    expect(state.equivalentWind10Mps).toBeGreaterThan(3);
    // H_s = 0.209 U19.5^2 / g at the body's own 1.06 m/s: about 0.17 m, not metres.
    const u19 = windAtHeight(1, CLIMATE_WIND_HEIGHT_M, 19.5);
    expect(state.significantHeightM).toBeCloseTo((0.2092 * u19 * u19) / TITAN_GRAVITY_MPS2, 3);
    expect(state.significantHeightM).toBeLessThan(0.25);
    // The peak's phase speed g / omega_p is U19.5 / 0.877: the wave age of a developed sea.
    const peakSpeed = TITAN_GRAVITY_MPS2 / state.peakAngularFrequencyRadS;
    expect(peakSpeed / u19).toBeCloseTo(1 / 0.877, 2);
  });

  it("meets Earth's threshold by the equivalent wind where the viscosity is not modelled", () => {
    const unviscous: SeaLiquid = { ...TITAN_LIQUID, viscosityPaS: null };
    const earth = generationThreshold(EARTH_SEA.gravityMps2, EARTH_SEA.liquid, EARTH_SEA.air);
    const threshold = generationThreshold(TITAN_GRAVITY_MPS2, unviscous, TITAN_AIR);
    expect(equivalentWind(threshold, TITAN_GRAVITY_MPS2, unviscous, TITAN_AIR)).toBeCloseTo(
      earth,
      12,
    );
  });
});
