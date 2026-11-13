import { describe, expect, it } from "vitest";

import { dot, normalise, type Vec3, vec3 } from "../../geometry/vec3";
import { memoised } from "../../test/memo";
import { goldenLevelTable, WGS84_FIGURE } from "../../test/terrainFixtures";
import type { Quaternion } from "../camera/pose";
import {
  perspectiveReversedInfinite,
  NEAR_PLANE_M,
  project,
  toViewAxes,
} from "../camera/projection";
import { lookAlong, quaternionFromAxisAngle, rotate } from "../camera/quaternion";
import type { ViewSize } from "../engine/types";
import { QUALITY_SETTINGS, type QualitySetting } from "../quality/qualitySetting";
import { relativeBounds } from "../terrain/bounds";
import { PATCH_QUADS, vertexDir } from "../terrain/cube";
import { type Frustum, frustumOf, inFrustum } from "../terrain/cull";
import { parentKey, type PatchKey, patchKeyString } from "../terrain/patchKey";
import { planetGeometry, surfacePoint } from "../terrain/planet";
import {
  type SelectedPatch,
  type Selection,
  selectPatches,
  type ViewSelectionInput,
} from "../terrain/select";
import {
  footprintView,
  MAX_DISPATCH_NS,
  PROBE_WORKGROUPS,
  planTiles,
  type StillTile,
  type StillTiling,
  splitDispatch,
  STILL_TILE_MAX_PX,
  tileFrustum,
  tileLimitPx,
  tileProjection,
} from "./stillPlan";

/** `maxTextureDimension2D`: WebGPU's default (the specification's Limits table), the UHD 620's. */
const LIMITS = [8192, 16_384] as const;

/** The stills the tiling is checked on: tiny, HD, UHD, wider and taller than L, widths off 64. */
const SIZES: ReadonlyArray<ViewSize> = [
  { widthPx: 1, heightPx: 1 },
  { widthPx: 1920, heightPx: 1080 },
  { widthPx: 3840, heightPx: 2160 },
  { widthPx: 9000, heightPx: 1200 },
  { widthPx: 1200, heightPx: 9000 },
  { widthPx: 1000, heightPx: 700 },
  { widthPx: 3001, heightPx: 2001 },
];

interface TilingCase {
  readonly name: string;
  readonly size: ViewSize;
  readonly limit: number;
  readonly setting: QualitySetting;
}

const CASES: ReadonlyArray<TilingCase> = [
  ...QUALITY_SETTINGS.flatMap((setting) =>
    LIMITS.flatMap((limit) =>
      SIZES.map((size) => ({
        name: `${size.widthPx} × ${size.heightPx}, ${setting}, limit ${limit}`,
        size,
        limit,
        setting,
      })),
    ),
  ),
  // Limits that are not multiples of 64, under each setting's figure.
  {
    name: "5000 × 4100, low, limit 2000",
    size: { widthPx: 5000, heightPx: 4100 },
    limit: 2000,
    setting: "low",
  },
  {
    name: "9000 × 9000, high, limit 3000",
    size: { widthPx: 9000, heightPx: 9000 },
    limit: 3000,
    setting: "high",
  },
];

function tiling(c: TilingCase): StillTiling {
  return planTiles(c.size, c.limit, c.setting);
}

function overlaps(a: StillTile, b: StillTile): boolean {
  return (
    a.rect.x0Px < b.rect.x1Px &&
    b.rect.x0Px < a.rect.x1Px &&
    a.rect.y0Px < b.rect.y1Px &&
    b.rect.y0Px < a.rect.y1Px
  );
}

/** The tiles of a 3840 × 2160 still on `setting`, as column, row and texture size. */
function uhdGrid(setting: QualitySetting): string[] {
  return planTiles({ widthPx: 3840, heightPx: 2160 }, 8192, setting).tiles.map(
    (t) => `${t.column},${t.row}:${t.texture.widthPx}×${t.texture.heightPx}`,
  );
}

