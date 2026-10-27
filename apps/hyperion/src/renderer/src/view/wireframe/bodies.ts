import { add, cross, dot, norm, normalise, scale, sub, type Vec3, vec3 } from "../../geometry/vec3";
import { type ProjectionCamera, toViewAxes, type Viewport } from "../camera/projection";
import { rotate } from "../camera/quaternion";
import {
  IDENTITY_ROTATION,
  type Rotation3,
  rotateToBody,
  rotation3FromRows,
} from "../coords/rotation";
import type { BodyFigure } from "../terrain/planet";
import { figurePole } from "../bodies/oblate";
import { cosineWindows, type ParameterWindow, type Polyline, sampleCurve } from "./curve";
import {
  angularDistortion,
  type DatumSpheroid,
  datumSpheroidOf,
  stretched,
  unstretched,
} from "./spheroid";

/**
 * How a body is drawn at its size on screen (plan R02, Design note 13): below 3 device px its
 * symbol, from there to {@link GRATICULE_FROM_PX} at the line scale its limb circle alone, and from
 * there its limb and graticule.
 */
export type BodyRegime = "symbol" | "limb" | "graticule";

/**
 * The apparent diameter below which a body is its symbol, device px: 3, the brainstorm's point
 * regime. It is the image's, not a line's, and stays in device px at every line scale, as the
 * photorealistic style's `POINT_BELOW_PX` does, so that both styles agree on which bodies are
 * points (decision-r07-t16d-followups, (c)).
 */
export const SYMBOL_BELOW_PX = 3;

/**
 * The apparent diameter from which a body has its graticule, CSS px: 8, where 12 lines stop being
 * lines, eight widths of the guide's 1 px line (Design note 13). Like the strokes' widths
 * (`STROKE_PX`, `CASING_PX`), it is drawn at the view's line scale, `strokeScale` device px for each
 * (R07.T16.d), so that the graticule's gaps stand to its lines as R02 built them: 8 device px at a
 * scale of 1, and 16 up to a ratio of 2 (decision-r07-t16d-followups, (c)).
 */
export const GRATICULE_FROM_PX = 8;

/**
 * The apparent diameter from which the graticule is drawn at 15° rather than 30°, CSS px: 64, at
 * the line scale as {@link GRATICULE_FROM_PX} is (128 device px up to a ratio of 2).
 */
export const FINE_GRATICULE_FROM_PX = 64;

/** The graticule's step, degrees, on a large body and on a small one (and at the low setting). */
export const GRATICULE_STEP_DEG = { fine: 15, coarse: 30 } as const;

/**
 * Whether a body of this apparent diameter, device px, is drawn as its symbol: under
 * {@link SYMBOL_BELOW_PX}, at every line scale.
 */
export function isSymbolSized(diameterPx: number): boolean {
  return diameterPx < SYMBOL_BELOW_PX;
}

/**
 * A body's regime by its apparent diameter, device px, at the view's line scale (`strokeScale`, the
 * device px drawn for each CSS px of a line): its graticule from {@link GRATICULE_FROM_PX} ×
 * the scale across.
 */
export function bodyRegime(diameterPx: number, strokeScale: number): BodyRegime {
  if (isSymbolSized(diameterPx)) {
    return "symbol";
  }
  return diameterPx < GRATICULE_FROM_PX * strokeScale ? "limb" : "graticule";
}

/**
 * The step of a body's graticule, degrees, by its apparent diameter, device px, at the view's line
 * scale: 15° from {@link FINE_GRATICULE_FROM_PX} × the scale across, else 30°, and 30° at the low
 * setting.
 */
export function graticuleStepDeg(
  diameterPx: number,
  strokeScale: number,
  lowSetting: boolean,
): number {
  return lowSetting || diameterPx < FINE_GRATICULE_FROM_PX * strokeScale
    ? GRATICULE_STEP_DEG.coarse
    : GRATICULE_STEP_DEG.fine;
}

/**
 * A sphere's apparent diameter on a view, px: 2 asin(r ÷ d) at the view's centre-pixel scale, or
 * infinite from inside it.
 *
 * @param centreM - The sphere's centre from the camera, m.
 */
export function angularDiameterPx(
  centreM: Vec3,
  radiusM: number,
  camera: ProjectionCamera,
  viewport: Viewport,
): number {
  const distanceM = norm(centreM);
  if (!(distanceM > radiusM)) {
    return Number.POSITIVE_INFINITY;
  }
  return 2 * Math.asin(radiusM / distanceM) * focalPx(camera, viewport);
}

/**
 * A body's outline's least apparent diameter on a view, px (plan R02, R02.T19): twice the least
 * angular semi-axis of the cone of lines of sight that graze its spheroid, at the view's
 * centre-pixel scale, as {@link angularDiameterPx} measures a sphere's; a sphere's
 * {@link angularDiameterPx} itself, and infinite from inside the body.
 *
 * @remarks
 * A direction u lies in the cone where the stretched ray L u is within α′ = asin(a ÷ |L c|) of
 * L c: uᵀMu ≥ 0, M = L(sin²α′ I − [ĉ′]ₓᵀ[ĉ′]ₓ)L, written so (by (x × ĉ′) · (y × ĉ′) =
 * x · y − (x · ĉ′)(y · ĉ′), ĉ′ a unit vector) that a small α′ loses nothing to cancellation. The
 * normal e of the plane of the centre's line of sight and the pole is fixed by L, and is an
 * eigenvector of M of eigenvalue −cos²α′; in that plane M's 2 × 2 block, in the basis of ĉ and
 * e × ĉ, has one positive eigenvalue λ₁, the cone's axis, and one negative, λ₂. Along each
 * principal direction the cone's half-angle θ has tan² θ = λ₁ ÷ (−λᵢ), the least of which is the
 * outline's least semi-axis. For a sphere it is α itself. Seen pole-on any plane through the line
 * of sight serves.
 */
