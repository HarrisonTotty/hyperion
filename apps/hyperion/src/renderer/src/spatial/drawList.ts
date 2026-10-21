import {
  type Camera,
  project,
  type Projected,
  viewBasis,
  type ViewBasis,
  type Viewport,
} from "./camera";
import { apparentOffset } from "./apparent";
import {
  type AnnulusMark,
  isAbovePlane,
  type PathMark,
  type PathRole,
  type PointMark,
  type SegmentMark,
  type SpatialScene,
  type SymbolShape,
} from "./marks";
import { gridLines, ringPolyline } from "./plane";
import { SIZE_CLASS_REM, SYMBOL_STROKE_PX } from "./symbols";
import { type LocalFrame, planeFrame } from "../geometry/frame";
import { add, dot, norm, scale, sub, type Vec3 } from "../geometry/vec3";

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

/** A dash's lengths, CSS px: drawn, then left. */
export interface DashPx {
  readonly onPx: number;
  readonly offPx: number;
}

/**
 * A straight line: a mark's stalk (with its `markId`), a grid line, or a scene's segment or the
 * tick across one (plan 12, P12.T11).
 */
export interface LineOp {
  readonly kind: "line";
  readonly from: ScreenPoint;
  readonly to: ScreenPoint;
  readonly stroke: ColourToken;
  readonly widthPx: number;
  /**
   * The mark whose stalk this is, or `null` for the grid and a segment; the observer's stalk is
   * {@link OBSERVER_MARK_ID}'s.
   */
  readonly markId: string | null;
  /**
   * Its dash, which `paint` draws at the line scale as it does the width, as a dashed segment's;
   * solid when absent.
   */
  readonly dashPx?: DashPx | undefined;
}

/** A polyline, such as a ring on the reference plane, or the observer's mark. */
export interface PolylineOp {
  readonly kind: "polyline";
  readonly points: ReadonlyArray<ScreenPoint>;
  readonly stroke: ColourToken;
  readonly widthPx: number;
  /**
   * Which the polyline is: `line`, drawn at the line scale as every line is; or `mark`, drawn at
   * the mark stroke as a symbol's outline is and not moved out, as the observer's mark is (plan 12,
   * P12.T11). A line when absent.
   */
  readonly weight?: "line" | "mark" | undefined;
  /**
   * The fill of a closed polyline, its last point its first, as a mark is filled above the
   * reference plane, such as the observer's eye; not filled when absent or `null`.
   */
  readonly fill?: ColourToken | null | undefined;
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
  /**
   * The guide's outline, 1.5 CSS px. `paint` draws a mark's outline at `markStrokeDevicePx` of the
   * ratio whatever this says (R07.T16.f), so that it matches the view's.
   */
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
  /**
   * The guide's outline, 1.5 CSS px. `paint` draws a reticle at `markStrokeDevicePx` of the ratio
   * whatever this says (R07.T16.f), as it draws a symbol's outline.
   */
  readonly widthPx: number;
}

/**
 * The mark of a commanded destination: four open chevrons about a mark, one on each screen axis,
 * pointing at it (`destinationChevrons`; decision-r07-quality-and-destination, Q2; R07.T16.h).
 *
 * @remarks
 * Its apices stand `gapPx` outside the selection's bracket's place about the same mark, whether or
 * not the mark is selected, and each arm is as long as the bracket's arm there. `paint` moves both
 * out with the bracket, by four times the outline shift.
 */
export interface ChevronsOp {
  readonly kind: "chevrons";
  readonly id: string;
  readonly centre: ScreenPoint;
  /** The half-size of the selection's bracket about the mark, as a {@link ReticleOp} gives it. */
  readonly bracketHalfSizePx: number;
  /** How far outside the bracket's place the apices stand: {@link destinationGapPx}. */
  readonly gapPx: number;
  readonly stroke: ColourToken;
  /**
   * The guide's outline, 1.5 CSS px. `paint` draws the chevrons at `markStrokeDevicePx` of the
   * ratio whatever this says, as it draws a reticle.
   */
  readonly widthPx: number;
}

