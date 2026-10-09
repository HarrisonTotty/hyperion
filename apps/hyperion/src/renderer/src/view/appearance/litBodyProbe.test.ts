import { describe, expect, it } from "vitest";

import { WGSL_CATALOGUE } from "../engine/catalogue";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import { brdf, brdfFromTable } from "./brdf";
import { PHASE_TABLE_SAMPLES, PHASE_TABLE_STEP_RAD, phaseFactorTableOf } from "./law";
import {
  type BrdfProbeCase,
  expectedBrdf,
  expectedLighting,
  LIGHTING_CASE_BYTES,
  LIT_BODY_PROBE,
  packBrdfProbeCases,
  packLightingProbeCases,
  packPhaseFactorRows,
  PROBE_CASE_BYTES,
  syntheticShare,
} from "./litBodyProbe";
import { vec3 } from "../../geometry/vec3";
import { annulusEdges } from "../lighting/annuli";
import { planetshineIrradiance } from "../lighting/planetshine";
import { lawFor } from "./phase";

const LAW = lawFor([0.17, 0.14, 0.1], [0.5, 0.48, 0.46], "mercury");

const CASE: BrdfProbeCase = {
  law: LAW,
  row: 2,
  mu0: 0.8,
  mu: 0.6,
  phaseRad: 0.5,
  perTexel: null,
};

describe("the lit-body probe", () => {
  it("is registered in the catalogue as a compute kernel", () => {
    expect(
      WGSL_CATALOGUE.some((entry) => entry.kind === "compute" && entry.spec === LIT_BODY_PROBE),
    ).toBe(true);
  });

  it("carries litBody.wgsl's constants as law.ts states them", () => {
    expect(litBodyWgsl).toContain(`PHASE_TABLE_SAMPLES : u32 = ${PHASE_TABLE_SAMPLES}u;`);
    const step = /PHASE_TABLE_STEP_RAD : f32 = ([\d.]+);/u.exec(litBodyWgsl)?.[1];
    expect(Number(step)).toBeCloseTo(PHASE_TABLE_STEP_RAD, 15);
  });

  it("packs a case as the kernel's 48-byte ProbeCase", () => {
    const bytes = packBrdfProbeCases([CASE, { ...CASE, perTexel: { normalAlbedo: 0.3 } }]);
    expect(bytes.byteLength).toBe(2 * PROBE_CASE_BYTES);
    const f32 = new Float32Array(bytes);
    const u32 = new Uint32Array(bytes);
    expect(f32[0]).toBeCloseTo(LAW.a[0], 6);
    expect(f32[3]).toBe(LAW.lommelSeeligerShare);
    expect(f32[4]).toBeCloseTo(LAW.phaseExponent[0], 6);
    expect(u32[7]).toBe(2);
    expect([f32[8], f32[9], f32[10]]).toEqual([
      Math.fround(0.8),
      Math.fround(0.6),
      Math.fround(0.5),
    ]);
    expect(u32[11]).toBe(0);
    expect(u32[12 + 11]).toBe(1);
    expect(f32[12]).toBeCloseTo(0.3, 7);
  });

  it("lays each law's f out as an rgba32float row", () => {
    const texels = packPhaseFactorRows([LAW, LAW]);
    expect(texels).toHaveLength(2 * PHASE_TABLE_SAMPLES * 4);
    const { rgb } = phaseFactorTableOf(LAW);
    expect([
      texels[4 * 100],
      texels[4 * 100 + 1],
      texels[4 * 100 + 2],
      texels[4 * 100 + 3],
    ]).toEqual([rgb[300], rgb[301], rgb[302], 0]);
  });

  it("expects brdf of the law the kernel builds per texel", () => {
    const perTexel: BrdfProbeCase = { ...CASE, perTexel: { normalAlbedo: 0.25 } };
    const [expected] = expectedBrdf([perTexel]);
    const alpha = Math.fround(0.5);
    const law = {
      ...LAW,
      a: [Math.fround(0.25), Math.fround(0.25) * 0.9, Math.fround(0.25) * 0.8] as const,
      lommelSeeligerShare: syntheticShare(alpha),
    };
    // f from the borrowed row (Mercury's own L), the disc term from the per-texel A and L.
    expect(expected).toEqual(
      brdfFromTable(law, phaseFactorTableOf(LAW), Math.fround(0.8), Math.fround(0.6), alpha),
    );
    expect(expected).not.toEqual(brdf(law, Math.fround(0.8), Math.fround(0.6), alpha));
  });
});

