/**
 * What R08 and R10 read of a lit body: its figure, photometry, regime and labels (plan R07, T5;
 * Design notes 5, 19 and 24).
 */
import type { BodyIdHex } from "@hyperion/protocol";

import type { NormalMapTexels } from "../bodies/discNormals";
import type { ClassMapTexels } from "../bodies/discSurface";
import type { LitRegime } from "../bodies/regime";
import type { TextureHandle } from "../engine/types";
import type { AppearanceLabel, BodyFigure, BodyPhotometry, WireAppearance } from "./fromWire";
import type { PhotometricLaw } from "./law";

/** A lit body's appearance on one view. */
export interface BodyAppearance {
  readonly body: BodyIdHex;
  readonly figure: BodyFigure;
  readonly photometry: BodyPhotometry;
  readonly regime: LitRegime;
  readonly labels: ReadonlyArray<AppearanceLabel>;
}

/**
 * What the disc shades with (Design note 24): the uniform law until R10 supplies its coarse
 * class-weights map over surveyed cells, with a law per class (R10.T10.d fills the `class-map`
 * case, which R07.T8.b builds; `bodies/discSurface.ts` makes one).
 */
export type DiscSurface = UniformDiscSurface | ClassMapDiscSurface;

/** The disc shaded with one law everywhere. */
export interface UniformDiscSurface {
  readonly kind: "uniform";
  readonly law: PhotometricLaw;
}

/**
 * The disc shaded through R10's class map: each texel's classes by their laws, weighted, and an
 * unsurveyed texel (no weight) by `elsewhere`.
 *
 * @remarks
 * The texture's layout is `bodies/discSurface.ts`' (`classMapSurface` makes it): R05's cube-sphere
 * faces, a texel per cell, each class's weight in one channel. Its owner makes it again after a
 * device loss. The texels it was made from travel with it (R10.T10.f), which the point regime
 * integrates on the CPU, so that a point and the disc it becomes read one map. The field's own
 * shading normals over its surveyed cells may travel with it too (R10.T10.g), in a texture of their
 * own with its CPU copy, so that the disc and the terrain share their cell-scale slopes.
 */
export interface ClassMapDiscSurface {
  readonly kind: "class-map";
  /** R10's coarse class weights, surveyed cells only (`CLASS_MAP_FORMAT`, a 2D array). */
  readonly weights: TextureHandle;
  /** The texture's texels, its CPU copy: the map the point regime integrates (R10.T10.f). */
  readonly texels: ClassMapTexels;
  /** One law per class, in the map's channel order. */
  readonly laws: ReadonlyArray<PhotometricLaw>;
  /** The law of unsurveyed texels: the uniform `lawFor(p, q, template)`. */
  readonly elsewhere: PhotometricLaw;
  /**
   * The field's shading normals over its surveyed cells, at the field's level (R10.T10.g,
   * `bodies/discNormals.ts`), or `null` where the surface has none: the datum's normal then shades
   * everywhere, as before.
   */
  readonly normals: DiscNormalMap | null;
}

/**
 * A disc's normal map as its surface carries it (R10.T10.g): the texture R07's disc samples and the
 * texels it was made from, its CPU copy, which the point regime and the disc's CPU twin read.
 *
 * @remarks
 * The layout is `bodies/discNormals.ts`' (`normalMapTexture` makes one): R05's cube-sphere faces at
 * the field's level, a texel per cell, each the octahedral pair of a body-fixed unit normal, `rg16float`
 * in a 2D array of six layers. Its owner makes it again after a device loss, with the class map.
 */
export interface DiscNormalMap {
  /** The normals (`NORMAL_MAP_FORMAT`, a 2D array of six layers). */
  readonly texture: TextureHandle;
  /** The texture's texels, its CPU copy. */
  readonly texels: NormalMapTexels;
}

/**
 * A body's appearance from what the wire gives it and its regime on the view, or `null` for a body
 * with no figure (no granted radius), which stays R02's mark.
 */
export function bodyAppearance(
  body: BodyIdHex,
  wire: WireAppearance,
  regime: LitRegime,
): BodyAppearance | null {
  if (wire.figure === null) {
    return null;
  }
  return {
    body,
    figure: wire.figure,
    photometry: wire.photometry,
    regime,
    labels: wire.labels,
  };
}

/** The disc surface of an appearance today: its photometry's uniform law. */
export function discSurfaceOf(appearance: BodyAppearance): DiscSurface {
  return { kind: "uniform", law: appearance.photometry.law };
}
