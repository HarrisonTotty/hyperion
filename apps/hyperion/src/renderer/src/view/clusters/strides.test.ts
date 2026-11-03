import { describe, expect, it } from "vitest";

import { seededRandom } from "../../test/seededRandom";
import terrainStridesWgsl from "../terrain/shaders/terrainStrides.wgsl?raw";
import clusterCullWgsl from "./clusterCull.wgsl?raw";
import {
  cameraStrideLevel,
  INSTANCE_FINEST,
  NO_STRIDE_BAND,
  STRIDE_BAND_FRACTION,
  STRIDE_MORPH_LEVELS,
  type StrideBandsM,
  strideBandsOf,
  STRIDE_ERROR_DECAY,
  strideHeldFactor,
  StrideLevelErrors,
  strideMorphFactor,
  strideMorphLevel,
  STRIDE_NODES,
  strideParents,
  stridePlace,
} from "./strides";

/**
 * A synthetic stretch of one cube face for the stride morph's twin: a heightfield on a lattice of
 * 1 m, drawn by a level-4 patch P (2 m quads) beside the four level-5 children of its neighbour
 * (1 m quads), so that it holds same-level edges (between the children) and a coarse–fine one
 * (P's x = 64 against the children's x = 0), each patch's 64 blocks drawn at the strides the
 * cull's twin gives them from a hand-made ladder.
 */
interface Patch {
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
  { level: 4, x0: 0, y0: 0, holds: [0, 1, 0, 0] },
  { level: 5, x0: 128, y0: 0, holds: [2, 0, 0, 0] },
  { level: 5, x0: 192, y0: 0, holds: [0, 0, 0, 0] },
  { level: 5, x0: 128, y0: 64, holds: [2, 0, 0, 0] },
  { level: 5, x0: 192, y0: 64, holds: [0, 0, 0, 0] },
];

/** U(n) by absolute level, m: level 4 takes U(3), U(2), U(1), level 5 U(4), U(3), U(2). */
const LADDER = Float64Array.from([NO_STRIDE_BAND, 200, 120, 60, 20, NO_STRIDE_BAND]);

/** A ladder whose bands overlap, as one of levels whose stated errors rule them: 55 to 70 m. */
const CLOSE_LADDER = Float64Array.from([NO_STRIDE_BAND, 70, 60, 60, 55, NO_STRIDE_BAND]);

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
function factorOf(patch: Patch, camera: Point, ladder: Float64Array) {
  const bands = strideBandsOf(ladder, patch.level);
  const word = quadrantWordOf(patch);
  return (x: number, y: number, level: number, values: ReadonlyArray<number>): number =>
    strideHeldFactor(x, y, level, word) ??
    strideMorphFactor(bands[level] ?? NO_STRIDE_BAND, distanceOf(values, camera));
}

/** One drawn patch: each vertex's morphed place, each block's stride. */
interface Drawn {
  readonly patch: Patch;
  readonly places: ReadonlyArray<ReadonlyArray<number>>;
  readonly strides: ReadonlyArray<number>;
}

function drawPatch(
  patch: Patch,
  height: (x: number, y: number) => number,
  camera: Point,
  ladder: Float64Array = LADDER,
): Drawn {
  const place = placeOf(patch, height, camera);
  const factor = factorOf(patch, camera, ladder);
  const places: number[][] = [];
  for (let y = 0; y <= QUADS; y += 1) {
    for (let x = 0; x <= QUADS; x += 1) {
      places.push(stridePlace(x, y, place, factor));
    }
  }
  const bands = strideBandsOf(ladder, patch.level);
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
      const touchesFiner =
        (bx === 0 && patch.holds[0] === 1) ||
        (bx === 7 && patch.holds[1] === 1) ||
        (by === 0 && patch.holds[2] === 1) ||
        (by === 7 && patch.holds[3] === 1);
      strides.push(1 << cameraStrideLevel(bands, touchesFiner ? 2 : 3, nearest));
    }
  }
  return { patch, places, strides };
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

  it("gives a level its ladder's three bands, none below level 0", () => {
    expect(strideBandsOf(LADDER, 4)).toEqual([60, 120, 200]);
    expect(strideBandsOf(LADDER, 5)).toEqual([20, 60, 120]);
    expect(strideBandsOf(LADDER, 2)).toEqual([200, NO_STRIDE_BAND, NO_STRIDE_BAND]);
  });
});