/** Short straight ticks, such as those along the edge of fetched data. */
export interface TicksOp {
  readonly kind: "ticks";
  readonly segments: ReadonlyArray<{ readonly from: ScreenPoint; readonly to: ScreenPoint }>;
  readonly stroke: ColourToken;
  readonly widthPx: number;
  /**
   * Which the ticks are: `line`, drawn at the line scale as every line is, such as a data edge's or
   * an annulus's ticks; or `mark`, drawn at the mark stroke as a symbol's outline is and not moved
   * out, such as the HR diagram's off-scale arrowhead, a state of the symbol beside it
   * (decision-r07-quality-and-destination, Q5; R07.T16.h).
   */
  readonly weight: "line" | "mark";
}

/** One drawing instruction, in the order the painter executes them. */
export type DrawOp = LineOp | PolylineOp | CircleOp | SymbolOp | ReticleOp | ChevronsOp | TicksOp;

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
  /**
   * Further points of its ring the label may stand beside where its own point has no room, as a
   * sphere's label moves round its circle: every 15° round the ring either way from its point, the
   * nearest first, out to the far side (R07.T16.j). A ring's and an annulus's outer edge's; absent
   * for a path's.
   */
  readonly alongPx?: ReadonlyArray<ScreenPoint>;
  /**
   * How far the label's text keeps from its point on every side, CSS px, where that is more than a
   * curve label's own room: the observer's, which names a mark (plan 12, P12.T11). A curve's room
   * when absent.
   */
  readonly clearOfPointPx?: number;
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

/**
 * The width each path is drawn at: the selected one 2 px, since `--text` against `--text-muted` is
 * only 1.88:1 and a stale view mutes both, so width, beside the reticle on the body, is the cue that
 * is not colour (the orchestrator's ruling 44.2).
 */
const PATH_WIDTH_PX: Readonly<Record<PathRole, number>> = {
  reference: 1,
  selected: 2,
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
    widthPx: PATH_WIDTH_PX[piece.path.role],
  };
}

/** The plane an annulus is drawn in, and its centre on that plane. */
interface BandPlane {
  readonly frame: LocalFrame;
  readonly centre: Vec3;
}

/**
 * The plane an annulus lies in: the reference plane, its centre moved along the plane's normal
 * onto it; or, with a normal of its own, that plane through its centre, the reference plane's
 * coreward laid onto it.
 */
function bandPlane(annulus: AnnulusMark, reference: LocalFrame): BandPlane {
  const centre = annulus.centre ?? { x: 0, y: 0, z: 0 };
  if (annulus.normal !== undefined) {
    return { frame: planeFrame(annulus.normal, reference.coreward), centre };
  }
  return {
    frame: reference,
    centre: sub(centre, scale(reference.north, dot(centre, reference.north))),
  };
}

/** The point on a plane at `radius` from `centre` towards `angleDeg` from its coreward. */
function onPlaneAt(frame: LocalFrame, centre: Vec3, radius: number, angleDeg: number): Vec3 {
  const angle = (angleDeg * Math.PI) / 180;
  return add(
    centre,
    add(
      scale(frame.coreward, radius * Math.cos(angle)),
      scale(frame.spinward, radius * Math.sin(angle)),
    ),
  );
}

/**
 * An annulus's edge ticks: at each edge every 10° from coreward, a tick of the data edge's length
 * on screen, pointing across the band towards its other edge, or outwards from a lone edge, and no
 * longer than half the band's width on screen.
 */
