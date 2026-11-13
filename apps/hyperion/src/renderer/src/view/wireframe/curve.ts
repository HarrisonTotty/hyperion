import { add, dot, norm, scale, sub, type Vec3 } from "../../geometry/vec3";
import { type ProjectionCamera, projector, type Viewport } from "../camera/projection";

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

/**
 * How deep a curve's chords are halved for their depth sag alone
 * ({@link CurveSpec.depthSagWithin}): 8, a 256th of a first span (R02.T19.b).
 *
 * @remarks
 * A graticule's chord over an arc Δ of a sphere of radius r sags r Δ² ÷ 8 inside it, and where the
 * occluder's slope gives it nothing, at the sub-camera point, its tolerance is half the occluder's
 * 4 × 10⁻⁶ d margin (`DATUM_DEPTH_SAG_PER_MARGIN`), so a span needs Δ under √(1.6 × 10⁻⁵ d ÷ r),
 * 0.23° even at the surface (d = r). Its widest first span, 15° (a meridian's 180° in 12, or a
 * parallel's 360° in 24), takes 7 halvings to reach that at the surface, 5 on `TEST SATURN` at
 * 340 Mm. The cap holds one more in hand and bounds the cost wherever the sag cannot fall, as
 * {@link MAX_DEPTH} does the lateral rule's.
 */
const DEPTH_SAG_MAX_DEPTH = 8;

/**
 * The most a drawn chord's ends' reversed depths may differ by, a factor: 2 (R08.T17).
 *
 * @remarks
 * A chord's reversed depth n ÷ w is affine along it on the view, and the GPU forms it there from
 * its ends in `f32`, so that across a factor of R its far end's depth is resolved to about R ε of
 * itself. A run cut at the near plane ends 0.1 m ahead of the camera, however far to the side,
 * where a moon's orbit seen from near its focus stands 10⁹ m off: its chord from there projected
 * some 10¹⁰ px long at depths 1 to 10⁻¹⁰, and drew across the planet it passes behind in its
 * casing alone (the served giant's moons' orbits, check-giant-disc-colour's captures, 2026-11-04).
 * Halved to {@link MAX_DEPTH} with the rest, a chord the view reaches holds the factor; pieces
 * wholly off one side of the view, which are not refined, may keep more. A cut run takes at most
 * that many more points each side of its cut, and a chord across a factor R about log₂ R.
 */
const DEPTH_RATIO_MAX = 2;

/** How many bisections locate a curve's visibility boundary. */
const BOUNDARY_ITERATIONS = 48;

/** How far outside the viewport a span may lie before it is no longer refined, px. */
const OFF_SCREEN_MARGIN_PX = 64;

/**
 * Whether a chord `halfChordM` long each side of its middle, whose point nearest the curve's point
 * `curveM` is `chordM`, sags little enough behind the curve there, and so all along it
 * (camera-relative, m; R02.T19.b).
 */
export type DepthSagWithin = (curveM: Vec3, chordM: Vec3, halfChordM: number) => boolean;

/**
 * How far a chord of a curve on no body's surface, an orbit or a ring's edge, may stand from it
 * along the line of sight, a share of the curve's distance: 10⁻³ (R08.T17; lean, 2026-11-05).
 *
 * @remarks
 * Seen from near its centre such a curve is nearly a great circle, which the view's gnomonic
 * projection draws straight, so its lateral sagitta took one chord across the field: over 2θ of
 * arc a chord stands R (1 − cos θ) inside it, 2% at the first spans' 22.5°, and the served giant
 * `41ffecb1ffc00013.0400`'s moons' orbits, 6–9.5 Gm out, drew across the planet they pass behind
 * from 243 Mm. At 10⁻³ an arc takes chords of 5.1° at most there, and the curve's order against a
 * body holds wherever their distances part by more than that share; seen from afar the share costs
 * no point.
 */
export const CURVE_DEPTH_SAG_SHARE = 1e-3;

