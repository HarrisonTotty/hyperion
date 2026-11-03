import { describe, expect, it } from "vitest";

import { kernelBindings } from "../engine/webgpu/compute";
import { type CountingRenderEngine, countingRenderEngine } from "../../test/countingRenderEngine";
import { EARTH_REFERENCE, EARTH_REFERENCE_CURVES, R05_EARTH_REFERENCE } from "./earth";
import {
  AERIAL_PERSPECTIVE_KERNEL,
  AERIAL_PERSPECTIVE_THERMAL_KERNEL,
  AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL,
  RAY_MARCH_DECK_THERMAL_KERNEL,
  RAY_MARCH_KERNEL,
  RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL,
  RAY_MARCH_SLOTS_THERMAL_KERNEL,
  RAY_MARCH_SLOTS_THICK_KERNEL,
  RAY_MARCH_SLOTS_THICK_THERMAL_KERNEL,
  RAY_MARCH_THERMAL_KERNEL,
  RAY_MARCH_THICK_THERMAL_KERNEL,
  SKY_VIEW_KERNEL,
  SKY_VIEW_THERMAL_KERNEL,
  SKY_VIEW_THICK_THERMAL_KERNEL,
  withThermalSource,
} from "./hillaire";
import { DISC_TABLE_FORMAT } from "./discTable";
import { AtmosphereTables, kernelMedium } from "./tables";
import type { MsSourceTable } from "./thick/bake";
import { patternedThickBake } from "../../test/thickTables";
import {
  blackBodyLuminance,
  type MediumTemperature,
  mediumTemperatureK,
  packThermalSource,
  THERMAL_SOURCE_HEIGHTS,
  type ThermalRgb,
  type ThermalScattering,
  thermalPlanckInto,
  thermalScatteringInto,
  thermalSourceOf,
} from "./thermalSource";
import thermalSourceWgsl from "./shaders/thermalSource.wgsl?raw";

/** ln s of two bands, at 0.9 and 1.1. */
const LN_BANDS = [Math.log(0.9), Math.log(1.1)];

const RISING: MediumTemperature = {
  heightsM: Float64Array.of(0, 50_000, 100_000),
  temperaturesK: Float64Array.of(1_500, 2_500, 2_000),
};

/** A band's scattered light of value `value` at every altitude and travel cosine. */
function flatScattering(value: number): ThermalScattering {
  const values = new Float32Array(3 * 16 * 3).fill(value);
  return { altitudesM: Float64Array.of(0, 40_000, 100_000), values };
}

