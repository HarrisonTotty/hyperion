import { describe, expect, it, vi } from "vitest";

import { countingRenderEngine } from "../../test/countingRenderEngine";
import type { AllocationEvent } from "../engine/memory";
import { EngineUnavailable } from "../engine/resilientEngine";
import { hydrostaticColumn } from "./column";
import { EARTH_REFERENCE, HILLAIRE_REFERENCE } from "./earth";
import {
  type AtmosphereMedium,
  columnLengthM,
  densityAt,
  type MediumTerm,
  type PhaseFunction,
  phaseAt,
  type PhaseTable,
  phaseTable,
  tabulatedDensity,
} from "./medium";
import {
  AtmosphereTables,
  DENSITY_TABLE_LEVELS,
  densityTableLevelsM,
  kernelMedium,
  MAX_TERMS,
  MULTI_SCATTERING_KERNEL,
  MULTI_SCATTERING_SAMPLES,
  packDensityTables,
  packMedium,
  packMediumUniform,
  packPhaseTables,
  PHASE_TABLE_ENTRIES,
  resampledDensity,
  resampledPhase,
  TERMS_BUFFER_BYTES,
  TRANSMITTANCE_KERNEL,
  TRANSMITTANCE_SAMPLES,
} from "./tables";
import { kernelOpticalDepth } from "./tablesCpu";

const R = 6_371_008.8;

/** A Henyey–Greenstein phase of asymmetry g at each u of a grid, sr⁻¹. */
function henyeyGreenstein(g: number, u: Float64Array): Float64Array {
  return u.map((x) => {
    const mu = Math.cos(Math.PI * x * x);
    return (1 - g * g) / (4 * Math.PI * (1 + g * g - 2 * g * mu) ** 1.5);
  });
}

/** The kernels' even grid of 256 entries in u. */
const EVEN_U = Float64Array.from({ length: PHASE_TABLE_ENTRIES }, (_, i) => i / 255);

/** A Henyey–Greenstein table on the kernels' grid, a different g a channel. */
const EVEN_TABLE: PhaseTable = phaseTable(EVEN_U, [
  henyeyGreenstein(0.75, EVEN_U),
  henyeyGreenstein(0.7, EVEN_U),
  henyeyGreenstein(0.65, EVEN_U),
]);

/** 181 entries denser towards the forward peak than the kernels' grid: u = (i ÷ 180)^1.3. */
const UNEVEN_U = Float64Array.from({ length: 181 }, (_, i) => (i / 180) ** 1.3);

/** A gas from the ground: R08.T3.a's column of an isothermal 250 K Earth-like atmosphere. */
const COLUMN = hydrostaticColumn({
  surfacePa: 101_325,
  temperature: { kind: "isothermal", temperatureK: 250 },
  meanMolarMassGPerMol: 28.97,
  referenceGravityMS2: 9.806,
  referenceRadiusM: R,
});

const GAS: MediumTerm = {
  name: "gas",
  density: COLUMN.density,
  scattering: [5e-6, 1.2e-5, 3e-5],
  absorption: [0, 0, 0],
  // Young 1981's ρ for air at 550 nm, 0.0279, as a test's value in every channel.
  phase: { kind: "rayleigh", depolarisation: [0.0279, 0.0279, 0.0279] },
};

/** A haze layer from 1 to 6 km, peaking at 2 km, with a tabulated phase on the kernels' grid. */
const HAZE: MediumTerm = {
  name: "haze",
  density: tabulatedDensity(Float64Array.of(1_000, 2_000, 6_000), Float64Array.of(0, 1, 0)),
  scattering: [3e-5, 3.5e-5, 4e-5],
  absorption: [3e-6, 2e-6, 1e-6],
  phase: { kind: "tabulated", table: EVEN_TABLE },
};

/** An absorber with no phase, a tent from 15 to 45 km. */
const ABSORBER: MediumTerm = {
  name: "absorber",
  density: { kind: "tent", bottomM: 15_000, peakM: 25_000, topM: 45_000 },
  scattering: [0, 0, 0],
  absorption: [1e-6, 2e-6, 0.2e-6],
  phase: { kind: "none" },
};

