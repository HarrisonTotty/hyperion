/**
 * Where the galaxy map's stream overlay draws each stream's track and each dwarf core on a view's
 * picture (plan 10, P10.T11), and a stream's figures as its readout states them: pure projections,
 * which the overlay and the readout draw as they are given.
 */
import type { MapView } from "@hyperion/protocol";

import type { Vec3 } from "../../geometry/vec3";
import {
  type LabelBox,
  MAP_FEATURE_MARK_REM,
  type MapFeatureLabel,
  type MapPicturePx,
  mapPlanePx,
  mapPointPx,
  type PicturePointPx,
  placeLabel,
  pxPerLy,
} from "./mapFeatures";
import type { MapGeometry } from "./mapGeometry";
import type { GlobalDwarfCore, StreamTrack, TrackPoint } from "./model";

/** A stream's label on the map: its term in the guide's nomenclature, as a feature's names its kind. */
export const STREAM_LABEL = "STREAM";

/** A dwarf core's label on the map. */
export const DWARF_CORE_LABEL = "DWARF CORE";

/** What the stream overlay draws, each switchable. */
export type StreamOverlayKind = "stream" | "dwarf_core";

/** The stream overlay's kinds, in its toggles' order. */
export const STREAM_OVERLAY_KINDS: ReadonlyArray<StreamOverlayKind> = ["stream", "dwarf_core"];

/** One stream's track as the overlay draws it on one view's picture. */
export interface PlacedTrack {
  /** Stable identity: `stream:` and its number. */
  readonly key: string;
  readonly number: number;
  /** The parts of its centre line inside the view, each a polyline of at least two points. */
  readonly runs: ReadonlyArray<ReadonlyArray<PicturePointPx>>;
  /** Its label at one end, or `null` where none fits clear of the labels placed before it. */
  readonly label: MapFeatureLabel | null;
}

/** One dwarf core as the overlay draws it: the feature mark, open, with its circle and label. */
export interface PlacedCore {
  /** Stable identity: `core:` and its number. */
  readonly key: string;
  readonly number: number;
  readonly xPx: number;
  readonly yPx: number;
  /** The radius of its circle at true radius, CSS px; `null` where it is no wider than its mark. */
  readonly circlePx: number | null;
  readonly label: MapFeatureLabel | null;
}

/** What the stream overlay draws on one view's picture. */
export interface StreamOverlayPlaces {
  /** The tracks, heaviest stream first, the order their labels are placed in. */
  readonly tracks: ReadonlyArray<PlacedTrack>;
  readonly cores: ReadonlyArray<PlacedCore>;
}

/** A point of a view's plane: x, and y face-on or z edge-on. */
interface PlanePoint {
  readonly hLy: number;
  readonly vLy: number;
}

function onPlane(view: MapView, positionLy: Vec3): PlanePoint {
  return { hLy: positionLy.x, vLy: view === "face_on" ? positionLy.y : positionLy.z };
}

/**
 * The part of the segment from `a` to `b` inside the view's extent, by the parameters of its ends
 * along it, or `null` where none of it is inside (Liang and Barsky's clipping).
 */
function clipped(
  a: PlanePoint,
  b: PlanePoint,
  geometry: MapGeometry,
): { readonly from: number; readonly to: number } | null {
  const dh = b.hLy - a.hLy;
  const dv = b.vLy - a.vLy;
  let from = 0;
  let to = 1;
  const edges: ReadonlyArray<readonly [number, number]> = [
    [-dh, a.hLy - geometry.horizontal.minLy],
    [dh, geometry.horizontal.maxLy - a.hLy],
    [-dv, a.vLy - geometry.vertical.minLy],
    [dv, geometry.vertical.maxLy - a.vLy],
  ];
  for (const [p, q] of edges) {
    if (p === 0) {
      if (q < 0) {
        return null;
      }
    } else {
      const t = q / p;
      if (p < 0) {
        from = Math.max(from, t);
      } else {
        to = Math.min(to, t);
      }
    }
  }
  return from <= to ? { from, to } : null;
}

