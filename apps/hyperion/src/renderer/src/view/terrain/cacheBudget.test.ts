/**
 * The terrain cache's sizing arithmetic (plan R10, R10.T14.a): `bytesPerPatch` against the slot
 * formats as built, the count model's own table, Design note 15's table held to a band, the model
 * against `selectPatches`, its inputs, the sizing rule, and the caches' allocations against the
 * storage-binding ceiling.
 */

import { describe, expect, it } from "vitest";

import { TEST_PLANET_FIGURE } from "../spike/testPlanetFigure";
import {
  TERRAIN_SETTINGS,
  type TerrainSettings,
  WIREFRAME_TERRAIN_SLOTS,
  WIREFRAME_TERRAIN_TAU_PX,
} from "../quality/qualitySetting";
import {
  ALL_ROUND_FACTOR,
  allocationSumBytes,
  bindingCeilingSlots,
  bytesPerPatch,
  cacheAllocationBytes,
  type CountModelCamera,
  type CountModelCounts,
  type CountModelPremise,
  FRUSTUM_FACTOR,
  frustumRuleSlots,
  type PatchCountModel,
  patchCountModel,
  REFERENCE_PX_PER_RAD,
  sizingRuleSlots,
  slotBudgetBytes,
  WEBGPU_DEFAULT_STORAGE_BINDING_BYTES,
  WGS84_EQUATORIAL_RADIUS_M,
} from "./cacheBudget";
import { PATCH_QUADS, vertexSpacing } from "./cube";
import { terrainLayout } from "./gpu/resources";
import { MAX_LEVEL } from "./patchKey";
import { planetGeometry } from "./planet";
import { selectPatches } from "./select";
import { selectionTolerancePx } from "./selectionTolerance";
import {
  CLASS_LAYERS_BYTES,
  DOUBLE_NORMALS_BYTES,
  HEIGHTS_BYTES,
  HORIZON_MAP_BYTES,
  MESH_NORMALS_BYTES,
  OFFSETS_BYTES,
  type SlotField,
  SURVEY_MASK_BYTES,
  type TerrainShading,
  terrainSlotFields,
  terrainSlotLayout,
} from "./slotLayout";
import { MAX_FRUSTUM_MARGIN_RAD } from "./terrainPass";
import { wireframeTerrainSettings } from "./terrainWire";

const MIB = 1024 * 1024;
const HIGH = TERRAIN_SETTINGS.high;
const LOW = TERRAIN_SETTINGS.low;
/** The high setting on its fallback path, `face-differences`, as `terrainLayout` falls back. */
const HIGH_FALLBACK: TerrainSettings = { ...HIGH, vertexPath: "face-differences" };

