import {
  type Camera,
  project,
  type Projected,
  viewBasis,
  type ViewBasis,
  type Viewport,
} from "./camera";
import {
  type AnnulusMark,
  isAbovePlane,
  type PathMark,
  type PathRole,
  type PointMark,
  type SpatialScene,
  type SymbolShape,
} from "./marks";
import { gridLines, ringPolyline } from "./plane";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX } from "./symbols";
import { add, dot, norm, scale, sub, type Vec3 } from "./vec3";

/**
 * A colour token an op is drawn in, named after its CSS custom property in camel case (`text` for
 * `--text`, `textMuted` for `--text-muted`).
 *
 * @remarks
 * The draw list never holds a colour: the painter resolves the name against the stylesheet.
 */
export type ColourToken = "text" | "textMuted" | "accent" | "target" | "line";

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

/**
 * The label of a curve beside one of its points: a ring on the reference plane at its coreward
 * point, an annulus at its outer edge's rimward point, a path at its label anchor.
 */
export interface RingLabel extends CurveLabelBase {
  readonly placement: "ring";
}

/** Where to put the DOM label of a sphere, a ring, an annulus or a path. */
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
const PATH_WIDTH_PX = 1;
const ANNULUS_WIDTH_PX = 1;
/** How often a belt's radial ticks join its edges. */
const BELT_TICK_EVERY_DEG = 10;
/**
 * Heights within this share of a path's reach, the largest distance of its points from the view
 * centre, count as on the reference plane, and so above it: an orbit drawn in the plane is one
 * piece, not one split at every rounding error.
 */
const ON_PLANE_SHARE = 1e-9;

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

/** A run of a path that lies wholly on one side of the reference plane. */
interface PathPiece {
  readonly path: PathMark;
  readonly points: ReadonlyArray<Vec3>;
  readonly above: boolean;
}

function samePoint(a: Vec3, b: Vec3): boolean {
  return a.x === b.x && a.y === b.y && a.z === b.z;
}

/**
 * A path cut where it crosses the reference plane, into runs that each lie on one side of it.
 *
 * @remarks
 * A point on the plane counts as above it, as a mark's does ({@link isAbovePlane}), to within
 * {@link ON_PLANE_SHARE} of the path's reach. Each cut is where the segment meets the plane, which
 * ends one run and starts the next, so the runs join. A closed path whose first and last runs lie
 * on the same side has them joined into one.
 */
function splitAtPlane(path: PathMark, north: Vec3): PathPiece[] {
  const first = path.points[0];
  if (first === undefined || path.points.length < 2) {
    return [];
  }
  const reach = Math.max(...path.points.map(norm));
  const level = -ON_PLANE_SHARE * reach;
  const height = (point: Vec3): number => dot(point, north);
  const runs: Array<{ points: Vec3[]; above: boolean }> = [];
  let current = { points: [first], above: height(first) >= level };
  let previous = first;
  for (const point of path.points.slice(1)) {
    const above = height(point) >= level;
    if (above !== current.above) {
      // At the plane itself; a point within the tolerance of it is its own crossing.
      const from = height(previous);
      const share = Math.min(1, Math.max(0, from / (from - height(point))));
      const crossing = add(previous, scale(sub(point, previous), share));
      current.points.push(crossing);
      runs.push(current);
      current = { points: [crossing], above };
    }
    current.points.push(point);
    previous = point;
  }
  runs.push(current);
  const last = runs.at(-1);
  const opening = runs[0];
  const closed = samePoint(first, previous);
  if (closed && runs.length > 1 && last !== undefined && opening !== undefined) {
    if (last.above === opening.above) {
      runs[0] = { points: [...last.points, ...opening.points.slice(1)], above: last.above };
      runs.pop();
    }
  }
  return runs
    .filter((run) => run.points.length >= 2)
    .map((run) => ({ path, points: run.points, above: run.above }));
}

/** The pieces of every path, reference paths before the selected one in each half. */
function pathPieces(scene: SpatialScene): { above: PathPiece[]; below: PathPiece[] } {
  const pieces = (scene.paths ?? []).flatMap((path) => splitAtPlane(path, scene.frame.north));
  // A polyline spans a range of depths, so no single order of whole pieces is right for depth; the
  // selected path goes last in its half so that no reference line crosses the one that carries
  // meaning.
  const ordered = [
    ...pieces.filter((piece) => piece.path.role === "reference"),
    ...pieces.filter((piece) => piece.path.role === "selected"),
  ];
  return {
    above: ordered.filter((piece) => piece.above),
    below: ordered.filter((piece) => !piece.above),
  };
}

/**
 * The token each path is drawn in: `--text-muted` for a reference path, which says where something
 * lies and so takes the guide's 6:1 for the parts of a symbol that carry meaning, which `--line`
 * does not reach; `--text` for the selected one (the orchestrator's ruling 35).
 */
const PATH_STROKE: Readonly<Record<PathRole, ColourToken>> = {
  reference: "textMuted",
  selected: "text",
};

