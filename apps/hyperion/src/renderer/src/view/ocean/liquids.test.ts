import type { LiquidDto } from "@hyperion/protocol";
import { describe, expect, it } from "vitest";

import { BAKE_WAVELENGTHS_NM, CHANNEL_WAVELENGTHS_NM } from "../atmosphere/medium";
import {
  binValueAt,
  columnTransmittance,
  deepReflectance,
  diffuseAttenuationPerM,
  fresnelReflectance,
  LIQUID_INDEX_CHECKS,
  normalReflectance,
  oceanLiquidOf,
  refractedCosine,
  remoteSensingReflectance,
  shallowReflectance,
} from "./liquids";

/** `testing/liquids.rs`' liquid water at 25 °C, as the surface payload carries it. */
const WATER: LiquidDto = {
  temperature_k: 298.15,
  density_kg_m3: 997.047,
  basis: "measured",
  refractive_index: [
    1.343789, 1.341476, 1.339564, 1.337962, 1.336599, 1.335422, 1.334396, 1.333487, 1.332676,
    1.331946, 1.331283, 1.330673, 1.330109, 1.329581, 1.329086,
  ],
  absorption_per_m: [
    0.001875, 0.00305405, 0.00624414, 0.0103012, 0.0177158, 0.039037, 0.0517617, 0.073031, 0.180294,
    0.27759, 0.334405, 0.442138, 0.601402, 1.44247, 2.80088,
  ],
  scattering_per_m: [
    0.005469387272826296, 0.004181643650890754, 0.0032546413241809694, 0.0025727588408902564,
    0.002061546784888825, 0.001671864057626776, 0.0013704710557889293, 0.001134243878283069,
    0.0009469343271450381, 0.0007968215862562356, 0.0006753565140614371, 0.0005761861743582379,
    0.0004945704130290098, 0.00042689541857878524, 0.00037039794350413535,
  ],
  surface_tension_n_per_m: 0.07197,
  viscosity_pa_s: 8.9e-4,
};

/** A methane sea's record as `testing/liquids.rs` has it: no absorption, tension or viscosity. */
const METHANE: LiquidDto = {
  temperature_k: 93.65,
  density_kg_m3: 447.6,
  basis: "derived",
  refractive_index: [
    1.295, 1.2945, 1.294, 1.2935, 1.293, 1.2925, 1.292, 1.2915, 1.291, 1.2905, 1.29, 1.2895, 1.289,
    1.2885, 1.288,
  ],
  scattering_per_m: [
    0.001, 0.001, 0.001, 0.001, 0.001, 0.001, 0.001, 0.001, 0.001, 0.001, 0.001, 0.001, 0.001,
    0.001, 0.001,
  ],
};

describe("binValueAt", () => {
  it("gives each bin's own value at its centre and the line between them", () => {
    const bins = Array.from({ length: 15 }, (_, i) => i * i);
    BAKE_WAVELENGTHS_NM.forEach((nm, i) => {
      expect(binValueAt(bins, nm)).toBeCloseTo(i * i, 9);
    });
    const [a, b] = [BAKE_WAVELENGTHS_NM[3] ?? 0, BAKE_WAVELENGTHS_NM[4] ?? 0];
    expect(binValueAt(bins, (a + b) / 2)).toBeCloseTo((9 + 16) / 2, 9);
  });

  it("holds the end bins' values outside the bins' centres", () => {
    const bins = Array.from({ length: 15 }, (_, i) => i + 1);
    expect(binValueAt(bins, 380)).toBe(1);
    expect(binValueAt(bins, 760)).toBe(15);
  });

  it("refuses a member of the wrong length", () => {
    expect(() => binValueAt([1, 2, 3], 500)).toThrow(RangeError);
  });
});

describe("oceanLiquidOf", () => {
  it("reads water's record at the render channels with nothing missing", () => {
    const water = oceanLiquidOf(WATER);
    expect(water.missing).toEqual([]);
    expect(water.approximate).toBe(false);
    expect(water.refractiveIndex[1]).toBeCloseTo(
      binValueAt(WATER.refractive_index, CHANNEL_WAVELENGTHS_NM[1]),
      15,
    );
    expect(water.refractiveIndex[1]).toBeGreaterThan(1.33);
    expect(water.refractiveIndex[1]).toBeLessThan(1.336);
    const a = water.absorptionPerM;
    expect(a).not.toBeNull();
    expect(a?.[0] ?? 0).toBeGreaterThan(10 * (a?.[2] ?? 1));
  });

  it("names a methane record's absent members and its derived basis", () => {
    const methane = oceanLiquidOf(METHANE);
    expect(methane.missing).toEqual(["absorption", "surface-tension", "viscosity"]);
    expect(methane.approximate).toBe(true);
    expect(methane.absorptionPerM).toBeNull();
    expect(methane.surfaceTensionNPerM).toBeNull();
  });
});

