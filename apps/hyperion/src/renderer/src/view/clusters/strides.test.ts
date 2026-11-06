import { describe, expect, it } from "vitest";

import { seededRandom } from "../../test/seededRandom";
import { WGS84_FIGURE } from "../../test/terrainFixtures";
import type { PatchKey } from "../terrain/patchKey";
import { planetGeometry } from "../terrain/planet";
import { selectionErrorM } from "../terrain/select";
import terrainStridesWgsl from "../terrain/shaders/terrainStrides.wgsl?raw";
import { StrideEdges } from "../terrain/strideEdges";
import clusterCullWgsl from "./clusterCull.wgsl?raw";
import {
  cameraStrideLevel,
  clusterStrideBandsM,
  EDGE_BANDS,
  edgeBandHalf,
  edgeBandIndex,
  edgeBandMOfHalf,
  heldEdgeBandsM,
  INSTANCE_FINEST,
  INSTANCE_SUB_GRID_STRIDES,
  largestStrideErrorsM,
  maskStripBlock,
  NO_STRIDE_BAND,
  NO_STRIDE_EDGE_BANDS,
  NO_STRIDE_SCALE,
  patchStrideScale,
  sharedStrideBandsM,
  STRIDE_BAND_FRACTION,
  STRIDE_MASK_BAND_FRACTION,
  strideBandFractionOf,
  STRIDE_MORPH_LEVELS,
  type StrideBandsM,
  type StrideEdgeBandsM,
  type StrideErrorsM,
  strideFactor,
  strideFadingEdges,
  strideMorphFactor,
  strideMorphLevel,
  strideNeedM,
  STRIDE_NODES,
  strideParents,
  stridePlace,
  type StrideScale,
  vertexStrideBandM,
} from "./strides";
import { TERRAIN_SUB_PATCH_MASK } from "./terrain";
import { subPatchWord } from "../terrain/gpu/uniforms";

/**
 * A synthetic stretch of one cube face for the stride morph's twin: a heightfield on a lattice of
 * 1 m, drawn by a level-4 patch P (2 m quads) beside the four level-5 children of its neighbour
 * (1 m quads), so that it holds same-level edges (between the children) and a coarse–fine one
 * (P's x = 64 against the children's x = 0), each patch's 64 blocks drawn at the strides the
 * cull's twin gives them from their shared bands, their own errors and their neighbours' under its
 * scale and its edges' bands, which `StrideEdges` shares over the patches' keys and their records
 * hold in half floats: face 0's level-4 patch (0, 0) and the four children of (1, 0), whose lattice
 * they are.
 */
interface Patch {
  readonly key: PatchKey;
  readonly level: 4 | 5;
  /** Its corner on the lattice, m. */
  readonly x0: number;
  readonly y0: number;
  /** Its edges' coarse–fine holds: 1 where a finer patch touches (k = 0), 2 a coarser (k = 1). */
  readonly holds: readonly [xMin: number, xMax: number, yMin: number, yMax: number];
}

const QUADS = 64;

/** Its quad's side, m. */
function quadM(patch: Patch): number {
  return patch.level === 4 ? 2 : 1;
}

const PATCHES: ReadonlyArray<Patch> = [
  { key: { face: 0, level: 4, i: 0, j: 0 }, level: 4, x0: 0, y0: 0, holds: [0, 1, 0, 0] },
  { key: { face: 0, level: 5, i: 2, j: 0 }, level: 5, x0: 128, y0: 0, holds: [2, 0, 0, 0] },
  { key: { face: 0, level: 5, i: 3, j: 0 }, level: 5, x0: 192, y0: 0, holds: [0, 0, 0, 0] },
  { key: { face: 0, level: 5, i: 2, j: 1 }, level: 5, x0: 128, y0: 64, holds: [2, 0, 0, 0] },
  { key: { face: 0, level: 5, i: 3, j: 1 }, level: 5, x0: 192, y0: 64, holds: [0, 0, 0, 0] },
];

/**
 * Each patch's strides as its records hold them: its scale, its blocks' errors and its edges' shared
 * bands, m, rounded up into half floats of kilometres and read back as the kernel and the vertex
 * stage read them.
 */
interface PatchStrides {
  readonly scale: StrideScale;
  readonly clusterErrors: Float16Array;
  readonly edges: StrideEdgeBandsM;
}

/** Blocks' errors of none: every one +∞, unmeasured. */
const UNMEASURED_ERRORS = new Float16Array(4 * 64).fill(Number.POSITIVE_INFINITY);

/**
 * Strides a patch, in {@link PATCHES}' order, at random from `seed`: a scale of E_L ÷ τ′ 10 to 30 m
 * and 1 ÷ τ′ 40, each block's e₂ 0.25 to 2.25 m and each next 1.2 to 2.6 times it, so that each
 * block's bands differ from its neighbours'; none for a patch at the indices `unmeasured`, as a
 * stand-in or an unmeasured bake. Their edges' bands are `StrideEdges`' over the patches' keys.
 */