describe("the stride morph's surface", () => {
  const cameras: Point[] = [];
  const random = seededRandom(0x5a7e);
  for (let n = 0; n < 24; n += 1) {
    cameras.push([-100 + 450 * random(), -80 + 300 * random(), 4 + 160 * random() ** 2]);
  }

  it.each([
    ["spaced", LADDER],
    ["overlapping", CLOSE_LADDER],
  ] as const)(
    "draws each block at the strides it takes exactly as the morph places its every vertex, its ladder's bands %s",
    (_name, ladder) => {
      let strided = 0;
      let worst = 0;
      cameras.forEach((camera, n) => {
        const height = heightOf(n);
        for (const patch of PATCHES) {
          const drawn = drawPatch(patch, height, camera, ladder);
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

  it.each([
    ["spaced", LADDER],
    ["overlapping", CLOSE_LADDER],
  ] as const)(
    "joins every patch to its neighbours along their edges, coarse–fine edges included, its ladder's bands %s",
    (_name, ladder) => {
      let worst = 0;
      let differing = 0;
      cameras.forEach((camera, n) => {
        const height = heightOf(n);
        const drawn = PATCHES.map((patch) => drawPatch(patch, height, camera, ladder));
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
        const edges: Array<readonly [Drawn, Drawn, (t: number) => readonly [number, number]]> = [
          [p, q00, (t) => [128, t]],
          [p, q01, (t) => [128, 64 + t]],
          [q00, q10, (t) => [192, t]],
          [q01, q11, (t) => [192, 64 + t]],
          [q00, q01, (t) => [128 + t, 64]],
          [q10, q11, (t) => [192 + t, 64]],
        ];
        for (const [a, b, along] of edges) {
          for (let t = 0; t <= 64; t += 0.25) {
            const [gx, gy] = along(t);
            worst = Math.max(worst, gap(surfaceAtLattice(a, gx, gy), surfaceAtLattice(b, gx, gy)));
          }
          differing += a.strides.some((s, i) => s !== b.strides[i]) ? 1 : 0;
        }
      });
      expect(differing).toBeGreaterThan(20);
      expect(worst).toBeLessThan(1e-9);
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
    for (const camera of cameras) {
      const drawn = drawPatch(patch, height, camera);
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

describe("the levels' measured errors", () => {
  it("raises a level's errors at once and lowers them by their decay a frame", () => {
    const errors = new StrideLevelErrors();
    errors.raise(5, [1, 2, 4]);
    errors.decay();
    expect([...errors.errorsM.subarray(15, 18)]).toEqual([
      1 - STRIDE_ERROR_DECAY,
      2 * (1 - STRIDE_ERROR_DECAY),
      4 * (1 - STRIDE_ERROR_DECAY),
    ]);
    errors.raise(5, [3, 0, 0]);
    expect(errors.errorsM[15]).toBe(3);
    expect(errors.errorsM[3]).toBe(-1);
  });

  it("keeps each level's largest e₂, e₄ and e₈ over the bakes it takes", () => {
    const errors = new StrideLevelErrors();
    errors.take(7, bakeErrors(1));
    errors.take(7, bakeErrors(0.5));
    errors.take(8, bakeErrors(2));
    expect([...errors.errorsM.subarray(21, 27)]).toEqual([1, 2, 4, 2, 4, 8]);
    expect(errors.errorsM[0]).toBe(-1);
    errors.clear();
    expect(errors.errorsM[21]).toBe(-1);
  });

  it("morphs at most three levels, towards strides 2, 4 and 8", () => {
    expect(STRIDE_MORPH_LEVELS).toBe(3);
  });
});

describe("the strides' WGSL constants", () => {
  it("are the twin's: the band, the morph's levels and nodes, and the forced region's bit", () => {
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
  });
});