function edgeTickSegments(
  { frame, centre }: BandPlane,
  radii: ReadonlyArray<number>,
  toScreen: (point: Vec3) => ScreenPoint,
  lengthPx: number,
): TicksOp["segments"] {
  const segments: Array<{ from: ScreenPoint; to: ScreenPoint }> = [];
  const [inner, outer] = radii;
  for (const radius of radii) {
    // Towards the other edge; a lone edge towards twice its radius, outwards.
    const towards = outer === undefined ? 2 * radius : radius === inner ? outer : inner;
    if (radius <= 0 || towards === undefined) {
      continue;
    }
    for (let angleDeg = 0; angleDeg < 360; angleDeg += TICK_EVERY_DEG) {
      const from = toScreen(onPlaneAt(frame, centre, radius, angleDeg));
      const aim = toScreen(onPlaneAt(frame, centre, towards, angleDeg));
      const dx = aim.xPx - from.xPx;
      const dy = aim.yPx - from.yPx;
      const spanPx = Math.hypot(dx, dy);
      // Seen edge-on at this angle, the band has no width on screen to point across.
      if (!(spanPx > SAME_RADIUS_TOLERANCE)) {
        continue;
      }
      const reachPx = Math.min(lengthPx, outer === undefined ? lengthPx : spanPx / 2);
      segments.push({
        from,
        to: { xPx: from.xPx + (dx / spanPx) * reachPx, yPx: from.yPx + (dy / spanPx) * reachPx },
      });
    }
  }
  return segments;
}