function stridesOf(
  seed: number,
  unmeasured: ReadonlyArray<number> = [],
): ReadonlyArray<PatchStrides> {
  const random = seededRandom(seed);
  const measured = PATCHES.map((_, n) => {
    const scale: StrideScale = { statedM: 10 + 20 * random(), perErrorM: 40 };
    const clusterErrors = new Float16Array(4 * 64);
    for (let block = 0; block < 64; block += 1) {
      const two = 0.25 + 2 * random();
      const four = two * (1.2 + 1.4 * random());
      clusterErrors.set([two / 2, two, four, four * (1.2 + 1.4 * random())], 4 * block);
    }
    return unmeasured.includes(n) ? null : { scale, clusterErrors };
  });
  const largest = new Map<number, StrideErrorsM>();
  measured.forEach((of, n) => {
    const level = PATCHES[n]?.level ?? 0;
    if (of !== null) {
      const own = largestStrideErrorsM(of.clusterErrors);
      const most = largest.get(level) ?? [0, 0, 0];
      largest.set(level, [
        Math.max(most[0], own[0]),
        Math.max(most[1], own[1]),
        Math.max(most[2], own[2]),
      ]);
    }
  });
  const edges = new StrideEdges(PATCHES.length);
  edges.update(
    PATCHES.map((patch, n) => ({
      key: patch.key,
      level: patch.level,
      measured: measured[n] ?? null,
    })),
    (level) => largest.get(level) ?? null,
  );
  return measured.map((of, n) => ({
    scale: of?.scale ?? NO_STRIDE_SCALE,
    clusterErrors: of?.clusterErrors ?? UNMEASURED_ERRORS,
    edges: heldEdgeBandsM(edges.edgesOf(n)),
  }));
}

/** `patch`'s quadrant word's coarse–fine holds: k = 0 in bits 2 to 5, k = 1 in bits 10 to 13. */
function quadrantWordOf(patch: Patch): number {
  return patch.holds.reduce(
    (word, hold, edge) =>
      word | (hold === 1 ? 1 << (2 + edge) : 0) | (hold === 2 ? 1 << (10 + edge) : 0),
    0,
  );
}

/** A height on the lattice, m: smooth relief and noise, the same for every patch. */
function heightOf(seed: number): (x: number, y: number) => number {
  const random = seededRandom(seed);
  const noise = new Map<string, number>();
  return (x, y) => {
    const key = `${String(x)},${String(y)}`;
    let n = noise.get(key);
    if (n === undefined) {
      n = random() - 0.5;
      noise.set(key, n);
    }
    return 6 * Math.sin(x / 37) * Math.cos(y / 23) + 1.5 * n;
  };
}

type Point = readonly [number, number, number];

/** Whether `n` is odd. */
function odd(n: number): boolean {
  return n % 2 === 1;
}

/** Patch `patch`'s vertex (x, y) as its draw places it before the stride morph. */
/** Each level's CDLOD morph band, m: its morph to its parent's mesh from the first to the second. */
const CDLOD_BANDS: Readonly<Record<Patch["level"], readonly [number, number]>> = {
  4: [150, 230],
  5: [60, 110],
};