describe("the still's tiles", () => {
  it.each(CASES)("cover the still exactly once ($name)", (c) => {
    const { tiles } = tiling(c);
    // Inside the still, pairwise disjoint and of the still's area between them: exactly once.
    const inside = tiles.every(
      (t) =>
        t.rect.x0Px >= 0 &&
        t.rect.y0Px >= 0 &&
        t.rect.x1Px <= c.size.widthPx &&
        t.rect.y1Px <= c.size.heightPx &&
        t.rect.x0Px < t.rect.x1Px &&
        t.rect.y0Px < t.rect.y1Px,
    );
    const disjoint = tiles.every((a, i) => tiles.slice(i + 1).every((b) => !overlaps(a, b)));
    const area = tiles.reduce(
      (sum, t) => sum + (t.rect.x1Px - t.rect.x0Px) * (t.rect.y1Px - t.rect.y0Px),
      0,
    );
    expect({ inside, disjoint, area }).toEqual({
      inside: true,
      disjoint: true,
      area: c.size.widthPx * c.size.heightPx,
    });
  });

  it.each(CASES)("hold no texture side over the tile limit ($name)", (c) => {
    const limit = Math.min(c.limit, STILL_TILE_MAX_PX[c.setting]);
    const over = tiling(c).tiles.filter(
      (t) => t.texture.widthPx > limit || t.texture.heightPx > limit,
    );
    expect(over).toEqual([]);
  });

  it.each(CASES)(
    "have texture widths a multiple of 64 px and bytesPerRow a multiple of 256 ($name)",
    (c) => {
      const misaligned = tiling(c).tiles.filter(
        (t) =>
          t.texture.widthPx % 64 !== 0 ||
          t.bytesPerRow % 256 !== 0 ||
          t.bytesPerRow !== 4 * t.texture.widthPx,
      );
      expect(misaligned).toEqual([]);
    },
  );

  it.each(CASES)("pad only past the still's right edge, by under 64 px ($name)", (c) => {
    const wrong = tiling(c).tiles.filter((t) => {
      const padPx = t.texture.widthPx - (t.rect.x1Px - t.rect.x0Px);
      return (
        t.texture.heightPx !== t.rect.y1Px - t.rect.y0Px ||
        padPx < 0 ||
        padPx >= 64 ||
        (padPx > 0 && t.rect.x1Px !== c.size.widthPx)
      );
    });
    expect(wrong).toEqual([]);
  });

  it("take L as the smaller of the adapter's limit and the setting's", () => {
    expect([
      tileLimitPx(8192, "low"),
      tileLimitPx(16_384, "low"),
      tileLimitPx(8192, "high"),
      tileLimitPx(16_384, "high"),
      tileLimitPx(2000, "low"),
      tileLimitPx(3000, "high"),
    ]).toEqual([2048, 2048, 4096, 4096, 2000, 3000]);
  });

  it("floor widths to a multiple of 64 under a limit that is not one", () => {
    const widths = planTiles({ widthPx: 5000, heightPx: 100 }, 2000, "low").tiles.map(
      (t) => t.texture.widthPx,
    );
    // 5000 px is 79 steps of 64; 2000 px holds 31, so three columns of 27, 26 and 26 steps.
    expect(widths).toEqual([1728, 1664, 1664]);
  });

  it("cut 3840 × 2160 into 2 × 2 tiles of 1920 × 1080 on low and one tile on high", () => {
    expect({ low: uhdGrid("low"), high: uhdGrid("high") }).toEqual({
      low: ["0,0:1920×1080", "1,0:1920×1080", "0,1:1920×1080", "1,1:1920×1080"],
      high: ["0,0:3840×2160"],
    });
  });

  it.each([
    ["a zero width", { widthPx: 0, heightPx: 10 }, 8192],
    ["a negative height", { widthPx: 10, heightPx: -1 }, 8192],
    ["a fractional width", { widthPx: 10.5, heightPx: 10 }, 8192],
    ["a non-finite height", { widthPx: 10, heightPx: Number.NaN }, 8192],
    ["a zero limit", { widthPx: 10, heightPx: 10 }, 0],
    ["a fractional limit", { widthPx: 10, heightPx: 10 }, 4096.5],
    ["a limit under 64 px", { widthPx: 10, heightPx: 10 }, 63],
  ] as const)("refuse %s", (_, size, limit) => {
    expect(() => planTiles(size, limit, "high")).toThrow(RangeError);
  });
});