export function outlineLeastDiameterPx(
  body: Pick<GraticuleBody, "centreM" | "radiusM" | "spheroid">,
  camera: ProjectionCamera,
  viewport: Viewport,
): number {
  const spheroid = body.spheroid;
  if (spheroid === null) {
    return angularDiameterPx(body.centreM, body.radiusM, camera, viewport);
  }
  const centre = stretched(body.centreM, spheroid);
  const d = norm(centre);
  if (!(d > body.radiusM)) {
    return Number.POSITIVE_INFINITY;
  }
  const sin2 = (body.radiusM / d) ** 2;
  const along = normalise(body.centreM);
  const stretchedAlong = scale(centre, 1 / d);
  const across = cross(along, spheroid.pole);
  const e = norm(across) > 1e-12 ? normalise(across) : perpendicularPair(along)[0];
  const lAlong = stretched(along, spheroid);
  const lAside = stretched(cross(e, along), spheroid);
  const m = (x: Vec3, y: Vec3): number =>
    sin2 * dot(x, y) - dot(cross(x, stretchedAlong), cross(y, stretchedAlong));
  const m11 = m(lAlong, lAlong);
  const m12 = m(lAlong, lAside);
  const m22 = m(lAside, lAside);
  const mean = (m11 + m22) / 2;
  const half = Math.hypot((m11 - m22) / 2, m12);
  const lambdaAxis = mean + half;
  // The steepest of the other two, −λᵢ largest, gives the least half-angle.
  const lambdaSteepest = Math.max(half - mean, 1 - sin2);
  const leastRad = Math.atan(Math.sqrt(lambdaAxis / lambdaSteepest));
  return 2 * leastRad * focalPx(camera, viewport);
}

/**
 * The view's focal length, px: width ÷ (2 tan(fovX ÷ 2)), the pixels a radian subtends at the
 * view's centre, and the pixels per unit of the image plane's tangent coordinates everywhere.
 */
function focalPx(camera: ProjectionCamera, viewport: Viewport): number {
  return viewport.widthPx / (2 * Math.tan(camera.fovXRad / 2));
}

/**
 * How a body's datum is stroked over the photorealistic image (plan R10, R10.T16.g; main's ruling,
 * `decision-r08-approximate-plate.md` item 2): inside the body's figure, so that no line or casing
 * covers the image beyond it, an atmosphere's glow above the limb included. The limb's centreline
 * runs its reach inside the figure's outline, so that its casing's coverage ends on the outline, and
 * every other line ends at least its own reach inside it. The wireframe, with no image there,
 * centres its limb on the outline.
 */
export interface DatumInside {
  /**
   * How far a cased datum line's coverage reaches from its centreline, device px: δ = w ÷ 2 + c +
   * 0.5 for a stroke w wide cased c on each side, since `lineStroke.wgsl`'s coverage falls to zero
   * half a pixel past a stroke's half-width and a batch's casing is drawn w + 2c wide. Its thin
   * lines', the limb's among them, and its heavy lines', the equator's and the prime meridian's
   * (`datumInsideAt`).
   */
  readonly reachPx: { readonly thin: number; readonly heavy: number };
}

/**
 * How much farther inside the outline than its reach a datum line is stroked over the image, device
 * px: 1/32, above the `f32` rounding of its segments' ends and of the GPU's projection (about 10⁻⁴
 * px at any range: `f32`'s 6 × 10⁻⁸ of a camera-relative vector, at some 1,700 px a radian), so
 * that its casing's coverage never reaches past the outline (R10.T16.g).
 */
export const DATUM_INSIDE_MARGIN_PX = 1 / 32;

/**
 * The least apparent radius over the image of a body whose datum is drawn there, in limb reaches
 * ({@link DatumInside.reachPx}'s thin δ): 2(2 + √2), about 6.83 (main's ruling for R10.T16.g,
 * `decision-r08-approximate-plate.md` §7, item 1).
 *
 * @remarks
 * The cased limb stroked inside a body covers a band 2δ wide from its limb in. At an apparent radius
 * R the band covers 1 − ((R − 2δ) ÷ R)² of the disc, half of it at R = 2δ ÷ (1 − 1 ÷ √2) = 2(2 + √2)δ:
 * 23.9 device px at a line scale of 2 (δ 3.5) and 34.1 at 3 (δ 5), which the guide gives rounded up,
 * 24 and 35. Below it the body has no limb or graticule over the image, which alone shows it, as it
 * shows a body under 3 device px; its entry in the view's list and, while it is selected, the
 * bracket still mark it. A cut with no hysteresis; a thinner stroke and an outside tick were both
 * refused. On a flattened body its least apparent radius, its outline's least semi-axis
 * ({@link outlineLeastDiameterPx}; R02.T19, decision-r02-t19-leans item 3): a band 2δ wide inside an
 * elliptic outline of semi-axes A ≥ B covers at most what it covers in the circle of radius B, so no
 * body that passes the cut has more than half its disc covered.
 */
export const DATUM_OVER_IMAGE_RADIUS_PER_REACH = 2 * (2 + Math.SQRT2);

/**
 * Whether a body of this apparent diameter, device px, has its datum over the photorealistic image:
 * an apparent radius of at least {@link DATUM_OVER_IMAGE_RADIUS_PER_REACH} times its limb's reach,
 * device px, on a flattened body its least ({@link outlineLeastDiameterPx}; R02.T19).
 */