describe("bytesPerPatch", () => {
  const ROWS: ReadonlyArray<{
    readonly name: string;
    readonly terrain: TerrainSettings;
    readonly shading: TerrainShading;
    readonly bytes: number;
  }> = [
    { name: "lit low", terrain: LOW, shading: "lit", bytes: 158_936 },
    { name: "lit high", terrain: HIGH, shading: "lit", bytes: 311_024 },
    { name: "lit high's fallback", terrain: HIGH_FALLBACK, shading: "lit", bytes: 209_624 },
    {
      name: "wireframe on face-differences",
      terrain: wireframeTerrainSettings("low"),
      shading: "wire",
      bytes: 38_028,
    },
    {
      name: "wireframe on baked-offsets",
      terrain: wireframeTerrainSettings("high"),
      shading: "wire",
      bytes: 139_428,
    },
    { name: "R05's spike low", terrain: LOW, shading: "spike", bytes: 51_756 },
    { name: "R05's spike high", terrain: HIGH, shading: "spike", bytes: 203_844 },
    {
      name: "R05's spike high's fallback",
      terrain: HIGH_FALLBACK,
      shading: "spike",
      bytes: 102_444,
    },
  ];

  it.each(ROWS)("gives the $name slot its bytes as built, to the byte", (row) => {
    const bytes = bytesPerPatch({ terrain: row.terrain, shading: row.shading });
    expect([bytes, terrainSlotLayout(row.terrain, row.shading).bytesPerSlot]).toEqual([
      row.bytes,
      row.bytes,
    ]);
  });

  it("gives the lit low slot on baked-offsets 260,336 B, 257 slots in 64 MiB, not 430", () => {
    // Built directly: `terrainSettingsFor` refuses the pair (Design note 15, "not with BakedOffsets").
    const terrain: TerrainSettings = { ...LOW, vertexPath: "baked-offsets" };
    expect([
      terrain.cacheBytes,
      bytesPerPatch({ terrain, shading: "lit" }),
      terrainSlotLayout(terrain, "lit").slotCount,
    ]).toEqual([64 * MIB, 260_336, 257]);
  });

  it("sizes no slot from a budget, so a budget under six slots does not throw", () => {
    const terrain: TerrainSettings = { ...HIGH, cacheBytes: 1 };
    expect(() => terrainSlotLayout(terrain, "lit")).toThrow("fewer than 6");
    expect(bytesPerPatch({ terrain, shading: "lit" })).toBe(311_024);
  });

  /** Design note 15's bytes for each field, and the departures as built (T14.a's record). */
  const FIELDS: ReadonlyArray<{
    readonly field: string;
    readonly note: number;
    readonly built: number;
    readonly departure: number;
  }> = [
    { field: "heights", note: 33_800, built: HEIGHTS_BYTES, departure: 0 },
    { field: "offsets", note: 101_400, built: OFFSETS_BYTES, departure: 0 },
    // The atlas's one-texel gutter (R05.T8).
    { field: "mesh normals", note: 16_900, built: MESH_NORMALS_BYTES, departure: 1_056 },
    { field: "double normals", note: 66_564, built: DOUBLE_NORMALS_BYTES, departure: 2_080 },
    // One storage buffer with the class laws, interpolated by hand (R10.T10.b).
    { field: "class weights", note: 33_800, built: CLASS_LAYERS_BYTES, departure: 256 },
    // The 9 × 9 far field (R10.T8.a, T8.b).
    { field: "horizon map", note: 67_600, built: HORIZON_MAP_BYTES, departure: 1_296 },
    // Word alignment and the cell-code byte (R10.T10.c).
    { field: "survey mask", note: 4_225, built: SURVEY_MASK_BYTES, departure: 3 },
  ];

  it.each(FIELDS)("departs from Design note 15's $field by $departure B", (row) => {
    expect(row.built - row.note).toBe(row.departure);
  });

  it("totals a low lit slot 1.7% over Design note 15's, and a high one with offsets 1.2% over", () => {
    const note = (names: ReadonlyArray<string>): number =>
      FIELDS.filter((f) => names.includes(f.field)).reduce((sum, f) => sum + f.note, 0);
    const common = ["heights", "class weights", "horizon map", "survey mask"];
    const lowNote = note([...common, "mesh normals"]);
    const highNote = note([...common, "double normals"]);
    expect([lowNote, highNote, highNote + 101_400]).toEqual([156_325, 205_989, 307_389]);
    expect((158_936 / lowNote - 1) * 100).toBeCloseTo(1.67, 2);
    expect((311_024 / (highNote + 101_400) - 1) * 100).toBeCloseTo(1.18, 2);
  });
});

/** One of Design note 15's columns: a setting's tolerance at its presented size, 60° across. */
interface Column {
  readonly name: "high" | "low" | "wire";
  readonly tauPx: number;
  readonly viewport: { readonly widthPx: number; readonly heightPx: number };
}

const FOV_X_RAD = Math.PI / 3;
const HD = { widthPx: 1920, heightPx: 1080 } as const;
const COLUMNS: ReadonlyArray<Column> = [
  { name: "high", tauPx: HIGH.tauPx, viewport: HD },
  { name: "low", tauPx: LOW.tauPx, viewport: { widthPx: 1280, heightPx: 720 } },
  { name: "wire", tauPx: WIREFRAME_TERRAIN_TAU_PX, viewport: HD },
];