function annulusOps(
  annulus: AnnulusMark,
  scene: SpatialScene,
  toScreen: (point: Vec3) => ScreenPoint,
  remPx: number,
): DrawOp[] {
  const ops: DrawOp[] = [];
  const band = bandPlane(annulus, scene.frame);
  const { frame, centre } = band;
  const { innerRadius, outerRadius } = annulus;
  // A selected band is drawn as the selected orbit is: brighter and wider, never by colour alone.
  const role: PathRole = annulus.selected === true ? "selected" : "reference";
  const stroke = PATH_STROKE[role];
  const widthPx = role === "selected" ? PATH_WIDTH_PX.selected : ANNULUS_WIDTH_PX;
  const radii = innerRadius === outerRadius ? [outerRadius] : [innerRadius, outerRadius];
  for (const radius of radii.filter((edge) => edge > 0)) {
    ops.push({
      kind: "polyline",
      points: ringPolyline(radius, frame).map((point) => toScreen(add(centre, point))),
      stroke,
      widthPx,
    });
  }
  if (annulus.ticks && outerRadius > innerRadius) {
    const segments: Array<{ from: ScreenPoint; to: ScreenPoint }> = [];
    for (let angleDeg = 0; angleDeg < 360; angleDeg += BELT_TICK_EVERY_DEG) {
      segments.push({
        from: toScreen(onPlaneAt(frame, centre, innerRadius, angleDeg)),
        to: toScreen(onPlaneAt(frame, centre, outerRadius, angleDeg)),
      });
    }
    ops.push({ kind: "ticks", segments, stroke, widthPx, weight: "line" });
  }
  if (annulus.edgeTicks === true) {
    const edges = radii.filter((edge) => edge > 0);
    const segments = edgeTickSegments(band, edges, toScreen, TICK_LENGTH_REM * remPx);
    if (segments.length > 0) {
      ops.push({ kind: "ticks", segments, stroke, widthPx, weight: "line" });
    }
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
    ops.push(...annulusOps(annulus, scene, toScreen, viewport.remPx));
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
        weight: "line",
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

/**
 * The selection's bracket's half-size about a mark of radius `markRadiusPx`, CSS px, before `paint`
 * moves it out: the mark's radius and half a margin, 0.25 rem. The bracket stands there whether or
 * not the mark is selected, so a label counts it either way (`placeLabels`).
 */
export function reticleHalfSizePx(markRadiusPx: number, remPx: number): number {
  return markRadiusPx + (RETICLE_MARGIN_REM * remPx) / 2;
}

/**
 * How far outside the selection's bracket's place the destination's chevrons' apices stand, CSS px:
 * half a margin, 0.25 rem, or the reticles' least gap where that is more, whether or not the
 * destination is also the selection (R07.T16.f and T16.h; decision-r07-t16d-followups, item 2;
 * decision-r07-quality-and-destination, Q2).
 *
 * @param minGapPx - The reticles' least gap, CSS px: `minReticleGapDevicePx` ÷ the ratio.
 */
export function destinationGapPx(remPx: number, minGapPx: number): number {
  return Math.max((RETICLE_MARGIN_REM * remPx) / 2, minGapPx);
}

/**
 * The selection's bracket, one margin larger than its mark ({@link reticleHalfSizePx}), and the
 * destination's chevrons, their apices {@link destinationGapPx} outside the bracket's place,
 * selected or not, so that a destination is told from the selection by its shape and the pair is
 * never told by colour alone (R07.T16.h; decision-r07-quality-and-destination, Q2).
 *
 * @remarks
 * The chevrons replace the as-built lone destination, a bracket in `--target` at the bracket's
 * place, and the pair's second bracket outside the first. Where the destination is also the
 * selection, the bracket keeps the corners and the apices stand over the open middle of its sides.
 *
 * @param minGapPx - The least space between the bracket's centreline and the apices, CSS px.
 */
function reticleOps(
  placed: ReadonlyArray<PlacedMark>,
  scene: SpatialScene,
  viewport: Viewport,
  minGapPx: number,
): Array<ReticleOp | ChevronsOp> {
  const ops: Array<ReticleOp | ChevronsOp> = [];
  const find = (id: string | null): PlacedMark | undefined =>
    id === null ? undefined : placed.find((candidate) => candidate.mark.id === id);
  const selected = find(scene.selectedId);
  if (selected !== undefined) {
    ops.push({
      kind: "reticle",
      id: selected.mark.id,
      centre: screen(selected.at),
      halfSizePx: reticleHalfSizePx(outerRadiusPx(selected.mark, viewport), viewport.remPx),
      stroke: "accent",
      widthPx: RETICLE_WIDTH_PX,
    });
  }
  const destination = find(scene.destinationId);
  if (destination !== undefined) {
    ops.push({
      kind: "chevrons",
      id: destination.mark.id,
      centre: screen(destination.at),
      bracketHalfSizePx: reticleHalfSizePx(
        outerRadiusPx(destination.mark, viewport),
        viewport.remPx,
      ),
      gapPx: destinationGapPx(viewport.remPx, minGapPx),
      stroke: "target",
      widthPx: RETICLE_WIDTH_PX,
    });
  }
  return ops;
}

/** How far apart the points along a ring that its label tries stand, in degrees round it. */
const RING_LABEL_STEP_DEG = 15;

/**
 * The angles from a ring label's own point at which it also tries the ring, nearest first: every
 * {@link RING_LABEL_STEP_DEG} either way, the far side last.
 */
const RING_LABEL_TURNS_DEG: ReadonlyArray<number> = Array.from(
  { length: 180 / RING_LABEL_STEP_DEG },
  (_, index) => (index + 1) * RING_LABEL_STEP_DEG,
).flatMap((turnDeg) => (turnDeg === 180 ? [180] : [turnDeg, -turnDeg]));

/**
 * The points along a ring of `radius` about `centre` on a plane that a label at `fromDeg` (from
 * the plane's coreward, towards its spinward) also tries, on the screen (R07.T16.j).
 */
function ringPointsPx(
  { frame, centre }: BandPlane,
  radius: number,
  fromDeg: number,
  toScreen: (point: Vec3) => ScreenPoint,
): ReadonlyArray<ScreenPoint> {
  return RING_LABEL_TURNS_DEG.map((turnDeg) =>
    toScreen(onPlaneAt(frame, centre, radius, fromDeg + turnDeg)),
  );
}

function ringLabels(
  scene: SpatialScene,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): CurveLabel[] {
  const labels: CurveLabel[] = [];
  const toScreen = (point: Vec3): ScreenPoint => screen(project(point, basis, camera, viewport));
  const reference: BandPlane = { frame: scene.frame, centre: { x: 0, y: 0, z: 0 } };
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
      alongPx: ringPointsPx(reference, ring.radius, 0, toScreen),
    });
  }
  return labels;
}

