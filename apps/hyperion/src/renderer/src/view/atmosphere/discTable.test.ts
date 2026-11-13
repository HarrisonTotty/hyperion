import { describe, expect, it } from "vitest";

import { normalise, vec3 } from "../../geometry/vec3";
import {
  DISC_AZIMUTHS,
  DISC_LAYER_BYTES,
  DISC_LAYER_VALUES,
  DISC_SUN_COSINES,
  DISC_SUN_ROWS_BELOW,
  DISC_TABLE_HEIGHT,
  DISC_TABLE_WIDTH,
  DISC_VIEW_COSINES,
  discAzimuthAt,
  discAzimuthOf,
  discAzimuthPlace,
  type DiscReflectanceTable,
  discSunCosineAt,
  discSunPlace,
  discTableAt,
  discTableBytes,
  discTableHalves,
  discTableValuesOf,
  discTexelIndex,
  discViewCosineAt,
  discViewPlace,
} from "./discTable";
import discAirWgsl from "./shaders/discAir.wgsl?raw";
import { fromHalfBits } from "../sky/half";

/** A table of `bands` bands and `slots` slots whose texels follow a smooth pattern, as halves. */
function patterned(bands: number, slots: number): DiscReflectanceTable {
  const values = new Float32Array(bands * slots * DISC_LAYER_VALUES);
  for (let layer = 0; layer < bands * slots; layer += 1) {
    for (let s = 0; s < DISC_SUN_COSINES; s += 1) {
      for (let v = 0; v < DISC_VIEW_COSINES; v += 1) {
        for (let k = 0; k < DISC_AZIMUTHS; k += 1) {
          const at = discTexelIndex(layer, s, v, k);
          for (let c = 0; c < 3; c += 1) {
            values[at + c] =
              0.05 + 0.02 * (layer + 1) * (1 + Math.sin(0.3 * s + 0.7 * v + 0.5 * k + c));
          }
          values[at + 3] = 0.4 + (0.5 * v) / 15;
        }
      }
    }
  }
  return { bands, slots, sunFloor: -0.18, values: discTableValuesOf(values) };
}

describe("the disc table's grid (R08.T16.b)", () => {
  it("places each node at its own index and is discAir.wgsl's", () => {
    const floor = -0.17;
    for (let v = 0; v < DISC_VIEW_COSINES; v += 1) {
      expect(discViewPlace(discViewCosineAt(v))).toBeCloseTo(v, 12);
    }
    for (let s = 0; s < DISC_SUN_COSINES; s += 1) {
      expect(discSunPlace(discSunCosineAt(s, floor), floor)).toBeCloseTo(s, 12);
    }
    for (let k = 0; k < DISC_AZIMUTHS; k += 1) {
      expect(discAzimuthPlace(discAzimuthAt(k))).toBeCloseTo(k, 12);
    }
    // Held at the ends: under the floor, the floor's row; past the vertical, the last.
    expect([discSunPlace(-0.5, floor), discSunPlace(1.2, floor), discViewPlace(-0.1)]).toEqual([
      0,
      DISC_SUN_COSINES - 1,
      0,
    ]);
    expect(discAirWgsl).toContain(`const DISC_VIEW_COSINES : u32 = ${DISC_VIEW_COSINES}u;`);
    expect(discAirWgsl).toContain(`const DISC_SUN_ROWS_BELOW : u32 = ${DISC_SUN_ROWS_BELOW}u;`);
    expect(discAirWgsl).toContain(`const DISC_SUN_COSINES : u32 = ${DISC_SUN_COSINES}u;`);
    expect(discAirWgsl).toContain(`const DISC_AZIMUTHS : u32 = ${DISC_AZIMUTHS}u;`);
    expect([DISC_TABLE_WIDTH, DISC_TABLE_HEIGHT, DISC_LAYER_BYTES]).toEqual([208, 32, 53_248]);
    expect(discTableBytes({ bands: 4, slots: 2 })).toBe(8 * 53_248);
  });

  it("measures Δφ between the horizontal parts of the view and the light, 0 where either is vertical", () => {
    const n = vec3(0, 0, 1);
    const v = normalise(vec3(1, 0, 1));
    expect(discAzimuthOf(n, v, normalise(vec3(1, 0, 0.3)))).toBeCloseTo(0, 12);
    expect(discAzimuthOf(n, v, normalise(vec3(0, -2, 0.3)))).toBeCloseTo(Math.PI / 2, 12);
    expect(discAzimuthOf(n, v, normalise(vec3(-1, 0, 0.3)))).toBeCloseTo(Math.PI, 12);
    expect(discAzimuthOf(n, n, normalise(vec3(1, 1, 1)))).toBe(0);
  });
});

describe("the disc table's read (R08.T16.b)", () => {
  it("holds half floats, reads each node's texel, and is linear between nodes", () => {
    const table = patterned(1, 1);
    const halves = discTableHalves(table);
    expect(Array.from(halves, (bits) => fromHalfBits(bits))).toEqual(Array.from(table.values));
    const at = (s: number, v: number, k: number): readonly [number, number, number, number] =>
      discTableAt(
        table,
        [0],
        1,
        0,
        discViewCosineAt(v),
        discSunCosineAt(s, table.sunFloor),
        discAzimuthAt(k),
      );
    const node = at(10, 4, 3);
    const texel = discTexelIndex(0, 10, 4, 3);
    for (const c of [0, 1, 2, 3] as const) {
      expect(node[c]).toBeCloseTo(table.values[texel + c] ?? Number.NaN, 12);
    }
    // Halfway between two azimuth nodes, their mean.
    const mid = discTableAt(
      table,
      [0],
      1,
      0,
      discViewCosineAt(4),
      discSunCosineAt(10, table.sunFloor),
      0.5 * (discAzimuthAt(3) + discAzimuthAt(4)),
    );
    const next = discTexelIndex(0, 10, 4, 4);
    for (const c of [0, 1, 2, 3] as const) {
      expect(mid[c]).toBeCloseTo(
        0.5 * ((table.values[texel + c] ?? 0) + (table.values[next + c] ?? 0)),
        12,
      );
    }
  });

  it("reads a light slot's own layers, its last past them, and its bands linear in ln s", () => {
    const table = patterned(2, 2);
    const lnBands = [Math.log(0.95), Math.log(1.05)];
    const read = (s: number, slot: number): number =>
      discTableAt(table, lnBands, s, slot, 1, 1, 0)[0];
    const texel = (layer: number): number =>
      table.values[discTexelIndex(layer, DISC_SUN_COSINES - 1, DISC_VIEW_COSINES - 1, 0)] ?? 0;
    expect([read(0.9, 0), read(0.95, 1), read(1.05, 0), read(1.2, 1)]).toEqual([
      texel(0),
      texel(1),
      texel(2),
      texel(3),
    ]);
    expect(read(1.2, 5)).toBe(texel(3));
    const w = -Math.log(0.95) / (Math.log(1.05) - Math.log(0.95));
    expect(read(1, 0)).toBeCloseTo((1 - w) * texel(0) + w * texel(2), 12);
  });
});
