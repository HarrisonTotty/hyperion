import { describe, expect, it } from "vitest";

import { add, cross, dot, normalise, scale, type Vec3, vec3 } from "../../geometry/vec3";
import { countingRenderEngine } from "../../test/countingRenderEngine";
import { faceDirection, octahedralPair } from "../../test/normalMapFixtures";
import { textureBytes } from "../engine/memory";
import { fieldCellIndex } from "../terrain/coverage";
import { EDGES, type Edge, edgeNeighbour, type Face } from "../terrain/patchKey";
import { decodeOctahedral } from "../terrain/shadingNormal";
import {
  isNoNormal,
  NO_NORMAL,
  NO_NORMAL_ABOVE,
  NORMAL_MAP_FORMAT,
  type NormalMapTexels,
  normalMapNormalAt,
  normalMapTexture,
  normalMapTextureSpec,
  packNormalMap,
} from "./discNormals";
import { classMapSide } from "./discSurface";

const FACES: ReadonlyArray<Face> = [0, 1, 2, 3, 4, 5];

/** The field levels a normal map is made at, and their N. */
const LEVELS = [5, 6, 7, 8] as const;

/** One cell of a map to pack: its place and its pair. */
interface Cell {
  readonly face: Face;
  readonly i: number;
  readonly j: number;
  readonly pair: readonly [number, number];
}

/** `cells` packed at `level`, their indices in rising order as a worker answers them. */
function pack(level: number, cells: ReadonlyArray<Cell>): NormalMapTexels {
  const indexed = cells
    .map((c) => ({ c, index: fieldCellIndex({ face: c.face, level, i: c.i, j: c.j }) }))
    .toSorted((a, b) => a.index - b.index);
  return packNormalMap(
    level,
    Uint32Array.from(indexed.map(({ index }) => index)),
    Float16Array.from(indexed.flatMap(({ c }) => c.pair)),
  );
}

/**
 * Whether `bytes` are `values`' bytes, compared by a loop: a deep equality over a level-8 map's
 * 1.6 MB takes seconds.
 */
function sameBytes(bytes: Uint8Array | undefined, values: Float16Array): boolean {
  const expected = new Uint8Array(values.buffer, values.byteOffset, values.byteLength);
  return bytes?.length === expected.length && bytes.every((b, k) => b === expected[k]);
}

/** The pair stored at texel (x, y) of `face`'s layer, as the layout documents it. */
function storedPair(map: NormalMapTexels, face: Face, x: number, y: number): [number, number] {
  const side = classMapSide(map.faceTexels);
  const at = ((face * side + y) * side + x) * 2;
  return [map.data[at] ?? Number.NaN, map.data[at + 1] ?? Number.NaN];
}

/** The decoded normal of the half-float pair `pair` is stored as. */
function storedNormal(pair: readonly [number, number]): Vec3 {
  const [x, y] = Float16Array.from(pair);
  return decodeOctahedral(x ?? Number.NaN, y ?? Number.NaN);
}

/** The angle between unit vectors `a` and `b`, rad. */
function angle(a: Vec3, b: Vec3): number {
  const c = cross(a, b);
  return Math.atan2(Math.hypot(c.x, c.y, c.z), dot(a, b));
}

/**
 * The normal of a ground tilted from the radial at the centre of cell (i, j) of `face` at N: a tilt
 * of up to 20° that changes from cell to cell, so that every texel's normal is its own.
 */
function tiltedNormal(face: Face, i: number, j: number, n: number): Vec3 {
  const radial = normalise(vec3(...faceDirection(face, (i + 0.5) / n, (j + 0.5) / n)));
  const helper = Math.abs(radial.z) < 0.9 ? vec3(0, 0, 1) : vec3(1, 0, 0);
  const east = normalise(cross(helper, radial));
  const tilt = ((((face * 7 + i * 3 + j * 5) % 11) - 5) / 5) * (20 * (Math.PI / 180));
  return normalise(add(scale(radial, Math.cos(tilt)), scale(east, Math.sin(tilt))));
}