/**
 * The labels of the annuli, each at its outer edge's rimward point, or its spinward point where it
 * asks, away from the rings' labels at their coreward points; then those of the paths, each at its
 * anchor.
 */
function markCurveLabels(
  scene: SpatialScene,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): CurveLabel[] {
  const labels: CurveLabel[] = [];
  const toScreen = (point: Vec3): ScreenPoint => screen(project(point, basis, camera, viewport));
  for (const annulus of scene.annuli ?? []) {
    if (annulus.label.length === 0) {
      continue;
    }
    const band = bandPlane(annulus, scene.frame);
    const fromDeg = annulus.labelSpinward === true ? 90 : 180;
    const at = toScreen(onPlaneAt(band.frame, band.centre, annulus.outerRadius, fromDeg));
    labels.push({
      key: `annulus:${annulus.id}`,
      text: annulus.label,
      placement: "ring",
      xPx: at.xPx,
      yPx: at.yPx,
      stack: 0,
      alongPx: ringPointsPx(band, annulus.outerRadius, fromDeg, toScreen),
    });
  }
  for (const path of scene.paths ?? []) {
    if (path.label.length === 0) {
      continue;
    }
    const at = toScreen(path.labelAt);
    labels.push({
      key: `path:${path.id}`,
      text: path.label,
      placement: "ring",
      xPx: at.xPx,
      yPx: at.yPx,
      stack: 0,
    });
  }
  return labels;
}

/**
 * The dash of a segment drawn `dashed`, CSS px on and off: a predicted path's 6 and 4, restated from
 * the view's `PREDICTED_DASH_PX` (`view/wireframe/drawList.ts`), which owns it, so that a dash
 * means the same on every spatial display (plan 12, P12.T11). `paint` draws both at the line scale.
 */
export const SEGMENT_DASH_PX: DashPx = { onPx: 6, offPx: 4 };

/** The length of the tick across a segment's `to` end, in `rem` (§ Accessibility: sizes in rem). */
const SEGMENT_TICK_REM = 0.25;

/**
 * The `markId` of the observer's stalk, so that the labels keep clear of it as they do of a mark's
 * (`markInksPx`); no system's ID, which is sixteen hex digits.
 */
export const OBSERVER_MARK_ID = "observer";

/** Half the width of the observer's mark, in `rem`: 1 rem across, the largest symbol's size. */
const OBSERVER_HALF_WIDTH_REM = 0.5;

/**
 * How far outside the place of a selection's bracket about the observer's mark its label's text
 * starts, in `rem`, as a mark's label's does (the guide's 3D conventions).
 */
const OBSERVER_LABEL_GAP_REM = 0.25;

/** The points each arc of the observer's eye is drawn through, ends included. */
const OBSERVER_ARC_POINTS = 9;

/** The point a share `t` of the way from `from` to `to`. */
function along(from: ScreenPoint, to: ScreenPoint, t: number): ScreenPoint {
  return { xPx: from.xPx + (to.xPx - from.xPx) * t, yPx: from.yPx + (to.yPx - from.yPx) * t };
}

/**
 * The part of a line on the screen that lies inside the viewport, or `null` where none does
 * (Liang and Barsky's clip): the view is orthographic, so a scene's straight line is straight on
 * the screen and is clipped there.
 */