/** Design note 15's camera heights, metres. */
const HEIGHTS_M = [100, 1_000, 10_000, 100_000, 400_000] as const;

/** A cell's counts: all-round, then the frustum's least and greatest peak. */
type Cell = readonly [allRound: number, least: number, greatest: number];

/** Design note 15's table, by column, at {@link HEIGHTS_M}. */
const NOTE: Readonly<Record<Column["name"], ReadonlyArray<Cell>>> = {
  high: [
    [1_468, 246, 268],
    [1_216, 200, 210],
    [936, 144, 164],
    [604, 92, 116],
    [424, 78, 92],
  ],
  low: [
    [332, 80, 88],
    [260, 64, 68],
    [208, 50, 54],
    [152, 38, 42],
    [116, 30, 32],
  ],
  wire: [
    [284, 76, 76],
    [212, 52, 56],
    [148, 38, 42],
    [104, 26, 30],
    [68, 18, 20],
  ],
};

/**
 * The model's own table at τ, k₀ = 5, no margin or relief: on the boxes' horizon (selection's).
 * Re-pinned for R10.T14.e's normal margin (2026-10-20): 100 m high from 3,014 / 537–554 to 2,397 /
 * 423–432, the rest as plan 10's T14.e record has it. Re-pinned for R09.T8.d (2026-10-20), whose
 * boxes take the subtree's range and its skirts: the wire column's greatest frustum peak at 400 km
 * from 32 to 33, no other figure.
 */
const MODEL_BOX: Readonly<Record<Column["name"], ReadonlyArray<Cell>>> = {
  high: [
    [2_397, 423, 432],
    [1_981, 327, 346],
    [1_497, 259, 273],
    [989, 177, 189],
    [692, 147, 151],
  ],
  low: [
    [428, 90, 93],
    [348, 76, 81],
    [287, 67, 69],
    [193, 46, 47],
    [156, 39, 43],
  ],
  wire: [
    [313, 80, 82],
    [252, 58, 63],
    [191, 44, 47],
    [135, 37, 41],
    [101, 31, 33],
  ],
};

/**
 * The same on the exact horizon, Design note 15's premise. Re-pinned for R10.T14.e (2026-10-20):
 * refinement and the frustum still measure the boxes, so it moved a little too (100 m high from
 * 2,360 / 423–432 to 2,348 / 414–424). Re-pinned for R09.T8.d (2026-10-20), as the boxes' table: the
 * wire column's greatest frustum peak at 400 km from 24 to 25, no other figure.
 */
const MODEL_EXACT: Readonly<Record<Column["name"], ReadonlyArray<Cell>>> = {
  high: [
    [2_348, 414, 424],
    [1_943, 321, 338],
    [1_424, 247, 260],
    [941, 167, 180],
    [642, 137, 140],
  ],
  low: [
    [400, 85, 88],
    [317, 71, 75],
    [259, 58, 60],
    [166, 39, 41],
    [126, 32, 33],
  ],
  wire: [
    [284, 71, 74],
    [218, 52, 55],
    [163, 34, 38],
    [109, 28, 34],
    [75, 24, 25],
  ],
};

function modelAt(column: Column, heightM: number, tauPx = column.tauPx): PatchCountModel {
  return patchCountModel({ heightM, fovXRad: FOV_X_RAD }, tauPx, column.viewport);
}

function cellOf(counts: CountModelCounts): Cell {
  return [counts.allRound, counts.frustum[0], counts.frustum[1]];
}

/** The model at every column and height, at τ: computed once, the table's tests read it. */
const TABLE: ReadonlyArray<{
  readonly column: Column;
  readonly models: ReadonlyArray<PatchCountModel>;
}> = COLUMNS.map((column) => ({ column, models: HEIGHTS_M.map((h) => modelAt(column, h)) }));

/** A cell's figure that missed its expected value. */
interface Miss {
  readonly row: number;
  readonly figure: number;
  readonly actual: number;
  readonly expected: number;
}

