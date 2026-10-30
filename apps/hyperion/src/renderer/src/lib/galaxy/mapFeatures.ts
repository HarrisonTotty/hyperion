/**
 * Where the galaxy map's feature overlay draws each feature on a view's picture (plan 09,
 * P09.T41.b): a pure projection and label placement, which the overlay draws as it is given.
 */
import type { MapFeatureKindWire, MapView } from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import { featureKindAbbreviation, mapFeatureKindLabel } from "./featureWords";
import type { MapGeometry } from "./mapGeometry";
import type { MapFeatureLayer } from "./model";

/**
 * The diameter of a feature's mark on the map, in `rem`: size class 2's, since every mark there is
 * open, the map having no reference plane to fill it by, and the globulars of the inner halo stand
 * close at the map's scale.
 */
export const MAP_FEATURE_MARK_REM = 0.75;

/** The labels' size, in `rem`, as the map's other words are set (0.875 rem). */
const LABEL_FONT_REM = 0.875;

/** A label's character width and line height, in `em` of its size, for B612 with its tracking. */
const LABEL_CHARACTER_EM = 0.7;
const LABEL_LINE_EM = 1.2;

/** How far a label's text stands from its mark's edge, in `rem`. */
const LABEL_GAP_REM = 0.25;

/** A view's picture as it stands on the page. */
export interface MapPicturePx {
  /** Its width in CSS px. */
  readonly widthPx: number;
  /** Its height in CSS px. */
  readonly heightPx: number;
  /** The root's rem, CSS px, which the marks and labels are sized by. */
  readonly remPx: number;
}

/** A label beside a feature's mark, its text's box in the picture's CSS px. */
export interface MapFeatureLabel {
  readonly text: string;
  /** Where its text starts, CSS px from the picture's left. */
  readonly leftPx: number;
  /** Its text's baseline, CSS px from the picture's top. */
  readonly baselinePx: number;
}

/** One feature or shell as the overlay draws it. */
export interface PlacedMapFeature {
  /** Stable identity: the feature's ID, or `shell:` and the exploded system's. */
  readonly key: string;
  readonly kind: MapFeatureKindWire;
  /** Its centre on the picture, CSS px from the left. */
  readonly xPx: number;
  /** Its centre on the picture, CSS px from the top. */
  readonly yPx: number;
  /** Whether it takes the Greek cross: a feature does, and a shell has no mark of its own. */
  readonly cross: boolean;
  /**
   * The radius of its circle at true radius, CSS px; `null` for a feature no wider on the picture
   * than its mark, which draws none.
   */
  readonly circlePx: number | null;
  /** Its label, or `null` where none fits clear of the labels placed before it. */
  readonly label: MapFeatureLabel | null;
}

/** A label's box, CSS px. */
interface Box {
  readonly leftPx: number;
  readonly topPx: number;
  readonly widthPx: number;
  readonly heightPx: number;
}

function overlap(a: Box, b: Box): boolean {
  return (
    a.leftPx < b.leftPx + b.widthPx &&
    b.leftPx < a.leftPx + a.widthPx &&
    a.topPx < b.topPx + b.heightPx &&
    b.topPx < a.topPx + a.heightPx
  );
}

/**
 * Where a point of the galaxy stands on a view's picture, CSS px from its top left, or `null` for
 * one beyond the map's extent, which is not drawn: the face-on picture turned a quarter-turn
 * clockwise, +x down and +y to the right, and the edge-on picture x across and z up, as the map's
 * own pictures and marks are (`rasterToScreen`).
 */
export function mapPointPx(
  view: MapView,
  geometry: MapGeometry,
  picture: MapPicturePx,
  positionLy: Vec3,
): { readonly xPx: number; readonly yPx: number } | null {
  const vertical = view === "face_on" ? positionLy.y : positionLy.z;
  const { horizontal, vertical: verticalRange } = geometry;
  if (
    !(positionLy.x >= horizontal.minLy && positionLy.x <= horizontal.maxLy) ||
    !(vertical >= verticalRange.minLy && vertical <= verticalRange.maxLy)
  ) {
    return null;
  }
  // On the raster: across from its left edge, down from its top row.
  const left = (positionLy.x - horizontal.minLy) / (horizontal.maxLy - horizontal.minLy);
  const top = (verticalRange.maxLy - vertical) / (verticalRange.maxLy - verticalRange.minLy);
  const screen = view === "face_on" ? { left: 1 - top, top: left } : { left, top };
  return { xPx: screen.left * picture.widthPx, yPx: screen.top * picture.heightPx };
}

/** CSS px of the picture per light-year: the same along both of its axes. */
function pxPerLy(view: MapView, geometry: MapGeometry, picture: MapPicturePx): number {
  // The face-on picture's width is the raster's height, turned.
  const acrossLy =
    view === "face_on"
      ? geometry.vertical.maxLy - geometry.vertical.minLy
      : geometry.horizontal.maxLy - geometry.horizontal.minLy;
  return picture.widthPx / acrossLy;
}

/** A layer's place in the drawing: shells under the globulars, the centre on top. */
function drawRank(kind: MapFeatureKindWire): number {
  let rank: number;
  switch (kind) {
    case "bright_shell":
      rank = 0;
      break;
    case "globular":
      rank = 1;
      break;
    case "galactic_centre":
      rank = 2;
      break;
  }
  return rank;
}