function along(a: PlanePoint, b: PlanePoint, t: number): PlanePoint {
  return { hLy: a.hLy + (b.hLy - a.hLy) * t, vLy: a.vLy + (b.vLy - a.vLy) * t };
}

/**
 * A track's centre line on a view's picture: the trailing arm from its tip to its root, then the
 * leading arm from its root to its tip, cut to the view's extent, as the parts that lie inside it,
 * each a polyline in the picture's CSS px.
 *
 * @remarks
 * A part leaves the extent where a segment crosses its edge, at the crossing, so that no line runs
 * past the picture and none is pegged; a track wholly beyond the extent has no parts. Projected as
 * the map's marks are ({@link mapPointPx}): face-on along z, edge-on along y.
 */
export function projectTrack(
  view: MapView,
  geometry: MapGeometry,
  picture: MapPicturePx,
  track: StreamTrack,
): ReadonlyArray<ReadonlyArray<PicturePointPx>> {
  const points = [...track.trailing, ...track.leading].map((point) =>
    onPlane(view, point.positionLy),
  );
  const toPx = (point: PlanePoint): PicturePointPx =>
    mapPlanePx(view, geometry, picture, point.hLy, point.vLy);
  const runs: PicturePointPx[][] = [];
  let run: PicturePointPx[] = [];
  for (let index = 1; index < points.length; index += 1) {
    const a = points[index - 1];
    const b = points[index];
    if (a === undefined || b === undefined) {
      break;
    }
    const part = clipped(a, b, geometry);
    if (part === null) {
      continue;
    }
    if (run.length === 0 || part.from > 0) {
      if (run.length > 1) {
        runs.push(run);
      }
      run = [toPx(along(a, b, part.from))];
    }
    run.push(toPx(along(a, b, part.to)));
    if (part.to < 1) {
      runs.push(run);
      run = [];
    }
  }
  if (run.length > 1) {
    runs.push(run);
  }
  return runs;
}

/** What {@link placeStreamOverlay} draws, and which of it is shown. */
export interface StreamOverlayInput {
  /** The tracks on hand, each with its stream's mass, which orders the labels. */
  readonly tracks: ReadonlyArray<{ readonly track: StreamTrack; readonly massMsun: number }>;
  /** The dwarf cores of the global list. */
  readonly cores: ReadonlyArray<GlobalDwarfCore>;
  /** Whether the tracks are shown. */
  readonly streamsShown: boolean;
  /** Whether the dwarf cores are shown. */
  readonly coresShown: boolean;
}

/** The box a mark of `radiusPx` about `at` covers, which no label may cross. */
function markBox(at: PicturePointPx, radiusPx: number): LabelBox {
  return {
    leftPx: at.xPx - radiusPx,
    topPx: at.yPx - radiusPx,
    widthPx: 2 * radiusPx,
    heightPx: 2 * radiusPx,
  };
}

/**
 * Everything the stream overlay draws on one view's picture, with the labels that fit.
 *
 * @remarks
 * A track is its centre line ({@link projectTrack}), labelled `STREAM` at one end: its leading
 * arm's tip, or where that lies beyond the view its trailing arm's, and not at all where neither
 * tip lies inside the view, so that no label stands where the picture's edge cuts a track and reads
 * as a peg. A dwarf core is the feature mark at its centre, open as the map's marks are, with its
 * circle at true radius where that is wider than the mark, labelled `DWARF CORE` outside its
 * circle. A core or a track beyond the view is not drawn, nor a core from a server that does not
 * send its place. Every label keeps clear of the boxes `taken` (the feature overlay's labels and
 * marks, which go before every one of these), of every core's mark, and of the labels placed before
 * it, and is dropped where it would cross one: the cores' first, then the tracks', heaviest stream
 * first.
 */
