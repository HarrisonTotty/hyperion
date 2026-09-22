import {
  type DecodedDensityMap,
  decodeDensityMap,
  type DensityMap,
  type MapPopulation,
  type MapView,
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
import { StatusLine } from "../../components/StatusLine";
import { formatSci } from "../../lib/format";
import { type MapGeometry, mapGeometry, pixelToLy } from "../../lib/galaxy/mapGeometry";
import {
  mapResolutionFor,
  paintLevels,
  reducedLevels,
  turnClockwise,
} from "../../lib/galaxy/mapPicture";
import type { CentreLy } from "../../lib/galaxy/model";
import { buildRamp, type DecodedCodes, parseHexColour, rasterise } from "../../lib/galaxy/ramp";
import { useServerRequest } from "../../lib/useServerRequest";
import { DensityLegend } from "./DensityLegend";
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

/** A density map that decoded and whose geometry holds, ready to paint. */
interface MapPicture {
  readonly geometry: MapGeometry;
  readonly decoded: DecodedDensityMap;
  /** The codes turned as the screen shows them. */
  readonly onScreen: DecodedCodes;
  readonly floorLog10PerLy2: number;
  readonly ceilingLog10PerLy2: number;
}

/**
 * What became of a map's answer: none yet, invalid with its cause in words, or a picture to paint.
 */
type Decoding =
  | { readonly kind: "none" }
  | { readonly kind: "invalid"; readonly cause: string }
  | { readonly kind: "ok"; readonly picture: MapPicture };

const NO_MAP: Decoding = { kind: "none" };

/** What a map request asked for, which its answer must echo. */
interface MapQuestion {
  readonly universe: UniverseIdHex;
  readonly view: MapView;
  readonly population: MapPopulation;
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
function hasScreenShape(map: DensityMap): boolean {
  const onScreen = screenSizePx(map.view, { widthPx: map.width_px, heightPx: map.height_px });
  const acrossLy = onScreen.widthPx * map.ly_per_px;
  return (
    onScreen.widthPx === SCREEN_ASPECT[map.view] * onScreen.heightPx &&
    Math.abs(acrossLy - MAP_ACROSS_LY) <= SPAN_TOLERANCE * MAP_ACROSS_LY
  );
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
function decodeMap(map: DensityMap | null, question: MapQuestion): Decoding {
  if (map === null) {
    return NO_MAP;
  }
  if (
    map.universe !== question.universe ||
    map.view !== question.view ||
    map.population !== question.population
  ) {
    return invalid("not the map requested");
  }
  const sizeValid =
    Number.isInteger(map.width_px) &&
    Number.isInteger(map.height_px) &&
    map.width_px > 0 &&
    map.height_px > 0 &&
    isPositiveFinite(map.ly_per_px) &&
    map.centre_ly.every((ly) => Number.isFinite(ly)) &&
    hasScreenShape(map);
  if (!sizeValid) {
    return invalid("size or scale unusable");
  }
  const rangeValid =
    Number.isFinite(map.floor_log10_per_ly2) &&
    Number.isFinite(map.ceiling_log10_per_ly2) &&
    map.ceiling_log10_per_ly2 >= map.floor_log10_per_ly2;
  if (!rangeValid) {
    return invalid("density range unusable");
  }
  let decoded: DecodedDensityMap;
  try {
    decoded = decodeDensityMap(map);
  } catch {
    // Bad base64, a bit depth other than 8 or 16, or a byte count that does not fill the map: the
    // server's fault, reported to the operator in words.
    return invalid("pixel codes unreadable");
  }
  return {
    kind: "ok",
    picture: {
      geometry: mapGeometry(map),
      decoded,
      onScreen: SCREEN_TURN[map.view] === "clockwise" ? turnClockwise(decoded) : decoded,
      floorLog10PerLy2: map.floor_log10_per_ly2,
      ceilingLog10PerLy2: map.ceiling_log10_per_ly2,
    },
  };
}

function sameBytes(a: Uint8ClampedArray, b: Uint8ClampedArray): boolean {
  return a.length === b.length && a.every((byte, index) => byte === b[index]);
}

/** The ramp from the colour tokens in effect at `element`: `--surface-0` up to `--text`. */
function rampAt(element: Element): Uint8ClampedArray {
  const style = getComputedStyle(element);
  return buildRamp(
    parseHexColour(style.getPropertyValue("--surface-0")),
    parseHexColour(style.getPropertyValue("--text")),
  );
}

/** RGBA bytes to put on a canvas of their own size, which is then drawn at the backing size. */
interface PictureSource {
  readonly rgba: Uint8ClampedArray;
  readonly widthPx: number;
  readonly heightPx: number;
}

/**
 * The map reduced to a backing store of `widthPx` by `heightPx` device pixels, which is smaller
 * than it, by area-weighted averaging of linear density (see `mapPicture.ts`), so that no map pixel
 * is dropped.
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
  const levels = reducedLevels(
    onScreen,
    picture.ceilingLog10PerLy2 - picture.floorLog10PerLy2,
    reducedWidthPx,
    reducedHeightPx,
  );
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

/** The column density under the cursor on one view, as the operator reads it. */
type CursorDensity =
  | { readonly kind: "density"; readonly log10PerLy2: number }
  | { readonly kind: "below_floor" }
  | { readonly kind: "off_map" };

/**
 * The column density of the map pixel under the cursor.
 *
 * @remarks
 * It is the value the pixel's code stands for, which is the centre of the code's step, since plan
 * 04's quantiser rounds to the nearest code. The guide keeps `~` for estimated values, derived or
 * sensor-limited, and says nothing of a value rounded for the wire, which every shown value is at
 * some precision, so it carries no `~`.
 */
function densityUnder(picture: MapPicture, view: MapView, cursorLy: CentreLy): CursorDensity {
  const index = pixelIndexAt(picture.geometry, cursorInView(view, cursorLy));
  if (index === null) {
    return { kind: "off_map" };
  }
  const code = picture.decoded.codes[index];
  if (code === undefined) {
    throw new Error(`the decoded map has no code for its pixel ${index}`);
  }
  const log10PerLy2 = picture.decoded.log10PerLy2(code);
  return log10PerLy2 === null ? { kind: "below_floor" } : { kind: "density", log10PerLy2 };
}

interface CursorDensityReadingProps {
  readonly density: CursorDensity;
}

/** The column density under the cursor: a value, `BELOW FLOOR`, or an em dash off the map. */
function CursorDensityReading({ density }: CursorDensityReadingProps) {
  let value: ReactNode;
  switch (density.kind) {
    case "density":
      value = (
        <>
          <span className="field__value map-view__density">
            {formatSci(10 ** density.log10PerLy2)}
          </span>{" "}
          <span className="map-view__unit">SYSTEMS/ly²</span>
        </>
      );
      break;
    case "below_floor":
      value = <span>BELOW FLOOR</span>;
      break;
    case "off_map":
      value = <span className="readout__missing">—</span>;
      break;
  }
  return (
    // Read out as a whole when it changes, as it does on each arrow key on the map.
    <p className="field" aria-live="polite" aria-atomic="true">
      <span className="field__label">CURSOR DENSITY</span> {value}
    </p>
  );
}

/** The marks drawn over a map: the cursor, and the chart's centre. */
type MarkKind = "cursor" | "centre";

/**
 * Each mark's shape and name. The cursor is a thin cross with a gap at its point, in the selection
 * colour; the chart's centre four corner brackets in `--text`, a shape the cross cannot be taken
 * for. Both are cased in the background colour, so that they keep their contrast over the
 * brightest part of the map.
 */
const MARKS: Readonly<Record<MarkKind, { readonly path: string; readonly name: string }>> = {
  cursor: { path: "M0 12H9M15 12H24M12 0V9M12 15V24", name: "Cursor" },
  centre: { path: "M3 9V3H9M15 3H21V9M21 15V21H15M9 21H3V15", name: "Chart centre" },
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
  /** The chart's centre, marked with a bracket reticle, or `null` before one is chosen. */
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
  // The narrowest map that has a pixel for every device pixel of the picture's width.
  const resolution = mapResolutionFor(backingWidthPx);
  const body: RequestOf<"density_map"> | null =
    backingWidthPx > 0
      ? { kind: "density_map", universe, view, population, resolution, bits: MAP_BITS }
      : null;
  const state = useServerRequest<"density_map">(body, MAP_TIMEOUT_MS);
  const map = state.kind === "ok" ? state.response : null;
  // Decoding 350 kB of base64 takes milliseconds, and the picture's identity decides when the
  // canvas is painted again, so it is kept until the map changes.
  const decoding = useMemo(
    () => decodeMap(map, { universe, view, population }),
    [map, universe, view, population],
  );
  const picture = decoding.kind === "ok" ? decoding.picture : null;

  const rootRef = useRef<HTMLElement>(null);
  const [ramp, setRamp] = useState<Uint8ClampedArray | null>(null);
  // The tokens are read when the view is mounted and each time its display is shown again, when
  // `Activity` runs its effects anew; an unchanged ramp keeps its identity, and paints nothing.
  useLayoutEffect(() => {
    if (rootRef.current === null) {
      return;
    }
    const current = rampAt(rootRef.current);
    setRamp((previous) => (previous !== null && sameBytes(previous, current) ? previous : current));
  }, []);
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

  let content: ReactNode;
  switch (decoding.kind) {
    case "none":
      content = <RequestStatus state={state} onRetry={onRetry} />;
      break;
    case "invalid":
      // A server fault, like a failed request, so it reads in caution and offers RETRY.
      content = (
        <StatusLine
          text={`MAP DATA INVALID: ${decoding.cause}`}
          standing="fault"
          action={{ label: "RETRY", onAction: onRetry }}
        />
      );
      break;
    case "ok":
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
            aria-label={`Galaxy map, ${VIEW_NAME[view]}`}
            aria-describedby={hintId}
            onClick={pick}
            onKeyDown={step}
          />
          {centrePlace === null ? null : <MapMark kind="centre" view={view} place={centrePlace} />}
          {cursorPlace === null ? null : <MapMark kind="cursor" view={view} place={cursorPlace} />}
          <span className="map-view__axis map-view__axis--across">{axes.across}</span>
          <span className={`map-view__axis map-view__axis--${axes.end}`}>{axes.vertical}</span>
        </div>
      );
      break;
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
        {view === "face_on" ? (
          <p className="map-view__rotation">ROTATION COUNTER-CLOCKWISE</p>
        ) : null}
        {picture === null ? null : (
          <CursorDensityReading density={densityUnder(picture, view, cursorLy)} />
        )}
        {picture === null || ramp === null ? null : (
          <DensityLegend
            ramp={ramp}
            floorLog10PerLy2={picture.floorLog10PerLy2}
            ceilingLog10PerLy2={picture.ceilingLog10PerLy2}
          />
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
 * The picture is `pictureWidthPx` wide, in its view's proportions. It asks for the narrowest
 * `density_map` at 8 bits (plan 05, design note D5) that has a pixel for every device pixel across
 * the picture, allowing it two minutes, and shows the request's state until it is answered, asking
 * again when the picture grows or shrinks past a resolution the protocol offers. A map that is not
 * the one asked for, is not of the M1 extents, cannot be drawn or does not decode reads
 * `MAP DATA INVALID` with the cause, a server fault. After any failure `RETRY` asks again from
 * `PENDING`: the view is mounted afresh, since a request sent again would keep an invalid answer on
 * show until its successor arrived. The codes are painted with the ramp from the `--surface-0` and
 * `--text` tokens, read when the view is shown, and drawn without smoothing (D6) onto a canvas
 * whose backing store follows the device pixel ratio: pixel for pixel or larger, or, where the map
 * still has more pixels than the backing store, reduced by area-weighted averaging of linear
 * density, so that no map pixel is dropped. Painting happens when the map, the size or the ramp
 * changes, never on a loop.
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
 * map pixel at a time as the screen shows the axes, ten with `Shift`, within the map's edge pixels.
 * The cursor is drawn over the picture as a cross in the selection colour, and the column density
 * under it is read beside the picture: the value its code stands for, `BELOW FLOOR` at code 0, or
 * an em dash off the map. The chart's centre is drawn as a bracket reticle in `--text`. A mark whose
 * point lies above or below the map is pegged to its edge with `↑` or `↓`. Moving either paints
 * nothing.
 */
export function GalaxyMapView(props: GalaxyMapViewProps) {
  const [attempt, setAttempt] = useState(0);
  return (
    <MapViewBody
      key={attempt}
      {...props}
      onRetry={() => {
        setAttempt((count) => count + 1);
      }}
    />
  );
}
