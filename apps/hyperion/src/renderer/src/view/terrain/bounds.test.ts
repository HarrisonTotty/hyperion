import { describe, expect, it } from "vitest";

import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { seededRandom } from "../../test/seededRandom";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import {
  distanceToBoxM,
  NORMAL_BEND_FACTOR,
  normalMarginM,
  PACKED_BOUNDS_LENGTH,
  packPatchBounds,
  PATCH_GEOMETRY_LENGTH,
  patchBounds,
  patchGeometryInto,
  relativeBounds,
  ROUNDING_ALLOWANCE_EPSILONS,
  type PatchBounds,
  unpackPatchBounds,
} from "./bounds";
import { faceUvToXyz, PATCH_QUADS, stToUv, unitDir, vertexDir, type Xyz } from "./cube";
import { FACES, MAX_LEVEL, type PatchKey, rootKey } from "./patchKey";
import {
  type BodyFigure,
  levelHeightRangeM,
  type PlanetGeometry,
  planetGeometry,
  spheroidNormal,
  spheroidPoint,
  surfacePoint,
} from "./planet";

const SPHERE: BodyFigure = { equatorialRadiusM: 6_371_000, polarRadiusM: 6_371_000, pole: null };

/** A figure of flattening 0.1 on WGS 84's equatorial radius: c = 0.9 a. */
const FLATTENED: BodyFigure = {
  equatorialRadiusM: WGS84_FIGURE.equatorialRadiusM,
  polarRadiusM: 0.9 * WGS84_FIGURE.equatorialRadiusM,
  pole: null,
};

/** 100 patches at levels 0 to 24 on every face, from a fixed seed. */
function samplePatches(): PatchKey[] {
  const random = seededRandom(0x626f756e);
  return Array.from({ length: 100 }, (_, n) => {
    const level = n % (MAX_LEVEL + 1);
    const side = 2 ** level;
    const face = FACES[n % 6] ?? 0;
    return {
      face,
      level,
      i: Math.floor(random() * side),
      j: Math.floor(random() * side),
    };
  });
}

/** Whether `p` lies inside the bounds' sphere and box, metres. */
function encloses(b: PatchBounds, p: { x: number; y: number; z: number }): boolean {
  const r = sub(p, b.centre);
  if (Math.sqrt(dot(r, r)) > b.radiusM) {
    return false;
  }
  const fromBox = sub(p, b.box.centre);
  return b.box.axes.every(
    (axis, k) => Math.abs(dot(fromBox, axis)) <= (b.box.halfExtentsM[k] ?? 0),
  );
}

function everyVertexEnclosed(planet: PlanetGeometry, key: PatchKey): boolean {
  const b = patchBounds(planet, key);
  for (let y = 0; y <= PATCH_QUADS; y += 1) {
    for (let x = 0; x <= PATCH_QUADS; x += 1) {
      const dir = vertexDir(key, x, y);
      for (const h of [b.minHeightM, b.maxHeightM, (b.minHeightM + b.maxHeightM) / 2]) {
        const [px, py, pz] = surfacePoint(planet.figure, dir, h);
        if (!encloses(b, vec3(px, py, pz))) {
          return false;
        }
      }
    }
  }
  return true;
}

