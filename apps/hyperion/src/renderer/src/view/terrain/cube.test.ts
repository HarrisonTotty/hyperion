import { describe, expect, it } from "vitest";

import golden from "../../../../../../../crates/hyperion-surface/tests/golden/cube_sphere.golden?raw";
import {
  faceOf,
  finestLevel,
  PATCH_QUADS,
  stToUv,
  unitDir,
  uvToSt,
  vertexDir,
  vertexSpacing,
  xyzToFaceUv,
} from "./cube";
import {
  cornerNeighbours,
  type Edge,
  EDGES,
  edgeNeighbourAndBack,
  type Face,
  type PatchKey,
  patchKeyString,
  patchKeyWord,
} from "./patchKey";
import { f64Digest } from "./workers/f32Digest";
import { EXPECTED_TEST_PLANET_VERSION } from "./workers/heightBake";

const scratch = new DataView(new ArrayBuffer(8));

/** The IEEE 754 bits of `x` as the golden writes them, `0x` and 16 hexadecimal digits. */
function hex(x: number): string {
  scratch.setFloat64(0, x);
  return `0x${scratch.getBigUint64(0).toString(16).padStart(16, "0")}`;
}

/** The number whose bits a golden field holds. */
function fromHex(text: string): number {
  scratch.setBigUint64(0, BigInt(text));
  return scratch.getFloat64(0);
}

function asFace(n: number): Face {
  if (n === 0 || n === 1 || n === 2 || n === 3 || n === 4 || n === 5) {
    return n;
  }
  throw new Error(`${n} is not a face`);
}

function asEdge(s: string): Edge {
  const edge = EDGES.find((e) => e === s);
  if (edge === undefined) {
    throw new Error(`${s} is not an edge`);
  }
  return edge;
}

function field(fields: readonly string[], n: number): string {
  const f = fields[n];
  if (f === undefined) {
    throw new Error(`field ${n} is missing from "${fields.join(" ")}"`);
  }
  return f;
}

function keyAt(fields: readonly string[], start: number): PatchKey {
  return {
    face: asFace(Number(field(fields, start))),
    level: Number(field(fields, start + 1)),
    i: Number(field(fields, start + 2)),
    j: Number(field(fields, start + 3)),
  };
}

const lines = golden.trimEnd().split("\n");
const records = lines.slice(1).map((line) => line.split(" "));

function recordsOf(kind: string): (readonly string[])[] {
  return records.filter((r) => r[0] === kind);
}

/** Each patch record with the records that follow it, up to the next patch or table. */
function patchBlocks(): { key: PatchKey; word: string; body: (readonly string[])[] }[] {
  const blocks: { key: PatchKey; word: string; body: (readonly string[])[] }[] = [];
  for (const r of records) {
    if (r[0] === "patch") {
      blocks.push({ key: keyAt(r, 2), word: field(r, 6), body: [] });
    } else if (r[0] === "vertex" || r[0] === "digest" || r[0] === "edge" || r[0] === "corner") {
      const last = blocks.at(-1);
      if (last === undefined) {
        throw new Error(`a ${r[0]} record comes before any patch`);
      }
      last.body.push(r);
    }
  }
  return blocks;
}