/** The mean of two places. */
function midpointOf(a: Point, b: Point): Point {
  return [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
}

/**
 * Patch `patch`'s vertex (x, y) as its draw places it before the stride morph, from `camera`: its
 * own place mixed towards its parent's mesh by CDLOD's k from its distance (R05 Design note 6), the
 * parent's mesh at an odd vertex the mean of its even neighbours on the (0, 0)–(1, 1) diagonal;
 * held at k = 1 on an edge a coarser patch touches and k = 0 on one a finer patch touches
 * (R10.T8.c), as the CDLOD morph's holds are.
 */
function placeOf(patch: Patch, height: (x: number, y: number) => number, camera: Point) {
  const q = quadM(patch);
  const at = (x: number, y: number): Point => {
    const gx = patch.x0 + x * q;
    const gy = patch.y0 + y * q;
    return [gx, gy, height(gx, gy)];
  };
  const parentOf = (x: number, y: number): Point => {
    if (odd(x) && odd(y)) {
      return midpointOf(at(x - 1, y - 1), at(x + 1, y + 1));
    }
    if (odd(x)) {
      return midpointOf(at(x - 1, y), at(x + 1, y));
    }
    return odd(y) ? midpointOf(at(x, y - 1), at(x, y + 1)) : at(x, y);
  };
  const [startM, endM] = CDLOD_BANDS[patch.level];
  return (x: number, y: number): Point => {
    const edges = [x === 0, x === QUADS, y === 0, y === QUADS];
    const held = (hold: number): boolean =>
      edges.some((on, edge) => on && patch.holds[edge] === hold);
    const own = at(x, y);
    const parent = parentOf(x, y);
    const k = held(2)
      ? 1
      : held(1)
        ? 0
        : Math.min(1, Math.max(0, (distanceOf(own, camera) - startM) / (endM - startM)));
    return [
      own[0] + k * (parent[0] - own[0]),
      own[1] + k * (parent[1] - own[1]),
      own[2] + k * (parent[2] - own[2]),
    ];
  };
}

function distanceOf(p: ReadonlyArray<number>, camera: Point): number {
  return Math.hypot((p[0] ?? 0) - camera[0], (p[1] ?? 0) - camera[1], (p[2] ?? 0) - camera[2]);
}

/** The morph's k of `patch`'s vertex, as `stride_factor` takes it, its edges' holds with it. */
function factorOf(patch: Patch, camera: Point, of: PatchStrides) {
  const word = quadrantWordOf(patch);
  return (x: number, y: number, level: number, values: ReadonlyArray<number>): number =>
    strideFactor(
      x,
      y,
      level,
      word,
      0,
      vertexStrideBandM(of.scale, of.clusterErrors, of.edges, x, y, level),
      distanceOf(values, camera),
    );
}

/** One drawn patch: each vertex's place before the stride morph and after it, each block's stride. */
interface Drawn {
  readonly patch: Patch;
  readonly unmoved: ReadonlyArray<ReadonlyArray<number>>;
  readonly places: ReadonlyArray<ReadonlyArray<number>>;
  readonly strides: ReadonlyArray<number>;
}

function drawPatch(
  patch: Patch,
  height: (x: number, y: number) => number,
  camera: Point,
  of: PatchStrides,
): Drawn {
  const place = placeOf(patch, height, camera);
  const factor = factorOf(patch, camera, of);
  const unmoved: Point[] = [];
  const places: number[][] = [];
  for (let y = 0; y <= QUADS; y += 1) {
    for (let x = 0; x <= QUADS; x += 1) {
      unmoved.push(place(x, y));
      places.push(stridePlace(x, y, place, factor));
    }
  }
  const strides: number[] = [];
  for (let by = 0; by < 8; by += 1) {
    for (let bx = 0; bx < 8; bx += 1) {
      // The block's sphere about its own vertices, which hold every morphed one.
      const own: Point[] = [];
      for (let ly = 0; ly <= 8; ly += 1) {
        for (let lx = 0; lx <= 8; lx += 1) {
          own.push(place(8 * bx + lx, 8 * by + ly));
        }
      }
      const centre = [0, 1, 2].map((n) => own.reduce((s, p) => s + (p[n] ?? 0), 0) / own.length);
      const radius = Math.max(
        ...own.map((p) => distanceOf(p, [centre[0] ?? 0, centre[1] ?? 0, centre[2] ?? 0])),
      );
      const nearest = Math.max(0, distanceOf(centre, camera) - radius);
      const edges =
        (bx === 0 ? 1 : 0) | (bx === 7 ? 2 : 0) | (by === 0 ? 4 : 0) | (by === 7 ? 8 : 0);
      const touchesFiner = patch.holds.some(
        (hold, edge) => hold === 1 && (edges & (1 << edge)) !== 0,
      );
      const bands = clusterStrideBandsM(of.scale, of.clusterErrors, of.edges, 8 * by + bx);
      strides.push(1 << cameraStrideLevel(bands, touchesFiner ? 2 : 3, nearest));
    }
  }
  return { patch, unmoved, places, strides };
}

/**
 * `drawn`'s surface at the place (u, v) of its grid, quads: its block's stride-s triangle there, on
 * the (0, 0)–(1, 1) diagonal, mixed from its corners' morphed places.
 */
function surfaceAt(drawn: Drawn, u: number, v: number): number[] {
  const bx = Math.min(7, Math.floor(u / 8));
  const by = Math.min(7, Math.floor(v / 8));
  const s = drawn.strides[8 * by + bx] ?? 1;
  const qx = Math.min(QUADS - s, Math.floor(u / s) * s);
  const qy = Math.min(QUADS - s, Math.floor(v / s) * s);
  const fx = (u - qx) / s;
  const fy = (v - qy) / s;
  const at = (x: number, y: number): ReadonlyArray<number> =>
    drawn.places[y * (QUADS + 1) + x] ?? [];
  const p0 = at(qx, qy);
  const p2 = at(qx + s, qy + s);
  const p1 = fx >= fy ? at(qx + s, qy) : at(qx, qy + s);
  const w0 = 1 - Math.max(fx, fy);
  const w1 = Math.abs(fx - fy);
  const w2 = Math.min(fx, fy);
  return [0, 1, 2].map((n) => w0 * (p0[n] ?? 0) + w1 * (p1[n] ?? 0) + w2 * (p2[n] ?? 0));
}

/** `drawn`'s surface at lattice place (gx, gy), m. */
function surfaceAtLattice(drawn: Drawn, gx: number, gy: number): number[] {
  const q = quadM(drawn.patch);
  return surfaceAt(drawn, (gx - drawn.patch.x0) / q, (gy - drawn.patch.y0) / q);
}

/** `drawn`'s surface at lattice place (gx, gy) at stride 1, before the stride morph, m. */
function unmovedAtLattice(drawn: Drawn, gx: number, gy: number): number[] {
  return surfaceAtLattice(
    { ...drawn, places: drawn.unmoved, strides: drawn.strides.map(() => 1) },
    gx,
    gy,
  );
}

function gap(a: ReadonlyArray<number>, b: ReadonlyArray<number>): number {
  return Math.hypot(
    (a[0] ?? 0) - (b[0] ?? 0),
    (a[1] ?? 0) - (b[1] ?? 0),
    (a[2] ?? 0) - (b[2] ?? 0),
  );
}

describe("the stride morph's vertices", () => {
  it("takes each grid vertex's morph level from its place's trailing zeros, at most 3", () => {
    expect(strideMorphLevel(1, 0)).toBe(0);
    expect(strideMorphLevel(2, 6)).toBe(1);
    expect(strideMorphLevel(4, 12)).toBe(2);
    expect(strideMorphLevel(8, 16)).toBe(3);
    expect(strideMorphLevel(0, 0)).toBe(3);
    expect(strideMorphLevel(64, 3)).toBe(0);
  });

  it("moves a vertex between its neighbours along its quad's side or its diagonal", () => {
    expect(strideParents(3, 4, 0)).toEqual([2, 4, 4, 4]);
    expect(strideParents(4, 6, 1)).toEqual([4, 4, 4, 8]);
    expect(strideParents(12, 4, 2)).toEqual([8, 0, 16, 8]);
  });

  it("places every vertex of the grid within its node budget", () => {
    let most = 0;
    for (let y = 0; y <= QUADS; y += 1) {
      for (let x = 0; x <= QUADS; x += 1) {
        let visited = 0;
        stridePlace(
          x,
          y,
          () => {
            visited += 1;
            return [0];
          },
          () => 0.5,
        );
        most = Math.max(most, visited);
      }
    }
    expect(most).toBeLessThanOrEqual(STRIDE_NODES);
    expect(most).toBe(STRIDE_NODES);
  });

  it("holds a level-2 vertex on an edge a finer patch touches at its band's k times its onset's share, 0 once it is whole", () => {
    // Edge x = 64 held at k = 0 (quadrant word bit 3), its share 128 of 255.
    const word = 0b0010 << 2;
    const far = (1 + STRIDE_BAND_FRACTION) * 100;
    expect([
      strideFactor(64, 4, 2, word, 128 << 8, 100, far),
      strideFactor(64, 4, 2, word, 0, 100, far),
      strideFactor(4, 4, 2, word, 128 << 8, 100, far),
    ]).toEqual([128 / 255, 0, 1]);
  });

  it("gives a level-2 vertex on an edge whose hold lets go its band's k times 1 less its release's byte, wholly once it is 0", () => {
    // Edge x = 64 not held, its release's byte 64 of 255; edge y = 64's byte alone elsewhere.
    const far = (1 + STRIDE_BAND_FRACTION) * 100;
    expect([
      strideFactor(64, 4, 2, 0, 64 << 8, 100, far),
      strideFactor(64, 4, 2, 0, 0, 100, far),
      strideFactor(4, 4, 2, 0, 64 << 8, 100, far),
      strideFactor(64, 2, 1, 0, 64 << 8, 100, far),
    ]).toEqual([1 - 64 / 255, 1, 1, 1]);
  });

  it("finds the edges whose hold fades in or out by their bytes", () => {
    expect([strideFadingEdges(0), strideFadingEdges((1 << 8) | (255 << 24))]).toEqual([0, 0b1010]);
  });

  it("moves a level-0 vertex on an edge a coarser patch touches wholly, whatever its band", () => {
    expect(strideFactor(64, 3, 0, 0b0010 << 10, 0, NO_STRIDE_BAND, 0)).toBe(1);
  });

  it("morphs across its band and stays outside it", () => {
    expect(strideMorphFactor(100, 99)).toBe(0);
    expect(strideMorphFactor(100, 100 + 0.5 * STRIDE_BAND_FRACTION * 100)).toBeCloseTo(0.5, 12);
    expect(strideMorphFactor(100, (1 + STRIDE_BAND_FRACTION) * 100)).toBe(1);
    expect(strideMorphFactor(NO_STRIDE_BAND, 1e9)).toBe(0);
  });
});

/** The distance at which the cull takes a band's stride, in `f32` as the kernel forms it. */
function threshold(band: number): number {
  return Math.fround(Math.fround(1 + STRIDE_BAND_FRACTION) * band);
}

/** A bake's errors, every cluster's e₂, e₄ and e₈ `e`, twice and four times it. */
function bakeErrors(e: number): Float16Array {
  const out = new Float16Array(4 * 96);
  for (let c = 0; c < 96; c += 1) {
    out.set([0, e, 2 * e, 4 * e], 4 * c);
  }
  return out;
}

describe("the cull's camera strides", () => {
  const bands: StrideBandsM = [100, 300, 900];

  it("takes the coarsest stride whose band it is past, by its nearest point", () => {
    expect(cameraStrideLevel(bands, 3, 50)).toBe(0);
    expect(cameraStrideLevel(bands, 3, threshold(100))).toBe(1);
    expect(cameraStrideLevel(bands, 3, threshold(100) - 1e-3)).toBe(0);
    expect(cameraStrideLevel(bands, 3, threshold(300))).toBe(2);
    expect(cameraStrideLevel(bands, 3, 1e6)).toBe(3);
  });

  it("keeps stride 1 with no bands, and stops at the first band it has not", () => {
    expect([
      cameraStrideLevel([NO_STRIDE_BAND, NO_STRIDE_BAND, NO_STRIDE_BAND], 3, 1e9),
      cameraStrideLevel([100, NO_STRIDE_BAND, 900], 3, 1e9),
    ]).toEqual([0, 1]);
  });

  it("takes no stride past its cap", () => {
    expect([cameraStrideLevel(bands, 0, 1e9), cameraStrideLevel(bands, 2, 1e9)]).toEqual([0, 2]);
  });

  it("gives a patch its scale from its level's stated error at its view's tolerance, none for a tolerance not above 0", () => {
    const planet = planetGeometry(WGS84_FIGURE, null);
    const statedM = selectionErrorM(planet, 9);
    expect(patchStrideScale(planet, 9, 1e-3)).toEqual({
      statedM: statedM / 1e-3,
      perErrorM: 1 / 1e-3,
    });
    expect(patchStrideScale(planet, 9, 0)).toEqual(NO_STRIDE_SCALE);
  });

  it("gives a cluster of error e the band (E_L + e) ÷ τ′ in f32, none for no scale or an error not finite", () => {
    const scale: StrideScale = { statedM: 10, perErrorM: 2 };
    expect([
      strideNeedM(scale, 0.1),
      strideNeedM(scale, Number.POSITIVE_INFINITY),
      strideNeedM(scale, Number.NaN),
      strideNeedM(scale, -1),
      strideNeedM(NO_STRIDE_SCALE, 1),
    ]).toEqual([
      Math.fround(10 + Math.fround(Math.fround(0.1) * 2)),
      NO_STRIDE_BAND,
      NO_STRIDE_BAND,
      NO_STRIDE_BAND,
      NO_STRIDE_BAND,
    ]);
  });

  it("shares two bands as the larger, none where either is none", () => {
    expect(sharedStrideBandsM([10, 30, NO_STRIDE_BAND], [20, 25, 40])).toEqual([
      20,
      30,
      NO_STRIDE_BAND,
    ]);
  });

  /**
   * A patch's blocks for the band rules: needs 10 + 2e, block 0's 12, 14, 16, block 1's 20, 12 and
   * none, block 8's 10, 30, 12, the other blocks unmeasured; edge x = 0's first block 11, 50, 13 and
   * edge y = 0's 15, 13, 40, the other edges none.
   */
  const SCALE: StrideScale = { statedM: 10, perErrorM: 2 };
  const ERRORS = new Float16Array(4 * 64).fill(Number.POSITIVE_INFINITY);
  ERRORS.set([0, 1, 2, 3], 0);
  ERRORS.set([0, 5, 1, Number.NaN], 4);
  ERRORS.set([0, 0, 10, 1], 32);
  const EDGES = new Float64Array(EDGE_BANDS).fill(NO_STRIDE_BAND);
  EDGES.set([11, 50, 13], edgeBandIndex(0, 0, 0));
  EDGES.set([15, 13, 40], edgeBandIndex(2, 0, 0));
  const NONE3 = [NO_STRIDE_BAND, NO_STRIDE_BAND, NO_STRIDE_BAND];

  it("gives a block the largest need of it, its side neighbours and its edges, none where one is", () => {
    expect([
      clusterStrideBandsM(SCALE, ERRORS, EDGES, 0),
      clusterStrideBandsM(NO_STRIDE_SCALE, ERRORS, EDGES, 0),
    ]).toEqual([[20, 50, NO_STRIDE_BAND], NONE3]);
  });

  it("gives a strip its edge's bands along its block, and a fin or head run none", () => {
    expect([
      clusterStrideBandsM(SCALE, ERRORS, EDGES, 64),
      clusterStrideBandsM(SCALE, ERRORS, EDGES, 64 + 8 * 3 + 7),
      clusterStrideBandsM(SCALE, ERRORS, EDGES, 96),
    ]).toEqual([[15, 13, 40], [11, 50, 13], NONE3]);
  });

  it("gives a vertex inside a block its block's band of its parts not none, none where its own need is", () => {
    // Block 1's band at stride 2 is none (blocks 2 and 9 unmeasured), its vertices' 20; block 2 is
    // unmeasured.
    expect([
      vertexStrideBandM(SCALE, ERRORS, EDGES, 3, 5, 0),
      vertexStrideBandM(SCALE, ERRORS, EDGES, 9, 3, 0),
      vertexStrideBandM(SCALE, ERRORS, EDGES, 17, 3, 0),
    ]).toEqual([20, 20, NO_STRIDE_BAND]);
  });

  it("gives a vertex on a block's side the smaller of its two blocks' bands", () => {
    // Block 8's band at stride 4 for its vertices is its own 30, block 0's 50.
    expect([
      vertexStrideBandM(SCALE, ERRORS, EDGES, 8, 3, 0),
      vertexStrideBandM(SCALE, ERRORS, EDGES, 2, 8, 1),
    ]).toEqual([20, 30]);
  });

  it("gives a vertex on a patch's edge that edge's shared band there", () => {
    expect([
      vertexStrideBandM(SCALE, ERRORS, EDGES, 0, 3, 0),
      vertexStrideBandM(SCALE, ERRORS, EDGES, 3, 0, 2),
      vertexStrideBandM(SCALE, ERRORS, EDGES, 64, 3, 0),
    ]).toEqual([11, 40, NO_STRIDE_BAND]);
  });

  /**
   * A mask's blocks (R14.T3.c's stand-ins): every block's needs 12, 14 and 16 at {@link SCALE} but
   * block 3's 50, block 5's 14 and block 12's 16, of a mask one level down at part (1, 0), its set
   * blocks 4 to 7 of rows 0 to 3.
   */
  const MASK_ERRORS = new Float16Array(4 * 64);
  for (let block = 0; block < 64; block += 1) {
    MASK_ERRORS.set([0, 1, 2, 3], 4 * block);
  }
  MASK_ERRORS.set([0, 20, 20, 20], 4 * 3);
  MASK_ERRORS.set([0, 2, 2, 2], 4 * 5);
  MASK_ERRORS.set([0, 3, 3, 3], 4 * 12);
  const MASK = (TERRAIN_SUB_PATCH_MASK | subPatchWord(1, 1, 0)) >>> 0;

  it("gives a mask's block the largest need of it and of its set's blocks on its sides alone, its set's sides in its edges' place", () => {
    expect([
      clusterStrideBandsM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 4, MASK),
      clusterStrideBandsM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 4),
      clusterStrideBandsM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 3, MASK),
      clusterStrideBandsM(NO_STRIDE_SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 4, MASK),
    ]).toEqual([[16, 16, 16], NONE3, NONE3, NONE3]);
  });

  it("gives a mask's strip m of its set's side e its block's bands there, and none past its set's side", () => {
    expect([
      clusterStrideBandsM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 64, MASK),
      clusterStrideBandsM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 64 + 8 + 1, MASK),
      clusterStrideBandsM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 64 + 16 + 3, MASK),
      clusterStrideBandsM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 64 + 24 + 3, MASK),
      clusterStrideBandsM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 64 + 4, MASK),
    ]).toEqual([
      // Block 4 at side 0's start; block 15 (7, 1) at side 1's second; blocks 28 (4, 3) and 4 (4, 0)
      // at the last of sides 2 and 3, anticlockwise.
      [16, 16, 16],
      [12, 14, 16],
      [12, 14, 16],
      [16, 16, 16],
      NONE3,
    ]);
  });

  it("gives a vertex on a mask's set's side its one block's band there, and one inside the set a patch's of its set's blocks", () => {
    expect([
      vertexStrideBandM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 32, 3, 0, MASK),
      vertexStrideBandM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 64, 3, 0, MASK),
      vertexStrideBandM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 35, 0, 0, MASK),
      vertexStrideBandM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 33, 3, 0, MASK),
      vertexStrideBandM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 40, 3, 0, MASK),
      vertexStrideBandM(SCALE, MASK_ERRORS, NO_STRIDE_EDGE_BANDS, 32, 3, 0),
    ]).toEqual([16, 12, 16, 16, 14, 50]);
  });

  it("bands every cluster of a random mask at least as far as every vertex it drops is banded, a strip as the vertices of its block's side, and none where one is none (R14.T3.c's stand-ins)", () => {
    const random = seededRandom(0x3a5c);
    let checked = 0;
    const short: string[] = [];
    for (let n = 0; n < 60; n += 1) {
      const levels = 1 + (n % 3);
      const parts = 2 ** levels;
      const subWord =
        (TERRAIN_SUB_PATCH_MASK |
          subPatchWord(levels, Math.floor(random() * parts), Math.floor(random() * parts))) >>>
        0;
      const errors = new Float16Array(4 * 64);
      for (let block = 0; block < 64; block += 1) {
        const two = random() < 0.1 ? Number.POSITIVE_INFINITY : 0.25 + 2 * random();
        const four = two * (1.2 + random());
        errors.set([two / 2, two, four, four * (1.2 + random())], 4 * block);
      }
      for (let cluster = 0; cluster < 96; cluster += 1) {
        const block = cluster < 64 ? cluster : maskStripBlock(subWord, cluster);
        const clusterBands = clusterStrideBandsM(
          SCALE,
          errors,
          NO_STRIDE_EDGE_BANDS,
          cluster,
          subWord,
        );
        if (block < 0 || clusterBands.every((band) => !(band > 0))) {
          continue;
        }
        const x0 = 8 * (block % 8);
        const y0 = 8 * Math.floor(block / 8);
        // A block's 9 × 9 vertices; a strip's along its block's side, as it hangs from it.
        const side = Math.floor((cluster - 64) / 8);
        const vertices: Array<readonly [number, number]> = [];
        for (let k = 0; k <= 8; k += 1) {
          for (let j = 0; j <= 8; j += 1) {
            const onSide = [k === 0, j === 8, k === 8, j === 0][side] ?? true;
            if (cluster < 64 || onSide) {
              vertices.push([x0 + j, y0 + k]);
            }
          }
        }
        for (const [x, y] of vertices) {
          const level = strideMorphLevel(x, y);
          const band = clusterBands[level] ?? NO_STRIDE_BAND;
          if (level >= 3 || !(band > 0)) {
            continue;
          }
          checked += 1;
          const own = vertexStrideBandM(SCALE, errors, NO_STRIDE_EDGE_BANDS, x, y, level, subWord);
          if (!(own > 0 && own <= band)) {
            short.push(`${String(subWord)} ${String(cluster)} (${String(x)}, ${String(y)})`);
          }
        }
      }
    }
    expect([checked > 10_000, short]).toEqual([true, []]);
  });

  it("takes a mask's strides and its vertices' morph over its own wider band, a whole patch's over β (R14.T3.c's stand-ins)", () => {
    const levels: StrideBandsM = [100, 200, 400];
    const beta = strideBandFractionOf(MASK);
    expect([
      cameraStrideLevel(levels, 3, 1_000),
      cameraStrideLevel(levels, 3, 1_000, beta),
      cameraStrideLevel(levels, 3, 450, beta),
      // Short of the stride-8 band by a metre at the mask's β, past it by far at a whole patch's.
      cameraStrideLevel(levels, 3, 400 * (1 + beta) - 1, beta),
      cameraStrideLevel(levels, 3, 400 * (1 + beta) - 1),
      strideBandFractionOf(subPatchWord(1, 1, 0)),
      strideMorphFactor(100, 110),
      // Half across the mask's band, which a whole patch's has long passed.
      strideMorphFactor(100, 100 + 50 * beta, beta),
    ]).toEqual([3, 3, 2, 2, 3, STRIDE_BAND_FRACTION, 1, 0.5]);
  });

  it("holds an edge's band in a half float of kilometres rounded up, none and +∞ as they are", () => {
    const random = seededRandom(0xed6e);
    for (let n = 0; n < 2_000; n += 1) {
      const bandM = 10 ** (1 + 5 * random());
      const held = edgeBandMOfHalf(edgeBandHalf(bandM));
      expect(held).toBeGreaterThanOrEqual(bandM);
      expect(held).toBeLessThan(bandM * (1 + 2 ** -9));
    }
    expect([
      edgeBandMOfHalf(edgeBandHalf(NO_STRIDE_BAND)),
      edgeBandMOfHalf(edgeBandHalf(0)),
      edgeBandMOfHalf(edgeBandHalf(Number.POSITIVE_INFINITY)),
      edgeBandMOfHalf(edgeBandHalf(1e12)),
    ]).toEqual([NO_STRIDE_BAND, NO_STRIDE_BAND, Infinity, Infinity]);
  });

  it("takes no stride morph at a band of +∞", () => {
    expect(strideMorphFactor(Number.POSITIVE_INFINITY, 1e30)).toBe(0);
  });
});