export function datumOverImage(diameterPx: number, limbReachPx: number): boolean {
  return diameterPx / 2 >= DATUM_OVER_IMAGE_RADIUS_PER_REACH * limbReachPx;
}

/** A body as the wireframe's geometry needs it. */
export interface GraticuleBody {
  /** The body's centre from the camera, m along the camera frame's axes. */
  readonly centreM: Vec3;
  /** Its radius, m: its equatorial radius, a, where it is a spheroid. */
  readonly radiusM: number;
  /**
   * Its rotation from body-fixed to body axes, or `null` where rotation is not modelled: the
   * graticule then does not turn (Design note 14).
   */
  readonly rotation: Rotation3 | null;
  /**
   * The pole a body whose rotation is not modelled is drawn about: its orbit normal (Design note
   * 14, as `displays/system/bodyFrame.ts` assumes), or `null` for the frame's +z.
   */
  readonly unmodelledPole: Vec3 | null;
  /**
   * The spheroid its datum is drawn on (R02.T19): its figure's stretch a ÷ c along its pole; `null`
   * for the sphere of {@link GraticuleBody.radiusM}.
   */
  readonly spheroid: DatumSpheroid | null;
}

/**
 * The spheroid a body's datum is drawn on (plan R02, R02.T19), from its figure (R07's
 * `BodyFigure`) where one is given; `null` for a sphere, a = c, or a body with none. Its axis is
 * the one the image draws the figure about, `figurePole` (its pole, or +z where it has none), so
 * that the datum lies on the drawn bulge (decision-r02-t19-leans, item 2).
 */
export function datumSpheroidOfFigure(figure: BodyFigure | null): DatumSpheroid | null {
  return figure === null ? null : datumSpheroidOf(figure, figurePole(figure));
}

/**
 * The rotation a graticule is drawn in: the body's own, or where it is not modelled one whose pole
 * is the given pole (the orbit normal) and whose prime meridian is fixed, so that it does not turn;
 * on a flattened body (a spheroid; R02.T19) the pole its figure is drawn about instead, so that its
 * equator lies on the figure's (decision-r02-t19-leans, item 2).
 */
export function graticuleRotation(
  body: Pick<GraticuleBody, "rotation" | "unmodelledPole" | "spheroid">,
): Rotation3 {
  if (body.rotation !== null) {
    return body.rotation;
  }
  const pole = body.spheroid?.pole ?? body.unmodelledPole;
  if (pole === null) {
    return IDENTITY_ROTATION;
  }
  const z = normalise(pole);
  const [x, y] = perpendicularPair(z);
  // Columns are the body-fixed axes in the frame, so the rows are their components.
  return rotation3FromRows([vec3(x.x, y.x, z.x), vec3(x.y, y.y, z.y), vec3(x.z, y.z, z.z)]);
}

/** One line of a body's wireframe: its limb, a meridian or a parallel. */
export interface GraticuleLine {
  /** What the line is. */
  readonly kind: "limb" | "meridian" | "parallel";
  /** Whether it is drawn a step heavier: the equator and the prime meridian. */
  readonly major: boolean;
  /** Its visible runs, camera-relative. */
  readonly runs: ReadonlyArray<Polyline>;
}

/** A body's wireframe at its regime. */
export interface BodyWireframe {
  /** The regime the body is drawn in. */
  readonly regime: BodyRegime;
  /** Its apparent diameter, px. */
  readonly diameterPx: number;
  /** Its lines: none as a symbol, the limb alone, or the limb and graticule. */
  readonly lines: ReadonlyArray<GraticuleLine>;
}

/**
 * The unit vector of latitude `latRad` and longitude `lonRad` along the body-fixed axes: the
 * direction of planetocentric (geocentric) latitude, as R10's `graticulePatch` takes it (R02.T19,
 * decision-r02-t19-leans item 1).
 */
function unitAt(latRad: number, lonRad: number): Vec3 {
  const c = Math.cos(latRad);
  return vec3(c * Math.cos(lonRad), c * Math.sin(lonRad), Math.sin(latRad));
}

/** Two unit vectors perpendicular to `axis` and to each other. */
export function perpendicularPair(axis: Vec3): readonly [Vec3, Vec3] {
  const n = normalise(axis);
  const helper = Math.abs(n.z) < 0.9 ? vec3(0, 0, 1) : vec3(1, 0, 0);
  const e1 = normalise(cross(helper, n));
  return [e1, cross(n, e1)];
}

/**
 * The point at `theta` of the body's horizon from the camera: a sphere's circle of tangency, or a
 * spheroid's, the stretched sphere's mapped back (R02.T19), the ellipse in which the cone of lines
 * of sight that graze the spheroid touches it.
 */
function horizonOf(
  body: Pick<GraticuleBody, "centreM" | "radiusM" | "spheroid">,
): (theta: number) => Vec3 {
  const spheroid = body.spheroid;
  const centre = stretched(body.centreM, spheroid);
  const d = norm(centre);
  const toCentre = scale(centre, 1 / d);
  const [u, v] = perpendicularPair(toCentre);
  const along = -(body.radiusM * body.radiusM) / d;
  const across = body.radiusM * Math.sqrt(1 - (body.radiusM / d) ** 2);
  const centreOffset = scale(toCentre, along);
  // Each point from the centre, mapped back from the stretched space before the centre is added,
  // so that it keeps its precision near the body.
  return (theta) =>
    add(
      body.centreM,
      unstretched(
        add(
          centreOffset,
          add(scale(u, across * Math.cos(theta)), scale(v, across * Math.sin(theta))),
        ),
        spheroid,
      ),
    );
}

/**
 * The limb: the body's silhouette from the camera, the true horizon of its sphere or spheroid,
 * subdivided to the sagitta tolerance.
 */