describe("a patch's bounds", () => {
  it("has no height range and bounds the spheroid alone without a level table", () => {
    const planet = planetGeometry(WGS84_FIGURE, null);
    const b = patchBounds(planet, { face: 2, level: 10, i: 300, j: 700 });
    expect([b.minHeightM, b.maxHeightM]).toEqual([0, 0]);
    expect(everyVertexEnclosed(planet, { face: 2, level: 10, i: 300, j: 700 })).toBe(true);
  });

  it.each([
    ["a sphere", SPHERE],
    ["WGS 84's figure", WGS84_FIGURE],
  ])("encloses every vertex of 100 sampled patches on %s", (_, figure) => {
    const planet = planetGeometry(figure, goldenLevelTable("off"));
    const misses = samplePatches().filter((k) => !everyVertexEnclosed(planet, k));
    expect(misses).toEqual([]);
  });

  it("takes the level's height range from the table", () => {
    const table = goldenLevelTable("off");
    const b = patchBounds(planetGeometry(WGS84_FIGURE, table), { face: 0, level: 5, i: 3, j: 4 });
    expect([b.minHeightM, b.maxHeightM]).toEqual([table[21], table[22]]);
  });

  it("bounds a whole face", () => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    for (const face of FACES) {
      expect(everyVertexEnclosed(planet, rootKey(face))).toBe(true);
    }
  });

  it("puts a camera inside the box at no distance from it, and one above it at its height", () => {
    const planet = planetGeometry(SPHERE, null);
    const key: PatchKey = { face: 0, level: 12, i: 2048, j: 2048 };
    const b = patchBounds(planet, key);
    expect(distanceToBoxM(relativeBounds(b, b.centre))).toBe(0);
    const [cx, cy, cz] = surfacePoint(SPHERE, vertexDir(key, 32, 32), 10_000);
    const d = distanceToBoxM(relativeBounds(b, vec3(cx, cy, cz)));
    // The box's top stands above the centre vertex by the normal's margin, the boundary's sagitta
    // and the rounding allowance, under a millimetre at level 12; R05's, the largest chord between
    // its boundary samples, was 150 m (R10.T14.e).
    expect(d).toBeGreaterThan(10_000 - 0.001);
    expect(d).toBeLessThanOrEqual(10_000);
  });
});

/** The samples' extremes along a box's axes, from its centre, and the chord margin, metres. */
interface SampledSpans {
  /** Along each of the box's axes, the least and the greatest sample. */
  readonly spans: readonly [
    readonly [number, number],
    readonly [number, number],
    readonly [number, number],
  ];
  /** The farthest sample from the sphere's centre. */
  readonly farthestM: number;
  /** The largest chord between neighbouring ring samples at the top of the range. */
  readonly chordM: number;
  /**
   * A bound on the square of the largest angle between neighbouring ring samples' directions,
   * rad².
   */
  readonly angleSqRad2: number;
}

/**
 * Patch `key`'s bounds' samples on `planet` (every fourth boundary vertex and the centre vertex at
 * both ends of `range`), measured from `b`, the bounds built over them.
 */
function sampledSpans(
  planet: PlanetGeometry,
  key: PatchKey,
  range: readonly [number, number],
  b: PatchBounds,
): SampledSpans {
  const half = PATCH_QUADS / 2;
  const spans: [number, number][] = [
    [Infinity, -Infinity],
    [Infinity, -Infinity],
    [Infinity, -Infinity],
  ];
  let farthestM = 0;
  let chordM = 0;
  let turnSq = 0;
  let previous: { readonly top: Vec3; readonly dir: Vec3 } | null = null;
  const take = (p: Vec3): void => {
    const r = sub(p, b.box.centre);
    b.box.axes.forEach((axis, k) => {
      const s = dot(r, axis);
      const span = spans[k];
      if (span !== undefined) {
        span[0] = Math.min(span[0], s);
        span[1] = Math.max(span[1], s);
      }
    });
    farthestM = Math.max(farthestM, norm(sub(p, b.centre)));
  };
  for (const [x, y] of [...REFERENCE_RING, REFERENCE_RING[0] ?? [0, 0], [half, half] as const]) {
    const dir = vertexDir(key, x, y);
    const top = toVec3(surfacePoint(planet.figure, dir, range[1]));
    take(top);
    take(toVec3(surfacePoint(planet.figure, dir, range[0])));
    if (previous !== null && !(x === half && y === half)) {
      chordM = Math.max(chordM, norm(sub(top, previous.top)));
      const turn = sub(toVec3(dir), previous.dir);
      turnSq = Math.max(turnSq, dot(turn, turn));
    }
    previous = { top, dir: toVec3(dir) };
  }
  const [s0 = [0, 0], s1 = [0, 0], s2 = [0, 0]] = spans;
  return {
    spans: [s0, s1, s2],
    farthestM,
    chordM,
    angleSqRad2: turnSq / (1 - turnSq / 4),
  };
}

