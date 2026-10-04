/**
 * What R08 and R10 read of a lit body: its figure, photometry, regime and labels (plan R07, T5;
 * Design notes 5, 19 and 24).
 */
import type { BodyIdHex } from "@hyperion/protocol";

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
 * device loss.
 */
export interface ClassMapDiscSurface {
  readonly kind: "class-map";
  /** R10's coarse class weights, surveyed cells only (`CLASS_MAP_FORMAT`, a 2D array). */
  readonly weights: TextureHandle;
  /** One law per class, in the map's channel order. */
  readonly laws: ReadonlyArray<PhotometricLaw>;
  /** The law of unsurveyed texels: the uniform `lawFor(p, q, template)`. */
  readonly elsewhere: PhotometricLaw;
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