/** The figures of `actual` that are not within `tolerance` of `expected`'s, relatively. */
function missesOf(
  actual: ReadonlyArray<Cell>,
  expected: ReadonlyArray<Cell>,
  tolerance: number,
): Miss[] {
  return actual.flatMap((cell, row) =>
    cell.flatMap((x, figure) => {
      const want = expected[row]?.[figure] ?? NaN;
      return Math.abs(x / want - 1) <= tolerance
        ? []
        : [{ row, figure, actual: x, expected: want }];
    }),
  );
}

describe("the count model's own table (k₀ = 5, no margin, no relief, at τ)", () => {
  it.each(TABLE)(
    "holds the $column.name column on the boxes' horizon to 1%",
    ({ column, models }) => {
      const cells = models.map((m) => cellOf(m));
      expect(missesOf(cells, MODEL_BOX[column.name], 0.01)).toEqual([]);
    },
  );

  it.each(TABLE)(
    "holds the $column.name column on the exact horizon to 1%",
    ({ column, models }) => {
      const cells = models.map((m) => cellOf(m.exactHorizon));
      expect(missesOf(cells, MODEL_EXACT[column.name], 0.01)).toEqual([]);
    },
  );

  it("counts no fewer on the boxes' horizon than on the exact one, cell by cell", () => {
    const fewer = TABLE.flatMap(({ models }) =>
      models.filter(
        (m) =>
          m.allRound < m.exactHorizon.allRound ||
          m.frustum[0] < m.exactHorizon.frustum[0] ||
          m.frustum[1] < m.exactHorizon.frustum[1],
      ),
    );
    expect(fewer).toEqual([]);
  });

  it("counts the low setting at k₀ = 15 as the high one at 5, the same bound in pixels", () => {
    // 15 × (1,280 ÷ 1,920) ÷ 2 px = 5 ÷ 1 px, and 16:9 both.
    const [high, low] = COLUMNS;
    if (high === undefined || low === undefined) {
      throw new Error("the columns hold the high and the low setting");
    }
    const lowAt15 = patchCountModel(
      { heightM: 1_000, fovXRad: FOV_X_RAD },
      low.tauPx,
      low.viewport,
      {
        edgesAtOnePx: 15,
      },
    );
    expect(cellOf(lowAt15)).toEqual(cellOf(modelAt(high, 1_000)));
  });

  it("takes the test planet's a, WGS 84's, as the premise's radius by default", () => {
    const [, , wire] = COLUMNS;
    if (wire === undefined) {
      throw new Error("the columns hold the wireframe");
    }
    const camera = { heightM: 400_000, fovXRad: FOV_X_RAD };
    const given = patchCountModel(camera, wire.tauPx, wire.viewport, {
      radiusM: TEST_PLANET_FIGURE.equatorialRadiusM,
    });
    expect(patchCountModel(camera, wire.tauPx, wire.viewport)).toEqual(given);
  });

  it("measures the reference view's pixels a radian as 1,662.77 px rad⁻¹", () => {
    expect(REFERENCE_PX_PER_RAD).toBeCloseTo(1_662.77, 2);
  });
});

describe("Design note 15's table against the model (decision-r10-t14a, R7)", () => {
  // Ruled (decision-r10-t14a, R7, and its #56 follow-up): each cell within [0.5, 2] of the model
  // on the exact horizon, the note's premise, with every ratio recorded in plan 10. The boxes'
  // ratios are recorded beside them, not asserted: R05's bounds' margin along the normal let coarse
  // patches wholly below the true horizon count there, and six of the high column's cells passed 2.
  // Since R10.T14.e's normal margin none does (the boxes' high column ×1.60–1.92).
  it.each(TABLE)(
    "holds every $column.name cell within [0.5, 2] of the model on the exact horizon",
    ({ column, models }) => {
      const ratios = models.flatMap((m, n) =>
        cellOf(m.exactHorizon).map((x, k) => x / (NOTE[column.name][n]?.[k] ?? NaN)),
      );
      expect(ratios.filter((r) => !(r >= 0.5 && r <= 2))).toEqual([]);
    },
  );
});

