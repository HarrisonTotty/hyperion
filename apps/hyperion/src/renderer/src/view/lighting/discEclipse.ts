/**
 * A body's eclipse over its disc: the fraction of the light a body reflects towards a far point
 * that the shadows on it leave, per display channel (plan R07, T10.b; decision-r07-earth-albedo,
 * Q3). The point regime (`pointFlux`) and planetshine's neighbour both take it.
 *
 * @remarks
 * V̄ = 1 − ∫ (1 − V) w dA ÷ ∫ w dA over the body's disc projected along the star's direction, V
 * the eclipse term (`eclipseVisible`'s arithmetic, through `occultationFrom`) at the surface there
 * and w = [L · 2 ÷ (μ₀ + μ) + (1 − L)] μ for μ > 0, the lunar-Lambert term per unit projected area
 * (the law's A and f(α) cancel). ∫ w dA is the closed form π R² [L + ⅔ (1 − L)] Φ_shape(α). The
 * body is its equivalent sphere, of radius √(a c), whose π a c its p is defined against. An oblate
 * body errs at first order in its flattening f: by about f ÷ 5 of a small central shadow's share
 * under Lambert (0.104% against 0.106% for Io's on Jupiter), and by up to f ÷ 2 in the length of
 * an ingress across a larger shadow (science check, 2026-10-06).
 *
 * V depends, but for the shadow cone's spread over the body's depth (about (R★ + R_o) R ÷ d in
 * radius), only on the distance ρ from the occluder's shadow axis, so each occluder's deficit is a
 * product quadrature in (ρ, θ) about its axis:
 *
 * - ρ runs over the penumbra's overlap with the disc, split at |b − R| and b + R (b the axis's
 *   distance from the disc's centre), |r_u|, each annulus's contacts and the distances at which
 *   the edge of the far point's view turns about the axis, by Gauss–Legendre under the
 *   substitution ρ = ρ₀ + (ρ₁ − ρ₀)(1 − cos πt) ÷ 2, which absorbs the square roots at the pieces'
 *   ends, with one eclipse term per node and channel;
 * - θ runs over each circle's arc inside the disc and in the far point's view (split where μ = 0),
 *   by the same rule.
 *
 * Each node's eclipse term is taken at its arc's w-weighted mean depth along the star's direction,
 * so the cone's spread over the depth enters at second order only. The occluders' deficits add,
 * as their hidden fractions do in `eclipseVisible`. A body clear of every penumbra keeps exactly 1,
 * and one wholly in an umbra exactly 0 (the shadow's edges from the exact tangent cones).
 */
import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import type { BodyFigure } from "../appearance/fromWire";
import { shapeGeometricAlbedo, shapePhase } from "../appearance/shapes";
import type { Rgb } from "../photometry/toneCurve";
import { type AnnulusSet, annulusVisibleFraction, type Occluder, occultationFrom } from "./annuli";
import { hostAnnuli, type PlacedLight } from "./hostLights";
import { gaussLegendre } from "./quadrature";

/** A body as its eclipse takes it. */
export interface EclipsedBody {
  /** Its centre, m, in the frame of the star's and the occluders' centres. */
  readonly centreM: Vec3;
  /** Its figure, taken as its equivalent sphere of radius √(a c). */
  readonly figure: BodyFigure;
}

/** A rule on [0, 1] under the cosine substitution: its nodes and its weights times dx ÷ dt. */
interface CosineRule {
  readonly x: Float64Array;
  readonly w: Float64Array;
}

/** An `n`-point Gauss–Legendre rule in t on [0, 1] for ∫₀¹ f(x) dx, x = (1 − cos πt) ÷ 2. */
function cosineRule(n: number): CosineRule {
  const { x: nodes, w: weights } = gaussLegendre(n);
  const x = new Float64Array(n);
  const w = new Float64Array(n);
  for (let i = 0; i < n; i += 1) {
    const t = ((nodes[i] ?? 0) + 1) / 2;
    x[i] = (1 - Math.cos(Math.PI * t)) / 2;
    w[i] = ((weights[i] ?? 0) / 2) * (Math.PI / 2) * Math.sin(Math.PI * t);
  }
  return { x, w };
}

/** Nodes per piece of ρ, and per arc of θ. */
const RHO_RULE = cosineRule(8);
const THETA_RULE = cosineRule(16);

/** Samples of μ along an arc, to find where it leaves the view, and bisections of each crossing. */
const ARC_SAMPLES = 24;
const CROSSING_STEPS = 30;

