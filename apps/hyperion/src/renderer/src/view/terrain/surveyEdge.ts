/**
 * The survey edge's mark (plan R10, R10.T10.c; Design note 12; the guide's `SURVEY EDGE`): where a
 * body's surveyed terrain meets ground the ship has not surveyed, a solid `--text` line `1px` wide
 * along the surveyed terrain's border, with a tick `0.25rem` long every `1rem` of the drawn line,
 * pointing into the unsurveyed side, cased as every mark is, and hidden by nearer ground as a limb
 * hides a graticule. The view's list notes it as `SURVEY EDGE` with the range to its nearest point
 * in the picture.
 *
 * @remarks
 * Pure: the edge is the coverage's ({@link surveyEdges}), each piece a side of a surveyed coarse
 * cell, joined into runs through the cells' corners with the surveyed cell on the left seen from
 * outside the body, so that the ticks' spacing runs on along the drawn line. Each side is drawn on
 * the spheroid in pieces of at most {@link SURVEY_EDGE_PIECE_PX} on the view, at the rim of the
 * step the skirts join (`terrainLit.wgsl`): the higher of the drawn ground's height there and the
 * datum's, so that the line stands on the edge of the higher surface and the step falls away from
 * it on one side. A point is drawn while it lies in front of the camera and above the spheroid's
 * horizon; the line and the ticks are view-space segments, so that the draw's depth test hides them
 * behind nearer ground (R02's lines). The first view to draw a body's terrain (R10.T10.e's
 * hand-over, T11's wireframe) gives the view its coverage and heights; T12 lists the note and draws
 * the line-of-sight mark the ticks keep clear of.
 */
import { type BodyDistanceUnit, formatBodyDistance } from "../../lib/format";
import { add, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { type ProjectionCamera, project, toViewAxes, type Viewport } from "../camera/projection";
import { rotationRows } from "../camera/quaternion";
import { narrow } from "../coords/narrow";
import { type Rotation3, rotateToBody, rotateToBodyFixed } from "../coords/rotation";
import type { ScreenPx, ScreenSegment } from "../wireframe/symbology";
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

/** The most pieces one side is drawn in: the cap where a side's ends lie off the view or behind. */
export const SURVEY_EDGE_MAX_PIECES = 64;

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
}