/** The largest magnitude of a height range's two ends, metres. */
function largestHeightM(range: readonly [number, number]): number {
  return Math.max(Math.abs(range[0]), Math.abs(range[1]));
}

/** Each axis's half-extent less the samples' half-span along it, metres: what the margin adds. */
function excessesM(b: PatchBounds, spans: SampledSpans["spans"]): readonly number[] {
  return b.box.halfExtentsM.map(
    (e, k) => e - ((spans[k]?.[1] ?? NaN) - (spans[k]?.[0] ?? NaN)) / 2,
  );
}

/**
 * A patch at each level from 0 to 24, from a fixed seed, with the level's range, one below the
 * datum and one above it.
 */
function marginCases(
  planet: PlanetGeometry,
): { readonly key: PatchKey; readonly range: readonly [number, number] }[] {
  const random = seededRandom(0x6e6f726d);
  return Array.from({ length: MAX_LEVEL + 1 }, (_, level) => {
    const side = 2 ** level;
    const key: PatchKey = {
      face: FACES[level % 6] ?? 0,
      level,
      i: Math.floor(random() * side),
      j: Math.floor(random() * side),
    };
    return [levelHeightRangeM(planet, level), [-24_500, -1_000] as const, [120, 180] as const].map(
      (range) => ({ key, range }),
    );
  }).flat();
}

/** Patches at levels 22 to 24 on every face, from a fixed seed. */
function finestPatches(): PatchKey[] {
  const random = seededRandom(0x756c7073);
  const keys: PatchKey[] = [];
  for (let level = 22; level <= MAX_LEVEL; level += 1) {
    for (const face of FACES) {
      const side = 2 ** level;
      keys.push({ face, level, i: Math.floor(random() * side), j: Math.floor(random() * side) });
    }
  }
  return keys;
}