describe("the medium's thermal source (R08.T18.b)", () => {
  it("reads the column's temperature linearly between its levels, held at its ends", () => {
    expect([
      mediumTemperatureK(RISING, -10),
      mediumTemperatureK(RISING, 25_000),
      mediumTemperatureK(RISING, 75_000),
      mediumTemperatureK(RISING, 2e5),
    ]).toEqual([1_500, 2_000, 2_250, 2_000]);
  });

  it("holds B̃ at 64 heights from 0 to the top, the top's the black body's there", () => {
    const source = thermalSourceOf(RISING, 100_000);
    expect(source.planck).toHaveLength(THERMAL_SOURCE_HEIGHTS * 3);
    expect(Array.from(source.planck.subarray(63 * 3, 64 * 3))).toEqual(
      blackBodyLuminance(2_000).map(Math.fround),
    );
  });

  it("reads B̃ linearly between its heights", () => {
    const source = thermalSourceOf(RISING, 100_000);
    const out: ThermalRgb = [0, 0, 0];
    thermalPlanckInto(source, 100_000 * (10.5 / 63), out);
    for (const c of [0, 1, 2] as const) {
      const a = source.planck[10 * 3 + c] ?? Number.NaN;
      const b = source.planck[11 * 3 + c] ?? Number.NaN;
      expect(out[c]).toBeCloseTo(0.5 * (a + b), 6);
    }
  });

  it("holds B̃ at its top above it", () => {
    const source = thermalSourceOf(RISING, 100_000);
    const out: ThermalRgb = [0, 0, 0];
    thermalPlanckInto(source, 5e5, out);
    expect(out).toEqual(blackBodyLuminance(2_000).map(Math.fround));
  });

  it("reads its scattered light linearly in ln s between the bands about s", () => {
    const source = thermalSourceOf(RISING, 100_000, [flatScattering(2), flatScattering(6)]);
    const out: ThermalRgb = [0, 0, 0];
    thermalScatteringInto(source, LN_BANDS, 1, 20_000, 0.3, out);
    // ln 1 lies at (0 − ln 0.9) ÷ (ln 1.1 − ln 0.9) of the way, 0.525.
    const w = -Math.log(0.9) / (Math.log(1.1) - Math.log(0.9));
    expect(out[0]).toBeCloseTo(2 * (1 - w) + 6 * w, 6);
  });

  it("reads its scattered light at the nearer band beyond the bands", () => {
    const source = thermalSourceOf(RISING, 100_000, [flatScattering(2), flatScattering(6)]);
    const out: ThermalRgb = [0, 0, 0];
    thermalScatteringInto(source, LN_BANDS, 2, 20_000, 0.3, out);
    expect(out[1]).toBeCloseTo(6, 6);
  });

  it("reads no scattered light without a bake's", () => {
    const out: ThermalRgb = [1, 1, 1];
    thermalScatteringInto(thermalSourceOf(RISING, 100_000), LN_BANDS, 1, 0, 0, out);
    expect(out).toEqual([0, 0, 0]);
  });

  it("packs its header, B̃ and each band's altitudes and J_th as thermalSource.wgsl reads them", () => {
    const packed = packThermalSource(thermalSourceOf(RISING, 100_000, [flatScattering(3)]));
    expect([
      Array.from(packed.subarray(0, 4)),
      packed[(1 + 64 + 1) * 4],
      packed[(1 + 64 + 3) * 4 + 1],
      packed.length,
    ]).toEqual([[100_000, 64, 1, 3], 40_000, 3, (1 + 64 + 3 * 17) * 4]);
  });
});

describe("the per-frame kernels' thermal forms (R08.T18.b)", () => {
  it.each([
    ["the sky view", SKY_VIEW_KERNEL],
    ["the aerial perspective", AERIAL_PERSPECTIVE_KERNEL],
    ["the ray march", RAY_MARCH_KERNEL],
    ["the thick ray march by slot", RAY_MARCH_SLOTS_THICK_KERNEL],
  ] as const)(
    "makes %s's from its own source, binding the thermal source beside its own",
    (_name, pair) => {
      const thermal = withThermalSource(pair);
      expect(thermal.name).toBe(`${pair.name}, thermal`);
      expect(thermal.reference.endsWith(`\n${thermalSourceWgsl}`)).toBe(true);
      expect([...kernelBindings(thermal.reference).keys()]).toEqual([
        ...kernelBindings(pair.reference).keys(),
        "thermalSource",
      ]);
      expect(thermal.reference).toContain("thermalPlanckAt(heightM) * view.marchSky.w, absorbing)");
      expect(thermal.reference).toContain("local.planck * (local.absorbing * dtM");
      expect(thermal.reference).toMatch(/let shares = thermalSharesOf\(/u);
    },
  );

  it.each([
    SKY_VIEW_THERMAL_KERNEL,
    AERIAL_PERSPECTIVE_THERMAL_KERNEL,
    RAY_MARCH_THERMAL_KERNEL,
    RAY_MARCH_SLOTS_THERMAL_KERNEL,
    SKY_VIEW_THICK_THERMAL_KERNEL,
    AERIAL_PERSPECTIVE_THICK_THERMAL_KERNEL,
    RAY_MARCH_THICK_THERMAL_KERNEL,
    RAY_MARCH_SLOTS_THICK_THERMAL_KERNEL,
    RAY_MARCH_DECK_THERMAL_KERNEL,
    RAY_MARCH_SLOTS_DECK_THERMAL_KERNEL,
  ])(
    "writes $name's targets and reads its source in signed formats, no unorm or ufloat",
    (pair) => {
      // A cool glow's green and blue are negative below about 2,000 K, so every table and target its
      // light passes through holds signed values.
      const targets = [...pair.reference.matchAll(/texture_storage_\w+<(\w+)/gu)].map((m) => m[1]);
      expect(targets.length).toBeGreaterThan(0);
      for (const format of targets) {
        expect(format).toMatch(/^rgba(16|32)float$/u);
      }
      expect(pair.reference).toContain("var<storage, read> thermalSource : array<vec4f>;");
      expect(DISC_TABLE_FORMAT).toBe("rgba16float");
    },
  );

  it("leaves the plain kernels as they were and refuses a source without its anchors", () => {
    expect(SKY_VIEW_KERNEL.reference).not.toContain("thermalSource");
    expect(SKY_VIEW_THERMAL_KERNEL.subgroup === null).toBe(SKY_VIEW_KERNEL.subgroup === null);
    expect(RAY_MARCH_THERMAL_KERNEL.readback).toBe(RAY_MARCH_KERNEL.readback);
    expect(() =>
      withThermalSource({
        ...SKY_VIEW_KERNEL,
        name: "bare",
        reference: "fn main() {}",
        subgroup: null,
      }),
    ).toThrow(/kernel bare holds 0 places/u);
  });
});

/** R05's Earth's tables over a counting engine, and the buffers live before any source. */
async function earthTables(): Promise<{
  readonly engine: CountingRenderEngine;
  readonly tables: AtmosphereTables;
  readonly buffers: number;
}> {
  const engine = await countingRenderEngine();
  const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, 6.371e6);
  return { engine, tables, buffers: engine.live.buffers };
}