/** The unit roundoff of `f32`: one rounding to nearest is off by at most 2⁻²⁴ of the value. */
const F32_UNIT_ROUNDOFF = 2 ** -24;

const FOV_X_RAD = (50 * Math.PI) / 180;
const ORIENTATION: Quaternion = quaternionFromAxisAngle(normalise(vec3(1, 2, 3)), 0.7);
const CAMERA = { orientation: ORIENTATION, fovXRad: FOV_X_RAD };

/** The camera-relative point, `distanceM` away, that R02's `project` puts at the still's pixel. */
function pointAt(still: ViewSize, xPx: number, yPx: number, distanceM: number): Vec3 {
  const s = 1 / Math.tan(FOV_X_RAD / 2);
  const aspect = still.widthPx / still.heightPx;
  const ndcX = (2 * xPx) / still.widthPx - 1;
  const ndcY = 1 - (2 * yPx) / still.heightPx;
  const view = vec3((ndcX / s) * distanceM, (ndcY / (s * aspect)) * distanceM, -distanceM);
  return rotate(ORIENTATION, view);
}

interface TilePixel {
  readonly xPx: number;
  readonly yPx: number;
  /** The most the pixel can be off, px, from the single `f32` rounding of M · P's entries. */
  readonly toleranceXPx: number;
  readonly toleranceYPx: number;
}

/**
 * `v` through the tile's M · P, applied in `f64` to its `f32` entries, as the still's pixel: the
 * tile's pixel plus (x₀, y₀).
 *
 * @remarks
 * The tolerance is derived from the single rounding: each entry of M · P is off its exact value by
 * at most u = 2⁻²⁴ of itself. Tile NDC x is m₀ x ÷ w + m₈ z ÷ w = m₀ x ÷ w − m₈, so it is off by at
 * most u (|m₀ x ÷ w| + |m₈|), and the pixel by half the texture's width times that; y likewise with
 * m₅ and m₉. The `f64` arithmetic on both sides adds a few units of 2⁻⁵³ of the coordinates, which
 * 64 ε (W + H) covers, ε = 2⁻⁵².
 */
function throughTile(m: Float32Array, tile: StillTile, still: ViewSize, v: Vec3): TilePixel {
  const m0 = m[0] ?? Number.NaN;
  const m5 = m[5] ?? Number.NaN;
  const m8 = m[8] ?? Number.NaN;
  const m9 = m[9] ?? Number.NaN;
  const view = toViewAxes(v, ORIENTATION);
  const w = -view.z;
  const clipX = m0 * view.x + m8 * view.z;
  const clipY = m5 * view.y + m9 * view.z;
  const slackPx = 64 * Number.EPSILON * (still.widthPx + still.heightPx);
  return {
    xPx: ((clipX / w + 1) / 2) * tile.texture.widthPx + tile.rect.x0Px,
    yPx: ((1 - clipY / w) / 2) * tile.texture.heightPx + tile.rect.y0Px,
    toleranceXPx:
      (tile.texture.widthPx / 2) *
        F32_UNIT_ROUNDOFF *
        (Math.abs((m0 * view.x) / w) + Math.abs(m8)) +
      slackPx,
    toleranceYPx:
      (tile.texture.heightPx / 2) *
        F32_UNIT_ROUNDOFF *
        (Math.abs((m5 * view.y) / w) + Math.abs(m9)) +
      slackPx,
  };
}

/** Pixels of the still near each corner of `tile`'s rect, on both sides of each edge. */
function nearCorners(tile: StillTile): Array<readonly [number, number]> {
  const xs = [
    tile.rect.x0Px - 0.3,
    tile.rect.x0Px + 0.3,
    tile.rect.x1Px - 0.3,
    tile.rect.x1Px + 0.3,
  ];
  const ys = [
    tile.rect.y0Px - 0.3,
    tile.rect.y0Px + 0.3,
    tile.rect.y1Px - 0.3,
    tile.rect.y1Px + 0.3,
  ];
  return xs.flatMap((x) => ys.map((y) => [x, y] as const));
}