/**
 * The count model's premise planet, for `selectPatches`: `terrainWireSelection.test.ts`'
 * `countModelTable()`, copied, on the model's sphere of WGS 84's a.
 */
function countModelTable(): Float64Array {
  const table = new Float64Array(4 * (MAX_LEVEL + 1));
  for (let level = 0; level <= MAX_LEVEL; level += 1) {
    const spacing = vertexSpacing(WGS84_EQUATORIAL_RADIUS_M, level);
    table[4 * level] = (5 * PATCH_QUADS * spacing.meanM) / REFERENCE_PX_PER_RAD;
    table[4 * level + 3] = spacing.maxM;
  }
  return table;
}

const PREMISE_PLANET = planetGeometry(
  {
    equatorialRadiusM: WGS84_EQUATORIAL_RADIUS_M,
    polarRadiusM: WGS84_EQUATORIAL_RADIUS_M,
    pole: null,
  },
  countModelTable(),
);

describe("the count model against selectPatches on its premise planet", () => {
  const CASES = COLUMNS.flatMap((column) =>
    [1_000, 10_000, 100_000].map((heightM) => ({ column, heightM })),
  );

  it.each(CASES)(
    "sees within [0.8, 2] of the model's frustum count at each heading's peak, $column.name at $heightM m",
    ({ column, heightM }) => {
      const model = modelAt(column, heightM);
      const ratios = model.peaks.map((peak) => {
        const selection = selectPatches({
          planet: PREMISE_PLANET,
          views: [
            {
              camera: { positionM: model.positionM, orientation: peak.orientation },
              fovXRad: FOV_X_RAD,
              viewport: column.viewport,
              weight: 1,
              tauPx: column.tauPx,
            },
          ],
          setting: "high",
          grounded: [],
        });
        const seen = [...selection.patches.values()].filter((p) => p.seen).length;
        return seen / peak.count;
      });
      expect(ratios.filter((r) => !(r >= 0.8 && r <= 2))).toEqual([]);
    },
  );
});

/** Every frustum figure of `models` at least the one before's, on both horizons. */
function neverFalls(models: ReadonlyArray<PatchCountModel>): boolean {
  return models.every((m, n) => {
    const before = models[n - 1];
    return (
      before === undefined ||
      (m.frustum[0] >= before.frustum[0] &&
        m.frustum[1] >= before.frustum[1] &&
        m.exactHorizon.frustum[0] >= before.exactHorizon.frustum[0] &&
        m.exactHorizon.frustum[1] >= before.exactHorizon.frustum[1])
    );
  });
}