function limb(body: GraticuleBody, camera: ProjectionCamera, viewport: Viewport): Polyline[] {
  return sampleCurve(
    { point: horizonOf(body), windows: [[0, 2 * Math.PI]], initialSpans: 32 },
    camera,
    viewport,
  );
}

/**
 * The body's spheroid with its pole along the view axes, for the sums in the image plane's tangent
 * coordinates; `null` for a sphere.
 */
function viewSpheroidOf(
  body: Pick<GraticuleBody, "spheroid">,
  camera: ProjectionCamera,
): DatumSpheroid | null {
  const spheroid = body.spheroid;
  return spheroid === null
    ? null
    : { pole: toViewAxes(spheroid.pole, camera.orientation), stretch: spheroid.stretch };
}

/**
 * The limb stroked inside the figure (R10.T16.g): each point of the horizon moved `insetPx` into
 * the outline along the outline's screen normal, then lifted back onto the sphere's near surface
 * along that pixel's line of sight, so that it takes the surface's own depth and the body's
 * occluder sphere treats it as any graticule line (`occluderSlopePx` covers its cased half-width).
 *
 * @remarks
 * In the image plane's tangent coordinates (a, b) = (x, y) ÷ −z along the view axes, which every
 * pixel of the view scales alike by the focal length f (`focalPx`), the ray u = (a, b, −1) meets
 * the sphere where F = (u · c)² − (d² − r²)|u|² ≥ 0, on its near nappe where u · c > 0; the outline
 * is F = 0, and its inward normal there is ∇F's direction. So a point `insetPx` ÷ f along it lies
 * `insetPx` px inside the outline, at every place in the field, the outline's curvature radius
 * being far above the inset wherever a datum is drawn over the image. The ray's nearer hit is
 * t = (d² − r²) ÷ (u · c + √F), the root that keeps its precision near the camera. A horizon point
 * behind the camera is left as it is, for `sampleCurve`'s near-plane cut.
 *
 * The outline bounds a convex region, and the curve inside it, parallel to it, is convex too, so
 * its chords lie farther in than their ends: the 0.25 px subdivision never puts a chord nearer the
 * outline than its points.
 *
 * A spheroid's (R02.T19) is the same in its stretched space, where the ray L u meets the sphere of
 * radius a about L c: F = (L u · L c)² − (|L c|² − a²)|L u|², whose gradient in (a, b) is L G's,
 * G = (L u · L c) L c − (|L c|² − a²) L u, L being symmetric; the ray's parameter t is the same in
 * both spaces. Its outline is a conic still, an ellipse whose least radius of curvature is far
 * above the inset wherever a datum is drawn over the image.
 */
function limbInside(
  body: GraticuleBody,
  camera: ProjectionCamera,
  viewport: Viewport,
  insetPx: number,
): Polyline[] {
  const horizon = horizonOf(body);
  const viewSpheroid = viewSpheroidOf(body, camera);
  const centre = stretched(body.centreM, body.spheroid);
  const d = norm(centre);
  const k = (d - body.radiusM) * (d + body.radiusM);
  const c = toViewAxes(centre, camera.orientation);
  const insetTan = insetPx / focalPx(camera, viewport);
  const inside = (theta: number): Vec3 => {
    const onHorizon = horizon(theta);
    const p = toViewAxes(onHorizon, camera.orientation);
    if (!(-p.z > 0)) {
      return onHorizon;
    }
    const a = p.x / -p.z;
    const b = p.y / -p.z;
    const u = stretched(vec3(a, b, -1), viewSpheroid);
    const uc = dot(u, c);
    // ∇F ÷ 2 in (a, b): the outline's inward normal.
    const gradient = stretched(sub(scale(c, uc), scale(u, k)), viewSpheroid);
    const ga = gradient.x;
    const gb = gradient.y;
    const g = Math.hypot(ga, gb);
    if (!(g > 0)) {
      return onHorizon;
    }
    const qa = a + (insetTan * ga) / g;
    const qb = b + (insetTan * gb) / g;
    const q = stretched(vec3(qa, qb, -1), viewSpheroid);
    const qc = dot(q, c);
    const f = Math.max(0, qc * qc - dot(q, q) * k);
    const t = k / (qc + Math.sqrt(f));
    return rotate(camera.orientation, vec3(t * qa, t * qb, -t));
  };
  return sampleCurve(
    { point: inside, windows: [[0, 2 * Math.PI]], initialSpans: 32 },
    camera,
    viewport,
  );
}

/**
 * The facing threshold of a graticule line stroked inside the figure (R10.T16.g), m: the value of
 * −n · c above which a point c + r n of the sphere stands on its near side at least `insetPx` px
 * inside the outline, d cos ψ_e in place of the horizon's r; or `null` where the inset closes the
 * disc.
 *
 * @remarks
 * A point ψ from the sub-camera point is seen at β from the centre's line of sight, where
 * ψ = asin(d sin β ÷ r) − β (the triangle of the camera, the centre and the point, obtuse at the
 * point on the near side), β rising with ψ to α = asin(r ÷ d) at the horizon. A pixel stands at
 * least f sec θ_min times its angle from the outline's nearest line of sight away from it on the
 * screen: the straight line between them is a great circle inside the body's cap, every point of
 * which lies at least θ_min = max(0, θ_c − α) off the view axis (θ_c the centre's), where the
 * view's scale is at least f sec θ in every direction (its tangential scale; its radial one is
 * f sec² θ). So every point whose β is at most α − ε, ε = insetPx cos θ_min ÷ f, stands at least
 * `insetPx` inside. The cut is safe, not exact: on the view axis it is exact for a small body, but
 * off it it stands up to sec² θ ÷ sec θ_min of the inset inside, about 1.4 of it at a corner of a
 * 60° field and 5 in a 120° field, where a line would end short of the limb's casing; so
 * `windowsInside` moves each cut out from it to the exact inset.
 *
 * A spheroid's (R02.T19) is the stretched sphere's, of radius a about L c, with its angular inset
 * s times as large: the stretch turns lines of sight apart by at most s (`angularDistortion`), and
 * maps the grazing lines of sight to the spheroid's, so a line of sight s ε inside the stretched
 * cone stands at least ε inside the spheroid's. θ_min is taken from the sphere of radius a about
 * c, which holds the spheroid, its least angle off the axis 0 where the camera stands inside it.
 */
