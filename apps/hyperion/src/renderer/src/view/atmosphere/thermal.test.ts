import { describe, expect, it } from "vitest";

import { SPEED_OF_LIGHT_M_PER_S } from "../../lib/scene/lightTime";
import {
  earthSurface,
  heldMagmaOceanSurface,
  lavaWorldSurface,
  magmaOceanSurface,
  warmAirlessSurface,
} from "../../test/thermalSurfaces";
import { starIlluminance } from "../lighting/illuminance";
import { sunLikeHostDisc } from "../lighting/hostDisc";
import type { Rgb } from "../photometry/toneCurve";
import { AU_M } from "../scenes/kept";
import matching from "./colourMatching.json" with { type: "json" };
import { BAKE_BIN_COUNT, BAKE_WAVELENGTHS_NM } from "./medium";
import { BOLTZMANN_J_PER_K } from "./rayleigh";
import { rec709Luminance } from "./solar";
import { MATCHING_FUNCTIONS, PIPELINE_WAVELENGTHS_NM } from "./spectralColour";
import { binGroundAlbedo } from "./thick/bake";
import { binOfRow } from "./thick/binConversion";
import {
  aboveThermalCut,
  EMISSION_CHANNEL_WEIGHTS,
  emittedLuminance,
  FIELD_LEAST_K,
  fieldPeakK,
  fieldTemperatureK,
  groundTemperatureFieldOf,
  kirchhoffEmissivity,
  LUMINOUS_EFFICACY_LM_PER_W,
  PLANCK_J_S,
  planckBins,
  planckRadiance,
  SECOND_RADIATION_NM_K,
  SILICATE_SOLIDUS_K,
  surfaceTemperatureFieldOf,
  THERMAL_CUT,
  THERMAL_WGSL,
  thermalLuminance,
} from "./thermal";

/**
 * Planck's law written out on its own, W m⁻² sr⁻¹ nm⁻¹, with the 2019 SI's exact h, c and k_B
 * typed here rather than imported, for the independent integrations.
 */
function planckOwn(wavelengthNm: number, temperatureK: number): number {
  const h = 6.626_070_15e-34;
  const c = 299_792_458;
  const k = 1.380_649e-23;
  const m = wavelengthNm * 1e-9;
  return ((2 * h * c * c) / m ** 5 / Math.expm1((h * c) / (m * k * temperatureK))) * 1e-9;
}

/** 2 h c² ÷ λ⁵, W m⁻² sr⁻¹ nm⁻¹ for λ in nm, from the SI's exact h and c typed here. */
function firstRadiationOver(wavelengthNm: number): number {
  return (2 * 6.626_070_15e-34 * 299_792_458 ** 2 * 1e36) / wavelengthNm ** 5;
}

/** The matching functions' rows as the JSON holds them: [nm, r̄, ḡ, b̄] at 1 nm, 360–830 nm. */
const ROWS: ReadonlyArray<ReadonlyArray<number>> = matching.rows;

/** r̄, ḡ, b̄ at any wavelength of the table, linear between its rows. */
function matchingAt(wavelengthNm: number): Rgb {
  const x = wavelengthNm - matching.firstNm;
  const k = Math.min(Math.floor(x), ROWS.length - 2);
  const t = x - k;
  const low = ROWS[k] ?? [];
  const high = ROWS[k + 1] ?? [];
  const at = (c: number): number => (low[c] ?? Number.NaN) * (1 - t) + (high[c] ?? Number.NaN) * t;
  return [at(1), at(2), at(3)];
}

/**
 * Planck's law times the matching functions, integrated over their whole table, 360–830 nm, by
 * the trapezoid rule at 0.1 nm, times 683 lm W⁻¹: the luminance per channel of a black body, cd m⁻²,
 * independent of the bins.
 */
function integratedLuminance(temperatureK: number): Rgb {
  const step = 0.1;
  const steps = Math.round((830 - 360) / step);
  const sum = [0, 0, 0];
  for (let i = 0; i <= steps; i += 1) {
    const nm = 360 + i * step;
    const weight = (i === 0 || i === steps ? 0.5 : 1) * step;
    const b = planckOwn(nm, temperatureK) * weight;
    const cmf = matchingAt(nm);
    for (const c of [0, 1, 2] as const) {
      sum[c] = (sum[c] ?? 0) + cmf[c] * b;
    }
  }
  return [683 * (sum[0] ?? 0), 683 * (sum[1] ?? 0), 683 * (sum[2] ?? 0)];
}

