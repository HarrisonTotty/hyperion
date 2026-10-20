/**
 * The survey edge's mark (plan R10, R10.T10.c; Design note 12; the guide's `SURVEY EDGE`): where a
 * body's surveyed terrain meets ground the ship has not surveyed, a solid `--text` line `1px` wide
 * along the surveyed terrain's border, with a tick `0.25rem` long every `1rem` of the drawn line,
 * pointing into the unsurveyed side, cased as every mark is, and hidden by nearer ground as a limb
 * hides a graticule. The view's list notes it as `SURVEY EDGE` with the body's designation and the
 * range to its nearest point in the picture ({@link surveyEdgeNote}).
 *
 * @remarks
 * Pure: the edge is the coverage's ({@link surveyEdges}), each piece a side of a surveyed coarse
 * cell, joined into runs through the cells' corners with the surveyed cell on the left seen from
 * outside the body, so that the ticks' spacing runs on along the drawn line. Each side is drawn on
 * the spheroid in pieces of at most {@link SURVEY_EDGE_PIECE_PX} on the view, at the rim of the
 * step the skirts join (`terrainLit.wgsl`): the higher of the drawn ground's height there and the
 * datum's, so that the line stands on the edge of the higher surface and the step falls away from
 * it on one side. A piece is clipped, not dropped: at the spheroid's horizon, bisected along its
 * side to the crossing, and at the near plane, where the piece is a straight segment in view space
 * cut at its view depth, so that the line runs on to the view's edge beneath a ship low over it.
 * The line and the ticks are view-space segments, so that the draw's depth test hides them behind
 * nearer ground (R02's lines). The first view to draw a body's terrain (R10.T10.e's hand-over,
 * T11's wireframe) gives the view its coverage and heights; R10.T16.c lists the note
 * (decision-r10-t16-review §1), and T12 draws the line-of-sight mark the ticks keep clear of.
 *
 * The note's range counts only the points the operator can see (plan R10, R10.T16.c; the
 * r10-shader handoff's "For R10.T16", item b): where the view draws the body's terrain it gives the
 * drawn ground ({@link SurveyEdgeView.ground}), and a drawn piece's nearest point counts only where
 * no ground stands between it and the camera ({@link groundHidesPoint}, a march against the drawn
 * heights on the CPU), as the depth test hides the line itself behind that ground over the image.
 * Without it, as in the wireframe until R10.T11 draws its terrain, nothing but the spheroid's own
 * horizon hides a point, as nothing but the occluder spheres hides the line there.
 */
