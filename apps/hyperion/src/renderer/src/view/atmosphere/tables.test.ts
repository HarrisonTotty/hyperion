import { describe, expect, it, vi } from "vitest";

import {
  amountValue,
  fiveLineSlotMedium,
  fourAbsorberMedium,
  lineAbsorberMedium,
} from "../../test/atmosphereFixtures";
import { forbiddenIn } from "../../test/atmosphereWgsl";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import type { AllocationEvent } from "../engine/memory";
import { EngineUnavailable } from "../engine/resilientEngine";
import {
  ABSORBER_COLUMNS,
  ABSORBER_CURVE_DECADES,
  ABSORBER_CURVE_NODES_PER_DECADE,
  ABSORBER_TRANSMITTANCE_FLOOR,
  type AbsorberCurve,
} from "./absorbers";
import { hydrostaticColumn } from "./column";
import { R05_EARTH_REFERENCE, HILLAIRE_REFERENCE } from "./earth";
import {
  ABSORBER_PRESSURE_FLOOR_PA,
  ABSORBER_PRESSURE_REFERENCE_PA,
  type AtmosphereMedium,
  columnLengthM,
  densityAt,
  layerEdgesM,
  type MediumTerm,
  type PhaseFunction,
  phaseAt,
  type PhaseTable,
  phaseTable,
  tabulatedDensity,
} from "./medium";
import commonWgsl from "./shaders/common.wgsl?raw";
import transmittanceWgsl from "./shaders/transmittance.wgsl?raw";
import {
  ABSORBER_CURVE_DEPTH_MAX,
  ABSORBER_CURVE_NODES,
  ABSORBER_SLOTS,
  absorberSlots,
  AtmosphereTables,
  DENSITY_TABLE_LEVELS,
  densityTableLevelsM,
  KERNEL_ARM_FORMAT,
  kernelMedium,
  type LayeredPlanes,
  lastPressureColumnSize,
  LAYER_EDGES_FLOATS,
  lineSlotMask,
  MAX_TERMS,
  MULTI_SCATTERING_KERNEL,
  MULTI_SCATTERING_SAMPLES,
  multiScatteringLayersOf,
  NO_ABSORBER_CURVES,
  packAbsorberCurves,
  PRESSURE_CHANNELS_A_TABLE,
  pressureChannelCount,
  pressureChannelOf,
  packDensityTables,
  packLayerEdges,
  packMedium,
  packLobeTables,
  packMediumUniform,
  packPhaseTables,
  PHASE_TABLE_ENTRIES,
  resampledDensity,
  resampledPhase,
  SHARED_TABLE_CURVE_SLOT,
  TERMS_BUFFER_BYTES,
  TRANSMITTANCE_KERNEL,
  TRANSMITTANCE_SAMPLES,
  TRANSMITTANCE_SIZE,
  withKernelArmFormat,
  withoutForwardDelta,
} from "./tables";
import { kernelOpticalDepth } from "./tablesCpu";
import {
  FORWARD_LOBE_MIN_SHARE,
  FORWARD_LOBE_X_NODES,
  FORWARD_ORDERS_ENTRIES,
  forwardLobeOfTable,
  lobeIsDrawn,
} from "./forwardLobe";

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
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, R);
    expect(tables.builds).toBe(1);
    expect(engine.dispatched.map((d) => d.kernel)).toEqual([
      "atmosphere transmittance",
      "atmosphere multiple scattering",
    ]);
    // The transmittance, absorber-column, pressure-column and multiple-scattering tables with the
    // fifth pressure channel's one texel (R08.T4.d), the four placeholders of a medium with
    // nothing tabulated and no absorber (the forward-lobe tables', R08.T7, among them), and the
    // terms buffer.
    expect(engine.counts.textures).toBe(9);
    expect(engine.counts.buffers).toBe(1);
    expect(engine.textureSpecs.every((s) => s.category === "atmosphere-tables")).toBe(true);
  });

  it("binds the terms and the density tables to both kernels, and the transmittance to the second", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, TABULATED, R);
    const [transmittance, multiScattering] = engine.dispatched;
    expect(transmittance?.bindings.buffers).toEqual({ terms: tables.terms });
    expect(transmittance?.bindings.sampled).toEqual({ densityTables: tables.densityTables });
    expect(transmittance?.bindings.storage).toEqual({
      transmittanceOut: { texture: tables.transmittance, level: 0 },
      absorberColumnsOut: { texture: tables.absorberColumns, level: 0 },
      absorberPressureColumnsOut: { texture: tables.absorberPressureColumns, level: 0 },
      lastAbsorberPressureColumnOut: { texture: tables.lastAbsorberPressureColumn, level: 0 },
    });
    expect(multiScattering?.bindings.buffers).toEqual({ terms: tables.terms });
    expect(multiScattering?.bindings.sampled).toEqual({
      densityTables: tables.densityTables,
      transmittance: tables.transmittance,
      absorberColumns: tables.absorberColumns,
      absorberPressureColumns: tables.absorberPressureColumns,
      lastAbsorberPressureColumn: tables.lastAbsorberPressureColumn,
      absorberCurves: tables.absorberCurves,
    });
    expect(transmittance?.bindings.uniforms["medium"]?.[3]).toBe(TRANSMITTANCE_SAMPLES);
    expect(multiScattering?.bindings.uniforms["medium"]?.[3]).toBe(MULTI_SCATTERING_SAMPLES);
  });

  it("hands the transmittance kernel the medium's layer edges, as the kernels read its tables (R08.T6.d)", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, TABULATED, R);
    expect(tables.builds).toBe(1);
    const [transmittance, multiScattering] = engine.dispatched;
    expect([...(transmittance?.bindings.uniforms["layerEdges"] ?? [])]).toEqual([
      ...packLayerEdges(TABULATED),
    ]);
    expect(packLayerEdges(TABULATED).some((h) => h > 0)).toBe(true);
    // R08.T6.i: the multiple-scattering kernel cuts its directions' steps at the same edges.
    expect([...(multiScattering?.bindings.uniforms["layerEdges"] ?? [])]).toEqual([
      ...packLayerEdges(TABULATED),
    ]);
    expect(tables.layerEdges).toEqual(packLayerEdges(TABULATED));
  });

  it("packs each ramp's middle height for the per-frame marches, and their count (R08.T6.i)", () => {
    const packed = packLayerEdges(TABULATED);
    expect(packed).toHaveLength(LAYER_EDGES_FLOATS);
    // The haze's two ramps, as the kernels read its table: from 0 to its peak and back to 0.
    const haze = kernelMedium(TABULATED).terms[2];
    if (haze === undefined) {
      throw new Error("the medium has no haze");
    }
    const edges = layerEdgesM(haze.density);
    expect(edges).toHaveLength(4);
    const middles = [
      0.5 * ((edges[0] ?? 0) + (edges[1] ?? 0)),
      0.5 * ((edges[2] ?? 0) + (edges[3] ?? 0)),
    ];
    const at = MAX_TERMS * 4;
    expect([...packed.subarray(at, at + 2)]).toEqual(middles.map(Math.fround));
    expect([...packed.subarray(at + 2, at + 16)].every((h) => h === -1)).toBe(true);
    expect([...packed.subarray(at + 16)]).toEqual([2, 0, 0, 0]);
    // A medium with no tabulated layer has none, and its marches take R05.T12.e's steps.
    const none = packLayerEdges(R05_EARTH_REFERENCE);
    expect([...none.subarray(at + 16)]).toEqual([0, 0, 0, 0]);
    expect([...none.subarray(0, at + 16)].every((h) => h === -1)).toBe(true);
  });

  it("makes a tabulated medium's tables at their shape and releases the placeholders", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((e) => events.push(e));
    new AtmosphereTables(engine, R05_EARTH_REFERENCE, R).setMedium(TABULATED, R);
    expect(destroyedNames(events)).toEqual([
      "atmosphere density tables",
      "atmosphere phase tables",
      "atmosphere forward-lobe tables",
    ]);
    const made = engine.textureSpecs.slice(-3).map((s) => [s.name, s.format, s.size]);
    expect(made).toEqual([
      ["atmosphere density tables", "r32float", [DENSITY_TABLE_LEVELS, 1, 3]],
      ["atmosphere phase tables", "rgba32float", [PHASE_TABLE_ENTRIES, 1, 3]],
      [
        "atmosphere forward-lobe tables",
        "rgba32float",
        [FORWARD_ORDERS_ENTRIES, FORWARD_LOBE_X_NODES, 3],
      ],
    ]);
  });

  it("uploads a tabulated medium's tables whole, and no placeholder", async () => {
    const engine = await countingRenderEngine();
    new AtmosphereTables(engine, R05_EARTH_REFERENCE, R).setMedium(TABULATED, R);
    const written = engine.textureWritten.map((w) => [w.texture, w.size]);
    expect(written).toEqual([
      ["atmosphere density tables", [DENSITY_TABLE_LEVELS, 1, 3]],
      ["atmosphere phase tables", [PHASE_TABLE_ENTRIES, 1, 3]],
      ["atmosphere forward-lobe tables", [FORWARD_ORDERS_ENTRIES, FORWARD_LOBE_X_NODES, 3]],
    ]);
  });

  it("uploads the packed terms at every build", async () => {
    const engine = await countingRenderEngine();
    new AtmosphereTables(engine, R05_EARTH_REFERENCE, R).setMedium(TABULATED, R);
    const terms = engine.writes.filter((w) => w.buffer === "atmosphere terms");
    expect(terms.map((w) => w.bytes)).toEqual([TERMS_BUFFER_BYTES, TERMS_BUFFER_BYTES]);
    expect(new Float32Array(terms[1]?.data.slice().buffer ?? new ArrayBuffer(0))).toEqual(
      packMedium(TABULATED),
    );
  });

  it("uploads a term with a forward delta as the kernels read it, the delta out (R08.T7)", async () => {
    const engine = await countingRenderEngine();
    const delta = phaseTable(
      EVEN_U,
      [
        henyeyGreenstein(0.75, EVEN_U),
        henyeyGreenstein(0.7, EVEN_U),
        henyeyGreenstein(0.65, EVEN_U),
      ],
      undefined,
      [0.2, 0.1, 0],
    );
    const peaked: AtmosphereMedium = {
      ...TABULATED,
      terms: [GAS, ABSORBER, { ...HAZE, phase: { kind: "tabulated", table: delta } }],
    };
    const tables = new AtmosphereTables(engine, peaked, R);
    expect(tables.medium).toBe(peaked);
    const terms = engine.writes.filter((w) => w.buffer === "atmosphere terms");
    const uploaded = new Float32Array(terms.at(-1)?.data.slice().buffer ?? new ArrayBuffer(0));
    const direct = { ...peaked, terms: peaked.terms.map(withoutForwardDelta) };
    expect(uploaded).toEqual(packMedium(direct));
    // The haze's scattering is its own times 1 − f: 0.8, 0.9 and 1 in its three channels.
    expect([...termOf(uploaded, 2).subarray(0, 3)]).toEqual(
      [0.8 * 3e-5, 0.9 * 3.5e-5, 4e-5].map(Math.fround),
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
      [FORWARD_ORDERS_ENTRIES, FORWARD_LOBE_X_NODES, 3],
    ]);
  });

  it("follows a medium a lost device refused, and builds it at the restore", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, R);
    const lost = vi.spyOn(engine, "createTexture").mockImplementationOnce(() => {
      throw new EngineUnavailable("createTexture");
    });
    expect(() => tables.setMedium(TABULATED, R)).toThrow(EngineUnavailable);
    lost.mockRestore();
    expect(tables.medium).toBe(TABULATED);
    engine.restore();
    expect(tables.builds).toBe(2);
    expect(engine.textureSpecs.at(-1)?.size).toEqual([
      FORWARD_ORDERS_ENTRIES,
      FORWARD_LOBE_X_NODES,
      3,
    ]);
  });

  it("returns to the placeholders for a medium with nothing tabulated, releasing the tables", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    const tables = new AtmosphereTables(engine, TABULATED, R);
    engine.onAllocation((e) => events.push(e));
    expect(tables.setMedium(R05_EARTH_REFERENCE, R)).toBe(true);
    expect(destroyedNames(events)).toEqual([
      "atmosphere density tables",
      "atmosphere phase tables",
      "atmosphere forward-lobe tables",
    ]);
    expect(engine.textureSpecs.slice(-3).map((s) => s.size)).toEqual([
      [1, 1, 1],
      [1, 1, 1],
      [1, 1, 1],
    ]);
  });

  it("does not rebuild for the same medium and radius", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, R);
    expect(tables.setMedium(R05_EARTH_REFERENCE, R)).toBe(false);
    expect(tables.setMedium({ ...R05_EARTH_REFERENCE }, R)).toBe(false);
    expect(engine.counts.dispatches).toBe(2);
  });

  it("rebuilds in place when the medium or the radius changes", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, R);
    expect(tables.setMedium(HILLAIRE_REFERENCE, R)).toBe(true);
    expect(tables.setMedium(HILLAIRE_REFERENCE, 6_360_000)).toBe(true);
    expect(tables.builds).toBe(3);
    expect(engine.counts.dispatches).toBe(6);
    expect(engine.counts.textures).toBe(9);
  });

  /** The tables on the line-absorber medium, then set to the five-line-slot one, with the events. */
  async function resizedToFive(): Promise<{
    readonly engine: Awaited<ReturnType<typeof countingRenderEngine>>;
    readonly tables: AtmosphereTables;
    readonly events: AllocationEvent[];
  }> {
    const engine = await countingRenderEngine();
    const lines = lineAbsorberMedium();
    const five = fiveLineSlotMedium();
    const tables = new AtmosphereTables(engine, lines.medium, R, lines.curves);
    const events: AllocationEvent[] = [];
    engine.onAllocation((e) => events.push(e));
    tables.setMedium(five.medium, R, five.curves);
    return { engine, tables, events };
  }

  it("makes the fifth pressure channel's table anew at the transmittance's size for five line slots (R08.T4.d)", async () => {
    const { engine, events } = await resizedToFive();
    const made = engine.textureSpecs.filter(
      (spec) => spec.name === "atmosphere last absorber pressure column",
    );
    const released = destroyedNames(events).filter(
      (name) => name === "atmosphere last absorber pressure column",
    );
    expect([released, made.map((spec) => spec.size)]).toEqual([
      ["atmosphere last absorber pressure column"],
      [
        [1, 1],
        [TRANSMITTANCE_SIZE.widthTexels, TRANSMITTANCE_SIZE.heightTexels],
      ],
    ]);
  });

  it("binds the table made anew to the transmittance kernel", async () => {
    const { engine, tables } = await resizedToFive();
    expect(engine.dispatched.at(-2)?.bindings.storage["lastAbsorberPressureColumnOut"]).toEqual({
      texture: tables.lastAbsorberPressureColumn,
      level: 0,
    });
  });

  it("returns the table to one texel for a medium of fewer line slots", async () => {
    const { engine, tables } = await resizedToFive();
    const lines = lineAbsorberMedium();
    tables.setMedium(lines.medium, R, lines.curves);
    const made = engine.textureSpecs.filter(
      (spec) => spec.name === "atmosphere last absorber pressure column",
    );
    expect(made.at(-1)?.size).toEqual([1, 1]);
  });

  it("makes the kernel arm's fifth table at the medium's size", async () => {
    const { engine, tables } = await resizedToFive();
    tables.buildKernelArm();
    const arm = engine.textureSpecs.find(
      (spec) => spec.name === "atmosphere last absorber pressure column (kernel arm)",
    );
    expect(arm?.size).toEqual([TRANSMITTANCE_SIZE.widthTexels, TRANSMITTANCE_SIZE.heightTexels]);
  });

  it("keeps the old key when a build fails, so the same medium is tried again", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, R);
    const first = R05_EARTH_REFERENCE.terms[0];
    if (first === undefined) {
      throw new Error("Earth's medium has no terms");
    }
    const tooMany: AtmosphereMedium = {
      ...R05_EARTH_REFERENCE,
      terms: Array.from({ length: MAX_TERMS + 1 }, () => first),
    };
    expect(() => tables.setMedium(tooMany, R)).toThrow(/at most 8/);
    expect(tables.builds).toBe(1);
    expect(engine.counts.bufferWrites).toBe(1);
    expect(tables.medium).toBe(R05_EARTH_REFERENCE);
    expect(tables.setMedium(R05_EARTH_REFERENCE, R)).toBe(false);
  });

  it("releases what it made when its first medium is refused", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    engine.onAllocation((e) => events.push(e));
    const above: AtmosphereMedium = { ...TABULATED, topHeightM: COLUMN.topHeightM + 1 };
    expect(() => new AtmosphereTables(engine, above, R)).toThrow(RangeError);
    expect(destroyedNames(events)).toHaveLength(10);
    engine.restore();
    expect(engine.counts.dispatches).toBe(0);
  });

  it("makes its textures and buffer again and rebuilds after the engine's restore", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, TABULATED, R);
    engine.restore();
    expect(engine.counts.textures).toBe(24);
    expect(engine.counts.buffers).toBe(2);
    expect(engine.counts.kernels).toBe(4);
    expect(tables.builds).toBe(2);
    expect(engine.dispatched.at(-1)?.bindings.uniforms["medium"]?.[0]).toBe(Math.fround(R));
    expect(engine.textureSpecs.at(-1)?.size).toEqual([
      FORWARD_ORDERS_ENTRIES,
      FORWARD_LOBE_X_NODES,
      3,
    ]);
    tables.dispose();
    engine.restore();
    expect(tables.builds).toBe(2);
  });
});