describe("the cube sphere's mirror against the Rust golden", () => {
  it("is written under the test planet's version", () => {
    expect(lines[0]).toBe(`# generator_version = ${String(EXPECTED_TEST_PLANET_VERSION)}`);
  });

  it("warps and unwarps the 1,000 values bit for bit", () => {
    const warp = recordsOf("warp");
    expect(warp).toHaveLength(1000);
    const misses = warp.filter((r) => {
      const s = fromHex(field(r, 2));
      const u = stToUv(s);
      return hex(u) !== field(r, 3) || hex(uvToSt(u)) !== field(r, 4);
    });
    expect(misses).toEqual([]);
  });

  it("packs each of the 50 patches into the same word", () => {
    const blocks = patchBlocks();
    expect(blocks).toHaveLength(50);
    for (const { key, word } of blocks) {
      expect(`0x${patchKeyWord(key).toString(16).padStart(16, "0")}`).toBe(word);
    }
  });

  it("gives every printed vertex direction bit for bit", () => {
    const misses: string[] = [];
    for (const { key, body } of patchBlocks()) {
      for (const r of body.filter((b) => b[0] === "vertex")) {
        const dir = vertexDir(key, Number(field(r, 1)), Number(field(r, 2)));
        const got = dir.map(hex).join(" ");
        const want = [field(r, 3), field(r, 4), field(r, 5)].join(" ");
        if (got !== want) {
          misses.push(`${patchKeyString(key)} (${field(r, 1)}, ${field(r, 2)}): ${got} ≠ ${want}`);
        }
      }
    }
    expect(misses).toEqual([]);
  });

  it("gives every vertex of every patch the digest of the Rust directions", () => {
    const misses: string[] = [];
    const all = new Float64Array((PATCH_QUADS + 1) * (PATCH_QUADS + 1) * 3);
    for (const { key, body } of patchBlocks()) {
      let n = 0;
      for (let y = 0; y <= PATCH_QUADS; y += 1) {
        for (let x = 0; x <= PATCH_QUADS; x += 1) {
          for (const c of vertexDir(key, x, y)) {
            all[n] = c;
            n += 1;
          }
        }
      }
      const digest = body.find((b) => b[0] === "digest");
      const got = `0x${f64Digest(all).toString(16).padStart(16, "0")}`;
      if (digest === undefined || got !== field(digest, 1)) {
        misses.push(patchKeyString(key));
      }
    }
    expect(misses).toEqual([]);
  });

  it("finds each patch's edge neighbours and the edges back", () => {
    for (const { key, body } of patchBlocks()) {
      const edges = body.filter((b) => b[0] === "edge");
      expect(edges).toHaveLength(4);
      for (const r of edges) {
        const [neighbour, back] = edgeNeighbourAndBack(key, asEdge(field(r, 1)));
        expect([patchKeyString(neighbour), back]).toEqual([
          patchKeyString(keyAt(r, 2)),
          field(r, 6),
        ]);
      }
    }
  });

  it("finds each patch's corner neighbours, none at a cube corner", () => {
    for (const { key, body } of patchBlocks()) {
      const corners = cornerNeighbours(key);
      for (const r of body.filter((b) => b[0] === "corner")) {
        const corner = corners[Number(field(r, 1))];
        const want = field(r, 2) === "none" ? null : patchKeyString(keyAt(r, 2));
        expect(corner === null || corner === undefined ? null : patchKeyString(corner)).toBe(want);
      }
    }
  });

  it("crosses all 24 directed face edges as the Rust table does", () => {
    const table = recordsOf("table");
    expect(table).toHaveLength(24);
    for (const r of table) {
      const [neighbour, back] = edgeNeighbourAndBack(keyAt(r, 1), asEdge(field(r, 5)));
      expect([patchKeyString(neighbour), back]).toEqual([
        patchKeyString(keyAt(r, 6)),
        field(r, 10),
      ]);
    }
  });

  it("agrees on the finest level and its largest spacing for the 20 radii", () => {
    const finest = recordsOf("finest");
    expect(finest).toHaveLength(20);
    for (const r of finest) {
      const radiusM = fromHex(field(r, 1));
      const level = finestLevel(radiusM);
      expect(level).toBe(Number(field(r, 2)));
      expect(hex(vertexSpacing(radiusM, level).maxM)).toBe(field(r, 3));
    }
  });
});

describe("the cube sphere's mirror", () => {
  it("makes the finest level of an Earth 19", () => {
    expect([finestLevel(6.371e6), finestLevel(6.378137e6)]).toEqual([19, 19]);
  });

  it("round-trips a direction through its face and (u, v)", () => {
    const p = unitDir([0.3, -0.5, 0.8]);
    const { face, u, v } = xyzToFaceUv(p);
    expect(face).toBe(2);
    expect(Math.abs(u + 0.3 / 0.8)).toBeLessThan(1e-15);
    expect(Math.abs(v - 0.5 / 0.8)).toBeLessThan(1e-15);
  });

  it("sends a tie to the lowest face index", () => {
    expect(faceOf([1, 1, 0])).toBe(0);
    expect(faceOf([-1, -1, -1])).toBe(3);
    expect(faceOf([0, -1, 1])).toBe(2);
  });
});