describe("a patch's box along its centre's normal (plan R10, R10.T14.e)", () => {
  const PLANETS = [
    ["WGS 84's figure", planetGeometry(WGS84_FIGURE, goldenLevelTable("off"))],
    ["a sphere", planetGeometry(SPHERE, goldenLevelTable("off"))],
    ["a figure of flattening 0.1", planetGeometry(FLATTENED, goldenLevelTable("off"))],
  ] as const;
  const [[, EARTH]] = PLANETS;

  // The test's own projections round apart from the bounds' by a few nanometres, against a
  // normal's margin of 111 nm at level 19 and the allowance's 91 nm from level 22.
  const ROUNDING_M = 1e-8;

  it.each(PLANETS)(
    "pads the normal by the boundary's sagitta and the rounding allowance alone, on %s",
    (_, planet) => {
      const { equatorialRadiusM: a, polarRadiusM: c } = planet.figure;
      const misses = marginCases(planet).filter(({ key, range }) => {
        const b = patchBounds(planet, key, range);
        const { spans, angleSqRad2 } = sampledSpans(planet, key, range, b);
        const [normal = NaN] = excessesM(b, spans);
        return !(
          Math.abs(normal - normalMarginM(angleSqRad2, a, c, largestHeightM(range))) < ROUNDING_M
        );
      });
      expect(misses).toEqual([]);
    },
  );

  it.each(PLANETS)(
    "pads the tangents and the sphere by the largest chord between the samples, as R05 built them, on %s",
    (_, planet) => {
      const misses = marginCases(planet).filter(({ key, range }) => {
        const b = patchBounds(planet, key, range);
        const { spans, farthestM, chordM } = sampledSpans(planet, key, range, b);
        const [, t1 = NaN, t2 = NaN] = excessesM(b, spans);
        return !(
          Math.abs(t1 - chordM) < ROUNDING_M &&
          Math.abs(t2 - chordM) < ROUNDING_M &&
          Math.abs(b.radiusM - farthestM - chordM) < ROUNDING_M
        );
      });
      expect(misses).toEqual([]);
    },
  );

  it("pads a level-10 patch on WGS 84 by under a centimetre along its normal, where R05's chord is hundreds of metres", () => {
    const key: PatchKey = { face: 2, level: 10, i: 300, j: 700 };
    const range = levelHeightRangeM(EARTH, 10);
    const b = patchBounds(EARTH, key, range);
    const [normal] = excessesM(b, sampledSpans(EARTH, key, range, b).spans);
    expect(normal).toBeGreaterThan(0);
    expect(normal).toBeLessThan(0.01);
  });

  // Past c² ÷ a, the least radius of curvature, the normal's argument does not hold.
  const LEAST_RADIUS_M =
    (FLATTENED.polarRadiusM * FLATTENED.polarRadiusM) / FLATTENED.equatorialRadiusM;
  it.each([
    {
      name: "takes the sagitta with every height just within",
      depthM: LEAST_RADIUS_M * (1 - 1e-9),
      chord: false,
    },
    { name: "keeps R05's chord with a height at", depthM: LEAST_RADIUS_M, chord: true },
  ])("$name the least radius of curvature c² ÷ a below the datum", ({ depthM, chord }) => {
    const [, , [, planet]] = PLANETS;
    const { equatorialRadiusM: a, polarRadiusM: c } = planet.figure;
    const key: PatchKey = { face: 1, level: 5, i: 11, j: 20 };
    const range = [-depthM, 0] as const;
    const b = patchBounds(planet, key, range);
    const { spans, chordM, angleSqRad2 } = sampledSpans(planet, key, range, b);
    const [normal = NaN] = excessesM(b, spans);
    const expected = chord ? chordM : normalMarginM(angleSqRad2, a, c, depthM);
    expect(Math.abs(normal - expected)).toBeLessThan(ROUNDING_M);
  });

  it("puts the sagitta of a 566 m chord on an Earth at about 6 mm", () => {
    const { equatorialRadiusM: a, polarRadiusM: c } = WGS84_FIGURE;
    // The chord's angle on the sphere of a, Θ ≈ L ÷ a; the sagitta R Θ² ÷ 8 ≈ L² ÷ (8R).
    const sagittaM = normalMarginM((566 / a) ** 2, a, c, 0) - normalMarginM(0, a, c, 0);
    expect(sagittaM).toBeCloseTo(566 ** 2 / (8 * a), 9);
    expect(sagittaM).toBeGreaterThan(0.006);
    expect(sagittaM).toBeLessThan(0.0065);
  });

  it("allows about 91 nm for the rounding on an Earth", () => {
    const { equatorialRadiusM: a, polarRadiusM: c } = WGS84_FIGURE;
    expect(normalMarginM(0, a, c, 0)).toBeCloseTo(9.06e-8, 9);
  });

  it("bends the sagitta 0.45% more under ±24.5 km of relief on an Earth", () => {
    const { equatorialRadiusM: a, polarRadiusM: c } = WGS84_FIGURE;
    const angleSq = (566 / a) ** 2;
    const sagittaM = (h: number): number =>
      normalMarginM(angleSq, a, c, h) - normalMarginM(0, a, c, h);
    const bend = sagittaM(24_500) / sagittaM(0) - 1;
    expect(bend).toBeCloseTo((NORMAL_BEND_FACTOR * 24_500 * a) / (c * c), 6);
    expect(bend).toBeCloseTo(0.0045, 4);
  });

  it("puts the sagitta below an ulp of a from level 22", () => {
    const { equatorialRadiusM: a, polarRadiusM: c } = EARTH.figure;
    // An ulp of a, 2⁻³⁰ m for a in [2²², 2²³).
    const ulpM = 2 ** (Math.floor(Math.log2(a)) - 52);
    const over = finestPatches().filter((key) => {
      const range = levelHeightRangeM(EARTH, key.level);
      const { angleSqRad2 } = sampledSpans(EARTH, key, range, patchBounds(EARTH, key, range));
      const heightM = largestHeightM(range);
      const sagittaM = normalMarginM(angleSqRad2, a, c, heightM) - normalMarginM(0, a, c, heightM);
      return !(sagittaM >= 0 && sagittaM < ulpM);
    });
    expect(over).toEqual([]);
  });

  it("encloses every vertex at levels 22 to 24, the rounding allowance alone padding the normal", () => {
    expect(finestPatches().filter((key) => !everyVertexEnclosed(EARTH, key))).toEqual([]);
  });
});
/** The figures the enclosure test covers, each with its name. */
const ENCLOSURE_FIGURES = [
  ["WGS 84's figure", WGS84_FIGURE],
  ["a sphere", SPHERE],
  ["a figure of flattening 0.1", FLATTENED],
] as const;