export function placeStreamOverlay(
  view: MapView,
  geometry: MapGeometry,
  picture: MapPicturePx,
  input: StreamOverlayInput,
  taken: ReadonlyArray<LabelBox>,
): StreamOverlayPlaces {
  const boxes = [...taken];
  const markRadiusPx = (MAP_FEATURE_MARK_REM * picture.remPx) / 2;
  const scale = pxPerLy(view, geometry, picture);
  const marks: Array<Omit<PlacedCore, "label">> = [];
  if (input.coresShown) {
    for (const core of input.cores) {
      const at =
        core.positionLy === null ? null : mapPointPx(view, geometry, picture, core.positionLy);
      if (at === null) {
        continue;
      }
      const circlePx = core.extentLy === null ? null : core.extentLy * scale;
      marks.push({
        key: `core:${String(core.number)}`,
        number: core.number,
        xPx: at.xPx,
        yPx: at.yPx,
        circlePx: circlePx !== null && circlePx > markRadiusPx ? circlePx : null,
      });
      boxes.push(markBox(at, markRadiusPx));
    }
  }
  const cores: PlacedCore[] = [];
  for (const { key, number, xPx, yPx, circlePx } of marks) {
    const at = { xPx, yPx };
    const label = placeLabel(at, DWARF_CORE_LABEL, circlePx ?? markRadiusPx, picture, boxes);
    cores.push({ key, number, xPx, yPx, circlePx, label });
  }
  const tracks: PlacedTrack[] = [];
  if (input.streamsShown) {
    const heaviest = input.tracks.toSorted((a, b) =>
      a.massMsun === b.massMsun ? a.track.number - b.track.number : b.massMsun - a.massMsun,
    );
    for (const { track } of heaviest) {
      const runs = projectTrack(view, geometry, picture, track);
      if (runs.length === 0) {
        continue;
      }
      const leadingTip = track.leading.at(-1) ?? track.trailing.at(-1);
      const trailingTip = track.trailing[0] ?? track.leading[0];
      const tip = [leadingTip, trailingTip]
        .map((point) =>
          point === undefined ? null : mapPointPx(view, geometry, picture, point.positionLy),
        )
        .find((at) => at !== null);
      tracks.push({
        key: `stream:${String(track.number)}`,
        number: track.number,
        runs,
        label: tip === undefined ? null : placeLabel(tip, STREAM_LABEL, 0, picture, boxes),
      });
    }
  }
  return { tracks, cores };
}

/** A stream's figures as its readout states them. */
export interface StreamFigures {
  /** Its centre line's length, in light-years, or `null` where the server does not send it. */
  readonly lengthLy: number | null;
  /** Its median rms width, the mean of the normal's and the binormal's, in light-years. */
  readonly widthLy: number | null;
  /** Its median rms velocity dispersion along one axis, in km/s, or `null` where none is sent. */
  readonly dispersionKmS: number | null;
}

function median(values: ReadonlyArray<number>): number | null {
  const sorted = values.toSorted((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  const upper = sorted[middle];
  if (upper === undefined) {
    return null;
  }
  const lower = sorted.length % 2 === 0 ? sorted[middle - 1] : upper;
  return lower === undefined ? upper : (lower + upper) / 2;
}

/**
 * A stream's length, width and dispersion from its track, as plan 10's P10.T12 states a tube's: its
 * length the server's, measured on every knot; its width and dispersion the medians over the
 * points sent, which thin each arm evenly to at most 128 of its knots.
 */
export function streamFigures(track: StreamTrack): StreamFigures {
  const points: ReadonlyArray<TrackPoint> = [...track.trailing, ...track.leading];
  return {
    lengthLy: track.lengthLy,
    widthLy: median(points.map((point) => (point.normalWidthLy + point.binormalWidthLy) / 2)),
    dispersionKmS: median(
      points.flatMap((point) => (point.dispersionKmS === null ? [] : [point.dispersionKmS])),
    ),
  };
}