/** Each channel's relative difference of `a` from `b`. */
function relative(a: Rgb, b: Rgb): Rgb {
  return [a[0] / b[0] - 1, a[1] / b[1] - 1, a[2] / b[2] - 1];
}

describe("the constants (R08.T18.a)", () => {
  it("takes h, c and k_B at the 2019 SI's exact values", () => {
    expect(PLANCK_J_S).toBe(6.626_070_15e-34);
    expect(SPEED_OF_LIGHT_M_PER_S).toBe(299_792_458);
    expect(BOLTZMANN_J_PER_K).toBe(1.380_649e-23);
    expect(LUMINOUS_EFFICACY_LM_PER_W).toBe(683);
  });

  it("forms c₂ = h c ÷ k_B as CODATA 2018 gives it, 1.438 776 877 × 10⁻² m K", () => {
    expect(SECOND_RADIATION_NM_K / 1.438_776_877e7 - 1).toBeLessThan(1e-9);
    expect(SECOND_RADIATION_NM_K / 1.438_776_877e7 - 1).toBeGreaterThan(-1e-9);
  });
});

describe("Planck's law (R08.T18.a)", () => {
  it("integrates to σT⁴ ÷ π, σ CODATA 2018's 5.670 374 419 × 10⁻⁸ W m⁻² K⁻⁴", () => {
    const sigma = 5.670_374_419e-8;
    for (const temperatureK of [300, 1_000, 2_500, 5_772]) {
      // Simpson's rule in ln λ over 10 nm to 10⁷ nm, B λ dlnλ.
      const n = 20_000;
      const low = Math.log(10);
      const high = Math.log(1e7);
      const h = (high - low) / n;
      let sum = 0;
      for (let i = 0; i <= n; i += 1) {
        const nm = Math.exp(low + i * h);
        const weight = i === 0 || i === n ? 1 : i % 2 === 1 ? 4 : 2;
        sum += weight * planckRadiance(nm, temperatureK) * nm;
      }
      const integral = (sum * h) / 3;
      expect(Math.abs((Math.PI * integral) / (sigma * temperatureK ** 4) - 1)).toBeLessThan(1e-7);
    }
  });

  it("peaks at Wien's b ÷ T, b CODATA 2018's 2.897 771 955 × 10⁻³ m K", () => {
    const temperatureK = 5_000;
    const peakNm = 2.897_771_955e6 / temperatureK;
    expect(planckRadiance(peakNm, temperatureK)).toBeGreaterThan(
      planckRadiance(peakNm * (1 - 1e-4), temperatureK),
    );
    expect(planckRadiance(peakNm, temperatureK)).toBeGreaterThan(
      planckRadiance(peakNm * (1 + 1e-4), temperatureK),
    );
  });

  it("takes B at the 15 bake bins' centres", () => {
    const bins = planckBins(2_500);
    expect(bins).toHaveLength(BAKE_BIN_COUNT);
    for (const [b, nm] of BAKE_WAVELENGTHS_NM.entries()) {
      expect(Math.abs((bins[b] ?? 0) / planckOwn(nm, 2_500) - 1)).toBeLessThan(1e-12);
    }
  });
});