/** Stills in several tiles, one with a padded last column, and one in a single tile. */
const PROJECTED_STILLS: ReadonlyArray<TilingCase> = [
  {
    name: "3840 × 2160 on low",
    size: { widthPx: 3840, heightPx: 2160 },
    limit: 8192,
    setting: "low",
  },
  {
    name: "3000 × 2000 on low",
    size: { widthPx: 3000, heightPx: 2000 },
    limit: 8192,
    setting: "low",
  },
  {
    name: "9000 × 1200 on low",
    size: { widthPx: 9000, heightPx: 1200 },
    limit: 16_384,
    setting: "low",
  },
  {
    name: "1000 × 700 on high",
    size: { widthPx: 1000, heightPx: 700 },
    limit: 8192,
    setting: "high",
  },
];

describe("a tile's projection", () => {
  it.each(PROJECTED_STILLS)(
    "puts a point on the still's pixel that R02's project gives untiled ($name)",
    (c) => {
      const misses: string[] = [];
      for (const tile of tiling(c).tiles) {
        const m = tileProjection(FOV_X_RAD, c.size, tile);
        for (const [x, y] of nearCorners(tile)) {
          for (const distanceM of [0.5, 1_000, 3.0e8]) {
            const v = pointAt(c.size, x, y, distanceM);
            const untiled = project(v, CAMERA, c.size);
            const tiled = throughTile(m, tile, c.size, v);
            const dx = Math.abs(tiled.xPx - untiled.xPx);
            const dy = Math.abs(tiled.yPx - untiled.yPx);
            if (dx > tiled.toleranceXPx || dy > tiled.toleranceYPx) {
              misses.push(`tile ${tile.column},${tile.row} at (${x}, ${y}): off ${dx}, ${dy}`);
            }
          }
        }
      }
      expect(misses).toEqual([]);
    },
  );

  it.each(PROJECTED_STILLS)("keeps P's rows 2 and 3 bit for bit ($name)", (c) => {
    const p = perspectiveReversedInfinite(
      FOV_X_RAD,
      c.size.widthPx / c.size.heightPx,
      NEAR_PLANE_M,
    );
    // Column-major: row r of column k is entry 4k + r.
    const depthRows = [2, 6, 10, 14, 3, 7, 11, 15];
    const differing = tiling(c).tiles.flatMap((tile) => {
      const m = tileProjection(FOV_X_RAD, c.size, tile);
      return depthRows
        .filter((i) => !Object.is(m[i], p[i]))
        .map((i) => `${tile.column},${tile.row}[${i}]`);
    });
    expect(differing).toEqual([]);
  });

  it("is P itself, bit for bit, for a still in one tile whose width is a multiple of 64", () => {
    const size = { widthPx: 1920, heightPx: 1080 };
    const [tile] = planTiles(size, 8192, "high").tiles;
    if (tile === undefined) {
      throw new Error("a 1920 × 1080 still has a tile");
    }
    const m = Array.from(tileProjection(FOV_X_RAD, size, tile));
    const p = Array.from(perspectiveReversedInfinite(FOV_X_RAD, 1920 / 1080, NEAR_PLANE_M));
    expect(m.map((x, i) => Object.is(x, p[i]))).toEqual(p.map(() => true));
  });
});

/** A frustum's planes as their normals' components and offsets, in order. */
function flatPlanes(frustum: Frustum): number[] {
  return frustum.planes.flatMap((p) => [p.normal.x, p.normal.y, p.normal.z, p.offsetM]);
}