describe("Fresnel", () => {
  it("meets the normal reflectance at normal incidence and 1 at grazing", () => {
    for (const n of [1.2, 1.333, 1.4, 1.6]) {
      expect(fresnelReflectance(n, 1)).toBeCloseTo(normalReflectance(n), 15);
      expect(fresnelReflectance(n, 0)).toBeCloseTo(1, 12);
    }
    expect(normalReflectance(1.333)).toBeCloseTo(0.0204, 4);
  });

  it("is half the s reflectance at Brewster's angle", () => {
    const n = 1.333;
    const cos = Math.cos(Math.atan(n));
    const sin = Math.sqrt(1 - cos * cos);
    const cosT = Math.sqrt(1 - (sin * sin) / (n * n));
    const rs = ((cos - n * cosT) / (cos + n * cosT)) ** 2;
    expect(fresnelReflectance(n, cos)).toBeCloseTo(rs / 2, 12);
  });

  it("keeps the registry's test indices' reflectances (ethane about 0.028 at 1.40)", () => {
    const ethane = LIQUID_INDEX_CHECKS.find((row) => row.liquid === "ethane");
    expect(ethane?.refractiveIndex).toBe(1.4);
    expect(normalReflectance(ethane?.refractiveIndex ?? 0)).toBeCloseTo(0.028, 3);
    expect(LIQUID_INDEX_CHECKS.map((row) => row.liquid)).toEqual([
      "water",
      "seawater",
      "methane",
      "ethane",
    ]);
  });
});

describe("the column", () => {
  it("gives a deep pure-water sea Morel and Prieur's blue, of order 0.1", () => {
    const deep = deepReflectance(oceanLiquidOf(WATER));
    expect(deep).not.toBeNull();
    const [red, , blue] = deep ?? [0, 0, 0];
    expect(blue).toBeGreaterThan(0.05);
    expect(blue).toBeLessThan(0.15);
    expect(red).toBeLessThan(blue / 50);
  });

  it("draws Beer–Lambert alone without scattering and an opaque column without absorption", () => {
    const water = oceanLiquidOf(WATER);
    expect(deepReflectance({ ...water, scatteringPerM: null })).toEqual([0, 0, 0]);
    expect(deepReflectance(oceanLiquidOf(METHANE))).toBeNull();
    expect(columnTransmittance(oceanLiquidOf(METHANE), 1, 1, 1)).toBeNull();
    expect(diffuseAttenuationPerM(oceanLiquidOf(METHANE), 1)).toBeNull();
  });

  it("passes everything through no water and dims the red first", () => {
    const water = oceanLiquidOf(WATER);
    expect(columnTransmittance(water, 0, 0.5, 0.5)).toEqual([1, 1, 1]);
    const [red, green, blue] = columnTransmittance(water, 5, 1, 1) ?? [0, 0, 0];
    expect(red).toBeLessThan(green);
    expect(green).toBeLessThan(blue);
    expect(blue).toBeCloseTo(Math.exp(-(water.absorptionPerM?.[2] ?? 0) * 10), 12);
  });

  it("goes from the bottom's albedo at no depth to the deep sea's far down", () => {
    const water = oceanLiquidOf(WATER);
    const deep = deepReflectance(water) ?? [0, 0, 0];
    const k = diffuseAttenuationPerM(water, 0.8) ?? [0, 0, 0];
    const sand = [0.3, 0.25, 0.2] as const;
    expect(shallowReflectance(deep, sand, 0, k)).toEqual(sand);
    const far = shallowReflectance(deep, sand, 1e5, k);
    far.forEach((value, i) => {
      expect(value).toBeCloseTo(deep[i] ?? 0, 12);
    });
  });

  it("refracts by Snell's law and leaves the water through its radiance law", () => {
    const n = 1.333;
    expect(refractedCosine(n, 1)).toBe(1);
    expect(refractedCosine(n, 0)).toBeCloseTo(Math.sqrt(1 - 1 / (n * n)), 15);
    const water = oceanLiquidOf(WATER);
    const reflectance = [0.1, 0.1, 0.1] as const;
    const rrs = remoteSensingReflectance(water, reflectance, 1, 1);
    const ni = water.refractiveIndex[1];
    const t = 1 - normalReflectance(ni);
    expect(rrs[1]).toBeCloseTo((t * t * 0.1) / (ni * ni * Math.PI), 15);
  });
});