function insideThresholdM(
  body: GraticuleBody,
  camera: ProjectionCamera,
  viewport: Viewport,
  insetPx: number,
): number | null {
  const spheroid = body.spheroid;
  const centre = stretched(body.centreM, spheroid);
  const d = norm(centre);
  const alpha = Math.asin(body.radiusM / d);
  const distanceM = norm(body.centreM);
  const forward = -toViewAxes(body.centreM, camera.orientation).z / distanceM;
  // The angular radius of the sphere that holds the body: the sphere's own, or about a spheroid
  // the sphere of its equatorial radius, a right angle from inside it.
  const holdingRad =
    spheroid === null
      ? alpha
      : distanceM > body.radiusM
        ? Math.asin(body.radiusM / distanceM)
        : Math.PI / 2;
  const thetaMin = Math.max(0, Math.acos(Math.min(1, Math.max(-1, forward))) - holdingRad);
  if (!(thetaMin < Math.PI / 2)) {
    return null;
  }
  const beta =
    alpha -
    (angularDistortion(spheroid) * insetPx * Math.cos(thetaMin)) / focalPx(camera, viewport);
  if (!(beta > 0)) {
    return null;
  }
  const psi = Math.asin(Math.min(1, (d / body.radiusM) * Math.sin(beta))) - beta;
  return d * Math.cos(psi);
}

/**
 * A sphere's outline on a view as a conic in the image plane's tangent coordinates x = (a, b):
 * F(x) = xᵀAx + 2gᵀx + h = (u · c)² − (d² − r²)|u|², u = (a, b, −1), c the centre along the view
 * axes, zero on the outline and positive inside it; a spheroid's (R02.T19) the same of L u and L c
 * in its stretched space, d = |L c| and r its equatorial radius.
 */
interface OutlineConic {
  readonly a11: number;
  readonly a12: number;
  readonly a22: number;
  readonly g1: number;
  readonly g2: number;
  readonly h: number;
}

function outlineConicOf(body: GraticuleBody, camera: ProjectionCamera): OutlineConic {
  const viewSpheroid = viewSpheroidOf(body, camera);
  const centre = stretched(body.centreM, body.spheroid);
  const d = norm(centre);
  const k = (d - body.radiusM) * (d + body.radiusM);
  const c = toViewAxes(centre, camera.orientation);
  if (viewSpheroid === null) {
    return {
      a11: c.x * c.x - k,
      a12: c.x * c.y,
      a22: c.y * c.y - k,
      g1: -c.z * c.x,
      g2: -c.z * c.y,
      h: c.z * c.z - k,
    };
  }
  // L u = a l₁ + b l₂ − l₃, lᵢ = L eᵢ, so L u · L c = a m₁ + b m₂ − m₃ with mᵢ = lᵢ · L c.
  const l1 = stretched(vec3(1, 0, 0), viewSpheroid);
  const l2 = stretched(vec3(0, 1, 0), viewSpheroid);
  const l3 = stretched(vec3(0, 0, 1), viewSpheroid);
  const [m1, m2, m3] = [dot(l1, c), dot(l2, c), dot(l3, c)];
  return {
    a11: m1 * m1 - k * dot(l1, l1),
    a12: m1 * m2 - k * dot(l1, l2),
    a22: m2 * m2 - k * dot(l2, l2),
    g1: -m3 * m1 + k * dot(l1, l3),
    g2: -m3 * m2 + k * dot(l2, l3),
    h: m3 * m3 - k * dot(l3, l3),
  };
}

/** How many Newton steps {@link outlineInsetPx} takes at most. */
const OUTLINE_NEWTON_STEPS = 32;

/**
 * How far inside a sphere's outline the point `p` is seen, device px: the distance of its pixel
 * from the outline's nearest point, negative outside the outline or behind the camera.
 *
 * @remarks
 * The nearest point x of the conic F = 0 to the pixel q satisfies x − q = μ(Ax + g), so
 * x(μ) = (I − μA)⁻¹(q + μg), and μ is the root of F(x(μ)) near 0, found by Newton's method from
 * its first-order value −F(q) ÷ 2|Aq + g|²; dF/dμ = 2 G · (I − μA)⁻¹G, G = Ax + g. Exact, and
 * unique, for a pixel nearer the outline than its least radius of curvature, f tan α (the conic's
 * semi-latus rectum, at every place in the field), as every pixel it is asked of near a cut is: a
 * datum's reach is at most 1 ÷ 6.8 of the apparent radius over the image
 * ({@link DATUM_OVER_IMAGE_RADIUS_PER_REACH}). A spheroid's outline (R02.T19) is an ellipse of
 * semi-axes A ≥ B, its least radius of curvature B² ÷ A, which the reach, at most B ÷ 6.8, stays
 * under wherever B ÷ A is above 0.15, and every flattening the wire allows keeps it above 0.8.
 */
