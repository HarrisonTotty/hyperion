/**
 * The fields of a density map that place its pixels in the galaxy.
 *
 * @remarks
 * A structural subset of `DensityMap` from `@hyperion/protocol` (plan 04, design note 12), so a
 * map from the wire is passed as it is. `centre_ly` is (x, y) face-on and (x, z) edge-on.
 */
export interface MapRaster {
  readonly view: "face_on" | "edge_on";
  readonly width_px: number;
  readonly height_px: number;
  readonly centre_ly: readonly [number, number];
  readonly ly_per_px: number;
}

/** The span of one axis of a map, from edge to edge of the picture. */
export interface AxisRangeLy {
  readonly minLy: number;
  readonly maxLy: number;
}

/**
 * Where a density map lies in the `GALACTIC` frame.
 *
 * @remarks
 * The horizontal axis is x in both views, increasing to the right. The vertical axis is y face-on
 * (seen from the north) and z edge-on (looking along +y), increasing upwards, so row 0, at the
 * top, has the largest vertical value.
 */
export interface MapGeometry {
  readonly widthPx: number;
  readonly heightPx: number;
  readonly lyPerPx: number;
  readonly centreLy: readonly [number, number];
  readonly horizontal: AxisRangeLy;
  readonly vertical: AxisRangeLy;
  /** The galactic axis the vertical axis of the picture runs along. */
  readonly verticalAxis: "y" | "z";
}

/** A point of a map's plane, in light-years along its horizontal (x) and vertical axes. */
export interface MapPointLy {
  readonly horizontalLy: number;
  readonly verticalLy: number;
}

/**
 * A position on a map's picture in pixel units: column `i` and row `j` have their centre at
 * (`i`, `j`), and the picture spans −0.5 to `width − 0.5` and −0.5 to `height − 0.5`.
 */
export interface PixelPoint {
  readonly column: number;
  readonly row: number;
}

/** Where a density map's picture lies, from its centre, pixel size and dimensions. */
export function mapGeometry(map: MapRaster): MapGeometry {
  const [centreH, centreV] = map.centre_ly;
  const halfWidthLy = (map.width_px * map.ly_per_px) / 2;
  const halfHeightLy = (map.height_px * map.ly_per_px) / 2;
  return {
    widthPx: map.width_px,
    heightPx: map.height_px,
    lyPerPx: map.ly_per_px,
    centreLy: map.centre_ly,
    horizontal: { minLy: centreH - halfWidthLy, maxLy: centreH + halfWidthLy },
    vertical: { minLy: centreV - halfHeightLy, maxLy: centreV + halfHeightLy },
    verticalAxis: map.view === "face_on" ? "y" : "z",
  };
}

/**
 * The point of the galaxy at a position on the picture: the pixel's centre for whole `column`
 * and `row`.
 *
 * @remarks
 * horizontal = centre + (column + 0.5 − width ÷ 2) × ly per px, and
 * vertical = centre + (height ÷ 2 − row − 0.5) × ly per px, as the wire's `DensityMap` states.
 * Fractional positions are allowed, so a pointer anywhere in a pixel maps to where it is.
 */
export function pixelToLy(geometry: MapGeometry, column: number, row: number): MapPointLy {
  const [centreH, centreV] = geometry.centreLy;
  return {
    horizontalLy: centreH + (column + 0.5 - geometry.widthPx / 2) * geometry.lyPerPx,
    verticalLy: centreV + (geometry.heightPx / 2 - row - 0.5) * geometry.lyPerPx,
  };
}

/**
 * The position on the picture of a point of the galaxy: the inverse of {@link pixelToLy},
 * fractional, and clamped to the picture's edges.
 */
export function lyToPixel(
  geometry: MapGeometry,
  horizontalLy: number,
  verticalLy: number,
): PixelPoint {
  const [centreH, centreV] = geometry.centreLy;
  const column = (horizontalLy - centreH) / geometry.lyPerPx + geometry.widthPx / 2 - 0.5;
  const row = geometry.heightPx / 2 - 0.5 - (verticalLy - centreV) / geometry.lyPerPx;
  return {
    column: Math.min(geometry.widthPx - 0.5, Math.max(-0.5, column)),
    row: Math.min(geometry.heightPx - 0.5, Math.max(-0.5, row)),
  };
}
