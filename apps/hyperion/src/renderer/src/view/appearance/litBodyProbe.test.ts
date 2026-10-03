import { describe, expect, it } from "vitest";

import { WGSL_CATALOGUE } from "../engine/catalogue";
import litBodyWgsl from "../shaders/litBody.wgsl?raw";
import { brdf } from "./brdf";
import { PHASE_TABLE_SAMPLES, PHASE_TABLE_STEP_RAD, phaseFactorTableOf } from "./law";
import {
  type BrdfProbeCase,
  expectedBrdf,
  LIT_BODY_PROBE,
  packBrdfProbeCases,
  packPhaseFactorRows,
  PROBE_CASE_BYTES,
  syntheticShare,
} from "./litBodyProbe";
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
    expect(expected).toEqual(brdf(law, Math.fround(0.8), Math.fround(0.6), alpha));
  });
});
