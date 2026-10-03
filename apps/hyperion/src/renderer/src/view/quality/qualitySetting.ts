/**
 * The quality settings of the view, gathered in one place (plan R05, Design note 26).
 *
 * @remarks
 * There are two settings, `high` (the recommended specification) and `low` (the UHD 620 class), and
 * one list from each to its {@link ViewSettings}. Later plans extend this module only by adding a
 * field to {@link ViewSettings}, with its high and low values in {@link SETTINGS}: R05.T12.c the
 * atmosphere's table sizes, R06 the sky, R07 its styles' passes, R08 the atmosphere's view, R11
 * decoration, R12 the ladder. They never add a setting name or a second list. Each feature builds its
 * low form beside its high form (the budget's third rule), so a field is never added for one setting
 * alone.
 *
 * The names exported here are stable: {@link QualitySetting}, {@link QUALITY_SETTINGS},
 * {@link ViewSettings}, {@link SETTINGS}, {@link TerrainSettings} and {@link TERRAIN_SETTINGS}.
 */

import { TABLE_SIZES, type TableSizes } from "../atmosphere/hillaire";
import type { TerrainNormals, TerrainVertexPath } from "./terrainKinds";

/** The view's quality setting: `high` on the recommended specification, `low` on the UHD 620. */
export type QualitySetting = "high" | "low";

/** Every {@link QualitySetting}, in order from the highest, for tests and settings menus. */
export const QUALITY_SETTINGS: ReadonlyArray<QualitySetting> = ["high", "low"];

/** Where the terrain's normals are evaluated, and how its vertices are formed: in
 * `terrainKinds.ts`, which the height workers read too. */
export type { TerrainNormals, TerrainVertexPath } from "./terrainKinds";

/** The terrain's settings for one {@link QualitySetting} (R05 Design notes 4, 7, 10 and 26). */
export interface TerrainSettings {
  /**
   * The screen-space error tolerance τ in pixels at the presented resolution: the largest a level's
   * error bound may subtend before a finer level is selected (R05 Design note 7).
   */
  readonly tauPx: number;
  /**
   * The height in device pixels at which the view renders before it is presented upscaled, or
   * `null` to render at the canvas's own size.
   */
  readonly renderHeightPx: number | null;
  /** The resolution at which normals are baked and stored. */
  readonly normals: TerrainNormals;
  /** The vertex path, provisional until R05.T18 chooses from the spike's measurements. */
  readonly vertexPath: TerrainVertexPath;
  /**
   * The patch cache's byte budget, from which its fixed slot count follows (R05 Design note 10),
   * provisional until R10.T14.b sizes it.
   */
  readonly cacheBytes: number;
}

/**
 * Everything the view's quality setting decides, one field per feature.
 *
 * @remarks
 * Later plans add fields here and their values to {@link SETTINGS} (see the module's remarks).
 */
export interface ViewSettings {
  /** The terrain's settings (R05). */
  readonly terrain: TerrainSettings;
  /** The atmosphere's table sizes and sample counts (R05.T12.c, Design note 16; R08 widens them). */
  readonly atmosphere: TableSizes;
}

const MIB = 1024 * 1024;

/**
 * The high setting's patch cache budget: about 400 MB, R10 Design note 15's 1.3 × the all-round
 * peak at 100 m (about 1,900 slots, 390 MB), until R10.T14.b measures.
 */
const HIGH_CACHE_BYTES = 400_000_000;

/** The low setting's patch cache budget: 64 MiB, 1,296 slots of R05's low layout. */
const LOW_CACHE_BYTES = 64 * MIB;

/**
 * The one list from each {@link QualitySetting} to its {@link ViewSettings} (R05 Design note 26).
 *
 * @remarks
 * The low setting renders at 720 rows presented upscaled with τ = 2 px, mesh-resolution normals,
 * the `face-differences` vertex path (`baked-offsets` does not fit its 64 MiB cache) and a 64 MiB
 * cache. The high setting renders at the canvas's size with τ = 1 px, doubled normals and
 * `baked-offsets`, its precision holding by construction until R05.T18 chooses. Under a grounded
 * body every setting draws the finest level whatever these say (R05 Design note 9).
 */
export const SETTINGS: Readonly<Record<QualitySetting, ViewSettings>> = {
  high: {
    terrain: {
      tauPx: 1,
      renderHeightPx: null,
      normals: "double",
      vertexPath: "baked-offsets",
      cacheBytes: HIGH_CACHE_BYTES,
    },
    atmosphere: TABLE_SIZES.high,
  },
  low: {
    terrain: {
      tauPx: 2,
      renderHeightPx: 720,
      normals: "mesh",
      vertexPath: "face-differences",
      cacheBytes: LOW_CACHE_BYTES,
    },
    atmosphere: TABLE_SIZES.low,
  },
};

/** The terrain's settings per {@link QualitySetting}: `SETTINGS[s].terrain`, for selection. */
export const TERRAIN_SETTINGS: Readonly<Record<QualitySetting, TerrainSettings>> = {
  high: SETTINGS.high.terrain,
  low: SETTINGS.low.terrain,
};
