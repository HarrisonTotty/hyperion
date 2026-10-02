import type { Vec3 } from "../../geometry/vec3";
import { type ProjectionCamera, project, type Viewport } from "../camera/projection";

/**
 * A run of points the wireframe strokes as one polyline: `f64` vectors from the camera, m along the
 * camera frame's axes, narrowed only when the draw list is packed.
 */
export type Polyline = ReadonlyArray<Vec3>;

/**
 * The largest screen-space sagitta a chord may leave, px: 0.25 (plan R02, Design note 13), so that
 * a body 400 km below the camera has a true horizon rather than a polygon's.
 */
export const SAGITTA_TOLERANCE_PX = 0.25;

/** How deep a curve's chords are halved before the subdivision gives up on a span. */
const MAX_DEPTH = 24;

/** How many bisections locate a curve's visibility boundary. */
const BOUNDARY_ITERATIONS = 48;

/** How far outside the viewport a span may lie before it is no longer refined, px. */
const OFF_SCREEN_MARGIN_PX = 64;

/** A closed interval of a curve's parameter. */
export type ParameterWindow = readonly [number, number];

/** A parametric curve and where it is drawn. */
export interface CurveSpec {
  /** The point at parameter `u`, camera-relative, m. */
  point(u: number): Vec3;
  /**
   * The windows of the parameter in which the curve may be drawn, each sampled on its own, so that
   * a visible arc however short is found (a graticule's, from `cosineWindows`).
   */
  readonly windows: ReadonlyArray<ParameterWindow>;
  /** How many even spans each window is first cut into, before refinement. */
  readonly initialSpans: number;
}

interface Sample {
  readonly u: number;
  readonly p: Vec3;
  readonly xPx: number;
  readonly yPx: number;
  readonly inFront: boolean;
}

/** The distance from `(x, y)` to the segment from `a` to `b`, px. */
function distanceToSegment(s: Sample, a: Sample, b: Sample): number {
  const dx = b.xPx - a.xPx;
  const dy = b.yPx - a.yPx;
  const lengthSquared = dx * dx + dy * dy;
  const t =
    lengthSquared > 0
      ? Math.min(1, Math.max(0, ((s.xPx - a.xPx) * dx + (s.yPx - a.yPx) * dy) / lengthSquared))
      : 0;
  return Math.hypot(s.xPx - (a.xPx + t * dx), s.yPx - (a.yPx + t * dy));
}

/** Whether samples all lie beyond one edge of the viewport, by the margin. */
function allOffOneSide(samples: readonly Sample[], viewport: Viewport): boolean {
  const beyond = (test: (s: Sample) => boolean): boolean => samples.every(test);
  return (
    beyond((s) => s.xPx < -OFF_SCREEN_MARGIN_PX) ||
    beyond((s) => s.yPx < -OFF_SCREEN_MARGIN_PX) ||
    beyond((s) => s.xPx > viewport.widthPx + OFF_SCREEN_MARGIN_PX) ||
    beyond((s) => s.yPx > viewport.heightPx + OFF_SCREEN_MARGIN_PX)
  );
}

/**
 * The windows of `u` within `[uMin, uMax]` where α cos u + β sin u > `threshold`: the facing test
 * of a point moving along a circle on a sphere, which is linear in (cos u, sin u).
 *
 * @remarks
 * With ρ = √(α² + β²) and ψ = atan2(β, α) the condition is cos(u − ψ) > threshold ÷ ρ: none where
 * the threshold is at or above ρ, the whole range where it is below −ρ, otherwise the arc
 * |u − ψ| < acos(threshold ÷ ρ), cut to the range (which may take two pieces of it across 2π).
 */
export function cosineWindows(
  alpha: number,
  beta: number,
  threshold: number,
  uMin: number,
  uMax: number,
): ParameterWindow[] {
  const rho = Math.hypot(alpha, beta);
  if (!(threshold < rho)) {
    return [];
  }
  if (threshold <= -rho) {
    return [[uMin, uMax]];
  }
  const psi = Math.atan2(beta, alpha);
  const half = Math.acos(threshold / rho);
  const windows: ParameterWindow[] = [];
  for (const turn of [-2, -1, 0, 1, 2]) {
    const lo = Math.max(uMin, psi - half + turn * 2 * Math.PI);
    const hi = Math.min(uMax, psi + half + turn * 2 * Math.PI);
    if (hi > lo) {
      windows.push([lo, hi]);
    }
  }
  return windows;
}