/** A medium of the three terms under a 100 km top, below the column's own. */
const TABULATED: AtmosphereMedium = {
  name: "tabulated",
  topHeightM: 100_000,
  groundAlbedo: [0.2, 0.15, 0.1],
  terms: [GAS, ABSORBER, HAZE],
};

/** The texture names an engine destroyed, from its allocation events. */
function destroyedNames(events: readonly AllocationEvent[]): string[] {
  return events.flatMap((e) => (e.kind === "destroyed" ? [e.name] : []));
}

describe("AtmosphereTables", () => {
  it("builds both tables once, with two dispatches, under atmosphere-tables", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_REFERENCE, R);
    expect(tables.builds).toBe(1);
    expect(engine.dispatched.map((d) => d.kernel)).toEqual([
      "atmosphere transmittance",
      "atmosphere multiple scattering",
    ]);
    // The two tables, the two placeholders of a medium with nothing tabulated, the terms buffer.
    expect(engine.counts.textures).toBe(4);
    expect(engine.counts.buffers).toBe(1);
    expect(engine.textureSpecs.every((s) => s.category === "atmosphere-tables")).toBe(true);
  });

  it("binds the terms and the density tables to both kernels, and the transmittance to the second", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, TABULATED, R);
    const [transmittance, multiScattering] = engine.dispatched;
    expect(transmittance?.bindings.buffers).toEqual({ terms: tables.terms });
    expect(transmittance?.bindings.sampled).toEqual({ densityTables: tables.densityTables });
    expect(multiScattering?.bindings.buffers).toEqual({ terms: tables.terms });
    expect(multiScattering?.bindings.sampled).toEqual({
      densityTables: tables.densityTables,
      transmittance: tables.transmittance,
    });
    expect(transmittance?.bindings.uniforms["medium"]?.[3]).toBe(TRANSMITTANCE_SAMPLES);
    expect(multiScattering?.bindings.uniforms["medium"]?.[3]).toBe(MULTI_SCATTERING_SAMPLES);
  });

  it("makes a tabulated medium's tables at their shape and releases the placeholders", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((e) => events.push(e));
    new AtmosphereTables(engine, EARTH_REFERENCE, R).setMedium(TABULATED, R);
    expect(destroyedNames(events)).toEqual([
      "atmosphere density tables",
      "atmosphere phase tables",
    ]);
    const made = engine.textureSpecs.slice(-2).map((s) => [s.name, s.format, s.size]);
    expect(made).toEqual([
      ["atmosphere density tables", "r32float", [DENSITY_TABLE_LEVELS, 1, 3]],
      ["atmosphere phase tables", "rgba32float", [PHASE_TABLE_ENTRIES, 1, 3]],
    ]);
  });

  it("uploads a tabulated medium's tables whole, and no placeholder", async () => {
    const engine = await countingRenderEngine();
    new AtmosphereTables(engine, EARTH_REFERENCE, R).setMedium(TABULATED, R);
    const written = engine.textureWritten.map((w) => [w.texture, w.size]);
    expect(written).toEqual([
      ["atmosphere density tables", [DENSITY_TABLE_LEVELS, 1, 3]],
      ["atmosphere phase tables", [PHASE_TABLE_ENTRIES, 1, 3]],
    ]);
  });

  it("uploads the packed terms at every build", async () => {
    const engine = await countingRenderEngine();
    new AtmosphereTables(engine, EARTH_REFERENCE, R).setMedium(TABULATED, R);
    const terms = engine.writes.filter((w) => w.buffer === "atmosphere terms");
    expect(terms.map((w) => w.bytes)).toEqual([TERMS_BUFFER_BYTES, TERMS_BUFFER_BYTES]);
    expect(new Float32Array(terms[1]?.data.slice().buffer ?? new ArrayBuffer(0))).toEqual(
      packMedium(TABULATED),
    );
  });

  it("rewrites tables of the same shape in place, making and releasing none", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, TABULATED, R);
    const events: AllocationEvent[] = [];
    engine.onAllocation((e) => events.push(e));
    const made = engine.counts.textures;
    engine.textureWritten.length = 0;
    expect(tables.setMedium({ ...TABULATED, terms: [HAZE, ABSORBER, GAS] }, R)).toBe(true);
    expect(destroyedNames(events)).toEqual([]);
    expect(engine.counts.textures).toBe(made);
    expect(engine.textureWritten.map((w) => w.size)).toEqual([
      [DENSITY_TABLE_LEVELS, 1, 3],
      [PHASE_TABLE_ENTRIES, 1, 3],
    ]);
  });

  it("follows a medium a lost device refused, and builds it at the restore", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, EARTH_REFERENCE, R);
    const lost = vi.spyOn(engine, "createTexture").mockImplementationOnce(() => {
      throw new EngineUnavailable("createTexture");
    });
    expect(() => tables.setMedium(TABULATED, R)).toThrow(EngineUnavailable);
    lost.mockRestore();
    expect(tables.medium).toBe(TABULATED);
    engine.restore();
    expect(tables.builds).toBe(2);
    expect(engine.textureSpecs.at(-1)?.size).toEqual([PHASE_TABLE_ENTRIES, 1, 3]);
  });

  it("returns to the placeholders for a medium with nothing tabulated, releasing the tables", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    const tables = new AtmosphereTables(engine, TABULATED, R);
    engine.onAllocation((e) => events.push(e));
    expect(tables.setMedium(EARTH_REFERENCE, R)).toBe(true);
    expect(destroyedNames(events)).toEqual([
      "atmosphere density tables",
      "atmosphere phase tables",
    ]);
    expect(engine.textureSpecs.slice(-2).map((s) => s.size)).toEqual([
      [1, 1, 1],
      [1, 1, 1],
    ]);
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
    expect(engine.counts.textures).toBe(4);
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
    expect(engine.counts.bufferWrites).toBe(1);
    expect(tables.medium).toBe(EARTH_REFERENCE);
    expect(tables.setMedium(EARTH_REFERENCE, R)).toBe(false);
  });

  it("releases what it made when its first medium is refused", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((e) => events.push(e));
    const above: AtmosphereMedium = { ...TABULATED, topHeightM: COLUMN.topHeightM + 1 };
    expect(() => new AtmosphereTables(engine, above, R)).toThrow(RangeError);
    expect(destroyedNames(events)).toHaveLength(5);
    engine.restore();
    expect(engine.counts.dispatches).toBe(0);
  });

  it("makes its textures and buffer again and rebuilds after the engine's restore", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, TABULATED, R);
    engine.restore();
    expect(engine.counts.textures).toBe(12);
    expect(engine.counts.buffers).toBe(2);
    expect(engine.counts.kernels).toBe(4);
    expect(tables.builds).toBe(2);
    expect(engine.dispatched.at(-1)?.bindings.uniforms["medium"]?.[0]).toBe(Math.fround(R));
    expect(engine.textureSpecs.at(-1)?.size).toEqual([PHASE_TABLE_ENTRIES, 1, 3]);
    tables.dispose();
    engine.restore();
    expect(tables.builds).toBe(2);
  });
});

