import {
  decodeDensityMap,
  decodeExtinctionMap,
  type DensityMap,
  type ExtinctionMap,
  type MapPopulation,
  type MapView,
  type RequestKind,
  type RequestOf,
  type UniverseIdHex,
} from "@hyperion/protocol";
import {
  type KeyboardEvent,
  type MouseEvent,
  type ReactNode,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import { RequestStatus } from "../../components/RequestStatus";
import { StaleMark } from "../../components/StaleMark";
import { StatusLine } from "../../components/StatusLine";
import { formatNumber, formatSci } from "../../lib/format";
import {
  type MapGeometry,
  mapGeometry,
  type MapRaster,
  pixelToLy,
} from "../../lib/galaxy/mapGeometry";
import {
  dustOverlay,
  mapResolutionFor,
  paintLevels,
  reducedLevels,
  turnClockwise,
} from "../../lib/galaxy/mapPicture";
import type { CentreLy } from "../../lib/galaxy/model";
import { buildRamp, type DecodedCodes, parseHexColour, rasterise } from "../../lib/galaxy/ramp";
import { type RequestState, useServerRequest } from "../../lib/useServerRequest";
import { DensityLegend } from "./DensityLegend";
import { ExtinctionLegend } from "./ExtinctionLegend";
import {
  ARROW_KEYS,
  cursorInView,
  LARGE_STEP_PX,
  MAP_ACROSS_LY,
  markOnPicture,
  type MarkPlace,
  pickCursor,
  pixelIndexAt,
  rasterDirection,
  rasterToScreen,
  SCREEN_ASPECT,
  SCREEN_TURN,
  screenSizePx,
  screenToRaster,
  stepCursor,
} from "./mapCursor";
import { usePageTabFocus } from "./pageTabFocus";

/** Bits per code of the map the display asks for: the ramp has 256 levels (plan 05, D5). */
const MAP_BITS = 8;

/**
 * How long a map may take before it is reported as timed out, in milliseconds.
 *
 * @remarks
 * A map takes the server seconds, not milliseconds, and queues behind other work (plan 05, design
 * note D4), so it is given four times the default.
 */
const MAP_TIMEOUT_MS = 120_000;

/** How far a map's span may differ from the root cube's edge and still be drawn at one scale. */
const SPAN_TOLERANCE = 1e-9;

const VIEW_TITLE: Readonly<Record<MapView, string>> = {
  face_on: "FACE-ON FROM NORTH",
  edge_on: "EDGE-ON ALONG +Y",
};

const VIEW_NAME: Readonly<Record<MapView, string>> = {
  face_on: "face-on",
  edge_on: "edge-on",
};

/** What a view's raster shows, for the picture's accessible name. */
type LayerKind = "density" | "extinction" | "overlay";

function layerKind(question: MapQuestion): LayerKind {
  if (question.quantity === "extinction") {
    return "extinction";
  }
  return question.overlay ? "overlay" : "density";
}

/** What the picture's name adds for each raster: nothing for the density map alone. */
const LAYER_NAME: Readonly<Record<LayerKind, string>> = {
  density: "",
  extinction: ", extinction",
  overlay: ", dust overlay",
};

/** The keys that move the cursor on each view: edge-on sets only z (plan 05, design note D19). */
const KEY_HINT: Readonly<Record<MapView, string>> = {
  face_on: "ARROWS MOVE CURSOR",
  edge_on: "↑ ↓ MOVE CURSOR",
};

/**
 * Each view's axes as the screen shows them: face-on +y to the right and +x down, since the raster
 * is turned (see `mapCursor.ts`); edge-on +x to the right and +z up.
 */
const AXIS_LABELS: Readonly<
  Record<
    MapView,
    { readonly across: string; readonly vertical: string; readonly end: "top" | "bottom" }
  >
> = {
  face_on: { across: "+Y", vertical: "+X", end: "bottom" },
  edge_on: { across: "+X", vertical: "+Z NORTH", end: "top" },
};

/**
 * Which quantity a view's raster shows (plan 07, P07.T11.b): the column density of systems, which
 * may carry the dust overlay, or the visual extinction through the galaxy.
 */
export type MapQuantity = "systems" | "extinction";

/**
 * The label the page shows while the overlay is on, naming it and its quantity, as the guide
 * requires; once for the page, not in each view.
 */
export const DUST_OVERLAY_LABEL = "DUST OVERLAY: A(V), WHOLE LINE OF SIGHT";

/** One map that decoded and whose geometry holds: a density map's or an extinction map's. */
interface MapLayer {
  readonly geometry: MapGeometry;
  /** The codes as the map has them, row by row from the top: what the cursor reads. */
  readonly codes: DecodedCodes;
  /** The codes turned as the screen shows them. */
  readonly onScreen: DecodedCodes;
  /** log₁₀ of the quantity of code 1, in the map's unit. */
  readonly floorLog10: number;
  /** log₁₀ of the quantity of the largest code, in the map's unit. */
  readonly ceilingLog10: number;
  /** The map's rule from a code to log₁₀ of its quantity, `null` for code 0. */
  readonly log10Of: (code: number) => number | null;
}

/** What a view paints, and the maps it was made from, ready to paint. */
interface MapPicture {
  readonly geometry: MapGeometry;
  /** The codes painted, as the screen shows them: a map's own, or the density dimmed by the dust. */
  readonly onScreen: DecodedCodes;
  /** The painted scale's ceiling less its floor, in dex. */
  readonly spanLog10: number;
  /** The density map, under `SYSTEMS`. */
  readonly density: MapLayer | null;
  /** The extinction map, under `EXTINCTION` or with the dust overlay. */
  readonly extinction: MapLayer | null;
}

/**
 * What became of a map's answer: none yet, invalid with its cause in words, or a picture to paint.
 */
type Decoding =
  | { readonly kind: "none" }
  | { readonly kind: "invalid"; readonly cause: string }
  | { readonly kind: "ok"; readonly picture: MapPicture };

const NO_MAP: Decoding = { kind: "none" };

/** What a view asked for: the maps it needs, which their answers must echo, and how it paints them. */
interface MapQuestion {
  readonly universe: UniverseIdHex;
  readonly view: MapView;
  readonly population: MapPopulation;
  readonly quantity: MapQuantity;
  /** Whether the density is dimmed by the dust, which it is only under `SYSTEMS`. */
  readonly overlay: boolean;
}

/** Whether a question needs the density map: under `SYSTEMS`, with or without the overlay. */
function needsDensity(question: MapQuestion): boolean {
  return question.quantity === "systems";
}

/** Whether a question needs the extinction map: under `EXTINCTION`, or for the overlay. */
function needsExtinction(question: MapQuestion): boolean {
  return question.quantity === "extinction" || question.overlay;
}

function isPositiveFinite(value: number): boolean {
  return Number.isFinite(value) && value > 0;
}

function invalid(cause: string): Decoding {
  return { kind: "invalid", cause };
}

/**
 * Whether a map has its view's shape on screen and spans the root cube's edge across it, so that
 * it can be drawn in its view's box at the scale both views share.
 */
function hasScreenShape(map: MapRaster): boolean {
  const onScreen = screenSizePx(map.view, { widthPx: map.width_px, heightPx: map.height_px });
  const acrossLy = onScreen.widthPx * map.ly_per_px;
  return (
    onScreen.widthPx === SCREEN_ASPECT[map.view] * onScreen.heightPx &&
    Math.abs(acrossLy - MAP_ACROSS_LY) <= SPAN_TOLERANCE * MAP_ACROSS_LY
  );
}

/** What became of one map's answer: none yet, invalid with its cause in words, or a layer. */
type LayerDecoding =
  | { readonly kind: "none" }
  | { readonly kind: "invalid"; readonly cause: string }
  | { readonly kind: "ok"; readonly layer: MapLayer };

/**
 * The cause, in words, of a raster that is not the one asked for or cannot be drawn, or `null`
 * for one that can: of another universe or view, or not of the M1 extents.
 */
function rasterFault(
  map: MapRaster & { readonly universe: UniverseIdHex },
  question: MapQuestion,
): string | null {
  if (map.universe !== question.universe || map.view !== question.view) {
    return "not the map requested";
  }
  const sizeValid =
    Number.isInteger(map.width_px) &&
    Number.isInteger(map.height_px) &&
    map.width_px > 0 &&
    map.height_px > 0 &&
    isPositiveFinite(map.ly_per_px) &&
    map.centre_ly.every((ly) => Number.isFinite(ly)) &&
    hasScreenShape(map);
  return sizeValid ? null : "size or scale unusable";
}

/** Whether a floor and ceiling can be drawn: finite, and the ceiling not below the floor. */
function rangeValid(floorLog10: number, ceilingLog10: number): boolean {
  return Number.isFinite(floorLog10) && Number.isFinite(ceilingLog10) && ceilingLog10 >= floorLog10;
}

/** A decoded map, with its turn for the screen, as a layer. */
function layerOf(
  map: MapRaster,
  codes: DecodedCodes,
  floorLog10: number,
  ceilingLog10: number,
  log10Of: (code: number) => number | null,
): LayerDecoding {
  return {
    kind: "ok",
    layer: {
      geometry: mapGeometry(map),
      codes,
      onScreen: SCREEN_TURN[map.view] === "clockwise" ? turnClockwise(codes) : codes,
      floorLog10,
      ceilingLog10,
      log10Of,
    },
  };
}

/**
 * Checks and decodes a density map's answer.
 *
 * @remarks
 * A map is invalid when it is not the map asked for, when its size, scale, centre or density range
 * cannot be drawn, or when its codes do not decode. The size and scale must be plan 04's M1
 * extents, since both views are drawn at one scale. Plan 04's map of an empty galaxy, with floor
 * and ceiling equal, is drawn.
 */
function decodeDensity(map: DensityMap | null, question: MapQuestion): LayerDecoding {
  if (map === null) {
    return { kind: "none" };
  }
  const fault =
    map.population === question.population ? rasterFault(map, question) : "not the map requested";
  if (fault !== null) {
    return { kind: "invalid", cause: fault };
  }
  if (!rangeValid(map.floor_log10_per_ly2, map.ceiling_log10_per_ly2)) {
    return { kind: "invalid", cause: "density range unusable" };
  }
  try {
    const decoded = decodeDensityMap(map);
    return layerOf(map, decoded, map.floor_log10_per_ly2, map.ceiling_log10_per_ly2, (code) =>
      decoded.log10PerLy2(code),
    );
  } catch {
    // Bad base64, a bit depth other than 8 or 16, or a byte count that does not fill the map: the
    // server's fault, reported to the operator in words.
    return { kind: "invalid", cause: "pixel codes unreadable" };
  }
}

/** Checks and decodes an extinction map's answer, as {@link decodeDensity} does a density map's. */
function decodeExtinction(map: ExtinctionMap | null, question: MapQuestion): LayerDecoding {
  if (map === null) {
    return { kind: "none" };
  }
  const fault = rasterFault(map, question);
  if (fault !== null) {
    return { kind: "invalid", cause: `extinction ${fault}` };
  }
  if (!rangeValid(map.floor_log10_mag, map.ceiling_log10_mag)) {
    return { kind: "invalid", cause: "extinction range unusable" };
  }
  try {
    const decoded = decodeExtinctionMap(map);
    return layerOf(map, decoded, map.floor_log10_mag, map.ceiling_log10_mag, (code) =>
      decoded.log10Mag(code),
    );
  } catch {
    return { kind: "invalid", cause: "extinction pixel codes unreadable" };
  }
}

/** Whether two layers cover the same pixels, so that one can dim the other. */
function sameRaster(a: MapLayer, b: MapLayer): boolean {
  return (
    a.codes.widthPx === b.codes.widthPx &&
    a.codes.heightPx === b.codes.heightPx &&
    a.geometry.lyPerPx === b.geometry.lyPerPx &&
    a.geometry.centreLy[0] === b.geometry.centreLy[0] &&
    a.geometry.centreLy[1] === b.geometry.centreLy[1]
  );
}

/**
 * Checks and decodes the maps a question needs, and makes the picture from them.
 *
 * @remarks
 * Under `SYSTEMS` the picture is the density map's, and with the dust overlay its codes dimmed by
 * the extinction map's ({@link dustOverlay}), once, as the screen shows them, before either way of
 * painting them. Under `EXTINCTION` it is the extinction map's. A picture that needs two maps
 * waits for both, and is invalid if either is or if they do not cover the same pixels.
 */
function decodePicture(
  densityMap: DensityMap | null,
  extinctionMap: ExtinctionMap | null,
  question: MapQuestion,
): Decoding {
  const density = needsDensity(question) ? decodeDensity(densityMap, question) : null;
  const extinction = needsExtinction(question) ? decodeExtinction(extinctionMap, question) : null;
  for (const decoding of [density, extinction]) {
    if (decoding?.kind === "invalid") {
      return invalid(decoding.cause);
    }
  }
  if (density?.kind === "none" || extinction?.kind === "none") {
    return NO_MAP;
  }
  const densityLayer = density?.kind === "ok" ? density.layer : null;
  const extinctionLayer = extinction?.kind === "ok" ? extinction.layer : null;
  if (densityLayer !== null && extinctionLayer !== null) {
    if (!sameRaster(densityLayer, extinctionLayer)) {
      return invalid("density and extinction maps do not match");
    }
    const spanLog10 = densityLayer.ceilingLog10 - densityLayer.floorLog10;
    return {
      kind: "ok",
      picture: {
        geometry: densityLayer.geometry,
        onScreen: dustOverlay(
          densityLayer.onScreen,
          spanLog10,
          extinctionLayer.onScreen,
          extinctionLayer.log10Of,
        ),
        spanLog10,
        density: densityLayer,
        extinction: extinctionLayer,
      },
    };
  }
  const shown = densityLayer ?? extinctionLayer;
  if (shown === null) {
    return NO_MAP;
  }
  return {
    kind: "ok",
    picture: {
      geometry: shown.geometry,
      onScreen: shown.onScreen,
      spanLog10: shown.ceilingLog10 - shown.floorLog10,
      density: densityLayer,
      extinction: extinctionLayer,
    },
  };
}

function sameBytes(a: Uint8ClampedArray, b: Uint8ClampedArray): boolean {
  return a.length === b.length && a.every((byte, index) => byte === b[index]);
}

/**
 * The ramps a map is painted on, from the colour tokens in effect at `element`: `--surface-0` up to
 * `--text` for a current picture, and up to `--text-muted` for a stale one, as the guide shows a
 * stale value in `--text-muted` ("Data states").
 */
interface Ramps {
  readonly current: Uint8ClampedArray;
  readonly stale: Uint8ClampedArray;
}

function rampsAt(element: Element): Ramps {
  const style = getComputedStyle(element);
  const background = parseHexColour(style.getPropertyValue("--surface-0"));
  return {
    current: buildRamp(background, parseHexColour(style.getPropertyValue("--text"))),
    stale: buildRamp(background, parseHexColour(style.getPropertyValue("--text-muted"))),
  };
}

function sameRamps(a: Ramps, b: Ramps): boolean {
  return sameBytes(a.current, b.current) && sameBytes(a.stale, b.stale);
}

function sameQuestion(a: MapQuestion, b: MapQuestion): boolean {
  return (
    a.universe === b.universe &&
    a.view === b.view &&
    a.population === b.population &&
    a.quantity === b.quantity &&
    a.overlay === b.overlay
  );
}

/** The last picture a view drew, with the question it answered. */
interface ShownPicture {
  readonly picture: MapPicture;
  readonly question: MapQuestion;
}

/** RGBA bytes to put on a canvas of their own size, which is then drawn at the backing size. */
interface PictureSource {
  readonly rgba: Uint8ClampedArray;
  readonly widthPx: number;
  readonly heightPx: number;
}

/**
 * The map reduced to a backing store of `widthPx` by `heightPx` device pixels, which is smaller
 * than it, by area-weighted averaging of the linear quantity (see `mapPicture.ts`), so that no map
 * pixel is dropped.
 */
function reducedSource(
  picture: MapPicture,
  ramp: Uint8ClampedArray,
  widthPx: number,
  heightPx: number,
): PictureSource {
  const { onScreen } = picture;
  const reducedWidthPx = Math.min(onScreen.widthPx, widthPx);
  const reducedHeightPx = Math.min(onScreen.heightPx, heightPx);
  const levels = reducedLevels(onScreen, picture.spanLog10, reducedWidthPx, reducedHeightPx);
  return { rgba: paintLevels(levels, ramp), widthPx: reducedWidthPx, heightPx: reducedHeightPx };
}

/**
 * Paints the pixels into a canvas of their own size, for drawing onto the visible one; `null` when
 * the canvas has no 2D context.
 */
function sourceCanvas(source: PictureSource): HTMLCanvasElement | null {
  const canvas = document.createElement("canvas");
  canvas.width = source.widthPx;
  canvas.height = source.heightPx;
  const context = canvas.getContext("2d");
  if (context === null) {
    return null;
  }
  const image = context.createImageData(source.widthPx, source.heightPx);
  image.data.set(source.rgba);
  context.putImageData(image, 0, 0);
  return canvas;
}

/** The value of a map under the cursor on one view, as the operator reads it. */
type CursorValue =
  | { readonly kind: "value"; readonly log10: number }
  | { readonly kind: "below_floor" }
  | { readonly kind: "off_map" };

/**
 * The value of the map pixel under the cursor.
 *
 * @remarks
 * It is the value the pixel's code stands for, which is the centre of the code's step, since plan
 * 04's quantiser rounds to the nearest code. The guide keeps `~` for estimated values, derived or
 * sensor-limited, and says nothing of a value rounded for the wire, which every shown value is at
 * some precision, so it carries no `~`. Under the overlay the density is the map's own, undimmed:
 * the dimmed picture is no quantity a reading could name.
 */
function valueUnder(layer: MapLayer, view: MapView, cursorLy: CentreLy): CursorValue {
  const index = pixelIndexAt(layer.geometry, cursorInView(view, cursorLy));
  if (index === null) {
    return { kind: "off_map" };
  }
  const code = layer.codes.codes[index];
  if (code === undefined) {
    throw new Error(`the decoded map has no code for its pixel ${index}`);
  }
  const log10 = layer.log10Of(code);
  return log10 === null ? { kind: "below_floor" } : { kind: "value", log10 };
}

/** One reading under the cursor: its label, its value, and how a value is written. */
interface CursorRow {
  readonly label: "CURSOR DENSITY" | "CURSOR A(V)";
  readonly value: CursorValue;
  /** The digits of a value of log₁₀ `log10`. */
  readonly digits: (log10: number) => string;
  readonly unit: string;
}

function densityDigits(log10PerLy2: number): string {
  return formatSci(10 ** log10PerLy2);
}

/** A(V) to two decimals, as every A(V) on the console is written. */
function extinctionDigits(log10Mag: number): string {
  return formatNumber(10 ** log10Mag, 2);
}

/** The readings under the cursor that a picture offers: the density, the extinction, or both. */
function cursorRows(picture: MapPicture, view: MapView, cursorLy: CentreLy): CursorRow[] {
  const rows: CursorRow[] = [];
  if (picture.density !== null) {
    rows.push({
      label: "CURSOR DENSITY",
      value: valueUnder(picture.density, view, cursorLy),
      digits: densityDigits,
      unit: "SYSTEMS/ly²",
    });
  }
  if (picture.extinction !== null) {
    rows.push({
      label: "CURSOR A(V)",
      value: valueUnder(picture.extinction, view, cursorLy),
      digits: extinctionDigits,
      unit: "mag",
    });
  }
  return rows;
}

interface CursorReadingProps {
  readonly row: CursorRow;
  /** Whether it is read from a stale picture: then in `--text-muted` with a trailing `S`. */
  readonly stale: boolean;
}

/** One reading under the cursor: a value, `BELOW FLOOR`, or an em dash off the map. */
function CursorReadingValue({ row, stale }: CursorReadingProps) {
  const { value } = row;
  let shown: ReactNode;
  switch (value.kind) {
    case "value":
      shown = (
        <>
          <span className={`field__value map-view__density${stale ? " stale" : ""}`}>
            {row.digits(value.log10)}
          </span>{" "}
          <span className="map-view__unit">{row.unit}</span>
          {stale ? <StaleMark /> : null}
        </>
      );
      break;
    case "below_floor":
      shown = (
        <span className={stale ? "stale" : undefined}>
          BELOW FLOOR{stale ? <StaleMark /> : null}
        </span>
      );
      break;
    case "off_map":
      shown = <span className="readout__missing">—</span>;
      break;
  }
  return shown;
}

interface CursorReadingsProps {
  readonly rows: ReadonlyArray<CursorRow>;
  readonly stale: boolean;
  /** Whether they announce their changes, which only the view whose picture has focus does. */
  readonly announce: boolean;
}

/**
 * The readings under the cursor, one live region for them all.
 *
 * @remarks
 * Read out as a whole when they change, but only beside the picture the operator is moving the
 * cursor over. The cursor is shared by both views and its x is in both, so one arrow key that moves
 * x changes both views' readings, and two live regions changing on one key press is two
 * announcements of the same reading — which is a defect, not verbosity (the orchestrator's ruling
 * 16). The arrow keys reach a picture only while it has focus, so the focused view's readings are
 * the ones the key asked for; the other's stay on screen to be read on demand. Under the dust
 * overlay a view has two readings, which share one region for the same reason.
 */
function CursorReadings({ rows, stale, announce }: CursorReadingsProps) {
  const live = { "aria-live": announce ? "polite" : "off", "aria-atomic": "true" } as const;
  const lines = rows.map((row) => (
    <p key={row.label} className="field" {...(rows.length === 1 ? live : {})}>
      <span className="field__label">{row.label}</span>{" "}
      <CursorReadingValue row={row} stale={stale} />
    </p>
  ));
  return rows.length === 1 ? lines : <div {...live}>{lines}</div>;
}

/** The marks drawn over a map: the cursor, and the chart's centre. */
type MarkKind = "cursor" | "centre";

/**
 * Each mark's shape and name, one meaning to a shape. The cursor is a thin upright cross with a gap
 * at its point, in the selection colour. The chart's centre is a small diagonal cross in `--text`:
 * not the bracket reticle, which marks a selection on a spatial display, and turned 45° from the
 * cursor, so that it reads apart from it, its arms clear of the cursor's even where the cursor
 * stands on the centre, as it does after `C`. Both are cased in the background colour, so that
 * they keep their contrast over the brightest part of the map.
 */
const MARKS: Readonly<Record<MarkKind, { readonly path: string; readonly name: string }>> = {
  cursor: { path: "M0 12H9M15 12H24M12 0V9M12 15V24", name: "Cursor" },
  centre: { path: "M8 8L16 16M16 8L8 16", name: "Chart centre" },
};

const PEG_ARROW: Readonly<Record<"above" | "below", string>> = { above: "↑", below: "↓" };

interface MapMarkProps {
  readonly kind: MarkKind;
  readonly view: MapView;
  readonly place: MarkPlace;
}

/**
 * A mark over a picture, at its point or, off the map, pegged to its edge with an arrow that says
 * which way the point lies, as the guide has an off-scale reading shown.
 */
function MapMark({ kind, view, place }: MapMarkProps) {
  const { path, name } = MARKS[kind];
  const fraction = rasterToScreen(view, place.fraction);
  const position = { left: `${fraction.left * 100}%`, top: `${fraction.top * 100}%` };
  return (
    <>
      <svg
        className={`map-view__mark map-view__mark--${kind}`}
        // A drawn mark with no text; its position is read out in the CURSOR panel.
        // oxlint-disable-next-line jsx-a11y/prefer-tag-over-role
        role="img"
        aria-label={place.peg === null ? name : `${name}, off the map ${place.peg}`}
        viewBox="0 0 24 24"
        style={position}
      >
        <path className="map-view__mark-casing" d={path} />
        <path className="map-view__mark-line" d={path} />
      </svg>
      {place.peg === null ? null : (
        <span className="map-view__peg" style={position} aria-hidden="true">
          {PEG_ARROW[place.peg]}
        </span>
      )}
    </>
  );
}

interface GalaxyMapViewProps {
  /** The universe whose galaxy is mapped. */
  readonly universe: UniverseIdHex;
  readonly view: MapView;
  readonly population: MapPopulation;
  /** The quantity the raster shows. */
  readonly quantity: MapQuantity;
  /** Whether the density is dimmed by the dust; read only under `SYSTEMS`. */
  readonly dustOverlay: boolean;
  /**
   * The picture's width in CSS pixels, which both views share so that they are at one scale; 0
   * before the display is laid out, when no map is asked for.
   */
  readonly pictureWidthPx: number;
  /** Device pixels in one CSS pixel, for the canvas's backing store. */
  readonly devicePixelRatio: number;
  /** The map cursor, shared by both views, in the `GALACTIC` frame. */
  readonly cursorLy: CentreLy;
  /** Moves the cursor, after a pick on the picture or an arrow key on it. */
  readonly onCursor: (cursorLy: CentreLy) => void;
  /** The chart's centre, marked with a small diagonal cross, or `null` before one is chosen. */
  readonly centreLy: CentreLy | null;
}

interface MapViewBodyProps extends GalaxyMapViewProps {
  /** Asks for the map afresh, from `PENDING`. */
  readonly onRetry: () => void;
}

/** The view itself, for one attempt at its map: {@link GalaxyMapView} starts a new one on `RETRY`. */
function MapViewBody({
  universe,
  view,
  population,
  quantity,
  dustOverlay: overlayAsked,
  pictureWidthPx,
  devicePixelRatio,
  cursorLy,
  onCursor,
  centreLy,
  onRetry,
}: MapViewBodyProps) {
  const titleId = useId();
  const hintId = useId();
  const pictureHeightPx = pictureWidthPx / SCREEN_ASPECT[view];
  const backingWidthPx = Math.round(pictureWidthPx * devicePixelRatio);
  const backingHeightPx = Math.round(pictureHeightPx * devicePixelRatio);
  // The narrowest map that, enlarged at most 1.25 times, covers the picture's device pixels.
  const resolution = mapResolutionFor(backingWidthPx);
  const overlay = quantity === "systems" && overlayAsked;
  const question: MapQuestion = { universe, view, population, quantity, overlay };
  const densityBody: RequestOf<"density_map"> | null =
    backingWidthPx > 0 && needsDensity(question)
      ? { kind: "density_map", universe, view, population, resolution, bits: MAP_BITS }
      : null;
  const extinctionBody: RequestOf<"extinction_map"> | null =
    backingWidthPx > 0 && needsExtinction(question)
      ? { kind: "extinction_map", universe, view, resolution, bits: MAP_BITS }
      : null;
  const densityState = useServerRequest<"density_map">(densityBody, MAP_TIMEOUT_MS);
  const extinctionState = useServerRequest<"extinction_map">(extinctionBody, MAP_TIMEOUT_MS);
  const densityMap = densityState.kind === "ok" ? densityState.response : null;
  const extinctionMap = extinctionState.kind === "ok" ? extinctionState.response : null;
  // What the status line reports: the first map the picture needs that has not answered.
  const densityWaits = needsDensity(question) && densityState.kind !== "ok";
  const state: RequestState<RequestKind> = densityWaits ? densityState : extinctionState;
  // With two maps asked for, the extinction map's state is named, so that one is told apart.
  const stateSubject = overlay && !densityWaits ? "EXTINCTION" : undefined;
  // Decoding 350 kB of base64 takes milliseconds, and the picture's identity decides when the
  // canvas is painted again, so it is kept until a map changes.
  const decoding = useMemo(
    () =>
      decodePicture(densityMap, extinctionMap, {
        universe,
        view,
        population,
        quantity,
        overlay,
      }),
    [densityMap, extinctionMap, universe, view, population, quantity, overlay],
  );
  const answered = decoding.kind === "ok" ? decoding.picture : null;

  // The last picture drawn stays, stale, while a map of another resolution for the same question is
  // on its way, or failed: a resize past a resolution step never blanks the view. A map of another
  // universe, view, population or quantity, or with the overlay put on or taken off, is other data,
  // and is waited for from `PENDING`.
  const [shown, setShown] = useState<ShownPicture | null>(null);
  if (answered !== null && shown?.picture !== answered) {
    setShown({ picture: answered, question });
  }
  const stale = answered === null && shown !== null && sameQuestion(shown.question, question);
  const picture = answered ?? (stale ? shown.picture : null);

  const rootRef = useRef<HTMLElement>(null);
  // Whether the picture has focus, which decides whether this view's density reading announces
  // (see `CursorReadings`): the arrow keys move the cursor only on the focused picture.
  const [focused, setFocused] = useState(false);
  // With no picture there is no canvas, and one removed while it had focus fires no `blur`, so the
  // flag is dropped here; otherwise the next picture's reading would announce without focus.
  if (picture === null && focused) {
    setFocused(false);
  }
  const [ramps, setRamps] = useState<Ramps | null>(null);
  // The tokens are read when the view is mounted and each time its display is shown again, when
  // `Activity` runs its effects anew; unchanged ramps keep their identity, and paint nothing.
  useLayoutEffect(() => {
    if (rootRef.current === null) {
      return;
    }
    const current = rampsAt(rootRef.current);
    setRamps((previous) =>
      previous !== null && sameRamps(previous, current) ? previous : current,
    );
  }, []);
  const ramp = ramps === null ? null : stale ? ramps.stale : ramps.current;
  // Where the backing store has room for every map pixel, the map's own pixels are painted once
  // and drawn larger without smoothing, which repeats pixels and drops none, and kept across a
  // resize that still has room; otherwise the map is reduced to the backing store, afresh for each
  // size.
  const hasRoom =
    picture !== null &&
    picture.onScreen.widthPx <= backingWidthPx &&
    picture.onScreen.heightPx <= backingHeightPx;
  const native = useMemo(
    () =>
      picture === null || ramp === null || !hasRoom
        ? null
        : {
            rgba: rasterise(picture.onScreen, ramp),
            widthPx: picture.onScreen.widthPx,
            heightPx: picture.onScreen.heightPx,
          },
    [picture, ramp, hasRoom],
  );
  const reduced = useMemo(
    () =>
      picture === null || ramp === null || hasRoom || backingWidthPx === 0 || backingHeightPx === 0
        ? null
        : reducedSource(picture, ramp, backingWidthPx, backingHeightPx),
    [picture, ramp, hasRoom, backingWidthPx, backingHeightPx],
  );
  const source = native ?? reduced;

  const canvasRef = useRef<HTMLCanvasElement>(null);
  // Under `Activity` the view's effects run again each time its display is shown. The picture may
  // have lost the focus while it was hidden without a `blur` reaching it, which is the browser's to
  // decide for an element that is not displayed, so whether it has focus is read afresh then.
  useLayoutEffect(() => {
    setFocused(canvasRef.current !== null && canvasRef.current === document.activeElement);
  }, []);
  const paintedRef = useRef<{
    readonly source: PictureSource;
    readonly canvas: HTMLCanvasElement | null;
  } | null>(null);
  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    if (canvas === null || source === null) {
      return;
    }
    let painted = paintedRef.current;
    if (painted?.source.rgba !== source.rgba) {
      painted = { source, canvas: sourceCanvas(source) };
      paintedRef.current = painted;
    }
    const context = canvas.getContext("2d");
    if (painted.canvas === null || context === null) {
      return;
    }
    // A pixel is what the server computed, or the mean of what it computed: no smoothing invents
    // values between them (D6).
    context.imageSmoothingEnabled = false;
    context.drawImage(painted.canvas, 0, 0, backingWidthPx, backingHeightPx);
  }, [source, backingWidthPx, backingHeightPx]);

  const pick = (event: MouseEvent<HTMLCanvasElement>): void => {
    if (picture === null) {
      return;
    }
    const rect = event.currentTarget.getBoundingClientRect();
    if (!(rect.width > 0 && rect.height > 0)) {
      return;
    }
    const { geometry } = picture;
    const onRaster = screenToRaster(view, {
      left: (event.clientX - rect.left) / rect.width,
      top: (event.clientY - rect.top) / rect.height,
    });
    const column = onRaster.left * geometry.widthPx - 0.5;
    const row = onRaster.top * geometry.heightPx - 0.5;
    onCursor(pickCursor(view, cursorLy, pixelToLy(geometry, column, row), geometry));
  };
  const step = (event: KeyboardEvent<HTMLCanvasElement>): void => {
    const onScreen = ARROW_KEYS[event.key];
    const modified = event.altKey || event.ctrlKey || event.metaKey;
    if (picture === null || onScreen === undefined || modified) {
      return;
    }
    const pixels = event.shiftKey ? LARGE_STEP_PX : 1;
    const direction = rasterDirection(view, onScreen);
    const moved = stepCursor(view, cursorLy, direction, pixels, picture.geometry);
    if (moved !== null) {
      // Only on the focused map: the page never scrolls under the cursor.
      event.preventDefault();
      onCursor(moved);
    }
  };
  const cursorPlace =
    picture === null ? null : markOnPicture(picture.geometry, cursorInView(view, cursorLy));
  const centrePlace =
    picture === null || centreLy === null
      ? null
      : markOnPicture(picture.geometry, cursorInView(view, centreLy));
  const axes = AXIS_LABELS[view];

  let status: ReactNode;
  switch (decoding.kind) {
    case "none":
      status = <RequestStatus state={state} onRetry={onRetry} subject={stateSubject} />;
      break;
    case "invalid":
      // A server fault, like a failed request, so it reads in caution and offers RETRY.
      status = (
        <StatusLine
          text={`MAP DATA INVALID: ${decoding.cause}`}
          standing="fault"
          action={{ label: "RETRY", onAction: onRetry }}
        />
      );
      break;
    case "ok":
      status = null;
      break;
  }

  let content: ReactNode = status;
  if (picture !== null) {
    content = (
      <div className="map-view__picture">
        <canvas
          ref={canvasRef}
          className="map-view__canvas"
          width={backingWidthPx}
          height={backingHeightPx}
          // A picture to pick from and move a cursor over with the arrow keys, which no native
          // element is; its title, axes and legend are text in the DOM around it. The rule
          // counts a canvas as interactive, which HTML does not; the plan names this role.
          // oxlint-disable-next-line jsx-a11y/no-interactive-element-to-noninteractive-role
          role="application"
          tabIndex={0}
          aria-label={`Galaxy map, ${VIEW_NAME[view]}${LAYER_NAME[layerKind(question)]}${stale ? ", stale" : ""}`}
          aria-describedby={hintId}
          onClick={pick}
          onKeyDown={step}
          onFocus={() => {
            setFocused(true);
          }}
          onBlur={() => {
            setFocused(false);
          }}
        />
        {centrePlace === null ? null : <MapMark kind="centre" view={view} place={centrePlace} />}
        {cursorPlace === null ? null : <MapMark kind="cursor" view={view} place={cursorPlace} />}
        <span className="map-view__axis map-view__axis--across">{axes.across}</span>
        <span className={`map-view__axis map-view__axis--${axes.end}`}>{axes.vertical}</span>
        {stale ? (
          // The guide's trailing stale mark, after the picture; the canvas's name says it too.
          <span className="stale-mark map-view__stale" aria-hidden="true">
            S
          </span>
        ) : null}
      </div>
    );
  }

  return (
    <section className={`map-view map-view--${view}`} aria-labelledby={titleId} ref={rootRef}>
      <div className="map-view__side">
        <div className="map-view__head">
          <h3 className="map-view__title" id={titleId}>
            {VIEW_TITLE[view]}
          </h3>
          {picture === null ? null : (
            <p className="map-view__hint" id={hintId}>
              {KEY_HINT[view]}
            </p>
          )}
        </div>
        {stale ? status : null}
        {view === "face_on" ? (
          <p className="map-view__rotation">ROTATION COUNTER-CLOCKWISE</p>
        ) : null}
        {picture === null ? null : (
          <CursorReadings
            rows={cursorRows(picture, view, cursorLy)}
            stale={stale}
            announce={focused}
          />
        )}
        {picture === null || ramp === null ? null : (
          <div
            className={`map-view__legends${
              picture.density !== null && picture.extinction !== null
                ? " map-view__legends--pair"
                : ""
            }`}
          >
            {picture.density === null ? null : (
              <DensityLegend
                ramp={ramp}
                floorLog10PerLy2={picture.density.floorLog10}
                ceilingLog10PerLy2={picture.density.ceilingLog10}
              />
            )}
            {picture.extinction === null ? null : (
              <ExtinctionLegend
                ramp={ramp}
                floorLog10Mag={picture.extinction.floorLog10}
                ceilingLog10Mag={picture.extinction.ceilingLog10}
              />
            )}
          </div>
        )}
      </div>
      <div className="map-view__area">{content}</div>
    </section>
  );
}