/**
 * A curve cut into polylines whose chords each leave a screen-space sagitta under
 * {@link SAGITTA_TOLERANCE_PX} (plan R02, Design note 13).
 *
 * @remarks
 * Each window is cut into even spans, and each span is halved while the projection of its midpoint
 * or of its quarter points lies farther than the tolerance from the projection of its chord (a
 * straight line in space projects to a straight line on screen, so this is the drawn error), all in
 * `f64`. A span wholly beyond one edge of the viewport is not refined, so an enormous body costs
 * little off screen. Where the curve passes behind the near plane its boundary is found by
 * bisection and the polyline ends there, and starts again where it returns, so no chord crosses
 * the hidden stretch.
 */
export function sampleCurve(
  spec: CurveSpec,
  camera: ProjectionCamera,
  viewport: Viewport,
): Polyline[] {
  const sample = (u: number): Sample => {
    const p = spec.point(u);
    const projected = project(p, camera, viewport);
    return { u, p, xPx: projected.xPx, yPx: projected.yPx, inFront: projected.inFront };
  };
  const boundary = (inside: Sample, outside: Sample): Sample => {
    let drawn = inside;
    let hidden = outside;
    for (let i = 0; i < BOUNDARY_ITERATIONS; i += 1) {
      const mid = sample((drawn.u + hidden.u) / 2);
      if (mid.inFront) {
        drawn = mid;
      } else {
        hidden = mid;
      }
    }
    return drawn;
  };
  const runs: Vec3[][] = [];
  let run: Vec3[] | null = null;
  const endRun = (): void => {
    if (run !== null && run.length >= 2) {
      runs.push(run);
    }
    run = null;
  };
  // Emits a drawn span's interior points and its end `b`, `a` having been emitted.
  const refine = (a: Sample, b: Sample, depth: number): void => {
    const m = sample((a.u + b.u) / 2);
    if (!m.inFront && depth >= MAX_DEPTH) {
      endRun();
      run = [b.p];
      return;
    }
    if (!m.inFront) {
      // The curve dips behind the near plane inside the span: end the run at the boundary on
      // each side, and start again after it.
      const leaving = boundary(a, m);
      refine(a, leaving, depth + 1);
      endRun();
      const returning = boundary(b, m);
      run = [returning.p];
      refine(returning, b, depth + 1);
      return;
    }
    if (depth >= MAX_DEPTH || allOffOneSide([a, m, b], viewport)) {
      run?.push(b.p);
      return;
    }
    const q1 = sample((3 * a.u + b.u) / 4);
    const q3 = sample((a.u + 3 * b.u) / 4);
    const flat =
      q1.inFront &&
      q3.inFront &&
      distanceToSegment(m, a, b) < SAGITTA_TOLERANCE_PX &&
      distanceToSegment(q1, a, b) < SAGITTA_TOLERANCE_PX &&
      distanceToSegment(q3, a, b) < SAGITTA_TOLERANCE_PX;
    if (flat) {
      run?.push(b.p);
      return;
    }
    refine(a, m, depth + 1);
    refine(m, b, depth + 1);
  };
  for (const [u0, u1] of spec.windows) {
    let previous = sample(u0);
    run = previous.inFront ? [previous.p] : null;
    for (let i = 1; i <= spec.initialSpans; i += 1) {
      const next = sample(u0 + ((u1 - u0) * i) / spec.initialSpans);
      if (previous.inFront && next.inFront) {
        refine(previous, next, 0);
      } else if (previous.inFront) {
        refine(previous, boundary(previous, next), 0);
        endRun();
      } else if (next.inFront) {
        const edge = boundary(next, previous);
        run = [edge.p];
        refine(edge, next, 0);
      }
      previous = next;
    }
    endRun();
  }
  return runs;
}