describe("packMediumUniform", () => {
  it("lays out the radii, the term count, the samples and the albedo", () => {
    const packed = packMediumUniform(EARTH_REFERENCE, 6_371_000, 128);
    expect([...packed]).toEqual([6_371_000, 6_471_000, 3, 128, 0.1, 0.1, 0.1, 0].map(Math.fround));
  });
});

/** Term i's four vec4f in a packed `terms` buffer. */
function termOf(packed: Float32Array, i: number): Float32Array {
  return packed.subarray(i * 16, (i + 1) * 16);
}

describe("packMedium", () => {
  it("packs each term's scattering, absorption, profile and phase as four vec4f", () => {
    const packed = packMedium(EARTH_REFERENCE);
    // The ozone tent is the third term.
    const ozone = termOf(packed, 2);
    expect([...ozone.subarray(0, 4)]).toEqual([0, 0, 0, 0]);
    expect(ozone[5]).toBeCloseTo(1.881e-6, 12);
    expect([...ozone.subarray(8, 12)]).toEqual([1, 10_000, 25_000, 40_000]);
    expect([...termOf(packed, 0).subarray(8, 12)]).toEqual([0, 8_434.5, 0, 0]);
  });

  it("packs each term's phase code in phase.w and its parameters in phase.xyz", () => {
    const packed = packMedium(EARTH_REFERENCE);
    const phase = (i: number): number[] => [...termOf(packed, i).subarray(12, 16)];
    // Rayleigh at R05's ρ = 0, then the Cornette–Shanks aerosol, then ozone with no phase.
    expect(phase(0)).toEqual([0, 0, 0, 1]);
    expect(phase(1)[3]).toBe(2);
    expect(phase(1)[0]).toBeCloseTo(0.584, 6);
    expect(phase(2)).toEqual([0, 0, 0, 0]);
  });

  it("leaves scattering.w and absorption.w 0, where R05 kept the phase", () => {
    const packed = packMedium(EARTH_REFERENCE);
    expect([termOf(packed, 1)[3], termOf(packed, 1)[7]]).toEqual([0, 0]);
  });

  it("packs a Rayleigh phase's depolarisation ratio per channel, which the kernels draw", () => {
    const [first, ...rest] = EARTH_REFERENCE.terms;
    if (first === undefined) {
      throw new Error("EARTH_REFERENCE has no terms");
    }
    const depolarised: AtmosphereMedium = {
      ...EARTH_REFERENCE,
      terms: [
        { ...first, phase: { kind: "rayleigh", depolarisation: [0.027, 0.027, 0.028] } },
        ...rest,
      ],
    };
    expect([...termOf(packMedium(depolarised), 0).subarray(12, 16)]).toEqual(
      [0.027, 0.027, 0.028, 1].map(Math.fround),
    );
  });

  it("gives a tabulated density code 2 with its table's top, and a tabulated phase code 3", () => {
    const packed = packMedium(TABULATED);
    expect([...termOf(packed, 0).subarray(8, 12)]).toEqual([2, 100_000, 0, 0]);
    expect([...termOf(packed, 2).subarray(8, 12)]).toEqual([2, 100_000, 0, 0]);
    expect([...termOf(packed, 2).subarray(12, 16)]).toEqual([0, 0, 0, 3]);
    expect(termOf(packed, 1)[8]).toBe(1);
  });

  it("is sized for the WGSL's MAX_TERMS terms whatever the medium: 128 floats, 512 B", () => {
    expect(packMedium(HILLAIRE_REFERENCE)).toHaveLength(MAX_TERMS * 16);
    expect(packMedium(TABULATED).byteLength).toBe(TERMS_BUFFER_BYTES);
    expect(TERMS_BUFFER_BYTES).toBe(512);
  });

  it("packs a medium of eight terms and refuses one of nine", () => {
    const eight: AtmosphereMedium = {
      ...EARTH_REFERENCE,
      terms: Array.from({ length: MAX_TERMS }, () => HAZE),
    };
    expect(termOf(packMedium(eight), 7)[15]).toBe(3);
    const nine: AtmosphereMedium = { ...eight, terms: [...eight.terms, GAS] };
    expect(() => packMedium(nine)).toThrow(/has 9 terms; at most 8 fit/);
  });

  it("refuses a tabulated density held up to the top from a last level below it", () => {
    const above: AtmosphereMedium = { ...TABULATED, topHeightM: COLUMN.topHeightM + 1 };
    expect(() => packMedium(above)).toThrow(RangeError);
    expect(() => packMedium(above)).toThrow(/holds a density/);
  });

  it("packs a tabulated layer that falls to 0 below the top", () => {
    expect(() => packMedium({ ...TABULATED, terms: [HAZE] })).not.toThrow();
  });
});