describe("the count model's inputs (decision-r10-t14a, R3)", () => {
  it.each(COLUMNS)(
    "never counts fewer in the $name frustum as the margin opens to the pass's cap",
    (column) => {
      const margins = [0, 0.25, 0.5, 0.75, 1].map((f) => f * MAX_FRUSTUM_MARGIN_RAD);
      const models = margins.map((frustumMarginRad) =>
        patchCountModel(
          { heightM: 1_000, fovXRad: FOV_X_RAD, frustumMarginRad },
          column.tauPx,
          column.viewport,
        ),
      );
      expect(neverFalls(models)).toBe(true);
    },
  );

  it("keeps the all-round count whatever the margin, which opens the frustum alone", () => {
    const [, , wire] = COLUMNS;
    if (wire === undefined) {
      throw new Error("the columns hold the wireframe");
    }
    const allRound = [0, MAX_FRUSTUM_MARGIN_RAD].map(
      (frustumMarginRad) =>
        patchCountModel(
          { heightM: 1_000, fovXRad: FOV_X_RAD, frustumMarginRad },
          wire.tauPx,
          wire.viewport,
        ).allRound,
    );
    expect(allRound[1]).toBe(allRound[0]);
  });

  it.each(COLUMNS)(
    "never counts fewer in the $name frustum as the relief grows to R05's ±24.5 km",
    (column) => {
      // R05's golden level table reaches ±24,477 m at its finest levels (`level_table.golden`).
      const reliefs = [0, 0.25, 0.5, 0.75, 1].map((f) => f * 24_500);
      const models = reliefs.map((reliefM) =>
        patchCountModel({ heightM: 1_000, fovXRad: FOV_X_RAD }, column.tauPx, column.viewport, {
          reliefM,
        }),
      );
      expect(neverFalls(models)).toBe(true);
    },
  );

  it("keeps the wireframe's share of the high setting's count above τ⁻² at k₀ = 5 and 15", () => {
    const [high, , wire] = COLUMNS;
    if (high === undefined || wire === undefined) {
      throw new Error("the columns hold the high setting and the wireframe");
    }
    const shares = [5, 15].flatMap((edgesAtOnePx) => {
      const count = (column: Column): PatchCountModel =>
        patchCountModel({ heightM: 1_000, fovXRad: FOV_X_RAD }, column.tauPx, column.viewport, {
          edgesAtOnePx,
        });
      const [h, w] = [count(high), count(wire)];
      return [w.allRound / h.allRound, w.frustum[1] / h.frustum[1]];
    });
    expect(shares.filter((r) => !(r > 1 / WIREFRAME_TERRAIN_TAU_PX ** 2))).toEqual([]);
  });

  const REFUSED: ReadonlyArray<{
    readonly name: string;
    readonly camera: CountModelCamera;
    readonly tauPx: number;
    readonly viewport: { readonly widthPx: number; readonly heightPx: number };
    readonly premise: CountModelPremise;
  }> = [
    {
      name: "a camera on the datum",
      camera: { heightM: 0, fovXRad: FOV_X_RAD },
      tauPx: 1,
      viewport: HD,
      premise: {},
    },
    {
      name: "a height that is not a number",
      camera: { heightM: NaN, fovXRad: FOV_X_RAD },
      tauPx: 1,
      viewport: HD,
      premise: {},
    },
    {
      name: "no field of view",
      camera: { heightM: 100, fovXRad: 0 },
      tauPx: 1,
      viewport: HD,
      premise: {},
    },
    {
      name: "a field of view of π",
      camera: { heightM: 100, fovXRad: Math.PI },
      tauPx: 1,
      viewport: HD,
      premise: {},
    },
    {
      name: "a margin below 0",
      camera: { heightM: 100, fovXRad: FOV_X_RAD, frustumMarginRad: -0.1 },
      tauPx: 1,
      viewport: HD,
      premise: {},
    },
    {
      name: "no tolerance",
      camera: { heightM: 100, fovXRad: FOV_X_RAD },
      tauPx: 0,
      viewport: HD,
      premise: {},
    },
    {
      name: "a viewport of no width",
      camera: { heightM: 100, fovXRad: FOV_X_RAD },
      tauPx: 1,
      viewport: { widthPx: 0, heightPx: 1_080 },
      premise: {},
    },
    {
      name: "a radius of 0",
      camera: { heightM: 100, fovXRad: FOV_X_RAD },
      tauPx: 1,
      viewport: HD,
      premise: { radiusM: 0 },
    },
    {
      name: "a bound ratio below 0",
      camera: { heightM: 100, fovXRad: FOV_X_RAD },
      tauPx: 1,
      viewport: HD,
      premise: { edgesAtOnePx: -1 },
    },
    {
      name: "a relief below 0",
      camera: { heightM: 100, fovXRad: FOV_X_RAD },
      tauPx: 1,
      viewport: HD,
      premise: { reliefM: -1 },
    },
  ];

  it.each(REFUSED)("refuses $name", ({ camera, tauPx, viewport, premise }) => {
    expect(() => patchCountModel(camera, tauPx, viewport, premise)).toThrow(RangeError);
  });
});

/** The forced region's peak in the replay of R05's descent (`cache.test.ts`; T14.a's record). */
const REPLAY_FORCED_PEAK = 71;

/** The sizing rule's slots for one view of `allRound` and `frustum` peaks, no forced region. */
function oneViewRule(allRound: number, frustum: number): number {
  return sizingRuleSlots([{ allRound, frustum }], 0);
}