function outlineInsetPx(
  conic: OutlineConic,
  p: Vec3,
  camera: ProjectionCamera,
  viewport: Viewport,
): number {
  const v = toViewAxes(p, camera.orientation);
  if (!(-v.z > 0)) {
    return Number.NEGATIVE_INFINITY;
  }
  const qa = v.x / -v.z;
  const qb = v.y / -v.z;
  const { a11, a12, a22, g1, g2, h } = conic;
  const valueAt = (x: number, y: number): number =>
    a11 * x * x + 2 * a12 * x * y + a22 * y * y + 2 * g1 * x + 2 * g2 * y + h;
  const fq = valueAt(qa, qb);
  const ga0 = a11 * qa + a12 * qb + g1;
  const gb0 = a12 * qa + a22 * qb + g2;
  let mu = -fq / (2 * (ga0 * ga0 + gb0 * gb0));
  let xa = qa;
  let xb = qb;
  for (let step = 0; step < OUTLINE_NEWTON_STEPS; step += 1) {
    const m11 = 1 - mu * a11;
    const m12 = -mu * a12;
    const m22 = 1 - mu * a22;
    const det = m11 * m22 - m12 * m12;
    const ra = qa + mu * g1;
    const rb = qb + mu * g2;
    xa = (m22 * ra - m12 * rb) / det;
    xb = (m11 * rb - m12 * ra) / det;
    const ga = a11 * xa + a12 * xb + g1;
    const gb = a12 * xa + a22 * xb + g2;
    const da = (m22 * ga - m12 * gb) / det;
    const db = (m11 * gb - m12 * ga) / det;
    const slope = 2 * (ga * da + gb * db);
    const next = mu - valueAt(xa, xb) / slope;
    if (!Number.isFinite(next) || Math.abs(next - mu) <= 1e-15 * Math.abs(mu)) {
      break;
    }
    mu = next;
  }
  const distancePx = Math.hypot(xa - qa, xb - qb) * focalPx(camera, viewport);
  return fq > 0 ? distancePx : -distancePx;
}

/** How many even steps a run's end is sought in, out from its safe end, before it is bisected. */
const CUT_STEPS = 8;

/**
 * How many bisections place a run's end at its reach inside the outline: 24, a step's 2⁻²⁴, under
 * 10⁻⁵ px on a body over the image.
 */
const CUT_BISECTIONS = 24;

/**
 * A line's windows inside the figure (R10.T16.g): its facing windows on the horizon's threshold
 * (`facing`) cut where its points come within `insetPx` of the outline, on their exact distance
 * inside it (`insetAt`). Each safe window (`safe`, `insideThresholdM`'s, within a facing one, its
 * points all far enough inside) is moved out at each end towards the facing window's end, or the
 * next safe window's: in even steps to the first point too near the outline, then by bisection
 * between it and the last point far enough inside, so that a run ends at its reach inside the
 * outline. Windows that meet are joined. A facing window with no safe one keeps the stretch about
 * the farthest inside of its middle, its ends and its even steps, or none, so that a window cut by a
 * parallel's λ = 0, or a meridian's whose middle runs near the limb, keeps the far stretch it has
 * (R02.T19's finding, ruled a fix of R10.T16.g's: `decision-r02-t19-leans.md` item 7; the middle
 * alone dropped it).
 *
 * @param insetAt - How far inside the outline the line's point at a parameter is seen, px.
 */
function windowsInside(
  facing: ReadonlyArray<ParameterWindow>,
  safe: ReadonlyArray<ParameterWindow>,
  insetAt: (u: number) => number,
  insetPx: number,
): ParameterWindow[] {
  const far = (u: number): boolean => insetAt(u) >= insetPx;
  // How far from `inner`, far enough inside, towards `outer` the line stays far enough inside.
  const endFrom = (outer: number, inner: number): number => {
    let ok = inner;
    for (let i = 1; i <= CUT_STEPS; i += 1) {
      const u = inner + ((outer - inner) * i) / CUT_STEPS;
      if (!far(u)) {
        let near = u;
        for (let j = 0; j < CUT_BISECTIONS; j += 1) {
          const mid = (near + ok) / 2;
          if (far(mid)) {
            ok = mid;
          } else {
            near = mid;
          }
        }
        return ok;
      }
      ok = u;
    }
    return outer;
  };
  // Where in a facing window with no safe one the line stands far enough inside: its middle, as
  // R10.T16.g took it, or else the farthest inside of its ends and even steps.
  const farFrom = (lo: number, hi: number): number | null => {
    const middle = (lo + hi) / 2;
    if (far(middle)) {
      return middle;
    }
    let best: number | null = null;
    let bestPx = insetPx;
    for (let i = 0; i <= CUT_STEPS; i += 1) {
      const u = lo + ((hi - lo) * i) / CUT_STEPS;
      const atPx = insetAt(u);
      if (atPx >= bestPx) {
        best = u;
        bestPx = atPx;
      }
    }
    return best;
  };
  const windows: ParameterWindow[] = [];
  for (const [lo, hi] of facing) {
    const inner = safe
      .filter(([sLo, sHi]) => sLo >= lo && sHi <= hi)
      .toSorted((x, y) => x[0] - y[0]);
    if (inner.length === 0) {
      const from = farFrom(lo, hi);
      if (from !== null) {
        windows.push([endFrom(lo, from), endFrom(hi, from)]);
      }
      continue;
    }
    inner.forEach(([sLo, sHi], i) => {
      windows.push([endFrom(inner[i - 1]?.[1] ?? lo, sLo), endFrom(inner[i + 1]?.[0] ?? hi, sHi)]);
    });
  }
  const joined: Array<[number, number]> = [];
  for (const [lo, hi] of windows.toSorted((x, y) => x[0] - y[0])) {
    const last = joined.at(-1);
    if (last !== undefined && lo <= last[1]) {
      last[1] = Math.max(last[1], hi);
    } else if (hi > lo) {
      joined.push([lo, hi]);
    }
  }
  return joined;
}