/** The relative margin on a shadow's edges for the exact 0 and 1, above the cones' rounding. */
const EDGE_MARGIN = 1e-9;

/** The disc as one ring about a shadow axis sees it, lengths in the body's radius. */
interface DiscView {
  /** The axis's distance b from the disc's centre. */
  readonly axis: number;
  /** L. */
  readonly share: number;
  /** The far point's unit direction along b̂ (the axis's from the centre), t̂ = ŝ × b̂ and ŝ. */
  readonly towardsAxis: number;
  readonly across: number;
  readonly towardsStar: number;
}

/** μ₀ (the depth h) and μ at angle φ round a ring of radius ρ, φ = 0 nearest the disc's centre. */
function anglesAt(view: DiscView, rho: number, phi: number): readonly [number, number] {
  const along = view.axis - rho * Math.cos(phi);
  const across = rho * Math.sin(phi);
  const depth = Math.sqrt(Math.max(0, 1 - along * along - across * across));
  return [
    depth,
    along * view.towardsAxis + across * view.across + depth * view.towardsStar,
  ] as const;
}

/** The lunar-Lambert weight per unit projected area, 0 out of view. */
function weightOf(share: number, mu0: number, mu: number): number {
  return mu > 0 ? ((share * 2) / (mu0 + mu) + (1 - share)) * mu : 0;
}

/** Where μ crosses 0 between φ₀ (μ > 0 there iff `inView`) and φ₁, by bisection. */
function crossing(
  view: DiscView,
  rho: number,
  phi0: number,
  phi1: number,
  inView: boolean,
): number {
  let low = phi0;
  let high = phi1;
  for (let step = 0; step < CROSSING_STEPS; step += 1) {
    const middle = (low + high) / 2;
    if (anglesAt(view, rho, middle)[1] > 0 === inView) {
      low = middle;
    } else {
      high = middle;
    }
  }
  return (low + high) / 2;
}

/** A ring's ∫ w dθ and ∫ w h dθ over its arc in the disc and in view. */
interface Ring {
  readonly weight: number;
  readonly depthWeight: number;
}

/**
 * The ring of radius ρ about the axis.
 *
 * @param mayCross - Whether ρ lies within the view's edge's distances from the axis
 *   ({@link viewEdges}); outside them the arc is wholly in view or wholly out of it, and is not
 *   sampled.
 */
function ringOf(view: DiscView, rho: number, mayCross: boolean): Ring {
  const b = view.axis;
  // Inside the disc where |q|² = b² + ρ² − 2bρ cos φ ≤ 1.
  let half: number;
  if (b * rho > 0) {
    const limit = (b * b + rho * rho - 1) / (2 * b * rho);
    half = limit <= -1 ? Math.PI : limit >= 1 ? 0 : Math.acos(limit);
  } else {
    half = rho < 1 ? Math.PI : 0;
  }
  if (!(half > 0)) {
    return { weight: 0, depthWeight: 0 };
  }
  if (!mayCross) {
    return anglesAt(view, rho, 0)[1] > 0
      ? arcSums(view, rho, [[-half, half]])
      : { weight: 0, depthWeight: 0 };
  }
  // The arc's pieces in view, between the crossings the samples bracket.
  const pieces: Array<readonly [number, number]> = [];
  let previousPhi = -half;
  let previousIn = anglesAt(view, rho, previousPhi)[1] > 0;
  let start: number | null = previousIn ? previousPhi : null;
  for (let j = 1; j <= ARC_SAMPLES; j += 1) {
    const phi = -half + (2 * half * j) / ARC_SAMPLES;
    const inView = anglesAt(view, rho, phi)[1] > 0;
    if (inView !== previousIn) {
      const at = crossing(view, rho, previousPhi, phi, previousIn);
      if (inView) {
        start = at;
      } else if (start !== null) {
        pieces.push([start, at]);
        start = null;
      }
    }
    previousPhi = phi;
    previousIn = inView;
  }
  if (start !== null) {
    pieces.push([start, half]);
  }
  return arcSums(view, rho, pieces);
}