/** Every cell of `face` at N surveyed, each with its {@link tiltedNormal}. */
function surveyedFace(
  face: Face,
  n: number,
  keep: (i: number, j: number) => boolean = () => true,
): Cell[] {
  const cells: Cell[] = [];
  for (let j = 0; j < n; j += 1) {
    for (let i = 0; i < n; i += 1) {
      if (keep(i, j)) {
        cells.push({ face, i, j, pair: octahedralPair(tiltedNormal(face, i, j, n)) });
      }
    }
  }
  return cells;
}

describe("a normal map's packing (R10.T10.g)", () => {
  it.each(LEVELS)(
    "lays out level %i as six layers of N + 2 texels a side, each face's gutter the edge texels across its edges, the corners and unsurveyed texels NO_NORMAL",
    (level) => {
      const n = 2 ** level;
      // Every cell its own pair, exact in a half float: x the face and row, y the column.
      const id = (face: Face, i: number, j: number): [number, number] => [
        (face * n + j) / (8 * n),
        i / n,
      ];
      // Every cell surveyed but those of row 1, whose gutter texels must then be NO_NORMAL too.
      const cells: Cell[] = [];
      for (const face of FACES) {
        for (let j = 0; j < n; j += 1) {
          for (let i = 0; i < n; i += 1) {
            if (j !== 1) {
              cells.push({ face, i, j, pair: id(face, i, j) });
            }
          }
        }
      }
      const map = pack(level, cells);
      expect(map.faceTexels).toBe(n);
      expect(map.data).toHaveLength(6 * (n + 2) * (n + 2) * 2);
      const wrong: string[] = [];
      const expectPair = (
        label: string,
        found: readonly number[],
        wanted: readonly number[],
      ): void => {
        if (found[0] !== wanted[0] || found[1] !== wanted[1]) {
          wrong.push(`${label}: (${String(found)}), not (${String(wanted)})`);
        }
      };
      const gutter = (edge: Edge, at: number): readonly [number, number, number, number] => {
        // The face texel beside the gutter texel, and the gutter texel's stored place.
        switch (edge) {
          case "UMin":
            return [0, at, 0, at + 1];
          case "UMax":
            return [n - 1, at, n + 1, at + 1];
          case "VMin":
            return [at, 0, at + 1, 0];
          case "VMax":
            return [at, n - 1, at + 1, n + 1];
        }
        throw new Error(`no edge ${String(edge)}`);
      };
      for (const face of FACES) {
        for (let j = 0; j < n; j += 1) {
          for (let i = 0; i < n; i += 1) {
            expectPair(
              `face ${face} (${i}, ${j})`,
              storedPair(map, face, i + 1, j + 1),
              j === 1 ? NO_NORMAL : id(face, i, j),
            );
          }
        }
        for (const edge of EDGES) {
          for (let at = 0; at < n; at += 1) {
            const [i, j, x, y] = gutter(edge, at);
            // The quadtree's own neighbour across the edge (R05's `edgeNeighbour`), independently.
            const across = edgeNeighbour({ face, level, i, j }, edge);
            const wanted = across.j === 1 ? NO_NORMAL : id(across.face, across.i, across.j);
            expectPair(`face ${face} ${edge} ${at}`, storedPair(map, face, x, y), wanted);
          }
        }
        for (const [x, y] of [
          [0, 0],
          [n + 1, 0],
          [0, n + 1],
          [n + 1, n + 1],
        ] as const) {
          expectPair(`face ${face} corner (${x}, ${y})`, storedPair(map, face, x, y), NO_NORMAL);
        }
      }
      expect(wrong).toEqual([]);
    },
  );

  it("marks no normal by a first coordinate past 1.5, which no real pair reaches", () => {
    expect([
      NO_NORMAL,
      NO_NORMAL_ABOVE,
      isNoNormal(2),
      isNoNormal(-2),
      isNoNormal(1),
      isNoNormal(-1),
    ]).toEqual([[2, 2], 1.5, true, true, false, false]);
    expect(Float16Array.from(NO_NORMAL)).toEqual(Float16Array.of(2, 2));
  });

  it.each([
    { name: "of another level", args: [4, [0], [0, 0]] as const, message: /5 to 8, not 4/u },
    {
      name: "of pairs not two a cell",
      args: [5, [0, 1], [0, 0, 0]] as const,
      message: /2 cells has 3 pair values/u,
    },
    {
      name: "of a cell past the level's",
      args: [5, [6 * 4 ** 5], [0, 0]] as const,
      message: /no cell 6144/u,
    },
    {
      name: "of a coordinate outside [-1, 1]",
      args: [5, [3], [1.25, 0]] as const,
      message: /cell 3's pair/u,
    },
  ])("refuses a map $name", ({ args, message }) => {
    const [level, cells, pairs] = args;
    expect(() => packNormalMap(level, cells, pairs)).toThrow(message);
  });

  it.each(LEVELS)(
    "uploads level %i's map as an rg16float 2D array of six layers, gutter included, under coarse-field-gpu",
    async (level) => {
      const n = 2 ** level;
      const spec = normalMapTextureSpec("normals", n);
      expect(spec).toMatchObject({
        size: { width: n + 2, height: n + 2, depthOrArrayLayers: 6 },
        dimension: "2d",
        format: NORMAL_MAP_FORMAT,
        mips: 1,
        category: "coarse-field-gpu",
      });
      // 27,744 B at level 5, 104,544 B at 6, 405,600 B at 7 and 1,597,536 B at 8.
      expect(textureBytes(spec)).toBe({ 5: 27_744, 6: 104_544, 7: 405_600, 8: 1_597_536 }[level]);
      const engine = await countingRenderEngine();
      const map = pack(level, [{ face: 3, i: 1, j: 2, pair: [0.5, -0.25] }]);
      const made = normalMapTexture(engine, "normals", map);
      expect(made.texels).toBe(map);
      expect(engine.textureSpecs).toEqual([spec]);
      const [written, ...more] = engine.textureWritten;
      expect([written?.texture, written?.origin, written?.size, more]).toEqual([
        "normals",
        { x: 0, y: 0, z: 0 },
        { width: n + 2, height: n + 2, depthOrArrayLayers: 6 },
        [],
      ]);
      expect(sameBytes(written?.data, map.data)).toBe(true);
    },
  );
});