/** Points a side of the enclosure test's sampling of a patch's surface. */
const SURFACE_SAMPLES = 257;

/**
 * One patch at each level from 0 to 19, in turn at a cube corner, on a face's edge and inside a
 * face, the turn shifted by `shift` so that each figure takes each kind at other levels.
 */
function enclosurePatches(shift: number): PatchKey[] {
  const random = seededRandom(0x656e636c + shift);
  return Array.from({ length: 20 }, (_, level) => {
    const last = 2 ** level - 1;
    const face = FACES[(level + shift) % 6] ?? 0;
    const inner = (): number => (last < 2 ? 0 : 1 + Math.floor(random() * (last - 1)));
    switch ((level + shift) % 3) {
      case 0:
        return { face, level, i: level % 2 === 0 ? 0 : last, j: last };
      case 1:
        return { face, level, i: last, j: inner() };
      default:
        return { face, level, i: inner(), j: inner() };
    }
  });
}

/** How a patch's volume lies against its bounds. */
interface Enclosure {
  /** The points outside the sphere or the box. */
  readonly outside: number;
  /** The most a point lies past the samples' extremes along the normal, metres (≤ 0 if none). */
  readonly worstNormalExcessM: number;
  /** The normal's margin as built: the box's half-extent less the samples' half-span, metres. */
  readonly normalMarginM: number;
}

/**
 * Every vertex of patch `key`'s 65 × 65 grid and a {@link SURFACE_SAMPLES}-a-side sampling of its
 * surface at its own face's (s, t), at both ends of `range`, against its bounds.
 */