/**
 * A body's wireframe (plan R02, Design note 13): by its apparent size, nothing (its symbol is the
 * symbology's), its limb circle, or its limb and a graticule at 15° (30° below 64 CSS px, and at
 * the low setting), the equator and prime meridian a step heavier; the thresholds but the symbol's
 * at the line scale, `strokeScale` device px for each CSS px (R07.T16.g).
 *
 * @remarks
 * Every point is generated in `f64`, camera-relative, as c + r · R · n, and subdivided until each
 * chord's sagitta is under 0.25 px. The graticule's far hemisphere is removed analytically: a
 * point is drawn only where its outward normal faces the camera (−n · c > r), which is exact for a
 * sphere and needs no depth; each line's facing arc is found in closed form (`cosineWindows`), so a
 * short visible arc near the limb is never missed. The graticule turns with the body through its
 * rotation; with none it is drawn about the orbit normal and does not turn (Design note 14).
 *
 * Over the photorealistic image (`inside`; plan R10, R10.T16.g) every line is stroked inside the
 * figure: the limb its thin reach inside the outline, its casing's coverage ending on it
 * (`limbInside`), and each meridian and parallel cut where it comes within its own reach of the
 * outline: inside a safe cut by the same closed form with a higher facing threshold
 * (`insideThresholdM`), then moved out to the exact reach by bisection on the exact distance inside
 * the outline (`windowsInside`, `outlineInsetPx`), so that the runs end under the limb's casing
 * anywhere in any field. A body with an apparent radius under
 * {@link DATUM_OVER_IMAGE_RADIUS_PER_REACH} limb reaches has no lines there ({@link datumOverImage}).
 * The regime is the body's at its size in either case.
 *
 * A flattened body (`body.spheroid`; plan R02, R02.T19) has its datum on its spheroid in both
 * styles, built on the sphere of radius a its stretch a ÷ c along the pole makes of it, with every
 * point mapped back, c + L⁻¹(a · R · n), so that it lies on the figure: the limb is the spheroid's
 * horizon from the camera, the graticule is faced by the spheroid's normal (−n · L c > a, which the
 * stretch keeps exact), and over the image its outline is the spheroid's conic and its cut on its
 * least apparent radius ({@link outlineLeastDiameterPx}). Its parallels stand at planetocentric
 * (geocentric) latitudes φ, as R10's `graticulePatch` takes them, at the reduced latitude
 * atan((a ÷ c) tan φ) on the stretched sphere; its meridians are a sphere's. Its regime and step stay
 * the equatorial sphere's, as the image sizes it. A sphere takes the sphere's own sums, as before.
 *
 * @param strokeScale - The device px drawn for each CSS px of a line (`ViewStrokes.strokeScale`).
 * @param lowSetting - The wireframe's low setting, which draws graticules at 30° only.
 * @param inside - How the datum is stroked inside the figure over the photorealistic image, or
 *   `null` for the wireframe's, its limb centred on the outline.
 */