function clipToViewport(
  from: ScreenPoint,
  to: ScreenPoint,
  viewport: Viewport,
): readonly [ScreenPoint, ScreenPoint] | null {
  const dx = to.xPx - from.xPx;
  const dy = to.yPx - from.yPx;
  // Each edge as p·t ≤ q: left, right, top, bottom.
  const edges: ReadonlyArray<readonly [number, number]> = [
    [-dx, from.xPx],
    [dx, viewport.widthPx - from.xPx],
    [-dy, from.yPx],
    [dy, viewport.heightPx - from.yPx],
  ];
  let enter = 0;
  let leave = 1;
  for (const [p, q] of edges) {
    if (p === 0) {
      // Parallel to this edge: wholly outside it, or never crossing it.
      if (q < 0) {
        return null;
      }
      continue;
    }
    const t = q / p;
    if (p < 0) {
      enter = Math.max(enter, t);
    } else {
      leave = Math.min(leave, t);
    }
  }
  return leave > enter ? [along(from, to, enter), along(from, to, leave)] : null;
}

/**
 * The shares along a line on the screen, from `from` to `to`, of its parts outside a circle: none
 * for a line wholly inside it, and the whole line for one that misses it or only touches it.
 */
function outsideCircle(
  from: ScreenPoint,
  to: ScreenPoint,
  centre: ScreenPoint,
  radiusPx: number,
): ReadonlyArray<readonly [number, number]> {
  const dx = to.xPx - from.xPx;
  const dy = to.yPx - from.yPx;
  const fx = from.xPx - centre.xPx;
  const fy = from.yPx - centre.yPx;
  const a = dx * dx + dy * dy;
  const b = 2 * (fx * dx + fy * dy);
  const c = fx * fx + fy * fy - radiusPx * radiusPx;
  if (!(a > 0)) {
    return c > 0 ? [[0, 1]] : [];
  }
  const discriminant = b * b - 4 * a * c;
  if (!(discriminant > 0)) {
    return [[0, 1]];
  }
  const root = Math.sqrt(discriminant);
  const enters = (-b - root) / (2 * a);
  const leaves = (-b + root) / (2 * a);
  const parts: Array<readonly [number, number]> = [];
  if (enters > 0) {
    parts.push([0, Math.min(1, enters)]);
  }
  if (leaves < 1) {
    parts.push([Math.max(0, leaves), 1]);
  }
  return parts.filter(([start, end]) => end > start);
}

/**
 * A scene's segments on the screen (plan 12, P12.T11): each one whose length there, its
 * {@link apparentOffset}, exceeds its least, cut where it leaves its mark's outline and clipped to
 * the viewport, then the tick across its `to` end, square to it, where that end lies outside the
 * outline. Lines, with no mark of their own, so that none is picked.
 */
function segmentOps(
  scene: SpatialScene,
  placed: ReadonlyArray<PlacedMark>,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): LineOp[] {
  const ops: LineOp[] = [];
  const segments = scene.segments ?? [];
  if (segments.length === 0) {
    return ops;
  }
  // By ID, once per list: a chart observed has a segment for each of up to a few thousand marks,
  // and a drag builds the list at every frame.
  const marks = new Map(placed.map((entry) => [entry.mark.id, entry]));
  const lineOf = (segment: SegmentMark, from: ScreenPoint, to: ScreenPoint, dashed: boolean) => {
    const clipped = clipToViewport(from, to, viewport);
    if (clipped !== null) {
      ops.push({
        kind: "line",
        from: clipped[0],
        to: clipped[1],
        stroke: segment.stroke,
        widthPx: segment.widthPx,
        markId: null,
        ...(dashed ? { dashPx: SEGMENT_DASH_PX } : {}),
      });
    }
  };
  for (const segment of segments) {
    const offset = apparentOffset(segment.from, segment.to, scene.frame, camera);
    const lengthPx = Math.hypot(offset.dxPx, offset.dyPx);
    if (!(lengthPx > (segment.minLengthPx ?? 0))) {
      continue;
    }
    const from = screen(project(segment.from, basis, camera, viewport));
    const to: ScreenPoint = { xPx: from.xPx + offset.dxPx, yPx: from.yPx + offset.dyPx };
    const leaves = segment.fromMark === undefined ? undefined : marks.get(segment.fromMark);
    const centre = leaves === undefined ? null : screen(leaves.at);
    const radiusPx = leaves === undefined ? 0 : outerRadiusPx(leaves.mark, viewport);
    const pieces: ReadonlyArray<readonly [number, number]> =
      centre === null ? [[0, 1]] : outsideCircle(from, to, centre, radiusPx);
    for (const [start, end] of pieces) {
      lineOf(segment, along(from, to, start), along(from, to, end), segment.line === "dashed");
    }
    const endOutside =
      centre === null || Math.hypot(to.xPx - centre.xPx, to.yPx - centre.yPx) > radiusPx;
    if (segment.endTick === true && endOutside) {
      const halfPx = (SEGMENT_TICK_REM * viewport.remPx) / 2;
      const acrossX = (-offset.dyPx / lengthPx) * halfPx;
      const acrossY = (offset.dxPx / lengthPx) * halfPx;
      lineOf(
        segment,
        { xPx: to.xPx + acrossX, yPx: to.yPx + acrossY },
        { xPx: to.xPx - acrossX, yPx: to.yPx - acrossY },
        false,
      );
    }
  }
  return ops;
}