import { formatSurfaceRange, type SurfaceRangeUnit } from "../../lib/format";
import { add, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import {
  NEAR_PLANE_M,
  type ProjectionCamera,
  project,
  toViewAxes,
  type Viewport,
} from "../camera/projection";
import { rotationRows } from "../camera/quaternion";
import { narrow } from "../coords/narrow";
import { type Rotation3, rotateToBody, rotateToBodyFixed } from "../coords/rotation";
import { type ScreenPx, type ScreenSegment, segmentDistancePx } from "../wireframe/symbology";
import { type CoverageView, type SurveyEdge, surveyEdges } from "./coverage";
import { faceUvToXyz, stToUv, unitDir } from "./cube";
import { edgeNeighbour, type Face, FACES, facePoint, type PatchKey } from "./patchKey";
import type { BodyFigure } from "./planet";

/** The mark's name on the guide's nomenclature list, and its note in the view's list. */
export const SURVEY_EDGE_NOTE = "SURVEY EDGE";

/** A tick's length, rem: the guide's `0.25rem`. */
export const SURVEY_EDGE_TICK_REM = 0.25;

/** The ticks' spacing along the drawn line, rem: the guide's one every `1rem`. */
export const SURVEY_EDGE_TICK_SPACING_REM = 1;

/** How far the ticks keep from the line-of-sight mark's ticks, rem: the guide's 0.5. */
export const SURVEY_EDGE_TICK_CLEARANCE_REM = 0.5;

/**
 * The longest piece a cell's side is drawn in, device px: a side's curve on the spheroid then sags
 * from its chords by a small part of a pixel.
 */
export const SURVEY_EDGE_PIECE_PX = 8;

/**
 * The pieces a side is first cut into where an end lies off the view, behind the camera or below
 * the horizon; each is then halved where it is longer than {@link SURVEY_EDGE_PIECE_PX} on the view.
 */
export const SURVEY_EDGE_MAX_PIECES = 64;

/**
 * The most times a piece is halved: 2⁻²⁰ of a 64th of a level-5 cell's side on an Earth-sized body
 * is some 5 mm, finer than a pixel wherever the camera stands above the near plane's 0.1 m.
 */
const MAX_SPLITS = 20;

/** The bisection's steps to a side's horizon crossing: 2⁻⁴⁰ of a piece, far below a pixel. */
const HORIZON_STEPS = 40;

/**
 * The samples a line of sight is marched in from where it enters the ground's highest shell to its
 * point: a ridge across it is missed only where it is narrower than a 32nd of that stretch.
 */
export const SIGHT_SAMPLES = 32;

/**
 * How far a sample of a line of sight must stand below the drawn ground to be hidden, m: a
 * metre, so that the bilinear heights' differences from the drawn triangles beside the point hide
 * nothing.
 */
export const SIGHT_TOLERANCE_M = 1;

/**
 * The most drawn pieces' nearest points tested for sight, nearest first: a hand cap on the note's
 * cost a frame (each test a march of {@link SIGHT_SAMPLES}). Where every point tested is hidden and
 * more were drawn, the note keeps the nearest drawn point's range rather than none, conservatively,
 * so that it never drops while the edge is drawn (the batch review's C1).
 */
export const MAX_SIGHT_TESTS = 256;

/**
 * The ground a view draws over a body, which hides the edge's points behind it (R10.T16.c): its
 * terrain's resident heights (`ResidentHeightsView.groundHeightAtM` and `ceilingM`).
 */
export interface SurveyEdgeGround {
  /**
   * The drawn ground's height above the datum at a body-fixed unit direction, m; `null` where the
   * view holds none, the datum there.
   */
  readonly heightAtM: (direction: Vec3) => number | null;
  /** The highest the drawn ground stands above the datum, m. */
  readonly ceilingM: number;
}

/** A body's survey as a view draws its edge. */
export interface SurveyEdgeView {
  /** The body, which names its batch. */
  readonly body: string;
  readonly coverage: CoverageView;
  readonly figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">;
  /** Its rotation, body-fixed axes into the axes of the camera's frame. */
  readonly rotation: Rotation3;
  /** Its centre from the camera, m along the frame's axes, in `f64`. */
  readonly centreM: Vec3;
  /**
   * The drawn ground's height above the datum at a body-fixed unit direction, m, where the view
   * holds it (its terrain's resident bakes); `null` where it holds none, the mark there on the datum.
   */
  readonly heightAtM: (direction: Vec3) => number | null;
  /**
   * The ground the view draws over the body (R10.T16.c), which hides a point of the edge behind it
   * from the note's range; absent where the view draws none, as the wireframe until R10.T11.
   */
  readonly ground?: SurveyEdgeGround;
}

/** What the mark needs of the view beyond its camera. */
export interface SurveyEdgeOptions {
  /** The interface's rem, device px. */
  readonly remPx: number;
  /**
   * The line-of-sight mark's ticks (R10.T12's tripod), device px, which no tick comes within
   * {@link SURVEY_EDGE_TICK_CLEARANCE_REM} of, measured between the two segments on the view; none
   * where it is not drawn.
   */
  readonly keepClear: ReadonlyArray<ScreenSegment>;
  /**
   * Where the range is measured from, m from the camera along the frame's axes: the own ship, or
   * `null` with none, from the camera (the list's ranges, Design note 17 of R02).
   */
  readonly rangeFromM: Vec3 | null;
}

/** A survey edge's mark on one view. */
export interface SurveyEdgeMark {
  /** The line's segments, six `f32` each, m from the camera along the frame's axes. */
  readonly line: Float32Array;
  /** The ticks' segments, laid out as {@link SurveyEdgeMark.line}'s. */
  readonly ticks: Float32Array;
  /**
   * The range to the drawn line's nearest point inside the view that the operator can see, m, over
   * each segment as clipped to the view's edges, its point hidden by no drawn ground
   * ({@link SurveyEdgeView.ground}), or `null` where none is.
   */
  readonly nearestRangeM: number | null;
}

/** A corner of a coarse cell on the cube, integer, the same from every face that holds it. */
function cornerKey(face: Face, a: number, b: number, cells: number): string {
  return facePoint(face, 2 * a - cells, 2 * b - cells, cells).join(" ");
}

/** A side of a surveyed cell, from corner to corner with the cell on its left seen from outside. */
interface DirectedSide {
  readonly face: Face;
  /** The corners, (a, b) in cells from the face's corner. */
  readonly from: readonly [number, number];
  readonly to: readonly [number, number];
  readonly fromKey: string;
  readonly toKey: string;
  /** The unsurveyed neighbour's centre, a body-fixed unit direction, which the ticks point to. */
  readonly outward: Vec3;
}

/** The unit direction of the point (a, b) in cells of `face`, at `cells` a side. */
function directionAt(face: Face, a: number, b: number, cells: number): Vec3 {
  const [x, y, z] = unitDir(faceUvToXyz(face, stToUv(a / cells), stToUv(b / cells)));
  return vec3(x, y, z);
}

/** The unit direction of the point `t` of the way along `side`, uniform in the cell coordinates. */
function sideDirection(side: DirectedSide, t: number, cells: number): Vec3 {
  return directionAt(
    side.face,
    side.from[0] + t * (side.to[0] - side.from[0]),
    side.from[1] + t * (side.to[1] - side.from[1]),
    cells,
  );
}

/** `edge` directed with its surveyed cell on the left (each face's e₁ × e₂ is its outward axis). */
function directed(edge: SurveyEdge, cells: number): DirectedSide {
  const { face, i, j } = edge.cell;
  let from: readonly [number, number];
  let to: readonly [number, number];
  switch (edge.side) {
    case "VMin":
      from = [i, j];
      to = [i + 1, j];
      break;
    case "UMax":
      from = [i + 1, j];
      to = [i + 1, j + 1];
      break;
    case "VMax":
      from = [i + 1, j + 1];
      to = [i, j + 1];
      break;
    case "UMin":
      from = [i, j + 1];
      to = [i, j];
      break;
  }
  const neighbour: PatchKey = edgeNeighbour(edge.cell, edge.side);
  return {
    face,
    from,
    to,
    fromKey: cornerKey(face, from[0], from[1], cells),
    toKey: cornerKey(face, to[0], to[1], cells),
    outward: directionAt(neighbour.face, neighbour.i + 0.5, neighbour.j + 0.5, cells),
  };
}

/**
 * The edge's sides joined into runs: each run's sides end to end through their shared corners, a
 * run closed where it comes back to its start; at a corner where two runs cross, the first side
 * found from it goes on.
 */
function runsOf(sides: ReadonlyArray<DirectedSide>): DirectedSide[][] {
  const from = new Map<string, number[]>();
  sides.forEach((side, n) => {
    const list = from.get(side.fromKey);
    if (list === undefined) {
      from.set(side.fromKey, [n]);
    } else {
      list.push(n);
    }
  });
  const used = new Uint8Array(sides.length);
  const runs: DirectedSide[][] = [];
  for (let start = 0; start < sides.length; start += 1) {
    if (used[start] === 1) {
      continue;
    }
    const run: DirectedSide[] = [];
    for (let n: number | undefined = start; n !== undefined;) {
      const side: DirectedSide | undefined = sides[n];
      if (side === undefined) {
        break;
      }
      used[n] = 1;
      run.push(side);
      n = from.get(side.toKey)?.find((next) => used[next] === 0);
    }
    runs.push(run);
  }
  return runs;
}

/** A point of a side on the ground. */
interface EdgePoint {
  /** Its place along the side, 0 at its start to 1 at its end. */
  readonly t: number;
  /** From the camera, m along the frame's axes. */
  readonly fromCameraM: Vec3;
  /** Its distance along the camera's view axis, m: in front of the near plane from 0.1. */
  readonly wM: number;
  /** Whether it lies above the spheroid's horizon seen from the camera. */
  readonly above: boolean;
}

/** Whether a point is drawn as it stands: above the horizon and in front of the near plane. */
function seen(p: EdgePoint): boolean {
  return p.above && p.wM >= NEAR_PLANE_M;
}

/** Whether the body-fixed point `p` lies above the spheroid's horizon seen from `camera`. */
function aboveHorizon(
  p: Vec3,
  camera: Vec3,
  figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">,
): boolean {
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  // On the unit sphere the spheroid maps to, a point is seen where the camera is above its plane.
  const q = vec3(p.x / a, p.y / a, p.z / c);
  const e = vec3(camera.x / a, camera.y / a, camera.z / c);
  return (e.x - q.x) * q.x + (e.y - q.y) * q.y + (e.z - q.z) * q.z > 0;
}

/** A segment's six `f32`, narrowed once from `f64` (R02's packing). */
function pushSegment(out: number[], a: Vec3, b: Vec3): void {
  out.push(...narrow(a), ...narrow(b));
}

/**
 * The part of the segment from `a` to `b`, as fractions of it, that lies inside the view's four
 * side planes (Cyrus–Beck), or `null` where none does; `a` and `b` in the view's axes, in front.
 */
function insideView(a: Vec3, b: Vec3, s: number, aspect: number): readonly [number, number] | null {
  let t0 = 0;
  let t1 = 1;
  // Inside each plane where −z ± s x ≥ 0 and −z ± s a y ≥ 0.
  for (const [kx, ky] of [
    [s, 0],
    [-s, 0],
    [0, s * aspect],
    [0, -s * aspect],
  ] as const) {
    const fa = -a.z - kx * a.x - ky * a.y;
    const fb = -b.z - kx * b.x - ky * b.y;
    if (fa < 0 && fb < 0) {
      return null;
    }
    if (fa < 0) {
      t0 = Math.max(t0, fa / (fa - fb));
    } else if (fb < 0) {
      t1 = Math.min(t1, fa / (fa - fb));
    }
  }
  return t0 <= t1 ? [t0, t1] : null;
}

/**
 * The part of the screen segment from `p` to `q`, as fractions of it, that lies within `marginPx`
 * of the viewport (Liang–Barsky), or `null` where none does.
 */
function onViewport(
  p: ScreenPx,
  q: ScreenPx,
  viewport: Viewport,
  marginPx: number,
): readonly [number, number] | null {
  let u0 = 0;
  let u1 = 1;
  const dx = q.xPx - p.xPx;
  const dy = q.yPx - p.yPx;
  for (const [d, low, high, at] of [
    [dx, -marginPx, viewport.widthPx + marginPx, p.xPx],
    [dy, -marginPx, viewport.heightPx + marginPx, p.yPx],
  ] as const) {
    if (d === 0) {
      if (at < low || at > high) {
        return null;
      }
      continue;
    }
    const ua = (low - at) / d;
    const ub = (high - at) / d;
    u0 = Math.max(u0, Math.min(ua, ub));
    u1 = Math.min(u1, Math.max(ua, ub));
  }
  return u0 <= u1 ? [u0, u1] : null;
}

/**
 * The survey edge's mark of `view` on a camera's view: its line and ticks as view-space segments,
 * and the range to its nearest point inside the view.
 *
 * @remarks
 * Each piece of a side is drawn where it lies above the spheroid's horizon and in front of the near
 * plane, clipped exactly at each: at the horizon by bisection along the side to the crossing, at
 * the near plane by the piece's view depth, along which it is straight. A piece that is in the view
 * and longer than {@link SURVEY_EDGE_PIECE_PX} there is halved, so that the line follows the
 * ground beneath a camera close above it. The range is the least over the drawn segments, each as
 * clipped to the view's edges, so that a line running out of the view is measured where it crosses
 * the edge. A tick stands at every {@link SURVEY_EDGE_TICK_SPACING_REM} of a drawn run's length on
 * the view, the first half a spacing from the run's start, at right angles to the line on the view
 * and {@link SURVEY_EDGE_TICK_REM} long, towards the side the unsurveyed cell lies on there; its
 * far end is placed at its foot's depth, so that its length on the view is the guide's. Only the
 * ticks within a tick's length of the view are placed. A tick any of which would come within
 * {@link SURVEY_EDGE_TICK_CLEARANCE_REM} of the line-of-sight mark's ticks, segment to segment on
 * the view, is left out. The line is never dashed.
 */
export function surveyEdgeMark(
  view: SurveyEdgeView,
  camera: ProjectionCamera,
  viewport: Viewport,
  options: SurveyEdgeOptions,
): SurveyEdgeMark {
  const { coverage, figure, rotation, centreM } = view;
  const cells = 2 ** coverage.level;
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const cameraBodyFixedM = rotateToBodyFixed(rotation, scale(centreM, -1));
  // The spheroid's point of unit direction d is M d (R05's), its normal M⁻¹ d normalised.
  const groundAt = (d: Vec3): Vec3 => {
    const datum = vec3(a * d.x, a * d.y, c * d.z);
    const height = Math.max(view.heightAtM(d) ?? 0, 0);
    return height > 0
      ? add(datum, scale(normalise(vec3(d.x / a, d.y / a, d.z / c)), height))
      : datum;
  };
  const fromCamera = (p: Vec3): Vec3 => add(centreM, rotateToBody(rotation, p));
  const depthOf = (fromCameraM: Vec3): number => -toViewAxes(fromCameraM, camera.orientation).z;
  const pointAt = (side: DirectedSide, t: number): EdgePoint => {
    const ground = groundAt(sideDirection(side, t, cells));
    const fromCameraM = fromCamera(ground);
    return {
      t,
      fromCameraM,
      wM: depthOf(fromCameraM),
      above: aboveHorizon(ground, cameraBodyFixedM, figure),
    };
  };
  // A hundredth of a cell into the unsurveyed side of `side` at `t`, from the camera.
  const outwardAt = (side: DirectedSide, t: number): Vec3 => {
    const d = sideDirection(side, t, cells);
    return fromCamera(groundAt(normalise(add(d, scale(sub(side.outward, d), 0.01)))));
  };
  const screenOf = (fromCameraM: Vec3): ScreenPx => {
    const { xPx, yPx } = project(fromCameraM, camera, viewport);
    return { xPx, yPx };
  };
  const sides = FACES.flatMap((face) =>
    surveyEdges(coverage, face).map((edge) => directed(edge, cells)),
  );
  const line: number[] = [];
  const ticks: number[] = [];
  const remPx = options.remPx;
  const spacingPx = SURVEY_EDGE_TICK_SPACING_REM * remPx;
  const tickPx = SURVEY_EDGE_TICK_REM * remPx;
  const clearancePx = SURVEY_EDGE_TICK_CLEARANCE_REM * remPx;
  const [right, up] = cameraAxes(camera);
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const aspect = viewport.widthPx / viewport.heightPx;
  const rangeFrom = options.rangeFromM ?? vec3(0, 0, 0);
  // Each drawn segment's nearest point inside the view, from the camera, with its range.
  const nearest: Array<{ readonly rangeM: number; readonly pointM: Vec3 }> = [];
  // The length along the drawn run, px, and that at which the next tick stands.
  let along = 0;
  let nextTickPx = spacingPx / 2;
  const gap = (): void => {
    along = 0;
    nextTickPx = spacingPx / 2;
  };

  // The least range over the part of the segment from `pM` to `qM` inside the view.
  const measure = (pM: Vec3, qM: Vec3): void => {
    const inside = insideView(
      toViewAxes(pM, camera.orientation),
      toViewAxes(qM, camera.orientation),
      s,
      aspect,
    );
    if (inside === null) {
      return;
    }
    const d = sub(qM, pM);
    const length2 = dot(d, d);
    const t = length2 > 0 ? dot(sub(rangeFrom, pM), d) / length2 : 0;
    const at = Math.min(inside[1], Math.max(inside[0], t));
    const pointM = add(pM, scale(d, at));
    nearest.push({ rangeM: norm(sub(pointM, rangeFrom)), pointM });
  };

  // A drawn segment, both ends in front: its line, its range and its ticks, the tick's side from
  // `reference`, the piece's end farther in front, on `side`.
  const draw = (pM: Vec3, qM: Vec3, side: DirectedSide, reference: EdgePoint): void => {
    pushSegment(line, pM, qM);
    measure(pM, qM);
    const p = screenOf(pM);
    const q = screenOf(qM);
    const dx = q.xPx - p.xPx;
    const dy = q.yPx - p.yPx;
    const lengthPx = Math.hypot(dx, dy);
    if (!(lengthPx > 0)) {
      return;
    }
    const placed = onViewport(p, q, viewport, tickPx);
    if (placed !== null) {
      // The line's normal on the view, towards the unsurveyed side of the piece's side.
      const outward = project(outwardAt(side, reference.t), camera, viewport);
      const at = screenOf(reference.fromCameraM);
      let nx = -dy / lengthPx;
      let ny = dx / lengthPx;
      if (outward.inFront && (outward.xPx - at.xPx) * nx + (outward.yPx - at.yPx) * ny < 0) {
        nx = -nx;
        ny = -ny;
      }
      const firstPx = along + placed[0] * lengthPx;
      if (nextTickPx < firstPx) {
        nextTickPx += Math.ceil((firstPx - nextTickPx) / spacingPx) * spacingPx;
      }
      const wp = depthOf(pM);
      const wq = depthOf(qM);
      while (nextTickPx <= along + placed[1] * lengthPx) {
        const u = (nextTickPx - along) / lengthPx;
        nextTickPx += spacingPx;
        const foot: ScreenPx = { xPx: p.xPx + u * dx, yPx: p.yPx + u * dy };
        const end: ScreenPx = { xPx: foot.xPx + tickPx * nx, yPx: foot.yPx + tickPx * ny };
        // Left out where any of it comes within the clearance of the line-of-sight mark's ticks.
        if (
          options.keepClear.some((segment) => segmentDistancePx([foot, end], segment) < clearancePx)
        ) {
          continue;
        }
        // The segment's point that lands u of the way along it on the view: 1 ÷ w is linear there.
        const t = (u * wp) / (u * wp + (1 - u) * wq);
        const footM = add(pM, scale(sub(qM, pM), t));
        // A view offset of (Δx, Δy) px at a point w along the view's axis is (Δx, −Δy) × 2w ÷ (s W)
        // along the camera's right and up axes (`project`'s inverse at that depth).
        const metresPerPx = (2 * depthOf(footM)) / (s * viewport.widthPx);
        const endM = add(
          footM,
          add(scale(right, tickPx * nx * metresPerPx), scale(up, -tickPx * ny * metresPerPx)),
        );
        pushSegment(ticks, footM, endM);
      }
    }
    along += lengthPx;
  };

  // The point of `side` between `p` (above the horizon) and `q` (below) where it crosses, as the
  // last point above.
  const horizonCrossing = (side: DirectedSide, p: EdgePoint, q: EdgePoint): EdgePoint => {
    let above = p;
    let below = q;
    for (let step = 0; step < HORIZON_STEPS; step += 1) {
      const mid = pointAt(side, (above.t + below.t) / 2);
      if (mid.above) {
        above = mid;
      } else {
        below = mid;
      }
    }
    return above;
  };

  // The piece of `side` from `p` to `q`, drawn as far as it is seen, halved where it is long.
  const piece = (side: DirectedSide, p: EdgePoint, q: EdgePoint, splits: number): void => {
    if (!(p.above || q.above)) {
      gap();
      return;
    }
    if (p.above !== q.above) {
      if (p.above) {
        piece(side, p, horizonCrossing(side, p, q), splits);
        gap();
      } else {
        gap();
        piece(side, horizonCrossing(side, q, p), q, splits);
      }
      return;
    }
    if (p.wM < NEAR_PLANE_M && q.wM < NEAR_PLANE_M) {
      gap();
      return;
    }
    // Straight in view space, the piece meets the near plane where its depth is the plane's.
    const cut = (): Vec3 =>
      add(
        p.fromCameraM,
        scale(sub(q.fromCameraM, p.fromCameraM), (NEAR_PLANE_M - p.wM) / (q.wM - p.wM)),
      );
    const pM = p.wM < NEAR_PLANE_M ? cut() : p.fromCameraM;
    const qM = q.wM < NEAR_PLANE_M ? cut() : q.fromCameraM;
    const ps = screenOf(pM);
    const qs = screenOf(qM);
    if (
      splits < MAX_SPLITS &&
      Math.hypot(qs.xPx - ps.xPx, qs.yPx - ps.yPx) > SURVEY_EDGE_PIECE_PX &&
      insideView(
        toViewAxes(pM, camera.orientation),
        toViewAxes(qM, camera.orientation),
        s,
        aspect,
      ) !== null
    ) {
      const mid = pointAt(side, (p.t + q.t) / 2);
      piece(side, p, mid, splits + 1);
      piece(side, mid, q, splits + 1);
      return;
    }
    if (p.wM < NEAR_PLANE_M) {
      gap();
    }
    draw(pM, qM, side, p.wM >= q.wM ? p : q);
    if (q.wM < NEAR_PLANE_M) {
      gap();
    }
  };

  for (const run of runsOf(sides)) {
    gap();
    for (const side of run) {
      const first = pointAt(side, 0);
      const last = pointAt(side, 1);
      const span = (): number => {
        const p = screenOf(first.fromCameraM);
        const q = screenOf(last.fromCameraM);
        return Math.hypot(q.xPx - p.xPx, q.yPx - p.yPx);
      };
      const pieces =
        seen(first) && seen(last)
          ? Math.min(SURVEY_EDGE_MAX_PIECES, Math.max(1, Math.ceil(span() / SURVEY_EDGE_PIECE_PX)))
          : SURVEY_EDGE_MAX_PIECES;
      let p = first;
      for (let k = 1; k <= pieces; k += 1) {
        const q = k === pieces ? last : pointAt(side, k / pieces);
        piece(side, p, q, 0);
        p = q;
      }
    }
  }
  return {
    line: Float32Array.from(line),
    ticks: Float32Array.from(ticks),
    nearestRangeM: nearestSeenM(view, cameraBodyFixedM, nearest),
  };
}

/**
 * The least range of `nearest`'s points that no drawn ground hides from the camera, m, tested
 * nearest first, at most {@link MAX_SIGHT_TESTS} of them; the least of all where the view draws no
 * ground, and where the cap is used up with none seen and more points were drawn; `null` where
 * every point was tested and none is seen.
 */
function nearestSeenM(
  view: SurveyEdgeView,
  cameraBodyFixedM: Vec3,
  nearest: ReadonlyArray<{ readonly rangeM: number; readonly pointM: Vec3 }>,
): number | null {
  const ground = view.ground;
  if (ground === undefined) {
    return nearest.reduce<number | null>(
      (least, { rangeM }) => (least === null ? rangeM : Math.min(least, rangeM)),
      null,
    );
  }
  const ordered = nearest.toSorted((p, q) => p.rangeM - q.rangeM);
  const visible = ordered
    .slice(0, MAX_SIGHT_TESTS)
    .find(
      ({ pointM }) =>
        !groundHidesPoint(
          cameraBodyFixedM,
          rotateToBodyFixed(view.rotation, sub(pointM, view.centreM)),
          view.figure,
          ground,
        ),
    );
  if (visible !== undefined) {
    return visible.rangeM;
  }
  // The cap used up with none seen: a point not tested may be, so the nearest drawn point's range.
  return ordered.length > MAX_SIGHT_TESTS ? (ordered[0]?.rangeM ?? null) : null;
}

/**
 * Whether the drawn ground hides the body-fixed point `pointM` from a camera at `cameraM` (plan
 * R10, R10.T16.c): the line of sight marched in {@link SIGHT_SAMPLES} from where it enters the
 * shell of the ground's highest ({@link SurveyEdgeGround.ceilingM} above the equatorial radius,
 * which bounds every drawn point of an oblate figure) to just short of the point, a sample hidden
 * where it stands more than {@link SIGHT_TOLERANCE_M} below the drawn ground over it; its height
 * the offset from the datum's point of its parametric direction along that point's normal.
 * Nothing hides a point where no ground stands above the datum: the spheroid's own horizon is
 * the mark's to keep.
 *
 * @param cameraM - The camera from the body's centre, body-fixed, m.
 * @param pointM - The point from the body's centre, body-fixed, m.
 */
export function groundHidesPoint(
  cameraM: Vec3,
  pointM: Vec3,
  figure: Pick<BodyFigure, "equatorialRadiusM" | "polarRadiusM">,
  ground: SurveyEdgeGround,
): boolean {
  const topM = ground.ceilingM;
  if (!(topM > 0)) {
    return false;
  }
  const a = figure.equatorialRadiusM;
  const c = figure.polarRadiusM;
  const shellM = a + topM;
  const d = sub(pointM, cameraM);
  let t0 = 0;
  const outside = dot(cameraM, cameraM) - shellM * shellM;
  if (outside > 0) {
    // The nearer root of |C + t d|² = shell², where the line of sight enters the shell.
    const qa = dot(d, d);
    const qb = 2 * dot(cameraM, d);
    const discriminant = qb * qb - 4 * qa * outside;
    if (!(discriminant > 0)) {
      return false;
    }
    t0 = (-qb - Math.sqrt(discriminant)) / (2 * qa);
    if (!(t0 < 1)) {
      return false;
    }
    t0 = Math.max(0, t0);
  }
  for (let k = 0; k < SIGHT_SAMPLES; k += 1) {
    const q = add(cameraM, scale(d, t0 + ((1 - t0) * k) / SIGHT_SAMPLES));
    const direction = normalise(vec3(q.x / a, q.y / a, q.z / c));
    const datum = vec3(a * direction.x, a * direction.y, c * direction.z);
    const normal = normalise(vec3(direction.x / a, direction.y / a, direction.z / c));
    const heightM = dot(sub(q, datum), normal);
    if (heightM < (ground.heightAtM(direction) ?? 0) - SIGHT_TOLERANCE_M) {
      return true;
    }
  }
  return false;
}

/** The camera's right and up axes in its frame's axes: its orientation's first two columns. */
function cameraAxes(camera: ProjectionCamera): readonly [Vec3, Vec3] {
  const [r0, r1, r2] = rotationRows(camera.orientation);
  return [vec3(r0.x, r1.x, r2.x), vec3(r0.y, r1.y, r2.y)];
}

/** A survey edge's note in a view's list (plan R10, R10.T16.c; decision-r10-t16-review §1). */
export interface SurveyEdgeNote {
  /** The note as shown: `SURVEY EDGE TEST MOON 4.21 km`, `SURVEY EDGE TEST MOON 412 m FROM CAMERA`. */
  readonly text: string;
  /** Its range with its unit: `4.21 km`, `412 m`. */
  readonly range: string;
  /** Its range as read, after the designation: `4.21 km`, `412 m FROM CAMERA`. */
  readonly reading: string;
  /** The unit its range is in, which the next showing keeps within its hysteresis. */
  readonly unit: SurfaceRangeUnit;
}

/**
 * The view list's note of a body's survey edge (the guide's Views and `SURVEY EDGE` row;
 * decision-r10-t16-review §1): `SURVEY EDGE <designation> <range>`, the range to the edge's nearest
 * point in the picture in `SLANT RANGE`'s units ({@link formatSurfaceRange}: `m` below 1 km, three
 * figures, the unit kept within its hysteresis from `previous`), `FROM CAMERA` after the unit where
 * the scene has no own ship to measure from.
 *
 * @param designation - The body's designation, as the list's option for it names it.
 * @param rangeM - The range to the edge's nearest point seen, m (`DrawnSurveyEdge.nearestRangeM`).
 * @param fromCamera - Whether the range is from the camera, the scene having no own ship.
 * @param previous - The unit the note's range was last shown in, `null` for its first showing.
 */
export function surveyEdgeNote(
  designation: string,
  rangeM: number,
  fromCamera: boolean,
  previous: SurfaceRangeUnit | null,
): SurveyEdgeNote {
  const range = formatSurfaceRange(rangeM, previous);
  const shown = `${range.value} ${range.unit}`;
  const reading = fromCamera ? `${shown} FROM CAMERA` : shown;
  return {
    text: `${SURVEY_EDGE_NOTE} ${designation} ${reading}`,
    range: shown,
    reading,
    unit: range.unit,
  };
}

/**
 * A survey edge's note's accessible name, in the options' form (the guide's Views;
 * decision-r10-t16-review §1): `TEST MOON, survey edge, range 412 m FROM CAMERA`, with `, stale`
 * while the list's ranges are stale, as the options' names take it.
 */
export function surveyEdgeNoteName(
  designation: string,
  note: Pick<SurveyEdgeNote, "reading">,
  stale: boolean,
): string {
  return `${designation}, survey edge, range ${note.reading}${stale ? ", stale" : ""}`;
}