/**
 * A shell's label's rank: above every globular's, whose rank is its mass of at most some 10⁶ M☉, and
 * below the centre's, so that a shell, the rarer and the brighter in X-rays, is never left unlabelled
 * for a globular beside it.
 */
const SHELL_LABEL_PRIORITY = 1e12;

/** A candidate before its label is placed, with the order labels are placed in. */
interface Candidate {
  readonly place: Omit<PlacedMapFeature, "label">;
  readonly text: string;
  /** Larger is labelled first. */
  readonly priority: number;
}

/**
 * Everything the overlay draws on one view's picture, in drawing order, with the labels that fit.
 *
 * @remarks
 * Only the shown kinds that are complete are drawn; a feature or a shell beyond the view's extent,
 * as a globular far above the disc is beyond the edge-on picture's, is not. A feature is the Greek
 * cross at {@link MAP_FEATURE_MARK_REM}, with its circle at true radius where that is wider than
 * the mark, which at the map's scale of some hundreds of light-years a pixel only the largest
 * features are; a shell is its circle alone. Each is labelled with its kind's abbreviation (`GC`,
 * `CENTRE`, `SNR`), the guide's rule for a feature's mark: the centre's first, then the shells',
 * then the globulars', heaviest first, each to the right of its mark, or to the left where that
 * runs past the picture's edge, and dropped where it would cross a label placed before it, so that
 * the inner halo's crowd is labelled where it can be read.
 */
export function placeMapFeatures(
  view: MapView,
  geometry: MapGeometry,
  layers: ReadonlyArray<MapFeatureLayer>,
  shown: ReadonlySet<MapFeatureKindWire>,
  picture: MapPicturePx,
): ReadonlyArray<PlacedMapFeature> {
  const scale = pxPerLy(view, geometry, picture);
  const markRadiusPx = (MAP_FEATURE_MARK_REM * picture.remPx) / 2;
  const candidates: Array<Candidate & { readonly rank: number }> = [];
  for (const layer of layers) {
    if (layer.status !== "complete" || !shown.has(layer.kind)) {
      continue;
    }
    const rank = drawRank(layer.kind);
    if (layer.kind === "bright_shell") {
      for (const shell of layer.shells) {
        const at = mapPointPx(view, geometry, picture, shell.siteLy);
        if (at !== null) {
          candidates.push({
            place: {
              key: `shell:${shell.system}`,
              kind: layer.kind,
              ...at,
              cross: false,
              circlePx: shell.radiusLy * scale,
            },
            text: mapFeatureKindLabel(layer.kind),
            priority: SHELL_LABEL_PRIORITY,
            rank,
          });
        }
      }
      continue;
    }
    for (const feature of layer.features) {
      const at = mapPointPx(view, geometry, picture, feature.positionLy);
      if (at !== null) {
        const circlePx = feature.extentLy * scale;
        candidates.push({
          place: {
            key: feature.id,
            kind: layer.kind,
            ...at,
            cross: true,
            circlePx: circlePx > markRadiusPx ? circlePx : null,
          },
          text: featureKindAbbreviation(feature.kind),
          // The centre's label first, then by mass among the rest.
          priority: layer.kind === "galactic_centre" ? Number.POSITIVE_INFINITY : feature.massMsun,
          rank,
        });
      }
    }
  }

  const labels = new Map<string, MapFeatureLabel>();
  const taken: Box[] = [];
  const fontPx = LABEL_FONT_REM * picture.remPx;
  const gapPx = markRadiusPx + LABEL_GAP_REM * picture.remPx;
  const byPriority = candidates.toSorted((a, b) =>
    a.priority === b.priority ? (a.place.key < b.place.key ? -1 : 1) : b.priority - a.priority,
  );
  for (const { place, text } of byPriority) {
    const widthPx = text.length * LABEL_CHARACTER_EM * fontPx;
    const heightPx = LABEL_LINE_EM * fontPx;
    const topPx = place.yPx - heightPx / 2;
    const sides = [place.xPx + gapPx, place.xPx - gapPx - widthPx];
    const leftPx = sides.find((left) => {
      const box = { leftPx: left, topPx, widthPx, heightPx };
      return (
        left >= 0 &&
        left + widthPx <= picture.widthPx &&
        topPx >= 0 &&
        topPx + heightPx <= picture.heightPx &&
        !taken.some((other) => overlap(box, other))
      );
    });
    if (leftPx !== undefined) {
      taken.push({ leftPx, topPx, widthPx, heightPx });
      // The baseline a third of the line below the mark's centre, centring capitals on it.
      labels.set(place.key, { text, leftPx, baselinePx: place.yPx + fontPx / 3 });
    }
  }

  return candidates
    .toSorted((a, b) => a.rank - b.rank)
    .map(({ place }): PlacedMapFeature => ({
      key: place.key,
      kind: place.kind,
      xPx: place.xPx,
      yPx: place.yPx,
      cross: place.cross,
      circlePx: place.circlePx,
      label: labels.get(place.key) ?? null,
    }));
}