/**
 * One view of the galaxy map: the column density of systems, face-on or edge-on, as a raster on
 * the single-hue logarithmic ramp, with its title, axes and legend.
 *
 * @remarks
 * The quantity is the page's choice (plan 07, P07.T11.b). Under `SYSTEMS` the view asks for the
 * `density_map`, and with the dust overlay the `extinction_map` of the same resolution too: the
 * density's codes are then lowered by 0.4 A(V) in dex before the ramp ({@link dustOverlay}), both
 * legends are shown, side by side on a compact page (the page names the overlay once,
 * `DUST OVERLAY: A(V), WHOLE LINE OF SIGHT`, as the guide requires); the density read under the cursor stays the map's own, with A(V) beside
 * it. Under `EXTINCTION` it asks for the `extinction_map` alone and shows A(V) on the same ramp,
 * logarithmic in magnitudes, with `ExtinctionLegend`. Each map is checked as a density map is, and
 * waits, goes stale and fails as one does, below; a picture of two maps waits for both.
 *
 * The picture is `pictureWidthPx` wide, in its view's proportions. It asks for the narrowest
 * `density_map` at 8 bits (plan 05, design note D5) that, enlarged at most 1.25 times, covers the
 * picture's device pixels (`mapResolutionFor`), allowing it two minutes, and shows the request's state until it is answered, asking
 * again when the picture grows or shrinks past a resolution the protocol offers. Until that map
 * arrives the last picture stays, resampled to the new size and marked stale as the guide has a
 * stale value shown: painted on a ramp up to `--text-muted`, with a trailing `S` and its legend and
 * density reading likewise, the request's state beside it; a failure of the new request leaves it
 * so, with the failure and `RETRY`, which asks again from `PENDING`. A map of another universe or
 * population is other data and is waited for from `PENDING`. A map that is not the one asked for,
 * is not of the M1 extents, cannot be drawn or does not decode reads `MAP DATA INVALID` with the
 * cause, a server fault. After any failure `RETRY` asks again from `PENDING`: the view is mounted
 * afresh, since a request sent again would keep an invalid answer on show until its successor
 * arrived, and the focus goes to the page's tab, since `RETRY` goes too. The codes are painted with
 * the ramp from the `--surface-0` and `--text` tokens, read when the view is shown, and drawn
 * without smoothing (D6) onto a canvas whose backing store follows the device pixel ratio: pixel
 * for pixel or larger, or, where the map still has more pixels than the backing store, reduced by
 * area-weighted averaging of linear density, so that no map pixel is dropped. Painting happens when
 * the map, the size or the ramp changes, never on a loop.
 *
 * The face-on picture is the raster turned a quarter-turn clockwise, +x down and +y to the right,
 * as the spatial view's `TOP` shows the galaxy from the +x axis; the edge-on picture is x across and
 * z up, as `FRONT` shows it there. The axes are labelled beside the picture; the view's title, key
 * hint, rotation sense face-on, the density under the cursor and the view's own legend (the two
 * views have different floors and ceilings, plan 04's design note 12) stand in the column beside
 * it. The frame and the scale, which both views share, are the page's.
 *
 * The picture takes focus and carries the map cursor, shared with the other view: a click or tap
 * sets x and y face-on and z alone edge-on (plan 05, design note D19), and the arrow keys move it a
 * map pixel at a time as the screen shows the axes, ten with `Shift`, stopping at the edge pixels on
 * the cursor's own lattice of whole pixels (the orchestrator's rulings 28 and 35).
 * The cursor is drawn over the picture as a cross in the selection colour, and the column density
 * under it is read beside the picture: the value its code stands for, `BELOW FLOOR` at code 0, or
 * an em dash off the map. That reading announces its changes only while the picture has focus, so
 * that one arrow key gives one announcement and not one from each view, whose readings both follow
 * the shared cursor's x. The chart's centre is drawn as a small diagonal cross in `--text`. A mark
 * whose point lies above or below the map is pegged to its edge with `↑` or `↓`. Moving either
 * paints nothing.
 */
export function GalaxyMapView(props: GalaxyMapViewProps) {
  const [attempt, setAttempt] = useState(0);
  const focusPageTab = usePageTabFocus();
  return (
    <MapViewBody
      key={attempt}
      {...props}
      onRetry={() => {
        // RETRY goes with the body it mounts afresh, so the focus goes to the page's tab, which
        // stays, and not to the document's body (the orchestrator's ruling 18).
        focusPageTab?.();
        setAttempt((count) => count + 1);
      }}
    />
  );
}
