import { mapFeatureKindWords } from "../../lib/galaxy/featureWords";
import { MAP_FEATURE_KINDS } from "../../lib/galaxy/featureWire";
import { MAP_FEATURE_MARK_REM, type PlacedMapFeature } from "../../lib/galaxy/mapFeatures";
import { SYMBOL_STROKE_PX, symbolOutline } from "../../spatial/symbols";

/** Props of {@link MapFeatureOverlay}. */
export interface MapFeatureOverlayProps {
  /** What to draw, in drawing order, from `placeMapFeatures`. */
  readonly places: ReadonlyArray<PlacedMapFeature>;
  /** The picture's width in CSS px, the overlay's own units. */
  readonly widthPx: number;
  /** The picture's height in CSS px. */
  readonly heightPx: number;
  /** The root's rem, CSS px. */
  readonly remPx: number;
  /** Whether the features are a snapshot the link no longer backs: they are then muted. */
  readonly stale: boolean;
}

/** The Greek cross's outline about the origin at `radiusPx`, as an SVG path's data. */
function crossPath(radiusPx: number): string {
  const outline = symbolOutline("greek-cross");
  const points = outline.kind === "polygon" ? outline.points : [];
  return `${points
    .map((point, index) => `${index === 0 ? "M" : "L"} ${point.x * radiusPx} ${point.y * radiusPx}`)
    .join(" ")} Z`;
}

/** The overlay's accessible name: what it draws, kind by kind in the toggles' order, in words. */
function overlayName(places: ReadonlyArray<PlacedMapFeature>, stale: boolean): string {
  const parts = MAP_FEATURE_KINDS.flatMap((kind) => {
    const count = places.filter((place) => place.kind === kind).length;
    return count === 0 ? [] : [mapFeatureKindWords(kind, count)];
  });
  return `Features: ${parts.length === 0 ? "none drawn" : parts.join(", ")}${stale ? ", stale" : ""}`;
}

/**
 * The galaxy map's feature overlay over one view's picture (plan 09, P09.T41.b): the globulars and
 * the centre as the Greek cross, open, since the map has no reference plane, and each bright
 * shell, and a feature wider than its mark, as a circle at its true radius, each labelled with its
 * kind's abbreviation where the label fits.
 *
 * @remarks
 * One SVG over the picture, in its CSS px, which takes no pointer, so that a pick goes through to
 * the picture beneath. Every line is cased in `--surface-0`, as any line over a raster is, so that
 * it keeps its contrast over the brightest part of the map: the crosses in `--text` at a mark's
 * outline, the circles in `--text-muted` 1 px wide, and the labels in `--text` cased as the lines
 * are. The circles come first, then the crosses, then the labels, so that no circle crosses a
 * mark. A stale overlay is muted, crosses and labels in `--text-muted`, and named stale.
 */
export function MapFeatureOverlay({
  places,
  widthPx,
  heightPx,
  remPx,
  stale,
}: MapFeatureOverlayProps) {
  // Drawn inside the mark's diameter, as a spatial display's outline is.
  const cross = crossPath((MAP_FEATURE_MARK_REM * remPx) / 2 - SYMBOL_STROKE_PX / 2);
  const circles = places.filter((place) => place.circlePx !== null);
  const crosses = places.filter((place) => place.cross);
  const labels = places.flatMap((place) => (place.label === null ? [] : [place]));
  return (
    <svg
      className={stale ? "map-features map-features--stale" : "map-features"}
      viewBox={`0 0 ${String(widthPx)} ${String(heightPx)}`}
      // A drawn picture of marks; what each is, the page's legend and the overlay's name say.
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
      role="img"
      aria-label={overlayName(places, stale)}
    >
      {circles.map((place) => (
        <g key={`circle:${place.key}`}>
          <circle
            className="map-features__circle-casing"
            cx={place.xPx}
            cy={place.yPx}
            r={place.circlePx ?? 0}
          />
          <circle
            className="map-features__circle"
            cx={place.xPx}
            cy={place.yPx}
            r={place.circlePx ?? 0}
          />
        </g>
      ))}
      {crosses.map((place) => (
        <g
          key={`cross:${place.key}`}
          transform={`translate(${String(place.xPx)} ${String(place.yPx)})`}
        >
          <path className="map-features__casing" d={cross} />
          <path className="map-features__cross" d={cross} />
        </g>
      ))}
      {labels.map((place) =>
        place.label === null ? null : (
          <text
            key={`label:${place.key}`}
            className="map-features__label"
            x={place.label.leftPx}
            y={place.label.baselinePx}
          >
            {place.label.text}
          </text>
        ),
      )}
    </svg>
  );
}