describe("a tile's frustum", () => {
  it("is frustumOf's for the full still's rect", () => {
    const size = { widthPx: 1920, heightPx: 1080 };
    const [tile] = planTiles(size, 8192, "high").tiles;
    if (tile === undefined) {
      throw new Error("a 1920 × 1080 still has a tile");
    }
    const camera = { ...CAMERA, viewport: size };
    const ours = flatPlanes(tileFrustum(camera, tile));
    const theirs = flatPlanes(frustumOf(camera));
    expect(ours.map((x, i) => Math.abs(x - (theirs[i] ?? Number.NaN)) < 1e-12)).toEqual(
      theirs.map(() => true),
    );
  });

  it("holds a point exactly when R02's project puts it inside the tile's rect", () => {
    const size = { widthPx: 3000, heightPx: 2000 };
    const camera = { ...CAMERA, viewport: size };
    const wrong: string[] = [];
    for (const tile of planTiles(size, 8192, "low").tiles) {
      const frustum = tileFrustum(camera, tile);
      // A grid over the still, a quarter pixel off every pixel edge, so that no point is on a seam.
      for (let x = 0.25; x < size.widthPx; x += 37) {
        for (let y = 0.25; y < size.heightPx; y += 41) {
          const v = pointAt(size, x, y, 50);
          const inRect =
            x >= tile.rect.x0Px && x < tile.rect.x1Px && y >= tile.rect.y0Px && y < tile.rect.y1Px;
          const inPlanes = frustum.planes.every((p) => dot(p.normal, v) + p.offsetM >= 0);
          if (inRect !== inPlanes) {
            wrong.push(`tile ${tile.column},${tile.row} at (${x}, ${y})`);
          }
        }
      }
    }
    expect(wrong).toEqual([]);
  });

  it("widens each side by marginPx", () => {
    const size = { widthPx: 3840, heightPx: 2160 };
    const camera = { ...CAMERA, viewport: size };
    const tile = planTiles(size, 8192, "low").tiles.find((t) => t.column === 1 && t.row === 1);
    if (tile === undefined) {
      throw new Error("a 3840 × 2160 still on low has a tile at 1, 1");
    }
    // 0.4 px left of and above the tile's top-left corner: outside it, inside it widened by 0.5 px.
    const v = pointAt(size, tile.rect.x0Px - 0.4, tile.rect.y0Px - 0.4, 50);
    const holds = (marginPx: number): boolean =>
      tileFrustum(camera, tile, marginPx).planes.every((p) => dot(p.normal, v) + p.offsetM >= 0);
    expect({ none: holds(0), half: holds(0.5) }).toEqual({ none: false, half: true });
  });
});

describe("the footprint view", () => {
  it("takes the full still's size as the viewport, whatever the view had", () => {
    const view: ViewSelectionInput = {
      camera: { positionM: vec3(0, 0, 7e6), orientation: ORIENTATION },
      fovXRad: FOV_X_RAD,
      viewport: { widthPx: 1920, heightPx: 1080 },
      weight: 1,
      tauPx: 1,
    };
    expect(footprintView(view, { widthPx: 7680, heightPx: 4320 })).toEqual({
      ...view,
      viewport: { widthPx: 7680, heightPx: 4320 },
    });
  });
});

const PLANET = planetGeometry(WGS84_FIGURE, goldenLevelTable("off"));
const SITE: PatchKey = { face: 0, level: 19, i: 300_001, j: 200_003 };

function above(altitudeM: number): Vec3 {
  const [x, y, z] = surfacePoint(WGS84_FIGURE, vertexDir(SITE, 32, 32), altitudeM);
  return vec3(x, y, z);
}

/** A camera at `positionM` looking down at the ground, tilted `tiltRad` towards the horizon. */
function lookingDown(positionM: Vec3, tiltRad: number): Quaternion {
  const up = normalise(positionM);
  const east = normalise(vec3(-up.y, up.x, 0));
  const forward = normalise(
    vec3(
      -up.x * Math.cos(tiltRad) + east.x * Math.sin(tiltRad),
      -up.y * Math.cos(tiltRad) + east.y * Math.sin(tiltRad),
      -up.z * Math.cos(tiltRad) + east.z * Math.sin(tiltRad),
    ),
  );
  return lookAlong(forward, up);
}

/** A still more than one tile wide on the low setting: 2 × 2 tiles of 1920 × 1080. */
const STILL: ViewSize = { widthPx: 3840, heightPx: 2160 };
const STILL_TILES = planTiles(STILL, 8192, "low").tiles;
const STILL_CAMERA_M = above(1_500);
const STILL_ORIENTATION = lookingDown(STILL_CAMERA_M, 1.2);

