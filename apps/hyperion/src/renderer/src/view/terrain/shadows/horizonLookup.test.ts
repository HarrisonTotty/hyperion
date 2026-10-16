import { describe, expect, it } from "vitest";

import blendFixture from "../../../../../../../../crates/hyperion-surface/tests/golden/horizon/blend.golden?raw";
import encodingFixture from "../../../../../../../../crates/hyperion-surface/tests/golden/horizon/encoding.golden?raw";
import {
  HORIZON_FAR_SIDE,
  HORIZON_MAP_BYTES,
  HORIZON_TEXELS,
  HORIZON_WORDS_PER_TEXEL,
  PATCH_VERTICES_PER_SIDE,
} from "../slotLayout";
import {
  blendedHorizonRad,
  decodeHorizonRad,
  horizonElevationRad,
  type HorizonWords,
  packCodes,
  snorm16,
  unpackCodes,
} from "./horizonLookup";

/** The `f64` of 16 hexadecimal digits of bits. */
function fromBits(hex: string | undefined): number {
  const view = new DataView(new ArrayBuffer(8));
  view.setBigUint64(0, BigInt(`0x${hex ?? "nan"}`));
  return view.getFloat64(0);
}

/** The `f32` of 8 hexadecimal digits of bits. */
function fromBits32(hex: string | undefined): number {
  const view = new DataView(new ArrayBuffer(4));
  view.setUint32(0, Number.parseInt(hex?.replace(/^0x/u, "") ?? "nan", 16));
  return view.getFloat32(0);
}

/** A map of the blend fixture, its words where the fixture gives them and 0 elsewhere. */
interface FixtureMap {
  readonly key: readonly [number, number, number, number];
  readonly words: HorizonWords;
  /** `HorizonMap::elevation_at` at (x, y) and an azimuth, radians. */
  readonly elevations: Array<{ x: number; y: number; azimuth: number; elevation: number }>;
}

function blendMaps(): { readonly child: FixtureMap; readonly parent: FixtureMap } {
  const maps = new Map<string, FixtureMap>();
  const of = (name: string | undefined): FixtureMap => {
    const map = maps.get(name ?? "");
    if (map === undefined) {
      throw new Error(`the fixture names no map ${String(name)}`);
    }
    return map;
  };
  for (const line of blendFixture.split("\n")) {
    const [head, name, ...rest] = line.split(" ");
    const n = rest.map(Number);
    const words = (from: number): number[] => rest.slice(from).map((w) => Number.parseInt(w, 16));
    if (head === "patch") {
      maps.set(name ?? "", {
        key: [n[0] ?? 0, n[1] ?? 0, n[2] ?? 0, n[3] ?? 0],
        words: {
          near: new Uint32Array(PATCH_VERTICES_PER_SIDE ** 2 * HORIZON_WORDS_PER_TEXEL),
          far: new Uint32Array(HORIZON_FAR_SIDE ** 2 * HORIZON_WORDS_PER_TEXEL),
        },
        elevations: [],
      });
    } else if (head === "far") {
      const texel = (n[1] ?? 0) * HORIZON_FAR_SIDE + (n[0] ?? 0);
      of(name).words.far.set(words(2), texel * HORIZON_WORDS_PER_TEXEL);
    } else if (head === "near") {
      const texel = (n[1] ?? 0) * PATCH_VERTICES_PER_SIDE + (n[0] ?? 0);
      of(name).words.near.set(words(2), texel * HORIZON_WORDS_PER_TEXEL);
    } else if (head === "elevation") {
      of(name).elevations.push({
        x: n[0] ?? 0,
        y: n[1] ?? 0,
        azimuth: fromBits(rest[2]),
        elevation: fromBits(rest[3]),
      });
    }
  }
  return { child: of("child"), parent: of("parent") };
}

