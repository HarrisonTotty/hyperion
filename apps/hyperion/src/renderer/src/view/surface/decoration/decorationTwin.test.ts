import { describe, expect, it } from "vitest";

import { WGSL_CATALOGUE } from "../../engine/catalogue";
import { DECORATION_SALT, MAX_LATTICE_LEVEL, mixOf } from "./decorationLattice";
import {
  DECORATION_TWIN_HASHES,
  DECORATION_TWIN_KERNEL,
  decorationHashCases,
  decorationTexelCases,
  HASH_CASE_WORDS,
  packHashCases,
  packTexelCases,
  TEXEL_CASE_WORDS,
  texelMix,
  texelSt,
} from "./decorationTwin";

describe("the decoration twin's cases", () => {
  it("hash 10⁴ lattice points of every salt the decoration reads, every face and every level", () => {
    const cases = decorationHashCases(DECORATION_TWIN_HASHES);
    expect(cases).toHaveLength(10_000);
    const salts = new Set(cases.map((c) => c.salt));
    for (const salt of Object.values(DECORATION_SALT)) {
      expect(salts.has(salt)).toBe(true);
    }
    expect(new Set(cases.map((c) => c.face)).size).toBe(6);
    expect(new Set(cases.map((c) => c.level)).size).toBe(MAX_LATTICE_LEVEL + 1);
    expect(cases.every((c) => c.x < 2 ** c.level && c.y < 2 ** c.level)).toBe(true);
  });

  it("are the same in every run", () => {
    expect(decorationHashCases(50)).toEqual(decorationHashCases(50));
    expect(decorationTexelCases(50)).toEqual(decorationTexelCases(50));
  });

  it("pack each hash case as six words in order", () => {
    const cases = decorationHashCases(3);
    const words = packHashCases(cases);
    expect(words).toHaveLength(3 * HASH_CASE_WORDS);
    const second = cases[1];
    expect(Array.from(words.subarray(HASH_CASE_WORDS, 2 * HASH_CASE_WORDS))).toEqual([
      second?.seed,
      second?.salt,
      second?.face,
      second?.level,
      second?.x,
      second?.y,
    ]);
  });

  it("pack each texel case as the kernel's TwinTexel lays it out", () => {
    const [first] = decorationTexelCases(1);
    if (first === undefined) {
      throw new Error("no texel case");
    }
    const buffer = packTexelCases([first]);
    expect(buffer.byteLength).toBe(TEXEL_CASE_WORDS * 4);
    const u32 = new Uint32Array(buffer);
    const f32 = new Float32Array(buffer);
    const i32 = new Int32Array(buffer);
    expect(Array.from(u32.subarray(0, 4))).toEqual([
      first.key.face,
      first.key.level,
      first.key.i,
      first.key.j,
    ]);
    expect([f32[4], f32[5]]).toEqual([first.u, first.v]);
    expect([f32[6], f32[7]]).toEqual(texelSt(first).map((x) => Math.fround(x)));
    expect([u32[8], u32[9]]).toEqual([first.seed, first.level]);
    expect([f32[10], f32[11]]).toEqual([Math.fround(first.distanceM), Math.fround(first.pxPerRad)]);
    expect([u32[12], u32[13], f32[14]]).toEqual([...first.classes, Math.fround(first.secondShare)]);
    const ripple = first.ripple ?? { p: 0, q: 0, n: 0 };
    expect(Array.from(i32.subarray(16, 20))).toEqual([ripple.p, ripple.q, ripple.n, 0]);
    expect(Array.from(f32.subarray(20, 24))).toEqual(first.scarShares.map((s) => Math.fround(s)));
    expect(f32[24]).toBe(Math.fround(first.radiusM));
  });

  it("pack a calm texel's ripple as (0, 0, 0)", () => {
    const calm = decorationTexelCases(9).find((c) => c.ripple === null);
    if (calm === undefined) {
      throw new Error("no calm case among the first nine");
    }
    const i32 = new Int32Array(packTexelCases([calm]));
    expect(Array.from(i32.subarray(16, 20))).toEqual([0, 0, 0, 0]);
  });

  it("blend each class's mix with sand's by the second share, as mixOf does", () => {
    const blended = decorationTexelCases(8).find((c) => c.secondShare > 0);
    if (blended === undefined) {
      throw new Error("no blended case among the first eight");
    }
    expect(texelMix(blended)).toEqual(mixOf(blended.classes, [0.5, 0.5]));
  });

  it("take places exact in f32, and every class, distance and ripple", () => {
    const cases = decorationTexelCases(2_048);
    expect(cases.every((c) => Math.fround(c.u) === c.u && Math.fround(c.v) === c.v)).toBe(true);
    expect(new Set(cases.map((c) => c.distanceM)).size).toBeGreaterThan(4);
    expect(cases.some((c) => c.ripple === null)).toBe(true);
    expect(cases.some((c) => c.ripple !== null && texelMix(c).ripples > 0)).toBe(true);
    expect(new Set(cases.map((c) => c.classes[0])).size).toBe(15);
  });
});

describe("the decoration twin's kernel", () => {
  it("composes the library and the detail textures' projection, its readback bit-exact", () => {
    for (const name of [
      "fn decorate(",
      "fn decoration_hash(",
      "fn detail_triplanar(",
      "fn detail_biplanar(",
    ]) {
      expect(DECORATION_TWIN_KERNEL.reference).toContain(name);
    }
    expect(DECORATION_TWIN_KERNEL.readback).toBe("bit-exact");
  });

  it("is registered in R01's catalogue", () => {
    expect(
      WGSL_CATALOGUE.some(
        (entry) => entry.kind === "compute" && entry.spec === DECORATION_TWIN_KERNEL,
      ),
    ).toBe(true);
  });
});
