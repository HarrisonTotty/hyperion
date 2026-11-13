import { describe, expect, it } from "vitest";

import type { ClassMapDiscSurface } from "../appearance/bodyAppearance";
import { geometricAlbedo, lawFor, lawPhaseIntegral } from "../appearance/phase";
import { packClassMap, type ClassMapTexel } from "../bodies/discSurface";
import type { TextureHandle } from "../engine/types";
import type { Rgb } from "../photometry/toneCurve";
import { classMapGroundAlbedo } from "./classMapGround";

/** The class map's texture, which the mean never reads. */
const WEIGHTS: TextureHandle = { kind: "texture", name: "class map ground test" };

/** The map's texels along a face's side. */
const N = 8;

/** A named class's law: a dark one, and a bright one of another shape. */
const DARK = lawFor([0.04, 0.05, 0.06], [1.5, 1.5, 1.5], "lambert");
const BRIGHT = lawFor([0.3, 0.25, 0.2], [0.6, 0.6, 0.6], "moon");

/** The body's uniform law, every unsurveyed texel's and an unnamed class's. */
const ELSEWHERE = lawFor([0.5, 0.5, 0.5], [1, 1, 1], "moon");

/** A class's p q per channel. */
function pqOf(law: ClassMapDiscSurface["elsewhere"]): Rgb {
  const p = geometricAlbedo(law);
  const q = lawPhaseIntegral(law);
  return [p[0] * q[0], p[1] * q[1], p[2] * q[2]];
}

/** A surface of three classes, the third unnamed, weighted at each texel by `weightsOf`. */
function surfaceOf(
  weightsOf: (texel: ClassMapTexel) => ReadonlyArray<number> | null,
): ClassMapDiscSurface {
  return {
    kind: "class-map",
    weights: WEIGHTS,
    texels: packClassMap(N, 3, weightsOf),
    laws: [DARK, BRIGHT, ELSEWHERE],
    elsewhere: ELSEWHERE,
    normals: null,
  };
}

describe("a class map's cloud-free ground (fix-r08-decked-air-opacity)", () => {
  it("is a named class's own p q where it covers every surveyed texel", () => {
    const ground = classMapGroundAlbedo(surfaceOf((t) => (t.face === 2 ? null : [1, 0, 0])));
    const pq = pqOf(DARK);
    expect(ground?.map((x, c) => x / (pq[c] ?? Number.NaN))).toEqual([
      expect.closeTo(1, 12),
      expect.closeTo(1, 12),
      expect.closeTo(1, 12),
    ]);
  });

  it("leaves out the unnamed class and the unsurveyed texels", () => {
    // Half of each surveyed texel unnamed, the brighter elsewhere's: the mean stays the named one's.
    const ground = classMapGroundAlbedo(surfaceOf((t) => (t.face === 0 ? null : [0, 0.5, 0.5])));
    const pq = pqOf(BRIGHT);
    expect(ground?.map((x, c) => x / (pq[c] ?? Number.NaN))).toEqual([
      expect.closeTo(1, 12),
      expect.closeTo(1, 12),
      expect.closeTo(1, 12),
    ]);
  });

  it("weights two classes by the solid angle each covers, not by their texels' count", () => {
    // The dark class on each face's corner texels, which subtend least, the bright one elsewhere.
    const corner = (t: ClassMapTexel): boolean =>
      (t.i === 0 || t.i === N - 1) && (t.j === 0 || t.j === N - 1);
    const ground = classMapGroundAlbedo(surfaceOf((t) => (corner(t) ? [1, 0, 0] : [0, 1, 0])));
    const dark = pqOf(DARK);
    const bright = pqOf(BRIGHT);
    const byCount = (4 * dark[0] + (N * N - 4) * bright[0]) / (N * N);
    expect(ground).not.toBeNull();
    // The corners' share of a face's solid angle is under their share of its texels.
    expect((ground?.[0] ?? Number.NaN) - byCount).toBeGreaterThan(0);
    expect(ground?.[0]).toBeLessThan(bright[0]);
  });

  it("gives none where no named class covers a surveyed texel", () => {
    expect(classMapGroundAlbedo(surfaceOf(() => null))).toBeNull();
    expect(classMapGroundAlbedo(surfaceOf(() => [0, 0, 1]))).toBeNull();
  });

  it("forms one ground a map", () => {
    const surface = surfaceOf(() => [1, 0, 0]);
    const ground = classMapGroundAlbedo(surface);
    expect(classMapGroundAlbedo(surface)).toBe(ground);
  });

  it("forms the ground again for the same map under other laws", () => {
    const surface = surfaceOf(() => [1, 0, 0]);
    classMapGroundAlbedo(surface);
    expect(
      classMapGroundAlbedo({ ...surface, laws: [BRIGHT, BRIGHT, ELSEWHERE] })?.[0],
    ).toBeCloseTo(pqOf(BRIGHT)[0], 12);
  });

  it("forms the ground again for the same map under another elsewhere", () => {
    // DARK as the map's elsewhere: its class is unnamed, and no named class covers a texel.
    const surface = surfaceOf(() => [1, 0, 0]);
    classMapGroundAlbedo(surface);
    expect(classMapGroundAlbedo({ ...surface, elsewhere: DARK })).toBeNull();
  });
});