describe("packMediumUniform", () => {
  it("lays out the ground's radius, the atmosphere's height, the term count, the samples and the albedo", () => {
    const packed = packMediumUniform(R05_EARTH_REFERENCE, 6_371_000, 128);
    expect([...packed]).toEqual(
      [6_371_000, 100_000, 3, 128, 0.1, 0.1, 0.1, 0, 0, 0, 0, 0].map(Math.fround),
    );
  });

  it("carries the atmosphere's height itself, not the top's radius (R08.T6.g)", () => {
    // In f32 the top's radius less the ground's would carry both radii's rounding: 4 m apart at
    // 6e7 m. The height is narrowed once, from f64.
    const bottomM = 60_268_001.5;
    const packed = packMediumUniform(
      { ...R05_EARTH_REFERENCE, topHeightM: 100_000.37 },
      bottomM,
      1,
    );
    expect(packed[1]).toBe(Math.fround(100_000.37));
    expect(Math.fround(bottomM + 100_000.37) - Math.fround(bottomM)).not.toBe(
      Math.fround(100_000.37),
    );
  });
});

/** Term i's four vec4f in a packed `terms` buffer. */
function termOf(packed: Float32Array, i: number): Float32Array {
  return packed.subarray(i * 16, (i + 1) * 16);
}