describe("a normal map's reconstruction (R10.T10.g)", () => {
  it.each(LEVELS)(
    "gives a surveyed texel's own decoded normal at its centre where its neighbours are surveyed, at level %i",
    (level) => {
      const n = 2 ** level;
      const face: Face = 1;
      const cells = surveyedFace(face, n);
      const map = pack(level, cells);
      let worst = 0;
      for (const c of cells) {
        if (c.i === 0 || c.j === 0 || c.i === n - 1 || c.j === n - 1) {
          continue;
        }
        const found = normalMapNormalAt(map, faceDirection(face, (c.i + 0.5) / n, (c.j + 0.5) / n));
        if (found === null) {
          throw new Error(`no normal at (${c.i}, ${c.j})`);
        }
        worst = Math.max(worst, angle(found, storedNormal(c.pair)));
      }
      expect(worst).toBeLessThan(1e-12);
    },
  );

  it("mixes two centres' decoded vectors bilinearly between them, renormalised", () => {
    const level = 5;
    const n = 2 ** level;
    const face: Face = 4;
    const cells = surveyedFace(face, n);
    const map = pack(level, cells);
    const at = (i: number, j: number): Vec3 => {
      const cell = cells.find((c) => c.i === i && c.j === j);
      if (cell === undefined) {
        throw new Error(`no cell (${i}, ${j})`);
      }
      return storedNormal(cell.pair);
    };
    let worst = 0;
    for (const f of [0.1, 0.25, 0.5, 0.75, 0.9]) {
      // Along s between (10, 7) and (11, 7), and along t between (10, 7) and (10, 8).
      const alongS = normalMapNormalAt(map, faceDirection(face, (10.5 + f) / n, 7.5 / n));
      const alongT = normalMapNormalAt(map, faceDirection(face, 10.5 / n, (7.5 + f) / n));
      const mix = (a: Vec3, b: Vec3): Vec3 => normalise(add(scale(a, 1 - f), scale(b, f)));
      if (alongS === null || alongT === null) {
        throw new Error("no normal between surveyed centres");
      }
      worst = Math.max(
        worst,
        angle(alongS, mix(at(10, 7), at(11, 7))),
        angle(alongT, mix(at(10, 7), at(10, 8))),
      );
      // The mean of the pairs would differ: the filter is on the vectors.
      expect(Math.hypot(alongS.x, alongS.y, alongS.z)).toBeCloseTo(1, 14);
    }
    expect(worst).toBeLessThan(1e-12);
  });

  it("gives no normal in an unsurveyed texel, up to its edge", () => {
    const level = 6;
    const n = 2 ** level;
    const face: Face = 0;
    // A checkerboard of surveyed texels.
    const map = pack(
      level,
      surveyedFace(face, n, (i, j) => (i + j) % 2 === 0),
    );
    const misses: string[] = [];
    for (const [i, j] of [
      [1, 0],
      [0, 1],
      [5, 8],
      [33, 20],
      [63, 62],
    ] as const) {
      for (const [ds, dt] of [
        [0.5, 0.5],
        [1e-6, 1e-6],
        [1 - 1e-6, 0.5],
        [0.5, 1 - 1e-6],
      ] as const) {
        if (normalMapNormalAt(map, faceDirection(face, (i + ds) / n, (j + dt) / n)) !== null) {
          misses.push(`(${i} + ${ds}, ${j} + ${dt})`);
        }
      }
    }
    expect(misses).toEqual([]);
    // Every other face is wholly unsurveyed.
    expect(normalMapNormalAt(map, faceDirection(3, 0.5, 0.5))).toBeNull();
  });

  it("renormalises a surveyed texel beside unsurveyed ones over the surveyed alone", () => {
    const level = 5;
    const n = 2 ** level;
    const face: Face = 2;
    // (12, 9) and (12, 10) surveyed; (13, 9) and (13, 10) not.
    const cells = surveyedFace(face, n, (i, j) => i <= 12 && j >= 9 && j <= 10);
    const map = pack(level, cells);
    const own = (j: number): Vec3 => {
      const cell = cells.find((c) => c.i === 12 && c.j === j);
      if (cell === undefined) {
        throw new Error(`no cell (12, ${j})`);
      }
      return storedNormal(cell.pair);
    };
    // Toward the unsurveyed column on the centre's row: the texel's own normal alone.
    const onRow = normalMapNormalAt(map, faceDirection(face, 12.8 / n, 9.5 / n));
    // Toward both the unsurveyed column and the surveyed row above: the two surveyed texels by their
    // bilinear weights, (1 − fx)(1 − fy) and (1 − fx) fy, renormalised.
    const fy = 0.3;
    const between = normalMapNormalAt(map, faceDirection(face, 12.8 / n, (9.5 + fy) / n));
    if (onRow === null || between === null) {
      throw new Error("no normal in a surveyed texel");
    }
    const fx = 0.3;
    const expected = normalise(
      add(scale(own(9), (1 - fx) * (1 - fy)), scale(own(10), (1 - fx) * fy)),
    );
    expect([angle(onRow, own(9)) < 1e-12, angle(between, expected) < 1e-12]).toEqual([true, true]);
  });

  it("reads across a face's edge through the gutter, as smoothly as within a face", () => {
    const level = 5;
    const n = 2 ** level;
    // Faces 0 and 1 surveyed whole, each texel the radial normal at its centre.
    const radialCells: Cell[] = [];
    for (const face of [0, 1] as const) {
      for (let j = 0; j < n; j += 1) {
        for (let i = 0; i < n; i += 1) {
          radialCells.push({
            face,
            i,
            j,
            pair: octahedralPair(
              normalise(vec3(...faceDirection(face, (i + 0.5) / n, (j + 0.5) / n))),
            ),
          });
        }
      }
    }
    const map = pack(level, radialCells);
    // Points straddling the edge u = 1 of face 0 (x = y), each within a texel of it.
    let worst = 0;
    for (const s of [1 - 0.6 / n, 1 - 0.2 / n, 1 - 1e-9]) {
      const d = faceDirection(0, s, 0.5);
      const found = normalMapNormalAt(map, d);
      if (found === null) {
        throw new Error("no normal at the edge");
      }
      worst = Math.max(worst, angle(found, normalise(vec3(...d))));
    }
    // A texel's normals are its centre's radials: within a texel's angular size, 2.8° at N = 32.
    expect(worst).toBeLessThan((2.8 * Math.PI) / 180);
  });
});
