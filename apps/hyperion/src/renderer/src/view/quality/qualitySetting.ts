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
import { HIGH_SKY, LOW_SKY, type SkySettings } from "../sky/setting";
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
  /** The sky's cube, sprites, census size and re-bake cadence (R06.T13.f, Design note 22). */
  readonly sky: SkySettings;
  /**
   * The least and greatest internal scale at which the resolution controller holds the
   * photorealistic view, as a fraction of its render resolution on each axis: [0.5, 1.0] on both
   * settings (R07 Design note 14). R12.T11 may move them.
   */
  readonly internalScaleBounds: readonly [min: number, max: number];
  /** The several views' budget: the photorealistic rate and how many such views (R07.T18). */
  readonly budget: ViewBudgetSettings;
}

/** What the per-view budget reads of a setting (R07 Design notes 14 and 18). */
export interface ViewBudgetSettings {
  /**
   * The rate a photorealistic primary view is budgeted at: 60 Hz for the 1080p60 target, 30 Hz for
   * the low setting's 30 fps at 720p, paced to every second vsync (R05 Design note 21).
   */
  readonly photorealisticRateHz: 60 | 30;
  /**
   * The most views drawn in the photorealistic style at once, or `null` for no limit: one on the
   * low setting (R07 Design note 18), the only limit the refusal's wording allows.
   */
  readonly photorealisticViews: 1 | null;
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
 * body every setting draws the finest level whatever these say (R05 Design note 9). Both settings
 * hold the photorealistic view's internal scale in [0.5, 1.0]; the low setting budgets a
 * photorealistic primary view at 30 Hz and allows one photorealistic view (R07 Design notes 14 and
 * 18).
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
    sky: HIGH_SKY,
    internalScaleBounds: [0.5, 1],
    budget: { photorealisticRateHz: 60, photorealisticViews: null },
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
    sky: LOW_SKY,
    internalScaleBounds: [0.5, 1],
    budget: { photorealisticRateHz: 30, photorealisticViews: 1 },
  },
};

/** The terrain's settings per {@link QualitySetting}: `SETTINGS[s].terrain`, for selection. */
export const TERRAIN_SETTINGS: Readonly<Record<QualitySetting, TerrainSettings>> = {
  high: SETTINGS.high.terrain,
  low: SETTINGS.low.terrain,
};

/**
 * A terrain variant for the spike's runs (T13.c's `--vertex-path` and `--normals`, T17's): the
 * fields given replace the setting's own; nothing else is overridable.
 */
export interface TerrainVariant {
  readonly vertexPath?: TerrainVertexPath;
  readonly normals?: TerrainNormals;
}

/**
 * `SETTINGS[setting].terrain` with the variant's fields replaced; an absent field keeps the
 * setting's (the spike variant flags ruling, decision-r05-spike-ux.md).
 *
 * @throws RangeError for `low` with `baked-offsets`, which does not fit the low cache (Design note 4).
 */
export function terrainSettingsFor(
  setting: QualitySetting,
  variant: TerrainVariant = {},
): TerrainSettings {
  const base = SETTINGS[setting].terrain;
  const vertexPath = variant.vertexPath ?? base.vertexPath;
  if (setting === "low" && vertexPath === "baked-offsets") {
    throw new RangeError("the low setting's cache does not fit the baked-offsets vertex path");
  }
  if (variant.vertexPath === undefined && variant.normals === undefined) {
    return base;
  }
  return { ...base, vertexPath, normals: variant.normals ?? base.normals };
}