function stillView(viewport: ViewSize): ViewSelectionInput {
  return {
    camera: { positionM: STILL_CAMERA_M, orientation: STILL_ORIENTATION },
    fovXRad: Math.PI / 3,
    viewport,
    weight: 1,
    tauPx: 1,
  };
}

/** Selection for one view, by its viewport: each is computed once for the file's tests. */
const selectionFor = memoised(
  (viewport: ViewSize): Selection =>
    selectPatches({ planet: PLANET, views: [stillView(viewport)], setting: "high", grounded: [] }),
  (viewport) => `${viewport.widthPx}×${viewport.heightPx}`,
);

/**
 * The still's one selection, with the footprint view, here made from a view that came with a
 * tile-sized viewport.
 */
const footprintSelection = memoised(
  (tileSize: ViewSize): Selection =>
    selectPatches({
      planet: PLANET,
      views: [footprintView(stillView(tileSize), STILL)],
      setting: "high",
      grounded: [],
    }),
  (tileSize) => `${tileSize.widthPx}×${tileSize.heightPx}`,
);

/**
 * The patches tile `tile` draws, by `patchKeyString`: the still's one selection, culled by the
 * tile's frustum.
 */
function tilePatches(tile: StillTile): ReadonlyMap<string, SelectedPatch> {
  const frustum = tileFrustum(
    { orientation: STILL_ORIENTATION, fovXRad: Math.PI / 3, viewport: STILL },
    tile,
  );
  const kept = new Map<string, SelectedPatch>();
  for (const [key, p] of footprintSelection(tile.texture).patches) {
    if (p.seen && inFrustum(relativeBounds(p.bounds, STILL_CAMERA_M), frustum)) {
      kept.set(key, p);
    }
  }
  return kept;
}

const tilePatchesOf = memoised(tilePatches, (t) => `${t.column},${t.row}`);

/** The selected leaf of `sel` covering `key`'s area, at its level or coarser, or `null`. */
function coveringLeaf(sel: Selection, key: PatchKey): PatchKey | null {
  let k: PatchKey | null = key;
  while (k !== null) {
    if (sel.patches.has(patchKeyString(k))) {
      return k;
    }
    k = parentKey(k);
  }
  return null;
}

/** Points of a patch's surface: every eighth vertex, at its lowest and highest heights. */
function surfaceSamples(p: SelectedPatch): Vec3[] {
  const points: Vec3[] = [];
  for (let x = 0; x <= PATCH_QUADS; x += 8) {
    for (let y = 0; y <= PATCH_QUADS; y += 8) {
      for (const heightM of [p.bounds.minHeightM, p.bounds.maxHeightM]) {
        const [px, py, pz] = surfacePoint(WGS84_FIGURE, vertexDir(p.key, x, y), heightM);
        points.push(vec3(px - STILL_CAMERA_M.x, py - STILL_CAMERA_M.y, pz - STILL_CAMERA_M.z));
      }
    }
  }
  return points;
}

