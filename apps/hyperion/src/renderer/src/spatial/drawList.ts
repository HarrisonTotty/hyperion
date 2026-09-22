import {
  type Camera,
  project,
  type Projected,
  viewBasis,
  type ViewBasis,
  type Viewport,
} from "./camera";
import { isAbovePlane, type PointMark, type SpatialScene, type SymbolShape } from "./marks";
import { gridLines, ringPolyline } from "./plane";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX } from "./symbols";
import { dot, scale, sub, type Vec3 } from "./vec3";

/**
 * A colour token an op is drawn in, named after its CSS custom property (`--text` and so on).
 *
 * @remarks
 * The draw list never holds a colour: the painter resolves the name against the stylesheet.
 */
export type ColourToken = "text" | "accent" | "target" | "line";

/** A point on the screen, in CSS pixels from the top left of the view. */
export interface ScreenPoint {
  readonly xPx: number;
  readonly yPx: number;
}

/** A straight line: a mark's stalk (with its `markId`) or a grid line. */
export interface LineOp {
  readonly kind: "line";
  readonly from: ScreenPoint;
  readonly to: ScreenPoint;
  readonly stroke: ColourToken;
  readonly widthPx: number;
  /** The mark whose stalk this is, or `null` for the grid. */
  readonly markId: string | null;
}

/** A polyline, such as a ring on the reference plane. */
export interface PolylineOp {
  readonly kind: "polyline";
  readonly points: ReadonlyArray<ScreenPoint>;
  readonly stroke: ColourToken;
  readonly widthPx: number;
}

/** A circle on the screen, such as a sphere's outline. */
export interface CircleOp {
  readonly kind: "circle";
  readonly centre: ScreenPoint;
  readonly radiusPx: number;
  readonly stroke: ColourToken;
  readonly widthPx: number;
}

/** A mark's symbol. */
export interface SymbolOp {
  readonly kind: "symbol";
  readonly id: string;
  readonly centre: ScreenPoint;
  readonly shape: SymbolShape;
  /** Radius of the outline's path: the symbol's radius less half the stroke, so it stays inside. */
  readonly radiusPx: number;
  readonly stroke: ColourToken;
  /** The fill above the reference plane; `null` below it, where the symbol is open. */
  readonly fill: ColourToken | null;
  readonly widthPx: number;
}

/** A bracket reticle: four corners of a square about a mark. */
export interface ReticleOp {
  readonly kind: "reticle";
  readonly id: string;
  readonly centre: ScreenPoint;
  /** Half the width of the square the corners mark. */
  readonly halfSizePx: number;
  readonly stroke: ColourToken;
  readonly widthPx: number;
}

/** Short straight ticks, such as those along the edge of fetched data. */
export interface TicksOp {
  readonly kind: "ticks";
  readonly segments: ReadonlyArray<{ readonly from: ScreenPoint; readonly to: ScreenPoint }>;
  readonly stroke: ColourToken;
  readonly widthPx: number;
}

/** One drawing instruction, in the order the painter executes them. */
export type DrawOp = LineOp | PolylineOp | CircleOp | SymbolOp | ReticleOp | TicksOp;

/** Where a mark ended up on the screen, for picking and labels. */
export interface Anchor {
  readonly id: string;
  readonly xPx: number;
  readonly yPx: number;
  /** Larger is further from the viewer. */
  readonly depth: number;
  /** The symbol's radius, outline included. */
  readonly radiusPx: number;
}

/** What every curve label gives: its text and the point it names. */
interface CurveLabelBase {
  /** Stable key for the label's element. */
  readonly key: string;
  readonly text: string;
  readonly xPx: number;
  readonly yPx: number;
  /** Position in a stack of labels sharing one point, 0 nearest the curve. */
  readonly stack: number;
}

/**
 * The label of a sphere, at the top of its circle, with the circle itself, so that the view can
 * move the label along it when the top is out of sight.
 */
export interface CircleLabel extends CurveLabelBase {
  readonly placement: "circle-top";
  readonly centre: ScreenPoint;
  readonly radiusPx: number;
}

/** The label of a ring on the reference plane, at the ring's coreward point. */
export interface RingLabel extends CurveLabelBase {
  readonly placement: "ring";
}

/** Where to put the DOM label of a sphere or a ring. */
export type CurveLabel = CircleLabel | RingLabel;

/** What a spatial view paints, and where its marks and curves ended up. */
export interface DrawList {
  readonly ops: ReadonlyArray<DrawOp>;
  readonly anchors: ReadonlyArray<Anchor>;
  readonly curveLabels: ReadonlyArray<CurveLabel>;
}

const STALK_WIDTH_PX = 1;
const GRID_WIDTH_PX = 1;
const RANGE_WIDTH_PX = 1.5;
const DATA_EDGE_WIDTH_PX = 1;
const RETICLE_WIDTH_PX = 1.5;
/** How much larger than its symbol a reticle is, per reticle, in `rem`. */
const RETICLE_MARGIN_REM = 0.5;
const TICK_EVERY_DEG = 10;
const TICK_LENGTH_REM = 0.25;
const SAME_RADIUS_TOLERANCE = 1e-9;

