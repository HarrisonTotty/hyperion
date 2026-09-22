/**
 * Where the map cursor is on each view of the galaxy map, how each view is turned on screen, and
 * how picking and the arrow keys move the cursor.
 *
 * @remarks
 * The cursor is a point of the `GALACTIC` frame in light-years. The face-on view shows and sets its
 * x and y; the edge-on view shows its x and z but sets only z, as the brainstorm has the edge-on
 * view pick a height (plan 05, design note D19). A pick or a step keeps the cursor within the
 * map's edge pixels, at most at their centres, so it never lies on the map's outer edge: the face-on
 * map's edges are the faces of the root cube, whose upper faces lie outside it (plan 01).
 *
 * Points, pixels and directions here are the raster's, as plan 04 sends it: x to the right, and y
 * (face-on) or z (edge-on) up. The screen shows the face-on raster turned a quarter-turn clockwise,
 * so that +x runs down and +y to the right, as the spatial view's `TOP` preset shows the galaxy
 * from a point on the +x axis, coreward up and spinward to the right; the edge-on raster is shown
 * as sent, x across and z up, as `FRONT` shows it there. The turn keeps the handedness, so the
 * galaxy still turns counter-clockwise on screen.
 */
import type { MapView } from "@hyperion/protocol";

import { type CentreLy, ROOT_CUBE_HALF_LY } from "../../lib/galaxy/model";
import { lyToPixel, type MapGeometry, type MapPointLy } from "../../lib/galaxy/mapGeometry";

/**
 * The width every map spans across the screen, in light-years: the root cube's edge, which is the
 * face-on map's side and the edge-on map's width in M1 (plan 04, design note 12). Both pictures
 * are drawn at one width, and so at one scale.
 */
export const MAP_ACROSS_LY = 2 * ROOT_CUBE_HALF_LY;

/**
 * Each view's picture on screen, width over height: face-on square, edge-on twice as wide as it is
 * tall (plan 04, design note 12).
 */
export const SCREEN_ASPECT: Readonly<Record<MapView, number>> = { face_on: 1, edge_on: 2 };

/** An arrow key's direction on a map: right and up are the positive directions of its axes. */
export type ArrowDirection = "left" | "right" | "up" | "down";

/** How a view's raster is turned on screen. */
export type ScreenTurn = "none" | "clockwise";

/**
 * Each view's turn: face-on a quarter-turn clockwise (+x down, +y right, as `TOP`), edge-on none
 * (x across, z up, as `FRONT`).
 */
export const SCREEN_TURN: Readonly<Record<MapView, ScreenTurn>> = {
  face_on: "clockwise",
  edge_on: "none",
};

/** A view's picture size in map pixels as it stands on screen, turned. */
export function screenSizePx(
  view: MapView,
  geometry: Pick<MapGeometry, "widthPx" | "heightPx">,
): { readonly widthPx: number; readonly heightPx: number } {
  return SCREEN_TURN[view] === "clockwise"
    ? { widthPx: geometry.heightPx, heightPx: geometry.widthPx }
    : { widthPx: geometry.widthPx, heightPx: geometry.heightPx };
}

/**
 * Where a place on the raster, as fractions of its width (from the left) and height (from the
 * top), stands on the screen's picture.
 */
export function rasterToScreen(view: MapView, fraction: PictureFraction): PictureFraction {
  return SCREEN_TURN[view] === "clockwise"
    ? { left: 1 - fraction.top, top: fraction.left }
    : fraction;
}

/** The place on the raster under a place on the screen's picture: {@link rasterToScreen} undone. */
export function screenToRaster(view: MapView, fraction: PictureFraction): PictureFraction {
  return SCREEN_TURN[view] === "clockwise"
    ? { left: fraction.top, top: 1 - fraction.left }
    : fraction;
}

/** Each arrow key's direction on the raster when the picture is turned a quarter-turn clockwise. */
const ON_CLOCKWISE_TURN: Readonly<Record<ArrowDirection, ArrowDirection>> = {
  right: "up",
  left: "down",
  down: "right",
  up: "left",
};

/**
 * The direction on the raster of an arrow key pressed on a view's picture: on the turned face-on
 * picture right is +y (the raster's up) and down is +x (its right).
 */
export function rasterDirection(view: MapView, onScreen: ArrowDirection): ArrowDirection {
  return SCREEN_TURN[view] === "clockwise" ? ON_CLOCKWISE_TURN[onScreen] : onScreen;
}

/** The arrow keys, by `KeyboardEvent.key`. */
export const ARROW_KEYS: Readonly<Record<string, ArrowDirection>> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
};

/** Map pixels a step with `Shift` held moves the cursor. */
export const LARGE_STEP_PX = 10;

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}

/** The cursor's point in a view's plane: (x, y) face-on, (x, z) edge-on. */
export function cursorInView(view: MapView, cursorLy: CentreLy): MapPointLy {
  const [xLy, yLy, zLy] = cursorLy;
  return { horizontalLy: xLy, verticalLy: view === "face_on" ? yLy : zLy };
}