describe("the horizon map's encoding, as the shader reads it", () => {
  it("lays a slot out as 4,306 texels of four words, 68,896 B", () => {
    expect(HORIZON_TEXELS).toBe(65 * 65 + 9 * 9);
    expect(HORIZON_MAP_BYTES).toBe(68_896);
  });

  it("rounds as pack2x16snorm and packs as the crate's fixture does", () => {
    const lines = encodingFixture.split("\n").map((line) => line.split(" "));
    const snorms = lines.filter((w) => w[0] === "snorm");
    const packs = lines.filter((w) => w[0] === "pack");
    expect(snorms.length).toBeGreaterThan(700);
    expect(packs).toHaveLength(24);
    expect(snorms.map((w) => snorm16(fromBits32(w[1])))).toEqual(snorms.map((w) => Number(w[2])));
    expect(packs.map((w) => packCodes(Number(w[1]), Number(w[2])))).toEqual(
      packs.map((w) => Number.parseInt(w[3] ?? "", 16)),
    );
    expect(packs.map((w) => unpackCodes(Number.parseInt(w[3] ?? "", 16)))).toEqual(
      packs.map((w) => [Number(w[1]), Number(w[2])]),
    );
  });

  it("decodes as unpack2x16snorm, −90° at both lowest codes, rising", () => {
    expect(decodeHorizonRad(-32_768)).toBe(-Math.PI / 2);
    expect(decodeHorizonRad(-32_767)).toBe(-Math.PI / 2);
    expect(decodeHorizonRad(0)).toBe(0);
    expect(decodeHorizonRad(32_767)).toBe(Math.PI / 2);
    for (let c = -32_767; c < 32_767; c += 97) {
      expect(decodeHorizonRad(c + 1)).toBeGreaterThan(decodeHorizonRad(c));
    }
  });
});

describe("the horizon lookup and its blend (decision-r10-t8a §1)", () => {
  const { child, parent } = blendMaps();
  const [, , ci, cj] = child.key;
  const quadrant = (ci & 1) | ((cj & 1) << 1);

  it("is the crate's elevation_at at every sampled vertex of both maps", () => {
    for (const map of [child, parent]) {
      expect(map.elevations.length).toBeGreaterThan(80);
      for (const { x, y, azimuth, elevation } of map.elevations) {
        expect(Math.abs(horizonElevationRad(map.words, x, y, azimuth) - elevation)).toBeLessThan(
          1e-12,
        );
      }
    }
    // The fixture is a child and its parent.
    expect(parent.key[1]).toBe(child.key[1] - 1);
    expect([parent.key[2], parent.key[3]]).toEqual([ci >> 1, cj >> 1]);
  });

  it("is the patch's own map at k = 0 and wherever no parent's map is resident", () => {
    for (const { x, y, azimuth, elevation } of child.elevations) {
      for (const k of [0, 0.25, 1]) {
        expect(
          Math.abs(blendedHorizonRad(child.words, null, quadrant, x, y, azimuth, k) - elevation),
        ).toBeLessThan(1e-12);
      }
      expect(
        Math.abs(
          blendedHorizonRad(child.words, parent.words, quadrant, x, y, azimuth, 0) - elevation,
        ),
      ).toBeLessThan(1e-12);
    }
  });

  it("is the parent's elevation_at at k = 1 on the child's even vertices, its parent's vertices", () => {
    let checked = 0;
    for (const { x, y, azimuth } of child.elevations) {
      if (x % 2 !== 0 || y % 2 !== 0) {
        continue;
      }
      const px = ((quadrant & 1) * 64 + x) / 2;
      const py = ((quadrant >> 1) * 64 + y) / 2;
      const expected = parent.elevations.find(
        (e) => e.x === px && e.y === py && e.azimuth === azimuth,
      );
      if (expected === undefined) {
        throw new Error(`the fixture has no parent's lookup at (${String(px)}, ${String(py)})`);
      }
      const blended = blendedHorizonRad(child.words, parent.words, quadrant, x, y, azimuth, 1);
      expect(Math.abs(blended - expected.elevation)).toBeLessThan(1e-12);
      checked += 1;
    }
    // Every eighth vertex of the child, at the fixture's five azimuths.
    expect(checked).toBeGreaterThanOrEqual(81 * 5);
  });

  it("is continuous in k", () => {
    for (const { x, y, azimuth } of child.elevations.slice(0, 60)) {
      const at = (k: number): number =>
        blendedHorizonRad(child.words, parent.words, quadrant, x, y, azimuth, k);
      const span = Math.abs(at(1) - at(0));
      let previous = at(0);
      for (let n = 1; n <= 100; n += 1) {
        const now = at(n / 100);
        expect(Math.abs(now - previous)).toBeLessThanOrEqual(span / 100 + 1e-12);
        previous = now;
      }
    }
  });
});
