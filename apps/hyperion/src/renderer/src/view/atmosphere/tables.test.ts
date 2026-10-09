import { describe, expect, it } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import { EARTH_REFERENCE, HILLAIRE_REFERENCE } from "./earth";
import { type AtmosphereMedium, tabulatedDensity } from "./medium";
import {
  AtmosphereTables,
  MAX_TERMS,
  MULTI_SCATTERING_KERNEL,
  packMedium,
  TRANSMITTANCE_KERNEL,
} from "./tables";

const R = 6_371_008.8;

describe("AtmosphereTables", () => {
  it("builds both tables once, with two dispatches, under atmosphere-tables", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_REFERENCE, R);
    expect(tables.builds).toBe(1);
    expect(engine.dispatched.map((d) => d.kernel)).toEqual([
      "atmosphere transmittance",
      "atmosphere multiple scattering",
    ]);
    expect(engine.counts.textures).toBe(2);
  });

  it("does not rebuild for the same medium and radius", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_REFERENCE, R);
    expect(tables.setMedium(EARTH_REFERENCE, R)).toBe(false);
    expect(tables.setMedium({ ...EARTH_REFERENCE }, R)).toBe(false);
    expect(engine.counts.dispatches).toBe(2);
  });

  it("rebuilds in place when the medium or the radius changes", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_REFERENCE, R);
    expect(tables.setMedium(HILLAIRE_REFERENCE, R)).toBe(true);
    expect(tables.setMedium(HILLAIRE_REFERENCE, 6_360_000)).toBe(true);
    expect(tables.builds).toBe(3);
    expect(engine.counts.dispatches).toBe(6);
    expect(engine.counts.textures).toBe(2);
  });

  it("keeps the old key when a build fails, so the same medium is tried again", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_REFERENCE, R);
    const first = EARTH_REFERENCE.terms[0];
    if (first === undefined) {
      throw new Error("Earth's medium has no terms");
    }
    const tooMany: AtmosphereMedium = {
      ...EARTH_REFERENCE,
      terms: Array.from({ length: MAX_TERMS + 1 }, () => first),
    };
    expect(() => tables.setMedium(tooMany, R)).toThrow(/at most 8/);
    expect(tables.builds).toBe(1);
    expect(tables.setMedium(EARTH_REFERENCE, R)).toBe(false);
  });

  it("makes its textures again and rebuilds after the engine's restore", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, HILLAIRE_REFERENCE, R);
    engine.restore();
    expect(engine.counts.textures).toBe(4);
    expect(engine.counts.kernels).toBe(4);
    expect(tables.builds).toBe(2);
    expect(engine.dispatched.at(-1)?.bindings.uniforms["medium"]?.[0]).toBe(Math.fround(R));
    tables.dispose();
    engine.restore();
    expect(tables.builds).toBe(2);
  });
});

describe("packMedium", () => {
  it("lays out the radii, the term count, the samples and the albedo first", () => {
    const packed = packMedium(EARTH_REFERENCE, 6_371_000, 128);
    expect([...packed.subarray(0, 8)]).toEqual(
      [6_371_000, 6_471_000, 3, 128, 0.1, 0.1, 0.1, 0].map(Math.fround),
    );
  });

  it("packs each term's scattering, absorption and profile as three vec4f", () => {
    const packed = packMedium(EARTH_REFERENCE, 6_371_000, 1);
    // The ozone tent is the third term.
    const ozone = packed.subarray(8 + 2 * 12, 8 + 3 * 12);
    expect([...ozone.subarray(0, 4)]).toEqual([0, 0, 0, 0]);
    expect(ozone[5]).toBeCloseTo(1.881e-6, 12);
    expect([...ozone.subarray(8, 12)]).toEqual([1, 10_000, 25_000, 40_000]);
    const rayleigh = packed.subarray(8, 20);
    expect([...rayleigh.subarray(8, 12)]).toEqual([0, 8_434.5, 0, 0]);
  });

  it("packs each term's phase in scattering.w and its g in absorption.w", () => {
    const packed = packMedium(EARTH_REFERENCE, 6_371_000, 1);
    const term = (i: number): Float32Array => packed.subarray(8 + i * 12, 8 + (i + 1) * 12);
    // Rayleigh, then the Cornette–Shanks aerosol, then ozone with no phase.
    expect([term(0)[3], term(0)[7]]).toEqual([1, 0]);
    expect(term(1)[3]).toBe(2);
    expect(term(1)[7]).toBeCloseTo(0.584, 6);
    expect([term(2)[3], term(2)[7]]).toEqual([0, 0]);
  });

  it("is sized for the WGSL's MAX_TERMS terms whatever the medium", () => {
    expect(packMedium(HILLAIRE_REFERENCE, 1, 1)).toHaveLength(8 + MAX_TERMS * 12);
  });

  it("refuses a medium with more terms than the uniform holds", () => {
    const tooMany: AtmosphereMedium = {
      ...EARTH_REFERENCE,
      terms: Array.from({ length: MAX_TERMS + 1 }, () => EARTH_REFERENCE.terms[0]).filter(
        (t) => t !== undefined,
      ),
    };
    expect(() => packMedium(tooMany, 1, 1)).toThrow(/at most 8/);
  });

  it("refuses a tabulated density, which the uniform cannot carry", () => {
    const [first, ...rest] = EARTH_REFERENCE.terms;
    if (first === undefined) {
      throw new Error("EARTH_REFERENCE has no terms");
    }
    const tabulated: AtmosphereMedium = {
      ...EARTH_REFERENCE,
      terms: [
        {
          ...first,
          density: tabulatedDensity(Float64Array.of(0, 10_000), Float64Array.of(1, 0.3)),
        },
        ...rest,
      ],
    };
    expect(() => packMedium(tabulated, 1, 1)).toThrow(/tabulated/);
  });
});

describe("the kernels", () => {
  it("prepend the common WGSL and declare MAX_TERMS as the packer does", () => {
    for (const kernel of [TRANSMITTANCE_KERNEL, MULTI_SCATTERING_KERNEL]) {
      expect(kernel.reference).toContain(`const MAX_TERMS: u32 = ${MAX_TERMS}u;`);
      expect(kernel.readback).toBe("presentation-only");
      expect(kernel.subgroup).toBeNull();
    }
  });
});