describe("the density tables", () => {
  it("stand on 1,024 levels spaced as the square of their index, from the ground to the top", () => {
    const levels = densityTableLevelsM(100_000);
    expect(levels).toHaveLength(DENSITY_TABLE_LEVELS);
    expect(levels[0]).toBe(0);
    expect(levels[1]).toBeCloseTo(100_000 / 1_023 ** 2, 9);
    expect(levels[512]).toBeCloseTo(100_000 * (512 / 1_023) ** 2, 6);
    expect(levels.at(-1)).toBe(100_000);
  });

  it("resample a profile by the tabulated rule, at its own density on each level", () => {
    const resampled = resampledDensity(COLUMN.density, 100_000);
    for (const k of [0, 1, 100, 700, 1_023]) {
      const h = resampled.altitudesM[k] ?? Number.NaN;
      expect(resampled.relative[k]).toBe(densityAt(COLUMN.density, h));
    }
  });

  it("hold R08.T3.a's column to its own column within 10⁻⁵", () => {
    const resampled = resampledDensity(COLUMN.density, 100_000);
    const column = columnLengthM(COLUMN.density, 100_000);
    expect(Math.abs(columnLengthM(resampled, 100_000) / column - 1)).toBeLessThan(1e-5);
  });

  it("hold R08.T3.a's column's density within 2 × 10⁻⁴ at every height", () => {
    const resampled = resampledDensity(COLUMN.density, 100_000);
    // 1.0 × 10⁻⁴ measured, at the top, where the levels are furthest apart (195 m).
    for (let h = 0; h <= 100_000; h += 37) {
      const exact = densityAt(COLUMN.density, h);
      expect(Math.abs(densityAt(resampled, h) / exact - 1)).toBeLessThan(2e-4);
    }
  });

  it("hold R08.T3.a's column's optical depth within 10⁻⁴ along every ray", () => {
    const medium: AtmosphereMedium = { ...TABULATED, terms: [GAS] };
    for (const mu of [1, 0.2, 0, -0.01]) {
      for (const heightM of [1_000, 10_000, 50_000]) {
        const exact = kernelOpticalDepth(medium, R, R + heightM, mu, 4_096);
        const resampledDepth = kernelOpticalDepth(kernelMedium(medium), R, R + heightM, mu, 4_096);
        expect(Math.abs(resampledDepth[2] / exact[2] - 1)).toBeLessThan(1e-4);
      }
    }
  });

  it("hold a haze layer's column to 10⁻⁵, its kinks rounded within one level", () => {
    const profile = HAZE.density;
    if (profile.kind !== "tabulated") {
      throw new Error("the haze is tabulated");
    }
    const resampled = resampledDensity(profile, 100_000);
    const column = columnLengthM(profile, 100_000);
    expect(Math.abs(columnLengthM(resampled, 100_000) / column - 1)).toBeLessThan(1e-5);
    expect(densityAt(resampled, 500)).toBe(0);
    expect(densityAt(resampled, 7_000)).toBe(0);
  });

  it("pack one layer a term, and zeros where a term is not tabulated", () => {
    const tables = packDensityTables(TABULATED);
    expect([tables.widthTexels, tables.layers]).toEqual([DENSITY_TABLE_LEVELS, 3]);
    const layer = (i: number): Float32Array =>
      tables.texels.subarray(i * DENSITY_TABLE_LEVELS, (i + 1) * DENSITY_TABLE_LEVELS);
    expect(layer(0)).toEqual(Float32Array.from(resampledDensity(COLUMN.density, 100_000).relative));
    expect(layer(1).every((v) => v === 0)).toBe(true);
    expect(Math.max(...layer(2))).toBeGreaterThan(0.99);
  });

  it("pack a one-texel placeholder for a medium with no tabulated density", () => {
    const none = packDensityTables(EARTH_REFERENCE);
    expect([none.widthTexels, none.layers, [...none.texels]]).toEqual([1, 1, [0]]);
  });
});