/** A bake's bands, each holding scattered thermal light of 5. */
function glowingBake(bake: readonly MsSourceTable[]): MsSourceTable[] {
  return bake.map((band) => Object.assign({}, band, { thermal: flatScattering(5) }));
}

describe("the atmosphere's tables' thermal source (R08.T18.b)", () => {
  it("uploads it as one buffer and counts its bytes in the row's", async () => {
    const { engine, tables, buffers } = await earthTables();
    const base = tables.rowBytes;
    tables.setThermalSource(RISING);
    const packed = packThermalSource(thermalSourceOf(RISING, R05_EARTH_REFERENCE.topHeightM));
    expect([engine.live.buffers - buffers, tables.thermalBytes, tables.rowBytes - base]).toEqual([
      1,
      packed.byteLength,
      packed.byteLength,
    ]);
    tables.dispose();
  });

  it("releases its buffer when taken away, made again, or the medium changes", async () => {
    const { engine, tables, buffers } = await earthTables();
    tables.setThermalSource(RISING);
    tables.setThermalSource(RISING);
    expect(engine.live.buffers - buffers).toBe(1);
    tables.setThermalSource(null);
    expect([engine.live.buffers - buffers, tables.thermalSource, tables.thermalBytes]).toEqual([
      0,
      null,
      0,
    ]);
    tables.setThermalSource(RISING);
    tables.setMedium(EARTH_REFERENCE, 6.371e6, EARTH_REFERENCE_CURVES, null);
    expect([engine.live.buffers - buffers, tables.thermal]).toEqual([0, null]);
    tables.dispose();
  });

  it("releases its buffer with the tables", async () => {
    const { engine, tables } = await earthTables();
    tables.setThermalSource(RISING);
    tables.dispose();
    expect(engine.live.buffers).toBe(0);
  });

  it("folds in the thick bake's scattered thermal light where every band holds it", async () => {
    const { tables } = await earthTables();
    tables.setThermalSource(RISING);
    const bake = patternedThickBake(tables.plan.bands, kernelMedium(R05_EARTH_REFERENCE), 1);
    tables.setThickTables(glowingBake(bake));
    expect(tables.thermal?.scattering?.map((band) => band.values[0])).toEqual([5]);
    tables.dispose();
  });

  it("lets the bake's scattered light go with a bake without it", async () => {
    const { tables } = await earthTables();
    tables.setThermalSource(RISING);
    const bake = patternedThickBake(tables.plan.bands, kernelMedium(R05_EARTH_REFERENCE), 1);
    tables.setThickTables(glowingBake(bake));
    tables.setThickTables(bake);
    expect(tables.thermal?.scattering).toBeNull();
    tables.dispose();
  });

  it("makes it again after a loss", async () => {
    const { engine, tables } = await earthTables();
    tables.setThermalSource(RISING);
    const before = tables.thermalSource;
    engine.restore();
    expect([tables.thermalSource === null, tables.thermalSource === before]).toEqual([
      false,
      false,
    ]);
    tables.dispose();
  });
});
