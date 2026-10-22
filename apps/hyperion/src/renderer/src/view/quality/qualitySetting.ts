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
import { ATMOSPHERE_VIEW_SETTINGS, type AtmosphereViewSettings } from "../atmosphere/settings";
import { DISC_ANNULI_HIGH, DISC_ANNULI_LOW } from "../lighting/annuli";
import { PLANETSHINE_SOURCES_HIGH, PLANETSHINE_SOURCES_LOW } from "../lighting/planetshine";
import { BLOOM_LEVELS, type BloomLevels } from "../post/bloom";
import { HIGH_SKY, LOW_SKY, type SkySettings } from "../sky/setting";
import type { TerrainNormals, TerrainVertexPath } from "./terrainKinds";

/** The view's quality setting: `high` on the recommended specification, `low` on the UHD 620. */
export type QualitySetting = "high" | "low";

/** Every {@link QualitySetting}, in order from the highest, for tests and settings menus. */
export const QUALITY_SETTINGS: ReadonlyArray<QualitySetting> = ["high", "low"];

/**
 * Each {@link QualitySetting} as the console reads it: the guide's `QUALITY` values, on `VIEW`'s
 * label block and the `LINK` display's `Graphics` panel (R07.T17).
 */
export const QUALITY_NAMES = {
  high: "HIGH",
  low: "LOW",
} as const satisfies Record<QualitySetting, string>;

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
   * held by R10.T14.b (decision-r10-t14b-constants) on the hard bound, re-sized after R10.T4.b.
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
  /**
   * The atmosphere's view: the sky's sources and the stated quality limits (plan R08, R08.T7;
   * `view/atmosphere/settings.ts`).
   */
  readonly atmosphereView: AtmosphereViewSettings;
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
  /** The photorealistic style's passes: the histogram, bloom and the lit bodies (R07.T17). */
  readonly photoreal: PhotorealSettings;
}

/**
 * What the photorealistic style's passes read of a setting (R07 Design note 18): the low setting's
 * forms, built beside the high ones.
 *
 * @remarks
 * Design note 18's other two items are settings already: the view renders at most
 * {@link TerrainSettings.renderHeightPx} rows (720 on low), presented upscaled by the tone-mapping
 * pass, the render resolution that `ViewBudget.renderScale` is a fraction of; and the one
 * photorealistic view is {@link ViewBudgetSettings.photorealisticViews}.
 */