describe("the stride morph's surface", () => {
  const cameras: Point[] = [];
  const random = seededRandom(0x5a7e);
  for (let n = 0; n < 24; n += 1) {
    cameras.push([-100 + 450 * random(), -80 + 300 * random(), 4 + 160 * random() ** 2]);
  }

  /**
   * Each camera's strides: every patch measured, or one not, as a stand-in or an unmeasured bake: a
   * finer patch beside the coarse one, the coarse one, or a finer one beside only its siblings.
   */
  const MEASURED = cameras.map((_, n) => stridesOf(0x3e + n));
  const UNMEASURED: ReadonlyArray<readonly [string, number]> = [
    ["a finer patch beside the coarse one unmeasured", 3],
    ["the coarse patch unmeasured", 0],
    ["a finer patch beside its siblings unmeasured", 4],
  ];
  const ALL = [
    ["every patch measured", MEASURED, -1] as const,
    ...UNMEASURED.map(
      ([name, k]) => [name, cameras.map((_, n) => stridesOf(0x3e + n, [k])), k] as const,
    ),
  ];

  it.each(ALL)(
    "draws each block at the strides it takes exactly as the morph places its every vertex, %s",
    (_name, bands, _unmeasured) => {
      let strided = 0;
      let worst = 0;
      cameras.forEach((camera, n) => {
        const height = heightOf(n);
        for (const [k, patch] of PATCHES.entries()) {
          const of = bands[n]?.[k];
          if (of === undefined) {
            throw new Error("the fixture's bands");
          }
          const drawn = drawPatch(patch, height, camera, of);
          strided += drawn.strides.filter((s) => s > 1).length;
          for (let y = 0; y <= QUADS; y += 1) {
            for (let x = 0; x <= QUADS; x += 1) {
              worst = Math.max(
                worst,
                gap(surfaceAt(drawn, x, y), drawn.places[y * (QUADS + 1) + x] ?? []),
              );
            }
          }
        }
      });
      expect(strided).toBeGreaterThan(1000);
      expect(worst).toBeLessThan(1e-9);
    },
  );

  it.each(ALL)(
    "joins every patch to its neighbours along their edges, coarse–fine edges included, a measured one beside one that is not parting by no more than its own stride morph moves it, %s",
    (_name, bands, unmeasured) => {
      let worst = 0;
      let parted = 0;
      let unmovedWorst = 0;
      let differing = 0;
      cameras.forEach((camera, n) => {
        const height = heightOf(n);
        const drawn = PATCHES.map((patch, k) => {
          const of = bands[n]?.[k];
          if (of === undefined) {
            throw new Error("the fixture's bands");
          }
          return drawPatch(patch, height, camera, of);
        });
        const [p, q00, q10, q01, q11] = drawn;
        if (
          p === undefined ||
          q00 === undefined ||
          q10 === undefined ||
          q01 === undefined ||
          q11 === undefined
        ) {
          throw new Error("the fixture's five patches");
        }
        const edges: Array<
          readonly [Drawn, Drawn, (t: number) => readonly [number, number], readonly number[]]
        > = [
          [p, q00, (t) => [128, t], [0, 1]],
          [p, q01, (t) => [128, 64 + t], [0, 3]],
          [q00, q10, (t) => [192, t], [1, 2]],
          [q01, q11, (t) => [192, 64 + t], [3, 4]],
          [q00, q01, (t) => [128 + t, 64], [1, 3]],
          [q10, q11, (t) => [192 + t, 64], [2, 4]],
        ];
        for (const [a, b, along, pair] of edges) {
          const beside = pair.includes(unmeasured);
          let moved = 0;
          let apart = 0;
          for (let t = 0; t <= 64; t += 0.25) {
            const [gx, gy] = along(t);
            const [sa, sb] = [surfaceAtLattice(a, gx, gy), surfaceAtLattice(b, gx, gy)];
            const [ua, ub] = [unmovedAtLattice(a, gx, gy), unmovedAtLattice(b, gx, gy)];
            apart = Math.max(apart, gap(sa, sb));
            moved = Math.max(moved, gap(sa, ua), gap(sb, ub));
            if (beside) {
              const [s, u] = pair[0] === unmeasured ? [sa, ua] : [sb, ub];
              unmovedWorst = Math.max(unmovedWorst, gap(s, u));
            }
          }
          if (beside) {
            parted = Math.max(parted, apart - moved);
          } else {
            worst = Math.max(worst, apart);
          }
          differing += a.strides.some((s, i) => s !== b.strides[i]) ? 1 : 0;
        }
      });
      expect(differing).toBeGreaterThan(20);
      expect(worst).toBeLessThan(1e-9);
      expect(parted).toBeLessThan(1e-9);
      expect(unmovedWorst).toBeLessThan(1e-9);
    },
  );

  it("moves nothing when a cluster's stride flips at its threshold", () => {
    // A block drawn at each stride the cull could give it at the camera's place: every one whose
    // dropped vertices the morph has fully moved is the same surface.
    const patch = PATCHES[0];
    if (patch === undefined) {
      throw new Error("the fixture's coarse patch");
    }
    const height = heightOf(7);
    let compared = 0;
    let worst = 0;
    for (const [n, camera] of cameras.entries()) {
      const of = MEASURED[n]?.[0];
      if (of === undefined) {
        throw new Error("the fixture's bands");
      }
      const drawn = drawPatch(patch, height, camera, of);
      for (let b = 0; b < 64; b += 1) {
        const chosen = drawn.strides[b] ?? 1;
        for (let s = 1; s < chosen; s *= 2) {
          const finer = { ...drawn, strides: drawn.strides.map((t, i) => (i === b ? s : t)) };
          const bx = 8 * (b % 8);
          const by = 8 * Math.floor(b / 8);
          for (let v = 0; v <= 8; v += 0.5) {
            for (let u = 0; u <= 8; u += 0.5) {
              worst = Math.max(
                worst,
                gap(surfaceAt(drawn, bx + u, by + v), surfaceAt(finer, bx + u, by + v)),
              );
            }
          }
          compared += 1;
        }
      }
    }
    expect(compared).toBeGreaterThan(200);
    expect(worst).toBeLessThan(1e-9);
  });

  it("holds every vertex unmoved where its k is 0, as within a contact's held radius", () => {
    const patch = PATCHES[1];
    if (patch === undefined) {
      throw new Error("the fixture's fine patch");
    }
    const place = placeOf(patch, heightOf(3), [0, 0, 1_000]);
    for (let y = 0; y <= QUADS; y += 3) {
      for (let x = 0; x <= QUADS; x += 5) {
        expect(stridePlace(x, y, place, () => 0)).toEqual([...place(x, y)]);
      }
    }
  });
});

