import { describe, expect, it } from "vitest";

import vertexGolden from "../../../../../../../../crates/hyperion-surface/tests/golden/vertex_f32.golden?raw";
import type { Vec3 } from "../../../geometry/vec3";
import {
  faceDifferenceMorphF32,
  faceDifferencePositionF32,
  type SlotTermsF32,
  sourceGridF32,
  surfacePointF32,
} from "./vertexEmulation";

/** The `f32` value of `0x`-prefixed IEEE 754 bits. */
function valueOf(hex: string | undefined): number {
  const u = new Uint32Array([Number.parseInt(hex ?? "", 16)]);
  return new Float32Array(u.buffer)[0] ?? Number.NaN;
}

function bitsOf(v: Vec3): readonly [number, number, number] {
  const f = new Float32Array([v.x, v.y, v.z]);
  const [x = -1, y = -1, z = -1] = new Uint32Array(f.buffer);
  return [x, y, z];
}

function vec(words: ReadonlyArray<string>, at: number): Vec3 {
  return { x: valueOf(words[at]), y: valueOf(words[at + 1]), z: valueOf(words[at + 2]) };
}

/** The `terms` line's fields, in `tests/vertex_golden.rs`'s order. */
function termsOf(words: ReadonlyArray<string>): SlotTermsF32 {
  return {
    axisA: vec(words, 1),
    axisE1: vec(words, 4),
    axisE2: vec(words, 7),
    s0: valueOf(words[10]),
    t0: valueOf(words[11]),
    u0: valueOf(words[12]),
    v0: valueOf(words[13]),
    step: valueOf(words[14]),
    scale: vec(words, 15),
    m0: vec(words, 18),
    nu0: vec(words, 21),
    h0M: valueOf(words[24]),
    straddles: words[25] === "1",
  };
}

interface GoldenVertex {
  /** Its patch's line in the golden, and the patch's level. */
  readonly patch: number;
  readonly level: number;
  readonly terms: SlotTermsF32;
  readonly x: number;
  readonly y: number;
  readonly h0: number;
  readonly h1: number;
  readonly ha: number;
  readonly hb: number;
  readonly p: readonly [number, number, number];
  readonly q: readonly [number, number, number];
}

/** Every `vertex` line of T4.b's golden with its patch's terms. */
function goldenVertices(): GoldenVertex[] {
  const out: GoldenVertex[] = [];
  let terms: SlotTermsF32 | null = null;
  let patch = -1;
  let level = -1;
  for (const line of vertexGolden.split("\n")) {
    const words = line.trim().split(/\s+/);
    if (words[0] === "patch") {
      patch = Number(words[1]);
      level = Number(words[3]);
    } else if (words[0] === "terms") {
      terms = termsOf(words);
    } else if (words[0] === "vertex") {
      if (terms === null) {
        throw new Error("a vertex line before any terms");
      }
      const bits = (at: number): readonly [number, number, number] => [
        Number.parseInt(words[at] ?? "", 16),
        Number.parseInt(words[at + 1] ?? "", 16),
        Number.parseInt(words[at + 2] ?? "", 16),
      ];
      out.push({
        patch,
        level,
        terms,
        x: Number(words[1]),
        y: Number(words[2]),
        h0: valueOf(words[3]),
        h1: valueOf(words[4]),
        ha: valueOf(words[5]),
        hb: valueOf(words[6]),
        p: bits(7),
        q: bits(10),
      });
    }
  }
  return out;
}

/** The two even neighbours an odd vertex's morph target reads, the first the lower. */
function firstNeighbour(x: number, y: number): readonly [number, number] {
  const oddX = x % 2 === 1;
  const oddY = y % 2 === 1;
  if (oddX && !oddY) {
    return [x - 1, y];
  }
  if (!oddX) {
    return [x, y - 1];
  }
  return [x - 1, y - 1];
}

/** The `f32` vector of IEEE 754 bits. */
function vectorOf(bits: readonly [number, number, number]): Vec3 {
  const f = new Float32Array(new Uint32Array(bits).buffer);
  return { x: f[0] ?? Number.NaN, y: f[1] ?? Number.NaN, z: f[2] ?? Number.NaN };
}