describe("packMedium", () => {
  it("packs each term's scattering, absorption, profile and phase as four vec4f", () => {
    const packed = packMedium(R05_EARTH_REFERENCE);
    // The ozone tent is the third term.
    const ozone = termOf(packed, 2);
    expect([...ozone.subarray(0, 4)]).toEqual([0, 0, 0, 0]);
    expect(ozone[5]).toBeCloseTo(1.881e-6, 12);
    expect([...ozone.subarray(8, 12)]).toEqual([1, 10_000, 25_000, 40_000]);
    expect([...termOf(packed, 0).subarray(8, 12)]).toEqual([0, 8_434.5, 0, 0]);
  });

  it("packs each term's phase code in phase.w and its parameters in phase.xyz", () => {
    const packed = packMedium(R05_EARTH_REFERENCE);
    const phase = (i: number): number[] => [...termOf(packed, i).subarray(12, 16)];
    // Rayleigh at R05's ρ = 0, then the Cornette–Shanks aerosol, then ozone with no phase.
    expect(phase(0)).toEqual([0, 0, 0, 1]);
    expect(phase(1)[3]).toBe(2);
    expect(phase(1)[0]).toBeCloseTo(0.584, 6);
    expect(phase(2)).toEqual([0, 0, 0, 0]);
  });

  it("leaves scattering.w and absorption.w 0, where R05 kept the phase", () => {
    const packed = packMedium(R05_EARTH_REFERENCE);
    expect([termOf(packed, 1)[3], termOf(packed, 1)[7]]).toEqual([0, 0]);
  });

  it("packs a Rayleigh phase's depolarisation ratio per channel, which the kernels draw", () => {
    const [first, ...rest] = R05_EARTH_REFERENCE.terms;
    if (first === undefined) {
      throw new Error("R05_EARTH_REFERENCE has no terms");
    }
    const depolarised: AtmosphereMedium = {
      ...R05_EARTH_REFERENCE,
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
    // Its forward lobe's share per channel in xyz (R08.T7), its lobe drawn.
    const lobe = forwardLobeOfTable(EVEN_TABLE);
    expect(lobeIsDrawn(lobe)).toBe(true);
    expect([...termOf(packed, 2).subarray(12, 16)]).toEqual([...lobe.share, 3].map(Math.fround));
    expect(termOf(packed, 1)[8]).toBe(1);
  });

  it("is sized for the WGSL's MAX_TERMS terms whatever the medium: 128 floats, 512 B", () => {
    expect(packMedium(HILLAIRE_REFERENCE)).toHaveLength(MAX_TERMS * 16);
    expect(packMedium(TABULATED).byteLength).toBe(TERMS_BUFFER_BYTES);
    expect(TERMS_BUFFER_BYTES).toBe(512);
  });

  it("packs a medium of eight terms and refuses one of nine", () => {
    const eight: AtmosphereMedium = {
      ...R05_EARTH_REFERENCE,
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
    const none = packDensityTables(R05_EARTH_REFERENCE);
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
    const none = packPhaseTables(R05_EARTH_REFERENCE);
    expect([none.widthTexels, none.layers, [...none.texels]]).toEqual([1, 1, [0, 0, 0, 0]]);
  });
});

/** Texel (k, j) of a layer of packed lobe tables, rgba. */
function texelOf(tables: LayeredPlanes, layer: number, k: number, j: number): number[] {
  const at = ((layer * tables.heightTexels + j) * tables.widthTexels + k) * 4;
  return [...tables.texels.subarray(at, at + 4)];
}

describe("the forward-lobe tables (R08.T7)", () => {
  it("pack a drawn lobe's Ẽ(x, θ) at its term's layer, the orders' entries across and x down", () => {
    const tables = packLobeTables(TABULATED);
    expect([tables.widthTexels, tables.heightTexels, tables.layers]).toEqual([
      FORWARD_ORDERS_ENTRIES,
      FORWARD_LOBE_X_NODES,
      3,
    ]);
    const lobe = forwardLobeOfTable(EVEN_TABLE);
    for (const [k, j] of [
      [0, 0],
      [10, 5],
      [FORWARD_ORDERS_ENTRIES - 1, FORWARD_LOBE_X_NODES - 1],
    ] as const) {
      const [r, g, b] = lobe.table[j] ?? [];
      expect(texelOf(tables, 2, k, j)).toEqual(
        [r?.[k], g?.[k], b?.[k], 0].map((v) => Math.fround(v ?? Number.NaN)),
      );
      // The gas's Rayleigh phase and the absorber take no lobe.
      expect([texelOf(tables, 0, k, j), texelOf(tables, 1, k, j)]).toEqual([
        [0, 0, 0, 0],
        [0, 0, 0, 0],
      ]);
    }
  });

  it("pack a one-texel placeholder for a medium with no tabulated phase", () => {
    const none = packLobeTables(R05_EARTH_REFERENCE);
    expect([none.widthTexels, none.heightTexels, none.layers, [...none.texels]]).toEqual([
      1,
      1,
      1,
      [0, 0, 0, 0],
    ]);
  });

  it("draw no lobe for a forward part under FORWARD_LOBE_MIN_SHARE: zeros and no share", () => {
    const broad = phaseTable(EVEN_U, [
      henyeyGreenstein(0.2, EVEN_U),
      henyeyGreenstein(0.2, EVEN_U),
      henyeyGreenstein(0.2, EVEN_U),
    ]);
    const lobe = forwardLobeOfTable(broad);
    expect(Math.max(...lobe.share)).toBeLessThan(FORWARD_LOBE_MIN_SHARE);
    const medium: AtmosphereMedium = {
      ...TABULATED,
      terms: [GAS, ABSORBER, { ...HAZE, phase: { kind: "tabulated", table: broad } }],
    };
    const tables = packLobeTables(medium);
    expect(tables.texels.every((v) => v === 0)).toBe(true);
    expect([...termOf(packMedium(medium), 2).subarray(12, 16)]).toEqual([0, 0, 0, 3]);
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
    expect(kernelMedium(R05_EARTH_REFERENCE)).toEqual(R05_EARTH_REFERENCE);
  });

  it("resamples the pressure profile to the density tables' levels, as the kernels read it (R08.T4.d)", () => {
    const { medium } = lineAbsorberMedium();
    const pressure = kernelMedium(medium).pressureBar;
    expect(pressure?.kind === "tabulated" && pressure.altitudesM.length).toBe(DENSITY_TABLE_LEVELS);
  });

  it("keeps a medium without a pressure profile without one", () => {
    expect("pressureBar" in kernelMedium(TABULATED)).toBe(false);
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

describe("the per-planet kernels' arithmetic (R08.T6.g)", () => {
  it.each([
    ["the transmittance kernel", TRANSMITTANCE_KERNEL.reference],
    ["the multiple-scattering kernel", MULTI_SCATTERING_KERNEL.reference],
  ])("%s calls no builtin trigonometry, R05 radius form or sebh remap", (_name, source) => {
    expect(forbiddenIn(source)).toEqual([]);
  });

  it("takes view/shaders/preciseTrig.wgsl first", () => {
    for (const kernel of [TRANSMITTANCE_KERNEL, MULTI_SCATTERING_KERNEL]) {
      expect(kernel.reference.indexOf("fn preciseSinCos")).toBeLessThan(
        kernel.reference.indexOf("const MAX_TERMS"),
      );
    }
  });
});

describe("the kernel arm (R08.T6.g)", () => {
  it("assembles a kernel again writing rgba32float, under its own name, and changes nothing else", () => {
    for (const kernel of [TRANSMITTANCE_KERNEL, MULTI_SCATTERING_KERNEL]) {
      const arm = withKernelArmFormat(kernel);
      expect(arm.name).toBe(`${kernel.name} (${KERNEL_ARM_FORMAT})`);
      expect(arm.reference).not.toContain("rgba16float, write>");
      expect(arm.reference.replaceAll("rgba32float, write>", "rgba16float, write>")).toBe(
        kernel.reference,
      );
      expect(arm.readback).toBe(kernel.readback);
    }
  });

  it("refuses a kernel with no rgba16float storage texture", () => {
    expect(() =>
      withKernelArmFormat({ ...TRANSMITTANCE_KERNEL, reference: "fn main() {}" }),
    ).toThrow(/declares no rgba16float storage texture/);
  });

  it("builds both tables into rgba32float, the multiple scattering over the stored transmittance", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, 6_371_000);
    engine.dispatched.length = 0;
    const arm = tables.buildKernelArm();
    expect(engine.dispatched.map((d) => d.kernel)).toEqual([
      `atmosphere transmittance (${KERNEL_ARM_FORMAT})`,
      `atmosphere multiple scattering (${KERNEL_ARM_FORMAT})`,
    ]);
    const formats = engine.textureSpecs.slice(-3).map((spec) => spec.format);
    expect(formats).toEqual([KERNEL_ARM_FORMAT, KERNEL_ARM_FORMAT, KERNEL_ARM_FORMAT]);
    expect(engine.dispatched[0]?.bindings.storage["transmittanceOut"]?.texture).toBe(
      arm.transmittance,
    );
    expect(engine.dispatched[0]?.bindings.storage["absorberColumnsOut"]?.texture).toBe(
      arm.absorberColumns,
    );
    expect(engine.dispatched[1]?.bindings.sampled["absorberColumns"]).toBe(tables.absorberColumns);
    expect(engine.dispatched[1]?.bindings.sampled["transmittance"]).toBe(tables.transmittance);
    expect(engine.dispatched[1]?.bindings.storage["multiScatteringOut"]?.texture).toBe(
      arm.multiScattering,
    );
  });

  it("makes the arm's kernels once", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, 6_371_000);
    tables.buildKernelArm();
    const kernels = engine.counts.kernels;
    tables.buildKernelArm();
    expect(engine.counts.kernels).toBe(kernels);
  });
});

// --- Optical depth and curves of growth (plan R08, Design notes 5 and 8, R08.T6.c) ----------------

/** A `const NAME : f32 = …;` literal of `common.wgsl`. */
function wgslConstant(name: string): number {
  const found = new RegExp(`const ${name} : f32 = (-?[0-9.]+);`, "u").exec(commonWgsl);
  return Number(found?.[1] ?? Number.NaN);
}

describe("the absorber slots' and curves' constants", () => {
  it("are absorbers.ts's: its five columns, its curve grid and its floor", () => {
    const [first, last] = ABSORBER_CURVE_DECADES;
    expect([ABSORBER_SLOTS, ABSORBER_CURVE_NODES, ABSORBER_CURVE_DEPTH_MAX]).toEqual([
      ABSORBER_COLUMNS,
      (last - first) * ABSORBER_CURVE_NODES_PER_DECADE + 1,
      -Math.log(ABSORBER_TRANSMITTANCE_FLOOR),
    ]);
  });

  it("are common.wgsl's, its f64 literals narrowed to f32 once", () => {
    const [first] = ABSORBER_CURVE_DECADES;
    expect(commonWgsl).toContain(`const ABSORBER_SLOTS: u32 = ${ABSORBER_SLOTS}u;`);
    expect(wgslConstant("CURVE_FIRST_LN")).toBeCloseTo(first * Math.LN10, 14);
    expect(wgslConstant("CURVE_NODES_PER_LN")).toBeCloseTo(
      ABSORBER_CURVE_NODES_PER_DECADE / Math.LN10,
      13,
    );
    expect(wgslConstant("CURVE_DEPTH_MAX")).toBe(ABSORBER_CURVE_DEPTH_MAX);
  });

  it("take the pressure-weighted columns over medium.ts's reference, 1 bar (R08.T4.d)", () => {
    expect(wgslConstant("ABSORBER_PRESSURE_REFERENCE_PA")).toBe(ABSORBER_PRESSURE_REFERENCE_PA);
  });

  it("read a path's p̄ no lower than medium.ts's floor (R08.T4.d)", () => {
    expect(wgslConstant("ABSORBER_PRESSURE_FLOOR_PA")).toBe(ABSORBER_PRESSURE_FLOOR_PA);
  });
});

describe("the absorber slots", () => {
  const { medium } = fourAbsorberMedium();
  const absorber = medium.terms.at(-1);
  if (absorber?.absorber?.kind !== "column") {
    throw new Error("the medium ends with a column absorber");
  }
  const own = absorber.absorber;

  it("are the medium's absorber terms in their order, the pair among them", () => {
    expect(
      absorberSlots(medium).map((slot) => [slot.slot, slot.termIndex, slot.absorber.kind]),
    ).toEqual([
      [0, 2, "column"],
      [1, 3, "column"],
      [2, 4, "pair"],
      [3, 5, "column"],
    ]);
  });

  it("carry a pair absorber's n_A n_B ÷ U, though its U is beyond f32's range", () => {
    const pair = absorberSlots(medium).find((slot) => slot.absorber.kind === "pair");
    // Earth's O₂–O₂ column is about 1.2 × 10⁵³ m⁻⁵ (1.2 × 10⁴³ cm⁻⁵), f32's greatest value
    // 3.4 × 10³⁸: n ÷ U, packed, is of the column absorbers' order.
    expect(pair === undefined ? 0 : amountValue(pair.verticalAmount)).toBeGreaterThan(1e52);
    expect(Math.fround(pair?.columnPerM ?? Number.NaN)).toBeCloseTo(pair?.columnPerM ?? 0, 9);
  });

  it("carry n ÷ U, whose product with the vertical column of relative density is 1", () => {
    for (const slot of absorberSlots(medium)) {
      // 1, but for methane's 10⁻⁶ above the medium's 100 km top.
      const vertical = slot.columnPerM * columnLengthM(slot.term.density, medium.topHeightM);
      expect(vertical).toBeCloseTo(1, 5);
    }
  });

  it("are packed with each absorber's slot + 1 in absorption.w and its n ÷ U in scattering.w", () => {
    const packed = packMedium(medium);
    const slots = absorberSlots(medium);
    expect(
      slots.map((slot) => [termOf(packed, slot.termIndex)[7], termOf(packed, slot.termIndex)[3]]),
    ).toEqual(slots.map((slot) => [slot.slot + 1, Math.fround(slot.columnPerM)]));
  });

  it("leave 0 in scattering.w and absorption.w for every term that is not an absorber", () => {
    const packed = packMedium(medium);
    expect([0, 1].map((i) => [termOf(packed, i)[3], termOf(packed, i)[7]])).toEqual([
      [0, 0],
      [0, 0],
    ]);
  });

  it("refuse a sixth absorber, which no slot holds", () => {
    const six: AtmosphereMedium = {
      ...medium,
      terms: Array.from({ length: 6 }, (_, k) => ({ ...absorber, name: `absorber ${k}` })),
    };
    expect(() => packMedium(six)).toThrow(/6 absorber terms; at most 5 fit/u);
  });

  it("refuse an absorber of no number density", () => {
    const empty: AtmosphereMedium = {
      ...medium,
      terms: [{ ...absorber, absorber: { ...own, numberDensityPerM3: 0 } }],
    };
    expect(() => absorberSlots(empty)).toThrow(/a density and a vertical amount above 0/u);
  });

  it("refuse an absorber whose column is not a number", () => {
    const unknown: AtmosphereMedium = {
      ...medium,
      terms: [
        { ...absorber, absorber: { ...own, spec: { ...own.spec, columnPerM2: Number.NaN } } },
      ],
    };
    expect(() => absorberSlots(unknown)).toThrow(/a density and a vertical amount above 0/u);
  });

  it("are counted, with the curve slot, in the Medium uniform's absorbers", () => {
    expect([...packMediumUniform(medium, R, 64, 2).subarray(8)]).toEqual([4, 2, 0, 0]);
  });

  it("take the shared table's curve slot, 0, by default", () => {
    expect(SHARED_TABLE_CURVE_SLOT).toBe(0);
    expect(packMediumUniform(medium, R, 64)[9]).toBe(0);
  });
});

describe("the lights' curves as the kernels read them", () => {
  const { medium, curves } = fourAbsorberMedium();
  const own = curves[0] ?? [];

  it("stand a header row, then each light's block of its absorbers' rows (R08.T4.d)", () => {
    // Four absorbers without lines: a row each, a block of four a light.
    const table = packAbsorberCurves(medium, curves);
    expect([table.widthTexels, table.heightTexels]).toEqual([ABSORBER_CURVE_NODES, 1 + 4]);
    expect(packAbsorberCurves(medium, [own, own]).heightTexels).toBe(1 + 2 * 4);
  });

  it("head with each absorber's first row, rows and read's top, and a light's block", () => {
    const table = packAbsorberCurves(medium, curves);
    const header = (k: number): number[] => [...table.texels.subarray(k * 4, k * 4 + 4)];
    expect([header(0), header(3), header(ABSORBER_SLOTS)]).toEqual([
      [0, 1, Math.fround(own[0]?.readTopLogPressure ?? Number.NaN), 0],
      [3, 1, Math.fround(own[3]?.readTopLogPressure ?? Number.NaN), 0],
      [4, 0, 0, 0],
    ]);
  });

  it("hold each node's optical depth per channel in rgb and the row's ln p in alpha", () => {
    const table = packAbsorberCurves(medium, curves);
    for (const [k, curve] of own.entries()) {
      for (const j of [0, 120, 240]) {
        const at = ((1 + k) * ABSORBER_CURVE_NODES + j) * 4;
        expect([...table.texels.subarray(at, at + 4)]).toEqual(
          [
            curve.opticalDepth[0][j],
            curve.opticalDepth[1][j],
            curve.opticalDepth[2][j],
            curve.logPressures[0],
          ].map((v) => Math.fround(v ?? Number.NaN)),
        );
      }
    }
  });

  it("are a one-texel placeholder for a medium with no absorber", () => {
    expect(packAbsorberCurves(R05_EARTH_REFERENCE, NO_ABSORBER_CURVES).texels).toHaveLength(4);
  });

  it("are refused for a medium with absorbers given none", () => {
    expect(() => packAbsorberCurves(medium, NO_ABSORBER_CURVES)).toThrow(/no light's curves/u);
  });

  it("are refused where a light gives another count than the medium's absorbers", () => {
    expect(() => packAbsorberCurves(medium, [own.slice(1)])).toThrow(/gives 3 curves/u);
    expect(() => packAbsorberCurves(R05_EARTH_REFERENCE, [own])).toThrow(/gives 4 curves/u);
  });

  it("are refused off the kernels' grid of 241 nodes", () => {
    const [ozone, ...rest] = own;
    if (ozone === undefined) {
      throw new Error("the medium has its ozone's curve");
    }
    const coarse = { ...ozone, logColumns: ozone.logColumns.filter((_, j) => j % 2 === 0) };
    expect(() => packAbsorberCurves(medium, [[coarse, ...rest]])).toThrow(
      /not on the tables' grid/u,
    );
  });

  it.each([
    ["below 0", -1],
    ["not a number", Number.NaN],
    ["past the floor's −ln 10⁻⁶", 14],
  ])("are refused with an optical depth %s", (_, depth) => {
    const [ozone, ...rest] = own;
    if (ozone === undefined) {
      throw new Error("the medium has its ozone's curve");
    }
    const red = Float64Array.from(ozone.opticalDepth[0]);
    red[120] = depth;
    const bad: AbsorberCurve = {
      ...ozone,
      opticalDepth: [red, ozone.opticalDepth[1], ozone.opticalDepth[2]],
    };
    expect(() => packAbsorberCurves(medium, [[bad, ...rest]])).toThrow(
      /its optical depths leave \[0, /u,
    );
  });

  it("are refused with fewer optical depths than nodes", () => {
    const [ozone, ...rest] = own;
    if (ozone === undefined) {
      throw new Error("the medium has its ozone's curve");
    }
    const short = ozone.opticalDepth[0].subarray(1);
    const bad: AbsorberCurve = {
      ...ozone,
      opticalDepth: [short, ozone.opticalDepth[1], ozone.opticalDepth[2]],
    };
    expect(() => packAbsorberCurves(medium, [[bad, ...rest]])).toThrow(RangeError);
  });

  it("are refused in another absorber's slot, whose grid stands about another column", () => {
    const [ozone, methane, pair, troposphere] = own;
    if (
      ozone === undefined ||
      methane === undefined ||
      pair === undefined ||
      troposphere === undefined
    ) {
      throw new Error("the medium has its four curves");
    }
    expect(() => packAbsorberCurves(medium, [[methane, ozone, pair, troposphere]])).toThrow(
      /not on the tables' grid/u,
    );
  });

  it("are refused where a pair's curve stands in a column absorber's slot, or the reverse", () => {
    const [ozone, methane, pair, troposphere] = own;
    if (
      ozone === undefined ||
      methane === undefined ||
      pair === undefined ||
      troposphere === undefined
    ) {
      throw new Error("the medium has its four curves");
    }
    expect(() => packAbsorberCurves(medium, [[ozone, methane, troposphere, pair]])).toThrow(
      /is a column absorber's, not a pair absorber's/u,
    );
  });
});

describe("AtmosphereTables with the lights' curves", () => {
  const { medium, curves } = fourAbsorberMedium();
  const own = curves[0] ?? [];

  it("uploads the curves with the medium, a row an absorber slot", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, medium, R, curves);
    const written = engine.textureWritten.find((w) => w.texture === tables.absorberCurves.name);
    expect(written?.size).toEqual([ABSORBER_CURVE_NODES, 1 + 4, 1]);
  });

  it("follows the curves it was given", async () => {
    const engine = await countingRenderEngine();
    expect(new AtmosphereTables(engine, medium, R, curves).curves).toBe(curves);
  });

  it("does not rebuild for the same curves", async () => {
    const engine = await countingRenderEngine();
    expect(new AtmosphereTables(engine, medium, R, curves).setMedium(medium, R, curves)).toBe(
      false,
    );
  });

  it("rebuilds for new curves, its curves texture made at their shape", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, medium, R, curves);
    expect(tables.setMedium(medium, R, [own, own])).toBe(true);
    const made = engine.textureSpecs.findLast((spec) => spec.name === tables.absorberCurves.name);
    expect(made?.size).toEqual([ABSORBER_CURVE_NODES, 1 + 2 * 4]);
  });

  it("returns to the curves' placeholder for a medium with no absorber, releasing the curves", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    const tables = new AtmosphereTables(engine, medium, R, curves);
    engine.onAllocation((e) => events.push(e));
    engine.textureWritten.length = 0;
    expect(tables.setMedium(R05_EARTH_REFERENCE, R)).toBe(true);
    expect(destroyedNames(events)).toContain(tables.absorberCurves.name);
    const made = engine.textureSpecs.findLast((s) => s.name === tables.absorberCurves.name);
    expect(made?.size).toEqual([1, 1]);
    expect(engine.textureWritten.some((w) => w.texture === tables.absorberCurves.name)).toBe(false);
  });

  it("releases the old curves texture when new curves take another shape", async () => {
    const engine = await countingRenderEngine();
    const events: AllocationEvent[] = [];
    const tables = new AtmosphereTables(engine, medium, R, curves);
    engine.onAllocation((e) => events.push(e));
    tables.setMedium(medium, R, [own, own]);
    // The multiple-scattering table takes a second layer for the second light (R08.T7).
    expect(destroyedNames(events)).toEqual([
      tables.absorberCurves.name,
      tables.multiScattering.name,
    ]);
  });

  it("uploads its curves again when the engine restores", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, medium, R, curves);
    engine.textureWritten.length = 0;
    engine.restore();
    expect(tables.builds).toBe(2);
    const written = engine.textureWritten.find((w) => w.texture === tables.absorberCurves.name);
    expect(written?.size).toEqual([ABSORBER_CURVE_NODES, 1 + 4, 1]);
  });

  it("follows curves a lost device refused, and builds them at the restore", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, R);
    const lost = vi.spyOn(engine, "createTexture").mockImplementationOnce(() => {
      throw new EngineUnavailable("createTexture");
    });
    expect(() => tables.setMedium(medium, R, curves)).toThrow(EngineUnavailable);
    lost.mockRestore();
    expect(tables.curves).toBe(curves);
    engine.textureWritten.length = 0;
    engine.restore();
    expect(tables.builds).toBe(2);
    const written = engine.textureWritten.find((w) => w.texture === tables.absorberCurves.name);
    expect(written?.size).toEqual([ABSORBER_CURVE_NODES, 1 + 4, 1]);
  });

  it("builds the multiple-scattering table one layer a light slot, each with its own curves (R08.T7)", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, medium, R, [own, own]);
    const multiScattering = engine.dispatched.filter(
      (d) => d.kernel === "atmosphere multiple scattering",
    );
    expect(multiScattering.map((d) => d.bindings.uniforms["medium"]?.[9])).toEqual([
      SHARED_TABLE_CURVE_SLOT,
      1,
    ]);
    for (const dispatch of multiScattering) {
      expect(dispatch.bindings.sampled["absorberCurves"]).toBe(tables.absorberCurves);
      expect(dispatch.bindings.storage["multiScatteringOut"]?.texture).toBe(tables.multiScattering);
    }
    expect(tables.multiScatteringLayers).toBe(2);
    const made = engine.textureSpecs.findLast((spec) => spec.name === tables.multiScattering.name);
    expect(made?.size).toEqual([32, 32, 2]);
  });

  it("counts one multiple-scattering layer a light under an absorber (R08.T7)", () => {
    expect(multiScatteringLayersOf(medium, [own, own, own])).toBe(3);
  });

  it("keeps one multiple-scattering layer for a medium with no absorber, whatever its lights (R08.T7)", async () => {
    const engine = await countingRenderEngine();
    const tables = new AtmosphereTables(engine, R05_EARTH_REFERENCE, R);
    expect(tables.multiScatteringLayers).toBe(1);
    expect(multiScatteringLayersOf(R05_EARTH_REFERENCE, [[], []])).toBe(1);
    expect(
      engine.dispatched.filter((d) => d.kernel === "atmosphere multiple scattering"),
    ).toHaveLength(1);
  });
});