describe("a patch's measured errors", () => {
  it("are its clusters' largest e₂, e₄ and e₈, a NaN's infinite", () => {
    const errors = bakeErrors(1);
    errors.set([0, 0.25, 7, 0.5], 4 * 10);
    expect(largestStrideErrorsM(errors)).toEqual([1, 7, 4]);
    errors[4 * 20 + 3] = Number.NaN;
    expect(largestStrideErrorsM(errors)[2]).toBe(Infinity);
  });

  it("morphs at most three levels, towards strides 2, 4 and 8", () => {
    expect(STRIDE_MORPH_LEVELS).toBe(3);
  });
});

describe("the strides' WGSL constants", () => {
  it("are the twin's: the band, the morph's levels and nodes, the forced region's bit, the own grids' and a mask's", () => {
    for (const source of [terrainStridesWgsl, clusterCullWgsl]) {
      expect(source).toContain(
        `const STRIDE_BAND_FRACTION : f32 = ${String(STRIDE_BAND_FRACTION)};`,
      );
    }
    expect(terrainStridesWgsl).toContain(
      `const STRIDE_MORPH_LEVELS : u32 = ${String(STRIDE_MORPH_LEVELS)}u;`,
    );
    expect(terrainStridesWgsl).toContain(`const STRIDE_NODES : u32 = ${String(STRIDE_NODES)}u;`);
    expect(clusterCullWgsl).toContain(
      `const INSTANCE_FINEST : u32 = 0x${INSTANCE_FINEST.toString(16)}u;`,
    );
    expect(clusterCullWgsl).toContain(
      `const INSTANCE_SUB_GRID_STRIDES : u32 = 0x${INSTANCE_SUB_GRID_STRIDES.toString(16)}u;`,
    );
    expect(terrainStridesWgsl).toContain(
      `const STRIDE_SUB_PATCH_MASK : u32 = 0x${TERRAIN_SUB_PATCH_MASK.toString(16)}u;`,
    );
    for (const source of [terrainStridesWgsl, clusterCullWgsl]) {
      expect(source).toContain(
        `const STRIDE_MASK_BAND_FRACTION : f32 = ${String(STRIDE_MASK_BAND_FRACTION)};`,
      );
    }
  });
});
