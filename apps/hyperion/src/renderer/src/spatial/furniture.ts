/**
 * Where a spatial view's furniture goes over its canvas: the axis triad, the core arrow and the
 * labels of its spheres and rings, kept inside the view and off each other.
 *
 * @remarks
 * Pure layout, in CSS pixels from the view's top left unless a name says `rem`. Text boxes are
 * estimated from their lengths ({@link textSizeRem}), since text cannot be measured before it is
 * laid out. The triad is placed first, in its corner; the core arrow stops short of it; and the
 * curve labels are fitted round both. No text stands on a mark or a stalk
 * (decision-r07-quality-and-destination, Q6 (b) to (d); R07.T16.j): the triad's labels keep clear
 * of the other axes' symbols and lines, the core arrow's label of its own arrow, and the curve
 * labels of the marks' stalks and symbols, each giving up that clearance only as its last resort.
 */

import {
  lineScale,
  RING_SHIFTS,
  reticleStrokesCssPx,
  type ReticleStrokesCss,
} from "../lib/strokes";
import { type CameraAngles, viewBasis, type ViewBasis, type Viewport } from "./camera";
import type { CircleLabel, CurveLabel, DrawOp, ScreenPoint, SymbolOp } from "./drawList";
import type { LocalFrame } from "../geometry/frame";
import { type BoxPx, halfExtentRem, type TextSizeRem, textSizeRem } from "./labels";
import { boxGapPx, LABEL_EDGE_CLEARANCE_REM, symbolOutline, unitInradius } from "./symbols";
import { dot, type Vec3 } from "../geometry/vec3";

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

/**
 * A stroke's ink on the screen, px: the segment from `from` to `to` and `halfWidthPx` either side of
 * it, a disc of that radius where its ends meet, as a symbol is taken. What text over a view keeps
 * clear of (R07.T16.j).
 */
export interface InkPx {
  readonly from: ScreenPoint;
  readonly to: ScreenPoint;
  readonly halfWidthPx: number;
}

function clamp(value: number, low: number, high: number): number {
  return Math.min(high, Math.max(low, value));
}

/** A gap this much short of a clearance still meets it: the rounding of the sums that make it. */
const GAP_TOLERANCE_PX = 1e-9;

function pointBoxGapPx(point: ScreenPoint, box: BoxPx): number {
  return Math.hypot(
    Math.max(0, box.leftPx - point.xPx, point.xPx - box.leftPx - box.widthPx),
    Math.max(0, box.topPx - point.yPx, point.yPx - box.topPx - box.heightPx),
  );
}

function pointSegmentGapPx(point: ScreenPoint, from: ScreenPoint, to: ScreenPoint): number {
  const dx = to.xPx - from.xPx;
  const dy = to.yPx - from.yPx;
  const lengthSquared = dx * dx + dy * dy;
  const share =
    lengthSquared > 0
      ? clamp(((point.xPx - from.xPx) * dx + (point.yPx - from.yPx) * dy) / lengthSquared, 0, 1)
      : 0;
  return Math.hypot(point.xPx - from.xPx - share * dx, point.yPx - from.yPx - share * dy);
}

/** Whether a segment passes through a box, or ends in it (Liang and Barsky's clip). */
function segmentMeetsBox(from: ScreenPoint, to: ScreenPoint, box: BoxPx): boolean {
  const dx = to.xPx - from.xPx;
  const dy = to.yPx - from.yPx;
  const slabs: ReadonlyArray<readonly [number, number]> = [
    [-dx, from.xPx - box.leftPx],
    [dx, box.leftPx + box.widthPx - from.xPx],
    [-dy, from.yPx - box.topPx],
    [dy, box.topPx + box.heightPx - from.yPx],
  ];
  let enter = 0;
  let leave = 1;
  for (const [step, room] of slabs) {
    if (step === 0) {
      if (room < 0) {
        return false;
      }
      continue;
    }
    const share = room / step;
    if (step < 0) {
      enter = Math.max(enter, share);
    } else {
      leave = Math.min(leave, share);
    }
    if (enter > leave) {
      return false;
    }
  }
  return true;
}

/**
 * The least distance between a segment and a box, px: 0 where they meet.
 *
 * @remarks
 * Apart, a segment and a box come nearest at an end of the segment or a corner of the box.
 */
