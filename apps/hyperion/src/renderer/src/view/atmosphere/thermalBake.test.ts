import { beforeAll, describe, expect, it } from "vitest";

import { R06_SOLAR_BAKE_SPECTRUM } from "./earth";
import type { AtmosphereMedium } from "./medium";
import {
  blackBodyLuminance,
  type MediumTemperature,
  packThermalSource,
  THERMAL_SOURCE_HEIGHTS,
  type ThermalRgb,
  thermalScatteringInto,
  thermalSourceOf,
} from "./thermalSource";
import {
  bakeMultipleScattering,
  type MsSourceTable,
  prepareBake,
  thickBakeLights,
} from "./thick/bake";
import { sphereBakeBody } from "./thick/keptBake";
import { channelSource, spectralColumnOf } from "./thick/spectralColumn";

/**
 * A hot column opaque in every bin (τ about 60 vertically, ω 0.5, Rayleigh's phase, so no forward
 * delta), over a ground of albedo 0.3, isothermal at 1,800 K.
 */
const HOT: AtmosphereMedium = {
  name: "hot opaque column",
  topHeightM: 100_000,
  groundAlbedo: [0.3, 0.3, 0.3],
  terms: [
    {
      name: "opaque",
      density: { kind: "exponential", scaleHeightM: 8_000 },
      scattering: [3.75e-3, 3.75e-3, 3.75e-3],
      absorption: [3.75e-3, 3.75e-3, 3.75e-3],
      phase: { kind: "rayleigh", depolarisation: [0, 0, 0] },
    },
  ],
};

const ISOTHERMAL: MediumTemperature = {
  heightsM: Float64Array.of(0, 100_000),
  temperaturesK: Float64Array.of(1_800, 1_800),
};

const BODY = sphereBakeBody(6_371_000, 9.81);

/** The hot column's band, baked with its temperature or without. */
function baked(temperature?: MediumTemperature): MsSourceTable {
  const column = spectralColumnOf(HOT, HOT.terms.map(channelSource), temperature);
  const prepared = prepareBake(column, thickBakeLights(column, [], [R06_SOLAR_BAKE_SPECTRUM]));
  return bakeMultipleScattering(prepared, BODY, 0);
}

describe("the thick bake's scattered thermal light (R08.T18.b)", () => {
  let table: MsSourceTable | undefined;
  beforeAll(() => {
    table = baked(ISOTHERMAL);
  }, 120_000);
  const tableOf = (): MsSourceTable => {
    if (table === undefined) {
      throw new Error("the hot column is baked before the tests");
    }
    return table;
  };

  it("holds J_th at the ground of an opaque isothermal column at B̃(T) in every direction", () => {
    const thermal = tableOf().thermal;
    if (thermal === undefined) {
      throw new Error("a column with a temperature bakes its thermal light");
    }
    // At the ground the field is B both ways: from τ 60 above, and from a ground at T emitting
    // 1 − A and reflecting A. J_th = ∫ p I dΩ ÷ (1 − f_c) is then B̃, as a luminance per channel.
    const want = blackBodyLuminance(1_800);
    for (let v = 0; v < 16; v += 1) {
      for (const c of [0, 1, 2] as const) {
        const got = thermal.values[v * 3 + c] ?? Number.NaN;
        expect(Math.abs(got / want[c] - 1)).toBeLessThan(1e-4);
      }
    }
    // Near the top the field is darker upward-looking than B: light leaves, none comes in.
    const top = (thermal.altitudesM.length - 1) * 16;
    expect(thermal.values[(top + 0) * 3]).toBeLessThan(want[0]);
  });

  it("bakes no thermal light for a column without a temperature", () => {
    expect(baked().thermal).toBeUndefined();
  }, 120_000);

  it("bakes the same sunlit J beside its thermal light as without it", () => {
    expect(baked().slots[0]?.values).toEqual(tableOf().slots[0]?.values);
  }, 120_000);
});

describe("the thick bake's signed thermal light (R08.T18.b)", () => {
  it("keeps a cool column's negative blue through J_th, the source's packing and its read", () => {
    // At 1,500 K B̃'s blue is negative (Rec. 709's matching functions' negative lobe).
    const cool: MediumTemperature = {
      heightsM: Float64Array.of(0, 100_000),
      temperaturesK: Float64Array.of(1_500, 1_500),
    };
    const thermal = baked(cool).thermal;
    if (thermal === undefined) {
      throw new Error("a column with a temperature bakes its thermal light");
    }
    const want = blackBodyLuminance(1_500)[2];
    const ground = thermal.values[2] ?? Number.NaN;
    expect([want < 0, Math.abs(ground / want - 1) < 1e-4]).toEqual([true, true]);
    // As the kernels read it (`thermalSource.wgsl`): B̃ at the ground's height, then the band's J_th
    // after its altitudes, each the same signed f32.
    const source = thermalSourceOf(cool, HOT.topHeightM, [thermal]);
    const packed = packThermalSource(source);
    const firstJ = 1 + THERMAL_SOURCE_HEIGHTS + thermal.altitudesM.length;
    expect([packed[1 * 4 + 2], packed[firstJ * 4 + 2]]).toEqual([Math.fround(want), ground]);
    const read: ThermalRgb = [0, 0, 0];
    thermalScatteringInto(source, [0], 1, 0, 0.3, read);
    expect(Math.abs(read[2] / want - 1)).toBeLessThan(1e-4);
  }, 120_000);
});