describe("the sizing rule (Design note 15)", () => {
  it("gives Design note 15's own figures: about 430 low, 1,900 high and 370 for the wireframe", () => {
    expect(oneViewRule(332, 88)).toBeCloseTo(431.6, 9);
    expect(oneViewRule(1_468, 268)).toBeCloseTo(1_908.4, 9);
    expect(oneViewRule(284, 76)).toBeCloseTo(369.2, 9);
  });

  it("gives Design note 15's frustum rule for high, about 800 slots, 165 MB at its 206 kB", () => {
    const slots = frustumRuleSlots([{ allRound: 1_468, frustum: 268 }], 0);
    expect([slots, slotBudgetBytes(slots, 205_989)]).toEqual([804, 165_615_156]);
  });

  it("sums the rule over the streaming views, then adds the forced region once", () => {
    const view = { allRound: 100, frustum: 50 };
    expect(sizingRuleSlots([view, view], 7)).toBe(2 * Math.max(ALL_ROUND_FACTOR * 100, 150) + 7);
    expect(frustumRuleSlots([view, view], 7)).toBe(2 * FRUSTUM_FACTOR * 50 + 7);
  });

  /**
   * The model at τ_sel = τ ÷ 1.1 and 100 m, each column's peak over Design note 15's heights, on
   * the boxes' horizon and then the exact one: the sizing rule's inputs, whose slots and bytes plan
   * 10 records beside the held constants (not asserted against them). Re-pinned for
   * R10.T14.e's normal margin (2026-10-20): high's boxes from 3,498 / 637–646.
   */
  const AT_SELECTION_TAU: Readonly<Record<Column["name"], readonly [box: Cell, exact: Cell]>> = {
    high: [
      [2_790, 500, 506],
      [2_741, 492, 497],
    ],
    low: [
      [490, 92, 105],
      [462, 87, 100],
    ],
    wire: [
      [352, 88, 91],
      [319, 80, 82],
    ],
  };

  it.each(COLUMNS)("holds the $name column's peaks at τ_sel to 1%, the rule's inputs", (column) => {
    const model = modelAt(column, 100, selectionTolerancePx(column.tauPx));
    const [box, exact] = AT_SELECTION_TAU[column.name];
    expect(missesOf([cellOf(model), cellOf(model.exactHorizon)], [box, exact], 0.01)).toEqual([]);
  });

  it("rounds a budget up to whole slots before it takes their bytes", () => {
    // T14.a's high peak at τ_sel, before R10.T14.e: kept as a case whose rule, 4,618.4 slots, needs
    // the rounding.
    const views = [{ allRound: 3_498, frustum: 646 }];
    const slots = sizingRuleSlots(views, REPLAY_FORCED_PEAK);
    expect([slots, slotBudgetBytes(slots, 311_024)]).toEqual([
      ALL_ROUND_FACTOR * 3_498 + REPLAY_FORCED_PEAK,
      4_619 * 311_024,
    ]);
  });
});