/**
 * The places of sub-patch instances' grid vertices whose quads have golden corners: for a part
 * `levels` deeper at (i, j), each vertex (x, y) of 0 to 64 whose place lies in a quad of corners in
 * the golden's rows and columns 0, 1, 31, 32, 33, 63 and 64, off the quads' corners.
 */
function goldenSubPlaces(): Array<readonly [number, number]> {
  const quads = new Set([0, 31, 32, 63]);
  const out: Array<readonly [number, number]> = [];
  for (const [levels, i, j] of [
    [1, 0, 0],
    [1, 1, 1],
    [2, 1, 2],
    [3, 7, 4],
    [6, 63, 0],
  ] as const) {
    for (let y = 0; y <= 64; y += 1) {
      for (let x = 0; x <= 64; x += 1) {
        const at = sourceGridF32(levels, i, j, x, y);
        const whole = Number.isInteger(at[0]) && Number.isInteger(at[1]);
        if (!whole && quads.has(Math.floor(at[0])) && quads.has(Math.floor(at[1]))) {
          out.push(at);
        }
      }
    }
  }
  return out;
}

describe("the FaceDifferences vertex emulation", () => {
  const vertices = goldenVertices();

  it("reads twelve patches of 49 vertices from T4.b's golden", () => {
    expect(vertices).toHaveLength(12 * 49);
  });

  it("gives every golden vertex's own position bit for bit", () => {
    const wrong = vertices.filter(
      (v) => bitsOf(faceDifferencePositionF32(v.terms, v.x, v.y, v.h0)).join() !== v.p.join(),
    );
    expect(wrong.map(({ x, y }) => [x, y])).toEqual([]);
  });

  it("puts a sub-patch's vertices on its slot patch's mesh, own and morph, within 1 mm at levels 10 and 19", () => {
    const places = goldenSubPlaces();
    let worstM = 0;
    let checked = 0;
    for (const patch of new Set(vertices.map((v) => v.patch))) {
      const of = vertices.filter((v) => v.patch === patch);
      if ((of[0]?.level ?? 0) < 10) {
        continue;
      }
      const at = new Map(of.map((v) => [`${v.x} ${v.y}`, v]));
      const corner = (x: number, y: number): GoldenVertex => {
        const v = at.get(`${x} ${y}`);
        if (v === undefined) {
          throw new Error(`no golden vertex (${x}, ${y})`);
        }
        return v;
      };
      for (const place of places) {
        for (const which of ["p", "q"] as const) {
          const drawn = surfacePointF32(place, (x, y) => vectorOf(corner(x, y)[which]));
          // The mesh's own triangle through its three golden corners, in f64.
          const [qx, qy] = [Math.floor(place[0]), Math.floor(place[1])];
          const [fx, fy] = [place[0] - qx, place[1] - qy];
          const mid = fx >= fy ? corner(qx + 1, qy) : corner(qx, qy + 1);
          const weights: ReadonlyArray<readonly [GoldenVertex, number]> = [
            [corner(qx, qy), 1 - Math.max(fx, fy)],
            [mid, Math.abs(fx - fy)],
            [corner(qx + 1, qy + 1), Math.min(fx, fy)],
          ];
          const mesh = weights.reduce(
            (sum, [v, w]) => {
              const p = vectorOf(v[which]);
              return { x: sum.x + w * p.x, y: sum.y + w * p.y, z: sum.z + w * p.z };
            },
            { x: 0, y: 0, z: 0 },
          );
          worstM = Math.max(
            worstM,
            Math.hypot(drawn.x - mesh.x, drawn.y - mesh.y, drawn.z - mesh.z),
          );
          checked += 1;
        }
      }
    }
    expect(checked).toBeGreaterThan(1000);
    expect(worstM).toBeLessThan(1e-3);
  });

  it("gives every golden vertex's morph position bit for bit", () => {
    const wrong = vertices.filter((v) => {
      const [ax, ay] = firstNeighbour(v.x, v.y);
      const morphHeight = (px: number, py: number): number => {
        if (px === v.x && py === v.y) {
          return v.h1;
        }
        return px === ax && py === ay ? v.ha : v.hb;
      };
      return bitsOf(faceDifferenceMorphF32(v.terms, v.x, v.y, morphHeight)).join() !== v.q.join();
    });
    expect(wrong.map(({ x, y }) => [x, y])).toEqual([]);
  });
});