/**
 * The observer's eye about `centre`: two arcs from a point `halfWidthPx` to its left to one as far
 * to its right, each a quadratic curve whose control point stands `halfWidthPx` above or below the
 * centre, so that the eye is half as tall as it is wide. The polyline starts and ends at the top
 * of its upper arc, where the outline is smooth, so that its seam does not blunt a point.
 */
function eyePoints(centre: ScreenPoint, halfWidthPx: number): ScreenPoint[] {
  // x = a(2t − 1), y = ∓2a·t(1 − t) on the upper and lower arcs, t from 0 to 1 left to right.
  const point = (t: number, upper: boolean): ScreenPoint => ({
    xPx: centre.xPx + halfWidthPx * (2 * t - 1),
    yPx: centre.yPx + (upper ? -2 : 2) * halfWidthPx * t * (1 - t),
  });
  const steps = OBSERVER_ARC_POINTS - 1;
  const half = steps / 2;
  const points: ScreenPoint[] = [];
  for (let step = half; step <= steps; step += 1) {
    points.push(point(step / steps, true));
  }
  for (let step = steps - 1; step >= 0; step -= 1) {
    points.push(point(step / steps, false));
  }
  for (let step = 1; step <= half; step += 1) {
    points.push(point(step / steps, true));
  }
  return points;
}

/**
 * The observer's stalk and mark, and its label, where it lies inside the view (plan 12, P12.T11):
 * the stalk to its foot on the reference plane in `--text`, as a mark's is, and the open eye at a
 * mark's outline. Neither gives an anchor, so the observer is never picked.
 */