/** Whether a point of a view's plane lies within the map's extent, edges included. */
export function withinExtent(geometry: MapGeometry, point: MapPointLy): boolean {
  return (
    point.horizontalLy >= geometry.horizontal.minLy &&
    point.horizontalLy <= geometry.horizontal.maxLy &&
    point.verticalLy >= geometry.vertical.minLy &&
    point.verticalLy <= geometry.vertical.maxLy
  );
}

/**
 * The cursor after a pick at `point` on a view: x and y from the face-on view, z alone from the
 * edge-on one, kept between the centres of the map's edge pixels.
 */
export function pickCursor(
  view: MapView,
  cursorLy: CentreLy,
  point: MapPointLy,
  geometry: MapGeometry,
): CentreLy {
  const [xLy, yLy] = cursorLy;
  const halfPixelLy = geometry.lyPerPx / 2;
  const horizontalLy = clamp(
    point.horizontalLy,
    geometry.horizontal.minLy + halfPixelLy,
    geometry.horizontal.maxLy - halfPixelLy,
  );
  const verticalLy = clamp(
    point.verticalLy,
    geometry.vertical.minLy + halfPixelLy,
    geometry.vertical.maxLy - halfPixelLy,
  );
  return view === "face_on" ? [horizontalLy, verticalLy, cursorLy[2]] : [xLy, yLy, verticalLy];
}

/**
 * The cursor after an arrow key on a view, one map pixel a step, kept as a pick is; or `null` for a
 * direction the view does not move, left and right edge-on.
 *
 * @param direction - The direction on the raster, from {@link rasterDirection} for a key on screen.
 */
export function stepCursor(
  view: MapView,
  cursorLy: CentreLy,
  direction: ArrowDirection,
  pixels: number,
  geometry: MapGeometry,
): CentreLy | null {
  const stepLy = pixels * geometry.lyPerPx;
  const point = cursorInView(view, cursorLy);
  let moved: MapPointLy;
  switch (direction) {
    case "left":
    case "right":
      if (view === "edge_on") {
        return null;
      }
      moved = {
        horizontalLy: point.horizontalLy + (direction === "right" ? stepLy : -stepLy),
        verticalLy: point.verticalLy,
      };
      break;
    case "up":
    case "down":
      moved = {
        horizontalLy: point.horizontalLy,
        verticalLy: point.verticalLy + (direction === "up" ? stepLy : -stepLy),
      };
      break;
  }
  return pickCursor(view, cursorLy, moved, geometry);
}

/** A place on a picture, as fractions of its width from the left and of its height from the top. */
export interface PictureFraction {
  readonly left: number;
  readonly top: number;
}

/**
 * Where a mark is drawn on the raster: at its point, or pegged to the top or bottom edge when the
 * point lies above or below the map, as a typed z beyond the edge-on map's ±32,768 ly can. Only
 * the edge-on map, which is not turned, can peg a mark: the face-on map's vertical axis, y, spans
 * the whole root cube, where every cursor lies.
 */
export interface MarkPlace {
  readonly fraction: PictureFraction;
  /** The edge the mark is pegged to, or `null` when it is at its point. */
  readonly peg: "above" | "below" | null;
}

/**
 * Where a mark for the point is drawn on the picture, or `null` when the point lies beyond the
 * map's sides, which no point in the root cube does.
 */
export function markOnPicture(geometry: MapGeometry, point: MapPointLy): MarkPlace | null {
  const { horizontal, vertical } = geometry;
  if (point.horizontalLy < horizontal.minLy || point.horizontalLy > horizontal.maxLy) {
    return null;
  }
  let peg: MarkPlace["peg"] = null;
  if (point.verticalLy > vertical.maxLy) {
    peg = "above";
  } else if (point.verticalLy < vertical.minLy) {
    peg = "below";
  }
  // `lyToPixel` clamps to the picture's edge, which is where a pegged mark belongs.
  const { column, row } = lyToPixel(geometry, point.horizontalLy, point.verticalLy);
  return {
    fraction: {
      left: (column + 0.5) / geometry.widthPx,
      top: (row + 0.5) / geometry.heightPx,
    },
    peg,
  };
}

/**
 * The map pixel a point lies in, as its index in the codes, or `null` outside the map's extent.
 *
 * @remarks
 * A point on the edge between two pixels lies in the one to its right or below, and a point on the
 * map's far edge in its last pixel.
 */
export function pixelIndexAt(geometry: MapGeometry, point: MapPointLy): number | null {
  if (!withinExtent(geometry, point)) {
    return null;
  }
  const { column, row } = lyToPixel(geometry, point.horizontalLy, point.verticalLy);
  const wholeColumn = clamp(Math.round(column), 0, geometry.widthPx - 1);
  const wholeRow = clamp(Math.round(row), 0, geometry.heightPx - 1);
  return wholeRow * geometry.widthPx + wholeColumn;
}