describe("the lighting probe's cases", () => {
  const cases = packLightingProbeCases([
    { kind: "sphere", h: 3, phiRad: 0.4, horizonRad: 0.1 },
    {
      kind: "eclipse",
      starRadiusRad: 0.01,
      limbC: 0.7,
      limbAlpha: 0.6,
      annuli: 4,
      occluderRadiusRad: 0.005,
      separationRad: 0.002,
    },
    { kind: "stubs" },
    {
      kind: "planetshine",
      toSource: vec3(30, 0.5, 0),
      sourceRadius: 1,
      centreDistance: 30,
      normal: vec3(0, 1, 0),
      horizonRad: 0.05,
    },
  ]);
  const u32 = new Uint32Array(cases);
  const f32 = new Float32Array(cases);

  it("packs each case in the kernel's 64 bytes, its kind first", () => {
    expect(cases.byteLength).toBe(4 * LIGHTING_CASE_BYTES);
    expect([u32[0], u32[16], u32[32], u32[48]]).toEqual([1, 2, 3, 4]);
  });

  it("packs a planetshine case's radius, distance and horizon, then its direction and normal", () => {
    expect([f32[50], f32[51], f32[52]]).toEqual([1, 30, Math.fround(0.05)]);
    expect(Array.from(f32.subarray(56, 59))).toEqual([30, 0.5, 0]);
    expect(Array.from(f32.subarray(60, 63))).toEqual([0, 1, 0]);
  });

  it("expects planetshine's factor from its twin", () => {
    const toSource = vec3(30, 0.5, 0);
    const normal = vec3(Math.cos(1), Math.sin(1), 0);
    const [value] = expectedLighting([
      { kind: "planetshine", toSource, sourceRadius: 1, centreDistance: 30, normal, horizonRad: 0 },
    ]);
    const rounded = vec3(Math.fround(normal.x), Math.fround(normal.y), 0);
    expect(value).toEqual([planetshineIrradiance(toSource, 1, 30, rounded, 0)]);
  });

  it("packs a horizon case's H, φ and horizon", () => {
    expect([f32[2], f32[3], f32[4]]).toEqual([3, Math.fround(0.4), Math.fround(0.1)]);
  });

  it("packs an eclipse case's annuli and angles", () => {
    expect(u32[17]).toBe(4);
    expect([f32[18], f32[21], f32[22]]).toEqual([
      Math.fround(0.01),
      Math.fround(0.005),
      Math.fround(0.002),
    ]);
  });

  it("packs an eclipse case's annuli as the CPU places them", () => {
    const { edges, flux } = annulusEdges(0.7, 0.6, 4);
    expect(Array.from(f32.subarray(24, 28))).toEqual(Array.from(edges.subarray(1), Math.fround));
    expect(Array.from(f32.subarray(28, 32))).toEqual(Array.from(flux, Math.fround));
  });

  it("expects the stubs' 1, 1 and 0", () => {
    expect(expectedLighting([{ kind: "stubs" }])).toEqual([[1, 1, 0]]);
  });

  it("expects a total eclipse to leave nothing", () => {
    const [eclipse] = expectedLighting([
      {
        kind: "eclipse",
        starRadiusRad: 0.01,
        limbC: 0.7,
        limbAlpha: 0.6,
        annuli: 4,
        occluderRadiusRad: 0.02,
        separationRad: 0,
      },
    ]);
    expect(eclipse).toEqual([0]);
  });
});