describe("the reduction to the channels (R08.T18.a)", () => {
  it("reads a 2,500 K surface of emissivity 1 as Planck's law times the matching functions, within 0.2%, 0.4% and 2% in r, g and b", () => {
    const drawn = thermalLuminance([0, 0, 0], 1, 2_500);
    const [r, g, b] = relative(drawn, integratedLuminance(2_500));
    // As measured: +0.14%, +0.31% and +1.51% (the bins' steps; `thermal.ts`' remarks).
    expect(Math.abs(r)).toBeLessThan(2e-3);
    expect(Math.abs(g)).toBeLessThan(4e-3);
    expect(Math.abs(b)).toBeLessThan(2e-2);
    // Its luminance, 5.6 × 10⁶ cd m⁻², to 0.4% (as measured, +0.25%).
    expect(
      Math.abs(rec709Luminance(drawn) / rec709Luminance(integratedLuminance(2_500)) - 1),
    ).toBeLessThan(4e-3);
  });

  it("weighs each bin by 683 Σ c̄ Δλ over the pipeline rows binConversion.ts puts in it", () => {
    const offset = (PIPELINE_WAVELENGTHS_NM[0] ?? 0) - MATCHING_FUNCTIONS.firstNm;
    const last = PIPELINE_WAVELENGTHS_NM.length - 1;
    const green = Array.from({ length: BAKE_BIN_COUNT }, () => 0);
    PIPELINE_WAVELENGTHS_NM.forEach((nm, i) => {
      const bin = binOfRow(nm);
      green[bin] =
        (green[bin] ?? 0) +
        683 * (MATCHING_FUNCTIONS.green[offset + i] ?? 0) * (i === 0 || i === last ? 0.5 : 1);
    });
    EMISSION_CHANNEL_WEIGHTS.forEach((weights, b) => {
      // ḡ is signed: negative in the blue bins.
      expect(Math.abs(weights[1] - (green[b] ?? 0))).toBeLessThanOrEqual(
        1e-12 * Math.abs(green[b] ?? 0),
      );
    });
  });

  it("weighs the bins together as the whole bake range", () => {
    const offset = (PIPELINE_WAVELENGTHS_NM[0] ?? 0) - MATCHING_FUNCTIONS.firstNm;
    const last = PIPELINE_WAVELENGTHS_NM.length - 1;
    let wholeRed = 0;
    PIPELINE_WAVELENGTHS_NM.forEach((_, i) => {
      wholeRed += (MATCHING_FUNCTIONS.red[offset + i] ?? 0) * (i === 0 || i === last ? 0.5 : 1);
    });
    const binsRed = EMISSION_CHANNEL_WEIGHTS.reduce((sum, w) => sum + w[0], 0);
    expect(Math.abs(binsRed / (683 * wholeRed) - 1)).toBeLessThan(1e-12);
  });

  it("is the bins' B times Kirchhoff's emissivity, by the kernel's arithmetic", () => {
    for (const albedo of [
      [0, 0, 0],
      [0.1, 0.08, 0.06],
      [0.9, 0.5, 0.2],
    ] satisfies Rgb[]) {
      for (const temperatureK of [900, 1_500, 2_500]) {
        const direct = emittedLuminance(kirchhoffEmissivity(albedo), temperatureK);
        const shared = thermalLuminance(albedo, 1, temperatureK);
        for (const c of [0, 1, 2] as const) {
          expect(Math.abs(shared[c] - direct[c])).toBeLessThanOrEqual(1e-12 * Math.abs(direct[0]));
        }
      }
    }
  });

  it("takes each bin's emissivity as 1 − A by the bake's ground rule", () => {
    const albedo: Rgb = [0.3, 0.12, 0.05];
    const emissivity = kirchhoffEmissivity(albedo);
    for (const [b, nm] of BAKE_WAVELENGTHS_NM.entries()) {
      expect(Math.abs((emissivity[b] ?? 0) - (1 - binGroundAlbedo(albedo, nm)))).toBeLessThan(
        1e-15,
      );
    }
  });

  it("emits a share of a surface in proportion, its laws' A summed by their shares", () => {
    const whole = thermalLuminance([0.1, 0.1, 0.1], 1, 1_800);
    const quarter = thermalLuminance([0.025, 0.025, 0.025], 0.25, 1_800);
    for (const c of [0, 1, 2] as const) {
      expect(Math.abs(quarter[c] - 0.25 * whole[c])).toBeLessThan(1e-12 * whole[0]);
    }
  });

  it("writes its tables into the kernel's WGSL to the last digit", () => {
    const table = /THERMAL_BIN_LUMINANCE = array<vec3f, 15>\(([^;]*)\);/u.exec(THERMAL_WGSL);
    const numbers = (table?.[1] ?? "").match(/-?\d\.\d*e[+-]\d+/gu)?.map(Number) ?? [];
    expect(numbers).toHaveLength(45);
    const expected = EMISSION_CHANNEL_WEIGHTS.flatMap((w, b) => {
      const s = firstRadiationOver(BAKE_WAVELENGTHS_NM[b] ?? 0);
      return [w[0] * s, w[1] * s, w[2] * s];
    });
    numbers.forEach((value, i) => {
      expect(Math.abs(value / (expected[i] ?? 0) - 1)).toBeLessThan(1e-12);
    });
  });
});

