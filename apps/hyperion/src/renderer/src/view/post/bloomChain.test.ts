import { describe, expect, it } from "vitest";

import { GLARE_SOURCE_BYTES, levelSize, packGlareSources, packGlareTerms } from "./bloomChain";
import {
  glareSourceSolidAngleSr,
  glareSpreadTerms,
  rectangleInsideLevel,
  type GlareSource,
} from "./glare";

const DEG = Math.PI / 180;
const TERMS = glareSpreadTerms("eye", { ageYears: 25, pigmentation: 0.5 });

describe("packGlareSources", () => {
  const sun: GlareSource = {
    direction: { x: 0, y: 0.6, z: -0.8 },
    angularRadiusRad: 0.267 * DEG,
    excessLuminance: [2e9, 1e9, 5e8],
  };
  const star: GlareSource = { ...sun, angularRadiusRad: 1e-9 };

  it("lays each source out as GlareSourceGpu: three vec4f, 48 bytes", () => {
    const packed = packGlareSources([sun, star], 1e-4, TERMS);
    expect(GLARE_SOURCE_BYTES).toBe(48);
    expect(packed.length).toBe(24);
    expect([...packed.subarray(0, 4)]).toEqual([0, 0.6, -0.8, 0.267 * DEG].map(Math.fround));
    expect([...packed.subarray(4, 8)]).toEqual(
      [2e5, 1e5, 5e4, glareSourceSolidAngleSr(sun)].map(Math.fround),
    );
    expect(packed[8]).toBe(Math.fround(rectangleInsideLevel(sun.angularRadiusRad, 0.0046 * DEG)));
    expect(packed[11]).toBe(0);
  });

  it("marks a source that is a point for a term with a level of −1", () => {
    const packed = packGlareSources([star], 1, TERMS);
    expect([...packed.subarray(8, 11)]).toEqual([-1, -1, -1]);
  });
});

describe("packGlareTerms", () => {
  it("packs absent terms with amplitude 0 and scale 1", () => {
    const camera = packGlareTerms(glareSpreadTerms("camera", { ageYears: 25, pigmentation: 0 }));
    expect([...(camera["glarePoisson0"] ?? [])]).toEqual([0, 1, 0, 0]);
    expect(camera["glareBroad"]?.[2]).toBe(0);
    expect(camera["glareBroad"]?.[3]).toBe(1);
  });

  it("refuses a spread function the pass cannot evaluate", () => {
    const four = { ...TERMS, poisson: [...TERMS.poisson, ...TERMS.poisson.slice(0, 1)] };
    expect(() => packGlareTerms(four)).toThrow(/at most 3 narrow terms/);
    const twoLorentz = { ...TERMS, lorentz: [...TERMS.lorentz, ...TERMS.lorentz] };
    expect(() => packGlareTerms(twoLorentz)).toThrow(/one Lorentz/);
  });
});

describe("levelSize", () => {
  it("halves each level, rounding up, never below one texel", () => {
    expect(levelSize({ widthPx: 1920, heightPx: 1080 }, 1)).toEqual({
      widthPx: 960,
      heightPx: 540,
    });
    expect(levelSize({ widthPx: 1920, heightPx: 1080 }, 3)).toEqual({
      widthPx: 240,
      heightPx: 135,
    });
    expect(levelSize({ widthPx: 1920, heightPx: 1080 }, 4)).toEqual({ widthPx: 120, heightPx: 68 });
    expect(levelSize({ widthPx: 3, heightPx: 1 }, 5)).toEqual({ widthPx: 1, heightPx: 1 });
  });
});