/** A curve's rows twice over: its nodes' values repeated, a second row. */
function doubled(d: Float64Array): Float64Array {
  return Float64Array.from([...d, ...d]);
}

/** A curve's first row of nodes alone. */
function one(d: Float64Array): Float64Array {
  return d.slice(0, ABSORBER_CURVE_NODES);
}

describe("a medium with a line absorber in the tables (R08.T4.d)", () => {
  const { medium, curves } = lineAbsorberMedium();
  /** The fixture's line slot, its pair in slot 0. */
  const LINE_SLOT = 1;
  const light = curves[0] ?? [];
  const lines = light[LINE_SLOT];
  const pair = light[0];
  const rows = lines?.logPressures.length ?? 0;
  /** The medium without its pressure profile. */
  const bare: AtmosphereMedium = {
    name: medium.name,
    topHeightM: medium.topHeightM,
    groundAlbedo: medium.groundAlbedo,
    terms: medium.terms,
  };
  /** Light slot 0's curves with the line slot's replaced. */
  const withLine = (curve: AbsorberCurve): ReadonlyArray<AbsorberCurve> =>
    light.map((c, k) => (k === LINE_SLOT ? curve : c));

  it("holds the pressure profile at the density tables' layer past the terms'", () => {
    const tables = packDensityTables(medium);
    const last = tables.texels.subarray(
      medium.terms.length * DENSITY_TABLE_LEVELS,
      (medium.terms.length + 1) * DENSITY_TABLE_LEVELS,
    );
    // p ÷ 10⁵ Pa at the ground, 101,325 Pa, falling with height.
    expect([tables.layers, last[0], (last[1] ?? 0) < (last[0] ?? 0)]).toEqual([
      medium.terms.length + 1,
      Math.fround(1.01325),
      true,
    ]);
  });

  it("flags the profile in the Medium uniform's absorbers.z", () => {
    expect([packMediumUniform(medium, R, 64)[10], packMediumUniform(bare, R, 64)[10]]).toEqual([
      1, 0,
    ]);
  });

  it("gives its line slots, slot 1 alone, as the bitmask in absorbers.w", () => {
    expect([lineSlotMask(medium), packMediumUniform(medium, R, 64)[11]]).toEqual([2, 2]);
  });

  it("packs the line curve's rows after the pair's one, each with its ln p in alpha, the header counting them", () => {
    const table = packAbsorberCurves(medium, curves);
    const header = [...table.texels.subarray(0, 8)];
    const lineFirstRow = 1 + 1;
    const firstRow = [
      ...table.texels.subarray(
        lineFirstRow * ABSORBER_CURVE_NODES * 4,
        lineFirstRow * ABSORBER_CURVE_NODES * 4 + 4,
      ),
    ];
    expect([
      rows > 4,
      header.slice(0, 2),
      header.slice(4, 6),
      table.heightTexels,
      firstRow[3],
    ]).toEqual([
      true,
      [0, 1],
      [1, rows],
      1 + 1 + rows,
      Math.fround(lines?.logPressures[0] ?? Number.NaN),
    ]);
  });

  it("refuses a curve with pressure rows on a medium with no pressure profile", () => {
    expect(() => packAbsorberCurves(bare, curves)).toThrow(/pressure profile/u);
  });

  /** The line curve with its rows' ln p and read's top replaced. */
  const withRows = (logPressures: Float64Array, readTopLogPressure: number): AbsorberCurve => {
    if (lines === undefined) {
      throw new Error("the fixture has a line absorber");
    }
    return { ...lines, logPressures, readTopLogPressure };
  };

  it("refuses a curve whose pressure rows do not ascend", () => {
    const swapped = Float64Array.from(lines?.logPressures ?? []);
    [swapped[0], swapped[1]] = [swapped[1] ?? Number.NaN, swapped[0] ?? Number.NaN];
    const bad = withRows(swapped, lines?.readTopLogPressure ?? Number.NaN);
    expect(() => packAbsorberCurves(medium, [withLine(bad)])).toThrow(/not finite and ascending/u);
  });

  it("refuses a second light whose rows differ from light slot 0's", () => {
    const moved = Float64Array.from(lines?.logPressures ?? [], (y, i) => (i === 3 ? y + 0.01 : y));
    const second = withRows(moved, lines?.readTopLogPressure ?? Number.NaN);
    expect(() => packAbsorberCurves(medium, [light, withLine(second)])).toThrow(
      /differ from light slot 0's/u,
    );
  });

  it("refuses a second light whose read's top differs from light slot 0's", () => {
    const second = withRows(
      Float64Array.from(lines?.logPressures ?? []),
      (lines?.readTopLogPressure ?? Number.NaN) - 0.5,
    );
    expect(() => packAbsorberCurves(medium, [light, withLine(second)])).toThrow(
      /differ from light slot 0's/u,
    );
  });

  it("refuses pressure rows on a slot without lines, which reads no pressure channel", () => {
    if (pair === undefined) {
      throw new Error("the fixture has a pair");
    }
    const first = pair.logPressures[0] ?? Number.NaN;
    const twoRows: AbsorberCurve = {
      ...pair,
      logPressures: Float64Array.of(first, first + 1),
      transmittance: [
        doubled(pair.transmittance[0]),
        doubled(pair.transmittance[1]),
        doubled(pair.transmittance[2]),
      ],
      opticalDepth: [
        doubled(pair.opticalDepth[0]),
        doubled(pair.opticalDepth[1]),
        doubled(pair.opticalDepth[2]),
      ],
    };
    expect(() =>
      packAbsorberCurves(medium, [light.map((c, k) => (k === 0 ? twoRows : c))]),
    ).toThrow(/on a slot without lines/u);
  });

  it("refuses a line slot's curve of one row", () => {
    if (lines === undefined) {
      throw new Error("the fixture has a line absorber");
    }
    const oneRow: AbsorberCurve = {
      ...lines,
      logPressures: lines.logPressures.slice(0, 1),
      transmittance: [
        one(lines.transmittance[0]),
        one(lines.transmittance[1]),
        one(lines.transmittance[2]),
      ],
      opticalDepth: [
        one(lines.opticalDepth[0]),
        one(lines.opticalDepth[1]),
        one(lines.opticalDepth[2]),
      ],
    };
    expect(() => packAbsorberCurves(medium, [withLine(oneRow)])).toThrow(
      /of one row on a slot with lines/u,
    );
  });
});

