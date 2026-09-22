/**
 * Where a spatial view's furniture goes over its canvas: the axis triad, the core arrow and the
 * labels of its spheres and rings, kept inside the view and off each other.
 *
 * @remarks
 * Pure layout, in CSS pixels from the view's top left unless a name says `rem`. Text boxes are
 * estimated from their lengths ({@link textSizeRem}), since text cannot be measured before it is
 * laid out. The triad is placed first, in its corner; the core arrow stops short of it; and the
 * curve labels are fitted round both.
 */

import { type CameraAngles, viewBasis, type ViewBasis, type Viewport } from "./camera";
import type { CircleLabel, CurveLabel, RingLabel, ScreenPoint } from "./drawList";
import type { LocalFrame } from "./frame";
import { type BoxPx, halfExtentRem, type TextSizeRem, textSizeRem } from "./labels";
import { dot, type Vec3 } from "./vec3";

/** The letter spacing of every label over a view, in `em`, as the stylesheet sets it. */
export const OVERLAY_LETTER_SPACING_EM = 0.1;

export type { BoxPx };

/** Whether two boxes share any area. */
export function boxesOverlap(a: BoxPx, b: BoxPx): boolean {
  return (
    a.leftPx < b.leftPx + b.widthPx &&
    b.leftPx < a.leftPx + a.widthPx &&
    a.topPx < b.topPx + b.heightPx &&
    b.topPx < a.topPx + a.heightPx
  );
}

