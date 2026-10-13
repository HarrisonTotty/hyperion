/**
 * The atmosphere's view settings (plan R08, "Settings"): the values this plan contributes to R05's
 * one settings list (`view/quality/qualitySetting.ts`, R05 Design note 26), and named projections
 * of them, as R05's `TERRAIN_SETTINGS` is of `SETTINGS[s].terrain`, for R12's ladder audit.
 *
 * @remarks
 * R08.T9.a projects the aerial perspective's scope here. R08.T7 adds the view's sources
 * (`AtmosphereViewSettings`, `ATMOSPHERE_VIEW_SETTINGS`, `SKY_SUN_CAP`, `MAX_SKY_SOURCES`) and
 * `ATMOSPHERE_QUALITY_LIMITS`. A projection is read from the field it projects, never held twice;
 * this module takes `TABLE_SIZES` from `hillaire.ts`, which is `SETTINGS[s].atmosphere`, so that
 * `qualitySetting.ts` may import it without a cycle.
 */

import type { QualitySetting } from "../quality/qualitySetting";
import { TABLE_SIZES, type TableSizes } from "./hillaire";

/** Where aerial perspective applies: R05's `TableSizes.aerialPerspectiveScope`. */
export type AerialPerspectiveScope = TableSizes["aerialPerspectiveScope"];

/**
 * Where aerial perspective applies on each setting (Design note 11; R08.T9.a): R05's
 * `SETTINGS[s].atmosphere.aerialPerspectiveScope`, projected here rather than held twice.
 *
 * @remarks
 * `scene` (high) applies it to everything opaque: the composite lays it over every opaque pass drawn
 * into the scene before it, and `HillaireAtmosphere.aerialPerspective` publishes the volume and the
 * transmittance tables to the passes it does not cover (R11's). `terrain` (low) applies it to the
 * terrain alone, in R05's one deferred composite, and publishes nothing: the budget's "aerial
 * perspective on terrain only". Beyond the volume's 32 km reach, on both, the composite's ray march
 * takes over.
 */
export const AERIAL_PERSPECTIVE_SCOPE: Readonly<Record<QualitySetting, AerialPerspectiveScope>> = {
  high: TABLE_SIZES.high.aerialPerspectiveScope,
  low: TABLE_SIZES.low.aerialPerspectiveScope,
};
