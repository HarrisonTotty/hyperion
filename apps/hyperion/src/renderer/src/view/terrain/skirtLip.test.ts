import { describe, expect, it } from "vitest";

import { vertexDir } from "./cube";
import { LIT_VERTEX_CODE } from "./gpu/resources";
import terrainLitWgsl from "./shaders/terrainLit.wgsl?raw";
import { MIN_SKIRT_LIP_M, shadesAsGrid, skirtLipM } from "./skirtLip";

/** The Moon-like test body's equatorial radius, m (`smoke/litTerrain.ts`' `MOON_LIKE`). */
const MOON_LIKE_A_M = 1.7374e6 / Math.cbrt(1 - 0.0012);

/** The slot record's step at level `level`: 2^−(level + 6). */
function stepAt(level: number): number {
  return 2 ** -(level + 6);
}

describe("a skirt's lip (R10.T8.c)", () => {
  it("is 1 m from the levels whose parent's quad sags less", () => {
    for (const level of [7, 10, 14, 20]) {
      expect(skirtLipM(MOON_LIKE_A_M, stepAt(level))).toBe(MIN_SKIRT_LIP_M);
    }
  });

  it("bounds a quad's length by (8 ÷ 3) a step, at a face's centre, edge and corner", () => {
    for (const level of [0, 3, 6, 9]) {
      const last = 2 ** level - 1;
      const step = stepAt(level);
      for (const [i, j] of [
        [0, 0],
        [last, 0],
        [last, last],
        [last >> 1, last >> 1],
      ] as const) {
        const key = { face: 1, level, i, j } as const;
        let longest = 0;
        for (let n = 0; n < 64; n += 1) {
          for (const [a, b] of [
            [vertexDir(key, n, 0), vertexDir(key, n + 1, 0)],
            [vertexDir(key, 0, n), vertexDir(key, 0, n + 1)],
            [vertexDir(key, n, 64), vertexDir(key, n + 1, 64)],
            [vertexDir(key, 64, n), vertexDir(key, 64, n + 1)],
          ] as const) {
            longest = Math.max(longest, Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]));
          }
        }
        expect(longest * MOON_LIKE_A_M).toBeLessThanOrEqual((8 / 3) * MOON_LIKE_A_M * step);
      }
    }
  });

  it("covers twice the sagitta of the parent's quad at every level", () => {
    for (let level = 0; level <= 22; level += 1) {
      const step = stepAt(level);
      // The parent's quad at its longest, (16 ÷ 3) a step, and its sagitta ℓ² ÷ 8a.
      const parentQuadM = (16 / 3) * MOON_LIKE_A_M * step;
      const sagittaM = (parentQuadM * parentQuadM) / (8 * MOON_LIKE_A_M);
      expect(skirtLipM(MOON_LIKE_A_M, step)).toBeGreaterThanOrEqual(2 * sagittaM * (1 - 1e-12));
    }
  });

  it("shades a foot strip as the grid within its lip, and as the reference surface below it", () => {
    const lipM = skirtLipM(MOON_LIKE_A_M, stepAt(9));
    // A crack's view of the strip, just under the edge.
    expect(shadesAsGrid(LIT_VERTEX_CODE.skirtFoot, 120, 119.5, lipM)).toBe(true);
    expect(shadesAsGrid(LIT_VERTEX_CODE.skirtFoot, 120, 120 - lipM, lipM)).toBe(true);
    // The survey edge's wall down to the datum, below the lip.
    expect(shadesAsGrid(LIT_VERTEX_CODE.skirtFoot, 120, 60, lipM)).toBe(false);
  });

  it("shades the grid as the grid, and a skirt's head strip and the fins as the reference surface", () => {
    expect(shadesAsGrid(LIT_VERTEX_CODE.grid, 120, 120, 1)).toBe(true);
    for (const kind of [
      LIT_VERTEX_CODE.skirtHead,
      LIT_VERTEX_CODE.columnFin,
      LIT_VERTEX_CODE.rowFin,
    ]) {
      expect(shadesAsGrid(kind, 120, 120, 1)).toBe(false);
    }
  });
});

describe("the shader's lip, as the twin takes it", () => {
  it("takes the same depth", () => {
    expect(terrainLitWgsl).toContain(
      `return max(${MIN_SKIRT_LIP_M.toFixed(1)}, (64.0 / 9.0) * rec.scale.x * rec.step * rec.step);`,
    );
  });

  it("shades a foot strip as the grid within its lip, as the twin does", () => {
    expect(terrainLitWgsl).toContain(
      "return in.kind == LIT_SKIRT_FOOT && in.gridHeightM - in.heightM <= in.lipM;",
    );
  });
});