function observerOps(
  scene: SpatialScene,
  basis: ViewBasis,
  camera: Camera,
  viewport: Viewport,
): { readonly ops: ReadonlyArray<DrawOp>; readonly labels: ReadonlyArray<CurveLabel> } {
  const observer = scene.observer;
  if (observer === undefined) {
    return { ops: [], labels: [] };
  }
  const at = project(observer.position, basis, camera, viewport);
  const inside =
    at.xPx >= 0 && at.xPx <= viewport.widthPx && at.yPx >= 0 && at.yPx <= viewport.heightPx;
  if (!inside) {
    return { ops: [], labels: [] };
  }
  const north = scene.frame.north;
  const foot = sub(observer.position, scale(north, dot(observer.position, north)));
  const centre = screen(at);
  const outerHalfWidthPx = OBSERVER_HALF_WIDTH_REM * viewport.remPx;
  return {
    ops: [
      {
        kind: "line",
        from: centre,
        to: screen(project(foot, basis, camera, viewport)),
        stroke: "text",
        widthPx: STALK_WIDTH_PX,
        markId: OBSERVER_MARK_ID,
      },
      {
        kind: "polyline",
        points: eyePoints(centre, outerHalfWidthPx - SYMBOL_STROKE_PX / 2),
        stroke: "text",
        widthPx: SYMBOL_STROKE_PX,
        weight: "mark",
        // Filled north of the plane and open south of it, as every mark is (the guide's 3D
        // conventions: fill means nothing else).
        fill: isAbovePlane(scene.frame, observer.position) ? "text" : null,
      },
    ],
    labels:
      observer.label.length === 0
        ? []
        : [
            {
              key: "observer",
              text: observer.label,
              placement: "ring",
              xPx: centre.xPx,
              yPx: centre.yPx,
              stack: 0,
              // As a mark's label stands: 0.25 rem outside the place of a selection's bracket.
              clearOfPointPx:
                reticleHalfSizePx(outerHalfWidthPx, viewport.remPx) +
                OBSERVER_LABEL_GAP_REM * viewport.remPx,
            },
          ],
  };
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
 * reference ones. Each piece is a polyline, 1 px wide in `--text-muted` for a reference path and
 * 2 px wide in `--text` for the selected one. An annulus is drawn with the plane, after its rings: each edge a
 * `--text-muted` polyline and, for a belt, one `--text-muted` `ticks` op of radial segments between
 * the edges every 10° from coreward; an annulus with edge ticks, the optimistic habitable zone,
 * then one `ticks` op of short ticks from each edge into the band. An annulus with a normal of its
 * own, a ring tilted to its planet's equatorial plane, is drawn so in its own plane, its angles from
 * the reference plane's coreward laid onto it. Neither a path nor an annulus gives an anchor, so neither is
 * picked, and their labels follow the rings' in the curve labels. `--line` is left to the grid and
 * the plane's rings (the orchestrator's ruling 35).
 *
 * A scene's segments and its observer (plan 12, P12.T11) change nothing else in the order. The
 * segments come first, before every mark, so that a joiner never crosses another system's symbol
 * as no path does. Each is a line, dashed at {@link SEGMENT_DASH_PX} where it is `dashed`, drawn
 * where its length on the screen exceeds its least, only outside the outline of the mark it
 * leaves, and clipped to the viewport, so that one wholly outside draws nothing; its tick follows
 * it. The observer, drawn only where it lies inside the view, comes after every mark and before
 * the screen annotations: its stalk, then its eye, a polyline at a mark's outline, filled above the
 * plane; its label follows the paths' in the curve labels and stands 0.25 rem outside the place of
 * a selection's bracket about it, as a mark's label does. Neither gives an anchor, so neither is
 * picked.
 *
 * The list holds the guide's CSS widths; `paint` draws them at no less than 2 device px
 * (R07.T16.f). A destination is four chevrons pointing at its mark, their apices at least
 * `minReticleGapPx` outside the selection's bracket's place, or 0.25 rem where that is more,
 * whether or not it is the selection (R07.T16.h).
 *
 * @param minReticleGapPx - The least space between the selection's bracket's centreline and the
 *   destination's chevrons' apices about one mark, CSS px: `minReticleGapDevicePx` of the
 *   device-pixel ratio ÷ the ratio, an outline and a casing (5.12, 4, 2.5 and 2.5 px at 0.78125, 1,
 *   2 and 3), so that the pair is the view's.
 */
export function buildDrawList(
  scene: SpatialScene,
  camera: Camera,
  viewport: Viewport,
  minReticleGapPx: number,
): DrawList {
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

  // Before every mark, so that a joiner never crosses another system's symbol.
  const ops: DrawOp[] = [...segmentOps(scene, placed, basis, camera, viewport)];
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
  const observer = observerOps(scene, basis, camera, viewport);
  ops.push(...observer.ops);
  const spheres = sphereOps(scene, camera, viewport);
  ops.push(...spheres.ops, ...reticleOps(placed, scene, viewport, minReticleGapPx));

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
      ...observer.labels,
    ],
  };
}