/** Whether a box lies wholly inside the view, `edgePx` in from its sides. */
function insideView(box: BoxPx, viewport: Viewport, edgePx: number): boolean {
  return (
    box.leftPx >= edgePx &&
    box.topPx >= edgePx &&
    box.leftPx + box.widthPx <= viewport.widthPx - edgePx &&
    box.topPx + box.heightPx <= viewport.heightPx - edgePx
  );
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

/** The box of a line of overlay text, in pixels. */
function textBoxPx(size: TextSizeRem, remPx: number): { widthPx: number; heightPx: number } {
  return { widthPx: size.widthRem * remPx, heightPx: size.heightRem * remPx };
}

/** The space kept between furniture and the edge of the view, in `rem`. */
const EDGE_REM = 0.25;

function edgeInsetPx(viewport: Viewport): number {
  return EDGE_REM * viewport.remPx;
}

// The axis triad.

/** The triad's box in the view's bottom left-hand corner, in `rem`, its origin at the centre. */
export const TRIAD_BOX_REM = { width: 15, height: 8 } as const;
/** The length of an axis seen side on, in `rem`. */
const AXIS_REM = 1.75;
/** The radius of the triad's away and towards symbols, in `rem`. */
export const TRIAD_MARKER_REM = 0.25;
/** The space between an axis's end and its label, in `rem`. */
const LABEL_GAP_REM = 0.25;
/**
 * The share of its length below which an axis is taken as seen end on: its label goes beside its
 * symbol, away from the other two axes, rather than beyond its end.
 */
const END_ON_SHARE = 0.25;
/**
 * How far a label that would cover another, and cannot go a line below or above it, is moved out
 * along its axis at each step, in `rem`.
 */
const NUDGE_REM = 0.25;
const MAX_NUDGES = 24;
/** A depth component smaller than this is an axis in the plane of the screen. */
const IN_SCREEN_DEPTH = 1e-9;

/** How an axis ends: towards the viewer, away from them, or across the screen. */
export type AxisEnd = "towards" | "away" | "across";

/** A point or a direction on the screen in `rem`, y downwards. */
export interface ScreenRem {
  readonly x: number;
  readonly y: number;
}

/** One of the frame's directions as the triad draws it, in `rem` from the triad's origin. */
export interface TriadAxis {
  readonly name: "coreward" | "spinward" | "north";
  readonly label: string;
  readonly tip: ScreenRem;
  readonly end: AxisEnd;
  /** Where the label's centre goes. */
  readonly labelAt: ScreenRem;
  readonly labelSize: TextSizeRem;
}

function onScreen(vector: Vec3, basis: ViewBasis): ScreenRem {
  return { x: dot(vector, basis.right), y: -dot(vector, basis.up) };
}

function length(point: ScreenRem): number {
  return Math.hypot(point.x, point.y);
}

function unit(point: ScreenRem): ScreenRem {
  const size = length(point);
  return { x: point.x / size, y: point.y / size };
}

function along(from: ScreenRem, direction: ScreenRem, distance: number): ScreenRem {
  return { x: from.x + direction.x * distance, y: from.y + direction.y * distance };
}

function axisEnd(depth: number): AxisEnd {
  if (depth < -IN_SCREEN_DEPTH) {
    return "towards";
  }
  return depth > IN_SCREEN_DEPTH ? "away" : "across";
}

/** A label's box about its centre, in `rem`, as a pixel box at 1 px a rem. */
function remBox(centre: ScreenRem, size: TextSizeRem): BoxPx {
  return {
    leftPx: centre.x - size.widthRem / 2,
    topPx: centre.y - size.heightRem / 2,
    widthPx: size.widthRem,
    heightPx: size.heightRem,
  };
}

/**
 * The triad's axes and labels for a frame seen from `angles` (plan 05, T10.f).
 *
 * @remarks
 * A label goes beyond its axis's end; an axis seen end on has its label beside its symbol, on the
 * side away from the other two axes; a label that would cover another goes a line below or above
 * it, or further out along its axis. Every label is kept inside the triad's box. On the galactic
 * axis the directions are labelled `−X` and `+Y`, as the frame falls back to them (design note
 * D11).
 */
export function triadLayout(frame: LocalFrame, angles: CameraAngles): ReadonlyArray<TriadAxis> {
  const basis = viewBasis(frame, angles);
  const labels = frame.onAxis
    ? { coreward: "−X", spinward: "+Y", north: "NORTH" }
    : { coreward: "COREWARD", spinward: "SPINWARD", north: "NORTH" };
  const axes = (["coreward", "spinward", "north"] as const).map((name) => ({
    name,
    vector: frame[name],
    projected: onScreen(frame[name], basis),
  }));
  const placed: BoxPx[] = [];
  return axes.map(({ name, vector, projected }): TriadAxis => {
    const tip = { x: projected.x * AXIS_REM, y: projected.y * AXIS_REM };
    const end = axisEnd(dot(vector, basis.forward));
    const others = axes
      .filter((other) => other.name !== name)
      .reduce((sum, other) => ({ x: sum.x + other.projected.x, y: sum.y + other.projected.y }), {
        x: 0,
        y: 0,
      });
    let direction: ScreenRem;
    if (length(projected) >= END_ON_SHARE) {
      direction = unit(projected);
    } else if (length(others) > 0) {
      direction = unit({ x: -others.x, y: -others.y });
    } else {
      direction = unit({ x: -1, y: 1 });
    }
    const size = textSizeRem(labels[name], OVERLAY_LETTER_SPACING_EM);
    const clear = (end === "across" ? 0 : TRIAD_MARKER_REM) + LABEL_GAP_REM;
    const base = along(tip, direction, clear + halfExtentRem(direction, size));
    const lineRem = size.heightRem + LABEL_GAP_REM / 2;
    // Kept inside the triad's box, whose origin is its centre.
    const inBox = (centre: ScreenRem): ScreenRem => ({
      x: clamp(
        centre.x,
        -TRIAD_BOX_REM.width / 2 + size.widthRem / 2,
        TRIAD_BOX_REM.width / 2 - size.widthRem / 2,
      ),
      y: clamp(
        centre.y,
        -TRIAD_BOX_REM.height / 2 + size.heightRem / 2,
        TRIAD_BOX_REM.height / 2 - size.heightRem / 2,
      ),
    });
    // Beyond the axis's end if that is clear; else the nearest clear place about it, a line or two
    // up or down and half a label or a label across, as when two axes run the same way on the
    // screen; else further out along the axis.
    const shifts = [0, 1, -1, 2, -2].flatMap((lines) =>
      [0, 0.5, -0.5, 1, -1].map((widths) => ({
        x: base.x + widths * (size.widthRem + LABEL_GAP_REM),
        y: base.y + lines * lineRem,
      })),
    );
    const candidates = [
      ...shifts.toSorted(
        (a, b) => Math.hypot(a.x - base.x, a.y - base.y) - Math.hypot(b.x - base.x, b.y - base.y),
      ),
      ...Array.from({ length: MAX_NUDGES }, (_, index) =>
        along(base, direction, (index + 1) * NUDGE_REM),
      ),
    ].map(inBox);
    const labelAt =
      candidates.find(
        (centre) => !placed.some((other) => boxesOverlap(remBox(centre, size), other)),
      ) ?? inBox(base);
    placed.push(remBox(labelAt, size));
    return { name, label: labels[name], tip, end, labelAt, labelSize: size };
  });
}

/**
 * The part of the view the triad covers, its axes, symbols and labels, in pixels: what the core
 * arrow and the curve labels keep clear of.
 */
export function triadFootprintPx(axes: ReadonlyArray<TriadAxis>, viewport: Viewport): BoxPx {
  let minX = -TRIAD_MARKER_REM;
  let minY = -TRIAD_MARKER_REM;
  let maxX = TRIAD_MARKER_REM;
  let maxY = TRIAD_MARKER_REM;
  for (const axis of axes) {
    minX = Math.min(
      minX,
      axis.tip.x - TRIAD_MARKER_REM,
      axis.labelAt.x - axis.labelSize.widthRem / 2,
    );
    maxX = Math.max(
      maxX,
      axis.tip.x + TRIAD_MARKER_REM,
      axis.labelAt.x + axis.labelSize.widthRem / 2,
    );
    minY = Math.min(
      minY,
      axis.tip.y - TRIAD_MARKER_REM,
      axis.labelAt.y - axis.labelSize.heightRem / 2,
    );
    maxY = Math.max(
      maxY,
      axis.tip.y + TRIAD_MARKER_REM,
      axis.labelAt.y + axis.labelSize.heightRem / 2,
    );
  }
  const remPx = viewport.remPx;
  const originX = (TRIAD_BOX_REM.width / 2) * remPx;
  const originY = viewport.heightPx - (TRIAD_BOX_REM.height / 2) * remPx;
  return {
    leftPx: originX + minX * remPx,
    topPx: originY + minY * remPx,
    widthPx: (maxX - minX) * remPx,
    heightPx: (maxY - minY) * remPx,
  };
}

// The core arrow.

/** How far inside the edge of the view the core arrow's head stands, in `rem`. */
const RIM_INSET_REM = 0.5;
/** The core arrow's length, head included, and its head's, in `rem`. */
export const CORE_ARROW_REM = 1.5;
export const CORE_HEAD_REM = 0.375;
/**
 * The radius of the core's away and towards symbols, in `rem`: 1.25 rem across, larger than any
 * mark's symbol (at most 1 rem, D14), so that neither is taken for a system.
 */
export const CORE_SYMBOL_REM = 0.625;
/** The space between the arrow and its label, and between the arrow and other furniture, in `rem`. */
const CORE_GAP_REM = 0.25;
/**
 * The angle, in degrees, within which coreward is taken as along the line of sight, where the
 * arrow would have no direction on the screen and becomes the away or towards symbol.
 */
const LINE_OF_SIGHT_DEG = 5;
/** How many half-lines up and down the core arrow's label may move to keep clear of furniture. */
const MAX_LABEL_STEPS = 16;

/** Where the core arrow and its label go. */
export type CoreArrowLayout =
  | {
      /** An arrow whose head stands at `tip`, pointing along the projected coreward direction. */
      readonly kind: "arrow";
      readonly tail: ScreenPoint;
      readonly tip: ScreenPoint;
      readonly label: BoxPx;
    }
  | {
      /** Coreward along the line of sight: the away or towards symbol about `centre`. */
      readonly kind: "away" | "towards";
      readonly centre: ScreenPoint;
      readonly label: BoxPx;
    }
  | {
      /** On the galactic axis, where there is no coreward: nothing is drawn. */
      readonly kind: "undefined";
    };

/**
 * How far a ray from `from` along the unit `direction` runs before it enters `box`, or `Infinity`
 * when it misses it or starts inside it.
 */
function distanceIntoBox(from: ScreenPoint, direction: ScreenRem, box: BoxPx): number {
  const inside =
    from.xPx >= box.leftPx &&
    from.xPx <= box.leftPx + box.widthPx &&
    from.yPx >= box.topPx &&
    from.yPx <= box.topPx + box.heightPx;
  if (inside) {
    return Infinity;
  }
  let enter = 0;
  let leave = Infinity;
  const slabs: ReadonlyArray<readonly [number, number, number]> = [
    [from.xPx, direction.x, box.leftPx],
    [from.yPx, direction.y, box.topPx],
  ];
  for (const [index, [start, step, low]] of slabs.entries()) {
    const high = low + (index === 0 ? box.widthPx : box.heightPx);
    if (step === 0) {
      if (start < low || start > high) {
        return Infinity;
      }
      continue;
    }
    const a = (low - start) / step;
    const b = (high - start) / step;
    enter = Math.max(enter, Math.min(a, b));
    leave = Math.min(leave, Math.max(a, b));
  }
  return enter <= leave ? enter : Infinity;
}

function grow(box: BoxPx, byPx: number): BoxPx {
  return {
    leftPx: box.leftPx - byPx,
    topPx: box.topPx - byPx,
    widthPx: box.widthPx + 2 * byPx,
    heightPx: box.heightPx + 2 * byPx,
  };
}

/** A box of `size` beside `point`, on `side`, `clearPx` from it, kept inside the view. */
function besidePoint(
  point: ScreenPoint,
  side: ScreenRem,
  clearPx: number,
  size: TextSizeRem,
  viewport: Viewport,
): BoxPx {
  const { widthPx, heightPx } = textBoxPx(size, viewport.remPx);
  const reach = clearPx + halfExtentRem(side, size) * viewport.remPx;
  const edgePx = edgeInsetPx(viewport);
  const centreX = clamp(
    point.xPx + side.x * reach,
    edgePx + widthPx / 2,
    viewport.widthPx - edgePx - widthPx / 2,
  );
  const centreY = clamp(
    point.yPx + side.y * reach,
    edgePx + heightPx / 2,
    viewport.heightPx - edgePx - heightPx / 2,
  );
  return { leftPx: centreX - widthPx / 2, topPx: centreY - heightPx / 2, widthPx, heightPx };
}

/**
 * Where the core arrow goes (plan 05, T10.f): its head where the projected coreward direction
 * leaves the view, or, sooner, where it would run into other furniture; its label beside it.
 *
 * @remarks
 * The label goes on the left of an arrow that runs up or down and below one that runs across, or
 * on the other side when that would cover other furniture, and inside the view. Within 5° of the
 * line of sight the arrow becomes the away or towards symbol at the top of the view, labelled on
 * its left.
 *
 * @param labelText - The label's text, as {@link coreLabelText} writes it.
 * @param obstacles - Furniture the arrow and its label keep clear of, such as the triad.
 */
export function coreArrowLayout(
  frame: LocalFrame,
  angles: CameraAngles,
  viewport: Viewport,
  labelText: string,
  obstacles: ReadonlyArray<BoxPx>,
): CoreArrowLayout {
  if (frame.onAxis) {
    return { kind: "undefined" };
  }
  const remPx = viewport.remPx;
  const basis = viewBasis(frame, angles);
  const projected = onScreen(frame.coreward, basis);
  const reach = length(projected);
  const size = textSizeRem(labelText, OVERLAY_LETTER_SPACING_EM);
  const clearOf = (box: BoxPx): boolean => !obstacles.some((other) => boxesOverlap(box, other));

  if (reach < Math.sin((LINE_OF_SIGHT_DEG * Math.PI) / 180)) {
    const centre = { xPx: viewport.widthPx / 2, yPx: (RIM_INSET_REM + CORE_SYMBOL_REM) * remPx };
    const clearPx = (CORE_SYMBOL_REM + CORE_GAP_REM) * remPx;
    return {
      kind: dot(frame.coreward, basis.forward) > 0 ? "away" : "towards",
      centre,
      label: besidePoint(centre, { x: -1, y: 0 }, clearPx, size, viewport),
    };
  }

  const direction = unit(projected);
  const insetPx = RIM_INSET_REM * remPx;
  const centre = { xPx: viewport.widthPx / 2, yPx: viewport.heightPx / 2 };
  const toRim = Math.min(
    Math.abs(direction.x) > 0
      ? Math.max(0, viewport.widthPx / 2 - insetPx) / Math.abs(direction.x)
      : Infinity,
    Math.abs(direction.y) > 0
      ? Math.max(0, viewport.heightPx / 2 - insetPx) / Math.abs(direction.y)
      : Infinity,
  );
  // Furniture is grown by the arrowhead's half-width and a gap, so that the whole arrow stays clear.
  const clearancePx = (CORE_HEAD_REM + CORE_GAP_REM) * remPx;
  const toFurniture = Math.min(
    ...obstacles.map((box) => distanceIntoBox(centre, direction, grow(box, clearancePx))),
  );
  const tipReach = Math.min(toRim, toFurniture);
  const tip = {
    xPx: centre.xPx + direction.x * tipReach,
    yPx: centre.yPx + direction.y * tipReach,
  };
  const arrowPx = CORE_ARROW_REM * remPx;
  const tail = { xPx: tip.xPx - direction.x * arrowPx, yPx: tip.yPx - direction.y * arrowPx };
  const middle = { xPx: (tip.xPx + tail.xPx) / 2, yPx: (tip.yPx + tail.yPx) / 2 };

  const across = { x: -direction.y, y: direction.x };
  const mostlyVertical = Math.abs(direction.y) >= Math.abs(direction.x);
  const flip = mostlyVertical ? across.x > 0 : across.y < 0;
  const preferred = flip ? { x: -across.x, y: -across.y } : across;
  const clearPx = (CORE_HEAD_REM + CORE_GAP_REM) * remPx;
  const sides = [
    besidePoint(middle, preferred, clearPx, size, viewport),
    besidePoint(middle, { x: -preferred.x, y: -preferred.y }, clearPx, size, viewport),
    // Beyond the tail, towards the view's centre.
    besidePoint(tail, { x: -direction.x, y: -direction.y }, CORE_GAP_REM * remPx, size, viewport),
  ];
  // Else the nearest of those moved up or down, clear of the furniture and inside the view.
  const stepPx = (size.heightRem / 2) * remPx;
  const moved = Array.from({ length: 2 * MAX_LABEL_STEPS }, (_, index) => {
    const steps = Math.floor(index / 2) + 1;
    return (index % 2 === 0 ? -1 : 1) * steps * stepPx;
  }).flatMap((dyPx) =>
    sides.map((box) => ({
      ...box,
      topPx: clamp(
        box.topPx + dyPx,
        edgeInsetPx(viewport),
        viewport.heightPx - edgeInsetPx(viewport) - box.heightPx,
      ),
    })),
  );
  const first = sides[0] ?? besidePoint(middle, preferred, clearPx, size, viewport);
  const label = [...sides, ...moved].find(clearOf) ?? first;
  return { kind: "arrow", tail, tip, label };
}

/** The core arrow's label: `CORE`, the distance to the axis and its unit. */
export function coreLabelText(distance: { readonly value: string; readonly unit: string }): string {
  return `CORE ${distance.value}${distance.unit.length > 0 ? ` ${distance.unit}` : ""}`;
}

/** The boxes the core arrow covers, its mark and its label, for other furniture to keep clear of. */
export function coreArrowBoxes(layout: CoreArrowLayout, remPx: number): ReadonlyArray<BoxPx> {
  let boxes: ReadonlyArray<BoxPx>;
  switch (layout.kind) {
    case "arrow": {
      const halfPx = CORE_HEAD_REM * remPx;
      boxes = [
        {
          leftPx: Math.min(layout.tip.xPx, layout.tail.xPx) - halfPx,
          topPx: Math.min(layout.tip.yPx, layout.tail.yPx) - halfPx,
          widthPx: Math.abs(layout.tip.xPx - layout.tail.xPx) + 2 * halfPx,
          heightPx: Math.abs(layout.tip.yPx - layout.tail.yPx) + 2 * halfPx,
        },
        layout.label,
      ];
      break;
    }
    case "away":
    case "towards": {
      const radiusPx = CORE_SYMBOL_REM * remPx;
      boxes = [
        {
          leftPx: layout.centre.xPx - radiusPx,
          topPx: layout.centre.yPx - radiusPx,
          widthPx: 2 * radiusPx,
          heightPx: 2 * radiusPx,
        },
        layout.label,
      ];
      break;
    }
    case "undefined":
      boxes = [];
      break;
  }
  return boxes;
}

// The labels of spheres and rings.

/** The room kept between a curve and its label, for a sphere's ticks, in `rem`. */
const CURVE_CLEAR_REM = 0.375;
/** How far to the side of the point it names a curve's label starts, in `rem`. */
const CURVE_SIDE_REM = 0.5;
/**
 * The points on a sphere's circle tried for its labels, as screen angles in degrees from the right,
 * counter-clockwise: the top first, then round both ways.
 */
const CIRCLE_ANGLES_DEG = [90, 60, 120, 45, 135, 30, 150, 0, 180, -30, -150, -60, -120, -90];

/** A curve's label where the view sets it: its box, estimated, in pixels. */
export interface PlacedCurveLabel extends BoxPx {
  readonly key: string;
  readonly text: string;
}

/** The labels of one sphere, set one above another. */
interface CircleBlock {
  readonly labels: ReadonlyArray<CircleLabel>;
  readonly centre: ScreenPoint;
  readonly radiusPx: number;
}

/**
 * Where a block of `widthPx` by `heightPx` goes beside a point on a circle at `angleDeg`: outside
 * the circle, or inside it, clear of the circle's line and ticks.
 */
function blockAtAngle(
  block: CircleBlock,
  angleDeg: number,
  side: "outside" | "inside",
  widthPx: number,
  heightPx: number,
  remPx: number,
): BoxPx {
  const angle = (angleDeg * Math.PI) / 180;
  const cos = Math.abs(angleDeg % 180) === 90 ? 0 : Math.cos(angle);
  const sin = Math.abs(angleDeg % 180) === 0 ? 0 : Math.sin(angle);
  const point = {
    xPx: block.centre.xPx + block.radiusPx * cos,
    yPx: block.centre.yPx - block.radiusPx * sin,
  };
  const out = side === "outside" ? 1 : -1;
  const clearPx = CURVE_CLEAR_REM * remPx;
  const sidePx = CURVE_SIDE_REM * remPx;
  // At the top or bottom the block starts to the right of the point, leaving the point itself to
  // the core arrow; elsewhere it stands off the circle on the side the point faces.
  let leftPx: number;
  if (cos === 0) {
    leftPx = point.xPx + sidePx;
  } else if (cos * out > 0) {
    leftPx = point.xPx + clearPx;
  } else {
    leftPx = point.xPx - clearPx - widthPx;
  }
  let topPx: number;
  if (sin === 0) {
    topPx = point.yPx - heightPx / 2;
  } else if (sin * out > 0) {
    topPx = point.yPx - clearPx - heightPx;
  } else {
    topPx = point.yPx + clearPx;
  }
  return { leftPx, topPx, widthPx, heightPx };
}

/** Where a ring's label may go beside its coreward point: below right first, then round. */
function ringCandidates(
  label: RingLabel,
  size: { readonly widthPx: number; readonly heightPx: number },
  remPx: number,
): BoxPx[] {
  const sidePx = CURVE_SIDE_REM * remPx;
  const clearPx = (CURVE_SIDE_REM / 2) * remPx;
  const right = label.xPx + sidePx;
  const left = label.xPx - sidePx - size.widthPx;
  const below = label.yPx + clearPx;
  const above = label.yPx - clearPx - size.heightPx;
  return [
    { leftPx: right, topPx: below, ...size },
    { leftPx: right, topPx: above, ...size },
    { leftPx: left, topPx: below, ...size },
    { leftPx: left, topPx: above, ...size },
  ];
}

/**
 * Sets the labels of the spheres and rings (plan 05, T10.h): each inside the view and clear of the
 * furniture and of each other, or not at all.
 *
 * @remarks
 * A sphere's labels stand one above another beside its circle: above its top, to the right of the
 * top point, when there is room; else at the first point round the circle, outside it and then
 * inside it, where they fit. A ring's label stands below and to the right of the ring's coreward
 * point, or on another side of it. A label is dropped only when nowhere fits, as when its circle is
 * out of the view.
 *
 * @param obstacles - Furniture the labels keep clear of: the triad and the core arrow.
 */
export function placeCurveLabels(
  labels: ReadonlyArray<CurveLabel>,
  viewport: Viewport,
  obstacles: ReadonlyArray<BoxPx>,
): ReadonlyArray<PlacedCurveLabel> {
  const remPx = viewport.remPx;
  const edgePx = edgeInsetPx(viewport);
  const taken: BoxPx[] = [...obstacles];
  const fits = (box: BoxPx): boolean =>
    insideView(box, viewport, edgePx) && !taken.some((other) => boxesOverlap(box, other));
  const placed: PlacedCurveLabel[] = [];

  // The labels of one circle come together, the first at stack 0.
  const blocks: Array<{ labels: CircleLabel[]; centre: ScreenPoint; radiusPx: number }> = [];
  for (const label of labels) {
    if (label.placement !== "circle-top") {
      continue;
    }
    const last = blocks.at(-1);
    if (label.stack > 0 && last !== undefined) {
      last.labels.push(label);
    } else {
      blocks.push({ labels: [label], centre: label.centre, radiusPx: label.radiusPx });
    }
  }
  for (const block of blocks) {
    const sizes = block.labels.map((label) =>
      textBoxPx(textSizeRem(label.text, OVERLAY_LETTER_SPACING_EM), remPx),
    );
    const widthPx = Math.max(...sizes.map((size) => size.widthPx));
    const lineHeightPx = Math.max(...sizes.map((size) => size.heightPx));
    const heightPx = lineHeightPx * block.labels.length;
    const candidates = (["outside", "inside"] as const).flatMap((side) =>
      CIRCLE_ANGLES_DEG.map((angleDeg) =>
        blockAtAngle(block, angleDeg, side, widthPx, heightPx, remPx),
      ),
    );
    const box = candidates.find(fits);
    if (box === undefined) {
      continue;
    }
    taken.push(box);
    for (const [line, label] of block.labels.entries()) {
      placed.push({
        key: label.key,
        text: label.text,
        leftPx: box.leftPx,
        topPx: box.topPx + line * lineHeightPx,
        widthPx: sizes[line]?.widthPx ?? widthPx,
        heightPx: lineHeightPx,
      });
    }
  }

  for (const label of labels) {
    if (label.placement !== "ring") {
      continue;
    }
    const size = textBoxPx(textSizeRem(label.text, OVERLAY_LETTER_SPACING_EM), remPx);
    const box = ringCandidates(label, size, remPx).find(fits);
    if (box === undefined) {
      continue;
    }
    taken.push(box);
    placed.push({ key: label.key, text: label.text, ...box });
  }
  return placed;
}