describe("the caches' allocations and the storage-binding ceiling (decision-r10-t14a, R5, R6)", () => {
  const LIT_LOW = terrainSlotLayout(LOW, "lit");
  const LIT_HIGH = terrainSlotLayout(HIGH, "lit");
  const WIRE_LOW = terrainSlotLayout(wireframeTerrainSettings("low"), "wire");
  const WIRE_HIGH = terrainSlotLayout(wireframeTerrainSettings("high"), "wire");

  it("holds 422 lit slots on low, 1,286 on high and 520 for the wireframe on both", () => {
    expect([LIT_LOW, LIT_HIGH, WIRE_LOW, WIRE_HIGH].map((l) => l.slotCount)).toEqual([
      422,
      1_286,
      WIREFRAME_TERRAIN_SLOTS,
      WIREFRAME_TERRAIN_SLOTS,
    ]);
  });

  it("allocates each case's caches whole, under 1 GB on low and 2 GB on high", () => {
    const cases = [
      { low: [LIT_LOW], high: [LIT_HIGH] },
      { low: [LIT_LOW, LIT_LOW], high: [LIT_HIGH, LIT_HIGH] },
      { low: [LIT_LOW, WIRE_LOW], high: [LIT_HIGH, WIRE_HIGH] },
      {
        low: [LIT_LOW, WIRE_LOW, LIT_LOW, WIRE_LOW],
        high: [LIT_HIGH, WIRE_HIGH, LIT_HIGH, WIRE_HIGH],
      },
    ];
    const sums = cases.map((c) => [allocationSumBytes(c.low), allocationSumBytes(c.high)]);
    expect(sums).toEqual([
      [67_070_992, 399_976_864],
      [134_141_984, 799_953_728],
      [86_845_552, 472_479_424],
      [173_691_104, 944_958_848],
    ]);
    expect(sums.filter(([low = NaN, high = NaN]) => !(low < 1e9 && high < 2e9))).toEqual([]);
  });

  it("allocates the wireframe's own cache 19,774,560 B on face-differences, 72,502,560 B on baked-offsets", () => {
    expect([cacheAllocationBytes(WIRE_LOW), cacheAllocationBytes(WIRE_HIGH)]).toEqual([
      19_774_560, 72_502_560,
    ]);
  });

  it("holds at most 1,323 high lit slots on baked-offsets in a default binding, 37 over 1,286", () => {
    const slots = bindingCeilingSlots(terrainSlotFields(HIGH, "lit"));
    expect([
      slots,
      slots * bytesPerPatch({ terrain: HIGH, shading: "lit" }),
      slots - 1_286,
    ]).toEqual([1_323, 411_484_752, 37]);
    // Its offsets bind first.
    expect(slots * OFFSETS_BYTES).toBeLessThanOrEqual(WEBGPU_DEFAULT_STORAGE_BINDING_BYTES);
    expect((slots + 1) * OFFSETS_BYTES).toBeGreaterThan(WEBGPU_DEFAULT_STORAGE_BINDING_BYTES);
  });

  it("holds at most 1,948 lit slots on the fallback, its horizon map binding first", () => {
    expect(bindingCeilingSlots(terrainSlotFields(HIGH_FALLBACK, "lit"))).toBe(
      Math.floor(WEBGPU_DEFAULT_STORAGE_BINDING_BYTES / HORIZON_MAP_BYTES),
    );
    expect(bindingCeilingSlots(terrainSlotFields(HIGH_FALLBACK, "lit"))).toBe(1_948);
  });

  it("holds 10,589 high lit slots on baked-offsets in an adapter's 1 GiB binding", () => {
    // ⌊2³⁰ ÷ 101,400⌋, the offsets again binding first (`requiredLimits`' most).
    expect(bindingCeilingSlots(terrainSlotFields(HIGH, "lit"), 2 ** 30)).toBe(10_589);
  });

  it("puts no ceiling on a layout whose fields are all textures", () => {
    const fields: ReadonlyArray<SlotField> = [
      { name: "normals", storage: "texture", bytes: DOUBLE_NORMALS_BYTES },
    ];
    expect(bindingCeilingSlots(fields)).toBe(Number.POSITIVE_INFINITY);
  });

  it("lays every setting's lit and wireframe terrain out on its own path at WebGPU's default limits", () => {
    // WebGPU's default limits (W3C WebGPU §3.6.2).
    const limits = {
      maxStorageBufferBindingSize: WEBGPU_DEFAULT_STORAGE_BINDING_BYTES,
      maxBufferSize: 268_435_456,
      maxTextureDimension2D: 8_192,
    };
    const layouts = [
      terrainLayout(LOW, limits, "lit"),
      terrainLayout(HIGH, limits, "lit"),
      terrainLayout(wireframeTerrainSettings("low"), limits, "wire"),
      terrainLayout(wireframeTerrainSettings("high"), limits, "wire"),
    ];
    expect(layouts.map((l) => [l.fallback, l.slots.slotCount])).toEqual([
      ["none", 422],
      ["none", 1_286],
      ["none", 520],
      ["none", 520],
    ]);
  });
});