/** What the mark needs of the view beyond its camera. */
export interface SurveyEdgeOptions {
  /** The interface's rem, device px. */
  readonly remPx: number;
  /**
   * The line-of-sight mark's ticks (R10.T12's tripod), device px, which no tick comes within
   * {@link SURVEY_EDGE_TICK_CLEARANCE_REM} of; none until it is drawn.
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
  /** The range to the drawn edge's nearest point inside the view, m, or `null` where none is. */
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

/** A point of the drawn edge. */
interface EdgePoint {
  /** From the camera, m along the frame's axes. */
  readonly fromCameraM: Vec3;
  readonly screen: ScreenPx;
  /** Whether it is drawn: in front of the camera and above the spheroid's horizon. */
  readonly drawn: boolean;
  /**
   * The unsurveyed side of the side it was placed on, a point a little into it, as
   * {@link fromCameraM}: for every point but a run's first, the side of the piece it ends.
   */
  readonly outwardM: Vec3;
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

/** The squared distance, px², from `p` to the segment `s`. */
function squaredDistanceToSegment(p: ScreenPx, s: ScreenSegment): number {
  const [a, b] = s;
  const dx = b.xPx - a.xPx;
  const dy = b.yPx - a.yPx;
  const length2 = dx * dx + dy * dy;
  const t =
    length2 > 0
      ? Math.min(1, Math.max(0, ((p.xPx - a.xPx) * dx + (p.yPx - a.yPx) * dy) / length2))
      : 0;
  const x = a.xPx + t * dx - p.xPx;
  const y = a.yPx + t * dy - p.yPx;
  return x * x + y * y;
}

/** A segment's six `f32`, narrowed once from `f64` (R02's packing). */
function pushSegment(out: number[], a: Vec3, b: Vec3): void {
  out.push(...narrow(a), ...narrow(b));
}

/**
 * The survey edge's mark of `view` on a camera's view: its line and ticks as view-space segments,
 * and the range to its nearest point inside the view.
 *
 * @remarks
 * A tick stands at every {@link SURVEY_EDGE_TICK_SPACING_REM} of a drawn run's length on the view,
 * the first half a spacing from the run's start, at right angles to the line on the view and
 * {@link SURVEY_EDGE_TICK_REM} long, towards the side the unsurveyed cell lies on there; its far
 * end is placed at its foot's depth, so that its length on the view is the guide's. A tick that
 * would come within {@link SURVEY_EDGE_TICK_CLEARANCE_REM} of the line-of-sight mark's ticks is
 * left out. The line is never dashed.
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
  const sides = FACES.flatMap((face) =>
    surveyEdges(coverage, face).map((edge) => directed(edge, cells)),
  );
  const line: number[] = [];
  const ticks: number[] = [];
  const remPx = options.remPx;
  const spacingPx = SURVEY_EDGE_TICK_SPACING_REM * remPx;
  const tickPx = SURVEY_EDGE_TICK_REM * remPx;
  const clearance2 = (SURVEY_EDGE_TICK_CLEARANCE_REM * remPx) ** 2;
  const [right, up] = cameraAxes(camera);
  const s = 1 / Math.tan(camera.fovXRad / 2);
  const rangeFrom = options.rangeFromM ?? vec3(0, 0, 0);
  let nearestRangeM: number | null = null;
  for (const run of runsOf(sides)) {
    const points = runPoints(
      run,
      cells,
      groundAt,
      fromCamera,
      (p) => aboveHorizon(p, cameraBodyFixedM, figure),
      camera,
      viewport,
    );
    // The length along the drawn run, px, at which the next tick stands.
    let along = 0;
    let nextTickPx = spacingPx / 2;
    for (let k = 1; k < points.length; k += 1) {
      const p = points[k - 1];
      const q = points[k];
      if (p === undefined || q === undefined) {
        continue;
      }
      if (!(p.drawn && q.drawn)) {
        along = 0;
        nextTickPx = spacingPx / 2;
        continue;
      }
      pushSegment(line, p.fromCameraM, q.fromCameraM);
      for (const point of [p, q]) {
        const { xPx, yPx } = point.screen;
        if (xPx >= 0 && xPx <= viewport.widthPx && yPx >= 0 && yPx <= viewport.heightPx) {
          const rangeM = norm(sub(point.fromCameraM, rangeFrom));
          nearestRangeM = nearestRangeM === null ? rangeM : Math.min(nearestRangeM, rangeM);
        }
      }
      const dx = q.screen.xPx - p.screen.xPx;
      const dy = q.screen.yPx - p.screen.yPx;
      const lengthPx = Math.hypot(dx, dy);
      if (!(lengthPx > 0)) {
        continue;
      }
      // The line's normal on the view, towards the unsurveyed side of the piece's side, which its
      // far end's is: its near end may be the corner it shares with the side before.
      const outward = project(q.outwardM, camera, viewport);
      let nx = -dy / lengthPx;
      let ny = dx / lengthPx;
      if (
        outward.inFront &&
        (outward.xPx - q.screen.xPx) * nx + (outward.yPx - q.screen.yPx) * ny < 0
      ) {
        nx = -nx;
        ny = -ny;
      }
      while (nextTickPx <= along + lengthPx) {
        const t = (nextTickPx - along) / lengthPx;
        nextTickPx += spacingPx;
        const foot: ScreenPx = { xPx: p.screen.xPx + t * dx, yPx: p.screen.yPx + t * dy };
        const end: ScreenPx = { xPx: foot.xPx + tickPx * nx, yPx: foot.yPx + tickPx * ny };
        if (
          options.keepClear.some(
            (segment) =>
              squaredDistanceToSegment(foot, segment) < clearance2 ||
              squaredDistanceToSegment(end, segment) < clearance2,
          )
        ) {
          continue;
        }
        const footM = add(p.fromCameraM, scale(sub(q.fromCameraM, p.fromCameraM), t));
        // A view offset of (Δx, Δy) px at a point w along the view's axis is (Δx, −Δy) × 2w ÷ (s W)
        // along the camera's right and up axes (`project`'s inverse at that depth).
        const w = -toViewAxes(footM, camera.orientation).z;
        const metresPerPx = (2 * w) / (s * viewport.widthPx);
        const endM = add(
          footM,
          add(scale(right, tickPx * nx * metresPerPx), scale(up, -tickPx * ny * metresPerPx)),
        );
        pushSegment(ticks, footM, endM);
      }
      along += lengthPx;
    }
  }
  return {
    line: Float32Array.from(line),
    ticks: Float32Array.from(ticks),
    nearestRangeM,
  };
}

/** The camera's right and up axes in its frame's axes: its orientation's first two columns. */
function cameraAxes(camera: ProjectionCamera): readonly [Vec3, Vec3] {
  const [r0, r1, r2] = rotationRows(camera.orientation);
  return [vec3(r0.x, r1.x, r2.x), vec3(r0.y, r1.y, r2.y)];
}

/**
 * A run's points: each side in pieces of at most {@link SURVEY_EDGE_PIECE_PX} on the view, its
 * last point the next side's first, uniform in the cell coordinates along it.
 */
function runPoints(
  run: ReadonlyArray<DirectedSide>,
  cells: number,
  groundAt: (d: Vec3) => Vec3,
  fromCamera: (p: Vec3) => Vec3,
  visible: (p: Vec3) => boolean,
  camera: ProjectionCamera,
  viewport: Viewport,
): EdgePoint[] {
  const points: EdgePoint[] = [];
  const pointAt = (side: DirectedSide, t: number): EdgePoint => {
    const ca = side.from[0] + t * (side.to[0] - side.from[0]);
    const cb = side.from[1] + t * (side.to[1] - side.from[1]);
    const d = directionAt(side.face, ca, cb, cells);
    const ground = groundAt(d);
    const fromCameraM = fromCamera(ground);
    const projected = project(fromCameraM, camera, viewport);
    // A hundredth of a cell into the unsurveyed side, on the datum.
    const into = normalise(add(d, scale(sub(side.outward, d), 0.01)));
    return {
      fromCameraM,
      screen: { xPx: projected.xPx, yPx: projected.yPx },
      drawn: projected.inFront && visible(ground),
      outwardM: fromCamera(groundAt(into)),
    };
  };
  run.forEach((side, n) => {
    const first = pointAt(side, 0);
    const last = pointAt(side, 1);
    const pieces =
      first.drawn && last.drawn
        ? Math.min(
            SURVEY_EDGE_MAX_PIECES,
            Math.max(
              1,
              Math.ceil(
                Math.hypot(last.screen.xPx - first.screen.xPx, last.screen.yPx - first.screen.yPx) /
                  SURVEY_EDGE_PIECE_PX,
              ),
            ),
          )
        : SURVEY_EDGE_MAX_PIECES;
    if (n === 0) {
      points.push(first);
    }
    for (let k = 1; k < pieces; k += 1) {
      points.push(pointAt(side, k / pieces));
    }
    points.push(last);
  });
  return points;
}

/**
 * The view list's note of a survey edge (the guide's `SURVEY EDGE 4.21 km`): its nearest range to
 * three figures, as the list's ranges are, `FROM CAMERA` where there is no own ship to measure
 * from, the unit kept within its hysteresis from `previous` (`formatBodyDistance`).
 */
export function surveyEdgeNote(
  rangeM: number,
  fromCamera: boolean,
  previous: BodyDistanceUnit | null,
): { readonly text: string; readonly unit: BodyDistanceUnit } {
  const distance = formatBodyDistance(rangeM / 1000, previous);
  const range = `${distance.value} ${distance.unit}`;
  return {
    text: fromCamera ? `${SURVEY_EDGE_NOTE} ${range} FROM CAMERA` : `${SURVEY_EDGE_NOTE} ${range}`,
    unit: distance.unit,
  };
}