/** A channel's integral over the sphere, by the midpoint rule on 2 × 10⁵ angles, read by phaseAt. */
function sphereIntegral(phase: PhaseFunction, channel: 0 | 1 | 2): number {
  const n = 200_000;
  let sum = 0;
  for (let i = 0; i < n; i += 1) {
    const theta = ((i + 0.5) / n) * Math.PI;
    sum += phaseAt(phase, Math.cos(theta))[channel] * Math.sin(theta);
  }
  return 2 * Math.PI * sum * (Math.PI / n);
}

describe("the phase tables", () => {
  it("take a table already on the kernels' grid as it is", () => {
    expect(resampledPhase(EVEN_TABLE)).toBe(EVEN_TABLE);
  });

  it("resample a table on another grid onto 256 entries even in u, keeping it normalised", () => {
    const g = [0.75, 0.7, 0.65] as const;
    const uneven = phaseTable(UNEVEN_U, [
      henyeyGreenstein(g[0], UNEVEN_U),
      henyeyGreenstein(g[1], UNEVEN_U),
      henyeyGreenstein(g[2], UNEVEN_U),
    ]);
    const resampled = resampledPhase(uneven);
    expect(resampled.u).toEqual(EVEN_U);
    for (const c of [0, 1, 2] as const) {
      const source = sphereIntegral({ kind: "tabulated", table: uneven }, c);
      const kernel = sphereIntegral({ kind: "tabulated", table: resampled }, c);
      expect(Math.abs(source - 1)).toBeLessThan(2e-3);
      expect(Math.abs(kernel / source - 1)).toBeLessThan(1e-6);
      // Close to the closed form it tabulates, beyond the first entries' forward peak.
      const exact = henyeyGreenstein(g[c], EVEN_U);
      for (let i = 8; i < PHASE_TABLE_ENTRIES; i += 1) {
        const value = resampled.values[c][i] ?? Number.NaN;
        expect(Math.abs(value / (exact[i] ?? Number.NaN) - 1)).toBeLessThan(3e-3);
      }
    }
  });

  it("resample a matrix alike, each element scaled by its channel's factor", () => {
    const values = [
      henyeyGreenstein(0.7, UNEVEN_U),
      henyeyGreenstein(0.7, UNEVEN_U),
      henyeyGreenstein(0.7, UNEVEN_U),
    ] as const;
    const uneven = phaseTable(UNEVEN_U, values, {
      a2: values,
      a3: values,
      a4: values,
      b1: values,
      b2: values,
    });
    const resampled = resampledPhase(uneven);
    expect(resampled.matrix?.a3[1]).toEqual(resampled.values[1]);
    expect(resampled.matrix?.b2[2]).toEqual(resampled.values[2]);
  });

  it("pack one rgba32float layer a term, the channels in rgb", () => {
    const tables = packPhaseTables(TABULATED);
    expect([tables.widthTexels, tables.layers]).toEqual([PHASE_TABLE_ENTRIES, 3]);
    const texel = (layer: number, k: number): number[] => [
      ...tables.texels.subarray(
        (layer * PHASE_TABLE_ENTRIES + k) * 4,
        (layer * PHASE_TABLE_ENTRIES + k + 1) * 4,
      ),
    ];
    expect(texel(0, 10)).toEqual([0, 0, 0, 0]);
    expect(texel(2, 10)).toEqual(
      [EVEN_TABLE.values[0][10], EVEN_TABLE.values[1][10], EVEN_TABLE.values[2][10], 0].map((v) =>
        Math.fround(v ?? Number.NaN),
      ),
    );
  });

  it("pack a one-texel placeholder for a medium with no tabulated phase", () => {
    const none = packPhaseTables(EARTH_REFERENCE);
    expect([none.widthTexels, none.layers, [...none.texels]]).toEqual([1, 1, [0, 0, 0, 0]]);
  });
});