function enclosure(
  planet: PlanetGeometry,
  key: PatchKey,
  range: readonly [number, number],
): Enclosure {
  const b = patchBounds(planet, key, range);
  const { spans } = sampledSpans(planet, key, range, b);
  const [[lo0, hi0]] = spans;
  const [n, t1, t2] = b.box.axes;
  const [e0, e1, e2] = b.box.halfExtentsM;
  const { x: bx, y: by, z: bz } = b.box.centre;
  const { x: sx, y: sy, z: sz } = b.centre;
  let outside = 0;
  let worst = -Infinity;
  const test = (dir: Xyz): void => {
    const p = spheroidPoint(planet.figure, dir);
    const nu = spheroidNormal(planet.figure, dir);
    for (const h of range) {
      // `surfacePoint`'s operations.
      const x = p[0] + h * nu[0];
      const y = p[1] + h * nu[1];
      const z = p[2] + h * nu[2];
      const rx = x - bx;
      const ry = y - by;
      const rz = z - bz;
      const s0 = rx * n.x + ry * n.y + rz * n.z;
      const s1 = rx * t1.x + ry * t1.y + rz * t1.z;
      const s2 = rx * t2.x + ry * t2.y + rz * t2.z;
      worst = Math.max(worst, s0 - hi0, lo0 - s0);
      const qx = x - sx;
      const qy = y - sy;
      const qz = z - sz;
      if (
        !(Math.abs(s0) <= e0 && Math.abs(s1) <= e1 && Math.abs(s2) <= e2) ||
        Math.sqrt(qx * qx + qy * qy + qz * qz) > b.radiusM
      ) {
        outside += 1;
      }
    }
  };
  for (let y = 0; y <= PATCH_QUADS; y += 1) {
    for (let x = 0; x <= PATCH_QUADS; x += 1) {
      test(vertexDir(key, x, y));
    }
  }
  const steps = SURFACE_SAMPLES - 1;
  const cells = steps * 2 ** key.level;
  const us = Array.from({ length: SURFACE_SAMPLES }, (_, k) => stToUv((key.i * steps + k) / cells));
  const vs = Array.from({ length: SURFACE_SAMPLES }, (_, k) => stToUv((key.j * steps + k) / cells));
  for (const v of vs) {
    for (const u of us) {
      test(unitDir(faceUvToXyz(key.face, u, v)));
    }
  }
  return { outside, worstNormalExcessM: worst, normalMarginM: e0 - (hi0 - lo0) / 2 };
}

describe("a patch's bounds enclose its surface (plan R10, R10.T14.e)", () => {
  it.each(ENCLOSURE_FIGURES.map(([name, figure], shift) => [name, figure, shift] as const))(
    "encloses every vertex and a 257 × 257 sampling of the surface, at both ends of the range, on %s",
    (_, figure, shift) => {
      const planet = planetGeometry(figure, goldenLevelTable("off"));
      const misses: string[] = [];
      for (const key of enclosurePatches(shift)) {
        // The level's range, a range wholly below the datum, as deep as the table reaches, and a
        // baked patch's narrow one above it.
        for (const range of [
          levelHeightRangeM(planet, key.level),
          [-24_500, -1_000] as const,
          [120, 180] as const,
        ]) {
          const e = enclosure(planet, key, range);
          if (e.outside > 0 || !(e.worstNormalExcessM <= e.normalMarginM)) {
            misses.push(
              `${key.face}/${key.level}/${key.i}/${key.j} [${range.join(", ")}]: ${e.outside} outside, ${e.worstNormalExcessM} m past against ${e.normalMarginM} m`,
            );
          }
        }
      }
      expect(misses).toEqual([]);
    },
  );
});

function toVec3(p: Xyz): Vec3 {
  return vec3(p[0], p[1], p[2]);
}

/** The boundary's vertices, every fourth, in order round the patch. */
const REFERENCE_RING: readonly (readonly [number, number])[] = [
  ...Array.from({ length: 16 }, (_, n) => [4 * n, 0] as const),
  ...Array.from({ length: 16 }, (_, n) => [PATCH_QUADS, 4 * n] as const),
  ...Array.from({ length: 16 }, (_, n) => [PATCH_QUADS - 4 * n, PATCH_QUADS] as const),
  ...Array.from({ length: 16 }, (_, n) => [0, PATCH_QUADS - 4 * n] as const),
];

/**
 * `patchBounds` in the vector form it had before R05.T7 perf (c) built it in scalars: the oracle the
 * scalar form must match bit for bit.
 */