/**
 * The {@link DepthSagWithin} of a curve on no body's surface: whether the chord's point nearest it
 * stands within {@link CURVE_DEPTH_SAG_SHARE} of the curve's distance from the camera.
 */
export function distanceSagWithin(curveM: Vec3, chordM: Vec3): boolean {
  const distanceM = norm(curveM);
  return Math.abs(distanceM - norm(chordM)) <= CURVE_DEPTH_SAG_SHARE * distanceM;
}

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
  /**
   * How far a chord may sag behind the curve: a body's graticule, drawn over its own occluder, asks
   * its depth inside the surface (`graticule`); an orbit or a ring's edge asks
   * {@link distanceSagWithin} (R08.T17). Absent for a curve subdivided for its lateral sagitta
   * alone.
   */
  readonly depthSagWithin?: DepthSagWithin;
}

interface Sample {
  readonly u: number;
  readonly p: Vec3;
  readonly xPx: number;
  readonly yPx: number;
  /** Its reversed depth, n ÷ w. */
  readonly depth: number;
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

/**
 * Whether the chord from `a` to `b` sags behind its midpoint `m` as `within` allows: from `m` to
 * the chord's point nearest it, the chord half its length each side.
 */
function depthSagAllowed(within: DepthSagWithin, m: Sample, a: Sample, b: Sample): boolean {
  const chord = sub(b.p, a.p);
  const lengthSquared = dot(chord, chord);
  const t =
    lengthSquared > 0 ? Math.min(1, Math.max(0, dot(sub(m.p, a.p), chord) / lengthSquared)) : 0;
  return within(m.p, add(a.p, scale(chord, t)), Math.sqrt(lengthSquared) / 2);
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
 * {@link SAGITTA_TOLERANCE_PX} (plan R02, Design note 13), and a depth sag its spec allows
 * ({@link CurveSpec.depthSagWithin}; R02.T19.b), their ends' depths within
 * {@link DEPTH_RATIO_MAX} of each other (R08.T17).
 *
 * @remarks
 * Each window is cut into even spans, and each span is halved while the projection of its midpoint
 * or of its quarter points lies farther than the tolerance from the projection of its chord (a
 * straight line in space projects to a straight line on screen, so this is the drawn error), all in
 * `f64`. A curve with a depth sag is also halved, to {@link DEPTH_SAG_MAX_DEPTH} halvings, while
 * its chord sags behind its midpoint more than its spec allows for the whole chord, the midpoint
 * alone since a body's graticule is convex, its chords sagging most about their middles: a line
 * that projects straight, through the sub-camera point, leaves no lateral sagitta at all, while its
 * chords sag inside the body, square to the view there, where its occluder stands only its margin
 * inside (R02.T19.b's finding). A span wholly beyond one edge of the viewport is not refined, so an
 * enormous body costs little off screen. Where the curve passes behind the near plane its boundary
 * is found by bisection and the polyline ends there, and starts again where it returns, so no
 * chord crosses the hidden stretch.
 */
export function sampleCurve(
  spec: CurveSpec,
  camera: ProjectionCamera,
  viewport: Viewport,
): Polyline[] {
  const project = projector(camera, viewport);
  const sample = (u: number): Sample => {
    const p = spec.point(u);
    const projected = project(p);
    return {
      u,
      p,
      xPx: projected.xPx,
      yPx: projected.yPx,
      depth: projected.depth,
      inFront: projected.inFront,
    };
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
    const within = depth < DEPTH_SAG_MAX_DEPTH ? spec.depthSagWithin : undefined;
    const flat =
      q1.inFront &&
      q3.inFront &&
      distanceToSegment(m, a, b) < SAGITTA_TOLERANCE_PX &&
      distanceToSegment(q1, a, b) < SAGITTA_TOLERANCE_PX &&
      distanceToSegment(q3, a, b) < SAGITTA_TOLERANCE_PX &&
      Math.max(a.depth, b.depth) <= DEPTH_RATIO_MAX * Math.min(a.depth, b.depth) &&
      (within === undefined || depthSagAllowed(within, m, a, b));
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