/**
 * A field over the sphere, its star over the equator (μ = cos φ cos λ), by the midpoint rule on
 * 400 rows in sin φ and 400 in longitude: its area mean, and its hemispheres' means weighted by
 * |μ|, Cowan and Agol's day and night.
 */
function overSphere(field: ReturnType<typeof surfaceTemperatureFieldOf>): {
  readonly mean: number;
  readonly dayWeighted: number;
  readonly nightWeighted: number;
} {
  let sum = 0;
  let day = 0;
  let dayWeight = 0;
  let night = 0;
  let nightWeight = 0;
  const n = 400;
  for (let i = 0; i < n; i += 1) {
    const x = -1 + (2 * (i + 0.5)) / n;
    for (let j = 0; j < n; j += 1) {
      const longitude = -Math.PI + (2 * Math.PI * (j + 0.5)) / n;
      const mu = Math.sqrt(1 - x * x) * Math.cos(longitude);
      const t = fieldTemperatureK(field, mu, x);
      sum += t;
      if (mu > 0) {
        day += mu * t;
        dayWeight += mu;
      } else {
        night -= mu * t;
        nightWeight -= mu;
      }
    }
  }
  return { mean: sum / (n * n), dayWeighted: day / dayWeight, nightWeighted: night / nightWeight };
}