/** ∫ w dθ and ∫ w h dθ over arcs of the ring of radius ρ, in view throughout. */
function arcSums(
  view: DiscView,
  rho: number,
  pieces: ReadonlyArray<readonly [number, number]>,
): Ring {
  let weight = 0;
  let depthWeight = 0;
  for (const [from, to] of pieces) {
    const span = to - from;
    for (let i = 0; i < THETA_RULE.x.length; i += 1) {
      const [mu0, mu] = anglesAt(view, rho, from + span * (THETA_RULE.x[i] ?? 0));
      const w = span * (THETA_RULE.w[i] ?? 0) * weightOf(view.share, mu0, mu);
      weight += w;
      depthWeight += w * mu0;
    }
  }
  return { weight, depthWeight };
}

/** Samples of the view's edge, to find its points nearest and furthest from the axis. */
const EDGE_SAMPLES = 32;

/**
 * The rings at which the edge of the far point's view (μ = 0) turns about the axis: the distances
 * from the axis to its two ends on the rim and to its points nearest and furthest from the axis,
 * where a ring's weight is not smooth in ρ.
 *
 * @remarks
 * The great circle μ = 0 on the lit hemisphere projects to the half-ellipse
 * q(t) = cos t r̂ + cos α sin t p̂, t from π to 2π, with p̂ the far point's direction across ŝ and
 * r̂ = ŝ × p̂; its distance from the axis is extreme where d|q − b|² ÷ dt = 0, found by sampling
 * and bisection.
 */
function viewEdges(view: DiscView): number[] {
  const sinAlpha = Math.hypot(view.towardsAxis, view.across);
  if (!(sinAlpha > 1e-12)) {
    return [];
  }
  // b in (r̂, p̂): p̂ = (v_b, v_t) ÷ sin α and r̂ = (−p_t, p_b) in (b̂, t̂).
  const pb = view.towardsAxis / sinAlpha;
  const pt = view.across / sinAlpha;
  const br = -view.axis * pt;
  const bp = view.axis * pb;
  const cosAlpha = view.towardsStar;
  const distance = (t: number): number => Math.hypot(Math.cos(t) - br, cosAlpha * Math.sin(t) - bp);
  const slope = (t: number): number =>
    -Math.sin(t) * (Math.cos(t) - br) + cosAlpha * Math.cos(t) * (cosAlpha * Math.sin(t) - bp);
  const edges = [distance(Math.PI), distance(2 * Math.PI)];
  let previous = Math.PI;
  let previousSlope = slope(previous);
  for (let j = 1; j <= EDGE_SAMPLES; j += 1) {
    const t = Math.PI * (1 + j / EDGE_SAMPLES);
    const tSlope = slope(t);
    if (previousSlope > 0 !== tSlope > 0) {
      let low = previous;
      let high = t;
      for (let step = 0; step < CROSSING_STEPS; step += 1) {
        const middle = (low + high) / 2;
        if (slope(middle) > 0 === previousSlope > 0) {
          low = middle;
        } else {
          high = middle;
        }
      }
      edges.push(distance((low + high) / 2));
    }
    previous = t;
    previousSlope = tSlope;
  }
  return edges;
}

/**
 * The radius of a tangent cone of an occluder of radius R_o at x behind it along the axis,
 * (R_o + x sin γ) ÷ cos γ, sin γ = (R★ + R_o) ÷ d for the penumbra and (R_o − R★) ÷ d for the
 * umbra (negative past the umbra's apex); `occluders.ts`' radii are its small-angle form.
 */
function coneRadius(occluderRadiusM: number, sinGamma: number, behindM: number): number {
  return (occluderRadiusM + behindM * sinGamma) / Math.sqrt(1 - sinGamma * sinGamma);
}

/** One occluder's shadow on the disc, lengths in the body's radius R. */
interface Shadow {
  readonly view: DiscView;
  /** The ρ range the penumbra meets the disc over, and its pieces' ends. */
  readonly ends: ReadonlyArray<number>;
  /** The rings that may cross the view's edge: from the least of its distances to the greatest. */
  readonly crossesFrom: number;
  readonly crossesTo: number;
  /** Within this ρ every lit point is in the umbra, at every lit depth. */
  readonly umbra: number;
  /** The axis: the star's centre from the body's, the star-to-occluder vector and its ŝ part. */
  readonly starM: Vec3;
  readonly axisM: Vec3;
  readonly axisAlongM: number;
  /** The unit direction across the axis towards the disc's centre, at which each node is taken. */
  readonly inwards: Vec3;
}

/** The star's distance d and unit direction ŝ from the body, and a basis across ŝ. */
interface StarFrame {
  readonly starM: Vec3;
  readonly distanceM: number;
  readonly towards: Vec3;
  readonly e1: Vec3;
}