describe("a tile's terrain", () => {
  it("draws each patch at the level the untiled still selects", () => {
    const untiled = selectionFor(STILL);
    const differing = STILL_TILES.flatMap((tile) =>
      [...tilePatchesOf(tile).values()]
        .filter((p) => untiled.patches.get(patchKeyString(p.key))?.key.level !== p.key.level)
        .map((p) => `tile ${tile.column},${tile.row}: ${patchKeyString(p.key)}`),
    );
    expect(differing).toEqual([]);
  });

  it("culls per tile: each tile keeps some of the still's patches and drops others", () => {
    const seen = [...selectionFor(STILL).patches.values()].filter((p) => p.seen).length;
    const kept = STILL_TILES.map((tile) => tilePatchesOf(tile).size);
    expect(kept.filter((n) => n > 10 && n < seen)).toHaveLength(STILL_TILES.length);
  });

  it("drops a patch from a tile only when none of it can be seen there", () => {
    const camera = { orientation: STILL_ORIENTATION, fovXRad: Math.PI / 3 };
    const dropped: string[] = [];
    for (const [key, p] of selectionFor(STILL).patches) {
      if (!p.seen) {
        continue;
      }
      const projected = surfaceSamples(p).map((v) => project(v, camera, STILL));
      for (const tile of STILL_TILES) {
        if (tilePatchesOf(tile).has(key)) {
          continue;
        }
        const seenThere = projected.some((at) => {
          return (
            at.inFront &&
            at.xPx >= tile.rect.x0Px &&
            at.xPx < tile.rect.x1Px &&
            at.yPx >= tile.rect.y0Px &&
            at.yPx < tile.rect.y1Px
          );
        });
        if (seenThere) {
          dropped.push(`tile ${tile.column},${tile.row}: ${key}`);
        }
      }
    }
    expect(dropped).toEqual([]);
  });

  it("would choose coarser levels with a tile-sized viewport (the negative control)", () => {
    const tile = STILL_TILES[0];
    if (tile === undefined) {
      throw new Error("the still has tiles");
    }
    const tileSized = selectionFor(tile.texture);
    const coarser = [...tilePatchesOf(tile).values()].filter((p) => {
      const leaf = coveringLeaf(tileSized, p.key);
      return leaf !== null && leaf.level < p.key.level;
    });
    expect(coarser.length).toBeGreaterThan(0);
  });
});

describe("the dispatch split", () => {
  it.each([
    [1, 1_000],
    [10_000, 1_000],
    [262_144, 7_919],
    [5, 49_000_000],
    [1_000_000, 50_000_000 / 3],
  ])("plans no piece of %i workgroups at %f ns each over 50 ms", (workgroups, costNs) => {
    const split = splitDispatch(workgroups, costNs);
    if (split.kind !== "split") {
      throw new Error(`expected a split, got ${split.kind}`);
    }
    const over = split.pieces.filter((p) => p.workgroups * costNs > MAX_DISPATCH_NS);
    expect(over).toEqual([]);
  });

  it("covers every workgroup once, in order", () => {
    const split = splitDispatch(262_144, 7_919);
    if (split.kind !== "split") {
      throw new Error(`expected a split, got ${split.kind}`);
    }
    let next = 0;
    const gaps = split.pieces.filter((p) => {
      const gap = p.firstWorkgroup !== next;
      next = p.firstWorkgroup + p.workgroups;
      return gap;
    });
    expect({ gaps: gaps.length, end: next }).toEqual({ gaps: 0, end: 262_144 });
  });

  it("splits into as few pieces as the bound allows", () => {
    // 50 ms holds 6,313 workgroups of 7,919 ns: 262,144 of them need 42 pieces.
    const split = splitDispatch(262_144, 7_919);
    expect(split.kind === "split" ? split.pieces.length : split.kind).toBe(42);
  });

  it("predicts its largest piece, the larger pieces first", () => {
    const split = splitDispatch(262_144, 7_919);
    if (split.kind !== "split") {
      throw new Error(`expected a split, got ${split.kind}`);
    }
    const sizes = split.pieces.map((p) => p.workgroups);
    expect({
      ordered: sizes.every((n, k) => k === 0 || n <= (sizes[k - 1] ?? 0)),
      largestNs: split.largestPredictedNs,
    }).toEqual({ ordered: true, largestNs: Math.max(...sizes) * 7_919 });
  });

  it("flags a workgroup that alone exceeds 50 ms", () => {
    expect(splitDispatch(100, 60_000_000)).toEqual({ kind: "over-bound", workgroupNs: 60_000_000 });
  });

  it("probes with one workgroup before any cost is known", () => {
    expect(splitDispatch(100, null)).toEqual({
      kind: "probe",
      piece: { firstWorkgroup: 0, workgroups: PROBE_WORKGROUPS },
    });
  });

  it.each([
    ["no workgroups", 0, 1_000],
    ["fractional workgroups", 2.5, 1_000],
    ["a zero cost", 10, 0],
    ["a negative cost", 10, -1],
    ["an infinite cost", 10, Number.POSITIVE_INFINITY],
  ])("refuses %s", (_, workgroups, costNs) => {
    expect(() => splitDispatch(workgroups, costNs)).toThrow(RangeError);
  });
});