describe("kernelMedium", () => {
  it("resamples the tabulated terms alone", () => {
    const uneven: MediumTerm = {
      ...HAZE,
      phase: {
        kind: "tabulated",
        table: phaseTable(UNEVEN_U, [
          henyeyGreenstein(0.7, UNEVEN_U),
          henyeyGreenstein(0.7, UNEVEN_U),
          henyeyGreenstein(0.7, UNEVEN_U),
        ]),
      },
    };
    const medium: AtmosphereMedium = { ...TABULATED, terms: [GAS, ABSORBER, uneven] };
    const kernel = kernelMedium(medium);
    expect(kernel.terms[1]).toEqual(ABSORBER);
    const [gas, , haze] = kernel.terms;
    expect(gas?.density.kind === "tabulated" && gas.density.altitudesM.length).toBe(
      DENSITY_TABLE_LEVELS,
    );
    expect(haze?.phase.kind === "tabulated" && haze.phase.table.u.length).toBe(PHASE_TABLE_ENTRIES);
    expect(kernelMedium(EARTH_REFERENCE)).toEqual(EARTH_REFERENCE);
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

  it("read the terms from a storage buffer and their densities from a 2D array texture", () => {
    for (const kernel of [TRANSMITTANCE_KERNEL, MULTI_SCATTERING_KERNEL]) {
      expect(kernel.reference).toMatch(/var<storage, read> terms : array<Term, MAX_TERMS>;/);
      expect(kernel.reference).toMatch(/var densityTables : texture_2d_array<f32>;/);
      expect(kernel.reference).not.toMatch(/struct Medium \{[^}]*\bterms\b/);
    }
  });
});
