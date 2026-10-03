import { describe, expect, it } from "vitest";

import vertexGolden from "../../../../../../../../crates/hyperion-surface/tests/golden/vertex_f32.golden?raw";
import type { Vec3 } from "../../../geometry/vec3";
import {
  faceDifferenceMorphF32,
  faceDifferencePositionF32,
  type SlotTermsF32,
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
  for (const line of vertexGolden.split("\n")) {
    const words = line.trim().split(/\s+/);
    if (words[0] === "terms") {
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