/**
 * An occluder's shadow on the disc, or `"umbra"` where the whole lit disc lies in its umbra, or
 * `null` where its penumbra misses the disc.
 */
function shadowOf(
  frame: StarFrame,
  starRadiusM: number,
  occluder: Occluder,
  radiusM: number,
  towardsPoint: Vec3,
  share: number,
  annuli: ReadonlyArray<AnnulusSet>,
): Shadow | "umbra" | null {
  const { starM, distanceM: d, towards: s } = frame;
  const occluderM = occluder.centreM;
  if (norm(occluderM) >= d) {
    return null;
  }
  const axisM = sub(occluderM, starM);
  const axisLengthM = norm(axisM);
  const axisAlongM = dot(axisM, s);
  if (!(axisAlongM < 0)) {
    return null;
  }
  // The axis meets the plane through the centre across ŝ at t, x behind the occluder.
  const t = -d / axisAlongM;
  const behindM = (t - 1) * axisLengthM;
  if (!(behindM > 0)) {
    return null;
  }
  // On the plane, less the rounding along ŝ, which would turn b̂ out of it.
  const crossingM = add(starM, scale(axisM, t));
  const axisPoint = sub(crossingM, scale(s, dot(crossingM, s)));
  const b = norm(axisPoint) / radiusM;
  const towardsAxis = b > 1e-12 ? normalise(axisPoint) : frame.e1;
  const acrossAxis = cross(s, towardsAxis);
  // The axis drifts across ŝ by tan τ a unit of depth; a lit point lies within R tan τ of its ring.
  const unitAxis = scale(axisM, 1 / axisLengthM);
  const drift = norm(cross(unitAxis, s)) / Math.abs(dot(unitAxis, s));
  const r = occluder.radiusM;
  // The penumbra widens behind the occluder, so the lit disc's widest is at the centre's plane; the
  // umbra narrows behind it unless the occluder outsizes the star, so its least is at one end of
  // the lit depths, x − R to x.
  const penumbra = coneRadius(r, (starRadiusM + r) / axisLengthM, behindM) / radiusM;
  const sinUmbra = (r - starRadiusM) / axisLengthM;
  const umbra = coneRadius(r, sinUmbra, behindM) / radiusM;
  const umbraLeast =
    Math.min(umbra * radiusM, coneRadius(r, sinUmbra, Math.max(0, behindM - radiusM))) / radiusM;
  const low = Math.max(0, b - 1 - drift);
  const high = Math.min(b + 1 + drift, penumbra * (1 + EDGE_MARGIN));
  if (!(low < high)) {
    return null;
  }
  if (b + 1 + drift <= umbraLeast * (1 - EDGE_MARGIN)) {
    return "umbra";
  }
  const view: DiscView = {
    axis: b,
    share,
    towardsAxis: dot(towardsPoint, towardsAxis),
    across: dot(towardsPoint, acrossAxis),
    towardsStar: dot(towardsPoint, s),
  };
  const edges = viewEdges(view);
  const contacts: number[] = [Math.abs(umbra), Math.abs(1 - b), 1 + b, ...edges];
  // Each annulus edge's contacts with the occluder's limb, σ = |e ρ★ ∓ k|, through ρ ≈ σ ÷ g with
  // g = 1 ÷ D_o − 1 ÷ D★ (rad per metre across the axis), at the plane's point on the axis.
  const toOccluderM = behindM;
  const toStarM = behindM + axisLengthM;
  if (toOccluderM > r) {
    const starRad = Math.asin(Math.min(1, starRadiusM / toStarM));
    const occluderRad = Math.asin(r / toOccluderM);
    const spread = (1 / toOccluderM - 1 / toStarM) * radiusM;
    for (const set of annuli) {
      for (let j = 1; j < set.edges.length - 1; j += 1) {
        const edge = (set.edges[j] ?? 0) * starRad;
        contacts.push(Math.abs(edge - occluderRad) / spread, (edge + occluderRad) / spread);
      }
    }
  }
  const ends = [low, high, ...contacts.filter((rho) => rho > low && rho < high)]
    .toSorted((x, y) => x - y)
    .filter((rho, i, all) => i === 0 || rho - (all[i - 1] ?? 0) > 1e-12 * high);
  const inwardsRaw = sub(scale(towardsAxis, -1), scale(unitAxis, -dot(towardsAxis, unitAxis)));
  return {
    view,
    ends,
    crossesFrom: edges.length > 0 ? Math.min(...edges) : 0,
    crossesTo: edges.length > 0 ? Math.max(...edges) : -1,
    umbra: Math.max(0, umbraLeast - drift),
    starM,
    axisM,
    axisAlongM,
    inwards: normalise(inwardsRaw),
  };
}

