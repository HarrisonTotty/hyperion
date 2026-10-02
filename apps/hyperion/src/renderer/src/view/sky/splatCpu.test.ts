import { describe, expect, it } from "vitest";

import { cubeTexelOf } from "./cube";
import { SPLAT_POINT_FLOATS, splatCpu, splatPoints } from "./splatCpu";

describe("cubeTexelOf", () => {
  it("maps each axis to its face's centre in WebGPU's layer order", () => {
    const axes: Array<[number, number, number]> = [
      [1, 0, 0],
      [-1, 0, 0],
      [0, 1, 0],
      [0, -1, 0],
      [0, 0, 1],
      [0, 0, -1],
    ];
    axes.forEach(([x, y, z], face) => {
      expect(cubeTexelOf(x, y, z, 4)).toEqual({ face, column: 2, row: 2 });
    });
  });

  it("orients each face as WebGPU's cube sampling does", () => {
    // +X: u = −z, v = −y; a direction up and towards −z is at the face's top right.
    expect(cubeTexelOf(1, 0.9, -0.9, 4)).toEqual({ face: 0, column: 3, row: 0 });
    // +Y: u = x, v = z.
    expect(cubeTexelOf(0.9, 1, 0.9, 4)).toEqual({ face: 2, column: 3, row: 3 });
    // −Z: u = −x, v = −y.
    expect(cubeTexelOf(0.9, -0.9, -1, 4)).toEqual({ face: 5, column: 0, row: 3 });
  });

  it("refuses a zero direction", () => {
    expect(() => cubeTexelOf(0, 0, 0, 4)).toThrow(/not a direction/);
  });
});

describe("splatCpu", () => {
  it("sums each point's illuminance into its texel, with the count in alpha", () => {
    const points = splatPoints(
      new Float32Array([1, 0, 0, 2, -0.01, -0.01, 0, 0, -1]),
      new Float32Array([1, 2, 3, 0.5, 0.5, 0.5, 4, 4, 4]),
    );
    expect(points.length).toBe(3 * SPLAT_POINT_FLOATS);
    const faces = splatCpu(points, 4);
    const centre = (2 * 4 + 2) * 4;
    expect([...(faces[0]?.subarray(centre, centre + 4) ?? [])]).toEqual([1.5, 2.5, 3.5, 2]);
    expect([...(faces[5]?.subarray(centre, centre + 4) ?? [])]).toEqual([4, 4, 4, 1]);
  });

  it("keeps the total light of the points, whatever their directions", () => {
    let state = 7;
    const next = (): number => {
      state = (state * 1_103_515_245 + 12_345) % 2_147_483_648;
      return state / 2_147_483_648;
    };
    const count = 10_000;
    const directions = Float32Array.from({ length: count * 3 }, () => next() * 2 - 1);
    const light = Float32Array.from({ length: count * 3 }, () => next() * 1e-6);
    const faces = splatCpu(splatPoints(directions, light), 16);
    let total = 0;
    let summed = 0;
    for (const value of light.filter((_, i) => i % 3 === 0)) {
      total += value;
    }
    for (const face of faces) {
      for (let texel = 0; texel < face.length; texel += 4) {
        summed += face[texel] ?? 0;
      }
    }
    expect(Math.abs(summed / total - 1)).toBeLessThan(1e-5);
  });

  it("refuses arrays that are not whole points", () => {
    expect(() => splatPoints(new Float32Array(3), new Float32Array(6))).toThrow(/whole points/);
    expect(() => splatCpu(new Float32Array(5), 4)).toThrow(/whole splat points/);
  });
});