export function segmentBoxGapPx(from: ScreenPoint, to: ScreenPoint, box: BoxPx): number {
  if (segmentMeetsBox(from, to, box)) {
    return 0;
  }
  const right = box.leftPx + box.widthPx;
  const bottom = box.topPx + box.heightPx;
  const corners: ReadonlyArray<ScreenPoint> = [
    { xPx: box.leftPx, yPx: box.topPx },
    { xPx: right, yPx: box.topPx },
    { xPx: right, yPx: bottom },
    { xPx: box.leftPx, yPx: bottom },
  ];
  return Math.min(
    pointBoxGapPx(from, box),
    pointBoxGapPx(to, box),
    ...corners.map((corner) => pointSegmentGapPx(corner, from, to)),
  );
}

/** The least distance between a box and a stroke's ink, px: 0 where they meet. */
export function inkGapPx(box: BoxPx, ink: InkPx): number {
  return Math.max(0, segmentBoxGapPx(ink.from, ink.to, box) - ink.halfWidthPx);
}

/** Whether a box stands at least `clearPx` from a stroke's ink. */
function clearOfInk(box: BoxPx, ink: InkPx, clearPx: number): boolean {
  return inkGapPx(box, ink) >= clearPx - GAP_TOLERANCE_PX;
}

/** How many halvings find a box's least reach: far finer than a pixel at any size. */
const REACH_HALVINGS = 50;

/**
 * The least distance along the unit `direction` from `from` at which a box of `widthPx` by
 * `heightPx`, centred there, stands `clearPx` from the segment `ends` by true distance, a point
 * being a segment whose ends meet (decision-r07-quality-and-destination, addendum D, D8 and D9;
 * R07.T16.j). It places a triad label beyond its axis's end and the core arrow's label beside and
 * beyond the arrow, so that each stands its clearance by its box's distance, not along a direction.
 *
 * @remarks
 * `from` lies on the segment, so the box starts on it, and its distance from it grows as it moves
 * out: the distance between two convex shapes, one moved along a line, is convex in how far it
 * moves. Halving then finds the least reach. Moved its half-width, its half-height and `clearPx`
 * out, the box stands at least `clearPx` off along `direction` from every point of a segment through
 * `from` that the move leaves behind it, as the arrow's tail and middle and the triad's tip are.
 *
 * The triad calls it in `rem`, as px at 1 px a rem, as it takes its boxes ({@link remBox}).
 */