/**
 * The fraction of a body's reflected light towards a far point that the occluders' shadows leave,
 * per display channel (r, g, b), from `star`'s B, V and R limb laws.
 *
 * @param star - The star, its centre in the frame of the body and the occluders.
 * @param body - The body; its eclipse is taken on its equivalent sphere √(a c).
 * @param occluders - The bodies that may eclipse the star for it (`occludersFor`'s list); their
 *   shadows are taken not to overlap.
 * @param towards - The unit direction from the body to the far point: the camera for a point body,
 *   the lit body for a planetshine neighbour.
 * @param share - The law's Lommel–Seeliger share L, 0 to 1.
 * @param k - The eclipse term's annuli: `DISC_ANNULI_HIGH` or `DISC_ANNULI_LOW`.
 */
export function discEclipseVisible(
  star: PlacedLight,
  body: EclipsedBody,
  occluders: ReadonlyArray<Occluder>,
  towards: Vec3,
  share: number,
  k: number,
): Rgb {
  if (occluders.length === 0) {
    return [1, 1, 1];
  }
  const radiusM = Math.sqrt(body.figure.equatorialRadiusM * body.figure.polarRadiusM);
  // Every position from the body's centre, so that no large numbers meet.
  const starM = sub(star.centreM, body.centreM);
  const distanceM = norm(starM);
  const s = scale(starM, 1 / distanceM);
  const seed = Math.abs(s.x) < 0.9 ? vec3(1, 0, 0) : vec3(0, 1, 0);
  const frame: StarFrame = { starM, distanceM, towards: s, e1: normalise(cross(seed, s)) };
  const alpha = Math.acos(Math.min(1, Math.max(-1, dot(towards, s))));
  const total = Math.PI * shapeGeometricAlbedo(share) * shapePhase(share, alpha);
  if (!(total > 0)) {
    return [1, 1, 1];
  }
  const annuli = hostAnnuli(star.disc, k);
  const starSphere = { centreM: starM, radiusM: star.disc.radius_m };
  const hidden: [number, number, number] = [0, 0, 0];
  for (const occluder of occluders) {
    const relative = { centreM: sub(occluder.centreM, body.centreM), radiusM: occluder.radiusM };
    const shadow = shadowOf(frame, star.disc.radius_m, relative, radiusM, towards, share, annuli);
    if (shadow === "umbra") {
      return [0, 0, 0];
    }
    if (shadow === null) {
      continue;
    }
    for (let piece = 1; piece < shadow.ends.length; piece += 1) {
      const from = shadow.ends[piece - 1] ?? 0;
      const span = (shadow.ends[piece] ?? 0) - from;
      for (let i = 0; i < RHO_RULE.x.length; i += 1) {
        const rho = from + span * (RHO_RULE.x[i] ?? 0);
        const ring = ringOf(shadow.view, rho, rho >= shadow.crossesFrom && rho <= shadow.crossesTo);
        if (!(ring.weight > 0)) {
          continue;
        }
        const area = span * (RHO_RULE.w[i] ?? 0) * rho * ring.weight;
        if (rho <= shadow.umbra) {
          hidden[0] += area;
          hidden[1] += area;
          hidden[2] += area;
          continue;
        }
        // The node on the axis at the ring's mean depth, ρ across it towards the disc's centre.
        const depthM = (ring.depthWeight / ring.weight) * radiusM;
        const onAxis = add(
          shadow.starM,
          scale(shadow.axisM, (depthM - distanceM) / shadow.axisAlongM),
        );
        const node = add(onAxis, scale(shadow.inwards, rho * radiusM));
        const occultation = occultationFrom(starSphere, node, relative);
        if (occultation === null) {
          continue;
        }
        for (const c of [0, 1, 2] as const) {
          const set = annuli[c];
          hidden[c] +=
            occultation === "inside"
              ? area
              : area * (1 - annulusVisibleFraction(set, occultation.ratio, occultation.separation));
        }
      }
    }
  }
  return [
    Math.min(1, Math.max(0, 1 - hidden[0] / total)),
    Math.min(1, Math.max(0, 1 - hidden[1] / total)),
    Math.min(1, Math.max(0, 1 - hidden[2] / total)),
  ];
}