interface PlacedMark {
  readonly mark: PointMark;
  readonly at: Projected;
  readonly above: boolean;
}

// At equal depth the lower ID is drawn last, on top, because it is the one `pick` chooses: what
// the operator sees on top is what a click selects.
function byFarToNear(a: PlacedMark, b: PlacedMark): number {
  if (a.at.depth !== b.at.depth) {
    return b.at.depth - a.at.depth;
  }
  if (a.mark.id === b.mark.id) {
    return 0;
  }
  return a.mark.id < b.mark.id ? 1 : -1;
}

function screen(point: Projected): ScreenPoint {
  return { xPx: point.xPx, yPx: point.yPx };
}

function outerRadiusPx(mark: PointMark, viewport: Viewport): number {
  return (SIZE_CLASS_REM[mark.sizeClass] * viewport.remPx) / 2;
}

function markOps(
  placed: PlacedMark,
  scene: SpatialScene,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): [LineOp, SymbolOp] {
  const { mark, at, above } = placed;
  const colour: ColourToken = mark.status === "available" ? "accent" : "text";
  const north = scene.frame.north;
  const foot = sub(mark.position, scale(north, dot(mark.position, north)));
  return [
    {
      kind: "line",
      from: screen(at),
      to: screen(project(foot, basis, camera, viewport)),
      stroke: colour,
      widthPx: STALK_WIDTH_PX,
      markId: mark.id,
    },
    {
      kind: "symbol",
      id: mark.id,
      centre: screen(at),
      shape: mark.shape,
      radiusPx: outerRadiusPx(mark, viewport) - SYMBOL_STROKE_PX / 2,
      stroke: colour,
      fill: above ? colour : null,
      widthPx: SYMBOL_STROKE_PX,
    },
  ];
}

function planeOps(
  scene: SpatialScene,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): DrawOp[] {
  const ops: DrawOp[] = [];
  const toScreen = (point: Vec3): ScreenPoint => screen(project(point, basis, camera, viewport));
  const grid = gridLines(scene.plane, scene.frame);
  for (const segment of [...grid.alongCoreward, ...grid.alongSpinward]) {
    ops.push({
      kind: "line",
      from: toScreen(segment.from),
      to: toScreen(segment.to),
      stroke: "line",
      widthPx: GRID_WIDTH_PX,
      markId: null,
    });
  }
  for (const ring of scene.plane.rings) {
    ops.push({
      kind: "polyline",
      points: ringPolyline(ring.radius, scene.frame).map(toScreen),
      stroke: "line",
      widthPx: GRID_WIDTH_PX,
    });
  }
  return ops;
}

function ticks(centre: ScreenPoint, radiusPx: number, lengthPx: number): TicksOp["segments"] {
  const segments: Array<{ from: ScreenPoint; to: ScreenPoint }> = [];
  for (let angleDeg = 0; angleDeg < 360; angleDeg += TICK_EVERY_DEG) {
    const angle = (angleDeg * Math.PI) / 180;
    const dx = Math.cos(angle);
    const dy = Math.sin(angle);
    segments.push({
      from: { xPx: centre.xPx + radiusPx * dx, yPx: centre.yPx + radiusPx * dy },
      to: {
        xPx: centre.xPx + (radiusPx + lengthPx) * dx,
        yPx: centre.yPx + (radiusPx + lengthPx) * dy,
      },
    });
  }
  return segments;
}

interface SphereGroup {
  readonly radius: number;
  readonly labels: string[];
  hasRange: boolean;
  hasDataEdge: boolean;
}

function sphereOps(
  scene: SpatialScene,
  camera: Camera,
  viewport: Viewport,
): { ops: DrawOp[]; labels: CurveLabel[] } {
  // One circle per distinct radius: a range sphere and a data edge of equal radius share it, with
  // the range's weight, the edge's ticks and both labels (plan 05, D13).
  const groups: SphereGroup[] = [];
  for (const sphere of scene.spheres) {
    let group = groups.find(
      (candidate) =>
        Math.abs(candidate.radius - sphere.radius) <=
        SAME_RADIUS_TOLERANCE * Math.max(candidate.radius, sphere.radius),
    );
    if (group === undefined) {
      group = { radius: sphere.radius, labels: [], hasRange: false, hasDataEdge: false };
      groups.push(group);
    }
    group.labels.push(sphere.label);
    if (sphere.role === "range") {
      group.hasRange = true;
    } else {
      group.hasDataEdge = true;
    }
  }

  const centre: ScreenPoint = { xPx: viewport.widthPx / 2, yPx: viewport.heightPx / 2 };
  const ops: DrawOp[] = [];
  const labels: CurveLabel[] = [];
  for (const [groupIndex, group] of groups.entries()) {
    const radiusPx = camera.pxPerUnit * group.radius;
    ops.push({
      kind: "circle",
      centre,
      radiusPx,
      stroke: "text",
      widthPx: group.hasRange ? RANGE_WIDTH_PX : DATA_EDGE_WIDTH_PX,
    });
    if (group.hasDataEdge) {
      ops.push({
        kind: "ticks",
        segments: ticks(centre, radiusPx, TICK_LENGTH_REM * viewport.remPx),
        stroke: "text",
        widthPx: DATA_EDGE_WIDTH_PX,
      });
    }
    for (const [stack, text] of group.labels.entries()) {
      labels.push({
        key: `sphere:${groupIndex}:${stack}`,
        text,
        placement: "circle-top",
        xPx: centre.xPx,
        yPx: centre.yPx - radiusPx,
        stack,
        centre,
        radiusPx,
      });
    }
  }
  return { ops, labels };
}