export function graticule(
  body: GraticuleBody,
  camera: ProjectionCamera,
  viewport: Viewport,
  strokeScale: number,
  lowSetting = false,
  inside: DatumInside | null = null,
): BodyWireframe {
  const diameterPx = angularDiameterPx(body.centreM, body.radiusM, camera, viewport);
  const regime = bodyRegime(diameterPx, strokeScale);
  const spheroid = body.spheroid;
  if (
    regime === "symbol" ||
    (inside !== null &&
      !datumOverImage(
        spheroid === null ? diameterPx : outlineLeastDiameterPx(body, camera, viewport),
        inside.reachPx.thin,
      ))
  ) {
    return { regime, diameterPx, lines: [] };
  }
  const lines: GraticuleLine[] = [
    {
      kind: "limb",
      major: false,
      runs:
        inside === null
          ? limb(body, camera, viewport)
          : limbInside(body, camera, viewport, inside.reachPx.thin + DATUM_INSIDE_MARGIN_PX),
    },
  ];
  if (regime === "limb") {
    return { regime, diameterPx, lines };
  }
  const rotation = graticuleRotation(body);
  const onSphere = (latRad: number, lonRad: number): Vec3 =>
    rotateToBody(rotation, unitAt(latRad, lonRad));
  // The body's axes turned into the frame, and each one's facing component −axis · c, on a
  // spheroid in its stretched space, −axis · L c, where it is the sphere of radius a (R02.T19).
  const xAxis = rotateToBody(rotation, vec3(1, 0, 0));
  const yAxis = rotateToBody(rotation, vec3(0, 1, 0));
  const zAxis = rotateToBody(rotation, vec3(0, 0, 1));
  const facingCentre = stretched(body.centreM, spheroid);
  const [fx, fy, fz] = [xAxis, yAxis, zAxis].map((axis) => -dot(axis, facingCentre));
  const facingX = fx ?? 0;
  const facingY = fy ?? 0;
  const facingZ = fz ?? 0;
  // c + L⁻¹(r n): on a spheroid the stretched sphere's point mapped back, from the centre.
  const pointOf = (n: Vec3): Vec3 =>
    add(body.centreM, unstretched(scale(n, body.radiusM), spheroid));
  // The facing threshold of a line, thin or heavy, m: the horizon's r, or inside the figure each
  // weight's own, `null` where its inset closes the disc.
  const facingAboveM = (major: boolean): number | null =>
    inside === null
      ? body.radiusM
      : insideThresholdM(
          body,
          camera,
          viewport,
          (major ? inside.reachPx.heavy : inside.reachPx.thin) + DATUM_INSIDE_MARGIN_PX,
        );
  const thresholdsM = { thin: facingAboveM(false), heavy: facingAboveM(true) };
  const conic = inside === null ? null : outlineConicOf(body, camera);
  /**
   * A line's windows of α cos u + β sin u > threshold − `offsetM` over [`uMin`, `uMax`]: its facing
   * arcs (`cosineWindows`), or inside the figure those arcs cut at the line's reach inside the
   * outline (`windowsInside`), from the safe cut of its weight's threshold out.
   */
  const lineWindows = (
    alpha: number,
    beta: number,
    offsetM: number,
    uMin: number,
    uMax: number,
    major: boolean,
    pointAt: (u: number) => Vec3,
  ): ParameterWindow[] => {
    const facing = cosineWindows(alpha, beta, body.radiusM - offsetM, uMin, uMax);
    const thresholdM = major ? thresholdsM.heavy : thresholdsM.thin;
    if (inside === null || conic === null) {
      return facing;
    }
    if (thresholdM === null) {
      return [];
    }
    return windowsInside(
      facing,
      cosineWindows(alpha, beta, thresholdM - offsetM, uMin, uMax),
      (u) => outlineInsetPx(conic, pointAt(u), camera, viewport),
      (major ? inside.reachPx.heavy : inside.reachPx.thin) + DATUM_INSIDE_MARGIN_PX,
    );
  };
  const stepDeg = graticuleStepDeg(diameterPx, strokeScale, lowSetting);
  const stepRad = (stepDeg * Math.PI) / 180;
  const count = Math.round(360 / stepDeg);
  for (let k = 0; k < count; k += 1) {
    const lonRad = k * stepRad;
    const major = k === 0;
    const point = (lat: number): Vec3 => pointOf(onSphere(lat, lonRad));
    lines.push({
      kind: "meridian",
      major,
      runs: sampleCurve(
        {
          point,
          // n = cos φ (cos λ x + sin λ y) + sin φ z faces the camera where −n · c > r, on a
          // spheroid −n · L c > a on its stretched sphere, φ the reduced latitude there.
          windows: lineWindows(
            Math.cos(lonRad) * facingX + Math.sin(lonRad) * facingY,
            facingZ,
            0,
            -Math.PI / 2,
            Math.PI / 2,
            major,
            point,
          ),
          initialSpans: 12,
        },
        camera,
        viewport,
      ),
    });
  }
  for (let k = 1; k < count / 2; k += 1) {
    const latRad = -Math.PI / 2 + k * stepRad;
    const major = Math.abs(latRad) < 1e-12;
    // The parallel's latitude on the stretched sphere: its own on a sphere, and on a spheroid the
    // parametric β of its planetocentric φ, tan β = (a ÷ c) tan φ (R02.T19).
    const onRad =
      spheroid === null
        ? latRad
        : Math.atan2(spheroid.stretch * Math.sin(latRad), Math.cos(latRad));
    const point = (lon: number): Vec3 => pointOf(onSphere(onRad, lon));
    lines.push({
      kind: "parallel",
      major,
      runs: sampleCurve(
        {
          point,
          windows: lineWindows(
            Math.cos(onRad) * facingX,
            Math.cos(onRad) * facingY,
            Math.sin(onRad) * facingZ,
            0,
            2 * Math.PI,
            major,
            point,
          ),
          initialSpans: 24,
        },
        camera,
        viewport,
      ),
    });
  }
  return { regime, diameterPx, lines };
}

/** A body's rings as the wireframe draws them. */
export interface RingGeometry {
  /** The ringed body's centre from the camera, m. */
  readonly centreM: Vec3;
  /** The inner edge's radius, m. */
  readonly innerRadiusM: number;
  /** The outer edge's radius, m. */
  readonly outerRadiusM: number;
  /** The ring plane's unit normal. */
  readonly normal: Vec3;
}

/** The angle between a ring's radial ticks, degrees: 10, as the orbit map draws them. */
export const RING_TICK_STEP_DEG = 10;

/**
 * A ring's ellipses as the camera sees them: its inner and outer edges, subdivided to the sagitta
 * tolerance, and a radial tick every 10° from one to the other (plan R02, Design note 13).
 */
export function ringEllipse(
  ring: RingGeometry,
  camera: ProjectionCamera,
  viewport: Viewport,
): ReadonlyArray<Polyline> {
  const [e1, e2] = perpendicularPair(ring.normal);
  const at = (radiusM: number, theta: number): Vec3 =>
    add(
      ring.centreM,
      add(scale(e1, radiusM * Math.cos(theta)), scale(e2, radiusM * Math.sin(theta))),
    );
  const edge = (radiusM: number): Polyline[] =>
    sampleCurve(
      {
        point: (t) => at(radiusM, t),
        windows: [[0, 2 * Math.PI]],
        initialSpans: 36,
      },
      camera,
      viewport,
    );
  const ticks: Polyline[] = [];
  for (let deg = 0; deg < 360; deg += RING_TICK_STEP_DEG) {
    const theta = (deg * Math.PI) / 180;
    const inner = at(ring.innerRadiusM, theta);
    const outer = at(ring.outerRadiusM, theta);
    // A straight tick, sampled only so that a part behind the near plane is cut away.
    ticks.push(
      ...sampleCurve(
        {
          point: (t) => add(inner, scale(sub(outer, inner), t)),
          windows: [[0, 1]],
          initialSpans: 1,
        },
        camera,
        viewport,
      ),
    );
  }
  return [...edge(ring.innerRadiusM), ...edge(ring.outerRadiusM), ...ticks];
}