function pathOp(
  piece: PathPiece,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): PolylineOp {
  return {
    kind: "polyline",
    points: piece.points.map((point) => screen(project(point, basis, camera, viewport))),
    stroke: PATH_STROKE[piece.path.role],
    widthPx: PATH_WIDTH_PX,
  };
}

/** An annulus's centre moved along the plane's normal onto the plane. */
function footOnPlane(annulus: AnnulusMark, north: Vec3): Vec3 {
  const centre = annulus.centre ?? { x: 0, y: 0, z: 0 };
  return sub(centre, scale(north, dot(centre, north)));
}

/** The point on the reference plane at `radius` from `centre` towards `angleDeg` from coreward. */
function onPlaneAt(scene: SpatialScene, centre: Vec3, radius: number, angleDeg: number): Vec3 {
  const angle = (angleDeg * Math.PI) / 180;
  return add(
    centre,
    add(
      scale(scene.frame.coreward, radius * Math.cos(angle)),
      scale(scene.frame.spinward, radius * Math.sin(angle)),
    ),
  );
}

function annulusOps(
  annulus: AnnulusMark,
  scene: SpatialScene,
  toScreen: (point: Vec3) => ScreenPoint,
): DrawOp[] {
  const ops: DrawOp[] = [];
  const centre = footOnPlane(annulus, scene.frame.north);
  const { innerRadius, outerRadius } = annulus;
  const radii = innerRadius === outerRadius ? [outerRadius] : [innerRadius, outerRadius];
  for (const radius of radii.filter((edge) => edge > 0)) {
    ops.push({
      kind: "polyline",
      points: ringPolyline(radius, scene.frame).map((point) => toScreen(add(centre, point))),
      stroke: "textMuted",
      widthPx: ANNULUS_WIDTH_PX,
    });
  }
  if (annulus.ticks && outerRadius > innerRadius) {
    const segments: Array<{ from: ScreenPoint; to: ScreenPoint }> = [];
    for (let angleDeg = 0; angleDeg < 360; angleDeg += BELT_TICK_EVERY_DEG) {
      segments.push({
        from: toScreen(onPlaneAt(scene, centre, innerRadius, angleDeg)),
        to: toScreen(onPlaneAt(scene, centre, outerRadius, angleDeg)),
      });
    }
    ops.push({ kind: "ticks", segments, stroke: "textMuted", widthPx: ANNULUS_WIDTH_PX });
  }
  return ops;
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
  for (const annulus of scene.annuli ?? []) {
    ops.push(...annulusOps(annulus, scene, toScreen));
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
 * The labels of the annuli, each at its outer edge's rimward point, away from the rings' labels at
 * their coreward points; then those of the paths, each at its anchor.
 */
function markCurveLabels(
  scene: SpatialScene,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): CurveLabel[] {
  const labels: CurveLabel[] = [];
  const pointLabel = (key: string, text: string, point: Vec3): void => {
    if (text.length === 0) {
      return;
    }
    const at = project(point, basis, camera, viewport);
    labels.push({ key, text, placement: "ring", xPx: at.xPx, yPx: at.yPx, stack: 0 });
  };
  for (const annulus of scene.annuli ?? []) {
    const centre = footOnPlane(annulus, scene.frame.north);
    pointLabel(
      `annulus:${annulus.id}`,
      annulus.label,
      onPlaneAt(scene, centre, annulus.outerRadius, 180),
    );
  }
  for (const path of scene.paths ?? []) {
    pointLabel(`path:${path.id}`, path.label, path.labelAt);
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
 *
 * A scene's paths and annuli (plan 14, T40.a) add to that order and change nothing else in it. A
 * path is cut where it crosses the plane, and each piece opens its own half, before the half's
 * marks, so that no line crosses a symbol; in each half the selected path comes after the
 * reference ones. Each piece is a polyline 1 px wide, in `--text-muted` for a reference path and
 * `--text` for the selected one. An annulus is drawn with the plane, after its rings: each edge a
 * `--text-muted` polyline and, for a belt, one `--text-muted` `ticks` op of radial segments between
 * the edges every 10° from coreward. Neither a path nor an annulus gives an anchor, so neither is
 * picked, and their labels follow the rings' in the curve labels. `--line` is left to the grid and
 * the plane's rings (the orchestrator's ruling 35).
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
  const cameraAbove = camera.elevationDeg >= 0;
  const [farHalf, nearHalf] = cameraAbove ? [below, above] : [above, below];
  const pieces = pathPieces(scene);
  const [farPieces, nearPieces] = cameraAbove
    ? [pieces.below, pieces.above]
    : [pieces.above, pieces.below];

  const ops: DrawOp[] = [];
  for (const piece of farPieces) {
    ops.push(pathOp(piece, basis, camera, viewport));
  }
  for (const entry of farHalf) {
    ops.push(...markOps(entry, scene, basis, camera, viewport));
  }
  ops.push(...planeOps(scene, basis, camera, viewport));
  for (const piece of nearPieces) {
    ops.push(pathOp(piece, basis, camera, viewport));
  }
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
    curveLabels: [
      ...spheres.labels,
      ...ringLabels(scene, basis, camera, viewport),
      ...markCurveLabels(scene, basis, camera, viewport),
    ],
  };
}