function leastReachPx(
  from: ScreenPoint,
  direction: ScreenRem,
  widthPx: number,
  heightPx: number,
  ends: readonly [ScreenPoint, ScreenPoint],
  clearPx: number,
): number {
  const gapAt = (reachPx: number): number =>
    segmentBoxGapPx(ends[0], ends[1], {
      leftPx: from.xPx + direction.x * reachPx - widthPx / 2,
      topPx: from.yPx + direction.y * reachPx - heightPx / 2,
      widthPx,
      heightPx,
    });
  let near = 0;
  let far = widthPx / 2 + heightPx / 2 + clearPx;
  for (let step = 0; step < REACH_HALVINGS && gapAt(far) < clearPx; step += 1) {
    near = far;
    far *= 2;
  }
  for (let step = 0; step < REACH_HALVINGS; step += 1) {
    const middle = (near + far) / 2;
    if (gapAt(middle) >= clearPx) {
      far = middle;
    } else {
      near = middle;
    }
  }
  return far;
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

/** The box of a line of overlay text, in pixels. */
function textBoxPx(size: TextSizeRem, remPx: number): { widthPx: number; heightPx: number } {
  return { widthPx: size.widthRem * remPx, heightPx: size.heightRem * remPx };
}

/**
 * The space kept between furniture and the edge of the view, in `rem`: the labels' own
 * (`LABEL_EDGE_CLEARANCE_REM`).
 */
const EDGE_REM = LABEL_EDGE_CLEARANCE_REM;

function edgeInsetPx(viewport: Viewport): number {
  return EDGE_REM * viewport.remPx;
}

// The axis triad.

/** The triad's box in the view's bottom left-hand corner, in `rem`, its origin at the centre. */
export const TRIAD_BOX_REM = { width: 15, height: 8 } as const;
/** The box a triad is drawn in, in `rem`. */
export interface TriadBoxRem {
  readonly width: number;
  readonly height: number;
}
/** The length of an axis seen side on, in `rem`, on a box with the room for it. */
const AXIS_REM = 1.75;
/** The radius of the triad's away and towards symbols, in `rem`. */
export const TRIAD_MARKER_REM = 0.25;
/** The length of each barb of the arrowhead that ends an axis across the screen, in `rem`. */
export const TRIAD_HEAD_REM = 0.3;
/** The space between an axis's end and its label, in `rem`. */
const LABEL_GAP_REM = 0.25;
/**
 * How far a triad label keeps clear of another axis's end symbol and line, and of the labels placed
 * before it, in `rem` (decision-r07-quality-and-destination, Q6 (b); R07.T16.j).
 */
const TRIAD_CLEAR_REM = 0.125;
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
 * The box a triad is drawn in on a view of this size: its usual 15 × 8 rem, or the view itself
 * where that is smaller.
 *
 * @remarks
 * The overlay clips whatever leaves it and the guide has a 3D view always show its axis triad, so a
 * stage shorter than the usual box (the local chart's, which gives way to its census table) gets a
 * shorter triad rather than a triad with its top cut off.
 */
export function triadBoxRem(viewport: Viewport): TriadBoxRem {
  if (!(viewport.remPx > 0)) {
    return TRIAD_BOX_REM;
  }
  return {
    width: Math.min(TRIAD_BOX_REM.width, viewport.widthPx / viewport.remPx),
    height: Math.min(TRIAD_BOX_REM.height, viewport.heightPx / viewport.remPx),
  };
}

/**
 * How far an axis reaches from the triad's origin in a box of this height, in `rem`.
 *
 * @remarks
 * The usual 1.75 rem, shortened on a short box so that an axis's end symbol and the label beyond it
 * still fit inside the box, since the box is all the room the overlay leaves.
 */
function axisReachRem(boxHeightRem: number, labelHeightRem: number): number {
  const room = boxHeightRem / 2 - TRIAD_MARKER_REM - LABEL_GAP_REM - labelHeightRem;
  return Math.max(0, Math.min(AXIS_REM, room));
}

/**
 * How far the triad's away and towards circles move out, `rem`: the outline shift δ at a
 * device-pixel ratio (`markShiftDevicePx` ÷ the ratio), over the rem. `AxisTriad` draws them there,
 * and their labels keep clear of them there (R07.T16.f and T16.j); 0 before the rem is known.
 */
export function triadShiftRem(devicePixelRatio: number, remPx: number): number {
  return remPx > 0 ? reticleStrokesCssPx(devicePixelRatio).shiftPx / remPx : 0;
}

/** An axis as the triad draws it, before its label is placed. */
interface DrawnAxis {
  readonly name: TriadAxis["name"];
  readonly projected: ScreenRem;
  readonly tip: ScreenRem;
  readonly end: AxisEnd;
}

/** A point in `rem` as a screen point at 1 px a rem, as {@link remBox} takes a box. */
function remPoint(point: ScreenRem): ScreenPoint {
  return { xPx: point.x, yPx: point.y };
}

/**
 * An axis as `AxisTriad` draws it, in `rem` at 1 px a rem, which the triad's labels keep clear of:
 * its line from the origin, which stops at its end symbol's circle; and its end symbol, the circle
 * of radius `TRIAD_MARKER_REM` + δ with what it holds, taken as a disc, or the arrowhead's two
 * barbs. Each is its drawn path, as the ruling names them (decision-r07-quality-and-destination,
 * Q6 (b)), with no width.
 */
function axisInk(
  axis: DrawnAxis,
  shiftRem: number,
): { readonly line: ReadonlyArray<InkPx>; readonly symbol: ReadonlyArray<InkPx> } {
  const reach = length(axis.tip);
  const markerRem = TRIAD_MARKER_REM + shiftRem;
  const lineReach = axis.end === "across" ? reach : reach - markerRem;
  const direction = reach > 0 ? { x: axis.tip.x / reach, y: axis.tip.y / reach } : { x: 0, y: 0 };
  const tip = remPoint(axis.tip);
  const line: InkPx[] =
    lineReach > 0
      ? [
          {
            from: { xPx: 0, yPx: 0 },
            to: { xPx: direction.x * lineReach, yPx: direction.y * lineReach },
            halfWidthPx: 0,
          },
        ]
      : [];
  if (axis.end !== "across") {
    return { line, symbol: [{ from: tip, to: tip, halfWidthPx: markerRem }] };
  }
  const heading = Math.atan2(direction.y, direction.x);
  const barb = (sign: number): InkPx => {
    const angle = heading + Math.PI + (sign * Math.PI) / 6;
    return {
      from: tip,
      to: {
        xPx: axis.tip.x + TRIAD_HEAD_REM * Math.cos(angle),
        yPx: axis.tip.y + TRIAD_HEAD_REM * Math.sin(angle),
      },
      halfWidthPx: 0,
    };
  };
  return { line, symbol: [barb(1), barb(-1)] };
}

/**
 * The triad's axes and labels for a frame seen from `angles` (plan 05, T10.f).
 *
 * @remarks
 * A label goes beyond its axis's end; an axis seen end on has its label beside its symbol, on the
 * side away from the other two axes; a label that would cover another goes a line below or above
 * it, or further out along its axis. Every label is kept inside the triad's box, and 0.25 rem inside
 * its left and bottom sides, the stage's edges (addendum D, D5; R07.T16.j). On the galactic
 * axis the directions are labelled `-X` and `+Y`, as the frame falls back to them (design note
 * D11), with the guide's `-` for a signed value.
 *
 * A label's first place stands beyond its axis's end, along the axis, at the least distance at which
 * its box's true distance from the tip is `TRIAD_MARKER_REM` + `LABEL_GAP_REM` for an away or
 * towards end and `LABEL_GAP_REM` for an arrowhead, not that distance along the axis, which at an
 * oblique axis put the box on its own circle (decision-r07-quality-and-destination, addendum D,
 * D8). Each place a label tries, in that order, must stand 0.125 rem clear of every axis's end
 * symbol and every axis line, its own included, and of the labels placed before it, so that no
 * label stands on a symbol, which would name the wrong axis or put a stroke against its letters
 * (Q6 (b); R07.T16.j). Of those, it takes the first whose box also stands nearer its own axis's tip
 * than any other's, so that it is read as its own axis's name (the orchestrator's ruling on T16.j's
 * by-hand finding; R07's Risks, "Deviations in T16.j, as built"); else the first that clears all
 * three. The triad always shows its labels, so none is dropped: where no place clears all three,
 * the first that clears the symbols and the labels is taken, crossing a line; where none does, the
 * label stands at its first place.
 *
 * @param frame - The scene's frame, which the camera's angles are measured in.
 * @param boxRem - The box the triad is drawn in, from {@link triadBoxRem}: the axes are shortened
 *   to fit a box shorter than {@link TRIAD_BOX_REM}, so that nothing leaves it.
 * @param shown - The directions the triad draws and names, as the camera sees them: the galactic
 *   ones at the view centre where the scene's frame is tilted to them, as the orbit map's is (plan
 *   14, D21); the scene's frame's own when absent.
 * @param shiftRem - How far the away and towards circles move out, which their labels keep clear
 *   of ({@link triadShiftRem}); none, as at a ratio of 2, when absent.
 */
export function triadLayout(
  frame: LocalFrame,
  angles: CameraAngles,
  boxRem: TriadBoxRem = TRIAD_BOX_REM,
  shown: LocalFrame = frame,
  shiftRem = 0,
): ReadonlyArray<TriadAxis> {
  const basis = viewBasis(frame, angles);
  const labels = shown.onAxis
    ? { coreward: "-X", spinward: "+Y", north: "NORTH" }
    : { coreward: "COREWARD", spinward: "SPINWARD", north: "NORTH" };
  const reachRem = axisReachRem(
    boxRem.height,
    textSizeRem(labels.coreward, OVERLAY_LETTER_SPACING_EM).heightRem,
  );
  const axes = (["coreward", "spinward", "north"] as const).map((name): DrawnAxis => {
    const projected = onScreen(shown[name], basis);
    return {
      name,
      projected,
      tip: { x: projected.x * reachRem, y: projected.y * reachRem },
      end: axisEnd(dot(shown[name], basis.forward)),
    };
  });
  const inks = axes.map((axis) => axisInk(axis, shiftRem));
  const endSymbols = inks.flatMap((ink) => ink.symbol);
  const axisLines = inks.flatMap((ink) => ink.line);
  const placed: BoxPx[] = [];
  return axes.map(({ name, projected, tip, end }): TriadAxis => {
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
    const tipPoint = remPoint(tip);
    const base = along(
      tip,
      direction,
      leastReachPx(tipPoint, direction, size.widthRem, size.heightRem, [tipPoint, tipPoint], clear),
    );
    const lineRem = size.heightRem + LABEL_GAP_REM / 2;
    // Kept inside the triad's box, whose origin is its centre, and 0.25 rem inside its left and
    // bottom sides, the stage's edges, off the canvas's focus ring (decision-r07-quality-and-
    // destination, addendum D, D5; R07.T16.j).
    const inBox = (centre: ScreenRem): ScreenRem => ({
      x: clamp(
        centre.x,
        -boxRem.width / 2 + LABEL_EDGE_CLEARANCE_REM + size.widthRem / 2,
        boxRem.width / 2 - size.widthRem / 2,
      ),
      y: clamp(
        centre.y,
        -boxRem.height / 2 + size.heightRem / 2,
        boxRem.height / 2 - LABEL_EDGE_CLEARANCE_REM - size.heightRem / 2,
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
    const clearOf = (marks: ReadonlyArray<InkPx>, centre: ScreenRem): boolean => {
      const box = remBox(centre, size);
      return marks.every((ink) => clearOfInk(box, ink, TRIAD_CLEAR_REM));
    };
    const clearOfLabels = (centre: ScreenRem): boolean => {
      const box = remBox(centre, size);
      return placed.every((other) => boxGapPx(box, other) >= TRIAD_CLEAR_REM - GAP_TOLERANCE_PX);
    };
    const clearOfAll = (centre: ScreenRem): boolean =>
      clearOf(endSymbols, centre) && clearOfLabels(centre) && clearOf(axisLines, centre);
    // Nearer its own tip than any other axis's, by its box's distance, so that it is read as its
    // own axis's name.
    const nearestOwnTip = (centre: ScreenRem): boolean => {
      const box = remBox(centre, size);
      const gapTo = (point: ScreenRem): number =>
        segmentBoxGapPx(remPoint(point), remPoint(point), box);
      return axes.every((other) => other.name === name || gapTo(tip) < gapTo(other.tip));
    };
    const labelAt =
      candidates.find((centre) => clearOfAll(centre) && nearestOwnTip(centre)) ??
      candidates.find(clearOfAll) ??
      candidates.find((centre) => clearOf(endSymbols, centre) && clearOfLabels(centre)) ??
      inBox(base);
    placed.push(remBox(labelAt, size));
    return { name, label: labels[name], tip, end, labelAt, labelSize: size };
  });
}

/**
 * The part of the view the triad covers, its axes, symbols and labels, in pixels: what the core
 * arrow and the curve labels keep clear of.
 *
 * @param boxRem - The box the triad was laid out in, which fixes where its origin sits.
 */
export function triadFootprintPx(
  axes: ReadonlyArray<TriadAxis>,
  viewport: Viewport,
  boxRem: TriadBoxRem = TRIAD_BOX_REM,
): BoxPx {
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
  const originX = (boxRem.width / 2) * remPx;
  const originY = viewport.heightPx - (boxRem.height / 2) * remPx;
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
/** The core arrow's length from its tail to the point of its head, in `rem`. */
export const CORE_ARROW_REM = 1.5;
/** The length of the core arrow's head, and so half the width of its barbs, in `rem`. */
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

/** A box moved as little as it takes to lie inside the view, {@link EDGE_REM} in from its sides. */
function clampedIntoView(box: BoxPx, viewport: Viewport): BoxPx {
  const edgePx = edgeInsetPx(viewport);
  return {
    ...box,
    leftPx: clamp(box.leftPx, edgePx, viewport.widthPx - edgePx - box.widthPx),
    topPx: clamp(box.topPx, edgePx, viewport.heightPx - edgePx - box.heightPx),
  };
}

/**
 * A box of `size` beside the core arrow, from `from`, a point of its segment, along `side`: at the
 * least reach at which its true distance from the segment `tail`–`tip` is `clearPx`
 * ({@link leastReachPx}), then kept inside the view (decision-r07-quality-and-destination, addendum
 * D, D9; R07.T16.j).
 */
function besideArrow(
  from: ScreenPoint,
  side: ScreenRem,
  tail: ScreenPoint,
  tip: ScreenPoint,
  clearPx: number,
  size: TextSizeRem,
  viewport: Viewport,
): BoxPx {
  const { widthPx, heightPx } = textBoxPx(size, viewport.remPx);
  const reachPx = leastReachPx(from, side, widthPx, heightPx, [tail, tip], clearPx);
  return clampedIntoView(
    {
      leftPx: from.xPx + side.x * reachPx - widthPx / 2,
      topPx: from.yPx + side.y * reachPx - heightPx / 2,
      widthPx,
      heightPx,
    },
    viewport,
  );
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
 * Each place the label tries must also stand `CORE_HEAD_REM` + `CORE_GAP_REM`, 0.625 rem, from the
 * arrow's segment, from its tail to the point of its head, by the box's true distance, so that the
 * arrow never crosses its own readout (decision-r07-quality-and-destination, Q6 (c); R07.T16.j): a
 * bounding box would refuse the places beside a slanting arrow. The three places, beside the
 * arrow's middle either side and beyond its tail towards the view's centre, each stand that far
 * from the segment by construction, at the least reach along their direction ({@link besideArrow};
 * addendum D, D9), so that a slanting arrow's ends come no nearer than its middle. A place that the
 * clamp into the view puts on the arrow is passed over. Where no place clears both, the first that
 * clears the arrow is taken, giving up the furniture's clearance first; so one always does but in a
 * view hardly larger than the label, where the first place stands as placed.
 *
 * @param frame - The scene's frame, which the camera's angles are measured in.
 * @param labelText - The label's text, as {@link coreLabelText} writes it.
 * @param obstacles - Furniture the arrow and its label keep clear of, such as the triad.
 * @param shown - The galactic directions at the view centre, whose coreward the arrow points
 *   along: apart from the scene's frame where that is tilted to them, as the orbit map's is (plan
 *   14, D21); the scene's frame's own when absent.
 */
export function coreArrowLayout(
  frame: LocalFrame,
  angles: CameraAngles,
  viewport: Viewport,
  labelText: string,
  obstacles: ReadonlyArray<BoxPx>,
  shown: LocalFrame = frame,
): CoreArrowLayout {
  if (shown.onAxis) {
    return { kind: "undefined" };
  }
  const remPx = viewport.remPx;
  const basis = viewBasis(frame, angles);
  const projected = onScreen(shown.coreward, basis);
  const reach = length(projected);
  const size = textSizeRem(labelText, OVERLAY_LETTER_SPACING_EM);
  const clearOf = (box: BoxPx): boolean => !obstacles.some((other) => boxesOverlap(box, other));

  if (reach < Math.sin((LINE_OF_SIGHT_DEG * Math.PI) / 180)) {
    const centre = { xPx: viewport.widthPx / 2, yPx: (RIM_INSET_REM + CORE_SYMBOL_REM) * remPx };
    const clearPx = (CORE_SYMBOL_REM + CORE_GAP_REM) * remPx;
    return {
      kind: dot(shown.coreward, basis.forward) > 0 ? "away" : "towards",
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
  // Beside its middle, either side, and beyond its tail, towards the view's centre: each clear of
  // the arrow by construction, but where the clamp into the view moves it back onto the arrow.
  const sides = [
    besideArrow(middle, preferred, tail, tip, clearPx, size, viewport),
    besideArrow(middle, { x: -preferred.x, y: -preferred.y }, tail, tip, clearPx, size, viewport),
    besideArrow(tail, { x: -direction.x, y: -direction.y }, tail, tip, clearPx, size, viewport),
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
  const first = sides[0] ?? besideArrow(middle, preferred, tail, tip, clearPx, size, viewport);
  // Off the arrow itself, by its segment's distance, as well as off the furniture.
  const clearOfArrow = (box: BoxPx): boolean =>
    segmentBoxGapPx(tail, tip, box) >= clearPx - GAP_TOLERANCE_PX;
  const candidates = [...sides, ...moved];
  const label =
    candidates.find((box) => clearOfArrow(box) && clearOf(box)) ??
    candidates.find(clearOfArrow) ??
    first;
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

/** Where a ring's label may go beside a point of its ring: below right first, then round. */
function ringCandidates(
  point: ScreenPoint,
  size: { readonly widthPx: number; readonly heightPx: number },
  remPx: number,
): BoxPx[] {
  const sidePx = CURVE_SIDE_REM * remPx;
  const clearPx = (CURVE_SIDE_REM / 2) * remPx;
  const right = point.xPx + sidePx;
  const left = point.xPx - sidePx - size.widthPx;
  const below = point.yPx + clearPx;
  const above = point.yPx - clearPx - size.heightPx;
  return [
    { leftPx: right, topPx: below, ...size },
    { leftPx: right, topPx: above, ...size },
    { leftPx: left, topPx: below, ...size },
    { leftPx: left, topPx: above, ...size },
  ];
}

/**
 * How far a curve label keeps clear of a mark's stalk and symbol, in `rem`
 * (decision-r07-quality-and-destination, Q6 (d); R07.T16.j).
 */
const CURVE_MARK_CLEAR_REM = 0.125;

/**
 * The side of a cell of the grid that marks' inks are sorted into, in `rem`, so that a box is
 * tested against the stalks and symbols near it alone and not every mark on a chart.
 */
const INK_CELL_REM = 2;

/** Whether a box comes near an ink where there are none: never. */
function nearNoInk(): boolean {
  return false;
}

/**
 * Whether any of `inks` comes within `clearPx` of a box inside the view, with the inks sorted once
 * into a grid over the view, so that each box is tested against those that reach its cells alone.
 * An ink wholly outside the view, `clearPx` and its width out, reaches no such box and is left out.
 */
function inksNear(
  inks: ReadonlyArray<InkPx>,
  viewport: Viewport,
  clearPx: number,
): (box: BoxPx) => boolean {
  if (inks.length === 0) {
    return nearNoInk;
  }
  const cellPx = Math.max(1, INK_CELL_REM * viewport.remPx);
  const columns = Math.max(1, Math.ceil(viewport.widthPx / cellPx));
  const rows = Math.max(1, Math.ceil(viewport.heightPx / cellPx));
  const column = (xPx: number): number => clamp(Math.floor(xPx / cellPx), 0, columns - 1);
  const row = (yPx: number): number => clamp(Math.floor(yPx / cellPx), 0, rows - 1);
  const cells = new Map<number, InkPx[]>();
  for (const ink of inks) {
    const reachPx = ink.halfWidthPx + clearPx;
    const leftPx = Math.min(ink.from.xPx, ink.to.xPx) - reachPx;
    const rightPx = Math.max(ink.from.xPx, ink.to.xPx) + reachPx;
    const topPx = Math.min(ink.from.yPx, ink.to.yPx) - reachPx;
    const bottomPx = Math.max(ink.from.yPx, ink.to.yPx) + reachPx;
    if (rightPx < 0 || bottomPx < 0 || leftPx > viewport.widthPx || topPx > viewport.heightPx) {
      continue;
    }
    for (let y = row(topPx); y <= row(bottomPx); y += 1) {
      for (let x = column(leftPx); x <= column(rightPx); x += 1) {
        const key = y * columns + x;
        const cell = cells.get(key);
        if (cell === undefined) {
          cells.set(key, [ink]);
        } else {
          cell.push(ink);
        }
      }
    }
  }
  return (box) => {
    for (let y = row(box.topPx); y <= row(box.topPx + box.heightPx); y += 1) {
      for (let x = column(box.leftPx); x <= column(box.leftPx + box.widthPx); x += 1) {
        if (cells.get(y * columns + x)?.some((ink) => !clearOfInk(box, ink, clearPx)) === true) {
          return true;
        }
      }
    }
    return false;
  };
}

/**
 * How far a symbol's ink reaches from its centre as `paint` draws it, CSS px: its outline's outer
 * edge, moved out by δ (a polygon's corners by δ over its unit inradius, a ringed circle's ring by
 * 3δ) and half the mark stroke beyond, or at a polygon's corner its mitre.
 */
function symbolReachPx(op: SymbolOp, strokes: ReticleStrokesCss): number {
  const outline = symbolOutline(op.shape);
  const halfStrokePx = strokes.markStrokePx / 2;
  let reachPx: number;
  switch (outline.kind) {
    case "circle":
      reachPx = op.radiusPx + strokes.shiftPx + halfStrokePx;
      break;
    case "polygon": {
      // A regular polygon's corner, of interior angle π(n − 2) ÷ n, is mitred out to half the
      // stroke over the sine of half that angle.
      const corners = outline.points.length - 1;
      const halfCornerAngle = (Math.PI * (corners - 2)) / (2 * corners);
      reachPx =
        op.radiusPx +
        strokes.shiftPx / unitInradius(outline.points) +
        halfStrokePx / Math.sin(halfCornerAngle);
      break;
    }
    case "ringed-circle":
      reachPx = op.radiusPx + RING_SHIFTS * strokes.shiftPx + halfStrokePx;
      break;
  }
  return reachPx;
}

/**
 * The marks' stalks and symbols as `paint` draws them at a device-pixel ratio, CSS px: what a curve
 * label keeps clear of (decision-r07-quality-and-destination, Q6 (d); R07.T16.j).
 *
 * @remarks
 * A stalk is its line at the line scale, `lineScale` ÷ the ratio times its width; a symbol the disc
 * its outline reaches ({@link symbolReachPx}), whatever it holds. The selection's bracket and the
 * destination's chevrons are not counted.
 */
export function markInksPx(
  ops: ReadonlyArray<DrawOp>,
  devicePixelRatio: number,
): ReadonlyArray<InkPx> {
  const strokes = reticleStrokesCssPx(devicePixelRatio);
  const lineFactor = lineScale(devicePixelRatio) / devicePixelRatio;
  const inks: InkPx[] = [];
  for (const op of ops) {
    if (op.kind === "line" && op.markId !== null) {
      inks.push({ from: op.from, to: op.to, halfWidthPx: (op.widthPx * lineFactor) / 2 });
    } else if (op.kind === "symbol") {
      inks.push({ from: op.centre, to: op.centre, halfWidthPx: symbolReachPx(op, strokes) });
    }
  }
  return inks;
}

/**
 * Sets the labels of the spheres and rings (plan 05, T10.h): each inside the view and clear of the
 * furniture and of each other, or not at all.
 *
 * @remarks
 * A sphere's labels stand one above another beside its circle: above its top, to the right of the
 * top point, when there is room; else at the first point round the circle, outside it and then
 * inside it, where they fit. A ring's label stands below and to the right of the ring's coreward
 * point, or on another side of it; else beside the first point along its ring, either way from
 * there, where it fits (`RingLabel.alongPx`). A label is dropped only when nowhere fits, as when its
 * circle is out of the view.
 *
 * Each place a label tries must also stand 0.125 rem clear of every mark's stalk and symbol, since
 * a stalk carries meaning where the `--line` grid does not (decision-r07-quality-and-destination,
 * Q6 (d); R07.T16.j). A label is never dropped for a stalk's sake: where no place clears the marks,
 * it takes the first that clears everything else. No stalk is cut, and no plate is drawn.
 *
 * @param obstacles - Furniture the labels keep clear of: the triad and the core arrow.
 * @param marks - The marks' stalks and symbols, as {@link markInksPx} gives them; none when absent.
 */
export function placeCurveLabels(
  labels: ReadonlyArray<CurveLabel>,
  viewport: Viewport,
  obstacles: ReadonlyArray<BoxPx>,
  marks: ReadonlyArray<InkPx> = [],
): ReadonlyArray<PlacedCurveLabel> {
  const remPx = viewport.remPx;
  const edgePx = edgeInsetPx(viewport);
  const taken: BoxPx[] = [...obstacles];
  const fits = (box: BoxPx): boolean =>
    insideView(box, viewport, edgePx) && !taken.some((other) => boxesOverlap(box, other));
  const nearMark = inksNear(marks, viewport, CURVE_MARK_CLEAR_REM * remPx);
  // The first place that fits clear of the marks, else the first that fits: the stalks' clearance is
  // given up first.
  const choose = (candidates: ReadonlyArray<BoxPx>): BoxPx | undefined =>
    candidates.find((box) => fits(box) && !nearMark(box)) ?? candidates.find(fits);
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
    const box = choose(candidates);
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
    const box = choose(
      [{ xPx: label.xPx, yPx: label.yPx }, ...(label.alongPx ?? [])].flatMap((point) =>
        ringCandidates(point, size, remPx),
      ),
    );
    if (box === undefined) {
      continue;
    }
    taken.push(box);
    placed.push({ key: label.key, text: label.text, ...box });
  }
  return placed;
}