export interface PhotorealSettings {
  /**
   * The exposure histogram's stride, px: 1 reads every pixel of the scene target, 2 every second
   * pixel of every second row, a quarter-resolution input (R07.T12).
   */
  readonly histogramStride: 1 | 2;
  /**
   * The bloom chain's weighted mip levels (R07.T14): 7 from full resolution, or 5 from quarter
   * resolution.
   */
  readonly bloom: BloomLevels;
  /** The eclipse term's annuli K per star (R07 Design note 6): 4, or 3. */
  readonly discAnnuli: number;
  /** The most lit neighbours that light a body by planetshine (R07 Design note 7): 2, or 1. */
  readonly planetshineSources: number;
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
 * The high setting's patch cache budget, 400,000,000 B, held by R10.T14.b
 * (decision-r10-t14b-constants) on the hard bound, re-sized after R10.T4.b (the rule in its union
 * form asked 8,666 high lit slots, 2,996 low lit and 3,764 wireframe on TERRAIN TEST's moon, about
 * seven times the held 1,286, 422 and 520): R10 Design note 15's research figure, 1.3 × the
 * all-round peak at 100 m (about 1,900 slots, 390 MB at its 206 kB a slot). As built it holds 1,208
 * lit slots of 330,996 B (`cacheBudget.ts`' `bytesPerPatch`, R10.T14.a, whose count model's figures
 * plan 10 records beside it), 1,286 of 311,024 B before R10.T1.c's maturity layer grew each slot's
 * class layers by its codes and nodes (decision-r10-t1c-client-scope §2.2: recorded, not re-ruled).
 */
const HIGH_CACHE_BYTES = 400_000_000;

/**
 * The low setting's patch cache budget, 64 MiB, held by R10.T14.b (decision-r10-t14b-constants) on
 * the hard bound, re-sized after R10.T4.b and the owner's UHD 620 run (the rule in its union form
 * asked 8,666 high lit slots, 2,996 low lit and 3,764 wireframe on TERRAIN TEST's moon, about seven
 * times the held 1,286, 422 and 520): 1,296 slots of R05's spike layout; as built, 375 lit slots of
 * 178,908 B (R10.T14.a), 422 of 158,936 B before R10.T1.c's maturity layer.
 */
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
 * 18). The low setting's photorealistic style takes its histogram at a stride of 2, blooms over 5
 * levels from quarter resolution, eclipses with 3 annuli and lights each body by planetshine from
 * one neighbour, against 1, 7 levels from full resolution, 4 annuli and two neighbours (R07
 * Design note 18, R07.T17).
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
    atmosphereView: ATMOSPHERE_VIEW_SETTINGS.high,
    sky: HIGH_SKY,
    internalScaleBounds: [0.5, 1],
    budget: { photorealisticRateHz: 60, photorealisticViews: null },
    photoreal: {
      histogramStride: 1,
      bloom: BLOOM_LEVELS.high,
      discAnnuli: DISC_ANNULI_HIGH,
      planetshineSources: PLANETSHINE_SOURCES_HIGH,
    },
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
    atmosphereView: ATMOSPHERE_VIEW_SETTINGS.low,
    sky: LOW_SKY,
    internalScaleBounds: [0.5, 1],
    budget: { photorealisticRateHz: 30, photorealisticViews: 1 },
    photoreal: {
      histogramStride: 2,
      bloom: BLOOM_LEVELS.low,
      discAnnuli: DISC_ANNULI_LOW,
      planetshineSources: PLANETSHINE_SOURCES_LOW,
    },
  },
};

/** The terrain's settings per {@link QualitySetting}: `SETTINGS[s].terrain`, for selection. */
export const TERRAIN_SETTINGS: Readonly<Record<QualitySetting, TerrainSettings>> = {
  high: SETTINGS.high.terrain,
  low: SETTINGS.low.terrain,
};

/**
 * The wireframe style's screen-space error tolerance τ, px, on either setting: its depth-only
 * terrain's (plan R10, T11) and the disc's hand-over to it (R10 Design note 13: τ is the style's
 * own, {@link TerrainSettings.tauPx} in the photorealistic style, 1 px high and 2 px low, and
 * 4 px in the wireframe).
 */
export const WIREFRAME_TERRAIN_TAU_PX = 4;

/**
 * The slots of the wireframe's terrain cache on either setting (plan R10, R10.T11), held by
 * R10.T14.b (decision-r10-t14b-constants) on the hard bound, re-sized after R10.T4.b and the
 * owner's UHD 620 run (the rule in its union form asked 8,666 high lit slots, 2,996 low lit and
 * 3,764 wireframe on TERRAIN TEST's moon, about seven times the held 1,286, 422 and 520): Design
 * note 15's sizing rule over its wireframe column (4 px at 1080p), max(1.3 × the all-round peak,
 * 3 × the frustum peak) = max(1.3 × 284, 3 × 76) ≈ 370 at 100 m, and the 20–40% the count model
 * leaves out for relief, the 2:1 rule and the forced region at its upper end, about 520. Sized by
 * its slots, not by the setting's bytes: a wireframe slot holds its heights, offsets on their path
 * and survey mask alone (139,428 B on `baked-offsets`, 42% of a lit slot; 38,028 B on
 * `face-differences`, a fifth), so the high setting's 400 MB would give it 2,868 slots, whose
 * offsets pass a default 128 MiB storage binding.
 */
export const WIREFRAME_TERRAIN_SLOTS = 520;

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