function referenceBounds(
  planet: PlanetGeometry,
  key: PatchKey,
  heightRangeM: readonly [number, number],
): PatchBounds {
  const [minHeightM, maxHeightM] = heightRangeM;
  const half = PATCH_QUADS / 2;
  const centreDir = vertexDir(key, half, half);
  const origin = toVec3(surfacePoint(planet.figure, centreDir, (minHeightM + maxHeightM) / 2));
  const n = toVec3(spheroidNormal(planet.figure, centreDir));
  const across = sub(toVec3(vertexDir(key, PATCH_QUADS, half)), toVec3(vertexDir(key, 0, half)));
  const t1 = normalise(sub(across, scale(n, dot(across, n))));
  const t2 = cross(n, t1);
  const axes: readonly [Vec3, Vec3, Vec3] = [n, t1, t2];
  const points: Vec3[] = [];
  let ringTop: Vec3 | null = null;
  let ringDir: Vec3 | null = null;
  let marginM = 0;
  let largestTurnSq = 0;
  for (const [x, y] of [...REFERENCE_RING, REFERENCE_RING[0] ?? [0, 0]]) {
    const dir = vertexDir(key, x, y);
    const top = toVec3(surfacePoint(planet.figure, dir, maxHeightM));
    if (ringTop !== null) {
      marginM = Math.max(marginM, norm(sub(top, ringTop)));
    }
    if (ringDir !== null) {
      const turn = sub(toVec3(dir), ringDir);
      largestTurnSq = Math.max(largestTurnSq, dot(turn, turn));
    }
    ringTop = top;
    ringDir = toVec3(dir);
    points.push(top, toVec3(surfacePoint(planet.figure, dir, minHeightM)));
  }
  // R10.T14.e: the normal takes the boundary's sagitta and the rounding allowance, the tangents and
  // the sphere the chord.
  const a = planet.figure.equatorialRadiusM;
  const c = planet.figure.polarRadiusM;
  const angleSq = largestTurnSq / (1 - largestTurnSq / 4);
  const heightM = Math.max(Math.abs(minHeightM), Math.abs(maxHeightM));
  const normalM =
    heightM < (c * c) / a
      ? (angleSq * (a + NORMAL_BEND_FACTOR * heightM * ((a / c) * (a / c)))) / 8 +
        ROUNDING_ALLOWANCE_EPSILONS * Number.EPSILON * (a + heightM)
      : marginM;
  const margins = [normalM, marginM, marginM];
  points.push(
    toVec3(surfacePoint(planet.figure, centreDir, maxHeightM)),
    toVec3(surfacePoint(planet.figure, centreDir, minHeightM)),
  );
  const lows = [Infinity, Infinity, Infinity];
  const highs = [-Infinity, -Infinity, -Infinity];
  for (const p of points) {
    const r = sub(p, origin);
    axes.forEach((axis, k) => {
      const s = dot(r, axis);
      lows[k] = Math.min(lows[k] ?? Infinity, s);
      highs[k] = Math.max(highs[k] ?? -Infinity, s);
    });
  }
  let boxCentre = origin;
  const extents = axes.map((axis, k) => {
    const lo = lows[k] ?? 0;
    const hi = highs[k] ?? 0;
    boxCentre = add(boxCentre, scale(axis, (lo + hi) / 2));
    return (hi - lo) / 2 + (margins[k] ?? NaN);
  });
  let radiusM = 0;
  for (const p of points) {
    radiusM = Math.max(radiusM, norm(sub(p, boxCentre)));
  }
  return {
    centre: boxCentre,
    radiusM: radiusM + marginM,
    minHeightM,
    maxHeightM,
    box: {
      centre: boxCentre,
      axes,
      halfExtentsM: [extents[0] ?? 0, extents[1] ?? 0, extents[2] ?? 0],
    },
  };
}

/** Every number of a patch's bounds, in a fixed order. */
function boundsNumbers(b: PatchBounds): number[] {
  return [
    b.centre.x,
    b.centre.y,
    b.centre.z,
    b.radiusM,
    b.minHeightM,
    b.maxHeightM,
    b.box.centre.x,
    b.box.centre.y,
    b.box.centre.z,
    ...b.box.axes.flatMap((a) => [a.x, a.y, a.z]),
    ...b.box.halfExtentsM,
  ];
}