function reticleOps(
  placed: ReadonlyArray<PlacedMark>,
  scene: SpatialScene,
  viewport: Viewport,
): ReticleOp[] {
  const ops: ReticleOp[] = [];
  const marginPx = RETICLE_MARGIN_REM * viewport.remPx;
  const find = (id: string | null): PlacedMark | undefined =>
    id === null ? undefined : placed.find((candidate) => candidate.mark.id === id);
  const selected = find(scene.selectedId);
  if (selected !== undefined) {
    ops.push({
      kind: "reticle",
      id: selected.mark.id,
      centre: screen(selected.at),
      halfSizePx: outerRadiusPx(selected.mark, viewport) + marginPx / 2,
      stroke: "accent",
      widthPx: RETICLE_WIDTH_PX,
    });
  }
  const destination = find(scene.destinationId);
  if (destination !== undefined) {
    // Outside the selection's reticle when the destination is also the selection.
    const margins = destination === selected ? 2 : 1;
    ops.push({
      kind: "reticle",
      id: destination.mark.id,
      centre: screen(destination.at),
      halfSizePx: outerRadiusPx(destination.mark, viewport) + (margins * marginPx) / 2,
      stroke: "target",
      widthPx: RETICLE_WIDTH_PX,
    });
  }
  return ops;
}

function ringLabels(
  scene: SpatialScene,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): CurveLabel[] {
  const labels: CurveLabel[] = [];
  for (const [index, ring] of scene.plane.rings.entries()) {
    if (ring.label.length === 0) {
      continue;
    }
    const at = project(scale(scene.frame.coreward, ring.radius), basis, camera, viewport);
    labels.push({
      key: `ring:${index}`,
      text: ring.label,
      placement: "ring",
      xPx: at.xPx,
      yPx: at.yPx,
      stack: 0,
    });
  }
  return labels;
}

/**
 * Turns a scene and a camera into drawing instructions, mark anchors and curve-label positions.
 *
 * @remarks
 * The order is plan 05's design note D12: with the camera above the plane (ε ≥ 0) the marks below
 * it far to near, then the plane's grid and rings, then the marks above it far to near; with ε < 0
 * the halves swap. Each stalk comes just before its own symbol, and at equal depth the lower ID is
 * drawn last, on top, as `pick` would choose it.
 * The sphere outlines, the data edge's ticks and the reticles are screen annotations and come
 * last. Nothing is dimmed or scaled by depth: an op has no opacity, and a symbol's size comes from
 * its size class and the root font size alone. Colours are token names.
 */
export function buildDrawList(scene: SpatialScene, camera: Camera, viewport: Viewport): DrawList {
  const basis = viewBasis(scene.frame, camera);
  const placed: PlacedMark[] = scene.points.map((mark) => ({
    mark,
    at: project(mark.position, basis, camera, viewport),
    above: isAbovePlane(scene.frame, mark.position),
  }));
  const above = placed.filter((entry) => entry.above).toSorted(byFarToNear);
  const below = placed.filter((entry) => !entry.above).toSorted(byFarToNear);
  const [farHalf, nearHalf] = camera.elevationDeg >= 0 ? [below, above] : [above, below];

  const ops: DrawOp[] = [];
  for (const entry of farHalf) {
    ops.push(...markOps(entry, scene, basis, camera, viewport));
  }
  ops.push(...planeOps(scene, basis, camera, viewport));
  for (const entry of nearHalf) {
    ops.push(...markOps(entry, scene, basis, camera, viewport));
  }
  const spheres = sphereOps(scene, camera, viewport);
  ops.push(...spheres.ops, ...reticleOps(placed, scene, viewport));

  const anchors: Anchor[] = [...farHalf, ...nearHalf].map((entry) => ({
    id: entry.mark.id,
    xPx: entry.at.xPx,
    yPx: entry.at.yPx,
    depth: entry.at.depth,
    radiusPx: outerRadiusPx(entry.mark, viewport),
  }));
  return {
    ops,
    anchors,
    curveLabels: [...spheres.labels, ...ringLabels(scene, basis, camera, viewport)],
  };
}