describe("the temperature field (R08.T18.a)", () => {
  it("gives the locked lava world its eyeball, whose day and night differ by the record's contrast and whose emission mean is its T_s", () => {
    const surface = lavaWorldSurface();
    const field = surfaceTemperatureFieldOf(surface);
    if (field.profile.kind !== "substellar") {
      throw new Error("the lava world is locked");
    }
    const { substellarK, nightK } = field.profile;
    const n4 = nightK ** 4;
    const s4 = substellarK ** 4;
    // Cowan and Agol's day: the dayside's μ-weighted mean of σT⁴, E_n + (2/3)(E_ss − E_n).
    const dayK = (n4 + (2 / 3) * (s4 - n4)) ** 0.25;
    expect(Math.abs(dayK - nightK - surface.dayNightContrastK)).toBeLessThan(1e-6);
    // The sphere's emission mean: the night's half and the day's ∫ E dμ.
    const mean4 = 0.5 * n4 + 0.5 * (n4 + 0.5 * (s4 - n4));
    expect(Math.abs(mean4 ** 0.25 / surface.meanSurfaceTemperatureK - 1)).toBeLessThan(1e-9);
    // T_ss ≈ √2 T_s on an airless world, its night its floor's 21 K.
    expect(substellarK).toBeCloseTo(1_751.8, 1);
    expect(nightK).toBeGreaterThan(20);
    expect(nightK).toBeLessThan(23);
  });

  it("finds the lava world's melt at its lid's transition, basalt_reduced's 1,473.15 K, from its melt area", () => {
    const field = surfaceTemperatureFieldOf(lavaWorldSurface());
    expect(field.meltK).not.toBeNull();
    expect(Math.abs((field.meltK ?? 0) - 1_473.15)).toBeLessThan(0.05);
    expect(field.floorK).toBe(FIELD_LEAST_K);
  });

  it("holds a young magma ocean everywhere at the solidus at least, its night included", () => {
    const field = surfaceTemperatureFieldOf(magmaOceanSurface());
    expect(field.floorK).toBe(SILICATE_SOLIDUS_K);
    expect(fieldTemperatureK(field, -1, 0)).toBe(SILICATE_SOLIDUS_K);
    expect(fieldTemperatureK(field, 1, 0)).toBeGreaterThan(1_700);
  });

  it("gives a locked magma ocean its star alone holds molten a solid night, drawn black", () => {
    const field = surfaceTemperatureFieldOf(heldMagmaOceanSurface());
    expect(field.floorK).toBe(FIELD_LEAST_K);
    const nightK = fieldTemperatureK(field, -1, 0);
    expect(nightK).toBeLessThan(30);
    expect(fieldTemperatureK(field, 1, 0)).toBeGreaterThan(2_000);
    expect(thermalLuminance([0.1, 0.1, 0.1], 1, nightK)).toEqual([0, 0, 0]);
    // Its melt's least temperature is the eyeball's at the melt's edge, its whole surface's: its
    // night's.
    expect(field.meltK).toBeCloseTo(nightK, 9);
  });

  it("holds a turning magma ocean at the solidus at least, its night included, whatever its age", () => {
    const held = heldMagmaOceanSurface();
    expect(held.surfaceAgeS).toBeGreaterThan(0);
    const field = surfaceTemperatureFieldOf({
      ...held,
      climateRegime: { ...held.climateRegime, forcing: "seasonal" },
    });
    expect(field.floorK).toBe(SILICATE_SOLIDUS_K);
    expect(fieldTemperatureK(field, -1, 0)).toBe(SILICATE_SOLIDUS_K);
  });

  it("gives a rotating world its zonal profile, its ends the equator–pole contrast apart", () => {
    const field = surfaceTemperatureFieldOf(warmAirlessSurface());
    expect(field.profile.kind).toBe("zonal");
    expect(fieldTemperatureK(field, 0, 0) - fieldTemperatureK(field, 0, 1)).toBeCloseTo(40, 12);
  });

  it("keeps a rotating world's area mean at T_s", () => {
    const field = surfaceTemperatureFieldOf(warmAirlessSurface());
    const { mean } = overSphere(field);
    // The midpoint rule's own error in x², 8 × 10⁻⁵ K at 400 rows.
    expect(mean).toBeCloseTo(300, 3);
  });

  it("parts a rotating world's hemispheres' μ-weighted means by the day–night contrast, as the sim's Cowan and Agol day and night", () => {
    const field = surfaceTemperatureFieldOf(warmAirlessSurface());
    const { dayWeighted, nightWeighted } = overSphere(field);
    expect(dayWeighted - nightWeighted).toBeCloseTo(10, 3);
  });

  it("gives a body with a medium its ground's field too, seen through its air (R08.T18.b)", () => {
    expect(groundTemperatureFieldOf(earthSurface())).toEqual(
      surfaceTemperatureFieldOf(earthSurface()),
    );
  });

  it("takes an airless body and a magma ocean with no volatile air", () => {
    expect(groundTemperatureFieldOf(lavaWorldSurface())).not.toBeNull();
    expect(groundTemperatureFieldOf(magmaOceanSurface())).not.toBeNull();
  });
});

describe("the visibility cut (R08.T18.a)", () => {
  const sun = sunLikeHostDisc();

  it("finds a 300 K world's emission under 10⁻⁶ of its reflected light in every channel, and computes nothing", () => {
    const field = surfaceTemperatureFieldOf(warmAirlessSurface());
    const albedo: Rgb = [0.3, 0.3, 0.3];
    const lit = starIlluminance(sun, AU_M);
    const emitted = thermalLuminance(albedo, 1, fieldPeakK(field));
    for (const c of [0, 1, 2] as const) {
      expect(emitted[c]).toBeLessThan(THERMAL_CUT * ((albedo[c] * lit[c]) / Math.PI));
    }
    expect(aboveThermalCut(field, [albedo], [lit])).toBe(false);
  });

  it("draws the locked lava world 0.048 au from its star", () => {
    const lava = surfaceTemperatureFieldOf(lavaWorldSurface());
    const albedo: Rgb = [0.1, 0.1, 0.1];
    expect(aboveThermalCut(lava, [albedo], [starIlluminance(sun, 0.048 * AU_M)])).toBe(true);
  });

  it("draws a magma ocean that no star lights, against the reflected light's floor", () => {
    const ocean = surfaceTemperatureFieldOf(magmaOceanSurface());
    expect(aboveThermalCut(ocean, [[0.1, 0.1, 0.1]], [])).toBe(true);
  });
});