describe("a patch's bounds, built in scalars", () => {
  it("equal the vector form's bit for bit, at face edges, cube corners and inside faces", () => {
    const random = seededRandom(0x62697473);
    const planets = [
      planetGeometry(WGS84_FIGURE, goldenLevelTable("off")),
      planetGeometry(WGS84_FIGURE, goldenLevelTable("on")),
      planetGeometry(SPHERE, null),
    ];
    const keys: PatchKey[] = [];
    for (const face of FACES) {
      for (let level = 0; level <= MAX_LEVEL; level += 1) {
        const last = 2 ** level - 1;
        // The four corners, an edge cell on each side and a random cell.
        for (const [i, j] of [
          [0, 0],
          [last, 0],
          [0, last],
          [last, last],
          [Math.floor(random() * (last + 1)), 0],
          [last, Math.floor(random() * (last + 1))],
          [Math.floor(random() * (last + 1)), last],
          [0, Math.floor(random() * (last + 1))],
          [Math.floor(random() * (last + 1)), Math.floor(random() * (last + 1))],
        ] as const) {
          keys.push({ face, level, i, j });
        }
      }
    }
    const mismatches: string[] = [];
    for (const planet of planets) {
      for (const key of keys) {
        const [low, high] = levelHeightRangeM(planet, key.level);
        const { equatorialRadiusM: a, polarRadiusM: c } = planet.figure;
        // The level's range, a baked one inside it, one of a single height, and one reaching the
        // least radius of curvature, where the normal keeps the chord.
        for (const range of [
          [low, high],
          [-12.25 - random() * 100, 40.5 + random() * 100],
          [-0, -0],
          [-((c * c) / a), 40.5],
        ] as const) {
          const got = boundsNumbers(patchBounds(planet, key, range));
          const want = boundsNumbers(referenceBounds(planet, key, range));
          if (!got.every((x, n) => Object.is(x, want[n]))) {
            mismatches.push(`${key.face}/${key.level}/${key.i}/${key.j} [${range.join(", ")}]`);
          }
        }
      }
    }
    expect(keys.length).toBe(6 * 25 * 9);
    expect(mismatches).toEqual([]);
  });

  it("are the same numbers built from a patch's geometry kept for several height ranges", () => {
    const random = seededRandom(0x67656f6d);
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("on"));
    const geometry = new Float64Array(PATCH_GEOMETRY_LENGTH);
    const packed = new Float64Array(PACKED_BOUNDS_LENGTH);
    const mismatches: string[] = [];
    for (let n = 0; n < 200; n += 1) {
      const level = Math.floor(random() * (MAX_LEVEL + 1));
      const side = 2 ** level;
      const key: PatchKey = {
        face: FACES[n % 6] ?? 0,
        level,
        i: n % 7 === 0 ? side - 1 : Math.floor(random() * side),
        j: n % 5 === 0 ? 0 : Math.floor(random() * side),
      };
      patchGeometryInto(geometry, planet, key);
      const [low, high] = levelHeightRangeM(planet, key.level);
      // The level's range, then tighter ones as bakes land, all from the one geometry.
      for (const range of [
        [low, high],
        [-300 - random() * 900, 200 + random() * 900],
        [-12.25, 40.5],
      ] as const) {
        packPatchBounds(packed, geometry, range[0], range[1]);
        const got = boundsNumbers(unpackPatchBounds(packed));
        const want = boundsNumbers(referenceBounds(planet, key, range));
        if (!got.every((x, m) => Object.is(x, want[m]))) {
          mismatches.push(`${key.face}/${key.level}/${key.i}/${key.j} [${range.join(", ")}]`);
        }
      }
    }
    expect(mismatches).toEqual([]);
  });

  it("takes the level's range by default", () => {
    const planet = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
    const key: PatchKey = { face: 4, level: 13, i: 8191, j: 77 };
    expect(boundsNumbers(patchBounds(planet, key))).toEqual(
      boundsNumbers(referenceBounds(planet, key, levelHeightRangeM(planet, key.level))),
    );
  });
});