describe("the pressure channels (R08.T4.d)", () => {
  it("rank each line slot among the line slots, in slot order, and give the rest none", () => {
    const mask = 0b10110;
    expect([0, 1, 2, 3, 4].map((k) => pressureChannelOf(mask, k))).toEqual([-1, 0, 1, -1, 2]);
  });

  it("count one a line slot", () => {
    expect([
      pressureChannelCount(0),
      pressureChannelCount(0b10110),
      pressureChannelCount(31),
    ]).toEqual([0, 3, 5]);
  });

  it("hold four in a table", () => {
    expect(PRESSURE_CHANNELS_A_TABLE).toBe(4);
  });

  it("keep the fifth channel's table to one texel for a medium of fewer than five line slots", () => {
    expect(lastPressureColumnSize(lineAbsorberMedium().medium)).toEqual({
      widthTexels: 1,
      heightTexels: 1,
    });
  });

  it("give the fifth channel's table the transmittance's size for five line slots", () => {
    expect(lastPressureColumnSize(fiveLineSlotMedium().medium)).toEqual(TRANSMITTANCE_SIZE);
  });

  it("hold transmittance.wgsl's channels a table to tables.ts's", () => {
    expect(transmittanceWgsl).toContain(
      `const PRESSURE_CHANNELS_A_TABLE : u32 = ${PRESSURE_CHANNELS_A_TABLE}u;`,
    );
  });
});
