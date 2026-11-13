import { formatNumber } from "../../lib/format";
import { MAP_FEATURE_MARK_REM, type PicturePointPx } from "../../lib/galaxy/mapFeatures";
import type { StreamOverlayPlaces } from "../../lib/galaxy/streamTracks";
import { SYMBOL_STROKE_PX } from "../../spatial/symbols";
import { featureCrossPath } from "./featureCross";

/** Props of {@link StreamTrackOverlay}. */
export interface StreamTrackOverlayProps {
  /** What to draw, from `placeStreamOverlay`. */
  readonly places: StreamOverlayPlaces;
  /** The picture's width in CSS px, the overlay's own units. */
  readonly widthPx: number;
  /** The picture's height in CSS px. */
  readonly heightPx: number;
  /** The root's rem, CSS px. */
  readonly remPx: number;
  /** Whether the tracks are a snapshot the link no longer backs: they are then muted. */
  readonly stale: boolean;
}

/** A polyline's points, as its attribute. */
function polylinePoints(run: ReadonlyArray<PicturePointPx>): string {
  return run.map((point) => `${String(point.xPx)},${String(point.yPx)}`).join(" ");
}

/** The overlay's accessible name: what it draws, in words. */
function overlayName({ tracks, cores }: StreamOverlayPlaces, stale: boolean): string {
  const parts = [
    tracks.length === 0
      ? null
      : `${formatNumber(tracks.length, 0)} stream ${tracks.length === 1 ? "track" : "tracks"}`,
    cores.length === 0
      ? null
      : `${formatNumber(cores.length, 0)} dwarf ${cores.length === 1 ? "core" : "cores"}`,
  ].filter((part) => part !== null);
  return `Streams: ${parts.length === 0 ? "none drawn" : parts.join(", ")}${stale ? ", stale" : ""}`;
}

/**
 * The galaxy map's stream overlay over one view's picture (plan 10, P10.T11): each stream's track
 * as a thin line, labelled `STREAM` at one end, and each dwarf core as the feature mark, open, with
 * its circle at true radius where that is wider than the mark, labelled `DWARF CORE`.
 *
 * @remarks
 * The guide's stream line: a solid `1px` `--text-muted` line, cased in `--surface-0` as any line
 * over a raster is, so that it keeps its contrast over the brightest part of the map. A core takes
 * the feature mark, the open Greek cross in `--text`, cased the same way. One SVG over the
 * picture, under the feature overlay, in its CSS px, which takes no pointer, so that a pick goes
 * through to the picture beneath. The lines come first, then the cores' circles and crosses, then
 * the labels, so that no line crosses a mark or a label. A stale overlay is muted, crosses and
 * labels in `--text-muted`, and named stale.
 */
export function StreamTrackOverlay({
  places,
  widthPx,
  heightPx,
  remPx,
  stale,
}: StreamTrackOverlayProps) {
  // Drawn inside the mark's diameter, as a spatial display's outline is.
  const cross = featureCrossPath((MAP_FEATURE_MARK_REM * remPx) / 2 - SYMBOL_STROKE_PX / 2);
  const labels = [...places.cores, ...places.tracks].flatMap((place) =>
    place.label === null ? [] : [{ key: place.key, label: place.label }],
  );
  return (
    <svg
      className={stale ? "map-features map-features--stale" : "map-features"}
      viewBox={`0 0 ${String(widthPx)} ${String(heightPx)}`}
      // A drawn picture of lines and marks; what each is, the page's legend and the overlay's name
      // say.
      // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
      role="img"
      aria-label={overlayName(places, stale)}
    >
      {places.tracks.map((track) => (
        <g key={track.key}>
          {track.runs.map((run) => {
            const points = polylinePoints(run);
            return (
              // Where a run starts is its identity: two runs of a track never start at one point.
              <g key={polylinePoints(run.slice(0, 1))}>
                <polyline className="map-streams__casing" points={points} />
                <polyline className="map-streams__line" points={points} />
              </g>
            );
          })}
        </g>
      ))}
      {places.cores.map((core) =>
        core.circlePx === null ? null : (
          <g key={`circle:${core.key}`}>
            <circle
              className="map-features__circle-casing"
              cx={core.xPx}
              cy={core.yPx}
              r={core.circlePx}
            />
            <circle
              className="map-features__circle"
              cx={core.xPx}
              cy={core.yPx}
              r={core.circlePx}
            />
          </g>
        ),
      )}
      {places.cores.map((core) => (
        <g
          key={`cross:${core.key}`}
          transform={`translate(${String(core.xPx)} ${String(core.yPx)})`}
        >
          <path className="map-features__casing" d={cross} />
          <path className="map-features__cross" d={cross} />
        </g>
      ))}
      {labels.map(({ key, label }) => (
        <text
          key={`label:${key}`}
          className="map-features__label"
          x={label.leftPx}
          y={label.baselinePx}
        >
          {label.text}
        </text>
      ))}
    </svg>
  );
}
